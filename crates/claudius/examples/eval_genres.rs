//! How alike claudius's songs are to real songs of their genre: the yardstick for a change to the generator (the
//! contributor guide's Benchmarks), which a comparison of claudius's songs judged by its own critic can never be.
//!
//! The real songs are MTG-Jamendo's (<https://github.com/MTG/mtg-jamendo-dataset>), as `eval_critic` takes them: 55,701
//! songs by independent artists, released on Jamendo under Creative Commons licences between 2004 and 2017, each tagged
//! with its genres, in 100 shards of about 557 songs, each a random one percent of the dataset. `GENRES` says which of
//! claudius's genres a song stands for, by its tags; a genre with no honest tag of its own (amapiano, phonk, synthwave
//! ...) is not measured. Each song, real or claudius's, is heard as the ears hear a song for its typicality: CLAP's
//! embedding of it, the mean of up to six windows of ten seconds spread across it, read from an MP3 at the dataset's
//! bitrate so that both come through the same encoder and decoder.
//!
//! `embed` hears the real songs: it downloads each shard of `--shards` into DIR (checked against its published
//! SHA-256, as `eval_critic` downloads them), hears the songs of it that stand for a genre, at most `--most` (300) of a
//! genre in each half of the shards (`run`'s halves: ambient alone has 6,825 songs, hip-hop 2,196, trap 42), adds a line
//! for each to `--out` (a song already there is not heard again, so a run stopped part way goes on), and with
//! `--drop`, deletes the shard's audio once it is heard, so that all 100 shards (about 170 GB) never have to be on the
//! disk at once.
//!
//! `run` hears claudius's: for each genre measured, the songs of `--seeds` composed and rendered as they are made
//! (on the installed sample packs, with no samples fetched from online libraries, so that a run does not depend on
//! what a library offers that day), each against the real songs of its genre in `--half`: `tune`, the even shards, on
//! which a change is tried, or `confirm`, the odd ones, on which it is confirmed (with seeds of its own). It writes a
//! line for `cargo xtask bench compare` for each genre (after one that says which way each metric is better):
//!
//! - **kad**, the kernel audio distance between claudius's songs and the real ones (Chung et al. 2025: the unbiased
//!   maximum mean discrepancy with a Gaussian kernel, its width the median distance between the real songs, times
//!   100): how far apart the two are as wholes, both how each sounds and how much they vary. What a change to the
//!   generator is made to improve, lower is better.
//! - **typical**, the mean over claudius's songs of how alike each is to the ten real songs most like it (cosine).
//! - **identified**, the share of claudius's songs heard as their own genre: nearer to its real songs' centroid than
//!   to any other genre's.
//! - **spread**, how much claudius's songs differ from one another, as a share of how much the real ones do (mean
//!   cosine distance between pairs): shown, never judged, as neither more nor less is better by itself.
//!
//! With `--songs`, `run` also writes a line for each song (its genre and seed the item, its typical and whether it is
//! identified), the yardstick for a change to one genre: a genre is one item of the lines above, and its songs, made
//! with the same seeds before and after the change, pair up one by one.
//!
//! `remake` hears claudius's remakes of real songs in a genre (`--how`: `flip`, a new song on a recording's hook and
//! pieces of it, or `rework`, the recording all through with the genre under it), the yardstick for a change to how a
//! recording is remade: the dataset's songs in the shards of `--half` that are in `--sources` (a folder `embed` or
//! `eval_critic` downloaded into), at most `--most` (40) of them, those tagged with a genre people remake (`SOURCES`:
//! pop, rock, dance, hip hop and the like) and with none of the target genre's own tags, each remade with each of
//! `--seeds` (1..2) as the studio remakes it (its stems separated by the ears, which may download their model), against
//! the real songs of the genre in the same half. It writes a line for each remake (after one that says which way each
//! metric is better, and which `--primary` names: `typical` unless told otherwise):
//!
//! - **typical**, how alike the remake is to the ten real songs of the genre most like it (cosine).
//! - **identified**, whether it is heard as the genre: nearer to its real songs' centroid than to any other genre's.
//! - **slowed**, how far the remake moved the recording from its own speed: the absolute natural log of the rate it
//!   plays the recording at (a flip its hook, a rework all of it), as the song counts it, at its own tempo, half or
//!   twice it; 0 for a recording played at its own speed, 0.29 for one slowed or sped by a quarter. Lower is better.
//!
//! and prints the kernel audio distance between the remakes and the real songs beside the means. Each remake's embedding
//! goes beside the results, in `FILE.embeddings.jsonl`.
//!
//! `run` and `remake` render, separate and hear in a scratch folder of their own in the system's temporary folder,
//! removed when they end; `--keep DIR` works in DIR instead and leaves it there.
//!
//! ```text
//! cargo run --release -p claudius --example eval_genres -- embed DIR [--shards 0..99] [--out FILE] [--drop]
//!     [--most 300] [--threads N]
//! cargo run --release -p claudius --example eval_genres -- run REAL [--seeds 1..20] [--half tune|confirm]
//!     [--out FILE] [--genres a,b,...] [--songs FILE] [--threads N] [--keep DIR]
//! cargo run --release -p claudius --example eval_genres -- remake REAL --sources DIR --genre G
//!     [--how flip|rework] [--seeds 1..2] [--half tune|confirm] [--most 40] [--out FILE] [--primary a,b]
//!     [--threads N] [--keep DIR]
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};

