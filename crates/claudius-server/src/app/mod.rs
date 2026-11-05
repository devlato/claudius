//! The session: one song with undo and redo, the latest render and verdict, a log, and one long job (render,
//! mutate, evolve, save) at a time. Every change bumps `version` and wakes the long-polling clients, so the UI, the
//! MCP tools and the HTTP API all see the same song live.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use claudius::ears::Ears;
use claudius::library::Library;
use claudius::model::*;
use claudius::{audio, config, export, song};
use claudius_api::*;

use claudius_session::views;

mod commands;
mod jobs;
mod phone;
mod release;
mod takes;

pub(crate) use phone::{Running, Take};

pub(crate) use commands::sampling;
pub use commands::{MAX_PROJECT, analysis_text};
pub use release::{MAX_ARTWORK, report_lines};
pub use takes::MAX_TAKE;

pub struct Settings {
    /// read the Apple Loops / user library for recorded sounds
    pub library: bool,
    /// load earshot's models in the background and use them in every verdict
    pub ears: bool,
    /// render after every change
    pub auto_render: bool,
    /// the folders commands and scripts may read and write (None: anywhere, for a claudius only you can reach)
    pub file_roots: Option<Vec<PathBuf>>,
    /// the file the person's voice is kept in, as a pitch check heard it (None: for this session only)
    pub voice: Option<PathBuf>,
    /// play the samples gathered for the genres built on them (they are neither Apple Loops nor the user library)
    pub samples: bool,
    /// gather what such a genre has none of, from free online libraries, before the first song of it is made (a
    /// shared claudius, one with `file_roots`, gathers nothing)
    pub gather: bool,
    /// where songs are kept: a song saved without a folder of its own, and a project opened (None: ~/Music/claudius)
    pub songs: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library: true,
            ears: true,
            auto_render: true,
            file_roots: None,
            voice: None,
            samples: true,
            gather: true,
            songs: None,
        }
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
    /// the last take asked of the person, and how many have been asked for
    take_request: Option<claudius_api::TakeRequest>,
    asked: u64,
    /// the last pitch check asked of the person: its phrases, and the check as the studio sees it
    pitch_check: Option<(claudius::voice::check::Check, claudius_api::PitchCheck)>,
    /// the person's voice, as their last pitch check heard it
    voice: Option<claudius::voice::singer::Voice>,
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
    /// what a genre built on samples is made with: the library, or without Apple Loops and sample folders, the
    /// prepared samples alone (so that every other genre's songs are made as they were)
    samples_lib: Option<Arc<Library>>,
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
    /// where songs are kept (Settings::songs)
    songs: PathBuf,
    /// where the person's voice is kept
    voice_file: Option<PathBuf>,
    /// whether a genre built on samples gathers what it has none of (Settings::gather)
    gather: bool,
    /// how many new songs have been asked for: a song whose samples were still being gathered when a newer one was
    /// asked for is not made
    new_songs: AtomicU64,
    /// what the libraries had nothing more of this session (`sampling_key`), which new songs do not search for again
    dug_out: Mutex<std::collections::HashSet<String>>,
    /// the studio's transport, and when it said so
    transport: Mutex<(Transport, Instant)>,
    /// the page a phone plays along on, while it is served
    phone: Mutex<Option<phone::Running>>,
    /// each track as the last render made it, so that the next renders only the tracks an edit changed
    kept: Mutex<claudius::render::Kept>,
}

