//! A session that keeps its song in memory and renders it on the spot: no files, no threads of its own and no ears.
//! The browser's demo runs one in a Web Worker, and the app's demo on a thread. `command` makes a command's change and
//! answers it; `render` renders the song as it is now, when `stale` says it is newer than its render, and hands over
//! the master itself.

use std::collections::BTreeMap;
use std::sync::Arc;

use claudius::compose::{self, Options};
use claudius::library::Library;
use claudius::library::prepared::Prepared;
use claudius::model::Blueprint;
use claudius::rng::Rng;
use claudius::theory::scale::Key;
use claudius::{genres, judge, render, script};
use claudius_api::*;

use crate::{edit, views};

/// the undo steps a session keeps
const UNDO: usize = 200;
/// the lines of the log a state carries
const LOG: usize = 200;
/// the bars a preview renders
const PREVIEW_BARS: u32 = 16;

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
            seeds: Rng::new(seed),
        }
    }

    /// Loops found for a genre built on samples, each prepared (its role, tempo, length, key and chords, at a path of
    /// its own), with its credit and its audio (interleaved stereo at 48 kHz): the songs of that genre play them from
    /// now on, along with the ones that came before.
    pub fn add_loops(&mut self, loops: Vec<(Prepared, String, Vec<f32>)>) {
        for (p, credit, samples) in loops {
            claudius::audio::remember(&p.path, claudius::audio::Stereo::from_interleaved(&samples));
            self.say(format!("  {}: {credit}", p.path));
            self.prepared.retain(|q| q.path != p.path);
            self.prepared.push(p);
        }
        self.lib = Some(Arc::new(Library::with_prepared(self.prepared.clone())));
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

    /// whether the song is newer than its render (a preview is not a render of it)
    pub fn stale(&self) -> bool {
        self.bp.is_some() && self.render.as_ref().is_none_or(|r| r.version < self.song_version || r.url == PREVIEW_URL)
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

    pub fn command(&mut self, c: Command) -> Reply {
        match c {
            Command::NewSong { genre, seed, bpm, key, .. } => self.new_song(&genre, seed, bpm, key),
            Command::OpenText { name, text } => self.open_text(&name, &text),
            Command::Undo => self.history(true),
            Command::Redo => self.history(false),
            Command::Render { .. } if self.stale() => self.ok("rendering"),
            Command::Render { .. } => self.ok("the render is up to date"),
            c => self.edit(&c),
        }
    }

    fn new_song(&mut self, genre: &str, seed: Option<u64>, bpm: Option<f64>, key: Option<String>) -> Reply {
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
        let opts = Options { bpm, key, ..Default::default() };
        let lib = self.lib.as_deref().filter(|_| g.sampling.is_some()).map(|l| l as &dyn compose::Library);
        let bp = compose::generate(&g, seed, &opts, lib);
        let msg =
            format!("new song: {} — {} at {:.0} BPM in {} (seed {seed})", bp.title, bp.genre, bp.bpm, bp.key.name());
        self.commit(Some(bp), msg)
    }

    /// a blueprint (`.json`) opened, or a script run as a new song
    fn open_text(&mut self, name: &str, text: &str) -> Reply {
        if !std::path::Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            return self.run_script(text, Some(format!("opened {name}")));
        }
        let bp = Blueprint::from_json(text);
        match bp.and_then(|bp| bp.check_size().and_then(|_| script::check_patterns(&bp)).map(|_| bp)) {
            Ok(bp) => self.commit(Some(bp), format!("opened {name}")),
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
                self.say(format!("render failed: {e}"));
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
    fn a_command_the_demo_does_not_make_says_so() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "trap", "seed": 1}));
        let r = command(&mut h, json!({"cmd": "save"}));
        assert!(!r.ok && r.message.contains("the demo does not save"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "new_song", "genre": "polka"}));
        assert!(!r.ok && r.message.contains("genres: "), "{}", r.message);
    }
}
