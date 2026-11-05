//! Playing from a phone. The studio shows a QR code of a page with pads, keys and a turntable, which a phone on the
//! same network opens. The page plays the song's own sounds itself, so a tap sounds at once whatever the Wi-Fi does; it
//! keeps its clock with the server's and follows the studio's transport (`POST /api/transport`), so what is played
//! lands in the song where it was heard; and it sends a take back as the command the studio's own recording would
//! (`add_hits`, `add_notes`, `add_scratch`).
//!
//! The page has a port of its own, open on the networks the computer is on, which answers only the page and its
//! routes, and only to the key the QR code carries (the phone keeps it as a cookie). `POST /api/phone` starts it with a
//! new key, and `DELETE /api/phone` stops it.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use claudius::audio::{self, Stereo};
use claudius::{pads, scratch};
use claudius_api::{Command, PhoneView};
use serde::{Deserialize, Serialize};

use crate::app::{App, Running, Take};

const PAGE: &str = include_str!("phone/index.html");
const SCRIPT: &str = include_str!("phone/phone.js");
const STYLE: &str = include_str!("phone/phone.css");

/// the port the page is served on when it is free (another when it is not)
pub const PORT: u16 = claudius_api::DEFAULT_PORT + 1;

/// the page's server: the session it plays into, the key that opens it, and the sounds it has rendered
struct Phone {
    app: Weak<App>,
    key: String,
    started: Instant,
    sounds: tokio::sync::Mutex<Sounds>,
}

/// what the page has been sent of the song, rendered once for the song's version
#[derive(Default)]
struct Sounds {
    version: u64,
    hits: HashMap<String, Arc<Vec<u8>>>,
    keys: HashMap<String, Arc<Vec<u8>>>,
    records: HashMap<String, Arc<Vec<u8>>>,
}

/// Serves the page for a phone, with a key of its own (or answers with the page being served).
pub async fn start(app: &Arc<App>) -> Result<PhoneView, String> {
    if let Some(v) = app.phone_view() {
        return Ok(v);
    }
    let ip = lan_ip().ok_or("this computer is on no network a phone could reach it on: join a Wi-Fi network")?;
    serve(app, Ipv4Addr::UNSPECIFIED, ip).await
}

/// the page served on `bind`, at `ip` in its address
async fn serve(app: &Arc<App>, bind: Ipv4Addr, ip: IpAddr) -> Result<PhoneView, String> {
    let listener = match tokio::net::TcpListener::bind((bind, PORT)).await {
        Ok(l) => l,
        Err(_) => {
            tokio::net::TcpListener::bind((bind, 0)).await.map_err(|e| format!("cannot listen for a phone: {e}"))?
        }
    };
    let port = listener.local_addr().map_err(|e| format!("cannot listen for a phone: {e}"))?.port();
    let key = new_key()?;
    let phone = Arc::new(Phone {
        app: Arc::downgrade(app),
        key: key.clone(),
        started: Instant::now(),
        sounds: tokio::sync::Mutex::default(),
    });
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let service = router(phone).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        let stopped = async {
            let _ = stopped.await;
        };
        if let Err(e) = axum::serve(listener, service).with_graceful_shutdown(stopped).await {
            eprintln!("claudius stopped serving the phone's page: {e}");
        }
    });
    let url = format!("http://{}/?key={key}", SocketAddr::new(ip, port));
    app.set_phone(Some(Running { url, _stop: stop, seen: HashMap::new() }));
    app.phone_view().ok_or_else(|| "the phone's page stopped as it started".into())
}

/// the address of this computer on the network its traffic goes out on (no packet is sent to find it)
fn lan_ip() -> Option<IpAddr> {
    let s = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // TEST-NET-1: an address nobody has, so the system only picks the way out to it
    s.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    Some(s.local_addr().ok()?.ip()).filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
}

fn new_key() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| format!("the system gave no randomness for the phone's key: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn router(p: Arc<Phone>) -> Router {
    Router::new()
        .route("/", get(page))
        .route("/phone.js", get(|| async { asset(SCRIPT, "text/javascript; charset=utf-8") }))
        .route("/phone.css", get(|| async { asset(STYLE, "text/css; charset=utf-8") }))
        .route("/api/song", get(song))
        .route("/api/clock", get(clock))
        .route("/api/hits/{lane}", get(hit))
        .route("/api/keys", get(keys))
        .route("/api/record", get(record))
        .route("/api/take", post(take).layer(DefaultBodyLimit::max(4 << 20)))
        .layer(middleware::from_fn_with_state(p.clone(), guard))
        .layer(middleware::from_fn(headers))
        .with_state(p)
}

