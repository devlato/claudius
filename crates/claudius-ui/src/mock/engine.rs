//! The demo's link to the claudius engine: a Web Worker running `claudius-engine` in a page, a thread of its own with
//! the same `Host` in the app. Both carry the same messages: commands, the loops found for a genre built on samples
//! and a question for the script reference go out; replies, states, renders, the sounds and the reference come in.

use claudius_api::{Reply, SoundList, State};

/// what the engine says
pub enum Heard {
    /// its answer to a command sent as `what`, and the state after it
    Answer {
        what: String,
        reply: Reply,
        state: Box<State>,
    },
    State(Box<State>),
    /// a render: the state with it, and the master, interleaved stereo at 48 kHz
    Rendered {
        state: Box<State>,
        samples: Vec<f32>,
    },
    Sounds(SoundList),
    Reference(String),
}

/// a loop found for a genre built on samples: as the engine prepares it (`Prepared`, as JSON), its credit, and its
/// audio, interleaved stereo at 48 kHz
pub struct Loop {
    pub prepared: serde_json::Value,
    pub credit: String,
    pub samples: Vec<f32>,
}

/// what a message from the engine says: its JSON, and the audio it hands over
#[cfg(any(target_arch = "wasm32", test))]
fn heard(json: &str, audio: Option<Vec<f32>>) -> Vec<Heard> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(r) = v["reference"].as_str() {
        out.push(Heard::Reference(r.to_string()));
    }
    if let Ok(s) = serde_json::from_value::<SoundList>(v["sounds"].clone()) {
        out.push(Heard::Sounds(s));
    }
    let Ok(state) = serde_json::from_value::<State>(v["state"].clone()).map(Box::new) else { return out };
    out.push(match (v["what"].as_str(), serde_json::from_value::<Reply>(v["reply"].clone()), audio) {
        (Some(what), Ok(reply), _) => Heard::Answer { what: what.to_string(), reply, state },
        (_, _, Some(samples)) => Heard::Rendered { state, samples },
        _ => Heard::State(state),
    });
    out
}

