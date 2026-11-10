//! The worker's side: a `Host` and the messages to and from the page.

use std::cell::RefCell;
use std::rc::Rc;

use claudius::audio::Stereo;
use claudius::export;
use claudius_api::{Command, Reply};
use claudius_session::host::{Host, Recording, SoundAsk};
use claudius_session::listen::Listen;
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
    /// the files of the song's instruments are being fetched, and the render waits for them
    fetching: bool,
    /// the files that could not be fetched, which are not asked for again
    missing: std::collections::BTreeSet<String>,
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
        fetching: false,
        missing: Default::default(),
    }));
    let e = engine.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| receive(&e, &m));
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    // the page holds what it sends until the engine says something, so the first song draws on the packs
    wasm_bindgen_futures::spawn_local(async move {
        packs(&scope).await;
        let state = engine.borrow().host.state();
        post(&scope, &serde_json::json!({ "state": state, "sounds": Host::sounds() }), None);
    });
}

/// The bytes at `url` (relative to the engine's own script), or why there are none.
async fn fetched(scope: &DedicatedWorkerGlobalScope, url: &str) -> Result<Vec<u8>, String> {
    body(scope.fetch_with_str(url), url).await
}

/// the bytes of the response `fetch` promises for `url`, or why there are none
async fn body(fetch: js_sys::Promise, url: &str) -> Result<Vec<u8>, String> {
    let why = |e: JsValue| format!("{url}: {}", e.as_string().unwrap_or_else(|| format!("{e:?}")));
    let response: web_sys::Response =
        wasm_bindgen_futures::JsFuture::from(fetch).await.map_err(why)?.dyn_into().map_err(why)?;
    if !response.ok() {
        return Err(format!("{url}: {} {}", response.status(), response.status_text()));
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().map_err(why)?).await.map_err(why)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// The web studio's sample packs beside the engine (`packs/index.json` names them, each with an index of its own),
/// handed to it as installed: songs then draw their instruments, whose files are fetched as a song needs them
/// (`fetch_files`). Without them (a page served without its packs), every sound is synthesized.
async fn packs(scope: &DedicatedWorkerGlobalScope) {
    let Ok(list) = fetched(scope, "packs/index.json").await else { return };
    let v: serde_json::Value = serde_json::from_slice(&list).unwrap_or_default();
    for name in v["packs"].as_array().into_iter().flatten().filter_map(|p| p.as_str()) {
        let index = fetched(scope, &format!("packs/{name}/index.json"))
            .await
            .and_then(|b| serde_json::from_slice::<claudius_packs::PackIndex>(&b).map_err(|e| e.to_string()));
        match index {
            Ok(ix) => claudius_packs::provide(ix),
            Err(why) => web_sys::console::warn_1(&format!("claudius engine: no pack {name}: {why}").into()),
        }
    }
}

/// Whether the song's instruments play files it has not fetched yet: if so they are fetched, all at once, and handed to
/// the engine, and the render comes once they are in (one that cannot be fetched is not asked for again).
fn fetch_files(engine: &Shared) -> bool {
    let mut e = engine.borrow_mut();
    if e.fetching {
        return true;
    }
    let wanted: Vec<(String, String, String)> = claudius_packs::wanted(&e.host.instruments())
        .into_iter()
        .filter(|(_, path, _)| !e.missing.contains(path))
        .collect();
    if wanted.is_empty() {
        return false;
    }
    e.fetching = true;
    let (later, scope) = (engine.clone(), e.scope.clone());
    wasm_bindgen_futures::spawn_local(async move {
        // the browser fetches them side by side, each from when it is asked for; each is handed over as it comes
        let pending: Vec<_> = wanted.into_iter().map(|w| (scope.fetch_with_str(&w.2), w)).collect();
        let mut missing = Vec::new();
        for (fetch, (pack, path, url)) in pending {
            let got = body(fetch, &url)
                .await
                .and_then(|b| claudius_packs::provide_file(&pack, &path, b).map_err(|e| format!("{e:#}")));
            if let Err(why) = got {
                web_sys::console::warn_1(&format!("claudius engine: {why}").into());
                missing.push(path);
            }
        }
        let mut e = later.borrow_mut();
        e.fetching = false;
        e.missing.extend(missing);
        drop(e);
        schedule(&later);
    });
    true
}

/// whether track `name` of `project` plays a sampled instrument, whose files only the engine holds
fn sampled(project: &claudius::model::Project, name: &str) -> bool {
    project
        .tracks
        .iter()
        .find(|t| t.name == name)
        .is_some_and(|t| t.sound.each().iter().any(|s| matches!(s, claudius::model::Sound::Instrument { .. })))
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
    if let Some(ask) = v.get("encode") {
        let samples = js_sys::Reflect::get(&data, &"audio".into())
            .ok()
            .and_then(|a| a.dyn_into::<js_sys::Float32Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default();
        encode(&engine.borrow().scope, ask, &samples);
        return;
    }
    if let Some(ask) = v.get("project") {
        let e = engine.borrow();
        let id = ask["id"].as_u64().unwrap_or(0);
        match e.host.project() {
            Ok(zip) => {
                let json =
                    serde_json::json!({ "encoded": { "id": id, "extension": "zip", "mime": "application/zip" } });
                post_bytes(&e.scope, &json, js_sys::Uint8Array::from(zip.as_slice()));
            }
            Err(why) => post(&e.scope, &serde_json::json!({ "encoded": { "id": id, "error": why } }), None),
        }
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
    if let Some(l) = v.get("listen") {
        let bytes = js_sys::Reflect::get(&data, &"bytes".into())
            .ok()
            .and_then(|b| b.dyn_into::<js_sys::Uint8Array>().ok())
            .map(|b| b.to_vec())
            .unwrap_or_default();
        let (what, reply) = match serde_json::from_value::<Listen>(l.clone()) {
            Ok(l) => (l.what().to_string(), engine.borrow_mut().host.listen(&l, bytes)),
            Err(e) => {
                let why = format!("nothing the engine listens for: {e}");
                ("listen".to_string(), Reply { ok: false, message: why, version: 0 })
            }
        };
        let e = engine.borrow();
        post(&e.scope, &serde_json::json!({ "what": what, "reply": reply, "state": e.host.state() }), None);
        drop(e);
        schedule(engine);
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

/// a render as a file in the format the page asked for (`spec`, `flac:16`), here rather than in the page, which
/// would stop for the seconds a song takes to encode: its bytes, or why there are none
fn encode(scope: &DedicatedWorkerGlobalScope, ask: &serde_json::Value, samples: &[f32]) {
    let id = ask["id"].as_u64().unwrap_or(0);
    let made =
        ask["spec"].as_str().unwrap_or_default().parse::<export::Spec>().map_err(|e| e.to_string()).and_then(|spec| {
            let tags = export::tags(ask["title"].as_str().unwrap_or_default());
            export::encode(&Stereo::from_interleaved(samples), &spec, &tags)
                .map(|bytes| (spec, bytes))
                .map_err(|e| format!("{e:#}"))
        });
    match made {
        Ok((spec, bytes)) => {
            let json =
                serde_json::json!({ "encoded": { "id": id, "extension": spec.extension(), "mime": spec.mime() } });
            post_bytes(scope, &json, js_sys::Uint8Array::from(bytes.as_slice()));
        }
        Err(why) => post(scope, &serde_json::json!({ "encoded": { "id": id, "error": why } }), None),
    }
}

/// a render soon, when the song is newer than its render and none is on its way, or the next candidate of a mutation
/// or an evolution
fn schedule(engine: &Shared) {
    let mut e = engine.borrow_mut();
    if e.scheduled || !(e.host.stale() || e.host.breeding()) {
        return;
    }
    e.scheduled = true;
    let later = engine.clone();
    let render = Closure::once_into_js(move || render(&later));
    let _ = e.scope.set_timeout_with_callback_and_timeout_and_arguments_0(render.unchecked_ref(), 30);
}

fn render(engine: &Shared) {
    engine.borrow_mut().scheduled = false;
    // nothing plays an instrument before its files are in: the render comes once they are
    if fetch_files(engine) {
        return;
    }
    let mut e = engine.borrow_mut();
    // a candidate a turn, with the messages that came meanwhile (a stop, an edit) between them, once the song is heard
    if !e.host.stale() && e.host.breeding() {
        let what = e.host.breeding_command().unwrap_or_default();
        let done = match e.host.step() {
            Some(reply) => serde_json::json!({ "bred": what, "reply": reply, "state": e.host.state() }),
            None => serde_json::json!({ "state": e.host.state() }),
        };
        post(&e.scope, &done, None);
        drop(e);
        schedule(engine);
        return;
    }
    // the song's first bars first, to play while the rest renders: the commands that come meanwhile are made before it
    if let Some(preview) = e.host.preview() {
        let state = serde_json::json!({ "state": e.host.state() });
        post(&e.scope, &state, Some(js_sys::Float32Array::from(preview.samples.as_slice())));
        drop(e);
        schedule(engine);
        return;
    }
    // the render waits for a crew making its tracks; one making none the song still needs stops. The crew makes the
    // tracks of synths and drums: the instruments' files are the engine's alone, and it plays them itself
    let make = e.host.to_make().map(|mut m| {
        let project = m.project.clone();
        m.tracks.retain(|(name, _, _)| !sampled(&project, name));
        m
    });
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

/// `json` and a file's `bytes`, handed over to the page rather than copied
fn post_bytes(scope: &DedicatedWorkerGlobalScope, json: &serde_json::Value, bytes: js_sys::Uint8Array) {
    let msg = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&msg, &"json".into(), &json.to_string().into());
    let _ = js_sys::Reflect::set(&msg, &"bytes".into(), &bytes);
    if let Err(e) = scope.post_message_with_transfer(&msg, &js_sys::Array::of1(&bytes.buffer())) {
        web_sys::console::error_1(&e);
    }
}
