//! How well does a sound match a text description (earshot/match.py): calibrated z, verdict bands, forced choice
//! against alternatives, closest vocabulary terms.
//!
//! Verdicts are bands of z: strong >= 3, plausible >= 2.5, weak >= 2, else no match. Music genre / mood,
//! instrument and voice descriptions were never validated (`validated` = false): treat their verdicts as hints.

use serde::Serialize;

use crate::dsp::round_to;
use crate::vocab::{Term, Vocab};

pub const VERDICTS: [(f64, &str); 3] = [(3.0, "strong"), (2.5, "plausible"), (2.0, "weak")];
pub const SKIP_CATS: [&str; 2] = ["quality", "production"];
pub const SILENT_DBFS: f64 = -70.0;

/// held-out evidence per verdict band (match.VERDICT_EVIDENCE): precision when searching a library (~3% of
/// candidate pairs true), lenient and strict
pub fn evidence(verdict: &str) -> (f64, f64) {
    match verdict {
        "strong" => (0.98, 0.61),
        "plausible" => (0.41, 0.22),
        "weak" => (0.28, 0.17),
        _ => (0.008, 0.008),
    }
}

pub fn verdict(z: f64) -> &'static str {
    for (t, name) in VERDICTS {
        if z >= t {
            return name;
        }
    }
    "no match"
}

/// softmax of scale * x
pub fn softmax(x: &[f64], scale: f64) -> Vec<f64> {
    let m = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let e: Vec<f64> = x.iter().map(|v| (scale * (v - m)).exp()).collect();
    let s: f64 = e.iter().sum();
    e.into_iter().map(|v| v / s).collect()
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Alt {
    pub query: String,
    pub p: f64,
    pub z: f64,
    pub cosine: f64,
}

/// one query's calibrated match (match.score_queries row + closest vocabulary terms)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Match {
    pub query: String,
    pub cosine: f64,
    pub z: f64,
    pub percentile: f64,
    pub rank_in_vocab: usize,
    pub vocab_size: usize,
    pub verdict: String,
    pub validated: bool,
    /// forced choice vs the alternatives (softmax of logit_scale * cosine); None without alternatives
    pub p_vs_alts: Option<f64>,
    pub alts: Vec<Alt>,
    pub closest_vocab: Vec<Term>,
}

/// per-query rows for one audio embedding (match.score_queries): q = (text, template-averaged embedding)
pub fn score_queries(v: &Vocab, a: &[f32], queries: &[(&str, &[f32])], alts: &[(&str, &[f32])]) -> Vec<Match> {
    let rows = v.scores(a, queries);
    let alt_rows = v.scores(a, alts);
    let scale = v.logit_scale;
    let multi = if rows.len() > 1 && alts.is_empty() {
        Some(softmax(&rows.iter().map(|r| r.cosine).collect::<Vec<_>>(), scale))
    } else {
        None
    };
    rows.into_iter()
        .enumerate()
        .map(|(i, r)| {
            let kind = v.nearest_category(queries[i].1);
            let (p_vs_alts, alt_v) = if !alts.is_empty() {
                let mut c = vec![r.cosine];
                c.extend(alt_rows.iter().map(|x| x.cosine));
                let p = softmax(&c, scale);
                let av = alt_rows
                    .iter()
                    .zip(&p[1..])
                    .map(|(x, pi)| Alt { query: x.query.clone(), p: round_to(*pi, 3), z: x.z, cosine: x.cosine })
                    .collect();
                (Some(round_to(p[0], 3)), av)
            } else {
                (multi.as_ref().map(|m| round_to(m[i], 3)), vec![])
            };
            Match {
                verdict: verdict(r.z).to_string(),
                validated: !crate::vocab::UNVALIDATED.contains(&kind.as_str()),
                query: r.query,
                cosine: r.cosine,
                z: r.z,
                percentile: r.percentile,
                rank_in_vocab: r.rank_in_vocab,
                vocab_size: r.vocab_size,
                p_vs_alts,
                alts: alt_v,
                closest_vocab: vec![],
            }
        })
        .collect()
}

/// z for text, floored to 0.1 so it can never contradict a verdict threshold (2.96 -> '+2.9')
pub fn zf(z: f64) -> String {
    let v = (z * 10.0 + 1e-9).floor() / 10.0;
    format!("{v:+.1}")
}

fn pct(p: f64) -> String {
    if p > 0.995 {
        ">99%".into()
    } else if p < 0.005 {
        "<1%".into()
    } else {
        format!("{:.0}%", 100.0 * p)
    }
}

/// earshot's plain-language match summary (rows sorted by z, best first)
pub fn summary(name: &str, rows: &[Match], near: &[Term], peak_dbfs: Option<f64>) -> String {
    let unval = " (unvalidated: music / instrument / voice description)";
    let parts: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "'{}' -> {}{} (z {}; would rank #{} among {} vocabulary descriptions; cosine {:.2})",
                r.query,
                r.verdict,
                if r.validated { "" } else { unval },
                zf(r.z),
                r.rank_in_vocab,
                r.vocab_size,
                r.cosine
            )
        })
        .collect();
    let mut s = format!("{name}: {}.", parts.join("; "));
    let best = &rows[0];
    if !best.alts.is_empty() {
        s += &format!(
            " Forced choice vs your alternatives: '{}' {}, {}.",
            best.query,
            pct(best.p_vs_alts.unwrap_or(0.0)),
            best.alts.iter().map(|x| format!("'{}' {}", x.query, pct(x.p))).collect::<Vec<_>>().join(", ")
        );
    } else if rows.len() > 1 {
        s += &format!(
            " Forced choice among your queries: {}.",
            rows.iter()
                .map(|r| format!("'{}' {}", r.query, pct(r.p_vs_alts.unwrap_or(0.0))))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let queries: Vec<&str> = rows.iter().map(|r| r.query.as_str()).collect();
    let alt_txt: Vec<String> = near
        .iter()
        .filter(|x| !queries.contains(&x.term.as_str()))
        .map(|x| format!("'{}' (z {})", x.term, zf(x.z)))
        .collect();
    if !alt_txt.is_empty() {
        s += &format!(" It sounds most like: {}.", alt_txt.join(", "));
    }
    if best.verdict != "no match" && best.validated {
        let (lib, strict) = evidence(&best.verdict);
        s += &format!(
            " Evidence for '{}' (held-out library search, ~3% of candidate pairs true): the right kind of sound {:.0}% of the time, the exact variant {:.0}% (a close variant such as an Enter key for a spacebar counts as wrong then).",
            best.verdict,
            100.0 * lib,
            100.0 * strict
        );
    }
    if let Some(pk) = peak_dbfs.filter(|p| *p < SILENT_DBFS) {
        s = format!("WARNING: near-silent input (peak {pk:.0} dBFS), scores are meaningless. {s}");
    }
    s
}
