//! The mock's songs: a plausible arrangement per genre (form, drum patterns, parts, mix), its script, and a critic
//! that reacts to edits (energy arc, mix balance, harmony) the way claudius' verdict reads.

use claudius_api::{
    DrumView, EarsView, GenreInfo, PackInfo, PartView, SectionView, SongView, SoundList, TrackView, VerdictView,
};

use super::Rng;
use crate::music::{self, Key};

#[derive(Clone, Copy, PartialEq)]
pub enum Feel {
    FourFloor,
    HalfTime,
    Breakbeat,
    BoomBap,
    Straight,
}

type PartSpec = (&'static str, &'static str, &'static [&'static str], &'static [&'static str], (i32, i32));

pub struct Genre {
    pub name: &'static str,
    pub description: &'static str,
    pub bpm: (f64, f64),
    pub swing: (f32, f32),
    pub feel: Feel,
    pub drums: &'static [&'static str],
    pub parts: &'static [PartSpec],
    pub form: &'static [(&'static str, &'static str, u32)],
    pub modes: &'static [&'static str],
    pub query: &'static str,
    pub alts: &'static [&'static str],
}

const BASS_HOUSE: &[&str] = &["pluck_bass", "organ_bass", "sub_bass", "saw_bass"];
const CHORD_PRESETS: &[&str] = &["organ", "stab", "epiano", "keys", "pluck", "strings"];
const PAD_PRESETS: &[&str] = &["strings", "warm_pad", "pad"];
const LEAD_PRESETS: &[&str] = &["supersaw", "pluck", "bell", "square_lead", "chip"];
const ARP_PRESETS: &[&str] = &["pluck", "bell", "chip"];

