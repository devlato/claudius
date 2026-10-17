//! Envelopes: analog-style ADSR, percussive decays, and automation curves (piecewise in beats).

use serde::{Deserialize, Serialize};

const K: f64 = 4.6; // exp(-4.6) ~ 1%: a segment counts as done when it is 99% of the way there

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Adsr {
    pub a: f64,
    pub d: f64,
    pub s: f64,
    pub r: f64,
}

impl Adsr {
    pub const fn new(a: f64, d: f64, s: f64, r: f64) -> Self {
        Adsr { a, d, s, r }
    }

    /// level `i` samples after note-on for a gate of `gate` samples (release appended after the gate)
    #[inline]
    pub fn level(&self, i: usize, gate: usize, sr: f64) -> f64 {
        let held = |t: f64| {
            let an = (self.a * sr).max(1.0);
            if t < an {
                (1.0 - (-K * t / an).exp()) / (1.0 - (-K).exp())
            } else {
                let dn = (self.d * sr).max(1.0);
                self.s + (1.0 - self.s) * (-K * (t - an) / dn).exp()
            }
        };
        if i < gate {
            held(i as f64)
        } else {
            let at_release = held(gate.max(1) as f64 - 1.0);
            let rn = (self.r * sr).max(1.0);
            at_release * (-6.9 * (i - gate) as f64 / rn).exp()
        }
    }

    pub fn release_samples(&self, sr: f64) -> usize {
        (self.r * sr).ceil() as usize + 1
    }

    /// whole envelope for a gate of `gate` samples, release tail included, ending exactly at zero
    pub fn render(&self, gate: usize, sr: f64) -> Vec<f32> {
        let n = gate + self.release_samples(sr);
        let mut e: Vec<f32> = (0..n).map(|i| self.level(i, gate, sr) as f32).collect();
        let k = n.min(96);
        for j in 0..k {
            e[n - 1 - j] *= j as f32 / k as f32;
        }
        e
    }
}

/// percussive envelope: linear attack, hold, exponential decay to -60 dB at t60; curve > 1 falls faster at first
pub fn perc(n: usize, sr: f64, attack: f64, hold: f64, t60: f64, curve: f64) -> Vec<f32> {
    let an = attack.max(1e-5);
    let mut e: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            let a = if t < an { t / an } else { 1.0 };
            let tt = (t - an - hold).max(0.0);
            (a * (-6.9 * (tt / t60.max(1e-4)).powf(curve)).exp()) as f32
        })
        .collect();
    let k = n.min(64);
    for j in 0..k {
        e[n - 1 - j] *= j as f32 / k as f32;
    }
    e
}

/// frequency multiplier starting `semis` semitones up and falling exponentially to 1 (time constant `time` s)
#[inline]
pub fn pitch_drop(t: f64, semis: f64, time: f64) -> f64 {
    2f64.powf(semis * (-t / time.max(1e-4)).exp() / 12.0)
}

/// a constant or an automation curve through (beat, value) points
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Param {
    Const(f32),
    Curve(Vec<(f64, f32)>),
}

impl Default for Param {
    fn default() -> Self {
        Param::Const(0.0)
    }
}

impl From<f32> for Param {
    fn from(v: f32) -> Self {
        Param::Const(v)
    }
}

impl Param {
    pub fn is_const(&self) -> bool {
        matches!(self, Param::Const(_))
    }

    pub fn first(&self) -> f32 {
        match self {
            Param::Const(v) => *v,
            Param::Curve(p) => p.first().map_or(0.0, |x| x.1),
        }
    }

    /// value at `beat`; log=true interpolates geometrically (cutoff frequencies)
    pub fn at(&self, beat: f64, log: bool) -> f32 {
        match self {
            Param::Const(v) => *v,
            Param::Curve(pts) => {
                if pts.is_empty() {
                    return 0.0;
                }
                if beat <= pts[0].0 {
                    return pts[0].1;
                }
                for w in pts.windows(2) {
                    let (b0, v0) = w[0];
                    let (b1, v1) = w[1];
                    if beat <= b1 {
                        let f = if b1 > b0 { ((beat - b0) / (b1 - b0)) as f32 } else { 1.0 };
                        return if log && v0 > 0.0 && v1 > 0.0 {
                            (v0.ln() + (v1.ln() - v0.ln()) * f).exp()
                        } else {
                            v0 + (v1 - v0) * f
                        };
                    }
                }
                pts[pts.len() - 1].1
            }
        }
    }

    /// per-sample values for n samples starting at sample 0 = beat 0
    pub fn render(&self, n: usize, samples_per_beat: f64, log: bool) -> Vec<f32> {
        match self {
            Param::Const(v) => vec![*v; n],
            Param::Curve(_) => (0..n).map(|i| self.at(i as f64 / samples_per_beat, log)).collect(),
        }
    }
}
