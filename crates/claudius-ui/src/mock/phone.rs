//! A phone playing along with the demo. The demo is a page, which a phone cannot reach, so both join a room of the
//! site's relay (`site/worker/room.js`), and the phone's page (`/phone/?room=`) asks the demo what it would ask a
//! claudius: the song, the time, each lane's hit, the keys' notes, a record, and a take. The demo answers from its own
//! engine, and makes a take as the studio would. The room's name is 128 random bits, kept for the tab's session, so a
//! reload keeps the phones that scanned its code.

use std::collections::{BTreeSet, HashMap};

use claudius_api::{Command, PhoneView, Reply, Transport};
use serde_json::{Value, json};

use super::Mock;
use crate::net::Event;

/// how long the page's question for a newer song is held, in seconds
const HOLD: f64 = 20.0;
/// the seconds between the notes of the keys' sound, as `claudius::pads::SLOT_SECS`
pub const SLOT_SECS: f64 = 1.5;
/// what a phone's take is sent to the engine as: this, and a number
const TAKE: &str = "phone take ";
/// the most answers kept for the phones while the room cannot be reached
const KEEP: usize = 256;

/// a question from a phone's page, as the relay brings it
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub phone: String,
    pub id: u64,
    pub method: String,
    pub path: String,
    pub body: Value,
}

/// what goes back to a phone: an answer, or a sound
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    Answer { phone: String, id: u64, status: u16, body: Value },
    Sound { phone: String, id: u64, wav: Vec<u8> },
}

/// what the room says
#[derive(Clone, Debug, PartialEq)]
pub enum Heard {
    Asked(Asked),
    Joined(String),
    Left(String),
    /// the connection to the room closed (`\n{"event":"closed"}`, as the socket says it)
    Closed,
    /// the room was taken by the demo opened again in another tab, which keeps the room's name
    /// (`\n{"event":"replaced"}`)
    Replaced,
}

/// the demo's side of its room
#[derive(Default)]
pub struct Relay {
    pub url: String,
    phones: BTreeSet<String>,
    /// questions for a song newer than a version, held until it comes or `HOLD` seconds pass
    held: Vec<(Asked, u64, f64)>,
    /// the sounds rendered for the phones as WAV files, by the song's version and what they are
    sounds: HashMap<(u64, String), Vec<u8>>,
    /// what the engine renders for the phones, by the id it answers as
    rendering: HashMap<u64, Rendering>,
    /// takes sent to the engine, by what they were sent as
    takes: HashMap<String, Asked>,
    /// what is to go to the phones, kept while the room cannot be reached (the oldest let go past `KEEP`)
    pub out: Vec<Said>,
    /// when to join the room again, after the connection closed
    rejoin: Option<f64>,
    #[cfg(target_arch = "wasm32")]
    socket: Option<socket::Socket>,
    /// what the room has brought, as it brings it: with no socket here, what a test puts in
    #[cfg(not(target_arch = "wasm32"))]
    pub inbox: Vec<String>,
}

/// sounds the engine renders for the phones: for which version of the song, what they are, and who waits for which
struct Rendering {
    version: u64,
    what: String,
    waiting: Vec<(Asked, String)>,
}

impl Relay {
    /// a room joined without a socket, for a test to bring what a room would and read what goes back
    #[cfg(test)]
    pub fn standing(url: &str) -> Relay {
        Relay { url: url.into(), ..Default::default() }
    }

    fn answer(&mut self, a: &Asked, status: u16, body: Value) {
        self.out.push(Said::Answer { phone: a.phone.clone(), id: a.id, status, body });
    }
}

/// what the relay brought: `<phone>\n<message>`, the message a question or what the room says of a phone
pub fn heard(text: &str) -> Option<Heard> {
    let (phone, message) = text.split_once('\n')?;
    let v: Value = serde_json::from_str(message).ok()?;
    match v["event"].as_str() {
        Some("joined") => return Some(Heard::Joined(phone.into())),
        Some("left") => return Some(Heard::Left(phone.into())),
        Some("closed") => return Some(Heard::Closed),
        Some("replaced") => return Some(Heard::Replaced),
        _ => {}
    }
    Some(Heard::Asked(Asked {
        phone: phone.into(),
        id: v["id"].as_u64()?,
        method: v["method"].as_str().unwrap_or("GET").into(),
        path: v["path"].as_str()?.into(),
        body: v["body"].clone(),
    }))
}

