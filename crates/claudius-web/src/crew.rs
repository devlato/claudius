//! The crew: workers that make a song's tracks side by side for the engine, as threads do where there are some. A
//! member is the engine started as `claudius-engine_loader.js?crew`: it is handed the song as the renderer reads it,
//! the tracks to make and the audio they play, and posts back each track's sound as it makes it (`render::track_sound`),
//! which the engine keeps (`Host::made`) for a render that then only mixes. What comes out is what the engine would
//! have made alone.

use std::collections::HashMap;

use claudius::audio::{self, Stereo};
use claudius::render;
use claudius_session::host::ToMake;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker};

use crate::worker::post;

/// the engine's own script, which a member of the crew runs too
const SCRIPT: &str = "./claudius-engine_loader.js?crew";

/// the most workers a crew has, each a copy of the engine with memory of its own
const MOST: usize = 3;

/// whether this worker was started as a member of a crew
pub fn member(scope: &DedicatedWorkerGlobalScope) -> bool {
    scope.location().search().contains("crew")
}

/// as a member of a crew, make the tracks each message asks for, once it has said it listens (`{"ready": true}`):
/// what is posted to a worker before it listens is lost
pub fn serve(scope: DedicatedWorkerGlobalScope) {
    let s = scope.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| make(&s, &m));
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    post(&scope, &serde_json::json!({ "ready": true }), None);
}

/// The tracks a message asks for (`{"make": {"project", "tracks", "audio": [{"path", "frames"}]}}`, the audio one
/// interleaved array after another), each posted as it is made: `{"made": {"name", "level"}}` with its sound, or
/// `{"made": {"name", "error"}}`.
fn make(scope: &DedicatedWorkerGlobalScope, m: &MessageEvent) {
    let data = m.data();
    let Some(text) = js_sys::Reflect::get(&data, &"json".into()).ok().and_then(|j| j.as_string()) else { return };
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let job = &v["make"];
    let all = js_sys::Reflect::get(&data, &"audio".into())
        .ok()
        .and_then(|a| a.dyn_into::<js_sys::Float32Array>().ok())
        .map(|a| a.to_vec())
        .unwrap_or_default();
    let mut at = 0;
    for a in job["audio"].as_array().into_iter().flatten() {
        let (Some(path), Some(frames)) = (a["path"].as_str(), a["frames"].as_u64()) else { continue };
        let n = (2 * frames as usize).min(all.len() - at);
        audio::remember(path, Stereo::from_interleaved(&all[at..at + n]));
        at += n;
    }
    let names: Vec<&str> = job["tracks"].as_array().into_iter().flatten().filter_map(|n| n.as_str()).collect();
    let project = serde_json::from_value::<claudius::model::Project>(job["project"].clone());
    for name in names {
        let made = project
            .as_ref()
            .map_err(|e| e.to_string())
            .and_then(|p| render::track_sound(p, &render::Options::default(), name).map_err(|e| format!("{e:#}")));
        match made {
            Ok((sound, level)) => {
                let json = serde_json::json!({ "made": { "name": name, "level": level } });
                post(scope, &json, Some(js_sys::Float32Array::from(sound.interleaved().as_slice())));
            }
            Err(error) => post(scope, &serde_json::json!({ "made": { "name": name, "error": error } }), None),
        }
    }
}

/// A crew at work on the tracks of a render: its workers, each with the tracks it is to make until it listens, what
/// each track it makes is kept under, and the handlers of what they post, which live as long as it does.
pub struct Crew {
    workers: Vec<(Worker, Option<js_sys::Object>)>,
    keys: HashMap<String, render::TrackKey>,
    _heard: Vec<Closure<dyn FnMut(JsValue)>>,
}

/// what a worker of a crew said: that it listens, a track it made, or that it failed (`take`)
pub enum Said {
    Ready,
    Made(Made),
    Failed(String),
}

/// a track a worker of a crew made: its name, the key it is kept under, its sound and the level the mix hears in it
pub struct Made {
    pub name: String,
    pub key: render::TrackKey,
    pub sound: Stereo,
    pub level: Option<f64>,
}

impl Crew {
    /// whether it is still making a track that `make` lists, under the same key
    pub fn makes_any_of(&self, make: &ToMake) -> bool {
        make.tracks.iter().any(|(name, key, _)| self.keys.get(name) == Some(key))
    }

