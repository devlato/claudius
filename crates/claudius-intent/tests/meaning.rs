//! The palette against the embedding model itself (potion-base-8M), on phrasings people use for each action: which it
//! finds first, and in its first three. It needs the model's files, `model.safetensors` and `vocab.txt`, in the folder
//! `CLAUDIUS_INTENT_MODEL` names; without them it is skipped.

use claudius_intent::catalogue::ENTRIES;
use claudius_intent::{Finder, Model};

/// a phrasing, and the actions that answer it
const ASKED: &[(&str, &[&str])] = &[
    ("play the song", &["play"]),
    ("start playback", &["play"]),
    ("let me hear it", &["play", "render"]),
    ("pause the music", &["play", "stop"]),
    ("go back to the beginning", &["to_start"]),
    ("rewind to the top", &["to_start", "stop"]),
    ("from the top", &["to_start"]),
    ("stop playing", &["stop", "play"]),
    ("repeat this part", &["loop"]),
    ("play the chorus on repeat", &["loop"]),
    ("loop the drop", &["loop"]),
    ("undo that", &["undo"]),
    ("go back one step", &["undo"]),
    ("revert my last change", &["undo"]),
    ("oops take that back", &["undo"]),
    ("bring back what I undid", &["redo"]),
    ("redo the change", &["redo"]),
    ("make a new song", &["new_song"]),
    ("start a fresh track", &["new_song"]),
    ("compose something new", &["new_song"]),
    ("generate a beat", &["new_song"]),
    ("write a song in trap", &["new_song"]),
    ("show me an example song", &["example"]),
    ("load a demo track", &["example", "open"]),
    ("open a file", &["open"]),
    ("load my project", &["open"]),
    ("open a song I saved", &["open"]),
    ("save it", &["save"]),
    ("download the song", &["save"]),
    ("export as mp3", &["save", "stems"]),
    ("get a wav file", &["save", "stems"]),
    ("render the song", &["render"]),
    ("how good is my song", &["render", "fix_all"]),
    ("what does the critic think", &["render", "fix_all"]),
    ("score my track", &["render"]),
    ("record my voice", &["record"]),
    ("sing over the track", &["record"]),
    ("record a guitar part", &["record"]),
    ("use the microphone", &["record"]),
    ("lay down some vocals", &["record", "add_recording"]),
    ("import an audio file", &["add_recording", "open"]),
    ("sample a song", &["add_recording"]),
    ("find a free sample online", &["add_recording", "tab:sounds"]),
    ("chop up a track", &["add_recording"]),
    ("use my phone as a controller", &["phone"]),
    ("scratch on my phone", &["phone"]),
    ("draw in the arrangement", &["paint"]),
    ("edit the drums", &["tab:drums"]),
    ("change the beat", &["tab:drums"]),
    ("program the kick pattern", &["tab:drums"]),
    ("change the bass sound", &["tab:instruments", "tab:sounds"]),
    ("edit the synth", &["tab:instruments"]),
    ("edit the melody", &["tab:notes"]),
    ("open the piano roll", &["tab:notes"]),
    ("change the chord progression", &["tab:chords"]),
    ("change the key of the song", &["tab:chords"]),
    ("make it louder", &["tab:mixer"]),
    ("turn the bass down", &["tab:mixer"]),
    ("add some reverb", &["tab:mixer"]),
    ("pan the guitar to the left", &["tab:mixer"]),
    ("balance the levels", &["tab:mixer"]),
    ("swap the kick sound", &["tab:sounds", "tab:drums"]),
    ("browse the samples", &["tab:sounds"]),
    ("edit the code", &["tab:script"]),
    ("show the routing of the effects", &["tab:flow", "flow_large"]),
    ("show me the signal chain", &["tab:flow", "flow_large"]),
    ("what happened", &["tab:log"]),
    ("show the history of changes", &["tab:log"]),
    ("give me some ideas", &["variations"]),
    ("try something different", &["variations"]),
    ("show me alternatives", &["variations"]),
    ("make the song better", &["improve", "fix_all"]),
    ("optimise the track", &["improve"]),
    ("fix the problems", &["fix_all"]),
    ("repair the mix", &["fix_all", "tab:mixer"]),
    ("stop the render", &["cancel"]),
    ("cancel that job", &["cancel"]),
    ("export the stems", &["stems"]),
    ("export each track separately", &["stems"]),
    ("install real instruments", &["packs"]),
    ("download better sounds", &["packs"]),
    ("release my song", &["publish"]),
    ("put it on spotify", &["publish"]),
    ("connect claude", &["assistants"]),
    ("use chatgpt with it", &["assistants"]),
    ("hook up an ai assistant", &["assistants"]),
    ("what functions can I use in a script", &["reference", "tab:script"]),
    ("script syntax help", &["reference"]),
    ("show me around", &["tour"]),
    ("how does this work", &["tour", "docs"]),
    ("open the manual", &["docs"]),
    ("documentation", &["docs"]),
];

