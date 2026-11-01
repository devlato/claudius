//! Recording a take in the browser, with the app's recorder's API (`record.rs`). The song plays through Web Audio from
//! a buffer filled here (`take::render_mix`: the song from the count-in, with its click), started at a set time on the
//! audio context's clock. The input (getUserMedia, with the browser's echo cancelling, noise suppression and gain
//! control off) goes to an AudioWorklet (`record-worklet.js`) that hands the page what it hears in chunks, each
//! stamped with the context frame it reached the worklet in. One clock serves both, so where the take starts is
//! arithmetic (`take::take_starts`): when its downbeat was to play, plus the output's latency (`baseLatency`,
//! `outputLatency`) and the input's (the track's `latency`), as far as the browser reports them, plus the Latency
//! setting for the rest, which Measure finds.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use js_sys::{Array, Float32Array, Object, Reflect};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AudioBufferSourceNode, AudioContext, AudioContextLatencyCategory, AudioContextOptions, AudioWorkletNode,
    AudioWorkletNodeOptions, ChannelCountMode, MediaDeviceInfo, MediaDeviceKind, MediaDevices, MediaStream,
    MediaStreamConstraints, MediaStreamTrack, MessageEvent,
};

use super::Pcm;
use super::take::{Captured, cut, heard_at, interleave, no_input, render_mix, take_starts, wav};
pub use super::take::{Plan, Take};

/// the worklet's script, beside the page (trunk copies it there)
const WORKLET: &str = "record-worklet.js";

/// a MIDI keyboard records in the app: Web MIDI is not in every browser, and the studio does not use it yet
const NO_MIDI: &str = "a MIDI keyboard records in the studio app";

/// an input the person can record from
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub name: String,
    pub channels: u16,
}

thread_local! {
    /// the audio inputs as the browser last listed them, each with its device id
    static INPUTS: RefCell<Vec<(Input, String)>> = const { RefCell::new(Vec::new()) };
    /// a listing is on its way, and when the last was asked for (ms since 1970): once a second is enough
    static LISTING: Cell<bool> = const { Cell::new(false) };
    static LISTED: Cell<f64> = const { Cell::new(f64::NEG_INFINITY) };
    /// the render a take is recorded over, by URL: decoded, or why it could not be (nothing yet while it loads)
    static SONG: RefCell<Option<Song>> = const { RefCell::new(None) };
}

type Song = (String, Option<Result<Arc<Pcm>, String>>);

/// The audio inputs, as the browser last listed them, with a new listing on its way; until the first arrives, the
/// default microphone. A browser keeps the inputs' names to itself until the page may use the microphone, so the names
/// fill in once the meter has started.
pub fn inputs() -> Vec<Input> {
    let now = js_sys::Date::now();
    if now - LISTED.with(Cell::get) > 1000.0 && !LISTING.with(|l| l.replace(true)) {
        LISTED.with(|l| l.set(now));
        spawn_local(async {
            if let Ok(list) = list_inputs().await {
                INPUTS.with(|i| *i.borrow_mut() = list);
            }
            LISTING.with(|l| l.set(false));
        });
    }
    let listed: Vec<Input> = INPUTS.with(|i| i.borrow().iter().map(|(i, _)| i.clone()).collect());
    if listed.is_empty() { vec![Input { name: "the default microphone".into(), channels: 2 }] } else { listed }
}

async fn list_inputs() -> Result<Vec<(Input, String)>, String> {
    let all = JsFuture::from(media_devices()?.enumerate_devices().map_err(|e| described(&e))?)
        .await
        .map_err(|e| described(&e))?;
    let mut out = Vec::new();
    for d in Array::from(&all).iter().filter_map(|d| d.dyn_into::<MediaDeviceInfo>().ok()) {
        if d.kind() == MediaDeviceKind::Audioinput {
            let label = d.label();
            let name = if label.is_empty() { format!("microphone {}", out.len() + 1) } else { label };
            out.push((Input { name, channels: 2 }, d.device_id()));
        }
    }
    Ok(out)
}

/// the MIDI inputs: none in the browser
pub fn midi_inputs() -> Vec<String> {
    Vec::new()
}