    /// What its worker `i` posted (`data`; null when the worker failed): that it listens, and so is handed its tracks
    /// now; a track it made (`{"made": ...}` with its sound); or why it could not make one. None when it is no track
    /// the crew still makes.
    pub fn hear(&mut self, i: usize, data: &JsValue) -> Option<Said> {
        if data.is_null() {
            return Some(Said::Failed("a worker of the crew stopped".into()));
        }
        let text = js_sys::Reflect::get(data, &"json".into()).ok()?.as_string()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        if v.get("ready").is_some() {
            let (worker, job) = self.workers.get_mut(i)?;
            let job = job.take()?;
            let samples = js_sys::Reflect::get(&job, &"audio".into()).ok()?;
            let buffer = samples.dyn_into::<js_sys::Float32Array>().ok()?.buffer();
            return Some(match worker.post_message_with_transfer(&job, &js_sys::Array::of1(&buffer)) {
                Ok(()) => Said::Ready,
                Err(_) => Said::Failed("a worker of the crew could not be handed its tracks".into()),
            });
        }
        let made = &v["made"];
        if let Some(error) = made["error"].as_str() {
            return Some(Said::Failed(error.to_string()));
        }
        let name = made["name"].as_str()?.to_string();
        let key = self.keys.remove(&name)?;
        let samples = js_sys::Reflect::get(data, &"audio".into()).ok()?.dyn_into::<js_sys::Float32Array>().ok()?;
        let sound = Stereo::from_interleaved(&samples.to_vec());
        Some(Said::Made(Made { name, key, sound, level: made["level"].as_f64() }))
    }

    /// whether every track it was given is made
    pub fn done(&self) -> bool {
        self.keys.is_empty()
    }

    /// stops its workers, and with them the memory they hold
    pub fn end(self) {
        for (w, _) in &self.workers {
            w.terminate();
        }
    }
}

/// How many workers a crew has here: the cores the browser says there are, less the page's and the engine's own. None
/// on a phone, whose browser holds a page's workers in memory it shares with the page and would sooner stop it than
/// hold three more copies of a song's tracks; and none for an engine started as `claudius-engine_loader.js?alone`,
/// which makes every track itself, as one does where there are no cores to spare.
pub fn size(scope: &DedicatedWorkerGlobalScope) -> usize {
    let n = scope.navigator();
    if scope.location().search().contains("alone") || n.user_agent().is_ok_and(|ua| ua.contains("Mobi")) {
        return 0;
    }
    (n.hardware_concurrency() as usize).saturating_sub(2).min(MOST)
}

/// Shares out what `make` lists among a crew of `size` workers, the costliest track first to the one with the least
/// to make, each to be handed the audio its tracks play with them once it listens; what worker `i` posts goes to
/// `heard(i, ...)`. None when a worker could not be started.
pub fn start(make: &ToMake, size: usize, heard: impl Fn(usize, JsValue) + Clone + 'static) -> Option<Crew> {
    let n = size.min(make.tracks.len());
    let mut parts: Vec<(f64, Vec<&str>)> = vec![(0.0, Vec::new()); n];
    for (name, _, cost) in &make.tracks {
        let least = (0..n).min_by(|&a, &b| parts[a].0.total_cmp(&parts[b].0)).unwrap_or(0);
        parts[least].0 += cost.max(1e-3);
        parts[least].1.push(name);
    }
    let project = serde_json::to_value(&make.project).ok()?;
    let mut crew = Crew {
        workers: Vec::new(),
        keys: make.tracks.iter().map(|(name, key, _)| (name.clone(), *key)).collect(),
        _heard: Vec::new(),
    };
    for (i, (_, names)) in parts.into_iter().enumerate() {
        let Ok(worker) = Worker::new(SCRIPT) else {
            crew.end();
            return None;
        };
        let h = heard.clone();
        let onmessage = Closure::<dyn FnMut(JsValue)>::new(move |m: JsValue| {
            h(i, js_sys::Reflect::get(&m, &"data".into()).unwrap_or(JsValue::NULL))
        });
        let h = heard.clone();
        let onerror = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| h(i, JsValue::NULL));
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        crew._heard.extend([onmessage, onerror]);
        // the audio its tracks play, one after another
        let mut list = Vec::new();
        let mut all: Vec<f32> = Vec::new();
        for t in make.project.tracks.iter().filter(|t| names.contains(&t.name.as_str())) {
            if let claudius::model::Sound::Sample { path, .. } | claudius::model::Sound::Loop { path, .. } = &t.sound
                && !list.iter().any(|(p, _): &(&str, usize)| p == path)
                && let Some((_, x)) = make.audio.iter().find(|(p, _)| p == path)
            {
                list.push((path.as_str(), x.len()));
                all.extend(x.interleaved());
            }
        }
        let audio: Vec<serde_json::Value> =
            list.iter().map(|(path, frames)| serde_json::json!({ "path": path, "frames": frames })).collect();
        let job = serde_json::json!({ "make": { "project": project, "tracks": names, "audio": audio } });
        let msg = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&msg, &"json".into(), &job.to_string().into());
        let _ = js_sys::Reflect::set(&msg, &"audio".into(), &js_sys::Float32Array::from(all.as_slice()));
        crew.workers.push((worker, Some(msg)));
    }
    Some(crew)
}
