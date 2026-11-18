use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

use super::{Pcm, Source};

const NO_SEEK: u64 = u64::MAX;

/// how many frames a patch fades in and out over against what plays beneath it (5 ms at 48 kHz), so that the mix
/// changing never clicks
const FADE: usize = 240;
/// the most patches kept: the latest few are all the playhead reaches before the next render comes
const MOST_PATCHES: usize = 32;

/// A stretch of the song mixed anew since the audio playing was rendered (the live mix), of the song at `version`: it
/// plays in that audio's place from `start` (in its frames), faded in and out against what plays beneath it.
#[derive(Clone)]
struct Patch {
    version: u64,
    start: usize,
    pcm: Arc<Pcm>,
}

impl Patch {
    fn end(&self) -> usize {
        self.start + self.pcm.frames()
    }
}

/// `list` and `pcm` played from frame `start` as the song is at `version` (`Player::patch`): joined to the last patch when
/// it is of the same version and follows on from it, the latest `MOST_PATCHES` kept
fn with_patch(list: &[Patch], version: u64, start: usize, pcm: Arc<Pcm>) -> Vec<Patch> {
    let mut list = list.to_vec();
    match list.last_mut() {
        Some(last) if last.version == version && last.end().abs_diff(start) <= 1 => {
            let mut samples = last.pcm.samples.clone();
            samples.extend_from_slice(&pcm.samples);
            last.pcm = Arc::new(Pcm { rate: pcm.rate, channels: pcm.channels, samples });
        }
        _ => list.push(Patch { version, start, pcm }),
    }
    let over = list.len().saturating_sub(MOST_PATCHES);
    list.drain(..over);
    list
}

/// channel `c` of frame `frame` of `base`, with `patches` played over it, the later over the earlier
fn sample(base: &Pcm, patches: &[Patch], frame: usize, c: usize) -> f32 {
    let ch = base.channels.max(1) as usize;
    let c = c.min(ch - 1);
    let mut v = base.samples.get(frame * ch + c).copied().unwrap_or(0.0);
    for p in patches.iter().filter(|p| (p.start..p.end()).contains(&frame)) {
        let x = p.pcm.samples.get((frame - p.start) * ch + c).copied().unwrap_or(0.0);
        let w = ((frame - p.start).min(p.end() - 1 - frame) as f32 / FADE as f32).min(1.0);
        v += (x - v) * w;
    }
    v
}

/// shared between the UI thread and the audio callback
struct Shared {
    pcm: Mutex<Option<Arc<Pcm>>>,
    /// bumped whenever `pcm` is replaced
    generation: AtomicU64,
    /// what plays over `pcm` (`Patch`), gone with it
    patches: Mutex<Arc<Vec<Patch>>>,
    /// bumped whenever `patches` change
    patched: AtomicU64,
    playing: AtomicBool,
    /// seconds, f64 bits
    position: AtomicU64,
    seek: AtomicU64,
    looping: AtomicBool,
    loop_start: AtomicU64,
    loop_end: AtomicU64,
    volume: AtomicU32,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            pcm: Mutex::new(None),
            generation: AtomicU64::new(0),
            patches: Mutex::new(Arc::new(Vec::new())),
            patched: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            position: AtomicU64::new(0f64.to_bits()),
            seek: AtomicU64::new(NO_SEEK),
            looping: AtomicBool::new(false),
            loop_start: AtomicU64::new(0f64.to_bits()),
            loop_end: AtomicU64::new(0f64.to_bits()),
            volume: AtomicU32::new(0.9f32.to_bits()),
        }
    }
}

/// an endless stereo source at the device rate that plays whatever `Shared` holds
struct Output {
    shared: Arc<Shared>,
    rate: f64,
    pcm: Option<Arc<Pcm>>,
    generation: u64,
    patches: Arc<Vec<Patch>>,
    patched: u64,
    /// in source frames
    pos: f64,
    gain: f32,
    frame: [f32; 2],
    channel: usize,
}

