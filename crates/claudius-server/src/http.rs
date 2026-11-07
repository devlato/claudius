//! The HTTP side: the `claudius-api` routes, the MCP endpoint and the web UI.
//!
//! Local by default: requests must name a loopback host (no DNS rebinding), commands must be JSON (a cross-site form
//! post cannot send them, and no foreign origin passes CORS). With a token, every request must carry it as
//! `Authorization: Bearer`, `?token=` (which also sets a cookie, for the web UI) or that cookie.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate as _};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::app::App;

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// the built web UI (trunk's dist folder)
    pub web: Option<PathBuf>,
    /// accept any Host (behind a tunnel or on a LAN); pair it with a token
    pub public: bool,
    pub token: Option<String>,
}

impl Options {
    /// an error for options that would leave a shared server open: --public without a token, or an empty one
    pub fn check(&self) -> anyhow::Result<()> {
        match &self.token {
            Some(t) if t.trim().is_empty() => anyhow::bail!("the token is empty; give one, or none to have one made"),
            None if self.public => anyhow::bail!("a public claudius needs a token"),
            _ => Ok(()),
        }
    }
}

#[derive(Clone)]
struct Ctx {
    app: Arc<App>,
}

pub fn router(app: Arc<App>, o: &Options) -> Router {
    STARTED.get_or_init(std::time::Instant::now);
    let ctx = Ctx { app: app.clone() };
    let api = Router::new()
        .route("/api/hello", get(hello))
        .route("/api/metrics", get(metrics))
        .route("/api/state", get(state))
        .route("/api/command", post(command).layer(DefaultBodyLimit::max(1 << 20)))
        .route("/api/script", get(script))
        .route("/api/reference", get(reference))
        .route("/api/sounds", get(sounds))
        .route("/api/genres", get(genres))
        .route("/api/library/search", get(library_search))
        .route("/api/library/audio", get(library_audio))
        .route("/api/pieces/{file}", get(piece_audio))
        .route("/api/sample/pieces", post(sample_pieces))
        .route("/api/audio/{file}", get(audio))
        .route("/api/stems/{version}/{file}", get(stem))
        .route("/api/export/{version}/{file}", get(export_file))
        .route(
            "/api/release/artwork",
            get(artwork).post(set_artwork).layer(DefaultBodyLimit::max(crate::app::MAX_ARTWORK)),
        )
        .route("/api/release/preview.png", get(artwork_preview))
        .route("/api/takes", post(add_take).layer(DefaultBodyLimit::max(crate::app::MAX_TAKE)))
        .route("/api/project", post(open_project).layer(DefaultBodyLimit::max(crate::app::MAX_PROJECT)))
        .route("/api/pitch-check", post(pitch_check).layer(DefaultBodyLimit::max(crate::app::MAX_TAKE)))
        .route("/api/groove", post(groove).layer(DefaultBodyLimit::max(crate::app::MAX_TAKE)))
        .route("/api/analyze", post(analyze).layer(DefaultBodyLimit::max(crate::app::MAX_TAKE)))
        .route("/api/remake", post(remake).layer(DefaultBodyLimit::max(crate::app::MAX_TAKE)))
        .route("/api/sample", post(sample).layer(DefaultBodyLimit::max(crate::app::MAX_TAKE)))
        .route("/api/phone", post(phone_start).delete(phone_stop))
        .route("/api/transport", post(transport))
        .with_state(ctx);
    let mut r = api.nest_service("/mcp", crate::mcp::http_service(app, o.public));
    r = match &o.web {
        Some(dir) if dir.join("index.html").exists() => {
            r.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))))
        }
        _ => r.fallback(no_ui),
    };
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            let o = origin.to_str().unwrap_or("");
            ["http://localhost", "http://127.0.0.1", "http://[::1]"]
                .iter()
                .any(|p| o == *p || o.strip_prefix(p).is_some_and(|rest| rest.starts_with(':')))
        }))
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::HeaderName::from_static("mcp-session-id"),
            header::HeaderName::from_static("mcp-protocol-version"),
        ]);
    let guard = Guard { public: o.public, token: o.token.clone() };
    r.layer(middleware::from_fn(move |req: Request, next: Next| {
        let g = guard.clone();
        async move { g.check(req, next).await }
    }))
    .layer(middleware::from_fn(security_headers))
    .layer(middleware::from_fn(cache_headers))
    .layer(cors)
    // brotli or gzip for whoever asks: the web studio's WebAssembly is a quarter of its size compressed, which
    // counts over a tunnel. Audio is left as it is: MP3 is compressed already, and WAV is read in ranges
    .layer(CompressionLayer::new().compress_when(DefaultPredicate::new().and(NotForContentType::const_new("audio/"))))
}