/// The render at `url` (a WAV) decoded, for a take to be recorded over: `None` while it loads. Asking for another URL
/// lets the last one go.
pub fn song(url: &str) -> Result<Option<Arc<Pcm>>, String> {
    let have = SONG.with(|s| s.borrow().as_ref().filter(|(u, _)| u == url).map(|(_, got)| got.clone()));
    match have {
        Some(Some(got)) => got.map(Some),
        Some(None) => Ok(None),
        None => {
            SONG.with(|s| *s.borrow_mut() = Some((url.to_string(), None)));
            let want = url.to_string();
            ehttp::fetch(ehttp::Request::get(url), move |res| {
                let got = match res {
                    Ok(r) if r.ok => Pcm::decode_wav(&r.bytes).map(Arc::new),
                    Ok(r) => Err(format!("the song did not load for the take: {} {}", r.status, r.status_text)),
                    Err(e) => Err(format!("the song did not load for the take: {e}")),
                };
                SONG.with(|s| {
                    let mut s = s.borrow_mut();
                    if s.as_ref().is_some_and(|(u, _)| *u == want) {
                        *s = Some((want, Some(got)));
                    }
                });
            });
            Ok(None)
        }
    }
}

/// a context, the input it listens to and what the worklet hands over from it; and the song, for a take
struct Session {
    ctx: AudioContext,
    /// the channels a take records: the first, and how many
    first: usize,
    count: usize,
    /// what the input captured (a meter keeps none of it)
    captured: Option<Captured>,
    level: f32,
    error: Option<String>,
    /// the input's name and its latency, as its track reports them
    input: String,
    input_latency: f64,
    /// when the song's first frame was to play, on the context's clock
    scheduled: Option<f64>,
    stream: Option<MediaStream>,
    node: Option<AudioWorkletNode>,
    player: Option<AudioBufferSourceNode>,
    on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    /// the worklet was told to stop, and has handed over all it held since
    stopping: bool,
    drained: bool,
    closed: bool,
}

impl Session {
    fn new(keep: bool, first: usize, count: usize) -> Result<Rc<RefCell<Session>>, String> {
        let opts = AudioContextOptions::new();
        opts.set_latency_hint_audio_context_latency_category(AudioContextLatencyCategory::Interactive);
        let ctx = AudioContext::new_with_context_options(&opts)
            .map_err(|e| format!("the browser gives the page no audio: {}", described(&e)))?;
        Ok(Rc::new(RefCell::new(Session {
            ctx,
            first,
            count,
            captured: keep.then(|| Captured::new(count)),
            level: 0.0,
            error: None,
            input: "the microphone".into(),
            input_latency: 0.0,
            scheduled: None,
            stream: None,
            node: None,
            player: None,
            on_message: None,
            stopping: false,
            drained: false,
            closed: false,
        })))
    }

    /// the output's latency, as far as the browser reports it
    fn out_latency(&self) -> f64 {
        let get = |key: &str| {
            Reflect::get(&self.ctx, &key.into()).ok().and_then(|v| v.as_f64()).filter(|v| v.is_finite()).unwrap_or(0.0)
        };
        get("baseLatency") + get("outputLatency")
    }

    /// a chunk from the worklet, `{ frame, channels: [Float32Array, ...] }`, or its `{ done: true }` once stopped
    fn take_in(&mut self, data: &JsValue) {
        if Reflect::get(data, &"done".into()).ok().and_then(|v| v.as_bool()) == Some(true) {
            self.drained = true;
            return;
        }
        let frame = Reflect::get(data, &"frame".into()).ok().and_then(|v| v.as_f64());
        let planes = Reflect::get(data, &"channels".into()).ok().filter(Array::is_array).map(|v| Array::from(&v));
        let (Some(frame), Some(planes)) = (frame, planes) else { return };
        let planes: Vec<Vec<f32>> =
            planes.iter().filter_map(|p| p.dyn_into::<Float32Array>().ok()).map(|a| a.to_vec()).collect();
        match interleave(&planes, self.first, self.count) {
            Ok(samples) => {
                self.level = samples.iter().fold(self.level, |m, s| m.max(s.abs()));
                if let Some(c) = &mut self.captured {
                    c.add(frame.max(0.0) as u64, &samples);
                }
            }
            Err(n) => {
                let what = format!("{} has {n} input channel{}", self.input, if n == 1 { "" } else { "s" });
                self.error.get_or_insert(what);
            }
        }
    }

