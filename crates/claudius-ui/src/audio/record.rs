//! Recording a take, natively. While it records, the song plays through an output stream of the recorder's own, from
//! the count-in (with a click) on, and an input stream captures the microphone or the interface. Each stream's
//! timestamps give its latency (when what it plays is heard, when what it hears was captured), and the system clock
//! relates the two streams, whose own clocks need not share an origin, so the take is cut where the section began as
//! the person heard it. `offset` adds what the devices do not report (a laptop's speakers and microphone, Bluetooth
//! headphones), which a measurement finds (`Plan::measure`, `Recorder::lateness`).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use super::take::{Mix, cut, fill, hushed, notes_of, pair, wav};
pub use super::take::{Plan, Played, Take};

/// an input the person can record from
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub name: String,
    pub channels: u16,
}

/// the inputs there are, the default one first
pub fn inputs() -> Vec<Input> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|d| d.to_string());
    let mut out: Vec<Input> = host
        .input_devices()
        .map(|devices| {
            devices
                .filter_map(|d| {
                    Some(Input { channels: d.default_input_config().ok()?.channels(), name: d.to_string() })
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(i) = default.and_then(|name| out.iter().position(|d| d.name == name)) {
        let d = out.remove(i);
        out.insert(0, d);
    }
    out
}

/// what a MIDI input played: seconds since the recorder began, pitch, velocity (0: the note ended)
type Heard = Arc<Mutex<Vec<(f64, u8, f32)>>>;

/// what the Record window offers for MIDI keyboards while it may not use them: nothing, in the app (a browser asks)
pub fn midi_offer() -> Option<(String, bool)> {
    None
}

/// a browser asks the person whether the page may use MIDI keyboards; the app need not
pub fn ask_for_midi() {}

/// the MIDI inputs there are (keyboards and controllers), by name
pub fn midi_inputs() -> Vec<String> {
    let Ok(input) = midir::MidiInput::new("claudius") else { return Vec::new() };
    input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect()
}

/// the soft tone that plays what a MIDI input plays while a take records, for a keyboard with no sound of its own
#[derive(Default)]
struct Guide {
    voices: Vec<Voice>,
}

struct Voice {
    pitch: u8,
    hz: f64,
    phase: f64,
    level: f32,
    env: f32,
    held: bool,
}

impl Guide {
    /// a note on (velocity above 0) or off
    fn note(&mut self, pitch: u8, velocity: f32) {
        if velocity > 0.0 {
            self.voices.retain(|v| v.pitch != pitch);
            if self.voices.len() >= 16 {
                self.voices.remove(0);
            }
            let hz = 440.0 * 2f64.powf((f64::from(pitch) - 69.0) / 12.0);
            self.voices.push(Voice { pitch, hz, phase: 0.0, level: velocity, env: 0.0, held: true });
        } else {
            for v in self.voices.iter_mut().filter(|v| v.pitch == pitch) {
                v.held = false;
            }
        }
    }

    /// adds the sounding notes into interleaved `out`: a sine and a little of its octave, 3 ms on and 150 ms off
    fn add(&mut self, out: &mut [f32], channels: usize, rate: f64) {
        let (on, off) = ((-1.0 / (0.003 * rate)).exp() as f32, (-1.0 / (0.15 * rate)).exp() as f32);
        for v in self.voices.iter_mut() {
            let step = std::f64::consts::TAU * v.hz / rate;
            for frame in out.chunks_exact_mut(channels.max(1)) {
                v.env = if v.held { 1.0 - (1.0 - v.env) * on } else { v.env * off };
                let s = (v.phase.sin() + 0.25 * (2.0 * v.phase).sin()) as f32 * v.env * v.level * 0.18;
                v.phase += step;
                for c in frame.iter_mut().take(2) {
                    *c += s;
                }
            }
        }
        self.voices.retain(|v| v.held || v.env > 1e-4);
    }
}

/// the earliest of a run of estimates of one instant, after the first few: a callback runs late, never early, so the
/// earliest is the least delayed
#[derive(Clone, Copy, Debug, Default)]
struct Earliest {
    seen: u32,
    best: Option<f64>,
}

impl Earliest {
    const SKIP: u32 = 4;

    fn add(&mut self, t: f64) {
        self.seen += 1;
        self.best = Some(match self.best {
            Some(b) if self.seen > Self::SKIP + 1 => b.min(t),
            _ => t,
        });
    }
}

struct Shared {
    /// frames the output has played, at its rate
    played: AtomicU64,
    /// the loudest input sample since the meter last read it (f32 bits: for values >= 0 their order is the floats')
    level: AtomicU32,
    /// when the output's first frame was heard, in seconds since the recorder began
    heard: Mutex<Earliest>,
    /// when a stream dropped audio (an overrun or underrun), in seconds since the recorder began
    dropped: Mutex<Vec<f64>>,
    failed: Mutex<Option<String>>,
}

impl Shared {
    /// a stream's error: dropped audio, or a route that changed, the stream goes on through; anything else ends it
    fn error(&self, which: &str, e: cpal::Error, epoch: Instant) {
        match e.kind() {
            cpal::ErrorKind::Xrun => {
                if let Ok(mut d) = self.dropped.lock() {
                    d.push(epoch.elapsed().as_secs_f64());
                }
            }
            cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied => {}
            _ => {
                if let Ok(mut f) = self.failed.lock() {
                    f.get_or_insert_with(|| format!("the {which} stopped: {e}"));
                }
            }
        }
    }
}

/// a chunk of captured input: its samples (the take's channels, interleaved) and when its first was captured
struct Chunk {
    samples: Vec<f32>,
    captured: f64,
}

pub struct Recorder {
    input: Option<cpal::Stream>,
    output: Option<cpal::Stream>,
    /// a MIDI take: the input, and what it played
    midi: Option<midir::MidiInputConnection<()>>,
    played: Option<Heard>,
    shared: Arc<Shared>,
    rx: Receiver<Chunk>,
    taken: Vec<f32>,
    frames: u64,
    /// when the input's first frame was captured, in seconds since the recorder began
    began: Earliest,
    in_rate: f64,
    channels: usize,
    from: f64,
    count_in: f64,
    offset: f64,
    epoch: Instant,
    /// what it plays through and records from, by name
    route: String,
}

/// the input of that name, else the default one
fn input_device(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    name.and_then(|n| host.input_devices().ok().and_then(|mut ds| ds.find(|d| d.to_string() == n)))
        .or_else(|| host.default_input_device())
        .ok_or_else(|| "no microphone or audio input".to_string())
}

impl Shared {
    fn new() -> Arc<Shared> {
        Arc::new(Shared {
            played: AtomicU64::new(0),
            level: AtomicU32::new(0),
            heard: Mutex::new(Earliest::default()),
            dropped: Mutex::new(Vec::new()),
            failed: Mutex::new(None),
        })
    }
}

/// an input stream of the chosen channels, whatever the device's sample format; returns it and its rate
fn open_input(
    input: &cpal::Device,
    channel: usize,
    stereo: bool,
    tx: std::sync::mpsc::Sender<Chunk>,
    shared: Arc<Shared>,
    epoch: Instant,
) -> Result<(cpal::Stream, f64, usize), String> {
    let config = input.default_input_config().map_err(|e| format!("{input}: {e}"))?;
    let of = config.channels() as usize;
    let channels = if stereo && channel + 1 < of { 2 } else { 1 };
    if channel + channels > of {
        return Err(format!("{input} has {of} input channel(s)"));
    }
    let pick = Pick { first: channel, channels, of };
    let (c, sh) = (config.config(), shared);
    let stream = match config.sample_format() {
        SampleFormat::F32 => input_stream::<f32>(input, c, pick, tx, sh, epoch),
        SampleFormat::I16 => input_stream::<i16>(input, c, pick, tx, sh, epoch),
        SampleFormat::I32 => input_stream::<i32>(input, c, pick, tx, sh, epoch),
        SampleFormat::I24 => input_stream::<cpal::I24>(input, c, pick, tx, sh, epoch),
        SampleFormat::U16 => input_stream::<u16>(input, c, pick, tx, sh, epoch),
        other => return Err(format!("{input} gives {other} samples, which claudius does not read")),
    }?;
    Ok((stream, config.sample_rate() as f64, channels))
}

/// the input alone, for the meter while the Record window is open and no take records
pub struct Listener {
    _stream: Option<cpal::Stream>,
    _midi: Option<midir::MidiInputConnection<()>>,
    shared: Arc<Shared>,
}

impl Listener {
    pub fn start(input: Option<&str>, first: usize, stereo: bool) -> Result<Listener, String> {
        let device = input_device(input)?;
        let shared = Shared::new();
        // only the meter is wanted: what the stream captures goes nowhere
        let (tx, _) = channel();
        let (stream, _, _) = open_input(&device, first, stereo, tx, shared.clone(), Instant::now())?;
        stream.play().map_err(|e| format!("cannot listen to {device}: {e}"))?;
        Ok(Listener { _stream: Some(stream), _midi: None, shared })
    }

    /// a MIDI input, the meter showing how hard each note is played
    pub fn midi(name: &str) -> Result<Listener, String> {
        let shared = Shared::new();
        let sh = shared.clone();
        let midi = connect_midi(name, move |msg| {
            for (_, v) in notes_of(msg) {
                sh.level.fetch_max(v.to_bits(), Relaxed);
            }
        })?;
        Ok(Listener { _stream: None, _midi: Some(midi), shared })
    }

    /// the loudest input since the last call, 0..1
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.swap(0, Relaxed))
    }

    pub fn error(&self) -> Option<String> {
        self.shared.failed.lock().ok().and_then(|f| f.clone())
    }
}

/// a MIDI input, by name, handing every message it receives to `f`
fn connect_midi(
    name: &str,
    mut f: impl FnMut(&[u8]) + Send + 'static,
) -> Result<midir::MidiInputConnection<()>, String> {
    let input = midir::MidiInput::new("claudius").map_err(|e| format!("no MIDI on this computer: {e}"))?;
    let ports = input.ports();
    let port = ports
        .iter()
        .find(|p| input.port_name(p).is_ok_and(|n| n == name))
        .ok_or_else(|| format!("no MIDI input called {name} any more"))?;
    input.connect(port, "claudius", move |_, msg, _| f(msg), ()).map_err(|e| format!("cannot listen to {name}: {e}"))
}

impl Recorder {
    pub fn start(plan: Plan) -> Result<Recorder, String> {
        let output = cpal::default_host().default_output_device().ok_or("no audio output to play the song through")?;
        let out_config = output.default_output_config().map_err(|e| format!("{output}: {e}"))?;
        let epoch = Instant::now();
        let shared = Shared::new();
        let (tx, rx) = channel();
        let (guide_tx, guide_rx) = channel();
        // the input starts first, so that it hears from before the song starts
        let mut route = output.to_string();
        let (input_stream, midi, played, in_rate, channels) = match &plan.midi {
            Some(name) => {
                let played: Heard = Arc::default();
                let (heard, sh) = (played.clone(), shared.clone());
                let midi = connect_midi(name, move |msg| {
                    let now = epoch.elapsed().as_secs_f64();
                    for (p, v) in notes_of(msg) {
                        if let Ok(mut h) = heard.lock() {
                            h.push((now, p, v));
                        }
                        sh.level.fetch_max(v.to_bits(), Relaxed);
                        let _ = guide_tx.send((p, v));
                    }
                })?;
                route = format!("{route} and {name}");
                (None, Some(midi), Some(played), 1.0, 1)
            }
            None => {
                let input = input_device(plan.input.as_deref())?;
                let (stream, rate, channels) =
                    open_input(&input, plan.channel, plan.stereo, tx, shared.clone(), epoch)?;
                stream.play().map_err(|e| format!("cannot record from {input}: {e}"))?;
                route = format!("{route} and {input}");
                (Some(stream), None, None, rate, channels)
            }
        };
        let guide = midi.as_ref().map(|_| guide_rx);
        let mix = Mix::of(&plan);
        let (sh, c) = (shared.clone(), out_config.config());
        let output_stream = match out_config.sample_format() {
            SampleFormat::F32 => output_stream::<f32>(&output, c, mix, guide, sh, epoch),
            SampleFormat::I16 => output_stream::<i16>(&output, c, mix, guide, sh, epoch),
            SampleFormat::I32 => output_stream::<i32>(&output, c, mix, guide, sh, epoch),
            other => return Err(format!("{output} takes {other} samples, which claudius does not write")),
        }?;
        output_stream.play().map_err(|e| format!("cannot play through {output}: {e}"))?;
        Ok(Recorder {
            input: input_stream,
            output: Some(output_stream),
            midi,
            played,
            shared,
            rx,
            taken: Vec::new(),
            frames: 0,
            began: Earliest::default(),
            in_rate,
            channels,
            from: plan.start - plan.count_in,
            count_in: plan.count_in,
            offset: plan.offset,
            epoch,
            route,
        })
    }

    /// what it plays through and records from: "MacBook Pro Speakers and MacBook Pro Microphone"
    pub fn route(&self) -> &str {
        &self.route
    }

    /// call every frame: takes in what the input captured since
    pub fn tick(&mut self) {
        while let Ok(c) = self.rx.try_recv() {
            self.began.add(c.captured - self.frames as f64 / self.in_rate);
            self.frames += (c.samples.len() / self.channels) as u64;
            self.taken.extend_from_slice(&c.samples);
        }
    }

    /// what the input captured arrives as it goes: there is nothing to wait for before `finish`
    pub fn stop_soon(&self) {}

    pub fn drained(&self) -> bool {
        true
    }

    /// the song is playing: the output has played its first frames
    pub fn playing(&self) -> bool {
        self.shared.heard.lock().is_ok_and(|h| h.best.is_some())
    }

    /// where in the song the person hears it now, in seconds
    pub fn time(&self) -> f64 {
        let heard = self.shared.heard.lock().map(|h| h.best).unwrap_or(None);
        heard.map_or(self.from, |h| self.from + (self.epoch.elapsed().as_secs_f64() - h).max(0.0))
    }

    /// the loudest input since the last call, 0..1
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.swap(0, Relaxed))
    }

    /// what went wrong with a stream, if anything did
    pub fn error(&self) -> Option<String> {
        self.shared.failed.lock().ok().and_then(|f| f.clone())
    }

    /// stops the streams and takes in the last of what the input captured; returns when, in seconds since it began
    fn stop(&mut self) -> f64 {
        let stop = self.epoch.elapsed().as_secs_f64();
        self.input.take();
        self.midi.take();
        self.output.take();
        self.tick();
        stop
    }

    /// when the output's first frame was heard, in seconds since the recorder began
    fn heard(&self) -> Result<f64, String> {
        self.shared.heard.lock().ok().and_then(|h| h.best).ok_or_else(|| "the song never played".into())
    }

    /// stops a measurement (`Plan::measure`), and says how late its clicks came back, in seconds: what a take cut the
    /// same way would land late by
    pub fn lateness(mut self) -> Result<f64, String> {
        self.stop();
        let heard = self.heard()?;
        let began = self.began.best.ok_or("nothing was recorded")?;
        let first: Vec<f32> = cut(&self.taken, self.channels, self.in_rate, heard - began)
            .into_iter()
            .step_by(self.channels.max(1))
            .collect();
        super::latency::lateness(&first, self.in_rate)
    }

    /// stops, and cuts the take from where the song reached the take's start, as heard, to now
    pub fn finish(mut self) -> Result<Take, String> {
        let stop = self.stop();
        let heard = self.heard()?;
        if let Some(played) = &self.played {
            let events = played.lock().map(|e| e.clone()).unwrap_or_default();
            let notes = pair(&events, heard + self.count_in + self.offset, stop);
            if notes.is_empty() {
                return Err("nothing was played after the count-in".into());
            }
            return Ok(Take::Notes(notes));
        }
        let began = self.began.best.ok_or("nothing was recorded")?;
        let at = heard + self.count_in + self.offset - began;
        let samples = cut(&self.taken, self.channels, self.in_rate, at);
        if samples.is_empty() {
            return Err("the take is empty: it stopped before the count-in was over".into());
        }
        let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
        let secs = samples.len() as f64 / self.channels as f64 / self.in_rate;
        // the streams often drop a buffer as they start: only what the take heard counts
        let take_began = heard + self.count_in;
        let dropouts = self.shared.dropped.lock().map_or(0, |d| d.iter().filter(|t| **t >= take_began).count());
        let hushed = hushed(&samples, self.channels, self.in_rate);
        Ok(Take::Audio { wav: wav(&samples, self.channels as u16, self.in_rate as u32), secs, peak, dropouts, hushed })
    }
}

