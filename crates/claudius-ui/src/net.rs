//! Talking to the claudius server: a long-poll loop on `/api/state?since=N`, commands as JSON POSTs to
//! `/api/command`, the sound list from `/api/sounds`. `ehttp` runs the requests (threads natively, `fetch` on the
//! web) and hands results back through a channel; every arrival requests a repaint. `Backend` hides whether the
//! other side is a real server or the built-in mock.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use claudius_api::{Command, ExportFormat, RenderView, Reply, SoundList, State, Transport};
use eframe::egui;

use crate::audio::Source;
use crate::keep::Delivery;
use crate::mock::Mock;

/// where the studio gets its state from
#[derive(Clone, Debug, PartialEq)]
pub enum Connection {
    /// a claudius server at this base URL (`http://127.0.0.1:7878`)
    Server(String),
    /// the built-in demo
    Mock,
}

impl Connection {
    /// `mock` / `demo`, a URL, or `host:port`
    pub fn parse(s: &str) -> Connection {
        let s = s.trim();
        if s.eq_ignore_ascii_case("mock") || s.eq_ignore_ascii_case("demo") {
            return Connection::Mock;
        }
        let url = if s.contains("://") { s.to_string() } else { format!("http://{s}") };
        Connection::Server(url.trim_end_matches('/').to_string())
    }

    /// `server` (argument), else env `CLAUDIUS_SERVER`, else `http://127.0.0.1:7878`; env `CLAUDIUS_MOCK=1` (or a
    /// server of `mock`) selects the demo
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native(server: Option<String>) -> Connection {
        let env = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        if env("CLAUDIUS_MOCK").is_some_and(|v| v != "0" && !v.eq_ignore_ascii_case("false")) {
            return Connection::Mock;
        }
        let server = server.filter(|s| !s.trim().is_empty()).or_else(|| env("CLAUDIUS_SERVER"));
        Connection::parse(&server.unwrap_or_else(|| format!("http://127.0.0.1:{}", claudius_api::DEFAULT_PORT)))
    }

    /// did the page choose a server or the demo itself (`?server=`, `?mock`)? If not, a page with no claudius behind
    /// it falls back to the demo
    #[cfg(target_arch = "wasm32")]
    pub fn web_chosen() -> bool {
        let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
        search
            .trim_start_matches('?')
            .split('&')
            .any(|p| matches!(p.split('=').next(), Some("mock" | "demo" | "server")))
    }

    /// does the page open on the Record window (`?record=<id>`: the link an assistant gives for a take it asks for)?
    #[cfg(target_arch = "wasm32")]
    pub fn web_records() -> bool {
        let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
        search.trim_start_matches('?').split('&').any(|p| p.split('=').next() == Some("record"))
    }

    /// the page's own origin; `?mock` selects the demo, `?server=http://host:port` another server
    #[cfg(target_arch = "wasm32")]
    pub fn web() -> Connection {
        let Some(location) = web_sys::window().map(|w| w.location()) else {
            return Connection::Mock;
        };
        let search = location.search().unwrap_or_default();
        for pair in search.trim_start_matches('?').split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            match k {
                "mock" | "demo" => return Connection::Mock,
                "server" if !v.is_empty() => {
                    let v = js_sys::decode_uri_component(v).map(String::from).unwrap_or_else(|_| v.to_string());
                    return Connection::parse(&v);
                }
                _ => {}
            }
        }
        match location.origin() {
            Ok(origin) if origin.starts_with("http") => Connection::Server(origin),
            _ => Connection::Mock,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Connection::Server(url) => url.clone(),
            Connection::Mock => "the built-in demo".into(),
        }
    }
}

