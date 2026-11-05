//! The built-in demo: the claudius engine itself, in a browser's Web Worker (in the app, on a thread of its own),
//! which makes, edits and renders songs as the server does, without files, jobs or ears (`engine`). What needs a
//! computer of one's own it turns down and says so, as for takes, folders and packs; a recording to listen to gets an
//! analysis made up from its name, since the demo hears nothing. A song built on samples is built on free recordings
//! the page finds for it first (`sample`), which the engine cuts as the app does.

mod engine;
mod phone;
mod sample;

use std::sync::Arc;

use claudius_api::{
    Command, NoteView, PitchCheck, ReleaseCheck, ReleaseDetails, ReleaseView, Reply, SoundList, State, TakeStatus,
    Transport,
};
use eframe::egui;

use crate::audio::Pcm;
use crate::net::{Event, command_name};
use engine::{Heard, Link};

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
}

/// the song the demo opens with, its genre and seed: deep house, the genre the critic and the ears like best of what the
/// demo plays on claudius' own synths (90/100, heard as deep house well clear of techno), and unhurried
const FIRST: (&str, u64) = ("deep_house", 3);
/// what the demo asks its first song as, so that its answer is not shown as an answer to the person
const OPENING: &str = "demo:first";
/// how long a new song waits for its samples before it is made with what has come
const SAMPLE_WAIT: f64 = 60.0;
const AUDIO: [&str; 7] = [".wav", ".mp3", ".flac", ".aif", ".aiff", ".m4a", ".ogg"];

/// a new song of a genre built on samples, waiting for them: its gathering, when it began, and the command that makes
/// the song once they are here
struct Sampling {
    gathering: sample::Gathering,
    started: f64,
    rng: Rng,
    genre: String,
    song: Command,
    /// a song asked for since, sent to the engine as this: once the engine makes it, it takes this one's place
    superseded: Option<String>,
}

