//! The example songs: one in the style of the hits of each scene of the 1990s, 2000s, 2010s and 2020s, and a few
//! melodies in the public domain remade in modern styles, as scripts (crates/claudius-api/songs/*.rhai) whose comments
//! say how the style is made. The studio's New song dialog, the assistants' `examples` and `open_example` tools and
//! `claudius examples` open them. They are here, beside the wire types, because the web studio holds them without the
//! engine, which runs in a worker of its own.
//!
//! None of them copies a record. A style is named for the artists it is heard on, but every melody, bass line,
//! chord voicing and drum pattern is the song's own, or a public-domain composer's, and it plays only claudius'
//! synths and drum voices, so that it sounds the same everywhere: without the sample packs, offline, and in the
//! online demo.

/// an example song: what it is like, and the script that makes it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Example {
    /// as it is asked for: `midnight_overdrive`, also its script's file name
    pub name: &'static str,
    pub title: &'static str,
    /// the decade whose hits it is made like, "1990s" to "2020s", or [`CLASSICS`]
    pub era: &'static str,
    /// the style, in words: "synth-pop, the 80s revival"
    pub style: &'static str,
    /// the artists the style is heard on, for orientation: the song is none of theirs. Of a classic remade, the piece
    pub like: &'static str,
    /// the genre its script starts from, `blank(genre)`
    pub genre: &'static str,
    pub script: &'static str,
}

impl Example {
    /// "like Massive Attack, Portishead, Tricky", or of a classic remade "on Beethoven's Ode to Joy (1824)"
    pub fn like_words(&self) -> String {
        if self.era == CLASSICS { format!("on {}", self.like) } else { format!("like {}", self.like) }
    }

    /// what it is: "Bristol trip hop of the 1990s, like Massive Attack, Portishead, Tricky", or of a classic remade
    /// "festival EDM on Beethoven's Ode to Joy (1824)"
    pub fn in_words(&self) -> String {
        if self.era == CLASSICS {
            format!("{} {}", self.style, self.like_words())
        } else {
            format!("{} of the {}, {}", self.style, self.era, self.like_words())
        }
    }
}

