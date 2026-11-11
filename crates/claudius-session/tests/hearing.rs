//! What is said to the palette, heard as the palette hears it (`hearing`), with `CLAUDIUS_WHISTLE_CLIPS` a folder of
//! WAV files, each beside the words said in it (`<name>.txt`, empty for silence). The speech model comes from
//! claudius's cache (`CLAUDIUS_HOME`), and downloads there when it is not in it.

use claudius_api::{SongView, State, TrackView};

#[test]
fn what_is_said_is_heard_as_it_was_said() {
    let Some(dir) = std::env::var_os("CLAUDIUS_WHISTLE_CLIPS") else {
        eprintln!("CLAUDIUS_WHISTLE_CLIPS names no folder of clips: skipped");
        return;
    };
    let tracks = ["bass", "drums", "piano", "guitar", "snare", "vocals"].map(|t| TrackView {
        name: t.into(),
        role: t.into(),
        ..TrackView::default()
    });
    let state = State { song: Some(SongView { tracks: tracks.to_vec(), ..SongView::default() }), ..State::default() };
    let mut clips: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "wav") && p.with_extension("txt").is_file())
        .collect();
    clips.sort();
    assert!(!clips.is_empty(), "no clip with its words beside it in {dir:?}");
    let (mut words, mut missed) = (0, 0);
    for clip in clips {
        let said = std::fs::read_to_string(clip.with_extension("txt")).unwrap();
        let heard = claudius_session::hearing::hear(std::fs::read(&clip).unwrap(), &state).unwrap();
        let (n, m) = (said.split_whitespace().count(), missed_words(&said, &heard.text));
        eprintln!("{}: {:?}, read as {:?}: {m} of {n} words missed", clip.display(), heard.heard, heard.text);
        (words, missed) = (words + n, missed + m);
    }
    eprintln!("{missed} of {words} words missed");
    assert!(missed * 5 <= words, "more than one word in five missed");
}

/// how many words of `said` are missed in `heard`: the fewest words changed, added or dropped to make one the other
fn missed_words(said: &str, heard: &str) -> usize {
    let words = |t: &str| -> Vec<String> {
        t.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .filter(|w| !w.is_empty())
            .collect()
    };
    let (a, b) = (words(said), words(heard));
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut diagonal = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let above = row[j];
            row[j] = (diagonal + usize::from(a[i - 1] != b[j - 1])).min(above + 1).min(row[j - 1] + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

#[test]
fn words_missed_count_the_fewest_changes_between_them() {
    assert_eq!(missed_words("more reverb on the piano", "More reverb on the piano."), 0);
    assert_eq!(missed_words("more reverb on the piano", "moreiver bone the piano"), 3);
    assert_eq!(missed_words("cut the guitar from one minute two seconds", "cut the guitar from one minute to two"), 2);
    assert_eq!(missed_words("", "uh"), 1);
}
