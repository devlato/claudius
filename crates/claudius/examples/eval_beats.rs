//! How well claudius hears where a real song's bars fall: the yardstick for a change to how it tracks beats and bars
//! (the contributor guide's Benchmarks). The songs are MTG-Jamendo's, as `eval_genres` takes them (a folder of its
//! shards and its tags); the reference is another beat tracker's beats (and downbeats, where it hears them) of the same
//! songs, a line each (`{"item": "track_0000214", "beats": [s...], "downbeats": [s...]}`), made apart from claudius.
//!
//! `run` analyses each song the reference has, as the studio analyses a recording: with its stems apart (`--stems`, as the
//! desktop hears one) or from the whole mix (as the web studio, which has no stem model, does). It writes a line for
//! `cargo xtask bench compare` for each song (after one that says which way each metric is better):
//!
//! - **on_beats**, the share of its bar lines within 70 ms of one of the reference's beats: a bar line between two beats
//!   cuts a piece of the song on the wrong place, and the piece plays against claudius's own parts. Higher is better.
//! - **on_downbeats**, the share within 70 ms of one of the reference's downbeats, where the reference has them: a bar
//!   that starts a beat early or late. Higher is better.
//! - **tempo**, whether its tempo is the reference's within 4 %, at the same tempo, half or twice it (the reference's
//!   from the median gap between its beats, or its own `tempo` where a line gives one).
//! - **tempo_exact**, whether its tempo is the reference's within 4 % at the same tempo: neither half nor twice it, the
//!   octave a listener taps. Only where people annotated the tempo does this say which is right.
//!
//! sliced by the song's genre family and whether claudius heard it keep one tempo (`steady`).
//!
//! A reference line may name its recording (`"path"`) and its family (`"family"`) itself, for a dataset other than
//! MTG-Jamendo's; then DIR is the folder its paths are in, and its own `"slice"` fields slice the results too.
//!
//! `tempo` hears only each recording's tempo, as the studio hears a whole mix's (`beats::track_with`, without stems), a
//! short loop repeated to `--tile` seconds first (a loop well cut is made to repeat): `tempo` and `tempo_exact` alone,
//! for a reference of people's tempos such as the Freesound Loop Dataset's.
//!
//! ```text
//! cargo run --release -p claudius --example eval_beats -- run DIR --reference FILE [--stems] [--out FILE]
//!     [--threads N] [--most N] [--primary on_beats|on_downbeats|tempo|tempo_exact]
//! cargo run --release -p claudius --example eval_beats -- tempo DIR --reference FILE [--tile SECONDS] [--out FILE]
//!     [--threads N] [--most N]
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "shared/families.rs"]
mod families;

use serde_json::{Value, json};

