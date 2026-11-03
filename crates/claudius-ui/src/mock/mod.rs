//! A stand-in for the claudius server, so the studio can be used (and developed) without one: a generated song held
//! in memory, edits applied with the same logic the studio uses for optimistic updates, undo / redo with labels, and
//! the jobs simulated with progress and log lines. Like the server it renders after every change (a rough preview,
//! synthesized a slice per frame) and keeps `song_version`, so stale renders, solo and history behave the same.

mod kit;
mod sample;
pub(crate) mod song;
mod synth;

use std::sync::Arc;

use claudius_api::{Command, ReleaseCheck, ReleaseDetails, ReleaseView, RenderView, Reply, SongView, State};

use crate::audio::Pcm;
use crate::edits;
use crate::net::{Event, command_name};

/// xorshift; deterministic per seed
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn f32(&mut self) -> f32 {
        self.f64() as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

enum Job {
    Render {
        synth: Box<synth::Render>,
        stems: bool,
        version: u64,
        after: Option<String>,
    },
    Mutate {
        count: usize,
        keep_best: bool,
        started: f64,
    },
    Evolve {
        generations: usize,
        children: usize,
        started: f64,
        reported: usize,
    },
    /// gathering the samples a new song is built on, then making it
    Sample(Box<Sampling>),
}

/// a new song of a genre built on samples, waiting for them
struct Sampling {
    gathering: sample::Gathering,
    /// when it began: a gathering waits a minute at most
    started: f64,
    rng: Rng,
    genre: String,
    seed: u64,
    bpm: Option<f64>,
    key: Option<String>,
    loops: bool,
}

pub struct Mock {
    state: State,
    /// earlier songs and what changed them (the newest last)
    undo: Vec<(SongView, String)>,
    redo: Vec<(SongView, String)>,
    job: Option<Job>,
    rng: Rng,
    events: Vec<Event>,
    audio: Option<(u64, Arc<Pcm>)>,
    now: f64,
    last_progress: f64,
    started: bool,
    /// when the render after a change starts (quick successive edits make one render)
    render_at: Option<f64>,
    stems_wanted: bool,
    /// the song changed since the last published state
    song_changed: bool,
    /// the demo's sounds, arriving while it runs
    kit: kit::Kit,
    /// what finds and fetches the samples a genre built on them plays
    sampler: sample::Sampler,
    /// how many times sounds have arrived, and how many had when the last render started
    kit_arrivals: usize,
    rendered_with: usize,
    /// the last render played the synth for something the kit plays: render again once more of it arrives
    stand_in: bool,
    /// renders wait for the sounds they play, since then; kept while the synth stands in, so that what comes later
    /// is mixed in as it comes instead of waited for again
    waiting_since: Option<f64>,
}

const MUTATE_SECS: f64 = 2.2;
const CHILD_SECS: f64 = 0.45;
const AUTO_RENDER_DELAY: f64 = 0.35;
/// how long a render waits for the demo's sounds before the synth stands in for what has not arrived
const KIT_WAIT: f64 = 10.0;
/// how long a new song waits for its samples before it is made with what has come
const SAMPLE_WAIT: f64 = 60.0;
/// voice-samples a frame renders: a few milliseconds' work in a browser
const RENDER_BUDGET: usize = 400_000;
const AUDIO: [&str; 7] = [".wav", ".mp3", ".flac", ".aif", ".aiff", ".m4a", ".ogg"];
const REFERENCE: &str = "# claudius script (demo)\n\nThe demo has no script engine: run `claudius serve` and the studio shows the real reference here \
(`GET /api/reference`).\n\nA claudius script is [Rhai](https://rhai.rs) with Strudel-style patterns:\n\n| | |\n|---|---|\n| `tempo(124)` | BPM 40..240 |\n\
| `key(\"A minor\")` | the key; progressions follow |\n| `prog(\"A\", \"i7 VImaj7 iv7 v7\")` | a progression in Roman numerals |\n\
| `mix(\"lead\", #{ level: -3, solo: true })` | mix a track |\n| `track(\"hats\", s(\"[~ hh]*4\").gain(0.6))` | a pattern track |\n\n\
```js\ntrack(\"keys\", chord(\"prog\").voicing().s(\"rhodes\"))\ns(\"bd*4\").bank(\"tr909\")\n```\n";

/// the demo's release: what `claudius publish` asks, and a note that nothing is checked here
fn demo_release() -> ReleaseView {
    let made = [
        ("written", "you wrote it: the notes, the parts and the arrangement, with claudius as the instrument"),
        ("assisted", "written with help: an assistant or claudius' generator wrote some of it, and you shaped it"),
        ("generated", "an assistant or claudius' generator wrote it"),
    ];
    ReleaseView {
        details: ReleaseDetails { genre: "Dance".into(), subgenre: "House".into(), ..Default::default() },
        checks: vec![ReleaseCheck {
            level: "note".into(),
            about: "details".into(),
            message: "the demo checks nothing and writes no files: start `claudius serve` to publish".into(),
        }],
        made_options: made.iter().map(|(m, d)| (m.to_string(), d.to_string())).collect(),
        ..Default::default()
    }
}

impl Mock {
    /// `ctx` is woken when sounds arrive
    pub fn new(ctx: Option<eframe::egui::Context>) -> Mock {
        let mut song = song::generate("house", 7, None, Some("D minor"), false);
        synth::write_cycles(&mut song);
        let state = State {
            version: 1,
            song_version: 1,
            song: Some(song),
            genres: song::genre_infos(),
            log: vec![
                "claudius demo: it all runs in the studio, and nothing is kept".into(),
                "generated: house seed 7".into(),
            ],
            release: Some(demo_release()),
            ..Default::default()
        };
        Mock {
            state,
            undo: Vec::new(),
            redo: Vec::new(),
            job: None,
            rng: Rng::new(42),
            events: Vec::new(),
            audio: None,
            now: 0.0,
            last_progress: 0.0,
            started: false,
            render_at: Some(0.0),
            stems_wanted: false,
            song_changed: false,
            sampler: sample::Sampler::new(ctx.clone()),
            kit: kit::Kit::new(ctx),
            kit_arrivals: 0,
            rendered_with: 0,
            stand_in: false,
            waiting_since: None,
        }
    }

    pub fn busy(&self) -> bool {
        self.job.is_some() || self.render_at.is_some()
    }

    pub fn audio(&self, version: u64) -> Option<Arc<Pcm>> {
        self.audio.as_ref().filter(|(v, _)| *v == version).map(|(_, p)| p.clone())
    }

    pub fn reference(&mut self) {
        self.events.push(Event::Reference(Ok(REFERENCE.to_string())));
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.state.log.push(msg.into());
        let n = self.state.log.len();
        if n > 200 {
            self.state.log.drain(..n - 200);
        }
    }

    /// a reply that turns down what the demo cannot do
    pub fn refuse(&mut self, what: &str, why: &str) {
        let reply = Reply { ok: false, message: why.into(), version: self.state.version };
        self.events.push(Event::Reply { what: what.into(), result: Ok(reply) });
    }

    fn publish(&mut self) {
        if let (Some(r), Some(s)) = (self.state.release.as_mut(), self.state.song.as_ref()) {
            r.song_title = s.title.clone();
        }
        self.state.version += 1;
        if std::mem::take(&mut self.song_changed) {
            self.state.song_version = self.state.version;
        }
        self.state.can_undo = !self.undo.is_empty();
        self.state.can_redo = !self.redo.is_empty();
        self.state.undo_label = self.undo.last().map(|(_, l)| l.clone());
        self.state.redo_label = self.redo.last().map(|(_, l)| l.clone());
        self.events.push(Event::State(Box::new(self.state.clone())));
    }

    fn reply(&mut self, cmd: &Command, result: Result<String, String>) {
        let (ok, message) = match result {
            Ok(m) => (true, m),
            Err(m) => (false, m),
        };
        let reply = Reply { ok, message, version: self.state.version };
        self.events.push(Event::Reply { what: command_name(cmd), result: Ok(reply) });
    }

    /// the song changed from `before`: remember it for undo, give it a new version, render it again soon
    fn changed(&mut self, before: Option<SongView>, label: &str) {
        if let Some(b) = before {
            self.undo.push((b, label.lines().next().unwrap_or("").chars().take(120).collect()));
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.touched();
    }

    fn touched(&mut self) {
        self.song_changed = true;
        if let Some(song) = self.state.song.as_mut() {
            synth::write_cycles(song);
            song.script = song::script(song);
        }
        self.render_at = Some(self.now + AUTO_RENDER_DELAY);
    }

    fn start_render(&mut self, stems: bool, after: Option<String>) {
        let Some(song) = self.state.song.as_ref() else { return };
        let synth = Box::new(synth::Render::new(song, &self.kit));
        self.stand_in = synth.from_synth > 0 && !self.kit.gone();
        self.rendered_with = self.kit_arrivals;
        self.job = Some(Job::Render { synth, stems, version: self.state.song_version, after });
        self.state.busy = Some("rendering".into());
        self.state.progress = Some(0.0);
        self.render_at = None;
    }

    fn render_current(&self) -> bool {
        self.state.render.as_ref().is_some_and(|r| r.version >= self.state.song_version)
    }

    pub fn send(&mut self, cmd: Command) {
        let busy = self.job.is_some();
        let before = self.state.song.clone();
        let result: Result<String, String> = match &cmd {
            Command::NewSong { genre, seed, bpm, key, loops }
                if self.sampler.on() && song::sampling(genre).is_some() =>
            {
                let seed = seed.unwrap_or_else(|| self.rng.next() % 1000);
                let g = song::genre(genre);
                let mut rng = Rng::new(seed ^ 0x5eed_5a3f);
                let spec = song::sampling(genre).expect("a genre built on samples");
                let gathering = self.sampler.start(spec, g.bpm, &mut rng);
                self.state.busy = Some(gathering.doing());
                self.state.progress = Some(0.0);
                self.job = Some(Job::Sample(Box::new(Sampling {
                    gathering,
                    started: self.now,
                    rng,
                    genre: genre.clone(),
                    seed,
                    bpm: *bpm,
                    key: key.clone(),
                    loops: *loops,
                })));
                let msg =
                    format!("sampling for a {} song (seed {seed}): loops from Freesound under CC0 or CC BY", g.name);
                self.log(msg.clone());
                Ok(msg)
            }
            Command::NewSong { genre, seed, bpm, key, loops } => {
                let seed = seed.unwrap_or_else(|| self.rng.next() % 1000);
                let song = song::generate(genre, seed, *bpm, key.as_deref(), *loops);
                let msg = format!(
                    "new song: {} — {} at {:.0} BPM in {} (seed {seed})",
                    song.title, song.genre, song.bpm, song.key
                );
                self.new_song(song, before, &msg);
                Ok(msg)
            }
            Command::Load { path: name } | Command::OpenText { name, .. } => {
                Err(format!("the demo cannot open files ({name}); start `claudius serve` for that"))
            }
            Command::SampleSong { .. } => {
                Err("the demo samples nothing: start `claudius serve` to sample a recording".into())
            }
            Command::RequestPitchCheck { note, .. } => {
                // the same six phrases each time: the demo hears no singing, so they only have to play
                let phrases = [
                    [60, 62, 64, 67],
                    [67, 65, 64, 62],
                    [60, 64, 67, 72],
                    [72, 71, 69, 67],
                    [65, 64, 62, 60],
                    [62, 65, 69, 67],
                ];
                let id = self.state.pitch_check.as_ref().map_or(1, |c| c.id + 1);
                let notes = phrases
                    .iter()
                    .enumerate()
                    .flat_map(|(i, ph)| {
                        ph.iter().enumerate().map(move |(k, &p)| claudius_api::NoteView {
                            t: i as f64 * 10.0 + k as f64,
                            d: 0.9,
                            p,
                            v: 0.8,
                        })
                    })
                    .collect();
                let answers = (0..6).map(|i| (i as f64 * 10.0 + 5.0, (i + 1) as f64 * 10.0)).collect();
                self.state.pitch_check = Some(claudius_api::PitchCheck {
                    id,
                    bpm: 90.0,
                    notes,
                    answers,
                    beats: 60.0,
                    note: note.clone(),
                    ..Default::default()
                });
                Ok(format!("asked the person for a pitch check (check {id}): six phrases of four notes to sing back"))
            }
            Command::CancelPitchCheck { id } => match self.state.pitch_check.as_mut() {
                Some(c) if c.id == *id && c.status == claudius_api::TakeStatus::Waiting => {
                    c.status = claudius_api::TakeStatus::Cancelled;
                    Ok(format!("pitch check {id}: not now, then"))
                }
                _ => Err(format!("no pitch check {id} waiting")),
            },
            Command::TranscribeDrums { .. } => {
                Err("the demo hears no drums: start `claudius serve` to turn a beat into lanes".into())
            }
            Command::ExtractGroove { .. } => {
                Err("the demo hears no grooves: start `claudius serve` to take a recording's groove".into())
            }
            Command::AssessVoice { .. } => {
                Err("the demo hears no singing: start `claudius serve` to hear how a take is sung".into())
            }
            Command::Save { .. } => Err("the demo keeps no folders: the download button hands you the song as a WAV \
                                         file"
                .into()),
            Command::Export { .. } => Err("the demo cannot export: start `claudius serve` for that".into()),
            Command::SetRelease { details, .. } => {
                if let Some(r) = self.state.release.as_mut() {
                    r.details = details.as_ref().clone();
                }
                Ok("kept: the demo checks nothing".into())
            }
            Command::WriteKit { .. } => {
                Err("the demo cannot write a release kit: start `claudius serve` for that".into())
            }
            Command::Analyze { path } => match audio_file(path) {
                Ok(()) => {
                    let msg = self.fake_analysis(path);
                    self.log(format!("analyzed {path}"));
                    Ok(msg)
                }
                Err(e) => Err(e),
            },
            Command::Remake { path, genre, seed, .. } => match audio_file(path) {
                Ok(()) => {
                    let genre = genre
                        .clone()
                        .or_else(|| self.state.song.as_ref().map(|s| s.genre.clone()))
                        .unwrap_or_else(|| "house".into());
                    let seed = seed.unwrap_or_else(|| self.rng.next() % 1000);
                    let mut song = song::generate(&genre, seed, None, None, false);
                    song.lineage = vec![format!("remade {path} as {genre} (demo: the file was not really read)")];
                    let msg = format!("remade {path} as {genre}:\n{}", self.fake_analysis(path));
                    self.new_song(song, before, &msg);
                    Ok(msg)
                }
                Err(e) => Err(e),
            },
            Command::RunScript { code } => match check_script(code) {
                Ok(n) => {
                    let song = self.state.song.get_or_insert_with(SongView::default);
                    let changes = run_demo_script(song, code);
                    if changes.is_empty() {
                        let msg = format!(
                            "ran {n} statements; nothing changed (the demo only follows tempo, key, swing, mute and solo)"
                        );
                        self.log(msg.clone());
                        Ok(msg)
                    } else {
                        let msg = format!("ran {n} statements: {}", changes.join(", "));
                        if let Some(song) = self.state.song.as_mut() {
                            song.lineage.push(format!("script: {}", changes.join(", ")));
                        }
                        self.changed(before, &msg);
                        self.log(msg.clone());
                        Ok(msg)
                    }
                }
                Err(e) => {
                    self.log(format!("script error: {e}"));
                    Err(e)
                }
            },
            Command::Render { stems } => {
                if self.state.song.is_none() {
                    Err("no song yet".into())
                } else if !stems && !busy && self.render_current() {
                    Ok("the render is up to date".into())
                } else {
                    self.stems_wanted |= *stems;
                    self.render_at = Some(self.now);
                    Ok(if busy { "rendering after the job in progress".into() } else { "rendering".into() })
                }
            }
            Command::Mutate { count, keep_best } => {
                if busy {
                    Err(format!(
                        "busy ({}); try again when it is done, or cancel it",
                        self.state.busy.clone().unwrap_or_default()
                    ))
                } else {
                    self.job = Some(Job::Mutate { count: *count, keep_best: *keep_best, started: self.now });
                    self.state.busy = Some(format!("mutating: auditioning {count} candidates"));
                    self.state.progress = Some(0.0);
                    self.log(format!("mutate: trying {count} random mutations"));
                    Ok("mutating…".into())
                }
            }
            Command::Evolve { generations, children } => {
                if busy {
                    Err(format!(
                        "busy ({}); try again when it is done, or cancel it",
                        self.state.busy.clone().unwrap_or_default()
                    ))
                } else {
                    self.job = Some(Job::Evolve {
                        generations: (*generations).max(1),
                        children: (*children).max(1),
                        started: self.now,
                        reported: 0,
                    });
                    self.state.busy = Some(format!("evolving: generation 1 of {generations}"));
                    self.state.progress = Some(0.0);
                    self.log(format!("evolve: {generations} generations of {children} children"));
                    Ok("evolving…".into())
                }
            }
            Command::InstallPacks { .. } => Err("the demo downloads nothing: install claudius to get the packs".into()),
            Command::RequestTake { .. } | Command::CancelTakeRequest { .. } => {
                Err("the demo records nothing: start `claudius serve` to ask for takes".into())
            }
            Command::Cancel => {
                if self.job.take().is_some() {
                    self.state.busy = None;
                    self.state.progress = None;
                    self.log("cancelled");
                    Ok("stopping after the current step".into())
                } else {
                    Err("nothing to cancel".into())
                }
            }
            Command::Undo | Command::Redo => {
                let undo = matches!(cmd, Command::Undo);
                let popped = if undo { self.undo.pop() } else { self.redo.pop() };
                match (popped, before) {
                    (Some((prev, label)), Some(cur)) => {
                        if undo {
                            self.redo.push((cur, label.clone()));
                        } else {
                            self.undo.push((cur, label.clone()));
                        }
                        self.state.song = Some(prev);
                        self.touched();
                        let msg = format!("{} {label}", if undo { "undid" } else { "redid" });
                        self.log(msg.clone());
                        Ok(msg)
                    }
                    _ => Err(if undo { "nothing to undo" } else { "nothing to redo" }.into()),
                }
            }
            edit => {
                let Some(song) = self.state.song.as_mut() else {
                    self.reply(&cmd, Err("no song yet: start one with new_song".into()));
                    return;
                };
                match edits::apply(song, edit) {
                    Some(Ok(msg)) => {
                        self.changed(before, &msg);
                        self.log(msg.clone());
                        Ok(msg)
                    }
                    Some(Err(e)) => Err(e),
                    None => Err("not supported by the demo".into()),
                }
            }
        };
        if result.is_ok() || matches!(cmd, Command::RunScript { .. }) {
            self.publish();
        }
        self.reply(&cmd, result);
    }

    /// the new song a gathering of samples was for, made from what it found
    fn sampled_song(&mut self, mut s: Sampling) {
        let cuts = std::mem::take(&mut s.gathering.cuts);
        for c in &cuts {
            self.kit.sample(&c.label(), c.clip.clone());
        }
        let song = song::generate_with(&s.genre, s.seed, s.bpm, s.key.as_deref(), s.loops, &cuts);
        let (key, bpm) = (crate::music::Key::parse(&song.key), song.bpm);
        let msg = format!(
            "new song: {} — {} at {:.0} BPM in {} (seed {}), {}",
            song.title,
            song.genre,
            song.bpm,
            song.key,
            s.seed,
            if cuts.is_empty() { "without samples" } else { "built on samples" }
        );
        // undo goes back to the song as it is now, edits made while the samples were gathered and all
        let before = self.state.song.clone();
        self.new_song(song, before, &msg);
        for c in &cuts {
            self.log(format!("  {}: {}; {}", c.role.noun(), c.about(key.as_ref(), bpm), c.found.credit()));
        }
        // the samples a song that can come back (by undo or redo) plays are kept; the rest let go
        let sounds = self
            .state
            .song
            .iter()
            .chain(self.undo.iter().map(|(s, _)| s))
            .chain(self.redo.iter().map(|(s, _)| s))
            .flat_map(|s| s.tracks.iter().filter(|t| t.kind == "loop").map(|t| t.sound.clone()))
            .collect();
        self.kit.keep_samples(&sounds);
        self.publish();
    }

    fn new_song(&mut self, song: SongView, before: Option<SongView>, msg: &str) {
        self.state.song = Some(song);
        self.state.render = None;
        self.state.verdict = None;
        self.job = None;
        self.state.busy = None;
        self.state.progress = None;
        self.changed(before, msg);
        self.log(msg.lines().next().unwrap_or("").to_string());
    }

    fn fake_analysis(&mut self, path: &str) -> String {
        let bpm = 118.0 + self.rng.range(0.0, 10.0).round();
        format!(
            "{path} (demo: made up, the file was not read)\ntempo {bpm:.1} BPM, key A minor (confidence 0.71)\nprogression A: i VI III VII  |  B: iv i VII v  (1 bar(s) per chord)\nsections: intro 16, groove 16, break 8, drop 32, outro 16\nkick on every beat, offbeat hats, swing 0.53"
        )
    }

    fn finish_render(&mut self, synth: synth::Render, stems: bool, version: u64, after: Option<String>) {
        let (pcm, waveform, loudness, lufs) = synth.finish();
        let seconds = pcm.seconds();
        let fresh = version >= self.state.song_version;
        let Some(song) = self.state.song.as_mut() else { return };
        if fresh {
            for t in &mut song.tracks {
                t.loudness = loudness.iter().find(|(n, _)| *n == t.name).map(|(_, l)| (*l * 10.0).round() / 10.0);
            }
        }
        let spb = 240.0 / song.bpm;
        let sections = song
            .sections
            .iter()
            .map(|s| (s.name.clone(), s.start_bar as f64 * spb, (s.start_bar + s.bars) as f64 * spb))
            .collect();
        let render = RenderView {
            version,
            url: format!("mock://render/{version}.mp3"),
            seconds,
            lufs: (lufs * 10.0).round() / 10.0,
            true_peak_db: -1.0 - self.rng.f32() * 0.3,
            bpm: song.bpm,
            sections,
            waveform,
            stems: if stems { song.tracks.iter().map(|t| t.name.clone()).collect() } else { Vec::new() },
        };
        let verdict = song::judge(song, &mut self.rng);
        let total = verdict.total;
        self.audio = Some((version, Arc::new(pcm)));
        self.state.render = Some(render);
        self.state.verdict = Some(verdict);
        self.state.busy = None;
        self.state.progress = None;
        self.log(format!("rendered {} at {lufs:.1} LUFS; verdict {total:.1}/100", fmt_time(seconds)));
        if let Some(a) = after {
            self.log(a);
        }
        if !fresh {
            // the song moved on while this rendered
            self.render_at = Some(self.now);
        }
        self.publish();
    }

    fn mutate_song(&mut self, n: usize) -> Vec<String> {
        let Some(song) = self.state.song.as_mut() else { return Vec::new() };
        let mut what = Vec::new();
        for _ in 0..n.max(1) {
            match self.rng.below(5) {
                0 if song.drums.iter().any(|d| !d.variants.is_empty()) => {
                    let lanes: Vec<usize> =
                        (0..song.drums.len()).filter(|&i| !song.drums[i].variants.is_empty()).collect();
                    let lane = &mut song.drums[*self.rng.pick(&lanes)];
                    let len = lane.variants[0].1.len().max(1);
                    let step = self.rng.below(len);
                    if let Some(v) = lane.variants[0].1.get_mut(step) {
                        *v = if *v > 0.0 { 0.0 } else { 0.6 };
                    }
                    what.push(format!("{} step {}", lane.lane, step + 1));
                }
                1 if !song.parts.is_empty() => {
                    let p = self.rng.below(song.parts.len());
                    let part = &mut song.parts[p];
                    if part.styles.len() > 1 {
                        part.style = self.rng.pick(&part.styles).clone();
                    }
                    what.push(format!("{} style {}", part.name, part.style));
                }
                2 if !song.tracks.is_empty() => {
                    let t = self.rng.below(song.tracks.len());
                    let track = &mut song.tracks[t];
                    track.level_db = (track.level_db + self.rng.range(-1.5, 1.5) * 10.0).round() / 10.0;
                    what.push(format!("{} level {:+.1} dB", track.name, track.level_db));
                }
                3 if !song.sections.is_empty() => {
                    let s = self.rng.below(song.sections.len());
                    let sec = &mut song.sections[s];
                    sec.energy = (sec.energy + self.rng.range(-0.08, 0.08)).clamp(0.1, 1.0);
                    what.push(format!("{} energy {:.2}", sec.name, sec.energy));
                }
                _ => {
                    song.swing = (song.swing + self.rng.range(-0.02, 0.02)).clamp(0.5, 0.7);
                    what.push(format!("swing {:.2}", song.swing));
                }
            }
        }
        what
    }

    /// steps the running job; `now` in seconds
    pub fn update(&mut self, now: f64) -> Vec<Event> {
        self.now = now;
        if !self.started {
            self.started = true;
            self.publish();
            self.events.push(Event::Sounds(song::sounds()));
        }
        self.sampler.poll();
        if self.kit.poll() {
            self.kit_arrivals += 1;
            if let Some(credits) = self.kit.credits.take() {
                self.log(format!("demo sounds: claudius' own synth presets, and drums: {}", credits.join("; ")));
            }
        }
        // sounds the last render had to play on the synth have arrived since
        if self.stand_in && self.kit_arrivals > self.rendered_with && self.job.is_none() && self.render_at.is_none() {
            self.render_at = Some(now);
        }
        if self.job.is_none() && self.render_at.is_some_and(|t| now >= t) {
            let needs = self.state.song.as_ref().map(|s| synth::needs(s, &self.kit)).unwrap_or_default();
            self.kit.want(&needs);
            let since = *self.waiting_since.get_or_insert(now);
            if self.kit.waiting(&needs) && now - since < KIT_WAIT {
                if self.state.busy.is_none() {
                    self.state.busy = Some("loading the demo's sounds".into());
                    self.publish();
                }
                self.render_at = Some(now + 0.15);
            } else {
                let stems = std::mem::take(&mut self.stems_wanted);
                self.start_render(stems, None);
                if !self.stand_in {
                    self.waiting_since = None;
                }
                self.publish();
            }
        }
        match self.job.take() {
            Some(Job::Render { mut synth, stems, version, after }) => {
                let progress = synth.step(RENDER_BUDGET);
                if synth.done() {
                    self.finish_render(*synth, stems, version, after);
                } else {
                    self.state.progress = Some(progress);
                    self.job = Some(Job::Render { synth, stems, version, after });
                    if now - self.last_progress > 0.1 {
                        self.last_progress = now;
                        self.publish();
                    }
                }
            }
            Some(Job::Mutate { count, keep_best, started }) => {
                let t = ((now - started) / MUTATE_SECS).clamp(0.0, 1.0);
                if t >= 1.0 {
                    let base = self.state.verdict.as_ref().map_or(80.0, |v| v.total);
                    let scores: Vec<f64> = (0..count).map(|_| base + self.rng.range(-4.0, 3.0) as f64).collect();
                    let best = scores.iter().copied().fold(f64::MIN, f64::max);
                    for (i, s) in scores.iter().enumerate() {
                        self.log(format!("  candidate {}: {s:.1}{}", i + 1, if *s == best { " (best)" } else { "" }));
                    }
                    self.state.busy = None;
                    self.state.progress = None;
                    if keep_best && best > base {
                        let before = self.state.song.clone();
                        let what = self.mutate_song(2);
                        let label = format!("mutated: {}", what.join(", "));
                        if let Some(song) = self.state.song.as_mut() {
                            song.lineage.push(label.clone());
                        }
                        self.changed(before, &label);
                        self.log(format!("kept the best mutation (+{:.1}): {}", best - base, what.join(", ")));
                    } else {
                        self.log(format!("no mutation beat the current song ({base:.1})"));
                    }
                    self.publish();
                } else {
                    let c = ((t * count as f64) as usize + 1).min(count);
                    self.state.busy = Some(format!("mutating: candidate {c} of {count}"));
                    self.state.progress = Some(t as f32);
                    self.job = Some(Job::Mutate { count, keep_best, started });
                    if now - self.last_progress > 0.1 {
                        self.last_progress = now;
                        self.publish();
                    }
                }
            }
            Some(Job::Evolve { generations, children, started, reported }) => {
                let total = (generations * children) as f64 * CHILD_SECS;
                let t = ((now - started) / total).clamp(0.0, 1.0);
                let done_gens = ((t * generations as f64) as usize).min(generations);
                let mut reported = reported;
                while reported < done_gens {
                    reported += 1;
                    let before = self.state.song.clone();
                    let what = self.mutate_song(1);
                    let gain = self.rng.range(0.1, 1.6);
                    let label = format!("evolved g{reported}: {}", what.join(", "));
                    if let Some(song) = self.state.song.as_mut() {
                        song.lineage.push(label.clone());
                    }
                    self.changed(before, &label);
                    self.log(format!("generation {reported}: best child +{gain:.1} ({})", what.join(", ")));
                }
                if t >= 1.0 {
                    self.state.busy = None;
                    self.state.progress = None;
                    self.log(format!("evolved {generations} generations"));
                } else {
                    let g = (done_gens + 1).min(generations);
                    let c = ((t * (generations * children) as f64) as usize % children) + 1;
                    self.state.busy =
                        Some(format!("evolving: generation {g} of {generations}, child {c} of {children}"));
                    self.state.progress = Some(t as f32);
                    self.job = Some(Job::Evolve { generations, children, started, reported });
                }
                if now - self.last_progress > 0.1 || t >= 1.0 || reported != done_gens {
                    self.last_progress = now;
                    self.publish();
                }
            }
            Some(Job::Sample(mut s)) => {
                let mut lines = self.sampler.advance(&mut s.gathering, &mut s.rng);
                if now - s.started > SAMPLE_WAIT {
                    lines.extend(s.gathering.give_up());
                }
                let moved = !lines.is_empty();
                for line in lines {
                    self.log(line);
                }
                if s.gathering.done() {
                    self.sampled_song(*s);
                } else {
                    self.state.busy = Some(s.gathering.doing());
                    self.state.progress = Some(s.gathering.progress());
                    self.job = Some(Job::Sample(s));
                    if moved || now - self.last_progress > 0.25 {
                        self.last_progress = now;
                        self.publish();
                    }
                }
            }
            None => {}
        }
        std::mem::take(&mut self.events)
    }
}

fn audio_file(path: &str) -> Result<(), String> {
    let p = path.trim().to_lowercase();
    if p.is_empty() {
        Err("no file".into())
    } else if AUDIO.iter().any(|ext| p.ends_with(ext)) {
        Ok(())
    } else {
        Err(format!("cannot read {path}: not an audio file the demo knows (wav, mp3, flac, aiff, m4a, ogg)"))
    }
}

/// what the demo follows of a script: tempo, key, swing, mute and solo
fn run_demo_script(song: &mut SongView, code: &str) -> Vec<String> {
    let mut changes = Vec::new();
    if let Some(bpm) = number_after(code, "tempo(").or_else(|| number_after(code, ".bpm("))
        && (40.0..=240.0).contains(&bpm)
        && (bpm - song.bpm).abs() > 1e-6
    {
        song.bpm = bpm;
        edits::relayout(song);
        changes.push(format!("tempo {bpm}"));
    }
    if let Some(swing) = number_after(code, "swing(")
        && (swing as f32 - song.swing).abs() > 1e-4
    {
        song.swing = (swing as f32).clamp(0.5, 0.75);
        changes.push(format!("swing {swing:.2}"));
    }
    if let Some(key) = string_after(code, "key(\"")
        && let Some(k) = crate::music::Key::parse(&key)
        && k.name() != song.key
    {
        song.key = k.name();
        changes.push(format!("key {}", song.key));
    }
    for (pat, solo, on) in
        [("unsolo(\"", true, false), ("solo(\"", true, true), ("unmute(\"", false, false), ("mute(\"", false, true)]
    {
        let mut rest = code;
        while let Some(i) = rest.find(pat) {
            let prefixed = i > 0 && rest[..i].ends_with("un");
            rest = &rest[i + pat.len()..];
            if prefixed && on {
                continue;
            }
            let Some(end) = rest.find('"') else { break };
            let name = &rest[..end];
            if let Some(t) = song.tracks.iter_mut().find(|t| t.name == name) {
                let flag = if solo { &mut t.solo } else { &mut t.muted };
                if *flag != on {
                    *flag = on;
                    changes.push(format!(
                        "{}{} {name}",
                        if on { "" } else { "un" },
                        if solo { "solo" } else { "mute" }
                    ));
                }
            }
        }
    }
    changes
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{}:{:02}", (s / 60.0) as u64, (s % 60.0) as u64)
}

fn number_after(code: &str, pat: &str) -> Option<f64> {
    let rest = &code[code.find(pat)? + pat.len()..];
    rest[..rest.find(')')?].trim().parse().ok()
}

fn string_after(code: &str, pat: &str) -> Option<String> {
    let rest = &code[code.find(pat)? + pat.len()..];
    Some(rest[..rest.find('"')?].to_string())
}

/// brackets and quotes balance; returns the number of statements
fn check_script(code: &str) -> Result<usize, String> {
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut statements = 0;
    for (n, line) in code.lines().enumerate() {
        let line_no = n + 1;
        let code_part = line.split("//").next().unwrap_or("");
        let mut quote: Option<char> = None;
        for c in code_part.chars() {
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
                continue;
            }
            match c {
                '"' | '\'' | '`' => quote = Some(c),
                '(' | '[' | '{' => stack.push((c, line_no)),
                ')' | ']' | '}' => {
                    let want = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    match stack.pop() {
                        Some((open, _)) if open == want => {}
                        Some((open, at)) => {
                            return Err(format!("line {line_no}: '{c}' closes the '{open}' from line {at}"));
                        }
                        None => return Err(format!("line {line_no}: unexpected '{c}'")),
                    }
                }
                _ => {}
            }
        }
        if let Some(q) = quote {
            return Err(format!("line {line_no}: unterminated string (missing {q})"));
        }
        if stack.is_empty() && !code_part.trim().is_empty() {
            statements += 1;
        }
    }
    match stack.last() {
        Some((open, at)) => Err(format!("line {at}: '{open}' is never closed")),
        None => Ok(statements),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states(events: &[Event]) -> Vec<&State> {
        events.iter().filter_map(|e| if let Event::State(s) = e { Some(s.as_ref()) } else { None }).collect()
    }

    fn replies(events: &[Event]) -> Vec<&Reply> {
        events.iter().filter_map(|e| if let Event::Reply { result: Ok(r), .. } = e { Some(r) } else { None }).collect()
    }

    fn render(mock: &mut Mock, mut now: f64) -> f64 {
        while mock.busy() {
            now += 0.05;
            mock.update(now);
        }
        now
    }

    #[test]
    fn renders_judges_and_plays_back() {
        let mut mock = Mock::new(None);
        let first = mock.update(0.0);
        assert!(states(&first).last().unwrap().busy.is_some(), "the demo renders at start");
        assert!(first.iter().any(|e| matches!(e, Event::Sounds(_))));
        render(&mut mock, 0.0);
        let r = mock.state.render.clone().expect("a render");
        assert_eq!(r.waveform.len(), 1000);
        assert!(r.seconds > 200.0 && (r.seconds - mock.state.song.as_ref().unwrap().seconds).abs() < 3.0);
        assert!(mock.audio(r.version).is_some_and(|p| p.frames() > 0));
        assert!(mock.state.verdict.as_ref().is_some_and(|v| v.total > 0.0 && !v.scores.is_empty()));
        assert!(mock.state.song.as_ref().unwrap().tracks.iter().any(|t| t.loudness.is_some()));
    }

    #[test]
    fn sounds_that_come_late_are_mixed_in_as_they_come() {
        let hit = || kit::Clip {
            l: vec![0.5; 4_800],
            r: Vec::new(),
            secs: 0.1,
            energy: vec![0.01; kit::TIMES.len()],
            power: 0.0,
            beats: 0.0,
        };
        let drums = ["kick", "snare", "clap", "hat"].iter().flat_map(|d| ["707", "808"].map(|m| format!("{m}:{d}")));
        let names: Vec<String> = drums.collect();
        let mut mock = Mock::new(None);
        mock.kit = kit::Kit::pending(names.iter().map(|n| (n.as_str(), false, 0.0, vec![(60, hit())])).collect());
        mock.update(0.0);
        assert_eq!(mock.state.busy.as_deref(), Some("loading the demo's sounds"));
        let now = render(&mut mock, 0.0);
        assert!(mock.stand_in && now >= KIT_WAIT, "the wait ran out, and the synth stood in");
        let needs = synth::needs(mock.state.song.as_ref().unwrap(), &mock.kit);
        assert!(needs.len() >= 2, "{needs:?}");
        mock.kit.arrive(&format!("{}-0.ogg", needs[0].sound), Ok(hit()));
        mock.update(now + 0.05);
        assert_eq!(
            mock.state.busy.as_deref(),
            Some("rendering"),
            "what came is mixed in at once, the rest not waited for"
        );
        let now = render(&mut mock, now + 0.05);
        mock.kit.arrive(&format!("{}-0.ogg", needs[1].sound), Err("gone".into()));
        mock.update(now + 0.05);
        assert!(mock.state.busy.is_none() && !mock.busy(), "a file that failed renders nothing again");
    }

    #[test]
    fn edits_undo_and_redo() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        let before = mock.state.song.clone().unwrap();
        mock.send(Command::ToggleLayer { section: 0, layer: "bass".into() });
        let after_toggle = mock.state.song.clone().unwrap();
        mock.send(Command::MoveSection { from: 0, to: 2 });
        let song = mock.state.song.clone().unwrap();
        assert_eq!(song.sections[2].name, before.sections[0].name);
        assert_eq!(song.sections[0].start_bar, 0);
        assert_eq!(song.sections[1].start_bar, song.sections[0].bars);
        assert!(
            song.sections[2].layers.contains(&"bass".to_string())
                != before.sections[0].layers.contains(&"bass".to_string())
        );
        mock.send(Command::Undo);
        mock.send(Command::Undo);
        assert_eq!(mock.state.song.as_ref().unwrap().sections, before.sections);
        assert!(mock.state.can_redo);
        mock.send(Command::Redo);
        assert_eq!(
            mock.state.song.as_ref().unwrap().sections,
            after_toggle.sections,
            "redo brings back the toggle, not the move"
        );
        let events = mock.update(1.0);
        assert!(replies(&events).iter().all(|r| r.ok));
    }

    #[test]
    fn tracks_are_added_and_removed() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        let add = |name: &str, kind: &str, role: Option<&str>, sound: &str| Command::AddTrack {
            name: name.into(),
            kind: kind.into(),
            role: role.map(String::from),
            sound: Some(sound.into()),
        };
        mock.send(add("lead2", "part", Some("lead"), "square_lead"));
        mock.send(add("bongo", "drum", None, "tom"));
        let song = mock.state.song.clone().unwrap();
        assert!(song.parts.iter().any(|p| p.name == "lead2" && p.role == "lead" && p.preset == "square_lead"));
        let lane = song.drums.iter().find(|d| d.lane == "bongo").expect("the new lane");
        assert!(lane.variants[0].1.iter().all(|v| *v == 0.0));
        assert!(song.tracks.iter().any(|t| t.name == "bongo" && t.bus == "drums"));
        assert!(song.sections.iter().filter(|s| s.energy >= 0.6).all(|s| s.layers.contains(&"lead2".to_string())));
        mock.send(add("lead2", "part", Some("lead"), "pluck"));
        mock.send(Command::RemoveTrack { name: "bongo".into() });
        let song = mock.state.song.clone().unwrap();
        assert_eq!(song.parts.iter().filter(|p| p.name == "lead2").count(), 1, "a name the song has is refused");
        assert!(!song.layers.contains(&"bongo".to_string()) && song.drums.iter().all(|d| d.lane != "bongo"));
        assert!(song.sections.iter().all(|s| !s.layers.contains(&"bongo".to_string())));
        mock.send(Command::Undo);
        assert!(mock.state.song.as_ref().unwrap().drums.iter().any(|d| d.lane == "bongo"));
    }

    #[test]
    fn the_piano_roll_writes_a_cycle_out_and_the_demo_plays_it() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        let song = mock.state.song.clone().unwrap();
        let lead = song.parts.iter().find(|p| p.role == "lead").expect("a lead");
        let cycle = lead.cycles.first().expect("the lead plays a progression");
        assert!(!cycle.written && !cycle.notes.is_empty() && cycle.beats > 0.0, "{cycle:?}");
        let (name, prog) = (lead.name.clone(), cycle.prog.clone());
        let note = |t: f64, d: f64, p: i32, v: f32| claudius_api::NoteView { t, d, p, v };
        let notes = vec![note(0.0, 1.0, 72, 0.9), note(2.0, 0.5, 74, 0.6)];
        mock.send(Command::SetNotes { part: name.clone(), prog: prog.clone(), notes: Some(notes.clone()) });
        // another edit later: the written cycle stays as it is
        mock.send(Command::SetTempo { bpm: 120.0 });
        let lead = mock.state.song.clone().unwrap().parts.into_iter().find(|p| p.name == name).unwrap();
        let c = lead.cycles.iter().find(|c| c.prog == prog).unwrap();
        assert!(c.written && c.notes == notes, "{c:?}");
        // the demo plays them
        let synth = synth::Render::new(mock.state.song.as_ref().unwrap(), &mock.kit);
        assert!(synth.plays(&name, 72), "the demo plays the written notes");
        mock.send(Command::SetNotes { part: name.clone(), prog: prog.clone(), notes: None });
        let lead = mock.state.song.clone().unwrap().parts.into_iter().find(|p| p.name == name).unwrap();
        assert!(lead.cycles.iter().all(|c| !c.written), "the demo writes it again");
    }

    #[test]
    fn a_cycle_written_out_as_the_demo_wrote_it_plays_the_same_and_outlasts_a_rearrangement() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        // swung, so that the written notes have to swing as the demo's own do
        mock.send(Command::SetSwing { swing: 0.6 });
        let heard =
            |mock: &Mock, name: &str| synth::Render::new(mock.state.song.as_ref().unwrap(), &mock.kit).notes_of(name);
        let song = mock.state.song.clone().unwrap();
        for p in &song.parts {
            let before = heard(&mock, &p.name);
            for c in &p.cycles {
                mock.send(Command::SetNotes {
                    part: p.name.clone(),
                    prog: c.prog.clone(),
                    notes: Some(c.notes.clone()),
                });
            }
            let after = heard(&mock, &p.name);
            assert!(
                !before.is_empty() && before.len() == after.len(),
                "{}: {} then {}",
                p.name,
                before.len(),
                after.len()
            );
            for (a, b) in before.iter().zip(&after) {
                let same = a.0.abs_diff(b.0) <= 2 && a.1.abs_diff(b.1) <= 2 && a.2 == b.2 && a.4 == b.4;
                assert!(same && (a.3 - b.3).abs() < 1e-5, "{}: {a:?} then {b:?}", p.name);
            }
        }
        // the part switched off everywhere, and on again: what was written out is still there
        let name = song.parts[0].name.clone();
        let written = |mock: &Mock| {
            let song = mock.state.song.as_ref().unwrap();
            song.parts
                .iter()
                .find(|p| p.name == name)
                .unwrap()
                .cycles
                .iter()
                .filter(|c| c.written)
                .cloned()
                .collect::<Vec<_>>()
        };
        let before = written(&mock);
        let playing: Vec<usize> =
            (0..song.sections.len()).filter(|&i| song.sections[i].layers.contains(&name)).collect();
        assert!(!before.is_empty() && !playing.is_empty());
        for &i in &playing {
            mock.send(Command::ToggleLayer { section: i, layer: name.clone() });
        }
        assert_eq!(written(&mock), before, "kept while {name} plays nowhere");
        for &i in &playing {
            mock.send(Command::ToggleLayer { section: i, layer: name.clone() });
        }
        assert_eq!(written(&mock), before, "and played again");
    }

    #[test]
    fn drums_copy_on_write_and_step_notation() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        mock.send(Command::SetDrumStep { lane: "kick".into(), variant: "fresh".into(), step: 3, velocity: 1.0 });
        mock.send(Command::SetDrumPattern {
            lane: "clap".into(),
            variant: "main".into(),
            steps: "....X.......x...".into(),
        });
        mock.send(Command::SetDrumPattern { lane: "clap".into(), variant: "main".into(), steps: "....?...".into() });
        let song = mock.state.song.as_ref().unwrap();
        let kick = song.drums.iter().find(|d| d.lane == "kick").unwrap();
        let (main, fresh) = (
            &kick.variants.iter().find(|(v, _)| v == "main").unwrap().1,
            &kick.variants.iter().find(|(v, _)| v == "fresh").unwrap().1,
        );
        assert_eq!(fresh[0], main[0], "a new variant starts as a copy of main");
        assert_eq!(fresh[3], 1.0);
        let clap = &song.drums.iter().find(|d| d.lane == "clap").unwrap().variants[0].1;
        assert_eq!(
            (clap[4], clap[12], clap[20], clap[0]),
            (1.0, 0.8, 1.0, 0.0),
            "16 steps repeat over the lane's bars"
        );
        let events = mock.update(1.0);
        let r = replies(&events);
        assert!(!r.last().unwrap().ok, "bad step notation is refused");
    }

    #[test]
    fn scripts_report_errors_with_lines() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        mock.send(Command::RunScript { code: "song(\"x\").bpm(126)\ntrack(\"kick\", s(\"bd*4\")".into() });
        let events = mock.update(1.0);
        let r = replies(&events);
        assert!(!r[0].ok && r[0].message.contains("line 2"), "{}", r[0].message);
        mock.send(Command::RunScript { code: "song(\"x\").bpm(126)\n".into() });
        assert_eq!(mock.state.song.as_ref().unwrap().bpm, 126.0);
    }

    #[test]
    fn versions_history_and_solo_follow_the_server() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        let now = render(&mut mock, 0.0);
        assert!(!crate::panels::render_stale(&mock.state));
        mock.send(Command::SetTempo { bpm: 128.0 });
        assert_eq!(mock.state.song_version, mock.state.version, "a song edit moves song_version");
        assert!(crate::panels::render_stale(&mock.state));
        assert_eq!(mock.state.undo_label.as_deref(), Some("tempo 128.0 BPM"));
        let now = render(&mut mock, now);
        assert!(!crate::panels::render_stale(&mock.state), "every change renders again by itself");
        mock.send(Command::SetMix {
            track: "lead".into(),
            level_db: None,
            pan: None,
            reverb: None,
            delay: None,
            duck: None,
            drive_db: None,
            muted: None,
            solo: Some(true),
            bus: None,
        });
        assert!(crate::panels::any_solo(mock.state.song.as_ref().unwrap()));
        mock.send(Command::Undo);
        assert!(!crate::panels::any_solo(mock.state.song.as_ref().unwrap()));
        assert!(mock.state.redo_label.as_deref().is_some_and(|l| l.contains("lead")), "{:?}", mock.state.redo_label);
        mock.send(Command::SetSection {
            index: 0,
            name: None,
            bars: None,
            energy: None,
            layers: None,
            prog: Some("B".into()),
            variant: None,
            filter_in: None,
            build: None,
            gap: None,
            impact: None,
            fills: None,
        });
        assert_eq!(mock.state.song.as_ref().unwrap().sections[0].prog, "B");
        mock.send(Command::Analyze { path: "~/Music/x.mp3".into() });
        mock.send(Command::Analyze { path: "notes.txt".into() });
        let events = mock.update(now);
        let r = replies(&events);
        assert!(r.iter().rev().nth(1).is_some_and(|r| r.ok && r.message.contains("tempo")) && !r.last().unwrap().ok);
    }

    /// answers what a gathering for trip-hop asks, as the site would, until the song is made; returns the time
    fn feed_samples(mock: &mut Mock, mut now: f64) -> f64 {
        use sample::tests::{chord_loop, found};
        let spec = song::sampling("trip_hop").unwrap();
        let noise = |secs: f64| {
            let mut rng = Rng::new(9);
            sample::Pcm2 { l: (0..(secs * 48_000.0) as usize).map(|_| rng.range(-0.2, 0.2)).collect(), r: Vec::new() }
        };
        for _ in 0..40 {
            now += 0.05;
            mock.update(now);
            for asked in mock.sampler.pending() {
                if let Some(q) = asked.strip_prefix("search:") {
                    let list = if spec.phrase.search.contains(&q) {
                        vec![found("p", "Orchestral Strings Loop 90 BPM", 5.333, &[])]
                    } else if spec.drum_break.search.contains(&q) {
                        vec![found("b", "drums 92 bpm", 2.6087, &[])]
                    } else {
                        vec![found("t", "Vinyl Crackle Analogue 1.wav", 6.0, &[])]
                    };
                    mock.sampler.arrive_search(q, Ok(list));
                } else if let Some(a) = asked.strip_prefix("audio:") {
                    let pcm = match a {
                        "audio/1/p_1.ogg" => chord_loop(90.0, &[&[57, 60, 64], &[53, 57, 60]], 0.0),
                        "audio/1/b_1.ogg" => noise(2.6087),
                        _ => noise(6.0),
                    };
                    mock.sampler.arrive_audio(a, Ok(pcm));
                }
            }
            if !matches!(mock.job, Some(Job::Sample(_))) {
                break;
            }
        }
        now
    }

    #[test]
    fn a_song_of_a_genre_built_on_samples_is_made_from_what_the_sampler_finds() {
        let mut mock = Mock::new(None);
        mock.sampler = sample::Sampler::by_hand();
        mock.send(Command::NewSong { genre: "trip_hop".into(), seed: Some(4), bpm: None, key: None, loops: false });
        assert!(mock.state.busy.as_deref().is_some_and(|b| b.starts_with("sampling")), "{:?}", mock.state.busy);
        let now = feed_samples(&mut mock, 0.0);
        let song = mock.state.song.clone().unwrap();
        assert_eq!((song.bpm, song.key.as_str()), (90.0, "A minor"), "the phrase's tempo and key");
        assert_eq!(song.progressions[0].1, ["i", "VI"], "and its chords");
        let sample = song.tracks.iter().find(|t| t.name == "sample").expect("a sample track");
        assert_eq!(sample.sound, "loop 'Orchestral Strings Loop 90 BPM' by someone (CC BY 4.0), 90 BPM");
        assert!(song.tracks.iter().any(|t| t.name == "break" && t.bus == "drums"));
        let verse = song.sections.iter().find(|s| s.name == "verse").unwrap();
        let has = |l: &str| verse.layers.iter().any(|x| x == l);
        assert!(has("break") && has("sample") && has("texture"), "{:?}", verse.layers);
        assert!(!has("snare") && !has("keys"), "what plays in the samples' place stays out: {:?}", verse.layers);
        assert!(song.lineage.iter().any(|l| l.starts_with("sampled: phrase 'Orchestral Strings Loop 90 BPM'")));
        let credit = "'Orchestral Strings Loop 90 BPM' by someone, CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/), https://freesound.org/s/p/";
        assert!(mock.state.log.iter().any(|l| l.contains(credit)), "{:#?}", mock.state.log);
        render(&mut mock, now);
        let song = mock.state.song.as_ref().unwrap();
        for name in ["sample", "break", "texture"] {
            let t = song.tracks.iter().find(|t| t.name == name).unwrap();
            assert!(t.loudness.is_some(), "{name} plays");
        }
    }

    #[test]
    fn a_tempo_asked_for_moves_the_phrase_and_its_chords_together_and_keeps_the_seeds_song() {
        use sample::tests::{chord_loop, found};
        // a seed and a tempo make the song they made before samples: no tempo is drawn
        let plain = song::generate("house", 5, Some(124.0), None, false);
        assert_eq!(plain.title, song::generate_with("house", 5, Some(124.0), None, false, &[]).title);
        // A minor, then F major, at 90; played at 101 (two semitones up) it is B minor: i, VI
        let pcm = chord_loop(90.0, &[&[57, 60, 64], &[53, 57, 60]], 0.0);
        let f = found("p", "strings 90bpm", pcm.l.len() as f64 / 48_000.0, &[]);
        let fit = sample::fit_loop(f.seconds, &sample::said_tempos(&f), (78.0, 94.0));
        let want = sample::Want { search: &[], words: &[], not: &[], fx: &[] };
        let cut = sample::cut(sample::Role::Phrase, &pcm, &f, fit, &want).unwrap();
        let song = song::generate_with("trip_hop", 3, Some(101.0), None, false, &[cut]);
        assert_eq!(song.key, "B minor");
        assert_eq!(song.progressions[0].1, ["i", "VI"]);
        // and the log names them as the song plays them
        let b_minor = crate::music::Key::parse("B minor").unwrap();
        let cut = sample::cut(sample::Role::Phrase, &pcm, &f, fit, &want).unwrap();
        assert!(cut.about(Some(&b_minor), 101.0).ends_with("in B minor: i VI"), "{}", cut.about(Some(&b_minor), 101.0));
    }

    #[test]
    fn an_edit_made_while_the_samples_are_gathered_is_kept_for_undo() {
        let mut mock = Mock::new(None);
        mock.sampler = sample::Sampler::by_hand();
        mock.update(0.0);
        mock.send(Command::NewSong { genre: "trip_hop".into(), seed: Some(4), bpm: None, key: None, loops: false });
        mock.send(Command::ToggleLayer { section: 0, layer: "bass".into() });
        let edited = mock.state.song.clone().unwrap();
        feed_samples(&mut mock, 0.0);
        assert_eq!(mock.state.song.as_ref().unwrap().genre, "trip_hop");
        mock.send(Command::Undo);
        assert_eq!(mock.state.song.as_ref().unwrap().sections, edited.sections, "the edit, not the song before it");
    }

    #[test]
    fn without_anywhere_to_ask_a_genre_built_on_samples_plays_what_stands_in_for_them() {
        let mut mock = Mock::new(None);
        mock.sampler = sample::Sampler::at(None, None);
        mock.send(Command::NewSong { genre: "trip_hop".into(), seed: Some(4), bpm: None, key: None, loops: false });
        let song = mock.state.song.as_ref().unwrap();
        assert!(!song.tracks.iter().any(|t| t.name == "sample"));
        let verse = song.sections.iter().find(|s| s.name == "verse").unwrap();
        assert!(["kick", "snare", "hat", "keys", "strings"].iter().all(|l| verse.layers.iter().any(|x| x == l)));
    }

    #[test]
    fn the_sites_worker_searches_for_every_sample_the_demo_looks_for() {
        let worker = include_str!("../../../../site/worker/index.js");
        for g in song::GENRES {
            let Some(spec) = song::sampling(g.name) else { continue };
            for role in sample::Role::ALL {
                for q in spec.want(role).search {
                    assert!(worker.contains(&format!("\"{q}\",")), "site/worker/index.js does not search for '{q}'");
                }
            }
        }
    }

    #[test]
    fn new_songs_follow_their_genre() {
        let mut mock = Mock::new(None);
        mock.update(0.0);
        for g in song::GENRES {
            mock.send(Command::NewSong { genre: g.name.into(), seed: Some(3), bpm: None, key: None, loops: false });
            let s = mock.state.song.as_ref().unwrap();
            assert_eq!(s.genre, g.name);
            assert!(s.bpm >= g.bpm.0 && s.bpm <= g.bpm.1, "{} at {}", g.name, s.bpm);
            assert!(!s.sections.is_empty() && !s.drums.is_empty() && !s.parts.is_empty());
            assert!(s.drums.iter().all(|d| d.variants.iter().all(|(_, v)| v.len() == (d.pattern_bars * 16) as usize)));
            assert!(s.sections.iter().all(|x| x.layers.iter().all(|l| s.layers.contains(l))));
            assert!(crate::music::Key::parse(&s.key).is_some());
        }
    }
}