pub enum Event {
    State(Box<State>),
    /// the state request failed (server down, wrong address, not a claudius server)
    PollFailed(String),
    Reply {
        what: String,
        result: Result<Reply, String>,
    },
    Sounds(SoundList),
    /// what `/api/hello` says about the server: its name and version ("claudius 0.1.0") and whether it has ears
    Hello {
        name: String,
        ears: Option<bool>,
    },
    /// the script reference (markdown)
    Reference(Result<String, String>),
    /// the release's artwork as a picture to show, for `ReleaseView.artwork_version`
    Preview {
        version: u64,
        image: Result<egui::ColorImage, String>,
    },
    /// news for the person that no request of theirs asked for: a take played on a phone, say
    Said {
        text: String,
        ok: bool,
    },
    /// the page a phone plays along on was not served, and why
    PhoneRefused(String),
    /// a file asked for to download, made since (`Backend::download`), or why there is none
    Delivered(Result<Delivery, String>),
}

/// the `"cmd"` tag of a command ("set_tempo")
pub fn command_name(cmd: &Command) -> String {
    serde_json::to_value(cmd)
        .ok()
        .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_else(|| "command".into())
}

/// what sending a recording to listen to is: the reply it answers, the endpoint, and the query naming the file (and
/// the genre and seed of a remake)
fn listen_request(l: &crate::panels::Listen) -> (&'static str, &'static str, String) {
    let name = format!("name={}", query_value(&l.file.name));
    if l.sample {
        return ("sample_song", "/api/sample", name);
    }
    if l.groove {
        return ("extract_groove", "/api/groove", name);
    }
    match &l.remake {
        None => ("analyze", "/api/analyze", name),
        Some((genre, seed)) => {
            let seed = seed.map_or_else(String::new, |s| format!("&seed={s}"));
            ("remake", "/api/remake", format!("{name}&genre={}{seed}", query_value(genre)))
        }
    }
}