/// how long a browser keeps the web studio's files: trunk names the script and the WebAssembly after a hash of their
/// contents, so those never change, and the page that names them is checked again each time
async fn cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let mut res = next.run(req).await;
    let web = !path.starts_with("/api/") && path != "/mcp" && !path.starts_with("/mcp/");
    if web && res.status().is_success() {
        let policy = if content_hashed(&path) { "public, max-age=31536000, immutable" } else { "no-cache" };
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    }
    res
}

/// whether a file's name carries trunk's hash of its contents: `claudius-ui-<hex>.js`, `claudius-ui-<hex>_bg.wasm`
fn content_hashed(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or_default();
    let Some((stem, extension)) = name.rsplit_once('.') else { return false };
    let stem = stem.strip_suffix("_bg").unwrap_or(stem);
    matches!(extension, "js" | "wasm" | "css")
        && stem
            .rsplit_once('-')
            .is_some_and(|(_, hash)| hash.len() >= 12 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// headers every answer carries: no MIME sniffing, no referrer leaks, no framing (clickjacking)
async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("frame-ancestors 'none'"));
    res
}

/// compares secrets in time that does not depend on where they differ
pub(crate) fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Clone)]
struct Guard {
    public: bool,
    token: Option<String>,
}

fn loopback(host: &str) -> bool {
    let h = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.split(':').next().unwrap_or("")
    };
    matches!(h, "localhost" | "127.0.0.1" | "::1")
}

impl Guard {
    async fn check(&self, req: Request, next: Next) -> Response {
        if !self.public {
            let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
            if !loopback(host) {
                return (
                    StatusCode::FORBIDDEN,
                    "claudius only answers on localhost (start it with --public and a --token to share it)",
                )
                    .into_response();
            }
        }
        let Some(token) = &self.token else { return next.run(req).await };
        let bearer = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::trim);
        let query = req.uri().query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("token=")));
        let cookie = req
            .headers()
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|c| c.to_str().ok())
            .flat_map(|c| c.split(';'))
            .find_map(|kv| kv.trim().strip_prefix("claudius_token="));
        // an empty token matches nothing, so that a server started with one stays shut rather than open
        let matches = |x: Option<&str>| x.is_some_and(|x| !token.is_empty() && same_secret(x, token));
        if matches(bearer) || matches(cookie) {
            return next.run(req).await;
        }
        if matches(query) {
            // Secure only when it came through https (a tunnel says so): over plain http on a LAN the browser would
            // drop the cookie
            let https =
                req.headers().get("x-forwarded-proto").is_some_and(|p| p.as_bytes().eq_ignore_ascii_case(b"https"));
            let mut res = next.run(req).await;
            let cookie = format!(
                "claudius_token={token}; Path=/; HttpOnly; SameSite=Strict{}",
                if https { "; Secure" } else { "" }
            );
            if let Ok(v) = HeaderValue::from_str(&cookie) {
                res.headers_mut().append(header::SET_COOKIE, v);
            }
            return res;
        }
        (StatusCode::UNAUTHORIZED, "this claudius needs its token (Authorization: Bearer <token>, or ?token=<token>)")
            .into_response()
    }
}

/// the page a phone plays along on, served from now on (or the one served already)
async fn phone_start(State(c): State<Ctx>) -> Response {
    match crate::phone::start(&c.app).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, e).into_response(),
    }
}

async fn phone_stop(State(c): State<Ctx>) -> StatusCode {
    c.app.set_phone(None);
    StatusCode::NO_CONTENT
}

async fn transport(State(c): State<Ctx>, Json(t): Json<claudius_api::Transport>) -> StatusCode {
    c.app.set_transport(t);
    StatusCode::NO_CONTENT
}

