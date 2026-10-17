//! earshot command line (Rust port): plain-language summaries for agents, --json for programs.
//!
//!     earshot [--threads N] [--offline] <command> [args] [--json]

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use earshot::{Ears, EarsOptions, audio, describe, features, matching, realness::fmt_t, search};

const USAGE: &str = "earshot (Rust port of earshot): ears for code that cannot listen

usage: earshot [--threads N] [--offline] <command> [args] [--json]

commands:
  describe PATH [--top N]                      what does it sound like: calibrated tags, AudioSet labels, measurements
  match PATH QUERY... [--alts ALT...]          how well does a sound match a description (calibrated z + verdict)
  rank QUERY PATH... [--top N]                 rank sound files (or folders) by a description
  find PATH QUERY [--top N] [--kind auto|event|texture] [--win S] [--hop S]
                                               best moments for a description inside a recording (sample trims)
  measure FILE... [--music] [--light]          objective measurements (loudness, peaks, envelope, spectrum; no models)
  realness PATH...                             real recording / pro-produced vs code-synthesized
  grid PATH                                    tempo, beats, downbeats, pulse clarity (no models)
  compare PATH --refs REF... [-k K]            nearest references, typicality, feature differences
  doctor [--load]                              check ffmpeg, the model cache and settings (--load: time the models)

environment: EARSHOT_HOME (cache, default ~/.cache/earshot-rs), EARSHOT_THREADS, EARSHOT_OFFLINE=1,
             EARSHOT_FFMPEG / EARSHOT_FFPROBE (tool paths)";

/// tiny argument parser: positionals plus --flags (value-taking flags listed in `takes`, multi-value in `multi`)
struct Args {
    pos: Vec<String>,
    flags: std::collections::HashMap<String, Vec<String>>,
}

impl Args {
    fn parse(raw: &[String], takes: &[&str], multi: &[&str]) -> Result<Args> {
        let mut pos = Vec::new();
        let mut flags: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut i = 0;
        while i < raw.len() {
            let a = &raw[i];
            if a.starts_with('-') && a.len() > 1 && a.parse::<f64>().is_err() {
                let (name, inline) = match a.split_once('=') {
                    Some((n, v)) => (n.to_string(), Some(v.to_string())),
                    None => (a.clone(), None),
                };
                let e = flags.entry(name.clone()).or_default();
                if multi.contains(&name.as_str()) {
                    if let Some(v) = inline {
                        e.push(v);
                    }
                    while i + 1 < raw.len() && !(raw[i + 1].starts_with("--") || raw[i + 1] == "-k") {
                        i += 1;
                        e.push(raw[i].clone());
                    }
                } else if takes.contains(&name.as_str()) {
                    match inline {
                        Some(v) => e.push(v),
                        None => {
                            i += 1;
                            e.push(raw.get(i).cloned().with_context(|| format!("{name} needs a value"))?);
                        }
                    }
                } else {
                    e.push(String::new());
                }
            } else {
                pos.push(a.clone());
            }
            i += 1;
        }
        Ok(Args { pos, flags })
    }
    fn has(&self, f: &str) -> bool {
        self.flags.contains_key(f)
    }
    fn val(&self, f: &str) -> Option<&str> {
        self.flags.get(f).and_then(|v| v.last()).map(|s| s.as_str())
    }
    fn vals(&self, f: &str) -> Vec<String> {
        self.flags.get(f).cloned().unwrap_or_default()
    }
    fn num<T: std::str::FromStr>(&self, f: &str) -> Result<Option<T>> {
        match self.val(f) {
            None => Ok(None),
            Some(v) => v.parse().map(Some).map_err(|_| anyhow::anyhow!("{f}: not a number: {v}")),
        }
    }
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

fn emit<T: Serialize>(json: bool, v: &T, text: String) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(v)?);
    } else {
        println!("{text}");
    }
    Ok(())
}

struct Ctx {
    threads: usize,
    offline: bool,
}

impl Ctx {
    fn ears(&self, ast: bool) -> Result<Ears> {
        Ears::load(EarsOptions { cache_dir: None, threads: self.threads, load_ast: ast, offline: self.offline })
    }
}

fn need(a: &Args, n: usize, what: &str) -> Result<()> {
    if a.pos.len() < n {
        bail!("missing argument: {what}");
    }
    Ok(())
}

