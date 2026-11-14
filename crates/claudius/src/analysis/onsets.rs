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

/// onset sample positions, refined to where a 1 ms RMS envelope starts rising in each flux peak's window
pub fn detect(mono: &[f32], delta: f32, min_gap_s: f64) -> Vec<usize> {
    let env = flux(mono, None);
    let frames = pick(&env, delta, ((min_gap_s * SRF) as usize / HOP).max(1));
    refine(mono, &frames, HOP, NFFT)
}

/// SuperFlux's frames: 200 a second, of 2048 samples
pub const SF_HOP: usize = (crate::config::SR / 200) as usize;
const SF_NFFT: usize = 2048;

/// What each SuperFlux frame of `mono` adds, band by band (Böck & Widmer, DAFx 2013): a frame each `SF_HOP` samples,
/// how much each band of a filterbank of 24 bands an octave (30 Hz to 17 kHz, `superflux_centres`) rises in log
/// magnitude over the same band two frames before, taking there the loudest of it and the bands beside it, so that a
/// vibrato's wandering from band to band is not heard as onsets. A window of silence comes before `mono`, so that a
/// hit on its first sample rises as any other does; frame `t` ends where sample `t * SF_HOP` of `mono` does.
pub fn superflux_rises(mono: &[f32]) -> Vec<Vec<f32>> {
    let bands = log_bands(SF_NFFT);
    let padded: Vec<f32> = std::iter::repeat_n(0.0, SF_NFFT).chain(mono.iter().copied()).collect();
    let filtered: Vec<Vec<f32>> = spectrogram(&padded, SF_NFFT, SF_HOP)
        .iter()
        .map(|frame| {
            bands.iter().map(|(_, b)| (1.0 + b.iter().map(|&(k, w)| frame[k] * w).sum::<f32>()).log10()).collect()
        })
        .collect();
    let lag = 2;
    let mut out = vec![vec![0.0f32; bands.len()]; filtered.len()];
    for t in lag..filtered.len() {
        let (now, before) = (&filtered[t], &filtered[t - lag]);
        let n = before.len();
        for k in 0..n {
            let most = before[k.saturating_sub(1)].max(before[k]).max(before[(k + 1).min(n - 1)]);
            out[t][k] = (now[k] - most).max(0.0);
        }
    }
    out
}

/// The SuperFlux onset strength of `mono`: how much each frame rises, over all its bands (`superflux_rises`).
pub fn superflux(mono: &[f32]) -> Vec<f32> {
    superflux_rises(mono).iter().map(|r| r.iter().sum()).collect()
}

/// the centre of each of SuperFlux's bands, in hertz, in the order `superflux_rises` gives them
pub fn superflux_centres() -> Vec<f64> {
    log_bands(SF_NFFT).into_iter().map(|(hz, _)| hz).collect()
}

/// triangular filters 24 to the octave from 30 Hz to 17 kHz over the bins of an `n_fft` spectrum, each its centre in
/// hertz, its bins and their weights, summing to 1; the low ones, narrower than a bin, merged into the first that is not
fn log_bands(n_fft: usize) -> Vec<(f64, Vec<(usize, f32)>)> {
    let bin = |hz: f64| hz / SRF * n_fft as f64;
    let mut centres: Vec<f64> =
        (0..).map(|k| 30.0 * 2f64.powf(k as f64 / 24.0)).take_while(|f| *f <= 17000.0).collect();
    centres.retain(|f| bin(*f) < (n_fft / 2) as f64);
    let mut edges: Vec<usize> = centres.iter().map(|f| bin(*f).round() as usize).collect();
    edges.dedup();
    edges
        .windows(3)
        .map(|w| {
            let (lo, mid, hi) = (w[0], w[1], w[2]);
            let up = (lo..mid).map(|k| (k, (k - lo) as f32 / (mid - lo).max(1) as f32));
            let down = (mid..=hi).map(|k| (k, (hi - k) as f32 / (hi - mid).max(1) as f32));
            let tri: Vec<(usize, f32)> = up.chain(down).filter(|(_, w)| *w > 0.0).collect();
            let sum: f32 = tri.iter().map(|t| t.1).sum::<f32>().max(1e-9);
            (mid as f64 * SRF / n_fft as f64, tri.into_iter().map(|(k, w)| (k, w / sum)).collect())
        })
        .collect()
}

/// An onset SuperFlux hears (`superflux_hits`): where its attack starts, in samples, and what it adds, band by band
/// (`superflux_rises` at its peak), which is the hit itself and nothing of what still rings from before it.
pub struct Onset {
    pub at: usize,
    pub rise: Vec<f32>,
}

