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
use crate::dsp::fft;
use crate::dsp::filter::{self, BiquadKind};
use crate::fx::{Effect, EqBand};
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
}

impl Default for Options {
    fn default() -> Self {
        Options { count: 2, bars: None, find: None, sections: Vec::new(), how: How::Phrases }
    }
}

/// a phrase of the recording worth sampling
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// where it starts in the recording, in seconds, the bar that is (counting from 0), and how many bars it lasts
    pub start: f64,
    pub first: usize,
    pub bars: u32,
    /// the ears' z for what they listened for, when they listened
    pub z: Option<f64>,
    /// the share of it that holds a pitch in a voice's range, and of its sound that is in the middle
    pub sung: f64,
    pub centred: f64,
    pub score: f64,
}

/// what the sampler did: the tracks it added, and a line for each saying where it came from and went
#[derive(Debug, Default)]
pub struct Sampled {
    pub tracks: Vec<String>,
    pub lines: Vec<String>,
    pub how: How,
}

impl Sampled {
    /// what came of sampling the recording `name`, in a line, then its lines: "sampled 2 phrases of song.mp3 into the
    /// song: vox1, vox2", so that whoever reads only the first knows what changed
    pub fn said(&self, name: &str) -> String {
        let n = self.tracks.len();
        let what = match self.how {
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
        None => [8, 4]
            .into_iter()
            .filter(|&b| b <= held)
            .find_map(|b| room(bp, &groups, b).map(|r| (b, r)))
            .ok_or("no section asked for has room for a passage of 4 bars")?,
    };
    let starts: Vec<usize> = a.sections.iter().map(|s| s.start_bar).collect();
    let lift = |p: &Phrase| {
        p.score + if starts.contains(&p.first) { 0.3 } else { 0.1 * p.first.is_multiple_of(4) as u8 as f64 }
    };
    let best = candidates(h.src, h.stems, a, ears, &Options { bars: Some(len), ..opt.clone() })
        .into_iter()
        .max_by(|x, y| lift(x).total_cmp(&lift(y)))
        .ok_or(NO_VOICE)?;
    let (cut, apart) = match h.stems {
        Some(s) => (voice(&a.cut(&s.vocals, best.first, len as usize)), true),
        None => (whole(&a.cut(h.src, best.first, len as usize)), false),
    };
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let file = crate::take::store(&cut, dir).map_err(|e| format!("cannot keep the passage: {e:#}"))?;
    let track = fresh(bp, "passage");
    let from_bar = best.first as i64 + 1;
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
    let (s0, s1) = ((a.bar_start(best.first) * SRF) as usize, (a.bar_start(best.first + len as usize) * SRF) as usize);
    let backed = h.stems.is_none_or(|s| rms(&s.backing(), s0, s1 - s0) > 0.25 * rms(&s.vocals, s0, s1 - s0));
    let played = backed
        && (best.first..best.first + len as usize)
            .all(|k| a.sections.iter().any(|s| s.harmony && (s.start_bar..s.start_bar + s.bars).contains(&k)));
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
    first: usize,
    len: u32,
    transpose: i32,
    key: &crate::theory::scale::Key,
    per: u32,
) -> Option<Vec<String>> {
    use crate::theory::chord::{Chord, roman_of};
    let up = |pc: u8| (pc as i32 + transpose).rem_euclid(12) as u8;
    (first..first + len as usize)
        .step_by(per.max(1) as usize)
        .map(|k| {
            let c = Chord::parse(a.chords.get(k)?)?;
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
                && bp.loops.iter().any(|l| l.sampled && !out.tracks.contains(&l.name) && s.layers.contains(&l.name))
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
    let from_bar = p.first as i64 + 1;
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
                let v = voice(&a.cut(&s.vocals, p.first, 2));
                (v.clone(), v)
            }
            None => {
                let mid = middle(&a.cut(h.src, p.first, 2));
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
    match stems {
        Some(s) => voice(&a.cut(&s.vocals, p.first, p.bars as usize)),
        None => clean(&a.cut(src, p.first, p.bars as usize)),
    }
}

/// what the sampler says of a recording it finds no voice in
const NO_VOICE: &str = "nothing in the recording sounds like a voice: try other words for what to find";

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
        .filter(|&si| bp.loops.iter().any(|l| l.sampled && bp.form[si].layers.contains(&l.name)))
        .collect();
    for (p, fx) in phrases {
        let cut = phrase_of(src, h.stems, a, p);
        let profile = shift(&pitch_profile(&cut), transpose);
        let period = 2 * p.bars;
        let mut best: Option<(f64, usize, usize, f64)> = None;
        for (g, (_, secs)) in groups.iter().enumerate() {
            for slot in 0..2 {
                if taken[g][slot] {
                    continue;
                }
                let offset = slot as u32 * p.bars;
                // a slot no section of the group has room for would never sound
                let Some(fits) = fit(&project, secs, offset, p.bars, period, &profile) else { continue };
                // the earlier groups suit a vocal best: the drops and hooks, then the breaks, the verses, the builds
                let suits = fits + 0.12 * (groups.len() - g) as f64 / groups.len() as f64 - 0.05 * slot as f64;
                if best.is_none_or(|b| suits > b.0) {
                    best = Some((suits, g, slot, fits));
                }
            }
        }
        // a phrase with no room left may be longer than the next one
        let Some((_, g, slot, fits)) = best else { continue };
        taken[g][slot] = true;
        let offset = slot as u32 * p.bars;
        let file = crate::take::store(&layout(&cut, offset as f64 * bar, period as f64 * bar), dir)
            .map_err(|e| format!("cannot keep the phrase: {e:#}"))?;
        let names = bp.layer_names();
        let track = (1..).map(|k| format!("vox{k}")).find(|n| !names.contains(n)).unwrap_or_default();
        let from_bar = p.first as i64 + 1;
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
        let names: Vec<&str> = secs.iter().map(|&si| bp.form[si].name.as_str()).collect();
        out.lines.push(format!(
            "{track}: {} of {name} ({heard}{:.0}% sung), {moved} at {:.0} BPM, every {period} bars {at} in the {label} \
             ({}), on chords it agrees with {:.0}%",
            bars_text(from_bar, p.bars),
            p.sung * 100.0,
            bp.bpm,
            names.join(", "),
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
/// its drums as `sample` cuts one, and the same through a treatment.
#[derive(Clone, Debug)]
pub struct Piece {
    pub phrase: Phrase,
    pub raw: Stereo,
    pub treated: Stereo,
    /// the treatment in words, and its effects, which a piece kept treated plays through
    pub treatment: String,
    pub fx: Vec<Effect>,
    /// the tempo it keeps, cut from the recording
    pub bpm: f64,
}

impl Piece {
    /// where it is in the recording, in words: "bars 12-13, 0:31"
    pub fn place(&self) -> String {
        let at = self.phrase.start.max(0.0) as u64;
        format!("{}, {}:{:02}", bars_text(self.phrase.first as i64 + 1, self.phrase.bars), at / 60, at % 60)
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

/// The pieces of `src` the sampler offers before any goes into the song: `opt.count` phrases (at most `MOST`), each
/// cut and cleaned as `sample` cuts one, and the same through a treatment. Take `seed` 0 offers the best phrases with
/// the gentler treatments; each other take draws again from the good ones (and their length, unless `opt.bars` says),
/// with other treatments. The ears, when given, listen for `opt.find`.
pub fn pieces(src: &Stereo, ears: Option<&crate::ears::Ears>, opt: &Options, seed: u64) -> Result<Vec<Piece>, String> {
    if let Some(b) = opt.bars.filter(|b| ![1, 2, 4].contains(b)) {
        return Err(format!("a phrase is 1, 2 or 4 bars, not {b}"));
    }
    if (src.len() as f64 / SRF) < 4.0 {
        return Err("the recording is too short to sample (4 s at least)".into());
    }
    let stems = crate::stems::of(src, &mut |_| {});
    let a = analysis::analyze_with(src, stems.as_deref(), None);
    let count = opt.count.clamp(1, MOST);
    let mut rng = crate::rng::Rng::seed_from(seed, "pieces");
    let bars = opt.bars.or_else(|| (seed > 0).then(|| *rng.pick(&[1, 2, 2, 4])));
    let wanted = Options { bars, ..opt.clone() };
    let mut pool = find(src, stems.as_deref(), &a, ears, &wanted, if seed == 0 { count } else { count * 3 });
    if pool.is_empty() {
        return Err(NO_VOICE.into());
    }
    // a later take draws from the good ones: the better a phrase, the likelier it comes
    if seed > 0 {
        let mut keyed: Vec<(f64, Phrase)> = pool.into_iter().map(|p| (p.score * rng.range(0.4, 1.6), p)).collect();
        keyed.sort_by(|x, y| y.0.total_cmp(&x.0));
        pool = keyed.into_iter().map(|(_, p)| p).collect();
    }
    pool.truncate(count);
    pool.sort_by(|x, y| x.start.total_cmp(&y.start));
    let first = if seed == 0 { 0 } else { rng.below(7) };
    Ok(pool
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let raw = phrase_of(src, stems.as_deref(), &a, &p);
            let (treatment, fx) = treatment(first + i, p.bars as f64 * 4.0);
            let mut treated = raw.clone();
            let ctx = crate::fx::Ctx::whole(SRF, a.cut_bpm(), seed.wrapping_add(i as u64), treated.len());
            crate::fx::apply_chain(&fx, &mut treated, &ctx);
            Piece { phrase: p, raw, treated, treatment, fx, bpm: a.cut_bpm() }
        })
        .collect())
}

/// The pieces `pieces` offered that the person kept, each as it was cut or treated (`true`), sampled into the song
/// as `sample` places phrases, in the sections `opt` asks for (those that suit each, when none).
pub fn keep(
    bp: &mut Blueprint,
    src: &Stereo,
    name: &str,
    kept: &[(&Piece, bool)],
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
    let chosen: Vec<(Phrase, Vec<Effect>)> =
        kept.iter().map(|(p, treated)| (p.phrase.clone(), if *treated { p.fx.clone() } else { Vec::new() })).collect();
    place(bp, &Heard { src, stems: stems.as_deref(), a: &a, name }, &chosen, opt, dir)
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
        let (a0, a1) = (p.first, p.first + p.bars as usize);
        if out.iter().all(|q| a1 <= q.first || a0 >= q.first + q.bars as usize) {
            out.push(p);
        }
        if out.len() == count {
            break;
        }
    }
    out
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
                cands.push(Phrase { start: t, first, bars, z: None, sung, centred, score });
            }
        }
    }
    let end = |p: &Phrase| a.bar_start(p.first + p.bars as usize);
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
    fn pieces_are_offered_raw_and_treated_drawn_anew_each_take_and_kept_as_chosen() {
        let src = hummed(4, 12);
        let opt = Options { count: 1, bars: Some(2), ..Options::default() };
        let first = pieces(&src, None, &opt, 0).unwrap();
        assert_eq!(first.len(), 1);
        let p = &first[0];
        assert!((4.0..=10.0).contains(&p.phrase.start), "the hum, not the hats: {:?}", p.phrase);
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
        let out = keep(&mut bp, &src, "song", &[(p, true)], &Options::default(), &dir).unwrap();
        let vox = bp.loops.iter().find(|l| l.name == out.tracks[0]).unwrap();
        assert_eq!(vox.mix.fx.len(), vocal_mix().fx.len() + p.fx.len());
        let out = keep(&mut bp, &src, "song", &[(p, false)], &Options::default(), &dir).unwrap();
        let vox = bp.loops.iter().find(|l| l.name == out.tracks[0]).unwrap();
        assert_eq!(vox.mix.fx.len(), vocal_mix().fx.len());
        assert!(keep(&mut bp, &src, "song", &[], &Options::default(), &dir).unwrap_err().starts_with("keep at least"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
