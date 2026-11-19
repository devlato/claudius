//! The radio in the app (design 0024): the station (`claudius_session::radio`) on an engine of its own, a `Host` on a
//! thread that writes and renders the station's songs while the person's song stays as it is, and the stream the
//! radio's player plays (`audio::radio`): each song, and each change from one into the next, placed ahead of where the
//! stream is.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use claudius::audio::Stereo;
use claudius::config::SRF;
use claudius_api::{Command, GenreInfo};
use claudius_session::host::Host;
use claudius_session::radio::{self, Spec, Station};
use eframe::egui;

use crate::audio::radio::{Placed, RadioPlayer};

/// how far ahead of where the stream plays a change is placed, in seconds, so that the audio thread meets it whole
const AHEAD: f64 = 0.5;
/// how many songs the radio says it played
const PLAYED: usize = 12;

/// A song of the station's, as the radio shows it.
#[derive(Clone, Debug)]
pub struct Info {
    pub title: String,
    pub genre: String,
    pub bpm: f64,
    pub key: String,
    pub seconds: f64,
}

/// a song the engine wrote and rendered for the station, as it asked for it
struct Made {
    spec: Spec,
    info: Info,
    song: radio::Song,
}

enum Said {
    Genres(Vec<GenreInfo>),
    Song(Result<Box<Made>, String>),
}

/// the engine: a `Host` on a thread of its own, asked for one song at a time
struct Engine {
    tx: Sender<Spec>,
    rx: Receiver<Said>,
}

impl Engine {
    fn start(ctx: Option<egui::Context>) -> Engine {
        let (tx, asks) = mpsc::channel::<Spec>();
        let (says, rx) = mpsc::channel::<Said>();
        let spawned = std::thread::Builder::new().name("claudius radio".into()).spawn(move || {
            let mut host = Host::new(seed());
            let _ = says.send(Said::Genres(host.state().genres));
            if let Some(c) = &ctx {
                c.request_repaint();
            }
            for spec in asks {
                if says.send(Said::Song(make(&mut host, &spec))).is_err() {
                    return;
                }
                if let Some(c) = &ctx {
                    c.request_repaint();
                }
            }
        });
        if let Err(e) = spawned {
            log::error!("the radio's engine did not start: {e}");
        }
        Engine { tx, rx }
    }
}

/// the song `spec` asks for, written and rendered whole
fn make(host: &mut Host, spec: &Spec) -> Result<Box<Made>, String> {
    let reply = host.command(Command::NewSong {
        genre: spec.genre.clone(),
        seed: Some(spec.seed),
        bpm: Some(spec.bpm),
        key: spec.key.map(|k| k.name()),
        loops: false,
        sampled: Some(false),
        sample_search: None,
        complexity: None,
    });
    if !reply.ok {
        return Err(reply.message);
    }
    let audio = host.render().ok_or("no song to render")??;
    let song = host.state().song.ok_or("no song")?;
    let stereo = Arc::new(Stereo::from_interleaved(&audio.samples));
    let info = Info { title: song.title, genre: song.genre, bpm: song.bpm, key: song.key, seconds: stereo.secs() };
    let sections = song.sections.iter().map(|s| (s.start_bar, s.bars, s.kind.clone())).collect();
    let song = radio::Song { audio: stereo, bpm: song.bpm, bars: song.bars, sections };
    Ok(Box::new(Made { spec: spec.clone(), info, song }))
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_millis() as u64)
}

/// a song placed in the stream: where its frame 0 falls, where the change into it begins, when it is on air, and how
/// it came in
struct Placing {
    made: Box<Made>,
    start: u64,
    change_at: u64,
    on_air_at: u64,
    said: String,
    blended: bool,
}

/// The app's radio: its engine, station and player, and what it plays.
pub struct Radio {
    engine: Engine,
    station: Option<Station>,
    player: RadioPlayer,
    on_air: Option<Placing>,
    next: Option<Placing>,
    /// a song asked of the engine and not made yet
    asked: Option<Spec>,
    /// a skip asked for while the next song was still being written: it comes in as soon as it is
    skipping: bool,
    /// the genres to keep to, chosen before the engine said what it plays
    keep: Vec<String>,
    tuned: bool,
    /// a file of the song on air being made
    downloading: Option<Receiver<Result<PathBuf, String>>>,
    /// what played, the latest first
    pub played: Vec<Info>,
    /// what the station last did, in words
    pub said: String,
}

