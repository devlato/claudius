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
    let ctx = Ctx { app: app.clone() };
    let api = Router::new()
        .route("/api/hello", get(hello))
        .route("/api/state", get(state))
        .route("/api/command", post(command).layer(DefaultBodyLimit::max(1 << 20)))
        .route("/api/script", get(script))
        .route("/api/reference", get(reference))
        .route("/api/sounds", get(sounds))
        .route("/api/audio/{file}", get(audio))
        .route("/api/stems/{version}/{file}", get(stem))
        .route("/api/export/{version}/{file}", get(export_file))
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
fn same_secret(a: &str, b: &str) -> bool {
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

async fn hello(State(c): State<Ctx>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({ "name": "claudius", "version": env!("CARGO_PKG_VERSION"), "state": c.app.version(), "ears": c.app.has_ears() }),
    )
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

async fn no_ui() -> Html<&'static str> {
    Html(
        "<!doctype html><meta charset=utf-8><title>claudius</title><body style='font:15px system-ui;max-width:42em;margin:3em auto'>\
         <h1>claudius</h1><p>The engine is running. The web UI is not built: run <code>trunk build --release</code> in \
         <code>crates/claudius-ui</code> (or start the native studio with <code>claudius studio</code>).</p>\
         <p>API: <a href='/api/state'>/api/state</a>, <a href='/api/script'>/api/script</a>, \
         <a href='/api/reference'>/api/reference</a>, MCP at <code>/mcp</code>.</p></body>",
    )
}
