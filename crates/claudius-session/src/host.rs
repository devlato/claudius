//! A session that keeps its song in memory and renders it on the spot: no files, no threads of its own and no ears.
//! The browser's demo runs one in a Web Worker, and the app's demo on a thread. `command` makes a command's change and
//! answers it; `render` renders the song as it is now, when `stale` says it is newer than its render, and hands over
//! the master itself.

use std::collections::BTreeMap;
use std::sync::Arc;

use claudius::audio::Stereo;
use claudius::compose::{self, Options};
use claudius::evolve::Settings;
use claudius::genres::SampleNeed;
use claudius::library::Library;
use claudius::library::prepared::{Prepared, Role};
use claudius::model::{self, Blueprint};
use claudius::rng::Rng;
use claudius::theory::scale::Key;
use claudius::voice::check;
use claudius::{audio, genres, judge, pads, render, sampling, scratch, script, take};
use claudius_api::*;
use claudius_packs::online::Found;

use crate::history::{History, OpId};
use crate::{edit, views};

/// the lines of the log a state carries
const LOG: usize = 200;
/// the bars a preview renders
const PREVIEW_BARS: u32 = 16;
/// the seconds from the playhead an edit to a song heard whole is previewed for
const WINDOW: f64 = 12.0;

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

/// What a song's next render would make anew (`Host::to_make`): the song as the renderer reads it, the tracks to make,
/// each with the key it is kept under and about what it costs (`render::track_cost`), the costliest first, and the
/// audio handed over that they play, by its path.
pub struct ToMake {
    pub project: model::Project,
    pub tracks: Vec<(String, render::TrackKey, f64)>,
    pub audio: Vec<(String, Arc<audio::Stereo>)>,
}

pub struct Host {
    bp: Option<Blueprint>,
    /// every change made to the song, and the undo and redo steps
    history: History,
    version: u64,
    song_version: u64,
    log: Vec<String>,
    loudness: BTreeMap<String, f64>,
    render: Option<RenderView>,
    verdict: Option<VerdictView>,
    /// the loops a genre built on samples plays: the ones the page found for it, and the library of them
    prepared: Vec<Prepared>,
    lib: Option<Arc<Library>>,
    /// each file cut from a recording: the recording, which a project credits, and whether the file keeps floats (a
    /// drum hit's does)
    cuts: BTreeMap<String, (Found, bool)>,
    /// each scratch kept as a take: what its record was cut from, which a project credits
    takes: BTreeMap<String, take::Sources>,
    /// the song version whose render failed: it is not tried again until the song changes
    failed: Option<u64>,
    seeds: Rng,
    /// each track as the last render made it, so that the next renders only the tracks an edit changed
    kept: render::Kept,
    /// the last whole render's master, and what it settled for the whole song: an edit is heard at the playhead in it
    /// first (`preview`)
    last: Option<(Arc<Stereo>, render::Settled)>,
    /// where the studio's playhead is, in seconds (`set_playhead`)
    playhead: f64,
    /// a mutation or an evolution under way, a candidate at a time (`step`)
    breeding: Option<Breeding>,
    /// the last pitch check asked for, as it was asked and as the studio shows it, and how many were asked for
    pitch_check: Option<(check::Check, PitchCheck)>,
    asked: u64,
    /// the person's voice as their last pitch check heard it
    voice: Option<claudius::voice::singer::Voice>,
    /// the auto-sampler's last offer, and the pieces as the studio sees them
    pieces: Option<(Offer, PiecesView)>,
    /// the recording analyzed last, by the name of its file, which a song like it is made from (`Command::SongLike`)
    heard: Option<(String, Box<claudius::analysis::song::Analysis>)>,
    /// what was heard in it as the studio shows it, until the page takes it with the answer (`take_listened`)
    listened: Option<claudius_api::Listened>,
}

/// what the auto-sampler offered last: the recording, the library's record of it (what the pieces kept credit), how
/// it was asked, and the pieces
struct Offer {
    name: String,
    src: Arc<Stereo>,
    found: Option<serde_json::Value>,
    opt: claudius::sampler::Options,
    seed: u64,
    pieces: Vec<claudius::sampler::Piece>,
}

/// A mutation or an evolution under way: its candidates, judged one a `step`, from the song as it was at `from` (an
/// edit meanwhile ends it), and whether the best is kept when it beats the song (a mutation's only when asked, and
/// only by 0.1).
struct Breeding {
    what: &'static str,
    run: claudius::evolve::Stepwise,
    from: u64,
    keep: bool,
}

impl Host {
    /// a session with no song yet; `seed` seeds the songs asked for without one
    pub fn new(seed: u64) -> Host {
        Host {
            bp: None,
            history: History::default(),
            version: 0,
            song_version: 0,
            log: Vec::new(),
            loudness: BTreeMap::new(),
            render: None,
            verdict: None,
            prepared: Vec::new(),
            lib: None,
            cuts: BTreeMap::new(),
            takes: BTreeMap::new(),
            failed: None,
            seeds: Rng::new(seed),
            kept: render::Kept::default(),
            last: None,
            playhead: 0.0,
            breeding: None,
            pitch_check: None,
            asked: 0,
            voice: None,
            pieces: None,
            heard: None,
            listened: None,
        }
    }

    /// what the recording analyzed last was heard to hold, as the studio shows it, once: the page sends it with the
    /// answer to `analyze`
    pub fn take_listened(&mut self) -> Option<claudius_api::Listened> {
        self.listened.take()
    }