impl Radio {
    pub fn new(ctx: Option<egui::Context>) -> Radio {
        Radio::with(ctx, RadioPlayer::new())
    }

    fn with(ctx: Option<egui::Context>, player: RadioPlayer) -> Radio {
        Radio {
            engine: Engine::start(ctx),
            station: None,
            player,
            on_air: None,
            next: None,
            asked: None,
            skipping: false,
            keep: Vec::new(),
            tuned: false,
            downloading: None,
            played: Vec::new(),
            said: "Starting the engine…".into(),
        }
    }

    /// why it cannot play
    pub fn error(&self) -> Option<&str> {
        self.player.error()
    }

    /// every genre the station plays, once the engine has said
    pub fn genres(&self) -> Vec<GenreInfo> {
        self.station.as_ref().map(|s| s.all().to_vec()).unwrap_or_default()
    }

    /// the genres it keeps to (none: every one)
    pub fn kept(&self) -> BTreeSet<String> {
        match &self.station {
            Some(s) => s.kept().clone(),
            None => self.keep.iter().cloned().collect(),
        }
    }

    /// keeps it to `genres` (none: every one), from the next song it writes
    pub fn keep_to(&mut self, genres: Vec<String>) {
        match self.station.as_mut() {
            Some(s) => s.keep_to(genres),
            None => self.keep = genres,
        }
    }

    pub fn tuned(&self) -> bool {
        self.tuned
    }

    pub fn playing(&self) -> bool {
        self.player.is_playing()
    }

    /// tunes in, or plays and pauses once tuned in
    pub fn toggle(&mut self) {
        if !self.tuned {
            self.tuned = true;
            self.said = "Writing the first song…".into();
            self.player.play();
            if self.station.is_some() {
                self.ask_first();
            }
        } else if self.player.is_playing() {
            self.player.pause();
        } else {
            self.player.play();
        }
    }

    pub fn volume(&self) -> f32 {
        self.player.volume()
    }

    pub fn set_volume(&self, v: f32) {
        self.player.set_volume(v);
    }

    /// what the engine said and the stream reached: the next song on air once the stream is at it, the one after it
    /// asked for, a file made
    pub fn tick(&mut self) {
        while let Ok(said) = self.engine.rx.try_recv() {
            match said {
                Said::Genres(genres) => {
                    let mut s = Station::new(&genres, seed());
                    s.keep_to(std::mem::take(&mut self.keep));
                    self.station = Some(s);
                    self.said = if self.tuned { "Writing the first song…".into() } else { String::new() };
                    if self.tuned && self.on_air.is_none() && self.asked.is_none() {
                        self.ask_first();
                    }
                }
                Said::Song(Ok(made)) => {
                    self.asked = None;
                    self.place(made);
                }
                Said::Song(Err(why)) => {
                    self.asked = None;
                    self.said = format!("A song could not be made ({why}); trying another.");
                }
            }
        }
        let clock = self.player.clock();
        if self.next.as_ref().is_some_and(|n| clock >= n.on_air_at)
            && let Some(n) = self.next.take()
        {
            self.said = format!("{}, {}", said(&n.made.info.genre), n.said);
            if let Some(old) = self.on_air.replace(n) {
                self.played.insert(0, old.made.info);
                self.played.truncate(PLAYED);
            }
        }
        if self.tuned && self.station.is_some() && self.asked.is_none() && self.next.is_none() {
            match &self.on_air {
                None => self.ask_first(),
                Some(_) => self.ask_next(),
            }
        }
        if let Some(rx) = &self.downloading
            && let Ok(done) = rx.try_recv()
        {
            self.downloading = None;
            self.said = match done {
                Ok(path) => format!("Saved to {}", path.display()),
                Err(why) => format!("The song could not be saved: {why}"),
            };
        }
    }

    fn ask_first(&mut self) {
        if let Some(spec) = self.station.as_mut().and_then(Station::first) {
            self.ask(spec);
        }
    }

    fn ask_next(&mut self) {
        let Some(a) = &self.on_air else { return };
        let key = radio::Key::parse(&a.made.info.key).unwrap_or(radio::Key { tonic: 9, minor: true });
        let (genre, bpm) = (a.made.info.genre.clone(), a.made.info.bpm);
        if let Some(spec) = self.station.as_mut().and_then(|s| s.next(&genre, bpm, key)) {
            self.ask(spec);
        }
    }