#[cfg(target_arch = "wasm32")]
pub use web::Link;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use claudius_api::Command;
    use eframe::egui;
    use js_sys::{Array, Float32Array, Object, Reflect};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    use super::{Heard, Loop};

    /// where trunk puts the engine's worker, beside the page
    const WORKER: &str = "./claudius-engine_loader.js";

    pub struct Link {
        worker: Option<web_sys::Worker>,
        heard: Rc<RefCell<VecDeque<Heard>>>,
        /// the worker has said something, so it listens: before then a message to it would be lost, since it starts
        /// listening only once its WebAssembly is loaded
        listening: Rc<std::cell::Cell<bool>>,
        /// what waits for it to listen
        held: Vec<(serde_json::Value, Vec<Float32Array>)>,
    }

    impl Link {
        /// starts the engine; `ctx` is woken when it says something
        pub fn new(ctx: Option<egui::Context>) -> Link {
            let heard = Rc::new(RefCell::new(VecDeque::new()));
            let worker = match web_sys::Worker::new(WORKER) {
                Ok(w) => w,
                Err(e) => {
                    web_sys::console::error_2(&"the demo's engine did not start:".into(), &e);
                    return Link { worker: None, heard, listening: Default::default(), held: Vec::new() };
                }
            };
            let (into, listening) = (heard.clone(), Rc::new(std::cell::Cell::new(false)));
            let heard_from = listening.clone();
            let onmessage = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |m: web_sys::MessageEvent| {
                heard_from.set(true);
                let data = m.data();
                let json = Reflect::get(&data, &"json".into()).ok().and_then(|j| j.as_string()).unwrap_or_default();
                let audio = Reflect::get(&data, &"audio".into()).ok().and_then(|a| a.dyn_into::<Float32Array>().ok());
                into.borrow_mut().extend(super::heard(&json, audio.map(|a| a.to_vec())));
                if let Some(c) = &ctx {
                    c.request_repaint();
                }
            });
            worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();
            Link { worker: Some(worker), heard, listening, held: Vec::new() }
        }

        pub fn command(&mut self, what: &str, cmd: &Command) {
            self.post(&serde_json::json!({ "what": what, "command": cmd }), Vec::new());
        }

        pub fn loops(&mut self, loops: Vec<Loop>) {
            let json: Vec<serde_json::Value> =
                loops.iter().map(|l| serde_json::json!({ "prepared": l.prepared, "credit": l.credit })).collect();
            let audio = loops.into_iter().map(|l| Float32Array::from(l.samples.as_slice())).collect();
            self.post(&serde_json::json!({ "loops": json }), audio);
        }

        pub fn reference(&mut self) {
            self.post(&serde_json::json!({ "reference": true }), Vec::new());
        }

        pub fn take(&mut self) -> Vec<Heard> {
            if self.listening.get() {
                for (json, audio) in std::mem::take(&mut self.held) {
                    self.post(&json, audio);
                }
            }
            self.heard.borrow_mut().drain(..).collect()
        }

        fn post(&mut self, json: &serde_json::Value, audio: Vec<Float32Array>) {
            if !self.listening.get() {
                self.held.push((json.clone(), audio));
                return;
            }
            let Some(worker) = &self.worker else { return };
            let msg = Object::new();
            let _ = Reflect::set(&msg, &"json".into(), &json.to_string().into());
            let sent = if audio.is_empty() {
                worker.post_message(&msg)
            } else {
                let transfer = Array::new();
                let arrays = Array::new();
                for a in audio {
                    transfer.push(&a.buffer());
                    arrays.push(&a);
                }
                let _ = Reflect::set(&msg, &"audio".into(), &arrays);
                worker.post_message_with_transfer(&msg, &transfer)
            };
            if let Err(e) = sent {
                web_sys::console::error_1(&e);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::Link;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

    use claudius::library::prepared::Prepared;
    use claudius_api::Command;
    use claudius_session::host::Host;
    use eframe::egui;

    use super::{Heard, Loop};

    enum Ask {
        Command(String, Command),
        Loops(Vec<Loop>),
        Reference,
    }

    pub struct Link {
        tx: Sender<Ask>,
        rx: Receiver<Heard>,
    }

    impl Link {
        /// starts the engine on a thread of its own; `ctx` is woken when it says something
        pub fn new(ctx: Option<egui::Context>) -> Link {
            let (tx, asks) = mpsc::channel::<Ask>();
            let (says, rx) = mpsc::channel::<Heard>();
            std::thread::spawn(move || {
                let say = |h: Heard| {
                    let _ = says.send(h);
                    if let Some(c) = &ctx {
                        c.request_repaint();
                    }
                };
                let mut host = Host::new(seed());
                say(Heard::State(Box::new(host.state())));
                say(Heard::Sounds(Host::sounds()));
                loop {
                    // what is asked comes first; once nothing is, a song newer than its render is rendered
                    let ask = if host.stale() {
                        match asks.try_recv() {
                            Ok(a) => Some(a),
                            Err(TryRecvError::Empty) => None,
                            Err(TryRecvError::Disconnected) => return,
                        }
                    } else {
                        match asks.recv() {
                            Ok(a) => Some(a),
                            Err(_) => return,
                        }
                    };
                    match ask {
                        Some(Ask::Command(what, c)) => {
                            let reply = host.command(c);
                            say(Heard::Answer { what, reply, state: Box::new(host.state()) });
                        }
                        Some(Ask::Loops(loops)) => {
                            let found = loops
                                .into_iter()
                                .filter_map(|l| {
                                    let p: Prepared = serde_json::from_value(l.prepared).ok()?;
                                    Some((p, l.credit, l.samples))
                                })
                                .collect();
                            host.add_loops(found);
                        }
                        Some(Ask::Reference) => say(Heard::Reference(Host::reference().to_string())),
                        // the song's first bars first, to play while the rest renders
                        None => match host.preview().map(Ok).or_else(|| host.render()) {
                            Some(Ok(a)) => say(Heard::Rendered { state: Box::new(host.state()), samples: a.samples }),
                            _ => say(Heard::State(Box::new(host.state()))),
                        },
                    }
                }
            });
            Link { tx, rx }
        }

        pub fn command(&mut self, what: &str, cmd: &Command) {
            let _ = self.tx.send(Ask::Command(what.to_string(), cmd.clone()));
        }

        pub fn loops(&mut self, loops: Vec<Loop>) {
            let _ = self.tx.send(Ask::Loops(loops));
        }

        pub fn reference(&mut self) {
            let _ = self.tx.send(Ask::Reference);
        }

        pub fn take(&mut self) -> Vec<Heard> {
            self.rx.try_iter().collect()
        }
    }

    fn seed() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn what_the_worker_sends_is_read_as_it_means() {
        let state = serde_json::to_value(State { version: 3, ..Default::default() }).unwrap();
        let reply = serde_json::to_value(Reply { ok: true, message: "tempo 100".into(), version: 3 }).unwrap();
        let answer = heard(&json!({ "what": "set_tempo", "reply": reply, "state": state }).to_string(), None);
        assert!(
            matches!(answer.as_slice(), [Heard::Answer { what, reply, .. }] if what == "set_tempo" && reply.ok),
            "an answer to a command"
        );
        let rendered = heard(&json!({ "state": state }).to_string(), Some(vec![0.5, -0.5]));
        assert!(matches!(rendered.as_slice(), [Heard::Rendered { samples, .. }] if samples.len() == 2), "a render");
        let first = heard(&json!({ "state": state, "sounds": SoundList::default() }).to_string(), None);
        assert!(matches!(first.as_slice(), [Heard::Sounds(_), Heard::State(_)]), "the first, with the sounds");
        let reference = heard(&json!({ "reference": "# claudius script" }).to_string(), None);
        assert!(matches!(reference.as_slice(), [Heard::Reference(r)] if r.starts_with("# claudius")));
        assert!(heard("not json", None).is_empty());
    }
}
