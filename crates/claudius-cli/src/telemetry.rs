//! Anonymous usage statistics, off until the person turns them on: the studio's tour asks once, and `claudius
//! telemetry on` or `off` changes the answer. When they are on, claudius sends a few counts, each kind at most once a
//! day: which command started it, the version, the system and its chip, where a setup failed and where a panic
//! happened. Never a file, a path, a song, a name or an identifier. `claudius telemetry` prints what is sent, and
//! docs/src/guide/privacy.md explains it. `DO_NOT_TRACK`, `CLAUDIUS_TELEMETRY=0` and `CI` keep them off.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

/// where the events go: the Worker in telemetry/ answers there
const ENDPOINT: &str = "https://claudius-telemetry.devlato.workers.dev/v1/event";

/// the sends still on their way, which `flush` waits for before the program ends
static PENDING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("claudius").join("telemetry.json"))
}

fn read(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn write(path: &Path, m: &Map<String, Value>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(m)? + "\n")
        .with_context(|| format!("cannot write {}", path.display()))
}

/// why the environment keeps statistics off whatever the answer
fn vetoed(var: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    let set =
        |name: &str| var(name).is_some_and(|v| !matches!(v.trim().to_ascii_lowercase().as_str(), "" | "0" | "false"));
    let off = var("CLAUDIUS_TELEMETRY")
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no"));
    if set("DO_NOT_TRACK") {
        Some("DO_NOT_TRACK is set")
    } else if off {
        Some("CLAUDIUS_TELEMETRY is off")
    } else if set("CI") {
        Some("it runs in CI")
    } else {
        None
    }
}

/// the person's answer (None until asked), and what in the environment overrides it
pub fn status() -> (Option<bool>, Option<&'static str>) {
    let answer = file().map(|f| read(&f)).and_then(|m| m.get("enabled").and_then(Value::as_bool));
    (answer, vetoed(|name| std::env::var(name).ok()))
}

pub fn set(on: bool) -> Result<()> {
    let path = file().context("no configuration folder to keep the answer in")?;
    let mut m = read(&path);
    m.insert("enabled".into(), Value::Bool(on));
    write(&path, &m)
}

/// the event as it is sent
fn payload(event: &str, command: &str, detail: &str) -> Value {
    json!({
        "v": 1,
        "event": event,
        "command": command,
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "detail": detail,
    })
}

/// `CLAUDIUS_TELEMETRY_URL` sends elsewhere (to test the endpoint); a debug build sends nowhere else
fn endpoint() -> Option<String> {
    match std::env::var("CLAUDIUS_TELEMETRY_URL") {
        Ok(u) if !u.is_empty() => Some(u),
        _ if cfg!(debug_assertions) => None,
        _ => Some(ENDPOINT.into()),
    }
}

fn today() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400)
}

/// notes that `key` goes out today; false when it already did (or statistics are off)
fn due(path: &Path, key: &str, day: u64) -> bool {
    let mut m = read(path);
    if m.get("enabled").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    let sent = m.entry("sent").or_insert_with(|| Value::Object(Map::new()));
    let Some(sent) = sent.as_object_mut() else { return false };
    if sent.get(key).and_then(Value::as_u64) == Some(day) {
        return false;
    }
    sent.insert(key.into(), Value::from(day));
    write(path, &m).is_ok()
}

fn send(url: &str, body: &Value, timeout: Duration) {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(timeout)).build().into();
    let _ = agent.post(url).content_type("application/json").send(body.to_string());
}

/// sends an event when statistics are on and this one has not gone out today; it never waits, and never fails
pub fn record(event: &'static str, command: &str, detail: &str) {
    if vetoed(|name| std::env::var(name).ok()).is_some() {
        return;
    }
    let (Some(path), Some(url)) = (file(), endpoint()) else { return };
    if !due(&path, &format!("{event}:{command}:{detail}"), today()) {
        return;
    }
    let body = payload(event, command, detail);
    let handle = std::thread::spawn(move || send(&url, &body, Duration::from_secs(3)));
    if let Ok(mut p) = PENDING.lock() {
        p.push(handle);
    }
}

