//! A recording the studio sends to be listened to: analyzed, remade in a genre, its vocals sampled into the song, its
//! groove taken. The same in the app (`claudius-server`) and on the demo's page (`Host::listen`); what each keeps of the
//! recording, and where, is its own.

use std::path::Path;

use claudius::analysis::{self, song::Analysis};
use claudius::audio::{self, Stereo};
use claudius::compose::{Library, Options};
use claudius::genres::GenreProfile;
use claudius::model::Blueprint;
pub use claudius_api::Listen;

/// how a remake is built on its recording (`Command::Remake`'s `sampled`, `flip` and `rebuild`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Remade {
    /// all of it, through, with the genre under it (`remix::rework`)
    Rework,
    /// a new song of the genre on its hook and pieces of it (`remix::remix`)
    Flip,
    /// the song itself, every part of it played on claudius's sounds (`rebuild::rebuild`)
    Rebuild,
    /// a new song on its skeleton only, none of its sound
    Skeleton,
}

impl Remade {
    pub fn of(sampled: Option<bool>, flip: Option<bool>, rebuild: Option<bool>) -> Remade {
        match (rebuild.unwrap_or(false), sampled.unwrap_or(true), flip.unwrap_or(false)) {
            (true, ..) => Remade::Rebuild,
            (false, false, _) => Remade::Skeleton,
            (false, true, true) => Remade::Flip,
            (false, true, false) => Remade::Rework,
        }
    }
}

/// a recording's bytes decoded: the format its name's extension says, else the one its first bytes show
pub fn decode(bytes: Vec<u8>, name: &str) -> Result<Stereo, String> {
    let named = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    let Some(ext) = named.or_else(|| kind(&bytes).map(String::from)) else {
        return Err("send the recording as the file itself: WAV, FLAC, MP3, AAC (m4a), Ogg or AIFF".into());
    };
    audio::load_bytes(bytes, &ext).map_err(|e| format!("cannot read {name}: {e:#}"))
}

/// an MP3 file as a URL that holds it, which a page plays as it is (what the demo's engine hands over to hear)
pub fn mp3_url(bytes: &[u8]) -> String {
    use base64::Engine;
    format!("data:audio/mpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// the MP3 file a URL from `mp3_url` holds; none for any other URL
pub fn mp3_of(url: &str) -> Option<Result<Vec<u8>, String>> {
    use base64::Engine;
    let data = url.strip_prefix("data:audio/mpeg;base64,")?;
    Some(
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| format!("the file did not arrive whole: {e}")),
    )
}

/// the format a file's first bytes show
pub fn kind(b: &[u8]) -> Option<&'static str> {
    let at = |i: usize, s: &[u8]| b.get(i..i + s.len()) == Some(s);
    if at(0, b"RIFF") && at(8, b"WAVE") {
        Some("wav")
    } else if at(0, b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")) {
        Some("aiff")
    } else if at(0, b"fLaC") {
        Some("flac")
    } else if at(0, b"OggS") {
        Some("ogg")
    } else if b.len() >= 2 && b[0] == 0xFF && b[1] & 0xF6 == 0xF0 {
        // an ADTS frame: MPEG audio's sync with layer bits 00, which no MP3 frame has
        Some("aac")
    } else if at(0, b"ID3") || (b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0) {
        Some("mp3")
    } else if at(4, b"ftyp") {
        Some("m4a")
    } else if at(0, b"caff") {
        Some("caf")
    } else {
        None
    }
}

