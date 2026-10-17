//! Sound names: synthesized drum voices, synth presets, sampled instruments from the packs, Apple Loops cuts and
//! audio files, and the mixing role each one gets.

use crate::compose::{self, Library as _};
use crate::genres::GenreProfile;
use crate::model::Sound;
use crate::pattern::Kind;
use crate::rng::{Rng, hash_str};
use crate::synth::drums::{DrumParams, DrumVoice};
use crate::synth::presets;

const DRUM_NAMES: &[(&str, DrumVoice, &str)] = &[
    ("bd", DrumVoice::Kick, "kick"),
    ("kick", DrumVoice::Kick, "kick"),
    ("sd", DrumVoice::Snare, "snare"),
    ("snare", DrumVoice::Snare, "snare"),
    ("cp", DrumVoice::Clap, "clap"),
    ("clap", DrumVoice::Clap, "clap"),
    ("hh", DrumVoice::ClosedHat, "hat"),
    ("ch", DrumVoice::ClosedHat, "hat"),
    ("hat", DrumVoice::ClosedHat, "hat"),
    ("oh", DrumVoice::OpenHat, "ohat"),
    ("ohat", DrumVoice::OpenHat, "ohat"),
    ("rd", DrumVoice::Ride, "ride"),
    ("ride", DrumVoice::Ride, "ride"),
    ("cr", DrumVoice::Crash, "crash"),
    ("crash", DrumVoice::Crash, "crash"),
    ("lt", DrumVoice::Tom, "tom"),
    ("mt", DrumVoice::Tom, "tom"),
    ("ht", DrumVoice::Tom, "tom"),
    ("tom", DrumVoice::Tom, "tom"),
    ("rim", DrumVoice::Rim, "perc"),
    ("rs", DrumVoice::Rim, "perc"),
    ("sh", DrumVoice::Shaker, "shaker"),
    ("shaker", DrumVoice::Shaker, "shaker"),
    ("cb", DrumVoice::Cowbell, "perc"),
    ("cowbell", DrumVoice::Cowbell, "perc"),
    ("snap", DrumVoice::Snap, "perc"),
    ("perc", DrumVoice::Perc, "perc"),
];

/// every drum name a sound pattern may use
pub fn drum_names() -> Vec<&'static str> {
    DRUM_NAMES.iter().map(|d| d.0).collect()
}

pub fn drum_name(name: &str) -> Option<(DrumVoice, &'static str)> {
    DRUM_NAMES.iter().find(|(n, _, _)| *n == name).map(|(_, v, r)| (*v, *r))
}

/// the settings drum word `word` plays with variation `idx` (0 is the voice as it is; "lt" "mt" "ht" are retuned toms)
fn drum_params(word: &str, voice: DrumVoice, idx: u64) -> DrumParams {
    let mut p = DrumParams::default_for(voice);
    match word {
        "lt" => p.tune = 90.0,
        "mt" => p.tune = 130.0,
        "ht" => p.tune = 180.0,
        _ => {}
    }
    if idx > 0 {
        // other indices: the same voice, a few steps across its parameter space
        let mut rng = Rng::new(idx ^ hash_str(word));
        let space = DrumParams::space(voice);
        for i in 0..9 {
            let crate::synth::drums::Range(lo, hi, _) = space[i];
            let v = p.get(i) + (hi - lo) * (rng.f32() - 0.5) * 0.35;
            p.set(i, v.clamp(lo.min(hi), hi.max(lo)));
        }
    }
    p
}

/// the drum word, with a variation when it needs one ("bd", "bd:3", "lt"), that plays exactly these settings; None
/// for settings the generator drew, which no word names
pub fn drum_label(voice: DrumVoice, params: &DrumParams) -> Option<String> {
    DRUM_NAMES.iter().filter(|d| d.1 == voice).find_map(|(word, _, _)| {
        (0..=32).find(|&idx| drum_params(word, voice, idx) == *params).map(|idx| match idx {
            0 => word.to_string(),
            i => format!("{word}:{i}"),
        })
    })
}

/// the drum word that plays a voice as it is by default
pub fn voice_word(voice: DrumVoice) -> &'static str {
    match voice {
        // "lt", "mt" and "ht" are retuned toms
        DrumVoice::Tom => "tom",
        v => DRUM_NAMES.iter().find(|d| d.1 == v).map_or("perc", |d| d.0),
    }
}

