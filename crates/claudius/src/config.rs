//! Constants and cache locations. Everything inside claudius runs at 48 kHz (earshot's rate) on planar stereo f32.

use std::path::{Component, Path, PathBuf};

pub const SR: u32 = 48_000;
pub const SRF: f64 = SR as f64;
pub const BEATS_PER_BAR: u32 = 4;
pub const STEPS_PER_BEAT: u32 = 4;
pub const STEPS_PER_BAR: u32 = BEATS_PER_BAR * STEPS_PER_BEAT;

/// cache root: $CLAUDIUS_HOME, or claudius in the user's cache folder (~/Library/Caches on macOS, ~/.cache on Linux)
pub fn home() -> PathBuf {
    std::env::var_os("CLAUDIUS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("claudius"))
}

pub fn cache_dir(name: &str) -> PathBuf {
    let d = home().join(name);
    let _ = std::fs::create_dir_all(&d);
    d
}

pub const APPLE_LOOP_DIRS: &[&str] = &["/Library/Audio/Apple Loops"];

/// where songs are saved unless told otherwise: ~/Music/claudius (./out without a Music folder)
pub fn songs_dir() -> PathBuf {
    dirs::audio_dir().map(|d| d.join("claudius")).unwrap_or_else(|| PathBuf::from("out"))
}

/// the folders a claudius shared beyond this computer may read and write: saved songs, its own caches, and the sound
/// libraries (sample packs, Apple Loops)
pub fn shared_roots() -> Vec<PathBuf> {
    // claudius' own folders exist from the start: a folder that does not exist cannot hold an allowed path
    for own in [songs_dir(), home()] {
        let _ = std::fs::create_dir_all(own);
    }
    let mut roots = vec![songs_dir(), home(), claudius_packs::packs_dir()];
    roots.extend(APPLE_LOOP_DIRS.iter().map(PathBuf::from));
    if let Some(h) = dirs::home_dir() {
        roots.push(h.join("Library/Audio/Apple Loops"));
    }
    roots
}

/// is the existing file or folder `p` inside one of `roots` once links and `..` are resolved? `None` allows anything
pub fn path_allowed(p: &Path, roots: Option<&[PathBuf]>) -> bool {
    let Some(roots) = roots else { return true };
    let Ok(real) = p.canonicalize() else { return false };
    roots.iter().filter_map(|r| r.canonicalize().ok()).any(|r| real.starts_with(r))
}

/// may `p` be created? Its nearest existing ancestor must be allowed and the rest of the path plain names
pub fn write_allowed(p: &Path, roots: Option<&[PathBuf]>) -> bool {
    if roots.is_none() {
        return true;
    }
    let mut existing = p;
    while !existing.exists() {
        match existing.parent() {
            Some(parent) => existing = parent,
            None => return false,
        }
    }
    let rest = p.strip_prefix(existing).unwrap_or(Path::new(""));
    rest.components().all(|c| matches!(c, Component::Normal(_))) && path_allowed(existing, roots)
}

/// why `p` cannot be read under `roots`, if it cannot. A missing file is called missing only where it would be inside
/// the folders, so that a shared claudius does not tell which files exist anywhere else.
pub fn read_error(p: &Path, roots: Option<&[PathBuf]>) -> Option<String> {
    if path_allowed(p, roots) && p.exists() {
        None
    } else if roots.is_none() || write_allowed(p, roots) {
        Some(format!("{} does not exist", p.display()))
    } else {
        Some(format!("{} is outside the folders this claudius may read", p.display()))
    }
}

/// GarageBand / Logic keep an index of every installed Apple Loop (tempo, key, instrument, genre, mood)
pub fn apple_loops_db() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let dirs = [
        home.join("Music/Audio Music Apps/Databases"),
        home.join("Library/Containers/com.apple.garageband10/Data/Music/Audio Music Apps/Databases"),
    ];
    let mut found: Vec<PathBuf> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("LoopsDatabaseV") && n.ends_with(".db"))
        })
        .collect();
    found.sort();
    found.pop()
}