/// what the analysis heard in a recording, as `analyze` answers
pub fn analysis_text(a: &Analysis) -> String {
    let mut out = format!(
        "{:.1} BPM (pulse clarity {:.2}), {} (confidence {:.2}), swing {:.2}, {} bars from {:.2} s\n",
        a.bpm,
        a.pulse_clarity,
        a.key.name(),
        a.key_confidence,
        a.swing,
        a.bars,
        a.first_downbeat
    );
    out.push_str(&format!(
        "progression A: {}  |  B: {}  ({} bar(s) per chord)\n",
        a.progression_a.join(" "),
        a.progression_b.join(" "),
        a.bars_per_chord
    ));
    for s in &a.sections {
        let mut l = Vec::new();
        for (on, n) in [(s.kick, "kick"), (s.bass, "bass"), (s.hats, "hats")] {
            if on {
                l.push(n);
            }
        }
        out.push_str(&format!(
            "bar {:3} +{:2}  {:7} energy {:.2}  {}\n",
            s.start_bar,
            s.bars,
            s.kind.name(),
            s.energy,
            l.join(" ")
        ));
    }
    let grid = analysis::song::grid;
    out.push_str(&format!(
        "kick  {}\nsnare {}\nhats  {}\nbass style: {}\n",
        grid(&a.kick_steps),
        grid(&a.snare_steps),
        grid(&a.hat_steps),
        a.bass_style
    ));
    out
}

/// What the analysis `a` heard in recording `x`, which came in a file named `name`, as the studio shows it while the
/// recording plays, the phrases its voice sings heard in its vocals when `stems` are apart; with none of its audio,
/// which whoever keeps it adds (`Listened::audio`).
pub fn listened(
    a: &Analysis,
    x: &Stereo,
    name: &str,
    stems: Option<&claudius::stems::Stems>,
) -> claudius_api::Listened {
    use claudius_api::{BarHeard, SectionHeard};
    let at = |v: &[String], b: usize| v.get(b).cloned().unwrap_or_default();
    let sings = stems.map_or(x, |s| &s.vocals);
    let sung = claudius::cuts::sung(sings, &analysis::melody::pitches(sings));
    claudius_api::Listened {
        phrases: sung.phrases.iter().map(|p| [p.start, p.end]).collect(),
        waveform: crate::views::waveform_of(x, 1000),
        name: name.to_string(),
        secs: x.len() as f64 / claudius::config::SRF,
        bpm: a.bpm,
        steady: a.steady,
        pulse: a.pulse_clarity,
        key: a.key.name(),
        key_confidence: a.key_confidence,
        swing: a.swing,
        lufs: claudius::loudness::integrated(x),
        bars: (0..a.bars)
            .map(|b| BarHeard { start: a.bar_start(b), chord: at(&a.chords, b), numeral: at(&a.numerals, b) })
            .collect(),
        end: a.bar_start(a.bars),
        sections: a
            .sections
            .iter()
            .map(|s| SectionHeard {
                start_bar: s.start_bar,
                bars: s.bars,
                kind: s.kind.name().to_string(),
                energy: s.energy,
                kick: s.kick,
                bass: s.bass,
                hats: s.hats,
                harmony: s.harmony,
            })
            .collect(),
        progression_a: a.progression_a.clone(),
        progression_b: a.progression_b.clone(),
        bars_per_chord: a.bars_per_chord,
        kick: a.kick_steps.clone(),
        snare: a.snare_steps.clone(),
        hats: a.hat_steps.clone(),
        bass: a.bass_style.clone(),
        audio: None,
    }
}

/// A new song like the recording analysis `a` heard, named `name`, in genre `g`: its tempo (halved or doubled into the
/// genre's range when it is outside), key, progressions, form and swing, none of its sound (`Command::SongLike`); and
/// what to say of it.
pub fn song_like(
    a: &Analysis,
    name: &str,
    g: &GenreProfile,
    seed: u64,
    lib: Option<&dyn Library>,
) -> (Blueprint, String) {
    let bp = analysis::song::to_blueprint(a, g, seed, &Options::default(), lib, name);
    let said = format!(
        "a {} song like {name}: at {:.0} BPM in {}, in its form, on its chords",
        g.name.replace('_', " "),
        bp.bpm,
        a.key.name()
    );
    (bp, said)
}