use claudius::compose::{self, Options};
use claudius::ears::Ears;
use claudius::model::{Blueprint, Sound};
use claudius::{export, genres, render};
use serde_json::{Value, json};

const META: &str = "https://raw.githubusercontent.com/MTG/mtg-jamendo-dataset/master/data";
const MIRROR: &str = "https://cdn.freesound.org/mtg-jamendo/raw_30s/audio-low";
/// the dataset's MP3s, which claudius's songs are written as before they are heard
const AS_THE_DATASET: &str = "mp3:96@44100";

/// Which songs of the dataset stand for each of claudius's genres: a genre, the family it is counted under, the tags a
/// song has at least one of, and those it has none of (a deep house song is not one of house's).
const GENRES: &[(&str, &str, &[&str], &[&str])] = &[
    ("house", "electronic", &["house"], &["deephouse", "techhouse", "electrohouse", "progressive", "acidhouse"]),
    ("deep_house", "electronic", &["deephouse"], &[]),
    ("tech_house", "electronic", &["techhouse"], &[]),
    ("electro_house", "electronic", &["electrohouse"], &[]),
    ("techno", "electronic", &["techno", "minimaltechno"], &["technoindustrial"]),
    ("trance", "electronic", &["trance"], &["psytrance"]),
    ("dnb", "electronic", &["drumnbass"], &["jungle"]),
    ("jungle", "electronic", &["jungle"], &[]),
    ("dubstep", "electronic", &["dubstep"], &[]),
    ("hardstyle", "electronic", &["hardstyle"], &[]),
    ("big_beat", "electronic", &["breakbeat"], &[]),
    ("idm", "electronic", &["idm"], &[]),
    ("ambient", "electronic", &["ambient"], &["darkambient"]),
    ("trip_hop", "electronic", &["triphop"], &[]),
    ("dance_pop", "pop", &["dancepop", "electropop"], &[]),
    ("nu_disco", "pop", &["disco"], &[]),
    ("lofi", "hip-hop", &["lofi"], &[]),
    ("hip_hop", "hip-hop", &["hiphop", "rap"], &["trap", "lofi"]),
    ("trap", "hip-hop", &["trap"], &[]),
    ("rnb", "soul", &["rnb"], &[]),
    ("funk", "soul", &["funk"], &["jazzfunk"]),
    ("reggae", "reggae", &["reggae"], &["dub", "dancehall"]),
    ("reggaeton", "latin", &["reggaeton"], &[]),
    ("alt_rock", "rock", &["alternativerock", "grunge"], &[]),
    ("pop_punk", "rock", &["punkrock"], &[]),
];

/// the real songs of a genre a measure needs at least, and the songs most like one that `typical` reads
const FEWEST: usize = 15;
const NEAREST: usize = 10;

