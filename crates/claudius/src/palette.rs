//! The sounds a generated song may play. A part plays one of its genre's own sounds (the part's `presets`) some of
//! the time, its genre's `core` chance for the role; otherwise it draws a kind from the genre's `palette` for the
//! role, by weight: `synth`, a synth preset that fits the genre's parts of the role, varied so that no two songs play
//! quite the same patch, or a category of the installed sample packs' instruments (`strings`, `brass`, `mallets` ...),
//! one whose sound pierces a mix only where the genre plays one of its kind itself. An
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

/// How a genre draws one role's sounds: the chance of one of its own (`core`), the weight of each kind, the synth
/// presets that fit it (`synths`: what a draw of `synth` plays, and those that stand in for a kind of instrument
/// none of which is installed), and the instruments that fit it (`instruments`, by name or alias: installed, they
/// play for their kind, one whose sound pierces a mix too). None given: the synths that fit its genre's parts of the
/// role (`compose::fitting_synths`), and the instruments those parts name (`compose::named_instruments`).
#[derive(Clone, Debug, Deserialize)]
pub struct RolePalette {
    #[serde(default = "half")]
    pub core: f64,
    #[serde(default)]
    pub synths: Vec<String>,
    #[serde(default)]
    pub instruments: Vec<String>,
    /// whether it plays the sounds of an era gone (`dated`): synthwave's FM piano and string machine
    #[serde(default)]
    pub retro: bool,
    #[serde(flatten)]
    pub kinds: BTreeMap<String, f64>,
}

/// General MIDI's sounds: a fallback, played only where nothing else of its kind is installed
fn general_midi(name: &str) -> bool {
    name.starts_with("freepats:")
}

/// Instruments that date a song to the era they come from: the FM and early digital keyboards (an FM piano, a
/// CP-80, a Pianet) and General MIDI's synth pads, strings, brass, leads and basses, the cheap polyphony of an 80s
/// or 90s sound card. A genre plays one only where its palette names it or is `retro`.
pub fn dated(name: &str) -> bool {
    let instrument = name.rsplit(':').next().unwrap_or(name);
    matches!(name, "epianos:cp80" | "epianos:pianet")
        || instrument.starts_with("fm_")
        || (general_midi(name) && ["synth_", "_pad", "bass_lead", "square_lead"].iter().any(|w| instrument.contains(w)))
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
    RolePalette {
        core,
        synths: Vec::new(),
        instruments: Vec::new(),
        retro: false,
        kinds: kinds.iter().map(|(k, w)| (k.to_string(), *w)).collect(),
    }
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
        // chords on strings, brass or woodwinds are the whole section's: one player plays one note
        Role::Chords | Role::Keys => !matches!(i.category.as_str(), "strings" | "brass" | "woodwinds") || ensemble,
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

/// the kinds of instrument whose sound pierces a mix (a glockenspiel, a piccolo, a trumpet): a genre plays one only
/// where one of its own synths stands in for it
const PIERCING: &[&str] = &["mallets", "woodwinds", "brass"];

/// the palette's synths: those it names, or any made for the role when it names none (of their era only for a
/// `retro` palette: `presets::DATED`)
fn pool(role: Role, palette: &RolePalette) -> Vec<&'static str> {
    if palette.synths.is_empty() {
        let dated = |p: &&str| !palette.retro && crate::synth::presets::DATED.contains(p);
        crate::synth::presets::drawable(role.name()).into_iter().filter(|p| !dated(p)).collect()
    } else {
        crate::synth::presets::names().into_iter().filter(|n| palette.synths.iter().any(|s| s == n)).collect()
    }
}