    /// a new song like the recording analyzed last, in `genre` (the song's, when none)
    fn song_like(&mut self, genre: Option<String>, seed: Option<u64>) -> Reply {
        let Some((name, a)) = self.heard.clone() else {
            return self
                .no("analyze a recording first: a song like it takes that recording's tempo, key, chords and form");
        };
        let genre = genre.or_else(|| self.bp.as_ref().map(|b| b.genre.clone())).unwrap_or_else(|| "house".into());
        let g = match genres::get(&genre) {
            Ok(g) => g,
            Err(e) => return self.no(e.to_string()),
        };
        let seed = seed.unwrap_or_else(|| self.seed());
        let lib = self.lib.clone();
        let (bp, said) =
            crate::listen::song_like(&a, &name, &g, seed, lib.as_deref().map(|l| l as &dyn compose::Library));
        self.forget_render();
        self.commit_song(Some(bp), said)
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
            let path = f.path.to_string_lossy().into_owned();
            self.cuts.insert(path.clone(), (r.found.clone(), f.float));
            audio::remember(&path, a);
        }
        let mut p = cut.prepared;
        p.asked = asked;
        self.say(format!("  {}: {}", p.path, r.found.credit()));
        self.prepared.retain(|q| q.path != p.path);
        self.prepared.push(p.clone());
        self.lib = Some(Arc::new(Library::with_prepared(self.prepared.clone())));
        Ok(p)
    }

    /// A recording the person chose on the page (`bytes`, the file itself), listened to as the app listens to one the
    /// studio sends (`listen`): analyzed, remade in a genre (a new song, as a new song replaces the old), its vocals
    /// sampled into the song, or its groove taken for the song's drums. What a remake or the sampler keeps of it is kept
    /// in memory (`take::IN_MEMORY`), and goes in the project the page saves.
    pub fn listen(&mut self, l: &crate::listen::Listen, bytes: Vec<u8>) -> Reply {
        use crate::listen::{self, Listen};
        let name = l.name().to_string();
        // a project is a zip, opened before anything is decoded as a recording
        if matches!(l, Listen::OpenProject { .. }) {
            return self.open_project(&name, &bytes);
        }
        let x = match listen::decode(bytes, &name) {
            Ok(x) => x,
            Err(e) => return self.no(e),
        };
        let dir = std::path::Path::new(take::IN_MEMORY);
        match l {
            Listen::Analyze { .. } => {
                let a = claudius::analysis::song::analyze(&x, None);
                let text = listen::analysis_text(&a);
                self.listened = Some(listen::listened(&a, &x, &name, None));
                self.heard = Some((name.clone(), Box::new(a)));
                self.say(format!("analyzed {name}"));
                self.version += 1;
                self.ok(&text)
            }
            Listen::Remake { genre, seed, flip, rebuild, .. } => {
                let genre = genre.clone().or_else(|| self.bp.as_ref().map(|b| b.genre.clone()));
                let genre = genre.unwrap_or_else(|| "house".into());
                let g = match genres::get(&genre) {
                    Ok(g) => g,
                    Err(e) => return self.no(e.to_string()),
                };
                let stems = claudius::stems::of(&x, &mut |_| {});
                let a = claudius::analysis::song::analyze_with(&x, stems.as_deref(), None);
                let seed = seed.unwrap_or_else(|| self.seed());
                let lib = self.lib.clone();
                let with = listen::With { lib: lib.as_deref().map(|l| l as &dyn compose::Library), dir, ears: None };
                let how = listen::Remade::of(None, Some(*flip), Some(*rebuild));
                let src = listen::Heard { x: &x, path: &name, name: &name, a: &a, stems: stems.as_deref() };
                let (bp, verb, told) = listen::remake(&src, &g, seed, None, how, &with);
                self.forget_render();
                self.commit_song(Some(bp), format!("{verb} {name} as {genre}{told}:\n{}", listen::analysis_text(&a)))
            }
            Listen::SampleSong { found, how, sections, every, .. } => {
                let Some(mut bp) = self.bp.clone() else {
                    return self.no("no song yet: start one with a new song, then sample a recording into it");
                };
                let opt = match listen::sampling(None, None, None, sections.clone(), how.as_deref())
                    .and_then(|o| Ok(claudius::sampler::Options { every: listen::every(every.as_deref())?, ..o }))
                {
                    Ok(opt) => opt,
                    Err(e) => return self.no(e),
                };
                let before = sampled(&bp);
                match claudius::sampler::sample(&mut bp, &x, &name, None, &opt, dir) {
                    Ok(s) => {
                        // each phrase cut from a library's recording credits it
                        if let Some(credit) = credit(found) {
                            for path in sampled(&bp).difference(&before) {
                                self.takes.insert(path.clone(), credit.clone());
                            }
                        }
                        self.commit(Some(bp), s.said(&name))
                    }
                    Err(e) => self.no(e),
                }
            }
            Listen::PitchCheck { id } => self.hear_pitch_check(*id, &x),
            Listen::OpenProject { .. } => self.no("a project is opened, not listened to"),
            Listen::SamplePieces { seed, count, found, .. } => {
                let opt = claudius::sampler::Options { count: count.map_or(2, |c| c as usize), ..Default::default() };
                self.offer_pieces(Arc::new(x), &name, found.clone(), opt, seed.unwrap_or(0))
            }
            Listen::Take { from, track, role, found, .. } => {
                let Some(mut bp) = self.bp.clone() else {
                    return self.no("no song yet: start one with a new song, then record a take for it");
                };
                let secs = x.secs();
                if secs > model::MAX_SECONDS {
                    return self.no(format!("the take is {secs:.0} s long, longer than a song may be"));
                }
                let path = match take::store(&x, dir) {
                    Ok(p) => p,
                    Err(e) => return self.no(format!("cannot keep the take: {e:#}")),
                };
                match take::add(&mut bp, &path, secs, from, track.as_deref(), Some(role)) {
                    Ok(msg) => {
                        if let Some(credit) = credit(found) {
                            self.takes.insert(path.to_string_lossy().into_owned(), credit);
                        }
                        self.commit(Some(bp), msg)
                    }
                    Err(e) => self.no(e),
                }
            }
            Listen::ExtractGroove { .. } => {
                let Some(mut bp) = self.bp.clone() else {
                    return self.no("no song yet: start one with a new song, then take a recording's groove");
                };
                let heard = match claudius::analysis::groove::hear(&x, claudius::analysis::groove::tempo_in_name(&name))
                {
                    Ok(h) => h,
                    Err(e) => return self.no(format!("{name}: {e}")),
                };
                match listen::groove(&mut bp, &heard, &name, &listen::groove_name(&name), &[]) {
                    Ok(msg) => self.commit(Some(bp), msg),
                    Err(e) => self.no(e),
                }
            }
        }
    }

    /// The song as a project to open in claudius (`song::pack_project`), a zip: the files it plays, which only this
    /// session holds, go in it (`project_path`), a drum hit as the 32-bit WAV it was cut as and the rest as 24-bit
    /// FLAC, each with the recording it was cut from (its page, creator and licence) in a `.json` beside it, and
    /// their credits in credits.txt.
    pub fn project(&self) -> Result<Vec<u8>, String> {
        let mut bp = self.bp.clone().ok_or("no song yet: start one with a new song")?;
        let tags = claudius::export::tags(&bp.title);
        let mut moved: BTreeMap<String, String> = BTreeMap::new();
        let (mut files, mut found) = (Vec::new(), Vec::new());
        for path in bp.files_mut() {
            if let Some(to) = moved.get(path.as_str()) {
                *path = to.clone();
                continue;
            }
            let audio = audio::remembered_audio(path)
                .ok_or_else(|| format!("the song plays {path}, which this page does not hold"))?;
            let cut = self.cuts.get(path.as_str());
            let float = cut.is_some_and(|c| c.1) || audio.l.iter().chain(&audio.r).any(|v| v.abs() > 1.0);
            let to = project_path(path, float);
            let spec = if float { "wav:32" } else { "flac:24" };
            let spec = spec.parse::<claudius::export::Spec>().map_err(|e| e.to_string())?;
            let bytes =
                claudius::export::encode(&audio, &spec, &tags).map_err(|e| format!("cannot keep {to}: {e:#}"))?;
            files.push((to.clone(), bytes));
            if let Some((f, _)) = cut {
                files.push((format!("{to}.json"), serde_json::to_vec_pretty(f).map_err(|e| e.to_string())?));
                found.push(f.clone());
            }
            if let Some(s) = self.takes.get(path.as_str()).filter(|s| !s.is_empty()) {
                let beside = take::sources_path(std::path::Path::new(&to)).to_string_lossy().into_owned();
                files.push((beside, serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?));
                found.extend(s.recordings.iter().cloned());
            }
            moved.insert(std::mem::replace(path, to.clone()), to);
        }
        let credits = claudius_packs::online::credits_of(&found);
        if !credits.is_empty() {
            files.push(("credits.txt".into(), credits.into_bytes()));
        }
        claudius::song::pack_project(&bp, &files).map_err(|e| format!("{e:#}"))
    }

    /// A project a page saved (`project`), the zip itself, called `name`, opened as the song: its blueprint, the
    /// recordings it plays held in memory again under the names a page holds them by (`demo:takes/...`), each with what
    /// it was cut from and the recordings a take credits, which a project it saves again keeps.
    pub fn open_project(&mut self, name: &str, bytes: &[u8]) -> Reply {
        let (mut bp, files) = match claudius::song::read_project(bytes, name) {
            Ok(x) => x,
            Err(e) => return self.no(format!("{e:#}")),
        };
        // every recording read before any is held, so that a zip that lacks one, or one that does not read, changes
        // nothing the page holds
        let mut held: BTreeMap<String, String> = BTreeMap::new();
        let (mut keep, mut cuts, mut takes): (Vec<(String, Stereo)>, _, _) = (Vec::new(), Vec::new(), Vec::new());
        for path in bp.files_mut() {
            if let Some(to) = held.get(path.as_str()) {
                *path = to.clone();
                continue;
            }
            let Some(audio) = files.get(path.as_str()) else {
                return self
                    .no(format!("{name} plays {path}, which it does not hold: it is not a project this page saved"));
            };
            let ext = std::path::Path::new(path.as_str())
                .extension()
                .map_or(String::new(), |e| e.to_string_lossy().into_owned());
            let x = match claudius::audio::load_bytes(audio.clone(), &ext) {
                Ok(x) => x,
                Err(e) => return self.no(format!("cannot read {path} in {name}: {e:#}")),
            };
            let to = free_name(path, &x, &keep);
            keep.push((to.clone(), x));
            if let Some(found) =
                files.get(&format!("{path}.json")).and_then(|f| serde_json::from_slice::<Found>(f).ok())
            {
                cuts.push((to.clone(), (found, ext == "wav")));
            }
            let beside = take::sources_path(std::path::Path::new(path.as_str())).to_string_lossy().into_owned();
            if let Some(s) = files.get(&beside).and_then(|f| serde_json::from_slice::<take::Sources>(f).ok()) {
                takes.push((to.clone(), s));
            }
            held.insert(std::mem::replace(path, to.clone()), to);
        }
        for (to, x) in keep {
            claudius::audio::remember(&to, x);
        }
        self.cuts.extend(cuts);
        self.takes.extend(takes);
        // the song before, its render and verdict with it, is not the one opened
        self.forget_render();
        let message = format!("opened {name}: {}", bp.title);
        self.commit_song(Some(bp), message)
    }

    /// the script language's reference, as the studio shows it
    pub fn reference() -> &'static str {
        script::REFERENCE
    }

    /// the sounds a part or a lane can play here: the synth's presets and drums (no packs are installed)
    pub fn sounds() -> SoundList {
        views::sounds()
    }

    /// The studio's actions `query` finds, at most `limit`, the best first, the changes in the history of `state` it
    /// finds that the song can go back to before, and what it asks of the song, read as what claudius would do for it:
    /// by name and by words, and by meaning too once the embedding model is there (`meaning`).
    pub fn find(query: &str, limit: usize, state: &State) -> claudius_intent::Answer {
        let held = crate::meaning::held();
        let mut answer = claudius_intent::answer(query, limit, state, &held.finder, &held.asker, held.model.as_deref());
        crate::meaning::ask_model(&mut answer, state, &held.asker);
        answer
    }

    pub fn state(&self) -> State {
        State {
            version: self.version,
            song_version: self.song_version,
            song: self.bp.as_ref().map(|b| views::song(b, &self.loudness)),
            render: self.render.clone(),
            verdict: self.verdict.clone(),
            busy: match &self.breeding {
                Some(b) => {
                    let (done, all) = b.run.progress();
                    Some(format!("{}: {done} of {all} judged", b.what))
                }
                None => self.stale().then(|| "rendering".to_string()),
            },
            progress: self.breeding.as_ref().map(|b| {
                let (done, all) = b.run.progress();
                done as f32 / all.max(1) as f32
            }),
            cancellable: self.breeding.is_some(),
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            undo_steps: self.history.steps().0,
            redo_steps: self.history.steps().1,
            undo_label: self.history.undo_label(),
            redo_label: self.history.redo_label(),
            history: self.history.done(),
            log: self.log.clone(),
            genres: views::genre_infos(),
            exports: views::export_formats(),
            pitch_check: self.pitch_check.as_ref().map(|(_, view)| view.clone()),
            pieces: self.pieces.as_ref().map(|(_, view)| view.clone()),
            voice: self.voice.as_ref().map(|v| v.describe()),
            ..Default::default()
        }
    }

    /// whether the song is newer than its render (a preview is not a render of it), unless rendering it failed
    pub fn stale(&self) -> bool {
        self.bp.is_some()
            && self.failed != Some(self.song_version)
            && self.render.as_ref().is_none_or(|r| r.version < self.song_version || r.url == PREVIEW_URL)
    }

    /// Something of a song newer than its render to play while the whole of it renders, handed over; None when there is
    /// nothing to preview: no song, a render of it already, a preview of it already, or a song no longer than a
    /// preview. A song heard whole before, and as long as it was, is that render with the bars at the playhead as they
    /// are now (`window`); another, its first bars.
    pub fn preview(&mut self) -> Option<Audio> {
        if !self.stale() || self.render.as_ref().is_some_and(|r| r.url == PREVIEW_URL && r.version == self.song_version)
        {
            return None;
        }
        let project = compose::arrange(self.bp.as_ref()?);
        if let Some(audio) = self.window(&project) {
            return Some(audio);
        }
        let bp = self.bp.as_ref()?;
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

    /// The last whole render with the `WINDOW` seconds from the playhead on as the song is now (`render::render_window`:
    /// the whole song's render there, but for the master's loudness, held as it was), crossfaded in, so that an edit is
    /// heard where the person listens within a moment; the rest of the song is as it was until the whole of it renders.
    fn window(&mut self, project: &model::Project) -> Option<Audio> {
        let (last, settled) = self.last.clone()?;
        let from = ((self.playhead - 0.25).max(0.0) * claudius::config::SRF) as usize;
        let n = (WINDOW * claudius::config::SRF) as usize;
        let opts = render::Options::default();
        let w = render::render_window(project, &opts, &mut self.kept, &settled, from, n).ok()??;
        let mut master = (*last).clone();
        let fade = (0.01 * claudius::config::SRF) as usize;
        let len = w.master.len();
        for i in 0..len.min(master.len().saturating_sub(w.start)) {
            // in over its first 10 ms and out over its last, unless it runs to the song's end
            let g = if i < fade {
                i as f32 / fade as f32
            } else if len - i <= fade && w.start + len < master.len() {
                (len - i) as f32 / fade as f32
            } else {
                1.0
            };
            let k = w.start + i;
            master.l[k] += (w.master.l[i] - master.l[k]) * g;
            master.r[k] += (w.master.r[i] - master.r[k]) * g;
        }
        let spb = 60.0 / project.bpm;
        let view = RenderView {
            version: self.song_version,
            url: PREVIEW_URL.into(),
            seconds: master.secs(),
            lufs: self.render.as_ref().map_or(f64::NAN, |r| r.lufs),
            true_peak_db: self.render.as_ref().map_or(f32::NAN, |r| r.true_peak_db),
            bpm: project.bpm,
            sections: project
                .sections
                .iter()
                .map(|s| (s.name.clone(), s.start_bar as f64 * 4.0 * spb, (s.start_bar + s.bars) as f64 * 4.0 * spb))
                .collect(),
            waveform: views::waveform_of(&master, 1000),
            stems: Vec::new(),
        };
        let at = w.start as f64 / claudius::config::SRF;
        self.say(format!(
            "the change heard from {}:{:02} for {WINDOW:.0} s, at the loudness of the render before, while the whole song \
             renders",
            (at / 60.0) as u64,
            (at % 60.0) as u64
        ));
        self.render = Some(view.clone());
        self.version += 1;
        Some(Audio { view, samples: master.interleaved() })
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
        // an edit it makes is recorded as it
        self.history.doing(serde_json::to_value(&c).ok().and_then(|v| v["cmd"].as_str().map(str::to_string)));
        let reply = self.run(c);
        self.history.doing(None);
        reply
    }

    fn run(&mut self, c: Command) -> Reply {
        match c {
            Command::NewSong { genre, seed, bpm, key, sampled, sample_search, complexity, .. } => {
                if let Some(c) = complexity.filter(|c| !(0.0..=1.0).contains(c)) {
                    return self.no(format!("a complexity is from 0 (sparse) to 1 (layered), not {c}"));
                }
                let sample_search = sample_search.as_deref().and_then(claudius::sampling::words);
                let opts = Options { bpm, sampled, sample_search, complexity, ..Default::default() };
                self.new_song(&genre, seed, key, opts)
            }
            Command::OpenText { name, text } => self.open_text(&name, &text),
            Command::Undo => self.history(true),
            Command::Redo => self.history(false),
            Command::GoBack { to } => self.go_back(&to),
            Command::Together { said, commands } => self.together(said, commands),
            Command::SongLike { genre, seed } => self.song_like(genre, seed),
            Command::ChangeGenre { genre, seed } => {
                if self.bp.is_none() {
                    return self.no("no song yet: start one with a new song, then change its genre");
                }
                let g = match genres::get(&genre) {
                    Ok(g) => g,
                    Err(e) => return self.no(e.to_string()),
                };
                let seed = seed.unwrap_or_else(|| self.seed());
                // a song twice as many bars at a faster genre's tempo may grow past what a song may be, which the
                // server's edits check and a take's do not
                self.edit_take(|bp| {
                    let (new, said) = claudius::regenre::change(bp, &g, seed, None);
                    new.check_size()?;
                    *bp = new;
                    Ok(said)
                })
            }
            Command::Render { .. } if self.stale() => self.ok("rendering"),
            Command::Render { .. } => self.ok("the render is up to date"),
            // a take's drums and singing, which the page holds in memory
            Command::TranscribeDrums { take: Some(t), bars, variant, keep_take, .. } => {
                let o = take::Drums { bars, variant, keep_take };
                self.edit_take(|bp| take::drums(bp, &t, &o))
            }
            Command::TranscribeTake { take: t, sounds, name, role, chromatic, keep_take, style } => {
                let m = take::Melody { sounds, name, role, chromatic, keep_take, style };
                self.edit_take(|bp| take::melody(bp, &t, &m))
            }
            Command::AssessVoice { take: Some(t), .. } => {
                let heard = match self.bp.as_ref() {
                    Some(bp) => take::assess(bp, &t),
                    None => Err("no song yet: start one with a new song".into()),
                };
                match heard {
                    Ok(a) => {
                        self.say(format!("heard how {t} is sung"));
                        self.version += 1;
                        self.ok(&a.describe())
                    }
                    Err(e) => self.no(e),
                }
            }
            Command::RequestPitchCheck { low, high, note } => self.request_pitch_check(low, high, note),
            Command::RedrawPieces { seed } => match &self.pieces {
                Some((o, _)) => {
                    let (src, name, found, opt, last) =
                        (o.src.clone(), o.name.clone(), o.found.clone(), o.opt.clone(), o.seed);
                    self.offer_pieces(src, &name, found, opt, seed.unwrap_or(last + 1))
                }
                None => self.no("no pieces to draw again: sample a recording first"),
            },
            Command::KeepPieces { keep, sections, every } => self.keep_pieces(&keep, sections, every.as_deref()),
            Command::CancelPitchCheck { id } => match &mut self.pitch_check {
                Some((_, view)) if view.id == id && view.status == TakeStatus::Waiting => {
                    view.status = TakeStatus::Cancelled;
                    self.say(format!("the pitch check will not be sung (check {id})"));
                    self.version += 1;
                    self.ok(&format!("pitch check {id}: not now, then"))
                }
                Some((_, view)) if view.id == id => self.no(format!("pitch check {id} is answered already")),
                _ => self.no(format!("no pitch check {id}")),
            },
            Command::Tune { track, strength, speed_ms, humanize, vibrato, scale, off } => {
                let asked = edit::Tuning { strength, speed_ms, humanize, vibrato, scale: scale.as_deref(), off };
                let voice = self.voice.clone();
                self.edit_take(|bp| edit::tune(bp, &track, &asked, voice.as_ref()))
            }
            Command::QuickFix { target } => {
                let Some(verdict) = self.verdict.clone() else {
                    return self.no("no verdict yet: let the song render first");
                };
                let stale = self.render.as_ref().is_none_or(|r| r.version < self.song_version || r.url == PREVIEW_URL);
                self.edit_take(|bp| edit::quick_fix(&verdict, bp, target.as_deref(), stale))
            }
            Command::TrimTake { track, start, end } => {
                let path = |bp: Option<&Blueprint>| {
                    let l = bp?.loops.iter().find(|l| l.name == track)?;
                    match &l.sound {
                        model::Sound::Loop { path, .. } => Some(path.clone()),
                        _ => None,
                    }
                };
                let old = path(self.bp.as_ref());
                let r = self.edit_take(|bp| take::trim(bp, &track, start, end, std::path::Path::new(take::IN_MEMORY)));
                // the cut credits what the take did
                if let (true, Some(old), Some(new)) = (r.ok, old, path(self.bp.as_ref())) {
                    let credits = self.credited(std::path::Path::new(&old));
                    self.takes.insert(new, credits);
                }
                r
            }
            Command::TranscribeDrums { take: None, path: None, .. } => {
                self.no("transcribe_drums wants a take, or the path of a recording")
            }
            Command::AssessVoice { take: None, path: None, .. } => {
                self.no("name a take of the song or an audio file to hear")
            }
            Command::AddScratch { from, track, bar, bars, moves, name, role, recording } => {
                match edit::scratch(track, bar, bars, &moves, recording.as_deref()) {
                    Ok(s) => self.add_scratch(&from, &s, name, role.as_deref()),
                    Err(e) => self.no(e),
                }
            }
            Command::Mutate { count, keep_best } => {
                let count = count.clamp(1, 32);
                let s = Settings {
                    generations: 1,
                    population: 1,
                    children: count,
                    crossover: 0.0,
                    ..self.breed_settings()
                };
                self.breed("mutating", s, keep_best)
            }
            Command::Evolve { generations, children } => {
                let s = Settings {
                    generations: generations.clamp(1, 20),
                    children: children.clamp(2, 32),
                    ..self.breed_settings()
                };
                self.breed("evolving", s, true)
            }
            Command::Cancel => match self.breeding.is_some() {
                true => {
                    self.say("stopped: the candidates judged so far are all there are".into());
                    self.finish_breeding()
                }
                false => self.no("nothing to cancel: the demo renders as it goes"),
            },
            c => self.edit(&c),
        }
    }

    /// what a mutation or an evolution breeds with: the server's, seeded anew each time
    fn breed_settings(&mut self) -> Settings {
        Settings { seed: self.seeds.next_u64(), ..Settings::default() }
    }

    /// starts a mutation or an evolution of the song (`what`), which `step` judges a candidate at a time; with
    /// `keep`, the best is kept when it beats the song
    fn breed(&mut self, what: &'static str, s: Settings, keep: bool) -> Reply {
        let Some(bp) = self.bp.clone() else { return self.no("no song yet: start one with a new song") };
        if let Some(b) = &self.breeding {
            return self.no(format!("{} already: stop it first", b.what));
        }
        let g = match genres::get(&bp.genre) {
            Ok(g) => g,
            Err(e) => return self.no(e.to_string()),
        };
        let children = s.children * s.generations;
        let run = claudius::evolve::Stepwise::new(&g, vec![bp], s);
        self.breeding = Some(Breeding { what, run, from: self.song_version, keep });
        self.version += 1;
        self.ok(&format!("{what}: judging the song and {children} variations of it, one at a time"))
    }

    /// whether a mutation or an evolution is under way, for `step`
    pub fn breeding(&self) -> bool {
        self.breeding.is_some()
    }

    /// Judges the next candidate of the mutation or evolution under way, and once the last is judged keeps the best if
    /// it beats the song. An edit since it began ends it, keeping nothing. Once it has ended, what came of it (the
    /// command's answer, which comes after its candidates); None while it goes on, or when none is under way.
    pub fn step(&mut self) -> Option<Reply> {
        let b = self.breeding.as_mut()?;
        if b.from != self.song_version {
            let what = b.what;
            self.breeding = None;
            self.say(format!("{what} stopped: the song changed"));
            self.version += 1;
            return Some(self.ok(&format!("{what} stopped: the song changed")));
        }
        let lib = self.lib.clone();
        let lib = lib.as_deref().map(|l| l as &dyn claudius::compose::Library);
        match b.run.step(lib) {
            Ok(Some(said)) => {
                self.say(said);
                self.version += 1;
                None
            }
            Ok(None) => Some(self.finish_breeding()),
            Err(e) => {
                let what = b.what;
                self.breeding = None;
                self.say(format!("{what} failed: {e:#}"));
                self.version += 1;
                Some(self.no(format!("{what} failed: {e:#}")))
            }
        }
    }

    /// the command a mutation or evolution under way answers, for its answer once it ends (`step`)
    pub fn breeding_command(&self) -> Option<&'static str> {
        self.breeding.as_ref().map(|b| if b.what == "mutating" { "mutate" } else { "evolve" })
    }

    /// ends the mutation or evolution under way: its best candidate becomes the song when it was asked to be kept and
    /// beats the song (a mutation's by 0.1)
    fn finish_breeding(&mut self) -> Reply {
        let Some(b) = self.breeding.take() else { return self.no("nothing to stop") };
        self.version += 1;
        // its candidates are of the song as it was: one edited since, or another song, stays as it is
        if b.from != self.song_version {
            self.say(format!("{} stopped: the song changed", b.what));
            return self.ok(&format!("{} stopped: the song changed", b.what));
        }
        let all = b.run.into_candidates();
        let start = all.iter().find(|c| c.generation == 0).map_or(f64::MIN, |c| c.score);
        let margin = if b.what == "mutating" { 0.1 } else { 0.0 };
        let best = all.into_iter().find(|c| c.generation > 0);
        match best {
            Some(best) if b.keep && best.score >= start + margin && best.score > start => {
                let mut child = best.blueprint;
                child.lineage.push(format!(
                    "{}: {}",
                    if b.what == "mutating" { "mutated" } else { "evolved" },
                    best.change
                ));
                let msg = format!("kept {} ({start:.1} -> {:.1} on the preview)", best.change, best.score);
                self.commit(Some(child), msg)
            }
            Some(best) => {
                let said =
                    format!("no variation beat the song ({start:.1}; best {:.1}): it stays as it was", best.score);
                self.say(said.clone());
                self.ok(&said)
            }
            None => self.ok("no variations were judged"),
        }
    }

    /// what a file this session holds credits: the recording it was cut from, or what a scratch take was cut from
    fn credited(&self, path: &std::path::Path) -> take::Sources {
        let path = path.to_string_lossy();
        let mut s = self.takes.get(path.as_ref()).cloned().unwrap_or_default();
        if let Some((f, _)) = self.cuts.get(path.as_ref()) {
            s.add(take::Sources { recordings: vec![f.clone()], instruments: Vec::new() });
        }
        s
    }

    /// a pitch check for the person to sing back, six phrases of four notes from `low` to `high` (A3 to A4 by
    /// default), as the app asks for one
    fn request_pitch_check(&mut self, low: Option<i32>, high: Option<i32>, note: String) -> Reply {
        let (low, high) = (low.unwrap_or(57), high.unwrap_or(69));
        if !(36..=84).contains(&low) || !(36..=84).contains(&high) || high < low + 7 {
            return self.no("a pitch check's notes go from low to high, MIDI 36 to 84, at least a fifth apart");
        }
        let c = check::Check::new(low, high, self.seed());
        let notes = c.notes().iter().map(|&(p, t, d)| claudius_api::NoteView { t, d, p, v: 0.8 }).collect();
        self.asked += 1;
        let id = self.asked;
        let view = PitchCheck {
            id,
            bpm: check::BPM,
            notes,
            answers: c.answers(),
            beats: c.beats(),
            note,
            ..Default::default()
        };
        self.pitch_check = Some((c, view));
        self.say(format!("asked the person for a pitch check (check {id})"));
        self.version += 1;
        self.ok(&format!("asked the person for a pitch check (check {id}): six phrases of four notes to sing back"))
    }

    /// the answers to pitch check `id`: what claudius hears in them, which answers the check, and the voice it hears,
    /// which becomes the person's
    fn hear_pitch_check(&mut self, id: u64, sung: &Stereo) -> Reply {
        let c = match &self.pitch_check {
            Some((c, view)) if view.id == id && view.status == TakeStatus::Waiting => c.clone(),
            Some((_, view)) if view.id == id => return self.no(format!("pitch check {id} is answered already")),
            _ => return self.no(format!("no pitch check {id}: ask for one with request_pitch_check")),
        };
        let heard = c.hear(sung);
        let said = heard.describe();
        if heard.answers.iter().all(|a| a.sung.is_none()) {
            return self.no(said);
        }
        if let Some((_, view)) = &mut self.pitch_check {
            (view.status, view.answer) = (TakeStatus::Taken, Some(said.clone()));
        }
        self.voice = Some(heard.voice);
        self.say(format!("heard the pitch check: {said}"));
        self.version += 1;
        self.ok(&said)
    }

    /// The pieces of recording `x`, called `name`, the auto-sampler offers for take `seed`, each to hear as it was cut
    /// and treated (small MP3 files in the state, which the page plays as they are), held until another offer
    /// replaces them.
    fn offer_pieces(
        &mut self,
        x: Arc<Stereo>,
        name: &str,
        found: Option<serde_json::Value>,
        opt: claudius::sampler::Options,
        seed: u64,
    ) -> Reply {
        if self.bp.is_none() {
            return self.no("no song yet: start one with a new song, then sample a recording into it");
        }
        let mut pieces = match claudius::sampler::pieces(&x, None, &opt, seed) {
            Ok(p) => p,
            Err(e) => return self.no(e),
        };
        // numbered as they come in the recording, so that the numbers follow it along its timeline
        pieces.sort_by(|a, b| {
            let at = |p: &claudius::sampler::Piece| p.phrase.within.map_or(p.phrase.start, |w| w.0);
            at(a).total_cmp(&at(b))
        });
        let spec: claudius::export::Spec = "mp3:64".parse().expect("a preview's format");
        let heard = |sound: &Stereo| -> Result<String, String> {
            let bytes =
                claudius::export::encode(sound, &spec, &claudius::export::tags(name)).map_err(|e| format!("{e:#}"))?;
            Ok(crate::listen::mp3_url(&bytes))
        };
        let mut views = Vec::new();
        for (i, p) in pieces.iter().enumerate() {
            let (raw, treated) = match (heard(&p.raw), heard(&p.treated)) {
                (Ok(r), Ok(t)) => (r, t),
                (Err(e), _) | (_, Err(e)) => return self.no(format!("cannot make the pieces to hear: {e}")),
            };
            let (from, to) = p.phrase.within.unwrap_or((p.phrase.start, p.phrase.start));
            views.push(PieceView {
                id: i as u32 + 1,
                place: p.place(),
                bars: p.phrase.bars,
                sung: p.phrase.sung,
                from,
                to,
                section: p.section.clone(),
                raw,
                treated,
                treatment: p.treatment.clone(),
            });
        }
        let lines: Vec<String> = views.iter().map(crate::listen::piece_said).collect();
        let n = views.len();
        let said = format!("{n} piece{} of {name}, take {seed}:\n{}", if n == 1 { "" } else { "s" }, lines.join("\n"));
        let view = PiecesView { recording: name.to_string(), seed, pieces: views };
        self.pieces = Some((Offer { name: name.to_string(), src: x, found, opt, seed, pieces }, view));
        self.say(format!("offered {n} pieces of {name} (take {seed})"));
        self.version += 1;
        self.ok(&said)
    }

    /// the pieces offered that `keep` names, each as it was cut or treated, sampled into the song in `sections`,
    /// crediting the library's recording they came from; the offer goes once they are in
    fn keep_pieces(&mut self, keep: &[KeptPiece], sections: Vec<String>, every: Option<&str>) -> Reply {
        let Some((o, _)) = &self.pieces else {
            return self.no("no pieces to keep: sample a recording first");
        };
        let every = match crate::listen::every(every) {
            Ok(every) => every,
            Err(e) => return self.no(e),
        };
        if let Some(k) = keep.iter().find(|k| o.pieces.get((k.id as usize).wrapping_sub(1)).is_none()) {
            let n = o.pieces.len();
            return self.no(format!("no piece {} (pieces 1 to {n})", k.id));
        }
        let Some(mut bp) = self.bp.clone() else { return self.no("no song yet: start one with a new song") };
        let (o, view) = self.pieces.take().expect("an offer, as above");
        let chosen: Vec<_> = keep.iter().map(|k| (&o.pieces[k.id as usize - 1], k.treated)).collect();
        let opt = claudius::sampler::Options { sections, every, ..o.opt.clone() };
        let before = sampled(&bp);
        let dir = std::path::Path::new(take::IN_MEMORY);
        match claudius::sampler::keep(&mut bp, &o.src, &o.name, &chosen, &opt, dir) {
            Ok(s) => {
                if let Some(credit) = credit(&o.found) {
                    for path in sampled(&bp).difference(&before) {
                        self.takes.insert(path.clone(), credit.clone());
                    }
                }
                self.commit(Some(bp), s.said(&o.name))
            }
            // not kept, they are still on offer
            Err(e) => {
                self.pieces = Some((o, view));
                self.no(e)
            }
        }
    }

    /// the song changed by `f`, from a take it holds
    fn edit_take(&mut self, f: impl FnOnce(&mut Blueprint) -> Result<String, String>) -> Reply {
        let Some(mut bp) = self.bp.clone() else {
            return self.no("no song yet: start one with a new song, then record a take for it");
        };
        match f(&mut bp) {
            Ok(msg) => self.commit(Some(bp), msg),
            Err(e) => self.no(e),
        }
    }

    /// a scratch kept as a take, its audio in memory under a name of its own
    fn add_scratch(&mut self, from: &str, s: &scratch::Take, name: Option<String>, role: Option<&str>) -> Reply {
        let Some(mut bp) = self.bp.clone() else {
            return self.no("no song yet: start one with a new song, then scratch one of its tracks");
        };
        let lib = self.lib.clone();
        let (x, sources) = match scratch::take(&bp, s, lib.as_deref(), |p| self.credited(p)) {
            Ok(made) => made,
            Err(e) => return self.no(e),
        };
        let name = name.unwrap_or_else(|| scratch::name(&bp));
        let path = format!("take:{name}:{}", self.version);
        let secs = x.secs();
        audio::remember(&path, x);
        self.takes.insert(path.clone(), sources);
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
        self.commit_song(Some(bp), msg)
    }

    /// a blueprint (`.json`) opened, or a script run as a new song
    fn open_text(&mut self, name: &str, text: &str) -> Reply {
        if !std::path::Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            let before = self.song_version;
            let r = self.run_script(text, Some(format!("opened {name}")));
            if self.song_version != before {
                self.forget_render();
                self.history.began(&self.bp);
            }
            return r;
        }
        let bp = Blueprint::from_json(text);
        match bp.and_then(|bp| bp.check_size().and_then(|_| script::check_patterns(&bp)).map(|_| bp)) {
            Ok(bp) => {
                self.forget_render();
                self.commit_song(Some(bp), format!("opened {name}"))
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
        match edit::change(c, &mut bp, seed) {
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
        self.history.commit(old, &self.bp, &label);
        self.changed(msg)
    }

    /// another song in the song's place, committed as `commit` does, the whole song its change (`History::commit_song`)
    fn commit_song(&mut self, bp: Option<Blueprint>, msg: String) -> Reply {
        let label = msg.lines().next().unwrap_or("").to_string();
        let old = std::mem::replace(&mut self.bp, bp);
        self.history.commit_song(old, &self.bp, &label);
        self.changed(msg)
    }

    /// Edits made as one (`Command::Together`), as the server makes them: each as it is made alone, on the song as the
    /// ones before left it, and committed once, as one change undo calls `said`; when one fails, none is made.
    fn together(&mut self, said: String, commands: Vec<Command>) -> Reply {
        let Some(mut bp) = self.bp.clone() else { return self.no("no song yet: start one with a new song") };
        let before = bp.clone();
        let lib = self.lib.clone();
        let run = |code: &str, bp: Blueprint| {
            script::run(code, Some(bp), lib.clone())?.bp.ok_or_else(|| "the script left no song".to_string())
        };
        match edit::together(&commands, &mut bp, self.seed(), run) {
            Err(e) => self.no(format!("{said}: none of it made, as {e}")),
            Ok(()) if bp == before => self.ok("nothing changed"),
            Ok(()) => {
                bp.lineage.push(said.clone());
                self.commit_as(Some(bp), said.clone(), said)
            }
        }
    }

    fn history(&mut self, undo: bool) -> Reply {
        let label = if undo { self.history.undo(&mut self.bp) } else { self.history.redo(&mut self.bp) };
        let Some(label) = label else {
            return self.no(if undo { "nothing to undo" } else { "nothing to redo" });
        };
        self.changed(format!("{}: {label}", if undo { "undid" } else { "redid" }))
    }

    /// back to the song as it was before the change `to` (`State::history`'s id for it)
    fn go_back(&mut self, to: &str) -> Reply {
        let Some(id) = OpId::parse(to) else {
            return self.no(format!("'{to}' names no change: use an id from the state's history"));
        };
        match self.history.back(&mut self.bp, &id) {
            Ok(msg) => self.changed(msg),
            Err(why) => self.no(why),
        }
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
        let r = match render::render_kept(&project, &render::Options::default(), &mut self.kept) {
            Ok(r) => r,
            Err(e) => {
                self.say(format!("render failed: {e}; change the song to try again"));
                self.failed = Some(self.song_version);
                self.version += 1;
                return Some(Err(e.to_string()));
            }
        };
        let verdict = views::verdict(&judge::judge(&project, &r, None), &bp);
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
        let samples = r.master.interleaved();
        self.last = r.settled.map(|s| (Arc::new(r.master), s));
        Some(Ok(Audio { view, samples }))
    }

    /// where the studio's playhead is, in seconds: an edit is heard there first
    pub fn set_playhead(&mut self, secs: f64) {
        self.playhead = secs.max(0.0);
    }

    /// What the song's next render would make anew, for other workers to make first (`render::track_sound`), so that
    /// the render only mixes; none when the song is not stale.
    pub fn to_make(&self) -> Option<ToMake> {
        if !self.stale() {
            return None;
        }
        let project = compose::arrange(self.bp.as_ref()?);
        let stale = self.kept.stale(&project, &render::Options::default()).ok()?;
        let mut tracks: Vec<(String, render::TrackKey, f64)> = stale
            .into_iter()
            .filter_map(|(name, key)| {
                let cost =
                    render::track_cost(project.tracks.iter().find(|t| t.name == name)?, project.bpm, project.secs());
                Some((name, key, cost))
            })
            .collect();
        tracks.sort_by(|a, b| b.2.total_cmp(&a.2));
        let mut audio: Vec<(String, Arc<audio::Stereo>)> = Vec::new();
        for t in project.tracks.iter().filter(|t| tracks.iter().any(|(n, _, _)| *n == t.name)) {
            if let model::Sound::Sample { path, .. } | model::Sound::Loop { path, .. } = &t.sound
                && !audio.iter().any(|(p, _)| p == path)
                && let Some(x) = audio::remembered_audio(path)
            {
                audio.push((path.clone(), x));
            }
        }
        Some(ToMake { project, tracks, audio })
    }

    /// the sampled instruments the song plays, each once (what a browser fetches the files of before it renders)
    pub fn instruments(&self) -> Vec<claudius_packs::InstrumentRef> {
        let Some(bp) = self.bp.as_ref() else { return Vec::new() };
        let mut refs: Vec<claudius_packs::InstrumentRef> = Vec::new();
        for t in &compose::arrange(bp).tracks {
            for s in t.sound.each() {
                if let model::Sound::Instrument { name, .. } = s
                    && let Some(r) = claudius_packs::resolve(name, None)
                    && !refs.contains(&r)
                {
                    refs.push(r);
                }
            }
        }
        refs
    }

    /// keeps a track `to_make` listed as another worker made it, for the next render
    pub fn made(&mut self, name: &str, key: render::TrackKey, sound: audio::Stereo, level: Option<f64>) {
        self.kept.keep(name, key, sound, level);
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

/// the files of the phrases sampled into `bp`
fn sampled(bp: &Blueprint) -> std::collections::BTreeSet<String> {
    bp.loops
        .iter()
        .filter(|l| l.sampled)
        .filter_map(|l| match &l.sound {
            model::Sound::Loop { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

/// what a recording a free library offered credits, from its record (`LibraryHit::found`)
fn credit(found: &Option<serde_json::Value>) -> Option<take::Sources> {
    let f: Found = serde_json::from_value(found.clone()?).ok()?;
    Some(take::Sources { recordings: vec![f], ..Default::default() })
}

/// a command's name, as the API tags it (`set_tempo`)
fn name(c: &Command) -> String {
    serde_json::to_value(c).ok().and_then(|v| v["cmd"].as_str().map(String::from)).unwrap_or_default()
}

/// where a file a session holds goes in a project, which the blueprint names it by: a sample cut from a recording at
/// the session's own name for it (`demo:samples/break/house/...` at `samples/break/house/...`), anything else (a
/// scratch take) in `takes/`; a `.wav` when it keeps floats, else a `.flac`
/// The name the page holds a recording `x` of a project by, from its `path` in the project: `demo:<path>`, unless the
/// page holds another recording by that name (another song's, which undo may bring back) or `keep` takes it for
/// another of the project's; then the first of `demo:<path>-2`, `-3` ... before its extension that is free, or holds
/// this very recording.
fn free_name(path: &str, x: &Stereo, keep: &[(String, Stereo)]) -> String {
    let (stem, ext) = path.rsplit_once('.').map_or((path, String::new()), |(s, e)| (s, format!(".{e}")));
    std::iter::once(format!("demo:{path}"))
        .chain((2..).map(|k| format!("demo:{stem}-{k}{ext}")))
        .find(|to| {
            let other = audio::remembered_audio(to).is_some_and(|held| held.l != x.l || held.r != x.r);
            !other && !keep.iter().any(|(k, _)| k == to)
        })
        .unwrap_or_default()
}

fn project_path(path: &str, float: bool) -> String {
    let ext = if float { "wav" } else { "flac" };
    match path.strip_prefix("demo:") {
        Some(rest) => {
            format!("{}.{ext}", rest.strip_suffix(".wav").or_else(|| rest.strip_suffix(".flac")).unwrap_or(rest))
        }
        None => format!("takes/{}.{ext}", claudius::song::slug(path)),
    }
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
        // the change, in the words the palette finds it by
        let said = &s.history[0].said;
        assert!(said.contains(if song.bpm > 100.0 { "slower" } else { "faster" }), "{said}");
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
    fn edits_made_together_are_one_change_as_the_server_makes_them() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3}));
        let song = h.state().song.unwrap();
        let track = song.tracks[0].name.clone();
        let said = "Mute the first track and change the tempo to 131 BPM";
        let together = json!({"cmd": "together", "said": said, "commands": [
            {"cmd": "set_mix", "track": track, "muted": true},
            {"cmd": "set_tempo", "bpm": 131},
        ]});
        let r = command(&mut h, together);
        assert!(r.ok, "{}", r.message);
        let s = h.state();
        let now = s.song.as_ref().unwrap();
        assert!(now.bpm == 131.0 && now.tracks[0].muted);
        assert_eq!((s.history.len(), s.undo_label.as_deref()), (2, Some(said)), "one change: {:?}", s.history);
        assert!(command(&mut h, json!({"cmd": "undo"})).ok);
        let back = h.state().song.unwrap();
        assert!(back.bpm == song.bpm && !back.tracks[0].muted, "undo takes both back");
        let wrong = json!({"cmd": "together", "said": "faster, and a track that is not there", "commands": [
            {"cmd": "set_tempo", "bpm": 140},
            {"cmd": "remove_track", "name": "no such track"},
        ]});
        let r = command(&mut h, wrong);
        assert!(!r.ok && r.message.contains("none of it made"), "{}", r.message);
        assert_eq!(h.state().song.unwrap().bpm, song.bpm);
        assert_eq!(h.state().redo_label.as_deref(), Some(said), "redo still brings back what was undone");
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
    fn a_song_whose_tracks_were_made_elsewhere_renders_as_it_would_have_alone() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 2}));
        let make = h.to_make().expect("a new song has tracks to make");
        assert!(make.tracks.len() >= 2 && make.tracks.windows(2).all(|w| w[0].2 >= w[1].2), "the costliest first");
        for (name, key, _) in &make.tracks {
            let (sound, level) = render::track_sound(&make.project, &render::Options::default(), name).unwrap();
            h.made(name, *key, sound, level);
        }
        assert!(h.to_make().is_some_and(|m| m.tracks.is_empty()), "nothing left to make");
        let shared = h.render().unwrap().unwrap();
        let mut alone = Host::new(1);
        command(&mut alone, json!({"cmd": "new_song", "genre": "house", "seed": 2}));
        assert!(shared.samples == alone.render().unwrap().unwrap().samples);
        assert!(h.to_make().is_none(), "a song rendered has nothing to make");
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
    fn an_edit_to_a_song_heard_whole_is_heard_at_the_playhead_in_that_render_first() {
        let mut h = Host::new(1);
        command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 2}));
        h.preview();
        let Some(Ok(before)) = h.render() else { panic!("a whole render") };
        let sr = claudius::config::SR as usize;
        h.set_playhead(30.0);
        command(&mut h, json!({"cmd": "set_mix", "track": "bass", "level_db": 4.0}));
        let heard = h.preview().expect("the change, heard at once");
        assert_eq!(heard.view.url, PREVIEW_URL);
        assert_eq!(heard.samples.len(), before.samples.len(), "the whole song, with the change in it");
        // before the window and after it, the song as it was; in it, the song as it is now
        let at = |sec: f64| (sec * sr as f64) as usize * 2;
        assert_eq!(heard.samples[..at(29.0)], before.samples[..at(29.0)]);
        assert_eq!(heard.samples[at(43.0)..], before.samples[at(43.0)..]);
        let (a, b) = (at(31.0), at(41.0));
        assert_ne!(heard.samples[a..b], before.samples[a..b], "the bass is louder there");
        assert!(h.stale(), "the whole song renders still");
        // which brings the master back to its loudness: the window differs from it by that, under a decibel or so
        let Some(Ok(after)) = h.render() else { panic!("the whole render") };
        let rms = |v: &mut dyn Iterator<Item = f32>| {
            let (s, n) = v.fold((0.0f64, 0usize), |(s, n), x| (s + (x as f64).powi(2), n + 1));
            (s / n.max(1) as f64).sqrt()
        };
        let off = rms(&mut after.samples[a..b].iter().zip(&heard.samples[a..b]).map(|(x, y)| x - y));
        let level = rms(&mut heard.samples[a..b].iter().copied());
        assert!(off < 0.15 * level, "{off} of {level}");
    }

    #[test]
    fn the_demo_mutates_and_evolves_a_song_a_candidate_at_a_time() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let r = command(&mut h, json!({"cmd": "mutate", "count": 2, "keep_best": true}));
        assert!(r.ok && h.breeding(), "{}", r.message);
        assert_eq!(h.state().busy.as_deref(), Some("mutating: 0 of 3 judged"));
        assert!(!command(&mut h, json!({"cmd": "evolve", "generations": 1, "children": 2})).ok, "one at a time");
        let mut judged = 0;
        let said = loop {
            match h.step() {
                Some(reply) => break reply,
                None => judged += 1,
            }
            assert!(judged <= 3, "the song and its two variations");
        };
        assert_eq!(judged, 3);
        assert!(
            said.ok && (said.message.starts_with("kept ") || said.message.contains("it stays as it was")),
            "{}",
            said.message
        );
        let s = h.state();
        assert!(!h.breeding() && s.busy.as_deref().is_none_or(|b| !b.starts_with("mutating")), "{:?}", s.busy);
        let log = s.log.join("\n");
        assert!(log.contains("g1-0") && log.contains("g1-1"), "{log}");
        let kept = h.bp.as_ref().unwrap().lineage.iter().any(|l| l.starts_with("mutated: "));
        assert!(kept || log.contains("no variation beat the song"), "{log}");
        // an evolution stops when the song changes under it, and when it is asked to
        assert!(command(&mut h, json!({"cmd": "evolve", "generations": 2, "children": 2})).ok);
        assert_eq!(h.breeding_command(), Some("evolve"));
        assert!(h.step().is_none());
        assert!(command(&mut h, json!({"cmd": "set_tempo", "bpm": 110})).ok);
        assert_eq!(h.step().map(|r| r.message), Some("evolving stopped: the song changed".to_string()));
        assert!(!h.breeding() && h.step().is_none());
        assert!(command(&mut h, json!({"cmd": "evolve", "generations": 2, "children": 2})).ok);
        assert!(h.step().is_none());
        let r = command(&mut h, json!({"cmd": "cancel"}));
        assert!(r.ok && !h.breeding(), "{}", r.message);
        assert!(!command(&mut h, json!({"cmd": "cancel"})).ok, "nothing to stop");
        // a stop after an edit, before the next step has seen it, keeps the edited song
        assert!(command(&mut h, json!({"cmd": "evolve", "generations": 2, "children": 2})).ok);
        assert!(h.step().is_none());
        assert!(command(&mut h, json!({"cmd": "set_tempo", "bpm": 104})).ok);
        let r = command(&mut h, json!({"cmd": "cancel"}));
        assert_eq!(r.message, "evolving stopped: the song changed");
        assert_eq!(h.bp.as_ref().unwrap().bpm, 104.0, "the edit stays");
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

    #[test]
    fn a_project_holds_the_song_and_the_files_it_plays_with_the_recordings_they_came_from() {
        let mut h = Host::new(1);
        assert!(h.project().is_err(), "no song, no project");
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let (hit, phrase) = ("demo:samples/break/house/amen/kick.wav", "demo:samples/phrase/house/keys/phrase.wav");
        let x: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin()).collect();
        // a hit is kept as floats, louder than a 24-bit file holds
        audio::remember(hit, Stereo::from_mono(&x.iter().map(|v| v * 1.5).collect::<Vec<_>>()));
        audio::remember(phrase, Stereo::from_mono(&x));
        h.cuts.insert(hit.into(), (found("1", "Amen", "by"), true));
        h.cuts.insert(phrase.into(), (found("2", "Keys", "cc0"), false));
        let sample =
            |path: &str| model::Sound::Sample { path: path.into(), root: 60, start: 0.0, end: None, label: "x".into() };
        let bp = h.bp.as_mut().unwrap();
        (bp.drums[0].sound, bp.drums[1].sound, bp.parts[0].sound) = (sample(hit), sample(hit), sample(phrase));

        let songs = std::env::temp_dir().join(format!("claudius-session-project-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&songs);
        let dir = claudius::song::unpack_project(&h.project().unwrap(), "song.zip", &songs).unwrap();
        let opened = claudius::song::read_blueprint(&dir.join("blueprint.json")).unwrap();
        let (kick, keys) =
            (dir.join("samples/break/house/amen/kick.wav"), dir.join("samples/phrase/house/keys/phrase.flac"));
        let at = |s: &model::Sound| match s {
            model::Sound::Sample { path, .. } => std::path::PathBuf::from(path),
            _ => panic!("a sample"),
        };
        assert_eq!([&opened.drums[0].sound, &opened.drums[1].sound].map(at), [kick.clone(), kick.clone()]);
        assert_eq!(at(&opened.parts[0].sound), keys);
        let peak = |p: &std::path::Path| audio::load(p).unwrap().l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((peak(&kick) - 1.5).abs() < 0.01, "the hit is kept as floats: {}", peak(&kick));
        assert!((peak(&keys) - 1.0).abs() < 0.01, "the phrase is kept as 24-bit FLAC: {}", peak(&keys));
        let about = std::fs::read_to_string(dir.join("samples/break/house/amen/kick.wav.json")).unwrap();
        assert!(about.contains("https://freesound.org/s/1/"), "{about}");
        let credits = std::fs::read_to_string(dir.join("credits.txt")).unwrap();
        let lines: Vec<&str> = credits.lines().collect();
        assert!(lines[1].starts_with("\"Amen\" by someone") && lines[2].starts_with("\"Keys\""), "{credits}");
        let _ = std::fs::remove_dir_all(&songs);
    }

    #[test]
    fn a_project_the_page_saved_opens_in_the_page_with_its_recordings_and_their_credits() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        // names of this test's own: the page's recordings are held once for every test
        let (hit, phrase) = ("demo:samples/break/house/opened/hit.wav", "demo:samples/phrase/house/opened/vox.wav");
        let x: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin()).collect();
        audio::remember(hit, Stereo::from_mono(&x.iter().map(|v| v * 1.5).collect::<Vec<_>>()));
        audio::remember(phrase, Stereo::from_mono(&x));
        h.cuts.insert(hit.into(), (found("1", "Amen", "by"), true));
        h.cuts.insert(phrase.into(), (found("2", "Vox", "cc0"), false));
        let sample =
            |path: &str| model::Sound::Sample { path: path.into(), root: 60, start: 0.0, end: None, label: "x".into() };
        let bp = h.bp.as_mut().unwrap();
        (bp.drums[0].sound, bp.parts[0].sound) = (sample(hit), sample(phrase));
        let zip = h.project().unwrap();
        // another song's recording held by the hit's name since, as one undo brings back would be
        audio::remember(hit, Stereo::from_mono(&[0.25; 480]));

        let mut page = Host::new(2);
        let r = page.listen(&Listen::OpenProject { name: "Faded Engine.zip".into() }, zip.clone());
        assert!(r.ok && r.message.starts_with("opened Faded Engine.zip"), "{}", r.message);
        let opened = page.blueprint().unwrap().clone();
        let at = |s: &model::Sound| match s {
            model::Sound::Sample { path, .. } => path.clone(),
            _ => panic!("a sample"),
        };
        // the hit takes a name of its own beside the other song's, which plays on; the phrase, kept as FLAC, is held
        // by the name the project gave it, and plays as it did
        let (hit2, flac) = ("demo:samples/break/house/opened/hit-2.wav", "demo:samples/phrase/house/opened/vox.flac");
        assert_eq!((at(&opened.drums[0].sound), at(&opened.parts[0].sound)), (hit2.to_string(), flac.to_string()));
        let peak = |p: &str| audio::remembered_audio(p).unwrap().l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((peak(flac) - 1.0).abs() < 0.01 && (peak(hit2) - 1.5).abs() < 0.01, "{} {}", peak(flac), peak(hit2));
        assert_eq!(peak(hit), 0.25, "the other song's hit is as it was");
        assert_eq!(page.cuts.get(flac).map(|c| c.0.title.as_str()), Some("Vox"), "what it was cut from comes with it");
        // a project that plays a file it does not hold opens nothing, and holds none of the others
        let (_, files) = claudius::song::read_project(&zip, "song.zip").unwrap();
        let mut lacking = opened.clone();
        (lacking.drums[0].sound, lacking.parts[0].sound) =
            (sample("samples/lacking/hit.wav"), sample("samples/lacking/gone.wav"));
        let bytes = files["samples/break/house/opened/hit.wav"].clone();
        let bad = claudius::song::pack_project(&lacking, &[("samples/lacking/hit.wav".into(), bytes)]).unwrap();
        let r = page.listen(&Listen::OpenProject { name: "lacking.zip".into() }, bad);
        assert!(!r.ok && r.message.contains("plays samples/lacking/gone.wav, which it does not hold"), "{}", r.message);
        assert!(audio::remembered_audio("demo:samples/lacking/hit.wav").is_none(), "nothing of it held");
        assert_eq!(page.blueprint().unwrap(), &opened, "the song open stays");
        // and it saves again as a project that credits both
        let again = page.project().unwrap();
        let (_, files) = claudius::song::read_project(&again, "again.zip").unwrap();
        let credits = String::from_utf8(files["credits.txt"].clone()).unwrap();
        assert!(credits.contains("\"Amen\"") && credits.contains("\"Vox\""), "{credits}");
        let not = page.listen(&Listen::OpenProject { name: "x.zip".into() }, b"not a zip".to_vec());
        assert!(!not.ok && not.message.contains("x.zip is not a zip"), "{}", not.message);
    }

    /// a recording an online library credits, as the page found it
    fn found(id: &str, title: &str, licence: &str) -> Found {
        serde_json::from_value(json!({"id": id, "source": "openverse", "provider": "freesound", "title": title,
            "creator": "someone", "licence": licence, "licence_name": "CC BY 4.0", "page": format!("https://freesound.org/s/{id}/"),
            "licence_url": "https://creativecommons.org/licenses/by/4.0/", "audio": "", "filetype": "mp3", "seconds": 4.0}))
        .unwrap()
    }

    #[test]
    fn a_project_credits_a_scratch_on_a_sample_from_the_library() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3, "bpm": 124})).ok);
        let path = "demo:samples/break/house/funk/break.wav";
        let x: Vec<f32> = (0..6 * 48_000).map(|i| (i as f32 * 0.03).sin() * 0.3).collect();
        audio::remember(path, Stereo::from_mono(&x));
        h.cuts.insert(path.into(), (found("3", "Funky Break", "by"), false));
        h.prepared.push(Prepared {
            path: path.into(),
            role: Role::Break,
            genre: "house".into(),
            bpm: 100.0,
            beats: 8.0,
            key: None,
            minor: false,
            kick: None,
            snare: None,
            hat: None,
            chords: Vec::new(),
            z: None,
            source: "freesound:3".into(),
            asked: None,
            second: None,
            root: None,
            pulse: None,
        });
        h.lib = Some(Arc::new(Library::with_prepared(h.prepared.clone())));
        let on = scratch::samples(h.lib.as_ref().unwrap(), "house", 124.0, 1, |p| h.credited(p));
        assert_eq!(on.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["Funky Break"]);
        let from = h.bp.as_ref().unwrap().form[0].name.clone();
        let moves = json!([{"t": 0.0, "at": 0.0}, {"t": 0.25, "at": 0.3}, {"t": 0.5, "at": 0.0}]);
        let scratch = json!({"cmd": "add_scratch", "from": from, "track": on[0].track(), "bars": 2, "moves": moves});
        let r = command(&mut h, scratch);
        assert!(r.ok, "{}", r.message);

        let songs = std::env::temp_dir().join(format!("claudius-session-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&songs);
        let dir = claudius::song::unpack_project(&h.project().unwrap(), "song.zip", &songs).unwrap();
        let credits = std::fs::read_to_string(dir.join("credits.txt")).unwrap();
        assert!(credits.contains("\"Funky Break\" by someone"), "{credits}");
        let _ = std::fs::remove_dir_all(&songs);
    }

    #[test]
    fn a_project_credits_what_a_scratch_was_cut_from_once_the_song_plays_it_no_more() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let phrase = "demo:samples/phrase/house/scratched/phrase.wav";
        let x: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        audio::remember(phrase, Stereo::from_mono(&x));
        h.cuts.insert(phrase.into(), (found("2", "Keys", "by"), false));
        let bp = h.bp.as_mut().unwrap();
        bp.parts[0].sound =
            model::Sound::Sample { path: phrase.into(), root: 60, start: 0.0, end: None, label: "keys".into() };
        let (keys, from) = (bp.parts[0].name.clone(), bp.form[0].name.clone());
        // a bar the keys play in, which a record of the whole mix holds them in
        let arranged = claudius::compose::arrange(bp);
        let first = arranged.tracks.iter().find(|t| t.name == keys).unwrap().notes.first().map(|n| n.t);
        let bar = (first.expect("the keys play") / 4.0).floor() as u32;
        let moves = json!([{"t": 0.0, "at": 0.0}, {"t": 0.25, "at": 0.3}, {"t": 0.5, "at": 0.0}]);
        let scratch = json!({"cmd": "add_scratch", "from": from, "track": "*", "bar": bar, "moves": moves});
        let r = command(&mut h, scratch);
        assert!(r.ok, "{}", r.message);
        assert!(command(&mut h, json!({"cmd": "remove_track", "name": keys})).ok);

        let songs = std::env::temp_dir().join(format!("claudius-session-scratch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&songs);
        let dir = claudius::song::unpack_project(&h.project().unwrap(), "song.zip", &songs).unwrap();
        let credits = std::fs::read_to_string(dir.join("credits.txt")).unwrap();
        assert!(credits.contains("\"Keys\" by someone"), "{credits}");
        // opened in claudius, the take credits it from what is kept beside it
        let mut opened = claudius::song::read_blueprint(&dir.join("blueprint.json")).unwrap();
        claudius::take::find_moved(&mut opened, &dir);
        let credits = claudius::song::credits(&claudius::compose::arrange(&opened));
        assert!(credits.contains("\"Keys\" by someone"), "{credits}");
        let _ = std::fs::remove_dir_all(&songs);
    }

    /// a file as a page hands one over: `x` as a WAV's bytes
    fn wav(x: &Stereo) -> Vec<u8> {
        claudius::export::encode(x, &"wav".parse().unwrap(), &claudius::export::tags("recording")).unwrap()
    }

    /// four bars of a beat at `bpm`: a kick falling from 150 Hz on one and three, a burst of noise on two and four
    fn a_beat(bpm: f64) -> Stereo {
        let (sr, spb) = (claudius::config::SRF, 60.0 / bpm);
        let mut x = Stereo::zeros((16.0 * spb * sr) as usize + 24_000);
        let mut noise = Rng::new(5);
        for b in 0..16 {
            let at = (b as f64 * spb * sr) as usize;
            let mut ph = 0.0;
            for i in 0..14_400 {
                let t = i as f64 / sr;
                let v = if b % 2 == 0 {
                    ph += std::f64::consts::TAU * (50.0 + 100.0 * (-t / 0.03).exp()) / sr;
                    0.9 * ph.sin() * (-t / 0.15).exp()
                } else {
                    0.5 * (noise.f32() as f64 * 2.0 - 1.0) * (-t / 0.06).exp()
                };
                x.l[at + i] += v as f32;
                x.r[at + i] += v as f32;
            }
        }
        x
    }

    /// sixteen bars at 96 BPM in A minor, `i VI III VII` a bar each as sine triads, a kick on one and three and a noise
    /// snare on two and four, quiet for eight bars and loud for eight more
    fn a_song() -> Stereo {
        let bar = 2.5;
        let n = (16.0 * bar * claudius::config::SRF) as usize;
        let mut x = Stereo::zeros(n);
        let roots = [57.0, 53.0, 48.0, 55.0];
        let thirds = [3.0, 4.0, 4.0, 4.0];
        let mut noise = Rng::new(9);
        for i in 0..n {
            let t = i as f64 / claudius::config::SRF;
            let b = (t / bar) as usize;
            let (root, third) = (roots[b % 4], thirds[b % 4]);
            let loud = if b >= 8 { 1.0 } else { 0.45 };
            let tone = |m: f64| (std::f64::consts::TAU * 440.0 * 2f64.powf((m - 69.0) / 12.0) * t).sin();
            let mut v = 0.12 * loud * (tone(root) + tone(root + third) + tone(root + 7.0));
            let beat = t / (bar / 4.0);
            let (k, into) = (beat as usize % 4, beat.fract() * bar / 4.0);
            if (k == 0 || k == 2) && into < 0.12 {
                v += 0.6 * (std::f64::consts::TAU * 55.0 * into).sin() * (1.0 - into / 0.12);
            }
            if (k == 1 || k == 3) && into < 0.08 {
                v += 0.3 * loud * noise.range(-1.0, 1.0) * (1.0 - into / 0.08);
            }
            x.l[i] = v as f32;
            x.r[i] = v as f32;
        }
        x
    }

    #[test]
    fn a_take_recorded_on_the_page_is_kept_in_memory_heard_and_turned_into_drums() {
        use crate::listen::Listen;
        let mut h = Host::new(1);
        let take = |from: &str, role: &str| Listen::Take {
            name: "take.wav".into(),
            from: from.into(),
            track: None,
            role: role.into(),
            recording: false,
            found: None,
        };
        assert_eq!(take("intro", "vocal").what(), "take");
        let added = Listen::Take {
            name: "a song.m4a".into(),
            from: "intro".into(),
            track: None,
            role: "vocal".into(),
            recording: true,
            found: None,
        };
        assert_eq!(added.what(), "add_recording", "the Add a recording window's answer is its own");
        let heard = Listen::Analyze { name: "a song.m4a".into(), recording: true };
        assert_eq!(heard.what(), "hear_recording", "and so is what it hears in one");
        let r = h.listen(&take("intro", "vocal"), wav(&a_song()));
        assert!(!r.ok && r.message.starts_with("no song yet"), "{}", r.message);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let from = h.blueprint().unwrap().form[0].name.clone();
        // sung: A4 held for four seconds
        let n = (4.0 * claudius::config::SRF) as usize;
        let mut sung = Stereo::zeros(n);
        for i in 0..n {
            let v = 0.3 * (std::f64::consts::TAU * 440.0 * i as f64 / claudius::config::SRF).sin() as f32;
            (sung.l[i], sung.r[i]) = (v, v);
        }
        let r = h.listen(&take(&from, "vocal"), wav(&sung));
        assert!(r.ok, "{}", r.message);
        let bp = h.blueprint().unwrap().clone();
        let kept = bp.loops.iter().find(|l| l.name == "take1").expect("the take plays in the song");
        let model::Sound::Loop { path, .. } = &kept.sound else { panic!("a loop") };
        assert!(path.starts_with("demo:takes/") && audio::remembered_audio(path).is_some(), "{path}");
        let r = h.command(Command::AssessVoice { take: Some("take1".into()), path: None, key: None });
        assert!(r.ok && r.message.contains("A4"), "{}", r.message);
        let melody = Command::TranscribeTake {
            take: "take1".into(),
            sounds: vec!["pluck".into()],
            name: None,
            role: None,
            chromatic: true,
            keep_take: true,
            style: None,
        };
        let r = h.command(melody);
        assert!(r.ok, "{}", r.message);
        let r = command(&mut h, json!({"cmd": "tune", "track": "take1", "speed_ms": 15}));
        assert!(r.ok && !r.message.contains("pitch check"), "no check yet, so no voice: {}", r.message);
        // trimmed to its middle two seconds, kept in memory as the take was
        let r = command(&mut h, json!({"cmd": "trim_take", "track": "take1", "start": 1.0, "end": 3.0}));
        assert!(r.ok && r.message.starts_with("take1: 1.00 to 3.00 s of its recording"), "{}", r.message);
        let bp = h.blueprint().unwrap().clone();
        let model::Sound::Loop { path, .. } = &bp.loops.iter().find(|l| l.name == "take1").unwrap().sound else {
            panic!("a loop")
        };
        let cut = audio::remembered_audio(path).expect("the cut is kept in memory");
        assert!(path.starts_with("demo:takes/") && (cut.secs() - 2.0).abs() < 1e-3, "{path}");
        // played: a beat at the song's tempo, whose kicks and snares become lanes
        let bpm = h.blueprint().unwrap().bpm;
        let r = h.listen(&take(&from, "drums"), wav(&a_beat(bpm)));
        assert!(r.ok, "{}", r.message);
        let drums = Command::TranscribeDrums {
            take: Some("take2".into()),
            path: None,
            bars: Some(2),
            variant: None,
            bpm: None,
            keep_take: false,
        };
        let r = h.command(drums);
        assert!(r.ok, "{}", r.message);
        let none =
            Command::TranscribeDrums { take: None, path: None, bars: None, variant: None, bpm: None, keep_take: false };
        assert!(!h.command(none).ok);
    }

    #[test]
    fn pieces_of_a_recording_chosen_on_the_page_are_offered_to_hear_drawn_again_and_kept() {
        use crate::listen::Listen;
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "trip_hop", "seed": 1})).ok);
        // sixteen seconds at 120 BPM: a hat to the left, and a sung A3 in the middle from 4 s to 12 s
        let sr = claudius::config::SR as usize;
        let mut x = Stereo::zeros(16 * sr);
        let mut phase = 0f64;
        for i in 0..x.len() {
            if i % (sr / 4) < sr / 50 {
                x.l[i] += if i % 7 < 3 { 0.3 } else { -0.3 };
            }
            if (4 * sr..12 * sr).contains(&i) {
                phase += std::f64::consts::TAU * 220.0 / claudius::config::SRF;
                let v = (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15) as f32;
                (x.l[i], x.r[i]) = (x.l[i] + v, x.r[i] + v);
            }
        }
        let found = json!({
            "id": "ccmixter:71182", "source": "ccmixter", "provider": "ccmixter", "title": "Hum",
            "creator": "Someone", "licence": "by", "licence_name": "CC BY 4.0",
            "licence_url": "https://creativecommons.org/licenses/by/4.0/", "page": "https://ccmixter.org/files/a/71182",
            "audio": "https://example.org/hum.mp3", "filetype": "mp3", "seconds": 16.0
        });
        let offer = Listen::SamplePieces { name: "hum.wav".into(), seed: None, count: Some(1), found: Some(found) };
        assert_eq!(offer.what(), "sample_pieces");
        let r = h.listen(&offer, wav(&x));
        assert!(r.ok && r.message.starts_with("1 piece of hum.wav, take 0"), "{}", r.message);
        let pieces = h.state().pieces.expect("the studio shows the pieces");
        let p = &pieces.pieces[0];
        for url in [&p.raw, &p.treated] {
            let bytes = crate::listen::mp3_of(url).expect("a file to hear, in its URL").unwrap();
            // a bar at least, two seconds at 120 BPM
            assert!(crate::listen::decode(bytes, "piece.mp3").unwrap().secs() > 1.5);
        }
        assert!(command(&mut h, json!({"cmd": "redraw_pieces"})).ok);
        assert_eq!(h.state().pieces.unwrap().seed, 1);
        let r = command(&mut h, json!({"cmd": "keep_pieces", "keep": [{"id": 9}]}));
        assert!(
            !r.ok && r.message.contains("no piece 9 (pieces 1 to 1)") && h.state().pieces.is_some(),
            "{}",
            r.message
        );
        let r = command(&mut h, json!({"cmd": "keep_pieces", "keep": [{"id": 1, "treated": true}]}));
        let said: Vec<&str> = r.message.lines().collect();
        assert!(
            r.ok && said[0].starts_with("sampled 1 phrase of ") && said[0].ends_with(" into the song: vox1"),
            "{said:?}"
        );
        assert!(said[1].starts_with("vox1:"), "{said:?}");
        assert!(h.state().pieces.is_none(), "kept, the offer goes");
        let bp = h.blueprint().unwrap().clone();
        let vox = bp.loops.iter().find(|l| l.name == "vox1").unwrap();
        let model::Sound::Loop { path, .. } = &vox.sound else { panic!("a recording") };
        assert!(vox.mix.fx.len() > 1, "with its treatment");
        let credited = h.credited(std::path::Path::new(path));
        assert_eq!(credited.recordings.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["ccmixter:71182"]);
    }

    #[test]
    fn chords_under_a_melody_go_in_where_it_plays_on_its_own_sound() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let r = command(&mut h, json!({"cmd": "harmonize", "part": "lead"}));
        assert!(
            r.ok && r.message.contains("lead_chords: a chord under each of its notes under lead, in "),
            "{}",
            r.message
        );
        let bp = h.blueprint().unwrap().clone();
        let part = |name: &str| bp.parts.iter().find(|p| p.name == name).unwrap();
        let (chords, lead) = (part("lead_chords"), part("lead"));
        assert_eq!((chords.harmonizes.as_deref(), chords.style.as_str()), (Some("lead"), "block"));
        let preset = |s: &model::Sound| match s {
            model::Sound::Synth { preset, .. } => preset.clone(),
            model::Sound::Instrument { name, .. } => name.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!(preset(&chords.sound), preset(&lead.sound), "block chords sound as the melody does");
        for s in &bp.form {
            let on = |name: &str| s.layers.iter().any(|l| l == name);
            assert_eq!(on("lead_chords"), on("lead"), "{} plays the chords where it plays the lead", s.name);
        }
        let r = command(&mut h, json!({"cmd": "harmonize", "part": "lead", "style": "sustain"}));
        assert!(r.ok && r.message.contains("lead_chords2: sustain chords under lead"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "harmonize", "part": "nope"}));
        assert!(!r.ok && r.message.contains("no part 'nope' to harmonize"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "harmonize", "part": "lead", "style": "motif"}));
        assert!(!r.ok && r.message.contains("chords under a melody play in one of block"), "{}", r.message);
    }

    #[test]
    fn a_track_is_written_in_another_genres_way_and_varied_and_each_undoes() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let r = command(&mut h, json!({"cmd": "generate_track", "what": "bass", "genre": "reggae"}));
        assert!(r.ok && r.message.starts_with("reggae_bass: reggae's bass, "), "{}", r.message);
        let style = |h: &Host, name: &str| {
            h.blueprint().unwrap().parts.iter().find(|p| p.name == name).map(|p| p.style.clone())
        };
        let reggae = genres::get("reggae").unwrap();
        assert!(
            reggae
                .parts
                .iter()
                .any(|p| p.role == model::Role::Bass && p.styles.contains(&style(&h, "reggae_bass").unwrap()))
        );
        // another take of the same, and a variation of it, each undone
        let seed = |h: &Host| h.blueprint().unwrap().parts.iter().find(|p| p.name == "reggae_bass").unwrap().seed;
        let was = seed(&h);
        assert!(command(&mut h, json!({"cmd": "vary_track", "track": "reggae_bass"})).ok);
        assert!(seed(&h) != was && seed(&h) >> 32 == was >> 32, "the same take's rhythm");
        assert!(command(&mut h, json!({"cmd": "undo"})).ok);
        assert_eq!(seed(&h), was);
        // reggaeton's drums in place of the song's lanes of their names
        let r = command(&mut h, json!({"cmd": "generate_track", "what": "drums", "genre": "reggaeton"}));
        assert!(r.ok && r.message.starts_with("reggaeton's drums"), "{}", r.message);
        assert!(h.blueprint().unwrap().drums.iter().any(|d| d.name == "snare"));
        let r = command(&mut h, json!({"cmd": "generate_track", "what": "tuba"}));
        assert!(!r.ok && r.message.contains("not 'tuba'"), "{}", r.message);
    }

    #[test]
    fn a_sound_swaps_for_the_next_that_fits_and_the_whole_song_for_new_ones() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let sound = |h: &Host, name: &str| claudius::swap::playing(h.blueprint().unwrap(), name);
        let view = |h: &Host, name: &str| {
            let song = crate::views::song(h.blueprint().unwrap(), &Default::default());
            song.tracks.into_iter().find(|t| t.name == name).unwrap()
        };
        // the bass onto the sound its view says comes next, in the song's script, and back with undo
        let before = view(&h, "bass");
        let next = before.next_swap.clone().expect("the bass has another sound");
        assert!(before.swaps.iter().any(|c| c.name == next) && before.playing.as_ref() != Some(&next));
        let r = command(&mut h, json!({"cmd": "swap_sound", "track": "bass"}));
        assert!(r.ok && r.message.contains(&format!("→ {next}")), "{}", r.message);
        assert_eq!(sound(&h, "bass").as_deref(), Some(next.as_str()));
        assert!(script::to_script(h.blueprint().unwrap()).contains(&format!("preset: \"{next}\"")));
        assert!(command(&mut h, json!({"cmd": "undo"})).ok);
        assert_eq!(sound(&h, "bass"), before.playing);
        // a drum lane onto a sound named
        assert!(command(&mut h, json!({"cmd": "swap_sound", "track": "kick", "to": "bd:2"})).ok);
        assert_eq!(sound(&h, "kick").as_deref(), Some("bd:2"));
        // block chords under the lead keep sounding as it does
        assert!(command(&mut h, json!({"cmd": "harmonize", "part": "lead"})).ok);
        assert!(command(&mut h, json!({"cmd": "swap_sound", "track": "lead"})).ok);
        assert_eq!(sound(&h, "lead_chords"), sound(&h, "lead"));
        // the whole song: every part another sound, no two the same, and the drums another kit
        let parts: Vec<String> = h.blueprint().unwrap().parts.iter().map(|p| p.name.clone()).collect();
        let was: Vec<Option<String>> = parts.iter().map(|p| sound(&h, p)).collect();
        let r = command(&mut h, json!({"cmd": "swap_sound"}));
        assert!(r.ok && r.message.contains("new sounds: the drums"), "{}", r.message);
        let now: Vec<Option<String>> = parts.iter().map(|p| sound(&h, p)).collect();
        for (i, p) in parts.iter().enumerate().filter(|(_, p)| *p != "lead_chords") {
            assert_ne!(now[i], was[i], "{p} swapped");
        }
        let bp = h.blueprint().unwrap();
        let own: Vec<String> = bp
            .parts
            .iter()
            .filter(|p| p.name != "lead_chords")
            .flat_map(|p| claudius::palette::identities(&p.sound))
            .collect();
        assert_eq!(own.iter().collect::<std::collections::BTreeSet<_>>().len(), own.len(), "{own:?}");
        assert_eq!(sound(&h, "lead_chords"), sound(&h, "lead"));
        assert_eq!(sound(&h, "kick").as_deref(), Some("bd:0"), "without packs, the drums' own voices as they are");
        // what it cannot swap
        let r = command(&mut h, json!({"cmd": "swap_sound", "track": "nope"}));
        assert!(!r.ok && r.message.contains("no part or drum lane 'nope'"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "swap_sound", "only": "vocals"}));
        assert!(!r.ok && r.message.contains("swaps the \"drums\" or the \"parts\", not 'vocals'"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "swap_sound", "only": "drums", "to": "nope"}));
        assert!(!r.ok && r.message.contains("no kit 'nope' (kits: synthesized)"), "{}", r.message);
    }

    #[test]
    fn a_take_a_free_library_offered_credits_it() {
        use crate::listen::Listen;
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let from = h.blueprint().unwrap().form[0].name.clone();
        // the library's record of it, as the page hands it over (`LibraryHit::found`)
        let found = json!({
            "id": "freesound:629052", "source": "openverse", "provider": "freesound", "title": "Rain",
            "creator": "Someone", "licence": "by", "licence_name": "CC BY 4.0",
            "licence_url": "https://creativecommons.org/licenses/by/4.0/", "page": "https://freesound.org/s/629052/",
            "audio": "https://example.org/rain.ogg", "filetype": "ogg", "seconds": 4.8
        });
        let take = Listen::Take {
            name: "rain".into(),
            from,
            track: Some("rain".into()),
            role: "texture".into(),
            recording: true,
            found: Some(found),
        };
        assert!(h.listen(&take, wav(&a_song())).ok);
        let bp = h.blueprint().unwrap().clone();
        let model::Sound::Loop { path, .. } = &bp.loops.iter().find(|l| l.name == "rain").unwrap().sound else {
            panic!("a recording")
        };
        let credited = h.credited(std::path::Path::new(path));
        assert_eq!(credited.recordings.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["freesound:629052"]);
    }

    #[test]
    fn a_pitch_check_asked_for_on_the_page_is_sung_back_heard_and_kept_as_the_voice() {
        use crate::listen::Listen;
        let mut h = Host::new(1);
        let r = h.listen(&Listen::PitchCheck { id: 1 }, wav(&a_song()));
        assert!(!r.ok && r.message.starts_with("no pitch check 1"), "{}", r.message);
        let r = command(&mut h, json!({"cmd": "request_pitch_check", "low": 60, "high": 62}));
        assert!(!r.ok && r.message.contains("at least a fifth apart"), "{}", r.message);
        assert!(command(&mut h, json!({"cmd": "request_pitch_check", "note": "for a melody in your range"})).ok);
        let view = h.state().pitch_check.expect("the studio shows the check");
        assert_eq!((view.id, view.status, view.notes.len()), (1, TakeStatus::Waiting, 24));
        // sung back as played, a beat to each note in the gap after its phrase
        let (sr, beat) = (claudius::config::SRF, 60.0 / view.bpm);
        let mut sung = Stereo::zeros((view.beats * beat * sr) as usize);
        for (phrase, (from, _)) in view.notes.chunks(4).zip(&view.answers) {
            for (k, n) in phrase.iter().enumerate() {
                let (at, hz) = (((from + k as f64) * beat * sr) as usize, 440.0 * 2f64.powf((n.p - 69) as f64 / 12.0));
                for i in 0..(0.9 * beat * sr) as usize {
                    let v = 0.3 * (std::f64::consts::TAU * hz * i as f64 / sr).sin() as f32;
                    (sung.l[at + i], sung.r[at + i]) = (v, v);
                }
            }
        }
        let r = h.listen(&Listen::PitchCheck { id: 1 }, wav(&sung));
        assert!(r.ok, "{}", r.message);
        let s = h.state();
        assert_eq!(s.pitch_check.map(|c| (c.status, c.answer.is_some())), Some((TakeStatus::Taken, true)));
        assert!(s.voice.is_some(), "the voice it heard is the person's");
        // a take tuned from now on sings as the check heard the person
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 3})).ok);
        let from = h.blueprint().unwrap().form[0].name.clone();
        let take = Listen::Take {
            name: "take.wav".into(),
            from,
            track: None,
            role: "vocal".into(),
            recording: false,
            found: None,
        };
        assert!(h.listen(&take, wav(&sung)).ok);
        let r = command(&mut h, json!({"cmd": "tune", "track": "take1"}));
        assert!(r.ok && r.message.ends_with("as your pitch check heard you sing"), "{}", r.message);
        let r = h.listen(&Listen::PitchCheck { id: 1 }, wav(&sung));
        assert!(!r.ok && r.message.contains("answered already"), "{}", r.message);
        // asked for again, and not sung after all
        assert!(command(&mut h, json!({"cmd": "request_pitch_check"})).ok);
        assert!(command(&mut h, json!({"cmd": "cancel_pitch_check", "id": 2})).ok);
        assert_eq!(h.state().pitch_check.map(|c| c.status), Some(TakeStatus::Cancelled));
    }

    #[test]
    fn a_song_like_a_recording_waits_for_one_to_be_analyzed() {
        let mut h = Host::new(1);
        let r = command(&mut h, json!({"cmd": "song_like", "genre": "house"}));
        assert!(!r.ok && r.message.starts_with("analyze a recording first"), "{}", r.message);
        assert!(h.blueprint().is_none());
    }

    #[test]
    fn a_recording_chosen_on_the_page_is_heard_reworked_flipped_and_grooved_in_memory() {
        use crate::listen::Listen;
        let mut h = Host::new(1);
        let bytes = wav(&a_song());
        let name = || "my song.wav".to_string();
        let r = h.listen(&Listen::Analyze { name: name(), recording: false }, bytes.clone());
        assert!(r.ok && r.message.contains(" BPM (pulse clarity"), "{}", r.message);
        // what was heard goes to the page with the answer, once, with nothing of the recording kept: the page plays
        // the file it has
        let heard = h.take_listened().expect("what was heard, as the studio shows it");
        assert!(h.take_listened().is_none(), "it goes once");
        assert_eq!((heard.name.as_str(), heard.audio.as_ref()), ("my song.wav", None));
        assert!(heard.bars.len() >= 8 && !heard.sections.is_empty() && heard.kick.len() == 16, "{heard:?}");
        assert!(heard.bars.windows(2).all(|w| w[0].start < w[1].start) && heard.end > heard.bars[0].start);
        let r = command(&mut h, json!({"cmd": "song_like", "genre": "techno", "seed": 5}));
        assert!(r.ok && r.message.starts_with("a techno song like my song.wav: at "), "{}", r.message);
        let like = h.blueprint().unwrap();
        assert_eq!((like.genre.as_str(), like.key.name()), ("techno", heard.key.clone()));
        // reworked: the recording plays all through, kept in memory, and goes in the project the page saves
        let remake = |flip, rebuild| Listen::Remake {
            name: name(),
            genre: Some("boom_bap".into()),
            seed: Some(3),
            flip,
            rebuild,
        };
        let r = h.listen(&remake(false, false), bytes.clone());
        assert!(r.ok && r.message.starts_with("reworked my song.wav as boom_bap"), "{}", r.message);
        let bp = h.blueprint().unwrap().clone();
        let original = bp.loops.iter().find(|l| l.name == "original").expect("the recording plays");
        let model::Sound::Loop { path, .. } = &original.sound else { panic!("a loop") };
        assert!(path.starts_with("demo:takes/my-song/") && audio::remembered_audio(path).is_some(), "{path}");
        assert!(h.render().is_some_and(|r| r.is_ok()), "it renders");
        let songs = std::env::temp_dir().join(format!("claudius-session-rework-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&songs);
        let dir = claudius::song::unpack_project(&h.project().unwrap(), "song.zip", &songs).unwrap();
        let kept = std::fs::read_dir(dir.join("takes/my-song")).unwrap().count();
        assert_eq!(kept, 1, "the recording, in the project");
        let _ = std::fs::remove_dir_all(&songs);
        // flipped, when asked
        let r = h.listen(&remake(true, false), bytes.clone());
        assert!(r.ok && r.message.starts_with("remixed my song.wav as boom_bap"), "{}", r.message);
        assert!(h.blueprint().unwrap().loops.iter().any(|l| l.name == "flip"));
        // rebuilt, when asked: without the ears, its drums' habits and the genre's bass and chords on its chords
        let r = h.listen(&remake(false, true), bytes.clone());
        assert!(r.ok && r.message.starts_with("rebuilt my song.wav as boom_bap"), "{}", r.message);
        assert!(r.message.contains("rebuilt from 'my song.wav': its "), "{}", r.message);
        assert!(h.blueprint().unwrap().loops.is_empty() && h.render().is_some_and(|r| r.is_ok()));
        // its groove, for the song's drums
        let r = h.listen(&Listen::ExtractGroove { name: name() }, bytes);
        assert!(r.ok && r.message.starts_with("groove my_song, heard in my song.wav"), "{}", r.message);
        // and a file that is no recording says so
        let r = h.listen(&Listen::Analyze { name: "notes".into(), recording: false }, b"hello".to_vec());
        assert!(!r.ok && r.message.starts_with("send the recording as the file itself"), "{}", r.message);
    }

    #[test]
    fn a_song_its_genre_would_make_too_long_keeps_the_genre_it_is_in() {
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "lofi", "seed": 5})).ok);
        // 288 bars of lofi, which drum and bass, at twice its tempo, would play as 576
        let long = format!("form([{}]);", [r#"sec("verse", 32, "*")"#; 9].join(", "));
        assert!(command(&mut h, json!({"cmd": "run_script", "code": long})).ok);
        let r = command(&mut h, json!({"cmd": "change_genre", "genre": "drum_and_bass", "seed": 2}));
        assert!(!r.ok && r.message.contains("the limit is 512"), "{}", r.message);
        assert_eq!(h.blueprint().unwrap().genre, "lofi");
    }

    #[test]
    fn a_recordings_vocals_chosen_on_the_page_are_sampled_into_the_song_in_memory() {
        use crate::listen::Listen;
        let mut h = Host::new(1);
        assert!(command(&mut h, json!({"cmd": "new_song", "genre": "house", "seed": 5})).ok);
        let short =
            r#"form([sec("intro", 4, "*"), sec("drop", 8, "*"), sec("break", 4, "pad"), sec("drop", 8, "*")]);"#;
        assert!(command(&mut h, json!({"cmd": "run_script", "code": short})).ok);
        // sixteen seconds: a sung A3 from the fourth second to the twelfth, a hat to the left all through
        let (sr, n) = (claudius::config::SR as usize, 16 * claudius::config::SR as usize);
        let mut x = Stereo::zeros(n);
        let mut phase = 0f64;
        for i in 0..n {
            if i % (sr / 4) < sr / 50 {
                x.l[i] += if i % 2 == 0 { 0.2 } else { -0.2 };
            }
            if (4 * sr..12 * sr).contains(&i) {
                let t = i as f64 / sr as f64;
                phase +=
                    std::f64::consts::TAU * 220.0 * (1.0 + 0.01 * (t * 5.0 * std::f64::consts::TAU).sin()) / sr as f64;
                let v = (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15) as f32;
                x.l[i] += v;
                x.r[i] += v;
            }
        }
        let sampling = |how: Option<&str>, every: Option<&str>| Listen::SampleSong {
            name: "vocal.wav".into(),
            found: None,
            how: how.map(str::to_string),
            sections: Vec::new(),
            every: every.map(str::to_string),
        };
        let refused = h.listen(&sampling(None, Some("3")), wav(&x));
        assert!(!refused.ok && refused.message.contains('3'), "{}", refused.message);
        let r = h.listen(&sampling(None, None), wav(&x));
        let said: Vec<&str> = r.message.lines().collect();
        assert!(
            r.ok && said[0].starts_with("sampled ") && said[0].contains(" of vocal.wav into the song: vox1"),
            "{said:?}"
        );
        assert!(said[1].starts_with("vox1: bars ") && said[1].contains("% sung"), "{said:?}");
        let bp = h.blueprint().unwrap();
        let vox = bp.loops.iter().find(|l| l.name == "vox1").expect("the phrase plays");
        let (model::Sound::Loop { path, .. } | model::Sound::Sample { path, .. }) = &vox.sound else {
            panic!("a recording: {:?}", vox.sound)
        };
        assert!(path.starts_with("demo:takes/") && audio::remembered_audio(path).is_some(), "{path}");
        // the page samples it the other ways too, and says which way it does not know
        let chop = |how: &str| sampling(Some(how), None);
        let r = h.listen(&chop("chops"), wav(&x));
        assert!(r.ok && r.message.starts_with("chopped vocal.wav into the song: chops"), "{}", r.message);
        let r = h.listen(&chop("slices"), wav(&x));
        assert!(!r.ok && r.message.starts_with("no way 'slices'"), "{}", r.message);
    }
}