/// how many sessions this process has started
static SESSIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// a session's renders are only for it: they go when it does (its lock first, which Windows wants closed)
impl Drop for App {
    fn drop(&mut self) {
        drop(self._in_use.take());
        let _ = std::fs::remove_dir_all(&self.cache);
    }
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
        // each session's own folder, though a process runs several (tests do): starting one clears its folder
        let n = SESSIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let cache = config::cache_dir("renders").join(format!("{}-{n}", std::process::id()));
        // a folder left by an ended process with the same id holds none of this session's renders
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::create_dir_all(&cache);
        let in_use = hold(&cache);
        clean_old_sessions(cache.parent().unwrap_or(&cache), &cache, ENDED_GRACE, UNLOCKED_AGE);
        let lib = if settings.library { Library::open().ok().filter(|l| !l.is_empty()).map(Arc::new) } else { None };
        let samples_lib = settings.samples.then(|| lib.clone().unwrap_or_else(|| Arc::new(Library::prepared_only())));
        let (tx, _) = tokio::sync::watch::channel(0);
        let app = Arc::new(App {
            s: Mutex::new(Session::default()),
            edit: Mutex::new(()),
            tx,
            lib,
            samples_lib,
            ears: Arc::new(OnceLock::new()),
            cancel: Arc::new(AtomicBool::new(false)),
            auto_render: AtomicBool::new(settings.auto_render),
            cache,
            _in_use: in_use,
            genres: views::genre_infos(),
            exports: claudius_session::views::export_formats(),
            last_export: Mutex::new(None),
            base: Mutex::new(format!("http://127.0.0.1:{DEFAULT_PORT}")),
            gather: settings.gather && settings.file_roots.is_none(),
            new_songs: AtomicU64::new(0),
            dug_out: Mutex::new(Default::default()),
            roots: settings.file_roots,
            songs: settings.songs.unwrap_or_else(config::songs_dir),
            voice_file: settings.voice.clone(),
            transport: Mutex::new((Transport::default(), Instant::now())),
            phone: Mutex::new(None),
            kept: Mutex::new(Default::default()),
        });
        {
            let mut s = app.lock();
            s.voice = settings
                .voice
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|t| serde_json::from_str(&t).ok());
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
        let phone = self.phone_view();
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
            take_request: s.take_request.clone(),
            pitch_check: s.pitch_check.as_ref().map(|(_, v)| v.clone()),
            voice: s.voice.as_ref().map(|v| v.describe()),
            phone,
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

#[cfg(test)]
mod tests {
    use super::*;
    use claudius::genres;
    use claudius::library::prepared::{Prepared, Role};

    fn app(samples: Option<Vec<Prepared>>, shared: bool) -> Arc<App> {
        let mut app = App::new(Settings {
            library: false,
            ears: false,
            auto_render: false,
            file_roots: shared.then(|| vec![std::env::temp_dir()]),
            voice: None,
            samples: true,
            gather: true,
            songs: None,
        });
        Arc::get_mut(&mut app).expect("no one else holds it yet").samples_lib =
            samples.map(|p| Arc::new(Library::with_prepared(p)));
        app
    }

    fn sample(role: Role, path: &str) -> Prepared {
        Prepared {
            path: path.into(),
            role,
            genre: "trip_hop".into(),
            bpm: 90.0,
            beats: 8.0,
            key: Some(2),
            minor: true,
            kick: None,
            snare: None,
            hat: None,
            chords: vec!["i".into(), "VI".into()],
            z: Some(3.0),
            source: format!("freesound:{path}"),
            asked: None,
            second: None,
            root: None,
            pulse: None,
        }
    }

    /// as many samples of each role as trip-hop keeps
    fn shelf() -> Vec<Prepared> {
        let g = genres::get("trip_hop").unwrap();
        Role::ALL
            .into_iter()
            .flat_map(|r| {
                let keep = claudius::sampling::need(&g, r).unwrap().keep;
                (0..keep).map(move |i| sample(r, &format!("/{}{i}.wav", r.name())))
            })
            .collect()
    }

