//! claudius studio: the visual front end of claudius, written with egui / eframe so the same code runs as a native
//! window and as a web app (wasm32, built with trunk). It is a client of the claudius server (`claudius-api` is the
//! wire contract): it long-polls `/api/state`, sends `Command`s to `/api/command` and plays the latest render.
//!
//! Without a server (`--mock`, env `CLAUDIUS_MOCK=1`, `?mock` on the web, or "Use the demo" in the app) a built-in
//! mock plays the server's part with a generated song, so every panel works offline.

#![forbid(unsafe_code)]

mod app;
mod audio;
mod cmds;
#[cfg(not(target_arch = "wasm32"))]
mod devplan;
mod edits;
mod mock;
mod music;
mod net;
mod panels;
mod theme;
mod widgets;

pub use app::Studio;
pub use net::Connection;

/// The window title and the id eframe stores the studio's preferences under.
pub const APP_NAME: &str = "claudius studio";

/// Where `trunk build --release` (run in this crate) puts the web app: serve this folder at `/` and the API under
/// `/api` on the same origin.
pub const WEB_DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/dist");

/// Opens the studio in a native window and runs until it is closed.
///
/// `server` is the base URL of the claudius server (`http://127.0.0.1:7878`, or `host:port`); when `None`, env
/// `CLAUDIUS_SERVER` is used, else `http://127.0.0.1:7878`. `"mock"` (or env `CLAUDIUS_MOCK=1`) runs against the
/// built-in demo instead of a server.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native(server: Option<String>) -> eframe::Result<()> {
    run_native_with(Connection::native(server))
}

/// Like [`run_native`], with the connection spelled out.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native_with(connection: Connection) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("claudius-studio")
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([1100.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(Studio::new(cc, connection)))))
}

/// Starts the studio in the page's `<canvas id="{canvas_id}">`, talking to the page's own origin (or `?server=URL`,
/// or the demo with `?mock`). Used by the trunk build (`index.html` + `main.rs`).
#[cfg(target_arch = "wasm32")]
pub async fn start_web(canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast as _;
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| format!("no element #{canvas_id}"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let connection = Connection::web();
    let fallback = !Connection::web_chosen();
    let result = eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |cc| {
                let mut studio = Studio::new(cc, connection);
                studio.fallback_to_demo = fallback;
                Ok(Box::new(studio))
            }),
        )
        .await;
    if let Some(loading) = document.get_element_by_id("loading") {
        match &result {
            Ok(()) => loading.remove(),
            Err(e) => loading.set_inner_html(&format!("<p>claudius studio could not start: {e:?}</p>")),
        }
    }
    result
}