/// the synths that may play `kind`: the pool for `synth`; for another kind, those of the pool that stand in for it,
/// or, for one that does not pierce a mix, the stand-ins made for the role
fn players(kind: &str, role: Role, pool: &[&'static str]) -> Vec<&'static str> {
    if kind == "synth" {
        return pool.to_vec();
    }
    let among = |of: &[&'static str]| -> Vec<&'static str> {
        stand_ins(kind).iter().copied().filter(|p| of.contains(p)).collect()
    };
    let own = among(pool);
    if own.is_empty() && !PIERCING.contains(&kind) {
        let drawable = crate::synth::presets::drawable(role.name());
        among(&drawable.into_iter().filter(|p| !crate::synth::presets::DATED.contains(p)).collect::<Vec<_>>())
    } else {
        own
    }
}

/// the installed instruments `palette` names (`pack:instrument`), its aliases answered by the installed packs
fn named(palette: &RolePalette) -> Vec<String> {
    palette
        .instruments
        .iter()
        .filter_map(|n| {
            if n.contains(':') { Some(n.clone()) } else { claudius_packs::resolve(n, None).map(|r| r.to_string()) }
        })
        .collect()
}

/// whether `i` may play for its kind where the palette's synths are `pool`: an instrument with the character of a
/// synth where that synth is the palette's, one of a kind that pierces a mix where one of them stands in for it, and
/// any other
fn admitted(i: &Instrument, role: Role, pool: &[&'static str]) -> bool {
    character(&i.name).map_or_else(
        || !PIERCING.contains(&i.category.as_str()) || !players(&i.category, role, pool).is_empty(),
        |s| pool.contains(&s),
    )
}

/// Whether the installed instrument `name` is one a genre whose palette for `role` is `palette` has a place for: one
/// it names, or one of a kind it lists and would draw. An instrument that is not installed is not judged.
pub fn instrument_has_place(role: Role, palette: &RolePalette, name: &str) -> bool {
    let Some(i) = instruments().into_iter().find(|i| i.name == name) else { return true };
    named(palette).contains(&i.name)
        || (palette.kinds.get(&i.category).is_some_and(|w| *w > 0.0)
            && admitted(&i, role, &pool(role, palette))
            && (palette.retro || !dated(&i.name)))
}

/// every synth a draw from `palette` may play for a part of `role`
pub fn synths_drawn(role: Role, palette: &RolePalette) -> Vec<&'static str> {
    let pool = pool(role, palette);
    let mut out = pool.clone();
    for kind in palette.kinds.iter().filter(|(_, w)| **w > 0.0).map(|(k, _)| k) {
        for p in players(kind, role, &pool) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// A sound for a part of `role` in `register`, drawn from `palette` among `available` instruments and not one of
/// `taken` (`identity`: what the song's other parts play), or None when it plays one of its genre's own (`core`).
/// A kind is drawn when something of it can play that no other part plays: an installed instrument of it that suits
/// the part, or a synth that stands in for it. One whose sound pierces a mix (`PIERCING`) is drawn only where one of
/// the palette's synths stands in for it, and an instrument with the character of a synth only where that synth is
/// the palette's (`character`), unless the palette names the instrument. So a song sounds only what its genre has a
/// place for: a genre that plays no whistle plays only the flute it names, and one that plays no bell no
/// glockenspiel. `synth` plays one of the palette's synths (any made for the role, when it names none).
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
    let pool = pool(role, palette);
    let named = named(palette);
    let fits: Vec<&Instrument> = available
        .iter()
        .filter(|i| named.contains(&i.name) || admitted(i, role, &pool) && (palette.retro || !dated(&i.name)))
        .filter(|i| suits(i, role, register) && !taken.contains(&i.name))
        .collect();
    // General MIDI's only where nothing of its kind but General MIDI's can play the part
    let better = |category: &str| fits.iter().any(|i| i.category == category && !general_midi(&i.name));
    let fits: Vec<&Instrument> = fits
        .iter()
        .copied()
        .filter(|i| named.contains(&i.name) || !general_midi(&i.name) || !better(&i.category))
        .collect();
    // the synths that may play a kind, which no other part of the song plays
    let synths_for = |kind: &str| -> Vec<&'static str> {
        players(kind, role, &pool).into_iter().filter(|p| !taken.contains(&format!("synth:{p}"))).collect()
    };
    let kinds: Vec<(&str, f64)> = palette
        .kinds
        .iter()
        .filter(|(k, w)| **w > 0.0 && (fits.iter().any(|i| i.category == **k) || !synths_for(k).is_empty()))
        .map(|(k, w)| (k.as_str(), *w))
        .collect();
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
    let options = synths_for(kind.0);
    options.get(rng.below(options.len().max(1))).map(|p| Drawn::Synth(p))
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
        "woodwinds" => &["whistle"],
        "choir" | "voice" => &["vowel_pad", "vox_lead"],
        "pad" => &["pad", "warm_pad", "glass_pad", "air_pad", "pwm_pad"],
        "bass" => &["deep_bass", "rubber_bass", "fm_bass", "moog_bass"],
        _ => &[],
    }
}

/// the synth an installed instrument sounds like, where its category holds instruments of more than one character:
/// a glockenspiel rings as the bell does, so it plays only where a genre has a place for a bell
fn character(instrument: &str) -> Option<&'static str> {
    const LIKE: &[(&[&str], &str)] = &[
        (&["glockenspiel", "tubular", "celesta", "xylophone", "chime", "bell"], "bell"),
        (&["vibraphone", "vibes"], "mallet"),
        (&["marimba"], "fm_marimba"),
        (&["kalimba", "mbira"], "fm_kalimba"),
    ];
    let name = instrument.rsplit(':').next().unwrap_or(instrument);
    LIKE.iter().find(|(words, _)| words.iter().any(|w| name.contains(w))).map(|(_, synth)| *synth)
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
        assert!(!suits(&inst("vsco2:trumpet", "brass", 54, 82), Role::Chords, (55, 76)), "one trumpet, one note");
        assert!(!suits(&inst("vcsl:tenor_sax", "woodwinds", 44, 75), Role::Keys, (50, 72)));
        assert!(suits(&inst("vsco2:brass", "brass", 34, 82), Role::Chords, (55, 76)), "the section plays chords");
        let choir = inst("voice:choir", "choir", 36, 58);
        assert!(!suits(&choir, Role::Pad, (60, 84)), "not up there");
        assert!(!suits(&inst("salamander:piano", "piano", 21, 108), Role::Pad, (50, 76)));
    }

    #[test]
    fn a_draw_leaves_the_genres_sounds_by_its_palette() {
        let available = [inst("vsco2:flute", "woodwinds", 60, 96), inst("vcsl:marimba", "mallets", 41, 96)];
        let palette = RolePalette {
            core: 0.0,
            synths: Vec::new(),
            instruments: Vec::new(),
            retro: false,
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
        let core = RolePalette { core: 1.0, ..palette.clone() };
        assert_eq!(draw(Role::Lead, (64, 86), &core, &available, &BTreeSet::new(), &mut Rng::new(1)), None);
        // what another part plays is not drawn again
        let taken: BTreeSet<String> = ["vsco2:flute".to_string()].into();
        let only = RolePalette { kinds: [("woodwinds".to_string(), 1.0)].into(), ..palette.clone() };
        let d = draw(Role::Lead, (64, 86), &only, &available, &taken, &mut Rng::new(1));
        assert_eq!(d, Some(Drawn::Synth("whistle")), "a synth woodwind stands in");
    }

    #[test]
    fn an_instrument_the_genre_names_plays_where_its_kind_would_not() {
        let winds = [inst("vcsl:tenor_sax", "woodwinds", 44, 75), inst("vsco2:flute", "woodwinds", 60, 96)];
        let palette = RolePalette {
            core: 0.0,
            synths: vec!["vox_lead".into()],
            instruments: vec!["vcsl:tenor_sax".into()],
            retro: false,
            kinds: [("woodwinds".to_string(), 1.0)].into(),
        };
        for seed in 0..20 {
            let d = draw(Role::Lead, (55, 72), &palette, &winds, &BTreeSet::new(), &mut Rng::new(seed));
            assert_eq!(d, Some(Drawn::Instrument("vcsl:tenor_sax".into())), "the saxophone it names, never the flute");
        }
    }

    #[test]
    fn a_genre_sounds_only_the_synths_it_has_a_place_for() {
        // nothing installed, as in a browser: the woodwinds and mallets have no synth of the genre's to stand in
        let palette = RolePalette {
            core: 0.0,
            synths: vec!["epiano".into(), "mallet".into()],
            instruments: Vec::new(),
            retro: false,
            kinds: [("synth".to_string(), 1.0), ("woodwinds".into(), 3.0), ("mallets".into(), 2.0)].into(),
        };
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..300 {
            match draw(Role::Lead, (64, 86), &palette, &[], &BTreeSet::new(), &mut Rng::new(seed)) {
                Some(Drawn::Synth(p)) => seen.insert(p),
                other => panic!("a synth of the genre's: {other:?}"),
            };
        }
        assert_eq!(seen, ["epiano", "mallet"].into(), "no whistle for the flute, no bell for the vibes");
        // a kind the genre plays nothing of is not drawn at all, nor an instrument of it that is installed
        let flutes = RolePalette {
            core: 0.0,
            synths: vec!["epiano".into()],
            instruments: Vec::new(),
            retro: false,
            kinds: [("woodwinds".to_string(), 1.0)].into(),
        };
        let flute = [inst("vsco2:flute", "woodwinds", 60, 96)];
        assert_eq!(draw(Role::Lead, (64, 86), &flutes, &[], &BTreeSet::new(), &mut Rng::new(1)), None);
        assert_eq!(draw(Role::Lead, (64, 86), &flutes, &flute, &BTreeSet::new(), &mut Rng::new(1)), None);
        // the FM lead is no woodwind's stand-in: a genre plays the flute where it plays the whistle
        let reeds = RolePalette { synths: vec!["epiano".into(), "fm_lead".into()], ..flutes };
        assert_eq!(draw(Role::Lead, (64, 86), &reeds, &flute, &BTreeSet::new(), &mut Rng::new(1)), None);
        let whistles = RolePalette { synths: vec!["epiano".into(), "whistle".into()], ..reeds };
        let d = draw(Role::Lead, (64, 86), &whistles, &flute, &BTreeSet::new(), &mut Rng::new(1));
        assert_eq!(d, Some(Drawn::Instrument("vsco2:flute".into())));
    }

    #[test]
    fn a_kind_that_does_not_pierce_a_mix_plays_where_the_palette_lists_it() {
        let rhodes = [inst("jrhodes:rhodes", "electric piano", 29, 96)];
        let keys = RolePalette {
            core: 0.0,
            synths: vec!["pluck".into()],
            instruments: Vec::new(),
            retro: false,
            kinds: [("electric piano".to_string(), 1.0)].into(),
        };
        let d = draw(Role::Lead, (64, 86), &keys, &rhodes, &BTreeSet::new(), &mut Rng::new(1));
        assert_eq!(d, Some(Drawn::Instrument("jrhodes:rhodes".into())));
        // no synth electric piano is made for a lead: with none installed, the kind waits for one
        assert_eq!(draw(Role::Lead, (64, 86), &keys, &[], &BTreeSet::new(), &mut Rng::new(1)), None);
        let voices = RolePalette { kinds: [("voice".to_string(), 1.0)].into(), ..keys };
        let d = draw(Role::Lead, (64, 86), &voices, &[], &BTreeSet::new(), &mut Rng::new(1));
        assert_eq!(d, Some(Drawn::Synth("vox_lead")), "the voice made for a lead stands in");
        assert_eq!(synths_drawn(Role::Lead, &voices), ["pluck", "vox_lead"]);
    }

    #[test]
    fn a_glockenspiel_plays_where_a_bell_does_and_a_vibraphone_where_a_mallet_does() {
        let available = [inst("vcsl:glockenspiel", "mallets", 67, 108), inst("vcsl:vibraphone", "mallets", 53, 89)];
        let drawn = |synths: &[&str]| {
            let palette = RolePalette {
                core: 0.0,
                synths: synths.iter().map(|s| s.to_string()).collect(),
                instruments: Vec::new(),
                retro: false,
                kinds: [("mallets".to_string(), 1.0)].into(),
            };
            let mut seen = BTreeSet::new();
            for seed in 0..50 {
                if let Some(Drawn::Instrument(n)) =
                    draw(Role::Lead, (70, 86), &palette, &available, &BTreeSet::new(), &mut Rng::new(seed))
                {
                    seen.insert(n);
                }
            }
            seen
        };
        assert_eq!(drawn(&["mallet", "fm_marimba"]), ["vcsl:vibraphone".to_string()].into());
        assert_eq!(drawn(&["bell", "pluck"]), ["vcsl:glockenspiel".to_string()].into());
    }

    fn drawn(role: Role, register: (i32, i32), palette: &RolePalette, available: &[Instrument]) -> BTreeSet<String> {
        (0..60)
            .filter_map(|seed| match draw(role, register, palette, available, &BTreeSet::new(), &mut Rng::new(seed)) {
                Some(Drawn::Instrument(n)) => Some(n),
                Some(Drawn::Synth(p)) => Some(format!("synth:{p}")),
                None => None,
            })
            .collect()
    }

    #[test]
    fn a_sound_of_an_era_gone_plays_where_its_genre_is_of_that_era_or_names_it() {
        for old in
            ["epianos:cp80", "epianos:pianet", "freepats:fm_piano", "freepats:synth_strings", "freepats:new_age_pad"]
        {
            assert!(dated(old), "{old}");
        }
        for now in ["epianos:wurlitzer", "jrhodes:rhodes", "freepats:upright_piano", "freepats:clean_guitar"] {
            assert!(!dated(now), "{now}");
        }
        let keys =
            [inst("epianos:cp80", "electric piano", 28, 103), inst("epianos:wurlitzer", "electric piano", 28, 103)];
        let palette = RolePalette {
            core: 0.0,
            synths: vec!["epiano".into()],
            instruments: Vec::new(),
            retro: false,
            kinds: [("electric piano".to_string(), 1.0)].into(),
        };
        assert_eq!(drawn(Role::Keys, (50, 72), &palette, &keys), ["epianos:wurlitzer".to_string()].into());
        let synthwave = RolePalette { retro: true, ..palette.clone() };
        let both: BTreeSet<String> = keys.iter().map(|i| i.name.clone()).collect();
        assert_eq!(drawn(Role::Keys, (50, 72), &synthwave, &keys), both);
        let named = RolePalette { instruments: vec!["epianos:cp80".into()], ..palette.clone() };
        assert!(drawn(Role::Keys, (50, 72), &named, &keys).contains("epianos:cp80"));
        // nor is an 80s preset drawn for a part: the FM and pulse leads, the FM piano standing in for one
        let any = RolePalette { synths: Vec::new(), kinds: [("synth".to_string(), 1.0)].into(), ..palette };
        let leads = synths_drawn(Role::Lead, &any);
        assert!(!leads.iter().any(|p| crate::synth::presets::DATED.contains(p)), "{leads:?}");
        let retro = synths_drawn(Role::Lead, &RolePalette { retro: true, ..any.clone() });
        assert!(retro.contains(&"fm_lead") && retro.contains(&"square_lead"), "{retro:?}");
        let pianos = RolePalette { kinds: [("piano".to_string(), 1.0)].into(), ..any };
        assert_eq!(
            drawn(Role::Keys, (50, 72), &pianos, &[]),
            ["synth:house_piano".to_string(), "synth:keys".into()].into()
        );
    }

    #[test]
    fn general_midi_plays_a_kind_only_where_nothing_else_of_it_is_installed() {
        let upright = inst("freepats:upright_piano", "piano", 21, 108);
        let grand = inst("salamander:piano", "piano", 21, 108);
        let palette = RolePalette {
            core: 0.0,
            synths: vec!["keys".into()],
            instruments: Vec::new(),
            retro: false,
            kinds: [("piano".to_string(), 1.0)].into(),
        };
        let both = [upright.clone(), grand.clone()];
        assert_eq!(drawn(Role::Keys, (50, 72), &palette, &both), [grand.name.clone()].into());
        assert_eq!(
            drawn(Role::Keys, (50, 72), &palette, std::slice::from_ref(&upright)),
            [upright.name.clone()].into()
        );
        let named = RolePalette { instruments: vec![upright.name.clone()], ..palette };
        assert!(drawn(Role::Keys, (50, 72), &named, &both).contains(&upright.name), "one the genre names plays");
    }
}