fn cmd_describe(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &["--top"], &[])?;
    need(&a, 1, "PATH")?;
    let ears = ctx.ears(true)?;
    let path = PathBuf::from(&a.pos[0]);
    let r = describe::describe(&ears, &path, a.num("--top")?.unwrap_or(8))?;
    let mut lines = vec![r.summary.clone()];
    if !r.top_tags.is_empty() {
        lines.push(String::new());
        lines.push("top tags (z calibrated: >= 3 clear, 2.5-3 plausible, 2-2.5 weak, < 2 tentative):".into());
        for b in &r.top_tags {
            lines.push(format!("  {:>5}  {}  [{}]  cos {:.3}", matching::zf(b.z), b.term, b.category, b.cosine));
        }
        if !r.music_hints.is_empty() {
            lines.push(format!(
                "unvalidated hints (music genre / mood, instruments, voice): {}",
                r.music_hints
                    .iter()
                    .take(5)
                    .map(|h| format!("{} ({})", h.term, matching::zf(h.z)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let best: Vec<String> = r
            .tags
            .iter()
            .filter_map(|(c, d)| {
                let d = d.as_array()?.first()?;
                Some(format!("{c}: {} ({})", d["term"].as_str()?, matching::zf(d["z"].as_f64()?)))
            })
            .collect();
        lines.push(format!("best per category: {}", best.join("; ")));
        lines.push(format!(
            "audioset: {}",
            r.audioset.iter().map(|d| format!("{} {:.2}", d.label, d.p)).collect::<Vec<_>>().join(", ")
        ));
    }
    lines.push(format!(
        "measurements: {}",
        r.measurements
            .iter()
            .map(|(k, v)| format!("{k} {}", v.as_str().map(String::from).unwrap_or_else(|| v.to_string())))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    emit(json, &r, lines.join("\n"))
}

fn cmd_match(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &[], &["--alts"])?;
    need(&a, 2, "PATH QUERY")?;
    let ears = ctx.ears(false)?;
    let path = PathBuf::from(&a.pos[0]);
    let queries: Vec<&str> = a.pos[1..].iter().map(|s| s.as_str()).collect();
    let alts = a.vals("--alts");
    let alts: Vec<&str> = alts.iter().map(|s| s.as_str()).collect();
    let r = describe::match_file(&ears, &path, &queries, &alts)?;
    emit(json, &r, r.summary.clone())
}

fn cmd_rank(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &["--top"], &[])?;
    need(&a, 2, "QUERY PATH...")?;
    let ears = ctx.ears(false)?;
    let paths: Vec<PathBuf> = a.pos[1..].iter().map(PathBuf::from).collect();
    let missing: Vec<&PathBuf> = paths.iter().filter(|p| !p.exists()).collect();
    if !missing.is_empty() {
        bail!(
            "no such file or folder: {}",
            missing.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        );
    }
    let r = describe::rank_files(&ears, &paths, &a.pos[0], a.num("--top")?)?;
    let mut lines = vec![r.summary.clone(), String::new()];
    for (i, x) in r.results.iter().enumerate() {
        lines.push(format!(
            "{:3}. z {:>5}  {:<9} cos {:.3}  {}  (sounds most like {}){}",
            i + 1,
            matching::zf(x.z),
            x.verdict,
            x.cosine,
            x.name,
            x.sounds_most_like,
            if x.near_silent { "  NEAR-SILENT" } else { "" }
        ));
    }
    emit(json, &r, lines.join("\n"))
}

fn cmd_find(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &["--top", "--kind", "--win", "--hop"], &[])?;
    need(&a, 2, "PATH QUERY")?;
    let ears = ctx.ears(false)?;
    let opt =
        search::FindOptions { kind: a.val("--kind").map(String::from), win: a.num("--win")?, hop: a.num("--hop")? };
    if let Some(k) = &opt.kind
        && !["auto", "event", "texture"].contains(&k.as_str())
    {
        bail!("--kind must be auto, event or texture");
    }
    let r = ears.find_file(Path::new(&a.pos[0]), &a.pos[1], a.num("--top")?.unwrap_or(5), &opt)?;
    let mut lines = vec![r.summary.clone(), String::new()];
    for (i, p) in r.segments.iter().enumerate() {
        lines.push(format!(
            "{}. {:.3}-{:.3} s ({}, {:.3} s)  z {} {:<9} {:<6} peak {:.1} dBFS  head {:.0} dB  tail {:.0} dB{}",
            i + 1,
            p.start,
            p.end,
            fmt_t(p.start),
            p.duration,
            matching::zf(p.z),
            p.verdict,
            p.kind,
            p.peak_dbfs,
            p.head_db,
            p.tail_db,
            if p.cut.is_empty() { String::new() } else { format!("  (ranked {}: {})", matching::zf(p.score), p.cut) }
        ));
    }
    emit(json, &r, lines.join("\n"))
}

fn cmd_measure(raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &[], &[])?;
    need(&a, 1, "FILE...")?;
    let files = audio::list_media(&a.pos.iter().map(PathBuf::from).collect::<Vec<_>>());
    if files.is_empty() {
        bail!("no audio/video files found in: {}", a.pos.join(", "));
    }
    let mut res = Vec::new();
    for f in &files {
        let m = earshot::measure_file(f, a.has("--music"), a.has("--light"))?;
        let mut v = serde_json::to_value(&m)?;
        v["file"] = serde_json::json!(f.display().to_string());
        v["summary"] = serde_json::json!(features::text(&m, &name_of(f)));
        res.push(v);
    }
    let text = res.iter().map(|r| r["summary"].as_str().unwrap().to_string()).collect::<Vec<_>>().join("\n");
    if res.len() == 1 {
        emit(json, &res[0], text)
    } else {
        emit(json, &serde_json::json!({"summary": format!("{} files measured.", res.len()), "files": res}), text)
    }
}

fn realness_text(r: &earshot::Realness) -> String {
    let mut lines = vec![r.summary.clone()];
    for e in &r.evidence {
        lines.push(format!("  - {e}"));
    }
    for s in r.segments.iter().flatten() {
        let pr = s.p_real.map_or("  -  ".to_string(), |p| format!("{p:.2}"));
        let lean = if s.verdict == "uncertain" && s.p_real.unwrap_or(1.0) < 0.5 { ", leans synthesized" } else { "" };
        let why = match &s.why {
            Some(w) if s.verdict != "real or pro-produced" => format!("  ({w})"),
            _ => String::new(),
        };
        lines.push(format!("  {}-{}  p_real {pr}  {}{lean}{why}", fmt_t(s.start), fmt_t(s.end), s.verdict));
    }
    if let Some(n) = &r.note {
        lines.push(format!("  note: {n}"));
    }
    lines.join("\n")
}

fn cmd_realness(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &[], &[])?;
    need(&a, 1, "PATH...")?;
    let files = audio::list_media(&a.pos.iter().map(PathBuf::from).collect::<Vec<_>>());
    if files.is_empty() {
        bail!("no audio/video files found in: {}", a.pos.join(", "));
    }
    let ears = ctx.ears(false)?;
    if files.len() == 1 {
        let r = ears.realness_file(&files[0])?;
        return emit(json, &r, realness_text(&r));
    }
    let mut rows = Vec::new();
    let mut lines = vec![format!("{:>6}  {:<24} file: main reason", "p_real", "verdict")];
    let mut counts = std::collections::BTreeMap::new();
    for f in &files {
        match ears.realness_file(f) {
            Ok(r) => {
                let p = r.p_real.map_or("  -  ".to_string(), |p| format!("{p:.2}"));
                let long = if r.segments.is_some() { " (file)" } else { "" };
                lines.push(format!("{p:>6}  {:<24} {}: {}", format!("{}{long}", r.verdict), name_of(f), r.why));
                *counts.entry(r.verdict.clone()).or_insert(0usize) += 1;
                let mut v = serde_json::to_value(&r)?;
                v["path"] = serde_json::json!(f.display().to_string());
                rows.push(v);
            }
            Err(e) => {
                *counts.entry("error".to_string()).or_insert(0usize) += 1;
                lines.push(format!("{:>6}  {:<24} {}: {e}", "  -  ", "error", name_of(f)));
                rows.push(serde_json::json!({"path": f.display().to_string(), "p_real": null, "verdict": "error", "why": e.to_string()}));
            }
        }
    }
    let n = |k: &str| counts.get(k).copied().unwrap_or(0);
    let mut parts = vec![
        format!("{} real or pro-produced", n("real or pro-produced")),
        format!("{} uncertain", n("uncertain")),
        format!("{} likely code-synthesized", n("likely code-synthesized")),
    ];
    if n("silent") > 0 {
        parts.push(format!("{} silent (not judged)", n("silent")));
    }
    if n("error") > 0 {
        parts.push(format!("{} could not be read (not judged: check them)", n("error")));
    }
    let summary = format!("{} files: {}.", rows.len(), parts.join(", "));
    lines.insert(0, summary.clone());
    emit(json, &serde_json::json!({"files": rows, "counts": counts, "summary": summary}), lines.join("\n"))
}

fn cmd_grid(raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &[], &[])?;
    need(&a, 1, "PATH")?;
    let path = PathBuf::from(&a.pos[0]);
    let (l, r) = audio::load_stereo(&path)?;
    let g = earshot::grid::music_grid(&audio::mean_mono(&l, &r));
    let text = match g.tempo {
        None => format!("{}: no beat grid ({})", name_of(&path), g.note),
        Some(t) => format!(
            "{}: {t:.1} BPM{} ({} pulse, clarity {:.2}, beat regularity {:.3}); {} beats from {:.2} s, downbeats (4/4 guess) at {}{}",
            name_of(&path),
            g.tempo_alt.map(|a| format!(" (or {a:.1})")).unwrap_or_default(),
            g.confidence,
            g.pulse_clarity.unwrap_or(0.0),
            g.beat_regularity.unwrap_or(0.0),
            g.beats.len(),
            g.beats.first().copied().unwrap_or(0.0),
            g.downbeats.iter().take(8).map(|d| format!("{d:.2}")).collect::<Vec<_>>().join(", "),
            if g.downbeats.len() > 8 { " ..." } else { "" }
        ),
    };
    emit(json, &g, text)
}

