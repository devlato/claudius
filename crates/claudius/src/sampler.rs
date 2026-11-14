//! The auto-sampler: the phrases of a recording that sound most like a voice, cut on the bars it plays (each made
//! as long as another, `Analysis::cut`), cleaned of its drums and of what plays to the sides, and placed in the song
//! where they fit. Each becomes a loop of
//! its own (`vox1`, `vox2` ...) that the renderer warps to the song's tempo and moves to its key: the phrase, then
//! the bars of silence before it comes round again, in the sections that suit it, from the bar on which its notes
//! agree best with the song's chords, mixed as a vocal.

use std::path::Path;

use crate::analysis::song::Analysis;
use crate::analysis::{chroma, melody, song as analysis};
use crate::audio::Stereo;
use crate::config::SRF;
use crate::cuts;
use crate::dsp::fft;
use crate::dsp::filter::{self, BiquadKind};
use crate::fx::{Effect, EqBand};
use crate::kinds::{self, Kind};
use crate::model::*;
use crate::stems::Stems;

/// how many phrases one call samples at most
pub const MOST: usize = 4;

/// what the ears listen for when nothing else is asked
pub const DEFAULT_FIND: &str = "a person singing";

/// How a recording goes into the song, as producers sample one: its phrases as loops that come round, a passage of it
/// as a verse or a chorus, its voice chopped into an instrument, or a short hit of it struck as a drum is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum How {
    /// its best sung phrases, a bar or two each, each a loop of its own that comes round where its notes agree with
    /// the song's chords (`vox1`, `vox2` ...)
    #[default]
    Phrases,
    /// a passage of its best sung bars, 4 to 16, played through in the drops and hooks (or the sections asked for) as
    /// their verse or chorus, the song's chords there following it (`passage`)
    Passage,
    /// its best phrase cut in eighths, the slices that hold the voice played in a new rhythm as an instrument of their
    /// own (`chops`)
    Chops,
    /// one short hit of it, a syllable or a stab, struck on a few beats as a drum lane of its own (`vox_stab`)
    Stabs,
}

impl How {
    pub const ALL: [How; 4] = [How::Phrases, How::Passage, How::Chops, How::Stabs];

    pub fn name(self) -> &'static str {
        match self {
            How::Phrases => "phrases",
            How::Passage => "passage",
            How::Chops => "chops",
            How::Stabs => "stabs",
        }
    }

    pub fn parse(s: &str) -> Result<How, String> {
        let s = s.trim().to_lowercase();
        How::ALL.into_iter().find(|h| h.name() == s).ok_or_else(|| {
            let names: Vec<&str> = How::ALL.iter().map(|h| h.name()).collect();
            format!("no way '{s}' to sample a recording (ways: {})", names.join(", "))
        })
    }

    /// the lengths it takes in bars, and what it calls what it cuts
    fn lengths(self) -> (&'static [u32], &'static str) {
        match self {
            How::Passage => (&[4, 8, 16], "a passage is 4, 8 or 16 bars"),
            _ => (&[1, 2, 4], "a phrase is 1, 2 or 4 bars"),
        }
    }
}

/// How often a phrase sampled into the song comes round in the sections it plays in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Every {
    /// once, from the start of each section
    Once,
    /// once every so many bars (2, 4, 8 or 16; as many as the phrase lasts, at least), silent between
    Bars(u32),
    /// all through each section, round and round
    Through,
}

impl Every {
    /// `once`, `2`, `4`, `8`, `16` or `through`
    pub fn parse(s: &str) -> Result<Every, String> {
        match s.trim().to_lowercase().as_str() {
            "once" => Ok(Every::Once),
            "through" => Ok(Every::Through),
            n => n
                .parse::<u32>()
                .ok()
                .filter(|n| [2, 4, 8, 16].contains(n))
                .map(Every::Bars)
                .ok_or_else(|| format!("no '{s}' for how often a phrase plays (once, 2, 4, 8, 16 or through)")),
        }
    }
}

/// what to sample, and where to
#[derive(Clone, Debug)]
pub struct Options {
    /// how many phrases, 1 to `MOST` (the other ways take one passage, one phrase to chop or one hit)
    pub count: usize,
    /// how long each phrase is, in bars (1, 2 or 4); none: whichever of 1 and 2 sounds best. A passage is 4, 8 or 16
    /// (none: 8 where a section has room, else 4), a phrase to chop 1, 2 or 4 (none: 2); a stab is a hit, of no bars.
    pub bars: Option<u32>,
    /// what the ears listen for (`DEFAULT_FIND` when not said)
    pub find: Option<String>,
    /// the sections to place the phrases in, by name or kind; empty: the sections that suit each phrase
    pub sections: Vec<String>,
    /// how it goes into the song
    pub how: How,
    /// how often each phrase comes round in its sections; none: as the song's genre and what its sections play suit
    /// (`period`)
    pub every: Option<Every>,
    /// the kinds of piece `pieces` offers; empty: every kind the recording holds
    pub kinds: Vec<Kind>,
    /// the pieces the take before offered, each its kind and where it is in the recording, in seconds (`shown`):
    /// another take draws others where there are any
    pub shown: Vec<(Kind, f64, f64)>,
    /// the kinds of piece the song's genre samples (`leaning`), which `pieces` offers more of
    pub lean: Vec<Kind>,
}

impl Default for Options {
    fn default() -> Self {
        let (how, kinds, shown, lean) = (How::Phrases, Vec::new(), Vec::new(), Vec::new());
        Options { count: 2, bars: None, find: None, sections: Vec::new(), how, every: None, kinds, shown, lean }
    }
}

/// a phrase of the recording worth sampling
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// where it starts in the recording, in seconds, the bar that is (counting from 0, the bars before the recording's
    /// first below it, as `bar_time` counts them), and how many bars it lasts
    pub start: f64,
    pub first: i64,
    pub bars: u32,
    /// the ears' z for what they listened for, when they listened
    pub z: Option<f64>,
    /// the share of it that holds a pitch in a voice's range, and of its sound that is in the middle
    pub sung: f64,
    pub centred: f64,
    pub score: f64,
    /// where its singing starts and ends in the recording, in seconds, when it is a phrase sung whole (`sung_whole`): its
    /// bars are silent outside it; none: all of its bars
    pub within: Option<(f64, f64)>,
}

/// what the sampler did: the tracks it added, and a line for each saying where it came from and went
#[derive(Debug, Default)]
pub struct Sampled {
    pub tracks: Vec<String>,
    pub lines: Vec<String>,
    pub how: How,
    /// the kind of each piece kept, when pieces were kept (`keep`): one for the lanes of a piece chopped onto pads
    pub kinds: Vec<Kind>,
}

impl Sampled {
    /// what came of sampling the recording `name`, in a line, then its lines: "sampled 2 phrases of song.mp3 into the
    /// song: vox1, vox2", so that whoever reads only the first knows what changed
    pub fn said(&self, name: &str) -> String {
        let n = self.tracks.len();
        let what = match self.how {
            How::Phrases if self.kinds.iter().any(|k| *k != Kind::Phrase) => {
                format!("sampled {} of {name} into the song", Kind::counted_all(&self.kinds))
            }
            How::Phrases => format!("sampled {n} phrase{} of {name} into the song", if n == 1 { "" } else { "s" }),
            How::Passage => format!("sampled a passage of {name} into the song"),
            How::Chops => format!("chopped {name} into the song"),
            How::Stabs => format!("sampled a stab of {name} into the song"),
        };
        format!("{what}: {}\n{}", self.tracks.join(", "), self.lines.join("\n"))
    }
}

/// Samples the best vocal phrases of `src`, a recording called `name`, into the song: cut, cleaned, stored in `dir`
/// with the song's takes, placed and mixed. The ears, when given, listen for `opt.find`.
pub fn sample(
    bp: &mut Blueprint,
    src: &Stereo,
    name: &str,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    checked(bp, src, opt)?;
    let stems = crate::stems::of(src, &mut |_| {});
    let a = analysis::analyze_with(src, stems.as_deref(), None);
    sample_from(bp, &Heard { src, stems: stems.as_deref(), a: &a, name }, ears, opt, dir)
}

/// whether the song has somewhere to put phrases of the recording `src` as `opt` asks, which is checked before the
/// recording is listened to
fn checked(bp: &Blueprint, src: &Stereo, opt: &Options) -> Result<(), String> {
    if bp.form.is_empty() {
        return Err("the song has no sections to put the phrases in".into());
    }
    let (lengths, said) = opt.how.lengths();
    if let Some(b) = opt.bars.filter(|b| !lengths.contains(b) && opt.how != How::Stabs) {
        return Err(format!("{said}, not {b}"));
    }
    if (src.len() as f64 / SRF) < 4.0 {
        return Err("the recording is too short to sample (4 s at least)".into());
    }
    groups(bp, &opt.sections).map(|_| ())
}

/// `sample`, of a recording already heard: its stems, when they are apart, and its analysis
pub(crate) fn sample_from(
    bp: &mut Blueprint,
    h: &Heard,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    checked(bp, h.src, opt)?;
    match opt.how {
        How::Phrases => {}
        How::Passage => return passage(bp, h, ears, opt, dir),
        How::Chops => return chops(bp, h, ears, opt, dir),
        How::Stabs => return stabs(bp, h, ears, opt, dir),
    }
    let count = opt.count.clamp(1, MOST);
    let phrases = find(h.src, h.stems, h.a, ears, opt, count);
    if phrases.is_empty() {
        return Err(NO_VOICE.into());
    }
    let plain: Vec<(Phrase, Vec<Effect>)> = phrases.into_iter().map(|p| (p, Vec::new())).collect();
    place(bp, h, &plain, opt, dir)
}

/// The sections of `groups` a sample `bars` long goes in: those of the first group (the drops and hooks first, or the
/// sections asked for) with a section of `bars` bars at least, and only those; none when no section has room.
fn room(bp: &Blueprint, groups: &[(String, Vec<usize>)], bars: u32) -> Option<(String, Vec<usize>)> {
    groups.iter().find_map(|(label, secs)| {
        let secs: Vec<usize> = secs.iter().copied().filter(|&si| bp.form[si].bars >= bars).collect();
        (!secs.is_empty()).then(|| (label.clone(), secs))
    })
}

/// a name for a new track of the song: `base`, else `base2`, `base3` ...
fn fresh(bp: &Blueprint, base: &str) -> String {
    let names = bp.layer_names();
    std::iter::once(base.to_string())
        .chain((2..).map(|k| format!("{base}{k}")))
        .find(|n| !names.contains(n))
        .unwrap_or_default()
}

/// semitones in words: "in its own key", "3 semitones up into A minor"
fn moved(t: i32, key: &crate::theory::scale::Key) -> String {
    match t {
        0 => "in its own key".to_string(),
        t => format!("{} {} into {}", t.abs(), if t > 0 { "semitones up" } else { "semitones down" }, key.name()),
    }
}

/// what the ears made of a span, in words, when they listened
fn heard_text(z: Option<f64>, opt: &Options) -> String {
    match z {
        Some(z) => format!("the ears {z:+.1} for '{}', ", opt.find.as_deref().unwrap_or(DEFAULT_FIND)),
        None => String::new(),
    }
}

/// A passage of the recording `h` as a part of the song: its best sung bars, 4 to 16 (`opt.bars`; 8 where a section
/// has room for them, else 4), the better where one of its own sections starts, from its vocals when they are apart
/// and else all of it as it plays (its lows out, for the song's bass and kick). It plays through from the first bar of
/// the sections it goes in (the drops and hooks, unless asked), where the song's chords follow the passage's.
fn passage(
    bp: &mut Blueprint,
    h: &Heard,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    let (a, name) = (h.a, h.name);
    let groups = groups(bp, &opt.sections)?;
    let long = h.src.len() as f64 / SRF + 1e-6;
    let held = (1..=64).take_while(|&b| a.bar_start(b) <= long).count() as u32;
    let short = |b: u32| format!("the recording holds {held} bars, fewer than a passage of {b}");
    let (len, (label, secs)) = match opt.bars {
        Some(b) if b > held => return Err(format!("{}: ask for a shorter one", short(b))),
        Some(b) => (
            b,
            room(bp, &groups, b)
                .ok_or_else(|| format!("no section asked for has room for a passage of {b} bars: try a shorter one"))?,
        ),
        None if held < 4 => return Err(format!("{}: sample its phrases instead", short(4))),
        // the drops and hooks before the rest, 8 bars in them before 4
        None => groups
            .iter()
            .find_map(|g| {
                [8, 4]
                    .into_iter()
                    .filter(|&b| b <= held)
                    .find_map(|b| room(bp, std::slice::from_ref(g), b).map(|r| (b, r)))
            })
            .ok_or("no section asked for has room for a passage of 4 bars")?,
    };
    let starts: Vec<i64> = a.sections.iter().map(|s| s.start_bar as i64).collect();
    let lift = |p: &Phrase| {
        p.score + if starts.contains(&p.first) { 0.3 } else { 0.1 * (p.first.rem_euclid(4) == 0) as u8 as f64 }
    };
    let best = candidates(h.src, h.stems, a, ears, &Options { bars: Some(len), ..opt.clone() })
        .into_iter()
        .max_by(|x, y| lift(x).total_cmp(&lift(y)))
        .ok_or(NO_VOICE)?;
    let (cut, apart) = match h.stems {
        Some(s) => (voice(&cut_bars(a, &s.vocals, best.first, len as usize)), true),
        None => (whole(&cut_bars(a, h.src, best.first, len as usize)), false),
    };
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let file = crate::take::store(&cut, dir).map_err(|e| format!("cannot keep the passage: {e:#}"))?;
    let track = fresh(bp, "passage");
    let from_bar = best.first + 1;
    bp.loops.push(LoopPart {
        name: track.clone(),
        role: if apart { "vocal" } else { "sample" }.into(),
        sound: Sound::Loop {
            path: file.to_string_lossy().into_owned(),
            bpm: a.cut_bpm(),
            beats: len as f64 * 4.0,
            key: Some(a.key.tonic),
            tonal: true,
            label: format!("{name}, {}", bars_text(from_bar, len)),
        },
        transpose,
        chop: Vec::new(),
        slice_beats: 0.0,
        retime: false,
        mix: if apart { vocal_mix() } else { record_mix() },
        from: None,
        sampled: true,
    });
    for &si in &secs {
        bp.form[si].layers.push(track.clone());
    }
    // the chords heard in a voice alone (an a cappella, whose other stems are all but silent) are only a guess at its
    // harmony: the song keeps its own where the recording plays none
    let (s0, s1) = ((bar_time(a, best.first) * SRF) as usize, (bar_time(a, best.first + len as i64) * SRF) as usize);
    let backed = h.stems.is_none_or(|s| rms(&s.backing(), s0, s1 - s0) > 0.25 * rms(&s.vocals, s0, s1 - s0));
    let played = backed
        && (best.first..best.first + len as i64).all(|k| {
            a.sections.iter().any(|s| s.harmony && (s.start_bar as i64..(s.start_bar + s.bars) as i64).contains(&k))
        });
    let chords =
        played.then(|| passage_chords(a, best.first, len, transpose, &bp.key, bp.harmony.bars_per_chord)).flatten();
    let chords_said = match chords {
        Some(chords) => {
            let names: Vec<&String> = bp.harmony.progressions.keys().collect();
            let prog = std::iter::once("sampled".to_string())
                .chain((2..).map(|k| format!("sampled{k}")))
                .find(|n| !names.contains(&n))
                .unwrap_or_default();
            let said = chords.join(" ");
            bp.harmony.progressions.insert(prog.clone(), chords);
            for &si in &secs {
                bp.form[si].prog = prog.clone();
            }
            format!("the song's chords there following it ({said})")
        }
        None if !played => "on the song's own chords, as the recording plays none there besides the voice".into(),
        None => "on the song's own chords, as the passage's were not all heard".into(),
    };
    let names: Vec<&str> = secs.iter().map(|&si| bp.form[si].name.as_str()).collect();
    let what = if apart { "its voice" } else { "all of it, its lows out" };
    let mut out = Sampled { how: How::Passage, ..Sampled::default() };
    out.lines.push(format!(
        "{track}: {} of {name}, {what} ({}{:.0}% sung), {} at {:.0} BPM, played through from the first bar of the {label} \
         ({}), {chords_said}",
        bars_text(from_bar, len),
        heard_text(best.z, opt),
        best.sung * 100.0,
        moved(transpose, &bp.key),
        bp.bpm,
        names.join(", ")
    ));
    out.tracks.push(track);
    step_back(bp, &secs, &mut out);
    bp.lineage.push(format!("sampled {}: a passage from {name}", out.tracks.join(" and ")));
    Ok(out)
}

/// The chords of bars `first` to `first + len` of the recording, moved `transpose` semitones as the passage is and
/// named as Roman numerals in the song's `key`, one each `per` bars (the song's bars per chord); none unless the
/// analysis heard the chord of every one of them.
fn passage_chords(
    a: &Analysis,
    first: i64,
    len: u32,
    transpose: i32,
    key: &crate::theory::scale::Key,
    per: u32,
) -> Option<Vec<String>> {
    use crate::theory::chord::{Chord, roman_of};
    let up = |pc: u8| (pc as i32 + transpose).rem_euclid(12) as u8;
    (first..first + len as i64)
        .step_by(per.max(1) as usize)
        .map(|k| {
            let c = Chord::parse(a.chords.get(usize::try_from(k).ok()?)?)?;
            Some(roman_of(&Chord { root: up(c.root), bass: c.bass.map(up), ..c }, key))
        })
        .collect()
}

/// a lead that plays where a sample now sings, in `secs`, steps back a little, once: beside a voice sampled before, it
/// has already
fn step_back(bp: &mut Blueprint, secs: &[usize], out: &mut Sampled) {
    let voiced_before = |p: &Part| {
        bp.form.iter().any(|s| {
            s.layers.contains(&p.name)
                && bp
                    .loops
                    .iter()
                    .any(|l| sampled_voice(l) && !out.tracks.contains(&l.name) && s.layers.contains(&l.name))
        })
    };
    let leads: Vec<String> = bp
        .parts
        .iter()
        .filter(|p| p.role == Role::Lead && secs.iter().any(|&si| bp.form[si].layers.contains(&p.name)))
        .filter(|p| !voiced_before(p))
        .map(|p| p.name.clone())
        .collect();
    for p in bp.parts.iter_mut().filter(|p| leads.contains(&p.name)) {
        p.mix.level_db -= 1.5;
        out.lines.push(format!("{} 1.5 dB down, to leave room for the voice", p.name));
    }
}

/// the rhythms chopped slices are played in, a bar of sixteenths each: syncopated, as chopped vocals are
const CHOP_RHYTHMS: [&str; 4] = ["x..x..x...x.x...", "x.x..x..x.x..x..", "x..x..x.x..x..x.", "x...x.xx..x.x..."];

/// the order the slices come in, by their place among the voiced ones: a motif that comes back, as a hook does
const CHOP_MOTIFS: [[usize; 8]; 3] = [[0, 1, 0, 2, 0, 1, 3, 2], [0, 0, 1, 2, 0, 0, 3, 1], [0, 1, 2, 1, 0, 1, 2, 3]];

