//! How the critic judges real music: songs people made and released, judged as they are and with a flaw any listener
//! hears put into each, so that a change to the critic is measured on music it was not made from (the contributor
//! guide's Benchmarks).
//!
//! The songs are MTG-Jamendo's (<https://github.com/MTG/mtg-jamendo-dataset>): 55,701 songs by independent artists,
//! released on Jamendo under Creative Commons licences between 2004 and 2017, each tagged with its genres and often
//! its instruments. The dataset comes in 100 shards of about 557 songs each, each shard a random one percent of it.
//! `--shards A..B` (0..2 by default) downloads those shards into DIR the first time, from the dataset's own mirror,
//! each checked against its published SHA-256 (MP3s at a low bitrate, 1.7 GB a shard), and keeps them there; none of
//! them goes into the repository. It holds no chart hits, and little trap, drill or K-pop.
//!
//! Each song is judged as it is, then with each of these flaws, turned to the song's own loudness so that only the
//! flaw tells them apart:
//!
//! - **clipped**: driven 15 dB into a hard ceiling;
//! - **muddy**: 12 dB more at 250 Hz, and next to nothing above 3 kHz;
//! - **thin**: next to nothing below 500 Hz;
//! - **noisy**: white noise 18 dB under the song;
//! - **gaps**: a quarter of a second of silence every two seconds;
//! - **detuned**: the song a semitone higher laid over it;
//! - **dragging**: the song 1.5% slower laid over it, falling out of time with it.
//!
//! It writes a line for `cargo xtask bench compare` (after one that says which way each metric is better) for each song
//! as it is, with its **total** and whether it is **condemned** (under 40, the studio's red), and for each flawed copy
//! whether the song was **ranked** above it, by how much (**gap**), and whether the flawed copy was **praised** (80 or
//! more, the studio's green, which a song with such a flaw should never get). `ranked` is what a change to the critic
//! is made to improve; the others are guards. Each line is sliced by the flaw, the song's family of genres (by its
//! genre tags) and whether it is tagged as sung.
//! A run stopped part way goes on where it stopped when started again with the same `--out`.
//!
//! The ears judge as the studio does when ONNX Runtime and their models are installed; `--no-ears` judges with the
//! meters alone. Each of the `--threads` (by default as many as the ears' threads leave cores for) loads its own ears.
//!
//!   cargo run --release -p claudius --example eval_critic -- DIR [--shards 0..2] [--songs N] [--out FILE]
//!       [--no-ears] [--threads N]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};

use claudius::audio::{self, Stereo};
use claudius::config::SRF;
use claudius::dsp::filter::{Biquad, BiquadKind};
use claudius::ears::Ears;
use serde_json::{Value, json};

const META: &str = "https://raw.githubusercontent.com/MTG/mtg-jamendo-dataset/master/data";
const MIRROR: &str = "https://cdn.freesound.org/mtg-jamendo/raw_30s/audio-low";

/// the studio's green and red: a flawed song praised, a released one condemned
const PRAISED: f64 = 80.0;
const CONDEMNED: f64 = 40.0;

