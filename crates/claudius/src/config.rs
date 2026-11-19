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
        .unwrap_or_else(|| dirs::cache_dir().unwrap_or_else(temp_dir).join("claudius"))
}

/// where claudius keeps what it learns of the person (their voice): $CLAUDIUS_HOME, or claudius in the user's settings
/// folder (~/Library/Application Support on macOS, ~/.config on Linux)
pub fn settings_dir() -> PathBuf {
    std::env::var_os("CLAUDIUS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::config_dir().unwrap_or_else(temp_dir).join("claudius"))
}

/// the system's temporary folder; in a browser, which has no files (and whose std panics when asked), no folder at all
pub fn temp_dir() -> PathBuf {
    if cfg!(target_arch = "wasm32") { PathBuf::new() } else { std::env::temp_dir() }
}

/// whether there are files to keep: none in a browser, where a cache is only memory
pub const FILES: bool = !cfg!(target_arch = "wasm32");

/// this process's id, which names what it writes before renaming it into place; a browser has no processes (and its
/// std panics when asked), and no files to write, and gets 0
pub fn process_id() -> u32 {
    if cfg!(target_arch = "wasm32") { 0 } else { std::process::id() }
}

/// Makes the file `path` with `write`, which writes it under a hidden name beside it (`.song.wav.<process id>.part`)
/// that then takes its place: the file appears only once it is whole, and a write that fails (on a full disk) leaves
/// no part of itself behind, and the file as it was.
pub fn write_whole(path: &Path, write: impl FnOnce(&Path) -> anyhow::Result<()>) -> anyhow::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let part = path.with_file_name(format!(".{name}.{}.part", process_id()));
    let done = write(&part).and_then(|()| Ok(std::fs::rename(&part, path)?));
    if done.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    done.map_err(|e| e.context(format!("cannot write {}", path.display())))
}

/// what a disk keeps free besides what is written to it: one filled to the last byte stops the system too
const SPARE: u64 = 256 << 20;

/// Whether the disk that holds `dir` (or will: the nearest folder of it there is) has room for `bytes` more, with
/// `SPARE` left; not, in a sentence that says what is free. A disk that does not say has room.
pub fn room_for(dir: &Path, bytes: u64) -> anyhow::Result<()> {
    free_space(dir).map_or(Ok(()), |free| room(free, bytes))
}

/// the room left for this user on the disk that holds `dir`, or will
#[cfg(not(target_arch = "wasm32"))]
fn free_space(dir: &Path) -> Option<u64> {
    dir.ancestors().find(|d| d.exists()).and_then(|d| fs4::available_space(d).ok())
}

/// a browser has no disk to fill
#[cfg(target_arch = "wasm32")]
fn free_space(_: &Path) -> Option<u64> {
    None
}

fn room(free: u64, bytes: u64) -> anyhow::Result<()> {
    if free >= bytes.saturating_add(SPARE) {
        return Ok(());
    }
    anyhow::bail!("there is not room for it on the disk: it takes about {}, and {} is free", size(bytes), size(free))
}

/// `bytes` as people read a file's size: `340 MB`, `1.2 GB`
pub fn size(bytes: u64) -> String {
    match bytes as f64 / 1e6 {
        mb if mb >= 1000.0 => format!("{:.1} GB", mb / 1000.0),
        mb if mb >= 10.0 => format!("{mb:.0} MB"),
        mb => format!("{mb:.1} MB"),
    }
}

pub fn cache_dir(name: &str) -> PathBuf {
    let d = home().join(name);
    let _ = std::fs::create_dir_all(&d);
    d
}

/// The audio caches on disk (`decoded`, `stretched`), each a folder of files under a cap: what is read is marked used,
/// and a folder written past its cap loses the files used least lately until it is three quarters of it. Without a cap
/// they grew with every recording ever opened: a song's decoded audio is about ten times its MP3.
pub mod cache {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// the caps: decoded sounds, and sounds stretched to a tempo
    pub const DECODED: u64 = 2 << 30;
    pub const STRETCHED: u64 = 1 << 30;

    /// what the cache `name` keeps under `key`, marked used
    pub fn get(name: &str, key: &str) -> Option<Vec<u8>> {
        let path = super::cache_dir(name).join(key);
        let bytes = std::fs::read(&path).ok()?;
        used(&path);
        Some(bytes)
    }

    /// the file at `path` marked used now, for the trimming to keep it over those used less lately
    #[cfg(not(target_arch = "wasm32"))]
    fn used(path: &Path) {
        if let Ok(f) = std::fs::File::options().write(true).open(path) {
            let _ = f.set_modified(crate::clock::SystemTime::now());
        }
    }

    /// a browser keeps no files, and has no clock std can read
    #[cfg(target_arch = "wasm32")]
    fn used(_: &Path) {}