/// a recording to remake: its audio, its path or the name of the file it came in, the name it goes by, and what was
/// heard in it
pub struct Heard<'a> {
    pub x: &'a Stereo,
    pub path: &'a str,
    pub name: &'a str,
    pub a: &'a Analysis,
    /// its vocals, drums, bass and the rest apart, when the ears separated them
    pub stems: Option<&'a claudius::stems::Stems>,
}

/// what a remake draws on: the library's sounds and samples, where it keeps what it takes of the recording, and the
/// ears, which a flip listens to its voice with
pub struct With<'a> {
    pub lib: Option<&'a dyn Library>,
    pub dir: &'a Path,
    pub ears: Option<&'a claudius::ears::Ears>,
}

/// The song a remake makes of `src` in genre `g`, built on it as `how` says (a song on its skeleton when it has
/// nothing a rework, a flip or a rebuild can be built on), and what to say of it: the verb ("reworked", "remixed",
/// "rebuilt" or "remade") and how it came about, each line after a newline
pub fn remake(
    src: &Heard,
    g: &GenreProfile,
    seed: u64,
    bpm: Option<f64>,
    how: Remade,
    with: &With,
) -> (Blueprint, &'static str, String) {
    let opts = Options { bpm, ..Options::default() };
    let s = claudius::remix::Source { x: src.x, name: src.name, a: src.a, stems: src.stems };
    let built = match how {
        Remade::Rework => Some(claudius::remix::rework(&s, g, seed, &opts, with.lib, with.dir)),
        Remade::Flip => Some(claudius::remix::remix(&s, g, seed, &opts, with.lib, with.dir, with.ears)),
        Remade::Rebuild => Some(claudius::rebuild::rebuild(&s, g, seed, &opts, with.lib, with.dir)),
        Remade::Skeleton => None,
    };
    match built {
        Some(Ok(bp)) => {
            let said: Vec<&String> = bp
                .lineage
                .iter()
                .filter(|l| ["remixed", "reworked", "rebuilt", "sampled"].iter().any(|w| l.starts_with(w)))
                .collect();
            let told = said.iter().map(|l| format!("\n{l}")).collect::<String>();
            let verb = match how {
                Remade::Flip => "remixed",
                Remade::Rebuild => "rebuilt",
                _ => "reworked",
            };
            (bp, verb, told)
        }
        built => {
            let bp = analysis::song::to_blueprint(src.a, g, seed, &opts, with.lib, src.path);
            let mut told = match built {
                Some(Err(e)) => format!("\na song on its skeleton, not a remix of it: {e}"),
                _ => String::new(),
            };
            if bpm.is_none() && bp.bpm != src.a.bpm.round() {
                told.push_str(&format!(
                    "\nat {:.0} BPM: the recording's {:.0} is outside {}'s {:.0}-{:.0}",
                    bp.bpm, src.a.bpm, g.name, g.bpm.0, g.bpm.1
                ));
            }
            (bp, "remade", told)
        }
    }
}

/// the name a groove heard in the file at `path` is kept by: its file's name, in lowercase letters, digits and `-`
pub fn groove_name(path: &str) -> String {
    let stem = Path::new(path).file_stem().map_or("groove".into(), |s| s.to_string_lossy().to_lowercase());
    let name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if name.is_empty() { "groove".into() } else { name }
}