/// only the key opens the page: from the QR code's address the first time, which leaves it as a cookie
async fn guard(State(p): State<Arc<Phone>>, req: Request, next: Next) -> Response {
    let query = req.uri().query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("key=")));
    let cookie = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|c| c.to_str().ok())
        .flat_map(|c| c.split(';'))
        .find_map(|kv| kv.trim().strip_prefix("claudius_phone="));
    let opens = |x: Option<&str>| x.is_some_and(|x| crate::http::same_secret(x, &p.key));
    if opens(cookie) {
        return next.run(req).await;
    }
    if opens(query) {
        let mut res = next.run(req).await;
        if let Ok(v) = HeaderValue::from_str(&format!("claudius_phone={}; Path=/; HttpOnly; SameSite=Strict", p.key)) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
        return res;
    }
    let says = "This page is closed: open Phone in the claudius studio and scan its code again.";
    (StatusCode::FORBIDDEN, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], says).into_response()
}

/// what every answer carries: the page runs only its own script, and its address (with the key) goes nowhere
async fn headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let csp = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; media-src 'self' blob:; \
               connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(csp));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

fn asset(body: &'static str, kind: &'static str) -> Response {
    ([(header::CONTENT_TYPE, kind)], body).into_response()
}

async fn page() -> Response {
    asset(PAGE, "text/html; charset=utf-8")
}

/// what the page hears once the session it played into has ended
fn gone() -> Response {
    (StatusCode::GONE, "the claudius session has ended").into_response()
}

impl Phone {
    /// the milliseconds from when the page started being served to `t` (before it, below zero)
    fn ms(&self, t: Instant) -> f64 {
        match t.checked_duration_since(self.started) {
            Some(d) => d.as_secs_f64() * 1e3,
            None => -(self.started.duration_since(t).as_secs_f64() * 1e3),
        }
    }

    /// a sound of the song, rendered (by `make`) once for its version
    async fn sound(
        &self,
        shelf: fn(&mut Sounds) -> &mut HashMap<String, Arc<Vec<u8>>>,
        name: String,
        make: impl FnOnce(claudius::model::Blueprint) -> Result<Vec<(String, Stereo)>, String> + Send + 'static,
    ) -> Response {
        let Some(app) = self.app.upgrade() else { return gone() };
        let Some(bp) = app.blueprint() else { return (StatusCode::NOT_FOUND, "no song yet").into_response() };
        let version = app.state().song_version;
        let mut s = self.sounds.lock().await;
        if s.version != version {
            *s = Sounds { version, ..Sounds::default() };
        }
        if !shelf(&mut s).contains_key(&name) {
            let made = tokio::task::spawn_blocking(move || make(bp)).await;
            match made {
                Ok(Ok(sounds)) => {
                    for (n, x) in sounds {
                        shelf(&mut s).insert(n, Arc::new(audio::wav_bytes(&x)));
                    }
                }
                Ok(Err(e)) => return (StatusCode::BAD_REQUEST, e).into_response(),
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("cannot render it: {e}")).into_response(),
            }
        }
        match shelf(&mut s).get(&name) {
            Some(b) => ([(header::CONTENT_TYPE, "audio/wav")], b.to_vec()).into_response(),
            None => (StatusCode::NOT_FOUND, format!("the song has no '{name}'")).into_response(),
        }
    }
}

/// what the page shows of the song, and what it plays
#[derive(Serialize)]
struct Song {
    /// the song's version: a newer one has new sounds
    version: u64,
    title: String,
    bpm: f64,
    key: String,
    sections: Vec<Section>,
    /// the drum lanes, a pad each, and their roles (kick, snare, hat ...), which a drum pad's MIDI note plays
    lanes: Vec<String>,
    roles: Vec<String>,
    /// the tracks a record can be cut from
    tracks: Vec<String>,
    /// what the keys can play: the sounds of the song's parts (with the part's name), the synth presets, and the
    /// installed instruments
    parts: Vec<(String, String)>,
    synths: Vec<String>,
    instruments: Vec<String>,
    /// the seconds between notes in `/api/keys`
    slot: f64,
}