/// The dataset's genres whose songs `remake` remakes: what people take to a remake, not soundtracks, classical music
/// or ambient beds. A song counts under the first of these it is tagged with.
const SOURCES: &[&str] = &[
    "pop",
    "rock",
    "dance",
    "hiphop",
    "rap",
    "rnb",
    "soul",
    "funk",
    "disco",
    "house",
    "techno",
    "trance",
    "electropop",
    "dancepop",
    "poprock",
    "indie",
    "alternative",
    "punkrock",
    "metal",
    "reggae",
    "latin",
    "folk",
    "popfolk",
    "country",
    "blues",
    "jazz",
    "electronic",
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let range = |name: &str, default: (u64, u64)| -> Vec<u64> {
        let (a, b) = flag(name)
            .and_then(|r| r.split_once("..").map(|(a, b)| (a.parse().ok(), b.parse().ok())))
            .and_then(|(a, b)| Some((a?, b?)))
            .unwrap_or(default);
        (a..=b).collect()
    };
    let threads = flag("--threads").and_then(|t| t.parse().ok()).unwrap_or_else(|| {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        (cores / 4).max(1)
    });
    let result = match (args.first().map(String::as_str), args.get(1)) {
        (Some("embed"), Some(dir)) => embed(
            Path::new(dir),
            &range("--shards", (0, 99)).into_iter().map(|s| s as u32).collect::<Vec<_>>(),
            &flag("--out").map_or_else(|| Path::new(dir).join("real-embeddings.jsonl"), PathBuf::from),
            args.iter().any(|a| a == "--drop"),
            flag("--most").and_then(|m| m.parse().ok()).unwrap_or(300),
            threads,
        ),
        (Some("run"), Some(real)) => run(
            Path::new(real),
            &Runs {
                seeds: range("--seeds", (1, 20)),
                half: flag("--half").unwrap_or_else(|| "tune".into()),
                out: flag("--out").map_or_else(|| PathBuf::from("genres.jsonl"), PathBuf::from),
                only: flag("--genres").map(|g| g.split(',').map(String::from).collect()),
                keep: flag("--keep").map(PathBuf::from),
                songs: flag("--songs").map(PathBuf::from),
            },
            threads,
        ),
        (Some("remake"), Some(real)) => match (flag("--sources"), flag("--genre")) {
            (Some(sources), Some(genre)) => remake(
                Path::new(real),
                &Remakes {
                    sources: PathBuf::from(sources),
                    genre,
                    how: flag("--how").unwrap_or_else(|| "flip".into()),
                    seeds: range("--seeds", (1, 2)),
                    half: flag("--half").unwrap_or_else(|| "tune".into()),
                    most: flag("--most").and_then(|m| m.parse().ok()).unwrap_or(40),
                    primary: flag("--primary")
                        .map_or_else(|| vec!["typical".to_string()], |p| p.split(',').map(String::from).collect()),
                },
                &flag("--out").map_or_else(|| PathBuf::from("remakes.jsonl"), PathBuf::from),
                threads,
                flag("--keep").map(PathBuf::from),
            ),
            _ => Err("remake needs --sources DIR and --genre G".into()),
        },
        _ => Err("usage: eval_genres embed DIR [--shards 0..99] [--out FILE] [--drop] [--threads N]\n       \
                  eval_genres run REAL [--seeds 1..20] [--half tune|confirm] [--out FILE] [--genres a,b] \
                  [--songs FILE] [--threads N] [--keep DIR]\n       \
                  eval_genres remake REAL --sources DIR --genre G [--how flip|rework] [--seeds 1..2] \
                  [--half tune|confirm] [--most 40] [--out FILE] [--primary a,b] [--threads N] [--keep DIR]"
            .into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

/// the genre a song with these tags stands for, if any
fn genre_of(tags: &BTreeSet<&str>) -> Option<&'static str> {
    GENRES
        .iter()
        .find(|(_, _, any, none)| any.iter().any(|t| tags.contains(t)) && !none.iter().any(|t| tags.contains(t)))
        .map(|(g, ..)| *g)
}

/// a song of the dataset that stands for a genre
struct Real {
    id: String,
    shard: u32,
    path: PathBuf,
    genre: &'static str,
}

/// the songs of shard `shard` in DIR that stand for a genre, in the dataset's order
fn reals(dir: &Path, shard: u32) -> Result<Vec<Real>, String> {
    let tsv = std::fs::read_to_string(dir.join("raw_30s_cleantags.tsv")).map_err(|e| format!("the tags: {e}"))?;
    let mut out = Vec::new();
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
        let (Some(id), Some(path)) = (cols.first(), cols.get(3)) else { continue };
        let Some((s, file)) = path.split_once('/') else { continue };
        if s.parse::<u32>().ok() != Some(shard) {
            continue;
        }
        let tags: BTreeSet<&str> = cols.iter().filter_map(|c| c.strip_prefix("genre---")).collect();
        if let Some(genre) = genre_of(&tags) {
            out.push(Real {
                id: id.to_string(),
                shard,
                path: dir.join(s).join(file.replace(".mp3", ".low.mp3")),
                genre,
            });
        }
    }
    Ok(out)
}

fn sh(cmd: &mut Command) -> Result<(), String> {
    let s = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if s.success() { Ok(()) } else { Err(format!("{cmd:?} failed ({s})")) }
}

fn sha256(path: &Path) -> Result<String, String> {
    for tool in [&["shasum", "-a", "256"][..], &["sha256sum"][..]] {
        if let Ok(o) = Command::new(tool[0]).args(&tool[1..]).arg(path).output()
            && o.status.success()
        {
            return Ok(String::from_utf8_lossy(&o.stdout).split_whitespace().next().unwrap_or_default().to_string());
        }
    }
    Err("neither shasum nor sha256sum is installed".into())
}

/// the tags, once, into `dir`
fn fetch_tags(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (file, url) in [
        ("raw_30s_cleantags.tsv", format!("{META}/raw_30s_cleantags.tsv")),
        ("sha256_tars.txt", format!("{META}/download/raw_30s_audio-low_sha256_tars.txt")),
    ] {
        if !dir.join(file).exists() {
            sh(Command::new("curl").args(["-sfL", "--retry", "5", "-o"]).arg(dir.join(file)).arg(url))?;
        }
    }
    Ok(())
}

/// shard `s`, downloaded into `dir` unless it is there, checked against its published SHA-256
fn fetch_shard(dir: &Path, s: u32) -> Result<(), String> {
    if dir.join(format!("{s:02}.ok")).exists() {
        return Ok(());
    }
    let sums = std::fs::read_to_string(dir.join("sha256_tars.txt")).map_err(|e| e.to_string())?;
    let tar = format!("raw_30s_audio-low-{s:02}.tar");
    let part = dir.join(format!("{tar}.part"));
    eprintln!("downloading shard {s:02} (1.7 GB)");
    let got = sh(Command::new("curl")
        .args(["-fsL", "--retry", "5", "-C", "-", "-o"])
        .arg(&part)
        .arg(format!("{MIRROR}/{tar}")));
    let want = sums.lines().find(|l| l.ends_with(&tar)).and_then(|l| l.split_whitespace().next());
    if !part.exists() || want != Some(sha256(&part)?.as_str()) {
        got?;
        let _ = std::fs::remove_file(&part);
        return Err(format!("{tar} does not match its published SHA-256"));
    }
    sh(Command::new("tar").arg("xf").arg(&part).args(["--no-same-owner", "-C"]).arg(dir))?;
    std::fs::remove_file(&part).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{s:02}.ok")), "").map_err(|e| e.to_string())
}

/// an embedding as kept: four places are plenty for a cosine
fn kept(e: &[f32]) -> Vec<f64> {
    e.iter().map(|x| (f64::from(*x) * 1e4).round() / 1e4).collect()
}

/// The real songs of `shards` that stand for a genre, heard and kept in `out`, a line each.
fn embed(dir: &Path, shards: &[u32], out: &Path, drop: bool, most: usize, threads: usize) -> Result<(), String> {
    fetch_tags(dir)?;
    let before: Vec<Value> = std::fs::read_to_string(out)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let done: BTreeSet<String> = before.iter().filter_map(|v| v["item"].as_str().map(String::from)).collect();
    // how many of each genre each half of the shards has heard
    let mut heard: BTreeMap<(String, u64), usize> = BTreeMap::new();
    for v in &before {
        if let (Some(g), Some(s)) = (v["genre"].as_str(), v["shard"].as_u64()) {
            *heard.entry((g.to_string(), s % 2)).or_default() += 1;
        }
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    let file = Mutex::new(file);
    for &s in shards {
        let mut todo: Vec<Real> = Vec::new();
        for r in reals(dir, s)?.into_iter().filter(|r| !done.contains(&r.id)) {
            let n = heard.entry((r.genre.to_string(), u64::from(r.shard % 2))).or_default();
            if *n < most {
                *n += 1;
                todo.push(r);
            }
        }
        if todo.is_empty() {
            continue;
        }
        fetch_shard(dir, s)?;
        eprintln!("shard {s:02}: hearing {} songs", todo.len());
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    let ears = match Ears::load(false) {
                        Ok(e) => e,
                        Err(e) => return eprintln!("no ears: {e:#}"),
                    };
                    while let Some(r) = todo.get(next.fetch_add(1, Ordering::Relaxed)) {
                        match ears.embed_refs(std::slice::from_ref(&r.path)).map(|mut v| v.pop()) {
                            Ok(Some(e)) => {
                                let line = json!({"item": r.id, "shard": r.shard, "genre": r.genre, "emb": kept(&e)});
                                let mut f = file.lock().unwrap_or_else(|p| p.into_inner());
                                let _ = writeln!(f, "{line}");
                            }
                            Ok(None) => eprintln!("{}: nothing heard", r.id),
                            Err(e) => eprintln!("{}: {e:#}", r.id),
                        }
                    }
                });
            }
        });
        // the marker goes with the audio, so that a later run that still needs a song of the shard (one that failed to
        // be heard, or another benchmark sharing the folder) downloads it again rather than finding nothing
        if drop {
            let _ = std::fs::remove_dir_all(dir.join(format!("{s:02}")));
            let _ = std::fs::remove_file(dir.join(format!("{s:02}.ok")));
        }
    }
    Ok(())
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    ab / (aa.sqrt() * bb.sqrt()).max(1e-12)
}