/// a sound for a phone: 16-bit stereo WAV at 48 kHz, from interleaved samples
fn wav(samples: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut out = std::io::Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
    if let Ok(mut w) = hound::WavWriter::new(&mut out, spec) {
        for s in samples {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32_767.0).round() as i16);
        }
        let _ = w.finalize();
    }
    out.into_inner()
}

/// a query's value for `key`, its %-escapes undone
fn param(query: &str, key: &str) -> Option<String> {
    let raw = query.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))?;
    let bytes = raw.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

impl Mock {
    /// serves the phone's page through the site's relay, or stops serving it
    pub fn phone(&mut self, on: bool) {
        if !on {
            self.relay = None;
            self.publish();
            return;
        }
        if self.relay.is_some() {
            return;
        }
        match Relay::join(self.ctx.clone()) {
            Ok(r) => self.relay = Some(r),
            Err(e) => self.events.push(Event::PhoneRefused(e)),
        }
        self.publish();
    }

    /// where the song is, as the studio says while a phone plays along
    pub fn transport(&mut self, t: Transport) {
        self.transport = (t, self.now);
    }

    /// what the page shows of the phones: the page's address, and how many have it open
    pub(super) fn phone_view(&self) -> Option<PhoneView> {
        self.relay.as_ref().map(|r| PhoneView { url: r.url.clone(), phones: r.phones.len() as u32 })
    }

    /// what the room brought since, answered, and the answers sent
    pub(super) fn phones(&mut self) {
        let Some(relay) = self.relay.as_mut() else { return };
        for h in relay.heard() {
            match h {
                Heard::Asked(a) => self.asked(a),
                Heard::Joined(p) => {
                    self.relay().phones.insert(p);
                    self.publish();
                }
                Heard::Left(p) => {
                    self.relay().phones.remove(&p);
                    self.publish();
                }
                Heard::Closed => {
                    let relay = self.relay.as_mut().expect("the relay");
                    relay.phones.clear();
                    relay.rejoin = Some(self.now + 3.0);
                    self.publish();
                }
                // the tab opened again plays with the phone now: this one lets it, rather than take the room back
                Heard::Replaced => {
                    self.relay = None;
                    let text = "the phone plays along with this demo opened in another tab now".to_string();
                    self.events.push(Event::Said { text, ok: true });
                    self.publish();
                    return;
                }
            }
        }
        let (version, now) = (self.state.song_version, self.now);
        let Some(relay) = self.relay.as_mut() else { return };
        let held = std::mem::take(&mut relay.held);
        let (due, wait): (Vec<_>, Vec<_>) =
            held.into_iter().partition(|(_, after, until)| version > *after || now >= *until);
        relay.held = wait;
        for (a, ..) in due {
            self.song(&a);
        }
        let ctx = self.ctx.clone();
        let relay = self.relay.as_mut().expect("the relay");
        if relay.rejoin.is_some_and(|t| now >= t) {
            relay.rejoin = None;
            relay.reopen(ctx);
        }
        relay.send();
    }

