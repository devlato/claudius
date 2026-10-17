//! Melody writing: motif-based leads (a one-bar idea, repeated, re-fitted to each chord, answered by a cadence),
//! arpeggios, and chord parts (sustained pads, stabs, offbeat pumps, syncopated chops) voiced with voice leading.

use super::chord::{Voicing, voice};
use super::scale::Key;
use super::{ChordSpan, Note, chord_at};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug)]
struct Cell {
    t: f64,
    d: f64,
    /// scale steps relative to the phrase's anchor
    step: i32,
    v: f32,
}

fn rhythm(density: f32, syncopated: bool, rng: &mut Rng) -> Vec<(f64, f64)> {
    // onsets on the 1/16 grid within one bar, weighted towards strong (or syncopated) positions
    let n = (2.0 + density.clamp(0.0, 1.0) * 6.0).round() as usize + rng.below(2);
    let weights: Vec<f64> = (0..16)
        .map(|s| {
            let base: f64 = match s % 4 {
                0 => 1.0,
                2 => 0.8,
                _ => 0.35,
            };
            if syncopated && matches!(s, 3 | 6 | 10 | 11 | 14) { base + 0.7 } else { base }
        })
        .collect();
    let mut picked = vec![0usize];
    while picked.len() < n.min(12) {
        let mut w = weights.clone();
        for &p in &picked {
            w[p] = 0.0;
        }
        picked.push(rng.weighted(&w));
    }
    picked.sort();
    picked.dedup();
    picked
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let next = picked.get(i + 1).copied().unwrap_or(16);
            let gap = (next - s) as f64 * 0.25;
            (s as f64 * 0.25, (gap * if rng.chance(0.3) { 0.5 } else { 0.9 }).max(0.2))
        })
        .collect()
}

fn contour(n: usize, rng: &mut Rng) -> Vec<i32> {
    let shape = rng.below(4);
    let mut cur = 0i32;
    let mut out = Vec::with_capacity(n);
    let mut last_leap: i32 = 0;
    for i in 0..n {
        out.push(cur);
        let f = i as f64 / n.max(1) as f64;
        let bias = match shape {
            0 => {
                if f < 0.5 {
                    1
                } else {
                    -1
                }
            }
            1 => -1,
            2 => 1,
            _ => {
                if i % 2 == 0 {
                    1
                } else {
                    -1
                }
            }
        };
        let mv = if last_leap != 0 {
            let r = -last_leap.signum();
            last_leap = 0;
            r
        } else {
            match rng.below(10) {
                0..=5 => bias,
                6 => -bias,
                7 => 0,
                _ => {
                    last_leap = bias * (2 + rng.below(2) as i32);
                    last_leap
                }
            }
        };
        cur = (cur + mv).clamp(-5, 7);
    }
    out
}

fn fit(p: i32, t: f64, d: f64, chords: &[ChordSpan], key: &Key) -> i32 {
    let strong = (t.fract() < 1e-6 && (t as i64 % 2 == 0)) || d >= 0.99;
    let p = key.snap(p);
    if !strong {
        return p;
    }
    let Some(span) = chord_at(chords, t) else { return p };
    let pcs = span.chord.pcs();
    (0..7).flat_map(|d| [p - d, p + d]).find(|q| pcs.contains(&(q.rem_euclid(12) as u8))).unwrap_or(p)
}

/// a lead over `bars` bars starting at `start`: A A' A B phrasing per 4 bars, register (low, high)
pub fn lead(
    chords: &[ChordSpan],
    key: &Key,
    start: f64,
    bars: u32,
    density: f32,
    syncopated: bool,
    register: (i32, i32),
    rng: &mut Rng,
) -> Vec<Note> {
    let rh = rhythm(density, syncopated, rng);
    let steps = contour(rh.len(), rng);
    let motif: Vec<Cell> = rh
        .iter()
        .zip(&steps)
        .map(|(&(t, d), &s)| Cell { t, d, step: s, v: if t.fract() < 1e-6 { 0.95 } else { 0.8 } })
        .collect();
    let rh_b = rhythm((density * 0.7).max(0.2), syncopated, rng);
    let cad_steps = contour(rh_b.len(), rng);
    let anchor = key.snap((register.0 + register.1) / 2);
    let anchor_idx = key.index_of(anchor).unwrap_or(0);
    let mut out = Vec::new();
    for bar in 0..bars {
        let b0 = start + bar as f64 * 4.0;
        let pos = bar % 4;
        // A' re-anchors the motif a third or step away, B is the answering cadence
        let (cells, shift): (Vec<Cell>, i32) = match pos {
            1 => (motif.clone(), if bar % 8 == 5 { -1 } else { 2 }),
            3 => {
                let mut c: Vec<Cell> =
                    rh_b.iter().zip(&cad_steps).map(|(&(t, d), &s)| Cell { t, d, step: s, v: 0.85 }).collect();
                if let Some(last) = c.last_mut() {
                    last.d = (4.0 - last.t).max(0.5);
                    last.step = if bar % 8 == 7 { 0 } else { 2 };
                }
                (c, 0)
            }
            _ => (motif.clone(), 0),
        };
        for c in cells {
            let t = b0 + c.t;
            let raw = key.from_index(anchor_idx + c.step + shift);
            let mut p = fit(raw, t, c.d, chords, key);
            while p > register.1 {
                p -= 12;
            }
            while p < register.0 {
                p += 12;
            }
            out.push(Note { t, d: c.d, p, v: c.v });
        }
    }
    out
}

