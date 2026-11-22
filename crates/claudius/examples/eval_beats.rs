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
//! - **in_step**, the share of its bars that start and end on the reference's beats, four of them apart: a bar a beat
//!   short or long, or one off the beat, is out of step, as the bars are where a beat the tracker lost or slipped in
//!   slides them across the beat. Higher is better.
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
//! `--drift DEPTH` plays each recording as a band drifts first: its speed wanders around 1, within DEPTH (0.025 is 2.5 %),
//! in two slow waves each recording draws from its name, and the reference's beats move with it.
//!
//! `grid` hears people's loops cut to whole bars of four (a reference line with the `tempo` people gave), each repeated
//! to `--tile` seconds, steady or drifting: their beats are known, the loop's grid from its start at the tempo its
//! length gives (as many bars as it holds at about the people's tempo), and a downbeat every fourth, so `on_beats`,
//! `on_downbeats`, `in_step` and `tempo_exact` are measured against the music itself.
//!
//! ```text
//! cargo run --release -p claudius --example eval_beats -- run DIR --reference FILE [--stems] [--drift DEPTH]
//!     [--out FILE] [--threads N] [--most N] [--primary on_beats|on_downbeats|in_step|tempo|tempo_exact]
//! cargo run --release -p claudius --example eval_beats -- grid DIR --reference FILE [--tile SECONDS] [--drift DEPTH]
//!     [--out FILE] [--threads N] [--most N]
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

use claudius::audio::Stereo;
use claudius::config::SRF;
use serde_json::{Value, json};

/// how near a bar line has to be to a reference beat to count as on it, in seconds (the usual tolerance of beat tracking's
/// F-measure)
const NEAR: f64 = 0.07;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let threads = flag("--threads").and_then(|t| t.parse().ok()).unwrap_or(4);
    let hearing = Hearing {
        stems: args.iter().any(|a| a == "--stems"),
        drift: flag("--drift").and_then(|d| d.parse().ok()),
        threads,
        most: flag("--most").and_then(|m| m.parse().ok()).unwrap_or(usize::MAX),
    };
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
            &hearing,
            &flag("--out").map_or_else(|| PathBuf::from("beats.jsonl"), PathBuf::from),
            &flag("--primary").unwrap_or_else(|| "on_beats".into()),
        ),
        (Some("grid"), Some(dir), Some(reference)) => grid(
            Path::new(dir),
            Path::new(&reference),
            &hearing,
            &flag("--out").map_or_else(|| PathBuf::from("grid.jsonl"), PathBuf::from),
            flag("--tile").and_then(|t| t.parse().ok()).unwrap_or(30.0),
        ),
        _ => Err("usage: eval_beats run|grid|tempo DIR --reference FILE [--stems] [--drift DEPTH] [--tile SECONDS] \
                  [--out FILE] [--threads N] [--most N] [--primary M]"
            .into()),
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

/// the share of the bars between `lines` (bar lines, sorted) that start and end on beats of `marks` (sorted) four of them
/// apart, of the bars from the first mark to the last: a bar a beat short or long, or off the beat, is out of step
fn in_step(lines: &[f64], marks: &[f64]) -> Option<f64> {
    let (first, last) = (*marks.first()?, *marks.last()?);
    let on = |t: f64| -> Option<usize> {
        let i = marks.partition_point(|m| *m < t);
        [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter(|&j| j < marks.len() && (marks[j] - t).abs() <= NEAR)
            .min_by(|&a, &b| (marks[a] - t).abs().total_cmp(&(marks[b] - t).abs()))
    };
    let bars: Vec<(f64, f64)> =
        lines.windows(2).map(|w| (w[0], w[1])).filter(|(a, b)| *a >= first - NEAR && *b <= last + NEAR).collect();
    if bars.is_empty() {
        return None;
    }
    let good = bars.iter().filter(|(a, b)| matches!((on(*a), on(*b)), (Some(i), Some(j)) if j == i + 4)).count();
    Some(good as f64 / bars.len() as f64)
}

/// How a recording is played as a band drifts: its speed wanders around 1, within `depth`, as two slow waves whose
/// periods (24 to 48 s and 8 to 16 s) and phases its name draws, so that each drifts its own way, and the same way in
/// every arm of an experiment.
struct Drift {
    depth: f64,
    /// each wave's weight, period (s) and phase
    waves: [(f64, f64, f64); 2],
}

impl Drift {
    fn of(id: &str, depth: f64) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in id.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        let mut next = || {
            h ^= h >> 33;
            h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
            h ^= h >> 33;
            (h >> 11) as f64 / (1u64 << 53) as f64
        };
        let tau = std::f64::consts::TAU;
        let (p1, t1) = (tau * next(), 24.0 + 24.0 * next());
        let (p2, t2) = (tau * next(), 8.0 + 8.0 * next());
        Drift { depth, waves: [(0.6, t1, p1), (0.4, t2, p2)] }
    }

    /// how fast the recording plays at its own second `u`
    fn speed(&self, u: f64) -> f64 {
        let wave = |&(w, period, phase): &(f64, f64, f64)| w * (std::f64::consts::TAU * u / period + phase).sin();
        1.0 + self.depth * self.waves.iter().map(wave).sum::<f64>()
    }

    /// `x` played so, and when each of its seconds sounds in what it plays: a table of (its second, when it sounds),
    /// every 5 ms of it
    fn play(&self, x: &Stereo) -> (Stereo, Vec<(f64, f64)>) {
        let secs = x.l.len() as f64 / SRF;
        let step = 0.005;
        let mut table = vec![(0.0, 0.0)];
        let (mut u, mut at) = (0.0, 0.0);
        while u < secs {
            at += step / self.speed(u + step / 2.0);
            u += step;
            table.push((u, at));
        }
        let n = (at * SRF) as usize;
        let (mut l, mut r) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let mut j = 0;
        for k in 0..n {
            let t = k as f64 / SRF;
            while j + 2 < table.len() && table[j + 1].1 < t {
                j += 1;
            }
            let ((u0, t0), (u1, t1)) = (table[j], table[j + 1]);
            let pos = (u0 + (u1 - u0) * (t - t0) / (t1 - t0).max(1e-12)) * SRF;
            l.push(cubic(&x.l, pos));
            r.push(cubic(&x.r, pos));
        }
        (Stereo { l, r }, table)
    }
}

