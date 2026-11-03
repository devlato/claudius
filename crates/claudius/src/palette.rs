//! The sounds a generated song may play. A part plays one of its genre's own sounds (the part's `presets`) some of
//! the time, its genre's `core` chance for the role; otherwise it draws a kind from the genre's `palette` for the
//! role, by weight: `synth`, any synth preset made for the role, varied so that no two songs play quite the same
//! patch, or a category of the installed sample packs' instruments (`strings`, `brass`, `mallets` ...). An
//! instrument is drawn only when its range holds the part's register, it suits the role (a pad sustains: no
//! pizzicato, no piano), and its pack's licence lets music made with it be published freely: packs without a
//! licence, and share-alike or non-commercial ones, are left to be chosen by hand.
//!
//! A drum lane plays its genre's kit, or the synthesized voice, and some of the time (the genre's `kit_mix`) a kit
//! of its own: another of the installed kits, or the synthesized voice, so that a song's drums come from more than
//! one machine.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::model::Role;
use crate::rng::Rng;

/// how a genre draws one role's sounds: the chance of one of its own (`core`), and the weight of each kind
#[derive(Clone, Debug, Deserialize)]
pub struct RolePalette {
    #[serde(default = "half")]
    pub core: f64,
    #[serde(flatten)]
    pub kinds: BTreeMap<String, f64>,
}

fn half() -> f64 {
    0.5
}

/// a role's palette when its genre gives none
pub fn default_for(role: Role) -> RolePalette {
    let (core, kinds): (f64, &[(&str, f64)]) = match role {
        Role::Bass => (0.6, &[("synth", 3.0), ("bass", 1.5), ("strings", 0.3), ("brass", 0.2)]),
        Role::Chords => (
            0.45,
            &[
                ("synth", 2.0),
                ("piano", 1.5),
                ("electric piano", 1.5),
                ("organ", 0.8),
                ("guitar", 1.0),
                ("strings", 0.8),
                ("brass", 0.3),
                ("harp", 0.3),
                ("mallets", 0.4),
            ],
        ),
        Role::Keys => (
            0.45,
            &[
                ("synth", 1.5),
                ("piano", 2.0),
                ("electric piano", 2.0),
                ("organ", 1.0),
                ("mallets", 0.6),
                ("harp", 0.3),
                ("guitar", 0.5),
            ],
        ),
        Role::Pad => (
            0.45,
            &[
                ("synth", 2.5),
                ("strings", 1.5),
                ("pad", 1.5),
                ("choir", 0.6),
                ("organ", 0.5),
                ("woodwinds", 0.3),
                ("brass", 0.3),
            ],
        ),
        Role::Lead => (
            0.4,
            &[
                ("synth", 3.0),
                ("woodwinds", 1.2),
                ("brass", 0.8),
                ("mallets", 1.0),
                ("strings", 0.8),
                ("piano", 0.8),
                ("electric piano", 0.5),
                ("guitar", 0.6),
                ("voice", 0.3),
            ],
        ),
        Role::Arp => (
            0.4,
            &[
                ("synth", 2.5),
                ("mallets", 1.5),
                ("harp", 1.0),
                ("piano", 1.0),
                ("guitar", 0.6),
                ("strings", 0.4),
                ("electric piano", 0.6),
            ],
        ),
    };
    RolePalette { core, kinds: kinds.iter().map(|(k, w)| (k.to_string(), *w)).collect() }
}

/// whether music made with a pack under `licence` may be published freely (no share-alike, no non-commercial
/// terms, and a licence at all)
pub fn publishable(licence: &str) -> bool {
    let l = licence.trim();
    !(l.is_empty()
        || l.eq_ignore_ascii_case("unknown")
        || ["CC-BY-SA", "CC BY-SA", "CC-BY-NC", "CC BY-NC"].iter().any(|p| l.starts_with(p)))
}

/// an installed instrument a part may play: its name (`pack:instrument`), category and recorded range
#[derive(Clone, Debug, PartialEq)]
pub struct Instrument {
    pub name: String,
    pub category: String,
    pub low: u8,
    pub high: u8,
    /// a struck or plucked note that rings out however long it is held (a marimba's), which no pad can be
    pub rings: bool,
}

