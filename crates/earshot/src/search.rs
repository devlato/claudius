//! Find the best sound for a description (earshot/search.py): rank many sounds, or find the best moments
//! (sample trims) inside one recording.
//!
//! find: candidate segments (onset strokes / events and sliding windows, snapped to the sound for one-shot queries)
//! are scored with CLAP and the best non-overlapping ones returned with their calibrated z and verdict.

use serde::Serialize;

use anyhow::Result;

use crate::dsp::{db, round_to, stats};
use crate::matching::{SILENT_DBFS, verdict};

const SRF: f64 = crate::SR as f64;
pub const FRAME: f64 = 0.005;
pub const PRE_ROLL: f64 = 0.005;
pub const MARGIN: f64 = 0.1;
pub const CAP_S: f64 = 2.0;
pub const MERGE_GAP: f64 = 0.25;
pub const DECAY_DB: f64 = 35.0;
pub const MAX_EVENTS: usize = 48;
pub const MAX_WINDOWS: usize = 40;
pub const MAX_WIN_S: f64 = 10.0;
pub const SINGLE_STAGE_S: f64 = 60.0;
pub const TOP_WINDOWS: usize = 6;
pub const BUCKET_S: f64 = 2.0;
pub const REFINE_DZ: f64 = 0.5;
pub const WEAK_DB: f64 = 15.0;
pub const CUT_DB: f64 = -15.0;
pub const TAIL_PENALTY: f64 = 0.5;
pub const EDIT_DB: f64 = -100.0;
pub const EDIT_S: f64 = 0.01;
pub const SNAP_S: f64 = 1.0;
pub const QUIET_DB: f64 = 40.0;
pub const EVENT_ANCHORS: [&str; 6] = [
    "a single short sound effect",
    "one short hit",
    "a click",
    "a single impact",
    "one short pop",
    "a quick one-shot sound",
];
pub const TEXTURE_ANCHORS: [&str; 6] = [
    "a continuous sound",
    "a long background atmosphere",
    "a steady drone",
    "a piece of music",
    "ambient room noise",
    "a soundscape",
];
pub const KIND_EVENT: f64 = -0.08;
pub const KIND_TEXTURE: f64 = -0.27;

/// a candidate (start s, end s, kind, priority)
pub type Cand = (f64, f64, &'static str, f64);

/// one returned trim (search.find segment)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Trim {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    /// z minus the cut penalty (what the ranking used)
    pub score: f64,
    pub z: f64,
    pub cosine: f64,
    pub verdict: String,
    pub kind: String,
    pub peak_dbfs: f64,
    pub head_db: f64,
    pub tail_db: f64,
    pub cut: String,
}

/// the whole find result (search.find)
#[derive(Serialize, Clone, Debug)]
pub struct FindResult {
    pub query: String,
    pub query_kind: String,
    pub query_kind_score: f64,
    pub duration: f64,
    pub n_candidates: usize,
    pub n_windows: usize,
    pub n_events: usize,
    pub segments: Vec<Trim>,
    pub summary: String,
}

/// 20 log10 of the RMS of `frame`-second frames (search._env_db)
pub fn env_db(x: &[f32], frame: f64) -> Vec<f64> {
    let w = ((frame * SRF).round_ties_even() as usize).max(1);
    crate::features::frame_rms(x, w).into_iter().map(|v| 20.0 * v.max(1e-12).log10()).collect()
}

fn fi(t: f64) -> usize {
    (t / FRAME).max(0.0) as usize
}

fn seg_of(env: &[f64], a: usize, b: usize) -> &[f64] {
    let n = env.len();
    &env[a.min(n)..b.min(n).max(a.min(n))]
}

fn refine_start(env1: &[f64], t: f64, floor: f64) -> f64 {
    let c = (t * 1000.0) as i64;
    let i0 = (c - 30).max(0) as usize;
    let i1 = ((c + 30).max(0) as usize).min(env1.len());
    if i1 <= i0 {
        return t;
    }
    let seg = &env1[i0..i1];
    let thr = (floor + 10.0).max(stats::max(seg) - 30.0);
    let k = seg.iter().position(|&v| v >= thr).unwrap_or(0);
    (i0 + k) as f64 / 1000.0
}