/// which of an input's channels a take records
#[derive(Clone, Copy)]
struct Pick {
    first: usize,
    channels: usize,
    of: usize,
}

fn input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    pick: Pick,
    tx: std::sync::mpsc::Sender<Chunk>,
    shared: Arc<Shared>,
    epoch: Instant,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let errors = shared.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], info: &cpal::InputCallbackInfo| {
                let now = epoch.elapsed().as_secs_f64();
                let ts = info.timestamp();
                let latency = ts.callback.duration_since(ts.capture).as_secs_f64();
                let mut samples = Vec::with_capacity(data.len() / pick.of.max(1) * pick.channels);
                let mut peak = 0f32;
                for frame in data.chunks_exact(pick.of.max(1)) {
                    for &s in &frame[pick.first..pick.first + pick.channels] {
                        let v = f32::from_sample(s);
                        peak = peak.max(v.abs());
                        samples.push(v);
                    }
                }
                shared.level.fetch_max(peak.to_bits(), Relaxed);
                let _ = tx.send(Chunk { samples, captured: now - latency });
            },
            move |e| errors.error("input", e, epoch),
            None,
        )
        .map_err(|e| format!("cannot record from {device}: {e}"))
}

fn output_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mix: Mix,
    guide: Option<Receiver<(u8, f32)>>,
    shared: Arc<Shared>,
    epoch: Instant,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let (channels, rate) = (config.channels as usize, config.sample_rate as f64);
    let errors = shared.clone();
    let mut buf = Vec::new();
    let mut tone = Guide::default();
    device
        .build_output_stream::<T, _, _>(
            config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let now = epoch.elapsed().as_secs_f64();
                let ts = info.timestamp();
                let latency = ts.playback.duration_since(ts.callback).as_secs_f64();
                let first = shared.played.load(Relaxed);
                if let Ok(mut h) = shared.heard.lock() {
                    h.add(now + latency - first as f64 / rate);
                }
                buf.resize(data.len(), 0.0);
                fill(&mut buf, channels, rate, first, &mix);
                if let Some(notes) = &guide {
                    while let Ok((p, v)) = notes.try_recv() {
                        tone.note(p, v);
                    }
                    tone.add(&mut buf, channels, rate);
                }
                for (d, s) in data.iter_mut().zip(&buf) {
                    *d = T::from_sample(*s);
                }
                shared.played.fetch_add((data.len() / channels.max(1)) as u64, Relaxed);
            },
            move |e| errors.error("output", e, epoch),
            None,
        )
        .map_err(|e| format!("cannot play through {device}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_earliest_estimate_after_the_first_few_wins() {
        let mut e = Earliest::default();
        for t in [0.5, 0.1, 0.9, 0.9, 1.2, 1.1, 1.0, 1.4] {
            e.add(t);
        }
        assert_eq!(e.best, Some(1.0), "the first four are the streams settling");
        let mut once = Earliest::default();
        once.add(0.7);
        assert_eq!(once.best, Some(0.7), "one is better than none");
    }

    #[test]
    fn the_guide_tone_plays_while_a_key_is_down_and_fades_after() {
        let rate = 48_000.0;
        let mut g = Guide::default();
        g.note(69, 1.0);
        let mut on = vec![0.0; 4_800 * 2];
        g.add(&mut on, 2, rate);
        assert!(on.iter().skip(2_000).any(|s| s.abs() > 0.1) && on[1] == on[0], "both sides, while held");
        g.note(69, 0.0);
        let mut off = vec![0.0; 72_000 * 2];
        g.add(&mut off, 2, rate);
        assert!(
            off[off.len() - 2].abs() < 1e-4 && g.voices.is_empty(),
            "gone a second and a half after the key came up"
        );
    }
}
