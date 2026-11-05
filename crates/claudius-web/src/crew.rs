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

/// The parts of a track and the tracks a message asks for (`{"make": {"project", "parts": [{"name", "index", "from",
/// "to"}], "tracks", "audio": [{"path", "frames"}]}}`, the audio one interleaved array after another), the parts first,
/// each posted as it is made: `{"part": {"name", "index", "from", "to"}}` or `{"made": {"name", "level"}}` with its
/// sound, or `{"made": {"name", "error"}}`.
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
    for part in job["parts"].as_array().into_iter().flatten() {
        let (Some(name), Some(from), Some(to)) = (part["name"].as_str(), part["from"].as_u64(), part["to"].as_u64())
        else {
            continue;
        };
        let range = from as usize..to as usize;
        let made = project.as_ref().map_err(|e| e.to_string()).and_then(|p| {
            render::track_part(p, &render::Options::default(), name, range).map_err(|e| format!("{e:#}"))
        });
        match made {
            Ok(sound) => {
                let json =
                    serde_json::json!({ "part": { "name": name, "index": part["index"], "from": from, "to": to } });
                post(scope, &json, Some(js_sys::Float32Array::from(sound.interleaved().as_slice())));
            }
            Err(error) => post(scope, &serde_json::json!({ "made": { "name": name, "error": error } }), None),
        }
    }
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
/// each track it makes whole is kept under, the track it makes in parts, and the handlers of what they post, which
/// live as long as it does.
pub struct Crew {
    workers: Vec<(Worker, Option<js_sys::Object>)>,
    keys: HashMap<String, render::TrackKey>,
    split: Option<Split>,
    _heard: Vec<Closure<dyn FnMut(JsValue)>>,
}

/// A track a crew makes in parts, one a worker (`render::track_part`): its name, the key it is kept under, where each
/// part begins and ends and its share of the work, the parts made so far, and the song, to put them together with
/// (`render::track_joined`).
struct Split {
    name: String,
    key: render::TrackKey,
    bounds: Vec<std::ops::Range<usize>>,
    shares: Vec<f64>,
    parts: Vec<Option<Stereo>>,
    project: claudius::model::Project,
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
        make.tracks.iter().any(|(name, key, _)| {
            self.keys.get(name) == Some(key) || self.split.as_ref().is_some_and(|s| s.name == *name && s.key == *key)
        })
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
        if let Some(part) = v.get("part") {
            return self.part(part, data);
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

    /// A part of the track it makes in parts (`{"part": {"name", "index", "from", "to"}}` with its sound): kept, and
    /// once every part is in, the track they make together, through its effects as a render makes it.
    fn part(&mut self, part: &serde_json::Value, data: &JsValue) -> Option<Said> {
        let s = self.split.as_mut().filter(|s| part["name"].as_str() == Some(s.name.as_str()))?;
        let i = part["index"].as_u64()? as usize;
        let b = s.bounds.get(i)?;
        if part["from"].as_u64() != Some(b.start as u64) || part["to"].as_u64() != Some(b.end as u64) {
            return None;
        }
        let samples = js_sys::Reflect::get(data, &"audio".into()).ok()?.dyn_into::<js_sys::Float32Array>().ok()?;
        s.parts[i] = Some(Stereo::from_interleaved(&samples.to_vec()));
        if s.parts.iter().any(Option::is_none) {
            return None;
        }
        let s = self.split.take()?;
        let parts: Vec<Stereo> = s.parts.into_iter().flatten().collect();
        Some(match render::track_joined(&s.project, &render::Options::default(), &s.name, &parts) {
            Ok((sound, level)) => Said::Made(Made { name: s.name, key: s.key, sound, level }),
            Err(e) => Said::Failed(format!("{e:#}")),
        })
    }

    /// whether every track it was given is made
    pub fn done(&self) -> bool {
        self.keys.is_empty() && self.split.is_none()
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

/// Shares out what `make` lists among a crew of `size` workers. The costliest track, when it is more than an even share
/// of the work and its notes play apart (`render::splits`), is made in parts, one each, so that no worker waits on one
/// that makes it alone; the others go the costliest first to the one with the least to make. Each worker is handed the
/// audio its tracks play with them once it listens, and what worker `i` posts goes to `heard(i, ...)`. None when a
/// worker could not be started.
pub fn start(make: &ToMake, size: usize, heard: impl Fn(usize, JsValue) + Clone + 'static) -> Option<Crew> {
    let total: f64 = make.tracks.iter().map(|(_, _, c)| c.max(1e-3)).sum();
    let opts = render::Options::default();
    let split = make.tracks.first().and_then(|(name, key, cost)| {
        let track = make.project.tracks.iter().find(|t| t.name == *name)?;
        let heavy = *cost > total / size as f64 && render::splits(track);
        // a split helps only when no part holds most of the work: notes held together all go in one part
        let parts = render::part_bounds(&make.project, &opts, name, size)
            .ok()
            .filter(|b| heavy && b.len() >= 2 && b.iter().map(|(_, share)| *share).fold(0.0, f64::max) < 0.75)?;
        let (bounds, shares): (Vec<_>, Vec<_>) = parts.into_iter().unzip();
        Some(Split {
            name: name.clone(),
            key: *key,
            parts: vec![None; bounds.len()],
            bounds,
            shares,
            project: make.project.clone(),
        })
    });
    let whole: Vec<&(String, render::TrackKey, f64)> =
        make.tracks.iter().filter(|(name, _, _)| split.as_ref().is_none_or(|s| s.name != *name)).collect();
    let n = if split.is_some() { size } else { size.min(whole.len()) };
    let mut shares: Vec<(f64, Vec<&str>)> = vec![(0.0, Vec::new()); n];
    if let Some(s) = &split {
        for ((load, _), share) in shares.iter_mut().zip(&s.shares) {
            *load += make.tracks[0].2 * share;
        }
    }
    for (name, _, cost) in whole {
        let least = (0..n).min_by(|&a, &b| shares[a].0.total_cmp(&shares[b].0)).unwrap_or(0);
        shares[least].0 += cost.max(1e-3);
        shares[least].1.push(name);
    }
    let project = serde_json::to_value(&make.project).ok()?;
    let parts: Vec<Option<serde_json::Value>> = (0..n)
        .map(|i| {
            let s = split.as_ref()?;
            let b = s.bounds.get(i)?;
            Some(serde_json::json!({ "name": s.name, "index": i, "from": b.start, "to": b.end }))
        })
        .collect();
    let mut crew = Crew {
        workers: Vec::new(),
        keys: make
            .tracks
            .iter()
            .filter(|(name, _, _)| split.as_ref().is_none_or(|s| s.name != *name))
            .map(|(name, key, _)| (name.clone(), *key))
            .collect(),
        split,
        _heard: Vec::new(),
    };
    for (i, (_, names)) in shares.into_iter().enumerate() {
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
        let part: Vec<&serde_json::Value> = parts[i].iter().collect();
        let job = serde_json::json!({ "make": { "project": project, "parts": part, "tracks": names, "audio": audio } });
        let msg = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&msg, &"json".into(), &job.to_string().into());
        let _ = js_sys::Reflect::set(&msg, &"audio".into(), &js_sys::Float32Array::from(all.as_slice()));
        crew.workers.push((worker, Some(msg)));
    }
    Some(crew)
}