/// the installed pitched instruments whose packs are publishable (read again when the installed packs change)
pub fn instruments() -> Vec<Instrument> {
    static SEEN: std::sync::Mutex<Option<(Vec<String>, Vec<Instrument>)>> = std::sync::Mutex::new(None);
    let packs: Vec<String> = claudius_packs::installed().iter().map(|p| p.name.clone()).collect();
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((was, list)) = seen.as_ref()
        && *was == packs
    {
        return list.clone();
    }
    let list: Vec<Instrument> = claudius_packs::instruments()
        .into_iter()
        .filter(|i| i.kind == "pitched" && i.variant == 0 && publishable(&i.licence))
        .map(|i| Instrument { name: i.name, category: i.category, low: i.lowest, high: i.highest, rings: i.one_shot })
        .collect();
    *seen = Some((packs, list.clone()));
    list
}

/// whether `i` can play a part of `role` in `register`: most of the register within its range (a few semitones
/// beyond its recorded notes still sound right), and articulations that suit the role
pub fn suits(i: &Instrument, role: Role, register: (i32, i32)) -> bool {
    let (lo, hi) = (i.low as i32 - 5, i.high as i32 + 5);
    let (a, b) = register;
    let inside = (b.min(hi) - a.max(lo)).max(0) as f64;
    if inside < 0.75 * (b - a).max(1) as f64 {
        return false;
    }
    let short = ["pizz", "spic", "stac"].iter().any(|w| i.name.contains(w));
    let instrument = i.name.rsplit(':').next().unwrap_or(&i.name);
    // a whole section of one family playing together, not one player
    let ensemble = ["strings", "woodwinds", "brass"].iter().any(|e| instrument.starts_with(e));
    match role {
        // a pad holds its notes
        Role::Pad => !short && !i.rings && !matches!(i.category.as_str(), "piano" | "mallets" | "guitar" | "harp"),
        // a bass is a low instrument
        Role::Bass => i.low <= 45,
        // a melody is one player's
        Role::Lead => (!ensemble || short) && !instrument.contains("contrabass"),
        // chords on strings are the whole section's
        Role::Chords | Role::Keys => i.category != "strings" || ensemble,
        Role::Arp => true,
    }
}

/// what a sound is, to tell two parts playing the same apart: `synth:` and its preset, or the instrument's name
pub fn identity(s: &crate::model::Sound) -> Option<String> {
    match s {
        crate::model::Sound::Synth { preset, .. } => Some(format!("synth:{preset}")),
        crate::model::Sound::Instrument { name, .. } => Some(name.clone()),
        _ => None,
    }
}

