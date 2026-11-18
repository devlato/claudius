//! The page's player, on Web Audio: the render decoded once into a buffer and played from it on the audio clock, so
//! that a window of the live mix (design 0023) goes into the buffer with a fade at each end, as the desktop player
//! plays one, and is heard from a source started again at the same moment, fading in over the one before.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode};

use super::{Pcm, Source};

/// how long a window fades in and out against what it plays over, in seconds, as on the desktop
const FADE: f64 = 0.005;
/// how far ahead of the audio clock a source is started, in seconds: time for the browser to schedule it
const AHEAD: f64 = 0.02;
/// the rate claudius renders at, the page's audio runs at, and a render decoded is brought to, so that a window of the
/// live mix fits the buffer it goes into sample for sample
const RATE: f32 = 48_000.0;

/// a source playing the buffer, and the gain it fades in and out through
struct Playing {
    source: AudioBufferSourceNode,
    gain: GainNode,
}

/// what a load hands over once its audio is decoded
type Decoded = Rc<RefCell<Option<Result<AudioBuffer, String>>>>;

pub struct Player {
    ctx: Option<AudioContext>,
    /// the volume, which every source plays through
    volume: Option<GainNode>,
    buffer: Option<AudioBuffer>,
    /// a load being decoded, with whether to play once it is, and from where
    decoding: Option<(Decoded, bool, f64)>,
    playing: Option<Playing>,
    /// where the song is when not playing, or where it was when the source playing started, in seconds
    offset: f64,
    /// the audio clock's time when the source playing started
    started: f64,
    /// a stretch to loop, or the whole song (`Some((a, b))` with b <= a)
    region: Option<(f64, f64)>,
    /// the generated audio itself (the demo engine's render): what a take is recorded over
    pcm: Option<std::sync::Arc<Pcm>>,
    error: Option<String>,
    loaded: bool,
}

