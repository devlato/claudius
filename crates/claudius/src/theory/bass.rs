//! Basslines: a one-bar groove motif (rhythm + intervals over the root) written once per song and transposed onto
//! every chord, in the styles electronic genres use (sustained, pulse, offbeat, rolling, syncopated, 808 following the
//! kick, acid, walking, reese).

use super::scale::Key;
use super::{ChordSpan, Note};
use crate::rng::Rng;

/// the pitch of class `pc` nearest to `center`
pub fn nearest(pc: u8, center: i32) -> i32 {
    let d = (pc as i32 - center).rem_euclid(12);
    center + if d > 6 { d - 12 } else { d }
}

/// interval markers resolved against the sounding chord (its third / fifth) rather than fixed semitones
const THIRD: i32 = 100;
const FIFTH: i32 = 101;
const WALK_A: i32 = 99;
const WALK_B: i32 = 98;
const APPROACH: i32 = 97;

#[derive(Clone, Copy, Debug)]
struct Step {
    t: f64,
    d: f64,
    /// semitones over the chord's bass note
    iv: i32,
    v: f32,
    slide: bool,
}

pub const STYLES: &[&str] = &[
    "sustain",
    "pulse",
    "root8",
    "offbeat",
    "octave",
    "rolling",
    "syncopated",
    "kick_follow",
    "acid",
    "walking",
    "reese",
];

fn motif(style: &str, kicks_in_bar: &[f64], minor: bool, rng: &mut Rng) -> Vec<Step> {
    let s = |t: f64, d: f64, iv: i32, v: f32| Step { t, d, iv, v, slide: false };
    match style {
        "sustain" | "reese" => vec![s(0.0, 4.0, 0, 0.9)],
        "pulse" => (0..4).map(|b| s(b as f64, 0.5, 0, if b == 0 { 1.0 } else { 0.8 })).collect(),
        "root8" => (0..8)
            .map(|k| {
                s(
                    k as f64 * 0.5,
                    0.42,
                    if k == 7 && rng.chance(0.5) { 12 } else { 0 },
                    if k % 2 == 0 { 0.95 } else { 0.75 },
                )
            })
            .collect(),
        "octave" => (0..8)
            .map(|k| s(k as f64 * 0.5, 0.4, if k % 2 == 1 { 12 } else { 0 }, if k % 2 == 0 { 0.95 } else { 0.8 }))
            .collect(),
        "offbeat" => {
            let mut v: Vec<Step> = (0..4).map(|b| s(b as f64 + 0.5, 0.42, 0, 0.95)).collect();
            if rng.chance(0.4) {
                v.push(s(3.75, 0.2, if rng.chance(0.5) { 12 } else { 7 }, 0.6));
            }
            v
        }
        "rolling" => (0..4)
            .flat_map(|b| (1..4).map(move |k| (b, k)))
            .map(|(b, k)| s(b as f64 + k as f64 * 0.25, 0.22, 0, if k == 1 { 0.95 } else { 0.7 }))
            .collect(),
        "syncopated" => {
            let grooves: [&[usize]; 5] = [
                &[0, 3, 6, 10, 11, 14],
                &[0, 6, 7, 10, 14],
                &[0, 3, 8, 11, 14],
                &[2, 6, 10, 13],
                &[0, 4, 7, 10, 12, 15],
            ];
            let g = grooves[rng.below(grooves.len())];
            g.iter()
                .enumerate()
                .map(|(i, &st)| {
                    let next = g.get(i + 1).copied().unwrap_or(16);
                    let iv = match rng.below(6) {
                        0 => 12,
                        1 => FIFTH,
                        2 if i > 0 => THIRD,
                        _ => 0,
                    };
                    s(
                        st as f64 * 0.25,
                        ((next - st) as f64 * 0.25 * 0.7).min(0.75),
                        if i == 0 { 0 } else { iv },
                        if st % 4 == 0 { 1.0 } else { 0.78 },
                    )
                })
                .collect()
        }
        "kick_follow" => {
            let mut ks: Vec<f64> = kicks_in_bar.to_vec();
            if ks.is_empty() {
                ks = vec![0.0, 2.5];
            }
            ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
            ks.iter()
                .enumerate()
                .map(|(i, &t)| {
                    let next = ks.get(i + 1).copied().unwrap_or(4.0);
                    let iv = if i > 0 && rng.chance(0.25) { [12, 7, -5, 10][rng.below(4)] } else { 0 };
                    Step { t, d: (next - t).max(0.25), iv, v: 1.0, slide: i > 0 && rng.chance(0.3) }
                })
                .collect()
        }
        "acid" => {
            let pent = if minor { [0, 3, 5, 7, 10, 12] } else { [0, 2, 4, 7, 9, 12] };
            (0..16)
                .filter_map(|k| {
                    if k > 0 && rng.chance(0.3) {
                        return None;
                    }
                    let iv = if k == 0 || rng.chance(0.45) { 0 } else { pent[rng.below(pent.len())] }
                        + if rng.chance(0.15) { 12 } else { 0 };
                    Some(Step {
                        t: k as f64 * 0.25,
                        d: 0.2,
                        iv,
                        v: if rng.chance(0.3) { 1.0 } else { 0.62 },
                        slide: rng.chance(0.2),
                    })
                })
                .collect()
        }
        "walking" => {
            vec![s(0.0, 0.9, 0, 0.95), s(1.0, 0.9, WALK_A, 0.8), s(2.0, 0.9, WALK_B, 0.85), s(3.0, 0.9, APPROACH, 0.8)]
        }
        _ => vec![s(0.0, 4.0, 0, 0.9)],
    }
}