/// The recording `h` chopped: its best sung phrase (`opt.bars` long, else 2 bars) cut in eighths of its beats, and the
/// slices that hold the voice (the loudest six, in the order sung) played in a new rhythm, two bars round, the second
/// moving the motif on and ending in a stutter, from the first bar of each of the sections it goes in (the drops and
/// hooks, unless asked): the voice as an instrument, as chopped vocals are.
fn chops(
    bp: &mut Blueprint,
    h: &Heard,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    let (a, name) = (h.a, h.name);
    let bars = opt.bars.unwrap_or(2);
    let groups = groups(bp, &opt.sections)?;
    let (label, secs) = room(bp, &groups, 2).ok_or("no section asked for has room for two bars of chops")?;
    let p = find(h.src, h.stems, a, ears, &Options { bars: Some(bars), ..opt.clone() }, 1)
        .into_iter()
        .next()
        .ok_or(NO_VOICE)?;
    let cut = phrase_of(h.src, h.stems, a, &p);
    let eighth = ((a.bar_secs / 8.0 * SRF).round() as usize).max(1);
    let loud: Vec<f32> = (0..cut.len() / eighth).map(|k| rms(&cut, k * eighth, eighth)).collect();
    let top = loud.iter().fold(0f32, |m, v| m.max(*v));
    let mut voiced: Vec<usize> = (0..loud.len()).filter(|&k| top > 0.0 && loud[k] >= 0.3 * top).collect();
    voiced.sort_by(|x, y| loud[*y].total_cmp(&loud[*x]));
    voiced.truncate(6);
    voiced.sort_unstable();
    if voiced.len() < 2 {
        return Err("the phrase holds too little voice to chop: try another recording, or phrases".into());
    }
    let mut rng = crate::rng::Rng::seed_from(bp.seed, &format!("chops {name}"));
    let rhythm = *rng.pick(&CHOP_RHYTHMS);
    let motif = *rng.pick(&CHOP_MOTIFS);
    let sixteenth = eighth / 2;
    let steps: Vec<usize> = rhythm.char_indices().filter(|(_, c)| *c == 'x').map(|(i, _)| i).collect();
    let mut out_audio = Stereo::zeros(32 * sixteenth);
    let (fade_in, fade_out) = ((0.002 * SRF) as usize, (0.01 * SRF) as usize);
    for bar in 0..2 {
        for (j, &s) in steps.iter().enumerate() {
            let next = steps.get(j + 1).copied().unwrap_or(16);
            let slice = voiced[motif[(j + 2 * bar) % motif.len()] % voiced.len()];
            let len = ((next - s) * sixteenth).min(eighth);
            let at = (bar * 16 + s) * sixteenth;
            // the last hit of the second bar stutters: its slice twice, a thirty-second each
            let strokes: Vec<(usize, usize)> = if bar == 1 && j + 1 == steps.len() && len >= sixteenth {
                vec![(at, sixteenth / 2), (at + sixteenth / 2, sixteenth / 2)]
            } else {
                vec![(at, len)]
            };
            for (at, len) in strokes {
                let mut piece = cut.slice(slice * eighth, (slice * eighth + len).min(cut.len()));
                let n = piece.len();
                piece.fade(fade_in.min(n / 4), fade_out.min(n / 2));
                out_audio.add_at(&piece, at as i64, 1.0, 1.0);
            }
        }
    }
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let file = crate::take::store(&out_audio, dir).map_err(|e| format!("cannot keep the chops: {e:#}"))?;
    let track = fresh(bp, "chops");
    let from_bar = p.first + 1;
    bp.loops.push(LoopPart {
        name: track.clone(),
        role: "vocal".into(),
        sound: Sound::Loop {
            path: file.to_string_lossy().into_owned(),
            bpm: a.cut_bpm(),
            beats: 8.0,
            key: Some(a.key.tonic),
            tonal: true,
            label: format!("{name}, {} chopped", bars_text(from_bar, p.bars)),
        },
        transpose,
        chop: Vec::new(),
        slice_beats: 0.0,
        retime: false,
        mix: TrackMix { level_db: -2.0, reverb: 0.18, delay: 0.1, ..vocal_mix() },
        from: None,
        sampled: true,
    });
    for &si in &secs {
        bp.form[si].layers.push(track.clone());
    }
    let names: Vec<&str> = secs.iter().map(|&si| bp.form[si].name.as_str()).collect();
    let mut out = Sampled { how: How::Chops, ..Sampled::default() };
    out.lines.push(format!(
        "{track}: {} of {name} ({}{:.0}% sung) cut in eighths, {} slices of it played as {rhythm}, {} at {:.0} BPM, \
         every 2 bars from the first bar in the {label} ({})",
        bars_text(from_bar, p.bars),
        heard_text(p.z, opt),
        p.sung * 100.0,
        voiced.len(),
        moved(transpose, &bp.key),
        bp.bpm,
        names.join(", ")
    ));
    out.tracks.push(track);
    step_back(bp, &secs, &mut out);
    bp.lineage.push(format!("sampled {}: chops from {name}", out.tracks.join(" and ")));
    Ok(out)
}

/// the rhythms a stab is struck in, two bars of sixteenths each: now and then, as a sample is dropped in for a beat
const STAB_RHYTHMS: [&str; 4] = [
    "..............x...............x.",
    "......x...............x.....x...",
    "x.........................x.....",
    "....x...............x.......x.x.",
];

/// A short hit of the recording `h` struck as a drum is: the sharpest syllable of its best sung phrases (80 to 350
/// ms, from its vocals when they are apart, else from the middle of the mix), a one-shot on a drum lane of its own
/// (`vox_stab`) that strikes it on a few beats of every two bars, moved to the song's key, in the drops and hooks and
/// the verses and grooves (or the sections asked for). Its rhythm is a drum lane's, to change in the Drums tab.
fn stabs(
    bp: &mut Blueprint,
    h: &Heard,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    use std::collections::BTreeMap;
    let (a, name) = (h.a, h.name);
    let groups = groups(bp, &opt.sections)?;
    let wanted = ["drops and hooks", "verses and grooves"];
    let mut secs: Vec<usize> = match opt.sections.is_empty() {
        true => groups.iter().filter(|(l, _)| wanted.contains(&l.as_str())).flat_map(|(_, s)| s.clone()).collect(),
        false => Vec::new(),
    };
    if secs.is_empty() {
        secs = groups.first().map(|g| g.1.clone()).unwrap_or_default();
    }
    secs.sort_unstable();
    let mut best: Option<(f64, Stereo, Phrase, f64)> = None;
    for p in find(h.src, h.stems, a, ears, &Options { bars: Some(2), ..opt.clone() }, 3) {
        let (sound, heard) = match h.stems {
            Some(s) => {
                let v = voice(&cut_bars(a, &s.vocals, p.first, 2));
                (v.clone(), v)
            }
            None => {
                let mid = middle(&cut_bars(a, h.src, p.first, 2));
                let sustained = fft::harmonic(&mid.l);
                (mid, Stereo { l: sustained.clone(), r: sustained })
            }
        };
        if let Some((score, at, len)) = hit(&heard)
            && best.as_ref().is_none_or(|b| score > b.0)
        {
            let shot = sound.slice(at, (at + len).min(sound.len()));
            best = Some((score, shot, p, at as f64 / SRF));
        }
    }
    let (_, mut shot, p, at) = best.ok_or("nothing in the recording strikes like a syllable: try phrases instead")?;
    let peak = shot.peak();
    if peak > 0.0 {
        let g = 0.891 / peak;
        shot.channels_mut().into_iter().for_each(|ch| ch.iter_mut().for_each(|s| *s *= g));
    }
    let n = shot.len();
    shot.fade(((0.002 * SRF) as usize).min(n / 4), ((0.025 * SRF) as usize).min(n / 2));
    let file = crate::take::store(&shot, dir).map_err(|e| format!("cannot keep the stab: {e:#}"))?;
    let lane = fresh(bp, "vox_stab");
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let mut rng = crate::rng::Rng::seed_from(bp.seed, &format!("stabs {name}"));
    let steps = *rng.pick(&STAB_RHYTHMS);
    let (hits, pattern_bars) = crate::script::steps_to_hits(steps, 16);
    let from = p.start + at;
    bp.drums.push(DrumLane {
        name: lane.clone(),
        sound: Sound::Sample {
            path: file.to_string_lossy().into_owned(),
            root: 60,
            start: 0.0,
            end: None,
            label: format!("{name}, a stab at {}:{:04.1}", (from / 60.0) as u64, from % 60.0),
        },
        patterns: BTreeMap::from([("main".to_string(), hits)]),
        pattern_bars,
        pitch: transpose,
        choke: None,
        mix: TrackMix {
            level_db: 0.0,
            fx: vec![Effect::Eq {
                bands: vec![EqBand { kind: BiquadKind::Highpass, freq: 180.0, gain_db: 0.0, q: 0.7 }],
            }],
            reverb: 0.3,
            delay: 0.2,
            ..TrackMix::default()
        },
        length: None,
        speed: 1.0,
        swing: None,
        groove: None,
        groove_amount: 1.0,
        late_ms: 0.0,
        genre: None,
        tonal: true,
    });
    for &si in &secs {
        bp.form[si].layers.push(lane.clone());
    }
    let names: Vec<&str> = secs.iter().map(|&si| bp.form[si].name.as_str()).collect();
    let mut out = Sampled { how: How::Stabs, ..Sampled::default() };
    out.lines.push(format!(
        "{lane}: {:.0} ms of {name} from {}:{:04.1}, struck as {steps}, {}, in {} (a drum lane: its steps change in the \
         Drums tab)",
        n as f64 / SRF * 1000.0,
        (from / 60.0) as u64,
        from % 60.0,
        moved(transpose, &bp.key),
        names.join(", ")
    ));
    out.tracks.push(lane);
    bp.lineage.push(format!("sampled {}: a stab from {name}", out.tracks.join(" and ")));
    Ok(out)
}

/// The sharpest short sound of `x`, as (how good a stab it makes, where it starts in samples, how long it is): where
/// its loudness leaps at least 2.5 times over the 40 ms before, to a third of its loudest at least, until it falls to
/// a quarter of its peak (80 to 350 ms); the louder, the more sudden and the nearer a fifth of a second long, the
/// better. None when nothing in it leaps. Its first 40 ms are not looked at: what sounds there may have started
/// before it was cut.
fn hit(x: &Stereo) -> Option<(f64, usize, usize)> {
    let hop = (0.005 * SRF) as usize;
    let env: Vec<f32> = (0..x.len() / hop).map(|k| rms(x, k * hop, hop)).collect();
    let top = env.iter().fold(0f32, |m, v| m.max(*v));
    if top <= 1e-4 {
        return None;
    }
    let (mut best, mut last): (Option<(f64, usize, usize)>, Option<usize>) = (None, None);
    for i in 8..env.len() {
        let before = env[i - 8..i].iter().sum::<f32>() / 8.0;
        if env[i] < 0.33 * top || env[i] < 2.5 * before || last.is_some_and(|l| i - l < 16) {
            continue;
        }
        last = Some(i);
        let stop = (i + 70).min(env.len());
        let peak = env[i..(i + 6).min(env.len())].iter().fold(0f32, |m, v| m.max(*v));
        let end = (i + 16..stop).find(|&j| env[j] < 0.25 * peak).unwrap_or(stop);
        if end < i + 16 {
            continue;
        }
        let secs = (end - i) as f64 * 0.005;
        let sudden = (env[i] / before.max(1e-6)).min(10.0) as f64 / 10.0;
        let score = (peak / top) as f64 + 0.5 * sudden + 0.5 * (1.0 - ((secs - 0.2).abs() / 0.2).min(1.0));
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, i.saturating_sub(1) * hop, (end - i + 1) * hop));
        }
    }
    best
}

/// the loudness (RMS of both sides) of `len` samples of `x` from `at`
fn rms(x: &Stereo, at: usize, len: usize) -> f32 {
    let end = (at + len).min(x.len());
    if end <= at {
        return 0.0;
    }
    let sum: f32 = (at..end).map(|i| 0.5 * (x.l[i] * x.l[i] + x.r[i] * x.r[i])).sum();
    (sum / (end - at) as f32).sqrt()
}

/// Phrase `p` as a sample: from the recording's vocals, when they are apart (`voice`), else from the mix without its
/// drums (`clean`).
fn phrase_of(src: &Stereo, stems: Option<&Stems>, a: &Analysis, p: &Phrase) -> Stereo {
    let mut cut = match stems {
        Some(s) => voice(&cut_bars(a, &s.vocals, p.first, p.bars as usize)),
        None => clean(&cut_bars(a, src, p.first, p.bars as usize)),
    };
    if let Some((from, to)) = p.within {
        gate(&mut cut, a, p.first, from, to);
        // as loud as the phrase itself, not as what is sung round it in its bars, which the gate took out
        let peak = cut.peak();
        if peak > 0.0 {
            let g = 0.891 / peak;
            cut.channels_mut().into_iter().for_each(|ch| ch.iter_mut().for_each(|s| *s *= g));
        }
    }
    cut
}

/// Where bar `k` of the recording `a` analyzed starts, in seconds: its own bars, then a grid of `a.bar_secs` before the
/// first of them and after the last, so that a phrase sung from the recording's first moment, or to its last, falls in
/// whole bars too.
fn bar_time(a: &Analysis, k: i64) -> f64 {
    match usize::try_from(k) {
        Ok(k) => a.bar_start(k),
        Err(_) => a.bar_start(0) + k as f64 * a.bar_secs,
    }
}

/// the bar `t` seconds into the recording falls in, as `bar_time` counts them
fn bar_of(a: &Analysis, t: f64) -> i64 {
    let (start, end, bar) = (a.bar_start(0), a.bar_start(a.bars), a.bar_secs.max(1e-3));
    if t < start {
        ((t - start) / bar).floor() as i64
    } else if t >= end {
        a.bars as i64 + ((t - end) / bar).floor() as i64
    } else {
        a.bar_at(t)
    }
}

/// Bars `first` to `first + count` of `x`, the recording analyzed, as `Analysis::cut` cuts them: each `a.bar_secs`
/// long, and silent where they run before the recording or after it.
fn cut_bars(a: &Analysis, x: &Stereo, first: i64, count: usize) -> Stereo {
    let times: Vec<f64> = (first..=first + count as i64).map(|k| bar_time(a, k)).collect();
    let from = (times[0] * SRF).round() as i64;
    let mut piece = Stereo::zeros(((times[count] * SRF).round() as i64 - from).max(0) as usize);
    for (i, (l, r)) in piece.l.iter_mut().zip(piece.r.iter_mut()).enumerate() {
        if let Ok(s) = usize::try_from(from + i as i64)
            && s < x.len()
        {
            (*l, *r) = (x.l[s], x.r[s]);
        }
    }
    let starts: Vec<f64> = times.iter().map(|t| t - times[0]).collect();
    crate::warp::flatten(&piece, &starts, a.bar_secs)
}

/// `x`, the bars of a recording from bar `first` on as `cut_bars` cuts them, silent outside `from`..`to` seconds of
/// the recording: faded in over 4 ms, as a cut there falls on a zero crossing, and out over 25 ms of its release
fn gate(x: &mut Stereo, a: &Analysis, first: i64, from: f64, to: f64) {
    // where `t` is in the cut, whose bars are each `a.bar_secs` long
    let at = |t: f64| {
        let b = bar_of(a, t).max(first);
        let (s, e) = (bar_time(a, b), bar_time(a, b + 1));
        let frac = ((t - s) / (e - s).max(1e-6)).clamp(0.0, 1.0);
        ((((b - first) as f64 + frac) * a.bar_secs * SRF) as usize).min(x.len())
    };
    let (i0, i1) = (at(from), at(to).max(at(from)));
    let (fade_in, fade_out) = ((0.004 * SRF) as usize, ((0.025 * SRF) as usize).min(i1 - i0));
    for i in 0..x.len() {
        let g = if i < i0 || i >= i1 {
            0.0
        } else if i < i0 + fade_in {
            (i - i0) as f32 / fade_in as f32
        } else if i + fade_out > i1 {
            (i1 - i) as f32 / fade_out.max(1) as f32
        } else {
            1.0
        };
        x.l[i] *= g;
        x.r[i] *= g;
    }
}

/// what the sampler says of a recording it finds no voice in
const NO_VOICE: &str = "nothing in the recording sounds like a voice: try other words for what to find";

/// how a phrase comes round in its sections: how often, its loop's length in bars, and why, when the song chose it
#[derive(Clone, Debug)]
struct Round {
    every: Every,
    bars: u32,
    why: Option<String>,
}

/// How a phrase `bars` long comes round in sections `secs` of the song `bp`: as `every` asks, else as the song suits
/// it. Once from the start of each where a lead or a voice sings in most of them already (`sung`), as two melodies at
/// once say neither; else every other time round, answered, in a genre built on samples (one that samples by
/// default: boom bap, lo-fi, trip hop ...), whose producers loop what they sample; and once every 8 bars in one that
/// is not, a voice that comes back now and then.
fn period(bp: &Blueprint, secs: &[usize], bars: u32, every: Option<Every>, sung: &[usize]) -> Round {
    let longest = secs.iter().map(|&si| bp.form[si].bars).max().unwrap_or(bars).max(bars);
    let round = |every: Every, why: Option<String>| {
        let n = match every {
            Every::Once => longest,
            Every::Bars(n) => n.max(bars),
            Every::Through => bars,
        };
        Round { every, bars: n, why }
    };
    if let Some(every) = every {
        return round(every, None);
    }
    if 2 * secs.iter().filter(|si| sung.contains(si)).count() > secs.len() {
        return round(Every::Once, Some("as a lead or a voice sings there already".into()));
    }
    let genre = bp.genre.replace('_', " ");
    match crate::genres::get(&bp.genre).is_ok_and(|g| g.sampling.is_some_and(|s| s.by_default)) {
        true => round(Every::Bars(2 * bars), Some(format!("as {genre} is built on samples"))),
        false => {
            round(Every::Bars((2 * bars).max(8)), Some(format!("now and then, as {genre} is not built on samples")))
        }
    }
}

/// whether a loop is a voice sampled into the song (a phrase, or chops of one), as a lead steps back for once; a break,
/// a bass line or a loop of instruments sampled is not
fn sampled_voice(l: &LoopPart) -> bool {
    l.sampled && l.role == "vocal"
}

/// whether a melody plays in section `si` of `bp`: a lead part, or a loop mixed as a voice or a lead (a phrase sampled
/// into the song, or a take, which `add_take` mixes as a lead unless told otherwise)
fn sings_in(bp: &Blueprint, si: usize) -> bool {
    let layers = &bp.form[si].layers;
    bp.parts.iter().any(|p| p.role == Role::Lead && layers.contains(&p.name))
        || bp.loops.iter().any(|l| matches!(l.role.as_str(), "vocal" | "lead") && layers.contains(&l.name))
}

/// a recording as the sampler heard it: its sound, its stems when they are apart, its analysis and its name
pub(crate) struct Heard<'a> {
    pub src: &'a Stereo,
    pub stems: Option<&'a Stems>,
    pub a: &'a Analysis,
    pub name: &'a str,
}