async fn hello(State(c): State<Ctx>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({ "name": "claudius", "version": env!("CARGO_PKG_VERSION"), "state": c.app.version(), "ears": c.app.has_ears() }),
    )
}

/// when the server started, for its uptime
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// how fast claudius has run since it started, in Prometheus' text format (`claudius::metrics`)
async fn metrics(State(c): State<Ctx>) -> Response {
    let up = STARTED.get().map_or(0.0, |t| t.elapsed().as_secs_f64());
    let version = format!("claudius_build_info{{version=\"{}\"}}", env!("CARGO_PKG_VERSION"));
    let text = claudius::metrics::prometheus(&[
        (&version, "claudius' version", "gauge", 1.0),
        ("claudius_uptime_seconds", "seconds since claudius started serving", "gauge", up),
        ("claudius_song_changes_total", "the song's changes since it started", "counter", c.app.version() as f64),
    ]);
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], text).into_response()
}

#[derive(Deserialize)]
struct Since {
    since: Option<u64>,
    /// wait until no job runs and the render is of the current song
    settled: Option<u64>,
    timeout: Option<u64>,
}

async fn state(State(c): State<Ctx>, Query(q): Query<Since>) -> Json<claudius_api::State> {
    let timeout = Duration::from_secs(q.timeout.unwrap_or(20).min(600));
    if q.settled.is_some_and(|s| s > 0) {
        return Json(c.app.settled(timeout).await);
    }
    Json(c.app.state_after(q.since, timeout).await)
}

async fn command(State(c): State<Ctx>, headers: HeaderMap, body: Bytes) -> Response {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if !json {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "commands are JSON (Content-Type: application/json)")
            .into_response();
    }
    let cmd: claudius_api::Command = match serde_json::from_slice(&body) {
        Ok(c) => c,
        Err(e) => {
            let reply =
                claudius_api::Reply { ok: false, message: format!("not a command: {e}"), version: c.app.version() };
            return (StatusCode::BAD_REQUEST, Json(reply)).into_response();
        }
    };
    let app = c.app.clone();
    let reply = tokio::task::spawn_blocking(move || app.command(cmd)).await.unwrap_or_else(|e| claudius_api::Reply {
        ok: false,
        message: format!("internal error: {e}"),
        version: 0,
    });
    Json(reply).into_response()
}

async fn script(State(c): State<Ctx>) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], c.app.script())
}

async fn reference() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], claudius::script::REFERENCE)
}

async fn sounds(State(c): State<Ctx>) -> Json<claudius_api::SoundList> {
    Json(c.app.sounds())
}

#[derive(Deserialize)]
struct GenreQuery {
    q: Option<String>,
}

/// the genres, or those that answer `q` best first (`claudius_api::search_genres`)
async fn genres(State(c): State<Ctx>, Query(q): Query<GenreQuery>) -> Json<Vec<claudius_api::GenreInfo>> {
    Json(claudius_api::search_genres(c.app.genres(), q.q.as_deref().unwrap_or("")).into_iter().cloned().collect())
}

#[derive(Deserialize)]
struct LibraryQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    limit: Option<usize>,
}

