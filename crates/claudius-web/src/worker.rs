//! The worker's side: a `Host` and the messages to and from the page.

use std::cell::RefCell;
use std::rc::Rc;

use claudius_api::{Command, Reply};
use claudius_session::host::{Host, Recording, SoundAsk};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

use crate::crew::{self, Crew, Said};

struct Engine {
    host: Host,
    scope: DedicatedWorkerGlobalScope,
    /// a render is on its way: the commands that come before it are made first, and it renders them all at once
    scheduled: bool,
    /// the crew making the tracks of the next render, which waits for it
    crew: Option<Crew>,
    /// how many workers a crew has (none after one failed: the engine makes the tracks itself)
    crew_size: usize,
}

type Shared = Rc<RefCell<Engine>>;

pub fn start() {
    std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&format!("claudius engine: {info}").into())));
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    if crew::member(&scope) {
        crew::serve(scope);
        return;
    }
    let engine = Rc::new(RefCell::new(Engine {
        host: Host::new(js_sys::Date::now() as u64),
        scope: scope.clone(),
        scheduled: false,
        crew: None,
        crew_size: crew::size(&scope),
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
    if let Some(r) = v.get("recording") {
        let id = v["id"].as_u64().unwrap_or(0);
        let bytes = js_sys::Reflect::get(&data, &"bytes".into())
            .ok()
            .and_then(|b| b.dyn_into::<js_sys::Uint8Array>().ok())
            .map(|b| b.to_vec())
            .unwrap_or_default();
        let made = serde_json::from_value::<Recording>(r.clone())
            .map_err(|e| format!("no recording the engine knows: {e}"))
            .and_then(|r| engine.borrow_mut().host.sample(r, bytes));
        let sampled = match made {
            Ok(p) => serde_json::json!({ "id": id, "ok": true, "prepared": p }),
            Err(why) => serde_json::json!({ "id": id, "ok": false, "why": why }),
        };
        let e = engine.borrow();
        post(&e.scope, &serde_json::json!({ "sampled": sampled, "state": e.host.state() }), None);
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
    // the render waits for a crew making its tracks; one making none the song still needs stops
    let make = e.host.to_make();
    if let Some(crew) = e.crew.take() {
        if make.as_ref().is_some_and(|m| crew.makes_any_of(m)) {
            e.crew = Some(crew);
            return;
        }
        crew.end();
    }
    if let Some(make) = make.filter(|m| m.tracks.len() >= 2 && e.crew_size >= 2) {
        let heard = {
            let engine = engine.clone();
            move |i: usize, data: JsValue| heard(&engine, i, &data)
        };
        if let Some(crew) = crew::start(&make, e.crew_size, heard) {
            e.crew = Some(crew);
            return;
        }
        e.crew_size = 0;
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

/// What worker `i` of the crew posted (`data`; null when it failed): a track it made is kept for the render, and once
/// the crew has made them all it stops, and the render that waited for it comes. A crew that fails stops too, and the
/// engine makes the tracks itself from then on.
fn heard(engine: &Shared, i: usize, data: &JsValue) {
    let mut guard = engine.borrow_mut();
    let e = &mut *guard;
    let Some(crew) = e.crew.as_mut() else { return };
    match crew.hear(i, data) {
        Some(Said::Made(m)) => e.host.made(&m.name, m.key, m.sound, m.level),
        Some(Said::Failed(why)) => {
            web_sys::console::warn_1(&format!("claudius engine: {why}; it makes the tracks itself").into());
            e.crew_size = 0;
            if let Some(c) = e.crew.take() {
                c.end();
            }
        }
        Some(Said::Ready) | None => return,
    }
    if e.crew.as_ref().is_some_and(|c| c.done())
        && let Some(c) = e.crew.take()
    {
        c.end();
    }
    if e.crew.is_none() {
        drop(guard);
        schedule(engine);
    }
}

pub(crate) fn post(scope: &DedicatedWorkerGlobalScope, json: &serde_json::Value, audio: Option<js_sys::Float32Array>) {
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
