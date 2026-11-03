use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use claudius_api::{Command, State};
use claudius_server::{App, Options, Settings, http};
use tower::ServiceExt;

fn app() -> Arc<App> {
    App::new(Settings {
        library: false,
        ears: false,
        auto_render: true,
        file_roots: None,
        voice: None,
        samples: false,
        gather: false,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_edits_renders_and_undoes() {
    let app = app();
    let r = app.command(Command::NewSong {
        genre: "house".into(),
        seed: Some(4),
        bpm: Some(122.0),
        key: None,
        loops: false,
    });
    assert!(r.ok, "{}", r.message);
    let st = app.settled(Duration::from_secs(120)).await;
    let render = st.render.clone().expect("a render");
    assert!(render.version >= st.song_version && render.seconds > 60.0 && !render.waveform.is_empty());
    assert!(st.verdict.as_ref().is_some_and(|v| v.total > 50.0));
    assert!(app.audio_path(render.version, "wav").is_some());

    let r = app.command(Command::RunScript { code: "tempo(128); solo(\"kick\");".into() });
    assert!(r.ok, "{}", r.message);
    let st = app.state();
    let song = st.song.clone().unwrap();
    assert_eq!(song.bpm, 128.0);
    assert!(song.tracks.iter().any(|t| t.name == "kick" && t.solo));
    assert!(st.undo_label.is_some());
    // a render of the old song is now stale
    assert!(st.render.as_ref().is_none_or(|r| r.version < st.song_version) || st.busy.is_some());
    let only: Vec<String> =
        claudius::compose::arrange(&app.blueprint().unwrap()).tracks.iter().map(|t| t.name.clone()).collect();
    assert_eq!(only, ["kick"]);

    let bad = app.command(Command::SetSection {
        index: 0,
        name: None,
        bars: None,
        energy: None,
        layers: None,
        prog: Some("Z".into()),
        variant: None,
        filter_in: None,
        build: None,
        gap: None,
        impact: None,
        fills: None,
    });
    assert!(!bad.ok && bad.message.contains("no progression 'Z'"), "{}", bad.message);

    assert!(app.command(Command::Undo).ok);
    assert_eq!(app.state().song.unwrap().bpm, 122.0);
    assert!(app.command(Command::Redo).ok);
    assert_eq!(app.state().song.unwrap().bpm, 128.0);
    let st = app.settled(Duration::from_secs(120)).await;
    assert!(st.busy.is_none() && st.render.is_some_and(|r| r.version >= st.song_version));
}

#[test]
fn tracks_are_added_and_removed() {
    let app = App::new(Settings {
        library: false,
        ears: false,
        auto_render: false,
        file_roots: None,
        voice: None,
        samples: false,
        gather: false,
    });
    let new = Command::NewSong { genre: "house".into(), seed: Some(4), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    let add = |name: &str, kind: &str, role: Option<&str>, sound: Option<&str>| {
        app.command(Command::AddTrack {
            name: name.into(),
            kind: kind.into(),
            role: role.map(String::from),
            sound: sound.map(String::from),
        })
    };
    let r = add("lead2", "part", Some("lead"), Some("saw_lead"));
    assert!(r.ok, "{}", r.message);
    let song = app.state().song.unwrap();
    let part = song.parts.iter().find(|p| p.name == "lead2").expect("the new part");
    assert_eq!((part.role.as_str(), part.preset.as_str()), ("lead", "saw_lead"));
    assert!(song.sections.iter().any(|s| s.layers.contains(&"lead2".to_string())));
    let r = add("conga", "drum", None, Some("tom"));
    assert!(r.ok, "{}", r.message);
    let song = app.state().song.unwrap();
    let lane = song.drums.iter().find(|d| d.lane == "conga").expect("the new lane");
    assert!(lane.variants.iter().all(|(_, steps)| steps.iter().all(|v| *v == 0.0)), "a new lane is silent");
    // a name the song has, or of two words, and a kind there is not
    for (name, kind, says) in
        [("lead2", "part", "already has a track"), ("my lead", "part", "one word"), ("x", "loop", "not 'loop'")]
    {
        let r = add(name, kind, Some("lead"), None);
        assert!(!r.ok && r.message.contains(says), "{name}: {}", r.message);
    }
    let r = app.command(Command::RemoveTrack { name: "lead2".into() });
    assert!(r.ok, "{}", r.message);
    let song = app.state().song.unwrap();
    assert!(!song.layers.contains(&"lead2".to_string()));
    assert!(song.sections.iter().all(|s| !s.layers.contains(&"lead2".to_string())));
    assert!(app.command(Command::Undo).ok);
    assert!(app.state().song.unwrap().parts.iter().any(|p| p.name == "lead2"));
    assert!(!app.command(Command::RemoveTrack { name: "nope".into() }).ok);
}

#[test]
fn a_parts_notes_are_written_out_and_given_back() {
    let app = App::new(Settings {
        library: false,
        ears: false,
        auto_render: false,
        file_roots: None,
        voice: None,
        samples: false,
        gather: false,
    });
    let new = Command::NewSong { genre: "house".into(), seed: Some(4), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    let lead = |app: &App| app.state().song.unwrap().parts.into_iter().find(|p| p.name == "lead").unwrap();
    let before = lead(&app);
    let cycle = before.cycles.first().expect("the lead plays a progression").clone();
    assert!(!cycle.written && !cycle.notes.is_empty() && cycle.beats >= 16.0, "{cycle:?}");
    let mut notes = cycle.notes.clone();
    notes[0].p += 1;
    notes.push(claudius_api::NoteView { t: 0.5, d: 0.25, p: 84, v: 0.6 });
    let set = |notes| Command::SetNotes { part: "lead".into(), prog: cycle.prog.clone(), notes };
    let r = app.command(set(Some(notes.clone())));
    assert!(r.ok, "{}", r.message);
    let written = lead(&app).cycles.into_iter().find(|c| c.prog == cycle.prog).unwrap();
    assert!(written.written);
    let mut sorted = notes.clone();
    sorted.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.p.cmp(&b.p)));
    assert_eq!(written.notes, sorted, "the notes as they were sent, in order");
    let bad = app.command(Command::SetNotes { part: "nope".into(), prog: "A".into(), notes: Some(Vec::new()) });
    assert!(!bad.ok && bad.message.contains("no part 'nope'"), "{}", bad.message);
    assert!(app.command(set(None)).ok);
    assert_eq!(lead(&app).cycles, before.cycles, "the theory engine writes what it wrote");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shared_session_keeps_to_its_folders() {
    let inside = std::env::temp_dir().join(format!("claudius-roots-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("claudius-outside-{}", std::process::id()));
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("song.rhai"), "tempo(100);").unwrap();
    std::fs::write(inside.join("song.rhai"), "tempo(100);").unwrap();
    let app = App::new(Settings {
        library: false,
        ears: false,
        auto_render: false,
        file_roots: Some(vec![inside.clone()]),
        voice: None,
        samples: false,
        gather: false,
    });
    assert!(
        app.command(Command::NewSong { genre: "lofi".into(), seed: Some(1), bpm: None, key: None, loops: false }).ok
    );
    let r = app.command(Command::Load { path: outside.join("song.rhai").display().to_string() });
    assert!(!r.ok && r.message.contains("outside the folders"), "{}", r.message);
    let r = app.command(Command::Load { path: inside.join("song.rhai").display().to_string() });
    assert!(r.ok, "{}", r.message);
    // outside the folders, a missing file gets the same answer as one that exists: no telling which files are there
    let r = app.command(Command::Load { path: outside.join("missing.rhai").display().to_string() });
    assert!(!r.ok && r.message.contains("outside the folders"), "{}", r.message);
    let r = app.command(Command::Load { path: inside.join("missing.rhai").display().to_string() });
    assert!(!r.ok && r.message.contains("does not exist"), "{}", r.message);
    let r = app.command(Command::Save { path: Some(outside.join("saved").display().to_string()) });
    assert!(!r.ok && r.message.contains("outside the folders"), "{}", r.message);
    let r = app.command(Command::Save { path: Some(inside.join("../escape").display().to_string()) });
    assert!(!r.ok, "{}", r.message);
    let wav = outside.join("x.wav");
    std::fs::write(&wav, b"RIFF").unwrap();
    let r = app
        .command(Command::RunScript { code: format!("track(\"v\", note(\"c4\").s({:?}))", wav.display().to_string()) });
    assert!(!r.ok && r.message.contains("outside the folders"), "{}", r.message);
    let r = app.command(Command::SetRelease {
        details: Default::default(),
        artwork: Some(outside.join("x.wav").display().to_string()),
    });
    assert!(!r.ok && r.message.contains("outside the folders"), "artwork is read as any file is: {}", r.message);
    std::fs::remove_dir_all(&inside).ok();
    std::fs::remove_dir_all(&outside).ok();
}

#[test]
fn packs_install_only_on_the_computer_claudius_runs_on() {
    let shared = App::new(Settings {
        library: false,
        ears: false,
        auto_render: false,
        file_roots: Some(vec![std::env::temp_dir()]),
        voice: None,
        samples: false,
        gather: false,
    });
    let r = shared.command(Command::InstallPacks { names: Vec::new() });
    assert!(!r.ok && r.message.contains("shared claudius"), "{}", r.message);
    let local = app();
    let r = local.command(Command::InstallPacks { names: vec!["nope".into()] });
    assert!(!r.ok && r.message.contains("no pack nope"), "{}", r.message);
    // an opt-in pack, on an unclear licence, is never installed from here
    let r = local.command(Command::InstallPacks { names: vec!["tidal-drum-machines".into()] });
    assert!(!r.ok && r.message.contains("--accept-licence"), "{}", r.message);
    assert!(local.state().busy.is_none());
}

async fn call(router: &axum::Router, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn the_web_studio_comes_compressed_and_its_hashed_files_cached() {
    let web = std::env::temp_dir().join(format!("claudius-web-{}", std::process::id()));
    std::fs::create_dir_all(&web).unwrap();
    std::fs::write(web.join("index.html"), "<!doctype html><title>studio</title>".repeat(20)).unwrap();
    let wasm = "claudius-ui-ee34c356f0e05a_bg.wasm";
    std::fs::write(web.join(wasm), vec![0u8; 4096]).unwrap();
    std::fs::write(web.join("take.wav"), vec![0u8; 4096]).unwrap();
    let router = http::router(app(), &Options { web: Some(web.clone()), ..Default::default() });
    let get = |path: &str| {
        Request::get(path)
            .header(header::HOST, "localhost")
            .header(header::ACCEPT_ENCODING, "br, gzip")
            .body(Body::empty())
            .unwrap()
    };
    let res = router.clone().oneshot(get(&format!("/{wasm}"))).await.unwrap();
    assert_eq!(res.headers()[header::CONTENT_ENCODING], "br");
    assert_eq!(res.headers()[header::CACHE_CONTROL], "public, max-age=31536000, immutable");
    let res = router.clone().oneshot(get("/")).await.unwrap();
    assert_eq!(res.headers()[header::CONTENT_ENCODING], "br");
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
    let res = router.clone().oneshot(get("/take.wav")).await.unwrap();
    assert!(res.headers().get(header::CONTENT_ENCODING).is_none(), "audio is sent as it is");
    let res = router.clone().oneshot(get("/api/hello")).await.unwrap();
    assert!(res.headers().get(header::CACHE_CONTROL).is_none(), "the API says nothing about caching");
    std::fs::remove_dir_all(&web).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_http_api_guards_itself() {
    let router = http::router(app(), &Options::default());
    let get = |path: &str, host: &str| Request::get(path).header(header::HOST, host).body(Body::empty()).unwrap();
    let (s, h, b) = call(&router, get("/api/hello", "127.0.0.1:7878")).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(b.contains("\"claudius\""));
    assert_eq!(h.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    assert_eq!(h.get(header::X_FRAME_OPTIONS).unwrap(), "DENY");
    let (s, _, _) = call(&router, get("/api/hello", "evil.example")).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let post = Request::post("/api/command")
        .header(header::HOST, "localhost:7878")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(r#"{"cmd":"undo"}"#))
        .unwrap();
    let (s, _, _) = call(&router, post).await;
    assert_eq!(s, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let post = Request::post("/api/command")
        .header(header::HOST, "localhost:7878")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"cmd":"nope"}"#))
        .unwrap();
    let (s, _, b) = call(&router, post).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, _, b) = call(&router, get("/api/state", "localhost")).await;
    assert_eq!(s, StatusCode::OK);
    let st: State = serde_json::from_str(&b).unwrap();
    assert!(!st.genres.is_empty());

    let locked = http::router(app(), &Options { token: Some("s3cret".into()), ..Default::default() });
    let (s, _, _) = call(&locked, get("/api/hello", "localhost")).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, h, _) = call(&locked, get("/api/hello?token=s3cret", "localhost")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h.get(header::SET_COOKIE).is_some());
    let (s, _, _) = call(
        &locked,
        Request::get("/api/hello")
            .header(header::HOST, "localhost")
            .header(header::AUTHORIZATION, "Bearer s3cret")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = call(&locked, get("/api/hello?token=", "localhost")).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    // a blank token leaves the server shut, not open, and serve refuses to start with one
    let blank = http::router(app(), &Options { public: true, token: Some(String::new()), ..Default::default() });
    let (s, _, _) = call(&blank, get("/api/hello?token=", "evil.example")).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(Options { token: Some(" ".into()), ..Default::default() }.check().is_err());
    assert!(Options { public: true, ..Default::default() }.check().is_err());
    assert!(Options { public: true, token: Some("s3cret".into()), ..Default::default() }.check().is_ok());
}

fn sse_json(body: &str) -> serde_json::Value {
    let line =
        body.lines().filter_map(|l| l.strip_prefix("data: ")).find(|l| l.trim_start().starts_with('{')).unwrap_or(body);
    serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {body}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn assistants_work_over_mcp() {
    let router = http::router(app(), &Options::default());
    let mcp = |body: serde_json::Value, session: Option<&str>| {
        let mut r = Request::post("/mcp")
            .header(header::HOST, "127.0.0.1:7878")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream");
        if let Some(s) = session {
            r = r.header("mcp-session-id", s);
        }
        r.body(Body::from(body.to_string())).unwrap()
    };
    let init = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}});
    let (s, h, b) = call(&router, mcp(init, None)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(sse_json(&b)["result"]["serverInfo"]["name"], "claudius");
    let session = h.get("mcp-session-id").and_then(|v| v.to_str().ok()).map(String::from);
    let sid = session.as_deref();
    call(&router, mcp(serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}), sid)).await;

    let (_, _, b) =
        call(&router, mcp(serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}), sid)).await;
    let tools: Vec<String> = sse_json(&b)["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for t in [
        "new_song",
        "get_song",
        "run_script",
        "listen",
        "mutate",
        "evolve",
        "undo",
        "script_reference",
        "save",
        "export",
        "publish",
    ] {
        assert!(tools.iter().any(|x| x == t), "{t} missing from {tools:?}");
    }

    let call_tool = |id: u32, name: &str, args: serde_json::Value| {
        mcp(
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}),
            sid,
        )
    };
    let (_, _, b) = call(&router, call_tool(3, "new_song", serde_json::json!({"genre": "techno", "seed": 2}))).await;
    let r = sse_json(&b);
    let text = r["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
    assert_ne!(r["result"]["isError"], true, "{b}");
    assert!(text.contains("techno") && text.contains("verdict"), "{text}");

    // checked for release, it has no artwork, and nobody has said how it was made
    let (_, _, b) = call(&router, call_tool(99, "publish", serde_json::json!({"artist": "Ana Lima"}))).await;
    let text = sse_json(&b)["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
    assert!(
        text.contains("problem(s)") && text.contains("-- artwork") && text.contains("how the song was made"),
        "{text}"
    );

    let (_, _, b) = call(
        &router,
        call_tool(4, "run_script", serde_json::json!({"code": "track(\"x\", s(\"nope*4\"))", "listen": false})),
    )
    .await;
    let r = sse_json(&b);
    assert_eq!(r["result"]["isError"], true, "{b}");
    assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("unknown sound 'nope'"));

    let (_, _, b) = call(&router, call_tool(5, "get_song", serde_json::json!({}))).await;
    let text = sse_json(&b)["result"]["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("script:") && text.contains("form(["), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn saves_during_a_render_wait_for_it_and_write_everything() {
    let app = app();
    let base = std::env::temp_dir().join(format!("claudius-save-{}", std::process::id()));
    let dirs = [base.join("a"), base.join("b")];
    let _ = std::fs::remove_dir_all(&base);
    let new = Command::NewSong { genre: "lofi".into(), seed: Some(2), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    // the new song's render is running: both saves wait for it instead of being dropped or replacing each other
    assert!(app.state().busy.is_some());
    for dir in &dirs {
        let r = app.command(Command::Save { path: Some(dir.display().to_string()) });
        assert!(r.ok && r.message.contains("once the current job"), "{}", r.message);
        assert!(dir.join("blueprint.json").is_file() && dir.join("song.rhai").is_file());
    }
    let st = app.settled(Duration::from_secs(240)).await;
    assert_eq!(st.log.iter().filter(|l| l.starts_with("saved ")).count(), 2, "{:?}", st.log);
    for dir in &dirs {
        for file in ["song.wav", "project.json", "verdict.json", "report.json", "song.rhai", "blueprint.json"] {
            assert!(dir.join(file).is_file(), "{file} is missing in {}", dir.display());
        }
        assert!(std::fs::read_dir(dir.join("stems")).is_ok_and(|d| d.count() > 3));
    }
    std::fs::remove_dir_all(&base).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_render_exports_by_command_and_as_a_download() {
    let app = app();
    let new = Command::NewSong { genre: "techno".into(), seed: Some(5), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    let st = app.settled(Duration::from_secs(240)).await;
    let v = st.render.as_ref().expect("a render").version;
    assert!(st.exports.iter().any(|f| f.spec == "flac:16" && f.label == "FLAC, 16-bit, 44.1 kHz" && f.lossless));

    // by command, into a folder
    let dir = std::env::temp_dir().join(format!("claudius-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let r = app.command(Command::Export {
        formats: vec!["mp3:192".into(), "opus".into()],
        path: Some(dir.display().to_string()),
    });
    assert!(r.ok, "{}", r.message);
    let mut files: Vec<String> =
        std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert!(files.len() == 2 && files[0].ends_with(".mp3") && files[1].ends_with(".opus"), "{files:?}");
    let r = app.command(Command::Export { formats: vec!["mp3:300".into()], path: Some(dir.display().to_string()) });
    assert!(!r.ok && r.message.contains("MP3 takes"), "{}", r.message);

    // as a download, named after the address
    let router = http::router(app.clone(), &Options::default());
    let get = |path: String| Request::get(path).header(header::HOST, "localhost").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(get(format!("/api/export/{v}/night-bus.flac?format=flac:16"))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "audio/flac");
    assert_eq!(res.headers()[header::CONTENT_DISPOSITION], "attachment; filename=\"night-bus.flac\"");
    let body = axum::body::to_bytes(res.into_body(), 1 << 28).await.unwrap();
    let d = claudius_decode::decode_bytes(body.to_vec(), "flac").unwrap();
    assert_eq!((d.rate, d.channels), (44_100, 2));
    let (s, _, b) = call(&router, get(format!("/api/export/{v}/x.mp3?format=flac"))).await;
    assert!(s == StatusCode::BAD_REQUEST && b.contains("makes .flac files, not .mp3"), "{s} {b}");
    let (s, _, _) = call(&router, get(format!("/api/export/{}/x.mp3", v + 1000))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    std::fs::remove_dir_all(&dir).ok();
}

/// a square cover with hard edges, as a PNG
fn cover(side: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(side, side, |x, y| {
        let v = if (x / 120 + y / 120).is_multiple_of(2) { 40 } else { 210 };
        image::Rgb([v, 90, 255 - v])
    });
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn a_release_is_checked_and_its_kit_written_and_kept_with_the_song() {
    let app = app();
    let new = Command::NewSong { genre: "lofi".into(), seed: Some(6), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    // a folder of the test's own: a song without one would release into ~/Music/claudius
    let dir = std::env::temp_dir().join(format!("claudius-release-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(app.command(Command::Save { path: Some(dir.display().to_string()) }).ok);
    let st = app.settled(Duration::from_secs(240)).await;
    let release = st.release.expect("a release for the song");
    assert_eq!(release.song_title, st.song.as_ref().unwrap().title);
    assert!(release.made_options.iter().any(|(m, _)| m == "assisted"));
    assert_eq!(release.checked, 0, "renders check a release only once something is given for it");
    assert!(!dir.join("release.json").exists(), "nor does saving the song keep one");

    let details = claudius_api::ReleaseDetails {
        artist: "Ana Lima".into(),
        writers: vec!["Ana Lima".into()],
        made: Some("assisted".into()),
        ..release.details
    };
    let r = app.command(Command::SetRelease { details: Box::new(details), artwork: None });
    assert!(r.ok, "{}", r.message);
    let release = app.state().release.unwrap();
    assert!(!release.ready && release.checked > 0);
    let problems: Vec<&str> =
        release.checks.iter().filter(|c| c.level == "problem").map(|c| c.about.as_str()).collect();
    assert_eq!(problems, ["artwork"], "{:?}", release.checks);
    let r = app.command(Command::WriteKit { path: None });
    assert!(!r.ok && r.message.contains("add the artwork"), "{}", r.message);

    // the artwork comes as the file itself
    let router = http::router(app.clone(), &Options::default());
    let post = |content_type: &str| {
        Request::post("/api/release/artwork")
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(cover(3000)))
            .unwrap()
    };
    let (s, _, _) = call(&router, post("text/plain")).await;
    assert_eq!(s, StatusCode::UNSUPPORTED_MEDIA_TYPE, "a cross-site form cannot send artwork");
    let (s, _, b) = call(&router, post("image/png")).await;
    assert!(s == StatusCode::OK && b.contains("3000 × 3000 PNG"), "{s} {b}");
    let release = app.state().release.unwrap();
    assert!(release.ready, "{:?}", release.checks);
    let get = |path: &str| Request::get(path).header(header::HOST, "localhost").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(get("/api/release/preview.png")).await.unwrap();
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/png");
    let png = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
    assert_eq!(image::load_from_memory(&png).unwrap().width(), 512);

    // the kit goes into the song's folder, and the details and the artwork stay with the song
    let r = app.command(Command::WriteKit { path: None });
    assert!(r.ok, "{}", r.message);
    assert!(dir.join("release/release.txt").is_file() && dir.join("artwork.png").is_file());
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("release.json")).unwrap()).unwrap();
    assert_eq!((kept["made"].as_str(), kept["artwork"].as_str()), (Some("assisted"), Some("artwork.png")));
    assert!(app.state().release.unwrap().kit.is_some_and(|k| k.ends_with("release")));

    // another song starts afresh, and opening this one again brings its release back
    let new = Command::NewSong { genre: "house".into(), seed: Some(3), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    assert_eq!(app.state().release.unwrap().artwork, None);
    assert!(app.command(Command::Load { path: dir.display().to_string() }).ok);
    let release = app.state().release.unwrap();
    assert_eq!(release.details.artist, "Ana Lima");
    assert_eq!(release.artwork.as_deref(), Some("3000 × 3000 PNG"));
    app.settled(Duration::from_secs(240)).await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_take_sent_over_http_plays_and_goes_with_the_song() {
    let app = app();
    let new = Command::NewSong { genre: "house".into(), seed: Some(5), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    // a short song: each save renders the stems of every track
    let short = r#"form([sec("intro", 4, "*"), sec("drop", 4, "*"), sec("outro", 4, "*")]);"#;
    let r = app.command(Command::RunScript { code: short.into() });
    assert!(r.ok, "{}", r.message);
    // folders of the test's own: a song without one keeps its takes in ~/Music/claudius
    let scratch = |what: &str| std::env::temp_dir().join(format!("claudius-takes-{what}-{}", std::process::id()));
    let (dir, other, moved) = (scratch("song"), scratch("saved"), scratch("moved"));
    for d in [&dir, &other, &moved] {
        let _ = std::fs::remove_dir_all(d);
    }
    assert!(app.command(Command::Save { path: Some(dir.display().to_string()) }).ok);
    let st = app.settled(Duration::from_secs(240)).await;
    let from = st.song.unwrap().sections[1].name.clone();

    // two seconds of a tone, sent as the studio sends a take: the WAV file itself
    let n = 2 * claudius::config::SR as usize;
    let l: Vec<f32> = (0..n).map(|i| (i as f32 * 0.03).sin() * 0.4).collect();
    let wav = dir.join("take.wav");
    claudius::audio::save_wav(&wav, &claudius::audio::Stereo { l: l.clone(), r: l }, true).unwrap();
    let bytes = std::fs::read(&wav).unwrap();
    std::fs::remove_file(&wav).unwrap();
    let router = http::router(app.clone(), &Options::default());
    let post = |content_type: &str, query: &str| {
        Request::post(format!("/api/takes?{query}"))
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes.clone()))
            .unwrap()
    };
    let (s, _, _) = call(&router, post("text/plain", &format!("from={from}"))).await;
    assert_eq!(s, StatusCode::UNSUPPORTED_MEDIA_TYPE, "a cross-site form cannot send a take");
    let (s, _, b) = call(&router, post("audio/wav", "from=nowhere")).await;
    assert!(s == StatusCode::OK && b.contains("no section 'nowhere'"), "{s} {b}");
    let (s, _, b) = call(&router, post("audio/wav", &format!("from={from}&name=vox&role=vocal"))).await;
    assert!(s == StatusCode::OK && b.contains("vox: a take of"), "{s} {b}");
    let song = app.state().song.unwrap();
    let vox = song.tracks.iter().find(|t| t.name == "vox").expect("the take is a track");
    assert_eq!((vox.kind.as_str(), vox.role.as_str()), ("take", "vocal"));
    assert!(song.sections[1].layers.iter().any(|l| l == "vox"));
    let kept: Vec<_> = std::fs::read_dir(dir.join("takes")).unwrap().flatten().collect();
    assert_eq!(kept.len(), 1, "kept in the song's folder");
    let st = app.settled(Duration::from_secs(240)).await;
    let vox = st.song.unwrap().tracks.into_iter().find(|t| t.name == "vox").unwrap();
    assert!(vox.loudness.is_some_and(|l| l > -40.0), "it plays: {:?}", vox.loudness);

    // the take is one held note (229 Hz): as notes, a part plays it, and the take goes quiet
    let r = app.command(Command::TranscribeTake {
        take: "vox".into(),
        sounds: vec!["pluck".into()],
        name: None,
        role: None,
        chromatic: false,
        keep_take: false,
        style: None,
    });
    assert!(r.ok && r.message.starts_with("1 note from vox"), "{}", r.message);
    // how it is sung: one note, 29 cents under A#3, by the take's name or as the file kept for it, in a key
    let hear = |take: Option<&str>, path: Option<String>, key: Option<&str>| {
        app.command(Command::AssessVoice { take: take.map(String::from), path, key: key.map(String::from) })
    };
    let r = hear(Some("vox"), None, None);
    assert!(
        r.ok && r.message.starts_with("1 note, 0% within 25 cents of the note meant and 100% within 50"),
        "{}",
        r.message
    );
    assert!(r.message.contains("furthest off: A#3 at 0.0 s (-29 cents)"), "{}", r.message);
    let file = kept[0].path().display().to_string();
    assert!(hear(None, Some(file.clone()), Some("A# minor")).ok);
    for (r, says) in [
        (hear(Some("nope"), None, None), "no take called 'nope' (takes: vox)"),
        (hear(None, Some(file), Some("H minor")), "cannot read the key 'H minor'"),
        (hear(None, None, None), "name a take of the song or an audio file to hear"),
    ] {
        assert!(!r.ok && r.message.starts_with(says), "{}", r.message);
    }
    // tuned hard, first in its chain, and the song renders with it; then untuned
    let tune = |track: &str, speed: Option<f32>, scale: Option<&str>, off: bool| Command::Tune {
        track: track.into(),
        strength: None,
        speed_ms: speed,
        humanize: speed.map(|_| 0.0),
        vibrato: speed.map(|_| 0.0),
        scale: scale.map(String::from),
        off,
    };
    let muted = |m: bool| Command::SetMix {
        track: "vox".into(),
        level_db: None,
        pan: None,
        reverb: None,
        delay: None,
        duck: None,
        drive_db: None,
        muted: Some(m),
        solo: None,
        bus: None,
    };
    assert!(app.command(muted(false)).ok);
    let r = app.command(tune("vox", Some(0.0), None, false));
    assert!(
        r.ok && r.message.starts_with("vox: tuned to the song's chords, each note brought on at once, keeping 0 cents"),
        "{}",
        r.message
    );
    let st = app.settled(Duration::from_secs(240)).await;
    let vox = st.song.unwrap().tracks.into_iter().find(|t| t.name == "vox").unwrap();
    assert_eq!(vox.fx[0]["fx"], "tune");
    assert_eq!((vox.fx[0]["speed_ms"].as_f64(), vox.fx[0]["scale"].as_str()), (Some(0.0), Some("chords")));
    assert!(vox.loudness.is_some_and(|l| l > -40.0), "it plays, tuned: {:?}", vox.loudness);
    let r = app.command(tune("vox", None, Some("C D E"), false));
    assert!(
        r.ok && r.message.contains("tuned to C D E, each note brought on at once"),
        "the rest as it was: {}",
        r.message
    );
    for (r, says) in [
        (app.command(tune("vox", None, Some("C H"), false)), "a tuner goes to chords, key, chromatic or notes"),
        (app.command(tune("nope", None, None, false)), "no track 'nope'"),
    ] {
        assert!(!r.ok && r.message.starts_with(says), "{}", r.message);
    }
    assert!(app.command(tune("vox", None, None, true)).message.starts_with("vox: tuned no longer"));
    let vox = app.state().song.unwrap().tracks.into_iter().find(|t| t.name == "vox").unwrap();
    assert!(vox.fx.iter().all(|e| e["fx"] != "tune"), "{:?}", vox.fx);
    assert!(app.command(muted(true)).ok, "quiet again, as its notes left it");
    let song = app.state().song.unwrap();
    let melody = song.tracks.iter().find(|t| t.name == "melody").expect("a part for the melody");
    assert_eq!(melody.kind, "part");
    assert!(song.tracks.iter().any(|t| t.name == "vox" && t.muted));
    app.settled(Duration::from_secs(240)).await;

    // a chord played on a keyboard, from the first section: a part of its own
    let first = song.sections[0].name.clone();
    let chord: Vec<claudius_api::NoteView> =
        [60, 64, 67].iter().map(|&p| claudius_api::NoteView { t: 0.0, d: 4.0, p, v: 0.8 }).collect();
    let add = Command::AddNotes {
        from: first,
        notes: chord,
        sounds: vec!["epiano".into()],
        name: None,
        role: Some("keys".into()),
        style: None,
        request: None,
    };
    let r = app.command(add);
    assert!(r.ok && r.message.starts_with("3 notes from the start of intro"), "{}", r.message);
    assert!(app.state().song.unwrap().tracks.iter().any(|t| t.name == "played" && t.role == "keys"));
    app.settled(Duration::from_secs(240)).await;

    // saved elsewhere, the song takes its take along; that folder moved, opening it finds the take there
    assert!(app.command(Command::Save { path: Some(other.display().to_string()) }).ok);
    app.settled(Duration::from_secs(240)).await;
    assert_eq!(std::fs::read_dir(other.join("takes")).unwrap().count(), 1);
    std::fs::rename(&other, &moved).unwrap();
    let r = app.command(Command::Load { path: moved.display().to_string() });
    assert!(r.ok, "{}", r.message);
    let bp = app.blueprint().unwrap();
    let files = bp.files();
    assert!(files.iter().any(|f| std::path::Path::new(f).starts_with(&moved)), "{files:?}");
    app.settled(Duration::from_secs(240)).await;
    for d in [&dir, &moved] {
        std::fs::remove_dir_all(d).ok();
    }
}

#[tokio::test]
async fn files_a_studio_read_open_and_are_listened_to_by_their_contents() {
    let app = app();
    let new = |genre: &str, seed| Command::NewSong {
        genre: genre.into(),
        seed: Some(seed),
        bpm: None,
        key: None,
        loops: false,
    };
    assert!(app.command(new("house", 5)).ok);
    let title = app.state().song.unwrap().title;
    // a song's own files, as a studio reads them on its computer: its blueprint and its script
    let dir = std::env::temp_dir().join(format!("claudius-open-text-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(app.command(Command::Save { path: Some(dir.display().to_string()) }).ok);
    let blueprint = std::fs::read_to_string(dir.join("blueprint.json")).unwrap();
    let script = std::fs::read_to_string(dir.join("song.rhai")).unwrap();
    assert!(app.command(new("techno", 9)).ok);
    let r = app.command(Command::OpenText { name: "blueprint.json".into(), text: blueprint });
    assert!(r.ok && r.message.contains("opened blueprint.json"), "{}", r.message);
    assert_eq!(app.state().song.unwrap().title, title, "the blueprint opens the song it holds");
    // a script runs on the open song, as a script file that `load` opens does
    assert!(app.command(new("house", 6)).ok);
    let r = app.command(Command::OpenText { name: "Song.RHAI".into(), text: script });
    assert!(r.ok && r.message.contains("opened Song.RHAI"), "{}", r.message);
    let r = app.command(Command::OpenText { name: "notes.JSON".into(), text: "{ not a song".into() });
    assert!(!r.ok && r.message.starts_with("cannot open notes.JSON"), "{}", r.message);
    let _ = std::fs::remove_dir_all(&dir);

    // eight seconds of a beat at 120 BPM, sent as the file itself: listened to and remade, nothing of it kept
    let sr = claudius::config::SR as usize;
    let l: Vec<f32> = (0..8 * sr)
        .map(|i| {
            let t = (i % (sr / 2)) as f32 / sr as f32;
            (std::f32::consts::TAU * 60.0 * t).sin() * (-t * 18.0).exp() * 0.8
        })
        .collect();
    let file = std::env::temp_dir().join(format!("claudius-beat-{}.wav", std::process::id()));
    claudius::audio::save_wav(&file, &claudius::audio::Stereo { l: l.clone(), r: l }, true).unwrap();
    let wav = std::fs::read(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    let router = http::router(app.clone(), &Options::default());
    let post = |path: &str, content_type: &str, bytes: &[u8]| {
        Request::post(path)
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes.to_vec()))
            .unwrap()
    };
    let (s, _, _) = call(&router, post("/api/analyze?name=beat.wav", "text/plain", &wav)).await;
    assert_eq!(s, StatusCode::UNSUPPORTED_MEDIA_TYPE, "a cross-site form cannot send a recording");
    let (s, _, b) = call(&router, post("/api/analyze?name=beat.wav", "audio/wav", &wav)).await;
    let reply: claudius_api::Reply = serde_json::from_str(&b).unwrap();
    assert!(s == StatusCode::OK && reply.ok && reply.message.contains(" BPM (pulse clarity"), "{s} {b}");
    // a file without a name says its format by its first bytes; one that is not audio says what is wanted
    let (_, _, b) = call(&router, post("/api/analyze", "application/octet-stream", &wav)).await;
    assert!(serde_json::from_str::<claudius_api::Reply>(&b).unwrap().ok, "{b}");
    let (_, _, b) = call(&router, post("/api/analyze", "application/octet-stream", b"hello")).await;
    assert!(b.contains("send the recording as the file itself"), "{b}");
    let (_, _, b) = call(&router, post("/api/analyze?name=notes.txt", "application/octet-stream", b"hello")).await;
    assert!(b.contains("cannot read notes.txt"), "{b}");
    let (s, _, b) = call(&router, post("/api/remake?name=beat.wav&genre=techno&seed=3", "audio/wav", &wav)).await;
    let reply: claudius_api::Reply = serde_json::from_str(&b).unwrap();
    assert!(s == StatusCode::OK && reply.ok && reply.message.starts_with("remade beat.wav as techno"), "{b}");
    let song = app.state().song.unwrap();
    assert_eq!((song.genre.as_str(), song.seed), ("techno", 3));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_take_an_assistant_asks_for_is_answered_by_the_take_sent_for_it() {
    use claudius_api::TakeStatus;
    let app = app();
    let ask = |from: &str, role: Option<&str>, note: &str| Command::RequestTake {
        from: from.into(),
        role: role.map(String::from),
        name: None,
        note: note.into(),
    };
    let r = app.command(ask("drop", None, ""));
    assert!(!r.ok && r.message.contains("no song yet"), "{}", r.message);
    let new = Command::NewSong { genre: "house".into(), seed: Some(5), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    // a folder of the test's own, for the take it sends: a song without one keeps its takes in ~/Music/claudius
    let short = r#"form([sec("intro", 4, "*"), sec("drop", 4, "*"), sec("outro", 4, "*")]);"#;
    assert!(app.command(Command::RunScript { code: short.into() }).ok);
    let folder = std::env::temp_dir().join(format!("claudius-asked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    assert!(app.command(Command::Save { path: Some(folder.display().to_string()) }).ok);
    app.settled(Duration::from_secs(240)).await;
    let from = app.state().song.unwrap().sections[1].name.clone();
    assert!(app.command(ask("nowhere", None, "")).message.contains("no section 'nowhere'"));
    assert!(app.command(ask(&from, Some("kazoo"), "")).message.contains("not 'kazoo'"));

    // asked, and the person says not now: once
    let r = app.command(ask(&from, Some("vocal"), "the hook, softly"));
    assert!(r.ok && r.message.contains("(request 1)"), "{}", r.message);
    let t = app.state().take_request.unwrap();
    assert_eq!(
        (t.id, t.status, t.note.as_str(), t.role.as_deref()),
        (1, TakeStatus::Waiting, "the hook, softly", Some("vocal"))
    );
    assert!(app.command(Command::CancelTakeRequest { id: 1 }).ok);
    assert_eq!(app.state().take_request.unwrap().status, TakeStatus::Cancelled);
    assert!(app.command(Command::CancelTakeRequest { id: 1 }).message.contains("answered already"));
    assert!(app.command(Command::CancelTakeRequest { id: 7 }).message.contains("no take request 7"));

    // asked again, and answered by the notes played for it, once claudius keeps them
    assert!(app.command(ask(&from, Some("keys"), "")).ok);
    let notes = |from: &str| Command::AddNotes {
        from: from.into(),
        notes: vec![claudius_api::NoteView { t: 0.0, d: 1.0, p: 60, v: 0.8 }],
        sounds: vec!["epiano".into()],
        name: None,
        role: Some("keys".into()),
        style: None,
        request: Some(2),
    };
    assert!(!app.command(notes("nowhere")).ok);
    assert_eq!(app.state().take_request.unwrap().status, TakeStatus::Waiting, "not by notes claudius refused");
    let r = app.command(notes(&from));
    assert!(r.ok, "{}", r.message);
    let t = app.state().take_request.unwrap();
    assert_eq!((t.id, t.status, t.answer.as_deref()), (2, TakeStatus::Taken, Some(r.message.as_str())));

    // an assistant asks over MCP, waits, and has the take the studio sent for it
    let router = http::router(app.clone(), &Options::default());
    let mcp = |body: serde_json::Value, session: Option<&str>| {
        let mut r = Request::post("/mcp")
            .header(header::HOST, "127.0.0.1:7878")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream");
        if let Some(s) = session {
            r = r.header("mcp-session-id", s);
        }
        r.body(Body::from(body.to_string())).unwrap()
    };
    let init = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}});
    let (_, h, _) = call(&router, mcp(init, None)).await;
    let session = h.get("mcp-session-id").and_then(|v| v.to_str().ok()).map(String::from);
    let sid = session.as_deref();
    call(&router, mcp(serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}), sid)).await;
    let tool = |id: u32, name: &str, args: serde_json::Value| {
        mcp(
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}),
            sid,
        )
    };
    let text = |b: &str| sse_json(b)["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
    let (_, _, b) =
        call(&router, tool(2, "request_take", serde_json::json!({"from": from, "role": "vocal", "note": "the hook"})))
            .await;
    let asked = text(&b);
    assert!(
        asked.contains("(request 3)") && asked.contains("/?record=3") && asked.contains("wait_for_take"),
        "{asked}"
    );
    let (_, _, b) = call(&router, tool(3, "wait_for_take", serde_json::json!({"id": 3, "seconds": 1}))).await;
    assert!(text(&b).contains("has not sent the take yet (request 3)"), "{b}");

    // two seconds of a tone, sent as the studio sends a take recorded for the request, while the assistant waits
    let n = 2 * claudius::config::SR as usize;
    let l: Vec<f32> = (0..n).map(|i| (i as f32 * 0.03).sin() * 0.4).collect();
    let file = std::env::temp_dir().join(format!("claudius-asked-{}.wav", std::process::id()));
    claudius::audio::save_wav(&file, &claudius::audio::Stereo { l: l.clone(), r: l }, true).unwrap();
    let wav = std::fs::read(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    let studio = router.clone();
    let query = format!("/api/takes?from={from}&role=vocal&request=3");
    let sent = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        let req = Request::post(query)
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, "audio/wav")
            .body(Body::from(wav))
            .unwrap();
        call(&studio, req).await
    });
    let (_, _, b) = call(&router, tool(4, "wait_for_take", serde_json::json!({"id": 3, "seconds": 60}))).await;
    let waited = text(&b);
    assert!(waited.starts_with("the take arrived: take1: a take of"), "{waited}");
    let (s, _, b) = sent.await.unwrap();
    assert!(s == StatusCode::OK && b.contains("take1: a take of"), "{s} {b}");
    assert_eq!(app.state().take_request.unwrap().status, TakeStatus::Taken);
    let (_, _, b) = call(&router, tool(5, "wait_for_take", serde_json::json!({"id": 2}))).await;
    assert!(text(&b).contains("gave way to request 3"), "{b}");
    assert_eq!(std::fs::read_dir(folder.join("takes")).unwrap().count(), 1, "the take is kept with the song");
    std::fs::remove_dir_all(&folder).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recordings_vocals_are_sampled_into_the_song_and_kept_with_it() {
    let app = app();
    let new = Command::NewSong { genre: "house".into(), seed: Some(5), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    let short = r#"form([sec("intro", 4, "*"), sec("drop", 8, "*"), sec("break", 4, "pad"), sec("drop", 8, "*")]);"#;
    assert!(app.command(Command::RunScript { code: short.into() }).ok);
    // a folder of the test's own: a song without one keeps what it samples in ~/Music/claudius
    let folder = std::env::temp_dir().join(format!("claudius-sampled-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    assert!(app.command(Command::Save { path: Some(folder.display().to_string()) }).ok);
    app.settled(Duration::from_secs(240)).await;
    // sixteen seconds at 120 BPM: a sung A3 in the middle from bar 3 to 6, a hat to the left all through
    let (sr, n) = (claudius::config::SR as usize, 16 * claudius::config::SR as usize);
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let mut phase = 0f64;
    for i in 0..n {
        if i % (sr / 4) < sr / 50 {
            l[i] += if i % 2 == 0 { 0.2 } else { -0.2 };
        }
        if (4 * sr..12 * sr).contains(&i) {
            let t = i as f64 / sr as f64;
            phase += std::f64::consts::TAU * 220.0 * (1.0 + 0.01 * (t * 5.0 * std::f64::consts::TAU).sin()) / sr as f64;
            let v = (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15) as f32;
            l[i] += v;
            r[i] += v;
        }
    }
    let file = folder.join("recording.wav");
    claudius::audio::save_wav(&file, &claudius::audio::Stereo { l, r }, true).unwrap();
    let ask = |sections: Vec<String>| Command::SampleSong {
        path: file.display().to_string(),
        count: Some(2),
        bars: Some(2),
        find: None,
        sections,
    };
    let r = app.command(ask(vec!["nowhere".into()]));
    assert!(!r.ok && r.message.contains("no section or kind 'nowhere'"), "{}", r.message);
    let r = app.command(ask(Vec::new()));
    assert!(r.ok, "{}", r.message);
    assert!(r.message.starts_with("vox1: bars ") && r.message.contains("% sung"), "{}", r.message);
    assert!(r.message.contains("in the drops and hooks (drop, drop2)"), "{}", r.message);
    let st = app.state();
    let song = st.song.unwrap();
    assert!(song.tracks.iter().any(|t| t.name == "vox1" && t.role == "vocal"), "{:?}", song.tracks);
    let drop = song.sections.iter().find(|s| s.name == "drop").unwrap();
    assert!(drop.layers.iter().any(|l| l == "vox1"));
    // the phrases are kept with the song, and the same recording sent as a file samples it again into the break
    let kept = std::fs::read_dir(folder.join("takes")).unwrap().count();
    assert!(kept >= 1, "{kept}");
    let router = http::router(app.clone(), &Options::default());
    let wav = std::fs::read(&file).unwrap();
    let req = Request::post("/api/sample?name=recording.wav&count=1&bars=2&sections=break")
        .header(header::HOST, "localhost")
        .header(header::CONTENT_TYPE, "audio/wav")
        .body(Body::from(wav))
        .unwrap();
    let (s, _, b) = call(&router, req).await;
    assert!(s == StatusCode::OK && b.contains("in the sections asked for (break)"), "{s} {b}");
    let song = app.state().song.unwrap();
    assert!(song.sections.iter().find(|s| s.name == "break").unwrap().layers.iter().any(|l| l.starts_with("vox")));
    // undone, the phrases go
    assert!(app.command(Command::Undo).ok && app.command(Command::Undo).ok);
    assert!(!app.state().song.unwrap().tracks.iter().any(|t| t.name.starts_with("vox")));
    std::fs::remove_dir_all(&folder).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn chains_of_effects_are_seen_and_set_as_the_graph_sets_them() {
    use serde_json::json;
    let app = app();
    let new = Command::NewSong { genre: "house".into(), seed: Some(5), bpm: None, key: None, loops: false };
    assert!(app.command(new).ok);
    // the song as the studio sees it carries every chain, as the song keeps it, and every track's bus
    let song = app.state().song.unwrap();
    assert!(song.mix.buses.iter().any(|(b, fx)| b == "drums" && !fx.is_empty()), "{:?}", song.mix.buses);
    assert!(!song.mix.master.is_empty() && song.mix.reverb["fx"] == "reverb" && song.mix.delay["fx"] == "delay");
    assert!(song.tracks.iter().all(|t| !t.bus.is_empty()));
    let template = |name: &str| {
        let (_, t) = claudius_api::EFFECT_TEMPLATES.iter().find(|(n, _)| *n == name).unwrap();
        serde_json::from_str::<serde_json::Value>(t).unwrap()
    };
    let set = |target: &str, fx: Vec<serde_json::Value>| app.command(Command::SetFx { target: target.into(), fx });
    // a track's chain, a bus of the song's own, a send's effect
    let r = set("track:lead", vec![json!({"fx": "saturate", "kind": "tape", "drive_db": 6}), template("parallel")]);
    assert!(r.ok && r.message == "track:lead: 2 effects", "{}", r.message);
    let lead = app.state().song.unwrap().tracks.into_iter().find(|t| t.name == "lead").unwrap();
    assert_eq!((lead.fx.len(), &lead.fx[1]["fx"]), (2, &json!("parallel")));
    let to_glue = Command::SetMix {
        track: "chords".into(),
        level_db: None,
        pan: None,
        reverb: None,
        delay: None,
        duck: None,
        drive_db: None,
        muted: None,
        solo: None,
        bus: Some("glue".into()),
    };
    assert!(app.command(to_glue).ok);
    assert!(set("bus:glue", vec![template("compressor")]).ok);
    assert!(set("send:delay", vec![template("delay")]).ok);
    let song = app.state().song.unwrap();
    assert_eq!(song.tracks.iter().find(|t| t.name == "chords").unwrap().bus, "glue");
    assert!(song.graph_text().contains("  glue bus -> compressor -18 dB, 3:1 -> master"), "{}", song.graph_text());
    // what is not a chain or an effect is refused, and says why
    let says = |target: &str, fx: Vec<serde_json::Value>, words: &str| {
        let r = set(target, fx);
        assert!(!r.ok && r.message.contains(words), "{target}: {}", r.message);
    };
    says("track:nobody", vec![], "no track 'nobody'");
    says("lead", vec![], "'lead' is no chain of effects");
    says("send:delay", vec![template("delay"), template("delay")], "the delay send feeds one effect, not 2");
    says("track:lead", vec![json!({"fx": "revrb"})], "track:lead's effect 1: unknown variant `revrb`");
    // every effect the studio offers to add is one claudius plays, by the name it shows
    for (name, t) in claudius_api::EFFECT_TEMPLATES {
        let e: claudius::fx::Effect = serde_json::from_str(t).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(e.name(), *name);
    }
    for name in claudius::fx::EFFECT_NAMES.iter().filter(|n| !["tape_stop", "stutter", "reverse"].contains(n)) {
        assert!(claudius_api::EFFECT_TEMPLATES.iter().any(|(n, _)| n == name), "no template for {name}");
    }
    // a chain set is one undo step
    assert!(app.command(Command::Undo).ok);
    assert!(app.state().song.unwrap().mix.delay != song.mix.delay);
}

#[test]
fn the_studio_offers_the_styles_the_engine_plays() {
    assert_eq!(claudius::take::STYLES, claudius_api::NOTE_STYLES);
}

#[test]
fn the_studio_reads_and_writes_steps_as_scripts_do() {
    use claudius::theory::rhythm::Hit;
    assert_eq!(
        (claudius::script::STEP_ACCENT, claudius::script::STEP_GHOST),
        (claudius_api::ACCENT, claudius_api::GHOST)
    );
    for i in 1..=100 {
        let v = i as f32 / 100.0;
        let (steps, _) = claudius::script::hits_to_steps(&[Hit::new(0.0, v)], 1);
        assert_eq!(steps.chars().next(), Some(claudius_api::step_char(v)), "velocity {v}");
    }
    for c in "Xxo123456789".chars() {
        let (hits, _) = claudius::script::steps_to_hits(&c.to_string(), 16);
        assert_eq!(hits.first().map(|h| h.v), claudius_api::step_velocity(c), "'{c}'");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pitch_check_sung_back_becomes_the_voice_tune_sings_as() {
    use claudius_api::TakeStatus;
    let home = std::env::temp_dir().join(format!("claudius-voice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let kept = home.join("voice.json");
    // a voice kept before is the session's from the start (one App only: each clears this process' renders)
    std::fs::create_dir_all(&home).unwrap();
    let before = claudius::voice::singer::Voice { notes: 3, ..Default::default() };
    std::fs::write(&kept, serde_json::to_string(&before).unwrap()).unwrap();
    let settings = Settings {
        library: false,
        ears: false,
        auto_render: false,
        file_roots: None,
        voice: Some(kept.clone()),
        samples: false,
        gather: false,
    };
    let app = App::new(settings);
    assert!(app.state().voice.is_some_and(|v| v.starts_with("range C3-C5")), "the kept voice");
    let r = app.command(Command::RequestPitchCheck { low: Some(60), high: Some(55), note: String::new() });
    assert!(!r.ok && r.message.contains("at least a fifth apart"), "{}", r.message);
    let r = app.command(Command::RequestPitchCheck { low: None, high: None, note: "so that I can tune you".into() });
    assert!(r.ok && r.message.contains("six phrases of four notes"), "{}", r.message);
    let check = app.state().pitch_check.expect("a pitch check");
    assert_eq!((check.notes.len(), check.answers.len(), check.status), (24, 6, TakeStatus::Waiting));
    assert!(check.notes.iter().all(|n| (57..=69).contains(&n.p)), "{:?}", check.notes);
    // sung back an octave down and 20 cents flat, a beat a note in each gap
    let (sr, beat) = (claudius::config::SR as f64, 60.0 / check.bpm);
    let mut l = vec![0f32; (check.beats * beat * sr) as usize];
    let mut phase = 0f64;
    for (i, (a, _)) in check.answers.iter().enumerate() {
        for k in 0..4 {
            let p = check.notes[i * 4 + k].p as f64 - 12.2;
            let at = ((a + k as f64) * beat * sr) as usize;
            for j in 0..(0.85 * beat * sr) as usize {
                let hz =
                    440.0 * 2f64.powf((p + 0.1 * (j as f64 / sr * std::f64::consts::TAU * 5.5).sin() - 69.0) / 12.0);
                phase += std::f64::consts::TAU * hz / sr;
                l[at + j] = ((phase.sin() + 0.5 * (2.0 * phase).sin()) * 0.3) as f32;
            }
        }
    }
    let file = home.join("answers.wav");
    claudius::audio::save_wav(&file, &claudius::audio::Stereo { l: l.clone(), r: l }, true).unwrap();
    let wav = std::fs::read(&file).unwrap();
    let router = http::router(app.clone(), &Options::default());
    let post = |query: String| {
        Request::post(format!("/api/pitch-check?{query}"))
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, "audio/wav")
            .body(Body::from(wav.clone()))
            .unwrap()
    };
    let (_, _, b) = call(&router, post("id=99".into())).await;
    assert!(b.contains("no pitch check 99"), "{b}");
    let (s, _, b) = call(&router, post(format!("id={}", check.id))).await;
    let reply: claudius_api::Reply = serde_json::from_str(&b).unwrap();
    assert!(s == StatusCode::OK && reply.ok, "{b}");
    assert!(reply.message.contains("of 24 notes sung back (1 octave below the notes played)"), "{}", reply.message);
    let st = app.state();
    assert_eq!(st.pitch_check.as_ref().map(|c| c.status), Some(TakeStatus::Taken));
    assert!(st.voice.as_deref().is_some_and(|v| v.contains("flat by ")), "{:?}", st.voice);
    let (_, _, b) = call(&router, post(format!("id={}", check.id))).await;
    assert!(b.contains("answered already"), "{b}");
    // the voice heard is kept in place of the one before
    let voice: claudius::voice::singer::Voice = serde_json::from_str(&std::fs::read_to_string(&kept).unwrap()).unwrap();
    assert!(voice.notes >= 20 && voice.bias.iter().all(|b| (b + 0.2).abs() < 0.08), "{voice:?}");
    // a take tuned from here on is sung as the check heard it
    assert!(
        app.command(Command::NewSong { genre: "house".into(), seed: Some(2), bpm: None, key: None, loops: false }).ok
    );
    let tune = Command::Tune {
        track: "kick".into(),
        strength: None,
        speed_ms: None,
        humanize: None,
        vibrato: None,
        scale: None,
        off: false,
    };
    let r = app.command(tune);
    assert!(r.ok && r.message.ends_with("as your pitch check heard you sing"), "{}", r.message);
    let kick = app.state().song.unwrap().tracks.into_iter().find(|t| t.name == "kick").unwrap();
    assert!(kick.fx[0]["voice"]["bias"].is_array(), "{:?}", kick.fx[0]);
    // one asked for and not sung now
    assert!(app.command(Command::RequestPitchCheck { low: None, high: None, note: String::new() }).ok);
    let id = app.state().pitch_check.unwrap().id;
    assert!(app.command(Command::CancelPitchCheck { id }).ok);
    assert_eq!(app.state().pitch_check.map(|c| c.status), Some(TakeStatus::Cancelled));
    assert!(!app.command(Command::CancelPitchCheck { id }).ok);
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_steps_trig_is_set_shown_kept_and_cleared() {
    use claudius_api::TrigView;
    let app = app();
    assert!(
        app.command(Command::NewSong { genre: "house".into(), seed: Some(3), bpm: None, key: None, loops: false }).ok
    );
    let lane = app.state().song.unwrap().drums.iter().find(|d| d.lane.contains("hat")).unwrap().lane.clone();
    let set = |trig: Option<TrigView>| Command::SetTrig { lane: lane.clone(), variant: "main".into(), step: 2, trig };
    let roll = TrigView { ratchet: 3, ramp: 1.0, cond: "1:2".into(), ..TrigView::default() };
    let r = app.command(set(Some(roll.clone())));
    assert!(r.ok && r.message.contains("only 1:2, 3 strokes swelling"), "{}", r.message);
    let shown = |app: &App| {
        let d = app.state().song.unwrap().drums.into_iter().find(|d| d.lane == lane).unwrap();
        d.trigs.into_iter().find(|(v, _)| v == "main").map(|(_, t)| t).unwrap_or_default()
    };
    assert_eq!(shown(&app), vec![(2, roll.clone())]);
    // a trig given whole replaces the one there: what is back at its default goes back
    let lighter = TrigView { prob: 0.5, ..TrigView::default() };
    assert!(app.command(set(Some(lighter.clone()))).ok);
    assert_eq!(shown(&app), vec![(2, lighter)]);
    assert!(app.command(set(Some(roll.clone()))).ok);
    // the view and the engine's trig are the same JSON, every setting set
    let every = TrigView {
        prob: 0.25,
        cond: "!last".into(),
        ratchet: 5,
        ramp: -0.5,
        flam: 12.0,
        nudge: -0.125,
        pitch: -7,
        decay: 0.5,
        tone: 0.75,
        pan: 0.5,
    };
    let engine: claudius::theory::rhythm::Trig = serde_json::from_value(serde_json::to_value(&every).unwrap()).unwrap();
    assert_eq!(serde_json::from_value::<TrigView>(serde_json::to_value(engine).unwrap()).unwrap(), every);
    // a step played softer keeps its trig; a trig of nothing clears it
    let softer = Command::SetDrumStep { lane: lane.clone(), variant: "main".into(), step: 2, velocity: 0.5 };
    assert!(app.command(softer).ok);
    assert_eq!(shown(&app), vec![(2, roll)]);
    assert!(app.command(set(None)).ok);
    assert!(shown(&app).is_empty());
    let bad = Command::SetTrig {
        lane: lane.clone(),
        variant: "main".into(),
        step: 2,
        trig: Some(TrigView { cond: "often".into(), ..TrigView::default() }),
    };
    assert!(app.command(bad).message.contains("not 'often'"));
    let r = app.command(Command::SetTrig { lane: "bongo".into(), variant: "main".into(), step: 0, trig: None });
    assert!(!r.ok && r.message.contains("no drum lane 'bongo'"), "{}", r.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lane_takes_its_own_time_and_a_recordings_groove() {
    let app = app();
    assert!(
        app.command(Command::NewSong { genre: "house".into(), seed: Some(3), bpm: None, key: None, loops: false }).ok
    );
    let lanes: Vec<String> = app.state().song.unwrap().drums.iter().map(|d| d.lane.clone()).collect();
    let hat = lanes.iter().find(|l| l.contains("hat")).unwrap().clone();
    let set = Command::SetLane {
        lane: hat.clone(),
        length: Some(12),
        speed: Some(0.75),
        swing: Some(0.62),
        groove: Some("dilla".into()),
        groove_amount: Some(0.5),
    };
    let r = app.command(set);
    assert!(
        r.ok && r.message.contains("12 steps round, at 0.75 speed, swing 0.62, in the dilla groove (50%)"),
        "{}",
        r.message
    );
    let lane = |app: &App| app.state().song.unwrap().drums.into_iter().find(|d| d.lane == hat).unwrap();
    let d = lane(&app);
    assert_eq!(
        (d.length, d.speed, d.swing, d.groove.as_deref(), d.groove_amount),
        (Some(12), 0.75, Some(0.62), Some("dilla"), 0.5)
    );
    assert_eq!(claudius::theory::rhythm::GROOVES, claudius_api::GROOVES);
    // a recording's groove: eight bars of sixteenths at 100 BPM, the off-beats 0.06 beats late, by path and as a file
    let (sr, spb) = (claudius::config::SR as f64, 0.6);
    let n = (8.0 * 4.0 * spb * sr) as usize + 4_800;
    let mut l = vec![0f32; n];
    for k in 0..8 * 16 {
        let b = k as f64 / 4.0 + if k % 2 == 1 { 0.06 } else { 0.0 };
        let at = (0.1 * sr + b * spb * sr) as usize;
        for i in 0..240.min(n - at) {
            l[at + i] += 0.5 * (-(i as f32) / 40.0).exp() * if i % 2 == 0 { 1.0 } else { -1.0 };
        }
    }
    let file = std::env::temp_dir().join(format!("claudius-swung-{}.wav", std::process::id()));
    claudius::audio::save_wav(&file, &claudius::audio::Stereo { l: l.clone(), r: l }, true).unwrap();
    let r = app.command(Command::ExtractGroove {
        path: file.display().to_string(),
        name: None,
        lanes: Vec::new(),
        bpm: None,
    });
    assert!(r.ok && r.message.contains("at 62% swing") && r.message.contains(&lanes.join(", ")), "{}", r.message);
    let swung = format!("claudius-swung-{}", std::process::id());
    assert_eq!(lane(&app).groove.as_deref(), Some(swung.as_str()), "every lane plays in it");
    assert_eq!(app.state().song.unwrap().grooves, vec![swung.clone()]);
    let r = app.command(Command::ExtractGroove {
        path: file.display().to_string(),
        name: Some("x".into()),
        lanes: vec!["bongo".into()],
        bpm: None,
    });
    assert!(!r.ok && r.message.contains("no drum lane 'bongo'"), "{}", r.message);
    let r = app.command(Command::ExtractGroove {
        path: file.display().to_string(),
        name: None,
        lanes: Vec::new(),
        bpm: Some(400.0),
    });
    assert!(!r.ok && r.message.contains("a groove's tempo is 40 to 250 BPM, not 400"), "{}", r.message);
    let router = http::router(app.clone(), &Options::default());
    let wav = std::fs::read(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    let post = |query: &str, kind: &str, bytes: Vec<u8>| {
        Request::post(format!("/api/groove?{query}"))
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, kind)
            .body(Body::from(bytes))
            .unwrap()
    };
    let (s, _, _) = call(&router, post("name=loop.wav", "text/plain", wav.clone())).await;
    assert_eq!(s, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    // a tempo given, or written in the file's name, is the one it is measured at (here an octave off, to tell)
    let (s, _, b) =
        call(&router, post(&format!("name=Loop%20A.wav&lanes={hat}&bpm=50"), "audio/wav", wav.clone())).await;
    assert!(
        s == StatusCode::OK
            && b.contains("groove loop_a, heard in Loop A.wav at 50 BPM")
            && b.contains(&format!("; {hat} play in it")),
        "{b}"
    );
    assert_eq!(lane(&app).groove.as_deref(), Some("loop_a"));
    let (s, _, b) = call(&router, post("name=Loop%20B%20200bpm.wav", "audio/wav", wav)).await;
    let at: f64 = b.split(" at ").nth(1).and_then(|r| r.split(" BPM").next()).and_then(|n| n.parse().ok()).unwrap();
    assert!(s == StatusCode::OK && (at / 200.0 - 1.0).abs() < 0.02, "{b}");
    let silent = claudius::audio::Stereo::zeros(48_000 * 2);
    let quiet = std::env::temp_dir().join(format!("claudius-quiet-{}.wav", std::process::id()));
    claudius::audio::save_wav(&quiet, &silent, true).unwrap();
    let r = app.command(Command::ExtractGroove {
        path: quiet.display().to_string(),
        name: None,
        lanes: Vec::new(),
        bpm: None,
    });
    assert!(!r.ok && r.message.contains("too few drum hits"), "{}", r.message);
    std::fs::remove_file(&quiet).unwrap();
}

/// `bars` bars of a kit at `bpm`, from `lead` seconds in: a kick on 1 and 3, a snare on 2 and 4, a hat on every off-beat
fn beat(bars: usize, bpm: f64, lead: f64) -> claudius::audio::Stereo {
    use claudius::dsp::filter::butter;
    let sr = claudius::config::SR as f64;
    let spb = 60.0 / bpm;
    let n = ((lead + bars as f64 * 4.0 * spb) * sr) as usize + 24_000;
    let mut l = vec![0f32; n];
    let mut rng = claudius::rng::Rng::new(5);
    let fade = |i: usize, len: usize| ((len - i) as f32 / 480.0).min(1.0);
    let mut noise = |lo: f64, hi: f64, len: usize| {
        let mut x: Vec<f32> = (0..len).map(|_| rng.f32() * 2.0 - 1.0).collect();
        butter(&mut x, true, lo, sr, 4);
        butter(&mut x, false, hi, sr, 4);
        x
    };
    for b in 0..bars * 4 {
        let at = |beat: f64| ((lead + (b as f64 + beat) * spb) * sr) as usize;
        if b % 2 == 0 {
            let (len, mut ph) = (14_400, 0f32);
            for i in 0..len {
                let t = i as f32 / sr as f32;
                ph += std::f32::consts::TAU * (50.0 + 100.0 * (-t / 0.03).exp()) / sr as f32;
                l[at(0.0) + i] += 0.9 * ph.sin() * (-t / 0.15).exp() * fade(i, len);
            }
        } else {
            let x = noise(1000.0, 6000.0, 9_600);
            for (i, v) in x.iter().enumerate() {
                l[at(0.0) + i] += 0.6 * v * (-(i as f32 / sr as f32) / 0.08).exp() * fade(i, x.len());
            }
        }
        let x = noise(7000.0, 20_000.0, 4_800);
        for (i, v) in x.iter().enumerate() {
            l[at(0.5) + i] += 0.35 * v * (-(i as f32 / sr as f32) / 0.03).exp() * fade(i, x.len());
        }
    }
    claudius::audio::Stereo { l: l.clone(), r: l }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_beat_played_or_beatboxed_becomes_the_drum_lanes() {
    let app = app();
    let song = Command::NewSong { genre: "house".into(), seed: Some(3), bpm: Some(100.0), key: None, loops: false };
    assert!(app.command(song).ok);
    let from = app.state().song.unwrap().sections[0].name.clone();
    let file = std::env::temp_dir().join(format!("claudius-played-beat-{}.wav", std::process::id()));
    claudius::audio::save_wav(&file, &beat(4, 100.0, 0.005), true).unwrap();
    let take = Command::AddTake {
        path: file.display().to_string(),
        from,
        name: Some("beat".into()),
        role: Some("drums".into()),
    };
    let r = app.command(take);
    assert!(r.ok, "{}", r.message);
    let drums =
        |bars: Option<u32>, take: Option<&str>, path: Option<String>, variant: Option<&str>| Command::TranscribeDrums {
            take: take.map(String::from),
            path,
            bars,
            variant: variant.map(String::from),
            bpm: None,
            keep_take: false,
        };
    let r = app.command(drums(Some(1), Some("beat"), None, None));
    assert!(r.ok, "{}", r.message);
    assert!(r.message.contains("beat into drums, 1 bar as their main patterns"), "{}", r.message);
    assert!(
        r.message.contains("kick 2 steps into kick, snare 2 steps into clap, hat 4 steps into hat"),
        "{}",
        r.message
    );
    assert!(r.message.ends_with("; the take is muted"), "{}", r.message);
    let song = app.state().song.unwrap();
    let steps = |lane: &str, variant: &str| -> Vec<usize> {
        let d = song.drums.iter().find(|d| d.lane == lane).unwrap();
        let v = &d.variants.iter().find(|(n, _)| n == variant).unwrap().1;
        (0..16).filter(|&i| v[i] > 0.0).collect()
    };
    assert_eq!(
        (steps("kick", "main"), steps("clap", "main"), steps("hat", "main")),
        (vec![0, 8], vec![4, 12], vec![2, 6, 10, 14])
    );
    assert!(song.tracks.iter().find(|t| t.name == "beat").unwrap().muted, "the take goes quiet");
    // the same beat as a file: its tempo heard, its own downbeat, into a variant of its own
    let r = app.command(drums(Some(1), None, Some(file.display().to_string()), Some("played")));
    assert!(r.ok && r.message.contains("as their played patterns"), "{}", r.message);
    let song = app.state().song.unwrap();
    let played =
        |lane: &str| song.drums.iter().find(|d| d.lane == lane).unwrap().variants.iter().any(|(n, _)| n == "played");
    assert!(played("kick") && played("clap") && played("hat"));
    for (c, says) in [
        (drums(Some(3), Some("beat"), None, None), "a beat's pattern is 1, 2 or 4 bars, not 3"),
        (drums(None, Some("vox"), None, None), "no take called 'vox' (takes: beat)"),
        (drums(None, None, None, None), "transcribe_drums wants a take, or the path of a recording"),
    ] {
        let r = app.command(c);
        assert!(!r.ok && r.message.contains(says), "{}", r.message);
    }
    std::fs::remove_file(&file).unwrap();
}