fn cmd_compare(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    let a = Args::parse(raw, &["-k"], &["--refs"])?;
    need(&a, 1, "PATH")?;
    let refs: Vec<PathBuf> = a.vals("--refs").iter().map(PathBuf::from).collect();
    if refs.is_empty() {
        bail!("--refs REF... is required");
    }
    let ears = ctx.ears(false)?;
    let r = describe::compare_files(&ears, Path::new(&a.pos[0]), &refs, a.num("-k")?.unwrap_or(5))?;
    let mut lines = vec![r["summary"].as_str().unwrap_or_default().to_string(), String::new(), "nearest:".into()];
    for n in r["nearest"].as_array().into_iter().flatten() {
        lines.push(format!("  cos {:.3}  {}", n["cosine"].as_f64().unwrap_or(0.0), n["name"].as_str().unwrap_or("")));
    }
    if let Some(f) = r["features"].as_object().filter(|f| !f.is_empty()) {
        lines.push("features (value / refs median / robust z):".into());
        for (k, d) in f {
            lines.push(format!(
                "  {k:<17} {:>9} / {:>9} / {:+.1}",
                pynum(&d["value"]),
                pynum(&d["refs_median"]),
                d["robust_z"].as_f64().unwrap_or(0.0)
            ));
        }
    }
    emit(json, &r, lines.join("\n"))
}

