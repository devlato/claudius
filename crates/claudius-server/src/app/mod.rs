//! The session: one song with undo and redo, the latest render and verdict, a log, and one long job (render,
//! mutate, evolve, save) at a time. Every change bumps `version` and wakes the long-polling clients, so the UI, the
//! MCP tools and the HTTP API all see the same song live.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use claudius::ears::Ears;
use claudius::library::Library;
use claudius::model::*;
use claudius::{audio, config, export, song};
use claudius_api::*;

use crate::views;

mod commands;
mod jobs;
mod release;

pub use commands::analysis_text;
pub use release::{MAX_ARTWORK, report_lines};

pub struct Settings {
    /// read the Apple Loops / user library for recorded sounds
    pub library: bool,
    /// load earshot's models in the background and use them in every verdict
    pub ears: bool,
    /// render after every change
    pub auto_render: bool,
    /// the folders commands and scripts may read and write (None: anywhere, for a claudius only you can reach)
    pub file_roots: Option<Vec<PathBuf>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { library: true, ears: true, auto_render: true, file_roots: None }
    }
}

struct RenderOut {
    view: RenderView,
    stems: BTreeMap<String, PathBuf>,
}

#[derive(Default)]
struct Session {
    version: u64,
    /// version of the last change to the song (what a render belongs to)
    song_version: u64,
    bp: Option<Blueprint>,
    view: Option<SongView>,
    /// earlier songs, with what the change after each was
    undo: Vec<(Option<Blueprint>, String)>,
    redo: Vec<(Option<Blueprint>, String)>,
    render: Option<RenderOut>,
    loudness: BTreeMap<String, f64>,
    verdict: Option<VerdictView>,
    busy: Option<String>,
    progress: Option<f32>,
    log: VecDeque<String>,
    job: bool,
    /// a render was asked for while a job ran
    rerender: Option<bool>,
    /// saves asked for while a job ran: one render into all these folders when the job is done
    pending_saves: Vec<PathBuf>,
    /// the folder the song was opened from or last saved to
    folder: Option<PathBuf>,
    /// the release in the making for the song
    release: Option<release::Prepared>,
}

impl Session {
    fn bump(&mut self) {
        self.version += 1;
    }

    fn say(&mut self, msg: impl Into<String>) {
        for line in msg.into().lines() {
            self.log.push_back(line.to_string());
        }
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }
}

/// an encoded export: the render's version, the format, the file
type Export = (u64, export::Spec, Arc<Vec<u8>>);

pub struct App {
    s: Mutex<Session>,
    /// edits run one at a time (scripts may take a moment: sounds get cut from the library)
    edit: Mutex<()>,
    tx: tokio::sync::watch::Sender<u64>,
    lib: Option<Arc<Library>>,
    ears: Arc<OnceLock<Option<Ears>>>,
    cancel: Arc<AtomicBool>,
    auto_render: AtomicBool,
    cache: PathBuf,
    /// the lock on `cache` that tells other sessions this one is running; the OS lets go of it when the process ends
    _in_use: Option<std::fs::File>,
    genres: Vec<GenreInfo>,
    exports: Vec<ExportFormat>,
    last_export: Mutex<Option<Export>>,
    base: Mutex<String>,
    roots: Option<Vec<PathBuf>>,
}

fn now_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64 % 1_000_000)
}

fn ok(message: impl Into<String>, version: u64) -> Reply {
    Reply { ok: true, message: message.into(), version }
}

fn no(message: impl Into<String>, version: u64) -> Reply {
    Reply { ok: false, message: message.into(), version }
}

impl App {
    pub fn new(settings: Settings) -> Arc<App> {
        let cache = config::cache_dir("renders").join(format!("{}", std::process::id()));
        // a folder left by an ended process with the same id holds none of this session's renders
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::create_dir_all(&cache);
        let in_use = hold(&cache);
        clean_old_sessions(cache.parent().unwrap_or(&cache), &cache, ENDED_GRACE, UNLOCKED_AGE);
        let lib = if settings.library { Library::open().ok().filter(|l| !l.is_empty()).map(Arc::new) } else { None };
        let (tx, _) = tokio::sync::watch::channel(0);
        let app = Arc::new(App {
            s: Mutex::new(Session::default()),
            edit: Mutex::new(()),
            tx,
            lib,
            ears: Arc::new(OnceLock::new()),
            cancel: Arc::new(AtomicBool::new(false)),
            auto_render: AtomicBool::new(settings.auto_render),
            cache,
            _in_use: in_use,
            genres: views::genre_infos(),
            exports: export_formats(),
            last_export: Mutex::new(None),
            base: Mutex::new(format!("http://127.0.0.1:{DEFAULT_PORT}")),
            roots: settings.file_roots,
        });
        {
            let mut s = app.lock();
            s.say(match (&app.lib, claudius_packs::installed().len()) {
                (Some(_), _) => "sample library: Apple Loops and your folders".to_string(),
                (None, 0) => "no sample packs, Apple Loops or sample folders found: every sound is synthesized".to_string(),
                (None, n) => format!(
                    "no Apple Loops or sample folders found: the sounds come from the {n} installed sample packs and the synths"
                ),
            });
            s.bump();
        }
        if settings.ears {
            let a = app.clone();
            std::thread::spawn(move || {
                let t = Instant::now();
                let e = Ears::load(false);
                let msg = match &e {
                    Ok(_) => format!(
                        "ears ready ({:.1} s): verdicts now include earshot's listening",
                        t.elapsed().as_secs_f64()
                    ),
                    Err(err) => format!("earshot is unavailable ({err}); judging with meters and theory only"),
                };
                let _ = a.ears.set(e.ok());
                a.update(|s| s.say(msg));
            });
        } else {
            let _ = app.ears.set(None);
        }
        app
    }