pub struct Mock {
    engine: Link,
    /// the engine's latest state, with what the page keeps itself: the release, a pitch check, sampling
    state: State,
    release: ReleaseView,
    pitch_check: Option<PitchCheck>,
    /// the page's own lines, after the engine's in the log
    said: Vec<String>,
    events: Vec<Event>,
    audio: Option<(u64, Arc<Pcm>)>,
    /// commands sent to the engine and not answered yet
    waiting: usize,
    /// a remake's made-up analysis, which goes before the engine's answer to it
    remade: Option<String>,
    sampler: sample::Sampler,
    sampling: Option<Sampling>,
    rng: Rng,
    now: f64,
    ctx: Option<egui::Context>,
    /// the engine's sounds, as the phone's keys offer them
    sounds: Option<SoundList>,
    /// the room a phone plays along through, while the page for it is served
    relay: Option<phone::Relay>,
    /// where the studio says the song is, and when (`now`) it said so
    transport: (Transport, f64),
    /// how many things the engine has been asked for the phones (`phone::Mock::ask_id`)
    phone_asks: u64,
}

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
    /// starts the engine, which makes the demo's first song; `ctx` is woken when the engine or the sampler says
    /// something
    pub fn new(ctx: Option<egui::Context>) -> Mock {
        let mut engine = Link::new(ctx.clone());
        let (genre, seed) = FIRST;
        let first = Command::NewSong {
            genre: genre.into(),
            seed: Some(seed),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        };
        engine.command(OPENING, &first);
        Mock {
            engine,
            state: State::default(),
            release: demo_release(),
            pitch_check: None,
            said: vec!["claudius demo: the engine runs in this page, and nothing is kept".into()],
            events: Vec::new(),
            audio: None,
            waiting: 1,
            remade: None,
            sampler: sample::Sampler::new(ctx.clone()),
            sampling: None,
            rng: Rng::new(42),
            now: 0.0,
            ctx,
            sounds: None,
            relay: None,
            transport: (Transport::default(), 0.0),
            phone_asks: 0,
        }
    }

    /// waiting for the engine or the sampler
    pub fn busy(&self) -> bool {
        self.waiting > 0 || self.sampling.is_some() || self.state.busy.is_some()
    }

    pub fn audio(&self, version: u64) -> Option<Arc<Pcm>> {
        self.audio.as_ref().filter(|(v, _)| *v == version).map(|(_, p)| p.clone())
    }

    /// the script reference, which arrives as an event
    pub fn reference(&mut self) {
        self.engine.reference();
    }

    /// a reply that turns down what the demo cannot do
    pub fn refuse(&mut self, what: &str, why: &str) {
        let reply = Reply { ok: false, message: why.into(), version: self.state.version };
        self.events.push(Event::Reply { what: what.into(), result: Ok(reply) });
    }

    /// where the playhead is: the engine previews an edit to a song heard whole there
    pub fn playhead(&mut self, secs: f64) {
        self.engine.playhead(secs);
    }

    pub fn send(&mut self, cmd: Command) {
        let what = command_name(&cmd);
        let replaces = matches!(cmd, Command::NewSong { .. } | Command::Remake { .. } | Command::OpenText { .. });
        let here: Option<Result<String, String>> = match &cmd {
            Command::NewSong { genre, sampled, sample_search, .. }
                if self.sampler.on()
                    && self
                        .sampling_of(genre)
                        .is_some_and(|s| sample::sampled(&s, *sampled, sample_search.as_deref())) =>
            {
                let search = sample_search.clone();
                Some(Ok(self.gather(genre, search.as_deref(), cmd.clone())))
            }
            Command::Remake { path, genre, seed, .. } => match audio_file(path) {
                Ok(()) => {
                    let genre = genre
                        .clone()
                        .or_else(|| self.state.song.as_ref().map(|s| s.genre.clone()))
                        .unwrap_or_else(|| FIRST.0.into());
                    self.remade = Some(format!("remade {path} as {genre}:\n{}", self.fake_analysis(path)));
                    let song = Command::NewSong {
                        genre,
                        seed: *seed,
                        bpm: None,
                        key: None,
                        loops: false,
                        sampled: None,
                        sample_search: None,
                    };
                    self.engine.command(&what, &song);
                    self.waiting += 1;
                    None
                }
                Err(e) => Some(Err(e)),
            },
            Command::Analyze { path } => Some(audio_file(path).map(|()| {
                let said = self.fake_analysis(path);
                self.say(format!("analyzed {path}"));
                said
            })),
            Command::RequestPitchCheck { note, .. } => Some(Ok(self.pitch_check(note))),
            Command::CancelPitchCheck { id } => Some(match self.pitch_check.as_mut() {
                Some(c) if c.id == *id && c.status == TakeStatus::Waiting => {
                    c.status = TakeStatus::Cancelled;
                    Ok(format!("pitch check {id}: not now, then"))
                }
                _ => Err(format!("no pitch check {id} waiting")),
            }),
            Command::SetRelease { details, .. } => {
                self.release.details = details.as_ref().clone();
                Some(Ok("kept: the demo checks nothing".into()))
            }
            Command::Save { .. } | Command::Export { .. } => {
                Some(Err("the demo keeps no files: the download button hands you the song as a WAV file".into()))
            }
            Command::Load { path } => {
                Some(Err(format!("the demo opens no paths ({path}): choose the file, and the studio hands it over")))
            }
            Command::WriteKit { .. } => {
                Some(Err("the demo cannot write a release kit: start `claudius serve` for that".into()))
            }
            Command::InstallPacks { .. } => {
                Some(Err("the demo downloads no packs: install claudius to get them".into()))
            }
            Command::RequestTake { .. } | Command::CancelTakeRequest { .. } => {
                Some(Err("the demo records nothing: start `claudius serve` to ask for takes".into()))
            }
            Command::SampleSong { .. } => {
                Some(Err("the demo samples no recordings: start `claudius serve` to sample one".into()))
            }
            Command::TranscribeDrums { .. } => {
                Some(Err("the demo hears no drums: start `claudius serve` to turn a beat into lanes".into()))
            }
            Command::ExtractGroove { .. } => {
                Some(Err("the demo hears no grooves: start `claudius serve` to take a recording's groove".into()))
            }
            Command::AssessVoice { .. } => {
                Some(Err("the demo hears no singing: start `claudius serve` to hear how a take is sung".into()))
            }
            Command::Mutate { .. } | Command::Evolve { .. } => {
                Some(Err("the demo neither mutates nor evolves songs: start `claudius serve` for that".into()))
            }
            Command::Cancel => Some(match self.sampling.take() {
                Some(s) => Ok(format!("stopped gathering the samples of a {} song", s.genre)),
                None => Err("nothing to cancel: the demo renders as it goes".into()),
            }),
            _ => None,
        };
        match here {
            Some(result) => {
                let (ok, message) = match result {
                    Ok(m) => (true, m),
                    Err(m) => (false, m),
                };
                self.publish();
                let reply = Reply { ok, message, version: self.state.version };
                self.events.push(Event::Reply { what, result: Ok(reply) });
            }
            None => {
                if !matches!(cmd, Command::Remake { .. }) {
                    self.engine.command(&what, &cmd);
                    self.waiting += 1;
                }
                // the song asked for now is the one the person wants, once the engine has made it
                if replaces && let Some(s) = self.sampling.as_mut() {
                    s.superseded = Some(what);
                }
            }
        }
    }

    /// what a genre's songs built on samples dig for, as the engine says (a fusion: the genre that leads it)
    fn sampling_of(&self, genre: &str) -> Option<claudius_api::GenreSampling> {
        let lead = genre.split('+').next().unwrap_or(genre).trim();
        let mut s = self.state.genres.iter().find(|g| g.name == lead).and_then(|g| g.sampling.clone())?;
        // a fusion's drums are the genre it fuses with: it samples no break (`genres::fuse`)
        if genre.contains('+') {
            s.roles.retain(|r| r.role != "break");
        }
        Some(s)
    }

    /// starts gathering the samples a song of `genre` is built on (its phrase searched for `search`, when given);
    /// `song` makes it once they are here
    fn gather(&mut self, genre: &str, search: Option<&str>, song: Command) -> String {
        let mut rng = Rng::new(self.rng.next());
        let sampling = self.sampling_of(genre).unwrap_or_default();
        let gathering = self.sampler.start(genre, &sampling, search, &mut rng);
        self.sampling =
            Some(Sampling { gathering, started: self.now, rng, genre: genre.to_string(), song, superseded: None });
        let msg = format!(
            "sampling for a {genre} song first: recordings from Freesound, Jamendo and Wikimedia Commons under CC0 or \
             CC BY"
        );
        self.say(msg.clone());
        msg
    }

    /// the gathering is done: the engine has cut what it found, and makes the song asked for, built on it
    fn sampled(&mut self, s: Sampling) {
        let n = s.gathering.kept.len();
        self.say(format!("{n} sample{} for the {} song", if n == 1 { "" } else { "s" }, s.genre));
        self.engine.command("new_song", &s.song);
        self.waiting += 1;
    }

    fn pitch_check(&mut self, note: &str) -> String {
        // the same six phrases each time: the demo hears no singing, so they only have to play
        let phrases = [
            [60, 62, 64, 67],
            [67, 65, 64, 62],
            [60, 64, 67, 72],
            [72, 71, 69, 67],
            [65, 64, 62, 60],
            [62, 65, 69, 67],
        ];
        let id = self.pitch_check.as_ref().map_or(1, |c| c.id + 1);
        let notes = phrases
            .iter()
            .enumerate()
            .flat_map(|(i, ph)| {
                ph.iter().enumerate().map(move |(k, &p)| NoteView { t: i as f64 * 10.0 + k as f64, d: 0.9, p, v: 0.8 })
            })
            .collect();
        let answers = (0..6).map(|i| (i as f64 * 10.0 + 5.0, (i + 1) as f64 * 10.0)).collect();
        self.pitch_check = Some(PitchCheck {
            id,
            bpm: 90.0,
            notes,
            answers,
            beats: 60.0,
            note: note.to_string(),
            ..Default::default()
        });
        format!("asked the person for a pitch check (check {id}): six phrases of four notes to sing back")
    }

    fn fake_analysis(&mut self, path: &str) -> String {
        let bpm = 118.0 + self.rng.range(0.0, 10.0).round();
        format!(
            "{path} (demo: made up, the file was not read)\ntempo {bpm:.1} BPM, key A minor (confidence 0.71)\nprogression A: i VI III VII  |  B: iv i VII v  (1 bar(s) per chord)\nsections: intro 16, groove 16, break 8, drop 32, outro 16\nkick on every beat, offbeat hats, swing 0.53"
        )
    }

    fn say(&mut self, line: String) {
        self.said.push(line);
        let n = self.said.len();
        if n > 50 {
            self.said.drain(..n - 50);
        }
    }

    /// the state as the studio sees it: the engine's, with what the page keeps
    fn publish(&mut self) {
        let mut s = self.state.clone();
        if let Some(song) = &s.song {
            self.release.song_title = song.title.clone();
        }
        s.release = Some(self.release.clone());
        s.pitch_check = self.pitch_check.clone();
        s.phone = self.phone_view();
        s.log.extend(self.said.iter().cloned());
        if let Some(g) = &self.sampling {
            s.busy = Some(g.gathering.doing());
            s.progress = Some(g.gathering.progress());
        }
        self.events.push(Event::State(Box::new(s)));
    }

    /// what the engine and the sampler have done since; `now` in seconds
    pub fn update(&mut self, now: f64) -> Vec<Event> {
        self.now = now;
        for heard in self.engine.take() {
            match heard {
                Heard::Answer { what, mut reply, state } => {
                    self.waiting = self.waiting.saturating_sub(1);
                    if let Some(s) = self.sampling.as_mut()
                        && s.superseded.as_deref() == Some(what.as_str())
                    {
                        if reply.ok {
                            let genre = s.genre.clone();
                            self.sampling = None;
                            self.say(format!("stopped gathering the samples of a {genre} song: another song was made"));
                        } else {
                            s.superseded = None;
                        }
                    }
                    self.state = *state;
                    self.publish();
                    if what == "remake"
                        && let Some(said) = self.remade.take()
                    {
                        reply.message = format!("{said}\n{}", reply.message);
                    }
                    // a take from a phone answers the phone, and is news to the person rather than an answer
                    if what != OPENING && !self.took(&what, &reply) {
                        self.events.push(Event::Reply { what, result: Ok(reply) });
                    }
                }
                Heard::State(state) => {
                    self.state = *state;
                    self.publish();
                }
                Heard::Rendered { state, samples } => {
                    if let Some(r) = &state.render {
                        self.audio = Some((r.version, Arc::new(Pcm { rate: 48_000, channels: 2, samples })));
                    }
                    self.state = *state;
                    self.publish();
                }
                Heard::Sounds(list) => {
                    self.sounds = Some(list.clone());
                    self.events.push(Event::Sounds(list));
                }
                Heard::Reference(text) => self.events.push(Event::Reference(Ok(text))),
                Heard::Sound { id, sounds } => self.rendered(id, sounds),
                Heard::Sampled { id, result } => {
                    if let Some(s) = self.sampling.as_mut() {
                        let lines = s.gathering.answer(id, result);
                        for line in lines {
                            self.say(line);
                        }
                        self.publish();
                    }
                }
            }
        }
        self.phones();
        self.sampler.poll();
        if let Some(mut s) = self.sampling.take() {
            let mut lines = self.sampler.advance(&mut s.gathering, &mut s.rng);
            for c in s.gathering.take_cuts() {
                self.engine.recording(c.id, c.recording, &c.bytes);
            }
            if now - s.started > SAMPLE_WAIT {
                lines.extend(s.gathering.give_up());
            }
            let moved = !lines.is_empty();
            for line in lines {
                self.say(line);
            }
            // done, unless a song asked for since may take its place: the engine's answer to it decides
            if s.gathering.done() && s.superseded.is_none() {
                self.sampled(s);
            } else {
                self.sampling = Some(s);
            }
            if moved {
                self.publish();
            }
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

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{}:{:02}", (s / 60.0) as u64, (s % 60.0) as u64)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// the seconds between the keys' notes, as the engine lays them out and as the demo tells the phone
    const SLOT: f64 = claudius::pads::SLOT_SECS;

    /// Steps the demo until `done` says it is there, its events kept: the engine works on a thread of its own.
    fn until(mock: &mut Mock, now: &mut f64, done: impl Fn(&Mock) -> bool) -> Vec<Event> {
        let (t0, mut events) = (Instant::now(), Vec::new());
        loop {
            *now += 0.05;
            events.extend(mock.update(*now));
            if done(mock) {
                return events;
            }
            assert!(t0.elapsed() < Duration::from_secs(180), "the demo never got there: {:#?}", mock.state.log);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// the whole song rendered as it is, not only its first bars
    fn rendered(m: &Mock) -> bool {
        let whole =
            |r: &claudius_api::RenderView| r.version == m.state.song_version && r.url != claudius_api::PREVIEW_URL;
        m.waiting == 0 && m.state.render.as_ref().is_some_and(whole)
    }

    fn replies(events: &[Event]) -> Vec<&Reply> {
        events.iter().filter_map(|e| if let Event::Reply { result: Ok(r), .. } = e { Some(r) } else { None }).collect()
    }

    #[test]
    fn a_phone_plays_along_through_the_relay() {
        use phone::Said;
        use serde_json::{Value, json};
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        // once the engine is idle, so that the phone's questions are its next work
        until(&mut mock, &mut now, |m| rendered(m) && m.sounds.is_some());
        // the room, as the relay gives it, without its socket: what it brings goes in the inbox
        mock.relay = Some(phone::Relay::standing("https://claudius.devlato.com/phone/?room=r"));
        let p = "0f0f0f0f-0f0f-0f0f-0f0f-0f0f0f0f0f0f";
        let ask = |m: &mut Mock, id: u64, method: &str, path: &str, body: Value| {
            let q = json!({ "id": id, "method": method, "path": path, "body": body });
            m.relay.as_mut().unwrap().inbox.push(format!("{p}\n{q}"));
        };
        let answer = |m: &Mock, want: u64| {
            m.relay.as_ref().unwrap().out.iter().find_map(|s| match s {
                Said::Answer { id, status, body, .. } if *id == want => Some((*status, body.clone())),
                Said::Sound { id, wav, .. } if *id == want => Some((200, json!(wav.len()))),
                _ => None,
            })
        };
        mock.relay.as_mut().unwrap().inbox.push(format!("{p}\n{{\"event\":\"joined\"}}"));
        mock.transport(Transport { playing: true, at: 3.0 });
        ask(&mut mock, 1, "GET", "api/clock", Value::Null);
        ask(&mut mock, 2, "GET", "api/song", Value::Null);
        ask(&mut mock, 3, "GET", "api/hits/kick", Value::Null);
        ask(&mut mock, 4, "POST", "api/take", json!({ "cmd": "set_tempo", "bpm": 90 }));
        ask(&mut mock, 5, "GET", "api/settings", Value::Null);
        until(&mut mock, &mut now, |m| answer(m, 3).is_some());
        let (_, clock) = answer(&mock, 1).unwrap();
        assert_eq!((clock["playing"].as_bool(), clock["at"].as_f64()), (Some(true), Some(3.0)));
        let (status, song) = answer(&mock, 2).unwrap();
        assert_eq!(status, 200);
        assert!(song["lanes"].as_array().unwrap().iter().any(|l| l == "kick"), "{song}");
        assert_eq!(song["slot"], json!(SLOT), "the phone's keys are laid out as the engine lays them");
        let hit = mock.relay.as_ref().unwrap().out.iter().find_map(|s| match s {
            Said::Sound { id: 3, wav, .. } => Some(wav.clone()),
            _ => None,
        });
        assert_eq!(&hit.unwrap()[..4], b"RIFF");
        assert_eq!(answer(&mock, 4).unwrap().0, 403, "a phone sends takes and nothing else");
        assert_eq!(answer(&mock, 5).unwrap().0, 404);
        assert_eq!(mock.phone_view().map(|v| v.phones), Some(1), "the studio sees the phone");
        // a song newer than this one is waited for; a take makes one
        let version = mock.state.song_version;
        ask(&mut mock, 6, "GET", &format!("api/song?after={version}"), Value::Null);
        let from = mock.state.song.as_ref().unwrap().sections[0].name.clone();
        let hits = json!([{ "lane": "kick", "t": 0 }, { "lane": "kick", "t": 2 }]);
        ask(&mut mock, 7, "POST", "api/take", json!({ "cmd": "add_hits", "from": from, "hits": hits, "bars": 1 }));
        mock.update(now);
        assert!(answer(&mock, 6).is_none(), "held until the song changes");
        let events = until(&mut mock, &mut now, |m| answer(m, 7).is_some() && answer(m, 6).is_some());
        let (status, reply) = answer(&mock, 7).unwrap();
        assert!(status == 200 && reply["ok"] == true, "{reply}");
        assert!(answer(&mock, 6).unwrap().1["version"].as_u64() > Some(version));
        assert!(
            events.iter().any(|e| matches!(e, Event::Said { text, ok: true } if text.starts_with("from the phone: ")))
        );
        assert!(replies(&events).is_empty(), "the phone's take is no answer to the person");
        mock.phone(false);
        assert_eq!(mock.phone_view(), None);
        // a sound still on its way when the page stopped answers nothing asked since it was served again
        mock.relay = Some(phone::Relay::standing("https://claudius.devlato.com/phone/?room=r"));
        ask(&mut mock, 8, "GET", "api/record?track=kick&bar=0&bars=1", Value::Null);
        mock.update(now);
        mock.relay = Some(phone::Relay::standing("https://claudius.devlato.com/phone/?room=r"));
        ask(&mut mock, 9, "GET", "api/keys?sound=epiano&low=59&high=65&step=3", Value::Null);
        until(&mut mock, &mut now, |m| answer(m, 9).is_some());
        assert!(answer(&mock, 8).is_none(), "the record went to no one");
        let keys = mock.relay.as_ref().unwrap().out.iter().find_map(|s| match s {
            Said::Sound { id: 9, wav, .. } => Some(wav.len()),
            _ => None,
        });
        // three notes a slot each, 16-bit stereo at 48 kHz: not the record of one bar
        assert_eq!(keys, Some(44 + (3.0 * SLOT * 48_000.0) as usize * 4));
        // the demo opened again in another tab took the room: this one lets it, and says so
        mock.relay.as_mut().unwrap().inbox.push("\n{\"event\":\"replaced\"}".into());
        let events = mock.update(now);
        assert_eq!(mock.phone_view(), None);
        assert!(events.iter().any(|e| matches!(e, Event::PhoneRefused(text) if text.contains("another tab"))));
    }

    #[test]
    fn the_demo_opens_with_a_song_of_the_engines_and_renders_judges_and_plays_it() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        let events = until(&mut mock, &mut now, rendered);
        assert!(events.iter().any(|e| matches!(e, Event::Sounds(s) if !s.synths.is_empty())), "the engine's sounds");
        assert!(replies(&events).is_empty(), "the demo's own first song is no answer to the person");
        let s = &mock.state;
        let song = s.song.as_ref().unwrap();
        assert_eq!((song.genre.as_str(), song.seed), FIRST);
        assert!(song.tracks.len() >= 8, "a whole arrangement, not {} tracks", song.tracks.len());
        let r = s.render.as_ref().unwrap();
        let pcm = mock.audio(r.version).expect("the render's audio");
        assert!((pcm.frames() as f64 / 48_000.0 - r.seconds).abs() < 0.01 && r.seconds > 60.0, "{} s", r.seconds);
        assert!(s.verdict.as_ref().is_some_and(|v| v.total > 0.0));
        assert!(song.tracks.iter().any(|t| t.loudness.is_some()), "the meters read the render");
    }

    #[test]
    fn edits_scripts_and_undo_are_the_engines() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        let bpm = mock.state.song.as_ref().unwrap().bpm;
        mock.send(Command::SetTempo { bpm: 128.0 });
        until(&mut mock, &mut now, |m| m.waiting == 0);
        assert_eq!(mock.state.song.as_ref().unwrap().bpm, 128.0);
        assert_eq!(mock.state.undo_label.as_deref(), Some("tempo 128"));
        mock.send(Command::RunScript { code: "mix(\"bass\", #{ level: 2 });\nnope(".into() });
        let events = until(&mut mock, &mut now, |m| m.waiting == 0);
        let said = replies(&events);
        assert!(said.iter().any(|r| !r.ok && r.message.contains("line 2")), "a script's error says where");
        mock.send(Command::Undo);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        assert_eq!(mock.state.song.as_ref().unwrap().bpm, bpm);
    }

    #[test]
    fn what_the_demo_cannot_do_it_says() {
        let mut mock = Mock::new(None);
        for (cmd, words) in [
            (Command::Save { path: None }, "the download button"),
            (Command::Mutate { count: 4, keep_best: true }, "neither mutates nor evolves"),
            (Command::InstallPacks { names: Vec::new() }, "install claudius"),
        ] {
            mock.send(cmd);
            let events = mock.update(0.0);
            assert!(replies(&events).iter().any(|r| !r.ok && r.message.contains(words)), "{words}");
        }
    }

    /// a WAV file of `l` (mono, at 48 kHz), as a library would serve a recording
    fn wav(l: &[f32]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = std::io::Cursor::new(Vec::new());
        let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
        for v in l {
            w.write_sample((v.clamp(-1.0, 1.0) * 32_000.0) as i16).unwrap();
        }
        w.finalize().unwrap();
        out.into_inner()
    }

    /// eight bars at 90 BPM of A minor and F major, a chord struck on each beat over a kick, as a record would play
    fn phrase() -> Vec<f32> {
        let (sr, beat) = (48_000.0, 60.0 / 90.0);
        let n = (32.0 * beat * sr) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                let (b, into) = ((t / beat) as usize, t % beat);
                let notes: &[f64] = if (b / 4) % 2 == 0 { &[57.0, 60.0, 64.0] } else { &[53.0, 57.0, 60.0] };
                let chord: f64 = notes
                    .iter()
                    .map(|m| (std::f64::consts::TAU * 440.0 * 2f64.powf((m - 69.0) / 12.0) * t).sin())
                    .sum();
                let kick = (std::f64::consts::TAU * 55.0 * t).sin() * (-into * 30.0).exp();
                (0.15 * chord * (-into * 3.0).exp() + 0.5 * kick) as f32
            })
            .collect()
    }

    fn noise(secs: f64, seed: u64) -> Vec<f32> {
        let mut rng = Rng::new(seed);
        (0..(secs * 48_000.0) as usize).map(|_| rng.range(-0.2, 0.2)).collect()
    }

    /// answers what the sampler asks for, as the site would: a phrase in A minor at 90 BPM, a break and some crackle,
    /// each found for the searches of its role in the genre's sampling, as files the engine cuts
    fn feed_samples(mock: &mut Mock, now: &mut f64) {
        use sample::tests::found;
        let roles = mock.sampling_of("trip_hop").expect("trip-hop samples").roles;
        let of = |q: &str| roles.iter().find(|r| r.search.iter().any(|s| s == q)).map(|r| r.role.clone());
        let t0 = Instant::now();
        loop {
            *now += 0.05;
            mock.update(*now);
            for asked in mock.sampler.pending() {
                if let Some(q) = asked.strip_prefix("search:") {
                    let list = match of(q).as_deref() {
                        Some("phrase") => vec![found("p", "Orchestral Strings", 21.333, &[])],
                        Some("break") => vec![found("b", "drums 92 bpm", 2.6087, &[])],
                        // no voices here: the song goes without one
                        Some("vocal") => Vec::new(),
                        _ => vec![found("t", "Vinyl Crackle Analogue 1", 6.0, &[])],
                    };
                    mock.sampler.arrive_search(q, Ok(list));
                } else if let Some(a) = asked.strip_prefix("audio:") {
                    let file = match a {
                        "audio/1/p_1.ogg" => wav(&phrase()),
                        "audio/1/b_1.ogg" => wav(&noise(2.6087, 9)),
                        _ => wav(&noise(6.0, 11)),
                    };
                    mock.sampler.arrive_audio(a, Ok(file));
                }
            }
            if mock.sampling.is_none() {
                break;
            }
            assert!(t0.elapsed() < Duration::from_secs(120), "the gathering never ended: {:#?}", mock.state.log);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_song_of_a_genre_built_on_samples_is_built_on_what_the_sampler_finds() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        mock.sampler = sample::Sampler::by_hand();
        mock.send(Command::NewSong {
            genre: "trip_hop".into(),
            seed: Some(4),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        let shown = mock.update(now);
        let busy = shown.iter().find_map(|e| if let Event::State(s) = e { s.busy.clone() } else { None });
        assert!(busy.is_some_and(|b| b.starts_with("sampling")), "it says it is sampling");
        feed_samples(&mut mock, &mut now);
        until(&mut mock, &mut now, rendered);
        let song = mock.state.song.as_ref().unwrap();
        // the phrase's tempo, played at its own speed or a few semitones slower, in trip-hop's range
        assert_eq!(song.genre, "trip_hop");
        assert!((78.0..=94.0).contains(&song.bpm), "{}", song.bpm);
        for name in ["sample", "break", "texture"] {
            let t = song.tracks.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("a {name} track"));
            assert!(t.loudness.is_some(), "{name} plays");
        }
        // credited as the app credits a sample it cut
        assert!(mock.state.log.iter().any(|l| l.contains("\"Orchestral Strings\" by someone, CC BY 4.0")), "credited");
    }

    #[test]
    fn a_house_song_asked_for_samples_is_built_on_them_and_one_not_asked_is_not() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        mock.sampler = sample::Sampler::by_hand();
        let house = |sampled: Option<bool>, sample_search: Option<&str>| Command::NewSong {
            genre: "house".into(),
            seed: Some(4),
            bpm: None,
            key: None,
            loops: false,
            sampled,
            sample_search: sample_search.map(str::to_string),
        };
        mock.send(house(None, None));
        assert!(mock.sampling.is_none(), "house is not sampled unless asked");
        until(&mut mock, &mut now, |m| m.waiting == 0);
        mock.send(house(None, Some("70s soul")));
        assert!(mock.sampling.is_some(), "words of one's own ask for samples");
        mock.update(now);
        let words: Vec<String> = mock.sampler.pending();
        assert!(words.iter().any(|w| w == "search:70s soul"), "{words:?}");
    }

    #[test]
    fn a_song_asked_for_while_samples_are_gathered_for_another_is_the_song_made() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        mock.sampler = sample::Sampler::by_hand();
        mock.send(Command::NewSong {
            genre: "trip_hop".into(),
            seed: Some(4),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        assert!(mock.sampling.is_some());
        mock.send(Command::Load { path: "elsewhere/song.json".into() });
        mock.send(Command::NewSong {
            genre: "polka".into(),
            seed: Some(2),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        until(&mut mock, &mut now, |m| m.waiting == 0);
        assert!(mock.sampling.is_some(), "a song that could not be made takes nothing's place");
        mock.send(Command::NewSong {
            genre: "dnb".into(),
            seed: Some(2),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        until(&mut mock, &mut now, |m| m.waiting == 0);
        assert!(mock.sampling.is_none(), "the gathering for the trip-hop song stops once the dnb song is made");
        feed_samples(&mut mock, &mut now);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        assert_eq!(mock.state.song.as_ref().unwrap().genre, "dnb", "and its song never replaces the one asked for");
        assert!(mock.said.iter().any(|l| l.starts_with("stopped gathering the samples of a trip_hop song")));
    }

    #[test]
    fn without_anywhere_to_ask_a_genre_built_on_samples_plays_what_stands_in_for_them() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        assert!(!mock.sampler.on(), "no address for the samples");
        mock.send(Command::NewSong {
            genre: "trip_hop".into(),
            seed: Some(4),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
        });
        until(&mut mock, &mut now, |m| m.waiting == 0);
        let song = mock.state.song.as_ref().unwrap();
        assert_eq!(song.genre, "trip_hop");
        assert!(!song.tracks.iter().any(|t| t.name == "sample"), "nothing sampled to play");
    }

    #[test]
    fn a_fusion_looks_for_no_break_its_drums_are_the_other_genres() {
        let (mut mock, mut now) = (Mock::new(None), 0.0);
        until(&mut mock, &mut now, |m| m.waiting == 0);
        let roles = |genre: &str| -> Vec<String> {
            mock.sampling_of(genre).map(|s| s.roles.into_iter().map(|r| r.role).collect()).unwrap_or_default()
        };
        assert!(roles("trip_hop").contains(&"break".to_string()));
        assert!(!roles("trip_hop+idm").contains(&"break".to_string()));
        assert!(roles("trip_hop+idm").contains(&"phrase".to_string()));
    }

    #[test]
    fn the_sites_worker_takes_every_search_a_genre_digs_with() {
        // the words the Worker passes on to the libraries, as it reads them (`WORDS` in site/worker/index.js)
        let worker = include_str!("../../../../site/worker/index.js");
        assert!(worker.contains(r"const WORDS = /^[\p{L}\p{N}][\p{L}\p{N} &'.-]{0,59}$/u;"), "the Worker's words");
        for g in claudius::genres::all().unwrap() {
            for r in g.sampling.iter().flat_map(|s| [&s.drum_break, &s.phrase, &s.texture, &s.vocal]).flatten() {
                for q in &r.search {
                    let ok = q.chars().count() <= 60
                        && q.chars().next().is_some_and(char::is_alphanumeric)
                        && q.chars().all(|c| c.is_alphanumeric() || " &'.-".contains(c));
                    assert!(ok, "{} digs with '{q}', which the site's Worker would turn down", g.name);
                }
            }
        }
    }
}