/// waits, up to a second and a half, for the events still on their way (a short command ends before they arrive)
pub fn flush() {
    let handles = PENDING.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    let deadline = Instant::now() + Duration::from_millis(1500);
    for h in handles {
        while !h.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// the last three parts of a source path: enough to find the line, nothing of the folders it was built in
fn short(file: &str) -> String {
    let parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    parts[parts.len().saturating_sub(3)..].join("/")
}

/// reports where a panic happens (the file and line, never its message), then panics as before
pub fn watch_panics(command: String) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        if let Some(l) = info.location() {
            record("panic", &command, &format!("{}:{}", short(l.file()), l.line()));
            flush();
        }
    }));
}

/// what `claudius telemetry` prints
pub fn describe() -> String {
    let (answer, veto) = status();
    let state = match (answer, veto) {
        (_, Some(why)) => format!("Usage statistics are off: {why}."),
        (Some(true), None) => "Usage statistics are on.".to_string(),
        (Some(false), None) => "Usage statistics are off.".to_string(),
        (None, None) => "Usage statistics are off: nobody has turned them on.".to_string(),
    };
    let example = serde_json::to_string_pretty(&payload("started", "studio", "")).unwrap_or_default();
    format!(
        "{state}\n\nWhen they are on, claudius sends each kind of event at most once a day: which command started it, \
         and where a setup or a panic failed. An event, exactly as it is sent:\n\n{example}\n\nIt goes to {ENDPOINT}, \
         which keeps no IP address. `claudius telemetry on` or `off` changes it; DO_NOT_TRACK=1 keeps it off \
         whatever the answer. More: docs/src/guide/privacy.md in github.com/devlato/claudius"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_can_keep_statistics_off() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
        };
        assert_eq!(vetoed(env(&[])), None);
        assert_eq!(vetoed(env(&[("DO_NOT_TRACK", "1")])), Some("DO_NOT_TRACK is set"));
        assert_eq!(vetoed(env(&[("DO_NOT_TRACK", "0")])), None);
        assert_eq!(vetoed(env(&[("CLAUDIUS_TELEMETRY", "off")])), Some("CLAUDIUS_TELEMETRY is off"));
        assert_eq!(vetoed(env(&[("CI", "true")])), Some("it runs in CI"));
    }

    #[test]
    fn an_event_holds_the_seven_fields_and_nothing_else() {
        let e = payload("setup", "setup", "packs:vcsl");
        let keys: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys.len(), 7);
        for k in ["v", "event", "command", "version", "os", "arch", "detail"] {
            assert!(keys.contains(&k), "{k} is missing from {e}");
        }
    }

    #[test]
    fn each_kind_goes_out_once_a_day_and_only_when_on() {
        let dir = std::env::temp_dir().join(format!("claudius-telemetry-{}", std::process::id()));
        let path = dir.join("telemetry.json");
        assert!(!due(&path, "started:studio:", 100), "never asked is off");
        write(&path, &read(&path).into_iter().chain([("enabled".to_string(), Value::Bool(true))]).collect()).unwrap();
        assert!(due(&path, "started:studio:", 100));
        assert!(!due(&path, "started:studio:", 100));
        assert!(due(&path, "started:make:", 100));
        assert!(due(&path, "started:studio:", 101));
        let mut m = read(&path);
        m.insert("enabled".into(), Value::Bool(false));
        write(&path, &m).unwrap();
        assert!(!due(&path, "started:serve:", 102));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_panic_names_its_place_without_the_folders_it_was_built_in() {
        assert_eq!(short("crates/claudius/src/render.rs"), "claudius/src/render.rs");
        assert_eq!(
            short("/Users/someone/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ort-2.0.0/src/session.rs"),
            "ort-2.0.0/src/session.rs"
        );
        assert_eq!(short(r"C:\Users\someone\.cargo\registry\src\x\hound-3.5.1\src\read.rs"), "hound-3.5.1/src/read.rs");
    }

    #[test]
    fn the_endpoint_knows_every_command() {
        let worker = include_str!("../../../telemetry/src/index.js");
        let commands: Vec<String> = <crate::args::Cli as clap::CommandFactory>::command()
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .filter(|n| n != "telemetry")
            .collect();
        assert!(commands.len() > 15);
        for c in commands {
            assert!(worker.contains(&format!("\"{c}\"")), "telemetry/src/index.js does not accept the command {c}");
        }
    }
}