/// the families of genres a song is counted under: the first whose tags it carries
const FAMILIES: &[(&str, &[&str])] = &[
    (
        "hip-hop",
        &[
            "hiphop",
            "rap",
            "trap",
            "gangstarap",
            "gangsta",
            "instrumentalhiphop",
            "rapfrancais",
            "abstracthiphop",
            "christianrap",
            "rapcore",
        ],
    ),
    ("reggae", &["reggae", "dub", "dancehall", "ska", "ragga", "rocksteady"]),
    (
        "metal",
        &[
            "metal",
            "heavymetal",
            "thrashmetal",
            "deathmetal",
            "death",
            "powermetal",
            "progressivemetal",
            "metalcore",
            "blackmetal",
            "numetal",
            "doom",
            "grindcore",
        ],
    ),
    (
        "electronic",
        &[
            "electronic",
            "dance",
            "techno",
            "house",
            "trance",
            "downtempo",
            "triphop",
            "trip",
            "electropop",
            "industrial",
            "synthpop",
            "minimal",
            "club",
            "dubstep",
            "drumnbass",
            "deephouse",
            "breakbeat",
            "electronica",
            "idm",
            "darkwave",
            "edm",
            "eurodance",
            "lofi",
            "technoindustrial",
            "electrohouse",
            "glitch",
            "psytrance",
            "chiptune",
            "8bit",
            "acid",
            "dancepop",
            "electrorock",
            "jungle",
            "hardstyle",
            "minimaltechno",
            "ebm",
            "acidhouse",
            "techhouse",
            "technopop",
            "indietronica",
            "trancemelodique",
            "rave",
            "progressivehouse",
            "technohouse",
            "elektro",
            "synthwave",
            "electrodance",
            "darkelectro",
            "breakcore",
            "dubelectro",
            "hardtrance",
            "electropunk",
            "dancehouse",
            "trancetechno",
            "tekno",
            "hardtek",
            "computermusic",
        ],
    ),
    (
        "rock",
        &[
            "rock",
            "alternative",
            "indie",
            "poprock",
            "punkrock",
            "progressive",
            "instrumentalrock",
            "psychedelic",
            "hardrock",
            "postrock",
            "grunge",
            "rocknroll",
            "alternativerock",
            "classicrock",
            "gothic",
            "progressiverock",
            "hardcore",
            "newwave",
            "powerpop",
            "ethnicrock",
            "bluesrock",
            "garage",
            "rockfrancais",
            "artrock",
            "southernrock",
            "postpunk",
            "shoegaze",
            "indierock",
            "acidrock",
            "stoner",
            "goth",
            "guitarrock",
            "emo",
            "dreampop",
            "britpop",
            "indierockpop",
            "countryrock",
            "spacerock",
            "krautrock",
            "jrock",
            "acousticrock",
            "experimentalrock",
            "glam",
            "poppunk",
            "emocore",
            "deutschrock",
        ],
    ),
    (
        "jazz",
        &[
            "jazz",
            "lounge",
            "fusion",
            "acidjazz",
            "jazzfusion",
            "jazzfunk",
            "swing",
            "bossanova",
            "jazzrock",
            "electrojazz",
            "smoothjazz",
            "souljazz",
            "nujazz",
            "freejazz",
            "bebop",
            "ragtime",
            "manouche",
            "chilloutlounge",
        ],
    ),
    ("soul, funk and blues", &["funk", "blues", "rnb", "soul", "disco", "groove", "popfunk", "gospel"]),
    (
        "latin and world",
        &[
            "world",
            "latin",
            "ethno",
            "worldfusion",
            "tribal",
            "oriental",
            "flamenco",
            "african",
            "asian",
            "reggaeton",
            "latino",
            "middleeastern",
            "worldmusik",
            "samba",
            "salsa",
            "tango",
            "island",
            "guitarras",
            "gipsy",
        ],
    ),
    ("folk and country", &["folk", "popfolk", "country", "celtic", "americana"]),
    (
        "pop",
        &[
            "pop",
            "instrumentalpop",
            "singersongwriter",
            "chanson",
            "chansonfrancaise",
            "indiepop",
            "variete",
            "cabaret",
        ],
    ),
    ("soundtrack", &["soundtrack", "score"]),
    (
        "classical",
        &[
            "classical",
            "orchestral",
            "symphonic",
            "contemporary",
            "choir",
            "baroque",
            "neoclassical",
            "medieval",
            "choral",
            "waltz",
            "opera",
            "march",
            "minimalism",
        ],
    ),
    (
        "ambient and chill",
        &[
            "ambient",
            "easylistening",
            "chillout",
            "newage",
            "atmospheric",
            "darkambient",
            "drone",
            "soundscapes",
            "downbeat",
            "electroambient",
            "droneambient",
        ],
    ),
    ("experimental", &["experimental", "improvisation", "avantgarde", "noise"]),
];

/// the flaws, each a way a song goes wrong that any listener hears
#[derive(Clone, Copy, Debug)]
enum Flaw {
    Clipped,
    Muddy,
    Thin,
    Noisy,
    Gaps,
    Detuned,
    Dragging,
}

impl Flaw {
    const ALL: [Flaw; 7] =
        [Flaw::Clipped, Flaw::Muddy, Flaw::Thin, Flaw::Noisy, Flaw::Gaps, Flaw::Detuned, Flaw::Dragging];