    /// asks the worklet for what it still holds; without one, there is nothing to wait for
    fn stop_soon(&mut self) {
        if std::mem::replace(&mut self.stopping, true) {
            return;
        }
        match self.node.as_ref().and_then(|n| n.port().ok()) {
            Some(port) if port.post_message(&"stop".into()).is_ok() => {}
            _ => self.drained = true,
        }
    }

    fn close(&mut self) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        if let Some(node) = self.node.take() {
            if let Ok(port) = node.port() {
                port.set_onmessage(None);
            }
            let _ = node.disconnect();
        }
        if let Some(p) = self.player.take() {
            let _ = web_sys::AudioScheduledSourceNode::stop(&p);
            let _ = p.disconnect();
        }
        if let Some(stream) = self.stream.take() {
            stop_tracks(&stream);
        }
        self.on_message = None;
        let _ = self.ctx.close();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// opens `input` (by name; the default one when there is none of that name) for `s`, and plays `song` (left, right)
/// once it listens; what goes wrong ends up in the session's error
fn open(s: &Rc<RefCell<Session>>, input: Option<String>, song: Option<[Vec<f32>; 2]>) {
    let weak = Rc::downgrade(s);
    spawn_local(async move {
        if let Err(e) = listen(&weak, input.as_deref(), song).await
            && let Some(s) = weak.upgrade()
        {
            s.borrow_mut().error.get_or_insert(e);
        }
    });
}

/// the session, while it is open
fn alive(s: &Weak<RefCell<Session>>) -> Option<Rc<RefCell<Session>>> {
    s.upgrade().filter(|s| !s.borrow().closed)
}

async fn listen(s: &Weak<RefCell<Session>>, input: Option<&str>, song: Option<[Vec<f32>; 2]>) -> Result<(), String> {
    let Some(ctx) = alive(s).map(|s| s.borrow().ctx.clone()) else { return Ok(()) };
    let worklet = ctx.audio_worklet().map_err(|e| format!("this browser cannot record: {}", described(&e)))?;
    let loading = worklet.add_module(WORKLET).map_err(|e| format!("the recorder did not load: {}", described(&e)))?;
    JsFuture::from(loading).await.map_err(|e| format!("the recorder did not load: {}", described(&e)))?;
    let Some((first, count)) = alive(s).map(|s| (s.borrow().first, s.borrow().count)) else { return Ok(()) };
    let id = input
        .and_then(|name| INPUTS.with(|i| i.borrow().iter().find(|(i, _)| i.name == name).map(|(_, id)| id.clone())));
    let asking = media_devices()?
        .get_user_media_with_constraints(&constraints(id.as_deref(), first + count))
        .map_err(|e| refused(&e, input))?;
    let stream: MediaStream =
        JsFuture::from(asking).await.map_err(|e| refused(&e, input))?.dyn_into().map_err(|e| described(&e))?;
    let Some(session) = alive(s) else {
        stop_tracks(&stream);
        return Ok(());
    };
    // the names of the inputs are the browser's to give now
    inputs();
    let track: MediaStreamTrack =
        stream.get_audio_tracks().get(0).dyn_into().map_err(|_| "the browser gave no audio track".to_string())?;
    let latency = Reflect::get(&track.get_settings(), &"latency".into()).ok().and_then(|v| v.as_f64());
    let graph = |e: JsValue| format!("the browser could not record: {}", described(&e));
    let from = ctx.create_media_stream_source(&stream).map_err(graph)?;
    let opts = AudioWorkletNodeOptions::new();
    opts.set_number_of_inputs(1);
    opts.set_number_of_outputs(1);
    opts.set_output_channel_count(&Array::of1(&1.into()));
    // as many channels as the input has: the take picks its own
    opts.set_channel_count_mode(ChannelCountMode::Max);
    let node = AudioWorkletNode::new_with_options(&ctx, "claudius-recorder", &opts).map_err(graph)?;
    let weak = Rc::downgrade(&session);
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        if let Some(s) = weak.upgrade() {
            s.borrow_mut().take_in(&e.data());
        }
    });
    node.port().map_err(graph)?.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    from.connect_with_audio_node(&node).map_err(graph)?;
    // the worklet plays nothing, but a node that goes nowhere may not be run
    node.connect_with_audio_node(&ctx.destination()).map_err(graph)?;
    {
        let mut st = session.borrow_mut();
        st.input = track.label();
        st.input_latency = latency.filter(|l| l.is_finite() && *l >= 0.0).unwrap_or(0.0);
        (st.stream, st.node, st.on_message) = (Some(stream), Some(node), Some(on_message));
    }
    drop(session);
    JsFuture::from(ctx.resume().map_err(graph)?).await.map_err(graph)?;
    let Some(song) = song else { return Ok(()) };
    let Some(session) = alive(s) else { return Ok(()) };
    if ctx.state() != web_sys::AudioContextState::Running {
        return Err("the browser did not let the page play: click in the page and record again".into());
    }
    let [left, right] = song;
    let buffer = ctx.create_buffer(2, left.len().max(1) as u32, ctx.sample_rate()).map_err(graph)?;
    buffer.copy_to_channel(&left, 0).map_err(graph)?;
    buffer.copy_to_channel(&right, 1).map_err(graph)?;
    let player = ctx.create_buffer_source().map_err(graph)?;
    player.set_buffer(Some(&buffer));
    player.connect_with_audio_node(&ctx.destination()).map_err(graph)?;
    // a moment from now: the input is already listening, so it hears from before the count-in
    let at = ctx.current_time() + 0.25;
    player.start_with_when(at).map_err(graph)?;
    let mut st = session.borrow_mut();
    (st.scheduled, st.player) = (Some(at), Some(player));
    Ok(())
}

