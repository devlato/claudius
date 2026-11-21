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
//! disk at once. `--shards` names shards and ranges of them, commas between (`0..4,15..19,35`). With `--top-up N`, it
//! then brings each genre up to N songs in each half from the other shards: of a genre's songs there, in the order of
//! their track ids' SHA-256 (fixed, and favouring no shard), as many as its half lacks, each fetched from its shard's
//! tar by byte ranges (the tar's headers read one by one, then the song read whole and kept only as an MP3 of the length
//! its header gives) and heard the same way.
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
//! - **beat**, with `--beats` (the real songs' beats, which `beats` hears): how far claudius's songs are from the real
//!   songs of their genre in how much their beat changes from bar to bar. A song's change is `Analysis::beat_change`'s
//!   `next`: of the kick, snare and hat hits on the 16th steps of two bars in a row, heard in its drums apart from the
//!   rest, the share in one bar and not both, the median over the bars of its middle 90 seconds; a genre's is the median over its songs, and
//!   `beat` is the difference from the real songs', shown beside both (`beat_songs`, `beat_real`). Lower is better: a
//!   beat that changes as much as the genre's real ones do, not more. Measured for a genre whose real songs in the half
//!   have a beat often enough to tell (`FEWEST`); a song of claudius's in which no kit is heard counts as a beat that
//!   never changes. A guard (worse in no genre) unless `--primary` names it, as a change to how beats are written does.
//!
//! With `--songs`, `run` also writes a line for each song (its genre and seed the item, its typical and whether it is
//! identified, with `--beats` its `beat`: how far its own change from bar to bar is from the real songs' middle one, and
//! its embedding), for a change to one genre or a few: a genre is one item of the lines above, and its songs, made with
//! the same seeds before and after the change, pair up one by one. `kad` tests such a change on two runs' lines: each
//! genre's kad before and after, as `run` measures it, the difference with the 95% interval of the paired bootstrap (the
//! seeds resampled 10,000 times, both runs' kad computed again on each resample, a seed's song never paired with itself)
//! and its p, and the same for the genres together, the mean of their differences. The change passes when kad is lower
//! overall and higher in no genre (Holm), and `bench compare` on the same lines finds typical and identified (and
//! `beat`) worse in no slice: they are guards, and `beat` a primary for a change to how drums are written
//! (`--primary beat`).
//!
//! `beats` hears the real songs' beats: for each song a line of `embed`'s lists (`--real`), how much its beat changes
//! from bar to bar, its stems apart as the studio hears a recording (`Analysis::beat_change`), a line each into `--out`
//! (a song already there is not heard again). A shard `embed --drop` deleted is downloaded again into DIR, and with
//! `--drop` deleted once its songs are heard.
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
//! cargo run --release -p claudius --example eval_genres -- embed DIR [--shards 0..99] [--top-up N] [--out FILE]
//!     [--drop] [--most 300] [--threads N]
//! cargo run --release -p claudius --example eval_genres -- beats DIR --real FILE [--out FILE] [--drop]
//!     [--threads N]
//! cargo run --release -p claudius --example eval_genres -- run REAL [--seeds 1..20] [--half tune|confirm]
//!     [--out FILE] [--genres a,b,...] [--songs FILE] [--beats FILE] [--primary a,b] [--threads N] [--keep DIR]
//! cargo run --release -p claudius --example eval_genres -- kad REAL BEFORE AFTER [--half tune|confirm]
//!     [--resamples 10000] [--out FILE]
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

use claudius::analysis::song::BeatChange;
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
        (Some("embed"), Some(dir)) => shard_list(flag("--shards").as_deref().unwrap_or("0..99")).and_then(|shards| {
            embed(
                Path::new(dir),
                &shards,
                &flag("--out").map_or_else(|| Path::new(dir).join("real-embeddings.jsonl"), PathBuf::from),
                args.iter().any(|a| a == "--drop"),
                flag("--most").and_then(|m| m.parse().ok()).unwrap_or(300),
                flag("--top-up").and_then(|n| n.parse().ok()),
                threads,
            )
        }),
        (Some("run"), Some(real)) => run(
            Path::new(real),
            &Runs {
                seeds: range("--seeds", (1, 20)),
                half: flag("--half").unwrap_or_else(|| "tune".into()),
                out: flag("--out").map_or_else(|| PathBuf::from("genres.jsonl"), PathBuf::from),
                only: flag("--genres").map(|g| g.split(',').map(String::from).collect()),
                keep: flag("--keep").map(PathBuf::from),
                songs: flag("--songs").map(PathBuf::from),
                beats: flag("--beats").map(PathBuf::from),
                primary: flag("--primary")
                    .map_or_else(|| vec!["kad".to_string()], |p| p.split(',').map(String::from).collect()),
            },
            threads,
        ),
        (Some("beats"), Some(dir)) => match flag("--real") {
            Some(real) => beats(
                Path::new(dir),
                Path::new(&real),
                &flag("--out").map_or_else(|| Path::new(dir).join("real-beats.jsonl"), PathBuf::from),
                args.iter().any(|a| a == "--drop"),
                threads,
            ),
            None => Err("beats needs --real FILE (embed's lines)".into()),
        },
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
        (Some("kad"), Some(real)) => match (args.get(2), args.get(3)) {
            (Some(before), Some(after)) if !before.starts_with("--") && !after.starts_with("--") => kad_paired(
                Path::new(real),
                Path::new(before),
                Path::new(after),
                &flag("--half").unwrap_or_else(|| "tune".into()),
                flag("--resamples").and_then(|f| f.parse().ok()).unwrap_or(RESAMPLES),
                flag("--out").map(PathBuf::from).as_deref(),
            ),
            _ => Err("kad needs two runs' songs: kad REAL BEFORE AFTER".into()),
        },
        _ => Err(
            "usage: eval_genres embed DIR [--shards 0..99] [--top-up N] [--out FILE] [--drop] [--threads N]\n       \
                  eval_genres beats DIR --real FILE [--out FILE] [--drop] [--threads N]\n       \
                  eval_genres run REAL [--seeds 1..20] [--half tune|confirm] [--out FILE] [--genres a,b] \
                  [--songs FILE] [--beats FILE] [--primary a,b] [--threads N] [--keep DIR]\n       \
                  eval_genres remake REAL --sources DIR --genre G [--how flip|rework] [--seeds 1..2] \
                  [--half tune|confirm] [--most 40] [--out FILE] [--primary a,b] [--threads N] [--keep DIR]\n       \
                  eval_genres kad REAL BEFORE AFTER [--half tune|confirm] [--resamples 10000] [--out FILE]"
                .into(),
        ),
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

