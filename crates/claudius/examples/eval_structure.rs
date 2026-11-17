//! How well the analysis hears a song's sections, on two sets of songs with what people heard in them.
//!
//! **JamendoLyrics** (the default): songs from Jamendo whose lyrics are written in stanzas, each line's start and end
//! annotated by hand (<https://huggingface.co/datasets/jamendolyrics/jamendolyrics>). A stanza's first line is where a
//! sung section starts, and the stanza sung again most often, word for word or nearly, is the chorus. It counts:
//!
//! - **starts**: the stanzas that start other music than the one before (or after a rest of two bars) heard within a
//!   bar of a section's start;
//! - **on**: the sections that start within a bar of a stanza's start, or of where the singing stops for two bars or
//!   more: a lower bound, as a section may rightly start where the lyrics do not say (a texture changing mid-verse);
//! - **chorus**: the choruses heard as choruses, and **false**, the other stanzas heard as choruses;
//! - **same**: the pairs of stanzas sung alike heard as the same music (sections with the same letter).
//!
//! **SALAMI** (`--salami DIR`): recordings from the Internet Archive with each section's start and function annotated
//! by one or two people (<https://github.com/DDMAL/salami-data-public>, `DIR/annotations/<id>/parsed`), the audio in
//! `DIR/audio/<id>.mp3`. It gives the measures papers give, against each annotator and averaged: **hr.5** and **hr3**,
//! the F-measure of the boundaries heard within 0.5 s and 3 s of the annotated ones (untrimmed); **label**, the share
//! of the time whose function is heard as annotated (verse, pre-chorus, chorus, post-chorus, bridge, intro, outro, the
//! rest instrumental); and **chorus**, the F-measure of the time heard as a chorus.
//!
//! The songs are downloaded into DIR the first time (JamendoLyrics, with curl) and kept there; none of them goes into
//! the repository. With the ears installed the analysis hears each song's stems apart, as the studio does; `--no-stems`
//! analyses the mix alone. `--heard JSONDIR` reads each song's analysis from `JSONDIR/<song>.json` instead (as `claudius
//! analyze --json` writes it, of any version), and `--write JSONDIR` keeps those it makes. A section starts where its
//! bars do (`bar_starts`, else on the grid of `bar_secs` from `first_downbeat`); `--grid` takes the grid always, as an
//! analysis written without its bars has to, so that two versions are measured alike.
//!
//!   cargo run --release -p claudius --example eval_structure -- DIR [--lang en|de|es|fr|all] [--songs N]
//!   cargo run --release -p claudius --example eval_structure -- --salami DIR [--songs N]
//!       [--no-stems] [--heard JSONDIR] [--write JSONDIR] [--grid]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

const DATASET: &str = "https://huggingface.co/datasets/jamendolyrics/jamendolyrics/resolve/main";

/// the options every song is measured with
struct Run {
    heard: Option<PathBuf>,
    write: Option<PathBuf>,
    stems: bool,
    grid: bool,
}

impl Run {
    /// the analysis of the song `name` in `audio`: read from `--heard`, or made (and kept with `--write`)
    fn analysis(&self, name: &str, audio: &dyn Fn() -> Result<PathBuf, String>) -> Result<Value, String> {
        let a: Value = match &self.heard {
            Some(d) => {
                let text =
                    std::fs::read_to_string(d.join(format!("{name}.json"))).map_err(|e| format!("{name}: {e}"))?;
                serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))?
            }
            None => {
                let x = claudius::audio::load(&audio()?).map_err(|e| format!("{name}: {e:#}"))?;
                let stems = if self.stems { claudius::stems::of(&x, &mut |_| {}) } else { None };
                let a = claudius::analysis::song::analyze_with(&x, stems.as_deref(), None);
                serde_json::to_value(&a).map_err(|e| e.to_string())?
            }
        };
        if let Some(w) = &self.write {
            let text = serde_json::to_string(&a).map_err(|e| e.to_string())?;
            std::fs::write(w.join(format!("{name}.json")), text).map_err(|e| e.to_string())?;
        }
        Ok(a)
    }
}

/// a section as an analysis heard it: where it starts and ends in seconds, what it is, and the letter of its music
struct Heard {
    start: f64,
    end: f64,
    label: String,
    group: Option<String>,
}