fn dist2(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() { 0.0 } else { v[v.len() / 2] }
}

/// The unbiased maximum mean discrepancy between `x` and `y` with a Gaussian kernel of width `sigma`, times 100 (the
/// kernel audio distance, Chung et al. 2025).
fn kad(x: &[Vec<f64>], y: &[Vec<f64>], sigma: f64) -> f64 {
    let k = |a: &[f64], b: &[f64]| (-dist2(a, b) / (2.0 * sigma * sigma)).exp();
    let within = |s: &[Vec<f64>]| {
        let n = s.len() as f64;
        let mut t = 0.0;
        for (i, a) in s.iter().enumerate() {
            for b in &s[i + 1..] {
                t += 2.0 * k(a, b);
            }
        }
        t / (n * (n - 1.0)).max(1.0)
    };
    let mut across = 0.0;
    for a in x {
        for b in y {
            across += k(a, b);
        }
    }
    across /= (x.len() * y.len()).max(1) as f64;
    100.0 * (within(x) + within(y) - 2.0 * across)
}

/// mean cosine distance between the pairs of `s`
fn spread(s: &[Vec<f64>]) -> f64 {
    let mut t = (0.0, 0usize);
    for (i, a) in s.iter().enumerate() {
        for b in &s[i + 1..] {
            t = (t.0 + 1.0 - cosine(a, b), t.1 + 1);
        }
    }
    t.0 / t.1.max(1) as f64
}

