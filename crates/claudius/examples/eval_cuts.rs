//! How often the auto-sampler's pieces cut a sung word, on JamendoLyrics: songs from Jamendo, each word's start and end
//! annotated by hand (<https://huggingface.co/datasets/jamendolyrics/jamendolyrics>). It counts the cuts of the pieces as
//! the bars cut them (`sampler::find`, as New song's sampler still does) and as phrases sung whole (`sampler::pieces`,
//! the Add a recording window's), and how many fall inside a word, and inside a line. The songs are downloaded into DIR
//! (with curl) the first time and kept there; none of them goes into the repository.
//!
//!   cargo run --release -p claudius --example eval_cuts -- DIR [--lang en|de|es|fr] [--songs N] [--why]
//!
//! `--why` says, for each cut inside a word, where in the word it falls and the words heard round it.

use std::path::{Path, PathBuf};

use claudius::analysis::song as analysis;
use claudius::sampler::{self, Options};

const DATASET: &str = "https://huggingface.co/datasets/jamendolyrics/jamendolyrics/resolve/main";
/// a cut this near a word's start or end is between words: the annotations are no closer than that
const SLACK: f64 = 0.02;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().ok_or("usage: eval_cuts DIR [--lang en] [--songs N]")?);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let lang = flag("--lang").unwrap_or_else(|| "en".into());
    let most: usize = flag("--songs").map_or(Ok(usize::MAX), |n| n.parse().map_err(|_| "--songs takes a number"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let index = fetched(&dir, "JamendoLyrics.csv")?;
    let language = match lang.as_str() {
        "en" => "English",
        "de" => "German",
        "es" => "Spanish",
        "fr" => "French",
        other => return Err(format!("no language {other}: en, de, es or fr")),
    };
    let songs: Vec<String> = std::fs::read_to_string(&index)
        .map_err(|e| e.to_string())?
        .lines()
        .skip(1)
        .filter_map(|l| {
            let cols: Vec<&str> = l.split(',').collect();
            (cols.get(6) == Some(&language)).then(|| cols[1].trim_end_matches(".mp3").to_string())
        })
        .take(most)
        .collect();
    let ears = claudius::ears::Ears::load(false).ok();
    eprintln!("{} songs in {language}; ears {}", songs.len(), if ears.is_some() { "on" } else { "off" });
    let (mut bars, mut whole) = (Count::default(), Count::default());
    println!("{:<40} {:>14} {:>14} {:>14} {:>14}", "song", "bars: words", "lines", "whole: words", "lines");
    for name in &songs {
        let (words, lines) = sung(&fetched(&dir, &format!("annotations/words/{name}.csv"))?)?;
        let x = claudius::audio::load(&fetched(&dir, &format!("subsets/{lang}/mp3/{name}.mp3"))?)
            .map_err(|e| e.to_string())?;
        let stems = claudius::stems::of(&x, &mut |_| {});
        let a = analysis::analyze_with(&x, stems.as_deref(), None);
        let opt = Options { count: 4, ..Options::default() };
        let mut b = Count::default();
        for p in sampler::find(&x, stems.as_deref(), &a, ears.as_ref(), &opt, opt.count) {
            let first = usize::try_from(p.first).unwrap_or(0);
            b.add(&words, &lines, &[a.bar_start(first), a.bar_start(first + p.bars as usize)]);
        }
        let why = args.iter().any(|a| a == "--why");
        let heard = why.then(|| {
            let sings = stems.as_deref().map_or(&x, |s| &s.vocals);
            claudius::cuts::sung(sings, &claudius::analysis::melody::pitches(sings))
        });
        let mut w = Count::default();
        if let Ok(pieces) = sampler::pieces(&x, ears.as_ref(), &opt, 0) {
            for p in pieces {
                let (from, to) = p.phrase.within.unwrap_or((p.phrase.start, p.phrase.start));
                w.add(&words, &lines, &[from, to]);
                let Some(heard) = &heard else { continue };
                for (end, t) in [("start", from), ("end", to)] {
                    if let Some((s, e)) = words.iter().find(|(s, e)| t > s + SLACK && t < e - SLACK) {
                        let near: Vec<String> = heard
                            .words
                            .iter()
                            .filter(|h| h.end > s - 0.5 && h.start < e + 0.5)
                            .map(|h| format!("{:.2}..{:.2}", h.start, h.end))
                            .collect();
                        eprintln!(
                            "  {name}: the {end} {t:.2} cuts the word {s:.2}..{e:.2} at {:.0}% of it; heard {}",
                            100.0 * (t - s) / (e - s),
                            near.join(" ")
                        );
                    }
                }
            }
        }
        let row = [b.said(b.words), b.said(b.lines), w.said(w.words), w.said(w.lines)];
        println!("{:<40} {:>14} {:>14} {:>14} {:>14}", short(name), row[0], row[1], row[2], row[3]);
        bars.join(&b);
        whole.join(&w);
    }
    let all = [bars.said(bars.words), bars.said(bars.lines), whole.said(whole.words), whole.said(whole.lines)];
    println!("{:<40} {:>14} {:>14} {:>14} {:>14}", "all", all[0], all[1], all[2], all[3]);
    Ok(())
}

/// the cuts counted, those inside a word, and those inside a line
#[derive(Default)]
struct Count {
    cuts: usize,
    words: usize,
    lines: usize,
}

impl Count {
    fn add(&mut self, words: &[(f64, f64)], lines: &[(f64, f64)], cuts: &[f64]) {
        let inside = |spans: &[(f64, f64)], t: f64| spans.iter().any(|(s, e)| t > s + SLACK && t < e - SLACK);
        for &t in cuts {
            self.cuts += 1;
            self.words += inside(words, t) as usize;
            self.lines += inside(lines, t) as usize;
        }
    }

    fn join(&mut self, other: &Count) {
        self.cuts += other.cuts;
        self.words += other.words;
        self.lines += other.lines;
    }

    fn said(&self, n: usize) -> String {
        let share = if self.cuts == 0 { 0.0 } else { 100.0 * n as f64 / self.cuts as f64 };
        format!("{n} of {} ({share:.0}%)", self.cuts)
    }
}

/// where something is sung: its start and end, in seconds
type Spans = Vec<(f64, f64)>;

/// each word's start and end, and each line's, as `annotations/words/<song>.csv` has them: a line ends where its last
/// word says
fn sung(path: &Path) -> Result<(Spans, Spans), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (mut words, mut lines, mut line) = (Vec::new(), Vec::new(), None);
    for l in text.lines().skip(1) {
        let cols: Vec<f64> = l.split(',').map(|c| c.parse().unwrap_or(f64::NAN)).collect();
        let (Some(&s), Some(&e)) = (cols.first(), cols.get(1)) else { continue };
        if !(s.is_finite() && e.is_finite()) {
            continue;
        }
        words.push((s, e));
        let start = *line.get_or_insert(s);
        if let Some(&end) = cols.get(2).filter(|end| end.is_finite()) {
            lines.push((start, end));
            line = None;
        }
    }
    Ok((words, lines))
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
    let url = format!("{DATASET}/{file}");
    let ok = std::process::Command::new("curl")
        .args(["-sSfL", "--retry", "2", "-o"])
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
    name.chars().take(46).collect()
}
