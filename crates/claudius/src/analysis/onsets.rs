//! Onset detection: log-compressed spectral flux with adaptive peak picking, refined to the sample where the
//! energy starts rising. Used to slice drum loops into one-shots and to transcribe drum patterns.

use crate::config::SRF;
use crate::dsp::fft::spectrogram;

pub const HOP: usize = 256;
pub const NFFT: usize = 1024;

/// onset strength per hop (spectral flux of log magnitudes), optionally restricted to a frequency band (Hz)
pub fn flux(mono: &[f32], band: Option<(f64, f64)>) -> Vec<f32> {
    let spec = spectrogram(mono, NFFT, HOP);
    let bin = |f: f64| ((f / SRF * NFFT as f64).round() as usize).min(NFFT / 2);
    let (lo, hi) = band.map_or((1, NFFT / 2), |(a, b)| (bin(a).max(1), bin(b).max(bin(a) + 1)));
    let mut out = vec![0.0f32; spec.len()];
    for t in 1..spec.len() {
        let mut s = 0.0f32;
        for k in lo..hi {
            let d = (1.0 + 100.0 * spec[t][k]).ln() - (1.0 + 100.0 * spec[t - 1][k]).ln();
            if d > 0.0 {
                s += d;
            }
        }
        out[t] = s;
    }
    out
}

/// frame indices of onset peaks: local maxima above a moving median + delta * max
pub fn pick(env: &[f32], delta: f32, min_gap_frames: usize) -> Vec<usize> {
    let n = env.len();
    if n < 3 {
        return Vec::new();
    }
    let mx = env.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    let w = ((0.25 * SRF / HOP as f64) as usize).max(4);
    let mut out: Vec<usize> = Vec::new();
    for t in 1..n - 1 {
        let a = t.saturating_sub(w);
        let b = (t + w).min(n);
        let mut win: Vec<f32> = env[a..b].to_vec();
        win.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let med = win[win.len() / 2];
        let local_max = (t.saturating_sub(3)..(t + 4).min(n)).all(|k| env[k] <= env[t]);
        if local_max && env[t] > med + delta * mx {
            if let Some(&last) = out.last()
                && t - last < min_gap_frames
            {
                if env[t] > env[last] {
                    out.pop();
                } else {
                    continue;
                }
            }
            out.push(t);
        }
    }
    out
}

/// onset sample positions, refined to where a 1 ms RMS envelope starts rising before each flux peak
pub fn detect(mono: &[f32], delta: f32, min_gap_s: f64) -> Vec<usize> {
    let env = flux(mono, None);
    let frames = pick(&env, delta, ((min_gap_s * SRF) as usize / HOP).max(1));
    let win = (0.001 * SRF) as usize;
    let rms = |a: usize| -> f32 {
        let b = (a + win).min(mono.len());
        if b <= a {
            return 0.0;
        }
        (mono[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32).sqrt()
    };
    frames
        .iter()
        .map(|&f| {
            // the flux frame lags the attack by up to one window: search back for the start of the rise
            let centre = f * HOP + NFFT / 2;
            let start = centre.saturating_sub(NFFT + HOP);
            let end = (centre + HOP).min(mono.len());
            let peak = (start..end).step_by(win.max(1)).map(rms).fold(0.0f32, f32::max);
            let mut pos = start;
            for s in (start..end).step_by(win.max(1)) {
                if rms(s) > peak * 0.1 {
                    pos = s;
                    break;
                }
            }
            pos.saturating_sub(win)
        })
        .collect()
}