/// text as a value in a URL's query: letters, digits and `-_.~` as they are, every other byte as %XX
fn query_value(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub struct Remote {
    base: String,
    ctx: egui::Context,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    polling: bool,
    since: Option<u64>,
    retry_at: Option<f64>,
    failures: u32,
    sounds_requested: bool,
    /// after `install_packs`: Some(false) until its job is seen running, Some(true) until it ends, when the sound list
    /// is fetched again
    sounds_after_install: Option<bool>,
    /// the last state said the ears were loading: `/api/hello` is asked again when they are done
    ears_loading: bool,
}

impl Remote {
    fn new(base: String, ctx: &egui::Context) -> Remote {
        let (tx, rx) = channel();
        Remote {
            base,
            ctx: ctx.clone(),
            tx,
            rx,
            polling: false,
            since: None,
            retry_at: None,
            failures: 0,
            sounds_requested: false,
            sounds_after_install: None,
            ears_loading: false,
        }
    }

    pub fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else if path.starts_with('/') {
            format!("{}{path}", self.base)
        } else {
            format!("{}/{path}", self.base)
        }
    }

    fn deliver(tx: &Sender<Event>, ctx: &egui::Context, ev: Event) {
        let _ = tx.send(ev);
        ctx.request_repaint();
    }

    fn poll_state(&mut self) {
        self.polling = true;
        let url = match self.since {
            Some(v) => self.url(&format!("/api/state?since={v}")),
            None => self.url("/api/state"),
        };
        let mut request = ehttp::Request::get(&url);
        request.timeout = Some(Duration::from_secs(45));
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let ev = match res {
                Ok(r) if r.ok => match serde_json::from_slice::<State>(&r.bytes) {
                    Ok(s) => Event::State(Box::new(s)),
                    Err(e) => Event::PollFailed(format!("{url} did not answer with a claudius state ({e})")),
                },
                Ok(r) => Event::PollFailed(format!("{url} answered {} {}", r.status, r.status_text)),
                Err(e) => Event::PollFailed(e),
            };
            Self::deliver(&tx, &ctx, ev);
        });
    }

    fn fetch_hello(&self) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(ehttp::Request::get(self.url("/api/hello")), move |res| {
            if let Ok(r) = res
                && r.ok
                && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&r.bytes)
            {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("claudius");
                let version = v.get("version").and_then(|x| x.as_str()).unwrap_or("");
                let ears = v.get("ears").and_then(|x| x.as_bool());
                Self::deliver(&tx, &ctx, Event::Hello { name: format!("{name} {version}"), ears });
            }
        });
    }

    fn fetch_reference(&self) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(ehttp::Request::get(self.url("/api/reference")), move |res| {
            let result = match res {
                Ok(r) if r.ok => Ok(String::from_utf8_lossy(&r.bytes).into_owned()),
                Ok(r) => Err(format!("/api/reference answered {} {}", r.status, r.status_text)),
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Reference(result));
        });
    }

    fn fetch_sounds(&mut self) {
        self.sounds_requested = true;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(ehttp::Request::get(self.url("/api/sounds")), move |res| {
            if let Ok(r) = res
                && r.ok
                && let Ok(list) = serde_json::from_slice::<SoundList>(&r.bytes)
            {
                Self::deliver(&tx, &ctx, Event::Sounds(list));
            }
        });
    }

    fn upload_artwork(&mut self, bytes: Vec<u8>) {
        let mut request = ehttp::Request::post(self.url("/api/release/artwork"), bytes);
        request.headers =
            ehttp::Headers::new(&[("Accept", "application/json"), ("Content-Type", "application/octet-stream")]);
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let result = match res {
                Ok(r) => {
                    serde_json::from_slice::<Reply>(&r.bytes).map_err(|_| format!("{} {}", r.status, r.status_text))
                }
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Reply { what: "artwork".into(), result });
        });
    }

    fn upload_take(&mut self, wav: Vec<u8>, query: &str) {
        let mut request = ehttp::Request::post(self.url(&format!("/api/takes?{query}")), wav);
        request.headers = ehttp::Headers::new(&[("Accept", "application/json"), ("Content-Type", "audio/wav")]);
        request.timeout = Some(Duration::from_secs(300));
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let result = match res {
                Ok(r) => {
                    serde_json::from_slice::<Reply>(&r.bytes).map_err(|_| format!("{} {}", r.status, r.status_text))
                }
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Reply { what: "take".into(), result });
        });
    }

    /// a project to open, the zip itself, named `name`
    fn upload_project(&mut self, name: &str, zip: Vec<u8>) {
        let url = self.url(&format!("/api/project?name={}", query_value(name)));
        let mut request = ehttp::Request::post(url, zip);
        request.headers = ehttp::Headers::new(&[("Accept", "application/json"), ("Content-Type", "application/zip")]);
        request.timeout = Some(Duration::from_secs(300));
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let result = match res {
                Ok(r) => {
                    serde_json::from_slice::<Reply>(&r.bytes).map_err(|_| format!("{} {}", r.status, r.status_text))
                }
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Reply { what: "open".into(), result });
        });
    }

    /// a recording to listen to or remake, the file itself, to `/api/analyze` or `/api/remake` with `query`
    fn listen(&mut self, bytes: Vec<u8>, path: &str, query: &str, what: &'static str) {
        let mut request = ehttp::Request::post(self.url(&format!("{path}?{query}")), bytes);
        request.headers =
            ehttp::Headers::new(&[("Accept", "application/json"), ("Content-Type", "application/octet-stream")]);
        request.timeout = Some(Duration::from_secs(300));
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let result = match res {
                Ok(r) => {
                    serde_json::from_slice::<Reply>(&r.bytes).map_err(|_| format!("{} {}", r.status, r.status_text))
                }
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Reply { what: what.into(), result });
        });
    }

    /// the page a phone plays along on: served (`on`), or stopped; the page itself comes with the state
    fn phone(&self, on: bool) {
        let url = self.url("/api/phone");
        let request = if on { ehttp::Request::post(url, Vec::new()) } else { ehttp::Request::delete(&url) };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let refused = match res {
                Ok(r) if r.ok => return,
                Ok(r) => {
                    let text = String::from_utf8_lossy(&r.bytes).trim().to_string();
                    if text.is_empty() { format!("{} {}", r.status, r.status_text) } else { text }
                }
                Err(e) => e,
            };
            Self::deliver(&tx, &ctx, Event::PhoneRefused(refused));
        });
    }

    /// where the song is, for a phone playing along
    fn transport(&self, t: Transport) {
        let mut request = ehttp::Request::post(self.url("/api/transport"), serde_json::to_vec(&t).unwrap_or_default());
        request.headers = ehttp::Headers::new(&[("Content-Type", "application/json")]);
        ehttp::fetch(request, |_| {});
    }

    fn fetch_preview(&self, version: u64) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let url = self.url(&format!("/api/release/preview.png?v={version}"));
        ehttp::fetch(ehttp::Request::get(url), move |res| {
            let image = match res {
                Ok(r) if r.ok => image::load_from_memory_with_format(&r.bytes, image::ImageFormat::Png)
                    .map(|img| {
                        let rgba = img.into_rgba8();
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        )
                    })
                    .map_err(|e| e.to_string()),
                Ok(r) => Err(format!("{} {}", r.status, r.status_text)),
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Preview { version, image });
        });
    }

    fn send(&mut self, cmd: &Command) {
        if matches!(cmd, Command::InstallPacks { .. }) {
            self.sounds_after_install = Some(false);
        }
        let what = command_name(cmd);
        let body = serde_json::to_vec(cmd).unwrap_or_default();
        let mut request = ehttp::Request::post(self.url("/api/command"), body);
        request.headers = ehttp::Headers::new(&[("Accept", "application/json"), ("Content-Type", "application/json")]);
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        ehttp::fetch(request, move |res| {
            let result = match res {
                Ok(r) => match serde_json::from_slice::<Reply>(&r.bytes) {
                    Ok(reply) => Ok(reply),
                    Err(_) if !r.ok => {
                        let text = String::from_utf8_lossy(&r.bytes);
                        let text: String = text.trim().chars().take(300).collect();
                        Err(format!(
                            "{} {}{}",
                            r.status,
                            r.status_text,
                            if text.is_empty() { String::new() } else { format!(": {text}") }
                        ))
                    }
                    Err(e) => Err(format!("unexpected answer: {e}")),
                },
                Err(e) => Err(e),
            };
            Self::deliver(&tx, &ctx, Event::Reply { what, result });
        });
    }

    fn update(&mut self, now: f64) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            match &ev {
                Event::State(s) => {
                    self.polling = false;
                    self.since = Some(s.version);
                    self.failures = 0;
                    self.retry_at = None;
                    if !self.sounds_requested {
                        self.fetch_sounds();
                        self.fetch_hello();
                    } else if self.ears_loading && !s.ears_loading {
                        self.fetch_hello();
                    }
                    self.ears_loading = s.ears_loading;
                    match (self.sounds_after_install, s.busy.is_some()) {
                        (Some(false), true) => self.sounds_after_install = Some(true),
                        (Some(true), false) => {
                            self.sounds_after_install = None;
                            self.fetch_sounds();
                        }
                        _ => {}
                    }
                }
                Event::PollFailed(_) => {
                    self.polling = false;
                    self.failures += 1;
                    self.since = None;
                    // a restarted server may have new sounds and settings
                    self.sounds_requested = false;
                    self.retry_at = Some(now + [1.0, 2.0, 3.0, 5.0][(self.failures as usize - 1).min(3)]);
                }
                _ => {}
            }
            out.push(ev);
        }
        if !self.polling && self.retry_at.is_none_or(|t| now >= t) {
            self.retry_at = None;
            self.poll_state();
        }
        out
    }
}

