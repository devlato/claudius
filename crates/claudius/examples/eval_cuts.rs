//! How often the auto-sampler's pieces cut a sung word, on JamendoLyrics: songs from Jamendo, each word's start and end
//! annotated by hand (<https://huggingface.co/datasets/jamendolyrics/jamendolyrics>). It counts the cuts of the pieces as
//! the bars cut them (`sampler::find`, as the auto-sampler's chops and stabs still find their phrases) and as phrases
//! sung whole (`sampler::pieces`, the Add a recording window's, which `sample_song` samples too), and how many fall
//! inside a word, and inside a line; and the phrases the Add a recording window shows (its boxes, which a click
//! plays: `cuts::sung` of the voice, or of the whole mix without the ears, as the web demo has it), as found ("shown")
//! and as the sampler cuts each ("cut": `cuts::cut_phrases`, what the window shows now), how many of their edges fall
//! inside a word or a line, and how many of the songs' lines a box, or a piece cut whole, holds whole. Run
//! with `EARSHOT_HOME` an empty folder (and `EARSHOT_OFFLINE=1`), it measures as the web demo listens: no voice apart,
//! no ears. The songs are downloaded into DIR (with curl) the first time and kept there; none of them goes into the
//! repository.
//!
//!   cargo run --release -p claudius --example eval_cuts -- DIR [--lang en|de|es|fr|all] [--songs N] [--why]
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
    const LANGUAGES: [(&str, &str); 4] = [("en", "English"), ("de", "German"), ("es", "Spanish"), ("fr", "French")];
    if lang != "all" && !LANGUAGES.iter().any(|(code, _)| *code == lang) {
        return Err(format!("no language {lang}: en, de, es, fr or all"));
    }
    let language =
        if lang == "all" { "every language" } else { LANGUAGES.iter().find(|l| l.0 == lang).map_or("", |l| l.1) };
    // each song, and the code of its language, whose folder holds it
    let songs: Vec<(String, &str)> = std::fs::read_to_string(&index)
        .map_err(|e| e.to_string())?
        .lines()
        .skip(1)
        .filter_map(|l| {
            let cols: Vec<&str> = l.split(',').collect();
            let code = LANGUAGES.iter().find(|(_, name)| cols.get(6) == Some(name)).map(|(code, _)| *code)?;
            (lang == "all" || code == lang).then(|| (cols[1].trim_end_matches(".mp3").to_string(), code))
        })
        .take(most)
        .collect();
    let ears = claudius::ears::Ears::load(false).ok();
    eprintln!("{} songs in {language}; ears {}", songs.len(), if ears.is_some() { "on" } else { "off" });
    let (mut bars, mut whole, mut shown, mut shown_cut) =
        (Count::default(), Count::default(), Count::default(), Count::default());
    let mut held_cut = 0usize;
    let (mut lines_all, mut held_shown, mut held_pieces) = (0usize, 0usize, 0usize);
    let (mut boxes, mut short_boxes, mut box_secs, mut line_secs) = (0usize, 0usize, 0.0f64, 0.0f64);
    println!(
        "{:<40} {:>14} {:>14} {:>14} {:>14} {:>14} {:>14} {:>12} {:>12} {:>14} {:>14} {:>12}",
        "song",
        "bars: words",
        "lines",
        "whole: words",
        "lines",
        "shown: words",
        "lines",
        "held: shown",
        "pieces",
        "cut: words",
        "lines",
        "held: cut"
    );
    for (name, code) in &songs {
        let (words, lines) = sung(&fetched(&dir, &format!("annotations/words/{name}.csv"))?)?;
        let x = claudius::audio::load(&fetched(&dir, &format!("subsets/{code}/mp3/{name}.mp3"))?)
            .map_err(|e| e.to_string())?;
        let stems = claudius::stems::of(&x, &mut |_| {});
        let a = analysis::analyze_with(&x, stems.as_deref(), None);
        let opt = Options { count: 4, kinds: vec![claudius::kinds::Kind::Phrase], ..Options::default() };
        let mut b = Count::default();
        for p in sampler::find(&x, stems.as_deref(), &a, ears.as_ref(), &opt, opt.count) {
            let first = usize::try_from(p.first).unwrap_or(0);
            b.add(&words, &lines, &[a.bar_start(first), a.bar_start(first + p.bars as usize)]);
        }
        let why = args.iter().any(|a| a == "--why");
        // the window's boxes: the phrases its voice sings as found, from its vocals apart or else the whole mix
        let sings = stems.as_deref().map_or(&x, |s| &s.vocals);
        let found = claudius::cuts::sung(sings, &claudius::analysis::melody::pitches(sings));
        let spans: Vec<(f64, f64)> = found.phrases.iter().map(|p| (p.start, p.end)).collect();
        let mut v = Count::default();
        for &(from, to) in &spans {
            v.add(&words, &lines, &[from, to]);
        }
        // and as the window shows them now: each phrase as the sampler cuts it (`cuts::cut_phrases`)
        let bar_at = |t: f64| {
            let k = ((t - a.bar_start(0)) / a.bar_secs.max(1e-3)).floor().max(0.0) as usize;
            a.bar_start(k.min(a.bars))
        };
        let cut_spans: Vec<(f64, f64)> =
            claudius::cuts::cut_phrases(sings, &found, bar_at).iter().map(|p| (p.start, p.end)).collect();
        let mut vc = Count::default();
        for &(from, to) in &cut_spans {
            vc.add(&words, &lines, &[from, to]);
        }
        let heard = why.then_some(&found);
        let mut w = Count::default();
        let mut cut: Vec<(f64, f64)> = Vec::new();
        if let Ok(pieces) = sampler::pieces(&x, ears.as_ref(), &opt, 0) {
            for p in pieces {
                let (from, to) = p.phrase.within.unwrap_or((p.phrase.start, p.phrase.start));
                w.add(&words, &lines, &[from, to]);
                cut.push((from, to));
                let Some(heard) = heard else { continue };
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
        let (hs, hp, hc) = (held(&lines, &spans), held(&lines, &cut), held(&lines, &cut_spans));
        let row =
            [b.said(b.words), b.said(b.lines), w.said(w.words), w.said(w.lines), v.said(v.words), v.said(v.lines)];
        println!(
            "{:<40} {:>14} {:>14} {:>14} {:>14} {:>14} {:>14} {:>12} {:>12} {:>14} {:>14} {:>12}",
            short(name),
            row[0],
            row[1],
            row[2],
            row[3],
            row[4],
            row[5],
            format!("{hs} of {}", lines.len()),
            format!("{hp} of {}", lines.len()),
            vc.said(vc.words),
            vc.said(vc.lines),
            format!("{hc} of {}", lines.len())
        );
        bars.join(&b);
        whole.join(&w);
        shown.join(&v);
        shown_cut.join(&vc);
        held_cut += hc;
        (lines_all, held_shown, held_pieces) = (lines_all + lines.len(), held_shown + hs, held_pieces + hp);
        boxes += spans.len();
        short_boxes += spans.iter().filter(|(a, b)| b - a < 1.5).count();
        box_secs += spans.iter().map(|(a, b)| b - a).sum::<f64>();
        line_secs += lines.iter().map(|(a, b)| b - a).sum::<f64>();
    }
    let all = [
        bars.said(bars.words),
        bars.said(bars.lines),
        whole.said(whole.words),
        whole.said(whole.lines),
        shown.said(shown.words),
        shown.said(shown.lines),
    ];
    let share = |n: usize, of: usize| if of == 0 { 0.0 } else { 100.0 * n as f64 / of as f64 };
    println!(
        "{:<40} {:>14} {:>14} {:>14} {:>14} {:>14} {:>14} {:>12} {:>12} {:>14} {:>14} {:>12}",
        "all",
        all[0],
        all[1],
        all[2],
        all[3],
        all[4],
        all[5],
        format!("{:.0}%", share(held_shown, lines_all)),
        format!("{:.0}%", share(held_pieces, lines_all)),
        shown_cut.said(shown_cut.words),
        shown_cut.said(shown_cut.lines),
        format!("{:.0}%", share(held_cut, lines_all))
    );
    println!(
        "boxes: {boxes}, {:.0}% under 1.5 s, {:.1} s on average; the songs' lines {:.1} s on average",
        share(short_boxes, boxes),
        box_secs / boxes.max(1) as f64,
        line_secs / lines_all.max(1) as f64
    );
    Ok(())
}

/// how many of `lines` one of `spans` holds whole, from before its first word to after its last
fn held(lines: &[(f64, f64)], spans: &[(f64, f64)]) -> usize {
    lines.iter().filter(|(s, e)| spans.iter().any(|(a, b)| *a <= s + SLACK && *b >= e - SLACK)).count()
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

/// `file` of the dataset, downloaded into `dir` the first time (curl told not to read the brackets some of its names
/// hold, "Intro_[Pulsschlag]_...", as a range of URLs)
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
    name.chars().take(46).collect()
}