/// arpeggio over the chords: `pattern` up | down | updown | random | pinky, one note every `rate` beats
pub fn arp(
    chords: &[ChordSpan],
    start: f64,
    len: f64,
    pattern: &str,
    rate: f64,
    octaves: u32,
    register: (i32, i32),
    gate: f64,
    rng: &mut Rng,
) -> Vec<Note> {
    let mut out = Vec::new();
    let mut t = start;
    let mut i = 0usize;
    while t < start + len - 1e-9 {
        let Some(span) = chord_at(chords, t) else { break };
        let base = voice(&span.chord, None, register.0, register.0 + 14, 3, Voicing::Close, None);
        let mut pool: Vec<i32> = (0..octaves.max(1) as i32)
            .flat_map(|o| base.iter().map(move |p| p + 12 * o))
            .filter(|p| *p <= register.1)
            .collect();
        if pool.is_empty() {
            pool = base.clone();
        }
        let n = pool.len();
        let idx = match pattern {
            "down" => n - 1 - i % n,
            "updown" => {
                let cyc = (2 * n).saturating_sub(2).max(1);
                let k = i % cyc;
                if k < n { k } else { cyc - k }
            }
            "random" => rng.below(n),
            "pinky" => {
                if i.is_multiple_of(2) {
                    n - 1
                } else {
                    (i / 2) % n.saturating_sub(1).max(1)
                }
            }
            _ => i % n,
        };
        out.push(Note { t, d: rate * gate, p: pool[idx], v: if (t - start).fract() < 1e-6 { 0.95 } else { 0.78 } });
        t += rate;
        i += 1;
    }
    out
}

pub const CHORD_RHYTHMS: &[&str] = &["sustain", "half", "stabs", "offbeat", "pump", "syncopated", "charleston"];

/// a chord part: voiced chords in a rhythm, voice-led from chord to chord
/// the rhythms `chord_part` plays chords in
pub const CHORD_STYLES: &[&str] = &["sustain", "half", "stabs", "offbeat", "pump", "syncopated", "charleston"];
/// the orders `arp` plays chord tones in
pub const ARP_STYLES: &[&str] = &["up", "down", "updown", "random", "pinky"];
/// leads have one style: motifs developed over the chords
pub const LEAD_STYLES: &[&str] = &["motif"];

pub fn chord_part(
    chords: &[ChordSpan],
    rhythm_name: &str,
    voicing: Voicing,
    register: (i32, i32),
    voices: usize,
    rng: &mut Rng,
) -> Vec<Note> {
    let mut out = Vec::new();
    let mut prev: Option<Vec<i32>> = None;
    let hits: Vec<(f64, f64, f32)> = match rhythm_name {
        "half" => vec![(0.0, 1.9, 0.9), (2.0, 1.9, 0.85)],
        "stabs" => vec![(0.0, 0.3, 0.95), (1.5, 0.3, 0.8), (3.0, 0.3, 0.85)],
        "offbeat" => (0..4).map(|b| (b as f64 + 0.5, 0.35, 0.85)).collect(),
        "pump" => (0..8).map(|k| (k as f64 * 0.5, 0.42, if k % 2 == 0 { 0.9 } else { 0.75 })).collect(),
        "syncopated" => vec![(0.0, 0.4, 0.95), (0.75, 0.4, 0.8), (1.5, 0.4, 0.85), (2.5, 0.4, 0.8), (3.25, 0.5, 0.85)],
        "charleston" => vec![(0.0, 0.6, 0.95), (1.5, 1.2, 0.85)],
        _ => vec![(0.0, 4.0, 0.85)],
    };
    for span in chords {
        let pitches = voice(&span.chord, prev.as_deref(), register.0, register.1, voices, voicing, Some(rng));
        let bars = (span.len / 4.0).ceil().max(1.0) as usize;
        for b in 0..bars {
            for &(ht, hd, hv) in &hits {
                let t = span.start + b as f64 * 4.0 + ht;
                if t >= span.start + span.len - 1e-6 {
                    continue;
                }
                let d = if rhythm_name == "sustain" { span.len } else { hd.min(span.start + span.len - t) };
                for &p in &pitches {
                    out.push(Note { t, d, p, v: hv });
                }
            }
            if rhythm_name == "sustain" {
                break;
            }
        }
        prev = Some(pitches);
    }
    out
}