fn centroid(s: &[Vec<f64>]) -> Vec<f64> {
    let mut c = vec![0.0; s.first().map_or(0, Vec::len)];
    for e in s {
        for (ci, x) in c.iter_mut().zip(e) {
            *ci += x / s.len() as f64;
        }
    }
    c
}

fn hundredths(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// the shards' parity a half takes: the even ones to tune on, the odd ones to confirm on
fn parity_of(half: &str) -> Result<u64, String> {
    match half {
        "tune" => Ok(0),
        "confirm" => Ok(1),
        other => Err(format!("--half is tune or confirm, not {other}")),
    }
}

/// the real songs `embed` heard in the shards of `parity`, by genre
fn real_songs(real: &Path, parity: u64) -> Result<BTreeMap<String, Vec<Vec<f64>>>, String> {
    let mut by_genre: BTreeMap<String, Vec<Vec<f64>>> = BTreeMap::new();
    for line in std::fs::read_to_string(real).map_err(|e| format!("{}: {e}", real.display()))?.lines() {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if v["shard"].as_u64().is_some_and(|s| s % 2 == parity)
            && let (Some(g), Some(e)) = (v["genre"].as_str(), v["emb"].as_array())
        {
            by_genre.entry(g.to_string()).or_default().push(e.iter().filter_map(Value::as_f64).collect());
        }
    }
    Ok(by_genre)
}

/// each genre's centroid, of those with enough real songs to measure
fn centroids_of(by_genre: &BTreeMap<String, Vec<Vec<f64>>>) -> Vec<(String, Vec<f64>)> {
    by_genre.iter().filter(|(_, s)| s.len() >= FEWEST).map(|(g, s)| (g.clone(), centroid(s))).collect()
}

/// the kernel's width from the real songs alone, so that it is the same for every version of the generator
fn width(reals: &[Vec<f64>]) -> f64 {
    let mut d = Vec::new();
    for (i, a) in reals.iter().enumerate() {
        for b in &reals[i + 1..] {
            d.push(dist2(a, b).sqrt());
        }
    }
    median(d).max(1e-6)
}

/// how alike a song is to the `NEAREST` real songs most like it
fn typical_of(e: &[f64], reals: &[Vec<f64>]) -> f64 {
    let mut c: Vec<f64> = reals.iter().map(|r| cosine(e, r)).collect();
    c.sort_by(|a, b| b.total_cmp(a));
    c.iter().take(NEAREST).sum::<f64>() / NEAREST.min(c.len()).max(1) as f64
}

/// the genre whose real songs' centroid a song is nearest
fn heard_as<'a>(e: &[f64], centroids: &'a [(String, Vec<f64>)]) -> Option<&'a str> {
    centroids.iter().max_by(|a, b| cosine(e, &a.1).total_cmp(&cosine(e, &b.1))).map(|(g, _)| g.as_str())
}

/// A song of genre `g` made with `seed`, rendered as it is made, written at the dataset's bitrate into `dir` and heard.
fn hear_song(ears: &Ears, g: &genres::GenreProfile, seed: u64, dir: &Path) -> Result<Vec<f64>, String> {
    let bp = compose::generate(g, seed, &Options { sampled: Some(false), ..Options::default() }, None);
    let project = compose::arrange(&bp);
    let r = render::render(&project, &render::Options::default()).map_err(|e| format!("{} {seed}: {e:#}", g.name))?;
    let spec: export::Spec = AS_THE_DATASET.parse().map_err(|e| format!("{e}"))?;
    let path = dir.join(format!("{}-{seed}.mp3", g.name));
    let bytes = export::encode(&r.master, &spec, &export::tags(&bp.title)).map_err(|e| format!("{e:#}"))?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let e = ears.embed_refs(std::slice::from_ref(&path)).map_err(|e| format!("{e:#}"))?;
    e.into_iter().next().map(|e| kept(&e)).ok_or_else(|| format!("{} {seed}: nothing heard", g.name))
}

/// Each measured genre's songs of `seeds`, heard against its real songs of `half`, a line each into `out`.
/// what `run` makes and where it writes it
struct Runs {
    seeds: Vec<u64>,
    half: String,
    out: PathBuf,
    /// the genres measured, when not every one
    only: Option<Vec<String>>,
    keep: Option<PathBuf>,
    /// where to write a line for each song as well (`--songs`), to compare a change to one genre song by song
    songs: Option<PathBuf>,
}

/// The folder a run renders, separates and hears in: its own in the system's temporary folder, removed when the run
/// ends however it ends, or the one `--keep` names, which stays (each run left about 1.7 GB of remakes behind before).
struct Scratch {
    dir: PathBuf,
    keep: bool,
}

