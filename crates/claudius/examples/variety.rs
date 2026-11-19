//! How alike a genre's songs are: for each genre, songs composed (not rendered) from seeds 1 to N, and how many
//! different forms, drum patterns, kits, parts' sounds and styles, mixes and transitions they have between them, told
//! apart as a listener hears them differ (`features`), not by every number that moves a little from seed to seed. A
//! genre that makes nearly the same song every time shows one or two of each where N songs could show N.
//!
//!   cargo run --release -p claudius --example variety -- [N] [GENRE...]
//!
//! Prints a table, a genre a line, and a last line of each column's mean over the genres.

use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};

use claudius::compose::{self, Options};
use claudius::genres;
use serde_json::Value;

/// what is told apart, a column each
const WHAT: [&str; 7] = ["forms", "drum patterns", "kits", "part sounds", "part styles", "mixes", "transitions"];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n: u64 = args.first().and_then(|a| a.parse().ok()).unwrap_or(20);
    let only: Vec<&String> = args.iter().skip(1).collect();
    println!("genre\t{}", WHAT.join("\t"));
    let mut sums = [0.0f64; WHAT.len()];
    let mut genres_seen = 0usize;
    for g in genres::all()? {
        if !only.is_empty() && !only.iter().any(|o| **o == g.name) {
            continue;
        }
        let mut sets: [BTreeSet<u64>; WHAT.len()] = Default::default();
        for seed in 1..=n {
            let bp = compose::generate(&g, seed, &Options::default(), None);
            let v = serde_json::to_value(&bp)?;
            for (set, part) in sets.iter_mut().zip(features(&v)) {
                set.insert(part);
            }
        }
        let counts: Vec<usize> = sets.iter().map(BTreeSet::len).collect();
        for (s, c) in sums.iter_mut().zip(&counts) {
            *s += *c as f64;
        }
        genres_seen += 1;
        let cells: Vec<String> = counts.iter().map(|c| c.to_string()).collect();
        println!("{}\t{}", g.name, cells.join("\t"));
    }
    let means: Vec<String> = sums.iter().map(|s| format!("{:.1}", s / genres_seen.max(1) as f64)).collect();
    println!("mean of {genres_seen} genres, {n} songs each\t{}", means.join("\t"));
    Ok(())
}

/// A song's parts as the columns tell them apart, each as a hash, as a listener would hear them differ rather than as
/// each number of them does (every seed moves a level, a length or a sound's knob a little): its form as its sections'
/// kinds in order; its drum lanes' hits on the 16th-note grid, without their velocities; its kit as each lane's voice or
/// sample, without its knobs; its parts' presets and instruments; their styles; the effects of its mix, a reverb's decay
/// to half a second and a delay's length in beats; and which transitions it has.
fn features(song: &Value) -> [u64; WHAT.len()] {
    let form: Vec<&str> = list(song, "form").iter().map(|s| s["kind"].as_str().unwrap_or("")).collect();
    let drums = list(song, "drums");
    let patterns: Vec<String> = drums
        .iter()
        .flat_map(|d| {
            let lane = d["name"].as_str().unwrap_or("").to_string();
            d["patterns"].as_object().into_iter().flatten().map(move |(variant, hits)| {
                let steps: Vec<i64> = hits
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|h| h["t"].as_f64())
                    .map(|t| (t * 4.0).round() as i64)
                    .collect();
                format!("{lane}:{variant}:{steps:?}")
            })
        })
        .collect();
    let kit: Vec<String> = drums.iter().map(|d| names(&d["sound"]).join("+")).collect();
    let parts = list(song, "parts");
    let sounds: Vec<String> = parts.iter().map(|p| names(&p["sound"]).join("+")).collect();
    let styles: Vec<&str> = parts.iter().map(|p| p["style"].as_str().unwrap_or("")).collect();
    let mut fx = Vec::new();
    effects(&song["mix"], &mut fx);
    let transitions: Vec<&String> =
        song["transitions"].as_object().into_iter().flatten().filter(|(_, v)| !v.is_null()).map(|(k, _)| k).collect();
    [hash(&form), hash(&patterns), hash(&kit), hash(&sounds), hash(&styles), hash(&fx), hash(&transitions)]
}

/// what a sound is by name: its presets, instruments, drum voices and sample files, wherever they are in it (a rack's
/// layers), its knobs left out
fn names(sound: &Value) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for key in ["preset", "voice", "instrument", "name"] {
                    if let Some(s) = o.get(key).and_then(Value::as_str) {
                        out.push(format!("{key}:{s}"));
                    }
                }
                if let Some(path) = o.get("path").and_then(Value::as_str) {
                    out.push(format!("file:{}", path.rsplit('/').next().unwrap_or(path)));
                }
                for (k, x) in o {
                    if !matches!(k.as_str(), "params" | "patch") {
                        walk(x, out);
                    }
                }
            }
            Value::Array(xs) => xs.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(sound, &mut out);
    out
}

/// the effects anywhere in a mix, by name, a reverb with its decay to half a second and a delay with its length
fn effects(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if let Some(fx) = o.get("fx").and_then(Value::as_str) {
                let detail = match fx {
                    "reverb" => {
                        o.get("decay").and_then(Value::as_f64).map(|d| format!(" {:.1}s", (d * 2.0).round() / 2.0))
                    }
                    "delay" => o.get("beats").and_then(Value::as_f64).map(|b| format!(" {b} beats")),
                    _ => None,
                };
                out.push(format!("{fx}{}", detail.unwrap_or_default()));
            }
            o.values().for_each(|x| effects(x, out));
        }
        Value::Array(xs) => xs.iter().for_each(|x| effects(x, out)),
        _ => {}
    }
}

fn list<'a>(song: &'a Value, key: &str) -> &'a [Value] {
    song[key].as_array().map_or(&[], Vec::as_slice)
}

fn hash(x: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    x.hash(&mut h);
    h.finish()
}
