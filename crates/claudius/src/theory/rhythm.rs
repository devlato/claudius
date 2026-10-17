//! Rhythm: drum hits, per-genre lane templates (step probabilities on the 1/16 grid), euclidean rhythms, trap
//! rolls, fills, swing, and the syncopation / density measures the critic uses.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;

/// one drum hit: position in beats within its pattern, velocity 0..1
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub t: f64,
    pub v: f32,
}

/// a lane's habits: probability of a hit on each 1/16 step (16 = one bar, 32 = two bars); values >= 1 always hit.
/// `vel` is the velocity range, `ghost` the chance of soft extra hits on empty steps, `rolls` the chance per beat of a
/// trap roll (1/32 or triplet bursts), `vary` how often the last bar of a phrase deviates
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaneTemplate {
    pub steps: Vec<f32>,
    #[serde(default = "default_vel")]
    pub vel: (f32, f32),
    #[serde(default)]
    pub ghost: f32,
    #[serde(default)]
    pub rolls: f32,
    #[serde(default)]
    pub vary: f32,
}

fn default_vel() -> (f32, f32) {
    (0.7, 1.0)
}

/// metrical weight of a 1/16 step within a bar (downbeat strongest)
pub fn step_weight(step: usize) -> f32 {
    match step % 16 {
        0 => 1.0,
        8 => 0.92,
        4 | 12 => 0.85,
        2 | 6 | 10 | 14 => 0.72,
        _ => 0.6,
    }
}

/// one bar (or the template's length) of hits sampled from the template
fn sample_bar(t: &LaneTemplate, rng: &mut Rng, vary: bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (s, &p) in t.steps.iter().enumerate() {
        let p = if vary && p < 1.0 { (p * 1.6).min(0.95) * if rng.chance(0.5) { 1.0 } else { 0.4 } } else { p };
        let hit = p >= 1.0 || rng.chance(p as f64);
        if hit {
            let v = t.vel.0 + (t.vel.1 - t.vel.0) * (0.35 + 0.65 * step_weight(s)) * (0.85 + 0.15 * rng.f32());
            hits.push(Hit { t: s as f64 * 0.25, v: v.clamp(0.05, 1.0) });
        } else if t.ghost > 0.0 && rng.chance(t.ghost as f64) {
            hits.push(Hit { t: s as f64 * 0.25, v: (t.vel.0 * 0.45).clamp(0.05, 1.0) });
        }
    }
    hits
}

