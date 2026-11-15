//! Words typed with a slip, read as the words meant: "genrate" as "generate", "revreb" as "reverb", "the whistel" as
//! "the whistle". A word that is no English word, and that none of the studio's actions, none of the example requests
//! the readings learn from and none of the song's tracks has, is read as the one word of theirs nearest it: a slip away
//! (a letter left out, one too many, one for another, or two swapped), or two in a word of eight letters or more, and
//! starting with the same letter, as a slip of the finger rarely changes the first. So the actions and the readings
//! find what the words meant, by name and by meaning alike: a word with a slip in it is another word to the embedding
//! model, which finds other actions for it.
//!
//! An English word is never read as another ("four" as "for", "centre" as "center", "wash" as "was"): the list is
//! SCOWL's (`data/english.txt`, words of four letters or more), with British spellings. The example requests held out
//! of the readings, and those written after them, are left out of the words known, so that the evals measure on them
//! what a request in words the studio has never seen meets.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use claudius_api::SongView;

/// the words of the studio's questions and requests that its actions and example requests may not have
const ALSO: &str = "what whats which how is are does where when tell show list volume level loud louder loudest quiet \
    quieter quietest reverb delay echo pan panned drive distortion overdrive tempo bpm key scale genre style title \
    length long tracks track instruments instrument sections section structure muted mute unmute soloed solo playing \
    plays sound sounds preset patch sing singing vocals voice drums drum bass kick snare hat hats lead pad chords keys \
    piano guitar strings synth arp bell organ choir brass horn flute sax cello violin percussion shaker clap tom toms \
    intro verse chorus hook drop break build outro bridge groove song mix master export render save open record \
    sample samples remix remake flip loop undo redo back before after swap replace remove delete cut fade faster slower";

/// the words of everyday English a request or a question has round what it asks, known so that a slip of one is read
/// as it ("lsat" as "last")
const COMMON: &str = "the this that these those there here they them their what with without from into onto about     over under again just only also very really little bit lot much many more less most least some any all every     each other another like make makes made want need needs please thanks thank good better best bad worse nice great     cool awesome right left now then than when while still even ever never always sometimes maybe really pretty     could would should will shall might must have has had been being were was does done doing going gone come came     give gave take took keep kept let lets put set turn turned bring brought try tried feel feels seems looks sound     sounds hear heard listen think know mean meant same different whole part parts first last next time times     weather today tomorrow yesterday hello help please okay yeah yes nope sure fine song songs music beat beats     track tracks thing things stuff something nothing everything anything someone everyone people person friend     write wrote read said says tell told call called name named start stop end ending begin beginning middle";

/// the words of `text`, lowercase, those of letters only
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && w.chars().all(char::is_alphabetic))
        .map(str::to_lowercase)
}

/// the words the studio knows, each with how often its actions and example requests have it
fn known() -> &'static HashMap<String, usize> {
    static KNOWN: OnceLock<HashMap<String, usize>> = OnceLock::new();
    KNOWN.get_or_init(|| {
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut add = |text: &str| {
            for w in words(text) {
                *m.entry(w).or_insert(0) += 1;
            }
        };
        for e in crate::catalogue::ENTRIES {
            add(e.name);
            add(e.words);
            add(e.place);
        }
        // those the readings learn from: the evals measure the others as requests never seen
        for e in crate::ask::examples().into_iter().filter(|e| e.set.is_none()) {
            add(&e.ask);
        }
        add(ALSO);
        add(COMMON);
        m
    })
}

/// English words of four letters or more, which are never read as another word (`data/english.txt`, from SCOWL)
fn english() -> &'static HashSet<&'static str> {
    static ENGLISH: OnceLock<HashSet<&'static str>> = OnceLock::new();
    ENGLISH.get_or_init(|| words_of_list(include_str!("../data/english.txt")))
}

/// The words of a list as `data/english.txt` holds them: one a line, after its notice and a line of three dashes. By
/// lines, whose carriage returns a checkout on Windows adds go with them.
fn words_of_list(list: &str) -> HashSet<&str> {
    list.lines().skip_while(|l| *l != "---").skip(1).filter(|w| !w.is_empty()).collect()
}

/// the words the song's tracks are called by: their names, roles and sounds
fn of_the_song(song: Option<&SongView>) -> Vec<String> {
    let Some(song) = song else { return Vec::new() };
    song.tracks.iter().flat_map(|t| [t.name.as_str(), t.role.as_str(), t.sound.as_str()]).flat_map(words).collect()
}

/// How many slips apart `a` and `b` are (a letter left out, one too many, one for another, two side by side swapped),
/// or none when it is more than `most`.
fn slips(a: &[char], b: &[char], most: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > most {
        return None;
    }
    // the optimal string alignment distance, a row at a time, with the two rows before it for a swap
    let mut before: Vec<usize> = Vec::new();
    let mut last: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut row = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let other = usize::from(a[i - 1] != b[j - 1]);
            row[j] = (last[j] + 1).min(row[j - 1] + 1).min(last[j - 1] + other);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                row[j] = row[j].min(before[j - 2] + 1);
            }
        }
        if row.iter().min().is_some_and(|m| *m > most) {
            return None;
        }
        (before, last) = (last, row);
    }
    let d = last[b.len()];
    (d <= most).then_some(d)
}