#[derive(Serialize)]
struct Section {
    name: String,
    start_bar: u32,
    bars: u32,
    /// the tracks that play in it
    layers: Vec<String>,
}

#[derive(Deserialize)]
struct After {
    /// wait (up to 20 s) for a song newer than this version
    after: Option<u64>,
}

async fn song(State(p): State<Arc<Phone>>, Query(q): Query<After>) -> Response {
    let Some(app) = p.app.upgrade() else { return gone() };
    let mut st = app.state();
    if let Some(after) = q.after {
        let deadline = Instant::now() + Duration::from_secs(20);
        while st.song_version <= after && Instant::now() < deadline {
            st = app.state_after(Some(st.version), deadline.saturating_duration_since(Instant::now())).await;
        }
    }
    let sounds = app.sounds();
    let Some(s) = st.song else {
        return (StatusCode::NOT_FOUND, "no song yet: start one in the studio").into_response();
    };
    Json(Song {
        version: st.song_version,
        title: s.title,
        bpm: s.bpm,
        key: s.key,
        sections: s
            .sections
            .into_iter()
            .map(|x| Section { name: x.name, start_bar: x.start_bar, bars: x.bars, layers: x.layers })
            .collect(),
        roles: s
            .drums
            .iter()
            .map(|d| s.tracks.iter().find(|t| t.name == d.lane).map_or_else(|| d.lane.clone(), |t| t.role.clone()))
            .collect(),
        lanes: s.drums.into_iter().map(|d| d.lane).collect(),
        tracks: s.tracks.into_iter().map(|t| t.name).collect(),
        parts: s.parts.into_iter().map(|p| (p.name, p.preset)).filter(|(_, s)| !s.is_empty()).collect(),
        synths: sounds.synths,
        instruments: sounds.instruments,
        slot: pads::SLOT_SECS,
    })
    .into_response()
}

/// the server's time and the studio's transport, for the page to keep its clock and follow the song
#[derive(Serialize)]
struct Clock {
    /// now, in milliseconds on the page's clock
    now: f64,
    playing: bool,
    /// where the song was, in seconds, at `since` (milliseconds on the page's clock)
    at: f64,
    since: f64,
    version: u64,
}

async fn clock(State(p): State<Arc<Phone>>, ConnectInfo(from): ConnectInfo<SocketAddr>) -> Response {
    let Some(app) = p.app.upgrade() else { return gone() };
    app.phone_seen(from.ip());
    let (t, when) = app.transport();
    let version = app.state().song_version;
    Json(Clock { now: p.ms(Instant::now()), playing: t.playing, at: t.at, since: p.ms(when), version }).into_response()
}

async fn hit(State(p): State<Arc<Phone>>, Path(lane): Path<String>) -> Response {
    p.sound(|s| &mut s.hits, lane, |bp| pads::hits(&bp)).await
}

#[derive(Deserialize)]
struct Keys {
    sound: String,
    low: i32,
    high: i32,
    #[serde(default = "three")]
    step: usize,
}

fn three() -> usize {
    3
}

/// notes of a sound from `low` to `high` every `step` semitones, in one WAV with a note every `slot` seconds
async fn keys(State(p): State<Arc<Phone>>, Query(k): Query<Keys>) -> Response {
    let name = format!("{}:{}:{}:{}", k.sound, k.low, k.high, k.step);
    let key = name.clone();
    p.sound(|s| &mut s.keys, name, move |bp| Ok(vec![(key, pads::keys(&bp, &k.sound, k.low, k.high, k.step)?)])).await
}

#[derive(Deserialize)]
struct Record {
    track: String,
    #[serde(default)]
    bar: u32,
    #[serde(default = "one")]
    bars: u32,
}

fn one() -> u32 {
    1
}

/// the record a scratch moves: bars of one track of the song
async fn record(State(p): State<Arc<Phone>>, Query(r): Query<Record>) -> Response {
    let name = format!("{}:{}:{}", r.track, r.bar, r.bars);
    p.sound(
        |s| &mut s.records,
        name.clone(),
        move |bp| Ok(vec![(name, scratch::record(&bp, &r.track, r.bar, r.bars)?)]),
    )
    .await
}