/// shards as `--shards` names them: ranges and single shards, commas between (`0..4,15..19,35`)
fn shard_list(spec: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once("..").unwrap_or((part, part));
        let bad = || format!("--shards: {part} is not a shard or a range of shards from 0 to 99");
        let (a, b): (u32, u32) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
        if a > b || b > 99 {
            return Err(bad());
        }
        out.extend(a..=b);
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// the songs of shard `shard` in DIR that stand for a genre, in the dataset's order
fn reals(dir: &Path, shard: u32) -> Result<Vec<Real>, String> {
    reals_of(dir, |s| s == shard)
}

/// the songs of the shards `keep` takes that stand for a genre, in the dataset's order
fn reals_of(dir: &Path, keep: impl Fn(u32) -> bool) -> Result<Vec<Real>, String> {
    let tsv = std::fs::read_to_string(dir.join("raw_30s_cleantags.tsv")).map_err(|e| format!("the tags: {e}"))?;
    let mut out = Vec::new();
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
        let (Some(id), Some(path)) = (cols.first(), cols.get(3)) else { continue };
        let Some((s, file)) = path.split_once('/') else { continue };
        let Some(shard) = s.parse::<u32>().ok().filter(|&n| keep(n)) else { continue };
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

/// The real songs of `shards` that stand for a genre, heard and kept in `out`, a line each; with `top_up`, then the
/// songs of the other shards that bring each genre up to that many in each half (`top_up_songs`).
fn embed(
    dir: &Path,
    shards: &[u32],
    out: &Path,
    drop: bool,
    most: usize,
    top_up: Option<usize>,
    threads: usize,
) -> Result<(), String> {
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
        hear(&todo, &file, threads);
        // the marker goes with the audio, so that a later run that still needs a song of the shard (one that failed to
        // be heard, or another benchmark sharing the folder) downloads it again rather than finding nothing
        if drop {
            let _ = std::fs::remove_dir_all(dir.join(format!("{s:02}")));
            let _ = std::fs::remove_file(dir.join(format!("{s:02}.ok")));
        }
    }
    if let Some(want) = top_up {
        let todo: Vec<Real> =
            top_up_songs(dir, shards, want, threads)?.into_iter().filter(|r| !done.contains(&r.id)).collect();
        eprintln!("the other shards: hearing {} songs", todo.len());
        hear(&todo, &file, threads);
    }
    Ok(())
}

/// The songs of `todo` heard, each kept in `file` as a line, shared out among `threads`, each with ears of its own.
fn hear(todo: &[Real], file: &Mutex<std::fs::File>, threads: usize) {
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
}

/// The songs that bring each genre up to `want` in each half from the shards not among `shards`: of a genre's songs in
/// the other shards, in the order of their track ids' SHA-256 (fixed, and favouring no shard), as many as its half lacks
/// (its songs in `shards`, by the tags, counted against `want`). Each is fetched by byte ranges from its shard's tar on
/// the mirror, which serves them, `threads` shards at a time (four at most): the tar's headers read one by one, then
/// each song wanted read whole and kept only as an MP3 of the length its header gives (a song cannot be checked against
/// the published SHA-256, which is of the whole tar). A song already fetched is not fetched again, and a shard that
/// fails is said and left for the next run. The songs fetched, in the dataset's order.
fn top_up_songs(dir: &Path, shards: &[u32], want: usize, threads: usize) -> Result<Vec<Real>, String> {
    let all = reals_of(dir, |_| true)?;
    let chosen = choose(&all, shards, want);
    let mut by_shard: BTreeMap<u32, Vec<&Real>> = BTreeMap::new();
    for &i in &chosen {
        by_shard.entry(all[i].shard).or_default().push(&all[i]);
    }
    let jobs: Vec<(&u32, &Vec<&Real>)> = by_shard.iter().collect();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, 4) {
            scope.spawn(|| {
                while let Some(&(&s, songs)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match fetch_members(dir, s, songs) {
                        Ok(got) => eprintln!("shard {s:02}: {got} of {} fetched", songs.len()),
                        Err(e) => eprintln!("shard {s:02}: stopped ({e}); the rest are fetched on the next run"),
                    }
                }
            });
        }
    });
    let chosen: BTreeSet<usize> = chosen.into_iter().collect();
    Ok(all.into_iter().enumerate().filter(|(i, r)| chosen.contains(i) && r.path.exists()).map(|(_, r)| r).collect())
}