impl Player {
    pub fn new(_ctx: &egui::Context) -> Self {
        // at claudius's own rate where the browser runs at it: it decodes a render to the context's rate
        let at_rate = web_sys::AudioContextOptions::new();
        at_rate.set_sample_rate(RATE);
        let ctx = AudioContext::new_with_context_options(&at_rate).or_else(|_| AudioContext::new()).ok();
        let volume = ctx.as_ref().and_then(|c| {
            let g = c.create_gain().ok()?;
            g.gain().set_value(0.9);
            g.connect_with_audio_node(&c.destination()).ok()?;
            Some(g)
        });
        let error = ctx.is_none().then(|| "this browser has no Web Audio".to_string());
        Player {
            ctx,
            volume,
            buffer: None,
            decoding: None,
            playing: None,
            offset: 0.0,
            started: 0.0,
            region: None,
            pcm: None,
            error,
            loaded: false,
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        self.decoding.is_some()
    }

    pub fn load(&mut self, source: Source, keep_position: bool) {
        let Some(ctx) = self.ctx.clone() else { return };
        let was_playing = self.is_playing();
        let pos = if keep_position { self.position() } else { 0.0 };
        // a song that carries on plays the audio it had until the new audio is ready, which then takes over where the
        // song has got to; another song stops the one before
        if !(keep_position && was_playing) {
            self.halt();
        }
        self.pcm = None;
        self.loaded = true;
        self.error = None;
        let decoded: Decoded = Rc::new(RefCell::new(None));
        match source {
            // audio handed over as samples (the demo engine's render) needs no decoding
            Source::Pcm(pcm) => {
                *decoded.borrow_mut() = Some(buffer_of(&ctx, &pcm).map_err(|e| format!("{e:?}")));
                self.pcm = Some(pcm);
            }
            Source::File(file) => decode(&ctx, file.bytes.clone(), decoded.clone()),
            Source::Url(url) if url.starts_with("data:") => match data_bytes(&url) {
                Ok(bytes) => decode(&ctx, bytes, decoded.clone()),
                Err(e) => *decoded.borrow_mut() = Some(Err(format!("{e:?}"))),
            },
            Source::Url(url) => {
                let into = decoded.clone();
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match ehttp::fetch_async(ehttp::Request::get(&url)).await {
                        Ok(r) if r.ok => decode(&ctx, r.bytes, into),
                        Ok(r) => *into.borrow_mut() = Some(Err(format!("{} {}", r.status, r.status_text))),
                        Err(e) => *into.borrow_mut() = Some(Err(e)),
                    }
                });
            }
        }
        self.decoding = Some((decoded, was_playing, pos));
        self.tick();
    }

    /// forget the loaded audio (the song it belonged to is gone)
    pub fn clear(&mut self) {
        self.halt();
        self.buffer = None;
        self.decoding = None;
        self.pcm = None;
        self.loaded = false;
        self.offset = 0.0;
    }

    pub fn tick(&mut self) {
        // a load decoded: it plays from where the one before it was, as it was
        let done = self.decoding.as_ref().and_then(|(d, _, _)| d.borrow_mut().take());
        if let Some(done) = done
            && let Some((_, play, pos)) = self.decoding.take()
        {
            match done {
                Ok(buffer) => {
                    // the audio before played on while this one was decoded: it takes over from where the song is now,
                    // fading in over it
                    let carried = self.playing.is_some();
                    let pos = if carried { self.position() } else { pos };
                    let fits = pos < buffer.duration();
                    self.buffer = Some(buffer);
                    match &self.ctx {
                        Some(ctx) if carried && fits => {
                            let when = ctx.current_time() + AHEAD;
                            let at = self.position_at(when);
                            self.start(at, when, FADE);
                        }
                        _ => {
                            self.halt();
                            self.offset = if fits { pos } else { 0.0 };
                            if play {
                                self.play();
                            }
                        }
                    }
                }
                Err(e) => {
                    self.halt();
                    self.error = Some(format!("the browser could not play the audio: {e}"));
                }
            }
        }
        // at the end of a song not looped, it stops there
        if self.playing.is_some() && self.region.is_none() && self.position() >= self.duration() {
            let end = self.duration();
            self.halt();
            self.offset = end;
        }
    }

    /// A window of the live mix, of the song at `version`, from `start` seconds: written into the buffer over what the
    /// render holds there, faded in and out over `FADE` against it, and heard at once by a source started again at the
    /// moment it would be, fading in over the one before (both play the same up to the window, which comes after it).
    pub fn patch(&mut self, _version: u64, start: f64, pcm: std::sync::Arc<Pcm>) {
        let (Some(ctx), Some(buffer)) = (&self.ctx, &self.buffer) else { return };
        let rate = buffer.sample_rate() as f64;
        // a browser that would not run at claudius's rate decoded the render to its own: the window follows it
        let resampled;
        let pcm = if (pcm.rate as f64 - rate).abs() < 0.5 {
            &*pcm
        } else {
            resampled = resample(&pcm, rate);
            &resampled
        };
        let first = (start * rate).round().max(0.0) as usize;
        let total = buffer.length() as usize;
        let frames = pcm.frames().min(total.saturating_sub(first));
        if frames == 0 {
            return;
        }
        let fade = ((FADE * rate) as usize).clamp(1, frames / 2 + 1);
        let channels = pcm.channels.max(1) as usize;
        for ch in 0..buffer.number_of_channels() as usize {
            let mut held = vec![0.0f32; frames];
            if buffer.copy_from_channel_with_start_in_channel(&mut held, ch as i32, first as u32).is_err() {
                return;
            }
            for (i, h) in held.iter_mut().enumerate() {
                let w = pcm.samples[i * channels + ch.min(channels - 1)];
                let k = ((i + 1).min(frames - i) as f32 / fade as f32).min(1.0);
                *h = *h * (1.0 - k) + w * k;
            }
            if buffer.copy_to_channel_with_start_in_channel(&held, ch as i32, first as u32).is_err() {
                return;
            }
        }
        // the source playing may have read the buffer when it started: one started again plays what it holds now
        if self.playing.is_some() {
            let when = ctx.current_time() + AHEAD;
            let at = self.position_at(when);
            self.start(at, when, FADE);
        }
    }

    pub fn has_audio(&self) -> bool {
        self.loaded
    }

    /// the audio loaded, when it was handed over as audio rather than a URL: what a take is recorded over
    pub fn pcm(&self) -> Option<std::sync::Arc<Pcm>> {
        self.pcm.clone()
    }

    pub fn play(&mut self) {
        let Some(ctx) = &self.ctx else { return };
        if self.buffer.is_none() || self.playing.is_some() {
            return;
        }
        // a context made before a click waits for one: a rejected resume (autoplay policy) must not surface
        if let Ok(promise) = ctx.resume() {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            });
        }
        let at = if self.offset >= self.duration() { 0.0 } else { self.offset };
        let when = ctx.current_time() + AHEAD;
        // faded in, as a song started in its middle would otherwise click
        self.start(at, when, FADE);
    }

    pub fn pause(&mut self) {
        if self.playing.is_some() {
            self.offset = self.position();
            self.halt();
        }
    }

    pub fn stop(&mut self) {
        self.pause();
        self.seek(0.0);
    }

    pub fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    pub fn seek(&mut self, secs: f64) {
        let secs = secs.max(0.0);
        match (&self.ctx, self.playing.is_some()) {
            (Some(ctx), true) => {
                let when = ctx.current_time() + AHEAD;
                self.start(secs, when, FADE);
            }
            _ => self.offset = secs,
        }
    }

    pub fn position(&self) -> f64 {
        match &self.ctx {
            Some(ctx) if self.playing.is_some() => self.position_at(ctx.current_time()),
            _ => self.offset,
        }
    }

    pub fn duration(&self) -> f64 {
        self.buffer.as_ref().map(|b| b.duration()).unwrap_or(0.0)
    }

    pub fn set_loop(&mut self, region: Option<(f64, f64)>) {
        self.region = region;
        // the source playing loops as asked from here on
        if let Some(p) = &self.playing {
            loop_as(&p.source, region);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if let Some(v) = &self.volume {
            v.gain().set_value(volume.clamp(0.0, 1.0));
        }
    }

    /// where the song is at the audio clock's time `t`, playing: from where the source started, through the loop
    fn position_at(&self, t: f64) -> f64 {
        let raw = self.offset + (t - self.started).max(0.0);
        match self.region {
            Some((a, b)) if b > a && raw >= b => a + (raw - a) % (b - a),
            Some(_) if self.duration() > 0.0 => raw % self.duration(),
            _ => raw,
        }
    }

    /// a source playing the buffer from `at` seconds at the audio clock's `when`, through a gain of its own: faded in
    /// over `fade` while the one before it fades out and stops, or at once when there is none or no fade
    fn start(&mut self, at: f64, when: f64, fade: f64) {
        let (Some(ctx), Some(buffer), Some(volume)) = (&self.ctx, &self.buffer, &self.volume) else { return };
        let made = (|| -> Result<Playing, JsValue> {
            let source = ctx.create_buffer_source()?;
            source.set_buffer(Some(buffer));
            loop_as(&source, self.region);
            let gain = ctx.create_gain()?;
            source.connect_with_audio_node(&gain)?;
            gain.connect_with_audio_node(volume)?;
            if fade > 0.0 {
                gain.gain().set_value_at_time(0.0, when)?;
                gain.gain().linear_ramp_to_value_at_time(1.0, when + fade)?;
            }
            source.start_with_when_and_grain_offset(when, at.max(0.0))?;
            Ok(Playing { source, gain })
        })();
        let new = match made {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(format!("the browser could not play the audio: {e:?}"));
                return;
            }
        };
        if let Some(old) = self.playing.take() {
            if fade > 0.0 {
                let g = old.gain.gain();
                let _ = g.set_value_at_time(g.value(), when);
                let _ = g.linear_ramp_to_value_at_time(0.0, when + fade);
            }
            let _ = scheduled(&old.source).stop_with_when(when + fade);
        }
        (self.offset, self.started) = (at, when);
        self.playing = Some(new);
    }

    /// stops the source playing, faded out over `FADE` so that it does not click
    fn halt(&mut self) {
        let Some(p) = self.playing.take() else { return };
        match &self.ctx {
            Some(ctx) => {
                let now = ctx.current_time();
                let g = p.gain.gain();
                let _ = g.cancel_scheduled_values(now);
                let _ = g.set_value_at_time(g.value(), now);
                let _ = g.linear_ramp_to_value_at_time(0.0, now + FADE);
                let _ = scheduled(&p.source).stop_with_when(now + FADE);
            }
            None => {
                let _ = scheduled(&p.source).stop();
            }
        }
    }
}