/// a take played on the phone, sent as the command it makes, and made as no more than the edit it is
async fn take(State(p): State<Arc<Phone>>, Json(c): Json<Command>) -> Response {
    let t = match c {
        Command::AddHits { from, hits, bars, variant, grid, start, beats } => Take::Hits {
            from,
            struck: hits.into_iter().map(|h| claudius::take::Struck { lane: h.lane, t: h.t, v: h.v }).collect(),
            pads: claudius::take::Pads { bars, variant, grid, start, beats },
        },
        Command::AddNotes { from, notes, sounds, name, role, style, request: _ } => Take::Notes {
            from,
            notes: notes
                .iter()
                .map(|n| claudius::theory::Note { t: n.t, d: n.d, p: n.p, v: n.v, lock: None })
                .collect(),
            played: claudius::take::Played { sounds, name, role, style },
        },
        Command::AddScratch { from, track, bar, bars, moves, name, role } => {
            Take::Scratch { from, scratch: claudius_session::edit::scratch(track, bar, bars, &moves), name, role }
        }
        _ => return (StatusCode::FORBIDDEN, "a phone sends takes: add_hits, add_notes or add_scratch").into_response(),
    };
    let Some(app) = p.app.upgrade() else { return gone() };
    match tokio::task::spawn_blocking(move || app.take(t)).await {
        Ok(reply) => Json(reply).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("the take was lost: {e}")).into_response(),
    }
}