/// Which of `all` (the dataset's songs that stand for a genre) bring each genre up to `want` in each half from the shards
/// not among `shards`: a genre's songs in those shards, in the order of their track ids' SHA-256, as many as the songs it
/// has in `shards` fall short of `want`. Their places in `all`, genre by genre.
fn choose(all: &[Real], shards: &[u32], want: usize) -> Vec<usize> {
    use sha2::{Digest, Sha256};
    let have: BTreeSet<u32> = shards.iter().copied().collect();
    let mut count: BTreeMap<(&str, u32), usize> = BTreeMap::new();
    let mut others: BTreeMap<(&str, u32), Vec<(String, usize)>> = BTreeMap::new();
    for (i, r) in all.iter().enumerate() {
        if have.contains(&r.shard) {
            *count.entry((r.genre, r.shard % 2)).or_default() += 1;
        } else {
            let key: String = Sha256::digest(r.id.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
            others.entry((r.genre, r.shard % 2)).or_default().push((key, i));
        }
    }
    let mut chosen = Vec::new();
    for ((genre, half), mut songs) in others {
        let need = want.saturating_sub(count.get(&(genre, half)).copied().unwrap_or(0));
        songs.sort();
        if need > 0 {
            eprintln!(
                "{genre}, the {} half: {} more",
                if half == 0 { "tune" } else { "confirm" },
                need.min(songs.len())
            );
        }
        chosen.extend(songs.into_iter().take(need).map(|(_, i)| i));
    }
    chosen
}

/// The songs `songs` of shard `s`, fetched by byte ranges from its tar into DIR as `fetch_shard` would unpack them;
/// how many are there.
fn fetch_members(dir: &Path, s: u32, songs: &[&Real]) -> Result<usize, String> {
    let names: BTreeMap<String, &Real> = songs
        .iter()
        .filter_map(|r| Some((r.path.strip_prefix(dir).ok()?.to_string_lossy().replace('\\', "/"), *r)))
        .collect();
    if names.values().all(|r| r.path.exists()) {
        return Ok(names.len());
    }
    let url = format!("{MIRROR}/raw_30s_audio-low-{s:02}.tar");
    // each member's place and length, from its 512-byte header; the archive ends with a header of zeros
    let (mut found, mut at) = (BTreeMap::new(), 0u64);
    while found.len() < names.len() {
        let h = ranged(&url, at, at + 511)?;
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let field =
            |a: usize, b: usize| String::from_utf8_lossy(&h[a..b]).split('\0').next().unwrap_or("").trim().to_string();
        let size = u64::from_str_radix(&field(124, 136), 8).map_err(|e| format!("a header at {at}: {e}"))?;
        let name = field(0, 100);
        if names.contains_key(&name) {
            found.insert(name, (at + 512, size));
        }
        at += 512 + size.div_ceil(512) * 512;
    }
    std::fs::create_dir_all(dir.join(format!("{s:02}"))).map_err(|e| e.to_string())?;
    let mut got = 0;
    for (name, &(start, size)) in &found {
        let dest = &names[name].path;
        if dest.metadata().is_ok_and(|m| m.len() == size) {
            got += 1;
            continue;
        }
        let data = ranged(&url, start, start + size.max(1) - 1)?;
        if !(data.starts_with(b"ID3") || (data.len() > 1 && data[0] == 0xFF && data[1] & 0xE0 == 0xE0)) {
            eprintln!("shard {s:02}: {name} is not an MP3; left out");
            continue;
        }
        let part = dest.with_extension("part");
        std::fs::write(&part, &data).and_then(|()| std::fs::rename(&part, dest)).map_err(|e| e.to_string())?;
        got += 1;
    }
    if found.len() < names.len() {
        eprintln!("shard {s:02}: {} of the songs wanted are not in its tar", names.len() - found.len());
    }
    Ok(got)
}

/// bytes `a` to `b` of `url`, by an HTTP range request, tried seven times
fn ranged(url: &str, a: u64, b: u64) -> Result<Vec<u8>, String> {
    for t in 0..7u32 {
        if let Ok(o) = Command::new("curl").args(["-sf", "--max-time", "120", "-r", &format!("{a}-{b}"), url]).output()
            && o.status.success()
            && o.stdout.len() as u64 == b - a + 1
        {
            return Ok(o.stdout);
        }
        std::thread::sleep(std::time::Duration::from_secs(1 << t.min(5)));
    }
    Err(format!("{url}, bytes {a} to {b}: no answer"))
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

/// how many paired resamples of the seeds `kad`'s intervals and tests are drawn from, as the guide asks of a measure
/// of a whole set
const RESAMPLES: usize = 10_000;

/// One genre's songs from two runs, paired by their seeds, with the kernel values (`kad`'s, at `width` of the genre's
/// real songs) that each run's kad needs, on any resample of the seeds.
struct Paired {
    genre: String,
    seeds: Vec<u64>,
    real: usize,
    /// the kernel between the first run's songs, and between the second's
    bb: Vec<Vec<f64>>,
    aa: Vec<Vec<f64>>,
    /// each song's mean kernel with the real songs, the first run's and the second's
    rb: Vec<f64>,
    ra: Vec<f64>,
    /// the real songs' mean kernel between themselves
    rr: f64,
}

impl Paired {
    fn new(genre: &str, seeds: &[u64], before: &[Vec<f64>], after: &[Vec<f64>], reals: &[Vec<f64>]) -> Self {
        let sigma = width(reals);
        let k = |a: &[f64], b: &[f64]| (-dist2(a, b) / (2.0 * sigma * sigma)).exp();
        let grid =
            |p: &[Vec<f64>]| -> Vec<Vec<f64>> { p.iter().map(|u| p.iter().map(|v| k(u, v)).collect()).collect() };
        let to_real = |p: &[Vec<f64>]| -> Vec<f64> {
            p.iter().map(|u| reals.iter().map(|r| k(u, r)).sum::<f64>() / reals.len().max(1) as f64).collect()
        };
        let m = reals.len();
        let mut rr = 0.0;
        for (i, u) in reals.iter().enumerate() {
            for v in &reals[i + 1..] {
                rr += 2.0 * k(u, v);
            }
        }
        Paired {
            genre: genre.to_string(),
            seeds: seeds.to_vec(),
            real: m,
            bb: grid(before),
            aa: grid(after),
            rb: to_real(before),
            ra: to_real(after),
            rr: rr / (m * m.saturating_sub(1)).max(1) as f64,
        }
    }

    /// the kad of a run's songs of the seeds `picks` names (by place, a seed as often as a resample draws it), the
    /// second run's with `after`: `kad`'s sum from the kernel values kept, a seed's song never paired with itself
    fn kad(&self, picks: &[usize], after: bool) -> f64 {
        let (grid, real) = if after { (&self.aa, &self.ra) } else { (&self.bb, &self.rb) };
        let (mut within, mut pairs, mut across) = (0.0, 0usize, 0.0);
        for (a, &i) in picks.iter().enumerate() {
            across += real[i];
            for (b, &j) in picks.iter().enumerate() {
                if a != b && i != j {
                    within += grid[i][j];
                    pairs += 1;
                }
            }
        }
        100.0 * (within / pairs.max(1) as f64 + self.rr - 2.0 * across / picks.len().max(1) as f64)
    }

    /// the second run's kad less the first's, on the seeds `picks` names
    fn diff(&self, picks: &[usize]) -> f64 {
        self.kad(picks, true) - self.kad(picks, false)
    }
}

/// a row of `kad`'s report: a genre, or all of them together (the mean of their differences)
struct Tested {
    name: String,
    seeds: usize,
    real: Option<usize>,
    before: f64,
    after: f64,
    diff: f64,
    low: f64,
    high: f64,
    p: f64,
    holm: Option<f64>,
}

/// splitmix64: the resamples' draws, the same every run
fn draw(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// the 2.5th and 97.5th percentiles of `v`, and the two-sided p of a difference of 0 among them
fn interval(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(f64::total_cmp);
    let at = |q: f64| v[((q * (v.len() - 1) as f64).round() as usize).min(v.len() - 1)];
    let (above, below) = (v.iter().filter(|&&d| d >= 0.0).count(), v.iter().filter(|&&d| d <= 0.0).count());
    let p = (2.0 * (above.min(below) + 1) as f64 / (v.len() + 1) as f64).min(1.0);
    (at(0.025), at(0.975), p)
}

/// Each genre's difference in kad between two runs, and all of them together (the mean of the genres' differences),
/// by the guide's test of a measure of a whole set: the paired bootstrap, the seeds of each genre resampled
/// `resamples` times and both runs' kad computed again on each resample; the interval the resamples' percentiles, p
/// how often they put the difference the other side of none, each genre's Holm-corrected across the genres.
fn paired_test(rows: &[Paired], resamples: usize) -> Vec<Tested> {
    let mut state = 0x6b61_645f_7465_7374_u64;
    let whole: Vec<Vec<usize>> = rows.iter().map(|r| (0..r.seeds.len()).collect()).collect();
    let observed: Vec<f64> = rows.iter().zip(&whole).map(|(r, w)| r.diff(w)).collect();
    let (mut each, mut all) = (vec![Vec::with_capacity(resamples); rows.len()], Vec::with_capacity(resamples));
    for _ in 0..resamples {
        let mut total = 0.0;
        for (g, r) in rows.iter().enumerate() {
            let n = r.seeds.len() as u64;
            let picks: Vec<usize> = (0..n).map(|_| (draw(&mut state) % n) as usize).collect();
            let d = r.diff(&picks);
            each[g].push(d);
            total += d;
        }
        all.push(total / rows.len() as f64);
    }
    let tested: Vec<(f64, f64, f64)> = each.into_iter().map(interval).collect();
    // Holm's step-down correction of the genres' p across them
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| tested[a].2.total_cmp(&tested[b].2));
    let (mut holm, mut running) = (vec![0.0; rows.len()], 0.0_f64);
    for (rank, &g) in order.iter().enumerate() {
        running = running.max(((rows.len() - rank) as f64 * tested[g].2).min(1.0));
        holm[g] = running;
    }
    let mut out: Vec<Tested> = rows
        .iter()
        .enumerate()
        .map(|(g, r)| Tested {
            name: r.genre.clone(),
            seeds: r.seeds.len(),
            real: Some(r.real),
            before: r.kad(&whole[g], false),
            after: r.kad(&whole[g], true),
            diff: observed[g],
            low: tested[g].0,
            high: tested[g].1,
            p: tested[g].2,
            holm: Some(holm[g]),
        })
        .collect();
    let (low, high, p) = interval(all);
    let mean = |f: &dyn Fn(&Tested) -> f64| out.iter().map(f).sum::<f64>() / out.len() as f64;
    let all_row = Tested {
        name: "all".to_string(),
        seeds: out.iter().map(|t| t.seeds).sum(),
        real: None,
        before: mean(&|t| t.before),
        after: mean(&|t| t.after),
        diff: observed.iter().sum::<f64>() / rows.len() as f64,
        low,
        high,
        p,
        holm: None,
    };
    out.insert(0, all_row);
    out
}

/// each genre's songs of a run (`run --songs`), by seed: their embeddings
fn songs_of(path: &Path) -> Result<BTreeMap<String, BTreeMap<u64, Vec<f64>>>, String> {
    let mut out: BTreeMap<String, BTreeMap<u64, Vec<f64>>> = BTreeMap::new();
    for line in std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?.lines() {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("{}: {e}", path.display()))?;
        let Some((g, seed)) = v["item"].as_str().and_then(|i| i.rsplit_once(':')) else { continue };
        let seed: u64 = seed.parse().map_err(|_| format!("{}: {seed} is not a seed", path.display()))?;
        let emb = v["emb"].as_array().ok_or_else(|| {
            format!("{}: its songs have no embeddings (a run before they were written): run it again", path.display())
        })?;
        out.entry(g.to_string()).or_default().insert(seed, emb.iter().filter_map(Value::as_f64).collect());
    }
    Ok(out)
}

/// Two runs' songs (`run --songs`), made with the same seeds before and after a change, against the real songs of
/// their genres in `half`: each genre's kad before and after, as `run` measures it, and the same for the genres
/// together, tested by `paired_test`. The test of a change to one genre or a few, whose kad `run` gives as one number a
/// genre and `bench compare` cannot test: lower overall and higher in no genre, as `bench compare` judges a primary
/// metric. `out` keeps a line for each row.
fn kad_paired(
    real: &Path,
    before: &Path,
    after: &Path,
    half: &str,
    resamples: usize,
    out: Option<&Path>,
) -> Result<(), String> {
    let by_genre = real_songs(real, parity_of(half)?)?;
    let (first, second) = (songs_of(before)?, songs_of(after)?);
    let mut rows = Vec::new();
    for (g, xs) in &first {
        let (Some(ys), Some(reals)) = (second.get(g), by_genre.get(g)) else { continue };
        if reals.len() < FEWEST {
            eprintln!("{g}: {} real songs in the half, fewer than the {FEWEST} a measure needs", reals.len());
            continue;
        }
        let seeds: Vec<u64> = xs.keys().filter(|s| ys.contains_key(s)).copied().collect();
        if seeds.len() < 3 {
            eprintln!("{g}: fewer than three seeds in both runs");
            continue;
        }
        let pick = |m: &BTreeMap<u64, Vec<f64>>| -> Vec<Vec<f64>> { seeds.iter().map(|s| m[s].clone()).collect() };
        rows.push(Paired::new(g, &seeds, &pick(xs), &pick(ys), reals));
    }
    if rows.is_empty() {
        return Err("no genre is in both runs with enough real songs in the half".into());
    }
    let tested = paired_test(&rows, resamples);
    println!("### kad (lower is better; paired bootstrap over the seeds, {resamples} resamples)\n");
    println!("| | seeds | real | before | after | difference (95% interval) | p | p, Holm |");
    println!("|---|---:|---:|---:|---:|---|---:|---:|");
    for t in &tested {
        println!(
            "| {} | {} | {} | {:.4} | {:.4} | {:+.4} ({:+.4} to {:+.4}) | {:.4} | {} |",
            t.name,
            t.seeds,
            t.real.map_or_else(String::new, |r| r.to_string()),
            t.before,
            t.after,
            t.diff,
            t.low,
            t.high,
            t.p,
            t.holm.map_or_else(String::new, |h| format!("{h:.3}"))
        );
    }
    let all = &tested[0];
    let higher: Vec<&str> = tested[1..]
        .iter()
        .filter(|t| t.diff > 0.0 && t.holm.is_some_and(|h| h < 0.05))
        .map(|t| t.name.as_str())
        .collect();
    let verdict = if !higher.is_empty() {
        format!("does not pass: higher in {}", higher.join(", "))
    } else if all.diff < 0.0 && all.p < 0.05 {
        "passes: lower overall, and higher in no genre".to_string()
    } else {
        "does not pass: not shown lower overall".to_string()
    };
    println!("\n**kad: {verdict}.**");
    if let Some(out) = out {
        let mut f = std::fs::File::create(out).map_err(|e| e.to_string())?;
        for t in &tested {
            let line = json!({
                "genre": t.name, "seeds": t.seeds, "real": t.real, "before": hundredths(t.before),
                "after": hundredths(t.after), "difference": hundredths(t.diff), "low": hundredths(t.low),
                "high": hundredths(t.high), "p": t.p, "p_holm": t.holm,
            });
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
        }
        writeln!(f, "{}", json!({"verdict": format!("kad: {verdict}")})).map_err(|e| e.to_string())?;
    }
    Ok(())
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
/// A song of genre `g` made with `seed`, as `run` hears it: its embedding, and with `beats` how much its beat changes
/// from bar to bar, heard from the MP3 as a real song's is (`beat_of`).
fn hear_song(ears: &Ears, g: &genres::GenreProfile, seed: u64, dir: &Path, beats: bool) -> Result<Song, String> {
    let bp = compose::generate(g, seed, &Options { sampled: Some(false), ..Options::default() }, None);
    let project = compose::arrange(&bp);
    let r = render::render(&project, &render::Options::default()).map_err(|e| format!("{} {seed}: {e:#}", g.name))?;
    let spec: export::Spec = AS_THE_DATASET.parse().map_err(|e| format!("{e}"))?;
    let path = dir.join(format!("{}-{seed}.mp3", g.name));
    let bytes = export::encode(&r.master, &spec, &export::tags(&bp.title)).map_err(|e| format!("{e:#}"))?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let e = ears.embed_refs(std::slice::from_ref(&path)).map_err(|e| format!("{e:#}"))?;
    let emb = e.into_iter().next().map(|e| kept(&e)).ok_or_else(|| format!("{} {seed}: nothing heard", g.name))?;
    let beat = if beats { beat_of(&path)? } else { None };
    Ok(Song { emb, beat })
}

/// a song of claudius's as `run` keeps it: its seed, its embedding, and how much its beat changes bar to bar (none: no
/// kit heard)
type Composed = (u64, Vec<f64>, Option<f64>);

/// a song of claudius's as `run` hears it
struct Song {
    emb: Vec<f64>,
    beat: Option<BeatChange>,
}

/// how many seconds of a song `beat_of` hears: from its middle, where its beat plays (an intro and an outro thin out), and
/// so that hearing its stems apart takes a fraction of the time a whole song would
const BEAT_SECS: f64 = 90.0;

/// How much the beat of the recording at `path` changes from bar to bar, heard as the studio hears a recording: its
/// stems apart (the ears' model), its drums in its drums (`Analysis::beat_change`), the middle `BEAT_SECS` of it (all of
/// a shorter one), as of every song, real or claudius's; none where no kit plays.
fn beat_of(path: &Path) -> Result<Option<BeatChange>, String> {
    let whole = claudius::audio::load(path).map_err(|e| format!("{}: {e:#}", path.display()))?;
    let keep = ((BEAT_SECS * claudius::config::SRF) as usize).min(whole.len());
    let from = (whole.len() - keep) / 2;
    let x = whole.slice(from, from + keep);
    let stems = claudius::stems::of(&x, &mut |_| {});
    Ok(claudius::analysis::song::analyze_with(&x, stems.as_deref(), None).beat_change())
}

/// The beat of each real song that `real` lists (`embed`'s lines), a line each into `out` (`beat_of`; a song already
/// there is not heard again, so a run stopped part way goes on). A shard whose audio is no longer in DIR (`embed
/// --drop` deleted it) is downloaded again, and with `drop` deleted once its songs are heard.
fn beats(dir: &Path, real: &Path, out: &Path, drop: bool, threads: usize) -> Result<(), String> {
    fetch_tags(dir)?;
    let lines = std::fs::read_to_string(real).map_err(|e| format!("{}: {e}", real.display()))?;
    let listed: Vec<(String, u32, String)> = lines
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| {
            Some((v["item"].as_str()?.to_string(), v["shard"].as_u64()? as u32, v["genre"].as_str()?.to_string()))
        })
        .collect();
    let done: BTreeSet<String> = std::fs::read_to_string(out)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok()?["item"].as_str().map(String::from))
        .collect();
    let tsv = std::fs::read_to_string(dir.join("raw_30s_cleantags.tsv")).map_err(|e| format!("the tags: {e}"))?;
    let paths: BTreeMap<&str, PathBuf> = tsv
        .lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
            let (s, file) = cols.get(3)?.split_once('/')?;
            Some((*cols.first()?, dir.join(s).join(file.replace(".mp3", ".low.mp3"))))
        })
        .collect();
    let mut by_shard: BTreeMap<u32, Vec<(String, String, PathBuf)>> = BTreeMap::new();
    for (id, shard, genre) in listed.into_iter().filter(|(id, ..)| !done.contains(id)) {
        let path = paths.get(id.as_str()).cloned().ok_or_else(|| format!("{id} is not in the dataset's tags"))?;
        by_shard.entry(shard).or_default().push((id, genre, path));
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    let file = Mutex::new(file);
    for (s, todo) in by_shard {
        // the shard downloaded again where its audio is gone
        let fetched = !todo.iter().all(|(.., p)| p.is_file());
        if fetched {
            let _ = std::fs::remove_file(dir.join(format!("{s:02}.ok")));
            fetch_shard(dir, s)?;
        }
        eprintln!("shard {s:02}: the beats of {} songs", todo.len());
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    while let Some((id, genre, path)) = todo.get(next.fetch_add(1, Ordering::Relaxed)) {
                        match beat_of(path) {
                            Ok(beat) => {
                                let line = json!({"item": id, "shard": s, "genre": genre, "beat": beat});
                                let mut f = file.lock().unwrap_or_else(|p| p.into_inner());
                                let _ = writeln!(f, "{line}");
                            }
                            Err(e) => eprintln!("{id}: {e}"),
                        }
                    }
                });
            }
        });
        if drop && fetched {
            let _ = std::fs::remove_dir_all(dir.join(format!("{s:02}")));
            let _ = std::fs::remove_file(dir.join(format!("{s:02}.ok")));
        }
    }
    Ok(())
}