/// every example, by decade and then by style
pub const ALL: &[Example] = &[
    Example {
        name: "blue_hour_static",
        title: "Blue Hour Static",
        era: "1990s",
        style: "Bristol trip hop",
        like: "Massive Attack, Portishead, Tricky",
        genre: "trip_hop",
        script: include_str!("../songs/blue_hour_static.rhai"),
    },
    Example {
        name: "acid_rocket",
        title: "Acid Rocket",
        era: "1990s",
        style: "big beat",
        like: "The Prodigy, The Chemical Brothers, Fatboy Slim",
        genre: "big_beat",
        script: include_str!("../songs/acid_rocket.rhai"),
    },
    Example {
        name: "concrete_diaries",
        title: "Concrete Diaries",
        era: "1990s",
        style: "boom bap",
        like: "Nas, Pete Rock, A Tribe Called Quest",
        genre: "boom_bap",
        script: include_str!("../songs/concrete_diaries.rhai"),
    },
    Example {
        name: "sunday_lowrider",
        title: "Sunday Lowrider",
        era: "1990s",
        style: "G-funk",
        like: "Dr. Dre, Snoop Dogg, Warren G",
        genre: "g_funk",
        script: include_str!("../songs/sunday_lowrider.rhai"),
    },
    Example {
        name: "diamond_floor",
        title: "Diamond Floor",
        era: "1990s",
        style: "piano house",
        like: "Robin S, Black Box, CeCe Peniston",
        genre: "house",
        script: include_str!("../songs/diamond_floor.rhai"),
    },
    Example {
        name: "chrome_boulevard",
        title: "Chrome Boulevard",
        era: "2000s",
        style: "French filter house",
        like: "Daft Punk, Stardust, Modjo",
        genre: "filter_house",
        script: include_str!("../songs/chrome_boulevard.rhai"),
    },
    Example {
        name: "flashbulb",
        title: "Flashbulb",
        era: "2000s",
        style: "electropop",
        like: "Lady Gaga, Britney Spears, Kesha",
        genre: "dance_pop",
        script: include_str!("../songs/flashbulb.rhai"),
    },
    Example {
        name: "sundown_shuffle",
        title: "Sundown Shuffle",
        era: "2000s",
        style: "UK garage",
        like: "Craig David, Artful Dodger, MJ Cole",
        genre: "uk_garage",
        script: include_str!("../songs/sundown_shuffle.rhai"),
    },
    Example {
        name: "afterlight_horizon",
        title: "Afterlight Horizon",
        era: "2000s",
        style: "uplifting trance",
        like: "Tiësto, ATB, Paul van Dyk",
        genre: "trance",
        script: include_str!("../songs/afterlight_horizon.rhai"),
    },
    Example {
        name: "floor_shaker",
        title: "Floor Shaker",
        era: "2000s",
        style: "crunk",
        like: "Lil Jon, Ying Yang Twins, Three 6 Mafia",
        genre: "crunk",
        script: include_str!("../songs/floor_shaker.rhai"),
    },
    Example {
        name: "satellite_hymn",
        title: "Satellite Hymn",
        era: "2000s",
        style: "art rock",
        like: "Radiohead, Muse, Coldplay",
        genre: "art_rock",
        script: include_str!("../songs/satellite_hymn.rhai"),
    },
    Example {
        name: "higher_than_the_lights",
        title: "Higher Than the Lights",
        era: "2010s",
        style: "festival EDM",
        like: "Avicii, Swedish House Mafia, Martin Garrix",
        genre: "electro_house",
        script: include_str!("../songs/higher_than_the_lights.rhai"),
    },
    Example {
        name: "coral_skies",
        title: "Coral Skies",
        era: "2010s",
        style: "future bass",
        like: "Flume, The Chainsmokers, Illenium",
        genre: "future_bass",
        script: include_str!("../songs/coral_skies.rhai"),
    },
    Example {
        name: "fracture_line",
        title: "Fracture Line",
        era: "2010s",
        style: "dubstep",
        like: "Skrillex, Rusko, Flux Pavilion",
        genre: "dubstep",
        script: include_str!("../songs/fracture_line.rhai"),
    },
    Example {
        name: "island_tide",
        title: "Island Tide",
        era: "2010s",
        style: "tropical house",
        like: "Kygo, Thomas Jack, Felix Jaehn",
        genre: "deep_house",
        script: include_str!("../songs/island_tide.rhai"),
    },
    Example {
        name: "ice_avenue",
        title: "Ice Avenue",
        era: "2010s",
        style: "trap",
        like: "Metro Boomin, Migos, 21 Savage",
        genre: "trap",
        script: include_str!("../songs/ice_avenue.rhai"),
    },
    Example {
        name: "glass_penthouse",
        title: "Glass Penthouse",
        era: "2010s",
        style: "alternative R&B",
        like: "The Weeknd, Frank Ocean, PARTYNEXTDOOR",
        genre: "alt_rnb",
        script: include_str!("../songs/glass_penthouse.rhai"),
    },
    Example {
        name: "midnight_overdrive",
        title: "Midnight Overdrive",
        era: "2020s",
        style: "synth-pop, the 80s revival",
        like: "The Weeknd, Dua Lipa, M83",
        genre: "synthwave",
        script: include_str!("../songs/midnight_overdrive.rhai"),
    },
    Example {
        name: "satin_signal",
        title: "Satin Signal",
        era: "2020s",
        style: "disco-pop",
        like: "Dua Lipa, Doja Cat, The Weeknd",
        genre: "nu_disco",
        script: include_str!("../songs/satin_signal.rhai"),
    },
    Example {
        name: "calm_waters",
        title: "Calm Waters",
        era: "2020s",
        style: "afrobeats",
        like: "Burna Boy, Wizkid, Rema",
        genre: "afrobeats",
        script: include_str!("../songs/calm_waters.rhai"),
    },
    Example {
        name: "luna_callejera",
        title: "Luna Callejera",
        era: "2020s",
        style: "reggaeton",
        like: "Bad Bunny, Karol G, J Balvin",
        genre: "reggaeton",
        script: include_str!("../songs/luna_callejera.rhai"),
    },
    Example {
        name: "midnight_in_jozi",
        title: "Midnight in Jozi",
        era: "2020s",
        style: "amapiano",
        like: "Kabza De Small, Tyla, DJ Maphorisa",
        genre: "amapiano",
        script: include_str!("../songs/midnight_in_jozi.rhai"),
    },
    Example {
        name: "late_checkout",
        title: "Late Checkout",
        era: "2020s",
        style: "tech house",
        like: "Fisher, Chris Lake, John Summit",
        genre: "tech_house",
        script: include_str!("../songs/late_checkout.rhai"),
    },
    Example {
        name: "joy_machine",
        title: "Joy Machine",
        era: CLASSICS,
        style: "festival EDM",
        like: "Beethoven's Ode to Joy (1824)",
        genre: "electro_house",
        script: include_str!("../songs/joy_machine.rhai"),
    },
    Example {
        name: "canon_study",
        title: "Canon Study",
        era: CLASSICS,
        style: "lo-fi hip hop",
        like: "Pachelbel's Canon in D (around 1700)",
        genre: "lofi",
        script: include_str!("../songs/canon_study.rhai"),
    },
    Example {
        name: "mountain_kings",
        title: "Mountain Kings",
        era: CLASSICS,
        style: "drum and bass",
        like: "Grieg's In the Hall of the Mountain King (1875)",
        genre: "dnb",
        script: include_str!("../songs/mountain_kings.rhai"),
    },
];