    /// a phone's question, answered at once, or once what it waits for is here
    pub(super) fn asked(&mut self, a: Asked) {
        let (path, query) = a.path.split_once('?').unwrap_or((a.path.as_str(), ""));
        let (path, query) = (path.to_string(), query.to_string());
        let n = |k: &str| param(&query, k).and_then(|v| v.parse::<i64>().ok());
        match (a.method.as_str(), path.as_str()) {
            ("GET", "api/clock") => {
                let (t, when) = self.transport;
                let body = json!({
                    "now": self.now * 1000.0,
                    "playing": t.playing,
                    "at": t.at,
                    "since": when * 1000.0,
                    "version": self.state.song_version,
                });
                self.relay().answer(&a, 200, body);
            }
            ("GET", "api/song") => match n("after") {
                Some(after) if self.state.song_version as i64 <= after => {
                    let until = self.now + HOLD;
                    self.relay().held.push((a, after as u64, until));
                }
                _ => self.song(&a),
            },
            ("GET", p) if p.starts_with("api/hits/") => {
                let lane = param(&format!("l={}", &p["api/hits/".len()..]), "l").unwrap_or_default();
                self.render(a, "hits", &format!("hit:{lane}"), json!({ "kind": "hits" }));
            }
            ("GET", "api/keys") => match (param(&query, "sound"), n("low"), n("high"), n("step")) {
                (Some(sound), Some(low), Some(high), step) => {
                    let step = step.unwrap_or(3);
                    let what = format!("keys:{sound}:{low}:{high}:{step}");
                    let ask = json!({ "kind": "keys", "sound": sound, "low": low, "high": high, "step": step });
                    self.render(a, &what, &what, ask);
                }
                _ => self.relay().answer(&a, 400, json!("keys are asked for by sound, low and high")),
            },
            ("GET", "api/record") => match (param(&query, "track"), n("bar"), n("bars")) {
                (Some(track), bar, bars) => {
                    let (bar, bars) = (bar.unwrap_or(0).max(0), bars.unwrap_or(1).max(0));
                    let what = format!("record:{track}:{bar}:{bars}");
                    let ask = json!({ "kind": "record", "track": track, "bar": bar, "bars": bars });
                    self.render(a, &what, &what, ask);
                }
                _ => self.relay().answer(&a, 400, json!("a record is asked for by track")),
            },
            ("POST", "api/take") => match serde_json::from_value::<Command>(a.body.clone()) {
                Ok(c @ (Command::AddHits { .. } | Command::AddNotes { .. } | Command::AddScratch { .. })) => {
                    let what = format!("{TAKE}{}", self.ask_id());
                    self.relay().takes.insert(what.clone(), a);
                    self.engine.command(&what, &c);
                    self.waiting += 1;
                }
                _ => self.relay().answer(&a, 403, json!("a phone sends takes: add_hits, add_notes or add_scratch")),
            },
            _ => self.relay().answer(&a, 404, json!(format!("the demo has no {}", a.path))),
        }
    }

    fn relay(&mut self) -> &mut Relay {
        self.relay.as_mut().expect("a relay, since a phone asked through it")
    }

    /// a number for what the engine is asked for the phones, never given before: an answer the engine was still
    /// working on when the page stopped being served cannot go to a question asked since
    fn ask_id(&mut self) -> u64 {
        self.phone_asks += 1;
        self.phone_asks
    }

    /// the song, as the page shows and plays it (the server's `phone.rs` says the same)
    fn song(&mut self, a: &Asked) {
        let Some(s) = self.state.song.as_ref() else {
            self.relay().answer(a, 404, json!("no song yet: make one in the demo"));
            return;
        };
        let synths = self.sounds.as_ref().map(|l| l.synths.clone()).unwrap_or_default();
        let instruments = self.sounds.as_ref().map(|l| l.instruments.clone()).unwrap_or_default();
        let sections: Vec<Value> = s
            .sections
            .iter()
            .map(|x| json!({ "name": x.name, "start_bar": x.start_bar, "bars": x.bars, "layers": x.layers }))
            .collect();
        let body = json!({
            "version": self.state.song_version,
            "title": s.title,
            "bpm": s.bpm,
            "key": s.key,
            "sections": sections,
            "lanes": s.drums.iter().map(|d| &d.lane).collect::<Vec<_>>(),
            "tracks": s.tracks.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "parts": s.parts.iter().filter(|p| !p.preset.is_empty()).map(|p| (&p.name, &p.preset)).collect::<Vec<_>>(),
            "synths": synths,
            "instruments": instruments,
            "slot": SLOT_SECS,
        });
        self.relay().answer(a, 200, body);
    }