/// The onsets SuperFlux hears in `mono`: the peaks of `superflux` that are the most of the 10 ms before and the 50 ms
/// after them, over the mean of the 150 ms before by 1.1, and 30 ms after the onset before them (the settings Böck &
/// Widmer give it), each placed where its attack rises steepest in its peak's frame, a millisecond before that.
pub fn superflux_hits(mono: &[f32]) -> Vec<Onset> {
    let rises = superflux_rises(mono);
    let sf: Vec<f32> = rises.iter().map(|r| r.iter().sum()).collect();
    let frames = |s: f64| (s * 200.0).round() as usize;
    let (pre_max, post_max, pre_avg, combine) = (frames(0.01), frames(0.05), frames(0.15), frames(0.03));
    let ms = (0.001 * SRF) as usize;
    let rms = |a: usize| -> f32 {
        let b = (a + ms).min(mono.len());
        if b <= a {
            return 0.0;
        }
        (mono[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32).sqrt()
    };
    let mut out: Vec<Onset> = Vec::new();
    let mut last: Option<usize> = None;
    for t in 0..sf.len() {
        let near = &sf[t.saturating_sub(pre_max)..(t + post_max + 1).min(sf.len())];
        let before = &sf[t.saturating_sub(pre_avg)..t.max(1).min(sf.len())];
        let mean = before.iter().sum::<f32>() / before.len().max(1) as f32;
        let most = near.iter().copied().fold(0.0f32, f32::max);
        if !(sf[t] > 0.0 && sf[t] >= most && sf[t] > mean + 1.1 && last.is_none_or(|p| t >= p + combine)) {
            continue;
        }
        last = Some(t);
        // the frame's window, in `mono`, after the onset before it
        let after = out.last().map_or(0, |o| o.at + 2 * ms);
        let (from, to) = (((t * SF_HOP).saturating_sub(SF_NFFT)).max(after), (t * SF_HOP).min(mono.len()));
        let steepest = (from + ms..to)
            .step_by((ms / 2).max(1))
            .max_by(|&a, &b| (rms(a) - rms(a - ms)).total_cmp(&(rms(b) - rms(b - ms))));
        if let Some(s) = steepest {
            out.push(Onset { at: (s - ms).saturating_sub(ms).max(after), rise: rises[t].clone() });
        }
    }
    out
}

/// The onsets SuperFlux hears in `mono`, as sample positions where each attack starts (`superflux_hits`).
pub fn superflux_onsets(mono: &[f32]) -> Vec<usize> {
    superflux_hits(mono).into_iter().map(|o| o.at).collect()
}

/// onset `frames` of `hop` samples in windows of `n_fft`, as sample positions: where a 1 ms RMS envelope starts rising
/// in each frame's window
fn refine(mono: &[f32], frames: &[usize], hop: usize, n_fft: usize) -> Vec<usize> {
    let win = (0.001 * SRF) as usize;
    let rms = |a: usize| -> f32 {
        let b = (a + win).min(mono.len());
        if b <= a {
            return 0.0;
        }
        (mono[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32).sqrt()
    };
    let mut out = Vec::with_capacity(frames.len());
    let mut after = 0usize;
    for &f in frames {
        // the attack is in the peak frame's window, anywhere in it (log flux peaks where it first comes in, at the
        // window's end): look from a hop before the window to a hop after it, never back before the onset found last
        let start = (f * hop).saturating_sub(hop).max(after);
        let end = (f * hop + n_fft + hop).min(mono.len());
        if start >= end {
            continue;
        }
        let peak = (start..end).step_by(win.max(1)).map(rms).fold(0.0f32, f32::max);
        let mut pos = start;
        for s in (start..end).step_by(win.max(1)) {
            if rms(s) > peak * 0.1 {
                pos = s;
                break;
            }
        }
        let pos = pos.saturating_sub(win).max(after);
        out.push(pos);
        after = pos + 2 * win;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superflux_hears_each_hit_once_where_it_starts() {
        let at = [0.25, 0.5, 0.75, 1.125, 1.5, 1.625];
        let mut x = vec![0.0f32; 2 * SRF as usize];
        let mut rng = crate::rng::Rng::new(9);
        for a in at {
            let from = (a * SRF) as usize;
            for i in 0..(0.08 * SRF) as usize {
                x[from + i] += (rng.f32() - 0.5) * (-(i as f32) / (0.02 * SRF as f32)).exp();
            }
        }
        let found: Vec<f64> = superflux_onsets(&x).iter().map(|&s| s as f64 / SRF).collect();
        assert_eq!(found.len(), at.len(), "{found:?}");
        assert!(found.iter().zip(at).all(|(f, a)| (f - a).abs() < 0.01), "{found:?}");
    }

    #[test]
    fn a_tone_wavering_in_vibrato_is_one_onset_not_many() {
        // A4 from 0.2 s, its pitch wavering six times a second a semitone either way
        let (tau, mut phase) = (std::f64::consts::TAU, 0.0);
        let x: Vec<f32> = (0..2 * SRF as usize)
            .map(|i| {
                let t = i as f64 / SRF;
                phase += tau * 440.0 * 2f64.powf((tau * 6.0 * t).sin() / 12.0) / SRF;
                if t < 0.2 { 0.0 } else { (phase.sin() * 0.4) as f32 }
            })
            .collect();
        let found: Vec<f64> = superflux_onsets(&x).iter().map(|&s| s as f64 / SRF).collect();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!((found[0] - 0.2).abs() < 0.01, "{found:?}");
    }
}