pub enum Backend {
    Remote(Remote),
    Mock(Box<Mock>),
}

impl Backend {
    pub fn new(connection: &Connection, ctx: &egui::Context) -> Backend {
        match connection {
            Connection::Server(url) => Backend::Remote(Remote::new(url.clone(), ctx)),
            Connection::Mock => Backend::Mock(Box::new(Mock::new(Some(ctx.clone())))),
        }
    }

    pub fn is_mock(&self) -> bool {
        matches!(self, Backend::Mock(_))
    }

    pub fn label(&self) -> String {
        match self {
            Backend::Remote(r) => r.base.clone(),
            Backend::Mock(_) => "the built-in demo".into(),
        }
    }

    pub fn send(&mut self, cmd: &Command) {
        match self {
            Backend::Remote(r) => r.send(cmd),
            Backend::Mock(m) => m.send(cmd.clone()),
        }
    }

    /// where the playhead is, in seconds: the demo's engine previews an edit there
    pub fn playhead(&mut self, secs: f64) {
        if let Backend::Mock(m) = self {
            m.playhead(secs);
        }
    }

    /// new events; keeps the long-poll running (or steps the mock's jobs)
    pub fn update(&mut self, now: f64) -> Vec<Event> {
        match self {
            Backend::Remote(r) => r.update(now),
            Backend::Mock(m) => m.update(now),
        }
    }

