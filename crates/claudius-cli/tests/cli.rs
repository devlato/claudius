//! The `claudius` binary, run the way people and assistants run it.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use assert_cmd::cargo::CommandCargoExt;
use predicates::prelude::*;

fn claudius() -> assert_cmd::Command {
    let mut c = assert_cmd::Command::cargo_bin("claudius").unwrap();
    // no sample packs or Apple Loops: the same result on every machine
    c.env("CLAUDIUS_PACKS", std::env::temp_dir().join("claudius-cli-test-no-packs"));
    c
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("claudius-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn help_lists_the_commands() {
    let out = claudius().arg("--help").assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    for cmd in ["make", "script", "studio", "serve", "mcp", "setup", "doctor", "connect", "evolve", "remake"] {
        assert!(text.contains(cmd), "--help does not mention {cmd}");
    }
}

#[test]
fn genres_and_reference() {
    claudius()
        .arg("genres")
        .assert()
        .success()
        .stdout(predicate::str::contains("house").and(predicate::str::contains("lofi")));
    claudius().arg("reference").assert().success().stdout(predicate::str::starts_with("# claudius script"));
}

#[test]
fn a_script_from_stdin_prints_its_song() {
    claudius()
        .args(["script", "-", "--print"])
        .write_stdin(r#"blank("house"); tempo(121); track("kick", s("bd*4"));"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("tempo(121)").and(predicate::str::contains(r#"track("kick", s("bd*4"));"#)));
}

#[test]
fn a_broken_script_fails_with_its_reason() {
    claudius()
        .args(["script", "-", "--print"])
        .write_stdin(r#"track("x", s("nope*4"));"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown sound 'nope'"));
}

#[test]
fn connect_print_shows_a_working_config() {
    let out = claudius().args(["connect", "print"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    let json: String =
        text.lines().skip_while(|l| *l != "{").take_while(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(v["mcpServers"]["claudius"]["args"][0], "mcp");
    for own in [
        "claude mcp add --scope user claudius -- ",
        "codex mcp add claudius -- ",
        "cursor://anysphere.cursor-deeplink/mcp/install?name=claudius&config=",
    ] {
        assert!(text.contains(own), "{own} in:\n{text}");
    }
}

#[test]
fn make_writes_a_song_folder() {
    let out = temp_dir("make");
    claudius()
        .args(["make", "--genre", "techno", "--seed", "3", "--short", "--synth-only", "--no-ears", "--out"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("verdict"));
    let dir = std::fs::read_dir(&out).unwrap().flatten().next().expect("a song folder").path();
    for f in ["song.wav", "song.mp3", "blueprint.json", "verdict.json", "report.json"] {
        assert!(dir.join(f).is_file(), "{f} missing in {}", dir.display());
    }
    let verdict: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("verdict.json")).unwrap()).unwrap();
    assert!(verdict["total"].as_f64().unwrap() > 50.0);

    // the song in three more formats, named after its title, next to it
    let exported = temp_dir("export");
    claudius()
        .arg("export")
        .arg(&dir)
        .args(["-f", "flac:16,mp3:192", "--format", "ogg", "--out"])
        .arg(&exported)
        .assert()
        .success()
        .stdout(predicate::str::contains("FLAC, 16-bit, 44.1 kHz").and(predicate::str::contains("MP3, 192 kbps")));
    let files: Vec<_> = std::fs::read_dir(&exported).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(files.len(), 3, "{files:?}");
    let master = claudius_decode::decode_native(&dir.join("song.wav")).unwrap();
    for f in &files {
        let d = claudius_decode::decode_native(f).unwrap();
        let rate = if f.extension().is_some_and(|e| e == "flac") { 44_100 } else { 48_000 };
        assert_eq!((d.rate, d.channels), (rate, 2), "{}", f.display());
        let secs = |d: &claudius_decode::Decoded| d.frames() as f64 / d.rate as f64;
        assert!((secs(&d) - secs(&master)).abs() < 0.1, "{}: {} s for {} s", f.display(), secs(&d), secs(&master));
    }
    std::fs::remove_dir_all(&exported).ok();
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn export_lists_its_formats_and_explains_bad_ones() {
    claudius()
        .args(["export", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("flac:16    FLAC, 16-bit, 44.1 kHz").and(predicate::str::contains("opus")));
    let dir = temp_dir("export-bad");
    claudius()
        .arg("export")
        .arg(&dir)
        .args(["-f", "mp3:300"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("MP3 takes 32, 40"));
    claudius()
        .arg("export")
        .arg(&dir)
        .args(["-f", "mp3"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no song.wav: render the song first"));
    std::fs::remove_dir_all(&dir).ok();
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn mcp_over_stdio_lists_and_runs_tools() {
    let mut child = Command::cargo_bin("claudius")
        .unwrap()
        .args(["mcp", "--no-ears", "--synth-only", "--no-auto-render", "--port", &free_port().to_string()])
        .env("CLAUDIUS_PACKS", std::env::temp_dir().join("claudius-cli-test-no-packs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut send = |v: serde_json::Value| writeln!(stdin, "{v}").unwrap();
    let mut recv = || {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    };
    send(
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}}),
    );
    assert_eq!(recv()["result"]["serverInfo"]["name"], "claudius");
    send(serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    send(serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let tools = recv();
    let names: Vec<&str> =
        tools["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"run_script") && names.contains(&"new_song"), "{names:?}");
    send(
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "run_script", "arguments": {"code": "blank(\"house\"); tempo(118);", "listen": false}}}),
    );
    let r = recv();
    assert_ne!(r["result"]["isError"], true, "{r}");
    send(
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "get_song", "arguments": {}}}),
    );
    let song = recv();
    assert!(song["result"]["content"][0]["text"].as_str().unwrap().contains("118 BPM"), "{song}");
    drop(stdin);
    let _ = child.wait();
}