impl Output {
    fn render_frame(&mut self) {
        let sh = &*self.shared;
        let generation = sh.generation.load(Relaxed);
        if generation != self.generation {
            self.generation = generation;
            self.pcm = sh.pcm.lock().map(|p| p.clone()).unwrap_or(None);
        }
        let patched = sh.patched.load(Relaxed);
        if patched != self.patched {
            self.patched = patched;
            self.patches = sh.patches.lock().map(|p| p.clone()).unwrap_or_default();
        }
        let Some(pcm) = self.pcm.as_deref() else {
            self.frame = [0.0; 2];
            return;
        };
        let src_rate = pcm.rate.max(1) as f64;
        let seek = sh.seek.swap(NO_SEEK, Relaxed);
        if seek != NO_SEEK {
            self.pos = (f64::from_bits(seek) * src_rate).clamp(0.0, pcm.frames() as f64);
            sh.position.store((self.pos / src_rate).to_bits(), Relaxed);
        }
        let playing = sh.playing.load(Relaxed);
        let target = if playing { f32::from_bits(sh.volume.load(Relaxed)) } else { 0.0 };
        // a ~5 ms ramp so pause / play never click
        self.gain += (target - self.gain).clamp(-0.004, 0.004);
        if !playing && self.gain <= 0.0 {
            self.gain = 0.0;
            self.frame = [0.0; 2];
            return;
        }
        let frames = pcm.frames();
        let ch = pcm.channels.max(1) as usize;
        let i = self.pos as usize;
        let t = (self.pos - i as f64) as f32;
        let patches = self.patches.as_slice();
        let at = |frame: usize, c: usize| sample(pcm, patches, frame, c);
        let lerp = |c: usize| at(i, c) + (at(i + 1, c) - at(i, c)) * t;
        let (l, r) = (lerp(0), if ch > 1 { lerp(1) } else { lerp(0) });
        self.frame = [l * self.gain, r * self.gain];

        self.pos += src_rate / self.rate;
        let secs = self.pos / src_rate;
        if sh.looping.load(Relaxed) {
            let a = f64::from_bits(sh.loop_start.load(Relaxed)).max(0.0);
            let b = f64::from_bits(sh.loop_end.load(Relaxed));
            let b = if b > a { b.min(pcm.seconds()) } else { pcm.seconds() };
            if secs >= b {
                self.pos = a * src_rate;
            }
        } else if self.pos >= frames as f64 {
            self.pos = 0.0;
            sh.playing.store(false, Relaxed);
        }
        sh.position.store((self.pos / src_rate).to_bits(), Relaxed);
    }
}

impl Iterator for Output {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            self.render_frame();
        }
        let s = self.frame[self.channel];
        self.channel ^= 1;
        Some(s)
    }
}

impl rodio::Source for Output {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        rodio::ChannelCount::new(2).expect("2 channels")
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        rodio::SampleRate::new(self.rate as u32).unwrap_or(rodio::SampleRate::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct Player {
    shared: Arc<Shared>,
    _device: Option<rodio::MixerDeviceSink>,
    error: Option<String>,
    duration: f64,
    loading: Option<(Receiver<Result<Pcm, String>>, bool)>,
    /// played before the first audio arrived: it plays as it arrives, from where it was sought meanwhile
    play_on_arrival: bool,
    seek_on_arrival: Option<f64>,
    ctx: egui::Context,
}

impl Player {
    pub fn new(ctx: &egui::Context) -> Self {
        let shared = Arc::new(Shared::default());
        let (device, error) = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut device) => {
                device.log_on_drop(false);
                let rate = device.config().sample_rate().get() as f64;
                device.mixer().add(Output {
                    shared: shared.clone(),
                    rate,
                    pcm: None,
                    generation: 0,
                    patches: Arc::new(Vec::new()),
                    patched: 0,
                    pos: 0.0,
                    gain: 0.0,
                    frame: [0.0; 2],
                    channel: 0,
                });
                (Some(device), None)
            }
            Err(e) => (None, Some(format!("no audio output: {e}"))),
        };
        Player {
            shared,
            _device: device,
            error,
            duration: 0.0,
            loading: None,
            play_on_arrival: false,
            seek_on_arrival: None,
            ctx: ctx.clone(),
        }
    }

