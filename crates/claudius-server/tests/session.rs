use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use claudius_api::{Command, State};
use claudius_server::{App, Options, Settings, http};
use tower::ServiceExt;

fn app() -> Arc<App> {
    App::new(Settings { library: false, ears: false, auto_render: true, file_roots: None })
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

#[tokio::test(flavor = "multi_thread")]
async fn a_shared_session_keeps_to_its_folders() {
    let inside = std::env::temp_dir().join(format!("claudius-roots-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("claudius-outside-{}", std::process::id()));
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("song.rhai"), "tempo(100);").unwrap();
    std::fs::write(inside.join("song.rhai"), "tempo(100);").unwrap();
    let app =
        App::new(Settings { library: false, ears: false, auto_render: false, file_roots: Some(vec![inside.clone()]) });
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
    std::fs::remove_dir_all(&inside).ok();
    std::fs::remove_dir_all(&outside).ok();
}

async fn call(router: &axum::Router, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
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
    for t in ["new_song", "get_song", "run_script", "listen", "mutate", "evolve", "undo", "script_reference"] {
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

#[test]
fn the_studio_reads_and_writes_steps_as_scripts_do() {
    use claudius::theory::rhythm::Hit;
    assert_eq!(
        (claudius::script::STEP_ACCENT, claudius::script::STEP_GHOST),
        (claudius_api::ACCENT, claudius_api::GHOST)
    );
    for i in 1..=100 {
        let v = i as f32 / 100.0;
        let (steps, _) = claudius::script::hits_to_steps(&[Hit { t: 0.0, v }], 1);
        assert_eq!(steps.chars().next(), Some(claudius_api::step_char(v)), "velocity {v}");
    }
    for c in "Xxo123456789".chars() {
        let (hits, _) = claudius::script::steps_to_hits(&c.to_string(), 16);
        assert_eq!(hits.first().map(|h| h.v), claudius_api::step_velocity(c), "'{c}'");
    }
}