/// what a part plays when it leaves its genre's own sounds: a synth preset for its role, or an instrument
#[derive(Clone, Debug, PartialEq)]
pub enum Drawn {
    Synth(&'static str),
    Instrument(String),
}

/// A sound for a part of `role` in `register`, drawn from `palette` among `available` instruments and not one of
/// `taken` (`identity`: what the song's other parts play), or None when it plays one of its genre's own (`core`).
pub fn draw(
    role: Role,
    register: (i32, i32),
    palette: &RolePalette,
    available: &[Instrument],
    taken: &BTreeSet<String>,
    rng: &mut Rng,
) -> Option<Drawn> {
    if rng.chance(palette.core) {
        return None;
    }
    let fits: Vec<&Instrument> =
        available.iter().filter(|i| suits(i, role, register) && !taken.contains(&i.name)).collect();
    let kinds: Vec<(&str, f64)> =
        palette.kinds.iter().filter(|(_, w)| **w > 0.0).map(|(k, w)| (k.as_str(), *w)).collect();
    let total: f64 = kinds.iter().map(|k| k.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = rng.f64() * total;
    let kind = kinds.iter().find(|(_, w)| {
        x -= w;
        x < 0.0
    })?;
    let of: Vec<&&Instrument> = fits.iter().filter(|i| i.category == kind.0).collect();
    if kind.0 != "synth" && !of.is_empty() {
        return Some(Drawn::Instrument(of[rng.below(of.len())].name.clone()));
    }
    // a synth: any made for the role, or the kind's stand-in when no instrument of it is installed
    let all = crate::synth::presets::drawable(role.name());
    let like: Vec<&str> = stand_ins(kind.0).iter().copied().filter(|p| all.contains(p)).collect();
    let pool = if kind.0 == "synth" || like.is_empty() { all } else { like };
    let free: Vec<&str> = pool.iter().copied().filter(|p| !taken.contains(&format!("synth:{p}"))).collect();
    let presets = if free.is_empty() { pool } else { free };
    presets.get(rng.below(presets.len().max(1))).map(|p| Drawn::Synth(p))
}

/// the synth presets that stand in for a category of instruments when none of them is installed
fn stand_ins(category: &str) -> &'static [&'static str] {
    match category {
        "piano" => &["house_piano", "keys", "tine_piano"],
        "electric piano" => &["epiano", "tine_piano", "keys"],
        "organ" => &["organ", "rock_organ"],
        "mallets" => &["mallet", "fm_marimba", "fm_kalimba", "bell"],
        "guitar" => &["harp_pluck", "pluck", "clav"],
        "harp" => &["harp_pluck"],
        "strings" => &["strings", "solina"],
        "brass" => &["brass_lead", "brass_stab"],
        "woodwinds" => &["whistle", "fm_lead"],
        "choir" | "voice" => &["vowel_pad", "vox_lead"],
        "pad" => &["pad", "warm_pad", "glass_pad", "air_pad", "pwm_pad"],
        "bass" => &["deep_bass", "rubber_bass", "fm_bass", "moog_bass"],
        _ => &[],
    }
}

/// a drum kit a lane may play: an installed pack with drum sounds and a publishable licence
pub fn kits() -> Vec<String> {
    claudius_packs::installed()
        .iter()
        .filter(|p| publishable(&p.licence) && p.categories.iter().any(|c| c == "drums"))
        .map(|p| p.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(name: &str, category: &str, low: u8, high: u8) -> Instrument {
        Instrument { name: name.into(), category: category.into(), low, high, rings: false }
    }

    #[test]
    fn only_packs_free_to_publish_with_are_drawn() {
        for ok in ["CC0-1.0", "CC-BY-3.0", "Public domain", "Unlicense", "jRhodes licence: free for making music"] {
            assert!(publishable(ok), "{ok}");
        }
        for no in ["unknown", "", "CC-BY-SA-4.0", "CC BY-NC 4.0"] {
            assert!(!publishable(no), "{no}");
        }
    }

    #[test]
    fn an_instrument_suits_a_part_by_range_and_by_how_it_plays() {
        let flute = inst("vsco2:flute", "woodwinds", 60, 96);
        assert!(suits(&flute, Role::Lead, (64, 86)));
        assert!(!suits(&flute, Role::Bass, (28, 52)), "too high for a bass");
        let pizz = inst("vsco2:strings_pizz", "strings", 40, 86);
        assert!(suits(&pizz, Role::Arp, (60, 84)) && !suits(&pizz, Role::Pad, (50, 76)), "a pad holds its notes");
        assert!(!suits(&inst("vsco2:woodwinds", "woodwinds", 34, 96), Role::Lead, (64, 86)), "a section is no soloist");
        assert!(suits(&inst("vsco2:strings_pizz", "strings", 40, 86), Role::Lead, (64, 86)), "a pizzicato line is");
        assert!(!suits(&inst("vsco2:cellos", "strings", 36, 77), Role::Chords, (48, 72)), "chords on the section");
        let choir = inst("voice:choir", "choir", 36, 58);
        assert!(!suits(&choir, Role::Pad, (60, 84)), "not up there");
        assert!(!suits(&inst("salamander:piano", "piano", 21, 108), Role::Pad, (50, 76)));
    }

    #[test]
    fn a_draw_leaves_the_genres_sounds_by_its_palette() {
        let available = [inst("vsco2:flute", "woodwinds", 60, 96), inst("vcsl:marimba", "mallets", 41, 96)];
        let palette = RolePalette {
            core: 0.0,
            kinds: [("synth".to_string(), 1.0), ("woodwinds".into(), 1.0), ("brass".into(), 5.0)].into(),
        };
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let d = draw(Role::Lead, (64, 86), &palette, &available, &BTreeSet::new(), &mut Rng::new(seed)).unwrap();
            assert_ne!(d, Drawn::Instrument("vcsl:marimba".into()), "mallets are not in this palette");
            seen.insert(match d {
                Drawn::Synth(p) => p.to_string(),
                Drawn::Instrument(n) => n,
            });
        }
        // no brass installed: a synth brass stands in
        let synths: Vec<&str> = seen.iter().filter(|s| *s != "vsco2:flute").map(String::as_str).collect();
        assert!(seen.contains("vsco2:flute") && synths.contains(&"brass_lead"), "{seen:?}");
        let core = RolePalette { core: 1.0, kinds: palette.kinds.clone() };
        assert_eq!(draw(Role::Lead, (64, 86), &core, &available, &BTreeSet::new(), &mut Rng::new(1)), None);
        // what another part plays is not drawn again
        let taken: BTreeSet<String> = ["vsco2:flute".to_string()].into();
        let only = RolePalette { core: 0.0, kinds: [("woodwinds".to_string(), 1.0)].into() };
        let d = draw(Role::Lead, (64, 86), &only, &available, &taken, &mut Rng::new(1));
        assert!(matches!(d, Some(Drawn::Synth("whistle" | "fm_lead"))), "a synth woodwind stands in: {d:?}");
    }
}