/// the drum role of a label, when it names a drum: a drum word anywhere in it ("bd", "bd:3", "tr808:sd"), or a drum
/// instrument from the packs (a conga is percussion)
pub fn drum_role(label: &str) -> Option<String> {
    if let Some((_, r)) = label.split(':').find_map(drum_name) {
        return Some(r.to_string());
    }
    // synth presets come before the packs, as they do when the sound is resolved
    if presets::get(label.split(':').next().unwrap_or(label)).is_some() {
        return None;
    }
    let r = claudius_packs::resolve(label, None)?;
    let info = claudius_packs::instrument_ref(&r).ok()?.info();
    (info.kind == "drum").then(|| drum_name(&r.instrument).map_or("perc", |(_, role)| role).to_string())
}

/// the mixing role of a sound label
pub fn role_of(label: &str, kind: Kind, median_pitch: f64) -> String {
    if let Some(r) = drum_role(label) {
        return r;
    }
    let base = label.split(':').next().unwrap_or(label);
    if let Some((_, roles)) = presets::ROLES.iter().find(|(n, _)| *n == base)
        && roles.contains(&"bass")
    {
        return "bass".into();
    }
    match kind {
        Kind::Chord => "chords".into(),
        _ if median_pitch < 52.0 => "bass".into(),
        _ => "lead".into(),
    }
}

/// the sound a label names: a drum voice, a synth preset, an Apple Loops one-shot (bank "apple"), or a file
pub fn resolve_sound(
    label: &str,
    bank: Option<&str>,
    lib: Option<&crate::library::Library>,
    genre: Option<&GenreProfile>,
    seed: u64,
    roots: Option<&[std::path::PathBuf]>,
) -> Result<Sound, String> {
    let (base, idx) = match label.split_once(':') {
        Some((b, i)) => (b, i.parse::<u64>().unwrap_or(0)),
        None => (label, 0),
    };
    match bank {
        Some("apple") | Some("apple_loops") => {
            if let (Some(l), Some(g), Some((_, lane))) = (lib, genre, drum_name(base)) {
                let mut rng = Rng::new(seed ^ hash_str(label));
                if let Some(s) = l.one_shot(lane, &compose::default_hints(lane), g, &mut rng) {
                    return Ok(s);
                }
            }
        }
        Some(b) => {
            return claudius_packs::resolve(label, Some(b)).map(|r| Sound::Instrument { name: r.to_string(), label: label.to_string() }).ok_or_else(|| {
                let packs: Vec<String> = claudius_packs::installed().iter().map(|p| p.name.clone()).collect();
                if claudius_packs::disabled() {
                    format!("bank '{b}' plays sample packs, which are off in this session (--synth-only)")
                } else if packs.is_empty() {
                    format!("no sample packs are installed for bank '{b}' (install them with: claudius-packs install --defaults)")
                } else {
                    format!("no '{label}' in bank '{b}' (installed packs: {}; kits: tr909 tr808 tr707 acoustic uzu)", packs.join(" "))
                }
            });
        }
        None => {}
    }
    if let Some((voice, _)) = drum_name(base) {
        return Ok(Sound::Drum { voice, params: drum_params(base, voice, idx) });
    }
    if let Some(patch) = presets::get(base) {
        return Ok(Sound::Synth { preset: base.to_string(), patch });
    }
    if let Some(r) = claudius_packs::resolve(label, None) {
        return Ok(Sound::Instrument { name: r.to_string(), label: label.to_string() });
    }
    if label.contains('/')
        || [".wav", ".mp3", ".aif", ".aiff", ".flac", ".caf", ".m4a"].iter().any(|e| label.to_lowercase().ends_with(e))
    {
        if let Some(e) = crate::config::read_error(std::path::Path::new(label), roots) {
            return Err(e);
        }
        return Ok(Sound::Sample { path: label.to_string(), root: 60, start: 0.0, end: None, label: String::new() });
    }
    let drums: Vec<&str> = DRUM_NAMES.iter().map(|d| d.0).collect();
    let packs = if claudius_packs::disabled() {
        "; sampled instruments are off in this session (--synth-only)"
    } else if claudius_packs::installed().is_empty() {
        "; sampled instruments need packs: claudius-packs install --defaults"
    } else {
        "; sampled: piano upright rhodes wurlitzer hammond string_ensemble violins cellos pizzicato brass trumpet flute harp marimba vibes kalimba upright_bass guitar choir sax ... (claudius-packs aliases)"
    };
    Err(format!("unknown sound '{base}' (drums: {}; synths: {}{packs})", drums.join(" "), presets::names().join(" ")))
}