/// the real songs' beats that `beats` wrote, by genre, of the shards of `parity`: how much each changes bar to bar
fn real_beats(path: &Path, parity: u64) -> Result<BTreeMap<String, Vec<f64>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for v in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
        if let (Some(g), Some(s), Some(next)) = (v["genre"].as_str(), v["shard"].as_u64(), v["beat"]["next"].as_f64())
            && s % 2 == parity
        {
            out.entry(g.to_string()).or_default().push(next);
        }
    }
    Ok(out)
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
    /// the real songs' beats (`beats`' lines), to hear how much claudius's beats change bar to bar beside them
    beats: Option<PathBuf>,
    /// the metrics a change is judged on (`--primary`, kad unless told otherwise)
    primary: Vec<String>,
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
    let real_beat = r.beats.as_deref().map(|p| real_beats(p, parity)).transpose()?;
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
    let (tx, rx) = mpsc::channel::<(String, u64, Result<Song, String>)>();
    let with_beats = real_beat.is_some();
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
                    let heard = genres::get(g)
                        .map_err(|e| format!("{e:#}"))
                        .and_then(|p| hear_song(&ears, &p, seed, scratch, with_beats));
                    let _ = tx.send((g.to_string(), seed, heard));
                }
            });
        }
        drop(tx);
    });
    // each song's embedding, and with the real songs' beats how much its own changes bar to bar (none: no kit heard)
    let mut songs: BTreeMap<String, Vec<Composed>> = BTreeMap::new();
    for (g, seed, heard) in rx {
        match heard {
            Ok(song) => songs.entry(g).or_default().push((seed, song.emb, song.beat.map(|b| b.next))),
            Err(e) => eprintln!("{e}"),
        }
    }
    let mut f = std::fs::File::create(&r.out).map_err(|e| e.to_string())?;
    // with the real songs' beats, how far claudius's beats are from theirs in how much they change bar to bar (lower is
    // better: as much as theirs, not more), and each's middle shown
    let (better, shown) = if with_beats {
        (json!({"kad": "lower", "beat": "lower"}), json!(["spread", "beat_songs", "beat_real"]))
    } else {
        (json!({"kad": "lower"}), json!(["spread"]))
    };
    let head = json!({"better": better, "primary": r.primary, "shown": shown});
    writeln!(f, "{head}").map_err(|e| e.to_string())?;
    // each song as an item of its own: how alike it is to the real songs of its genre most like it, and whether it is
    // heard as its genre, paired across two runs by its genre and seed
    let mut each = match &r.songs {
        Some(p) => {
            let mut file = std::fs::File::create(p).map_err(|e| e.to_string())?;
            // what a change is made to improve is its genres' kad, which `kad` tests on these lines: typical and
            // identified are guards, and so is each song's beat (with the real songs' beats) unless --primary names it,
            // as a change to how a genre's drums are written does
            let primary: Vec<&str> =
                if with_beats && r.primary.iter().any(|m| m == "beat") { vec!["beat"] } else { Vec::new() };
            let head = if with_beats {
                json!({"better": {"beat": "lower"}, "primary": primary})
            } else {
                json!({"primary": primary})
            };
            writeln!(file, "{head}").map_err(|e| e.to_string())?;
            Some(file)
        }
        None => None,
    };
    println!("genre\treal\tsongs\tkad\ttypical\tidentified\tspread");
    for (g, family, ..) in measured {
        let reals = &by_genre[*g];
        let mut mine = songs.remove(*g).unwrap_or_default();
        mine.sort_by_key(|(s, ..)| *s);
        // the real songs' middle change, where they have a beat often enough to tell (`FEWEST`, a matter of the real
        // songs alone, so that two runs measure the same genres); a song of claudius's in which no kit is heard counts
        // as a beat that never changes
        let theirs =
            real_beat.as_ref().and_then(|b| b.get(*g)).filter(|t| t.len() >= FEWEST).map(|t| median(t.clone()));
        if let Some(file) = each.as_mut() {
            for (seed, e, beat) in &mine {
                let mut metrics = json!({"typical": hundredths(typical_of(e, reals)), "identified": heard_as(e, &centroids) == Some(*g)});
                if let Some(theirs) = theirs {
                    metrics["beat"] = json!(hundredths((beat.unwrap_or(0.0) - theirs).abs()));
                }
                let line = json!({
                    "item": format!("{g}:{seed}"),
                    "slice": {"family": family, "genre": g},
                    "metrics": metrics,
                    "emb": e,
                });
                writeln!(file, "{line}").map_err(|e| e.to_string())?;
            }
        }
        let beats_of_mine: Vec<f64> = mine.iter().map(|(_, _, b)| b.unwrap_or(0.0)).collect();
        let mine: Vec<Vec<f64>> = mine.into_iter().map(|(_, e, _)| e).collect();
        if mine.len() < 2 {
            eprintln!("{g}: fewer than two songs heard");
            continue;
        }
        let sigma = width(reals);
        let typical = mine.iter().map(|e| typical_of(e, reals)).sum::<f64>() / mine.len() as f64;
        let identified = mine.iter().filter(|e| heard_as(e, &centroids) == Some(*g)).count() as f64 / mine.len() as f64;
        let (kad, spread) = (kad(&mine, reals, sigma), spread(&mine) / spread(reals).max(1e-9));
        println!("{g}\t{}\t{}\t{kad:.3}\t{typical:.3}\t{identified:.2}\t{spread:.2}", reals.len(), mine.len());
        let mut metrics = json!({
            "kad": hundredths(kad),
            "typical": hundredths(typical),
            "identified": hundredths(identified),
            "spread": hundredths(spread),
        });
        if let Some(theirs) = theirs {
            let ours = median(beats_of_mine);
            println!("{g}\tbeat change: claudius's {ours:.3}, the real songs' {theirs:.3}");
            metrics["beat"] = json!(hundredths((ours - theirs).abs()));
            metrics["beat_songs"] = json!(hundredths(ours));
            metrics["beat_real"] = json!(hundredths(theirs));
        }
        let line = json!({"item": g, "slice": {"family": family}, "metrics": metrics});
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` points in eight dimensions about `centre`, the same for the same `seed`
    fn cloud(n: usize, centre: f64, seed: u64) -> Vec<Vec<f64>> {
        let mut state = seed;
        (0..n).map(|_| (0..8).map(|_| centre + (draw(&mut state) % 1000) as f64 / 1000.0 - 0.5).collect()).collect()
    }

    #[test]
    fn shards_are_named_by_ranges_and_single_shards() {
        assert_eq!(shard_list("0..4,15..19,35").unwrap(), [0, 1, 2, 3, 4, 15, 16, 17, 18, 19, 35]);
        assert_eq!(shard_list("7").unwrap(), [7]);
        assert_eq!(shard_list("3..5,4").unwrap(), [3, 4, 5], "once each");
        assert!(shard_list("5..2").is_err() && shard_list("0..100").is_err() && shard_list("x").is_err());
    }

    #[test]
    fn a_top_up_takes_what_each_half_lacks_in_a_fixed_order_from_the_other_shards() {
        let song =
            |id: &str, shard: u32, genre: &'static str| Real { id: id.into(), shard, path: PathBuf::new(), genre };
        let all = vec![
            song("track_1", 0, "dnb"),
            song("track_2", 2, "dnb"),
            song("track_3", 1, "dnb"),
            song("track_4", 10, "dnb"),
            song("track_5", 12, "dnb"),
            song("track_6", 14, "dnb"),
            song("track_7", 11, "dnb"),
            song("track_8", 10, "house"),
        ];
        // dnb has two songs in the even shards it was given and one in the odd: three wanted in each half
        let chosen = choose(&all, &[0, 1, 2], 3);
        let even: Vec<usize> =
            chosen.iter().copied().filter(|&i| all[i].shard % 2 == 0 && all[i].genre == "dnb").collect();
        let odd: Vec<usize> = chosen.iter().copied().filter(|&i| all[i].shard % 2 == 1).collect();
        assert_eq!((even.len(), odd.len()), (1, 1), "one more even, one more odd (the only one there): {chosen:?}");
        assert!(even.iter().chain(&odd).all(|&i| ![0, 1, 2].contains(&all[i].shard)), "from the other shards only");
        assert_eq!(
            chosen.iter().filter(|&&i| all[i].genre == "house").count(),
            1,
            "house has none: it gets what is there"
        );
        // the order is the track ids' SHA-256, so the same songs every time, whatever order the tags list them in
        let shuffled: Vec<Real> = all.iter().rev().map(|r| song(&r.id, r.shard, r.genre)).collect();
        let again: BTreeSet<String> =
            choose(&shuffled, &[0, 1, 2], 3).into_iter().map(|i| shuffled[i].id.clone()).collect();
        let first: BTreeSet<String> = chosen.iter().map(|&i| all[i].id.clone()).collect();
        assert_eq!(again, first);
    }

    #[test]
    fn a_paired_kad_is_the_runs_own_and_tells_a_real_change_from_none() {
        let reals = cloud(30, 0.0, 1);
        let (before, after) = (cloud(12, 0.8, 2), cloud(12, 0.1, 3));
        let seeds: Vec<u64> = (1..=12).collect();
        let whole: Vec<usize> = (0..12).collect();
        let p = Paired::new("g", &seeds, &before, &after, &reals);
        // each run's kad on all its seeds is `kad`'s, which `run` reports
        let sigma = width(&reals);
        assert!((p.kad(&whole, false) - kad(&before, &reals, sigma)).abs() < 1e-9);
        assert!((p.kad(&whole, true) - kad(&after, &reals, sigma)).abs() < 1e-9);
        // songs moved towards the real ones: lower, the whole interval below none, and seldom by chance
        let t = paired_test(std::slice::from_ref(&p), 2000);
        assert!(t[0].diff < 0.0 && t[0].high < 0.0 && t[0].p < 0.01, "{} {} {}", t[0].diff, t[0].high, t[0].p);
        assert!((t[0].diff - t[1].diff).abs() < 1e-12, "one genre: all of them is that genre");
        // the same songs in both runs: no difference on any resample
        let same = Paired::new("g", &seeds, &before, &before, &reals);
        let t = paired_test(std::slice::from_ref(&same), 2000);
        assert!(t[1].diff.abs() < 1e-12 && t[1].low.abs() < 1e-12 && t[1].high.abs() < 1e-12 && t[1].p > 0.99);
    }
}
