use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

use super::{Pcm, Source};

const NO_SEEK: u64 = u64::MAX;

/// shared between the UI thread and the audio callback
struct Shared {
    pcm: Mutex<Option<Arc<Pcm>>>,
    /// bumped whenever `pcm` is replaced
    generation: AtomicU64,
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
        let at = |frame: usize, c: usize| pcm.samples.get(frame * ch + c.min(ch - 1)).copied().unwrap_or(0.0);
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
                    pos: 0.0,
                    gain: 0.0,
                    frame: [0.0; 2],
                    channel: 0,
                });
                (Some(device), None)
            }
            Err(e) => (None, Some(format!("no audio output: {e}"))),
        };
        Player { shared, _device: device, error, duration: 0.0, loading: None, ctx: ctx.clone() }
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
        match source {
            Source::Pcm(pcm) => self.install(pcm, keep_position),
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
                self.loading = Some((rx, keep_position));
            }
        }
    }

    fn install(&mut self, pcm: Arc<Pcm>, keep_position: bool) {
        let duration = pcm.seconds();
        let pos = if keep_position { self.position() } else { 0.0 };
        if let Ok(mut slot) = self.shared.pcm.lock() {
            *slot = Some(pcm);
        }
        self.shared.generation.fetch_add(1, Relaxed);
        self.duration = duration;
        self.seek(if pos < duration { pos } else { 0.0 });
    }

    /// forget the loaded audio (the song it belonged to is gone)
    pub fn clear(&mut self) {
        self.pause();
        if let Ok(mut slot) = self.shared.pcm.lock() {
            *slot = None;
        }
        self.shared.generation.fetch_add(1, Relaxed);
        self.shared.position.store(0f64.to_bits(), Relaxed);
        self.duration = 0.0;
        self.loading = None;
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
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

    pub fn has_audio(&self) -> bool {
        self.duration > 0.0
    }

    pub fn play(&mut self) {
        if self.has_audio() {
            self.shared.playing.store(true, Relaxed);
        }
    }

    pub fn pause(&mut self) {
        self.shared.playing.store(false, Relaxed);
    }

    pub fn stop(&mut self) {
        self.pause();
        self.seek(0.0);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Relaxed)
    }

    pub fn seek(&mut self, secs: f64) {
        let secs = secs.clamp(0.0, self.duration.max(0.0));
        self.shared.seek.store(secs.to_bits(), Relaxed);
        self.shared.position.store(secs.to_bits(), Relaxed);
    }

    pub fn position(&self) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn output(shared: &Arc<Shared>, rate: f64) -> Output {
        Output {
            shared: shared.clone(),
            rate,
            pcm: None,
            generation: 0,
            pos: 0.0,
            gain: 0.0,
            frame: [0.0; 2],
            channel: 0,
        }
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