/// how near a bar line has to be to a reference beat to count as on it, in seconds (the usual tolerance of beat tracking's
/// F-measure)
const NEAR: f64 = 0.07;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let threads = flag("--threads").and_then(|t| t.parse().ok()).unwrap_or(4);
    let result = match (args.first().map(String::as_str), args.get(1), flag("--reference")) {
        (Some("tempo"), Some(dir), Some(reference)) => tempo(
            Path::new(dir),
            Path::new(&reference),
            &flag("--out").map_or_else(|| PathBuf::from("tempo.jsonl"), PathBuf::from),
            threads,
            flag("--most").and_then(|m| m.parse().ok()).unwrap_or(usize::MAX),
            flag("--tile").and_then(|t| t.parse().ok()).unwrap_or(30.0),
        ),
        (Some("run"), Some(dir), Some(reference)) => run(
            Path::new(dir),
            Path::new(&reference),
            args.iter().any(|a| a == "--stems"),
            &flag("--out").map_or_else(|| PathBuf::from("beats.jsonl"), PathBuf::from),
            threads,
            flag("--most").and_then(|m| m.parse().ok()).unwrap_or(usize::MAX),
            &flag("--primary").unwrap_or_else(|| "on_beats".into()),
        ),
        _ => Err(
            "usage: eval_beats run DIR --reference FILE [--stems] [--out FILE] [--threads N] [--most N] [--primary M]"
                .into(),
        ),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

/// a song the reference has: where its audio is, its family, and the reference's beats and downbeats
struct Song {
    id: String,
    path: PathBuf,
    family: String,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
    tempo: Option<f64>,
    /// the reference line's own slices (`"slice"`), beside the family
    slices: serde_json::Map<String, Value>,
}

fn songs(dir: &Path, reference: &Path, most: usize) -> Result<Vec<Song>, String> {
    // MTG-Jamendo's tags, when the folder has them: a line that names no recording of its own is one of its songs
    let tsv = std::fs::read_to_string(dir.join("raw_30s_cleantags.tsv")).unwrap_or_default();
    let mut tags: BTreeMap<&str, (PathBuf, &'static str)> = BTreeMap::new();
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
        let (Some(id), Some(path)) = (cols.first(), cols.get(3)) else { continue };
        let Some((s, file)) = path.split_once('/') else { continue };
        let genres: BTreeSet<&str> = cols.iter().filter_map(|c| c.strip_prefix("genre---")).collect();
        let family = families::family(&genres);
        tags.insert(id, (dir.join(s).join(file.replace(".mp3", ".low.mp3")), family));
    }
    let text = std::fs::read_to_string(reference).map_err(|e| format!("{}: {e}", reference.display()))?;
    let mut out = Vec::new();
    for v in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
        let Some(id) = v["item"].as_str() else { continue };
        let (path, family) = match (v["path"].as_str(), tags.get(id)) {
            (Some(p), _) => (dir.join(p), v["family"].as_str().unwrap_or("other").to_string()),
            (None, Some((p, f))) => (p.clone(), f.to_string()),
            (None, None) => continue,
        };
        let times = |k: &str| -> Vec<f64> { v[k].as_array().into_iter().flatten().filter_map(Value::as_f64).collect() };
        let (beats, tempo) = (times("beats"), v["tempo"].as_f64().filter(|t| *t > 0.0));
        if (beats.len() < 16 && tempo.is_none()) || !path.is_file() {
            continue;
        }
        let slices = v["slice"].as_object().cloned().unwrap_or_default();
        out.push(Song { id: id.to_string(), path, family, beats, downbeats: times("downbeats"), tempo, slices });
        if out.len() == most {
            break;
        }
    }
    Ok(out)
}

/// the share of `lines` within `NEAR` of one of `marks` (sorted), of the lines from the first mark to the last
fn share_near(lines: &[f64], marks: &[f64]) -> Option<f64> {
    let (first, last) = (*marks.first()?, *marks.last()?);
    let inside: Vec<f64> = lines.iter().copied().filter(|t| *t >= first - NEAR && *t <= last + NEAR).collect();
    if inside.is_empty() {
        return None;
    }
    let near = |t: f64| {
        let i = marks.partition_point(|m| *m < t);
        [i.checked_sub(1), Some(i)].into_iter().flatten().filter_map(|j| marks.get(j)).any(|m| (m - t).abs() <= NEAR)
    };
    Some(inside.iter().filter(|t| near(**t)).count() as f64 / inside.len() as f64)
}

/// the reference's tempo: the one its line gives, else from the median gap between its beats
fn reference_tempo(s: &Song) -> Option<f64> {
    s.tempo.or_else(|| {
        let mut gaps: Vec<f64> = s.beats.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.sort_by(f64::total_cmp);
        gaps.get(gaps.len() / 2).map(|g| 60.0 / g.max(1e-3))
    })
}

/// whether `bpm` is `theirs` within 4 %: at it, or (`octaves`) at half or twice it as well
fn same_tempo(bpm: f64, theirs: f64, octaves: bool) -> bool {
    let ks: &[f64] = if octaves { &[0.5, 1.0, 2.0] } else { &[1.0] };
    ks.iter().any(|k| ((bpm * k) / theirs - 1.0).abs() <= 0.04)
}

