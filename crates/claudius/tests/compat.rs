//! Songs saved by earlier releases still open. tests/compat/v<version>/ holds what a release saved: the blueprint.json
//! of a song in each genre, and of one edited by a script (`<name>.json`), with the song.rhai it printed for each
//! (`<name>.rhai`). The release PR adds its own (`cargo xtask release prepare`), so every later claudius is held to it.

use std::path::{Path, PathBuf};

use claudius::compose::{self, Options};
use claudius::genres;
use claudius::model::{BLUEPRINT_FORMAT, Blueprint};

fn saved(ext: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/compat");
    let mut out = Vec::new();
    for release in std::fs::read_dir(&root).unwrap().flatten() {
        for f in std::fs::read_dir(release.path()).unwrap().flatten() {
            if f.path().extension().is_some_and(|e| e == ext) {
                out.push(f.path());
            }
        }
    }
    out.sort();
    out
}

fn open(path: &Path) -> Blueprint {
    let text = std::fs::read_to_string(path).unwrap();
    Blueprint::from_json(&text).unwrap_or_else(|e| panic!("{} no longer opens: {e}", path.display()))
}

#[test]
fn every_song_an_earlier_release_saved_still_opens() {
    let songs = saved("json");
    assert!(songs.len() >= 18, "the songs of 0.1.0 and 0.2.0 at least: {songs:?}");
    for path in songs {
        let bp = open(&path);
        bp.check_size().unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        claudius::script::check_patterns(&bp).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let p = compose::arrange(&bp);
        assert!(p.bars >= 16 && p.tracks.iter().any(|t| !t.notes.is_empty()), "{} arranges to nothing", path.display());
    }
}

#[test]
fn every_script_an_earlier_release_printed_still_runs_on_its_song() {
    let scripts = saved("rhai");
    assert!(scripts.len() >= 18, "{scripts:?}");
    for path in scripts {
        let code = std::fs::read_to_string(&path).unwrap();
        let song = open(&path.with_extension("json"));
        claudius::script::run(&code, Some(song), None)
            .unwrap_or_else(|e| panic!("{} no longer runs: {e}", path.display()));
    }
}

#[test]
fn a_song_from_a_newer_claudius_asks_for_an_update() {
    let bp = compose::generate(&genres::get("house").unwrap(), 1, &Options { short: true, ..Default::default() }, None);
    assert_eq!(bp.version, BLUEPRINT_FORMAT);
    let mut json: serde_json::Value = serde_json::from_str(&bp.to_json()).unwrap();
    json["version"] = (BLUEPRINT_FORMAT + 1).into();
    let e = Blueprint::from_json(&json.to_string()).unwrap_err();
    assert!(e.contains("a newer claudius wrote it") && e.contains("update claudius"), "{e}");
}