    fn lock(&self) -> MutexGuard<'_, Session> {
        self.s.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn update<T>(&self, f: impl FnOnce(&mut Session) -> T) -> T {
        let mut s = self.lock();
        let out = f(&mut s);
        s.bump();
        let v = s.version;
        drop(s);
        self.tx.send_replace(v);
        out
    }

    /// like `update` when `f` succeeds; when it fails it must leave the session as it was, and there is no new version
    /// (so nobody waiting on the state wakes up for nothing)
    fn try_update<T, E>(&self, f: impl FnOnce(&mut Session) -> Result<T, E>) -> Result<T, E> {
        let mut s = self.lock();
        let out = f(&mut s)?;
        s.bump();
        let v = s.version;
        drop(s);
        self.tx.send_replace(v);
        Ok(out)
    }

    /// an error unless `p` may be read under this session's file policy
    fn readable(&self, p: &Path) -> Result<(), String> {
        config::read_error(p, self.roots.as_deref()).map_or(Ok(()), Err)
    }

    /// where the HTTP server answers (for links to renders)
    pub fn base_url(&self) -> String {
        self.base.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_base_url(&self, url: String) {
        *self.base.lock().unwrap_or_else(|e| e.into_inner()) = url;
    }

    pub fn version(&self) -> u64 {
        self.lock().version
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.tx.subscribe()
    }

    /// the state once its version is past `since` (or after `timeout` with whatever it is then)
    pub async fn state_after(&self, since: Option<u64>, timeout: Duration) -> State {
        if let Some(since) = since {
            let mut rx = self.subscribe();
            let deadline = tokio::time::sleep(timeout);
            tokio::pin!(deadline);
            loop {
                if *rx.borrow_and_update() > since || self.version() > since {
                    break;
                }
                tokio::select! {
                    r = rx.changed() => if r.is_err() { break },
                    _ = &mut deadline => break,
                }
            }
        }
        self.state()
    }

    /// the state once no job runs and the render is of the current song (or after `timeout`)
    pub async fn settled(&self, timeout: Duration) -> State {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut rx = self.subscribe();
        loop {
            rx.borrow_and_update();
            if self.is_settled() {
                break;
            }
            tokio::select! {
                r = rx.changed() => if r.is_err() { break },
                _ = tokio::time::sleep_until(deadline) => break,
            }
        }
        self.state()
    }

    fn is_settled(&self) -> bool {
        let s = self.lock();
        let fresh = s.bp.is_none()
            || s.render.as_ref().is_some_and(|r| r.view.version >= s.song_version)
            || (!self.auto_render.load(Ordering::Relaxed) && s.rerender.is_none());
        !s.job && fresh
    }

    pub fn state(&self) -> State {
        let mut s = self.lock();
        let release = self.release_view(&mut s);
        State {
            version: s.version,
            song: s.view.clone(),
            render: s.render.as_ref().map(|r| r.view.clone()),
            verdict: s.verdict.clone(),
            busy: s.busy.clone(),
            progress: s.progress,
            ears_loading: self.ears.get().is_none(),
            song_version: s.song_version,
            can_undo: !s.undo.is_empty(),
            can_redo: !s.redo.is_empty(),
            undo_label: s.undo.last().map(|u| u.1.clone()),
            redo_label: s.redo.last().map(|r| r.1.clone()),
            log: s.log.iter().cloned().collect(),
            genres: self.genres.clone(),
            exports: self.exports.clone(),
            release,
        }
    }

    pub fn script(&self) -> String {
        self.lock()
            .view
            .as_ref()
            .map(|v| v.script.clone())
            .unwrap_or_else(|| "// no song yet: try song(\"house\")\n".into())
    }

    pub fn blueprint(&self) -> Option<Blueprint> {
        self.lock().bp.clone()
    }

    pub fn sounds(&self) -> SoundList {
        views::sounds()
    }

    pub fn has_ears(&self) -> bool {
        self.ears.get().is_some_and(|e| e.is_some())
    }

    /// waits, up to `timeout`, until the models have loaded or failed to: ONNX Runtime tears down its globals when the
    /// process exits, and a thread still loading them then crashes it
    pub fn wait_for_ears(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while self.ears.get().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// the master of the render of song version `version` (`ext` wav or mp3; the mp3 is encoded on first use)
    pub fn audio_path(&self, version: u64, ext: &str) -> Option<PathBuf> {
        let wav = self.cache.join(format!("{version}.wav"));
        if !wav.exists() {
            return None;
        }
        match ext {
            "wav" => Some(wav),
            "mp3" => {
                let mp3 = self.cache.join(format!("{version}.mp3"));
                if !mp3.exists() {
                    let spec: export::Spec = export::FOLDER_MP3.parse().ok()?;
                    let made = audio::load_once(&wav).and_then(|m| export::write(&m, &spec, &export::tags(""), &mp3));
                    // a browser plays the WAV just as well
                    if made.is_err() {
                        return Some(wav);
                    }
                }
                Some(mp3)
            }
            _ => None,
        }
    }

    /// The render of song version `version` as `spec` (`/api/export`), encoded on request. Only the latest export is
    /// kept, in memory, so a second click on the same download does not encode it again.
    pub fn export_bytes(&self, version: u64, spec: &export::Spec) -> Result<Arc<Vec<u8>>, String> {
        let mut last = self.last_export.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((v, s, bytes)) = last.as_ref()
            && *v == version
            && s == spec
        {
            return Ok(bytes.clone());
        }
        let wav = self.cache.join(format!("{version}.wav"));
        if !wav.exists() {
            return Err(format!("no render of version {version}"));
        }
        let title = self.blueprint().map(|b| b.title).unwrap_or_default();
        let bytes = audio::load_once(&wav)
            .and_then(|m| export::encode(&m, spec, &export::tags(&title)))
            .map(Arc::new)
            .map_err(|e| format!("{e:#}"))?;
        *last = Some((version, *spec, bytes.clone()));
        Ok(bytes)
    }

    pub fn stem_path(&self, version: u64, track: &str) -> Option<PathBuf> {
        let s = self.lock();
        let r = s.render.as_ref().filter(|r| r.view.version == version)?;
        r.stems.get(track).cloned().or_else(|| r.stems.get(&song::slug(track)).cloned())
    }

    /// wait (blocking) until no job runs, at most `timeout`; true when idle
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let t = Instant::now();
        while t.elapsed() < timeout {
            if !self.lock().job {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

/// in each session's render folder, the file whose lock says the session is still running
const IN_USE: &str = ".in-use";
/// a folder whose lock is free waits this long since it last changed, in case its session has only just started
const ENDED_GRACE: Duration = Duration::from_secs(60);
/// folders without a lock file, from claudius before 0.3, go once they have not changed for this long
const UNLOCKED_AGE: Duration = Duration::from_secs(12 * 3600);

/// a lock on the session's render folder for as long as the session runs, however the process ends
fn hold(dir: &Path) -> Option<std::fs::File> {
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(IN_USE)).ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// removes the render folders of sessions that have ended, other than `mine`: each session's renders are only for it,
/// and a day of edits leaves gigabytes of them
fn clean_old_sessions(root: &Path, mine: &Path, grace: Duration, unlocked: Duration) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let dir = e.path();
        if dir == mine {
            continue;
        }
        let unchanged_for = |d: Duration| {
            e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age >= d)
        };
        let ended = match std::fs::File::open(dir.join(IN_USE)) {
            Ok(lock) => lock.try_lock().is_ok() && unchanged_for(grace),
            Err(_) => unchanged_for(unlocked),
        };
        if ended {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// what `State.exports` lists: every preset, with why this machine cannot write it when it cannot
fn export_formats() -> Vec<ExportFormat> {
    export::presets()
        .iter()
        .map(|p| ExportFormat {
            spec: p.to_string(),
            label: p.label(config::SR),
            extension: p.extension().to_string(),
            lossless: p.lossless(),
            unavailable: p.unavailable(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ended_sessions_lose_their_renders_and_running_ones_keep_them() {
        let root = std::env::temp_dir().join(format!("claudius-renders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = |name: &str| {
            let d = root.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("1.wav"), b"RIFF").unwrap();
            d
        };
        let (mine, running, ended, unlocked) = (folder("1"), folder("2"), folder("3"), folder("4"));
        let _mine = hold(&mine).unwrap();
        let _running = hold(&running).unwrap();
        drop(hold(&ended).unwrap());
        clean_old_sessions(&root, &mine, Duration::from_secs(3600), UNLOCKED_AGE);
        assert!(ended.join("1.wav").exists(), "a session that has only just let go may be starting");
        clean_old_sessions(&root, &mine, Duration::ZERO, UNLOCKED_AGE);
        assert!(mine.join("1.wav").exists(), "this session's own");
        assert!(running.join("1.wav").exists(), "a session that still holds its lock");
        assert!(!ended.exists(), "a session whose lock is free");
        assert!(unlocked.join("1.wav").exists(), "an older claudius's, changed within twelve hours");
        std::fs::remove_dir_all(&root).ok();
    }
}