/// `phrases` of the recording `h` (each with the effects it plays through) placed in the song, in the sections `opt`
/// asks for, as `sample` places them
fn place(
    bp: &mut Blueprint,
    h: &Heard,
    phrases: &[(Phrase, Vec<Effect>)],
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    let (src, a, name) = (h.src, h.a, h.name);
    let groups = groups(bp, &opt.sections)?;
    let bar = a.bar_secs;
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let project = crate::compose::arrange(bp);
    // each group of sections takes a phrase from its first bar and another from the bar after it ends, answering it
    let mut taken: Vec<[bool; 2]> = vec![[false; 2]; groups.len()];
    let mut out = Sampled::default();
    let mut voiced: Vec<usize> = Vec::new();
    let voiced_before: Vec<usize> = (0..bp.form.len())
        .filter(|&si| bp.loops.iter().any(|l| sampled_voice(l) && bp.form[si].layers.contains(&l.name)))
        .collect();
    // where a lead or a voice already sings, before any of these phrases
    let sung: Vec<usize> = (0..bp.form.len()).filter(|&si| sings_in(bp, si)).collect();
    for (p, fx) in phrases {
        let cut = phrase_of(src, h.stems, a, p);
        let profile = shift(&pitch_profile(&cut), transpose);
        let mut best: Option<(f64, usize, usize, f64, Round)> = None;
        for (g, (_, secs)) in groups.iter().enumerate() {
            let round = period(bp, secs, p.bars, opt.every, &sung);
            for slot in 0..2 {
                let offset = slot as u32 * p.bars;
                // an answer needs room after the phrase it answers, before the phrase comes round again
                if taken[g][slot] || offset + p.bars > round.bars {
                    continue;
                }
                // a slot no section of the group has room for would never sound
                let Some(fits) = fit(&project, secs, offset, p.bars, round.bars, &profile) else { continue };
                // the earlier groups suit a vocal best: the drops and hooks, then the breaks, the verses, the builds
                let suits = fits + 0.12 * (groups.len() - g) as f64 / groups.len() as f64 - 0.05 * slot as f64;
                if best.as_ref().is_none_or(|b| suits > b.0) {
                    best = Some((suits, g, slot, fits, round.clone()));
                }
            }
        }
        // a phrase with no room left may be longer than the next one
        let Some((_, g, slot, fits, round)) = best else { continue };
        taken[g][slot] = true;
        let period = round.bars;
        let offset = slot as u32 * p.bars;
        let file = crate::take::store(&layout(&cut, offset as f64 * bar, period as f64 * bar), dir)
            .map_err(|e| format!("cannot keep the phrase: {e:#}"))?;
        let names = bp.layer_names();
        let track = (1..).map(|k| format!("vox{k}")).find(|n| !names.contains(n)).unwrap_or_default();
        let from_bar = p.first + 1;
        bp.loops.push(LoopPart {
            name: track.clone(),
            role: "vocal".into(),
            sound: Sound::Loop {
                path: file.to_string_lossy().into_owned(),
                bpm: a.cut_bpm(),
                beats: period as f64 * 4.0,
                key: Some(a.key.tonic),
                tonal: true,
                label: format!("{name}, {}", bars_text(from_bar, p.bars)),
            },
            transpose,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix: TrackMix { fx: vocal_mix().fx.into_iter().chain(fx.iter().cloned()).collect(), ..vocal_mix() },
            from: None,
            sampled: true,
        });
        let (label, secs) = &groups[g];
        let secs: Vec<usize> = secs.iter().copied().filter(|&si| offset + p.bars <= bp.form[si].bars).collect();
        for &si in &secs {
            bp.form[si].layers.push(track.clone());
            voiced.push(si);
        }
        let heard = match p.z {
            Some(z) => format!("the ears {z:+.1} for '{}', ", opt.find.as_deref().unwrap_or(DEFAULT_FIND)),
            None => String::new(),
        };
        let moved = match transpose {
            0 => "in its own key".to_string(),
            t => {
                format!("{} {} into {}", t.abs(), if t > 0 { "semitones up" } else { "semitones down" }, bp.key.name())
            }
        };
        let at = if slot == 0 { "from the first bar" } else { "answering, from the bar after" };
        let names = secs.iter().map(|&si| bp.form[si].name.as_str()).collect::<Vec<_>>().join(", ");
        let comes = match round.every {
            Every::Through => format!("all through the {label} ({names})"),
            Every::Once => format!("once in each of the {label} ({names}), {at}"),
            Every::Bars(_) => format!("every {period} bars {at} in the {label} ({names})"),
        };
        let why = round.why.map(|w| format!(", {w}")).unwrap_or_default();
        out.lines.push(format!(
            "{track}: {} of {name} ({heard}{:.0}% sung), {moved} at {:.0} BPM, {comes}{why}, on chords it agrees with \
             {:.0}%",
            bars_text(from_bar, p.bars),
            p.sung * 100.0,
            bp.bpm,
            fits * 100.0
        ));
        out.tracks.push(track);
    }
    if out.tracks.is_empty() {
        let bars = phrases.iter().map(|(p, _)| p.bars).min().unwrap_or_default();
        return Err(format!("no section asked for has room for a phrase of {bars} bars: try shorter phrases"));
    }
    // a lead that plays with the voice steps back a little, once: beside a phrase sampled before, it has already
    let plays = |p: &Part, at: &[usize]| at.iter().any(|&si| bp.form[si].layers.contains(&p.name));
    let leads: Vec<String> = bp
        .parts
        .iter()
        .filter(|p| p.role == Role::Lead && plays(p, &voiced) && !plays(p, &voiced_before))
        .map(|p| p.name.clone())
        .collect();
    for p in bp.parts.iter_mut().filter(|p| leads.contains(&p.name)) {
        p.mix.level_db -= 1.5;
        out.lines.push(format!("{} 1.5 dB down, to leave room for the voice", p.name));
    }
    bp.lineage.push(format!("sampled {}: {} from {name}", out.tracks.join(" and "), plural(out.tracks.len())));
    Ok(out)
}

/// A piece the sampler offers before any goes into the song (`pieces`): a phrase of the recording, cut and cleaned of
/// its drums as `sample` cuts one, or a piece of another kind (`kinds`), and the same through a treatment.
#[derive(Clone, Debug)]
pub struct Piece {
    /// what it is, and what it scored for, in words (nothing for a phrase)
    pub kind: Kind,
    pub about: String,
    /// where it is in the recording: a phrase's, or the bars of a piece of another kind
    pub phrase: Phrase,
    pub raw: Stereo,
    pub treated: Stereo,
    /// the treatment in words, and its effects, which a piece kept treated plays through
    pub treatment: String,
    pub fx: Vec<Effect>,
    /// the tempo it keeps, cut from the recording
    pub bpm: f64,
    /// the section of the recording its phrase starts in, by kind and count ("hook 2"); none where the analysis heard
    /// no section there
    pub section: String,
}

impl Piece {
    /// where it is in the recording, in words: "bars 12-13, 0:31"
    pub fn place(&self) -> String {
        let at = self.phrase.within.map_or(self.phrase.start, |(from, _)| from).max(0.0) as u64;
        format!("{}, {}:{:02}", bars_text(self.phrase.first + 1, self.phrase.bars), at / 60, at % 60)
    }
}

/// the treatments a piece is offered with, gentlest first: in words, and as effects, for a piece `beats` long
fn treatment(k: usize, beats: f64) -> (String, Vec<Effect>) {
    let chain = |json: serde_json::Value| -> Vec<Effect> { serde_json::from_value(json).unwrap_or_default() };
    let (words, fx) = match k % 7 {
        0 => (
            "dusty: through tape, filtered at 5 kHz with a little wow, over vinyl crackle",
            serde_json::json!([
                {"fx": "tape", "wow": 0.25, "flutter": 0.15, "drive_db": 3, "lowpass_hz": 5000},
                {"fx": "vinyl", "crackle": 0.25, "hiss_db": -42}
            ]),
        ),
        1 => (
            "washed: an echo on the dotted eighth into a long hall",
            serde_json::json!([
                {"fx": "delay", "beats": 0.75, "feedback": 0.35, "damping": 0.4, "pingpong": true, "mix": 0.3},
                {"fx": "reverb", "decay": 3.5, "damping": 0.4, "predelay_ms": 30, "mix": 0.35}
            ]),
        ),
        2 => (
            "lo-fi: crushed to 10 bits and filtered at 3.5 kHz",
            serde_json::json!([
                {"fx": "bitcrush", "bits": 10, "downsample": 2, "mix": 0.8},
                {"fx": "filter", "mode": "lp", "cutoff": 3500, "res": 0.15}
            ]),
        ),
        3 => (
            "phased: a slow phaser over a chorus",
            serde_json::json!([
                {"fx": "chorus", "rate_hz": 0.6, "depth_ms": 3, "delay_ms": 14, "mix": 0.35},
                {"fx": "phaser", "rate_hz": 0.25, "stages": 6, "depth": 0.8, "feedback": 0.4, "mix": 0.5}
            ]),
        ),
        4 => (
            "telephone: only its middle, from 400 Hz to 3 kHz, driven",
            serde_json::json!([
                {"fx": "filter", "mode": "hp", "cutoff": 400, "res": 0.3},
                {"fx": "filter", "mode": "lp", "cutoff": 3000, "res": 0.3},
                {"fx": "saturate", "kind": "hard", "drive_db": 9, "mix": 0.6}
            ]),
        ),
        5 => (
            "stuttered: its last beat repeated, faster and faster",
            serde_json::json!([{"fx": "stutter", "start": (beats - 1.0).max(0.0), "len": 1.0, "slice": 0.25}]),
        ),
        _ => (
            "reversed: its last two beats backwards, into an echo",
            serde_json::json!([
                {"fx": "reverse", "start": (beats - 2.0).max(0.0), "len": 2.0},
                {"fx": "delay", "beats": 0.5, "feedback": 0.3, "damping": 0.3, "mix": 0.25}
            ]),
        ),
    };
    (words.to_string(), chain(fx))
}

/// What `offer` offers: the pieces, and why none of them is a phrase, when phrases were asked for with the other kinds
/// and none could be cut (no voice sings in it, or none of its phrases fits in the bars asked for).
pub struct Offered {
    pub pieces: Vec<Piece>,
    pub no_phrase: Option<String>,
}

/// The pieces of a take, as the take after is drawn away from (`Options::shown`): each its kind and where it is in the
/// recording, in seconds.
pub fn shown(pieces: &[Piece]) -> Vec<(Kind, f64, f64)> {
    let span = |p: &Piece| {
        let bars = p.phrase.bars as f64 * 240.0 / p.bpm.max(1.0);
        p.phrase.within.unwrap_or((p.phrase.start, p.phrase.start + bars))
    };
    pieces.iter().map(|p| (p.kind, span(p).0, span(p).1)).collect()
}

/// The kinds of piece the producers of `genre` sample (its `sampling`; a fusion's lead genre's): a drum break, a bass,
/// a phrase to build the song on (here a loop of a hook), a texture under it (a loop of another passage) and a vocal
/// (a sung phrase).
pub fn leaning(genre: &str) -> Vec<Kind> {
    let lead = genre.split('+').next().unwrap_or(genre).trim();
    let Some(s) = crate::genres::get(lead).ok().and_then(|g| g.sampling) else { return Vec::new() };
    let samples = [
        (s.drum_break.is_some(), Kind::Break),
        (s.bass.is_some(), Kind::Bass),
        (s.phrase.is_some(), Kind::Hook),
        (s.texture.is_some(), Kind::Passage),
        (s.vocal.is_some(), Kind::Phrase),
    ];
    samples.into_iter().filter_map(|(does, k)| does.then_some(k)).collect()
}

/// The pieces `offer` offers, without why none of them is a phrase.
pub fn pieces(src: &Stereo, ears: Option<&crate::ears::Ears>, opt: &Options, seed: u64) -> Result<Vec<Piece>, String> {
    offer(src, ears, opt, seed).map(|o| o.pieces)
}

/// how many pieces of the other kinds `offer` offers: the best of each kind it holds, and as many as `MORE` besides of
/// the kinds producers sample most where they are good and unlike the others (`kinds::varied`); `FEW` to choose
/// among when only a kind or two is asked for
const MORE: usize = 2;
const FEW: usize = 4;

/// The pieces of `src` the sampler offers before any goes into the song, of the kinds `opt.kinds` asks for (every kind
/// it holds, when none): `opt.count` phrases it sings whole (`sung_whole`, at most `MOST`), each cleaned as `sample`
/// cleans one, and a varied choice of the other kinds it holds (`kinds::varied`), the best of each kind and more of
/// those producers sample most and the song's genre does (`opt.lean`); each as it was cut and through a treatment.
/// Take `seed` 0 offers the best with the gentler treatments; each other take draws again from the good ones, away
/// from those the take before offered (`opt.shown`), with other treatments. The ears, when given, listen for
/// `opt.find`. When its phrases cannot be cut, the other kinds it holds are still offered, with why there is no
/// phrase.
pub fn offer(src: &Stereo, ears: Option<&crate::ears::Ears>, opt: &Options, seed: u64) -> Result<Offered, String> {
    if let Some(b) = opt.bars.filter(|b| !(1..=8).contains(b)) {
        return Err(format!("a piece is 1 to 8 bars, not {b}"));
    }
    if (src.len() as f64 / SRF) < 4.0 {
        return Err("the recording is too short to sample (4 s at least)".into());
    }
    let stems = crate::stems::of(src, &mut |_| {});
    let a = analysis::analyze_with(src, stems.as_deref(), None);
    let asked = |k: Kind| opt.kinds.is_empty() || opt.kinds.contains(&k);
    let mut rng = crate::rng::Rng::seed_from(seed, "pieces");
    let phrases = match asked(Kind::Phrase) {
        true => sung_pieces(src, stems.as_deref(), &a, ears, opt, seed, &mut rng),
        false => Ok(Vec::new()),
    };
    // the pieces of each other kind worth offering, each kind drawn on its own (so that asking for one draws it as it
    // is drawn among them all), then a varied choice of them
    let mut pool: Vec<(kinds::Cut, Stereo)> = Vec::new();
    for kind in Kind::ALL.into_iter().filter(|&k| k != Kind::Phrase && asked(k)) {
        let Some((x, rest)) = kinds::source(kind, src, stems.as_deref()) else { continue };
        let mut rng = crate::rng::Rng::seed_from(seed, kind.name());
        for c in kinds::cuts(kind, &a, &x, &rest, seed, &opt.shown, &mut rng) {
            let cut = match c.span {
                Some((from, to)) => x.slice(((from * SRF) as usize).min(x.len()), ((to * SRF) as usize).min(x.len())),
                None => cut_bars(&a, &x, c.first as i64, c.bars as usize),
            };
            pool.push((c, kinds::level(&cut)));
        }
    }
    let held: Vec<kinds::Cut> = pool.iter().map(|(c, _)| c.clone()).collect();
    let mut kinds_held: Vec<Kind> = held.iter().map(|c| c.kind).collect();
    kinds_held.dedup();
    let want = (kinds_held.len() + MORE).max(FEW);
    let cuts: Vec<(kinds::Cut, Stereo)> =
        kinds::varied(&held, want, &opt.lean).into_iter().map(|i| pool[i].clone()).collect();
    let (phrases, no_phrase) = match phrases {
        Ok(p) => (p, None),
        // what else a recording holds is still offered when its phrases cannot be cut, and why there is none
        Err(e) if !cuts.is_empty() => (Vec::new(), Some(e)),
        Err(e) => return Err(e),
    };
    if phrases.is_empty() && cuts.is_empty() {
        let names: Vec<&str> = opt.kinds.iter().map(|k| k.name()).collect();
        return Err(format!("nothing of the kinds asked for ({}) plays in the recording", names.join(", ")));
    }
    let first = if seed == 0 { 0 } else { rng.below(7) };
    let treated = |i: usize, raw: &Stereo, beats: f64| {
        let (treatment, fx) = treatment(first + i, beats);
        let mut out = raw.clone();
        let ctx = crate::fx::Ctx::whole(SRF, a.cut_bpm(), seed.wrapping_add(i as u64), out.len());
        crate::fx::apply_chain(&fx, &mut out, &ctx);
        (out, treatment, fx)
    };
    let mut out: Vec<Piece> = Vec::new();
    for (i, p) in phrases.into_iter().enumerate() {
        let raw = phrase_of(src, stems.as_deref(), &a, &p);
        let (treated, treatment, fx) = treated(i, &raw, p.bars as f64 * 4.0);
        let section = section_at(&a, bar_of(&a, p.within.map_or(p.start, |w| w.0)));
        let (kind, about, bpm) = (Kind::Phrase, String::new(), a.cut_bpm());
        out.push(Piece { kind, about, phrase: p, raw, treated, treatment, fx, bpm, section });
    }
    for (c, raw) in cuts {
        // a stab is a beat of its hit, where it is; a loop, a break or a bass line its bars
        let beats = c.span.map_or(c.bars as f64 * 4.0, |_| 1.0);
        let (treated, treatment, fx) = treated(out.len(), &raw, beats);
        let first = c.first as i64;
        let phrase = Phrase {
            start: c.span.map_or_else(|| bar_time(&a, first), |s| s.0),
            first,
            bars: c.bars,
            z: None,
            sung: 0.0,
            centred: 0.0,
            score: c.score,
            within: c.span,
        };
        let section = section_at(&a, first);
        let (kind, about, bpm) = (c.kind, c.about, a.cut_bpm());
        out.push(Piece { kind, about, phrase, raw, treated, treatment, fx, bpm, section });
    }
    Ok(Offered { pieces: out, no_phrase })
}

/// The phrases `pieces` offers: `opt.count` the recording sings whole, the best on take `seed` 0 and drawn again from
/// the good ones on any other (`rng`), no two overlapping, in the order they are sung; why there are none, when none.
fn sung_pieces(
    src: &Stereo,
    stems: Option<&Stems>,
    a: &Analysis,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    seed: u64,
    rng: &mut crate::rng::Rng,
) -> Result<Vec<Phrase>, String> {
    let count = opt.count.clamp(1, MOST);
    let mut pool = sung_whole(src, stems, a, ears, opt);
    if pool.is_empty() {
        let (b, or) = opt.bars.map_or((8, " or fewer"), |b| (b, ""));
        return Err(match fewest(src, stems, a) {
            Some(n) => format!("no phrase of it is sung whole in {b} bars{or}: the shortest takes {n}"),
            None => NO_VOICE.into(),
        });
    }
    if seed > 0 {
        let span = |p: &Phrase| p.within.unwrap_or((p.start, bar_time(a, p.first + p.bars as i64)));
        let shown: Vec<(f64, f64)> = opt.shown.iter().filter(|s| s.0 == Kind::Phrase).map(|s| (s.1, s.2)).collect();
        pool = drawn(pool, &|p| shown.iter().any(|&s| kinds::same_span(span(p), s)), rng);
    }
    let mut chosen: Vec<Phrase> = Vec::new();
    for p in pool {
        let span = p.within.unwrap_or((p.start, bar_time(a, p.first + p.bars as i64)));
        if chosen.iter().all(|q| q.within.is_none_or(|(s, e)| span.1 <= s || span.0 >= e)) {
            chosen.push(p);
        }
        if chosen.len() == count {
            break;
        }
    }
    chosen.sort_by(|x, y| x.start.total_cmp(&y.start));
    Ok(chosen)
}

/// A piece `pieces` offered that the person kept: as it was cut or through its treatment, and whether chopped onto
/// pads (`kinds::chop`) rather than kept whole.
#[derive(Clone, Copy)]
pub struct Keep<'a> {
    pub piece: &'a Piece,
    pub treated: bool,
    pub chopped: bool,
}