    fn name(self) -> &'static str {
        match self {
            Flaw::Clipped => "clipped",
            Flaw::Muddy => "muddy",
            Flaw::Thin => "thin",
            Flaw::Noisy => "noisy",
            Flaw::Gaps => "gaps",
            Flaw::Detuned => "detuned",
            Flaw::Dragging => "dragging",
        }
    }

    /// `x` with the flaw, at its loudness (and no louder at its peaks than it was)
    fn put(self, x: &Stereo, seed: u64) -> Stereo {
        let mut y = match self {
            Flaw::Clipped => {
                let g = audio::db_to_gain(15.0);
                let mut y = x.clone();
                y.map(|c| c.iter_mut().for_each(|v| *v = (*v * g).clamp(-1.0, 1.0)));
                y
            }
            Flaw::Muddy => filtered(
                x,
                &[
                    (BiquadKind::Peak, 250.0, 1.0, 12.0),
                    (BiquadKind::Lowpass, 3000.0, 0.707, 0.0),
                    (BiquadKind::Lowpass, 3000.0, 0.707, 0.0),
                ],
            ),
            Flaw::Thin => {
                filtered(x, &[(BiquadKind::Highpass, 500.0, 0.707, 0.0), (BiquadKind::Highpass, 500.0, 0.707, 0.0)])
            }
            Flaw::Noisy => {
                let level = x.rms() * audio::db_to_gain(-18.0) * 3f32.sqrt();
                let mut rng = seed | 1;
                let mut y = x.clone();
                y.map(|c| {
                    for v in c.iter_mut() {
                        // xorshift64*: the same noise for the same song every run
                        rng ^= rng >> 12;
                        rng ^= rng << 25;
                        rng ^= rng >> 27;
                        let u = (rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32;
                        *v += (2.0 * u - 1.0) * level;
                    }
                });
                y
            }
            Flaw::Gaps => {
                let mut y = x.clone();
                y.map(|c| {
                    for (i, v) in c.iter_mut().enumerate() {
                        if (i as f64 / SRF) % 2.0 >= 1.75 {
                            *v = 0.0;
                        }
                    }
                });
                y
            }
            Flaw::Detuned => laid_over(x, &claudius::warp::stretch(x, x.len(), 1, false)),
            Flaw::Dragging => {
                laid_over(x, &claudius::warp::stretch(x, (x.len() as f64 * 1.015).round() as usize, 0, false))
            }
        };
        let to_loudness =
            audio::db_to_gain((claudius::loudness::integrated(x) - claudius::loudness::integrated(&y)) as f32);
        let to_peak = x.peak() / y.peak().max(1e-9);
        let g = if to_loudness.is_finite() { to_loudness.min(to_peak) } else { to_peak.min(1.0) };
        y.scale(g);
        y
    }

    /// does it need the stretcher, which only a build with the `native` feature has?
    fn stretches(self) -> bool {
        matches!(self, Flaw::Detuned | Flaw::Dragging)
    }
}

/// `x` through the filters (kind, frequency, Q, gain in dB) one after another, each channel apart
fn filtered(x: &Stereo, filters: &[(BiquadKind, f64, f64, f64)]) -> Stereo {
    let mut y = x.clone();
    y.map(|c| {
        for &(kind, f, q, db) in filters {
            Biquad::new(kind, f, SRF, q, db).process(c);
        }
    });
    y
}

/// `x` with `other` laid over it at 70%, as long as `x`
fn laid_over(x: &Stereo, other: &Stereo) -> Stereo {
    let mut y = x.clone();
    let mut o = other.clone();
    o.ensure_len(x.len());
    o.truncate(x.len());
    y.mix(&o, 0.7);
    y
}

/// a song of the dataset, with the labels it is counted under
struct Song {
    id: String,
    path: PathBuf,
    family: &'static str,
    sung: bool,
}

fn family(tags: &BTreeSet<&str>) -> &'static str {
    FAMILIES.iter().find(|(_, ts)| ts.iter().any(|t| tags.contains(t))).map_or("other", |(f, _)| f)
}

