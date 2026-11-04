//! The worker's side: a `Host` and the messages to and from the page.

use std::cell::RefCell;
use std::rc::Rc;

use claudius::library::prepared::Prepared;
use claudius_api::{Command, Reply};
use claudius_session::host::{Host, SoundAsk};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

struct Engine {
    host: Host,
    scope: DedicatedWorkerGlobalScope,
    /// a render is on its way: the commands that come before it are made first, and it renders them all at once
    scheduled: bool,
}

type Shared = Rc<RefCell<Engine>>;

pub fn start() {
    std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&format!("claudius engine: {info}").into())));
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let engine = Rc::new(RefCell::new(Engine {
        host: Host::new(js_sys::Date::now() as u64),
        scope: scope.clone(),
        scheduled: false,
    }));
    let e = engine.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| receive(&e, &m));
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    let state = engine.borrow().host.state();
    post(&scope, &serde_json::json!({ "state": state, "sounds": Host::sounds() }), None);
}

fn receive(engine: &Shared, m: &MessageEvent) {
    let data = m.data();
    let Some(text) = js_sys::Reflect::get(&data, &"json".into()).ok().and_then(|j| j.as_string()) else { return };
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    if let Some(secs) = v["playhead"].as_f64() {
        engine.borrow_mut().host.set_playhead(secs);
        return;
    }
    if v.get("reference").is_some() {
        post(&engine.borrow().scope, &serde_json::json!({ "reference": Host::reference() }), None);
        return;
    }
    if let Some(ask) = v.get("sound") {
        sound(&engine.borrow(), ask);
        return;
    }
    if let Some(loops) = v["loops"].as_array() {
        let audio = js_sys::Reflect::get(&data, &"audio".into()).unwrap_or_default();
        let audio: js_sys::Array = audio.dyn_into().unwrap_or_default();
        let found = loops
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let p: Prepared = serde_json::from_value(l["prepared"].clone()).ok()?;
                let samples = audio.get(i as u32).dyn_into::<js_sys::Float32Array>().ok()?.to_vec();
                Some((p, l["credit"].as_str().unwrap_or_default().to_string(), samples))
            })
            .collect();
        engine.borrow_mut().host.add_loops(found);
        return;
    }
    let what = v["what"].as_str().unwrap_or_default().to_string();
    let reply = match serde_json::from_value::<Command>(v["command"].clone()) {
        Ok(c) => engine.borrow_mut().host.command(c),
        Err(e) => Reply { ok: false, message: format!("{what} is no command the engine knows: {e}"), version: 0 },
    };
    let e = engine.borrow();
    post(&e.scope, &serde_json::json!({ "what": what, "reply": reply, "state": e.host.state() }), None);
    drop(e);
    schedule(engine);
}

/// a sound of the song for a phone playing along: the sounds asked for, one after another in one array, with their
/// names and lengths in frames, or why there are none
fn sound(e: &Engine, ask: &serde_json::Value) {
    let id = ask["id"].as_u64().unwrap_or(0);
    let made =
        serde_json::from_value::<SoundAsk>(ask.clone()).map_err(|e| e.to_string()).and_then(|a| e.host.sound(&a));
    match made {
        Ok(sounds) => {
            let names: Vec<&str> = sounds.iter().map(|(n, _)| n.as_str()).collect();
            let frames: Vec<usize> = sounds.iter().map(|(_, x)| x.len()).collect();
            let all: Vec<f32> = sounds.iter().flat_map(|(_, x)| x.interleaved()).collect();
            let json = serde_json::json!({ "sound": { "id": id, "names": names, "frames": frames } });
            post(&e.scope, &json, Some(js_sys::Float32Array::from(all.as_slice())));
        }
        Err(err) => post(&e.scope, &serde_json::json!({ "sound": { "id": id, "error": err } }), None),
    }
}

/// a render soon, when the song is newer than its render and none is on its way
fn schedule(engine: &Shared) {
    let mut e = engine.borrow_mut();
    if e.scheduled || !e.host.stale() {
        return;
    }
    e.scheduled = true;
    let later = engine.clone();
    let render = Closure::once_into_js(move || render(&later));
    let _ = e.scope.set_timeout_with_callback_and_timeout_and_arguments_0(render.unchecked_ref(), 30);
}

fn render(engine: &Shared) {
    let mut e = engine.borrow_mut();
    e.scheduled = false;
    // the song's first bars first, to play while the rest renders: the commands that come meanwhile are made before it
    if let Some(preview) = e.host.preview() {
        let state = serde_json::json!({ "state": e.host.state() });
        post(&e.scope, &state, Some(js_sys::Float32Array::from(preview.samples.as_slice())));
        drop(e);
        schedule(engine);
        return;
    }
    let Some(done) = e.host.render() else { return };
    let state = serde_json::json!({ "state": e.host.state() });
    match done {
        Ok(audio) => post(&e.scope, &state, Some(js_sys::Float32Array::from(audio.samples.as_slice()))),
        Err(_) => post(&e.scope, &state, None),
    }
    drop(e);
    // what came while it rendered
    schedule(engine);
}

fn post(scope: &DedicatedWorkerGlobalScope, json: &serde_json::Value, audio: Option<js_sys::Float32Array>) {
    let msg = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&msg, &"json".into(), &json.to_string().into());
    let sent = match audio {
        Some(samples) => {
            let _ = js_sys::Reflect::set(&msg, &"audio".into(), &samples);
            scope.post_message_with_transfer(&msg, &js_sys::Array::of1(&samples.buffer()))
        }
        None => scope.post_message(&msg),
    };
    if let Err(e) = sent {
        web_sys::console::error_1(&e);
    }
}