impl<'a> Keep<'a> {
    /// `piece` as it was cut, whole
    pub fn of(piece: &'a Piece) -> Keep<'a> {
        Keep { piece, treated: false, chopped: false }
    }
}

/// The pieces `pieces` offered that the person kept (`Keep`), sampled into the song as `sample` places phrases, in the
/// sections `opt` asks for (those that suit each, when none).
pub fn keep(
    bp: &mut Blueprint,
    src: &Stereo,
    name: &str,
    kept: &[Keep],
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    if bp.form.is_empty() {
        return Err("the song has no sections to put the pieces in".into());
    }
    if kept.is_empty() {
        return Err("keep at least one of the pieces".into());
    }
    groups(bp, &opt.sections)?;
    let stems = crate::stems::of(src, &mut |_| {});
    let a = analysis::analyze_with(src, stems.as_deref(), None);
    let heard = Heard { src, stems: stems.as_deref(), a: &a, name };
    let fx = |k: &Keep| if k.treated { k.piece.fx.clone() } else { Vec::new() };
    let whole = |k: &&Keep| k.piece.kind == Kind::Phrase && !k.chopped;
    let phrases: Vec<(Phrase, Vec<Effect>)> =
        kept.iter().filter(whole).map(|k| (k.piece.phrase.clone(), fx(k))).collect();
    // phrases that cannot go in leave the song as it was before them, the other pieces kept to go in, and say so
    let (mut out, refused) = match phrases.is_empty() {
        true => (Sampled::default(), None),
        false => {
            let before = bp.clone();
            match place(bp, &heard, &phrases, opt, dir) {
                Ok(s) => (s, None),
                Err(e) => {
                    *bp = before;
                    (Sampled::default(), Some(e))
                }
            }
        }
    };
    out.kinds = vec![Kind::Phrase; out.tracks.len()];
    let others: Vec<&Keep> = kept.iter().filter(|k| !whole(k)).collect();
    // a piece chopped plays where its kind does, a phrase chopped where chops of a voice do: in the drops and hooks
    let plays_as = |k: &Keep| if k.chopped && k.piece.kind == Kind::Phrase { Kind::Hook } else { k.piece.kind };
    // several pieces of a kind kept, and no sections named: they share the sections their kind plays in, the fuller of
    // them in the louder sections (`dealt`), so that none plays over another
    let mut places: Vec<Option<Vec<usize>>> = vec![None; others.len()];
    for kind in Kind::ALL.into_iter().filter(|_| opt.sections.is_empty()) {
        let mut of: Vec<usize> = (0..others.len()).filter(|&i| plays_as(others[i]) == kind).collect();
        if of.len() < 2 {
            continue;
        }
        of.sort_by(|&i, &j| fullness(&others[i].piece.raw).total_cmp(&fullness(&others[j].piece.raw)));
        let groups = dealt(bp, &sections_for(bp, kind, &[])?, of.len());
        of.into_iter().zip(groups).for_each(|(i, g)| places[i] = Some(g));
    }
    let mut failed: Vec<String> = Vec::new();
    for (k, place) in others.into_iter().zip(places) {
        let p = k.piece;
        if place.as_ref().is_some_and(Vec::is_empty) {
            let said = p.kind.said(p.phrase.bars);
            out.lines
                .push(format!("{said} of {name}: no section of its own is left for it (name the sections for it)"));
            continue;
        }
        let secs = match place {
            Some(s) => s,
            None => sections_for(bp, plays_as(k), &opt.sections)?,
        };
        let before = bp.clone();
        let placed = match (p.kind, k.chopped) {
            // one hit, which nothing chops further
            (Kind::Stab, _) => place_stab(bp, &heard, p, &fx(k), &secs, dir).map(|(t, l)| (vec![t], l)),
            (_, true) => place_chops(bp, &heard, k, &secs, dir),
            (_, false) => place_cut(bp, &heard, p, &fx(k), opt, Some(secs.as_slice()), dir).map(|(t, l)| (vec![t], l)),
        };
        // a piece that cannot go in leaves the song as it was before it, as a phrase does, and says why
        let (tracks, line) = match placed {
            Ok(placed) => placed,
            Err(e) => {
                *bp = before;
                failed.push(e);
                continue;
            }
        };
        out.tracks.extend(tracks);
        out.lines.push(line);
        out.kinds.push(p.kind);
    }
    if out.tracks.is_empty() {
        return match refused.into_iter().chain(failed).reduce(|a, b| format!("{a}; {b}")) {
            Some(e) => Err(e),
            None => Ok(out),
        };
    }
    out.lines.extend(failed);
    if let Some(e) = refused {
        out.lines.push(format!("no phrase went in: {e}"));
    }
    Ok(out)
}

/// A piece of a kind but a phrase (`kinds`) kept, placed in the song as its kind plays: a break as a drum loop beside the
/// song's drums, a bass line in place of the song's bass, a loop of the hook or of another passage as a sample, each
/// all through the sections `opt` asks for (`sections_for`), or those `secs` deals it; its track, and a line saying
/// where it came from and went.
fn place_cut(
    bp: &mut Blueprint,
    h: &Heard,
    p: &Piece,
    fx: &[Effect],
    opt: &Options,
    secs: Option<&[usize]>,
    dir: &Path,
) -> Result<(String, String), String> {
    let (a, name) = (h.a, h.name);
    let secs = match secs {
        Some(s) => s.to_vec(),
        None => sections_for(bp, p.kind, &opt.sections)?,
    };
    let transpose = match p.kind.tonal() {
        true => crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor())),
        false => 0,
    };
    let file = crate::take::store(&p.raw, dir).map_err(|e| format!("cannot keep the piece: {e:#}"))?;
    let track = fresh(
        bp,
        match p.kind {
            Kind::Break => "break",
            Kind::Bass => "bassline",
            Kind::Hook => "hookloop",
            _ => "loop",
        },
    );
    let bpm = a.cut_bpm();
    // the mix levels it as the drums, the bass or a sample (`render::target_level`), a break through the drums' bus and
    // a bass line through the bus of the bass it plays in place of
    let bass_bus = bp.parts.iter().find(|q| q.role == Role::Bass).map_or_else(|| "bass".into(), |q| q.mix.bus.clone());
    let (role, mix) = match p.kind {
        Kind::Break => ("drums", TrackMix { bus: "drums".into(), ..TrackMix::default() }),
        Kind::Bass => ("bass", TrackMix { bus: bass_bus, ..TrackMix::default() }),
        _ => ("sample", record_mix()),
    };
    let from_bar = p.phrase.first + 1;
    bp.loops.push(LoopPart {
        name: track.clone(),
        role: role.into(),
        sound: Sound::Loop {
            path: file.to_string_lossy().into_owned(),
            bpm,
            beats: p.phrase.bars as f64 * 4.0,
            key: p.kind.tonal().then_some(a.key.tonic),
            tonal: p.kind.tonal(),
            label: format!("{name}, {}", bars_text(from_bar, p.phrase.bars)),
        },
        transpose,
        chop: Vec::new(),
        slice_beats: 0.5,
        // a break played far from its own tempo is cut onto the song's grid at its own speed: stretched, its hits smear
        retime: p.kind == Kind::Break && (bp.bpm / bpm).ln().abs() > 0.03,
        mix: TrackMix { fx: mix.fx.iter().cloned().chain(fx.iter().cloned()).collect(), ..mix },
        from: None,
        sampled: true,
    });
    let basses: Vec<String> = bp.parts.iter().filter(|q| q.role == Role::Bass).map(|q| q.name.clone()).collect();
    let mut instead = false;
    for &si in &secs {
        let layers = &mut bp.form[si].layers;
        if p.kind == Kind::Bass {
            let before = layers.len();
            layers.retain(|l| !basses.contains(l));
            instead |= layers.len() < before;
        }
        layers.push(track.clone());
    }
    let names = secs.iter().map(|&si| bp.form[si].name.as_str()).collect::<Vec<_>>().join(", ");
    let pitch = if p.kind.tonal() { format!("{}, ", moved(transpose, &bp.key)) } else { String::new() };
    let instead = if instead { ", in place of the song's bass there" } else { "" };
    let said = p.kind.said(p.phrase.bars);
    let place = bars_text(from_bar, p.phrase.bars);
    let line = format!(
        "{track}: {said} of {name} ({place}), {}, {pitch}at {:.0} BPM, all through {names}{instead}",
        p.about, bp.bpm
    );
    bp.lineage.push(format!("sampled {track}: {said} from {name}"));
    Ok((track, line))
}

/// A stab of the instruments kept (`Kind::Stab`), struck as a drum is: its hit a one-shot on a drum lane of its own
/// (`stab`) that strikes it on a few beats of every two bars, as a stab of a voice is struck (`STAB_RHYTHMS`), moved to
/// the song's key, high-passed out of the bass's way, all through `secs`; its rhythm is a drum lane's, to change in the
/// Drums tab. Its lane, and a line saying where it came from and went.
fn place_stab(
    bp: &mut Blueprint,
    h: &Heard,
    p: &Piece,
    fx: &[Effect],
    secs: &[usize],
    dir: &Path,
) -> Result<(String, String), String> {
    use std::collections::BTreeMap;
    let (a, name) = (h.a, h.name);
    let file = crate::take::store(&p.raw, dir).map_err(|e| format!("cannot keep the stab: {e:#}"))?;
    let lane = fresh(bp, "stab");
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let mut rng = crate::rng::Rng::seed_from(bp.seed, &format!("stabs {name} {}", p.phrase.start));
    let steps = *rng.pick(&STAB_RHYTHMS);
    let (hits, pattern_bars) = crate::script::steps_to_hits(steps, 16);
    let at = p.phrase.start;
    let when = format!("{}:{:04.1}", (at / 60.0) as u64, at % 60.0);
    bp.drums.push(DrumLane {
        name: lane.clone(),
        sound: Sound::Sample {
            path: file.to_string_lossy().into_owned(),
            root: 60,
            start: 0.0,
            end: None,
            label: format!("{name}, a stab of its chords at {when}"),
        },
        patterns: BTreeMap::from([("main".to_string(), hits)]),
        pattern_bars,
        pitch: transpose,
        choke: None,
        mix: TrackMix {
            fx: [Effect::Eq { bands: vec![EqBand { kind: BiquadKind::Highpass, freq: 120.0, gain_db: 0.0, q: 0.7 }] }]
                .into_iter()
                .chain(fx.iter().cloned())
                .collect(),
            reverb: 0.25,
            delay: 0.15,
            ..TrackMix::default()
        },
        length: None,
        speed: 1.0,
        swing: None,
        groove: None,
        groove_amount: 1.0,
        late_ms: 0.0,
        genre: None,
        tonal: true,
    });
    for &si in secs {
        bp.form[si].layers.push(lane.clone());
    }
    let names = secs.iter().map(|&si| bp.form[si].name.as_str()).collect::<Vec<_>>().join(", ");
    let line = format!(
        "{lane}: a stab of {name}'s chords at {when}, {}, struck as {steps}, {}, in {names} (a drum lane: its steps \
         change in the Drums tab)",
        p.about,
        moved(transpose, &bp.key)
    );
    bp.lineage.push(format!("sampled {lane}: a stab of the chords of {name}"));
    Ok((lane, line))
}

/// A piece kept chopped (`Keep::chopped`), as producers chop a record onto a sampler's pads (`kinds::chop`): cut at its
/// onsets, the hits that sound as one sharing a pad, each pad a drum lane of its own (`chop`, `chop2`, ...) striking it
/// where its hits came in the piece, as they came, so that the lanes play it as it was until they are played otherwise,
/// in the Drums tab or from a phone. Through its treatment when kept treated, all through `secs`, moved to the song's
/// key when it plays notes, at the song's tempo hit by hit with none of its hits stretched. Its lanes, and a line
/// saying where it came from and went.
fn place_chops(
    bp: &mut Blueprint,
    h: &Heard,
    k: &Keep,
    secs: &[usize],
    dir: &Path,
) -> Result<(Vec<String>, String), String> {
    use std::collections::BTreeMap;
    let (a, name, p) = (h.a, h.name, k.piece);
    let said = p.kind.said(p.phrase.bars);
    let x = if k.treated { &p.treated } else { &p.raw };
    let chopped = kinds::chop(x, p.phrase.bars, p.kind.tonal())
        .ok_or_else(|| format!("{said} of {name} holds too few hits to chop: keep it whole"))?;
    let transpose = match p.kind.tonal() {
        true => crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor())),
        false => 0,
    };
    // its pads go through the bus that what it was cut from plays through: a break the drums', a bass line the bass's,
    // the others the music's
    let bus = match p.kind {
        Kind::Break => "drums".to_string(),
        Kind::Bass => {
            bp.parts.iter().find(|q| q.role == Role::Bass).map_or_else(|| "bass".into(), |q| q.mix.bus.clone())
        }
        _ => "music".to_string(),
    };
    let place = bars_text(p.phrase.first + 1, p.phrase.bars);
    let mut lanes = Vec::new();
    for (i, (pad, hits)) in chopped.pads.iter().zip(&chopped.hits).enumerate() {
        let file = crate::take::store(pad, dir).map_err(|e| format!("cannot keep the chops: {e:#}"))?;
        let lane = fresh(bp, "chop");
        let hits: Vec<_> = hits.iter().map(|&(at, v)| crate::theory::rhythm::Hit::new(at, v)).collect();
        let label = format!("{name}, {place}, pad {}", i + 1);
        bp.drums.push(DrumLane {
            name: lane.clone(),
            sound: Sound::Sample { path: file.to_string_lossy().into_owned(), root: 60, start: 0.0, end: None, label },
            patterns: BTreeMap::from([("main".to_string(), hits)]),
            pattern_bars: p.phrase.bars,
            pitch: transpose,
            choke: None,
            mix: TrackMix { bus: bus.clone(), ..TrackMix::default() },
            length: None,
            speed: 1.0,
            // as it was played: straight, and none of the song's groove
            swing: Some(0.5),
            groove: None,
            groove_amount: 0.0,
            late_ms: 0.0,
            genre: None,
            tonal: p.kind.tonal(),
        });
        for &si in secs {
            bp.form[si].layers.push(lane.clone());
        }
        lanes.push(lane);
    }
    let names = secs.iter().map(|&si| bp.form[si].name.as_str()).collect::<Vec<_>>().join(", ");
    let pitch = if p.kind.tonal() { format!(", {}", moved(transpose, &bp.key)) } else { String::new() };
    let line = format!(
        "{}: {said} of {name} ({place}) chopped at its {} onsets onto {} pads, playing it as it was{pitch}, all through \
         {names} (drum lanes: play them otherwise in the Drums tab, or from a phone)",
        lanes.join(", "),
        chopped.onsets,
        lanes.len()
    );
    bp.lineage.push(format!("sampled {}: {said} from {name}, chopped", lanes.join(", ")));
    Ok((lanes, line))
}

/// The sections of `bp` a piece of `kind` plays all through: those asked for, by name or kind; else those where its
/// kind plays in the song (a break where the drums play, a bass line where the bass does), for a loop of the hook the
/// drops and hooks, for another passage the rest but the intro and the outro; any but those when none is.
fn sections_for(bp: &Blueprint, kind: Kind, asked: &[String]) -> Result<Vec<usize>, String> {
    use SectionKind::*;
    if !asked.is_empty() {
        return Ok(groups(bp, asked)?.into_iter().flat_map(|(_, secs)| secs).collect());
    }
    let drums: Vec<&str> = bp.drums.iter().map(|d| d.name.as_str()).collect();
    let basses: Vec<&str> = bp.parts.iter().filter(|p| p.role == Role::Bass).map(|p| p.name.as_str()).collect();
    let plays = |si: usize, names: &[&str]| bp.form[si].layers.iter().any(|l| names.contains(&l.as_str()));
    let of = |f: &dyn Fn(usize) -> bool| (0..bp.form.len()).filter(|&si| f(si)).collect::<Vec<usize>>();
    let secs = match kind {
        Kind::Break => of(&|si| plays(si, &drums)),
        Kind::Bass => of(&|si| plays(si, &basses)),
        Kind::Hook => of(&|si| matches!(bp.form[si].kind, Drop | Hook)),
        Kind::Stab => of(&|si| !matches!(bp.form[si].kind, Intro | Outro)),
        _ => of(&|si| !matches!(bp.form[si].kind, Drop | Hook | Intro | Outro)),
    };
    let between = of(&|si| !matches!(bp.form[si].kind, Intro | Outro));
    Ok([secs, between, (0..bp.form.len()).collect()].into_iter().find(|s| !s.is_empty()).unwrap_or_default())
}

/// The song's sections `secs` dealt out to `n` pieces of a kind, the least full first (`fullness`), from the quietest
/// up, as evenly as they go: the fullest piece the loudest of them, sections as loud as each other together where they
/// can be. With fewer sections than pieces, the fullest pieces have one each and the least full none.
fn dealt(bp: &Blueprint, secs: &[usize], n: usize) -> Vec<Vec<usize>> {
    let energy = |si: usize| bp.form[si].energy;
    let mut by = secs.to_vec();
    by.sort_by(|&p, &q| energy(p).total_cmp(&energy(q)).then(p.cmp(&q)));
    let (m, len) = (n.min(by.len()), by.len().max(1) as f64);
    // each section's place among them from the quietest, those as loud as each other at the middle of theirs
    let level = |k: usize| {
        let same: Vec<usize> = (0..by.len()).filter(|&j| (energy(by[j]) - energy(by[k])).abs() < 1e-6).collect();
        (same[0] + same[same.len() - 1]) as f64 / 2.0
    };
    // into as many shares as there are pieces to have one, the fullest of them
    let deal = |place: &dyn Fn(usize) -> f64| {
        let mut out = vec![Vec::new(); m];
        for (k, &si) in by.iter().enumerate() {
            out[((place(k) * m as f64 / len) as usize).min(m.max(1) - 1)].push(si);
        }
        out
    };
    let mut shares = deal(&level);
    if shares.iter().any(Vec::is_empty) {
        shares = deal(&|k| k as f64);
    }
    shares.iter_mut().for_each(|g| g.sort_unstable());
    let mut out = vec![Vec::new(); n - m];
    out.extend(shares);
    out
}

/// how full a piece sounds, 0 to 1: its root mean square over its peak, which a busy break or a moving bass line has
/// more of than a sparse or a steady one
fn fullness(x: &Stereo) -> f64 {
    let n = (2 * x.len()).max(1) as f64;
    let rms = (x.l.iter().chain(&x.r).map(|v| (*v as f64).powi(2)).sum::<f64>() / n).sqrt();
    rms / (x.peak() as f64).max(1e-9)
}

/// the section of the recording `a` heard that bar `bar` is in, by its kind and how many of its kind come up to it
/// ("hook 2"); empty where none is
fn section_at(a: &Analysis, bar: i64) -> String {
    let at = usize::try_from(bar).ok();
    let Some(s) = a.sections.iter().find(|s| at.is_some_and(|b| (s.start_bar..s.start_bar + s.bars).contains(&b)))
    else {
        return String::new();
    };
    let nth = a.sections.iter().filter(|o| o.kind == s.kind && o.start_bar <= s.start_bar).count();
    format!("{} {nth}", s.kind.name())
}

/// where a phrase was in the recording, in its bars counted from 1
fn bars_text(from: i64, bars: u32) -> String {
    if bars == 1 { format!("bar {from}") } else { format!("bars {from}-{}", from + bars as i64 - 1) }
}

fn plural(n: usize) -> String {
    if n == 1 { "a vocal phrase".into() } else { format!("{n} vocal phrases") }
}