/// phrasings written after the words of the actions were last changed, and not looked at while changing them: how well
/// the palette does on what it was not shaped by
const HELD_OUT: &[(&str, &[&str])] = &[
    ("hit play", &["play"]),
    ("stop the music", &["stop", "play"]),
    ("restart from the beginning", &["to_start"]),
    ("keep repeating this section", &["loop"]),
    ("take back what I just did", &["undo"]),
    ("i want a brand new song", &["new_song"]),
    ("make me a house track", &["new_song"]),
    ("export my song", &["save", "stems"]),
    ("rate my track", &["render"]),
    ("record vocals", &["record"]),
    ("bring in an mp3", &["add_recording", "open"]),
    ("change the snare", &["tab:drums", "tab:sounds"]),
    ("change the lead synth", &["tab:instruments", "tab:sounds"]),
    ("write a melody", &["tab:notes"]),
    ("change the harmony", &["tab:chords"]),
    ("turn it down", &["tab:mixer"]),
    ("more echo on the vocals", &["tab:mixer"]),
    ("give me variations", &["variations"]),
    ("improve it automatically", &["improve"]),
    ("fix the issues", &["fix_all"]),
    ("abort the job", &["cancel"]),
    ("separate stems", &["stems"]),
    ("get the sample packs", &["packs"]),
    ("publish to streaming services", &["publish"]),
    ("connect cursor", &["assistants"]),
    ("help", &["tour", "docs", "reference"]),
    ("see what changed", &["tab:log"]),
    ("show the effects routing", &["tab:flow", "flow_large"]),
    ("paint the arrangement", &["paint"]),
    ("play pads on my phone", &["phone"]),
];

/// phrasings written once the first held-out set had been looked at, before the palette was run on them
const HELD_OUT_2: &[(&str, &[&str])] = &[
    ("louder please", &["tab:mixer"]),
    ("mute the drums", &["tab:mixer"]),
    ("solo the bass", &["tab:mixer"]),
    ("start recording", &["record"]),
    ("new beat", &["new_song"]),
    ("generate music", &["new_song"]),
    ("open a recent song", &["open"]),
    ("judge it", &["render"]),
    ("listen to it again", &["play", "render"]),
    ("show the drum machine", &["tab:drums"]),
    ("piano notes", &["tab:notes"]),
    ("edit chords", &["tab:chords"]),
    ("open the code editor", &["tab:script"]),
    ("try another version", &["variations"]),
    ("let it evolve", &["improve"]),
    ("cancel", &["cancel"]),
    ("download the stems", &["stems"]),
    ("upload it to spotify", &["publish"]),
    ("connect an mcp client", &["assistants"]),
    ("user guide", &["docs", "tour"]),
];

/// how many of `asked` the palette finds first, and in its first three, with each it misses said on stderr
fn score(finder: &Finder, model: &Model, asked: &[(&str, &[&str])]) -> (usize, usize) {
    let (mut first, mut three) = (0, 0);
    for (words, answers) in asked {
        let ids: Vec<String> = finder.find(words, Some(model), 3).into_iter().map(|f| f.id).collect();
        first += usize::from(ids.first().is_some_and(|id| answers.contains(&id.as_str())));
        let hit = ids.iter().any(|id| answers.contains(&id.as_str()));
        three += usize::from(hit);
        if !hit {
            eprintln!("'{words}': {ids:?}, where {answers:?}");
        }
    }
    (first, three)
}

#[test]
fn phrasings_people_use_find_the_action_they_mean() {
    // a folder of Model2Vec's files, or claudius's own file of the model, as its cache keeps it
    let Some(path) = std::env::var_os("CLAUDIUS_INTENT_MODEL").map(std::path::PathBuf::from) else {
        eprintln!("CLAUDIUS_INTENT_MODEL names no model: skipped");
        return;
    };
    let started = std::time::Instant::now();
    let bytes = if path.is_file() {
        std::fs::read(&path).unwrap()
    } else {
        Model::from_model2vec(
            &std::fs::read(path.join("model.safetensors")).unwrap(),
            &std::fs::read_to_string(path.join("vocab.txt")).unwrap(),
        )
        .unwrap()
        .to_bytes()
    };
    let converted = started.elapsed();
    // the model as claudius keeps it and reads it back, which is what the palette finds with
    let started = std::time::Instant::now();
    let model = Model::from_bytes(&bytes).unwrap();
    let read = started.elapsed();
    let started = std::time::Instant::now();
    let finder = Finder::new(ENTRIES, Some(&model));
    let embedded = started.elapsed();
    let started = std::time::Instant::now();
    for (words, _) in ASKED {
        finder.find(words, Some(&model), 12);
    }
    let each = started.elapsed() / ASKED.len() as u32;
    eprintln!(
        "converted in {converted:?} to {} bytes, read in {read:?}, the actions embedded in {embedded:?}, a search in \
         {each:?}",
        bytes.len()
    );
    let (first, three) = score(&finder, &model, ASKED);
    let n = ASKED.len();
    eprintln!("first: {first} of {n}; in the first three: {three} of {n}");
    assert!(three * 100 >= n * 95 && first * 100 >= n * 80, "{first} first and {three} in three of {n}");
    for (name, set) in [("held out", HELD_OUT), ("held out later", HELD_OUT_2)] {
        let (held_first, held_three) = score(&finder, &model, set);
        let h = set.len();
        eprintln!("{name}: first {held_first} of {h}; in the first three {held_three} of {h}");
        assert!(held_three * 100 >= h * 85 && held_first * 100 >= h * 70, "{name}: {held_first}, {held_three} of {h}");
    }
}