pub const GENRES: &[Genre] = &[
    Genre {
        name: "house",
        description: "House: four-on-the-floor kick, offbeat hats, claps on 2 and 4, soulful 7th chords, a bouncing offbeat or syncopated bass, breakdown and build into the drop",
        bpm: (120.0, 126.0),
        swing: (0.5, 0.56),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat", "shaker", "perc"],
        parts: &[
            ("bass", "bass", &["offbeat", "syncopated"], BASS_HOUSE, (33, 52)),
            ("chords", "chords", &["stabs", "offbeat", "syncopated", "charleston"], CHORD_PRESETS, (55, 76)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("lead", "lead", &["motif"], LEAD_PRESETS, (64, 86)),
            ("arp", "arp", &["up", "updown", "pinky"], ARP_PRESETS, (62, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 16),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("break2", "break", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "minor", "dorian", "major"],
        query: "house music",
        alts: &["techno music", "minimal electronic music", "upbeat corporate pop music"],
    },
    Genre {
        name: "deep_house",
        description: "Deep house: softer four-on-the-floor, shuffled hats and shakers, minor 9th chords on electric piano and organ, a round syncopated bass, long warm reverbs",
        bpm: (118.0, 124.0),
        swing: (0.53, 0.58),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "shaker", "perc"],
        parts: &[
            ("bass", "bass", &["syncopated", "offbeat"], BASS_HOUSE, (33, 50)),
            (
                "chords",
                "chords",
                &["charleston", "syncopated", "stabs", "half"],
                &["epiano", "organ", "keys"],
                (52, 74),
            ),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("lead", "lead", &["motif"], &["pluck", "bell", "epiano"], (62, 84)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 16),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian"],
        query: "deep house music",
        alts: &["house music", "lounge music", "jazz"],
    },
    Genre {
        name: "techno",
        description: "Techno: a driving distorted kick, offbeat hats and rides, rolling bass locked between the kicks, one- or two-chord stabs in minor or phrygian, long builds of layers, optional acid line",
        bpm: (126.0, 134.0),
        swing: (0.5, 0.52),
        feel: Feel::FourFloor,
        drums: &["kick", "hat", "ohat", "clap", "ride", "perc"],
        parts: &[
            ("bass", "bass", &["rolling", "offbeat", "pulse"], &["reese", "saw_bass", "sub_bass"], (28, 45)),
            ("acid", "bass", &["acid"], &["acid"], (36, 55)),
            ("chords", "chords", &["stabs", "offbeat"], &["stab", "organ"], (50, 70)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 72)),
            ("lead", "lead", &["motif"], &["square_lead", "bell", "acid"], (60, 82)),
        ],
        form: &[
            ("intro", "intro", 32),
            ("groove", "groove", 32),
            ("break", "break", 16),
            ("build", "build", 8),
            ("peak", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "phrygian"],
        query: "techno music",
        alts: &["house music", "industrial music", "minimal electronic music"],
    },
    Genre {
        name: "trap",
        description: "Trap: half-time feel with the snare on beat 3, syncopated kicks, 808s gliding on the kick hits, rolling hi-hats, dark minor or harmonic-minor keys and bells, hook / verse form",
        bpm: (130.0, 150.0),
        swing: (0.5, 0.54),
        feel: Feel::HalfTime,
        drums: &["kick", "snare", "clap", "hat", "ohat", "perc"],
        parts: &[
            ("bass", "bass", &["kick_follow"], &["808", "sub_bass"], (28, 45)),
            ("keys", "keys", &["sustain", "half"], &["bell", "keys", "epiano"], (55, 79)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 72)),
            ("lead", "lead", &["motif"], &["bell", "pluck", "square_lead"], (64, 88)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("hook", "hook", 16),
            ("verse", "verse", 16),
            ("hook2", "hook", 16),
            ("verse2", "verse", 8),
            ("hook3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "harmonic_minor"],
        query: "trap music",
        alts: &["hip hop music", "dubstep", "drill music"],
    },
    Genre {
        name: "lofi",
        description: "Lo-fi hip hop: a lazy swung boom-bap beat, jazzy 7th and 9th chords on electric piano, a walking or sustained bass, tape wobble, vinyl crackle and a dusty lowpassed drum bus",
        bpm: (70.0, 90.0),
        swing: (0.56, 0.63),
        feel: Feel::BoomBap,
        drums: &["kick", "snare", "hat", "shaker"],
        parts: &[
            (
                "bass",
                "bass",
                &["walking", "sustain", "syncopated"],
                &["sub_bass", "organ_bass", "pluck_bass"],
                (31, 50),
            ),
            ("keys", "keys", &["sustain", "charleston", "half"], &["epiano", "keys"], (52, 76)),
            ("lead", "lead", &["motif"], &["bell", "epiano", "pluck"], (62, 84)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 74)),
        ],
        form: &[
            ("intro", "intro", 4),
            ("a", "verse", 16),
            ("b", "hook", 16),
            ("a2", "verse", 16),
            ("outro", "outro", 8),
        ],
        modes: &["dorian", "minor", "major"],
        query: "lo-fi hip hop music",
        alts: &["jazz", "hip hop music", "ambient music"],
    },
    Genre {
        name: "dnb",
        description: "Drum and bass: 174 BPM two-step breakbeat (kick on 1 and the and-of-3, snares on 2 and 4 with ghost notes), a growling reese bass, atmospheric pads, long intro and breakdown",
        bpm: (170.0, 176.0),
        swing: (0.5, 0.51),
        feel: Feel::Breakbeat,
        drums: &["kick", "snare", "hat", "ride", "shaker"],
        parts: &[
            ("bass", "bass", &["reese", "sustain", "rolling"], &["reese", "sub_bass", "wobble"], (26, 45)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 76)),
            ("lead", "lead", &["motif"], LEAD_PRESETS, (62, 86)),
            ("arp", "arp", &["up", "updown"], ARP_PRESETS, (60, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("build", "build", 16),
            ("drop", "drop", 32),
            ("break", "break", 16),
            ("drop2", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian"],
        query: "drum and bass music",
        alts: &["jungle music", "dubstep", "breakbeat"],
    },
    Genre {
        name: "future_bass",
        description: "Future bass: half-time drums, lush 7th / 9th supersaw chords chopped in syncopated rhythms and pumped hard by the sidechain, a bright plucky hook, builds into drops",
        bpm: (140.0, 160.0),
        swing: (0.5, 0.53),
        feel: Feel::HalfTime,
        drums: &["kick", "snare", "hat", "ohat", "perc"],
        parts: &[
            ("chords", "chords", &["syncopated", "pump", "sustain"], &["supersaw", "pad", "pluck"], (55, 79)),
            ("bass", "bass", &["sustain", "kick_follow"], &["sub_bass", "808", "wobble"], (28, 47)),
            ("lead", "lead", &["motif"], &["pluck", "supersaw", "chip"], (64, 88)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("arp", "arp", &["up", "updown", "random"], ARP_PRESETS, (62, 88)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("build", "build", 8),
            ("drop", "drop", 16),
            ("break", "break", 8),
            ("build2", "build", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 8),
        ],
        modes: &["major", "minor"],
        query: "future bass music",
        alts: &["electronic dance music", "pop music", "dubstep"],
    },
    Genre {
        name: "synthwave",
        description: "Synthwave: 80s-style straight drums with a big reverberant snare on 2 and 4, a driving eighth-note saw bass, arpeggios, lush pads and a chorused square or supersaw lead over minor progressions",
        bpm: (85.0, 118.0),
        swing: (0.5, 0.5),
        feel: Feel::Straight,
        drums: &["kick", "snare", "hat"],
        parts: &[
            ("bass", "bass", &["root8", "octave"], &["saw_bass", "sub_bass"], (31, 50)),
            ("arp", "arp", &["up", "updown"], ARP_PRESETS, (60, 84)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 74)),
            ("lead", "lead", &["motif"], &["square_lead", "supersaw"], (62, 86)),
            ("chords", "chords", &["half", "sustain"], &["supersaw", "strings", "pad"], (52, 76)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("chorus", "hook", 16),
            ("verse2", "verse", 16),
            ("chorus2", "hook", 16),
            ("bridge", "break", 8),
            ("chorus3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "minor", "dorian"],
        query: "synthwave music",
        alts: &["new wave music", "electronic music", "video game music"],
    },
];

pub fn genre(name: &str) -> &'static Genre {
    GENRES.iter().find(|g| g.name == name).unwrap_or(&GENRES[0])
}

/// what the demo offers to scripts
pub fn sounds() -> SoundList {
    let list = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let pack = |name: &str, description: &str, license: &str, installed: bool, size_mb: f64| PackInfo {
        name: name.into(),
        description: description.into(),
        license: license.into(),
        installed,
        size_mb,
        ..Default::default()
    };
    SoundList {
        drums: list(&[
            "kick", "snare", "clap", "hh", "oh", "ride", "crash", "tom", "rim", "shaker", "cowbell", "snap", "perc",
        ]),
        synths: list(&[
            "sub_bass",
            "808",
            "reese",
            "acid",
            "saw_bass",
            "pluck_bass",
            "organ_bass",
            "wobble",
            "pluck",
            "supersaw",
            "pad",
            "warm_pad",
            "stab",
            "epiano",
            "keys",
            "bell",
            "organ",
            "strings",
            "square_lead",
            "chip",
        ]),
        instruments: list(&[
            "piano", "rhodes", "strings", "tr909:bd", "tr909:sd", "tr909:hh", "tr808:bd", "tr808:cp", "tr808:hh",
        ]),
        packs: vec![
            pack("tr909", "Roland TR-909 one-shots", "CC0", true, 4.2),
            pack("tr808", "Roland TR-808 one-shots", "CC0", true, 3.1),
            pack("salamander-piano", "Salamander Grand Piano, 16 velocity layers", "CC-BY 3.0", false, 380.0),
            pack("vsco2-strings", "VSCO 2 community orchestra strings", "CC0", false, 210.0),
        ],
    }
}

pub fn genre_infos() -> Vec<GenreInfo> {
    GENRES.iter().map(|g| GenreInfo { name: g.name.into(), description: g.description.into(), bpm: g.bpm }).collect()
}

const ADJECTIVES: &[&str] = &[
    "Glass", "Hollow", "Silent", "Liquid", "Neon", "Velvet", "Golden", "Midnight", "Paper", "Electric", "Distant",
    "Crystal", "Static", "Amber", "Lunar", "Copper",
];
const NOUNS: &[&str] = &[
    "Garden", "Harbor", "Circuit", "Avenue", "Echo", "Signal", "Tides", "Mirror", "Engine", "Bloom", "Horizon",
    "Motel", "Lights", "Season", "Arcade", "Orchard",
];

fn progressions(mode: &str, rng: &mut Rng) -> Vec<(String, Vec<String>)> {
    let lib: &[&[&str]] = match mode {
        "major" => &[
            &["Imaj7", "vi7", "ii7", "V7"],
            &["I", "V", "vi", "IV"],
            &["Imaj7", "iii7", "vi7", "IVmaj7"],
            &["IVmaj7", "V7", "iii7", "vi7"],
        ],
        "dorian" => {
            &[&["i7", "IV7"], &["i9", "IV9", "i7", "VIImaj7"], &["i7", "ii7", "IV7", "i7"], &["i7", "IV7", "v7", "IV7"]]
        }
        "phrygian" => &[&["i", "II"], &["i7", "IImaj7", "i7", "vii7"], &["i", "II", "VII", "i"]],
        "harmonic_minor" => &[&["i", "VI", "iv", "V"], &["i", "iv", "V7", "i"], &["i", "VI", "V", "V7"]],
        _ => &[
            &["i7", "VImaj7", "iv7", "v7"],
            &["i7", "iv7", "VImaj7", "V7"],
            &["i", "VI", "III", "VII"],
            &["i9", "VImaj7", "IIImaj7", "VII7"],
            &["i7", "VII7", "VImaj7", "v7"],
        ],
    };
    let a = rng.pick(lib);
    let mut b: Vec<String> = a.iter().map(|s| s.to_string()).collect();
    let n = b.len().max(1);
    b.rotate_left((n / 2).max(1) % n);
    vec![("A".into(), a.iter().map(|s| s.to_string()).collect()), ("B".into(), b)]
}

/// one drum lane's variants over `bars` bars of 16 steps
fn drum_lane(lane: &str, feel: Feel, rng: &mut Rng) -> DrumView {
    let bars: u32 = match lane {
        "ohat" => 1,
        "shaker" | "ride" => 2,
        _ => 4,
    };
    let n = (bars * 16) as usize;
    let mut main = vec![0.0f32; n];
    let mut sparse: Option<Vec<f32>> = None;
    let mut alt: Option<Vec<f32>> = None;
    for bar in 0..bars as usize {
        let at = |s: usize| bar * 16 + s;
        let last = bar + 1 == bars as usize;
        match (lane, feel) {
            ("kick", Feel::FourFloor) => [0, 4, 8, 12].iter().for_each(|&s| main[at(s)] = rng.range(0.93, 1.0)),
            ("kick", Feel::Straight) => [0, 4, 8, 12].iter().for_each(|&s| main[at(s)] = rng.range(0.9, 1.0)),
            ("kick", Feel::HalfTime) => {
                main[at(0)] = 1.0;
                main[at(10)] = rng.range(0.85, 0.95);
                if bar % 2 == 1 {
                    main[at(7)] = rng.range(0.7, 0.85);
                }
                if last {
                    main[at(14)] = rng.range(0.7, 0.8);
                }
            }
            ("kick", Feel::Breakbeat) => {
                main[at(0)] = 1.0;
                main[at(10)] = rng.range(0.88, 0.96);
                if bar % 2 == 1 {
                    main[at(11)] = rng.range(0.6, 0.7);
                }
            }
            ("kick", Feel::BoomBap) => {
                main[at(0)] = 1.0;
                main[at(10)] = rng.range(0.85, 0.95);
                if bar % 2 == 0 {
                    main[at(7)] = rng.range(0.55, 0.7);
                }
            }
            ("clap" | "snare", Feel::HalfTime) => {
                main[at(8)] = rng.range(0.95, 1.0);
                if last && lane == "snare" {
                    main[at(15)] = 0.35;
                }
            }
            ("clap" | "snare", _) => {
                main[at(4)] = rng.range(0.92, 1.0);
                main[at(12)] = rng.range(0.92, 1.0);
                if lane == "snare" && matches!(feel, Feel::Breakbeat | Feel::BoomBap) {
                    for s in [7, 9, 15] {
                        if rng.chance(0.4) {
                            main[at(s)] = rng.range(0.25, 0.4);
                        }
                    }
                }
                if last && rng.chance(0.5) {
                    main[at(15)] = 0.4;
                }
            }
            ("hat", Feel::FourFloor) => {
                for s in (0..16).step_by(2) {
                    main[at(s)] = if s % 4 == 2 { rng.range(0.72, 0.82) } else { rng.range(0.3, 0.42) };
                }
                for s in [3, 7, 11, 15] {
                    if rng.chance(0.3) {
                        main[at(s)] = rng.range(0.25, 0.35);
                    }
                }
            }
            ("hat", Feel::HalfTime) => {
                for s in (0..16).step_by(2) {
                    main[at(s)] = rng.range(0.6, 0.75);
                }
                let roll = if bar % 2 == 1 { 12 } else { 6 };
                for s in roll..roll + 3 {
                    main[at(s)] = rng.range(0.45, 0.6);
                }
            }
            ("hat", _) => {
                for s in (0..16).step_by(2) {
                    main[at(s)] = if s % 4 == 0 { rng.range(0.7, 0.8) } else { rng.range(0.45, 0.55) };
                }
            }
            ("ohat", _) => [2, 6, 10, 14].iter().for_each(|&s| main[at(s)] = rng.range(0.74, 0.82)),
            ("shaker", _) => {
                for s in 0..16 {
                    if s % 4 != 0 || rng.chance(0.3) {
                        main[at(s)] = if s % 2 == 1 { rng.range(0.42, 0.5) } else { rng.range(0.3, 0.38) };
                    }
                }
            }
            ("ride", _) => (0..16)
                .step_by(2)
                .for_each(|s| main[at(s)] = if s % 4 == 0 { rng.range(0.6, 0.7) } else { rng.range(0.4, 0.5) }),
            _ => {
                let pool = [3, 6, 7, 10, 11, 13, 14];
                for s in pool {
                    if rng.chance(0.4) {
                        main[at(s)] = rng.range(0.6, 0.75);
                    }
                }
            }
        }
    }
    match lane {
        "kick" | "hat" | "perc" => {
            sparse = Some(
                main.iter()
                    .enumerate()
                    .map(|(i, &x)| {
                        if lane == "kick" {
                            x * 0.94
                        } else if i % 4 == 2 && x > 0.0 {
                            0.72
                        } else if lane == "perc" && i % 8 == 6 {
                            0.6
                        } else {
                            0.0
                        }
                    })
                    .collect(),
            )
        }
        "clap" | "snare" => {
            let mut a = main.clone();
            for bar in 0..bars as usize {
                a[bar * 16 + 14] = 0.45;
                a[bar * 16 + 15] = 0.6;
            }
            alt = Some(a);
        }
        _ => {}
    }
    let mut variants = vec![("main".to_string(), main)];
    if let Some(s) = sparse {
        variants.push(("sparse".into(), s));
    }
    if let Some(a) = alt {
        variants.push(("alt".into(), a));
    }
    DrumView { lane: lane.into(), pattern_bars: bars, variants }
}

fn energy(kind: &str, rng: &mut Rng) -> f32 {
    let base = match kind {
        "intro" | "outro" => 0.4,
        "break" => 0.35,
        "verse" => 0.55,
        "build" => 0.6,
        "groove" => 0.65,
        "hook" => 0.85,
        "drop" => 0.95,
        _ => 0.6,
    };
    ((base + (rng.f32() - 0.5) * 0.06) * 100.0).round() / 100.0
}

fn section_layers(kind: &str, song: &SongView, genre: &Genre) -> Vec<String> {
    let drums: Vec<&str> = song.drums.iter().map(|d| d.lane.as_str()).collect();
    let parts: Vec<&str> = song.parts.iter().map(|p| p.name.as_str()).collect();
    let loops: Vec<&str> = song.tracks.iter().filter(|t| t.kind == "loop").map(|t| t.name.as_str()).collect();
    let is = |l: &str, set: &[&str]| set.contains(&l);
    let four = genre.feel == Feel::FourFloor;
    let pick = |l: &&str| -> bool {
        let l = *l;
        match kind {
            "intro" => {
                (four && l == "kick")
                    || is(l, &["hat", "shaker", "perc", "ride"])
                    || (genre.feel == Feel::BoomBap && is(l, &["keys", "pad"]))
                    || is(l, &loops)
            }
            "verse" | "groove" => !is(l, &["ohat", "lead", "arp", "pad"]) || (kind == "groove" && l == "ohat"),
            "build" => is(l, &["clap", "snare", "hat", "chords", "keys", "lead", "pad"]),
            "break" => is(l, &["pad", "chords", "keys", "lead", "hat"]),
            "outro" => is(l, &["kick", "hat", "shaker", "perc", "bass"]),
            _ => true,
        }
    };
    drums.iter().chain(parts.iter()).chain(loops.iter()).copied().filter(|l| pick(l)).map(|s| s.to_string()).collect()
}

/// a whole new song: title, tempo, key, form, drums, parts, tracks
pub fn generate(genre_name: &str, seed: u64, bpm: Option<f64>, key: Option<&str>, loops: bool) -> SongView {
    let genre = genre(genre_name);
    let mut rng = Rng::new(seed ^ 0x9e37_79b9_7f4a_7c15);
    let bpm = bpm.unwrap_or_else(|| (genre.bpm.0 + (genre.bpm.1 - genre.bpm.0) * rng.f64()).round());
    let swing = ((genre.swing.0 + (genre.swing.1 - genre.swing.0) * rng.f32()) * 100.0).round() / 100.0;
    let mode = *rng.pick(genre.modes);
    let key = key.and_then(Key::parse).unwrap_or(Key {
        tonic: rng.below(12) as i32,
        mode: music::MODES.iter().position(|(m, _)| *m == mode).unwrap_or(1),
    });
    let title = format!("{} {}", rng.pick(ADJECTIVES), rng.pick(NOUNS));

    let drums: Vec<DrumView> = genre.drums.iter().map(|l| drum_lane(l, genre.feel, &mut rng)).collect();
    let parts: Vec<PartView> = genre
        .parts
        .iter()
        .map(|(name, role, styles, presets, register)| PartView {
            name: name.to_string(),
            role: role.to_string(),
            style: rng.pick(styles).to_string(),
            styles: styles.iter().map(|s| s.to_string()).collect(),
            preset: presets[0].to_string(),
            presets: presets.iter().map(|s| s.to_string()).collect(),
            register: *register,
            density: ((0.3 + rng.f32() * 0.35) * 100.0).round() / 100.0,
        })
        .collect();
    let mut tracks: Vec<TrackView> = drums
        .iter()
        .map(|d| {
            let lane = d.lane.as_str();
            let sound = match lane {
                "clap" if loops => "clap cut from 'Deep Blue Void Claps' (Apple Loops)".to_string(),
                "perc" if loops => "perc cut from 'Sparkle and Shine Topper 01' (Apple Loops)".to_string(),
                "hat" => "synth closed_hat".into(),
                "ohat" => "synth open_hat".into(),
                other => format!("synth {other}"),
            };
            let (pan, reverb, delay) = match lane {
                "hat" => (0.15, 0.0, 0.0),
                "ohat" => (-0.1, 0.0, 0.0),
                "shaker" => (-0.3, 0.0, 0.0),
                "perc" => (0.35, 0.0, 0.15),
                "ride" => (0.2, 0.05, 0.0),
                "clap" => (0.0, 0.18, 0.0),
                "snare" => (0.0, if genre.feel == Feel::Straight { 0.35 } else { 0.15 }, 0.0),
                _ => (0.0, 0.0, 0.0),
            };
            TrackView {
                name: lane.into(),
                kind: "drum".into(),
                role: lane.into(),
                sound,
                level_db: 0.0,
                pan,
                reverb,
                delay,
                duck: 0.0,
                muted: false,
                solo: false,
                loudness: None,
            }
        })
        .collect();
    for p in &parts {
        let (reverb, delay, duck) = match p.role.as_str() {
            "bass" => (0.0, 0.0, 0.55),
            "chords" | "keys" => (0.2, 0.08, 0.45),
            "pad" => (0.35, 0.0, 0.5),
            "lead" => (0.25, 0.2, 0.2),
            _ => (0.2, 0.25, 0.3),
        };
        tracks.push(TrackView {
            name: p.name.clone(),
            kind: "part".into(),
            role: p.role.clone(),
            sound: format!("synth patch '{}'", p.preset),
            level_db: 0.0,
            pan: if p.role == "arp" { -0.2 } else { 0.0 },
            reverb,
            delay,
            duck,
            muted: false,
            solo: false,
            loudness: None,
        });
    }
    if loops || genre.name == "house" {
        tracks.push(TrackView {
            name: "top".into(),
            kind: "loop".into(),
            role: "top".into(),
            sound: format!("loop 'Sparkle and Shine Topper 01' ({bpm:.0} BPM, Apple Loops)"),
            level_db: -2.0,
            pan: 0.1,
            reverb: 0.1,
            delay: 0.0,
            duck: 0.3,
            muted: false,
            solo: false,
            loudness: None,
        });
    }
    tracks.push(TrackView {
        name: "fx".into(),
        kind: "fx".into(),
        role: "fx".into(),
        sound: "synth riser, impact, reverse crash".into(),
        level_db: -1.0,
        pan: 0.0,
        reverb: 0.3,
        delay: 0.0,
        duck: 0.0,
        muted: false,
        solo: false,
        loudness: None,
    });

    let mut song = SongView {
        title,
        genre: genre.name.into(),
        seed,
        bpm,
        key: key.name(),
        swing,
        progressions: progressions(key.mode_name(), &mut rng),
        drums,
        parts,
        tracks,
        lineage: vec![format!(
            "generated: {} seed {seed}{}",
            genre.name,
            if loops { " (around Apple Loops)" } else { "" }
        )],
        ..Default::default()
    };
    song.layers = song
        .drums
        .iter()
        .map(|d| d.lane.clone())
        .chain(song.parts.iter().map(|p| p.name.clone()))
        .chain(song.tracks.iter().filter(|t| t.kind == "loop").map(|t| t.name.clone()))
        .collect();
    for (name, kind, bars) in genre.form {
        let has_sparse = |s: &SongView| s.drums.iter().any(|d| d.variants.iter().any(|(v, _)| v == "sparse"));
        let variant = if matches!(*kind, "intro" | "outro") && has_sparse(&song) {
            "sparse"
        } else if *name == "drop2" || *name == "hook3" {
            "alt"
        } else {
            "main"
        };
        let section = SectionView {
            name: name.to_string(),
            kind: kind.to_string(),
            start_bar: 0,
            bars: *bars,
            energy: energy(kind, &mut rng),
            layers: section_layers(kind, &song, genre),
            prog: if matches!(*kind, "break" | "build") { "B".into() } else { "A".into() },
            variant: variant.into(),
            filter_in: *kind == "intro",
            build: *kind == "build" || (*kind == "break" && rng.chance(0.4)),
            gap: *kind == "build",
            impact: matches!(*kind, "drop" | "hook"),
            fills: matches!(*kind, "drop" | "groove" | "hook" | "verse"),
        };
        song.sections.push(section);
    }
    crate::edits::relayout(&mut song);
    song.script = script(&song);
    song
}

fn mini(steps: &[f32], sound: &str) -> String {
    // the first bar in mini-notation: "bd ~ ~ ~ bd ~ ~ ~ ..."
    let bar: Vec<String> = steps
        .iter()
        .take(16)
        .map(|&v| {
            if v <= 0.0 {
                "~".to_string()
            } else if v < 0.45 {
                format!("{sound}:ghost")
            } else {
                sound.to_string()
            }
        })
        .collect();
    let mut groups = Vec::new();
    for g in bar.chunks(4) {
        if g.iter().all(|s| s == "~") {
            groups.push("~".to_string());
        } else {
            groups.push(format!("[{}]", g.join(" ")));
        }
    }
    groups.join(" ")
}

/// the song as a (Strudel-flavoured) claudius script
pub fn script(song: &SongView) -> String {
    let mut out = String::new();
    out += &format!("// {}: {} in {} at {:.0} BPM (seed {})\n", song.title, song.genre, song.key, song.bpm, song.seed);
    out += &format!(
        "song(\"{}\").genre(\"{}\").bpm({}).key(\"{}\").swing({:.2})\n\n",
        song.title, song.genre, song.bpm, song.key, song.swing
    );
    for (name, chords) in &song.progressions {
        out += &format!("chords(\"{name}\", \"<{}>\")\n", chords.join(" "));
    }
    out += "\n// drums: one bar of each lane's main pattern\n";
    let short = |lane: &str| match lane {
        "kick" => "bd",
        "snare" => "sd",
        "clap" => "cp",
        "hat" => "hh",
        "ohat" => "oh",
        "ride" => "rd",
        "shaker" => "sh",
        _ => "perc",
    };
    for d in &song.drums {
        let steps = d.variants.first().map(|(_, s)| s.as_slice()).unwrap_or(&[]);
        let t = song.tracks.iter().find(|t| t.name == d.lane);
        let mut line = format!("track(\"{}\", s(\"{}\")", d.lane, mini(steps, short(&d.lane)));
        if let Some(t) = t {
            line += &format!(".gain({:.2})", (10f32.powf(t.level_db / 20.0) * 0.8).min(1.0));
            if t.pan.abs() > 0.01 {
                line += &format!(".pan({:.2})", t.pan);
            }
            if t.reverb > 0.0 {
                line += &format!(".room({:.2})", t.reverb);
            }
        }
        out += &line;
        out += ")\n";
    }
    out += "\n// parts\n";
    for p in &song.parts {
        out += &format!(
            "part(\"{}\", synth(\"{}\").style(\"{}\").range(\"{}\", \"{}\").density({:.2}))\n",
            p.name,
            p.preset,
            p.style,
            music::note_name(p.register.0),
            music::note_name(p.register.1),
            p.density
        );
    }
    out += "\n// form\nform([\n";
    for s in &song.sections {
        let mut flags = String::new();
        for (on, name) in
            [(s.filter_in, "filterIn"), (s.build, "build"), (s.gap, "gap"), (s.impact, "impact"), (s.fills, "fills")]
        {
            if on {
                flags += &format!(".{name}()");
            }
        }
        out += &format!(
            "  section(\"{}\", \"{}\", {}).energy({:.2}).drums(\"{}\").layers(\"{}\"){flags},\n",
            s.name,
            s.kind,
            s.bars,
            s.energy,
            s.variant,
            s.layers.join(" ")
        );
    }
    out += "])\n";
    out
}

/// the critic's reading of a song (deterministic in the song, a little noise from `rng`)
pub fn judge(song: &SongView, rng: &mut Rng) -> VerdictView {
    let key = Key::parse(&song.key).unwrap_or(Key { tonic: 9, mode: 1 });
    let genre = genre(&song.genre);
    // arc: do louder sections carry more layers, is the drop above the break
    let n = song.sections.len().max(1) as f64;
    let (me, ml) =
        song.sections.iter().fold((0.0, 0.0), |(e, l), s| (e + s.energy as f64 / n, l + s.layers.len() as f64 / n));
    let (mut cov, mut ve, mut vl) = (0.0, 0.0, 0.0);
    for s in &song.sections {
        let (de, dl) = (s.energy as f64 - me, s.layers.len() as f64 - ml);
        cov += de * dl;
        ve += de * de;
        vl += dl * dl;
    }
    let corr = if ve > 0.0 && vl > 0.0 { cov / (ve.sqrt() * vl.sqrt()) } else { 0.0 };
    let max_layers = |kinds: &[&str]| {
        song.sections.iter().filter(|s| kinds.contains(&s.kind.as_str())).map(|s| s.layers.len()).max().unwrap_or(0)
            as f64
    };
    let drop_minus_break = (max_layers(&["drop", "hook"]) - max_layers(&["break"])) * 0.9 + rng.f64() * 0.6;
    let arc = (0.45 + 0.45 * corr.max(0.0) + if drop_minus_break >= 4.0 { 0.1 } else { 0.0 }).clamp(0.0, 1.0);
    // mix: levels far from their targets and muted essentials cost
    let spread: f64 =
        song.tracks.iter().map(|t| (t.level_db as f64).abs()).sum::<f64>() / song.tracks.len().max(1) as f64;
    let kick_muted = song.tracks.iter().any(|t| t.role == "kick" && t.muted);
    let hat_level = song.tracks.iter().find(|t| t.name == "hat").map_or(0.0, |t| t.level_db as f64);
    let mix = (0.99 - spread * 0.035 - if kick_muted { 0.25 } else { 0.0 } - rng.f64() * 0.03).clamp(0.0, 1.0);
    // theory: every numeral should name a chord
    let bad: Vec<&String> =
        song.progressions.iter().flat_map(|(_, c)| c).filter(|c| music::chord(c, &key).is_none()).collect();
    let theory = (1.0 - 0.15 * bad.len() as f64).clamp(0.0, 1.0);
    let in_range = (genre.bpm.0 - 2.0..=genre.bpm.1 + 2.0).contains(&song.bpm);
    let genre_score = (0.62 + if in_range { 0.14 } else { -0.12 } + rng.f64() * 0.12).clamp(0.0, 1.0);
    let production = (0.55 + rng.f64() * 0.18 - spread * 0.01).clamp(0.0, 1.0);
    let scores = vec![
        ("mix".to_string(), mix),
        ("arc".into(), arc),
        ("theory".into(), theory),
        ("genre".into(), genre_score),
        ("production".into(), production),
    ];
    let weights = [0.25, 0.25, 0.15, 0.2, 0.15];
    let total = scores.iter().zip(weights).map(|((_, s), w)| s * w).sum::<f64>() * 100.0;

    let presence = -24.2 + rng.f64() * 2.0 + hat_level * 0.4;
    let mut issues = Vec::new();
    if presence < -23.0 {
        issues.push((
            0.12 + (-23.0 - presence) * 0.1,
            format!("dull 4-8 kHz: hats, claps and snare lack bite (presence {presence:.1} dB, healthy from -23)"),
            "band:presence-".to_string(),
        ));
    }
    if drop_minus_break < 4.0 {
        issues.push((0.35, format!("the drop is only {drop_minus_break:.1} LU above the break (aim for 4 or more): thin the break or add layers to the drop"), "form:drop+".into()));
    }
    if kick_muted {
        issues.push((0.6, "the kick is muted: the groove has no floor".into(), "mix:kick".into()));
    }
    if hat_level > 3.0 {
        issues.push((0.25, format!("hats sit {hat_level:.1} dB over their target: harsh top end"), "mix:hat-".into()));
    }
    for c in &bad {
        issues.push((0.3, format!("'{c}' is not a chord claudius can read in {}", song.key), "harmony".into()));
    }
    if !in_range {
        issues.push((
            0.3,
            format!("{:.0} BPM is unusual for {} ({:.0}-{:.0})", song.bpm, genre.name, genre.bpm.0, genre.bpm.1),
            "tempo".into(),
        ));
    }
    if spread > 3.0 {
        issues.push((
            0.2,
            format!("levels drift {spread:.1} dB from their role targets on average"),
            "mix:levels".into(),
        ));
    }
    issues.sort_by(|a, b| b.0.total_cmp(&a.0));

    let alt = *rng.pick(genre.alts);
    let flaws = [
        "muddy low end",
        "harsh high frequencies",
        "thin sound",
        "clipping distortion",
        "amateur home recording",
        "lo-fi low quality audio",
    ];
    let ears = EarsView {
        genre_query: genre.query.into(),
        genre_z: 1.2 + genre_score * 1.2 + rng.f64() * 0.3,
        best_alt: alt.into(),
        best_alt_z: 0.6 + rng.f64() * 0.8,
        production_z: production * 1.4 - 0.3,
        worst_flaw: rng.pick(&flaws).to_string(),
        worst_flaw_z: -0.4 + rng.f64() * 0.7,
        sounds_like: vec![genre.query.into(), "electronic dance music".into(), alt.into()],
        p_real: Some(0.2 + rng.f64() * 0.3),
    };
    let lufs = -10.0 + rng.f64() * 0.4;
    let measures = vec![
        ("lufs".to_string(), lufs),
        ("true_peak_db".into(), -1.05 - rng.f64() * 0.2),
        ("lra_lu".into(), 3.2 + rng.f64() * 1.5),
        ("band_sub_db".into(), -5.8 + rng.f64()),
        ("band_low_db".into(), -3.4 + rng.f64()),
        ("band_lowmid_db".into(), -6.8 + rng.f64()),
        ("band_mid_db".into(), -12.3 + rng.f64()),
        ("band_presence_db".into(), presence),
        ("band_air_db".into(), -18.6 + rng.f64()),
        ("stereo_correlation".into(), 0.93 + rng.f64() * 0.05),
        ("low_end_correlation".into(), 0.995),
        ("drop_minus_break_lu".into(), drop_minus_break),
        ("arc_correlation".into(), corr),
        ("lead_in_key".into(), 1.0),
        ("bass_roots_on_changes".into(), theory),
        ("length_s".into(), song.seconds),
    ];
    let fix = issues.first().map(|(_, w, _)| format!(" Fix first: {w}")).unwrap_or_default();
    let summary = format!("{total:.0}/100 (arc {arc:.2}, mix {mix:.2}, theory {theory:.2}).{fix}");
    VerdictView { total, summary, scores, issues, ears: Some(ears), measures }
}