/// the recordings the free libraries offer for `q` (`App::search_library`), or why there are none
async fn library_search(State(c): State<Ctx>, Query(q): Query<LibraryQuery>) -> Response {
    let app = c.app.clone();
    match tokio::task::spawn_blocking(move || app.search_library(&q.q, q.limit.unwrap_or(20))).await {
        Ok(Ok(hits)) => Json(hits).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_GATEWAY, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct LibraryAudioQuery {
    id: String,
}

/// a recording a search found, downloaded once (`App::library_file`): to hear it before using it
async fn library_audio(State(c): State<Ctx>, Query(q): Query<LibraryAudioQuery>, req: Request) -> Response {
    let app = c.app.clone();
    match tokio::task::spawn_blocking(move || app.library_file(&q.id)).await {
        Ok(Ok(path)) => serve_file(path, req).await,
        Ok(Err(e)) => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// a piece the auto-sampler offers, to hear (`App::pieces_file`)
async fn piece_audio(State(c): State<Ctx>, Path(file): Path<String>, req: Request) -> Response {
    match c.app.pieces_file(&file) {
        Some(p) => serve_file(p, req).await,
        None => (StatusCode::NOT_FOUND, format!("no piece {file}: the pieces offered last are the ones to hear"))
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PiecesQuery {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    bars: Option<u32>,
    #[serde(default)]
    find: Option<String>,
}

/// the auto-sampler's pieces of a recording sent as the file itself, as `sample_pieces` offers a path's
async fn sample_pieces(
    State(c): State<Ctx>,
    Query(q): Query<PiecesQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !sent_as_file(&headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the recording as the file itself, with Content-Type audio/mpeg, audio/wav or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let name = q.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "the recording".into());
    let opt = crate::app::sampling(q.count, q.bars, q.find, Vec::new());
    let seed = q.seed.unwrap_or(0);
    match tokio::task::spawn_blocking(move || app.pieces_bytes(body.to_vec(), &name, opt, seed)).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn serve_file(path: PathBuf, req: Request) -> Response {
    match ServeFile::new(path).oneshot(req).await {
        Ok(r) => r.map(Body::new).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn audio(State(c): State<Ctx>, Path(file): Path<String>, req: Request) -> Response {
    let Some((v, ext)) = file.split_once('.').and_then(|(v, e)| Some((v.parse::<u64>().ok()?, e.to_string()))) else {
        return (StatusCode::NOT_FOUND, "audio is /api/audio/{version}.wav or .mp3").into_response();
    };
    let app = c.app.clone();
    match tokio::task::spawn_blocking(move || app.audio_path(v, &ext)).await.ok().flatten() {
        Some(p) => serve_file(p, req).await,
        None => (StatusCode::NOT_FOUND, format!("no render of version {v}")).into_response(),
    }
}

async fn stem(State(c): State<Ctx>, Path((version, file)): Path<(u64, String)>, req: Request) -> Response {
    let track = file.strip_suffix(".wav").unwrap_or(&file);
    match c.app.stem_path(version, track) {
        Some(p) => serve_file(p, req).await,
        None => (StatusCode::NOT_FOUND, format!("no stem '{track}' in render {version} (render with stems)"))
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ExportQuery {
    format: Option<String>,
}

/// a render in another format, as a download named after the URL's file: `/api/export/12/glass-garden.mp3?format=mp3:192`
async fn export_file(
    State(c): State<Ctx>,
    Path((version, file)): Path<(String, String)>,
    Query(q): Query<ExportQuery>,
) -> Response {
    let (Ok(version), Some((name, ext))) = (version.parse::<u64>(), file.rsplit_once('.')) else {
        return (StatusCode::NOT_FOUND, "export is /api/export/{version}/{name}.{ext}?format=SPEC").into_response();
    };
    let spec: claudius::export::Spec = match q.format.as_deref().unwrap_or(ext).parse() {
        Ok(s) => s,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    if !spec.extension().eq_ignore_ascii_case(ext) {
        return (StatusCode::BAD_REQUEST, format!("{spec} makes .{} files, not .{ext}", spec.extension()))
            .into_response();
    }
    let download: String =
        name.chars().map(|ch| if ch.is_ascii_alphanumeric() || "-_. ".contains(ch) { ch } else { '-' }).collect();
    let app = c.app.clone();
    match tokio::task::spawn_blocking(move || app.export_bytes(version, &spec)).await {
        Ok(Ok(bytes)) => {
            let mut r = Body::from(bytes.as_ref().clone()).into_response();
            let h = r.headers_mut();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static(spec.mime()));
            if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{download}.{}\"", spec.extension())) {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            r
        }
        Ok(Err(e)) if e.starts_with("no render") => (StatusCode::NOT_FOUND, e).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// the release's artwork, as the file itself: a picture's type, so that a cross-site form cannot send one
async fn set_artwork(State(c): State<Ctx>, headers: HeaderMap, body: Bytes) -> Response {
    let picture = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("image/") || v.starts_with("application/octet-stream"));
    if !picture {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the artwork as the file itself, with Content-Type image/jpeg, image/png or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let reply = tokio::task::spawn_blocking(move || app.set_artwork_bytes(body.to_vec()))
        .await
        .unwrap_or_else(|e| claudius_api::Reply { ok: false, message: format!("internal error: {e}"), version: 0 });
    Json(reply).into_response()
}

#[derive(Deserialize)]
struct TakeQuery {
    from: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    role: Option<String>,
    /// the take request (`request_take`) it was recorded for
    #[serde(default)]
    request: Option<u64>,
    /// the name of the file it is, which says its kind when its bytes do not
    #[serde(default)]
    file: Option<String>,
}

/// a body sent as a file itself: an audio type, or bytes, which a cross-site form cannot send
fn sent_as_file(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("audio/") || v.starts_with("application/octet-stream"))
}

/// a take: the recording as the file itself, an audio type so that a cross-site form cannot send one, and where it
/// starts in the query
async fn add_take(State(c): State<Ctx>, Query(q): Query<TakeQuery>, headers: HeaderMap, body: Bytes) -> Response {
    if !sent_as_file(&headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the take as the file itself, with Content-Type audio/wav, audio/flac or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let reply = tokio::task::spawn_blocking(move || {
        app.add_take_bytes(body.to_vec(), &q.from, q.name.as_deref(), q.role.as_deref(), q.request, q.file.as_deref())
    })
    .await
    .unwrap_or_else(|e| claudius_api::Reply { ok: false, message: format!("internal error: {e}"), version: 0 });
    Json(reply).into_response()
}

#[derive(Deserialize)]
struct ProjectQuery {
    /// the zip's file name, which names the song folder it is unpacked into
    #[serde(default)]
    name: Option<String>,
}

/// a project the demo saved, to open: the zip itself, a zip's type so that a cross-site form cannot send one
async fn open_project(
    State(c): State<Ctx>,
    Query(q): Query<ProjectQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let zip = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| {
        ["application/zip", "application/x-zip-compressed", "application/octet-stream"].iter().any(|t| v.starts_with(t))
    });
    if !zip {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the project as the zip itself, with Content-Type application/zip or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let name = q.name.unwrap_or_else(|| "project.zip".to_string());
    let reply = tokio::task::spawn_blocking(move || app.open_project(&name, &body))
        .await
        .unwrap_or_else(|e| claudius_api::Reply { ok: false, message: format!("internal error: {e}"), version: 0 });
    Json(reply).into_response()
}

#[derive(Deserialize)]
struct GrooveQuery {
    /// the file's name, whose extension says its format
    #[serde(default)]
    name: Option<String>,
    /// what to call the groove, and the lanes (comma-separated) to put in it
    #[serde(default)]
    groove: Option<String>,
    #[serde(default)]
    lanes: Option<String>,
    /// its tempo, when known
    #[serde(default)]
    bpm: Option<f64>,
}

/// a recording whose groove to take, as the file itself, as `extract_groove` takes one from a path
async fn groove(State(c): State<Ctx>, Query(q): Query<GrooveQuery>, headers: HeaderMap, body: Bytes) -> Response {
    if !sent_as_file(&headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the recording as the file itself, with Content-Type audio/wav or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let name = q.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "the recording".into());
    let lanes: Vec<String> =
        q.lanes.unwrap_or_default().split(',').map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    let reply = tokio::task::spawn_blocking(move || app.groove_bytes(body.to_vec(), &name, q.groove, &lanes, q.bpm))
        .await
        .unwrap_or_else(|e| claudius_api::Reply { ok: false, message: format!("internal error: {e}"), version: 0 });
    Json(reply).into_response()
}

#[derive(Deserialize)]
struct CheckQuery {
    /// the pitch check (`request_pitch_check`) it was sung for
    id: u64,
}

/// the answers to a pitch check: the recording as the file itself, from the check's first beat, and which check
async fn pitch_check(State(c): State<Ctx>, Query(q): Query<CheckQuery>, headers: HeaderMap, body: Bytes) -> Response {
    if !sent_as_file(&headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the pitch check as the file itself, with Content-Type audio/wav or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let reply = tokio::task::spawn_blocking(move || app.pitch_check_bytes(body.to_vec(), q.id))
        .await
        .unwrap_or_else(|e| claudius_api::Reply { ok: false, message: format!("internal error: {e}"), version: 0 });
    Json(reply).into_response()
}

#[derive(Deserialize)]
struct ListenQuery {
    /// the file's name, whose extension says its format
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    genre: Option<String>,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    bpm: Option<f64>,
    /// false: a remake on the recording's skeleton only, none of its sound
    #[serde(default)]
    sampled: Option<bool>,
    /// true: a flip of it, a new song on its hook and pieces of it, rather than a rework of all of it
    #[serde(default)]
    flip: Option<bool>,
}

#[derive(Deserialize)]
struct SampleQuery {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    bars: Option<u32>,
    #[serde(default)]
    find: Option<String>,
    /// sections or kinds, comma-separated
    #[serde(default)]
    sections: Option<String>,
}

/// a recording whose vocals to sample into the song, as the file itself, as `sample_song` samples a path's
async fn sample(State(c): State<Ctx>, Query(q): Query<SampleQuery>, headers: HeaderMap, body: Bytes) -> Response {
    if !sent_as_file(&headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the recording as the file itself, with Content-Type audio/mpeg, audio/wav or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let name = q.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "the recording".into());
    let sections = q.sections.unwrap_or_default().split(',').map(|s| s.trim().to_string()).collect();
    let opt = crate::app::sampling(q.count, q.bars, q.find, sections);
    let reply = tokio::task::spawn_blocking(move || app.sample_bytes(body.to_vec(), &name, &opt)).await;
    match reply {
        Ok(r) => Json(r).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// a recording to listen to, as the file itself: what claudius hears in it, as `analyze` says it of a path
async fn analyze(State(c): State<Ctx>, Query(q): Query<ListenQuery>, headers: HeaderMap, body: Bytes) -> Response {
    listen(c, q, &headers, body, false).await
}

/// a recording to remake, as the file itself: a new song on its skeleton, as `remake` makes one from a path
async fn remake(State(c): State<Ctx>, Query(q): Query<ListenQuery>, headers: HeaderMap, body: Bytes) -> Response {
    listen(c, q, &headers, body, true).await
}

async fn listen(c: Ctx, q: ListenQuery, headers: &HeaderMap, body: Bytes, remake: bool) -> Response {
    if !sent_as_file(headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the recording as the file itself, with Content-Type audio/mpeg, audio/wav or application/octet-stream",
        )
            .into_response();
    }
    let app = c.app.clone();
    let name = q.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "the recording".into());
    let reply = tokio::task::spawn_blocking(move || match remake {
        true => {
            app.remake_bytes(body.to_vec(), &name, q.genre, q.seed, q.bpm, crate::app::Remade::of(q.sampled, q.flip))
        }
        false => app.analyze_bytes(body.to_vec(), &name),
    })
    .await
    .unwrap_or_else(|e| claudius_api::Reply { ok: false, message: format!("internal error: {e}"), version: 0 });
    Json(reply).into_response()
}

async fn artwork(State(c): State<Ctx>) -> Response {
    let Some(bytes) = c.app.artwork() else {
        return (StatusCode::NOT_FOUND, "the release has no artwork yet").into_response();
    };
    let mime = if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else {
        "application/octet-stream"
    };
    ([(header::CONTENT_TYPE, mime)], bytes.as_ref().clone()).into_response()
}

async fn artwork_preview(State(c): State<Ctx>) -> Response {
    let app = c.app.clone();
    match tokio::task::spawn_blocking(move || app.artwork_preview()).await.ok().flatten() {
        Some(png) => ([(header::CONTENT_TYPE, "image/png")], png.as_ref().clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "no artwork that reads as a picture").into_response(),
    }
}

async fn no_ui() -> Html<&'static str> {
    Html(
        "<!doctype html><meta charset=utf-8><title>claudius</title><body style='font:15px system-ui;max-width:42em;margin:3em auto'>\
         <h1>claudius</h1><p>The engine is running. The web UI is not built: run <code>trunk build --release</code> in \
         <code>crates/claudius-ui</code> (or start the native studio with <code>claudius studio</code>).</p>\
         <p>API: <a href='/api/state'>/api/state</a>, <a href='/api/script'>/api/script</a>, \
         <a href='/api/reference'>/api/reference</a>, MCP at <code>/mcp</code>.</p></body>",
    )
}