/// a JSON number as Python prints it (ints bare, floats with at least one decimal)
fn pynum(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
        serde_json::Value::Number(n) => features::pyf(n.as_f64().unwrap_or(0.0)),
        other => other.to_string(),
    }
}

fn cmd_doctor(ctx: &Ctx, raw: &[String], json: bool) -> Result<()> {
    use earshot::models::*;
    let a = Args::parse(raw, &[], &[])?;
    let cache = default_cache_dir();
    let tool = |p: PathBuf| -> String {
        match std::process::Command::new(&p).arg("-version").output() {
            Ok(o) if o.status.success() => {
                format!("{} ({})", p.display(), String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim())
            }
            _ => format!("MISSING ({}): install ffmpeg or set EARSHOT_FFMPEG / EARSHOT_FFPROBE", p.display()),
        }
    };
    let files = [
        (CLAP_REPO, CLAP_REV, CLAP_AUDIO),
        (CLAP_REPO, CLAP_REV, CLAP_TEXT),
        (CLAP_REPO, CLAP_REV, CLAP_TOKENIZER),
        (AST_REPO, AST_REV, AST_MODEL),
        (AST_REPO, AST_REV, AST_CONFIG),
    ];
    let mut cached = serde_json::Map::new();
    for (repo, rev, f) in files {
        cached.insert(format!("{repo}/{f}"), serde_json::json!(model_path(&cache, repo, rev, f).is_file()));
    }
    let ffmpeg = tool(audio::ffmpeg());
    let ffprobe = tool(audio::ffprobe());
    let mut problems: Vec<String> = Vec::new();
    if ffmpeg.starts_with("MISSING") {
        problems.push("ffmpeg missing".into());
    }
    if ffprobe.starts_with("MISSING") {
        problems.push("ffprobe missing".into());
    }
    let all_cached = cached.values().all(|v| v.as_bool() == Some(true));
    let mut r = serde_json::json!({
        "earshot_home": cache.display().to_string(), "threads": if ctx.threads == 0 { default_threads() } else { ctx.threads },
        "offline": ctx.offline || env_offline(), "ffmpeg": ffmpeg, "ffprobe": ffprobe, "models_cached": cached,
        "onnxruntime": format!("ort {}", ort::MINOR_VERSION),
    });
    if a.has("--load") {
        let t = std::time::Instant::now();
        let ears = ctx.ears(true)?;
        r["load_s"] = serde_json::json!(earshot::dsp::round_to(t.elapsed().as_secs_f64(), 2));
        let x = vec![0.0f32; 48000];
        let t = std::time::Instant::now();
        ears.embed(&x)?;
        r["clap_one_window_s"] = serde_json::json!(earshot::dsp::round_to(t.elapsed().as_secs_f64(), 3));
        let t = std::time::Instant::now();
        ears.audioset(&x)?;
        r["ast_one_window_s"] = serde_json::json!(earshot::dsp::round_to(t.elapsed().as_secs_f64(), 3));
    }
    let summary = (if problems.is_empty() {
        "earshot looks healthy.".to_string()
    } else {
        format!("Problems: {}", problems.join("; "))
    }) + if all_cached {
        ""
    } else {
        " Models are not cached yet: the first use downloads them (~1.13 GB) into the cache."
    };
    r["problems"] = serde_json::json!(problems);
    r["summary"] = serde_json::json!(summary);
    let text = r
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| *k != "summary")
        .fold(summary.clone(), |s, (k, v)| s + &format!("\n{k}: {v}"));
    emit(json, &r, text)
}

