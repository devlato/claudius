//! Taking its port from an older claudius, for the studio window. An assistant set up before this version starts a
//! claudius of its own on the port (`claudius mcp`), which the studio then steps round (`beside`): the two hold different
//! songs, and what the assistant changes never shows in the studio. Stopped, with its song saved first, the port serves
//! the studio's engine, which the assistant's claudius finds there when the assistant starts it again: every claudius
//! drives the one it finds on its port.

use anyhow::{Context, Result, bail};
use claudius_server::mcp::{Probe, older_than_this, probe};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Stops the older claudius on `port` of this computer and serves `own`, the studio's engine, there as well. Its song is
/// saved first, into a folder of its own under the songs folder, and nothing is stopped when it cannot be. Refuses what
/// is not claudius, a claudius no older than this one, and one that asks for a token. What it did, in a line, and the
/// folder its song went into.
pub fn take_over(
    port: u16,
    own: &Arc<claudius_server::App>,
    options: claudius_server::Options,
) -> Result<(String, Option<PathBuf>)> {
    let base = format!("http://127.0.0.1:{port}");
    let older = match probe(&base, &None) {
        Probe::Claudius(v) if older_than_this(&v) => Some(v),
        Probe::Claudius(v) => bail!("the claudius on port {port} ({v}) is no older than this one"),
        Probe::NeedsToken => bail!("the claudius on port {port} asks for a token: stop it where it runs"),
        Probe::Nothing => None,
    };
    let mut saved = None;
    if let Some(v) = &older {
        saved = save_song(&base, v, &claudius::config::songs_dir())
            .map_err(|e| anyhow::anyhow!("could not save its song first ({e}), so it still runs"))?;
        let pid = listener(port).with_context(|| format!("cannot tell which program holds port {port}"))?;
        let program = program_of(pid).unwrap_or_default();
        if !is_claudius(&program) {
            bail!("port {port} is held by {program} (process {pid}), which is not claudius: stop it where it runs");
        }
        stop(pid)?;
        let since = Instant::now();
        while !matches!(probe(&base, &None), Probe::Nothing) {
            if since.elapsed() > Duration::from_secs(5) {
                bail!("the older claudius (process {pid}) did not let port {port} go");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    claudius_server::spawn(own.clone(), SocketAddr::from(([127, 0, 0, 1], port)), options)
        .with_context(|| format!("port {port} is free, but this studio could not take it"))?;
    Ok((said(port, older.as_deref(), saved.as_deref()), saved))
}

/// what taking the port did, in a line
fn said(port: u16, older: Option<&str>, saved: Option<&Path>) -> String {
    let was = match older {
        Some("") => format!("Stopped the older claudius on port {port}"),
        Some(v) => format!("Stopped claudius {v} on port {port}"),
        None => format!("Port {port} was free"),
    };
    let song = match (older, saved) {
        (_, Some(dir)) => format!(" Its song is saved in {}.", dir.display()),
        (Some(_), None) => " It held no song.".to_string(),
        (None, None) => String::new(),
    };
    format!(
        "{was}: this studio answers there now.{song} Restart the assistant that started it, and connect it again \
         (Connect an assistant…) so that it starts this version from now on."
    )
}

/// The song of the claudius at `base`, saved into a folder of its own in `songs` named for where it came from ("from
/// claudius 0.8.0"); none when it holds no song. A claudius saves its takes with its song, and renders it, which takes
/// a while.
fn save_song(base: &str, version: &str, songs: &Path) -> Result<Option<PathBuf>, String> {
    let name =
        if version.is_empty() { "from an earlier claudius".to_string() } else { format!("from claudius {version}") };
    let dir = (1..)
        .map(|n| if n == 1 { songs.join(&name) } else { songs.join(format!("{name} {n}")) })
        .find(|d| !d.exists())
        .ok_or("no folder name is free")?;
    let body = serde_json::json!({ "cmd": "save", "path": dir.display().to_string() }).to_string();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(180)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut res = agent
        .post(format!("{base}/api/command"))
        .header("Content-Type", "application/json")
        .send(body)
        .map_err(|e| format!("it did not answer: {e}"))?;
    let reply: serde_json::Value =
        serde_json::from_str(&res.body_mut().read_to_string().unwrap_or_default()).map_err(|e| e.to_string())?;
    let message = reply["message"].as_str().unwrap_or_default();
    match reply["ok"].as_bool() {
        Some(true) => Ok(Some(dir)),
        _ if message.starts_with("no song") => Ok(None),
        _ => Err(if message.is_empty() { "it said nothing".to_string() } else { message.to_string() }),
    }
}

/// whether `program`, as the system names a process's program, is claudius (`claudius`, `claudius.exe`, or a path to
/// one)
fn is_claudius(program: &str) -> bool {
    Path::new(program.trim())
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_lowercase().starts_with("claudius"))
}

/// the process listening on TCP `port` of this computer, by `lsof`, or `ss` where there is no `lsof`
#[cfg(unix)]
fn listener(port: u16) -> Option<u32> {
    let lsof = Command::new("lsof").args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"]).output();
    if let Some(pid) = lsof.ok().and_then(|o| first_pid(&String::from_utf8_lossy(&o.stdout))) {
        return Some(pid);
    }
    let ss = Command::new("ss").args(["-ltnpH", &format!("sport = :{port}")]).output().ok()?;
    ss_pid(&String::from_utf8_lossy(&ss.stdout))
}

/// the process listening on TCP `port` of this computer, by `netstat`
#[cfg(windows)]
fn listener(port: u16) -> Option<u32> {
    let out = Command::new("netstat").args(["-ano", "-p", "TCP"]).output().ok()?;
    netstat_pid(&String::from_utf8_lossy(&out.stdout), port)
}

/// the first process id `lsof -t` names
#[cfg(any(unix, test))]
fn first_pid(lsof: &str) -> Option<u32> {
    lsof.lines().find_map(|l| l.trim().parse().ok())
}

/// the process id `ss -p` names: `users:(("claudius",pid=1234,fd=9))`
#[cfg(any(unix, test))]
fn ss_pid(ss: &str) -> Option<u32> {
    ss.split("pid=").nth(1)?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// the process `netstat -ano` says listens on `port`: `TCP  127.0.0.1:7878  0.0.0.0:0  LISTENING  1234`
#[cfg(any(windows, test))]
fn netstat_pid(netstat: &str, port: u16) -> Option<u32> {
    let on = format!(":{port}");
    netstat.lines().find_map(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        (f.len() >= 5 && f[0] == "TCP" && f[1].ends_with(&on) && f[3] == "LISTENING").then(|| f[4].parse().ok())?
    })
}

/// the program process `pid` runs, as `ps` names it
#[cfg(unix)]
fn program_of(pid: u32) -> Option<String> {
    let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// the program process `pid` runs, as `tasklist` names it
#[cfg(windows)]
fn program_of(pid: u32) -> Option<String> {
    let filter = format!("PID eq {pid}");
    let out = Command::new("tasklist").args(["/FI", &filter, "/FO", "CSV", "/NH"]).output().ok()?;
    let line = String::from_utf8_lossy(&out.stdout).lines().next()?.to_string();
    Some(line.split(',').next()?.trim_matches('"').to_string()).filter(|s| !s.is_empty())
}

/// stops process `pid`: asked to end, then made to after three seconds
#[cfg(unix)]
fn stop(pid: u32) -> Result<()> {
    let id = pid.to_string();
    let alive = || Command::new("kill").args(["-0", &id]).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    Command::new("kill").args(["-TERM", &id]).status().context("cannot run kill")?;
    let since = Instant::now();
    while alive() && since.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(100));
    }
    if alive() {
        Command::new("kill").args(["-KILL", &id]).status().context("cannot run kill")?;
    }
    Ok(())
}

/// stops process `pid`
#[cfg(windows)]
fn stop(pid: u32) -> Result<()> {
    let s = Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).status()?;
    anyhow::ensure!(s.success(), "taskkill could not stop process {pid}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_process_on_a_port_is_read_from_lsof_ss_or_netstat_and_only_claudius_is_stopped() {
        assert_eq!(first_pid("57363\n"), Some(57363));
        assert_eq!(first_pid(""), None);
        assert_eq!(ss_pid("LISTEN 0 1024 127.0.0.1:7878 0.0.0.0:* users:((\"claudius\",pid=4242,fd=9))"), Some(4242));
        let netstat = "  Proto  Local Address  Foreign Address  State  PID\n  TCP  0.0.0.0:135  0.0.0.0:0  LISTENING  1000\n  \
                       TCP  127.0.0.1:7878  0.0.0.0:0  LISTENING  5150\n  TCP  127.0.0.1:7878  127.0.0.1:51000  ESTABLISHED  77";
        assert_eq!(netstat_pid(netstat, 7878), Some(5150));
        assert_eq!(netstat_pid(netstat, 7879), None);
        for program in
            ["claudius", "/Users/me/.local/bin/claudius", "/Applications/Claudius.app/Contents/MacOS/claudius"]
        {
            assert!(is_claudius(program), "{program}");
        }
        assert!(is_claudius("claudius.exe"));
        assert!(!is_claudius("python3") && !is_claudius("/usr/bin/node") && !is_claudius(""));
    }

    #[test]
    fn a_line_says_what_was_stopped_where_its_song_went_and_what_to_do_next() {
        let line = said(7878, Some("0.8.0"), Some(Path::new("/m/from claudius 0.8.0")));
        assert!(line.starts_with("Stopped claudius 0.8.0 on port 7878: this studio answers there now."), "{line}");
        assert!(
            line.contains("Its song is saved in /m/from claudius 0.8.0.") && line.contains("Restart the assistant")
        );
        assert!(said(7878, Some(""), None).contains("Stopped the older claudius on port 7878: this studio answers"));
        assert!(said(7878, Some("0.8.0"), None).contains("It held no song."));
    }

    #[test]
    fn a_song_is_saved_into_a_folder_named_for_the_claudius_it_came_from_and_none_is_reported_for_no_song() {
        use std::io::{Read, Write};
        // a claudius of an older version that saves what it is asked to, then one that holds no song
        let songs = std::env::temp_dir().join(format!("claudius-takeover-{}", std::process::id()));
        std::fs::create_dir_all(songs.join("from claudius 0.8.0")).unwrap();
        for reply in [r#"{"ok":true,"message":"saved"}"#, r#"{"ok":false,"message":"no song to save"}"#] {
            let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", server.local_addr().unwrap());
            let answered = std::thread::spawn(move || {
                let (mut s, _) = server.accept().unwrap();
                // the whole request: its head, then as much body as it says it has
                let (mut asked, mut chunk) = (Vec::new(), [0u8; 4096]);
                let complete = |a: &[u8]| {
                    let text = String::from_utf8_lossy(a);
                    let Some((head, body)) = text.split_once("\r\n\r\n") else { return false };
                    let length = head.lines().find_map(|l| {
                        l.to_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok())
                    });
                    body.len() >= length.unwrap_or(0)
                };
                while !complete(&asked) {
                    let n = s.read(&mut chunk).unwrap();
                    if n == 0 {
                        break;
                    }
                    asked.extend_from_slice(&chunk[..n]);
                }
                let body =
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
                s.write_all(body.as_bytes()).unwrap();
                String::from_utf8_lossy(&asked).to_string()
            });
            let got = save_song(&base, "0.8.0", &songs);
            let asked = answered.join().unwrap();
            match reply.contains("true") {
                true => {
                    // the first name is taken: the next one
                    assert_eq!(got, Ok(Some(songs.join("from claudius 0.8.0 2"))));
                    assert!(asked.contains("\"cmd\":\"save\"") && asked.contains("from claudius 0.8.0 2"), "{asked}");
                }
                false => assert_eq!(got, Ok(None)),
            }
        }
        let _ = std::fs::remove_dir_all(&songs);
    }
}