/// The sections of analysis `a`: each from where its bars start (or the grid, with `grid`), named by its label, or, in
/// an analysis that names none, by its kind as the function it stands for (its drops and hooks for choruses, its builds
/// for pre-choruses, its grooves and breaks for instrumentals); and how long a bar is.
fn sections(a: &Value, grid: bool) -> (Vec<Heard>, f64) {
    let bar = a["bar_secs"].as_f64().unwrap_or(2.0);
    let first = a["first_downbeat"].as_f64().unwrap_or(0.0);
    let starts: Vec<f64> =
        a["bar_starts"].as_array().map(|v| v.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    let at = |b: f64| -> f64 {
        let i = b as usize;
        if !grid && i < starts.len() { starts[i] } else { first + b * bar }
    };
    let out = a["sections"]
        .as_array()
        .map(|s| {
            s.iter()
                .map(|s| {
                    let (start_bar, bars) = (s["start_bar"].as_f64().unwrap_or(0.0), s["bars"].as_f64().unwrap_or(0.0));
                    let label = match s["label"].as_str() {
                        Some(l) => l.to_string(),
                        None => match s["kind"].as_str().unwrap_or("") {
                            "hook" | "drop" => "chorus",
                            "build" => "pre-chorus",
                            "verse" => "verse",
                            "intro" => "intro",
                            "outro" => "outro",
                            _ => "instrumental",
                        }
                        .to_string(),
                    };
                    Heard {
                        start: at(start_bar),
                        end: at(start_bar + bars),
                        label,
                        group: s["group"].as_str().map(str::to_string),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    (out, bar)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let most: usize = flag("--songs").map_or(Ok(usize::MAX), |n| n.parse().map_err(|_| "--songs takes a number"))?;
    let run = Run {
        heard: flag("--heard").map(PathBuf::from),
        write: flag("--write").map(PathBuf::from),
        stems: !args.iter().any(|a| a == "--no-stems"),
        grid: args.iter().any(|a| a == "--grid"),
    };
    if let Some(w) = &run.write {
        std::fs::create_dir_all(w).map_err(|e| e.to_string())?;
    }
    eprintln!(
        "{}",
        match &run.heard {
            Some(d) => format!("analyses read from {}", d.display()),
            None if run.stems => "stems apart where the ears are installed".into(),
            None => "the mix alone".into(),
        }
    );
    match flag("--salami") {
        Some(dir) => salami(Path::new(&dir), most, &run),
        None => {
            let dir = PathBuf::from(args.first().ok_or("usage: eval_structure DIR | --salami DIR")?);
            jamendo(&dir, &flag("--lang").unwrap_or_else(|| "en".into()), most, &run)
        }
    }
}

/// hits of how many, for each JamendoLyrics measure
#[derive(Default)]
struct Count {
    starts: (usize, usize),
    on: (usize, usize),
    chorus: (usize, usize),
    false_chorus: (usize, usize),
    same: (usize, usize),
}

impl Count {
    fn said(&self, (hit, of): (usize, usize)) -> String {
        if of == 0 { "-".into() } else { format!("{:.0}% /{of}", 100.0 * hit as f64 / of as f64) }
    }

    fn add(&mut self, o: &Count) {
        for (a, b) in [
            (&mut self.starts, o.starts),
            (&mut self.on, o.on),
            (&mut self.chorus, o.chorus),
            (&mut self.false_chorus, o.false_chorus),
            (&mut self.same, o.same),
        ] {
            a.0 += b.0;
            a.1 += b.1;
        }
    }

    fn row(&self, name: &str) -> String {
        format!(
            "{:<40} {:>9} {:>9} {:>9} {:>9} {:>9}",
            short(name),
            self.said(self.starts),
            self.said(self.on),
            self.said(self.chorus),
            self.said(self.false_chorus),
            self.said(self.same)
        )
    }
}

fn jamendo(dir: &Path, lang: &str, most: usize, run: &Run) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let index = fetched(dir, "JamendoLyrics.csv")?;
    let languages: &[&str] = match lang {
        "en" => &["English"],
        "de" => &["German"],
        "es" => &["Spanish"],
        "fr" => &["French"],
        "all" => &["English", "German", "Spanish", "French"],
        other => return Err(format!("no language {other}: en, de, es, fr or all")),
    };
    let songs: Vec<(String, &str)> = std::fs::read_to_string(&index)
        .map_err(|e| e.to_string())?
        .lines()
        .skip(1)
        .filter_map(|l| {
            let cols: Vec<&str> = l.split(',').collect();
            let language = *cols.get(6)?;
            let code = match language {
                "English" => "en",
                "German" => "de",
                "Spanish" => "es",
                "French" => "fr",
                _ => return None,
            };
            languages.contains(&language).then(|| (cols[1].trim_end_matches(".mp3").to_string(), code))
        })
        .take(most)
        .collect();
    let mut total = Count::default();
    println!("{:<40} {:>9} {:>9} {:>9} {:>9} {:>9}", "song", "starts", "on", "chorus", "false", "same");
    for (name, code) in &songs {
        let stanzas = match stanzas(dir, name) {
            Ok(s) => s,
            Err(e) => {
                println!("{:<40} skipped: {e}", short(name));
                continue;
            }
        };
        let a = run.analysis(name, &|| fetched(dir, &format!("subsets/{code}/mp3/{name}.mp3")))?;
        let c = measure_stanzas(&stanzas, &a, run.grid);
        println!("{}", c.row(name));
        total.add(&c);
    }
    println!("{}", total.row("all"));
    Ok(())
}

/// a stanza as JamendoLyrics has it: when its first line starts and its last ends, and its words
struct Stanza {
    start: f64,
    end: f64,
    words: Vec<String>,
}

/// whether two stanzas are sung alike: most of their words the same (Jaccard 0.75 or more)
fn alike(a: &Stanza, b: &Stanza) -> bool {
    let (x, y): (BTreeSet<&String>, BTreeSet<&String>) = (a.words.iter().collect(), b.words.iter().collect());
    let both = x.intersection(&y).count() as f64;
    let either = x.union(&y).count().max(1) as f64;
    both / either >= 0.75
}

fn measure_stanzas(stanzas: &[Stanza], a: &Value, grid: bool) -> Count {
    let (sections, bar) = sections(a, grid);
    let first = a["first_downbeat"].as_f64().unwrap_or(0.0);
    let mut c = Count::default();
    if sections.is_empty() || stanzas.is_empty() {
        return c;
    }
    // the stanzas alike, and the chorus: the one sung most often, twice at least, the longer if as often
    let n = stanzas.len();
    let kin: Vec<Vec<usize>> = (0..n).map(|i| (0..n).filter(|&j| alike(&stanzas[i], &stanzas[j])).collect()).collect();
    let chorus_of = (0..n)
        .filter(|&i| kin[i].len() >= 2)
        .max_by(|&i, &j| (kin[i].len(), stanzas[i].words.len()).cmp(&(kin[j].len(), stanzas[j].words.len())));
    let is_chorus = |i: usize| chorus_of.is_some_and(|k| kin[k].contains(&i));
    let near = |t: f64| sections.iter().any(|s| (s.start - t).abs() <= bar);
    let holding = |t: f64| sections.iter().find(|s| t >= s.start && t < s.end).or_else(|| sections.last());
    // where sung sections start: a stanza unlike the one before it, or after a rest of two bars
    for i in 0..n {
        let new_music =
            i == 0 || !alike(&stanzas[i], &stanzas[i - 1]) || stanzas[i].start - stanzas[i - 1].end >= 2.0 * bar;
        if new_music {
            c.starts.1 += 1;
            c.starts.0 += near(stanzas[i].start) as usize;
        }
    }
    // where a section may start: a stanza's start, where the singing stops for two bars, the song's first downbeat
    let mut marks: Vec<f64> = vec![first];
    for i in 0..n {
        marks.push(stanzas[i].start);
        let rest = stanzas.get(i + 1).map_or(f64::MAX, |next| next.start - stanzas[i].end);
        if rest >= 2.0 * bar {
            marks.push(stanzas[i].end);
        }
    }
    let last_sung = stanzas[n - 1].end;
    for s in sections.iter().skip(1) {
        // sections after the singing ends (an outro and its parts) are not counted: the lyrics say nothing of them
        if s.start > last_sung + bar {
            continue;
        }
        c.on.1 += 1;
        c.on.0 += marks.iter().any(|m| (s.start - m).abs() <= bar) as usize;
    }
    for i in 0..n {
        let mid = 0.5 * (stanzas[i].start + stanzas[i].end);
        let heard_chorus = holding(mid).is_some_and(|s| s.label == "chorus");
        if is_chorus(i) {
            c.chorus.1 += 1;
            c.chorus.0 += heard_chorus as usize;
        } else {
            c.false_chorus.1 += 1;
            c.false_chorus.0 += heard_chorus as usize;
        }
    }
    if sections.iter().any(|s| s.group.is_some()) {
        let g = |k: usize| holding(0.5 * (stanzas[k].start + stanzas[k].end)).and_then(|s| s.group.clone());
        for i in 0..n {
            for &j in kin[i].iter().filter(|&&j| j > i) {
                c.same.1 += 1;
                c.same.0 += (g(i).is_some() && g(i) == g(j)) as usize;
            }
        }
    }
    c
}

/// a song's stanzas: its lyrics' lines in stanzas (`lyrics/<song>.txt`, a blank line between), each line's start and
/// end from `annotations/lines/<song>.csv`, in the same order
fn stanzas(dir: &Path, name: &str) -> Result<Vec<Stanza>, String> {
    let lyrics = std::fs::read_to_string(fetched(dir, &format!("lyrics/{name}.txt"))?).map_err(|e| e.to_string())?;
    let lines =
        std::fs::read_to_string(fetched(dir, &format!("annotations/lines/{name}.csv"))?).map_err(|e| e.to_string())?;
    let times: Vec<(f64, f64)> = lines
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut cols = l.splitn(3, ',');
            let s: f64 = cols.next()?.trim().parse().ok()?;
            let e: f64 = cols.next()?.trim().parse().ok()?;
            Some((s, e))
        })
        .collect();
    let text = lyrics.replace('\r', "");
    let blocks: Vec<Vec<&str>> = text
        .split("\n\n")
        .map(|b| b.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>())
        .filter(|b| !b.is_empty())
        .collect();
    let count: usize = blocks.iter().map(Vec::len).sum();
    if count != times.len() {
        return Err(format!("{count} lines of lyrics, {} timed", times.len()));
    }
    let mut at = 0;
    let mut out = Vec::new();
    for b in blocks {
        let (start, end) = (times[at].0, times[at + b.len() - 1].1);
        let words = b
            .iter()
            .flat_map(|l| l.split_whitespace())
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        out.push(Stanza { start, end, words });
        at += b.len();
    }
    Ok(out)
}

/// the function a SALAMI label stands for, as the analysis names them; none for what is no part of the music (silence,
/// applause, the end) or has no function
fn function(label: &str) -> Option<&'static str> {
    Some(match label.trim().to_lowercase().as_str() {
        "verse" | "pre-verse" => "verse",
        "pre-chorus" => "pre-chorus",
        "chorus" => "chorus",
        "post-chorus" => "post-chorus",
        "bridge" => "bridge",
        "intro" => "intro",
        "outro" | "coda" | "fade-out" => "outro",
        "solo" | "interlude" | "instrumental" | "theme" | "main_theme" | "secondary_theme" | "head" | "break"
        | "transition" | "development" | "variation" => "instrumental",
        _ => return None,
    })
}

/// the function a section the analysis heard is compared as: its drops as choruses, its builds as pre-choruses, its
/// breaks and grooves as instrumentals
fn heard_function(label: &str) -> &'static str {
    match label {
        "verse" => "verse",
        "pre-chorus" | "build" => "pre-chorus",
        "chorus" | "drop" => "chorus",
        "post-chorus" => "post-chorus",
        "bridge" => "bridge",
        "intro" => "intro",
        "outro" => "outro",
        _ => "instrumental",
    }
}

/// what one annotator heard: each section's start and function (none for silence and the like), and where it ends
type Annotated = (Vec<(f64, Option<&'static str>)>, f64);

fn annotated(path: &Path) -> Option<Annotated> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut rows: Vec<(f64, String)> = text
        .lines()
        .filter_map(|l| {
            let mut cols = l.splitn(2, '\t');
            let t: f64 = cols.next()?.trim().parse().ok()?;
            Some((t, cols.next().unwrap_or("").trim().to_string()))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let end = rows.iter().rev().find(|r| r.1.eq_ignore_ascii_case("end")).or(rows.last()).map(|r| r.0)?;
    let parts = rows
        .into_iter()
        .filter(|r| r.0 < end && !r.1.eq_ignore_ascii_case("end"))
        .map(|(t, l)| (t, function(&l)))
        .collect();
    Some((parts, end))
}

/// the F-measure of boundaries `heard` against `truth` within `window` seconds, each matched once, nearest first
fn hit_rate(heard: &[f64], truth: &[f64], window: f64) -> f64 {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (i, h) in heard.iter().enumerate() {
        for (j, t) in truth.iter().enumerate() {
            let d = (h - t).abs();
            if d <= window {
                pairs.push((d, i, j));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut used_h, mut used_t) = (vec![false; heard.len()], vec![false; truth.len()]);
    let mut hits = 0.0;
    for (_, i, j) in pairs {
        if !used_h[i] && !used_t[j] {
            (used_h[i], used_t[j]) = (true, true);
            hits += 1.0;
        }
    }
    let (p, r) = (hits / heard.len().max(1) as f64, hits / truth.len().max(1) as f64);
    if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 }
}

/// a song's SALAMI measures against one annotator: the boundary F-measures at 0.5 s and 3 s, the share of the
/// annotated time heard as its function, and the chorus's F-measure over time
fn measure_salami(sections: &[Heard], truth: &[(f64, Option<&'static str>)], end: f64) -> [f64; 4] {
    let mut heard_b: Vec<f64> = sections.iter().map(|s| s.start).collect();
    heard_b.push(sections.last().map_or(end, |s| s.end));
    let mut truth_b: Vec<f64> = truth.iter().map(|t| t.0).collect();
    truth_b.push(end);
    let (hr5, hr3) = (hit_rate(&heard_b, &truth_b, 0.5), hit_rate(&heard_b, &truth_b, 3.0));
    // every 0.1 s of the annotated music, what it is and what it is heard as
    let (mut known, mut right, mut both, mut heard_c, mut true_c) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut t = 0.05;
    while t < end {
        let truth_f = truth.iter().rev().find(|p| p.0 <= t).and_then(|p| p.1);
        let heard_f = sections.iter().find(|s| t >= s.start && t < s.end).map(|s| heard_function(&s.label));
        if let Some(f) = truth_f {
            known += 1;
            right += (heard_f == Some(f)) as usize;
            true_c += (f == "chorus") as usize;
            both += (f == "chorus" && heard_f == Some("chorus")) as usize;
            heard_c += (heard_f == Some("chorus")) as usize;
        }
        t += 0.1;
    }
    let label = right as f64 / known.max(1) as f64;
    let (p, r) = (both as f64 / heard_c.max(1) as f64, both as f64 / true_c.max(1) as f64);
    let chorus = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
    [hr5, hr3, label, chorus]
}

fn salami(dir: &Path, most: usize, run: &Run) -> Result<(), String> {
    let mut ids: Vec<u32> = std::fs::read_dir(dir.join("audio"))
        .map_err(|e| format!("{}: {e}", dir.join("audio").display()))?
        .filter_map(|e| e.ok()?.path().file_stem()?.to_str()?.parse().ok())
        .collect();
    ids.sort_unstable();
    ids.truncate(most);
    println!("{:<8} {:>7} {:>7} {:>7} {:>7}", "song", "hr.5", "hr3", "label", "chorus");
    let (mut sum, mut songs) = ([0.0; 4], 0usize);
    for id in ids {
        let name = id.to_string();
        let parsed = dir.join("annotations").join(&name).join("parsed");
        let truths: Vec<Annotated> = ["textfile1_functions.txt", "textfile2_functions.txt"]
            .iter()
            .filter_map(|f| annotated(&parsed.join(f)))
            .collect();
        if truths.is_empty() {
            continue;
        }
        let a = run.analysis(&name, &|| Ok(dir.join("audio").join(format!("{name}.mp3"))))?;
        let (heard, _) = sections(&a, run.grid);
        if heard.is_empty() {
            continue;
        }
        let mut m = [0.0; 4];
        for (truth, end) in &truths {
            let one = measure_salami(&heard, truth, *end);
            for (k, v) in one.iter().enumerate() {
                m[k] += v / truths.len() as f64;
            }
        }
        println!("{name:<8} {:>7.3} {:>7.3} {:>7.3} {:>7.3}", m[0], m[1], m[2], m[3]);
        for (s, v) in sum.iter_mut().zip(m) {
            *s += v;
        }
        songs += 1;
    }
    let n = songs.max(1) as f64;
    println!(
        "{:<8} {:>7.3} {:>7.3} {:>7.3} {:>7.3}  ({songs} songs, the mean of each song's)",
        "all",
        sum[0] / n,
        sum[1] / n,
        sum[2] / n,
        sum[3] / n
    );
    Ok(())
}

/// `file` of the dataset, downloaded into `dir` the first time
fn fetched(dir: &Path, file: &str) -> Result<PathBuf, String> {
    let path = dir.join(file);
    if path.exists() {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // the path as a URL: a name with brackets, parentheses or spaces encoded, and curl told not to read them as globs
    let encoded: String = file
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    let url = format!("{DATASET}/{encoded}");
    let ok = std::process::Command::new("curl")
        .args(["-gsSfL", "--retry", "2", "-o"])
        .arg(&path)
        .arg(&url)
        .status()
        .map_err(|e| format!("curl: {e}"))?
        .success();
    if !ok {
        let _ = std::fs::remove_file(&path);
        return Err(format!("could not download {url}"));
    }
    Ok(path)
}

fn short(name: &str) -> String {
    name.chars().take(40).collect()
}
