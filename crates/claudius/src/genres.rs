//! Genre profiles: the production knowledge claudius composes from, as data (crates/claudius/data/genres/*.json,
//! plus any *.json in claudius/genres under the user's config folder, ~/Library/Application Support on macOS and
//! ~/.config on Linux, to add or override genres without recompiling).

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::fx::Effect;
use crate::model::{Role, SectionSpec, TrackMix};
use crate::synth::drums::DrumVoice;
use crate::theory::chord::Voicing;
use crate::theory::rhythm::LaneTemplate;
use crate::theory::scale::Mode;

const BUILTIN: &[(&str, &str)] = &[
    ("house", include_str!("../data/genres/house.json")),
    ("deep_house", include_str!("../data/genres/deep_house.json")),
    ("techno", include_str!("../data/genres/techno.json")),
    ("trap", include_str!("../data/genres/trap.json")),
    ("lofi", include_str!("../data/genres/lofi.json")),
    ("dnb", include_str!("../data/genres/dnb.json")),
    ("future_bass", include_str!("../data/genres/future_bass.json")),
    ("synthwave", include_str!("../data/genres/synthwave.json")),
];

fn straight() -> (f32, f32) {
    (0.5, 0.5)
}
fn yes() -> bool {
    true
}
fn eight() -> u32 {
    8
}
fn quarter() -> f32 {
    0.3
}
fn mid_density() -> (f32, f32) {
    (0.4, 0.6)
}
fn close() -> Voicing {
    Voicing::Close
}

#[derive(Clone, Debug, Deserialize)]
pub struct LaneSpec {
    pub name: String,
    pub voice: DrumVoice,
    #[serde(default)]
    pub template: Option<LaneTemplate>,
    /// extra pattern variants by name (alt, sparse, half ...)
    #[serde(default)]
    pub variants: BTreeMap<String, LaneTemplate>,
    /// (onsets, steps) instead of a template
    #[serde(default)]
    pub euclid: Option<(usize, usize)>,
    /// parameter ranges overriding the voice defaults ("decay": [0.3, 0.6])
    #[serde(default)]
    pub params: BTreeMap<String, (f32, f32)>,
    /// Apple Loops subtypes whose one-shots may replace the synthesized voice ("Kick", "Snare", "Hi-hat")
    #[serde(default)]
    pub samples: Vec<String>,
    #[serde(default)]
    pub choke: Option<String>,
    #[serde(default)]
    pub tune_to_key: bool,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PartSpec {
    pub name: String,
    pub role: Role,
    pub presets: Vec<String>,
    pub styles: Vec<String>,
    pub register: (i32, i32),
    #[serde(default = "mid_density")]
    pub density: (f32, f32),
    #[serde(default)]
    pub voices: usize,
    #[serde(default = "close")]
    pub voicing: Voicing,
    #[serde(default)]
    pub rates: Vec<f64>,
    /// probability that a generated song leaves this part out
    #[serde(default)]
    pub optional: f64,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoopSpec {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub instrument: Vec<String>,
    #[serde(default)]
    pub subtype: Vec<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub descriptors: Vec<String>,
    #[serde(default)]
    pub tonal: bool,
    #[serde(default)]
    pub optional: f64,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TransitionSpec {
    #[serde(default = "yes")]
    pub riser: bool,
    #[serde(default = "yes")]
    pub impact: bool,
    #[serde(default = "yes")]
    pub downlifter: bool,
    #[serde(default = "yes")]
    pub reverse_crash: bool,
    #[serde(default = "eight")]
    pub riser_bars: u32,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MixSpec {
    pub lufs: f32,
    pub reverb: Effect,
    pub delay: Effect,
    #[serde(default)]
    pub buses: BTreeMap<String, Vec<Effect>>,
    #[serde(default)]
    pub master: Vec<Effect>,
    #[serde(default = "quarter")]
    pub duck_release: f32,
    /// role -> dB offset from the default target loudness
    #[serde(default)]
    pub levels: BTreeMap<String, f32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenreProfile {
    pub name: String,
    pub description: String,
    /// what the ears should hear (a CLAP description; earshot's vocabulary terms calibrate best)
    pub query: String,
    #[serde(default)]
    pub alts: Vec<String>,
    pub bpm: (f64, f64),
    #[serde(default = "straight")]
    pub swing: (f32, f32),
    #[serde(default)]
    pub humanize: f32,
    pub modes: Vec<(Mode, f64)>,
    pub harmony: Vec<(String, f64)>,
    /// chance of writing a fresh progression with the Markov chain instead of picking from the library
    #[serde(default)]
    pub markov: f64,
    /// chord extensions for generated progressions (0 triads, 1 sevenths, 2 ninths)
    #[serde(default)]
    pub ext: u8,
    pub bars_per_chord: Vec<(u32, f64)>,
    pub forms: Vec<Vec<SectionSpec>>,
    pub drums: Vec<LaneSpec>,
    /// drum machines / kits from the sample packs a song may play its drums on (bank name, weight); the remaining
    /// weight up to 1 keeps the lanes' own sounds
    #[serde(default)]
    pub kits: Vec<(String, f64)>,
    pub parts: Vec<PartSpec>,
    #[serde(default)]
    pub loops: Vec<LoopSpec>,
    pub transitions: TransitionSpec,
    pub mix: MixSpec,
}

/// the genres claudius comes with (`all` adds the user's own)
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|(name, _)| *name)
}

fn user_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("claudius").join("genres"))
}

pub fn all() -> Result<Vec<GenreProfile>> {
    let mut out: BTreeMap<String, GenreProfile> = BTreeMap::new();
    for (name, src) in BUILTIN {
        let g: GenreProfile = serde_json::from_str(src).with_context(|| format!("built-in genre {name}"))?;
        out.insert(g.name.clone(), g);
    }
    if let Some(dir) = user_dir()
        && let Ok(rd) = std::fs::read_dir(&dir)
    {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "json") {
                let src = std::fs::read_to_string(&p)?;
                let g: GenreProfile =
                    serde_json::from_str(&src).with_context(|| format!("genre file {}", p.display()))?;
                out.insert(g.name.clone(), g);
            }
        }
    }
    Ok(out.into_values().collect())
}

pub fn get(name: &str) -> Result<GenreProfile> {
    let key = name.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    all()?.into_iter().find(|g| g.name == key).with_context(|| {
        format!(
            "unknown genre '{name}' (try: {})",
            all().map(|v| v.iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", ")).unwrap_or_default()
        )
    })
}
