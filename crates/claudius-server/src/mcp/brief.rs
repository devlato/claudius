//! What a person asks for in a sentence, read for a new song: the genre (two named are a fusion, the first leading),
//! a tempo, a key, how full or sparse it is and whether it is built on samples. "a chill lo-fi beat around 80 bpm in D
//! minor, minimal" is lofi at 80 BPM in D minor, sparse.

use claudius_api::GenreInfo;

/// a new song as a brief asks for it, and what was read from where, to tell the person
#[derive(Debug, Default, PartialEq)]
pub struct Brief {
    pub genre: String,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub complexity: Option<f64>,
    pub sampled: Option<bool>,
    /// what was taken from the brief, a phrase each ("lofi, which it names", "80 BPM")
    pub read: Vec<String>,
}

const MODES: [&str; 9] =
    ["major", "minor", "dorian", "phrygian", "lydian", "mixolydian", "aeolian", "ionian", "locrian"];
const SPARSE: [&str; 7] = ["minimal", "sparse", "stripped", "bare", "simple", "minimalist", "empty"];
const FULL: [&str; 8] = ["lush", "layered", "full", "rich", "dense", "big", "epic", "huge"];
/// words of a brief that say nothing of its sound, left out when its genre is searched by what it describes
const SAYS_NOTHING: [&str; 30] = [
    "a",
    "an",
    "the",
    "and",
    "or",
    "for",
    "with",
    "of",
    "to",
    "in",
    "on",
    "at",
    "it",
    "its",
    "some",
    "something",
    "like",
    "that",
    "this",
    "me",
    "my",
    "i",
    "make",
    "song",
    "beat",
    "track",
    "music",
    "around",
    "about",
    "bpm",
];

/// the words of a brief as written, with the punctuation around them off and `#` kept ("F#m")
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '#').to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

fn capital(w: &str) -> bool {
    w.starts_with(|c: char| c.is_ascii_uppercase())
}

/// whether the words between two genres a brief names join them into one song ("trip hop with idm", "house and
/// techno", "dub meets techno"), rather than one only saying what the other is like ("chill lo-fi")
fn joined(between: &[String]) -> bool {
    between.iter().any(|w| matches!(w.as_str(), "with" | "and" | "meets" | "plus" | "x" | "fused" | "into" | "over"))
}

/// a tonic as written ("d", "F#", "eb", "Bb"), as claudius names it ("D", "F#", "Eb")
fn tonic(w: &str) -> Option<String> {
    let mut c = w.chars();
    let letter = c.next().map(|l| l.to_ascii_lowercase()).filter(|l| ('a'..='g').contains(l))?.to_ascii_uppercase();
    match c.as_str() {
        "" => Some(letter.to_string()),
        "#" | "b" => Some(format!("{letter}{}", c.as_str())),
        _ => None,
    }
}

/// a tempo: a number from 40 to 240 that "bpm" follows or ends, or "at" comes before
fn tempo(w: &[String]) -> Option<f64> {
    let lower = |i: usize| w.get(i).map(|x| x.to_lowercase());
    for (i, word) in w.iter().enumerate() {
        let n = |s: &str| s.parse::<f64>().ok().filter(|v| (40.0..=240.0).contains(v));
        if let Some(v) = word.to_lowercase().strip_suffix("bpm").and_then(n) {
            return Some(v);
        }
        let before = i.checked_sub(1).and_then(lower);
        if let Some(v) = n(word)
            && (matches!(lower(i + 1).as_deref(), Some("bpm" | "beats"))
                || matches!(before.as_deref(), Some("at" | "around" | "about")))
        {
            return Some(v);
        }
    }
    None
}

/// A key: a tonic a mode follows ("D minor", "eb major", "in a dorian"), a capital one with an m on it ("F#m", "Am"),
/// or a capital one that "in" comes before ("in C", major). "a" alone is an article: "a major hit" is no key, and "I am"
/// none either.
fn key(w: &[String]) -> Option<String> {
    for (i, word) in w.iter().enumerate() {
        let after_in = i > 0 && w[i - 1].eq_ignore_ascii_case("in");
        let mode = w.get(i + 1).map(|m| m.to_lowercase()).filter(|m| MODES.contains(&m.as_str()));
        if let (Some(t), Some(mode)) = (tonic(word), mode)
            && (word != "a" || after_in)
        {
            return Some(format!("{t} {mode}"));
        }
        if capital(word)
            && let Some(t) = word.strip_suffix('m').filter(|t| !t.is_empty()).and_then(tonic)
        {
            return Some(format!("{t} minor"));
        }
        if after_in
            && capital(word)
            && let Some(t) = tonic(word)
        {
            return Some(format!("{t} major"));
        }
    }
    None
}

