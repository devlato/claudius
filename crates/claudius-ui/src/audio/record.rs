//! Recording a take, natively. While it records, the song plays through an output stream of the recorder's own, from
//! the count-in (with a click) on, and an input stream captures the microphone or the interface. Each stream's
//! timestamps give its latency (when what it plays is heard, when what it hears was captured), and the system clock
//! relates the two streams, whose own clocks need not share an origin, so the take is cut where the section began as
//! the person heard it. `offset` adds what the devices do not report (Bluetooth headphones, say).

use std::io::Cursor;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use super::Pcm;

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

/// what to record, and what plays meanwhile
pub struct Plan {
    /// the input, by name (the default input when there is none of that name)
    pub input: Option<String>,
    /// the first of the input's channels to record, and whether the one after it too (a stereo take)
    pub channel: usize,
    pub stereo: bool,
    /// the song as rendered
    pub song: Arc<Pcm>,
    /// where the take starts in the song, and the count-in before it, in seconds
    pub start: f64,
    pub count_in: f64,
    /// seconds per beat and beats per bar, for the click
    pub beat: f64,
    pub bar: u32,
    /// the click goes on while the take records, not only through the count-in
    pub click: bool,
    /// seconds the setup adds that the devices do not report
    pub offset: f64,
}

/// a recorded take
pub struct Take {
    /// a 32-bit float WAV file at the input's rate
    pub wav: Vec<u8>,
    pub secs: f64,
    /// the loudest sample, 0..1
    pub peak: f32,
    /// how often a stream dropped audio while the take recorded (each a click or a gap in it)
    pub dropouts: usize,
}

/// what the output plays: the song from `from` seconds on, and a click until `click_until`
#[derive(Clone)]
struct Mix {
    song: Arc<Pcm>,
    from: f64,
    start: f64,
    click_until: f64,
    beat: f64,
    bar: u32,
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
    _stream: cpal::Stream,
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
        Ok(Listener { _stream: stream, shared })
    }

    /// the loudest input since the last call, 0..1
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.swap(0, Relaxed))
    }

    pub fn error(&self) -> Option<String> {
        self.shared.failed.lock().ok().and_then(|f| f.clone())
    }
}

impl Recorder {
    pub fn start(plan: Plan) -> Result<Recorder, String> {
        let input = input_device(plan.input.as_deref())?;
        let output = cpal::default_host().default_output_device().ok_or("no audio output to play the song through")?;
        let out_config = output.default_output_config().map_err(|e| format!("{output}: {e}"))?;
        let epoch = Instant::now();
        let shared = Shared::new();
        let (tx, rx) = channel();
        let (input_stream, in_rate, channels) =
            open_input(&input, plan.channel, plan.stereo, tx, shared.clone(), epoch)?;
        let mix = Mix {
            song: plan.song.clone(),
            from: plan.start - plan.count_in,
            start: plan.start,
            click_until: if plan.click { f64::INFINITY } else { plan.start },
            beat: plan.beat,
            bar: plan.bar.max(1),
        };
        let sh = shared.clone();
        let output_stream = match out_config.sample_format() {
            SampleFormat::F32 => output_stream::<f32>(&output, out_config.config(), mix, sh, epoch),
            SampleFormat::I16 => output_stream::<i16>(&output, out_config.config(), mix, sh, epoch),
            SampleFormat::I32 => output_stream::<i32>(&output, out_config.config(), mix, sh, epoch),
            other => return Err(format!("{output} takes {other} samples, which claudius does not write")),
        }?;
        // the input first, so that it has been capturing before the song starts
        input_stream.play().map_err(|e| format!("cannot record from {input}: {e}"))?;
        output_stream.play().map_err(|e| format!("cannot play through {output}: {e}"))?;
        Ok(Recorder {
            input: Some(input_stream),
            output: Some(output_stream),
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
        })
    }