    /// keeps `bytes` in the cache `name` under `key`, written whole (a part file renamed), and now and then trims the
    /// cache to `cap`
    pub fn put(name: &str, key: &str, bytes: &[u8], cap: u64) {
        let dir = super::cache_dir(name);
        let path = dir.join(key);
        let tmp = path.with_extension(format!("{}.tmp", super::process_id()));
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        // a look at the folder every few writes: listing it each time would cost more than the writes
        static WRITES: AtomicU32 = AtomicU32::new(0);
        if WRITES.fetch_add(1, Ordering::Relaxed).is_multiple_of(16) {
            trim(&dir, cap);
        }
    }

    /// the files of `dir` used least lately removed until what it holds is three quarters of `cap`, when it holds more
    /// than `cap`; part files of other processes are left alone. Returns how many bytes were freed.
    pub fn trim(dir: &Path, cap: u64) -> u64 {
        let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
        // when each was used last, as its file says: a file that cannot say counts as used longest ago
        let mut files: Vec<(_, u64, PathBuf)> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x != "tmp"))
            .filter_map(|e| {
                let md = e.metadata().ok().filter(|m| m.is_file())?;
                Some((md.modified().ok(), md.len(), e.path()))
            })
            .collect();
        let mut held: u64 = files.iter().map(|(_, len, _)| len).sum();
        if held <= cap {
            return 0;
        }
        files.sort_by_key(|(used, _, _)| *used);
        let (goal, mut freed) = (cap / 4 * 3, 0);
        for (_, len, path) in files {
            if held <= goal {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                held -= len;
                freed += len;
            }
        }
        freed
    }
}

pub const APPLE_LOOP_DIRS: &[&str] = &["/Library/Audio/Apple Loops"];

/// where songs are saved unless told otherwise: ~/Music/claudius (./out without a Music folder)
pub fn songs_dir() -> PathBuf {
    dirs::audio_dir().map(|d| d.join("claudius")).unwrap_or_else(|| PathBuf::from("out"))
}

/// the folders a claudius shared beyond this computer may read and write: saved songs, its own caches, and the sound
/// libraries (sample packs, samples downloaded from online libraries, Apple Loops)
pub fn shared_roots() -> Vec<PathBuf> {
    // claudius' own folders exist from the start: a folder that does not exist cannot hold an allowed path
    for own in [songs_dir(), home()] {
        let _ = std::fs::create_dir_all(own);
    }
    let mut roots = vec![songs_dir(), home(), claudius_packs::packs_dir(), claudius_packs::online::samples_dir()];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cache_past_its_cap_loses_what_was_used_least_lately() {
        let dir = std::env::temp_dir().join(format!("claudius-cache-trim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let now = std::time::SystemTime::now();
        // a used longest ago, d last
        for (i, name) in ["a.f32", "b.f32", "c.f32", "d.f32"].iter().enumerate() {
            let p = dir.join(name);
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let f = std::fs::File::options().write(true).open(&p).unwrap();
            f.set_modified(now - std::time::Duration::from_secs(100 * (4 - i as u64))).unwrap();
        }
        // another process's file being written is left alone
        std::fs::write(dir.join("e.123.tmp"), vec![0u8; 100]).unwrap();
        assert_eq!(cache::trim(&dir, 400), 0, "within its cap");
        // 400 held, the cap 300: down to three quarters of it, so the two used least lately go
        assert_eq!(cache::trim(&dir, 300), 200);
        let mut left: Vec<String> =
            std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["c.f32", "d.f32", "e.123.tmp"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_written_whole_appears_only_once_complete_and_a_failed_write_leaves_the_old_one_and_no_part() {
        let dir = std::env::temp_dir().join(format!("claudius-whole-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("song.wav");
        write_whole(&file, |p| Ok(std::fs::write(p, "the first")?)).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "the first");
        let failed = write_whole(&file, |p| {
            std::fs::write(p, "half of the sec")?;
            anyhow::bail!("No space left on device")
        });
        let e = format!("{:#}", failed.unwrap_err());
        assert!(e.contains("cannot write") && e.contains("No space left"), "{e}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "the first");
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["song.wav"], "the part is left behind");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_write_too_big_for_the_disk_is_refused_with_what_it_takes_and_what_is_free() {
        assert!(room(2_000_000_000, 1_000_000_000).is_ok());
        // what the system needs is kept free too
        let e = room(1_100_000_000, 1_000_000_000).unwrap_err().to_string();
        assert_eq!(e, "there is not room for it on the disk: it takes about 1.0 GB, and 1.1 GB is free");
        assert!(room_for(&std::env::temp_dir().join("not yet/made"), u64::MAX / 4).is_err());
        assert_eq!([size(340_400_000), size(1_234_000_000), size(4_200_000)], ["340 MB", "1.2 GB", "4.2 MB"]);
    }
}