/// a buffer source as the scheduled source it is, whose `stop` is the one not deprecated
fn scheduled(source: &AudioBufferSourceNode) -> &web_sys::AudioScheduledSourceNode {
    source
}

/// a source looping the stretch `region` (a, b), or the whole buffer (b <= a), or not
fn loop_as(source: &AudioBufferSourceNode, region: Option<(f64, f64)>) {
    match region {
        Some((a, b)) if b > a => {
            source.set_loop(true);
            source.set_loop_start(a);
            source.set_loop_end(b);
        }
        Some(_) => {
            source.set_loop(true);
            source.set_loop_start(0.0);
            source.set_loop_end(0.0);
        }
        None => source.set_loop(false),
    }
}

/// `pcm` at `rate`, each sample read between the two either side of where it falls
fn resample(pcm: &Pcm, rate: f64) -> Pcm {
    let channels = pcm.channels.max(1) as usize;
    let step = pcm.rate as f64 / rate;
    let frames = ((pcm.frames() as f64) / step).floor() as usize;
    let mut samples = Vec::with_capacity(frames * channels);
    for i in 0..frames {
        let at = i as f64 * step;
        let (k, f) = (at as usize, at.fract() as f32);
        for ch in 0..channels {
            let a = pcm.samples[k * channels + ch];
            let b = pcm.samples.get((k + 1) * channels + ch).copied().unwrap_or(a);
            samples.push(a + (b - a) * f);
        }
    }
    Pcm { rate: rate.round() as u32, channels: pcm.channels, samples }
}