    /// open a project the demo saved, the zip itself, named `name`: claudius unpacks it into a song folder of its own
    /// (the answer arrives as a reply to "open")
    pub fn open_project(&mut self, name: &str, zip: Vec<u8>) {
        match self {
            Backend::Remote(r) => r.upload_project(name, zip),
            Backend::Mock(m) => m.refuse("open", "the demo opens no projects: open it in claudius, which keeps it"),
        }
    }

    /// send the release's artwork, the file itself (the answer arrives as a reply to "artwork")
    pub fn upload_artwork(&mut self, bytes: Vec<u8>) {
        match self {
            Backend::Remote(r) => r.upload_artwork(bytes),
            Backend::Mock(m) => m.refuse("artwork", "the demo keeps no artwork: start `claudius serve` to publish"),
        }
    }

    /// send a take, the WAV file itself, to play from the start of section `from`, for take request `request` if
    /// one asked for it (the answer arrives as a reply to "take")
    pub fn upload_take(&mut self, wav: Vec<u8>, from: &str, name: Option<&str>, role: &str, request: Option<u64>) {
        let mut query = format!("from={}&role={}", query_value(from), query_value(role));
        if let Some(name) = name {
            query.push_str(&format!("&name={}", query_value(name)));
        }
        if let Some(id) = request {
            query.push_str(&format!("&request={id}"));
        }
        match self {
            Backend::Remote(r) => r.upload_take(wav, &query),
            Backend::Mock(m) => m.refuse("take", "the demo keeps no takes: start `claudius serve` to record"),
        }
    }

    /// Sends the answers to pitch check `id`, sung from its first beat (the answer arrives as a reply to "pitch_check").
    pub fn upload_pitch_check(&mut self, wav: Vec<u8>, id: u64) {
        match self {
            Backend::Remote(r) => r.listen(wav, "/api/pitch-check", &format!("id={id}"), "pitch_check"),
            Backend::Mock(m) => {
                m.refuse("pitch_check", "the demo hears no singing: start `claudius serve` for a pitch check")
            }
        }
    }

    /// Sends a recording chosen on this computer, the file itself, to be listened to, remade or sampled (the answer
    /// arrives as a reply to "analyze", "remake" or "sample_song").
    pub fn listen(&mut self, l: &crate::panels::Listen) {
        let (what, path, query) = listen_request(l);
        match self {
            Backend::Remote(r) => r.listen(l.file.bytes.clone(), path, &query, what),
            Backend::Mock(m) if l.groove => {
                m.send(Command::ExtractGroove { path: l.file.name.clone(), name: None, lanes: Vec::new(), bpm: None })
            }
            Backend::Mock(m) if l.sample => m.send(Command::SampleSong {
                path: l.file.name.clone(),
                count: None,
                bars: None,
                find: None,
                sections: Vec::new(),
            }),
            // the demo listens to nothing: it makes up an analysis from the file's name, as for a path
            Backend::Mock(m) => match &l.remake {
                Some((genre, seed)) => m.send(Command::Remake {
                    path: l.file.name.clone(),
                    genre: Some(genre.clone()),
                    seed: *seed,
                    bpm: None,
                }),
                None => m.send(Command::Analyze { path: l.file.name.clone() }),
            },
        }
    }