#[cfg(test)]
pub(crate) fn test_router(app: &Arc<App>, key: &str) -> Router {
    router(Arc::new(Phone {
        app: Arc::downgrade(app),
        key: key.into(),
        started: Instant::now(),
        sounds: tokio::sync::Mutex::default(),
    }))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::app::Settings;

    fn app() -> Arc<App> {
        let s = Settings {
            library: false,
            ears: false,
            auto_render: false,
            file_roots: None,
            voice: None,
            samples: false,
            gather: false,
            songs: None,
        };
        App::new(s)
    }

    async fn ask(r: &Router, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut req = req;
        req.extensions_mut().insert(ConnectInfo(SocketAddr::from(([192, 168, 1, 23], 50000))));
        let res = r.clone().oneshot(req).await.unwrap();
        let (parts, body) = res.into_parts();
        (parts.status, parts.headers, axum::body::to_bytes(body, usize::MAX).await.unwrap().to_vec())
    }

    fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut b = Request::get(uri);
        if let Some(c) = cookie {
            b = b.header(header::COOKIE, c);
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn only_the_key_opens_the_page() {
        let app = app();
        let r = test_router(&app, "k3y");
        assert_eq!(ask(&r, get("/", None)).await.0, StatusCode::FORBIDDEN);
        assert_eq!(ask(&r, get("/?key=nope", None)).await.0, StatusCode::FORBIDDEN);
        let (status, headers, body) = ask(&r, get("/?key=k3y", None)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(String::from_utf8(body).unwrap().contains("phone.js"));
        let cookie = headers[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.starts_with("claudius_phone=k3y;") && cookie.contains("HttpOnly"), "{cookie}");
        assert_eq!(headers[header::REFERRER_POLICY], "no-referrer", "the key stays in the address bar");
        assert!(headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("script-src 'self'"));
        assert_eq!(ask(&r, get("/phone.js", Some("claudius_phone=k3y"))).await.0, StatusCode::OK);
        assert_eq!(ask(&r, get("/api/clock", Some("claudius_phone=other"))).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn the_page_gets_the_song_its_sounds_and_the_studios_transport() {
        let app = app();
        let r = test_router(&app, "k");
        let c = Some("claudius_phone=k");
        assert_eq!(ask(&r, get("/api/song", c)).await.0, StatusCode::NOT_FOUND, "no song yet");
        let song = Command::NewSong {
            genre: "house".into(),
            seed: Some(3),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
            complexity: None,
        };
        assert!(app.command(song).ok);
        let (status, _, body) = ask(&r, get("/api/song", c)).await;
        assert_eq!(status, StatusCode::OK);
        let s: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let lanes: Vec<String> = serde_json::from_value(s["lanes"].clone()).unwrap();
        assert!(lanes.iter().any(|l| l == "kick"), "{lanes:?}");
        let roles: Vec<String> = serde_json::from_value(s["roles"].clone()).unwrap();
        assert_eq!(roles.len(), lanes.len());
        assert_eq!(roles[lanes.iter().position(|l| l == "kick").unwrap()], "kick");
        assert!(s["synths"].as_array().unwrap().iter().any(|x| x == "epiano"));
        assert!(!s["parts"].as_array().unwrap().is_empty(), "the song's own sounds come first");
        let (status, headers, wav) = ask(&r, get("/api/hits/kick", c)).await;
        assert_eq!((status, &wav[..4]), (StatusCode::OK, &b"RIFF"[..]));
        assert_eq!(headers[header::CONTENT_TYPE], "audio/wav");
        assert_eq!(ask(&r, get("/api/hits/nothing", c)).await.0, StatusCode::NOT_FOUND);
        let (status, _, wav) = ask(&r, get("/api/keys?sound=epiano&low=59&high=65&step=3", c)).await;
        let x = audio::load_bytes(wav, "wav").unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!((x.secs() - 3.0 * pads::SLOT_SECS).abs() < 0.01, "three notes, a slot each: {} s", x.secs());
        for wild in
            ["low=-2147483648&high=2147483647", "low=60&high=59", "low=12&high=120&step=1", "low=60&high=72&step=0"]
        {
            let (status, ..) = ask(&r, get(&format!("/api/keys?sound=epiano&{wild}"), c)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{wild}");
        }
        let (status, _, wav) = ask(&r, get("/api/record?track=kick&bar=0&bars=1", c)).await;
        assert_eq!((status, &wav[..4]), (StatusCode::OK, &b"RIFF"[..]));
        // the clock: stopped, then where the studio says the song is
        let clock = |b: Vec<u8>| serde_json::from_slice::<serde_json::Value>(&b).unwrap();
        let t = clock(ask(&r, get("/api/clock", c)).await.2);
        assert_eq!(t["playing"], false);
        app.set_transport(claudius_api::Transport { playing: true, at: 12.5 });
        let t = clock(ask(&r, get("/api/clock", c)).await.2);
        assert_eq!((t["playing"].as_bool(), t["at"].as_f64()), (Some(true), Some(12.5)));
        assert!(t["since"].as_f64().unwrap() <= t["now"].as_f64().unwrap());
        assert_eq!(app.state().phone, None, "no page is served: the test asked the router itself");
    }

    #[tokio::test]
    async fn a_phone_sends_takes_and_nothing_else() {
        let app = app();
        let r = test_router(&app, "k");
        let song = Command::NewSong {
            genre: "house".into(),
            seed: Some(3),
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
            complexity: None,
        };
        assert!(app.command(song).ok);
        let from = app.state().song.unwrap().sections[0].name.clone();
        let post = |body: serde_json::Value| {
            Request::post("/api/take")
                .header(header::COOKIE, "claudius_phone=k")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };
        let (status, _, body) = ask(&r, post(serde_json::json!({"cmd": "set_tempo", "bpm": 90}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{}", String::from_utf8_lossy(&body));
        let hits = serde_json::json!({"cmd": "add_hits", "from": from, "hits": [{"lane": "kick", "t": 0}], "bars": 1});
        let (status, _, body) = ask(&r, post(hits)).await;
        let reply: claudius_api::Reply = serde_json::from_slice(&body).unwrap();
        assert!(status == StatusCode::OK && reply.ok, "{}", reply.message);
    }

    #[tokio::test]
    async fn the_studio_starts_and_stops_the_page() {
        let app = app();
        let v = serve(&app, Ipv4Addr::LOCALHOST, Ipv4Addr::LOCALHOST.into()).await.unwrap();
        assert!(v.url.starts_with("http://127.0.0.1:") && v.url.contains("/?key="), "{}", v.url);
        assert_eq!(app.state().phone.map(|p| p.url), Some(v.url.clone()));
        assert_eq!(start(&app).await.unwrap().url, v.url, "the page being served");
        // the page answers on its port, and stops with the studio's word
        let addr = v.url.trim_start_matches("http://").split('/').next().unwrap().to_string();
        let page = tokio::task::spawn_blocking(move || ureq::get(&format!("http://{addr}/")).call().map(|_| ()));
        assert!(page.await.unwrap().is_err(), "without the key the page stays shut");
        app.set_phone(None);
        assert_eq!(app.state().phone, None);
    }
}
