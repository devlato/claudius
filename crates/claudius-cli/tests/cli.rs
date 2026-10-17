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
    let json: String = text.lines().skip(1).take_while(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}\n{json}"));
    assert_eq!(v["mcpServers"]["claudius"]["args"][0], "mcp");
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
    for f in ["song.wav", "blueprint.json", "verdict.json", "report.json"] {
        assert!(dir.join(f).is_file(), "{f} missing in {}", dir.display());
    }
    let verdict: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("verdict.json")).unwrap()).unwrap();
    assert!(verdict["total"].as_f64().unwrap() > 50.0);
    std::fs::remove_dir_all(&out).ok();
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