    /// ask for the artwork as a picture to show (arrives as `Event::Preview`)
    /// the page a phone plays along on, served or stopped
    pub fn phone(&mut self, on: bool) {
        match self {
            Backend::Remote(r) => r.phone(on),
            Backend::Mock(m) => m.phone(on),
        }
    }

    /// where the song is, while a phone plays along
    pub fn transport(&mut self, t: Transport) {
        match self {
            Backend::Remote(r) => r.transport(t),
            Backend::Mock(m) => m.transport(t),
        }
    }

    pub fn fetch_preview(&mut self, version: u64) {
        if let Backend::Remote(r) = self {
            r.fetch_preview(version);
        }
    }

    /// ask for the script reference (arrives as `Event::Reference`)
    pub fn fetch_reference(&mut self) {
        match self {
            Backend::Remote(r) => r.fetch_reference(),
            Backend::Mock(m) => m.reference(),
        }
    }

    /// poll at once instead of waiting for the next retry
    pub fn retry(&mut self) {
        if let Backend::Remote(r) = self
            && !r.polling
        {
            r.retry_at = None;
        }
    }

    /// seconds until the next automatic retry while the server is unreachable
    pub fn retry_in(&self, now: f64) -> Option<f64> {
        match self {
            Backend::Remote(r) => r.retry_at.map(|t| (t - now).max(0.0)),
            Backend::Mock(_) => None,
        }
    }

    /// how soon the backend wants another frame without any input (mock jobs, retries)
    pub fn next_wake(&self, now: f64) -> Option<f64> {
        match self {
            Backend::Remote(r) => r.retry_at.map(|t| (t - now).max(0.05)),
            Backend::Mock(m) => m.busy().then_some(0.03),
        }
    }

    /// what the player should load for a render
    pub fn audio(&self, render: &RenderView) -> Option<Source> {
        match self {
            Backend::Remote(r) => {
                #[cfg(target_arch = "wasm32")]
                let url = render.url.clone();
                #[cfg(not(target_arch = "wasm32"))]
                let url = match render.url.strip_suffix(".mp3") {
                    Some(stem) => format!("{stem}.wav"),
                    None => render.url.clone(),
                };
                Some(Source::Url(r.url(&url)))
            }
            Backend::Mock(m) => m.audio(render.version).map(Source::Pcm),
        }
    }

    /// What downloading the song as rendered in `render` hands over, named `name` and its extension, in `format` (a
    /// spec of `exports`) when one is asked for: a server's file, else its MP3, else its WAV, else the audio the player
    /// plays. The demo makes the file in its engine, which hands it over later as `Event::Delivered`: None.
    pub fn download(
        &mut self,
        render: &RenderView,
        exports: &[ExportFormat],
        name: &str,
        format: Option<&str>,
    ) -> Result<Option<Delivery>, String> {
        let asked = format.and_then(|s| exports.iter().find(|f| f.spec == s && f.unavailable.is_none()));
        match self {
            Backend::Mock(m) => m.download(render.version, name, format).map(|()| None),
            Backend::Remote(_) if format == Some(crate::mock::PROJECT) => {
                Err("claudius keeps the song in its folder, with the samples it plays: share that folder".into())
            }
            Backend::Remote(r) => Ok(Some(match asked.or_else(|| download_format(exports)) {
                Some(f) => {
                    let file = format!("{name}.{}", f.extension);
                    let url = r.url(&format!("/api/export/{}/{file}?format={}", render.version, f.spec));
                    Delivery::Url { name: file, url }
                }
                None => {
                    let extension = render.url.rsplit_once('.').map_or("mp3", |(_, e)| e);
                    Delivery::Url { name: format!("{name}.{extension}"), url: r.url(&render.url) }
                }
            })),
        }
    }
}

