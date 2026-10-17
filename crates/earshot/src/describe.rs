//! File-level conveniences mirroring earshot's Python API: describe (describe.py), match (match.py), rank
//! (search.rank) and compare (compare.compare), with earshot's plain-language summaries.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::dsp::{round_to, stats};
use crate::features::Measure;
use crate::matching::{self, Match, SKIP_CATS, VERDICTS, verdict, zf};
use crate::realness::fmt_t;
use crate::vocab::{self, Term, UNVALIDATED};
use crate::{Ears, audio, compare, features};

const SRF: f64 = crate::SR as f64;
pub const LONG_S: f64 = 20.0;
pub const SILENT_DBFS: f64 = -70.0;
pub const MONO_DB: f64 = -40.0;
pub const MUSIC_P: f64 = 0.5;

fn name_of(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

fn hz(f: f64) -> String {
    if f >= 1000.0 { format!("{:.1} kHz", f / 1000.0) } else { format!("{f:.0} Hz") }
}

fn ms(v: f64) -> String {
    if v >= 2000.0 { format!("{:.1} s", v / 1000.0) } else { format!("{v:.0} ms") }
}

fn note(f: f64) -> String {
    let m = (69.0 + 12.0 * (f / 440.0).log2()).round_ties_even() as i64;
    format!("{}{}", features::NOTE[m.rem_euclid(12) as usize], m.div_euclid(12) - 1)
}

/// a few words on the envelope / texture from the measurements (describe.shape)
pub fn shape(m: &Measure) -> String {
    let d = m.duration;
    let (att, dec, ons) = (m.attack_ms, m.decay_ms, m.onset_count);
    let rate = m.onsets_per_s.unwrap_or(0.0);
    if d > LONG_S {
        let mut words = vec![match ons {
            None => "long recording".to_string(),
            Some(_) if rate >= 1.5 => format!("busy, {rate:.1} onsets/s"),
            Some(o) if o as f64 <= d / 10.0 => "steady, few onsets".to_string(),
            Some(o) => format!("{o} separate events"),
        }];
        if let Some(l) = m.lra_lu {
            words.push(format!("loudness range {l:.0} LU"));
        }
        return words.join(", ");
    }
    let mut words = if d < 2.5 && ons.is_none_or(|o| o <= 3) {
        let kind = match att {
            Some(a) if a <= 15.0 => "transient",
            Some(a) if a >= 150.0 => "swell",
            _ => "one-shot",
        };
        vec![format!("short {kind}")]
    } else if rate >= 1.5 {
        vec![format!("busy, {rate:.1} onsets/s")]
    } else if ons.unwrap_or(0) >= 2 {
        vec![format!("{} separate events", ons.unwrap())]
    } else {
        vec!["sustained".to_string()]
    };
    if let Some(f0) = m.f0_hz.filter(|v| *v != 0.0) {
        words.push(format!("tonal, pitch ~{f0:.0} Hz ({})", note(f0)));
    } else if m.flatness.unwrap_or(0.0) >= 0.3 {
        words.push("noise-like".into());
    }
    let mut env = Vec::new();
    if let Some(a) = att {
        env.push(format!("attack {a:.0} ms"));
    }
    if let Some(dd) = dec {
        env.push(format!("decays in {}", ms(dd)));
    }
    words.join(", ") + &if env.is_empty() { String::new() } else { format!(" ({})", env.join(", ")) }
}

/// the measurements worth showing (describe.measurements), in earshot's key order
pub fn measurements(m: &Measure) -> Map<String, Value> {
    let width = match m.side_to_mid_db {
        Some(sm) if sm >= MONO_DB => format!("side/mid {sm:.0} dB"),
        _ => "mono".into(),
    };
    let mut out = Map::new();
    let mut put = |k: &str, v: Option<Value>| {
        if let Some(v) = v {
            out.insert(k.into(), v);
        }
    };
    put("duration", Some(json!(m.duration)));
    put("peak_dbfs", m.peak_dbfs.map(|v| json!(v)));
    put("true_peak_dbtp", m.true_peak_dbtp.map(|v| json!(v)));
    put("rms_dbfs", m.rms_dbfs.map(|v| json!(v)));
    put("lufs", m.lufs.map(|v| json!(v)));
    put("lra_lu", m.lra_lu.map(|v| json!(v)));
    put("attack_ms", m.attack_ms.map(|v| json!(v)));
    put("decay_ms", m.decay_ms.map(|v| json!(v)));
    put("brightness_hz", m.brightness_hz.map(|v| json!(v)));
    put("bandwidth_hz", m.bandwidth_hz.map(|v| json!(v)));
    put("noise_floor_dbfs", m.noise_floor_dbfs.filter(|_| m.duration >= 1.0).map(|v| json!(v)));
    put("flatness", m.flatness.map(|v| json!(v)));
    put("harmonic_ratio", m.harmonic_ratio.map(|v| json!(v)));
    put("f0_hz", m.f0_hz.map(|v| json!(v)));
    put("onset_count", m.onset_count.map(|v| json!(v)));
    put("onsets_per_s", m.onsets_per_s.map(|v| json!(v)));
    put("width", Some(json!(width)));
    put("digital_silence_frac", m.digital_silence_frac.map(|v| json!(v)));
    out
}

fn meas_text(m: &Map<String, Value>) -> String {
    let f = |k: &str| m.get(k).and_then(|v| v.as_f64());
    let mut s = vec![format!("peak {:.1} dBFS", f("peak_dbfs").unwrap_or(0.0))];
    if let Some(l) = f("lufs") {
        s.push(format!("{l:.1} LUFS"));
    }
    if let Some(b) = f("brightness_hz").filter(|v| *v != 0.0) {
        s.push(format!("brightness {}", hz(b)));
    }
    if let Some(b) = f("bandwidth_hz").filter(|v| *v != 0.0) {
        s.push(format!("bandwidth {}", hz(b)));
    }
    if let Some(nf) = f("noise_floor_dbfs")
        && f("duration").unwrap_or(0.0) <= LONG_S
    {
        s.push(if nf < -120.0 { "digital silence between sounds".into() } else { format!("noise floor {nf:.0} dBFS") });
    }
    s.push(m.get("width").and_then(|v| v.as_str()).unwrap_or("mono").to_string());
    s.join(", ")
}

#[derive(Serialize, Clone, Debug)]
pub struct Tag {
    pub term: String,
    pub category: String,
    pub z: f64,
    pub cosine: f64,
    pub verdict: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Label {
    pub label: String,
    pub p: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Window {
    pub start: f64,
    pub end: f64,
    pub top_tag: Tag,
    pub audioset: Vec<Label>,
}

/// describe.describe result
#[derive(Serialize, Clone, Debug)]
pub struct Description {
    pub path: String,
    pub duration: f64,
    pub summary: String,
    pub shape: String,
    pub top_tags: Vec<Tag>,
    pub music_hints: Vec<Term>,
    pub tags: Map<String, Value>,
    pub audioset: Vec<Label>,
    pub character: Vec<String>,
    pub measurements: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct_windows: Option<Vec<Window>>,
}

fn reading(z: f64) -> &'static str {
    match verdict(z) {
        "strong" => "clear reading",
        "plausible" => "plausible reading",
        "weak" => "weak reading",
        _ => "tentative: nothing in the vocabulary stands out",
    }
}

fn tags(ears: &Ears, a: &[f32], top: usize) -> (Vec<Tag>, Vec<Term>, Map<String, Value>) {
    let v = &ears.vocab;
    let rows = v.top_terms(a, v.data.names.len(), None, &SKIP_CATS);
    let best: Vec<Tag> = rows
        .iter()
        .filter(|t| !UNVALIDATED.contains(&t.category.as_str()))
        .take(top)
        .map(|t| Tag {
            term: t.term.clone(),
            category: t.category.clone(),
            z: t.z,
            cosine: t.cosine,
            verdict: verdict(t.z).into(),
        })
        .collect();
    let hints: Vec<Term> =
        rows.iter().filter(|t| UNVALIDATED.contains(&t.category.as_str())).take(top).cloned().collect();
    let mut per = Map::new();
    for (cat, _) in &v.data.cats {
        let t = v.top_terms(a, 3, Some(&[cat.as_str()]), &[]);
        per.insert(
            cat.clone(),
            json!(t.iter().map(|x| json!({"term": x.term, "z": x.z, "cosine": x.cosine})).collect::<Vec<_>>()),
        );
    }
    (best, hints, per)
}

fn ast_top(ears: &Ears, p: &[f32], k: usize, min_p: f64) -> Vec<Label> {
    let a = ears.ast.as_ref().unwrap();
    a.top(p, k, 0.0)
        .into_iter()
        .enumerate()
        .filter(|(i, (_, pr))| *pr >= min_p || *i == 0)
        .map(|(_, (label, p))| Label { label, p })
        .collect()
}

fn ast_text(top: &[Label]) -> String {
    let labels: Vec<String> = top.iter().take(3).map(|d| format!("{} {:.2}", d.label, d.p)).collect();
    if top[0].p >= 0.2 {
        format!("AudioSet: {}.", labels.join(", "))
    } else {
        format!("AudioSet tagger unsure ({}).", labels.join(", "))
    }
}

/// up to n non-overlapping windows, farthest from the file's mean embedding first, each with a different top tag
pub fn distinct_windows(spans: &[(f64, f64)], w: &[Vec<f32>], top_tags: &[String], n: usize) -> Vec<usize> {
    if spans.len() <= 1 {
        return vec![];
    }
    let mean = crate::clap::mean_norm(w);
    let d0: Vec<f64> =
        w.iter().map(|v| 1.0 - v.iter().zip(&mean).map(|(a, b)| *a as f64 * *b as f64).sum::<f64>()).collect();
    let mut picked: Vec<usize> = Vec::new();
    for i in stats::argsort_desc(&d0) {
        if picked.len() >= n {
            break;
        }
        let free = picked.iter().all(|&j| spans[i].1 <= spans[j].0 + 1e-6 || spans[i].0 >= spans[j].1 - 1e-6);
        if free && !picked.iter().any(|&j| top_tags[j] == top_tags[i]) {
            picked.push(i);
        }
    }
    if picked.len() > 1 {
        picked.sort();
        picked
    } else {
        vec![]
    }
}

/// describe.describe for a file
pub fn describe(ears: &Ears, path: &Path, top: usize) -> Result<Description> {
    let (l, r) = audio::load_stereo(path)?;
    let dur = l.len() as f64 / SRF;
    let m = features::measure(&l, &r, false, dur > 60.0);
    let mut res = Description {
        path: path.display().to_string(),
        duration: round_to(dur, 3),
        summary: String::new(),
        shape: String::new(),
        top_tags: vec![],
        music_hints: vec![],
        tags: Map::new(),
        audioset: vec![],
        character: vec![],
        measurements: Map::new(),
        distinct_windows: None,
    };
    let untagged = |mut res: Description, why: String| {
        res.shape = why.clone();
        res.summary = why;
        res
    };
    if m.error.is_some() {
        res.measurements.insert("duration".into(), json!(round_to(dur, 3)));
        return Ok(untagged(
            res,
            format!("{:.1} ms of audio: too short to describe or measure (needs at least 5 ms).", dur * 1000.0),
        ));
    }
    let meas = measurements(&m);
    res.measurements = meas.clone();
    let pk = m.peak_dbfs.unwrap_or(-200.0);
    if pk < SILENT_DBFS {
        return Ok(untagged(
            res,
            format!(
                "{dur:.2} s of near-silence (peak {pk:.0} dBFS): nothing to describe; model tags of silence are meaningless."
            ),
        ));
    }
    let Some(ast) = ears.ast.as_ref() else { bail!("describe needs the AudioSet tagger (EarsOptions::load_ast)") };
    let mono = audio::load_mono(path)?;
    let (a, p, wins) = if dur > LONG_S {
        let spans = vocab::spans_for(0.0, dur, vocab::MAX_WIN);
        let w = ears.embed_spans(&mono, &spans, crate::Pad::RepeatPad)?;
        let a = crate::clap::mean_norm(&w);
        let xm = audio::mean_mono(&l, &r);
        let segs: Vec<&[f32]> = spans
            .iter()
            .map(|&(s, e)| {
                let (i, j) = (((s * SRF) as usize).min(xm.len()), ((e * SRF) as usize).min(xm.len()));
                &xm[i..j.max(i)]
            })
            .collect();
        let pw = ast.probs(&segs, crate::Reduce::Max)?;
        let mut p = pw[0].clone();
        for row in &pw[1..] {
            for (x, y) in p.iter_mut().zip(row) {
                *x = x.max(*y);
            }
        }
        (a, p, Some((spans, w, pw)))
    } else {
        let a = ears.embed(&mono)?;
        let p = ast.probs(&[&mono], crate::Reduce::Max)?.remove(0);
        (a, p, None)
    };
    let (best, hints, per) = tags(ears, &a, top);
    let at = ast_top(ears, &p, 5, 0.05);
    let p_music = ast.prob_of(&p, "Music").unwrap_or(0.0);
    let mut other: Vec<&Tag> = Vec::new();
    let mut seen: Vec<String> = best.iter().take(3).map(|b| b.category.clone()).collect();
    for b in best.iter().skip(3) {
        if !seen.contains(&b.category) && other.len() < 2 {
            other.push(b);
            seen.push(b.category.clone());
        }
    }
    let character: Vec<String> = per
        .get("quality")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|d| d["z"].as_f64().unwrap_or(0.0) >= VERDICTS[1].0)
                .filter_map(|d| d["term"].as_str())
                .map(|t| {
                    let rest = t.split_once(' ').map_or(t, |(_, r)| r);
                    rest.strip_suffix(" sound").unwrap_or(rest).to_string()
                })
                .take(2)
                .collect()
        })
        .unwrap_or_default();
    res.shape = shape(&m);
    let mut s = format!("{dur:.2} s {}.", res.shape);
    if p_music >= MUSIC_P {
        let genre: Vec<String> = hints
            .iter()
            .filter(|h| h.category == "music_genre")
            .take(2)
            .map(|h| format!("{} (z {})", h.term, zf(h.z)))
            .collect();
        let mood: Vec<String> = hints
            .iter()
            .filter(|h| h.category == "music_mood")
            .take(2)
            .map(|h| format!("{} (z {})", h.term, zf(h.z)))
            .collect();
        s += &format!(
            " Music (AudioSet Music {p_music:.2}). Unvalidated hints - genre: {}; mood (wrong for ~1 in 5 tracks in a test): {}.",
            genre.join(", "),
            mood.join(", ")
        );
    }
    if best.first().is_some_and(|b| b.z >= VERDICTS[2].0) {
        s += &format!(
            " Sounds most like: {} [{}].",
            best.iter().take(3).map(|b| format!("{} (z {})", b.term, zf(b.z))).collect::<Vec<_>>().join(", "),
            reading(best[0].z)
        );
        if !other.is_empty() {
            s += &format!(
                " Other readings: {}.",
                other
                    .iter()
                    .map(|b| format!("{} ({}, z {})", b.term, b.category, zf(b.z)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    } else {
        s += &format!(
            " No sound-effect description stands out (closest: {}; tentative).",
            best.iter().take(2).map(|b| format!("{} z {}", b.term, zf(b.z))).collect::<Vec<_>>().join(", ")
        );
    }
    if p_music < MUSIC_P && hints.first().is_some_and(|h| h.z >= VERDICTS[0].0) {
        s += &format!(" Unvalidated hint: {} (z {}).", hints[0].term, zf(hints[0].z));
    }
    if !character.is_empty() {
        s += &format!(" Character: {}.", character.join(", "));
    }
    s += &format!(" {} {}.", ast_text(&at), meas_text(&meas));
    if let Some((spans, w, pw)) = wins {
        let wt: Vec<Tag> = w.iter().map(|v| tags(ears, v, 1).0.remove(0)).collect();
        let names: Vec<String> = wt.iter().map(|t| t.term.clone()).collect();
        let wins: Vec<Window> = distinct_windows(&spans, &w, &names, 3)
            .into_iter()
            .map(|i| Window {
                start: round_to(spans[i].0, 3),
                end: round_to(spans[i].1, 3),
                top_tag: wt[i].clone(),
                audioset: ast_top(ears, &pw[i], 3, 0.1),
            })
            .collect();
        if !wins.is_empty() {
            s += &format!(
                " Windows that read differently: {}.",
                wins.iter()
                    .map(|w| format!(
                        "{}-{} {} (z {}; AudioSet {} {:.2})",
                        fmt_t(w.start),
                        fmt_t(w.end),
                        w.top_tag.term,
                        zf(w.top_tag.z),
                        w.audioset[0].label,
                        w.audioset[0].p
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        } else {
            s += &format!(" Uniform: all {} analysed 10 s windows read as {}.", spans.len(), wt[0].term);
        }
        res.distinct_windows = Some(wins);
    }
    res.top_tags = best;
    res.music_hints = hints;
    res.tags = per;
    res.audioset = at;
    res.character = character;
    res.summary = s;
    Ok(res)
}

/// match.match result for a file
#[derive(Serialize, Clone, Debug)]
pub struct FileMatch {
    pub path: String,
    pub summary: String,
    pub peak_dbfs: f64,
    pub best: Match,
    pub results: Vec<Match>,
    pub closest_vocab: Vec<Term>,
}

/// match.match: calibrated match of one or several descriptions against a file
pub fn match_file(ears: &Ears, path: &Path, queries: &[&str], alts: &[&str]) -> Result<FileMatch> {
    let mono = audio::load_mono(path)?;
    let a = ears.embed(&mono)?;
    let rows = ears.match_queries(&a, queries, alts)?;
    let near = rows[0].closest_vocab.clone();
    let (l, r) = audio::load_stereo(path)?;
    let pk = crate::dsp::db(l.iter().chain(&r).fold(0.0f32, |m, v| m.max(v.abs())) as f64);
    let summary = matching::summary(&name_of(path), &rows, &near, Some(pk));
    Ok(FileMatch {
        path: path.display().to_string(),
        summary,
        peak_dbfs: round_to(pk, 1),
        best: rows[0].clone(),
        results: rows,
        closest_vocab: near,
    })
}

#[derive(Serialize, Clone, Debug)]
pub struct Ranked {
    pub path: String,
    pub name: String,
    pub z: f64,
    pub cosine: f64,
    pub vocab_percentile: f64,
    pub verdict: String,
    pub near_silent: bool,
    pub sounds_most_like: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct RankResult {
    pub query: String,
    pub n_files: usize,
    pub results: Vec<Ranked>,
    pub skipped: Vec<Value>,
    pub summary: String,
}

fn weakly(v: &str) -> String {
    if v == "no match" { "not a match".into() } else { format!("only a {v} match") }
}

/// search.rank: files sorted by how well they match `query` (near-silent files last)
pub fn rank_files(ears: &Ears, paths: &[PathBuf], query: &str, top: Option<usize>) -> Result<RankResult> {
    let files = audio::list_media(paths);
    let mut monos = Vec::new();
    let mut ok = Vec::new();
    let mut skipped = Vec::new();
    for f in files {
        match audio::load_mono(&f) {
            Ok(x) => {
                monos.push(x);
                ok.push(f);
            }
            Err(e) => skipped.push(json!({"path": f.display().to_string(), "error": e.to_string()})),
        }
    }
    let skip_txt = if skipped.is_empty() {
        String::new()
    } else {
        format!(
            " Skipped {} unreadable file(s): {}.",
            skipped.len(),
            skipped
                .iter()
                .take(3)
                .filter_map(|s| s["path"].as_str())
                .map(|p| name_of(Path::new(p)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    if ok.is_empty() {
        return Ok(RankResult {
            query: query.into(),
            n_files: 0,
            results: vec![],
            skipped,
            summary: format!("no readable media files found.{skip_txt}"),
        });
    }
    let refs: Vec<&[f32]> = monos.iter().map(|v| v.as_slice()).collect();
    let embs = ears.embed_many(&refs)?;
    let q = ears.embed_text(&[query])?.remove(0);
    let mut rows: Vec<(usize, f64, f64, f64, bool)> = Vec::new();
    for (i, e) in embs.iter().enumerate() {
        let c = ears.vocab.calibrate(e);
        let r = ears.vocab.query(&c, e, &q);
        let silent = audio::peak_db(&monos[i]) < SILENT_DBFS;
        rows.push((i, round_to(r.z, 2), r.cos, r.pct, silent));
    }
    let mut order = rows.clone();
    order.sort_by(|a, b| a.4.cmp(&b.4).then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)));
    let take = top.unwrap_or(order.len()).min(order.len());
    let results: Vec<Ranked> = order[..take]
        .iter()
        .map(|&(i, z, cos, pct, silent)| {
            let bt = ears.vocab.top_terms(&embs[i], 1, None, &SKIP_CATS).remove(0);
            Ranked {
                path: ok[i].display().to_string(),
                name: name_of(&ok[i]),
                z,
                cosine: round_to(cos, 3),
                vocab_percentile: round_to(100.0 * pct, 1),
                verdict: if silent { "no match".into() } else { verdict(z).into() },
                near_silent: silent,
                sounds_most_like: format!("{} (z {})", bt.term, zf(bt.z)),
            }
        })
        .collect();
    let count = |n: &str| order.iter().filter(|r| !r.4 && verdict(r.1) == n).count();
    let head: Vec<String> = results
        .iter()
        .take(3)
        .filter(|x| !x.near_silent)
        .map(|x| format!("{} (z {}, {})", x.name, zf(x.z), x.verdict))
        .collect();
    let mut s = format!(
        "Ranked {} files for '{query}'. Best: {}. {} strong, {} plausible, {} weak match(es).",
        ok.len(),
        if head.is_empty() { "none (all near-silent)".to_string() } else { head.join(", ") },
        count("strong"),
        count("plausible"),
        count("weak")
    );
    if let Some(r0) = results.first()
        && !r0.near_silent
        && (r0.verdict == "weak" || r0.verdict == "no match")
    {
        s += &format!(" Even the best is {}: it sounds more like {}.", weakly(&r0.verdict), r0.sounds_most_like);
    }
    if count("strong") > 1 {
        let (lib, strict) = matching::evidence("strong");
        s += &format!(
            " In a held-out test 'strong' was right {:.0}% of the time, {:.0}% when a close variant (a mouth pop for a cork pop) counts as wrong.",
            100.0 * lib,
            100.0 * strict
        );
    }
    let n_silent = rows.iter().filter(|r| r.4).count();
    if n_silent > 0 {
        s += &format!(
            " {n_silent} near-silent file(s) (peak < {SILENT_DBFS:.0} dBFS) ranked last: their scores mean nothing."
        );
    }
    Ok(RankResult { query: query.into(), n_files: ok.len(), results, skipped, summary: s + &skip_txt })
}

fn ordinal(v: f64) -> String {
    let n = v.round_ties_even() as i64;
    let suf = if (10..=20).contains(&(n % 100)) {
        "th"
    } else {
        match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{n}{suf}")
}

/// compare.compare: nearest references, typicality vs the set, feature-level differences in words
pub fn compare_files(ears: &Ears, path: &Path, refs: &[PathBuf], k: usize) -> Result<Value> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let me = canon(path);
    let mut rpaths = Vec::new();
    let mut skipped = Vec::new();
    let mut monos = Vec::new();
    for f in audio::list_media(refs).into_iter().filter(|f| canon(f) != me) {
        match audio::load_mono(&f) {
            Ok(x) => {
                monos.push(x);
                rpaths.push(f);
            }
            Err(e) => skipped.push(json!({"path": f.display().to_string(), "error": e.to_string()})),
        }
    }
    let skip_txt =
        if skipped.is_empty() { String::new() } else { format!(" Skipped {} unreadable file(s).", skipped.len()) };
    if rpaths.len() < 2 {
        return Ok(
            json!({"summary": format!("need at least 2 readable reference files, found {}.{skip_txt}", rpaths.len()), "path": path.display().to_string(), "n_refs": rpaths.len(), "nearest": [], "skipped": skipped}),
        );
    }
    let x = audio::load_mono(path)?;
    let mut all: Vec<&[f32]> = vec![&x];
    all.extend(monos.iter().map(|v| v.as_slice()));
    let e = ears.embed_many(&all)?;
    let (ax, ar) = (&e[0], &e[1..]);
    let cos: Vec<f64> = ar.iter().map(|r| r.iter().zip(ax).map(|(a, b)| *a as f64 * *b as f64).sum()).collect();
    let order = stats::argsort_desc(&cos);
    let nearest: Vec<Value> = order
        .iter()
        .take(k)
        .map(|&i| json!({"path": rpaths[i].display().to_string(), "name": name_of(&rpaths[i]), "cosine": round_to(cos[i], 3)}))
        .collect();
    let (sx, z, pct) = compare::typicality_raw(ax, ar, k);
    let (l, r) = audio::load_stereo(path)?;
    let fx = compare::measure_feats(&l, &r);
    let fr: Vec<compare::Feats> =
        rpaths.iter().filter_map(|p| audio::load_stereo(p).ok().map(|(l, r)| compare::measure_feats(&l, &r))).collect();
    let keys = [
        "loudness_lufs",
        "peak_dbfs",
        "brightness_hz",
        "bandwidth_hz",
        "attack_ms",
        "decay_ms",
        "length_s",
        "duration",
        "noise_floor_dbfs",
        "width_db",
        "low_end_db",
    ];
    let min_scale = |k: &str| match k {
        "loudness_lufs" | "peak_dbfs" | "low_end_db" => 1.0,
        "noise_floor_dbfs" | "width_db" => 2.0,
        "brightness_hz" | "bandwidth_hz" | "length_s" => 0.05,
        "attack_ms" | "decay_ms" => 0.1,
        _ => 0.1,
    };
    let mut fz = Map::new();
    let mut robust: Vec<(String, f64, f64, f64)> = Vec::new();
    for key in keys {
        let Some(fxv) = fx.get(key) else { continue };
        let vals: Vec<f64> = fr.iter().filter_map(|f| f.get(key)).map(|v| compare::log_value(key, v)).collect();
        if vals.len() < 2 || key == "duration" {
            continue;
        }
        let med = stats::median(&vals);
        let mad: Vec<f64> = vals.iter().map(|v| (v - med).abs()).collect();
        let scale = (1.4826 * stats::median(&mad)).max(min_scale(key));
        let rz = round_to((compare::log_value(key, fxv) - med) / scale, 2);
        let raw: Vec<f64> = fr.iter().filter_map(|f| f.get(key)).collect();
        let refs_median = round_to(
            if ["brightness_hz", "bandwidth_hz", "attack_ms", "decay_ms", "length_s"].contains(&key) {
                med.exp()
            } else {
                med
            },
            3,
        );
        // measure() reports these two as integers
        let value = if key == "brightness_hz" || key == "bandwidth_hz" { json!(fxv as i64) } else { json!(fxv) };
        fz.insert(
            key.into(),
            json!({"value": value, "refs_median": refs_median, "robust_z": rz,
                   "refs_p10_p90": [round_to(stats::percentile(&raw, 10.0), 3), round_to(stats::percentile(&raw, 90.0), 3)]}),
        );
        robust.push((key.to_string(), rz, fxv, refs_median));
    }
    let order_k: Vec<&str> = compare::DIFFS.iter().map(|d| d.0).collect();
    let mut outl: Vec<&(String, f64, f64, f64)> =
        robust.iter().filter(|r| r.1.abs() >= 2.0 && order_k.contains(&r.0.as_str())).collect();
    outl.sort_by_key(|r| order_k.iter().position(|k| *k == r.0).unwrap());
    let words: Vec<String> = outl
        .iter()
        .take(4)
        .map(|(kk, rz, v, med)| {
            let up = *rz > 0.0;
            if kk == "loudness_lufs" {
                format!("{:.0} dB {} than their median", (v - med).abs(), if up { "louder" } else { "quieter" })
            } else {
                compare::phrase(kk, up, &format!("median {}", compare::fmt(kk, *med)), &compare::fmt(kk, *v))
            }
        })
        .collect();
    let fdist = if robust.is_empty() {
        None
    } else {
        Some(round_to(stats::median(&robust.iter().map(|r| r.1.abs()).collect::<Vec<_>>()), 2))
    };
    let mut s = format!(
        "{} vs {} references. Nearest: {}. Typicality z {} ({} percentile of how close the references are to each other): {}.",
        name_of(path),
        rpaths.len(),
        order
            .iter()
            .take(k.min(3))
            .map(|&i| format!("{} (cos {:.2})", name_of(&rpaths[i]), cos[i]))
            .collect::<Vec<_>>()
            .join(", "),
        zf(z),
        ordinal(pct),
        compare::fit_words(z)
    );
    s += &if words.is_empty() {
        " Every measurement is within the references' spread.".to_string()
    } else {
        format!(" Measured against the references it is {}.", words.join("; "))
    };
    Ok(json!({
        "summary": s + &skip_txt, "path": path.display().to_string(), "n_refs": rpaths.len(), "nearest": nearest,
        "typicality": {"mean_cos_knn": round_to(sx, 3), "z": round_to(z, 2), "percentile": round_to(pct, 1), "k": k, "verdict": compare::fit_words(z)},
        "features": fz, "feature_distance": fdist, "skipped": skipped,
    }))
}