/// write a bassline over `chords`; `kicks` = absolute kick beats (for kick-locked styles); `center` = register
pub fn bassline(style: &str, chords: &[ChordSpan], key: &Key, kicks: &[f64], center: i32, rng: &mut Rng) -> Vec<Note> {
    let minor = key.mode.is_minor();
    let bar_kicks: Vec<f64> = kicks.iter().filter(|k| **k < 4.0).copied().collect();
    let m = motif(style, &bar_kicks, minor, rng);
    let mut out: Vec<Note> = Vec::new();
    let mut prev = center;
    for (ci, span) in chords.iter().enumerate() {
        let root = {
            let r = nearest(span.chord.bass_pc(), prev);
            if (r - center).abs() > 7 { nearest(span.chord.bass_pc(), center) } else { r }
        };
        let next_root = chords.get(ci + 1).map(|c| nearest(c.chord.bass_pc(), root));
        let bars = (span.len / 4.0).ceil().max(1.0) as usize;
        for b in 0..bars {
            for (k, st) in m.iter().enumerate() {
                let t = span.start + b as f64 * 4.0 + st.t;
                if t >= span.start + span.len - 1e-6 {
                    continue;
                }
                let mut p = match st.iv {
                    THIRD => nearest(span.chord.third_pc(), root + 4),
                    FIFTH => nearest(span.chord.fifth_pc(), root + 7),
                    WALK_A | WALK_B => {
                        let tones = span.chord.pcs();
                        nearest(tones[k % tones.len()], root + if st.iv == WALK_A { 4 } else { 7 })
                    }
                    APPROACH => next_root.map_or(root + 7, |n| n + if rng.chance(0.5) { -1 } else { 1 }),
                    iv => root + iv,
                };
                if style == "acid" {
                    p = key.snap(p);
                }
                let mut d = st.d.min(span.start + span.len - t);
                if style == "sustain" || style == "reese" {
                    d = (span.start + span.len - t).min(4.0 * bars as f64);
                }
                if st.slide {
                    d += 0.06;
                }
                out.push(Note { t, d, p, v: st.v });
            }
            if style == "sustain" || style == "reese" {
                break;
            }
        }
        prev = root;
    }
    // kick-locked 808s also hit on every kick, not just the first bar's pattern
    if style == "kick_follow" && !kicks.is_empty() {
        out.clear();
        let mut ks = kicks.to_vec();
        ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (i, &t) in ks.iter().enumerate() {
            let Some(span) = super::chord_at(chords, t) else { continue };
            let root = nearest(span.chord.bass_pc(), center);
            let next = ks.get(i + 1).copied().unwrap_or(span.start + span.len);
            let iv = if i % 4 == 3 && rng.chance(0.35) { [12, 7, 10][rng.below(3)] } else { 0 };
            let slide = iv != 0 && rng.chance(0.5);
            out.push(Note { t, d: (next - t).clamp(0.25, 4.0) + if slide { 0.06 } else { 0.0 }, p: root + iv, v: 1.0 });
        }
    }
    out
}