/// the format a download is in: MP3, which plays anywhere, else WAV, of those this machine can write
fn download_format(exports: &[ExportFormat]) -> Option<&ExportFormat> {
    ["mp3", "wav"].iter().find_map(|ext| exports.iter().find(|f| f.extension == *ext && f.unavailable.is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::Listen;
    use crate::pick::Picked;

    /// the file the demo's engine makes of `render` for a download, in `format`, once it hands it over
    fn delivered(
        backend: &mut Backend,
        render: &RenderView,
        exports: &[ExportFormat],
        format: Option<&str>,
    ) -> Delivery {
        let made = backend.download(render, exports, "glass-garden", format);
        assert!(matches!(made, Ok(None)), "the demo's engine makes the file");
        let (mut now, t0) = (1000.0, std::time::Instant::now());
        while t0.elapsed() < Duration::from_secs(60) {
            now += 0.05;
            for ev in backend.update(now) {
                if let Event::Delivered(file) = ev {
                    return file.expect("a file");
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the demo's engine made no file")
    }

    #[test]
    fn the_demo_hands_over_its_render_as_a_flac_named_after_the_song_or_in_the_format_asked_for() {
        let mut backend = Backend::Mock(Box::new(Mock::new(None)));
        let (mut now, mut render, mut exports, t0) = (0.0, None, Vec::new(), std::time::Instant::now());
        // the demo's engine renders on a thread of its own
        while render.is_none() && t0.elapsed() < Duration::from_secs(180) {
            now += 0.05;
            for ev in backend.update(now) {
                if let Event::State(s) = ev {
                    exports = s.exports.clone();
                    render = render.or(s.render.filter(|r| r.url != claudius_api::PREVIEW_URL));
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let render = render.expect("the demo renders its song");
        assert!(exports.iter().any(|f| f.spec == crate::mock::DOWNLOAD && f.unavailable.is_none()), "{exports:?}");
        let Delivery::Bytes { name, mime, bytes } = delivered(&mut backend, &render, &exports, None) else {
            panic!("the demo hands over bytes")
        };
        assert_eq!((name.as_str(), mime.as_str(), &bytes[..4]), ("glass-garden.flac", "audio/flac", &b"fLaC"[..]));
        let Backend::Mock(m) = &backend else { unreachable!() };
        let wav = m.audio(render.version).unwrap().samples.len() * 2 * 44_100 / 48_000;
        assert!(bytes.len() < wav * 85 / 100, "{} bytes, where a 16-bit WAV takes {wav}", bytes.len());
        let Delivery::Bytes { name, bytes, .. } = delivered(&mut backend, &render, &exports, Some("mp3:192")) else {
            panic!("the demo hands over bytes")
        };
        assert_eq!(name, "glass-garden.mp3");
        assert!(bytes.starts_with(b"ID3") || (bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0), "an MP3");
        let project = delivered(&mut backend, &render, &exports, Some(crate::mock::PROJECT));
        let Delivery::Bytes { name, mime, bytes } = project else { panic!("the demo hands over bytes") };
        assert_eq!((name.as_str(), mime.as_str()), ("glass-garden.zip", "application/zip"));
        let songs = std::env::temp_dir().join(format!("claudius-ui-project-{}", std::process::id()));
        let dir = claudius::song::unpack_project(&bytes, &name, &songs).expect("a project claudius opens");
        assert!(claudius::song::read_blueprint(&dir.join("blueprint.json")).is_ok() && dir.join("song.rhai").is_file());
        let _ = std::fs::remove_dir_all(&songs);
        let later = RenderView { version: render.version + 1, ..render };
        assert!(backend.download(&later, &exports, "glass-garden", None).is_err(), "a render the demo does not hold");
    }

    #[test]
    fn a_server_hands_over_its_mp3_else_its_wav_else_what_the_player_plays() {
        let mut backend = Backend::Remote(Remote::new("http://127.0.0.1:7878".into(), &egui::Context::default()));
        let render = RenderView { version: 7, url: "/api/audio/7.mp3".into(), ..Default::default() };
        let format = |spec: &str, extension: &str, unavailable: Option<&str>| ExportFormat {
            spec: spec.into(),
            label: spec.into(),
            extension: extension.into(),
            lossless: extension == "wav",
            unavailable: unavailable.map(Into::into),
        };
        let all = [format("wav:24", "wav", None), format("mp3", "mp3", None), format("opus", "opus", None)];
        let url = |name: &str, url: &str| Delivery::Url { name: name.into(), url: url.into() };
        assert_eq!(
            backend.download(&render, &all, "glass-garden", None),
            Ok(Some(url("glass-garden.mp3", "http://127.0.0.1:7878/api/export/7/glass-garden.mp3?format=mp3")))
        );
        assert_eq!(
            backend.download(&render, &all, "glass-garden", Some("opus")),
            Ok(Some(url("glass-garden.opus", "http://127.0.0.1:7878/api/export/7/glass-garden.opus?format=opus"))),
            "the format asked for"
        );
        let no_mp3 = [format("mp3", "mp3", Some("no encoder")), format("wav:24", "wav", None)];
        assert_eq!(
            backend.download(&render, &no_mp3, "glass-garden", Some("mp3")),
            Ok(Some(url("glass-garden.wav", "http://127.0.0.1:7878/api/export/7/glass-garden.wav?format=wav:24"))),
            "one it cannot write: the WAV"
        );
        assert_eq!(
            backend.download(&render, &[], "glass-garden", None),
            Ok(Some(url("glass-garden.mp3", "http://127.0.0.1:7878/api/audio/7.mp3")))
        );
        let project = backend.download(&render, &all, "glass-garden", Some(crate::mock::PROJECT));
        assert!(
            project.is_err_and(|e| e.contains("keeps the song in its folder")),
            "a song in claudius has its folder"
        );
    }

    #[test]
    fn a_recording_to_listen_to_is_named_in_the_query() {
        let file =
            std::sync::Arc::new(Picked { name: "My Song (live) #2.m4a".into(), bytes: vec![1, 2], folder: None });
        let analyze = listen_request(&Listen { file: file.clone(), remake: None, sample: false, groove: false });
        assert_eq!(analyze, ("analyze", "/api/analyze", "name=My%20Song%20%28live%29%20%232.m4a".into()));
        let remake = listen_request(&Listen {
            file: file.clone(),
            remake: Some(("drum_and_bass".into(), Some(42))),
            sample: false,
            groove: false,
        });
        assert_eq!(remake.0, "remake");
        assert_eq!(remake.1, "/api/remake");
        assert!(remake.2.ends_with("&genre=drum_and_bass&seed=42"), "{}", remake.2);
        let unseeded = listen_request(&Listen {
            file: file.clone(),
            remake: Some(("house".into(), None)),
            sample: false,
            groove: false,
        });
        assert!(unseeded.2.ends_with("&genre=house"), "a random seed is claudius' to pick");
        let groove = listen_request(&Listen { file: file.clone(), remake: None, sample: false, groove: true });
        assert_eq!(groove, ("extract_groove", "/api/groove", "name=My%20Song%20%28live%29%20%232.m4a".into()));
        let sample = listen_request(&Listen { file, remake: None, sample: true, groove: false });
        assert_eq!(sample, ("sample_song", "/api/sample", "name=My%20Song%20%28live%29%20%232.m4a".into()));
    }
}
