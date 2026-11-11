//! Every history a release's song folder kept still replays in this claudius: to the song the folder holds, with the
//! steps to undo and redo it had (`crates/claudius/tests/compat/v<version>/folder/history.jsonl`, written by the
//! compat_history example as the release is prepared; no release before 0.6.0 kept a history).

use std::path::{Path, PathBuf};

use claudius_session::history::History;

/// the release a folder of tests/compat is from (`v0.6.0`)
fn release(dir: &Path) -> Option<(u64, u64, u64)> {
    let name = dir.file_name()?.to_str()?.strip_prefix('v')?;
    let mut parts = name.split('.').map(|p| p.parse().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

#[test]
fn every_history_a_release_kept_still_replays() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../claudius/tests/compat");
    let mut releases: Vec<PathBuf> = std::fs::read_dir(&root).unwrap().flatten().map(|r| r.path()).collect();
    releases.sort();
    assert!(releases.iter().filter(|r| r.join("folder").is_dir()).count() >= 5, "no song folders in {root:?}");
    for dir in releases.iter().filter(|r| release(r).is_some_and(|v| v > (0, 5, 0))) {
        let folder = dir.join("folder");
        let log = std::fs::read_to_string(folder.join("history.jsonl"))
            .unwrap_or_else(|e| panic!("{}: a release after 0.5.0 keeps its history: {e}", folder.display()));
        let song = claudius::song::read_blueprint(&folder.join("blueprint.json"))
            .unwrap_or_else(|e| panic!("{}: the song no longer opens: {e:#}", folder.display()));
        let mut h = History::open(&log, &Some(song.clone()));
        // a log that no longer reads is a history begun anew, and one that no longer comes to the song gains an edit
        // "changed outside claudius"
        let lines = log.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(h.log().len(), lines, "{}: the history no longer replays to the song", folder.display());
        let (undo, redo) = h.steps();
        assert!(undo >= 1 && redo >= 1, "{}: the steps are gone: {undo} to undo, {redo} to redo", folder.display());
        let mut now = Some(song.clone());
        h.redo(&mut now).unwrap_or_else(|| panic!("{}: nothing redoes", folder.display()));
        assert!(now.is_some_and(|n| n != song), "{}: the redo changed nothing", folder.display());
    }
}
