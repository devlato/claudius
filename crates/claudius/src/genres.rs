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
    ("trip_hop", include_str!("../data/genres/trip_hop.json")),
    ("big_beat", include_str!("../data/genres/big_beat.json")),
    ("electro_house", include_str!("../data/genres/electro_house.json")),
    ("nu_disco", include_str!("../data/genres/nu_disco.json")),
    ("filter_house", include_str!("../data/genres/filter_house.json")),
    ("dance_pop", include_str!("../data/genres/dance_pop.json")),
    ("neo_psychedelia", include_str!("../data/genres/neo_psychedelia.json")),
    ("art_rock", include_str!("../data/genres/art_rock.json")),
    ("idm", include_str!("../data/genres/idm.json")),
    ("dub_techno", include_str!("../data/genres/dub_techno.json")),
    ("boom_bap", include_str!("../data/genres/boom_bap.json")),
    ("uk_drill", include_str!("../data/genres/uk_drill.json")),
    ("dubstep", include_str!("../data/genres/dubstep.json")),
    ("uk_garage", include_str!("../data/genres/uk_garage.json")),
    ("jungle", include_str!("../data/genres/jungle.json")),
    ("afrobeats", include_str!("../data/genres/afrobeats.json")),
    ("amapiano", include_str!("../data/genres/amapiano.json")),
    ("reggaeton", include_str!("../data/genres/reggaeton.json")),
    ("trance", include_str!("../data/genres/trance.json")),
    ("neo_soul", include_str!("../data/genres/neo_soul.json")),
];

fn straight() -> (f32, f32) {
    (0.5, 0.5)
}
fn kit_mix() -> f64 {
    0.3
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
    /// the chance it plays one of its own presets, in place of its genre's palette's for the role (an acid line
    /// stays an acid line)
    #[serde(default)]
    pub core: Option<f64>,
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
    /// production flaws of the ears' (`ears::FLAWS`) that are this genre's sound, such as lofi's hiss: the
    /// production reading leaves them out
    #[serde(default)]
    pub accepted_flaws: Vec<String>,
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
    /// how each role's sounds are drawn beyond the parts' own presets (`palette`): by role name
    #[serde(default)]
    pub palette: BTreeMap<String, crate::palette::RolePalette>,
    /// the chance that a drum lane plays a kit of its own rather than the song's
    #[serde(default = "kit_mix")]
    pub kit_mix: f64,
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

/// how many genres one fusion takes at most
pub const MOST_FUSED: usize = 3;

/// A genre by name, or two or three fused (`trip_hop+idm`): the first leads and each after it plays the drums
/// (`fuse`).
pub fn get(name: &str) -> Result<GenreProfile> {
    if name.contains('+') {
        let names: Vec<&str> = name.split('+').map(str::trim).collect();
        if names.len() > MOST_FUSED || names.iter().any(|n| n.is_empty()) {
            anyhow::bail!("a fusion is two or three genres joined by +, such as trip_hop+idm, not '{name}'");
        }
        let first = get(names[0])?;
        return names[1..].iter().try_fold(first, |g, n| Ok(fuse(&g, &get(n)?)));
    }
    let key = name.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    all()?.into_iter().find(|g| g.name == key).with_context(|| {
        format!(
            "unknown genre '{name}' (try: {})",
            all().map(|v| v.iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", ")).unwrap_or_default()
        )
    })
}

/// the lanes that are drums, for the levels a fusion takes from the genre that plays its drums
const DRUM_ROLES: &[&str] = &["kick", "snare", "clap", "hat", "ohat", "shaker", "perc", "ride", "tom", "crash", "top"];