impl Scratch {
    fn new(keep: Option<PathBuf>, name: &str) -> Result<Scratch, String> {
        let (dir, keep) = match keep {
            Some(dir) => (dir, true),
            None => (std::env::temp_dir().join(format!("{name}-{}", std::process::id())), false),
        };
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Scratch { dir, keep })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn run(real: &Path, r: &Runs, threads: usize) -> Result<(), String> {
    let (seeds, only) = (&r.seeds, &r.only);
    let parity = parity_of(&r.half)?;
    let by_genre = real_songs(real, parity)?;
    let centroids = centroids_of(&by_genre);
    let measured: Vec<&(&str, &str, &[&str], &[&str])> = GENRES
        .iter()
        .filter(|(g, ..)| by_genre.get(*g).is_some_and(|s| s.len() >= FEWEST))
        .filter(|(g, ..)| only.as_ref().is_none_or(|o| o.iter().any(|x| x == g)))
        .collect();
    let held = Scratch::new(r.keep.clone(), "eval-genres")?;
    let scratch = &held.dir;
    // every song of every genre, shared out among the threads, each with ears of its own
    let jobs: Vec<(&str, u64)> = measured.iter().flat_map(|(g, ..)| seeds.iter().map(move |&s| (*g, s))).collect();
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<(String, u64, Result<Vec<f64>, String>)>();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let tx = tx.clone();
            let (jobs, next) = (&jobs, &next);
            scope.spawn(move || {
                let ears = match Ears::load(false) {
                    Ok(e) => e,
                    Err(e) => return eprintln!("no ears: {e:#}"),
                };
                while let Some(&(g, seed)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let heard =
                        genres::get(g).map_err(|e| format!("{e:#}")).and_then(|p| hear_song(&ears, &p, seed, scratch));
                    let _ = tx.send((g.to_string(), seed, heard));
                }
            });
        }
        drop(tx);
    });
    let mut songs: BTreeMap<String, Vec<(u64, Vec<f64>)>> = BTreeMap::new();
    for (g, seed, heard) in rx {
        match heard {
            Ok(e) => songs.entry(g).or_default().push((seed, e)),
            Err(e) => eprintln!("{e}"),
        }
    }
    let mut f = std::fs::File::create(&r.out).map_err(|e| e.to_string())?;
    let head = json!({"better": {"kad": "lower"}, "primary": ["kad"], "shown": ["spread"]});
    writeln!(f, "{head}").map_err(|e| e.to_string())?;
    // each song as an item of its own: how alike it is to the real songs of its genre most like it, and whether it is
    // heard as its genre, paired across two runs by its genre and seed
    let mut each = match &r.songs {
        Some(p) => {
            let mut file = std::fs::File::create(p).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json!({"primary": ["typical"]})).map_err(|e| e.to_string())?;
            Some(file)
        }
        None => None,
    };
    println!("genre\treal\tsongs\tkad\ttypical\tidentified\tspread");
    for (g, family, ..) in measured {
        let reals = &by_genre[*g];
        let mut mine = songs.remove(*g).unwrap_or_default();
        mine.sort_by_key(|(s, _)| *s);
        if let Some(file) = each.as_mut() {
            for (seed, e) in &mine {
                let line = json!({
                    "item": format!("{g}:{seed}"),
                    "slice": {"family": family, "genre": g},
                    "metrics": {"typical": hundredths(typical_of(e, reals)), "identified": heard_as(e, &centroids) == Some(*g)},
                });
                writeln!(file, "{line}").map_err(|e| e.to_string())?;
            }
        }
        let mine: Vec<Vec<f64>> = mine.into_iter().map(|(_, e)| e).collect();
        if mine.len() < 2 {
            eprintln!("{g}: fewer than two songs heard");
            continue;
        }
        let sigma = width(reals);
        let typical = mine.iter().map(|e| typical_of(e, reals)).sum::<f64>() / mine.len() as f64;
        let identified = mine.iter().filter(|e| heard_as(e, &centroids) == Some(*g)).count() as f64 / mine.len() as f64;
        let (kad, spread) = (kad(&mine, reals, sigma), spread(&mine) / spread(reals).max(1e-9));
        println!("{g}\t{}\t{}\t{kad:.3}\t{typical:.3}\t{identified:.2}\t{spread:.2}", reals.len(), mine.len());
        let line = json!({
            "item": g,
            "slice": {"family": family},
            "metrics": {
                "kad": hundredths(kad),
                "typical": hundredths(typical),
                "identified": hundredths(identified),
                "spread": hundredths(spread),
            },
        });
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// a remake heard, or why not: its embedding and how far it moved its recording from its own speed (`slowed`)
type Heard = Result<(Vec<f64>, f64), String>;

/// what `remake` remakes and how
struct Remakes {
    /// the folder of the dataset's audio and its tags
    sources: PathBuf,
    genre: String,
    /// `flip` or `rework`
    how: String,
    seeds: Vec<u64>,
    half: String,
    /// the most songs remade
    most: usize,
    /// the metrics a change is made to improve
    primary: Vec<String>,
}

/// a real song a remake is made of
struct Source {
    id: String,
    path: PathBuf,
    /// the first of `SOURCES` it is tagged with
    tag: &'static str,
}

/// The songs of the shards of `parity` in `dir` that a remake in `genre` is made of: tagged with one of `SOURCES` and
/// with none of the genre's own tags (a trip hop song remade as trip hop says nothing), whose audio is there, in the
/// dataset's order, `most` at most.
fn sources(dir: &Path, parity: u64, genre: &str, most: usize) -> Result<Vec<Source>, String> {
    let tsv = std::fs::read_to_string(dir.join("raw_30s_cleantags.tsv")).map_err(|e| format!("the tags: {e}"))?;
    let own: &[&str] = GENRES.iter().find(|(g, ..)| *g == genre).map_or(&[], |(_, _, tags, _)| *tags);
    let mut out = Vec::new();
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
        let (Some(id), Some(path)) = (cols.first(), cols.get(3)) else { continue };
        let Some((s, file)) = path.split_once('/') else { continue };
        if !s.parse::<u64>().is_ok_and(|s| s % 2 == parity) {
            continue;
        }
        let tags: BTreeSet<&str> = cols.iter().filter_map(|c| c.strip_prefix("genre---")).collect();
        if own.iter().any(|t| tags.contains(t)) {
            continue;
        }
        let Some(tag) = SOURCES.iter().find(|t| tags.contains(*t)) else { continue };
        let audio = dir.join(s).join(file.replace(".mp3", ".low.mp3"));
        if audio.is_file() {
            out.push(Source { id: id.to_string(), path: audio, tag });
        }
        if out.len() == most {
            break;
        }
    }
    Ok(out)
}