/// the songs of the shards in DIR, in the dataset's order
fn songs(dir: &Path, shards: &[u32]) -> Result<Vec<Song>, String> {
    let tsv = std::fs::read_to_string(dir.join("raw_30s_cleantags.tsv")).map_err(|e| format!("the tags: {e}"))?;
    let mut out = Vec::new();
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
        let (Some(id), Some(path)) = (cols.first(), cols.get(3)) else { continue };
        let Some((shard, file)) = path.split_once('/') else { continue };
        if !shard.parse::<u32>().is_ok_and(|s| shards.contains(&s)) {
            continue;
        }
        let low = dir.join(shard).join(file.replace(".mp3", ".low.mp3"));
        if !low.exists() {
            continue;
        }
        let genres: BTreeSet<&str> = cols.iter().filter_map(|c| c.strip_prefix("genre---")).collect();
        let sung = cols.contains(&"instrument---voice");
        out.push(Song { id: id.to_string(), path: low, family: family(&genres), sung });
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

/// the tags and the shards, downloaded into `dir` once
fn fetch(dir: &Path, shards: &[u32]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (file, url) in [
        ("raw_30s_cleantags.tsv", format!("{META}/raw_30s_cleantags.tsv")),
        ("sha256_tars.txt", format!("{META}/download/raw_30s_audio-low_sha256_tars.txt")),
    ] {
        if !dir.join(file).exists() {
            sh(Command::new("curl").args(["-sfL", "--retry", "5", "-o"]).arg(dir.join(file)).arg(url))?;
        }
    }
    let sums = std::fs::read_to_string(dir.join("sha256_tars.txt")).map_err(|e| e.to_string())?;
    for s in shards {
        let done = dir.join(format!("{s:02}.ok"));
        if done.exists() {
            continue;
        }
        let tar = format!("raw_30s_audio-low-{s:02}.tar");
        let part = dir.join(format!("{tar}.part"));
        eprintln!("downloading shard {s:02} (1.7 GB)");
        // a download stopped part way goes on where it stopped, and one already whole is checked as it is
        let got = sh(Command::new("curl")
            .args(["-fL", "--retry", "5", "-C", "-", "-o"])
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
        std::fs::write(&done, "").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// the lines of a song: as it is, then with each flaw
fn judge(song: &Song, ears: Option<&Ears>, flaws: &[Flaw]) -> Result<Vec<Value>, String> {
    let x = audio::load(&song.path).map_err(|e| format!("{}: {e:#}", song.id))?;
    let hear = |_: &claudius::model::Project, r: &claudius::render::Rendered| {
        ears.and_then(|e| e.report(&r.master, None, None, true).ok())
    };
    let total = |x: Stereo| claudius::song::judge_audio(x, None, Some(&hear)).total;
    let sung = if song.sung { "sung" } else { "not tagged sung" };
    let mut lines = Vec::new();
    let t = total(x.clone());
    lines.push(json!({
        "item": song.id,
        "slice": {"family": song.family, "voice": sung},
        "metrics": {"total": hundredths(t), "condemned": t < CONDEMNED},
    }));
    let seed = song.id.trim_start_matches("track_").parse::<u64>().unwrap_or(1);
    for f in flaws {
        let ft = total(f.put(&x, seed.wrapping_mul(31).wrapping_add(*f as u64)));
        lines.push(json!({
            "item": format!("{}/{}", song.id, f.name()),
            "slice": {"flaw": f.name(), "family": song.family, "voice": sung},
            "metrics": {"ranked": t > ft, "gap": hundredths(t - ft), "praised": ft >= PRAISED},
        }));
    }
    Ok(lines)
}

/// a score to the hundredth, as the lines keep it
fn hundredths(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// the line that heads the results: which way each metric is better, and what a change is made to improve
fn head() -> Value {
    json!({"better": {"condemned": "lower", "praised": "lower"}, "primary": ["ranked"]})
}

/// what the run found, flaw by flaw and for the songs as they are
fn summary(lines: &[Value]) {
    let mut by: BTreeMap<String, (usize, usize, f64, usize)> = BTreeMap::new();
    let (mut n, mut sum, mut condemned) = (0usize, 0.0, 0usize);
    for l in lines {
        let m = &l["metrics"];
        if let Some(t) = m["total"].as_f64() {
            n += 1;
            sum += t;
            condemned += usize::from(m["condemned"].as_bool() == Some(true));
        } else if let Some(f) = l["slice"]["flaw"].as_str() {
            let e = by.entry(f.to_string()).or_default();
            e.0 += 1;
            e.1 += usize::from(m["ranked"].as_bool() == Some(true));
            e.2 += m["gap"].as_f64().unwrap_or(0.0);
            e.3 += usize::from(m["praised"].as_bool() == Some(true));
        }
    }
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!(
        "{n} songs as they are: total {:.1} on average, {:.1}% condemned (under {CONDEMNED})",
        sum / n.max(1) as f64,
        pct(condemned, n)
    );
    println!("| flaw | songs | ranked above it | gap | flawed copy praised ({PRAISED}+) |");
    println!("|---|---|---|---|---|");
    for (f, (k, ranked, gap, praised)) in &by {
        println!(
            "| {f} | {k} | {:.1}% | {:.1} | {:.1}% |",
            pct(*ranked, *k),
            gap / *k.max(&1) as f64,
            pct(*praised, *k)
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let Some(dir) = args.first().filter(|a| !a.starts_with("--")).map(PathBuf::from) else {
        eprintln!("usage: eval_critic DIR [--shards 0..2] [--songs N] [--out FILE] [--no-ears] [--threads N]");
        std::process::exit(2);
    };
    let shards: Vec<u32> = flag("--shards")
        .and_then(|s| s.split_once("..").and_then(|(a, b)| Some(a.parse().ok()?..=b.parse().ok()?)))
        .unwrap_or(0..=2)
        .collect();
    if let Err(e) = fetch(&dir, &shards) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    let mut all = songs(&dir, &shards).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    if let Some(n) = flag("--songs").and_then(|n| n.parse().ok()) {
        all.truncate(n);
    }
    let flaws: Vec<Flaw> = Flaw::ALL.into_iter().filter(|f| cfg!(feature = "native") || !f.stretches()).collect();
    let out = flag("--out").map_or_else(|| dir.join("critic.jsonl"), PathBuf::from);
    // a run stopped part way: the songs it finished stay, the rest are judged
    let kept: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let have: BTreeSet<&str> = kept.iter().filter_map(|l| l["item"].as_str()).collect();
    let todo: Vec<&Song> = all
        .iter()
        .filter(|s| {
            !have.contains(s.id.as_str())
                || flaws.iter().any(|f| !have.contains(format!("{}/{}", s.id, f.name()).as_str()))
        })
        .collect();
    let threads = flag("--threads").and_then(|n| n.parse().ok()).unwrap_or_else(|| {
        // the threads each one's ears take, as the ears count them
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        let ears = std::env::var("EARSHOT_THREADS").ok().and_then(|v| v.parse::<usize>().ok());
        (cores / ears.unwrap_or((cores / 2).clamp(1, 4)).max(1)).max(1)
    });
    let no_ears = args.iter().any(|a| a == "--no-ears");
    eprintln!(
        "{} songs from shards {:?}, {} judged already; {} flaws; {threads} at a time{}",
        all.len(),
        shards,
        all.len() - todo.len(),
        flaws.len(),
        if no_ears { ", without the ears" } else { "" }
    );
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&out).unwrap_or_else(|e| {
        eprintln!("{}: {e}", out.display());
        std::process::exit(1)
    });
    if kept.is_empty() {
        let _ = writeln!(file, "{}", head());
    }
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<Result<Vec<Value>, String>>();
    let started = std::time::Instant::now();
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.min(todo.len()) {
            let tx = tx.clone();
            let (todo, next, flaws) = (&todo, &next, &flaws);
            scope.spawn(move || {
                let ears = if no_ears {
                    None
                } else {
                    Ears::load(false).map_err(|e| eprintln!("judging without the ears: {e:#}")).ok()
                };
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(song) = todo.get(i) else { break };
                    if tx.send(judge(song, ears.as_ref(), flaws)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let mut done = 0;
        let mut fresh = Vec::new();
        for r in rx {
            done += 1;
            match r {
                Ok(lines) => {
                    for l in &lines {
                        let _ = writeln!(file, "{l}");
                    }
                    let _ = file.flush();
                    fresh.extend(lines);
                }
                Err(e) => failed.lock().unwrap().push(e),
            }
            if done % 10 == 0 || done == todo.len() {
                let secs = started.elapsed().as_secs_f64();
                eprintln!("{done}/{} songs, {:.1} s a song", todo.len(), secs / done as f64);
            }
        }
        let mut lines = kept;
        lines.extend(fresh);
        summary(&lines);
    });
    let failed = failed.into_inner().unwrap();
    if !failed.is_empty() {
        eprintln!("{} songs could not be judged:", failed.len());
        for e in failed.iter().take(10) {
            eprintln!("  {e}");
        }
    }
}