    fn ask(&mut self, spec: Spec) {
        if self.engine.tx.send(spec.clone()).is_ok() {
            self.asked = Some(spec);
        }
    }

    /// a song the engine made, placed in the stream: the first from where the stream is, the next one after a change
    /// from the one on air (at once on its next beat, when a skip waits for it)
    fn place(&mut self, made: Box<Made>) {
        let clock = self.player.clock();
        let Some(a) = &self.on_air else {
            let len = made.song.audio.len();
            self.player.place(vec![Placed { at: clock, audio: made.song.audio.clone(), from: 0, len }]);
            self.said = "On air".into();
            self.on_air = Some(Placing {
                made,
                start: clock,
                change_at: clock,
                on_air_at: clock,
                said: String::new(),
                blended: false,
            });
            return;
        };
        let into_a = clock.saturating_sub(a.start) as usize;
        let c = if std::mem::take(&mut self.skipping) {
            radio::cut(&a.made.song, &made.song, into_a)
        } else {
            radio::change(&a.made.song, &made.song, made.spec.how, into_a + (AHEAD * SRF) as usize)
        };
        let change_at = a.start + c.at as u64;
        let mixed = c.mixed.len();
        let start = change_at + mixed as u64 - c.then as u64;
        let mut program: Vec<Placed> = self
            .player
            .program()
            .iter()
            .filter(|p| p.at < change_at)
            .map(|p| Placed { len: p.len.min((change_at - p.at) as usize), ..p.clone() })
            .collect();
        program.push(Placed { at: change_at, audio: Arc::new(c.mixed), from: 0, len: mixed });
        let rest = made.song.audio.len().saturating_sub(c.then);
        program.push(Placed { at: change_at + mixed as u64, audio: made.song.audio.clone(), from: c.then, len: rest });
        self.player.place(program);
        let on_air_at = if c.blended { change_at } else { start };
        self.next = Some(Placing { made, start, change_at, on_air_at, said: c.said, blended: c.blended });
    }

    /// the next song in on the playing one's next beat; or, while it is still being written, as soon as it is. A
    /// change already under way goes on as it is.
    pub fn skip(&mut self) {
        if self.on_air.is_none() {
            return;
        }
        let clock = self.player.clock();
        match self.next.take() {
            None => {
                self.skipping = true;
                self.said = "Skipping: the next song comes in as soon as it is written.".into();
            }
            Some(n) if clock >= n.change_at => self.next = Some(n),
            Some(n) => {
                self.skipping = true;
                self.place(n.made);
            }
        }
    }

    /// the song on air, how far into it the stream is and how long it is, in seconds
    pub fn on_air(&self) -> Option<(&Info, f64, f64)> {
        let a = self.on_air.as_ref()?;
        let at = self.player.clock().saturating_sub(a.start) as f64 / SRF;
        Some((&a.made.info, at.min(a.made.info.seconds), a.made.info.seconds))
    }

    /// what comes next, in words
    pub fn coming(&self) -> Option<String> {
        if let Some(n) = &self.next {
            let secs = n.on_air_at.saturating_sub(self.player.clock()) as f64 / SRF;
            let i = &n.made.info;
            let how = if n.blended { "mixes in" } else { "comes in" };
            return Some(format!(
                "{}: {}, {:.0} BPM, {}; {how} {} from now",
                i.title,
                said(&i.genre),
                i.bpm,
                i.key,
                crate::mock::fmt_time(secs)
            ));
        }
        let s = self.asked.as_ref()?;
        let key = s.key.map(|k| format!(" in {}", k.name())).unwrap_or_default();
        let soon = if self.skipping { ", to come in as soon as it is written" } else { "" };
        Some(format!("writing a {} song at {:.0} BPM{key}{soon}…", said(&s.genre), s.bpm))
    }

    /// The song on air saved as an MP3 in the songs folder's `radio`, made on a thread of its own; `tick` says where
    /// it went.
    pub fn download(&mut self) {
        let Some(a) = &self.on_air else { return };
        if self.downloading.is_some() {
            return;
        }
        let (audio, title) = (a.made.song.audio.clone(), a.made.info.title.clone());
        let (tx, rx) = mpsc::channel();
        self.downloading = Some(rx);
        self.said = format!("Making a file of {title}…");
        std::thread::spawn(move || {
            let _ = tx.send(save(&audio, &title));
        });
    }