/// level of the last 10 ms of [a, b) relative to the segment's peak (5 ms frames)
pub fn tail_db(env: &[f64], a: f64, b: f64) -> f64 {
    let s = seg_of(env, fi(a), (fi(a) + 2).max(fi(b)));
    if s.is_empty() {
        return 0.0;
    }
    stats::max(&s[s.len().saturating_sub(2)..]) - stats::max(s)
}

/// level of the first 5 ms frame of [a, b) relative to the segment's peak
pub fn head_db(env: &[f64], a: f64, b: f64) -> f64 {
    let s = seg_of(env, fi(a), (fi(a) + 2).max(fi(b)));
    if s.is_empty() { 0.0 } else { s[0] - stats::max(s) }
}

/// True when [a, b) holds an edit gap: >= 10 ms of exact digital silence (5 ms frames)
pub fn edit_gaps(env: &[f64], a: f64, b: f64) -> bool {
    let s = seg_of(env, fi(a), fi(b));
    let n = (EDIT_S / FRAME).round_ties_even() as usize;
    if s.len() < n {
        return false;
    }
    s.windows(n).any(|w| w.iter().all(|v| *v <= EDIT_DB))
}

fn quiet_level(env: &[f64], floor: f64, i0: usize, i1: usize) -> f64 {
    let s = seg_of(env, i0, i1);
    if s.is_empty() { floor + 6.0 } else { (floor + 6.0).max(stats::max(s) - QUIET_DB) }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// (a, b) of a window moved onto the sound's own edges; lo / hi bound the search (search.snap_window)
pub fn snap_window(env: &[f64], floor: f64, a: f64, b: f64, lo: f64, hi: f64) -> (f64, f64) {
    let n = env.len();
    let mut i0 = (a / FRAME).round_ties_even() as usize;
    let mut i1 = (i0 + 1).max((b / FRAME).round_ties_even() as usize).min(n);
    if i0 + 1 >= i1 {
        return (a, b);
    }
    let q = quiet_level(env, floor, i0, i1);
    let q_end = (floor + 4.0).max(stats::max(&env[i0..i1]) - DECAY_DB);
    let w = 6usize;
    let n_lo = ((lo / FRAME).round_ties_even().max(0.0)) as usize;
    let n_hi = (((hi / FRAME).round_ties_even()) as usize).min(n);
    let step = (SNAP_S / FRAME) as usize;
    if env[i0] > q {
        let falling = i0 >= w + n_lo && i0 + w <= n && mean(&env[i0 - w..i0]) > mean(&env[i0..i0 + w]) + 1.0;
        let falling = falling
            || (i0 >= w + n_lo && i0 + w > n && {
                let tail = &env[i0..n];
                !tail.is_empty() && mean(&env[i0 - w..i0]) > mean(tail) + 1.0
            });
        if falling {
            let k = (i0..(i1 - 1).min(i0 + step)).find(|&k| env[k] <= q);
            if let Some(k) = k
                && (i1 - k) as f64 * FRAME >= 0.1
            {
                i0 = k;
            }
        } else {
            let stop = n_lo.max(i0.saturating_sub(step)) as i64 - 1;
            let mut k = i0 as i64;
            let mut found = None;
            while k > stop {
                if env[k as usize] <= q {
                    found = Some(k as usize);
                    break;
                }
                k -= 1;
            }
            if let Some(k) = found {
                i0 = k;
            }
        }
    }
    let last = i1 - 1;
    if env[last] > q_end {
        let rising = last + 1 + w <= n_hi && {
            let lo_i = i0.max(last.saturating_sub(w));
            mean(&env[last + 1..last + 1 + w]) > mean(&env[lo_i..last + 1]) + 1.0
        };
        if rising {
            let k = (i0 + 1..=last).rev().find(|&k| env[k] <= q);
            if let Some(k) = k
                && (k - i0) as f64 * FRAME >= 0.1
            {
                i1 = k + 1;
            }
        } else {
            let mut k = last;
            let mut low = env[last];
            while k + 1 < n_hi.min(last + step) && env[k] > q_end && env[k + 1] <= low + 6.0 {
                k += 1;
                low = low.min(env[k]);
            }
            i1 = k + 1;
        }
    }
    let loud: Vec<usize> = (i0..i1).filter(|&i| env[i] > q).collect();
    if let Some(&first) = loud.first() {
        i0 += (first - i0).saturating_sub(1);
        if let Some(d) = (i0..i1).rev().find(|&i| env[i] > q_end) {
            i1 = d + 1;
        }
    }
    let (a2, b2) = (i0 as f64 * FRAME, i1 as f64 * FRAME);
    (lo.max(a2), hi.min(b2 + if b2 < hi { 0.02 } else { 0.0 }))
}

/// True when [a, b) holds two or more separate sounds (a quiet stretch >= gap, or an edit gap, between them)
pub fn multi_sound(env: &[f64], floor: f64, a: f64, b: f64, gap: f64) -> bool {
    let i0 = (a / FRAME).round_ties_even() as usize;
    let i1 = (b / FRAME).round_ties_even() as usize;
    let s = seg_of(env, i0, i1);
    if s.len() < 3 {
        return false;
    }
    let q = quiet_level(env, floor, i0, i1);
    let loud: Vec<usize> = (0..s.len()).filter(|&i| s[i] > q).collect();
    if loud.len() < 2 {
        return false;
    }
    let (l0, l1) = (loud[0], loud[loud.len() - 1]);
    if edit_gaps(env, a + l0 as f64 * FRAME, a + (l1 + 1) as f64 * FRAME) {
        return true;
    }
    let n = (gap / FRAME).round_ties_even() as usize;
    let quiet: Vec<bool> = s[l0..=l1].iter().map(|v| *v <= q).collect();
    quiet.len() >= n && quiet.windows(n).any(|w| w.iter().all(|q| *q))
}

/// at most n candidates, round-robin over 2 s buckets, best priority first within each bucket
pub fn select(cands: &[Cand], n: usize) -> Vec<Cand> {
    let mut sorted: Vec<Cand> = cands.to_vec();
    sorted.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    let mut keys: Vec<i64> = Vec::new();
    let mut by: std::collections::HashMap<i64, std::collections::VecDeque<Cand>> = Default::default();
    for c in sorted {
        let k = (c.0 / BUCKET_S).floor() as i64;
        if !by.contains_key(&k) {
            keys.push(k);
        }
        by.entry(k).or_default().push_back(c);
    }
    keys.sort();
    let mut out = Vec::new();
    while out.len() < n && by.values().any(|v| !v.is_empty()) {
        for k in &keys {
            if out.len() >= n {
                break;
            }
            if let Some(c) = by.get_mut(k).and_then(|v| v.pop_front()) {
                out.push(c);
            }
        }
    }
    out
}

/// onset strokes and events of a mono 48 kHz array (search.event_candidates); n = None returns them all
pub fn event_candidates(x: &[f32], n: Option<usize>) -> Vec<Cand> {
    let dur = x.len() as f64 / SRF;
    let on = crate::features::onsets(x);
    if on.is_empty() {
        return vec![];
    }
    let env = env_db(x, FRAME);
    let env1 = env_db(x, 0.001);
    let floor = stats::percentile(&env, 10.0);
    let mut strokes: Vec<(f64, f64, f64)> = Vec::new();
    for (i, &o) in on.iter().enumerate() {
        let nxt = if i + 1 < on.len() { on[i + 1] } else { dur };
        let f0 = (o / FRAME) as usize;
        let f1 = (f0 + 1).max((nxt / FRAME) as usize);
        let seg = seg_of(&env, f0, f1);
        if seg.is_empty() {
            continue;
        }
        let head = &seg[..seg.len().min((0.06 / FRAME) as usize + 1)];
        let pk = f0 + stats::argmax(head);
        let lim = (env[pk] - DECAY_DB).max(floor + 4.0);
        let below = seg_of(&env, pk, f1).iter().position(|&v| v < lim);
        let mut end = match below {
            Some(b) => (pk + b) as f64 * FRAME + 0.02,
            None => nxt,
        };
        let mut st = (refine_start(&env1, o, floor) - PRE_ROLL).max(0.0);
        if let Some(last) = strokes.last() {
            st = st.max(last.1);
        }
        end = end.min(if i + 1 < on.len() { nxt - 0.002 } else { dur }).min(st + CAP_S);
        if end - st >= 0.02 {
            strokes.push((st, end, env[pk] - floor));
        }
    }
    let mut out: Vec<Cand> = strokes.iter().map(|&(s, e, p)| (s, e, "stroke", p)).collect();
    let joins: Vec<bool> =
        strokes.windows(2).map(|w| w[1].0 - w[0].1 < MERGE_GAP && !edit_gaps(&env, w[0].0, w[1].0)).collect();
    for (k, w) in strokes.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        if joins[k] && b.1 - a.0 <= CAP_S && a.2 >= b.2 - WEAK_DB {
            out.push((a.0, b.1, "event", a.2.max(b.2)));
        }
    }
    let mut i = 0;
    while i < strokes.len() {
        let mut j = i;
        while j + 1 < strokes.len() && joins[j] && strokes[j + 1].1 - strokes[i].0 <= CAP_S {
            j += 1;
        }
        let mut ch = &strokes[i..=j];
        let top = ch.iter().map(|s| s.2).fold(f64::NEG_INFINITY, f64::max);
        while !ch.is_empty() && ch[0].2 < top - WEAK_DB {
            ch = &ch[1..];
        }
        if ch.len() > 2 {
            out.push((ch[0].0, ch[ch.len() - 1].1, "event", top));
        }
        i = j + 1;
    }
    let res: Vec<Cand> = out
        .into_iter()
        .map(|(s, e, k, p)| {
            let si = fi(s);
            let pre = seg_of(&env, si.saturating_sub(6), si);
            let iso = (pre.is_empty() || mean(pre) <= floor + 10.0) && tail_db(&env, s, e) <= -25.0;
            (s, e, k, p + if iso { 10.0 } else { 0.0 })
        })
        .collect();
    match n {
        None => res,
        Some(n) => select(&res, n),
    }
}