    /// sound `key` for `a`, from what the engine rendered for this song, or rendered now (as `what`, by `ask`)
    fn render(&mut self, a: Asked, what: &str, key: &str, ask: Value) {
        let version = self.state.song_version;
        let relay = self.relay();
        if let Some(wav) = relay.sounds.get(&(version, key.to_string())) {
            relay.out.push(Said::Sound { phone: a.phone.clone(), id: a.id, wav: wav.clone() });
            return;
        }
        if let Some(r) = relay.rendering.values_mut().find(|r| r.version == version && r.what == what) {
            r.waiting.push((a, key.to_string()));
            return;
        }
        let id = self.ask_id();
        let waiting = vec![(a, key.to_string())];
        self.relay().rendering.insert(id, Rendering { version, what: what.to_string(), waiting });
        self.engine.sound(id, ask);
    }

    /// the engine's sounds for the phones, kept and handed to who waits for them
    pub(super) fn rendered(&mut self, id: u64, sounds: Result<Vec<(String, Vec<f32>)>, String>) {
        let Some(relay) = self.relay.as_mut() else { return };
        let Some(Rendering { version, what, waiting }) = relay.rendering.remove(&id) else { return };
        let made = match sounds {
            Ok(list) => list,
            Err(e) => {
                for (a, _) in &waiting {
                    relay.answer(a, 400, json!(e));
                }
                return;
            }
        };
        relay.sounds.retain(|(v, _), _| *v == version);
        for (name, samples) in made {
            let key = if what == "hits" { format!("hit:{name}") } else { what.clone() };
            relay.sounds.insert((version, key), wav(&samples));
        }
        for (a, key) in waiting {
            match relay.sounds.get(&(version, key.clone())) {
                Some(w) => relay.out.push(Said::Sound { phone: a.phone.clone(), id: a.id, wav: w.clone() }),
                None => relay.answer(&a, 404, json!(format!("the song has no {}", key.trim_start_matches("hit:")))),
            }
        }
    }

    /// the engine's answer to a take sent as `what`, if a phone sent it: to the phone, and to the person as news
    pub(super) fn took(&mut self, what: &str, reply: &Reply) -> bool {
        let Some(a) = self.relay.as_mut().and_then(|r| r.takes.remove(what)) else { return what.starts_with(TAKE) };
        let body = serde_json::to_value(reply).unwrap_or_default();
        self.relay().answer(&a, 200, body);
        let text = format!("from the phone: {}", reply.message);
        self.events.push(Event::Said { text, ok: reply.ok });
        true
    }
}

