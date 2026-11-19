//! The radio's player (design 0024): one stream of the station's songs and of the changes between them, each a piece
//! placed at a frame of the stream, played from a device sink of the radio's own. The station places the pieces ahead
//! of where the stream is (`clock`) and may place them again (a skip); the audio thread plays whatever covers its frame.

use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use claudius::audio::Stereo;

/// the rate the station's songs are rendered at, and the stream counts its frames in
const RATE: f64 = 48_000.0;

/// A piece of the stream: frames `from..from + len` of `audio`, from frame `at` of the stream on.
#[derive(Clone)]
pub struct Placed {
    pub at: u64,
    pub audio: Arc<Stereo>,
    pub from: usize,
    pub len: usize,
}

impl Placed {
    pub fn end(&self) -> u64 {
        self.at + self.len as u64
    }
}

/// shared between the station and the audio thread
struct Shared {
    program: Mutex<Arc<Vec<Placed>>>,
    /// bumped whenever `program` is placed again
    generation: AtomicU64,
    playing: AtomicBool,
    volume: AtomicU32,
    /// the frame of the stream the audio thread has reached, as f64 bits
    clock: AtomicU64,
}

pub struct RadioPlayer {
    shared: Arc<Shared>,
    _device: Option<rodio::MixerDeviceSink>,
    error: Option<String>,
}

impl RadioPlayer {
    pub fn new() -> RadioPlayer {
        let shared = Arc::new(Shared {
            program: Mutex::new(Arc::new(Vec::new())),
            generation: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            volume: AtomicU32::new(0.8f32.to_bits()),
            clock: AtomicU64::new(0f64.to_bits()),
        });
        let (device, error) = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut device) => {
                device.log_on_drop(false);
                let rate = device.config().sample_rate().get() as f64;
                device.mixer().add(Output {
                    shared: shared.clone(),
                    rate,
                    program: Arc::new(Vec::new()),
                    generation: 0,
                    t: 0.0,
                    piece: 0,
                    gain: 0.0,
                    frame: [0.0; 2],
                    channel: 0,
                });
                (Some(device), None)
            }
            Err(e) => (None, Some(format!("no audio output: {e}"))),
        };
        RadioPlayer { shared, _device: device, error }
    }

    /// why the radio cannot play
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// the stream as it is placed now
    pub fn program(&self) -> Arc<Vec<Placed>> {
        self.shared.program.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// places the stream's pieces, in order, none over another
    pub fn place(&self, program: Vec<Placed>) {
        if let Ok(mut p) = self.shared.program.lock() {
            *p = Arc::new(program);
        }
        self.shared.generation.fetch_add(1, Relaxed);
    }

    /// the frame of the stream it has reached
    pub fn clock(&self) -> u64 {
        f64::from_bits(self.shared.clock.load(Relaxed)) as u64
    }

    pub fn play(&self) {
        self.shared.playing.store(true, Relaxed);
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Relaxed);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Relaxed)
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.shared.volume.load(Relaxed))
    }

    pub fn set_volume(&self, v: f32) {
        self.shared.volume.store(v.clamp(0.0, 1.0).to_bits(), Relaxed);
    }
}

/// an endless stereo source at the device rate that plays the stream's pieces as they are placed
struct Output {
    shared: Arc<Shared>,
    rate: f64,
    program: Arc<Vec<Placed>>,
    generation: u64,
    /// the frame of the stream, fractional where the device runs at another rate
    t: f64,
    /// the piece that covered the last frame
    piece: usize,
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
            self.program = sh.program.lock().map(|p| p.clone()).unwrap_or_default();
            self.piece = 0;
        }
        let playing = sh.playing.load(Relaxed);
        let target = if playing { f32::from_bits(sh.volume.load(Relaxed)) } else { 0.0 };
        // a ~5 ms ramp so pause and play never click
        self.gain += (target - self.gain).clamp(-0.004, 0.004);
        if !playing && self.gain <= 0.0 {
            self.gain = 0.0;
            self.frame = [0.0; 2];
            return;
        }
        self.frame = sample(&self.program, &mut self.piece, self.t).map(|v| v * self.gain);
        self.t += RATE / self.rate;
        sh.clock.store(self.t.to_bits(), Relaxed);
    }
}

/// the stream's frame `t` (fractional) as `program` places it, `piece` the one that covered the frame before; silence
/// where no piece does
fn sample(program: &[Placed], piece: &mut usize, t: f64) -> [f32; 2] {
    let frame = t as u64;
    if program.get(*piece).is_none_or(|p| frame < p.at || frame >= p.end()) {
        *piece = program.partition_point(|p| p.end() <= frame);
    }
    let Some(p) = program.get(*piece).filter(|p| frame >= p.at && frame < p.end()) else { return [0.0; 2] };
    let i = p.from + (frame - p.at) as usize;
    let f = (t - frame as f64) as f32;
    let last = p.from + p.len - 1;
    let at = |src: &[f32], k: usize| src.get(k.min(last)).copied().unwrap_or(0.0);
    let lerp = |src: &[f32]| at(src, i) + (at(src, i + 1) - at(src, i)) * f;
    [lerp(&p.audio.l), lerp(&p.audio.r)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, v: f32) -> Arc<Stereo> {
        Arc::new(Stereo::from_mono(&vec![v; n]))
    }

    #[test]
    fn the_stream_plays_each_piece_where_it_is_placed_and_silence_between() {
        let program = vec![
            Placed { at: 0, audio: tone(100, 0.25), from: 10, len: 50 },
            Placed { at: 50, audio: tone(10, 0.5), from: 0, len: 10 },
            Placed { at: 70, audio: tone(100, 0.75), from: 40, len: 60 },
        ];
        let mut piece = 0;
        let at = |t: f64, piece: &mut usize| sample(&program, piece, t)[0];
        assert_eq!(at(0.0, &mut piece), 0.25);
        assert_eq!(at(49.0, &mut piece), 0.25);
        assert_eq!(at(55.0, &mut piece), 0.5);
        assert_eq!(at(65.0, &mut piece), 0.0, "between the pieces");
        assert_eq!(at(70.0, &mut piece), 0.75);
        assert_eq!(at(129.0, &mut piece), 0.75);
        assert_eq!(at(130.0, &mut piece), 0.0, "past the last");
        // found again from anywhere, as after the stream is placed again
        let mut fresh = 2;
        assert_eq!(at(10.0, &mut fresh), 0.25);
        assert_eq!(fresh, 0);
    }
}
