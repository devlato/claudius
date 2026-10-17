//! Tempo and beats: autocorrelation of the onset envelope (with a soft prior around 120 BPM, or a range the caller
//! knows), dynamic-programming beat tracking (Ellis 2007), and the downbeat as the beat phase where kicks and
//! harmony changes land.

use super::chroma;
use super::onsets::{self, HOP};
use crate::config::SRF;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Grid {
    pub bpm: f64,
    /// beat times (s)
    pub beats: Vec<f64>,
    /// index into `beats` of the first downbeat
    pub downbeat: usize,
    /// 0..1: how clearly the music pulses at this tempo
    pub clarity: f64,
}

fn autocorr(env: &[f32], max_lag: usize) -> Vec<f64> {
    let n = env.len();
    let m = env.iter().map(|v| *v as f64).sum::<f64>() / n.max(1) as f64;
    let x: Vec<f64> = env.iter().map(|v| *v as f64 - m).collect();
    let r0: f64 = x.iter().map(|v| v * v).sum::<f64>().max(1e-12);
    (0..max_lag.min(n)).map(|l| x[..n - l].iter().zip(&x[l..]).map(|(a, b)| a * b).sum::<f64>() / r0).collect()
}

fn standardise(v: &[f32]) -> Vec<f32> {
    let mean = v.iter().map(|x| *x as f64).sum::<f64>() / v.len().max(1) as f64;
    let sd = (v.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt().max(1e-9);
    v.iter().map(|x| (*x as f64 / sd) as f32).collect()
}

pub fn track(mono: &[f32], range: Option<(f64, f64)>) -> Grid {
    // in beat-driven music the kick marks the beat: the low band counts twice as much as the full-band flux
    let full = standardise(&onsets::flux(mono, None));
    let kick = standardise(&onsets::flux(mono, Some((30.0, 160.0))));
    let env: Vec<f32> = full.iter().zip(&kick).map(|(a, b)| a + 2.0 * b).collect();
    let fps = SRF / HOP as f64;
    let (lo, hi) = range.unwrap_or((70.0, 180.0));
    let max_lag = (fps * 60.0 / lo * 2.2) as usize + 2;
    let ac = autocorr(&env, max_lag);
    let mut best = (0.0f64, fps * 0.5);
    let centre = range.map_or(120.0, |(a, b)| (a * b).sqrt());
    let lag_lo = (fps * 60.0 / hi).floor().max(2.0) as usize;
    let lag_hi = ((fps * 60.0 / lo).ceil() as usize).min(ac.len().saturating_sub(1) / 2);
    for l in lag_lo..=lag_hi.max(lag_lo) {
        let bpm = 60.0 * fps / l as f64;
        let prior = (-0.5 * ((bpm / centre).log2() / 0.9).powi(2)).exp();
        let s = (ac[l] + 0.5 * ac.get(2 * l).copied().unwrap_or(0.0)) * prior;
        if s > best.0 {
            best = (s, l as f64);
        }
    }
    let period = best.1;
    let clarity = ac.get(period as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    // dynamic programming: each beat rewards onset strength and penalises deviating from the period
    let n = env.len();
    let mut score: Vec<f64> = env.iter().map(|v| *v as f64).collect();
    let mut back = vec![usize::MAX; n];
    let p = period;
    for t in 0..n {
        let a = (t as f64 - 2.0 * p).max(0.0) as usize;
        let b = (t as f64 - p / 2.0).max(0.0) as usize;
        let mut bs = f64::MIN;
        let mut bi = usize::MAX;
        for tau in a..b.min(t) {
            let d = ((t - tau) as f64 / p).ln();
            let s = score[tau] - 100.0 * d * d;
            if s > bs {
                bs = s;
                bi = tau;
            }
        }
        if bi != usize::MAX && bs > 0.0 {
            score[t] += bs;
            back[t] = bi;
        }
    }
    let tail = (n as f64 - p).max(0.0) as usize;
    let mut t = (tail..n).max_by(|a, b| score[*a].partial_cmp(&score[*b]).unwrap()).unwrap_or(0);
    let mut frames = Vec::new();
    while t != usize::MAX {
        frames.push(t);
        t = back[t];
    }
    frames.reverse();
    let tracked: Vec<f64> = frames.iter().map(|f| (*f * HOP + HOP / 2) as f64 / SRF).collect();
    let bpm0 = if tracked.len() > 8 {
        let mut ibi: Vec<f64> = tracked.windows(2).map(|w| w[1] - w[0]).collect();
        ibi.sort_by(|a, b| a.partial_cmp(b).unwrap());
        60.0 / ibi[ibi.len() / 2]
    } else {
        60.0 * fps / p
    };
    // music made in a DAW keeps one tempo: fit the constant grid (tempo within 0.5 %, every phase) that lands on
    // the most onset energy; the dynamic-programming beats drift and insert beats through breakdowns
    let sample = |pos: f64| -> f64 {
        let i = pos.floor() as usize;
        if i + 1 >= env.len() {
            return 0.0;
        }
        let f = pos - i as f64;
        env[i] as f64 * (1.0 - f) + env[i + 1] as f64 * f
    };
    let mut best_grid = (f64::MIN, bpm0, 0.0f64);
    let mut b = bpm0 * 0.995;
    while b <= bpm0 * 1.005 {
        let per = fps * 60.0 / b;
        let mut off = 0.0;
        while off < per {
            let mut s = 0.0;
            let mut pos = off;
            while pos < env.len() as f64 - 1.0 {
                s += sample(pos);
                pos += per;
            }
            if s > best_grid.0 {
                best_grid = (s, b, off);
            }
            off += 1.0;
        }
        b += 0.005;
    }
    let (_, bpm, off) = best_grid;
    let per_s = 60.0 / bpm;
    let first = (off * HOP as f64 + HOP as f64 / 2.0) / SRF;
    let beats: Vec<f64> =
        (0..).map(|k| first + k as f64 * per_s).take_while(|t| *t < mono.len() as f64 / SRF).collect();
    // downbeat: the phase where the harmony changes (mean chroma over the half-beat before vs after the beat,
    // bass included) and the low end hits hardest
    let low = onsets::flux(mono, Some((30.0, 150.0)));
    let low_sd = (low.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / low.len().max(1) as f64).sqrt().max(1e-9);
    let chop = HOP * 4;
    let ch = chroma::chroma_band(mono, chop, 110.0, 2000.0);
    let beat_s = 60.0 / bpm.max(1.0);
    let mean_chroma = |a: f64, b: f64| -> [f64; 12] {
        let (fa, fb) = (((a * SRF) / chop as f64).max(0.0) as usize, ((b * SRF) / chop as f64) as usize);
        let mut m = [0.0f64; 12];
        for f in ch.iter().take(fb.min(ch.len())).skip(fa) {
            for i in 0..12 {
                m[i] += f[i] as f64;
            }
        }
        m
    };
    let cos_dist = |a: &[f64; 12], c: &[f64; 12]| {
        let d = a.iter().zip(c).map(|(x, y)| x * y).sum::<f64>();
        let n = a.iter().map(|x| x * x).sum::<f64>().sqrt() * c.iter().map(|x| x * x).sum::<f64>().sqrt();
        if n > 1e-12 { 1.0 - d / n } else { 0.0 }
    };
    let rms_at = |a: f64, b: f64| -> f64 {
        let (sa, sb) = (((a * SRF) as usize).min(mono.len()), ((b * SRF) as usize).min(mono.len()));
        if sb <= sa {
            return -120.0;
        }
        10.0 * (mono[sa..sb].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (sb - sa) as f64).max(1e-14).log10()
    };
    // three cues per beat phase, each scaled to its best phase: kick strength, harmony change (a bar of chroma
    // before vs after), and votes from the biggest level jumps (sections start on downbeats)
    let mut kick_p = [0.0f64; 4];
    let mut change_p = [0.0f64; 4];
    let mut jumps: Vec<(f64, usize)> = Vec::new();
    for (i, &b) in beats.iter().enumerate() {
        kick_p[i % 4] += low.get((b * SRF / HOP as f64) as usize).copied().unwrap_or(0.0) as f64 / low_sd;
        change_p[i % 4] += cos_dist(
            &mean_chroma(b - beat_s * 4.0, b - beat_s * 0.05),
            &mean_chroma(b + beat_s * 0.05, b + beat_s * 4.0),
        );
        jumps.push(((rms_at(b, b + 4.0 * beat_s) - rms_at(b - 4.0 * beat_s, b)).abs(), i % 4));
    }
    jumps.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut jump_p = [0.0f64; 4];
    for (j, ph) in jumps.iter().take(12) {
        jump_p[*ph] += j;
    }
    let norm = |v: [f64; 4]| -> [f64; 4] {
        let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        if hi - lo < 1e-9 { [0.0; 4] } else { v.map(|x| (x - lo) / (hi - lo)) }
    };
    let (kn, cn, jn) = (norm(kick_p), norm(change_p), norm(jump_p));
    // harmony change leads: level jumps also come from pre-drop gaps and risers, the kick from rolling basses
    let phase_score: [f64; 4] = std::array::from_fn(|i| 0.5 * kn[i] + 1.5 * cn[i] + 0.75 * jn[i]);
    if std::env::var_os("CLAUDIUS_DEBUG").is_some() {
        eprintln!(
            "beats: first {:.3} s, {} beats; per phase: kick {kn:.2?} harmony change {cn:.2?} jumps {jn:.2?} total {phase_score:.2?}",
            beats.first().copied().unwrap_or(0.0),
            beats.len()
        );
    }
    let downbeat = (0..4).max_by(|a, b| phase_score[*a].partial_cmp(&phase_score[*b]).unwrap()).unwrap_or(0);
    Grid { bpm: (bpm * 10.0).round() / 10.0, beats, downbeat, clarity }
}
