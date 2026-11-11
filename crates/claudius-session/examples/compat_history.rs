//! Writes the history a song's folder keeps (`history.jsonl`) into the folder compat_fixtures writes for a release,
//! which every later claudius has to replay:
//!
//!   cargo run -p claudius-session --example compat_history -- crates/claudius/tests/compat/v<version>/folder
//!
//! The history of the house song in that folder: the song made, edits of it, and each undone, so that the log comes to
//! the song the folder holds with a step to undo and steps to redo. `cargo xtask release prepare` runs it after
//! compat_fixtures.

use std::path::Path;

use claudius::compose::{self, Options};
use claudius::{genres, script};
use claudius_session::history::History;

/// edits of the tempo, a part, the mix and a drum lane
const EDITS: [(&str, &str); 4] = [
    ("tempo 122", "tempo(122);"),
    ("chords on the electric piano", r#"part("chords", #{ preset: "epiano" });"#),
    ("lead down 2 dB", r#"mix("lead", #{ level: -2, reverb: 0.35 });"#),
    ("four on the floor", r#"drum("kick", "main", "X...X...X...X...");"#),
];

fn main() -> Result<(), String> {
    let dir = std::env::args().nth(1).ok_or("usage: compat_history <folder>")?;
    let house = genres::get("house").map_err(|e| e.to_string())?;
    let mut song = Some(compose::generate(&house, 7, &Options { short: true, ..Default::default() }, None));
    let mut h = History::default();
    h.commit_song(None, &song, "made a house song");
    for (label, code) in EDITS {
        let before = song.clone();
        song = script::run(code, song.clone(), None)?.bp;
        h.commit(before, &song, label);
    }
    for _ in EDITS {
        h.undo(&mut song).ok_or("an edit would not undo")?;
    }
    let file = Path::new(&dir).join("history.jsonl");
    std::fs::write(&file, h.to_jsonl()).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    eprintln!("compat_history: wrote the history of this claudius into {}", file.display());
    Ok(())
}