/// Read a brief for a new song; an error when it names no genre and none is near it, to ask the person which.
pub fn read(text: &str, genres: &[GenreInfo]) -> Result<Brief, String> {
    let w = words(text);
    let mut b = Brief::default();
    let named = claudius_api::genres_named(genres, text);
    match named.as_slice() {
        [] => {
            // the nearest by what it sounds like, from the brief's words that say something of it
            let plain: Vec<String> = w
                .iter()
                .map(|x| x.to_lowercase())
                .filter(|x| x.parse::<f64>().is_err() && !SAYS_NOTHING.contains(&x.as_str()) && tonic(x).is_none())
                .filter(|x| !MODES.contains(&x.as_str()))
                .collect();
            // each word searched on its own, as the search wants every word it is given: the genre they find most,
            // a word of its name or another name of it counting for more than one that only begins one ("hard" of
            // "hardcore"), and that for more than any word its description has
            let mut found: Vec<(&GenreInfo, usize)> = Vec::new();
            for word in &plain {
                let of = claudius_api::search_genres(genres, word);
                if of.len() == genres.len() {
                    continue;
                }
                for g in of {
                    let names: Vec<String> = std::iter::once(&g.name)
                        .chain(&g.aliases)
                        .flat_map(|n| claudius_api::search_words(n))
                        .collect();
                    let weight = if names.iter().any(|n| n == word) {
                        1000
                    } else if names.iter().any(|n| n.starts_with(word.as_str())) {
                        30
                    } else {
                        1
                    };
                    match found.iter_mut().find(|(f, _)| f.name == g.name) {
                        Some((_, n)) => *n += weight,
                        None => found.push((g, weight)),
                    }
                }
            }
            let most = found.iter().map(|(_, n)| *n).max().unwrap_or(0);
            let Some((g, _)) = found.iter().find(|(_, n)| *n == most && most > 0) else {
                return Err(format!(
                    "the brief names no genre, and none is near it: ask the person which, or find_genre with their \
                     words (\"{text}\")"
                ));
            };
            b.genre = g.name.clone();
            b.read.push(format!("{}, the genre nearest what it describes (it names none)", g.name));
        }
        [(one, _)] => {
            b.genre = one.name.clone();
            b.read.push(format!("{}, which it names", one.name));
        }
        [(lead, at), (drums, then), ..] if joined(&claudius_api::search_words(text)[at.end..then.start]) => {
            b.genre = format!("{}+{}", lead.name, drums.name);
            b.read.push(format!(
                "{} fused with {}, which it names: the first leads, the second plays the drums",
                lead.name, drums.name
            ));
        }
        several => {
            // a word that names a genre and only says what a song of another is like ("chill lo-fi", "afro house"):
            // the genre named at the greatest length, the last of those, as the word a phrase ends on says what it is
            let longest = several.iter().map(|(_, r)| r.len()).max().unwrap_or(1);
            let (g, _) = several.iter().rev().find(|(_, r)| r.len() == longest).unwrap_or(&several[0]);
            b.genre = g.name.clone();
            let others: Vec<&str> =
                several.iter().filter(|(o, _)| o.name != g.name).map(|(o, _)| o.name.as_str()).collect();
            b.read.push(format!(
                "{}, which it names ({} it names as what the song is like; \"{} with {}\" would fuse them)",
                g.name,
                others.join(" and "),
                g.name,
                others.first().copied().unwrap_or_default()
            ));
        }
    }
    if let Some(bpm) = tempo(&w) {
        b.bpm = Some(bpm);
        b.read.push(format!("{bpm:.0} BPM"));
    }
    if let Some(k) = key(&w) {
        b.read.push(format!("in {k}"));
        b.key = Some(k);
    }
    // the brief's words outside the genres it names: "big" in "big beat" asks for nothing
    let lower: Vec<String> = claudius_api::search_words(text)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !named.iter().any(|(_, at)| at.contains(i)))
        .map(|(_, x)| x)
        .collect();
    if let Some(s) = lower.iter().find(|x| SPARSE.contains(&x.as_str())) {
        b.complexity = Some(0.25);
        b.read.push(format!("sparse, as \"{s}\" asks"));
    } else if let Some(f) = lower.iter().find(|x| FULL.contains(&x.as_str())) {
        b.complexity = Some(0.85);
        b.read.push(format!("full and layered, as \"{f}\" asks"));
    }
    let samples = |x: &String| matches!(x.as_str(), "sample" | "samples" | "sampled" | "chopped");
    if let Some(i) = lower.iter().position(samples) {
        let no = i.checked_sub(1).is_some_and(|j| matches!(lower[j].as_str(), "no" | "without"));
        b.sampled = Some(!no);
        b.read.push(if no {
            "without samples".to_string()
        } else {
            "built on samples from free libraries".to_string()
        });
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genres() -> Vec<GenreInfo> {
        let g = |name: &str, family: &str, aliases: &[&str], description: &str| GenreInfo {
            name: name.into(),
            family: family.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            description: description.into(),
            ..Default::default()
        };
        vec![
            g("lofi", "hip hop", &["lo-fi"], "Lofi: dusty drums, warm keys, vinyl crackle"),
            g("trip_hop", "downtempo", &["bristol"], "Trip hop: slow breaks and dark strings"),
            g("idm", "electronica", &[], "IDM: glitchy drums"),
            g("techno", "techno", &[], "Techno: a driving kick, hypnotic and dark"),
            g("house", "house", &[], "House: four on the floor"),
        ]
    }

    #[test]
    fn the_real_genres_answer_briefs_as_people_write_them() {
        let real = claudius_session::views::genre_infos();
        let b = read("big beat for a car chase", &real).unwrap();
        assert_eq!((b.genre.as_str(), b.complexity), ("big_beat", None), "naming big beat asks for no layers");
        let b = read("a big, lush big beat track", &real).unwrap();
        assert_eq!(b.complexity, Some(0.85), "the words around the genre still ask");
        let genre = |brief: &str| read(brief, &real).map(|b| b.genre).unwrap_or_default();
        assert_eq!(genre("a chill lo-fi beat for studying"), "lofi");
        assert_eq!(genre("90s west coast rap"), "g_funk");
        assert_eq!(genre("trip hop with idm drums"), "trip_hop+idm");
        assert_eq!(genre("drum and bass, liquid, 174 bpm"), "dnb");
        assert!(genre("hard hitting drill beat").ends_with("drill"), "{}", genre("hard hitting drill beat"));
        assert!(read("something for a summer beach party", &real).is_err(), "nothing near: ask the person");
    }

    #[test]
    fn a_brief_gives_its_genre_tempo_key_and_how_full_it_is() {
        let b = read("a chill lo-fi beat around 80 bpm in D minor, minimal", &genres()).unwrap();
        assert_eq!(
            (b.genre.as_str(), b.bpm, b.key.as_deref(), b.complexity),
            ("lofi", Some(80.0), Some("D minor"), Some(0.25))
        );
        assert_eq!(b.read, ["lofi, which it names", "80 BPM", "in D minor", "sparse, as \"minimal\" asks"]);
    }

    #[test]
    fn two_genres_named_are_a_fusion_and_tempos_and_keys_are_read_as_people_write_them() {
        let b = read("trip hop with idm drums at 92, F#m, lush", &genres()).unwrap();
        assert_eq!(b.genre, "trip_hop+idm");
        assert_eq!((b.bpm, b.key.as_deref(), b.complexity), (Some(92.0), Some("F# minor"), Some(0.85)));
        let b = read("dark idm trip hop", &genres()).unwrap();
        assert_eq!(b.genre, "trip_hop", "no word joins them: the one named at length, which ends the phrase");
        assert!(b.read[0].contains("\"trip_hop with idm\" would fuse them"), "{:?}", b.read);
        let b = read("house at 124bpm in C", &genres()).unwrap();
        assert_eq!((b.bpm, b.key.as_deref()), (Some(124.0), Some("C major")));
        let b = read("techno 130 bpm, eb dorian", &genres()).unwrap();
        assert_eq!((b.bpm, b.key.as_deref()), (Some(130.0), Some("Eb dorian")));
    }

    #[test]
    fn a_brief_that_names_no_genre_gets_the_nearest_by_its_sound_or_an_answer_to_ask() {
        let b = read("something hypnotic and dark for a club", &genres()).unwrap();
        assert_eq!(b.genre, "techno");
        assert!(b.read[0].contains("nearest"), "{:?}", b.read);
        assert!(read("zzz qqq", &genres()).unwrap_err().contains("ask the person"));
    }

    #[test]
    fn an_article_or_a_number_alone_is_no_key_or_tempo() {
        let b = read("a house song for a friend, 3 minutes", &genres()).unwrap();
        assert_eq!((b.bpm, b.key), (None, None));
        let b = read("house with samples", &genres()).unwrap();
        assert_eq!(b.sampled, Some(true));
        let b = read("house, no samples", &genres()).unwrap();
        assert_eq!(b.sampled, Some(false));
    }
}