fn run() -> Result<ExitCode> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut threads = 0usize;
    let mut offline = false;
    let mut json = false;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--threads" => {
                i += 1;
                threads = raw.get(i).and_then(|v| v.parse().ok()).context("--threads needs a number")?;
            }
            "--offline" => offline = true,
            "--json" => json = true,
            "-h" | "--help" if rest.is_empty() => {
                println!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            _ => rest.push(raw[i].clone()),
        }
        i += 1;
    }
    let Some(cmd) = rest.first().cloned() else {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    };
    let args = &rest[1..];
    let ctx = Ctx { threads, offline };
    match cmd.as_str() {
        "describe" => cmd_describe(&ctx, args, json)?,
        "match" => cmd_match(&ctx, args, json)?,
        "rank" => cmd_rank(&ctx, args, json)?,
        "find" => cmd_find(&ctx, args, json)?,
        "measure" => cmd_measure(args, json)?,
        "realness" => cmd_realness(&ctx, args, json)?,
        "grid" => cmd_grid(args, json)?,
        "compare" => cmd_compare(&ctx, args, json)?,
        "doctor" => cmd_doctor(&ctx, args, json)?,
        _ => bail!("unknown command '{cmd}' (see earshot --help)"),
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match run() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("{e:#}").split_whitespace().collect::<Vec<_>>().join(" ");
            eprintln!("earshot {cmd}: error: {msg}");
            if std::env::args().any(|a| a == "--json") {
                println!("{}", serde_json::json!({"summary": format!("error: {msg}"), "error": msg}));
            }
            ExitCode::FAILURE
        }
    }
}