/// numpy.arange(a, b, step) for floats
fn arange(a: f64, b: f64, step: f64) -> Vec<f64> {
    let n = ((b - a) / step).ceil().max(0.0) as usize;
    (0..n).map(|i| a + i as f64 * step).collect()
}

/// sliding windows covering [t0, dur) without gaps (search.window_candidates)
pub fn window_candidates(dur: f64, win: Option<f64>, hop: Option<f64>, t0: f64) -> Vec<Cand> {
    let span = dur - t0;
    let win = match win.filter(|w| *w > 0.0) {
        Some(w) => w,
        None => {
            let mut w = (span / 6.0).clamp(1.0, 5.0);
            while w < MAX_WIN_S && (span - w) / (w / 2.0) + 1.0 > MAX_WINDOWS as f64 {
                w = MAX_WIN_S.min(w * 1.25);
            }
            w
        }
    };
    if span <= win {
        return vec![(t0, dur, "window", 0.0)];
    }
    let hop = match hop.filter(|h| *h > 0.0) {
        Some(h) => h,
        None => {
            let mut h = win / 2.0;
            if (span - win) / h + 1.0 > MAX_WINDOWS as f64 {
                h = win.min((span - win) / (MAX_WINDOWS - 1) as f64);
            }
            h
        }
    };
    let mut st = arange(t0, dur - win + 1e-6, hop);
    if let Some(&l) = st.last()
        && dur - win - l > 1e-3
    {
        st.push(dur - win);
    }
    st.into_iter().map(|s| (s, s + win, "window", 0.0)).collect()
}

