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

/// how a remake is built on its recording (`Command::Remake`'s `sampled` and `flip`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Remade {
    /// all of it, through, with the genre under it (`remix::rework`)
    Rework,
    /// a new song of the genre on its hook and pieces of it (`remix::remix`)
    Flip,
    /// a new song on its skeleton only, none of its sound
    Skeleton,
}

impl Remade {
    pub fn of(sampled: Option<bool>, flip: Option<bool>) -> Remade {
        match (sampled.unwrap_or(true), flip.unwrap_or(false)) {
            (false, _) => Remade::Skeleton,
            (true, true) => Remade::Flip,
            (true, false) => Remade::Rework,
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
    } else if at(0, b"ID3") || (b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0) {
        Some("mp3")
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

/// a recording to remake: its audio, its path or the name of the file it came in, the name it goes by, and what was
/// heard in it
pub struct Heard<'a> {
    pub x: &'a Stereo,
    pub path: &'a str,
    pub name: &'a str,
    pub a: &'a Analysis,
}

/// what a remake draws on: the library's sounds and samples, where it keeps what it takes of the recording, and the
/// ears, which a flip listens to its voice with
pub struct With<'a> {
    pub lib: Option<&'a dyn Library>,
    pub dir: &'a Path,
    pub ears: Option<&'a claudius::ears::Ears>,
}

/// The song a remake makes of `src` in genre `g`, built on it as `how` says (a song on its skeleton when it has
/// nothing a rework or a flip can be built on), and what to say of it: the verb ("reworked", "remixed" or "remade")
/// and how it came about, each line after a newline
pub fn remake(
    src: &Heard,
    g: &GenreProfile,
    seed: u64,
    bpm: Option<f64>,
    how: Remade,
    with: &With,
) -> (Blueprint, &'static str, String) {
    let opts = Options { bpm, ..Options::default() };
    let s = claudius::remix::Source { x: src.x, name: src.name, a: src.a };
    let built = match how {
        Remade::Rework => Some(claudius::remix::rework(&s, g, seed, &opts, with.lib, with.dir)),
        Remade::Flip => Some(claudius::remix::remix(&s, g, seed, &opts, with.lib, with.dir, with.ears)),
        Remade::Skeleton => None,
    };
    match built {
        Some(Ok(bp)) => {
            let said: Vec<&String> = bp
                .lineage
                .iter()
                .filter(|l| ["remixed", "reworked", "sampled"].iter().any(|w| l.starts_with(w)))
                .collect();
            let told = said.iter().map(|l| format!("\n{l}")).collect::<String>();
            (bp, if how == Remade::Flip { "remixed" } else { "reworked" }, told)
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

/// The groove of the drums in recording `x` (heard in `from`, a path or a name, at `bpm` when given) kept with the
/// song as `name`, and its lanes `lanes` (every drum lane when none) put in it: what to say of it
pub fn groove(
    bp: &mut Blueprint,
    x: &Stereo,
    from: &str,
    name: &str,
    lanes: &[String],
    bpm: Option<f64>,
) -> Result<String, String> {
    let heard = analysis::groove::hear(x, bpm).map_err(|e| format!("{from}: {e}"))?;
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

/// how the vocals of a recording are sampled into the song, from what the request says (`Command::SampleSong`)
pub fn sampling(
    count: Option<u32>,
    bars: Option<u32>,
    find: Option<String>,
    sections: Vec<String>,
) -> claudius::sampler::Options {
    claudius::sampler::Options {
        count: count.map_or(2, |c| c as usize),
        bars,
        find: find.filter(|f| !f.trim().is_empty()),
        sections: sections.into_iter().filter(|s| !s.trim().is_empty()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remake_is_a_rework_unless_asked_for_a_flip_or_a_skeleton() {
        assert_eq!(Remade::of(None, None), Remade::Rework);
        assert_eq!(Remade::of(Some(true), Some(true)), Remade::Flip);
        assert_eq!(Remade::of(Some(false), Some(true)), Remade::Skeleton);
    }

    #[test]
    fn a_file_without_an_extension_is_read_by_its_first_bytes() {
        assert_eq!(kind(b"RIFF\0\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(kind(b"fLaC\0"), Some("flac"));
        assert_eq!(kind(b"hello"), None);
        assert!(decode(b"hello".to_vec(), "noise").unwrap_err().starts_with("send the recording as the file itself"));
        assert_eq!(groove_name("/x/My Break (92).wav"), "my_break__92_");
    }

    #[test]
    fn what_the_demo_asks_reads_as_the_studio_sends_it() {
        let l: Listen = serde_json::from_value(serde_json::json!({"what": "remake", "name": "a.mp3", "genre": "trap",
            "seed": 3, "flip": true}))
        .unwrap();
        assert_eq!(l, Listen::Remake { name: "a.mp3".into(), genre: Some("trap".into()), seed: Some(3), flip: true });
        assert_eq!((l.name(), l.what()), ("a.mp3", "remake"));
    }
}
