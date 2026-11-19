//! Cuts a site demo from a claudius song folder, with claudius's own decoder and encoder (it was a Python script that
//! needed ffmpeg and numpy).
//!
//!   cargo run --release -p claudius --example excerpt -- analyze SONG_DIR
//!   cargo run --release -p claudius --example excerpt -- cut SONG_DIR SLUG START_BAR BARS OUT_DIR [--title T] [--key K]
//!
//! `analyze` prints the loudness of every bar with the section it belongs to, to find the drop or the groove. `cut`
//! writes OUT_DIR/SLUG.mp3 (160 kbps at 44.1 kHz, a short fade in and a 3 s fade out) and OUT_DIR/SLUG.json: 600 peak
//! and RMS values (0 to 255) read back from the MP3, the sections inside the excerpt in seconds, and where the excerpt
//! sits in the song (START_BAR counts from 0), which the site's players use for their bar readout.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use claudius::audio::{self, Stereo};
use claudius::config::SRF;
use claudius::export;
use serde_json::{Value, json};

/// the peak and RMS values the players draw
const POINTS: usize = 600;
const FADE_IN: f64 = 0.4;
const FADE_OUT: f64 = 3.0;

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        let v = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        v
    };
    let (title, key) = (flag("--title"), flag("--key"));
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["analyze", dir] => analyze(Path::new(dir)),
        ["cut", dir, slug, start, bars, out] => {
            cut(Path::new(dir), slug, start.parse()?, bars.parse()?, Path::new(out), title, key)
        }
        _ => bail!("excerpt analyze SONG_DIR | excerpt cut SONG_DIR SLUG START_BAR BARS OUT_DIR [--title T] [--key K]"),
    }
}