    /// call every frame: takes in what the input captured since
    pub fn tick(&mut self) {
        while let Ok(c) = self.rx.try_recv() {
            self.began.add(c.captured - self.frames as f64 / self.in_rate);
            self.frames += (c.samples.len() / self.channels) as u64;
            self.taken.extend_from_slice(&c.samples);
        }
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

    /// stops, and cuts the take from where the song reached the take's start, as heard, to now
    pub fn finish(mut self) -> Result<Take, String> {
        self.input.take();
        self.output.take();
        self.tick();
        let heard = self.shared.heard.lock().ok().and_then(|h| h.best).ok_or("the song never played")?;
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
        Ok(Take { wav: wav(&samples, self.channels as u16, self.in_rate as u32), secs, peak, dropouts })
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
    shared: Arc<Shared>,
    epoch: Instant,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let (channels, rate) = (config.channels as usize, config.sample_rate as f64);
    let errors = shared.clone();
    let mut buf = Vec::new();
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

/// The output's frames from `first` on: the song from `mix.from` seconds (silence before its start and after its end),
/// and a click on every beat (counted from the take's start, the bar's first louder) until `mix.click_until`.
fn fill(out: &mut [f32], channels: usize, rate: f64, first: u64, mix: &Mix) {
    let ch = channels.max(1);
    let song_ch = mix.song.channels.max(1) as usize;
    let song_frames = mix.song.frames();
    for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
        let t = mix.from + (first + i as u64) as f64 / rate;
        let (mut l, mut r) = (0f32, 0f32);
        let pos = t * mix.song.rate as f64;
        if pos >= 0.0 && (pos as usize) + 1 < song_frames {
            let (k, x) = (pos as usize, (pos - pos.floor()) as f32);
            let at = |frame: usize, c: usize| mix.song.samples[frame * song_ch + c.min(song_ch - 1)];
            l = at(k, 0) + (at(k + 1, 0) - at(k, 0)) * x;
            r = at(k, 1) + (at(k + 1, 1) - at(k, 1)) * x;
        }
        if t < mix.click_until && mix.beat > 0.0 {
            let beat = ((t - mix.start) / mix.beat).floor();
            let since = t - (mix.start + beat * mix.beat);
            if since < 0.03 {
                let accent = (beat as i64).rem_euclid(mix.bar as i64) == 0;
                let hz = if accent { 1760.0 } else { 1320.0 };
                let c = ((std::f64::consts::TAU * hz * since).sin() * (-since * 150.0).exp() * 0.45) as f32;
                l += c;
                r += c;
            }
        }
        frame[0] = l;
        if ch > 1 {
            frame[1] = r;
        }
        for s in frame.iter_mut().skip(2) {
            *s = 0.0;
        }
    }
}

/// the take: the captured samples from `at` seconds after the first was captured (silence first, when that was before
/// it), to the end
fn cut(taken: &[f32], channels: usize, rate: f64, at: f64) -> Vec<f32> {
    let skip = (at * rate).round() as i64 * channels as i64;
    if skip >= 0 {
        taken.get(skip as usize..).map(<[f32]>::to_vec).unwrap_or_default()
    } else {
        let mut out = vec![0.0; (-skip) as usize];
        out.extend_from_slice(taken);
        out
    }
}

/// interleaved samples as a 32-bit float WAV file
fn wav(samples: &[f32], channels: u16, rate: u32) -> Vec<u8> {
    let spec =
        hound::WavSpec { channels, sample_rate: rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut out = Cursor::new(Vec::new());
    if let Ok(mut w) = hound::WavWriter::new(&mut out, spec) {
        for &s in samples {
            let _ = w.write_sample(s);
        }
        let _ = w.finalize();
    }
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(secs: f64) -> Arc<Pcm> {
        let n = (secs * 48_000.0) as usize;
        Arc::new(Pcm { rate: 48_000, channels: 2, samples: (0..n).flat_map(|i| [i as f32 / n as f32, -0.5]).collect() })
    }

    #[test]
    fn the_output_plays_the_song_from_the_count_in_and_clicks_on_its_beats() {
        let mix = Mix { song: song(10.0), from: 2.0, start: 4.0, click_until: 4.0, beat: 0.5, bar: 4 };
        let rate = 48_000.0;
        let mut out = vec![0.0; 3 * 48_000 * 3];
        fill(&mut out, 3, rate, 0, &mix);
        let at = |t: f64, c: usize| out[(t * rate) as usize * 3 + c];
        // between clicks: the song as it is at that point, on the first two channels only
        assert!((at(0.25, 0) - 2.25 / 10.0).abs() < 1e-3 && (at(0.25, 1) + 0.5).abs() < 1e-6);
        assert_eq!(at(0.25, 2), 0.0);
        // a click on each beat of the count-in, from the take's start back (2.0 s in is the count-in's first beat)
        let loud = |t: f64| {
            (0..240).map(|k| (at(t + k as f64 / rate, 0) - (t + 2.0) as f32 / 10.0).abs()).fold(0f32, f32::max)
        };
        for beat in [0.0, 0.5, 1.0, 1.5] {
            assert!(loud(beat) > 0.2, "a click at {beat} s");
        }
        assert!(loud(0.2) < 1e-3, "nothing between them");
        assert!(loud(2.0) < 1e-3 && loud(2.5) < 1e-3, "and none once the take starts");
        let mut before = vec![1.0; 4];
        fill(&mut before, 2, rate, 0, &Mix { from: -1.0, click_until: f64::NEG_INFINITY, ..mix });
        assert_eq!(before, [0.0; 4], "silence before the song");
    }

    #[test]
    fn a_take_is_cut_where_the_section_began_as_heard() {
        let taken: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let rate = 10.0;
        // heard 0.3 s after the input began, a count-in of 2 s and 0.1 s more for the headphones: 2.4 s in
        let at = 0.3 + 2.0 + 0.1;
        assert_eq!(cut(&taken, 1, rate, at)[..3], [24.0, 25.0, 26.0]);
        assert_eq!(cut(&taken, 2, rate, at)[..2], [48.0, 49.0], "frames, not samples");
        assert_eq!(cut(&taken, 1, rate, -0.2)[..3], [0.0, 0.0, 0.0], "silence first when it began too late");
        assert!(cut(&taken, 1, rate, 20.0).is_empty());
    }

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
    fn a_take_is_a_float_wav() {
        let wav = wav(&[0.25, -0.5, 1.5, 0.0], 2, 44_100);
        let pcm = Pcm::decode_wav(&wav).unwrap();
        assert_eq!((pcm.rate, pcm.channels, pcm.samples), (44_100, 2, vec![0.25, -0.5, 1.5, 0.0]));
    }
}