fn run(
    dir: &Path,
    reference: &Path,
    stems: bool,
    out: &Path,
    threads: usize,
    most: usize,
    primary: &str,
) -> Result<(), String> {
    let songs = songs(dir, reference, most)?;
    eprintln!("{} songs, {}", songs.len(), if stems { "their stems apart" } else { "the whole mix" });
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let file = Mutex::new(file);
    let head = json!({
        "better": {"on_beats": "higher", "on_downbeats": "higher", "tempo": "higher", "tempo_exact": "higher"},
        "primary": [primary],
    });
    writeln!(file.lock().unwrap(), "{head}").map_err(|e| e.to_string())?;
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(s) = songs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let x = match claudius::audio::load(&s.path) {
                        Ok(x) => x,
                        Err(e) => {
                            eprintln!("{}: {e:#}", s.id);
                            continue;
                        }
                    };
                    let parts = if stems { claudius::stems::of(&x, &mut |_| {}) } else { None };
                    let a = claudius::analysis::song::analyze_with(&x, parts.as_deref(), None);
                    let mut metrics = json!({});
                    if let Some(theirs) = reference_tempo(s) {
                        metrics["tempo"] = json!(same_tempo(a.bpm, theirs, true));
                        metrics["tempo_exact"] = json!(same_tempo(a.bpm, theirs, false));
                    }
                    if let Some(on) = share_near(&a.bar_starts, &s.beats) {
                        metrics["on_beats"] = json!((on * 1000.0).round() / 1000.0);
                    }
                    if let Some(on) = share_near(&a.bar_starts, &s.downbeats) {
                        metrics["on_downbeats"] = json!((on * 1000.0).round() / 1000.0);
                    }
                    let mut slice = slice_of(s);
                    slice["steady"] = json!(if a.steady { "steady" } else { "drifting" });
                    let line = json!({"item": s.id, "slice": slice, "metrics": metrics});
                    let _ = writeln!(file.lock().unwrap_or_else(|p| p.into_inner()), "{line}");
                }
            });
        }
    });
    Ok(())
}

/// `songs` slice by their family, and by what their reference line says of them
fn slice_of(s: &Song) -> Value {
    let mut slice = s.slices.clone();
    slice.insert("family".into(), json!(s.family));
    Value::Object(slice)
}

fn tempo(dir: &Path, reference: &Path, out: &Path, threads: usize, most: usize, tile: f64) -> Result<(), String> {
    let songs: Vec<Song> = songs(dir, reference, most)?.into_iter().filter(|s| s.tempo.is_some()).collect();
    eprintln!("{} recordings with a tempo, each heard as a whole mix, repeated to {tile} s", songs.len());
    let file = Mutex::new(std::fs::File::create(out).map_err(|e| e.to_string())?);
    let head = json!({"better": {"tempo": "higher", "tempo_exact": "higher"}, "primary": ["tempo_exact"]});
    writeln!(file.lock().unwrap(), "{head}").map_err(|e| e.to_string())?;
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(s) = songs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let x = match claudius::audio::load(&s.path) {
                        Ok(x) => x,
                        Err(e) => {
                            eprintln!("{}: {e:#}", s.id);
                            continue;
                        }
                    };
                    let one = x.mono();
                    let want = (tile * claudius::config::SRF) as usize;
                    let mono: Vec<f32> = if one.is_empty() { one } else { one.iter().copied().cycle().take(want.max(one.len())).collect() };
                    let g = claudius::analysis::beats::track_with(&mono, &mono, None, false);
                    let theirs = s.tempo.unwrap_or_default();
                    let line = json!({
                        "item": s.id,
                        "slice": slice_of(s),
                        "metrics": {"tempo": same_tempo(g.bpm, theirs, true), "tempo_exact": same_tempo(g.bpm, theirs, false)},
                        "bpm": (g.bpm * 10.0).round() / 10.0,
                    });
                    let _ = writeln!(file.lock().unwrap_or_else(|p| p.into_inner()), "{line}");
                }
            });
        }
    });
    Ok(())
}
