//! A session that keeps its song in memory and renders it on the spot: no files, no threads of its own and no ears.
//! The browser's demo runs one in a Web Worker, and the app's demo on a thread. `command` makes a command's change and
//! answers it; `render` renders the song as it is now, when `stale` says it is newer than its render, and hands over
//! the master itself.

use std::collections::BTreeMap;
use std::sync::Arc;

use claudius::audio::Stereo;
use claudius::compose::{self, Options};
use claudius::genres::SampleNeed;
use claudius::library::Library;
use claudius::library::prepared::{Prepared, Role};
use claudius::model::Blueprint;
use claudius::rng::Rng;
use claudius::theory::scale::Key;
use claudius::{audio, genres, judge, pads, render, sampling, scratch, script, take};
use claudius_api::*;
use claudius_packs::online::Found;

use crate::{edit, views};

/// the undo steps a session keeps
const UNDO: usize = 200;
/// the lines of the log a state carries
const LOG: usize = 200;
/// the bars a preview renders
const PREVIEW_BARS: u32 = 16;

/// A sound a phone playing along asks for: every drum lane's hit, notes of a sound for its keys, or a record of bars of
/// a track to scratch.
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SoundAsk {
    Hits,
    Keys { sound: String, low: i32, high: i32, step: usize },
    Record { track: String, bar: u32, bars: u32 },
}

/// A recording the page fetched for a song built on samples, as a library offers it: what `Host::sample` cuts it into
/// (`role`), for songs of `genre` or for the words `asked`, and the kind of file it is (`mp3`, `ogg`).
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Recording {
    pub role: Role,
    pub genre: String,
    #[serde(default)]
    pub asked: Option<String>,
    pub found: Found,
    pub ext: String,
}

/// A render: what the API shows of it, and the master, interleaved stereo at 48 kHz.
pub struct Audio {
    pub view: RenderView,
    pub samples: Vec<f32>,
}

pub struct Host {
    bp: Option<Blueprint>,
    undo: Vec<(Option<Blueprint>, String)>,
    redo: Vec<(Option<Blueprint>, String)>,
    version: u64,
    song_version: u64,
    log: Vec<String>,
    loudness: BTreeMap<String, f64>,
    render: Option<RenderView>,
    verdict: Option<VerdictView>,
    /// the loops a genre built on samples plays: the ones the page found for it, and the library of them
    prepared: Vec<Prepared>,
    lib: Option<Arc<Library>>,
    /// the song version whose render failed: it is not tried again until the song changes
    failed: Option<u64>,
    seeds: Rng,
}

impl Host {
    /// a session with no song yet; `seed` seeds the songs asked for without one
    pub fn new(seed: u64) -> Host {
        Host {
            bp: None,
            undo: Vec::new(),
            redo: Vec::new(),
            version: 0,
            song_version: 0,
            log: Vec::new(),
            loudness: BTreeMap::new(),
            render: None,
            verdict: None,
            prepared: Vec::new(),
            lib: None,
            failed: None,
            seeds: Rng::new(seed),
        }
    }

    /// Cuts a recording the page fetched (`bytes`, a file of its `ext`) as claudius cuts one (`sampling::cut`), keeps
    /// what it cut in memory, and lists it for the songs after it, along with what came before; what it cut (its role,
    /// tempo, length, key and chords, at a path of its own), or why nothing.
    pub fn sample(&mut self, r: Recording, bytes: Vec<u8>) -> Result<Prepared, String> {
        let g = genres::get(&r.genre).map_err(|e| e.to_string())?;
        let n = sampling::need(&g, r.role).ok_or_else(|| format!("{} samples no {}", g.name, r.role.name()))?;
        let asked = r.asked.as_deref().and_then(sampling::words).filter(|_| r.role == Role::Phrase);
        // without the ears a phrase is the loudest of the recording's harmonic bars, as it is in the app without them
        let n = SampleNeed { listen: asked.clone().unwrap_or_else(|| n.listen.clone()), ..n.clone() };
        let x = audio::load_bytes(bytes, &r.ext).map_err(|e| format!("cannot read it: {e:#}"))?;
        let name = format!("{}-{}", claudius::song::slug(&r.found.title), claudius::song::slug(&r.found.id));
        let dir = std::path::PathBuf::from(format!(
            "demo:samples/{}/{}/{name}",
            r.role.name(),
            sampling::whose(&g, asked.as_deref())
        ));
        let cut = sampling::cut(r.role, &x, &r.found, &g, &n, None, &dir).map_err(|e| format!("{e:#}"))?;
        if !sampling::plays_in(&cut.prepared, &g) {
            return Err(format!("{} plays nothing at {:.0} BPM as it was played", g.name, cut.prepared.bpm));
        }
        for f in cut.files {
            let mut a = f.audio;
            if !f.float {
                // as the 24-bit file it would be written to keeps it
                for v in a.l.iter_mut().chain(a.r.iter_mut()) {
                    *v = v.clamp(-1.0, 1.0);
                }
            }
            audio::remember(&f.path.to_string_lossy(), a);
        }
        let mut p = cut.prepared;
        p.asked = asked;
        self.say(format!("  {}: {}", p.path, r.found.credit()));
        self.prepared.retain(|q| q.path != p.path);
        self.prepared.push(p.clone());
        self.lib = Some(Arc::new(Library::with_prepared(self.prepared.clone())));
        Ok(p)
    }