/// a looping pattern of `bars` bars: the template's main bar repeated, the phrase's last bar sometimes varied,
/// rolls added on top
pub fn generate(t: &LaneTemplate, bars: u32, rng: &mut Rng) -> Vec<Hit> {
    let bar_steps = 16usize;
    let tmpl_bars = (t.steps.len().max(1) as f64 / bar_steps as f64).ceil().max(1.0) as u32;
    let main = sample_bar(t, rng, false);
    let mut out = Vec::new();
    let mut b = 0;
    while b < bars {
        let last = b + tmpl_bars >= bars;
        let part =
            if last && t.vary > 0.0 && rng.chance(t.vary as f64) { sample_bar(t, rng, true) } else { main.clone() };
        for h in &part {
            out.push(Hit { t: h.t + b as f64 * 4.0, v: h.v });
        }
        b += tmpl_bars;
    }
    if t.rolls > 0.0 {
        out = add_rolls(&out, bars as f64 * 4.0, t.rolls, rng);
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    out.dedup_by(|a, b| (a.t - b.t).abs() < 1e-6);
    out
}

/// trap hi-hat rolls: some beats are replaced by bursts of 1/32 notes or 1/16 triplets with a velocity ramp
pub fn add_rolls(hits: &[Hit], len_beats: f64, prob: f32, rng: &mut Rng) -> Vec<Hit> {
    let mut out: Vec<Hit> = Vec::new();
    let beats = len_beats as usize;
    let mut rolled = vec![false; beats];
    for (b, r) in rolled.iter_mut().enumerate() {
        // rolls mostly on the second half of a bar, never on the downbeat
        let pos = b % 4;
        let p = prob as f64
            * if pos == 0 {
                0.1
            } else if pos == 3 {
                1.6
            } else {
                0.8
            };
        *r = rng.chance(p.min(0.9));
    }
    for h in hits {
        if !rolled[(h.t as usize).min(beats.saturating_sub(1))] {
            out.push(*h);
        }
    }
    for (b, r) in rolled.iter().enumerate() {
        if !*r {
            continue;
        }
        let (div, len) = match rng.below(4) {
            0 => (8.0, 1.0),
            1 => (6.0, 1.0),
            2 => (8.0, 0.5),
            _ => (12.0, 1.0),
        };
        let start = b as f64 + if len < 1.0 { 0.5 } else { 0.0 };
        let n = (div * len) as usize;
        let up = rng.chance(0.6);
        for k in 0..n {
            let f = k as f32 / n.max(1) as f32;
            let v = if up { 0.45 + 0.5 * f } else { 0.95 - 0.45 * f };
            out.push(Hit { t: start + k as f64 / div, v });
        }
    }
    out
}

/// Bjorklund: k onsets spread as evenly as possible over n steps, rotated by `rot`
pub fn euclid(k: usize, n: usize, rot: usize) -> Vec<bool> {
    if n == 0 {
        return Vec::new();
    }
    let mut v = bjorklund(k.min(n), n);
    v.rotate_right(rot % n);
    v
}

/// Bjorklund's algorithm, as Tidal and Strudel run it: (3,8) is x..x..x., (5,8) is x.xx.xx.
fn bjorklund(k: usize, n: usize) -> Vec<bool> {
    let mut a: Vec<Vec<bool>> = vec![vec![true]; k];
    let mut b: Vec<Vec<bool>> = vec![vec![false]; n - k];
    while a.len().min(b.len()) > 1 {
        // pair as many of the first group with the second as there are of the smaller; the leftovers of the
        // larger become the new second group
        let pairs = a.len().min(b.len());
        let rest = if a.len() > b.len() { a.split_off(pairs) } else { b.split_off(pairs) };
        for (x, y) in a.iter_mut().zip(b.drain(..)) {
            x.extend(y);
        }
        b = rest;
    }
    a.into_iter().chain(b).flatten().collect()
}

/// a euclidean pattern spread over one bar per cycle, repeated for `bars`
pub fn euclid_hits(k: usize, n: usize, rot: usize, bars: u32, vel: f32) -> Vec<Hit> {
    let pat = euclid(k, n, rot);
    let step = 4.0 / n.max(1) as f64;
    let mut out = Vec::new();
    for b in 0..bars {
        for (i, on) in pat.iter().enumerate() {
            if *on {
                out.push(Hit { t: b as f64 * 4.0 + i as f64 * step, v: vel * (0.75 + 0.25 * step_weight(i)) });
            }
        }
    }
    out
}

/// snare build-up: quarters, eighths, sixteenths, then thirty-seconds, rising in velocity
pub fn snare_roll(len_beats: f64) -> Vec<Hit> {
    let mut out = Vec::new();
    let quarters = (len_beats * 0.5).floor();
    let mut t = 0.0;
    let phases = [(1.0, quarters), (0.5, len_beats * 0.25), (0.25, len_beats * 0.125), (0.125, len_beats * 0.125)];
    for (step, span) in phases {
        let end = t + span;
        while t < end - 1e-9 && t < len_beats {
            let v = 0.4 + 0.6 * (t / len_beats) as f32;
            out.push(Hit { t, v });
            t += step;
        }
    }
    out
}

/// a fill for the last `len_beats` of a phrase
pub fn fill(kind: &str, len_beats: f64, rng: &mut Rng) -> Vec<Hit> {
    match kind {
        "roll" => {
            let n = (len_beats * 4.0) as usize;
            (0..n).map(|i| Hit { t: i as f64 * 0.25, v: 0.5 + 0.5 * i as f32 / n as f32 }).collect()
        }
        "triplets" => {
            let n = (len_beats * 3.0) as usize;
            (0..n).map(|i| Hit { t: i as f64 / 3.0, v: 0.6 + 0.4 * i as f32 / n as f32 }).collect()
        }
        _ => {
            let mut out = Vec::new();
            let n = (len_beats * 4.0) as usize;
            for i in 0..n {
                if i % 2 == 1 && rng.chance(0.5) || rng.chance(0.55) {
                    out.push(Hit { t: i as f64 * 0.25, v: 0.55 + 0.45 * rng.f32() });
                }
            }
            out
        }
    }
}

/// swung position of a beat time: off-beat 1/16ths move later (swing 0.5 = straight, 0.66 = triplet feel)
pub fn swing(t: f64, amount: f32) -> f64 {
    if amount <= 0.501 {
        return t;
    }
    let pair = (t / 0.5).floor() * 0.5;
    let within = t - pair;
    if (within - 0.25).abs() < 0.01 { pair + 0.5 * amount as f64 } else { t }
}

/// Longuet-Higgins & Lee style syncopation per bar: onsets on weak positions followed by silence on stronger ones
pub fn syncopation(hits: &[Hit], len_beats: f64) -> f64 {
    let steps = (len_beats * 4.0).round() as usize;
    if steps == 0 {
        return 0.0;
    }
    let mut on = vec![false; steps];
    for h in hits {
        let s = (h.t * 4.0).round() as usize;
        if s < steps {
            on[s] = true;
        }
    }
    let lvl = |s: usize| -> i32 {
        match s % 16 {
            0 => 0,
            8 => -1,
            4 | 12 => -2,
            2 | 6 | 10 | 14 => -3,
            _ => -4,
        }
    };
    let mut total = 0.0;
    for s in 0..steps {
        if !on[s] {
            continue;
        }
        let mut n = s + 1;
        while n < steps && !on[n] && lvl(n) <= lvl(s) {
            n += 1;
        }
        if n < steps && !on[n] && lvl(n) > lvl(s) {
            total += (lvl(n) - lvl(s)) as f64;
        }
    }
    total / (len_beats / 4.0).max(1.0)
}

pub fn density(hits: &[Hit], len_beats: f64) -> f64 {
    hits.len() as f64 / len_beats.max(1.0)
}