/// Where phrases may go: groups of sections by what suits a vocal, the drops and hooks first, then the breaks, the
/// verses and grooves, the builds, intros and outros; or the sections asked for, by name or kind, as one group.
fn groups(bp: &Blueprint, asked: &[String]) -> Result<Vec<(String, Vec<usize>)>, String> {
    use SectionKind::*;
    if !asked.is_empty() {
        let mut idx = Vec::new();
        for a in asked {
            let hits: Vec<usize> =
                (0..bp.form.len()).filter(|&i| bp.form[i].name == *a || bp.form[i].kind.name() == a).collect();
            if hits.is_empty() {
                let names: Vec<&str> = bp.form.iter().map(|s| s.name.as_str()).collect();
                return Err(format!("no section or kind '{a}' (sections: {})", names.join(" ")));
            }
            idx.extend(hits);
        }
        idx.sort_unstable();
        idx.dedup();
        return Ok(vec![("sections asked for".into(), idx)]);
    }
    let order: [(&str, &[SectionKind]); 4] = [
        ("drops and hooks", &[Drop, Hook]),
        ("breaks", &[Break]),
        ("verses and grooves", &[Verse, Groove]),
        ("builds, intro and outro", &[Build, Intro, Outro]),
    ];
    Ok(order
        .iter()
        .map(|(label, kinds)| {
            (label.to_string(), (0..bp.form.len()).filter(|&i| kinds.contains(&bp.form[i].kind)).collect::<Vec<_>>())
        })
        .filter(|(_, idx)| !idx.is_empty())
        .collect())
}

/// The best `count` phrases of the recording: spans of whole bars as it plays them, scored by how much of them holds
/// a pitch in a voice's range (A2 to C6) and how much of their sound is the voice (a longer one a little more), and,
/// when the ears listen, by how much they sound like what they listen for; no two overlapping, the best first. With
/// its `stems` apart the pitch and the voice are its vocals', else the mix's, and the voice the middle of its stereo
/// picture.
pub fn find(
    src: &Stereo,
    stems: Option<&Stems>,
    a: &Analysis,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    count: usize,
) -> Vec<Phrase> {
    let mut out: Vec<Phrase> = Vec::new();
    for p in candidates(src, stems, a, ears, opt) {
        let (a0, a1) = (p.first, p.first + p.bars as i64);
        if out.iter().all(|q| a1 <= q.first || a0 >= q.first + q.bars as i64) {
            out.push(p);
        }
        if out.len() == count {
            break;
        }
    }
    out
}

/// a later take of the pieces draws among the phrases that score at most this much under the best, and the smaller
/// `CHOOSY`, the likelier the better of them
const NEAR_BEST: f64 = 1.2;
const CHOOSY: f64 = 0.6;

/// `pool`, the best first, drawn again for a later take: those that score within `NEAR_BEST` of the best, in an order
/// the better of which come first the likelier (Gumbel-top-k), whatever the ears made of the scores; one the take
/// before offered (`shown`) as though it scored `NEAR_BEST` less, so that the take offers others where there are any.
fn drawn(pool: Vec<Phrase>, shown: &dyn Fn(&Phrase) -> bool, rng: &mut crate::rng::Rng) -> Vec<Phrase> {
    let Some(best) = pool.first().map(|p| p.score) else { return pool };
    let mut keyed: Vec<(f64, Phrase)> = pool
        .into_iter()
        .filter(|p| p.score >= best - NEAR_BEST)
        .map(|p| {
            let less = if shown(&p) { NEAR_BEST } else { 0.0 };
            ((p.score - less - best) / CHOOSY - (-rng.range(1e-12, 1.0).ln()).ln(), p)
        })
        .collect();
    keyed.sort_by(|x, y| y.0.total_cmp(&x.0));
    keyed.into_iter().map(|(_, p)| p).collect()
}

/// A span `p` of what `sings` sings (`sung`) cut whole: from the quiet before its first word to the quiet after its
/// last word's release (`cuts`), and the bars of `a` that falls in, from the one its cut before falls in to the one its
/// cut after does. A bar more where its lead-in or its release crosses a bar line, for a bar line through a consonant
/// or a held note cuts it, and the bars before the recording's first or after its last where it is sung from its first
/// moment or to its last.
fn cut_whole(sings: &Stereo, sung: &cuts::Sung, a: &Analysis, p: &cuts::Span) -> (f64, f64, i64, i64) {
    let before = sung.words.iter().filter(|w| w.end <= p.start + 1e-9).map(|w| w.end).fold(0.0, f64::max);
    let after = sung.words.iter().filter(|w| w.start >= p.end - 1e-9).map(|w| w.start).fold(sings.secs(), f64::min);
    let from = cuts::cut_before(sings, p.start, before, bar_time(a, bar_of(a, p.start)));
    let to = cuts::cut_after(sings, p.end, after);
    let first = bar_of(a, from);
    (from, to, first, bar_of(a, to - 1e-6).max(first))
}

/// The fewest bars a phrase of `src` takes sung whole, cut as `sung_whole` cuts one and parted at every rest within it
/// of 120 ms at least; none where its voice sings none.
fn fewest(src: &Stereo, stems: Option<&Stems>, a: &Analysis) -> Option<u32> {
    let sings = stems.map_or(src, |s| &s.vocals);
    let sung = cuts::sung(sings, &melody::pitches(sings));
    let mut units = Vec::new();
    for p in &sung.phrases {
        let words: Vec<cuts::Span> = sung.words_of(*p).copied().collect();
        let mut start = p.start;
        for w in words.windows(2).filter(|w| w[1].start - w[0].end >= 0.12) {
            units.push(cuts::Span { start, end: w[0].end });
            start = w[1].start;
        }
        units.push(cuts::Span { start, end: p.end });
    }
    units
        .iter()
        .map(|u| {
            let (_, _, first, last) = cut_whole(sings, &sung, a, u);
            (last - first + 1) as u32
        })
        .min()
}

/// The phrases of `src` its voice sings whole (`cuts::sung`), the best first: each cut as `cut_whole` cuts it, in the
/// fewest whole bars that holds (1, 2, 4 or 8; `opt.bars` when said), silent where they run before the recording or
/// after it, scored as `candidates` scores a span (and a longer phrase a little more), and by the ears when they
/// listen. A phrase longer than that is parted at its longest rest within, of 120 ms at least, or left out where it
/// has none: a cut never falls inside a word or a held note.
fn sung_whole(
    src: &Stereo,
    stems: Option<&Stems>,
    a: &Analysis,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
) -> Vec<Phrase> {
    let sings = stems.map_or(src, |s| &s.vocals);
    let pitches = melody::pitches(sings);
    let sung = cuts::sung(sings, &pitches);
    let longest = opt.bars.unwrap_or(8);
    let cut = |p: &cuts::Span| cut_whole(sings, &sung, a, p);
    let mut spans = Vec::new();
    for p in &sung.phrases {
        let fits = |s: &cuts::Span| {
            let (_, _, first, last) = cut(s);
            (last - first + 1) as u32 <= longest
        };
        parted(&sung, *p, &fits, &mut spans);
    }
    let mut cands: Vec<Phrase> = Vec::new();
    for p in spans {
        let (from, to, first, last) = cut(&p);
        let need = (last - first + 1) as u32;
        let bars = match opt.bars {
            Some(_) => (longest >= need).then_some(longest),
            None => [1, 2, 4, 8].into_iter().find(|b| *b >= need),
        };
        let Some(bars) = bars else { continue };
        let (f0, f1) = ((from * 100.0) as usize, ((to * 100.0) as usize).min(pitches.len()));
        let held = pitches[f0.min(f1)..f1].iter().filter(|q| q.is_some_and(|m| (45.0..=84.0).contains(&m))).count();
        let sung_share = held as f64 / (f1 - f0).max(1) as f64;
        let (s0, s1) = ((from * SRF) as usize, ((to * SRF) as usize).min(src.len()));
        let (mut mid, mut side, mut voice) = (0f64, 0f64, 0f64);
        for i in s0..s1 {
            mid += ((src.l[i] + src.r[i]) as f64 * 0.5).powi(2);
            side += ((src.l[i] - src.r[i]) as f64 * 0.5).powi(2);
            voice += ((sings.l[i] + sings.r[i]) as f64 * 0.5).powi(2);
        }
        let centred = match stems {
            Some(_) => (voice / (mid + side).max(1e-12)).sqrt().min(1.0),
            None => mid.sqrt() / (mid.sqrt() + side.sqrt()).max(1e-12),
        };
        let score = 2.0 * sung_share + centred + 0.3 * (p.len() / 2.0).min(1.0);
        cands.push(Phrase {
            start: bar_time(a, first),
            first,
            bars,
            z: None,
            sung: sung_share,
            centred,
            score,
            within: Some((from, to)),
        });
    }
    let spans: Vec<(f64, f64)> = cands.iter().filter_map(|p| p.within).collect();
    if let Some(z) = heard(ears, sings, &spans, opt.find.as_deref().unwrap_or(DEFAULT_FIND)) {
        for (p, z) in cands.iter_mut().zip(z) {
            p.z = Some(z);
            p.score += z;
        }
    }
    cands.sort_by(|x, y| y.score.total_cmp(&x.score));
    cands
}

/// `p`, a phrase of `sung`, as phrases that each `fits`: itself, or its halves at its longest rest within (of 120 ms at
/// least), parted again as they need; none where it has no such rest
fn parted(sung: &cuts::Sung, p: cuts::Span, fits: &dyn Fn(&cuts::Span) -> bool, out: &mut Vec<cuts::Span>) {
    if fits(&p) {
        out.push(p);
        return;
    }
    let words: Vec<cuts::Span> = sung.words_of(p).copied().collect();
    let rest = words
        .windows(2)
        .filter(|w| w[1].start - w[0].end >= 0.12)
        .max_by(|x, y| (x[1].start - x[0].end).total_cmp(&(y[1].start - y[0].end)));
    if let Some(w) = rest {
        let (end, start) = (w[0].end, w[1].start);
        parted(sung, cuts::Span { start: p.start, end }, fits, out);
        parted(sung, cuts::Span { start, end: p.end }, fits, out);
    }
}

/// Every span `find` scores, the best first (the 64 best, when the ears listen), overlapping as they may.
fn candidates(
    src: &Stereo,
    stems: Option<&Stems>,
    a: &Analysis,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
) -> Vec<Phrase> {
    let secs = src.len() as f64 / SRF;
    let lens: Vec<u32> = opt.bars.map_or_else(|| vec![1, 2], |b| vec![b]);
    let sings = stems.map_or(src, |s| &s.vocals);
    let pitches = melody::pitches(sings);
    let mut cands: Vec<Phrase> = Vec::new();
    for &bars in &lens {
        // the phrases start on the bars the recording plays, its own beats and not a grid of one tempo
        for first in 0.. {
            let (t, end) = (a.bar_start(first), a.bar_start(first + bars as usize));
            if end > secs + 1e-6 {
                break;
            }
            if t < 0.0 {
                continue;
            }
            let len = end - t;
            let (f0, f1) = ((t * 100.0) as usize, (((t + len) * 100.0) as usize).min(pitches.len()));
            let sung = if f1 > f0 {
                pitches[f0..f1].iter().filter(|p| p.is_some_and(|m| (45.0..=84.0).contains(&m))).count() as f64
                    / (f1 - f0) as f64
            } else {
                0.0
            };
            let (s0, s1) = ((t * SRF) as usize, (((t + len) * SRF) as usize).min(src.len()));
            let (mut mid, mut side, mut voice) = (0f64, 0f64, 0f64);
            for i in s0..s1 {
                mid += ((src.l[i] + src.r[i]) as f64 * 0.5).powi(2);
                side += ((src.l[i] - src.r[i]) as f64 * 0.5).powi(2);
                voice += ((sings.l[i] + sings.r[i]) as f64 * 0.5).powi(2);
            }
            let quiet = mid / (s1 - s0).max(1) as f64 <= 1e-6 || voice / (s1 - s0).max(1) as f64 <= 1e-7;
            let centred = match (quiet, stems) {
                (true, _) => 0.0,
                (false, Some(_)) => (voice / (mid + side)).sqrt().min(1.0),
                (false, None) => mid.sqrt() / (mid.sqrt() + side.sqrt()),
            };
            if !quiet {
                // a longer phrase that sounds as sung says more
                let score = 2.0 * sung + centred + 0.02 * (bars - 1) as f64;
                cands.push(Phrase { start: t, first: first as i64, bars, z: None, sung, centred, score, within: None });
            }
        }
    }
    let end = |p: &Phrase| bar_time(a, p.first + p.bars as i64);
    // the ears listen to the spans that sound most sung, and their reading leads
    cands.sort_by(|x, y| y.score.total_cmp(&x.score));
    cands.truncate(64);
    let spans: Vec<(f64, f64)> = cands.iter().map(|p| (p.start, end(p))).collect();
    if let Some(z) = heard(ears, sings, &spans, opt.find.as_deref().unwrap_or(DEFAULT_FIND)) {
        for (p, z) in cands.iter_mut().zip(z) {
            p.z = Some(z);
            p.score += z;
        }
        cands.sort_by(|x, y| y.score.total_cmp(&x.score));
    }
    cands
}

/// the ears' z for `query` over each span, when they listen
#[cfg(feature = "ears")]
fn heard(ears: Option<&crate::ears::Ears>, src: &Stereo, spans: &[(f64, f64)], query: &str) -> Option<Vec<f64>> {
    let e = ears?;
    if spans.is_empty() {
        return None;
    }
    let mono = crate::ears::Ears::mono(src);
    let qe = e.inner.embed_text(&[query]).ok()?.pop()?;
    let cands: Vec<earshot::search::Cand> = spans.iter().map(|&(a, b)| (a, b, "span", 0.0)).collect();
    e.inner.score_spans(&mono, &cands, &qe).ok().map(|(z, _)| z)
}

#[cfg(not(feature = "ears"))]
fn heard(_: Option<&crate::ears::Ears>, _: &Stereo, _: &[(f64, f64)], _: &str) -> Option<Vec<f64>> {
    None
}

/// A phrase as a sample: the middle of the stereo picture, where a voice sits; its sustained part, without the drums
/// (`fft::harmonic`); above 160 Hz and below 11 kHz; its loudest moment at -1 dBFS, faded in and out.
pub fn clean(x: &Stereo) -> Stereo {
    let mid: Vec<f32> = x.l.iter().zip(&x.r).map(|(l, r)| 0.5 * (l + r)).collect();
    let mut v = fft::harmonic(&mid);
    filter::butter(&mut v, true, 160.0, SRF, 4);
    filter::butter(&mut v, false, 11_000.0, SRF, 4);
    let peak = v.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        let g = 0.891 / peak;
        v.iter_mut().for_each(|s| *s *= g);
    }
    let mut out = Stereo { l: v.clone(), r: v };
    let n = out.len();
    out.fade(((0.005 * SRF) as usize).min(n / 4), ((0.03 * SRF) as usize).min(n / 4));
    out
}

/// A phrase of a recording's vocals, apart from the rest (`crate::stems`), as a sample: in stereo as it was sung and
/// mixed, its rumble under 90 Hz out, its loudest moment at -1 dBFS, faded in and out.
pub fn voice(x: &Stereo) -> Stereo {
    let mut out = x.clone();
    for ch in out.channels_mut() {
        filter::butter(ch, true, 90.0, SRF, 2);
    }
    let peak = out.peak();
    if peak > 0.0 {
        let g = 0.891 / peak;
        for ch in out.channels_mut() {
            ch.iter_mut().for_each(|s| *s *= g);
        }
    }
    let n = out.len();
    out.fade(((0.005 * SRF) as usize).min(n / 4), ((0.03 * SRF) as usize).min(n / 4));
    out
}

/// A passage of a recording as it plays, when its voice is not apart: in stereo, its lows under 150 Hz out (where the
/// song's bass and kick play), its loudest moment at -1 dBFS, faded in and out.
pub fn whole(x: &Stereo) -> Stereo {
    let mut out = x.clone();
    for ch in out.channels_mut() {
        filter::butter(ch, true, 150.0, SRF, 4);
    }
    let peak = out.peak();
    if peak > 0.0 {
        let g = 0.891 / peak;
        out.channels_mut().into_iter().for_each(|ch| ch.iter_mut().for_each(|s| *s *= g));
    }
    let n = out.len();
    out.fade(((0.005 * SRF) as usize).min(n / 4), ((0.03 * SRF) as usize).min(n / 4));
    out
}

/// the middle of a recording's stereo picture, where a voice is mixed, above 200 Hz
fn middle(x: &Stereo) -> Stereo {
    let mut mid: Vec<f32> = x.l.iter().zip(&x.r).map(|(l, r)| 0.5 * (l + r)).collect();
    filter::butter(&mut mid, true, 200.0, SRF, 2);
    Stereo { l: mid.clone(), r: mid }
}

/// a passage of a whole record mixed under the song: as a sample, its lows out again in the mix, a little compressed
fn record_mix() -> TrackMix {
    TrackMix {
        level_db: -2.0,
        fx: vec![
            Effect::Eq { bands: vec![EqBand { kind: BiquadKind::Highpass, freq: 150.0, gain_db: 0.0, q: 0.7 }] },
            Effect::Compressor {
                threshold_db: -18.0,
                ratio: 2.0,
                attack_ms: 10.0,
                release_ms: 120.0,
                knee_db: 6.0,
                makeup_db: 1.0,
                mix: 1.0.into(),
            },
        ],
        reverb: 0.1,
        duck: 0.3,
        ..TrackMix::default()
    }
}

/// the phrase after `lead` seconds of silence, in `period` seconds: what a loop plays round
fn layout(cut: &Stereo, lead: f64, period: f64) -> Stereo {
    let mut out = Stereo::zeros((period * SRF).round() as usize);
    let at = (lead * SRF).round() as usize;
    for i in 0..cut.len().min(out.len().saturating_sub(at)) {
        out.l[at + i] = cut.l[i];
        out.r[at + i] = cut.r[i];
    }
    out
}

/// how much of each pitch class the phrase holds, its loudest 1
fn pitch_profile(x: &Stereo) -> [f32; 12] {
    let mono: Vec<f32> = x.l.iter().zip(&x.r).map(|(l, r)| 0.5 * (l + r)).collect();
    let mut sum = [0f32; 12];
    for frame in chroma::chroma(&mono, 2048) {
        for (s, v) in sum.iter_mut().zip(frame) {
            *s += v;
        }
    }
    let top = sum.iter().fold(0f32, |m, v| m.max(*v));
    if top > 0.0 {
        sum.iter_mut().for_each(|v| *v /= top);
    }
    sum
}

/// a pitch profile `semis` semitones up
fn shift(p: &[f32; 12], semis: i32) -> [f32; 12] {
    let mut out = [0f32; 12];
    for (pc, v) in p.iter().enumerate() {
        out[(pc as i32 + semis).rem_euclid(12) as usize] = *v;
    }
    out
}