    /// why playback is unavailable or the last load failed
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    /// load audio; `keep_position` continues from the current position (clamped) instead of the start
    pub fn load(&mut self, source: Source, keep_position: bool) {
        // a load forgets why the one before failed; that there is no output stays
        if self._device.is_some() {
            self.error = None;
        }
        // and lets go of the one still on its way, which is not this one
        self.loading = None;
        if !keep_position {
            (self.play_on_arrival, self.seek_on_arrival) = (false, None);
        }
        match source {
            Source::Pcm(pcm) => self.install(pcm, keep_position),
            Source::File(file) => match Pcm::decode_wav(&file.bytes) {
                Ok(pcm) => self.install(Arc::new(pcm), keep_position),
                // any other kind claudius reads (MP3, FLAC, AAC, Ogg, AIFF), decoded as claudius decodes it, off the
                // window's thread
                Err(_) => {
                    let (tx, rx) = channel();
                    let ctx = self.ctx.clone();
                    std::thread::spawn(move || {
                        let decoded = claudius_session::listen::decode(file.bytes.clone(), &file.name);
                        let _ = tx.send(decoded.map(|x| pcm_of(&x)));
                        ctx.request_repaint();
                    });
                    self.on_its_way(rx, keep_position);
                }
            },
            Source::Url(url) => {
                let (tx, rx) = channel();
                let ctx = self.ctx.clone();
                let mut request = ehttp::Request::get(&url);
                request.timeout = Some(Duration::from_secs(180));
                ehttp::fetch(request, move |res| {
                    let pcm = match res {
                        Ok(r) if r.ok => Pcm::decode_wav(&r.bytes),
                        Ok(r) => Err(format!("{url} answered {} {}", r.status, r.status_text)),
                        Err(e) => Err(format!("could not fetch {url}: {e}")),
                    };
                    let _ = tx.send(pcm);
                    ctx.request_repaint();
                });
                self.on_its_way(rx, keep_position);
            }
        }
    }

    /// Audio on its way. What played before plays on meanwhile when the new audio continues from it (`keep_position`,
    /// a render of the same song); otherwise it goes, so that nothing of it plays in the new audio's place.
    fn on_its_way(&mut self, rx: Receiver<Result<Pcm, String>>, keep_position: bool) {
        if !keep_position {
            self.let_go();
        }
        self.loading = Some((rx, keep_position));
    }

    /// stops, and forgets the audio installed
    fn let_go(&mut self) {
        self.pause();
        if let Ok(mut slot) = self.shared.pcm.lock() {
            *slot = None;
        }
        self.shared.generation.fetch_add(1, Relaxed);
        self.unpatch();
        self.shared.position.store(0f64.to_bits(), Relaxed);
        self.duration = 0.0;
    }

    fn install(&mut self, pcm: Arc<Pcm>, keep_position: bool) {
        let duration = pcm.seconds();
        let pos = if keep_position { self.position() } else { 0.0 };
        if let Ok(mut slot) = self.shared.pcm.lock() {
            *slot = Some(pcm);
        }
        self.shared.generation.fetch_add(1, Relaxed);
        self.unpatch();
        self.duration = duration;
        self.seek(if pos < duration { pos } else { 0.0 });
    }

    /// Plays `pcm`, the song's audio from `start` seconds on as the song is at `version` (a window of the live mix), in
    /// place of what plays there, until other audio is loaded. A window that follows on from the last of the same
    /// version joins it, so that no fade falls between them. Audio of another rate or channel count than what plays is
    /// not played.
    pub fn patch(&mut self, version: u64, start: f64, pcm: Arc<Pcm>) {
        let base = self.shared.pcm.lock().ok().and_then(|p| p.clone());
        let Some(base) = base.filter(|b| b.rate == pcm.rate && b.channels == pcm.channels) else { return };
        let start = (start.max(0.0) * base.rate as f64).round() as usize;
        let Ok(mut slot) = self.shared.patches.lock() else { return };
        *slot = Arc::new(with_patch(&slot, version, start, pcm));
        self.shared.patched.fetch_add(1, Relaxed);
    }

