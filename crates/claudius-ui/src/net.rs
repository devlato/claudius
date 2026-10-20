//! Talking to the claudius server: a long-poll loop on `/api/state?since=N`, commands as JSON POSTs to
//! `/api/command`, the sound list from `/api/sounds`. `ehttp` runs the requests (threads natively, `fetch` on the
//! web) and hands results back through a channel; every arrival requests a repaint. `Backend` hides whether the
//! other side is a real server or the built-in mock.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use claudius_api::{Command, RenderView, Reply, SoundList, State};
use eframe::egui;

use crate::audio::Source;
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
}

/// the `"cmd"` tag of a command ("set_tempo")
pub fn command_name(cmd: &Command) -> String {
    serde_json::to_value(cmd)
        .ok()
        .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_else(|| "command".into())
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
            Connection::Mock => Backend::Mock(Box::new(Mock::new())),
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

    /// new events; keeps the long-poll running (or steps the mock's jobs)
    pub fn update(&mut self, now: f64) -> Vec<Event> {
        match self {
            Backend::Remote(r) => r.update(now),
            Backend::Mock(m) => m.update(now),
        }
    }

    /// send the release's artwork, the file itself (the answer arrives as a reply to "artwork")
    pub fn upload_artwork(&mut self, bytes: Vec<u8>) {
        match self {
            Backend::Remote(r) => r.upload_artwork(bytes),
            Backend::Mock(m) => m.refuse("artwork", "the demo keeps no artwork: start `claudius serve` to publish"),
        }
    }

    /// ask for the artwork as a picture to show (arrives as `Event::Preview`)
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
}