/// The groove of the drums `heard` in a recording (`from`: its path or its name, `analysis::groove::hear`), kept with
/// the song as `name`, and its lanes `lanes` (every drum lane when none) put in it: what to say of it. It is heard
/// before, as a long recording takes a while, and the song need not wait for it.
pub fn groove(
    bp: &mut Blueprint,
    heard: &analysis::groove::Heard,
    from: &str,
    name: &str,
    lanes: &[String],
) -> Result<String, String> {
    let names: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    if let Some(l) = lanes.iter().find(|l| !names.contains(l)) {
        return Err(format!("no drum lane '{l}' (lanes: {})", names.join(" ")));
    }
    let lanes = if lanes.is_empty() { names } else { lanes.to_vec() };
    bp.grooves.insert(name.to_string(), heard.groove.clone());
    for d in bp.drums.iter_mut().filter(|d| lanes.contains(&d.name)) {
        (d.groove, d.groove_amount) = (Some(name.to_string()), 1.0);
    }
    bp.lineage.push(format!("groove {name} from {from}"));
    Ok(format!(
        "groove {name}, heard in {from} at {:.0} BPM over {} hits: {}; {} play in it",
        heard.bpm,
        heard.onsets,
        heard.groove.describe(),
        lanes.join(", ")
    ))
}

/// how a recording is sampled into the song, from what the request says (`Command::SampleSong`): its phrases unless
/// `how` names another of the ways
pub fn sampling(
    count: Option<u32>,
    bars: Option<u32>,
    find: Option<String>,
    sections: Vec<String>,
    how: Option<&str>,
) -> Result<claudius::sampler::Options, String> {
    let how = how.filter(|h| !h.trim().is_empty()).map(claudius::sampler::How::parse).transpose()?;
    Ok(claudius::sampler::Options {
        count: count.map_or(2, |c| c as usize),
        bars,
        find: find.filter(|f| !f.trim().is_empty()),
        sections: sections.into_iter().filter(|s| !s.trim().is_empty()).collect(),
        how: how.unwrap_or_default(),
        every: None,
        kinds: Vec::new(),
        shown: Vec::new(),
        lean: Vec::new(),
    })
}

/// a piece offered, as `sample_pieces` lists it: "1: in hook 2, bars 12-13, 0:31, 96% sung; treated, dusty: ...", and
/// a piece of another kind as what it is: "3: a 2-bar drum break, the drums alone, busy, it loops cleanly, in drop 1,
/// bars 33-34, 1:12; treated, ..."
pub fn piece_said(p: &claudius_api::PieceView) -> String {
    let section = if p.section.is_empty() { String::new() } else { format!("in {}, ", p.section) };
    match claudius::kinds::Kind::parse(&p.kind) {
        Ok(k) if k != claudius::kinds::Kind::Phrase => {
            format!("{}: {}, {}, {section}{}; treated, {}", p.id, k.said(p.bars), p.about, p.place, p.treatment)
        }
        _ => format!("{}: {section}{}, {:.0}% sung; treated, {}", p.id, p.place, p.sung * 100.0, p.treatment),
    }
}

/// the kinds of piece a request asks for, by name (`claudius_api::PIECE_KINDS`); none: every kind
pub fn kinds(said: &[String]) -> Result<Vec<claudius::kinds::Kind>, String> {
    said.iter().filter(|k| !k.trim().is_empty()).map(|k| claudius::kinds::Kind::parse(k)).collect()
}