/// what to ask the browser for: the input, if one was chosen, with enough channels, and with its voice processing off
fn constraints(device: Option<&str>, channels: usize) -> MediaStreamConstraints {
    let set = |o: &Object, key: &str, value: &JsValue| {
        let _ = Reflect::set(o, &key.into(), value);
    };
    let wrapped = |key: &str, value: JsValue| {
        let o = Object::new();
        set(&o, key, &value);
        o
    };
    let audio = Object::new();
    for key in ["echoCancellation", "noiseSuppression", "autoGainControl"] {
        set(&audio, key, &false.into());
    }
    set(&audio, "channelCount", &wrapped("ideal", (channels as f64).into()));
    if let Some(id) = device {
        set(&audio, "deviceId", &wrapped("exact", id.into()));
    }
    let c = Object::new();
    set(&c, "audio", &audio);
    set(&c, "video", &false.into());
    c.unchecked_into()
}

fn media_devices() -> Result<MediaDevices, String> {
    let navigator = web_sys::window().map(|w| w.navigator()).ok_or("no browser window")?;
    let devices = Reflect::get(&navigator, &"mediaDevices".into()).unwrap_or(JsValue::UNDEFINED);
    if devices.is_undefined() {
        return Err("recording in the browser needs a secure page: open claudius on localhost, or over https".into());
    }
    Ok(devices.unchecked_into())
}

fn stop_tracks(stream: &MediaStream) {
    for t in stream.get_tracks().iter().filter_map(|t| t.dyn_into::<MediaStreamTrack>().ok()) {
        t.stop();
    }
}

/// why the browser gave no input, said
fn refused(e: &JsValue, input: Option<&str>) -> String {
    match e.dyn_ref::<web_sys::DomException>() {
        Some(d) => no_input(&d.name(), &d.message(), input),
        None => no_input("", &described(e), input),
    }
}

/// a JavaScript error, as words
fn described(e: &JsValue) -> String {
    e.dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| e.as_string())
        .unwrap_or_else(|| format!("{e:?}"))
}

/// the input alone, for the meter while the Record window is open and no take records
pub struct Listener {
    s: Rc<RefCell<Session>>,
}

impl Listener {
    pub fn start(input: Option<&str>, first: usize, stereo: bool) -> Result<Listener, String> {
        let s = Session::new(false, first, if stereo { 2 } else { 1 })?;
        open(&s, input.map(str::to_string), None);
        Ok(Listener { s })
    }

    pub fn midi(_name: &str) -> Result<Listener, String> {
        Err(NO_MIDI.into())
    }

    /// the loudest input since the last call, 0..1
    pub fn level(&self) -> f32 {
        std::mem::take(&mut self.s.borrow_mut().level)
    }

    pub fn error(&self) -> Option<String> {
        self.s.borrow().error.clone()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.s.borrow_mut().close();
    }
}