/// the song folder's `project.json`
fn project(dir: &Path) -> Result<Value> {
    let path = dir.join("project.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

fn sections(p: &Value) -> &[Value] {
    p["sections"].as_array().map_or(&[], Vec::as_slice)
}

/// the section bar `bar` (from 0) is in
fn section_at(p: &Value, bar: f64) -> Option<&Value> {
    sections(p).iter().find(|s| {
        let (a, n) = (s["start_bar"].as_f64().unwrap_or(0.0), s["bars"].as_f64().unwrap_or(0.0));
        a <= bar && bar < a + n
    })
}

/// a bar of the song, in seconds
fn bar_secs(p: &Value) -> f64 {
    240.0 / p["bpm"].as_f64().unwrap_or(120.0)
}

fn analyze(dir: &Path) -> Result<()> {
    let p = project(dir)?;
    let bar = bar_secs(&p);
    let x = audio::load_once(&dir.join("song.wav"))?;
    let per = (bar * SRF) as usize;
    for b in 0..x.len() / per.max(1) {
        let (a, e) = (b * per, (b + 1) * per);
        let sq: f64 = (a..e).map(|i| (x.l[i] as f64).powi(2) + (x.r[i] as f64).powi(2)).sum::<f64>() / (2 * per) as f64;
        let db = 20.0 * (sq.sqrt() + 1e-9).log10();
        let name = section_at(&p, b as f64).and_then(|s| s["name"].as_str()).unwrap_or("-");
        let meter = "#".repeat(((db + 40.0) * 1.5).max(0.0) as usize);
        println!("bar {:3} {:7.1}s {name:8} {db:6.1} dB {meter}", b + 1, b as f64 * bar);
    }
    Ok(())
}

fn cut(
    dir: &Path,
    slug: &str,
    start_bar: f64,
    bars: f64,
    out: &Path,
    title: Option<String>,
    key: Option<String>,
) -> Result<()> {
    let p = project(dir)?;
    let bar = bar_secs(&p);
    let (start, dur) = (start_bar * bar, bars * bar);
    let song = audio::load_once(&dir.join("song.wav"))?;
    let (a, n) = ((start * SRF) as usize, (dur * SRF) as usize);
    let mut x = song.slice(a.min(song.len()), (a + n).min(song.len()));
    fades(&mut x);
    let title = title.unwrap_or_else(|| p["title"].as_str().unwrap_or("claudius").to_string());
    let tags = export::Tags { artist: "claudius".into(), comment: "Made with claudius".into(), ..export::tags(&title) };
    let spec: export::Spec = "mp3:160@44100".parse().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mp3: PathBuf = out.join(format!("{slug}.mp3"));
    std::fs::write(&mp3, export::encode(&x, &spec, &tags)?)
        .with_context(|| format!("cannot write {}", mp3.display()))?;
    // the players draw what the MP3 holds, read back
    let heard = audio::load_once(&mp3)?;
    let duration = heard.secs();
    let (peaks, rms) = points(&heard);
    let end_bar = start_bar + bars;
    let inside: Vec<Value> = sections(&p)
        .iter()
        .filter_map(|s| {
            let (sa, sn) = (s["start_bar"].as_f64()?, s["bars"].as_f64()?);
            let (lo, hi) = (sa.max(start_bar), (sa + sn).min(end_bar));
            (hi > lo).then(|| {
                json!({
                    "name": s["name"],
                    "kind": s["kind"],
                    "start": round3((lo - start_bar) * bar),
                    "end": round3(duration.min((hi - start_bar) * bar)),
                })
            })
        })
        .collect();
    let names: Vec<&str> = inside.iter().filter_map(|s| s["name"].as_str()).collect();
    let meta = json!({
        "title": title,
        "genre": p["genre"],
        "bpm": p["bpm"],
        "key": key,
        "songSeconds": (p["bars"].as_f64().unwrap_or(0.0) * bar * 100.0).round() / 100.0,
        "songBars": p["bars"],
        "startBar": start_bar,
        "startSeconds": round3(start),
        "duration": round3(duration),
        "points": POINTS,
        "scale": 255,
        "peaks": peaks,
        "rms": rms,
        "sections": inside,
    });
    let json_path = out.join(format!("{slug}.json"));
    std::fs::write(&json_path, serde_json::to_string(&meta)?)?;
    println!("{slug}: {start:.1}s + {dur:.1}s -> {duration:.2}s, sections {names:?}");
    Ok(())
}

/// a fade in over `FADE_IN` and out over the last `FADE_OUT` seconds, straight lines as ffmpeg's afade draws them
fn fades(x: &mut Stereo) {
    let n = x.len();
    let (fi, fo) = ((FADE_IN * SRF) as usize, (FADE_OUT * SRF) as usize);
    for i in 0..n {
        let up = if i < fi { i as f32 / fi as f32 } else { 1.0 };
        let left = n - i;
        let down = if left < fo { left as f32 / fo as f32 } else { 1.0 };
        x.l[i] *= up * down;
        x.r[i] *= up * down;
    }
}

/// `POINTS` peak and RMS values of `x`, each over an equal stretch of it, from 0 to 255
fn points(x: &Stereo) -> (Vec<u8>, Vec<u8>) {
    let q = |v: f64| (v.min(1.0) * 255.0).round().min(255.0) as u8;
    let n = x.len();
    let (mut peaks, mut rms) = (Vec::with_capacity(POINTS), Vec::with_capacity(POINTS));
    for k in 0..POINTS {
        let (a, b) = (k * n / POINTS, (k + 1) * n / POINTS);
        if b <= a {
            peaks.push(0);
            rms.push(0);
            continue;
        }
        let peak = (a..b).map(|i| x.l[i].abs().max(x.r[i].abs())).fold(0.0f32, f32::max);
        let sq = (a..b).map(|i| (x.l[i] as f64).powi(2) + (x.r[i] as f64).powi(2)).sum::<f64>() / (2 * (b - a)) as f64;
        peaks.push(q(peak as f64));
        rms.push(q(sq.sqrt()));
    }
    (peaks, rms)
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