    pub fn downloading(&self) -> bool {
        self.downloading.is_some()
    }

    /// what makes the song on air the studio's, as the station asked the engine for it
    pub fn studio(&self) -> Option<Command> {
        let s = &self.on_air.as_ref()?.made.spec;
        Some(Command::NewSong {
            genre: s.genre.clone(),
            seed: Some(s.seed),
            bpm: Some(s.bpm),
            key: s.key.map(|k| k.name()),
            loops: false,
            sampled: Some(false),
            sample_search: None,
            complexity: None,
        })
    }
}

/// `audio` as an MP3 named after `title` in the songs folder's `radio`: where it went
fn save(audio: &Stereo, title: &str) -> Result<PathBuf, String> {
    let spec: claudius::export::Spec = "mp3".parse().map_err(|e| format!("{e}"))?;
    let bytes = claudius::export::encode(audio, &spec, &claudius::export::tags(title)).map_err(|e| format!("{e:#}"))?;
    let dir = claudius::config::songs_dir().join("radio");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name: String = title.chars().filter(|c| c.is_alphanumeric() || " -_'".contains(*c)).collect();
    let name = if name.trim().is_empty() { "claudius radio".to_string() } else { name.trim().to_string() };
    let path = dir.join(format!("{name}.{}", spec.extension()));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// a genre's name as people say it
pub fn said(genre: &str) -> String {
    if genre == "dnb" { "drum and bass".into() } else { genre.replace('_', " ") }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// ticks `r` until `done` says it is there
    fn until(r: &mut Radio, done: impl Fn(&Radio) -> bool) {
        let t0 = Instant::now();
        while !done(r) {
            r.tick();
            assert!(t0.elapsed() < Duration::from_secs(600), "the station never got there: {}", r.said);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn the_station_writes_its_first_song_places_the_next_after_a_change_and_skips_to_it_on_the_beat() {
        let mut r = Radio::with(None, RadioPlayer::silent());
        r.keep_to(vec!["house".into()]);
        r.toggle();
        until(&mut r, |r| r.on_air().is_some());
        let (first, _, secs) = r.on_air().map(|(i, at, s)| (i.clone(), at, s)).unwrap();
        assert_eq!(first.genre, "house");
        assert!(secs > 60.0, "a whole song, {secs} s");
        let program = r.player.program();
        assert_eq!((program.len(), program[0].at, program[0].from), (1, 0, 0), "from where the stream is");
        // the next one, made while the first plays, after a change placed on one of the first's bar lines
        until(&mut r, |r| r.next.is_some());
        let n = r.next.as_ref().unwrap();
        assert_eq!(n.made.info.genre, "house", "kept to the genre chosen");
        let program = r.player.program();
        assert_eq!(program.len(), 3, "the first up to the change, the change, the next from where it goes on");
        assert_eq!(program[0].end(), program[1].at);
        assert_eq!(program[1].end(), program[2].at);
        assert_eq!(program[2].from, n.made.song.audio.len() - program[2].len);
        let bar = 240.0 / first.bpm * SRF;
        // a blend and a record stop begin on the bar line, an echo out a beat before it
        let echoed = !n.blended && n.made.spec.how != radio::How::Brake;
        let at = n.change_at as f64 + if echoed { bar / 4.0 } else { 0.0 };
        assert!((at / bar - (at / bar).round()).abs() < 1e-3, "on a bar line: {} bars", at / bar);
        assert!(r.coming().is_some_and(|c| c.contains("from now")), "{:?}", r.coming());
        // a skip: the next song in on the playing one's next beat, from its start
        r.skip();
        let n = r.next.as_ref().unwrap();
        assert!(n.made.info.genre == "house" && n.said.starts_with("skipped to"), "{}", n.said);
        let program = r.player.program();
        assert_eq!((program.len(), program[0].at), (2, 0), "the cut from the stream's start, then the next song");
        assert!(r.studio().is_some_and(|c| matches!(c, Command::NewSong { ref genre, .. } if genre == "house")));
    }
}