pub struct Recorder {
    s: Rc<RefCell<Session>>,
    /// where in the song the output starts (the count-in's start), the count-in, and the Latency setting, in seconds
    from: f64,
    count_in: f64,
    offset: f64,
}

impl Recorder {
    pub fn start(plan: Plan) -> Result<Recorder, String> {
        if plan.midi.is_some() {
            return Err(NO_MIDI.into());
        }
        let s = Session::new(true, plan.channel, if plan.stereo { 2 } else { 1 })?;
        let rate = s.borrow().ctx.sample_rate() as f64;
        open(&s, plan.input.clone(), Some(render_mix(&plan, rate)));
        Ok(Recorder { s, from: plan.start - plan.count_in, count_in: plan.count_in, offset: plan.offset })
    }

    /// what it plays through and records from
    pub fn route(&self) -> String {
        format!("this browser's output and {}", self.s.borrow().input)
    }

    /// what the input captured arrives by itself, from the worklet
    pub fn tick(&mut self) {}

    /// asks the worklet for the last of what it heard: `finish` and `lateness` wait until it is `drained`
    pub fn stop_soon(&self) {
        self.s.borrow_mut().stop_soon();
    }

    /// the worklet has handed over all it heard (or there was none to ask)
    pub fn drained(&self) -> bool {
        self.s.borrow().drained
    }

    /// the song is playing: the browser let the page use the microphone, and the count-in has begun
    pub fn playing(&self) -> bool {
        self.s.borrow().scheduled.is_some()
    }

    /// where in the song the person hears it now, in seconds
    pub fn time(&self) -> f64 {
        let s = self.s.borrow();
        heard_at(s.ctx.current_time(), s.scheduled, s.out_latency(), self.from)
    }

    /// the loudest input since the last call, 0..1
    pub fn level(&self) -> f32 {
        std::mem::take(&mut self.s.borrow_mut().level)
    }

    /// what went wrong, if anything did
    pub fn error(&self) -> Option<String> {
        self.s.borrow().error.clone()
    }

    /// stops, and cuts the take from where the song reached the take's start, as heard, to now
    pub fn finish(self) -> Result<Take, String> {
        let Cut { samples, channels, rate, dropouts, hushed } = self.stop(self.count_in, self.offset)?;
        if samples.is_empty() {
            return Err("the take is empty: it stopped before the count-in was over".into());
        }
        let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
        let secs = samples.len() as f64 / channels as f64 / rate;
        Ok(Take::Audio { wav: wav(&samples, channels as u16, rate as u32), secs, peak, dropouts, hushed })
    }

    /// stops a measurement (`Plan::measure`), and says how late its clicks came back, in seconds: what a take cut the
    /// same way would land late by
    pub fn lateness(self) -> Result<f64, String> {
        let Cut { samples, channels, rate, hushed, .. } = self.stop(0.0, 0.0)?;
        let first: Vec<f32> = samples.iter().step_by(channels.max(1)).copied().collect();
        super::latency::lateness_where(&first, rate, hushed)
    }

    /// stops, and gives what the input captured from `lead` into the song as heard (and `offset` more) on
    fn stop(&self, lead: f64, offset: f64) -> Result<Cut, String> {
        let mut s = self.s.borrow_mut();
        let out = s.out_latency();
        s.close();
        if let Some(e) = s.error.clone() {
            return Err(e);
        }
        let scheduled =
            s.scheduled.ok_or("the song never played: the browser may still be asking to use the microphone")?;
        let captured = s.captured.as_ref().ok_or("nothing was recorded")?;
        let first = captured.first().ok_or("nothing was recorded")?;
        let rate = s.ctx.sample_rate() as f64;
        let at = take_starts(scheduled, lead, out, s.input_latency, offset, first as f64 / rate);
        let samples = cut(captured.samples(), s.count, rate, at);
        let from = (at * rate).max(0.0).round() as u64;
        let (dropouts, hushed) = (captured.gaps_from(from), captured.hushed_from(from, rate));
        Ok(Cut { samples, channels: s.count, rate, dropouts, hushed })
    }
}

/// what the input captured, cut where it starts: its samples (interleaved), its channels and rate, how often frames
/// went missing in it, and how often it fell silent apart from those (`take::hushed`)
struct Cut {
    samples: Vec<f32>,
    channels: usize,
    rate: f64,
    dropouts: usize,
    hushed: usize,
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.s.borrow_mut().close();
    }
}