/// How well a phrase of `bars` bars with pitch profile `profile` agrees with the chords under it, playing from
/// `offset` bars into each of the sections `secs` and every `period` bars after: the profile's cosine with the chord
/// tones under each playing, weighted by how long each chord lasts, averaged over every playing; none when no section
/// has room for it.
fn fit(p: &Project, secs: &[usize], offset: u32, bars: u32, period: u32, profile: &[f32; 12]) -> Option<f64> {
    let (mut sum, mut n) = (0f64, 0usize);
    for &si in secs {
        let Some(s) = p.sections.get(si) else { continue };
        let mut b = offset;
        while b + bars <= s.bars {
            let (t0, t1) = ((s.start_bar + b) as f64 * 4.0, (s.start_bar + b + bars) as f64 * 4.0);
            let mut tones = [0f32; 12];
            for c in p.chords.iter().filter(|c| c.start < t1 && c.start + c.len > t0) {
                let held = ((c.start + c.len).min(t1) - c.start.max(t0)) as f32;
                for pc in c.chord.pcs() {
                    tones[pc as usize] += held;
                }
            }
            sum += cosine(profile, &tones);
            n += 1;
            b += period.max(1);
        }
    }
    (n > 0).then(|| sum / n as f64)
}

fn cosine(a: &[f32; 12], b: &[f32; 12]) -> f64 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32; 12]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let d = norm(a) * norm(b);
    if d > 0.0 { (dot / d) as f64 } else { 0.0 }
}