    /// what plays over the audio, gone
    fn unpatch(&mut self) {
        if let Ok(mut slot) = self.shared.patches.lock() {
            *slot = Arc::new(Vec::new());
        }
        self.shared.patched.fetch_add(1, Relaxed);
    }

    /// forget the loaded audio (the song it belonged to is gone)
    pub fn clear(&mut self) {
        self.let_go();
        self.loading = None;
        (self.play_on_arrival, self.seek_on_arrival) = (false, None);
    }

    /// call once per frame: picks up finished downloads
    pub fn tick(&mut self) {
        if let Some((rx, keep)) = &self.loading {
            let keep = *keep;
            if let Ok(result) = rx.try_recv() {
                self.loading = None;
                match result {
                    Ok(pcm) => {
                        if self._device.is_some() {
                            self.error = None;
                        }
                        self.install(Arc::new(pcm), keep);
                        if let Some(at) = self.seek_on_arrival.take() {
                            self.seek(at);
                        }
                        if std::mem::take(&mut self.play_on_arrival) {
                            self.play();
                        }
                    }
                    Err(e) => (self.error, self.play_on_arrival, self.seek_on_arrival) = (Some(e), false, None),
                }
            }
        }
    }

    pub fn has_audio(&self) -> bool {
        self.duration > 0.0
    }

    /// the audio loaded, decoded: what a take is recorded over
    pub fn pcm(&self) -> Option<Arc<Pcm>> {
        self.shared.pcm.lock().ok().and_then(|p| p.clone())
    }

    pub fn play(&mut self) {
        if self.has_audio() {
            self.shared.playing.store(true, Relaxed);
        } else if self.loading.is_some() {
            self.play_on_arrival = true;
        }
    }

    pub fn pause(&mut self) {
        self.shared.playing.store(false, Relaxed);
        self.play_on_arrival = false;
    }

