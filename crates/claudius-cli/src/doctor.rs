//! `claudius doctor`: what this installation has, what it is missing, and how to get it.

use std::path::Path;

use crate::connect;

struct Report {
    problems: usize,
}

impl Report {
    fn line(&mut self, ok: bool, what: &str, detail: impl AsRef<str>) {
        if !ok {
            self.problems += 1;
        }
        println!("  {}  {what:<15} {}", if ok { "ok" } else { "--" }, detail.as_ref());
    }
}

fn tilde(p: &Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let s = p.display().to_string();
    match s.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => s,
    }
}

pub fn run(port: u16, web: Option<std::path::PathBuf>) {
    println!("claudius {} ({} {})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    let mut r = Report { problems: 0 };

    match web {
        Some(dir) => r.line(true, "web studio", tilde(&dir)),
        None => r.line(false, "web studio", "not found: reinstall, or build it with `cargo xtask web`"),
    }

    let packs = claudius_packs::packs();
    let defaults: Vec<_> = packs.iter().filter(|p| p.default).collect();
    let installed = defaults.iter().filter(|p| p.installed).count();
    let dir = tilde(&claudius_packs::packs_dir());
    if installed == defaults.len() {
        let gb = defaults.iter().map(|p| p.size_mb as f64).sum::<f64>() / 1000.0;
        r.line(true, "sample packs", format!("{installed} default packs ({gb:.1} GB) in {dir}"));
    } else {
        r.line(
            false,
            "sample packs",
            format!("{installed} of {} default packs in {dir}: run `claudius setup`", defaults.len()),
        );
    }

    #[cfg(feature = "ears")]
    {
        let cache = earshot::models::default_cache_dir();
        if earshot::models::cached(&cache, false) {
            let tags = if earshot::models::cached(&cache, true) { ", AudioSet tagger too" } else { "" };
            r.line(true, "ears", format!("models in {}{tags}", tilde(&cache)));
        } else {
            r.line(
                false,
                "ears",
                "models not downloaded: run `claudius setup` (verdicts use meters and theory until then)",
            );
        }
    }
    #[cfg(not(feature = "ears"))]
    println!("  ..  {:<15} not in this build (verdicts use meters and theory)", "ears");

    match claudius::library::Library::open() {
        Ok(l) if !l.is_empty() => r.line(true, "Apple Loops", format!("{} loops and samples", l.loops.len())),
        _ => println!("  ..  {:<15} not installed (optional: GarageBand or Logic content)", "Apple Loops"),
    }

    let version = claudius_decode::ffmpeg()
        .and_then(|ff| std::process::Command::new(ff).arg("-version").output().ok())
        .filter(|o| o.status.success());
    match version {
        Some(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let v = text.split_whitespace().nth(2).unwrap_or("installed").to_string();
            r.line(true, "ffmpeg", format!("{v} (writes song.mp3)"));
        }
        None => println!("  ..  {:<15} not installed (optional: songs are saved as WAV only)", "ffmpeg"),
    }

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    if std::net::TcpListener::bind(addr).is_ok() {
        r.line(true, &format!("port {port}"), "free");
    } else if claudius_server::mcp::probe(&format!("http://{addr}"), &None) != claudius_server::mcp::Probe::Nothing {
        r.line(true, &format!("port {port}"), "claudius is running here");
    } else {
        r.line(false, &format!("port {port}"), "used by another program: pass --port to studio, serve and mcp");
    }

    for (name, path, hint) in [
        ("Claude Desktop", connect::claude_desktop_config(), "claude-desktop"),
        ("Cursor", connect::cursor_config(), "cursor"),
    ] {
        if connect::configured(&path) {
            r.line(true, name, "connected");
        } else if path.exists() {
            println!("  ..  {name:<15} not connected: `claudius connect {hint}`");
        }
    }
    let codex = connect::codex_config();
    if connect::codex_configured(&codex) {
        r.line(true, "Codex", "connected");
    } else if codex.parent().is_some_and(|d| d.exists()) {
        println!("  ..  {:<15} not connected: `claudius connect codex`", "Codex");
    }

    println!();
    if r.problems == 0 {
        println!("Everything claudius needs is here.");
    } else {
        println!("{} thing(s) to fix above. claudius still works without them, with less.", r.problems);
    }
}
