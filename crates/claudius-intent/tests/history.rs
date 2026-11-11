//! Evals of finding a change made to a song from words of one's own, as the palette offers to go back to before it:
//! a song's history as claudius describes its changes (`claudius_session::history::describe`), the latest first, and
//! the ways people ask to go back to one. Like `meaning.rs`, they need the embedding model (`CLAUDIUS_INTENT_MODEL`:
//! claudius's own file of it, or a folder of Model2Vec's files), and are skipped without it.

use claudius_intent::{Item, Model, find_among};

/// a song's changes, the latest first: each change's id, what undo calls it, and what it changed in words
const HISTORY: &[(&str, &str, &str)] = &[
    ("13@x", "mix guitar: drive 0.95", "guitar: fx 0 drive 0.6 to 0.95, more distortion"),
    ("12@x", "add track whistle (lead)", "added the whistle track (lead), playing the synth whistle"),
    ("11@x", "tempo 128", "bpm 124 to 128, faster"),
    ("10@x", "remove track hat", "took out the hat drum lane (it played the drum hat)"),
    ("9@x", "key D minor", "the key's tonic A to D; another key, transposed"),
    (
        "8@x",
        "add section bridge",
        "added the bridge section of 8 bars, a verse, a bridge, a middle eight; put the sections in another order",
    ),
    ("7@x", "sound bass: reese", "bass plays the synth reese"),
    ("6@x", "mix vocals: level -3", "vocals: mix level db -6 to -3, louder"),
    ("5@x", "swing 0.6", "swing 0.5 to 0.6, more swing"),
    ("4@x", "add track strings (pad)", "added the strings track (pad), playing the instrument vsco2:strings"),
    (
        "3@x",
        "remove section breakdown",
        "took out the breakdown section of 16 bars, a breakdown, a quiet break in the middle",
    ),
    ("2@x", "add track congas (percussion)", "added the congas track (percussion), playing the instrument vcsl:congas"),
    (
        "1@x",
        "new song: Night Bus — house at 124 BPM in A minor (seed 3)",
        "a new song: Night Bus house at 124 BPM in A minor, playing kick, clap, hat, bass, chords, pad, lead",
    ),
];

/// what people say, and the change each means
const ASKED: &[(&str, &str)] = &[
    ("go back to before we added the whistles", "12@x"),
    ("before the whistle", "12@x"),
    ("undo everything since the whistle came in", "12@x"),
    ("before the tempo change", "11@x"),
    ("go back to before we sped it up to 128", "11@x"),
    ("before we took the hats out", "10@x"),
    ("back to when it still had hi hats", "10@x"),
    ("before the key change", "9@x"),
    ("go back to before it was in D minor", "9@x"),
    ("before we added the bridge", "8@x"),
    ("before the bridge section", "8@x"),
    ("before the reese bass", "7@x"),
    ("back to before the bass sound changed", "7@x"),
    ("before I turned the vocals up", "6@x"),
    ("go back to before the vocal level change", "6@x"),
    ("before the swing", "5@x"),
    ("go back to before we made it swing more", "5@x"),
    ("before we added strings", "4@x"),
    ("back to before the string pad", "4@x"),
    ("before the breakdown was removed", "3@x"),
    ("go back to when there was still a breakdown", "3@x"),
    ("before the congas", "2@x"),
    ("back to before the percussion track", "2@x"),
    ("before we cranked the guitar drive", "13@x"),
    ("go back to before the guitar overdrive went to 95", "13@x"),
];

/// phrasings that share few words with the change they mean, held out from the ranking's tuning
const HELD_OUT: &[(&str, &str)] = &[
    ("rewind to before the whistling", "12@x"),
    ("before it got faster", "11@x"),
    ("back to the slower tempo", "11@x"),
    ("before the high hats disappeared", "10@x"),
    ("before we transposed it", "9@x"),
    ("before the middle eight", "8@x"),
    ("before the bass got grittier", "7@x"),
    ("before the singing got louder", "6@x"),
    ("back to the straighter groove", "5@x"),
    ("before the orchestra came in", "4@x"),
    ("before the violins", "4@x"),
    ("before the bongos", "2@x"),
    ("before the guitar got heavier", "13@x"),
    ("before more distortion on the guitar", "13@x"),
    ("back to the version with the quiet part in the middle", "3@x"),
];

/// how many of `asked` find the change they mean first, and among the first three, with each missed said on stderr
fn score(items: &[Item], model: &Model, asked: &[(&str, &str)]) -> (usize, usize) {
    let (mut first, mut three) = (0, 0);
    for (words, meant) in asked {
        let ids: Vec<String> = find_among(words, items, Some(model), 3).into_iter().map(|f| f.id).collect();
        first += usize::from(ids.first().is_some_and(|id| id == meant));
        let hit = ids.iter().any(|id| id == meant);
        three += usize::from(hit);
        if !hit {
            eprintln!("'{words}': {ids:?}, where {meant}");
        }
    }
    (first, three)
}

#[test]
fn the_change_people_mean_is_found_among_a_songs_history() {
    // a folder of Model2Vec's files, or claudius's own file of the model, as its cache keeps it
    let Some(path) = std::env::var_os("CLAUDIUS_INTENT_MODEL").map(std::path::PathBuf::from) else {
        eprintln!("CLAUDIUS_INTENT_MODEL names no model: skipped");
        return;
    };
    let model = if path.is_file() {
        Model::from_bytes(&std::fs::read(&path).unwrap()).unwrap()
    } else {
        Model::from_model2vec(
            &std::fs::read(path.join("model.safetensors")).unwrap(),
            &std::fs::read_to_string(path.join("vocab.txt")).unwrap(),
        )
        .unwrap()
    };
    let items: Vec<Item> = HISTORY.iter().map(|(id, name, words)| Item { id, name, words }).collect();
    let started = std::time::Instant::now();
    let (first, three) = score(&items, &model, ASKED);
    let each = started.elapsed() / ASKED.len() as u32;
    let n = ASKED.len();
    eprintln!(
        "first: {first} of {n}; in the first three: {three} of {n}; a search of {} changes in {each:?}",
        HISTORY.len()
    );
    assert!(three * 100 >= n * 90 && first * 100 >= n * 75, "{first} first and {three} in three of {n}");
    let (held_first, held_three) = score(&items, &model, HELD_OUT);
    let h = HELD_OUT.len();
    eprintln!("held out: first {held_first} of {h}; in the first three {held_three} of {h}");
    assert!(
        held_three * 100 >= h * 75 && held_first * 100 >= h * 70,
        "held out: {held_first} first and {held_three} in three of {h}"
    );
}