    pub fn stop(&mut self) {
        self.pause();
        self.seek(0.0);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Relaxed)
    }

    pub fn seek(&mut self, secs: f64) {
        // sought while the audio is on its way: where it plays from once it is here
        if self.loading.is_some() {
            self.seek_on_arrival = Some(secs.max(0.0));
            return;
        }
        let secs = secs.clamp(0.0, self.duration.max(0.0));
        self.shared.seek.store(secs.to_bits(), Relaxed);
        self.shared.position.store(secs.to_bits(), Relaxed);
    }

    pub fn position(&self) -> f64 {
        if let Some(at) = self.seek_on_arrival {
            return at;
        }
        let seek = self.shared.seek.load(Relaxed);
        if seek != NO_SEEK { f64::from_bits(seek) } else { f64::from_bits(self.shared.position.load(Relaxed)) }
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// loop between `start` and `end` seconds (`end <= start` loops the whole render), or not at all
    pub fn set_loop(&mut self, region: Option<(f64, f64)>) {
        match region {
            Some((a, b)) => {
                self.shared.loop_start.store(a.to_bits(), Relaxed);
                self.shared.loop_end.store(b.to_bits(), Relaxed);
                self.shared.looping.store(true, Relaxed);
            }
            None => self.shared.looping.store(false, Relaxed),
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.shared.volume.store(volume.clamp(0.0, 1.5).to_bits(), Relaxed);
    }
}

/// a recording claudius decoded, at its rate, as the player plays it
fn pcm_of(x: &claudius::audio::Stereo) -> Pcm {
    Pcm { rate: claudius::config::SR, channels: 2, samples: x.l.iter().zip(&x.r).flat_map(|(l, r)| [*l, *r]).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(shared: &Arc<Shared>, rate: f64) -> Output {
        Output {
            shared: shared.clone(),
            rate,
            pcm: None,
            generation: 0,
            patches: Arc::new(Vec::new()),
            patched: 0,
            pos: 0.0,
            gain: 0.0,
            frame: [0.0; 2],
            channel: 0,
        }
    }

    #[test]
    fn a_window_of_the_live_mix_plays_in_the_renders_place_faded_in_and_out_and_the_next_one_joins_it() {
        let (rate, secs) = (1_000u32, 4usize);
        let base = Pcm { rate, channels: 1, samples: vec![0.5; rate as usize * secs] };
        let window = |v: f32, n: usize| Arc::new(Pcm { rate, channels: 1, samples: vec![v; n] });
        // a second of the song mixed anew from 1 s, and the next second of the same version after it
        let list = with_patch(&[], 7, 1_000, window(1.0, 1_000));
        let list = with_patch(&list, 7, 2_000, window(1.0, 1_000));
        assert_eq!(list.len(), 1, "two windows of a version that follow on are one stretch");
        assert_eq!((list[0].start, list[0].end()), (1_000, 3_000));
        let at = |frame: usize| sample(&base, &list, frame, 0);
        assert_eq!(
            (at(999), at(1_000), at(1_000 + FADE), at(2_000), at(2_999), at(3_000)),
            (0.5, 0.5, 1.0, 1.0, 0.5, 0.5)
        );
        let half = at(1_000 + FADE / 2);
        assert!(half > 0.5 && half < 1.0, "faded in: {half}");
        // a later version's window over it from 2.5 s plays over both
        let list = with_patch(&list, 8, 2_500, window(-1.0, 1_000));
        assert_eq!(list.len(), 2);
        let at = |frame: usize| sample(&base, &list, frame, 0);
        assert_eq!((at(2_000), at(2_500 + FADE), at(3_499 - FADE)), (1.0, -1.0, -1.0));
        // no more than the latest few are kept
        let many = (0..40).fold(Vec::new(), |l, k| with_patch(&l, k, k as usize * 10, window(0.0, 5)));
        assert_eq!((many.len(), many[0].version), (MOST_PATCHES, 40 - MOST_PATCHES as u64));
    }

    fn install(shared: &Shared, pcm: Pcm) {
        *shared.pcm.lock().unwrap() = Some(Arc::new(pcm));
        shared.generation.fetch_add(1, Relaxed);
    }

    fn pull(out: &mut Output, secs: f64) {
        for _ in 0..(secs * out.rate) as usize * 2 {
            out.next();
        }
    }

    fn position(shared: &Shared) -> f64 {
        f64::from_bits(shared.position.load(Relaxed))
    }

    #[test]
    fn plays_seeks_loops_and_stops() {
        let shared = Arc::new(Shared::default());
        let mut out = output(&shared, 48_000.0);
        install(&shared, Pcm { rate: 24_000, channels: 1, samples: vec![0.5; 24_000 * 4] });
        pull(&mut out, 0.5);
        assert_eq!(position(&shared), 0.0, "paused: the position holds");

        shared.playing.store(true, Relaxed);
        pull(&mut out, 1.0);
        assert!((position(&shared) - 1.0).abs() < 0.01, "plays at the source rate whatever the device rate");
        assert!(out.frame[0] > 0.3 && out.frame[1] > 0.3, "mono reaches both channels once the fade-in is done");

        shared.seek.store(3.0f64.to_bits(), Relaxed);
        pull(&mut out, 0.5);
        assert!((position(&shared) - 3.5).abs() < 0.01);

        shared.loop_start.store(1.0f64.to_bits(), Relaxed);
        shared.loop_end.store(1.5f64.to_bits(), Relaxed);
        shared.looping.store(true, Relaxed);
        pull(&mut out, 1.2);
        let p = position(&shared);
        assert!((1.0..1.5).contains(&p), "stays inside the loop region, at {p}");

        shared.looping.store(false, Relaxed);
        shared.seek.store(3.9f64.to_bits(), Relaxed);
        pull(&mut out, 0.3);
        assert!(!shared.playing.load(Relaxed), "stops at the end");
        assert!(position(&shared) < 0.01, "and rewinds");
    }

    #[test]
    fn a_player_sought_and_played_while_its_audio_is_on_its_way_plays_from_there_once_it_is_here() {
        let mut p = Player::new(&egui::Context::default());
        let (tx, rx) = channel();
        p.loading = Some((rx, false));
        p.seek(2.5);
        p.play();
        assert_eq!(p.position(), 2.5, "where it will play from");
        tx.send(Ok(Pcm { rate: 24_000, channels: 1, samples: vec![0.0; 24_000 * 4] })).unwrap();
        p.tick();
        // an output, where there is one, plays on from there at once
        assert!(p.is_playing() && (2.5..2.7).contains(&p.position()), "at {}", p.position());
        p.pause();
    }

    #[test]
    fn audio_on_its_way_lets_go_of_what_played_unless_it_continues_it_and_one_chosen_meanwhile_stays() {
        let silence = |secs: usize| Arc::new(Pcm { rate: 24_000, channels: 1, samples: vec![0.0; 24_000 * secs] });
        let mut p = Player::new(&egui::Context::default());
        p.load(Source::Pcm(silence(4)), false);
        p.play();
        // a render of the same song on its way: the last one plays on meanwhile
        let (_tx, rx) = channel();
        p.on_its_way(rx, true);
        assert!(p.has_audio() && p.is_playing());
        // another recording on its way: nothing of the last one plays in its place, and it plays once it is here
        let (tx, rx) = channel();
        p.on_its_way(rx, false);
        assert!(!p.has_audio() && !p.is_playing());
        p.play();
        assert!(!p.is_playing() && p.play_on_arrival);
        // one chosen meanwhile, there at once: the one on its way is let go of, and does not replace it later
        p.load(Source::Pcm(silence(2)), false);
        assert!(!p.is_loading() && (p.duration() - 2.0).abs() < 1e-9);
        let _ = tx.send(Ok(Pcm { rate: 24_000, channels: 1, samples: vec![0.0; 24_000 * 8] }));
        p.tick();
        assert!((p.duration() - 2.0).abs() < 1e-9, "{} s", p.duration());
        p.pause();
    }

    #[test]
    fn a_file_not_a_wav_is_decoded_as_claudius_decodes_one_and_plays_from_where_it_was_sought() {
        let silence = claudius::audio::Stereo::from_mono(&vec![0.0; claudius::config::SR as usize * 2]);
        let flac =
            claudius::export::encode(&silence, &"flac:24".parse().unwrap(), &claudius::export::tags("a")).unwrap();
        let mut p = Player::new(&egui::Context::default());
        let file = crate::pick::Picked { name: "phrases.flac".into(), bytes: flac, folder: None };
        p.load(Source::File(Arc::new(file)), false);
        assert!(p.is_loading(), "decoded off the window's thread");
        p.seek(1.0);
        p.play();
        for _ in 0..500 {
            p.tick();
            if !p.is_loading() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(p.error().is_none_or(|e| e.starts_with("no audio output")), "{:?}", p.error());
        assert!((p.duration() - 2.0).abs() < 0.05, "{} s", p.duration());
        assert!(p.is_playing() && (1.0..1.2).contains(&p.position()), "at {}", p.position());
        p.pause();
    }

    #[test]
    fn decodes_24_bit_wav() {
        let pcm = Pcm { rate: 48_000, channels: 2, samples: vec![0.25, -0.5, 0.75, -1.0] };
        let back = Pcm::decode_wav(&pcm.encode_wav()).unwrap();
        assert_eq!((back.rate, back.channels, back.samples.len()), (48_000, 2, 4));
        assert!((back.samples[1] + 0.5).abs() < 1e-3);
        let mut wav = Vec::new();
        {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 44_100,
                bits_per_sample: 24,
                sample_format: hound::SampleFormat::Int,
            };
            let mut w = hound::WavWriter::new(std::io::Cursor::new(&mut wav), spec).unwrap();
            for v in [0i32, 4_194_304, -8_388_608] {
                w.write_sample(v).unwrap();
            }
            w.finalize().unwrap();
        }
        let p = Pcm::decode_wav(&wav).unwrap();
        assert_eq!(p.samples, vec![0.0, 0.5, -1.0]);
    }
}