/// when the recording's second `u` sounds, by a `Drift::play` table (`None` past its end)
fn sounds_at(table: &[(f64, f64)], u: f64) -> Option<f64> {
    let i = table.partition_point(|(v, _)| *v < u).max(1);
    let ((u0, t0), (u1, t1)) = (*table.get(i - 1)?, *table.get(i)?);
    Some(t0 + (t1 - t0) * (u - u0) / (u1 - u0).max(1e-12))
}

/// `x` between its samples, by the cubic through the four around (Catmull and Rom's)
fn cubic(x: &[f32], pos: f64) -> f32 {
    let i = pos.floor() as isize;
    let f = (pos - i as f64) as f32;
    let at = |k: isize| x.get(k.clamp(0, x.len() as isize - 1) as usize).copied().unwrap_or(0.0);
    let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
    p1 + 0.5 * f * (p2 - p0 + f * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + f * (3.0 * (p1 - p2) + p3 - p0)))
}

/// The bar lines of `x` as the studio hears them, its tempo, and whether it keeps one: from its stems apart (`parts`, by
/// the whole analysis), or as a whole mix by the beat tracker alone, laid into bars as `analyze` lays them
/// (`song::bars_on`): the same bars, minutes faster on a whole song than the analysis that also hears its sections,
/// chords and drums.
fn bars_of(x: &Stereo, parts: Option<&claudius::stems::Stems>) -> (Vec<f64>, f64, bool) {
    if parts.is_some() {
        let a = claudius::analysis::song::analyze_with(x, parts, None);
        return (a.bar_starts, a.bpm, a.steady);
    }
    let mono = x.mono();
    let g = claudius::analysis::beats::track_with(&mono, &mono, None, false);
    let bars = claudius::analysis::song::bars_on(&g.beats, g.downbeat, 60.0 / g.bpm.max(1.0), mono.len() as f64 / SRF);
    (bars, g.bpm, g.steady)
}