/// a sampled phrase mixed as a vocal: above 200 Hz, a little presence, compressed, with some reverb and delay, ducked
/// a little under the kick
pub(crate) fn vocal_mix() -> TrackMix {
    TrackMix {
        level_db: -1.0,
        fx: vec![
            Effect::Eq {
                bands: vec![
                    EqBand { kind: BiquadKind::Highpass, freq: 200.0, gain_db: 0.0, q: 0.7 },
                    EqBand { kind: BiquadKind::Peak, freq: 3000.0, gain_db: 1.5, q: 1.0 },
                ],
            },
            Effect::Compressor {
                threshold_db: -20.0,
                ratio: 3.0,
                attack_ms: 5.0,
                release_ms: 80.0,
                knee_db: 4.0,
                makeup_db: 2.0,
                mix: 1.0.into(),
            },
        ],
        reverb: 0.25,
        delay: 0.15,
        duck: 0.35,
        ..TrackMix::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::{Chord, Quality};

    #[test]
    fn a_phrase_fits_where_its_notes_are_the_chords_tones() {
        // two sections of 8 bars: A minor for four bars, then F major
        let am = Chord::new(9, Quality::Min);
        let f = Chord::new(5, Quality::Maj);
        let mut p = crate::compose::arrange(&crate::script::blank("house").unwrap());
        p.sections = vec![
            SectionInfo { name: "drop".into(), kind: SectionKind::Drop, start_bar: 0, bars: 8, energy: 1.0 },
            SectionInfo { name: "drop2".into(), kind: SectionKind::Drop, start_bar: 8, bars: 8, energy: 1.0 },
        ];
        p.chords = (0..4)
            .map(|k| crate::theory::ChordSpan {
                start: k as f64 * 16.0,
                len: 16.0,
                chord: if k % 2 == 0 { am } else { f },
            })
            .collect();
        // a phrase on A, C and E fits the A minor bars, and is half as good every 2 bars across both chords
        let mut amin = [0f32; 12];
        for pc in [9, 0, 4] {
            amin[pc] = 1.0;
        }
        assert!(fit(&p, &[0], 0, 4, 8, &amin).unwrap() > 0.99, "on A minor every time");
        let across = fit(&p, &[0, 1], 0, 2, 2, &amin).unwrap();
        assert!(across > 0.5 && across < 0.9, "{across}");
        assert!(fit(&p, &[0], 4, 4, 8, &amin) < fit(&p, &[0], 0, 4, 8, &amin), "on F major it fits less");
        assert_eq!(fit(&p, &[0], 8, 4, 16, &amin), None, "no room 8 bars into 8 bars");
        assert_eq!(shift(&amin, 3)[0], 1.0, "A minor three up is C minor: C is there");
    }

    #[test]
    fn a_phrase_waits_its_turn_in_its_loop() {
        let cut = Stereo { l: vec![0.5; 4_800], r: vec![0.5; 4_800] };
        let lay = layout(&cut, 0.25, 0.5);
        assert_eq!(lay.len(), 24_000);
        assert!(lay.l[..12_000].iter().all(|&v| v == 0.0), "silence before it");
        assert!(lay.l[12_000..16_800].iter().all(|&v| v == 0.5) && lay.l[16_800..].iter().all(|&v| v == 0.0));
    }

    /// 120 BPM, eight bars: a noisy hat to the left all through, and in the middle a hum from second `from` to `to`
    fn hummed(from: usize, to: usize) -> Stereo {
        let sr = SRF as usize;
        let n = 16 * sr;
        let mut x = Stereo::zeros(n);
        let mut rng = crate::rng::Rng::new(3);
        let mut phase = 0f64;
        for i in 0..n {
            if i % (sr / 4) < sr / 50 {
                x.l[i] += (rng.f32() - 0.5) * 0.6;
            }
            if (from * sr..to * sr).contains(&i) {
                // a sung A3 with a little vibrato, its second harmonic too
                let t = i as f64 / SRF;
                phase += std::f64::consts::TAU * 220.0 * (1.0 + 0.01 * (t * 5.0 * std::f64::consts::TAU).sin()) / SRF;
                let v = (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15) as f32;
                x.l[i] += v;
                x.r[i] += v;
            }
        }
        x
    }

    #[test]
    fn a_sung_line_in_the_middle_is_found_and_the_drums_to_the_sides_are_not() {
        // a hum in bars 3-4
        let (sr, x) = (SRF as usize, hummed(4, 8));
        let a = Analysis {
            bpm: 120.0,
            first_downbeat: 0.0,
            bar_secs: 2.0,
            bar_starts: Vec::new(),
            ..analysis::analyze(&x, Some((110.0, 130.0)))
        };
        let found = find(&x, None, &a, None, &Options { bars: Some(2), ..Options::default() }, 1);
        assert_eq!(found.len(), 1);
        assert!((found[0].start - 4.0).abs() < 1e-9, "{:?}", found);
        assert!(found[0].sung > 0.8 && found[0].centred > 0.6, "{:?}", found);
        let clean = clean(&x.slice(4 * sr, 8 * sr));
        assert!((clean.peak() - 0.891).abs() < 0.01 && clean.l == clean.r);
    }

    #[test]
    fn a_phrase_plays_only_where_it_has_room_and_a_lead_steps_back_once() {
        let g = crate::genres::get("trip_hop").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = crate::compose::generate(&g, 1, &o, None);
        let lead = bp.parts.iter().find(|p| p.role == Role::Lead).expect("trip-hop has a lead").name.clone();
        // a hook of 4 bars and one of 2: a phrase of 2 bars answering from bar 3 has room only in the first
        let mut hook = bp.form[0].clone();
        (hook.name, hook.kind, hook.bars, hook.layers) = ("hook".into(), SectionKind::Drop, 4, bp.layer_names());
        let short = SectionSpec { name: "hook2".into(), bars: 2, ..hook.clone() };
        bp.form = vec![hook, short];
        let level = |bp: &Blueprint| bp.parts.iter().find(|p| p.name == lead).unwrap().mix.level_db;
        let on = |bp: &Blueprint, track: &str| -> Vec<String> {
            bp.form.iter().filter(|s| s.layers.iter().any(|l| l == track)).map(|s| s.name.clone()).collect()
        };
        let (was, src) = (level(&bp), hummed(4, 12));
        let dir = std::env::temp_dir().join(format!("claudius-sampler-{}", std::process::id()));
        let ask =
            |sections: &str| Options { count: 2, bars: Some(2), sections: vec![sections.into()], ..Options::default() };
        let out = sample(&mut bp, &src, "song", None, &ask("drop"), &dir).unwrap();
        assert_eq!(out.tracks, ["vox1", "vox2"], "{:?}", out.lines);
        assert!(out.lines.iter().any(|l| l.contains("answering")), "{:?}", out.lines);
        for (track, line) in out.tracks.iter().zip(&out.lines) {
            let room: &[&str] = if line.contains("answering") { &["hook"] } else { &["hook", "hook2"] };
            assert_eq!(on(&bp, track), room, "{line}");
        }
        assert_eq!(level(&bp), was - 1.5);
        // the short hook alone has room for one phrase, from its first bar; the lead stepped back for the first ones
        let out = sample(&mut bp, &src, "song", None, &ask("hook2"), &dir).unwrap();
        assert_eq!(out.tracks, ["vox3"], "{:?}", out.lines);
        assert_eq!(level(&bp), was - 1.5, "the lead steps back once");
        assert!(bp.loops.iter().all(|l| l.sampled && l.kept() && l.from.is_none()));
        // phrases of 4 bars have no room in it at all
        let four = Options { bars: Some(4), ..ask("hook2") };
        let err = sample(&mut bp, &src, "song", None, &four, &dir).unwrap_err();
        assert!(err.contains("no section asked for has room for a phrase of 4 bars"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// a short trip-hop song of an intro and a drop of 8 bars each
    fn song() -> Blueprint {
        let g = crate::genres::get("trip_hop").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = crate::compose::generate(&g, 1, &o, None);
        let mut drop = bp.form[0].clone();
        (drop.name, drop.kind, drop.bars, drop.layers) = ("drop".into(), SectionKind::Drop, 8, bp.layer_names());
        let intro = SectionSpec { name: "intro".into(), kind: SectionKind::Intro, ..drop.clone() };
        bp.form = vec![intro, drop];
        bp
    }

    /// 120 BPM, eight bars: a noisy hat to the left all through, and from second 4 to 12 a sung "ah" on every beat,
    /// 0.3 s long, in the middle
    fn syllables() -> Stereo {
        let sr = SRF as usize;
        let mut x = hummed(0, 0);
        for beat in 8..24 {
            let at = beat * sr / 2;
            for i in 0..(0.3 * SRF) as usize {
                let t = i as f64 / SRF;
                let v = ((std::f64::consts::TAU * 220.0 * t).sin() * 0.4 * (1.0 - t / 0.3)) as f32;
                x.l[at + i] += v;
                x.r[at + i] += v;
            }
        }
        x
    }

    fn on(bp: &Blueprint, track: &str) -> Vec<String> {
        bp.form.iter().filter(|s| s.layers.iter().any(|l| l == track)).map(|s| s.name.clone()).collect()
    }

    #[test]
    fn how_often_a_phrase_comes_round_is_asked_or_suits_the_genre_and_what_its_sections_play() {
        let mut bp = song();
        let drop = bp.form.iter().position(|s| s.kind == SectionKind::Drop).unwrap();
        let (secs, lead) = (vec![drop], bp.parts.iter().find(|p| p.role == Role::Lead).unwrap().name.clone());
        assert_eq!(Every::parse(" Once "), Ok(Every::Once));
        assert_eq!((Every::parse("8"), Every::parse("through")), (Ok(Every::Bars(8)), Ok(Every::Through)));
        assert!(Every::parse("3").unwrap_err().starts_with("no '3' for how often a phrase plays"));
        // as asked: once in a section of 8 bars, every 4 bars (never shorter than the phrase), all through
        let asked = |every, bars| period(&bp, &secs, bars, Some(every), &[]);
        assert_eq!(
            (asked(Every::Once, 2).bars, asked(Every::Bars(4), 2).bars, asked(Every::Through, 2).bars),
            (8, 4, 2)
        );
        assert_eq!(asked(Every::Bars(4), 8).bars, 8);
        assert!(asked(Every::Once, 2).why.is_none(), "asked, it says no reason");
        // as it suits: once from the start of each section where the lead sings
        let sung: Vec<usize> = (0..bp.form.len()).filter(|&si| sings_in(&bp, si)).collect();
        assert!(bp.form[drop].layers.contains(&lead) && sung.contains(&drop), "the lead sings in the drop");
        let r = period(&bp, &secs, 2, None, &sung);
        assert_eq!((r.every, r.why.as_deref()), (Every::Once, Some("as a lead or a voice sings there already")));
        // where nothing sings: every other time round in trip-hop, built on samples; every 8 bars in house, not
        bp.form[drop].layers.retain(|l| *l != lead);
        assert!(!sings_in(&bp, drop));
        // a take mixed as a lead, as add_take mixes one, is a melody there too; a take of drums is none
        let take = |name: &str, role: &str| LoopPart {
            name: name.into(),
            role: role.into(),
            sound: Sound::Loop {
                path: "take.flac".into(),
                bpm: 120.0,
                beats: 16.0,
                key: None,
                tonal: true,
                label: name.into(),
            },
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix: TrackMix::default(),
            from: Some("drop".into()),
            sampled: false,
        };
        bp.loops.extend([take("beat1", "drums"), take("take1", "lead")]);
        bp.form[drop].layers.push("beat1".into());
        assert!(!sings_in(&bp, drop), "a take of drums sings nothing");
        bp.form[drop].layers.push("take1".into());
        assert!(sings_in(&bp, drop), "a take mixed as a lead sings");
        bp.form[drop].layers.retain(|l| l != "beat1" && l != "take1");
        let r = period(&bp, &secs, 2, None, &[]);
        assert_eq!((r.every, r.bars, r.why.as_deref()), (Every::Bars(4), 4, Some("as trip hop is built on samples")));
        bp.genre = "house".into();
        let r = period(&bp, &secs, 2, None, &[]);
        let why = Some("now and then, as house is not built on samples");
        assert_eq!((r.every, r.bars, r.why.as_deref()), (Every::Bars(8), 8, why));
    }

    #[test]
    fn each_way_has_a_name_and_the_lengths_it_takes() {
        for h in How::ALL {
            assert_eq!(How::parse(h.name()), Ok(h));
        }
        assert_eq!(How::parse(" Chops "), Ok(How::Chops));
        let err = How::parse("slices").unwrap_err();
        assert!(err.contains("no way 'slices'") && err.contains("phrases, passage, chops, stabs"), "{err}");
        let (bp, src) = (song(), hummed(4, 12));
        let ask = |how, bars| Options { how, bars: Some(bars), ..Options::default() };
        assert_eq!(checked(&bp, &src, &ask(How::Passage, 2)).unwrap_err(), "a passage is 4, 8 or 16 bars, not 2");
        assert_eq!(checked(&bp, &src, &ask(How::Phrases, 8)).unwrap_err(), "a phrase is 1, 2 or 4 bars, not 8");
        assert!(checked(&bp, &src, &ask(How::Passage, 16)).is_ok() && checked(&bp, &src, &ask(How::Stabs, 3)).is_ok());
    }

    #[test]
    fn a_passage_plays_through_the_drops_on_its_own_chords_in_the_songs_key() {
        use crate::theory::scale::{Key, Mode};
        let src = hummed(2, 16);
        // eight bars of one section, its chords played
        let section = |harmony| analysis::SectionGuess {
            start_bar: 0,
            bars: 8,
            energy: 1.0,
            kind: SectionKind::Verse,
            kick: false,
            bass: false,
            hats: false,
            harmony,
        };
        let a = Analysis {
            bpm: 120.0,
            first_downbeat: 0.0,
            bar_secs: 2.0,
            bar_starts: Vec::new(),
            key: Key::new(9, Mode::Minor),
            chords: ["Am", "F", "C", "G", "Am", "F", "C", "G"].map(String::from).to_vec(),
            sections: vec![section(true)],
            ..analysis::analyze(&src, Some((110.0, 130.0)))
        };
        let dir = std::env::temp_dir().join(format!("claudius-passage-{}", std::process::id()));
        let opt = Options { how: How::Passage, bars: Some(4), ..Options::default() };
        // up five semitones, Am F C G is Dm Bb F C: i VI III VII in D minor, vi IV I V in F major, its relative
        for (key, numerals) in
            [(Key::new(2, Mode::Minor), ["i", "VI", "III", "VII"]), (Key::new(5, Mode::Major), ["vi", "IV", "I", "V"])]
        {
            let mut bp = song();
            (bp.key, bp.harmony.bars_per_chord) = (key, 1);
            let out =
                sample_from(&mut bp, &Heard { src: &src, stems: None, a: &a, name: "song" }, None, &opt, &dir).unwrap();
            assert_eq!(out.tracks, ["passage"], "{:?}", out.lines);
            assert!(out.said("song").starts_with("sampled a passage of song into the song: passage"));
            let lp = bp.loops.iter().find(|l| l.name == "passage").unwrap();
            assert!(matches!(lp.sound, Sound::Loop { beats: 16.0, .. }) && lp.role == "sample" && lp.transpose == 5);
            assert_eq!(on(&bp, "passage"), ["drop"], "the drops before the intro");
            assert_eq!(bp.harmony.progressions[&bp.form[1].prog], numerals, "{:?}", out.lines);
            assert!(out.lines[0].contains("bars 5-8 of song"), "where 4 bars of it start: {:?}", out.lines);
            assert!(out.lines[0].contains("following it"), "{:?}", out.lines);
        }
        // a chord not heard leaves the song's own, as does a voice with no chords played under it; 16 bars have no
        // room in the drop of 8
        let unheard = Analysis { chords: vec!["Am".into(); 3], ..a.clone() };
        let mut bp = song();
        let out = sample_from(&mut bp, &Heard { src: &src, stems: None, a: &unheard, name: "song" }, None, &opt, &dir)
            .unwrap();
        assert!(out.lines[0].contains("were not all heard") && bp.form[1].prog != "sampled", "{:?}", out.lines);
        let sung = Analysis { sections: vec![section(false)], ..a.clone() };
        let mut bp = song();
        let out =
            sample_from(&mut bp, &Heard { src: &src, stems: None, a: &sung, name: "song" }, None, &opt, &dir).unwrap();
        assert!(out.lines[0].contains("plays none there besides the voice") && bp.form[1].prog != "sampled");
        // a voice apart with nothing under it is an a cappella: its voice, as a vocal, on the song's own chords
        let silent = Stereo::zeros(src.len());
        let alone = Stems { vocals: src.clone(), drums: silent.clone(), bass: silent.clone(), other: silent };
        let mut bp = song();
        let heard = Heard { src: &src, stems: Some(&alone), a: &a, name: "song" };
        let out = sample_from(&mut bp, &heard, None, &opt, &dir).unwrap();
        let lp = bp.loops.iter().find(|l| l.name == "passage").unwrap();
        assert!(lp.role == "vocal" && out.lines[0].contains("song, its voice"), "{:?}", out.lines);
        assert!(out.lines[0].contains("plays none there besides the voice") && bp.form[1].prog != "sampled");
        let long = Options { bars: Some(16), ..opt.clone() };
        let err = sample_from(&mut bp, &Heard { src: &src, stems: None, a: &a, name: "song" }, None, &long, &dir);
        assert_eq!(err.unwrap_err(), "the recording holds 8 bars, fewer than a passage of 16: ask for a shorter one");
        let mut tight = song();
        tight.form[1].bars = 4;
        let eight = Options { bars: Some(8), sections: vec!["drop".into()], ..opt.clone() };
        let r = sample_from(&mut tight, &Heard { src: &src, stems: None, a: &a, name: "song" }, None, &eight, &dir);
        assert!(r.unwrap_err().contains("no section asked for has room for a passage of 8 bars"));
        // none asked for: 8 bars where the recording and a section hold them, else 4
        let six = src.slice(0, 12 * SRF as usize);
        let unsaid = Options { bars: None, ..opt.clone() };
        let mut bp = song();
        let out = sample_from(&mut bp, &Heard { src: &six, stems: None, a: &a, name: "song" }, None, &unsaid, &dir);
        let lp = bp.loops.iter().find(|l| l.name == "passage").expect("a passage of 4 bars");
        assert!(matches!(lp.sound, Sound::Loop { beats: 16.0, .. }), "{:?}", out);
        let mut bp = song();
        sample_from(&mut bp, &Heard { src: &src, stems: None, a: &a, name: "song" }, None, &unsaid, &dir).unwrap();
        let lp = bp.loops.iter().find(|l| l.name == "passage").unwrap();
        assert!(matches!(lp.sound, Sound::Loop { beats: 32.0, .. }), "{:?}", lp.sound);
        // a drop of 4 bars takes 4 of them before the intro takes 8
        let mut short_drop = song();
        short_drop.form[1].bars = 4;
        let heard = Heard { src: &src, stems: None, a: &a, name: "song" };
        sample_from(&mut short_drop, &heard, None, &unsaid, &dir).unwrap();
        let lp = short_drop.loops.iter().find(|l| l.name == "passage").unwrap();
        assert!(matches!(lp.sound, Sound::Loop { beats: 16.0, .. }), "{:?}", lp.sound);
        assert_eq!(on(&short_drop, "passage"), ["drop"]);
        let three = src.slice(0, 6 * SRF as usize);
        let err = sample_from(&mut bp, &Heard { src: &three, stems: None, a: &a, name: "song" }, None, &unsaid, &dir);
        assert_eq!(
            err.unwrap_err(),
            "the recording holds 3 bars, fewer than a passage of 4: sample its phrases instead"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chops_play_slices_of_the_voice_in_a_new_rhythm_with_rests() {
        let (mut bp, src) = (song(), hummed(4, 12));
        let dir = std::env::temp_dir().join(format!("claudius-chops-{}", std::process::id()));
        let opt = Options { how: How::Chops, ..Options::default() };
        let out = sample(&mut bp, &src, "song", None, &opt, &dir).unwrap();
        assert_eq!(out.tracks, ["chops"], "{:?}", out.lines);
        assert!(out.said("song").starts_with("chopped song into the song: chops"));
        let lp = bp.loops.iter().find(|l| l.name == "chops").unwrap();
        assert!(matches!(lp.sound, Sound::Loop { beats: 8.0, .. }) && lp.role == "vocal");
        assert_eq!(on(&bp, "chops"), ["drop"]);
        let Sound::Loop { path, .. } = &lp.sound else { unreachable!() };
        let audio = crate::audio::load(Path::new(path)).unwrap();
        let sixteenth = audio.len() / 32;
        assert!(sixteenth.abs_diff((0.125 * SRF) as usize) < 30, "two bars at about 120 BPM: {}", audio.len());
        let loud: Vec<f32> = (0..32).map(|k| rms(&audio, k * sixteenth, sixteenth)).collect();
        let rhythm = CHOP_RHYTHMS.iter().find(|r| out.lines[0].contains(*r)).expect("the line says its rhythm");
        for (k, c) in rhythm.chars().enumerate() {
            if c == 'x' {
                assert!(loud[k] > 0.05 && loud[16 + k] > 0.05, "a slice on step {k}: {loud:?}");
            }
        }
        assert!(loud.iter().filter(|&&v| v < 1e-3).count() >= 4, "rests between the slices: {loud:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stab_is_a_syllable_struck_on_a_few_beats_by_a_drum_lane_of_its_own() {
        let (mut bp, src) = (song(), syllables());
        let dir = std::env::temp_dir().join(format!("claudius-stab-{}", std::process::id()));
        let opt = Options { how: How::Stabs, ..Options::default() };
        let out = sample(&mut bp, &src, "song", None, &opt, &dir).unwrap();
        assert_eq!(out.tracks, ["vox_stab"], "{:?}", out.lines);
        let lane = bp.drums.iter().find(|d| d.name == "vox_stab").unwrap();
        assert_eq!(lane.role(), "perc", "a percussion's hit, which plays out whole rather than for a sixteenth");
        let hits = &lane.patterns["main"];
        assert!(lane.pattern_bars == 2 && (2..=4).contains(&hits.len()), "{hits:?}");
        assert_eq!(on(&bp, "vox_stab"), ["drop"], "the drops, and the verses had there been any");
        let Sound::Sample { path, label, .. } = &lane.sound else { panic!("{:?}", lane.sound) };
        assert!(label.starts_with("song, a stab at 0:"), "{label}");
        let shot = crate::audio::load(Path::new(path)).unwrap();
        let secs = shot.len() as f64 / SRF;
        assert!((0.2..=0.36).contains(&secs), "about the syllable's 0.3 s: {secs}");
        // a second stab is another lane; a hum held all through has no attack to strike
        let out = sample(&mut bp, &src, "song", None, &opt, &dir).unwrap();
        assert_eq!(out.tracks, ["vox_stab2"]);
        let err = sample(&mut bp, &hummed(0, 16), "song", None, &opt, &dir).unwrap_err();
        assert!(err.starts_with("nothing in the recording strikes like a syllable"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tone_held_from_the_first_moment_with_no_beat_is_a_piece_too() {
        let tone = |i: usize| 0.25 * (std::f64::consts::TAU * 440.0 * i as f64 / SRF).sin() as f32;
        let l: Vec<f32> = (0..(6.0 * SRF) as usize).map(tone).collect();
        let x = Stereo { r: l.clone(), l };
        let pieces = pieces(&x, None, &Options { count: 1, kinds: vec![Kind::Phrase], ..Options::default() }, 0)
            .expect("a piece");
        let p = &pieces[0];
        let (from, to) = p.phrase.within.expect("sung whole");
        assert!(from <= 0.01 && to >= 5.99, "all of it: {from}..{to}");
        // the bars before the recording's first and after its last are silent: as cut, the tone sounds all through
        // its six seconds and no longer
        let (one, all) = (|v: &f32| v.abs() > 0.01, p.raw.l.iter().filter(|v| v.abs() > 0.01).count());
        let first = p.raw.l.iter().position(one).unwrap();
        assert!((all as f64 / SRF - 6.0).abs() < 0.2, "{} s of it sound", all as f64 / SRF);
        assert!(p.place().starts_with("bars 0-"), "the bar before the first is 0: {}", p.place());
        assert!(first as f64 / SRF > 0.3, "a pickup into the first bar: {first}");
    }

    #[test]
    fn a_phrase_too_long_for_eight_bars_says_how_many_it_takes() {
        // twenty seconds of a tone, with no rest in it to part it at
        let tone = |i: usize| 0.25 * (std::f64::consts::TAU * 440.0 * i as f64 / SRF).sin() as f32;
        let l: Vec<f32> = (0..(20.0 * SRF) as usize).map(tone).collect();
        let x = Stereo { r: l.clone(), l };
        let said =
            pieces(&x, None, &Options { count: 1, kinds: vec![Kind::Phrase], ..Options::default() }, 0).unwrap_err();
        assert!(said.starts_with("no phrase of it is sung whole in 8 bars or fewer: the shortest takes "), "{said}");
    }

    #[test]
    fn a_piece_is_as_loud_as_its_phrase_not_as_what_is_sung_round_it_in_its_bars() {
        // a word twelve decibels louder than the phrase sung a rest after it, in the bars the phrase takes
        let line: &[(f64, u32)] = &[(0.4, 1), (0.55, 2), (0.3, 1)];
        let (mut src, words) = crate::cuts::tests::sung_over_drums(&[(0.6, &[(0.4, 1)]), (1.6, line)], 10.0);
        let loud = (words[0].start * SRF) as usize..(words[0].end * SRF) as usize;
        for ch in src.channels_mut() {
            ch[loud.clone()].iter_mut().for_each(|v| *v *= 4.0);
        }
        let all =
            pieces(&src, None, &Options { count: 2, kinds: vec![Kind::Phrase], ..Options::default() }, 0).unwrap();
        let spans: Vec<_> = all.iter().map(|p| (p.phrase.start, p.phrase.bars, p.phrase.within)).collect();
        let quiet = all.iter().find(|p| p.phrase.within.is_some_and(|(from, _)| from > words[0].end));
        let quiet = quiet.unwrap_or_else(|| panic!("the phrase apart from the word: {spans:?}"));
        let (start, end) = (quiet.phrase.start, quiet.phrase.start + quiet.raw.secs());
        assert!(start <= words[0].start && end >= words[0].end, "its bars hold the loud word: {:?}", quiet.phrase);
        assert!((quiet.raw.peak() - 0.891).abs() < 0.02, "as cut, at -1 dBFS: {}", quiet.raw.peak());
    }

    #[test]
    fn a_later_take_draws_among_the_good_phrases_however_the_ears_scored_them() {
        let phrase = |score: f64| Phrase {
            start: 0.0,
            first: 0,
            bars: 1,
            z: Some(score),
            sung: 0.0,
            centred: 0.0,
            score,
            within: None,
        };
        // the ears may mark every phrase down below nothing
        let pool = || vec![phrase(-0.5), phrase(-0.9), phrase(-3.0)];
        let mut first = std::collections::BTreeMap::new();
        for seed in 1..=60 {
            let drawn = drawn(pool(), &|_| false, &mut crate::rng::Rng::new(seed));
            assert_eq!(
                drawn.len(),
                2,
                "the two near the best, of {:?}",
                drawn.iter().map(|p| p.score).collect::<Vec<_>>()
            );
            *first.entry((drawn[0].score * 10.0) as i64).or_insert(0) += 1;
        }
        assert!(first[&-5] > first[&-9] && first[&-9] > 0, "the better first the likelier: {first:?}");
    }

    #[test]
    fn a_piece_is_a_phrase_sung_whole_and_never_cuts_a_word() {
        // three phrases at 120 BPM, each starting off a bar line and going over one
        let line: &[(f64, u32)] = &[(0.4, 1), (0.55, 2), (0.3, 1)];
        let (src, words) = crate::cuts::tests::sung_over_drums(&[(1.27, line), (5.63, line), (9.41, line)], 14.0);
        let all =
            pieces(&src, None, &Options { count: 3, kinds: vec![Kind::Phrase], ..Options::default() }, 0).unwrap();
        assert_eq!(all.len(), 3, "{:?}", all.iter().map(|p| &p.phrase).collect::<Vec<_>>());
        for p in &all {
            let (from, to) = p.phrase.within.expect("a phrase sung whole");
            assert!(!words.iter().any(|w| w.cut_by(from) || w.cut_by(to)), "{from}..{to} cuts a word of {words:?}");
            let touched: Vec<&crate::cuts::Span> = words.iter().filter(|w| w.end > from && w.start < to).collect();
            assert_eq!(touched.len(), 3, "the phrase's words, all three, and no other: {from}..{to}");
            // silent in its bars before the phrase starts
            let lead = (((from - p.phrase.start) * SRF) as usize).saturating_sub((0.02 * SRF) as usize);
            assert!(p.raw.l[..lead].iter().all(|v| *v == 0.0), "silent before {from} in {:?}", p.phrase);
        }
        // a later take draws another of the good ones
        let one = Options { count: 1, kinds: vec![Kind::Phrase], ..Options::default() };
        let first = pieces(&src, None, &one, 0).unwrap()[0].phrase.within;
        let later: Vec<_> = (1..8).map(|seed| pieces(&src, None, &one, seed).unwrap()[0].phrase.within).collect();
        assert!(later.iter().any(|w| *w != first), "{first:?} every take: {later:?}");
    }

    /// 120 BPM, sixteen bars, apart: a kick on every beat and a hat on every off-beat all through, alone for four bars;
    /// then a bass line on the chords' roots, an octave up every other beat, and the chords themselves, A minor, F, C
    /// and G a bar each, three times round; no voice. The stems, and the mix of them.
    fn band() -> (Stereo, Stems) {
        let (sr, beat) = (SRF as usize, SRF as usize / 2);
        let n = 16 * 4 * beat;
        let (mut drums, mut bass, mut other) = (Stereo::zeros(n), Stereo::zeros(n), Stereo::zeros(n));
        let mut rng = crate::rng::Rng::new(5);
        let tau = std::f64::consts::TAU;
        for b in 0..64 {
            let at = b * beat;
            for i in 0..(0.15 * SRF) as usize {
                let t = i as f64 / SRF;
                let v = ((tau * 60.0 * t).sin() * 0.8 * (-t / 0.05).exp()) as f32;
                (drums.l[at + i], drums.r[at + i]) = (v, v);
            }
            for i in 0..sr / 30 {
                let v = (rng.f32() - 0.5) * 0.3 * (1.0 - i as f32 / (sr / 30) as f32);
                drums.l[at + beat / 2 + i] += v;
                drums.r[at + beat / 2 + i] += v;
            }
        }
        let chords: [[f64; 3]; 4] =
            [[220.0, 261.63, 329.63], [174.61, 220.0, 261.63], [261.63, 329.63, 392.0], [196.0, 246.94, 293.66]];
        let roots = [55.0, 43.65, 65.41, 49.0];
        for bar in 4..16 {
            let c = (bar - 4) % 4;
            for i in 0..4 * beat {
                let t = (bar * 4 * beat + i) as f64 / SRF;
                let up = if (i / beat) % 2 == 1 { 2.0 } else { 1.0 };
                let low = ((tau * roots[c] * up * t).sin() * 0.4) as f32;
                let high = chords[c].iter().map(|f| (tau * f * t).sin() * 0.12).sum::<f64>() as f32;
                let at = bar * 4 * beat + i;
                (bass.l[at], bass.r[at], other.l[at], other.r[at]) = (low, low, high, high);
            }
        }
        let mut mix = drums.clone();
        mix.add_at(&bass, 0, 1.0, 1.0);
        mix.add_at(&other, 0, 1.0, 1.0);
        (mix, Stems { vocals: Stereo::zeros(n), drums, bass, other })
    }

    #[test]
    fn a_break_a_bass_line_and_a_hook_are_each_cut_where_their_kind_is_best() {
        let (mix, stems) = band();
        let a = analysis::analyze_with(&mix, Some(&stems), None);
        let best = |kind: Kind| {
            let (x, rest) = kinds::source(kind, &mix, Some(&stems)).expect("its stems");
            kinds::best(kind, &a, &x, &rest, 0, &mut crate::rng::Rng::new(1)).expect("a piece of it")
        };
        let brk = best(Kind::Break);
        assert!(brk.first < 4 && brk.about.starts_with("the drums alone"), "the drums alone are the break: {brk:?}");
        let bass = best(Kind::Bass);
        assert!(bass.first >= 3 && bass.about.contains("moving"), "{bass:?}");
        let hook = best(Kind::Hook);
        assert!(hook.first >= 3 && hook.about.contains("comes back"), "the chords that come round: {hook:?}");
        // another take draws among the good ones
        let (x, rest) = kinds::source(Kind::Break, &mix, Some(&stems)).unwrap();
        let takes: Vec<usize> = (1..9)
            .filter_map(|seed| kinds::best(Kind::Break, &a, &x, &rest, seed, &mut crate::rng::Rng::new(seed)))
            .map(|c| c.first)
            .collect();
        assert!(!takes.is_empty() && takes.iter().all(|f| *f < 4), "drawn from the drums alone: {takes:?}");
    }

    #[test]
    fn a_stab_is_a_chord_the_instruments_strike_alone_and_kept_a_drum_lane_in_the_songs_key() {
        let (mix, stems) = band();
        let a = analysis::analyze_with(&mix, Some(&stems), None);
        // a chord struck alone on the second beat of the third bar, before the band's chords come in, ringing a beat
        let (beat, tau) = ((SRF / 2.0) as usize, std::f64::consts::TAU);
        let at = 2 * 4 * beat + beat;
        let mut other = stems.other.clone();
        for i in 0..beat {
            let t = i as f64 / SRF;
            let v = [261.63, 329.63, 392.0].iter().map(|f| (tau * f * t).sin() * 0.3).sum::<f64>() * (-t / 0.15).exp();
            (other.l[at + i], other.r[at + i]) = (other.l[at + i] + v as f32, other.r[at + i] + v as f32);
        }
        let mut rest = stems.drums.clone();
        rest.add_at(&stems.bass, 0, 1.0, 1.0);
        let c = kinds::best(Kind::Stab, &a, &other, &rest, 0, &mut crate::rng::Rng::new(1)).expect("a stab");
        let (from, to) = c.span.expect("where its hit is");
        assert!((from - at as f64 / SRF).abs() < 0.02 && to > from, "{c:?}");
        assert!(c.about.contains("struck hard") && c.about.contains("ringing a beat alone"), "{}", c.about);
        // kept, a drum lane strikes it in the song's key, all through the song but its intro
        let x = kinds::level(&other.slice((from * SRF) as usize, (to * SRF) as usize));
        let phrase = Phrase {
            start: from,
            first: c.first as i64,
            bars: 1,
            z: None,
            sung: 0.0,
            centred: 0.0,
            score: c.score,
            within: c.span,
        };
        let (about, treatment, section) = (c.about.clone(), String::new(), String::new());
        let stab = Piece {
            kind: Kind::Stab,
            about,
            phrase,
            raw: x.clone(),
            treated: x,
            treatment,
            fx: Vec::new(),
            bpm: a.cut_bpm(),
            section,
        };
        let mut bp = song();
        let dir = std::env::temp_dir().join(format!("claudius-kinds-stab-{}", std::process::id()));
        let out = keep(&mut bp, &mix, "band", &[Keep::of(&stab)], &Options::default(), &dir).unwrap();
        assert_eq!(out.tracks, ["stab"]);
        assert!(out.said("band").starts_with("sampled a stab of band into the song: stab"), "{}", out.said("band"));
        let lane = bp.drums.iter().find(|d| d.name == "stab").expect("a drum lane");
        // as `keep` hears the recording: here, without the ears' stems
        let heard = analysis::analyze_with(&mix, None, None);
        let transpose = crate::compose::transpose_into(&bp.key, (heard.key.tonic, heard.key.mode.is_minor()));
        assert!(matches!(lane.sound, Sound::Sample { .. }), "{:?}", lane.sound);
        assert_eq!(lane.pitch, transpose, "in the song's key");
        assert!(!lane.patterns["main"].is_empty(), "struck on a few beats");
        assert_eq!(on(&bp, "stab"), ["drop"], "all through the song but its intro");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recording_no_one_sings_in_offers_its_other_pieces_and_kinds_asked_for_alone() {
        let (mix, _) = band();
        // without the ears, from the mix: its break and loops, as it was cut and treated
        let all = pieces(&mix, None, &Options::default(), 0).unwrap();
        let kinds: Vec<Kind> = all.iter().map(|p| p.kind).collect();
        assert!(kinds.contains(&Kind::Break) && kinds.contains(&Kind::Hook), "{kinds:?}");
        assert!(!kinds.contains(&Kind::Bass), "a bass line needs the stems apart: {kinds:?}");
        for p in all.iter().filter(|p| p.kind != Kind::Phrase) {
            assert!(!p.about.is_empty() && p.raw.len() == p.treated.len(), "{:?}", p.kind);
            // whole bars at the tempo the recording was heard at, or a stab's beat
            let beats = if p.kind == Kind::Stab { 1.0 } else { p.phrase.bars as f64 * 4.0 };
            let long = beats * 60.0 / p.bpm * SRF;
            assert!((p.raw.len() as f64 - long).abs() < 0.01 * SRF, "{:?}: {} of {long}", p.kind, p.raw.len());
        }
        let breaks = pieces(&mix, None, &Options { kinds: vec![Kind::Break], ..Options::default() }, 0).unwrap();
        assert!(
            breaks.iter().all(|p| p.kind == Kind::Break),
            "{:?}",
            breaks.iter().map(|p| p.kind).collect::<Vec<_>>()
        );
        assert_eq!(breaks.len(), 1, "its drums play the same all through: one break, not cuts of it alike");
    }

    #[test]
    fn breaks_asked_for_are_varied_its_sparse_groove_and_its_busy_one_not_two_cuts_of_one() {
        // eight bars of a kick alone on every beat, then eight more with a hat on every eighth too
        let (sr, beat) = (SRF as usize, SRF as usize / 2);
        let n = 16 * 4 * beat;
        let mut drums = Stereo::zeros(n);
        let mut rng = crate::rng::Rng::new(7);
        for b in 0..64 {
            let at = b * beat;
            for i in 0..(0.15 * SRF) as usize {
                let t = i as f64 / SRF;
                let v = ((std::f64::consts::TAU * 60.0 * t).sin() * 0.8 * (-t / 0.05).exp()) as f32;
                (drums.l[at + i], drums.r[at + i]) = (v, v);
            }
            for half in (0..2).filter(|_| b >= 32) {
                for i in 0..sr / 40 {
                    let v = (rng.f32() - 0.5) * 0.6 * (1.0 - i as f32 / (sr / 40) as f32);
                    drums.l[at + half * beat / 2 + i] += v;
                    drums.r[at + half * beat / 2 + i] += v;
                }
            }
        }
        let a = analysis::analyze_with(&drums, None, None);
        let cuts = kinds::cuts(Kind::Break, &a, &drums, &Stereo::zeros(n), 0, &[], &mut crate::rng::Rng::new(1));
        let chosen: Vec<&kinds::Cut> = kinds::varied(&cuts, 4, &[]).into_iter().map(|i| &cuts[i]).collect();
        let at: Vec<(f64, f64)> =
            chosen.iter().map(|c| (a.bar_start(c.first), a.bar_start(c.first + c.bars as usize))).collect();
        assert!(at.iter().any(|s| s.1 <= 16.0 + 0.1) && at.iter().any(|s| s.0 >= 16.0 - 0.1), "{at:?}");
        assert_eq!(chosen.len(), 2, "one of each groove: {at:?}");
    }

    #[test]
    fn a_genre_leans_toward_the_kinds_its_producers_sample() {
        // house samples a drum break, a phrase to build on and a vocal, and no bass line
        assert_eq!(leaning("house"), [Kind::Break, Kind::Hook, Kind::Phrase]);
        assert_eq!(leaning("house+jazz"), leaning("house"), "a fusion as its lead genre does");
        assert!(leaning("no such genre").is_empty());
    }

    #[test]
    fn another_take_draws_other_pieces_than_the_take_before_offered() {
        let (mix, _) = band();
        let opt = Options { kinds: vec![Kind::Break, Kind::Hook], ..Options::default() };
        let first = shown(&pieces(&mix, None, &opt, 1).unwrap());
        let again = Options { shown: first.clone(), ..opt.clone() };
        let repeats = |opt: &Options| -> usize {
            let take = |seed| shown(&pieces(&mix, None, opt, seed).unwrap());
            let shown_before =
                |p: &(Kind, f64, f64)| first.iter().any(|s| s.0 == p.0 && kinds::same_span((p.1, p.2), (s.1, s.2)));
            (2..5).map(|seed| take(seed).iter().filter(|p| shown_before(p)).count()).sum()
        };
        let (blind, away) = (repeats(&opt), repeats(&again));
        assert!(away < blind, "{away} of the pieces shown before offered again, drawing away from them; {blind} not");
    }

    #[test]
    fn sections_are_dealt_the_louder_to_the_fuller_pieces_and_the_loudest_to_the_fullest() {
        let mut bp = song();
        (bp.form[0].energy, bp.form[1].energy) = (0.3, 0.9);
        assert_eq!(dealt(&bp, &[0, 1], 2), [vec![0], vec![1]]);
        assert_eq!(dealt(&bp, &[0, 1], 3), [vec![], vec![0], vec![1]], "more pieces than sections: the fullest");
        // sections as loud as each other are shared all the same
        (bp.form[0].energy, bp.form[1].energy) = (0.6, 0.6);
        assert_eq!(dealt(&bp, &[0, 1], 2), [vec![0], vec![1]]);
        assert_eq!(dealt(&bp, &[0, 1], 3), [vec![], vec![0], vec![1]]);
        assert_eq!(dealt(&bp, &[0, 1], 1), [vec![0, 1]], "one piece, all of them");
    }

    #[test]
    fn two_breaks_kept_share_the_songs_sections_the_fuller_in_the_louder() {
        let (mix, stems) = band();
        let a = analysis::analyze_with(&mix, Some(&stems), None);
        let mut bp = song();
        (bp.form[0].energy, bp.form[1].energy) = (0.3, 0.9);
        let n = 4 * SRF as usize;
        // a click on every beat, and noise all through
        let mut sparse = Stereo::zeros(n);
        for i in (0..n).step_by(SRF as usize / 2) {
            (sparse.l[i], sparse.r[i]) = (0.9, 0.9);
        }
        let mut busy = Stereo::zeros(n);
        let mut rng = crate::rng::Rng::new(3);
        for i in 0..n {
            let v = (rng.f32() - 0.5) * 0.9;
            (busy.l[i], busy.r[i]) = (v, v);
        }
        let piece = |x: Stereo| {
            let phrase =
                Phrase { start: 0.0, first: 0, bars: 2, z: None, sung: 0.0, centred: 0.0, score: 0.8, within: None };
            let (about, treatment, bpm, section) = (String::new(), String::new(), a.cut_bpm(), String::new());
            Piece {
                kind: Kind::Break,
                about,
                phrase,
                raw: x.clone(),
                treated: x,
                treatment,
                fx: Vec::new(),
                bpm,
                section,
            }
        };
        let (busy, sparse) = (piece(busy), piece(sparse));
        let dir = std::env::temp_dir().join(format!("claudius-kinds-share-{}", std::process::id()));
        let out =
            keep(&mut bp, &mix, "band", &[Keep::of(&busy), Keep::of(&sparse)], &Options::default(), &dir).unwrap();
        assert_eq!(out.tracks, ["break", "break2"]);
        assert_eq!((on(&bp, "break"), on(&bp, "break2")), (vec!["drop".to_string()], vec!["intro".to_string()]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hook_heard_again_in_the_last_bars_comes_back_and_one_heard_once_says_so() {
        let (mix, stems) = band();
        let mut a = analysis::analyze_with(&mix, Some(&stems), None);
        let n = a.bars;
        // every bar a chord of its own, but the first two come again as the recording's last two
        let chord = |b: usize| std::array::from_fn(|i| ((b * 12 + i) as f32 * 12.9898).sin().abs() * 43758.547 % 1.0);
        a.bar_chroma = (0..n).map(chord).collect();
        (a.bar_chroma[n - 2], a.bar_chroma[n - 1]) = (chord(0), chord(1));
        assert_eq!(kinds::fitness(&a, 0, 2), 1.0 / 3.0, "its last bars come round, and no others");
        assert!(kinds::about(Kind::Hook, &[0.0, 0.8, 5.0], &a, 0).contains("heard once"));
    }

    #[test]
    fn pieces_say_why_there_is_no_phrase_and_a_phrase_with_no_room_leaves_the_rest_kept() {
        let (mix, stems) = band();
        // its drums alone: nothing sounds like a voice, and its break is offered with why there is no phrase, and no
        // loop of instruments that are not there, however its kick rings
        let o = offer(&stems.drums, None, &Options::default(), 0).unwrap();
        let kinds: Vec<Kind> = o.pieces.iter().map(|p| p.kind).collect();
        assert!(!kinds.is_empty() && kinds.iter().all(|k| *k == Kind::Break), "the break alone: {kinds:?}");
        assert_eq!(o.no_phrase.as_deref(), Some(NO_VOICE));
        // a phrase too long for any section, kept with a break: the break goes in, and the reply says why the phrase
        // did not
        let a = analysis::analyze_with(&mix, Some(&stems), None);
        let mut bp = song();
        let piece = |kind: Kind, bars: u32| {
            let x = kinds::level(&cut_bars(&a, &mix, 0, bars as usize));
            let phrase = Phrase {
                start: bar_time(&a, 0),
                first: 0,
                bars,
                z: None,
                sung: 1.0,
                centred: 1.0,
                score: 1.0,
                within: None,
            };
            let (about, treatment, bpm, section) = (String::new(), String::new(), a.cut_bpm(), String::new());
            Piece { kind, about, phrase, raw: x.clone(), treated: x, treatment, fx: Vec::new(), bpm, section }
        };
        let (long, brk) = (piece(Kind::Phrase, 16), piece(Kind::Break, 2));
        let dir = std::env::temp_dir().join(format!("claudius-kinds-room-{}", std::process::id()));
        let out = keep(&mut bp, &mix, "band", &[Keep::of(&long), Keep::of(&brk)], &Options::default(), &dir).unwrap();
        assert_eq!(out.tracks, ["break"]);
        assert!(out.lines.last().is_some_and(|l| l.starts_with("no phrase went in: ")), "{:?}", out.lines);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_break_kept_chopped_is_drum_lanes_each_a_pad_playing_its_hits_where_they_came() {
        let (mix, _) = band();
        let mut bp = song();
        let x = kinds::tests::three_drums();
        let phrase =
            Phrase { start: 0.0, first: 0, bars: 2, z: None, sung: 0.0, centred: 0.0, score: 0.8, within: None };
        let (about, treatment, section) = (String::new(), String::new(), String::new());
        let brk = Piece {
            kind: Kind::Break,
            about,
            phrase,
            raw: x.clone(),
            treated: x,
            treatment,
            fx: Vec::new(),
            bpm: 120.0,
            section,
        };
        let dir = std::env::temp_dir().join(format!("claudius-kinds-chop-{}", std::process::id()));
        let chopped = Keep { chopped: true, ..Keep::of(&brk) };
        let out = keep(&mut bp, &mix, "drums", &[chopped], &Options::default(), &dir).unwrap();
        assert_eq!(out.tracks, ["chop", "chop2", "chop3"]);
        assert!(out.said("drums").starts_with("sampled a drum break of drums into the song: chop, chop2, chop3"));
        assert!(
            out.lines[0].contains("chopped at its 16 onsets onto 3 pads, playing it as it was"),
            "{}",
            out.lines[0]
        );
        let lane = |n: &str| bp.drums.iter().find(|d| d.name == n).unwrap();
        let hits: Vec<usize> = out.tracks.iter().map(|t| lane(t).patterns["main"].len()).collect();
        assert_eq!(hits, [4, 8, 4], "the kicks, the hats and the snares, each on a pad");
        let l = lane("chop");
        assert!(
            matches!(l.sound, Sound::Sample { .. })
                && l.mix.bus == "drums"
                && l.swing == Some(0.5)
                && l.pattern_bars == 2
        );
        let drums: Vec<String> =
            bp.drums.iter().filter(|d| !d.name.starts_with("chop")).map(|d| d.name.clone()).collect();
        for s in &bp.form {
            let has = s.layers.iter().any(|l| drums.contains(l));
            assert_eq!(s.layers.contains(&"chop".to_string()), has, "{}: where the drums play", s.name);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hook_kept_chopped_plays_through_the_music_and_a_piece_too_sparse_to_chop_leaves_the_rest_kept() {
        let (mix, _) = band();
        let mut bp = song();
        let x = kinds::tests::three_drums();
        let piece = |kind, x: Stereo| {
            let phrase =
                Phrase { start: 0.0, first: 0, bars: 2, z: None, sung: 0.0, centred: 0.0, score: 0.8, within: None };
            let (about, treatment, section) = (String::new(), String::new(), String::new());
            Piece { kind, about, phrase, raw: x.clone(), treated: x, treatment, fx: Vec::new(), bpm: 120.0, section }
        };
        let (hook, silent) = (piece(Kind::Hook, x.clone()), piece(Kind::Passage, Stereo::zeros(x.len())));
        let dir = std::env::temp_dir().join(format!("claudius-kinds-chop-music-{}", std::process::id()));
        let kept = [Keep { chopped: true, ..Keep::of(&hook) }, Keep { chopped: true, ..Keep::of(&silent) }];
        let out = keep(&mut bp, &mix, "band", &kept, &Options::default(), &dir).unwrap();
        assert!(!out.tracks.is_empty() && out.tracks.iter().all(|t| t.starts_with("chop")), "{:?}", out.tracks);
        assert!(out.lines.iter().any(|l| l.ends_with("holds too few hits to chop: keep it whole")), "{:?}", out.lines);
        let arranged = crate::compose::arrange(&bp);
        for t in &out.tracks {
            let track = arranged.tracks.iter().find(|a| &a.name == t).unwrap();
            assert_eq!(track.mix.bus, "music", "{t}: a hook's pads play through the music's bus, not the drums'");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lead_steps_back_for_a_voice_sampled_where_a_kept_break_plays() {
        let g = crate::genres::get("trip_hop").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = crate::compose::generate(&g, 1, &o, None);
        let lead = bp.parts.iter().find(|p| p.role == Role::Lead).expect("trip-hop has a lead").name.clone();
        let mut hook = bp.form[0].clone();
        (hook.name, hook.kind, hook.bars, hook.layers) = ("hook".into(), SectionKind::Drop, 4, bp.layer_names());
        bp.form = vec![hook];
        let level = |bp: &Blueprint| bp.parts.iter().find(|p| p.name == lead).unwrap().mix.level_db;
        let was = level(&bp);
        let (mix, stems) = band();
        let a = analysis::analyze_with(&mix, Some(&stems), None);
        let x = kinds::level(&cut_bars(&a, &stems.drums, 0, 2));
        let phrase =
            Phrase { start: 0.0, first: 0, bars: 2, z: None, sung: 0.0, centred: 0.0, score: 1.0, within: None };
        let (about, treatment, bpm, section) = ("about it".into(), String::new(), a.cut_bpm(), String::new());
        let brk = Piece {
            kind: Kind::Break,
            about,
            phrase,
            raw: x.clone(),
            treated: x,
            treatment,
            fx: Vec::new(),
            bpm,
            section,
        };
        let dir = std::env::temp_dir().join(format!("claudius-kinds-lead-{}", std::process::id()));
        keep(&mut bp, &mix, "band", &[Keep::of(&brk)], &Options::default(), &dir).unwrap();
        assert_eq!(level(&bp), was, "a break is no voice to step back for");
        let ask = Options { count: 1, bars: Some(2), sections: vec!["drop".into()], ..Options::default() };
        let out = sample(&mut bp, &hummed(4, 12), "song", None, &ask, &dir).unwrap();
        assert_eq!(level(&bp), was - 1.5, "the lead steps back for the voice beside the break: {:?}", out.lines);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_break_kept_loops_where_the_drums_play_and_a_bass_line_in_place_of_the_bass() {
        let (mix, stems) = band();
        let a = analysis::analyze_with(&mix, Some(&stems), None);
        let mut bp = song();
        let basses: Vec<String> = bp.parts.iter().filter(|p| p.role == Role::Bass).map(|p| p.name.clone()).collect();
        assert!(!basses.is_empty(), "the song has a bass");
        let piece = |kind: Kind, first: i64| {
            let x = kinds::level(&cut_bars(&a, &mix, first, 2));
            let phrase = Phrase {
                start: bar_time(&a, first),
                first,
                bars: 2,
                z: None,
                sung: 0.0,
                centred: 0.0,
                score: 1.0,
                within: None,
            };
            let (about, treatment, bpm, section) = ("about it".into(), String::new(), a.cut_bpm(), String::new());
            Piece { kind, about, phrase, raw: x.clone(), treated: x, treatment, fx: Vec::new(), bpm, section }
        };
        let dir = std::env::temp_dir().join(format!("claudius-kinds-{}", std::process::id()));
        let (brk, bass) = (piece(Kind::Break, 0), piece(Kind::Bass, 4));
        let out = keep(&mut bp, &mix, "band", &[Keep::of(&brk), Keep::of(&bass)], &Options::default(), &dir).unwrap();
        assert_eq!(out.tracks, ["break", "bassline"]);
        assert!(out.said("band").starts_with("sampled a drum break and a bass line of band into the song"));
        let l = |name: &str| bp.loops.iter().find(|l| l.name == name).unwrap();
        assert!(matches!(&l("break").sound, Sound::Loop { tonal: false, key: None, .. }) && l("break").role == "drums");
        assert_eq!((l("break").transpose, l("bassline").role.as_str()), (0, "bass"));
        let bass_bus = &bp.parts.iter().find(|p| p.role == Role::Bass).unwrap().mix.bus;
        assert_eq!((l("break").mix.bus.as_str(), &l("bassline").mix.bus), ("drums", bass_bus), "through their buses");
        let drums: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
        for s in &bp.form {
            let has = |names: &[String]| s.layers.iter().any(|l| names.contains(l));
            assert_eq!(s.layers.contains(&"break".to_string()), has(&drums), "{}: where the drums play", s.name);
            if s.layers.contains(&"bassline".to_string()) {
                assert!(!has(&basses), "{}: the bass line, not the song's bass", s.name);
            }
        }
        assert!(out.lines[1].contains("in place of the song's bass"), "{}", out.lines[1]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pieces_are_offered_raw_and_treated_drawn_anew_each_take_and_kept_as_chosen() {
        let src = hummed(4, 12);
        let opt = Options { count: 1, kinds: vec![Kind::Phrase], ..Options::default() };
        let first = pieces(&src, None, &opt, 0).unwrap();
        assert_eq!(first.len(), 1);
        let p = &first[0];
        // the hum, not the hats: all of it, from just before it, its bars silent outside it
        let (from, to) = p.phrase.within.expect("the hum sung whole");
        assert!((3.8..=4.0).contains(&from) && (12.0..=12.2).contains(&to), "{:?}", p.phrase);
        assert_eq!((p.raw.len(), p.treated.len()), (p.raw.len(), p.raw.len()));
        assert!(p.raw.l != p.treated.l && !p.fx.is_empty(), "treated is another sound");
        assert!(p.treatment.starts_with("dusty"), "take 0 is the gentlest: {}", p.treatment);
        assert_eq!(pieces(&src, None, &opt, 0).unwrap()[0].raw.l, p.raw.l, "the same take, the same pieces");
        let later: Vec<String> =
            (1..6).map(|seed| pieces(&src, None, &opt, seed).unwrap()[0].treatment.clone()).collect();
        assert!(later.iter().any(|t| *t != p.treatment), "other takes, other treatments: {later:?}");
        // kept treated, a piece plays through its treatment; kept as it was, through the vocal's chain alone
        let g = crate::genres::get("trip_hop").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = crate::compose::generate(&g, 1, &o, None);
        let dir = std::env::temp_dir().join(format!("claudius-pieces-{}", std::process::id()));
        let out =
            keep(&mut bp, &src, "song", &[Keep { treated: true, ..Keep::of(p) }], &Options::default(), &dir).unwrap();
        let vox = bp.loops.iter().find(|l| l.name == out.tracks[0]).unwrap();
        assert_eq!(vox.mix.fx.len(), vocal_mix().fx.len() + p.fx.len());
        let out = keep(&mut bp, &src, "song", &[Keep::of(p)], &Options::default(), &dir).unwrap();
        let vox = bp.loops.iter().find(|l| l.name == out.tracks[0]).unwrap();
        assert_eq!(vox.mix.fx.len(), vocal_mix().fx.len());
        assert!(keep(&mut bp, &src, "song", &[], &Options::default(), &dir).unwrap_err().starts_with("keep at least"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