    #[test]
    fn a_song_built_on_samples_gathers_what_it_has_too_few_of() {
        let g = genres::get("trip_hop").unwrap();
        let wanted =
            |app: &Arc<App>, g: &genres::GenreProfile, opts: &claudius::compose::Options| app.samples_wanted(g, opts);
        let plain = claudius::compose::Options::default();
        assert_eq!(
            wanted(&app(Some(Vec::new()), false), &g, &plain).as_deref(),
            Some("a break, a phrase, a texture, a vocal and a bass note")
        );
        let some = vec![sample(Role::Break, "/b.wav")];
        assert_eq!(
            wanted(&app(Some(some), false), &g, &plain).as_deref(),
            Some("another break, a phrase, a texture, a vocal and a bass note"),
            "each new song brings a few more, until the genre has as many as it keeps"
        );
        assert_eq!(wanted(&app(Some(shelf()), false), &g, &plain), None);
        assert_eq!(wanted(&app(None, false), &g, &plain), None, "without the library: no samples at all");
        // house is sampled when asked, for the words asked
        let house = genres::get("house").unwrap();
        assert_eq!(wanted(&app(Some(Vec::new()), false), &house, &plain), None);
        let asked = claudius::compose::Options { sample_search: Some("70s soul".into()), ..Default::default() };
        assert_eq!(
            wanted(&app(Some(Vec::new()), false), &house, &asked).as_deref(),
            Some("a break, a phrase for \"70s soul\" and a vocal")
        );
        let shared = app(Some(Vec::new()), true);
        assert_eq!(wanted(&shared, &g, &plain), None, "a shared claudius gathers nothing");
        assert!(shared.state().log.iter().any(|l| l.contains("claudius samples gather trip_hop")));
    }

    #[test]
    fn a_role_the_libraries_had_nothing_more_of_is_not_searched_again_this_session() {
        let app = app(Some(Vec::new()), false);
        let g = genres::get("trip_hop").unwrap();
        app.dug_out.lock().unwrap().insert(jobs::sampling_key(&g, Role::Texture, None));
        let plain = claudius::compose::Options::default();
        assert_eq!(app.samples_wanted(&g, &plain).as_deref(), Some("a break, a phrase, a vocal and a bass note"));
    }

    #[test]
    fn with_its_samples_a_new_song_of_a_genre_built_on_them_is_made_at_once() {
        let app = app(Some(shelf()), false);
        let r = app.command(Command::NewSong {
            genre: "trip_hop".into(),
            seed: Some(3),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        assert!(r.ok && r.message.starts_with("new song"), "{}", r.message);
        let bp = app.blueprint().unwrap();
        assert_eq!((bp.bpm, bp.key.name()), (90.0, "D minor".to_string()), "the phrase's tempo and key");
        assert!(bp.loops.iter().any(|l| l.name == "sample"), "the phrase plays");
    }

    #[test]
    fn a_song_whose_samples_came_after_a_newer_one_was_asked_for_is_not_made() {
        let app = app(Some(Vec::new()), false);
        app.new_songs.store(2, Ordering::Relaxed);
        // stopped at once, so that no library is asked
        app.cancel.store(true, Ordering::Relaxed);
        app.sampled_song_job(genres::get("trip_hop").unwrap(), 3, claudius::compose::Options::default(), 1);
        assert!(app.blueprint().is_none(), "the newer song stays");
        assert!(app.state().log.iter().any(|l| l.contains("a newer song was asked for")), "{:?}", app.state().log);
    }

    #[test]
    fn a_new_song_turned_away_as_busy_is_no_newer_song() {
        let app = app(Some(Vec::new()), false);
        app.lock().job = true;
        let r = app.command(Command::NewSong {
            genre: "trip_hop".into(),
            seed: Some(3),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        assert!(!r.ok && r.message.contains("busy"), "{}", r.message);
        assert_eq!(app.new_songs.load(Ordering::Relaxed), 0, "the job already running stays the newest");
    }

    #[test]
    fn a_render_asked_for_while_a_new_song_is_made_waits_for_it() {
        let app = app(None, false);
        assert!(!app.command(Command::Render { stems: false }).ok, "no song yet");
        app.lock().job = true;
        let r = app.command(Command::Render { stems: false });
        assert!(r.ok && r.message.contains("after the job"), "{}", r.message);
        assert_eq!(app.lock().rerender, Some(false));
    }

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