/// How far a remake moved its recording from the recording's own speed: the absolute natural log of the rate it plays
/// it at (a flip its hook and the passages it plays in its sections, `sampling::PHRASE` and the names it numbers from
/// it; a rework all of it, `remix::ORIGINAL` or its voice), as the song counts it: at its own tempo, half or twice it.
/// None when no recording plays in it.
fn slowed(bp: &Blueprint) -> Option<f64> {
    let whole = [claudius::remix::ORIGINAL, claudius::remix::VOICE];
    let recording = |name: &str| name.starts_with(claudius::sampling::PHRASE) || whole.contains(&name);
    let own = bp.loops.iter().filter(|l| recording(&l.name)).find_map(|l| match &l.sound {
        Sound::Loop { bpm, .. } if *bpm > 0.0 => Some(*bpm),
        _ => None,
    })?;
    [0.5, 1.0, 2.0].into_iter().map(|k: f64| (k * bp.bpm / own).ln().abs()).min_by(f64::total_cmp)
}

/// The recording at `src` remade in genre `g` as `how` says with each of `seeds`, as the studio remakes it (its stems
/// apart, analysed with them): each seed's embedding and how far it moved the recording from its own speed
/// (`heard_remake`).
fn remake_song(
    ears: &Ears,
    lib: Option<&dyn compose::Library>,
    g: &genres::GenreProfile,
    how: &str,
    src: &Source,
    seeds: &[u64],
    dir: &Path,
) -> Vec<(u64, Heard)> {
    let x = match claudius::audio::load(&src.path) {
        Ok(x) => x,
        Err(e) => return seeds.iter().map(|&s| (s, Err(format!("{}: {e:#}", src.id)))).collect(),
    };
    let stems = claudius::stems::of(&x, &mut |_| {});
    let a = claudius::analysis::song::analyze_with(&x, stems.as_deref(), None);
    let name = src.path.file_name().map_or_else(|| src.id.clone(), |n| n.to_string_lossy().into_owned());
    let s = claudius::remix::Source { x: &x, name: &name, a: &a, stems: stems.as_deref() };
    let made = Made { ears, lib, g, how, id: &src.id, dir };
    seeds.iter().map(|&seed| (seed, heard_remake(&made, &s, seed))).collect()
}

/// what a remake of one recording is made with, and where it is written
struct Made<'a> {
    ears: &'a Ears,
    lib: Option<&'a dyn compose::Library>,
    g: &'a genres::GenreProfile,
    how: &'a str,
    id: &'a str,
    dir: &'a Path,
}