/// How often a phrase sampled into the song comes round, as `SampleSong` and `KeepPieces` say it (`Every::parse`);
/// none, or nothing said: as the song suits.
pub fn every(said: Option<&str>) -> Result<Option<claudius::sampler::Every>, String> {
    said.filter(|s| !s.trim().is_empty()).map(claudius::sampler::Every::parse).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_heard_in_a_recording_says_where_its_voice_sings_a_phrase() {
        // sixteen seconds at 120 BPM: a hat to the left all through, and a sung A3 from 4 s to 12 s
        let sr = claudius::config::SR as usize;
        let mut x = Stereo::zeros(16 * sr);
        let mut phase = 0f64;
        for i in 0..x.len() {
            if i % (sr / 4) < sr / 50 {
                x.l[i] += if i % 7 < 3 { 0.3 } else { -0.3 };
            }
            if (4 * sr..12 * sr).contains(&i) {
                phase += std::f64::consts::TAU * 220.0 / claudius::config::SRF;
                let v = (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15) as f32;
                (x.l[i], x.r[i]) = (x.l[i] + v, x.r[i] + v);
            }
        }
        let heard = listened(&analysis::song::analyze(&x, None), &x, "hum.wav", None);
        assert_eq!(heard.phrases.len(), 1, "{:?}", heard.phrases);
        let [from, to] = heard.phrases[0];
        assert!((from - 4.0).abs() < 0.1 && (to - 12.0).abs() < 0.15, "sung from {from} to {to}");
        // its waveform, to draw it by: louder where the voice sings over the hat
        let w = &heard.waveform;
        assert!((990..=1010).contains(&w.len()), "{} points", w.len());
        let at = |t: f64| w[(t / 16.0 * w.len() as f64) as usize];
        assert!(at(8.0) > at(14.0) + 0.1, "{} sung, {} not", at(8.0), at(14.0));
        assert_eq!(every(Some(" 4 ")), Ok(Some(claudius::sampler::Every::Bars(4))));
        assert_eq!((every(None), every(Some(""))), (Ok(None), Ok(None)));
    }

    #[test]
    fn the_ways_to_sample_offered_are_the_samplers_own() {
        use claudius::sampler::How;
        let offered: Vec<&str> = claudius_api::SAMPLING_WAYS.iter().map(|w| w.0).collect();
        let own: Vec<&str> = How::ALL.iter().map(|h| h.name()).collect();
        assert_eq!(offered, own);
        let way = |how| sampling(None, None, None, Vec::new(), how).map(|o| o.how);
        assert_eq!(
            (way(None), way(Some(" ")), way(Some("stabs"))),
            (Ok(How::Phrases), Ok(How::Phrases), Ok(How::Stabs))
        );
        assert!(way(Some("slices")).unwrap_err().starts_with("no way 'slices' to sample a recording"));
    }

    #[test]
    fn a_remake_is_a_rework_unless_asked_for_a_flip_a_rebuild_or_a_skeleton() {
        assert_eq!(Remade::of(None, None, None), Remade::Rework);
        assert_eq!(Remade::of(Some(true), Some(true), None), Remade::Flip);
        assert_eq!(Remade::of(Some(false), Some(true), Some(false)), Remade::Skeleton);
        assert_eq!(Remade::of(Some(false), Some(true), Some(true)), Remade::Rebuild);
    }

    #[test]
    fn a_file_without_an_extension_is_read_by_its_first_bytes() {
        assert_eq!(kind(b"RIFF\0\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(kind(b"fLaC\0"), Some("flac"));
        assert_eq!(kind(b"\0\0\0\x20ftypM4A "), Some("m4a"), "an AAC file, as iTunes and phones write one");
        assert_eq!(kind(b"caff\0\x01"), Some("caf"));
        assert_eq!(kind(&[0xFF, 0xF1, 0x50, 0x80]), Some("aac"), "an ADTS frame, not an MP3 one");
        assert_eq!(kind(&[0xFF, 0xFB, 0x90, 0x64]), Some("mp3"));
        assert_eq!(kind(b"hello"), None);
        assert!(decode(b"hello".to_vec(), "noise").unwrap_err().starts_with("send the recording as the file itself"));
        assert_eq!(groove_name("/x/My Break (92).wav"), "my_break__92_");
    }

    #[test]
    fn what_the_demo_asks_reads_as_the_studio_sends_it() {
        let l: Listen = serde_json::from_value(serde_json::json!({"what": "remake", "name": "a.mp3", "genre": "trap",
            "seed": 3, "flip": true}))
        .unwrap();
        assert_eq!(
            l,
            Listen::Remake {
                name: "a.mp3".into(),
                genre: Some("trap".into()),
                seed: Some(3),
                flip: true,
                rebuild: false
            }
        );
        assert_eq!((l.name(), l.what()), ("a.mp3", "remake"));
    }
}