/// Two genres as one. `a` leads: its tempo, keys, harmony, forms, parts, loops, transitions and mix, and what the ears
/// listen for. `b` plays the drums: its lanes, kits, drums bus and drum levels, in `a`'s sections where `a`'s drums
/// played. Its harmony comes in now and then, its first preset and its styles join `a`'s parts of the same role (so
/// that each part mostly sounds as it did), and its parts of roles `a` has none of come in now and then too. Sounding like `b` no longer counts against the song.
pub fn fuse(a: &GenreProfile, b: &GenreProfile) -> GenreProfile {
    let mut g = a.clone();
    g.name = format!("{}+{}", a.name, b.name);
    g.description = format!(
        "{} Fused with {}: its drums and their sound, and its sounds and styles in the parts.",
        a.description.trim_end_matches('.'),
        b.name.replace('_', " ")
    );
    g.alts.retain(|alt| *alt != b.query);
    for flaw in &b.accepted_flaws {
        if !g.accepted_flaws.contains(flaw) {
            g.accepted_flaws.push(flaw.clone());
        }
    }
    g.swing = ((a.swing.0 + b.swing.0) / 2.0, (a.swing.1 + b.swing.1) / 2.0);
    g.humanize = a.humanize.max(b.humanize);
    // b's progressions a quarter of the time
    for h in g.harmony.iter_mut() {
        h.1 *= 0.75;
    }
    for (style, w) in &b.harmony {
        match g.harmony.iter_mut().find(|h| h.0 == *style) {
            Some(h) => h.1 += w * 0.25,
            None => g.harmony.push((style.clone(), w * 0.25)),
        }
    }
    // b's drums play where a's did: a lane of the same name where that lane played, b's others where most of a's did
    let lanes_a: Vec<&str> = a.drums.iter().map(|d| d.name.as_str()).collect();
    let lanes_b: Vec<String> = b.drums.iter().map(|d| d.name.clone()).collect();
    let mut added: Vec<String> = Vec::new();
    for p in &b.parts {
        if !a.parts.iter().any(|q| q.role == p.role || q.name == p.name) {
            added.push(p.name.clone());
        }
    }
    for form in g.forms.iter_mut() {
        for s in form.iter_mut() {
            let drums_a = s.layers.iter().filter(|l| lanes_a.contains(&l.as_str())).count();
            let most = drums_a * 2 >= lanes_a.len().max(1);
            let mut layers: Vec<String> = s.layers.iter().filter(|l| !lanes_a.contains(&l.as_str())).cloned().collect();
            for lane in &lanes_b {
                if s.layers.contains(lane) || (!lanes_a.contains(&lane.as_str()) && most) {
                    layers.push(lane.clone());
                }
            }
            if s.energy >= 0.6 {
                layers.extend(added.iter().cloned());
            }
            s.layers = layers;
        }
    }
    g.drums = b.drums.clone();
    g.kits = b.kits.clone();
    for p in g.parts.iter_mut() {
        if let Some(q) = b.parts.iter().find(|q| q.role == p.role) {
            if let Some(preset) = q.presets.first().filter(|x| !p.presets.contains(x)) {
                p.presets.push(preset.clone());
            }
            for style in &q.styles {
                if !p.styles.contains(style) {
                    p.styles.push(style.clone());
                }
            }
        }
    }
    for q in b.parts.iter().filter(|q| added.contains(&q.name)) {
        let mut q = q.clone();
        q.optional = q.optional.max(0.4);
        g.parts.push(q);
    }
    for l in &b.loops {
        if !g.loops.iter().any(|x| x.name == l.name) {
            g.loops.push(l.clone());
        }
    }
    match b.mix.buses.get("drums") {
        Some(chain) => g.mix.buses.insert("drums".into(), chain.clone()),
        None => g.mix.buses.remove("drums"),
    };
    g.mix.levels.retain(|role, _| !DRUM_ROLES.contains(&role.as_str()));
    for (role, db) in &b.mix.levels {
        if DRUM_ROLES.contains(&role.as_str()) {
            g.mix.levels.insert(role.clone(), *db);
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fusion_keeps_the_first_genre_and_takes_the_seconds_drums() {
        let (a, b) = (get("trip_hop").unwrap(), get("idm").unwrap());
        let g = get("Trip-Hop + IDM").unwrap();
        assert_eq!((g.name.as_str(), g.bpm, &g.query), ("trip_hop+idm", a.bpm, &a.query));
        assert_eq!(
            g.drums.iter().map(|d| &d.name).collect::<Vec<_>>(),
            b.drums.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        assert_eq!((&g.kits, g.mix.buses.get("drums")), (&b.kits, b.mix.buses.get("drums")));
        assert_eq!(
            (g.mix.lufs, &g.mix.master, g.mix.buses.get("music")),
            (a.mix.lufs, &a.mix.master, a.mix.buses.get("music"))
        );
        // the parts: trip-hop's, the lead with the bells' presets too, and IDM's arpeggio, which trip-hop has none of
        let lead = g.parts.iter().find(|p| p.name == "lead").unwrap();
        let bells = &b.parts.iter().find(|p| p.role == lead.role).unwrap().presets[0];
        assert!(lead.presets.ends_with(std::slice::from_ref(bells)), "{:?}", lead.presets);
        let arp = g.parts.iter().find(|p| p.name == "arp").expect("the arp comes in");
        assert!(arp.optional >= 0.4);
        // IDM's lanes play where trip-hop's did: a section with most of the drums has them all, the intro's one
        // lane none of them but its own name's
        let hook = g.forms[0].iter().find(|s| s.name == "hook").unwrap();
        for lane in ["kick", "snare", "hat", "click", "tom", "arp"] {
            assert!(hook.layers.iter().any(|l| l == lane), "{lane} in {:?}", hook.layers);
        }
        assert!(!hook.layers.iter().any(|l| l == "ohat"), "trip-hop's open hat goes with its drums");
        let intro = &g.forms[0][0];
        assert!(!intro.layers.iter().any(|l| l == "click" || l == "kick"), "{:?}", intro.layers);
        // the harmony leans on the first genre's, the flaws either genre accepts are accepted
        let lead_weight: f64 = g.harmony.iter().filter(|h| a.harmony.iter().any(|x| x.0 == h.0)).map(|h| h.1).sum();
        assert!(lead_weight >= 0.75);
        assert!(g.accepted_flaws.iter().any(|f| f == "a cheap synthesized beep"));
    }

    #[test]
    fn a_fusion_says_what_it_takes() {
        assert!(get("house+techno+dnb").unwrap().name == "house+techno+dnb");
        let e = |n: &str| get(n).unwrap_err().to_string();
        assert!(e("house+techno+dnb+trap").contains("two or three genres"));
        assert!(e("house+").contains("two or three genres"));
        assert!(e("house+polka").contains("unknown genre 'polka'"));
        // what the second genre is no longer counts against the first
        let g = get("house+dnb").unwrap();
        assert!(!g.alts.iter().any(|a| a == "drum and bass music"), "{:?}", g.alts);
    }

    #[test]
    fn the_accepted_flaws_are_flaws_the_ears_know() {
        for (name, src) in BUILTIN {
            let g: GenreProfile = serde_json::from_str(src).unwrap();
            for flaw in &g.accepted_flaws {
                assert!(crate::ears::FLAWS.contains(&flaw.as_str()), "{name}: {flaw}");
            }
        }
    }
}