/// The word `w` meant, when it is a word with a slip in it: the one known word nearest it (the more common of two as
/// near), within a slip for a word of up to seven letters and two for a longer one, and starting as it does; none for a
/// word known, an English word or a word near none, and for words of three letters or fewer, which are near too many.
fn meant_word(w: &str, song: &[String]) -> Option<String> {
    let known = known();
    if w.chars().count() < 4 || known.contains_key(w) || song.iter().any(|s| s == w) || english().contains(w) {
        return None;
    }
    let chars: Vec<char> = w.chars().collect();
    let most = if chars.len() >= 8 { 2 } else { 1 };
    let theirs = song.iter().filter(|s| !known.contains_key(s.as_str())).map(|s| (s.as_str(), 1));
    let mut near: Vec<(usize, usize, &str)> = known
        .iter()
        .map(|(k, n)| (k.as_str(), *n))
        .chain(theirs)
        .filter(|(k, _)| k.len() >= 3 && k.chars().next() == chars.first().copied())
        .filter_map(|(k, n)| slips(&chars, &k.chars().collect::<Vec<_>>(), most).map(|d| (d, n, k)))
        .collect();
    // the nearest, and of those as near the most common; two as near and as common leave the word as it is
    near.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(b.2)));
    near.dedup_by(|a, b| a.2 == b.2);
    match near.as_slice() {
        [a, b, ..] if (a.0, a.1) == (b.0, b.1) => None,
        [a, ..] => Some(a.2.to_string()),
        [] => None,
    }
}

/// `query` with each word typed with a slip read as the word meant (`meant_word`), the song's tracks' words known too;
/// none when no word changes.
pub fn meant(query: &str, song: Option<&SongView>) -> Option<String> {
    let song = of_the_song(song);
    let mut out = String::with_capacity(query.len());
    let mut changed = false;
    let mut word = String::new();
    let mut flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        match meant_word(&word.to_lowercase(), &song) {
            Some(m) => {
                out.push_str(&m);
                changed = true;
            }
            None => out.push_str(word),
        }
        word.clear();
    };
    for c in query.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_with_a_slip_is_read_as_the_word_meant_and_a_word_known_or_near_none_as_it_is() {
        assert_eq!(meant("genrate", None).as_deref(), Some("generate"));
        assert_eq!(meant("more revreb on the snare", None).as_deref(), Some("more reverb on the snare"));
        assert_eq!(meant("open the mxier", None).as_deref(), Some("open the mixer"));
        assert_eq!(meant("Genrate a song!", None).as_deref(), Some("generate a song!"));
        // known words, numbers, and words near nothing the studio knows are as they were
        for q in ["generate a song", "make it 128", "write me a poem", "what's the weather like", "the bas", "hello"] {
            assert_eq!(meant(q, None), None, "{q}");
        }
        // and English words, though a slip from one the studio knows: these the first reading took for slips
        for q in ["redo four times", "centre the hats", "wash the pad in reverb", "before the tempo went up"] {
            assert_eq!(meant(q, None), None, "{q}");
        }
    }

    #[test]
    fn the_list_reads_the_same_with_the_carriage_returns_a_checkout_on_windows_adds() {
        let list = include_str!("../data/english.txt");
        let windows = list.replace('\n', "\r\n");
        assert_eq!(words_of_list(&windows), words_of_list(list));
        assert_eq!(words_of_list("a notice\r\n---\r\nfour\r\nwash\r\n"), HashSet::from(["four", "wash"]));
    }

    #[test]
    fn the_english_words_are_those_of_four_letters_or_more_and_no_slip_is_among_them() {
        let english = english();
        assert!(english.len() > 30_000, "{} words", english.len());
        assert!(english.iter().all(|w| w.len() >= 4 && w.chars().all(|c| c.is_ascii_lowercase())));
        for w in ["four", "centre", "colour", "string", "second", "liked", "sped", "went", "roomy", "point", "wash"] {
            assert!(english.contains(w), "{w}");
        }
        for slip in ["genrate", "revreb", "mxier", "whistel", "dwon", "lsat", "stlye"] {
            assert!(!english.contains(slip), "{slip}");
        }
    }

    #[test]
    fn a_slip_is_a_letter_out_one_too_many_one_for_another_or_two_swapped() {
        let s = |a: &str, b: &str| slips(&a.chars().collect::<Vec<_>>(), &b.chars().collect::<Vec<_>>(), 2);
        assert_eq!(s("genrate", "generate"), Some(1));
        assert_eq!(s("revreb", "reverb"), Some(1));
        assert_eq!(s("mixr", "mixer"), Some(1));
        assert_eq!(s("snrae", "snare"), Some(1));
        assert_eq!(s("snare", "snare"), Some(0));
        assert_eq!(s("snare", "sn"), None);
        assert_eq!(s("abc", "acb"), Some(1));
    }

    #[test]
    fn the_palette_finds_for_words_with_a_slip_what_it_finds_for_the_words_meant_and_says_so() {
        let finder = crate::find::Finder::new(crate::catalogue::ENTRIES, None);
        let asker = crate::ask::Asker::new(None);
        let state = claudius_api::State::default();
        let typed = crate::find::answer("genrate", 8, &state, &finder, &asker, None);
        let meant = crate::find::answer("generate", 8, &state, &finder, &asker, None);
        assert_eq!(typed.read_as.as_deref(), Some("generate"));
        assert_eq!((typed.query.as_str(), meant.read_as), ("genrate", None), "the words typed stay the answer's");
        assert_eq!(typed.found, meant.found);
        assert!(typed.found.iter().any(|f| f.id == "new_song"), "{:?}", typed.found);
    }
}
