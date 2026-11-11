//! What a person says to the palette, as the speech model heard it, in the words the readings read (`ask`). Speech is
//! heard as words, and a word claudius knows can come out as another that sounds like it ("undu" for undo, "base" for
//! bass). A word heard that claudius does not know, one letter from one word it does and from no other, is taken as
//! that word: the song's tracks first, then the words of the studio's actions and of the example requests.

use std::collections::HashSet;
use std::sync::LazyLock;

use claudius_api::State;

/// the words of the studio's actions and of the example requests, which the readings know
static KNOWN: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut known = HashSet::new();
    for e in crate::catalogue::ENTRIES {
        known.extend(words(e.name).chain(words(e.words)));
    }
    for line in crate::ask::ASKS.lines() {
        if let Ok(example) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(ask) = example["ask"].as_str()
        {
            known.extend(words(ask));
        }
    }
    known
});

/// a text's words, in lower case
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).map(str::to_lowercase)
}

/// `heard` in the words the readings read: each word claudius does not know put right where it is one letter from a
/// word it knows and from no other, the song's tracks before the rest, and without the stop a sentence ends with
pub fn heard(heard: &str, state: &State) -> String {
    let tracks: HashSet<String> = state
        .song
        .iter()
        .flat_map(|s| &s.tracks)
        .flat_map(|t| [&t.name, &t.role, &t.sound])
        .flat_map(|w| words(w).collect::<Vec<_>>())
        .collect();
    let said: Vec<String> = heard.split_whitespace().map(|w| put_right(w, &tracks)).collect();
    said.join(" ").trim_end_matches(['.', '!', '?']).to_string()
}

/// a word heard, put right if claudius does not know it and it is one letter from a word claudius knows and no other
fn put_right(heard: &str, tracks: &HashSet<String>) -> String {
    let bare = heard.trim_matches(|c: char| !c.is_alphanumeric());
    let word = bare.to_lowercase();
    let unknown = word.chars().count() >= 4
        && !word.chars().any(|c| c.is_ascii_digit())
        && !tracks.contains(&word)
        && !KNOWN.contains(&word);
    let near = |among: &HashSet<String>| {
        let mut near = among.iter().filter(|k| k.chars().count() >= 4 && one_apart(&word, k));
        match (near.next(), near.next()) {
            (Some(k), None) => Some(k.clone()),
            _ => None,
        }
    };
    match unknown.then(|| near(tracks).or_else(|| near(&KNOWN))).flatten() {
        Some(known) => heard.replacen(bare, &known, 1),
        None => heard.to_string(),
    }
}

/// whether one letter changed, added or dropped, or two beside each other swapped, make `a` into `b`
fn one_apart(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (short, long) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    match long.len() - short.len() {
        0 => {
            let apart: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
            match apart[..] {
                [_] => true,
                [i, j] => j == i + 1 && a[i] == b[j] && a[j] == b[i],
                _ => false,
            }
        }
        1 => {
            let i = (0..short.len()).find(|&i| short[i] != long[i]).unwrap_or(short.len());
            short[i..] == long[i + 1..]
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use claudius_api::{SongView, State, TrackView};

    use super::{heard, one_apart};

    fn song(tracks: &[(&str, &str, &str)]) -> State {
        let tracks = tracks
            .iter()
            .map(|&(name, role, sound)| TrackView {
                name: name.into(),
                role: role.into(),
                sound: sound.into(),
                ..TrackView::default()
            })
            .collect();
        State { song: Some(SongView { tracks, ..SongView::default() }), ..State::default() }
    }

    #[test]
    fn a_word_heard_near_one_claudius_knows_is_taken_as_it() {
        let state = song(&[("bass", "bass", "reese"), ("piano", "keys", "grand piano")]);
        assert_eq!(heard("Undu the last two changes.", &state), "undo the last two changes");
        assert_eq!(heard("Turn the base down by three decibels.", &state), "Turn the bass down by three decibels");
        assert_eq!(heard("Make the vocals louder.", &state), "Make the vocals louder", "words it knows stay");
        assert_eq!(heard("Moreiver bone the piano", &state), "Moreiver bone the piano", "far from any it knows");
    }

    #[test]
    fn the_songs_tracks_are_the_words_it_knows_best() {
        let state = song(&[("whistle", "lead", "synth whistle")]);
        assert_eq!(heard("mute the whsitle", &state), "mute the whistle");
        assert_eq!(heard("mute the whsitle", &State::default()), "mute the whistle", "a word of the examples too");
    }

    #[test]
    fn one_letter_changed_added_dropped_or_two_swapped_is_one_apart() {
        assert!(one_apart("base", "bass") && one_apart("undu", "undo"));
        assert!(one_apart("drum", "drums") && one_apart("drums", "drum") && one_apart("vocasl", "vocals"));
        assert!(!one_apart("bass", "bass") && !one_apart("base", "bars") && !one_apart("bass", "piano"));
    }
}
