//! The example songs: one in the style of the hits of each scene of the 1990s, 2000s, 2010s and 2020s, as scripts
//! (crates/claudius-api/songs/*.rhai) whose comments say how the style is made. The studio's New song dialog, the
//! assistants' `examples` and `open_example` tools and `claudius examples` open them. They are here, beside the wire
//! types, because the web studio holds them without the engine, which runs in a worker of its own.
//!
//! None of them copies a record. A style is named for the artists it is heard on, but every melody, bass line,
//! chord voicing and drum pattern is the song's own, and it plays only claudius' synths and drum voices, so that it
//! sounds the same everywhere: without the sample packs, offline, and in the online demo.

/// an example song: what it is like, and the script that makes it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Example {
    /// as it is asked for: `midnight_overdrive`, also its script's file name
    pub name: &'static str,
    pub title: &'static str,
    /// the decade whose hits it is made like: "1990s", "2000s", "2010s" or "2020s"
    pub era: &'static str,
    /// the style, in words: "synth-pop, the 80s revival"
    pub style: &'static str,
    /// the artists the style is heard on, for orientation: the song is none of theirs
    pub like: &'static str,
    /// the genre its script starts from, `blank(genre)`
    pub genre: &'static str,
    pub script: &'static str,
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
];

/// the decades the examples are of, in order
pub const ERAS: [&str; 4] = ["1990s", "2000s", "2010s", "2020s"];

/// The example asked for by its name or its title, whatever its case, spaces, hyphens or underscores:
/// `midnight_overdrive`, "Midnight Overdrive", "midnight-overdrive".
pub fn get(asked: &str) -> Option<&'static Example> {
    let plain = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let asked = plain(asked);
    ALL.iter().find(|e| plain(e.name) == asked || plain(e.title) == asked)
}

/// What a script's leading comment says of how its song is made: the lines after its first, without the `//`.
pub fn notes(e: &Example) -> String {
    let lines: Vec<&str> =
        e.script.lines().take_while(|l| l.starts_with("//")).map(|l| l.trim_start_matches("//").trim()).collect();
    lines.join("\n").trim().to_string()
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
}