impl Relay {
    #[cfg(target_arch = "wasm32")]
    fn join(ctx: Option<eframe::egui::Context>) -> Result<Relay, String> {
        let (url, socket) = socket::join(ctx)?;
        Ok(Relay { url, socket: Some(socket), ..Default::default() })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn join(_ctx: Option<eframe::egui::Context>) -> Result<Relay, String> {
        Err("a phone plays along with the demo in a browser, or with the app's own claudius".into())
    }

    #[cfg(target_arch = "wasm32")]
    fn reopen(&mut self, ctx: Option<eframe::egui::Context>) {
        match socket::join(ctx) {
            Ok((_, s)) => self.socket = Some(s),
            Err(_) => self.rejoin = None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn reopen(&mut self, _ctx: Option<eframe::egui::Context>) {}

    #[cfg(target_arch = "wasm32")]
    fn heard(&mut self) -> Vec<Heard> {
        self.socket.as_ref().map(|s| s.take()).unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn heard(&mut self) -> Vec<Heard> {
        self.inbox.drain(..).filter_map(|t| heard(&t)).collect()
    }

    /// what is to go to the phones, sent while the room can be reached, and kept (the newest `KEEP` of it) until it can:
    /// with no socket here, kept in `out`, where a test reads it
    fn send(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(s) = self.socket.as_ref().filter(|s| s.open()) {
            for said in std::mem::take(&mut self.out) {
                s.send(&said);
            }
        }
        let n = self.out.len();
        self.out.drain(..n.saturating_sub(KEEP));
    }
}

#[cfg(target_arch = "wasm32")]
mod socket {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use eframe::egui;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{BinaryType, MessageEvent, WebSocket};

    use super::{Heard, Said};

    /// what the tab keeps of its room for the session
    const ROOM: &str = "claudius-phone-room";

    /// the room's WebSocket, and what it has brought
    pub struct Socket {
        ws: WebSocket,
        heard: Rc<RefCell<VecDeque<Heard>>>,
        _onmessage: Closure<dyn FnMut(MessageEvent)>,
        _onclose: Closure<dyn FnMut(web_sys::Event)>,
    }

    /// the tab's room, joined as its host: the phone's page's address, and the socket
    pub fn join(ctx: Option<egui::Context>) -> Result<(String, Socket), String> {
        let window = web_sys::window().ok_or("the demo has no window")?;
        let location = window.location();
        let origin = location.origin().map_err(|_| "the demo does not know where it is")?;
        let host = location.host().map_err(|_| "the demo does not know where it is")?;
        let https = location.protocol().is_ok_and(|p| p == "https:");
        let storage = window.session_storage().ok().flatten();
        let kept = storage.as_ref().and_then(|s| s.get_item(ROOM).ok().flatten());
        let room = match kept.filter(|r| r.len() == 32 && r.bytes().all(|b| b.is_ascii_hexdigit())) {
            Some(r) => r,
            None => {
                let mut b = [0u8; 16];
                let crypto = window.crypto().map_err(|_| "the browser gives the demo no randomness")?;
                crypto
                    .get_random_values_with_u8_array(&mut b)
                    .map_err(|_| "the browser gives the demo no randomness")?;
                b.iter().map(|x| format!("{x:02x}")).collect()
            }
        };
        if let Some(s) = &storage {
            let _ = s.set_item(ROOM, &room);
        }
        let ws = WebSocket::new(&format!("{}://{host}/relay/{room}?as=host", if https { "wss" } else { "ws" }))
            .map_err(|_| "the site's relay cannot be reached".to_string())?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let heard = Rc::new(RefCell::new(VecDeque::new()));
        let (into, wake) = (heard.clone(), ctx.clone());
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| {
            if let Some(h) = m.data().as_string().as_deref().and_then(super::heard) {
                into.borrow_mut().push_back(h);
            }
            if let Some(c) = &wake {
                c.request_repaint();
            }
        });
        let into = heard.clone();
        let onclose = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
            // the room closes a tab's socket with 4000 when the demo is opened again in another tab
            let replaced = e.dyn_ref::<web_sys::CloseEvent>().is_some_and(|c| c.code() == 4000);
            let said = if replaced { "\n{\"event\":\"replaced\"}" } else { "\n{\"event\":\"closed\"}" };
            into.borrow_mut().extend(super::heard(said));
            if let Some(c) = &ctx {
                c.request_repaint();
            }
        });
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        let socket = Socket { ws, heard, _onmessage: onmessage, _onclose: onclose };
        Ok((format!("{origin}/phone/?room={room}"), socket))
    }

    impl Socket {
        pub fn take(&self) -> Vec<Heard> {
            self.heard.borrow_mut().drain(..).collect()
        }

        pub fn open(&self) -> bool {
            self.ws.ready_state() == WebSocket::OPEN
        }

        pub fn send(&self, said: &Said) {
            let _ = match said {
                Said::Answer { phone, id, status, body } => {
                    let answer = serde_json::json!({ "id": id, "status": status, "body": body });
                    self.ws.send_with_str(&format!("{phone}\n{answer}"))
                }
                Said::Sound { phone, id, wav } => {
                    let mut b = Vec::with_capacity(phone.len() + 4 + wav.len());
                    b.extend_from_slice(phone.as_bytes());
                    b.extend_from_slice(&(*id as u32).to_le_bytes());
                    b.extend_from_slice(wav);
                    self.ws.send_with_u8_array(&b)
                }
            };
        }
    }

    impl Drop for Socket {
        fn drop(&mut self) {
            self.ws.set_onmessage(None);
            self.ws.set_onclose(None);
            let _ = self.ws.close();
        }
    }
}