/// drop candidates overlapping an earlier one by 90% or more
pub fn dedupe(cands: &[Cand]) -> Vec<Cand> {
    let mut out: Vec<Cand> = Vec::new();
    for c in cands {
        if out.iter().all(|d| c.1.min(d.1) - c.0.max(d.0) < 0.9 * (c.1.max(d.1) - c.0.min(d.0))) {
            out.push(*c);
        }
    }
    out
}

fn kind_words(k: &str) -> &'static str {
    match k {
        "event" => "read as a one-shot query",
        "texture" => "read as a continuous-sound query (windows only)",
        _ => "read as either a one-shot or a continuous sound",
    }
}

fn weakly(v: &str) -> String {
    if v == "no match" { "not a match".into() } else { format!("only a {v} match") }
}

/// search options (find --kind / --win / --hop)
#[derive(Clone, Debug, Default)]
pub struct FindOptions {
    /// "auto" (default), "event" or "texture"
    pub kind: Option<String>,
    pub win: Option<f64>,
    pub hop: Option<f64>,
}

/// search.find on in-memory audio: `mono` is the model input (ffmpeg downmix), `stereo` (when given) the true-level
/// decode used for the peaks; scorer(spans in seconds) -> (z, cosine) per span
pub fn find_with(
    ears: &crate::Ears,
    mono: &[f32],
    stereo: Option<(&[f32], &[f32])>,
    query: &str,
    top: usize,
    opt: &FindOptions,
    name: &str,
) -> Result<FindResult> {
    anyhow::ensure!(!query.trim().is_empty(), "no query given");
    anyhow::ensure!(!mono.is_empty(), "empty audio: nothing to search");
    let kind = opt.kind.as_deref().unwrap_or("auto");
    let x = mono;
    let (off, dur) = (0.0, x.len() as f64 / SRF);
    let qe = ears.embed_text(&[query])?.remove(0);
    let (qkind, qscore) = ears.query_kind(query)?;
    let use_ev = kind == "event" || (kind == "auto" && qkind != "texture");
    let prefer_ev = kind == "event" || (kind == "auto" && qkind == "event");
    let env = env_db(x, FRAME);
    let floor = stats::percentile(&env, 10.0);
    let mut wins = window_candidates(dur, opt.win, opt.hop, off);
    let (n_win, win_s) = (wins.len(), wins[0].1 - wins[0].0);
    if prefer_ev {
        let snapped: Vec<Cand> = wins
            .iter()
            .map(|&(a, b, k, p)| {
                let (a2, b2) = snap_window(&env, floor, a, b, off, dur);
                (a2, b2, k, p)
            })
            .collect();
        wins = dedupe(&snapped);
    }
    let score = |cands: &[Cand]| -> Result<(Vec<f64>, Vec<f64>)> { ears.score_spans(x, cands, &qe) };
    let (mut zw, mut cw) = score(&wins)?;
    let mut evs: Vec<Cand> = Vec::new();
    let mut stage = "one stage".to_string();
    if use_ev {
        let allev: Vec<Cand> = event_candidates(x, None).into_iter().filter(|c| c.0 + PRE_ROLL >= off - 1e-3).collect();
        if dur <= SINGLE_STAGE_S {
            evs = select(&allev, MAX_EVENTS);
        } else {
            let order = stats::argsort_desc(&zw);
            let best: Vec<Cand> = order.iter().take(TOP_WINDOWS).map(|&i| wins[i]).collect();
            let inside: Vec<Cand> = allev.into_iter().filter(|c| best.iter().any(|w| c.0 < w.1 && c.1 > w.0)).collect();
            evs = select(&inside, MAX_EVENTS);
            stage = format!("two stages: events inside the {} best windows", best.len());
        }
    }
    let evs = dedupe(&evs);
    let (ze, ce) = score(&evs)?;
    if kind == "event" && !evs.is_empty() {
        wins.clear();
        zw.clear();
        cw.clear();
    }
    let cands: Vec<Cand> = evs.iter().chain(wins.iter()).copied().collect();
    let z: Vec<f64> = ze.iter().chain(zw.iter()).map(|v| round_to(*v, 2)).collect();
    let cos: Vec<f64> = ce.iter().chain(cw.iter()).copied().collect();
    let tails: Vec<f64> = cands.iter().map(|c| tail_db(&env, c.0, c.1)).collect();
    let heads: Vec<f64> = cands.iter().map(|c| head_db(&env, c.0, c.1)).collect();
    let judged: Vec<bool> = cands.iter().map(|c| use_ev && (prefer_ev || c.2 != "window")).collect();
    let cut: Vec<bool> = (0..cands.len()).map(|i| (tails[i] > CUT_DB || heads[i] > CUT_DB) && judged[i]).collect();
    let sc: Vec<f64> = (0..cands.len()).map(|i| z[i] - if cut[i] { TAIL_PENALTY } else { 0.0 }).collect();
    let mut rank_z = sc.clone();
    if prefer_ev {
        for (i, c) in cands.iter().enumerate() {
            if c.2 != "window" {
                continue;
            }
            let inside: Vec<f64> = cands
                .iter()
                .enumerate()
                .filter(|(_, d)| d.2 != "window" && d.0 >= c.0 - 0.05 && d.1 <= c.1 + 0.05)
                .map(|(j, _)| z[j])
                .collect();
            let refine = !inside.is_empty() && stats::max(&inside) >= z[i] - REFINE_DZ;
            if refine || (!evs.is_empty() && multi_sound(&env, floor, c.0, c.1, MERGE_GAP)) {
                rank_z[i] = f64::NEG_INFINITY;
            }
        }
    }
    let peak_of = |a: usize, b: usize| -> f64 {
        match stereo {
            Some((l, r)) => {
                let (a, b) = (a.min(l.len()), b.min(l.len()).max(a.min(l.len())));
                l[a..b].iter().chain(&r[a..b]).fold(0.0f32, |m, v| m.max(v.abs())) as f64
            }
            None => {
                let (a, b) = (a.min(x.len()), b.min(x.len()).max(a.min(x.len())));
                x[a..b].iter().fold(0.0f32, |m, v| m.max(v.abs())) as f64
            }
        }
    };
    let total = stereo.map_or(x.len(), |(l, _)| l.len());
    let loud_file = db(peak_of(0, total)) >= SILENT_DBFS;
    let mut picked: Vec<(f64, f64, Trim)> = Vec::new();
    for i in stats::argsort_desc(&rank_z) {
        if !rank_z[i].is_finite() {
            break;
        }
        let (a, b) = (cands[i].0, cands[i].1);
        if picked.iter().all(|(pa, pb, _)| b <= pa + 1e-3 || a >= pb - 1e-3) {
            let pk = peak_of((a * SRF) as usize, (b * SRF) as usize);
            if loud_file && db(pk) < SILENT_DBFS {
                continue;
            }
            let cut_txt: Vec<&str> = [("head cut", heads[i] > CUT_DB), ("tail cut", tails[i] > CUT_DB)]
                .iter()
                .filter(|(_, c)| *c && judged[i])
                .map(|(w, _)| *w)
                .collect();
            picked.push((
                a,
                b,
                Trim {
                    start: round_to(a, 3),
                    end: round_to(b, 3),
                    duration: round_to(b - a, 3),
                    score: round_to(sc[i], 2),
                    z: round_to(z[i], 2),
                    cosine: round_to(cos[i], 3),
                    verdict: verdict(z[i]).into(),
                    kind: cands[i].2.into(),
                    peak_dbfs: round_to(db(pk), 1),
                    head_db: round_to(heads[i], 1),
                    tail_db: round_to(tails[i], 1),
                    cut: cut_txt.join(", "),
                },
            ));
        }
        if picked.len() >= top {
            break;
        }
    }
    let segments: Vec<Trim> = picked.into_iter().map(|(_, _, t)| t).collect();
    let nw = if wins.is_empty() { n_win } else { wins.len() };
    let how = format!(
        "{}; searched all {:.1} s: {nw} windows of ~{win_s:.1} s{}{}{}",
        if kind == "auto" { kind_words(&qkind).to_string() } else { format!("kind={kind}") },
        dur,
        if prefer_ev { " (edges snapped to the sound)" } else { "" },
        if wins.is_empty() { " to locate events, none returned" } else { "" },
        if use_ev { format!(" + {} onset-based candidates ({stage})", evs.len()) } else { String::new() }
    );
    let mut s = if !segments.is_empty() {
        let segs: Vec<String> = segments
            .iter()
            .map(|p| {
                format!(
                    "{}-{} ({:.2} s {}, z {} {}{})",
                    crate::realness::fmt_t(p.start),
                    crate::realness::fmt_t(p.end),
                    p.duration,
                    p.kind,
                    crate::matching::zf(p.z),
                    p.verdict,
                    if p.cut.is_empty() { String::new() } else { format!(", {}", p.cut) }
                )
            })
            .collect();
        let mut s = format!(
            "Best moments for '{query}' in {name} ({how}): {}. start/end are sample trims in seconds (strokes / events start 5 ms before the attack{}; a 'head cut' / 'tail cut' trim cuts into a sound).",
            segs.join("; "),
            if prefer_ev {
                "; windows are snapped to the sound"
            } else {
                "; windows are plain stretches: crossfade them"
            }
        );
        if segments[0].verdict == "weak" || segments[0].verdict == "no match" {
            s += &format!(
                " Even the best segment is {}: the recording may not contain this sound.",
                weakly(&segments[0].verdict)
            );
        }
        s
    } else {
        format!("No candidate segments in {name} ({how}).")
    };
    let pk = db(peak_of(0, total));
    if pk < SILENT_DBFS {
        s = format!("WARNING: near-silent input (peak {pk:.0} dBFS), scores are meaningless. {s}");
    }
    Ok(FindResult {
        query: query.into(),
        query_kind: qkind,
        query_kind_score: qscore,
        duration: round_to(dur, 3),
        n_candidates: cands.len(),
        n_windows: wins.len(),
        n_events: evs.len(),
        segments,
        summary: s,
    })
}