    /// the script language's reference, as the studio shows it
    pub fn reference() -> &'static str {
        script::REFERENCE
    }

    /// the sounds a part or a lane can play here: the synth's presets and drums (no packs are installed)
    pub fn sounds() -> SoundList {
        views::sounds()
    }

    pub fn state(&self) -> State {
        State {
            version: self.version,
            song_version: self.song_version,
            song: self.bp.as_ref().map(|b| views::song(b, &self.loudness)),
            render: self.render.clone(),
            verdict: self.verdict.clone(),
            busy: self.stale().then(|| "rendering".to_string()),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            undo_label: self.undo.last().map(|(_, l)| l.clone()),
            redo_label: self.redo.last().map(|(_, l)| l.clone()),
            log: self.log.clone(),
            genres: views::genre_infos(),
            ..Default::default()
        }
    }

    /// whether the song is newer than its render (a preview is not a render of it), unless rendering it failed
    pub fn stale(&self) -> bool {
        self.bp.is_some()
            && self.failed != Some(self.song_version)
            && self.render.as_ref().is_none_or(|r| r.version < self.song_version || r.url == PREVIEW_URL)
    }

    /// Renders the first bars of a song newer than its render, to play while the whole of it renders, and hands them
    /// over; None when there is nothing to preview: no song, a render of it already, a preview of it already, or a
    /// song no longer than a preview.
    pub fn preview(&mut self) -> Option<Audio> {
        let bp = self.bp.as_ref()?;
        if !self.stale() || self.render.as_ref().is_some_and(|r| r.url == PREVIEW_URL && r.version == self.song_version)
        {
            return None;
        }
        let project = compose::arrange(bp);
        if project.bars <= PREVIEW_BARS {
            return None;
        }
        let r = render::render(&project.slice(0, PREVIEW_BARS), &render::Options::default()).ok()?;
        let spb = 60.0 / bp.bpm;
        let view = RenderView {
            version: self.song_version,
            url: PREVIEW_URL.into(),
            seconds: r.master.secs(),
            lufs: r.lufs,
            true_peak_db: r.true_peak_db,
            bpm: bp.bpm,
            sections: project
                .sections
                .iter()
                .filter(|s| s.start_bar < PREVIEW_BARS)
                .map(|s| {
                    (
                        s.name.clone(),
                        s.start_bar as f64 * 4.0 * spb,
                        (s.start_bar + s.bars).min(PREVIEW_BARS) as f64 * 4.0 * spb,
                    )
                })
                .collect(),
            waveform: views::waveform(&r, 1000),
            stems: Vec::new(),
        };
        self.say(format!("the first {PREVIEW_BARS} bars, to play while the whole song renders"));
        self.render = Some(view.clone());
        self.version += 1;
        Some(Audio { view, samples: r.master.interleaved() })
    }

    /// the song as it is now
    pub fn blueprint(&self) -> Option<&Blueprint> {
        self.bp.as_ref()
    }

    /// a sound of the song for a phone playing along, named: each lane's hit by the lane, the others by what they are
    pub fn sound(&self, ask: &SoundAsk) -> Result<Vec<(String, Stereo)>, String> {
        let Some(bp) = &self.bp else { return Err("no song yet".into()) };
        match ask {
            SoundAsk::Hits => pads::hits(bp),
            SoundAsk::Keys { sound, low, high, step } => {
                Ok(vec![(sound.clone(), pads::keys(bp, sound, *low, *high, *step)?)])
            }
            SoundAsk::Record { track, bar, bars } => {
                Ok(vec![(track.clone(), scratch::record(bp, track, *bar, *bars)?)])
            }
        }
    }

    pub fn command(&mut self, c: Command) -> Reply {
        match c {
            Command::NewSong { genre, seed, bpm, key, sampled, sample_search, .. } => {
                let sample_search = sample_search.as_deref().and_then(claudius::sampling::words);
                let opts = Options { bpm, sampled, sample_search, ..Default::default() };
                self.new_song(&genre, seed, key, opts)
            }
            Command::OpenText { name, text } => self.open_text(&name, &text),
            Command::Undo => self.history(true),
            Command::Redo => self.history(false),
            Command::Render { .. } if self.stale() => self.ok("rendering"),
            Command::Render { .. } => self.ok("the render is up to date"),
            Command::AddScratch { from, track, bar, bars, moves, name, role } => {
                self.add_scratch(&from, &edit::scratch(track, bar, bars, &moves), name, role.as_deref())
            }
            c => self.edit(&c),
        }
    }

    /// a scratch kept as a take, its audio in memory under a name of its own
    fn add_scratch(&mut self, from: &str, s: &scratch::Take, name: Option<String>, role: Option<&str>) -> Reply {
        let Some(mut bp) = self.bp.clone() else {
            return self.no("no song yet: start one with a new song, then scratch one of its tracks");
        };
        let x = match scratch::take(&bp, s) {
            Ok(x) => x,
            Err(e) => return self.no(e),
        };
        let name = name.unwrap_or_else(|| scratch::name(&bp));
        let path = format!("take:{name}:{}", self.version);
        let secs = x.secs();
        audio::remember(&path, x);
        match take::add(&mut bp, std::path::Path::new(&path), secs, from, Some(&name), role) {
            Ok(msg) => self.commit(Some(bp), msg),
            Err(e) => self.no(e),
        }
    }

    /// a new song of `genre` made with `opts`, in `key` when one is given
    fn new_song(&mut self, genre: &str, seed: Option<u64>, key: Option<String>, opts: Options) -> Reply {
        let g = match genres::get(genre) {
            Ok(g) => g,
            Err(e) => {
                let names: Vec<String> = views::genre_infos().into_iter().map(|g| g.name).collect();
                return self.no(format!("{e}; genres: {}", names.join(", ")));
            }
        };
        let key = match key.map(|k| Key::parse(&k).ok_or(k)).transpose() {
            Ok(k) => k,
            Err(k) => return self.no(format!("cannot read key '{k}' (try \"A minor\", \"F#m\", \"Eb\")")),
        };
        let seed = seed.unwrap_or_else(|| self.seed());
        let opts = Options { key, ..opts };
        let sampled = claudius::sampling::sampled(&g, opts.sampled, opts.sample_search.as_deref());
        let lib = self.lib.as_deref().filter(|_| sampled).map(|l| l as &dyn compose::Library);
        let bp = compose::generate(&g, seed, &opts, lib);
        let msg =
            format!("new song: {} — {} at {:.0} BPM in {} (seed {seed})", bp.title, bp.genre, bp.bpm, bp.key.name());
        self.forget_render();
        self.commit(Some(bp), msg)
    }

    /// a blueprint (`.json`) opened, or a script run as a new song
    fn open_text(&mut self, name: &str, text: &str) -> Reply {
        if !std::path::Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            let before = self.song_version;
            let r = self.run_script(text, Some(format!("opened {name}")));
            if self.song_version != before {
                self.forget_render();
            }
            return r;
        }
        let bp = Blueprint::from_json(text);
        match bp.and_then(|bp| bp.check_size().and_then(|_| script::check_patterns(&bp)).map(|_| bp)) {
            Ok(bp) => {
                self.forget_render();
                self.commit(Some(bp), format!("opened {name}"))
            }
            Err(e) => self.no(format!("cannot open {name}: {e}")),
        }
    }

    fn edit(&mut self, c: &Command) -> Reply {
        let seed = self.seed();
        match edit::script(c, self.bp.as_ref(), seed) {
            Some(Ok(s)) => return self.run_script(&s.code, s.label),
            Some(Err(e)) => return self.no(e),
            None => {}
        }
        let Some(mut bp) = self.bp.clone() else {
            return self.no("no song yet: start one with a new song");
        };
        match edit::change(c, &mut bp) {
            None => {
                self.no(format!("the demo does not {}: start `claudius serve` for that", name(c).replace('_', " ")))
            }
            Some(Err(e)) => self.no(e),
            Some(Ok(msg)) => match bp.check_size() {
                Err(e) => self.no(e),
                Ok(()) if self.bp.as_ref() == Some(&bp) => self.ok("nothing changed"),
                Ok(()) => self.commit(Some(bp), msg),
            },
        }
    }

    /// a script run on the song (or on none: the script starts one)
    fn run_script(&mut self, code: &str, what: Option<String>) -> Reply {
        match script::run(code, self.bp.clone(), self.lib.clone()) {
            Ok(out) if out.bp == self.bp => {
                let log = out.log.join("\n");
                let said = if log.is_empty() { "ran; nothing changed".to_string() } else { log };
                self.say(said.clone());
                self.version += 1;
                Reply { ok: true, message: said, version: self.version }
            }
            Ok(out) => {
                let mut bp = out.bp;
                let label = what.unwrap_or_else(|| {
                    let first = code.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("//"));
                    let more = if code.lines().count() > 1 { " …" } else { "" };
                    format!("script: {}{more}", first.unwrap_or("").chars().take(80).collect::<String>())
                });
                if let Some(b) = bp.as_mut() {
                    b.lineage.push(label.clone());
                }
                let log = out.log.join("\n");
                let msg = if log.is_empty() { label.clone() } else { log };
                self.commit_as(bp, msg, label)
            }
            Err(e) => {
                self.say(format!("script error: {e}"));
                self.version += 1;
                Reply { ok: false, message: e, version: self.version }
            }
        }
    }

    /// another song takes the place of this one: its render, verdict and meters are not that song's
    fn forget_render(&mut self) {
        (self.render, self.verdict) = (None, None);
        self.loudness.clear();
    }

    fn commit(&mut self, bp: Option<Blueprint>, msg: String) -> Reply {
        let label = msg.lines().next().unwrap_or("").to_string();
        self.commit_as(bp, msg, label)
    }

    fn commit_as(&mut self, bp: Option<Blueprint>, msg: String, label: String) -> Reply {
        let old = std::mem::replace(&mut self.bp, bp);
        self.undo.push((old, label.chars().take(120).collect()));
        if self.undo.len() > UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.changed(msg)
    }

    fn history(&mut self, undo: bool) -> Reply {
        let Some((prev, label)) = (if undo { self.undo.pop() } else { self.redo.pop() }) else {
            return self.no(if undo { "nothing to undo" } else { "nothing to redo" });
        };
        let cur = std::mem::replace(&mut self.bp, prev);
        if undo {
            self.redo.push((cur, label.clone()));
        } else {
            self.undo.push((cur, label.clone()));
        }
        self.changed(format!("{}: {label}", if undo { "undid" } else { "redid" }))
    }

    /// the song changed: a new version, and a render to come
    fn changed(&mut self, msg: String) -> Reply {
        self.say(msg.clone());
        self.version += 1;
        self.song_version = self.version;
        Reply { ok: true, message: msg, version: self.version }
    }

    /// Renders the song as it is now and judges it, with the meters and the theory (the ears are not here), and hands
    /// over the master; None without a song.
    pub fn render(&mut self) -> Option<Result<Audio, String>> {
        let bp = self.bp.clone()?;
        let version = self.song_version;
        let project = compose::arrange(&bp);
        let r = match render::render(&project, &render::Options::default()) {
            Ok(r) => r,
            Err(e) => {
                self.say(format!("render failed: {e}; change the song to try again"));
                self.failed = Some(self.song_version);
                self.version += 1;
                return Some(Err(e.to_string()));
            }
        };
        let verdict = views::verdict(&judge::judge(&project, &r, None));
        let spb = 60.0 / bp.bpm;
        let view = RenderView {
            version,
            url: String::new(),
            seconds: r.master.secs(),
            lufs: r.lufs,
            true_peak_db: r.true_peak_db,
            bpm: bp.bpm,
            sections: project
                .sections
                .iter()
                .map(|s| (s.name.clone(), s.start_bar as f64 * 4.0 * spb, (s.start_bar + s.bars) as f64 * 4.0 * spb))
                .collect(),
            waveform: views::waveform(&r, 1000),
            stems: Vec::new(),
        };
        // each track as it sits in the master: its own level, its gain staging, then the master's gain
        let master_gain = if r.premaster_lufs.is_finite() { r.lufs - r.premaster_lufs } else { 0.0 };
        self.loudness = r
            .tracks
            .iter()
            .filter_map(|t| {
                t.level_lufs.map(|l| (t.name.clone(), ((l + t.gain_db as f64 + master_gain) * 10.0).round() / 10.0))
            })
            .collect();
        self.say(format!(
            "rendered: {:.1} LUFS, true peak {:.1} dBTP; verdict {:.0}/100",
            r.lufs, r.true_peak_db, verdict.total
        ));
        self.render = Some(view.clone());
        self.verdict = Some(verdict);
        self.version += 1;
        Some(Ok(Audio { view, samples: r.master.interleaved() }))
    }

    fn seed(&mut self) -> u64 {
        1 + self.seeds.next_u64() % 1_000_000
    }

    fn say(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > LOG {
            self.log.remove(0);
        }
    }

    fn ok(&mut self, msg: &str) -> Reply {
        Reply { ok: true, message: msg.to_string(), version: self.version }
    }

    fn no(&mut self, msg: impl Into<String>) -> Reply {
        Reply { ok: false, message: msg.into(), version: self.version }
    }
}