/// how a run hears its recordings: their stems apart or the whole mix, steady or played as a band drifts, on how many
/// threads, and how many of them at most
struct Hearing {
    stems: bool,
    drift: Option<f64>,
    threads: usize,
    most: usize,
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

fn run(dir: &Path, reference: &Path, hearing: &Hearing, out: &Path, primary: &str) -> Result<(), String> {
    let &Hearing { stems, drift, threads, most } = hearing;
    let songs = songs(dir, reference, most)?;
    eprintln!(
        "{} songs, {}{}",
        songs.len(),
        if stems { "their stems apart" } else { "the whole mix" },
        drift.map_or(String::new(), |d| format!(", played as a band drifts (within {:.1} %)", d * 100.0))
    );
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let file = Mutex::new(file);
    let head = json!({
        "better": {"on_beats": "higher", "on_downbeats": "higher", "in_step": "higher", "tempo": "higher", "tempo_exact": "higher"},
        "primary": [primary],
    });
    writeln!(file.lock().unwrap(), "{head}").map_err(|e| e.to_string())?;
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(s) = songs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let mut x = match claudius::audio::load(&s.path) {
                        Ok(x) => x,
                        Err(e) => {
                            eprintln!("{}: {e:#}", s.id);
                            continue;
                        }
                    };
                    // the reference's marks, moved with the recording where it drifts
                    let (mut beats, mut downbeats, mut theirs) =
                        (s.beats.clone(), s.downbeats.clone(), reference_tempo(s));
                    if let Some(depth) = drift {
                        let (played, table) = Drift::of(&s.id, depth).play(&x);
                        let speed = x.l.len() as f64 / played.l.len().max(1) as f64;
                        beats = beats.iter().filter_map(|&u| sounds_at(&table, u)).collect();
                        downbeats = downbeats.iter().filter_map(|&u| sounds_at(&table, u)).collect();
                        theirs = theirs.map(|t| t * speed);
                        x = played;
                    }
                    let parts = if stems { claudius::stems::of(&x, &mut |_| {}) } else { None };
                    let (bar_starts, bpm, steady) = bars_of(&x, parts.as_deref());
                    let mut metrics = json!({});
                    if let Some(theirs) = theirs {
                        metrics["tempo"] = json!(same_tempo(bpm, theirs, true));
                        metrics["tempo_exact"] = json!(same_tempo(bpm, theirs, false));
                    }
                    for (name, value) in [
                        ("on_beats", share_near(&bar_starts, &beats)),
                        ("on_downbeats", share_near(&bar_starts, &downbeats)),
                        ("in_step", in_step(&bar_starts, &beats)),
                    ] {
                        if let Some(v) = value {
                            metrics[name] = json!((v * 1000.0).round() / 1000.0);
                        }
                    }
                    let mut slice = slice_of(s);
                    slice["steady"] = json!(if steady { "steady" } else { "drifting" });
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

fn grid(dir: &Path, reference: &Path, hearing: &Hearing, out: &Path, tile: f64) -> Result<(), String> {
    let &Hearing { drift, threads, most, .. } = hearing;
    let loops: Vec<Song> = songs(dir, reference, most)?.into_iter().filter(|s| s.tempo.is_some()).collect();
    eprintln!(
        "{} loops with people's tempo, each heard as a whole mix, repeated to {tile} s{}",
        loops.len(),
        drift.map_or(String::new(), |d| format!(" and played as a band drifts (within {:.1} %)", d * 100.0))
    );
    let file = Mutex::new(std::fs::File::create(out).map_err(|e| e.to_string())?);
    let head = json!({
        "better": {"on_beats": "higher", "on_downbeats": "higher", "in_step": "higher", "tempo_exact": "higher"},
        "primary": ["on_downbeats"],
    });
    writeln!(file.lock().unwrap(), "{head}").map_err(|e| e.to_string())?;
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(s) = loops.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let one = match claudius::audio::load(&s.path) {
                        Ok(x) => x.mono(),
                        Err(e) => {
                            eprintln!("{}: {e:#}", s.id);
                            continue;
                        }
                    };
                    // the tempo the loop's length gives: the whole bars of four it holds at about the people's tempo
                    let (len, people) = (one.len() as f64 / SRF, s.tempo.unwrap_or_default());
                    let bars = (len * people / 240.0).round();
                    let exact = bars * 240.0 / len.max(1e-9);
                    if bars < 1.0 || (exact / people - 1.0).abs() > 0.02 {
                        eprintln!("{}: {len:.3} s is not a whole number of bars at {people} BPM", s.id);
                        continue;
                    }
                    let want = (tile * SRF) as usize;
                    let tiled: Vec<f32> = one.iter().copied().cycle().take(want.max(one.len())).collect();
                    let secs = tiled.len() as f64 / SRF;
                    let mut beats: Vec<f64> =
                        (0..).map(|k| k as f64 * 60.0 / exact).take_while(|t| *t < secs - 1e-6).collect();
                    let (mut x, mut theirs) = (Stereo::from_mono(&tiled), exact);
                    if let Some(depth) = drift {
                        let (played, table) = Drift::of(&s.id, depth).play(&x);
                        theirs = exact * secs / (played.l.len().max(1) as f64 / SRF);
                        beats = beats.iter().filter_map(|&u| sounds_at(&table, u)).collect();
                        x = played;
                    }
                    let downbeats: Vec<f64> = beats.iter().copied().step_by(4).collect();
                    let (bar_starts, bpm, steady) = bars_of(&x, None);
                    let mut metrics = json!({"tempo_exact": same_tempo(bpm, theirs, false)});
                    for (name, value) in [
                        ("on_beats", share_near(&bar_starts, &beats)),
                        ("on_downbeats", share_near(&bar_starts, &downbeats)),
                        ("in_step", in_step(&bar_starts, &beats)),
                    ] {
                        if let Some(v) = value {
                            metrics[name] = json!((v * 1000.0).round() / 1000.0);
                        }
                    }
                    let mut slice = slice_of(s);
                    slice["steady"] = json!(if steady { "steady" } else { "drifting" });
                    let line =
                        json!({"item": s.id, "slice": slice, "metrics": metrics, "bpm": (bpm * 10.0).round() / 10.0});
                    let _ = writeln!(file.lock().unwrap_or_else(|p| p.into_inner()), "{line}");
                }
            });
        }
    });
    Ok(())
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