/// The recording `s` remade with `seed`, rendered as it is made, written at the dataset's bitrate and heard: its
/// embedding and how far it moved the recording from its own speed (`slowed`).
fn heard_remake(m: &Made, s: &claudius::remix::Source, seed: u64) -> Heard {
    let takes = m.dir.join(format!("{}-{seed}", m.id));
    let opts = Options::default();
    let bp = match m.how {
        "flip" => claudius::remix::remix(s, m.g, seed, &opts, m.lib, &takes, Some(m.ears)),
        "rework" => claudius::remix::rework(s, m.g, seed, &opts, m.lib, &takes),
        other => Err(format!("--how is flip or rework, not {other}")),
    }
    .map_err(|e| format!("{} {seed}: {e}", m.id))?;
    let moved = slowed(&bp).ok_or_else(|| format!("{} {seed}: no recording plays in it", m.id))?;
    let project = compose::arrange(&bp);
    let r = render::render(&project, &render::Options::default()).map_err(|e| format!("{} {seed}: {e:#}", m.id))?;
    let spec: export::Spec = AS_THE_DATASET.parse().map_err(|e| format!("{e}"))?;
    let path = m.dir.join(format!("{}-{seed}.mp3", m.id));
    let bytes = export::encode(&r.master, &spec, &export::tags(&bp.title)).map_err(|e| format!("{e:#}"))?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let e = m.ears.embed_refs(std::slice::from_ref(&path)).map_err(|e| format!("{e:#}"))?;
    let e = e.into_iter().next().map(|e| kept(&e)).ok_or_else(|| format!("{} {seed}: nothing heard", m.id))?;
    Ok((e, moved))
}

/// The sources' remakes of `r`, heard against the real songs of the genre in the same half, a line each into `out`.
fn remake(real: &Path, r: &Remakes, out: &Path, threads: usize, keep: Option<PathBuf>) -> Result<(), String> {
    let parity = parity_of(&r.half)?;
    let g = genres::get(&r.genre).map_err(|e| format!("{e:#}"))?;
    let by_genre = real_songs(real, parity)?;
    let centroids = centroids_of(&by_genre);
    let reals = by_genre
        .get(&r.genre)
        .filter(|s| s.len() >= FEWEST)
        .ok_or_else(|| format!("{} has fewer than {FEWEST} real songs in the {} half", r.genre, r.half))?;
    let songs = sources(&r.sources, parity, &r.genre, r.most)?;
    if songs.is_empty() {
        return Err(format!("no song to remake in {} for the {} half", r.sources.display(), r.half));
    }
    // a song's stems are separated as the studio separates them, the ears' model downloaded first when it is not there
    claudius::stems::may_download();
    let held = Scratch::new(keep, "eval-remakes")?;
    let scratch = &held.dir;
    eprintln!("remaking {} songs as {} ({}), seeds {:?}", songs.len(), r.genre, r.how, r.seeds);
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<(usize, u64, Heard)>();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let tx = tx.clone();
            let (songs, next, g) = (&songs, &next, &g);
            scope.spawn(move || {
                let ears = match Ears::load(false) {
                    Ok(e) => e,
                    Err(e) => return eprintln!("no ears: {e:#}"),
                };
                // the samples installed here, as the studio's remakes draw on them, and none fetched
                let lib = claudius::library::Library::open().ok().filter(|l| !l.is_empty());
                let lib_ref = lib.as_ref().map(|l| l as &dyn compose::Library);
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(song) = songs.get(i) else { break };
                    for (seed, heard) in remake_song(&ears, lib_ref, g, &r.how, song, &r.seeds, scratch) {
                        let _ = tx.send((i, seed, heard));
                    }
                }
            });
        }
        drop(tx);
    });
    let mut heard: BTreeMap<(usize, u64), (Vec<f64>, f64)> = BTreeMap::new();
    for (i, seed, h) in rx {
        match h {
            Ok(h) => {
                heard.insert((i, seed), h);
            }
            Err(e) => eprintln!("{e}"),
        }
    }
    if heard.len() < 2 {
        return Err("fewer than two remakes heard".into());
    }
    let mut f = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let head = json!({"better": {"slowed": "lower"}, "primary": r.primary});
    writeln!(f, "{head}").map_err(|e| e.to_string())?;
    // each remake's embedding beside the results, where the scratch folder that goes at the end is not
    let mut embs = std::fs::File::create(out.with_extension("embeddings.jsonl")).map_err(|e| e.to_string())?;
    let (mut typical, mut identified, mut moved) = (0.0, 0usize, 0.0);
    for ((i, seed), (e, slow)) in &heard {
        let src = &songs[*i];
        let item = format!("{}:{seed}", src.id);
        let t = typical_of(e, reals);
        let id = heard_as(e, &centroids) == Some(r.genre.as_str());
        typical += t;
        identified += usize::from(id);
        moved += slow;
        let line = json!({
            "item": item,
            "slice": {"source": src.tag},
            "metrics": {"typical": hundredths(t), "identified": id, "slowed": hundredths(*slow)},
        });
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
        writeln!(embs, "{}", json!({"item": item, "emb": e})).map_err(|e| e.to_string())?;
    }
    let mine: Vec<Vec<f64>> = heard.values().map(|(e, _)| e.clone()).collect();
    let n = heard.len() as f64;
    println!("genre\thow\treal\tremakes\tkad\ttypical\tidentified\tslowed");
    println!(
        "{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.2}\t{:.3}",
        r.genre,
        r.how,
        reals.len(),
        heard.len(),
        kad(&mine, reals, width(reals)),
        typical / n,
        identified as f64 / n,
        moved / n
    );
    Ok(())
}
