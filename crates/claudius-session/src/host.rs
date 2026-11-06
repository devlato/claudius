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
            cuts: BTreeMap::new(),
            takes: BTreeMap::new(),
            failed: None,
            seeds: Rng::new(seed),
            kept: render::Kept::default(),
            last: None,
            playhead: 0.0,
            breeding: None,
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
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            undo_label: self.undo.last().map(|(_, l)| l.clone()),
            redo_label: self.redo.last().map(|(_, l)| l.clone()),
            log: self.log.clone(),
            genres: views::genre_infos(),
            exports: views::export_formats(),
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
            Command::Render { .. } if self.stale() => self.ok("rendering"),
            Command::Render { .. } => self.ok("the render is up to date"),
            Command::AddScratch { from, track, bar, bars, moves, name, role } => {
                self.add_scratch(&from, &edit::scratch(track, bar, bars, &moves), name, role.as_deref())
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
        let r = match render::render_kept(&project, &render::Options::default(), &mut self.kept) {
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

/// a command's name, as the API tags it (`set_tempo`)
fn name(c: &Command) -> String {
    serde_json::to_value(c).ok().and_then(|v| v["cmd"].as_str().map(String::from)).unwrap_or_default()
}

/// where a file a session holds goes in a project, which the blueprint names it by: a sample cut from a recording at
/// the session's own name for it (`demo:samples/break/house/...` at `samples/break/house/...`), anything else (a
/// scratch take) in `takes/`; a `.wav` when it keeps floats, else a `.flac`
fn project_path(path: &str, float: bool) -> String {
    let ext = if float { "wav" } else { "flac" };
    match path.strip_prefix("demo:") {
        Some(rest) => format!("{}.{ext}", rest.strip_suffix(".wav").unwrap_or(rest)),
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
}