/// the era of the melodies in the public domain remade in a modern style
pub const CLASSICS: &str = "classics remade";

/// the eras the examples are of, in order: the decades, then the classics remade
pub const ERAS: [&str; 5] = ["1990s", "2000s", "2010s", "2020s", CLASSICS];

/// The example asked for by its name or its title, whatever its case, spaces, hyphens or underscores:
/// `midnight_overdrive`, "Midnight Overdrive", "midnight-overdrive".
pub fn get(asked: &str) -> Option<&'static Example> {
    let plain = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let asked = plain(asked);
    ALL.iter().find(|e| plain(e.name) == asked || plain(e.title) == asked)
}

/// What a script's leading comment says of its song and how it is made, without the `//` and the comment's own line
/// breaks: a paragraph, then "How it is made:" and a line for each point, for a tooltip or an assistant to wrap.
pub fn notes(e: &Example) -> String {
    let mut out = String::new();
    let mut gap = false;
    for line in e.script.lines().take_while(|l| l.starts_with("//")) {
        let text = line.trim_start_matches("//").trim();
        if text.is_empty() {
            gap = !out.is_empty();
            continue;
        }
        if gap {
            out.push_str("\n\n");
        } else if text.starts_with("- ") || out.ends_with(':') {
            out.push('\n');
        } else if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(text);
        gap = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_examples_are_named_once_each_of_a_decade_and_say_how_they_are_made() {
        for (i, e) in ALL.iter().enumerate() {
            assert!(ALL[..i].iter().all(|o| o.name != e.name && o.title != e.title), "{} twice", e.name);
            assert!(e.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{}", e.name);
            assert!(ERAS.contains(&e.era), "{}: {}", e.name, e.era);
            assert!(notes(e).contains("How it is made"), "{} has no notes", e.name);
            assert!(notes(e).contains("original"), "{} does not say it copies no record", e.name);
        }
        assert!(
            ALL.windows(2)
                .all(|w| ERAS.iter().position(|x| *x == w[0].era) <= ERAS.iter().position(|x| *x == w[1].era))
        );
    }

    #[test]
    fn an_example_is_found_by_its_name_or_its_title() {
        assert_eq!(get("midnight_overdrive").map(|e| e.title), Some("Midnight Overdrive"));
        assert_eq!(get("Midnight Overdrive").map(|e| e.name), Some("midnight_overdrive"));
        assert_eq!(get("midnight-overdrive").map(|e| e.name), Some("midnight_overdrive"));
        assert_eq!(get("no such song"), None);
    }

    #[test]
    fn the_notes_are_a_paragraph_then_a_line_for_each_point() {
        for e in ALL {
            let notes = notes(e);
            let (about, how) = notes.split_once("\n\n").unwrap_or((&notes, ""));
            assert!(about.starts_with(&format!("\"{}\"", e.title)) && !about.contains('\n'), "{}: {about}", e.name);
            let mut lines = how.lines();
            assert_eq!(lines.next(), Some("How it is made:"), "{}", e.name);
            assert!(lines.all(|l| l.starts_with("- ") && !l.contains("  ")), "{}: {how}", e.name);
        }
    }

    #[test]
    fn an_example_says_what_it_is_in_words() {
        let decade = get("blue_hour_static").map(Example::in_words);
        assert_eq!(decade.as_deref(), Some("Bristol trip hop of the 1990s, like Massive Attack, Portishead, Tricky"));
        let classic = get("joy_machine").map(Example::in_words);
        assert_eq!(classic.as_deref(), Some("festival EDM on Beethoven's Ode to Joy (1824)"));
    }
}