/// a command's name, as the API tags it (`set_tempo`)
fn name(c: &Command) -> String {
    serde_json::to_value(c).ok().and_then(|v| v["cmd"].as_str().map(String::from)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn command(h: &mut Host, v: serde_json::Value) -> Reply {
        h.command(serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}")))
    }

    #[test]
    fn a_song_is_made_edited_undone_and_rendered_as_the_server_makes_one() {
        let mut h = Host::new(1);
        assert!(!h.stale() && h.state().song.is_none());
        let r = command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3}));
        assert!(r.ok, "{}", r.message);
        let song = h.state().song.unwrap();
        assert!(song.tracks.len() >= 8, "a whole arrangement, not {} tracks", song.tracks.len());
        assert!(h.stale(), "a new song is to be rendered");
        let r = command(&mut h, json!({"cmd": "set_tempo", "bpm": 100}));
        assert!(r.ok, "{}", r.message);
        let s = h.state();
        assert_eq!((s.song.unwrap().bpm, s.undo_label.as_deref()), (100.0, Some("tempo 100")));
        assert!(command(&mut h, json!({"cmd": "undo"})).ok);
        assert_eq!(h.state().song.unwrap().bpm, song.bpm);
        let audio = h.render().unwrap().unwrap();
        assert!(!h.stale());
        let s = h.state();
        assert_eq!(s.render.as_ref().map(|r| r.version), Some(s.song_version));
        let frames = audio.samples.len() / 2;
        assert!((frames as f64 / 48_000.0 - audio.view.seconds).abs() < 0.01, "{frames} frames");
        assert!((audio.view.lufs + 10.0).abs() < 2.5, "a mastered house song, not {:.1} LUFS", audio.view.lufs);
        assert!(s.verdict.is_some_and(|v| v.total > 0.0));
        assert!(s.song.unwrap().tracks.iter().any(|t| t.loudness.is_some()), "the meters read the render");
    }

    #[test]
    fn drums_played_on_pads_and_a_scratch_join_the_song_in_memory() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3}));
        let from = h.state().song.unwrap().sections[0].name.clone();
        let hits = json!([{"lane": "kick", "t": 0}, {"lane": "kick", "t": 2.5, "v": 0.6}]);
        let r = command(&mut h, json!({"cmd": "add_hits", "from": from, "hits": hits, "bars": 1}));
        assert!(r.ok, "{}", r.message);
        let moves = json!([{"t": 0, "at": 0}, {"t": 0.3, "at": 0.4}, {"t": 0.5, "at": 0.1}]);
        let r = command(&mut h, json!({"cmd": "add_scratch", "from": from, "track": "kick", "moves": moves}));
        assert!(r.ok, "{}", r.message);
        let take = h.state().song.unwrap().tracks.into_iter().find(|t| t.name == "scratch1").expect("a take");
        assert_eq!(take.role, "lead");
        // the take's audio is kept in memory, so the song renders with it
        let audio = h.render().unwrap().unwrap();
        assert!(audio.samples.iter().any(|s| s.abs() > 0.01));
        assert!(command(&mut h, json!({"cmd": "undo"})).ok);
        assert!(h.state().song.unwrap().tracks.iter().all(|t| t.name != "scratch1"));
    }

    #[test]
    fn a_phone_gets_the_songs_sounds_and_plays_keys_into_it() {
        let mut h = Host::new(1);
        assert!(h.sound(&SoundAsk::Hits).is_err(), "no song yet");
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3}));
        let hits = h.sound(&SoundAsk::Hits).unwrap();
        assert_eq!(hits.len(), h.blueprint().unwrap().drums.len());
        let keys = SoundAsk::Keys { sound: "epiano".into(), low: 59, high: 65, step: 3 };
        assert!((h.sound(&keys).unwrap()[0].1.secs() - 3.0 * pads::SLOT_SECS).abs() < 1e-6);
        let ask: SoundAsk =
            serde_json::from_value(json!({"kind": "record", "track": "kick", "bar": 0, "bars": 1})).unwrap();
        assert!(h.sound(&ask).unwrap()[0].1.secs() > 0.0);
        let from = h.state().song.unwrap().sections[0].name.clone();
        let notes = json!([{"t": 0, "d": 1, "p": 60}, {"t": 1, "d": 1, "p": 64}]);
        let r = command(&mut h, json!({"cmd": "add_notes", "from": from, "notes": notes, "sounds": ["epiano"]}));
        assert!(r.ok, "{}", r.message);
        assert!(h.state().song.unwrap().tracks.iter().any(|t| t.name == "played"));
    }

    #[test]
    fn a_long_song_plays_its_first_bars_while_the_rest_renders() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3}));
        let preview = h.preview().expect("a preview of a long song");
        let bar = 240.0 / preview.view.bpm;
        assert_eq!(preview.view.url, PREVIEW_URL);
        assert!(
            (preview.view.seconds - 16.0 * bar).abs() < 4.0,
            "16 bars and a tail, not {:.1} s",
            preview.view.seconds
        );
        assert!(h.stale(), "a preview is not a render of the song");
        assert!(h.preview().is_none(), "one preview a version");
        let whole = h.render().unwrap().unwrap();
        assert!(whole.view.url.is_empty() && whole.view.seconds > preview.view.seconds * 3.0);
        assert!(!h.stale() && h.preview().is_none());
    }

    #[test]
    fn a_new_song_forgets_the_render_of_the_one_before() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "trap", "seed": 1, "bpm": 140}));
        h.render().unwrap().unwrap();
        assert!(h.state().render.is_some() && h.state().verdict.is_some());
        command(&mut h, json!({"cmd": "set_swing", "swing": 0.55}));
        assert!(h.state().render.is_some(), "an edit keeps the render until the next one lands, as the server does");
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 2}));
        let s = h.state();
        assert!(s.render.is_none() && s.verdict.is_none(), "another song's render is no render of this one");
        assert!(s.song.unwrap().tracks.iter().all(|t| t.loudness.is_none()), "nor are its meters");
    }

    #[test]
    fn a_render_that_failed_is_not_tried_again_until_the_song_changes() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 2}));
        // longer than the renderer takes (the edits would refuse it)
        h.bp.as_mut().unwrap().form[0].bars = 100_000;
        assert!(h.preview().is_some(), "its first bars still play");
        assert!(h.render().unwrap().is_err());
        assert!(!h.stale() && h.preview().is_none(), "not tried again, over and over");
        assert!(h.state().log.iter().any(|l| l.contains("render failed")));
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3}));
        assert!(h.stale(), "another song renders");
    }

    #[test]
    fn a_command_the_demo_does_not_make_says_so() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "trap", "seed": 1}));
        let r = command(&mut h, json!({"cmd": "save"}));
        assert!(!r.ok && r.message.contains("the demo does not save"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "new_song", "genre": "polka"}));
        assert!(!r.ok && r.message.contains("genres: "), "{}", r.message);
    }
}