/// a buffer holding `pcm` as it is
fn buffer_of(ctx: &AudioContext, pcm: &Pcm) -> Result<AudioBuffer, JsValue> {
    let channels = pcm.channels.max(1) as usize;
    let frames = pcm.frames().max(1);
    let buffer = ctx.create_buffer(channels as u32, frames as u32, pcm.rate as f32)?;
    for ch in 0..channels {
        let one: Vec<f32> = (0..pcm.frames()).map(|i| pcm.samples[i * channels + ch]).collect();
        buffer.copy_to_channel(&one, ch as i32)?;
    }
    Ok(buffer)
}

/// `bytes` of an audio file decoded by the browser into a buffer, put in `into` when done
fn decode(ctx: &AudioContext, bytes: Vec<u8>, into: Decoded) {
    let array = js_sys::Uint8Array::from(bytes.as_slice()).buffer();
    let promise = match ctx.decode_audio_data(&array) {
        Ok(p) => p,
        Err(e) => {
            *into.borrow_mut() = Some(Err(format!("{e:?}")));
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let got = wasm_bindgen_futures::JsFuture::from(promise).await;
        *into.borrow_mut() = Some(match got {
            Ok(b) => Ok(b.unchecked_into::<AudioBuffer>()),
            Err(e) => Err(format!("{e:?}")),
        });
    });
}

/// a `data:<kind>;base64,<file>` URL's file
fn data_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    let (head, file) = url["data:".len()..].split_once(',').ok_or_else(|| JsValue::from_str("not a data URL"))?;
    head.strip_suffix(";base64").ok_or_else(|| JsValue::from_str("not a base64 data URL"))?;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    Ok(window.atob(file)?.chars().map(|c| c as u8).collect())
}
