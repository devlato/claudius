//! The claudius engine as a service. One `App` holds the session (the song with undo and redo, renders, verdicts,
//! jobs); the HTTP API of `claudius-api`, the web UI and the MCP tools are views of that one session, so an assistant
//! editing over MCP and a person in the studio see each other's changes as they happen.

#![forbid(unsafe_code)]

pub mod app;
pub mod http;
pub mod mcp;
pub mod views;

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

pub use app::{App, Settings};
pub use claudius_api::DEFAULT_PORT;
pub use http::Options;

/// how the program around the server hears what it may count: `(event, detail)`, such as `("setup", "packs:vcsl")`.
/// `claudius-cli` hands these to its usage statistics, which send nothing unless the person turned them on.
pub static REPORT: OnceLock<Reporter> = OnceLock::new();

pub type Reporter = Box<dyn Fn(&'static str, &str) + Send + Sync>;

fn report(event: &'static str, detail: &str) {
    if let Some(f) = REPORT.get() {
        f(event, detail);
    }
}

fn url_of(addr: SocketAddr) -> String {
    if addr.ip().is_unspecified() { format!("http://127.0.0.1:{}", addr.port()) } else { format!("http://{addr}") }
}

fn cannot_listen(addr: SocketAddr, e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::AddrInUse {
        anyhow::anyhow!("port {} is taken, by another claudius or another program (--port picks another)", addr.port())
    } else {
        anyhow::anyhow!("cannot listen on {addr}: {e}")
    }
}

/// serve on `addr` until the process ends; `listening` runs once the port is bound, with its address
pub async fn serve(
    app: Arc<App>,
    addr: SocketAddr,
    o: Options,
    listening: impl FnOnce(SocketAddr),
) -> anyhow::Result<()> {
    o.check()?;
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| cannot_listen(addr, e))?;
    let local = listener.local_addr()?;
    app.set_base_url(url_of(local));
    listening(local);
    axum::serve(listener, http::router(app, &o)).await?;
    Ok(())
}

/// serve from a thread with its own runtime (for a native UI on the main thread); returns the address once listening
pub fn spawn(app: Arc<App>, addr: SocketAddr, o: Options) -> anyhow::Result<SocketAddr> {
    o.check()?;
    let listener = std::net::TcpListener::bind(addr).map_err(|e| cannot_listen(addr, e))?;
    listener.set_nonblocking(true)?;
    let local = listener.local_addr()?;
    app.set_base_url(url_of(local));
    std::thread::Builder::new().name("claudius-server".into()).spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a tokio runtime");
        rt.block_on(async move {
            match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => {
                    if let Err(e) = axum::serve(l, http::router(app, &o)).await {
                        eprintln!("claudius server stopped: {e}");
                    }
                }
                Err(e) => eprintln!("claudius server cannot listen: {e}"),
            }
        });
    })?;
    Ok(local)
}
