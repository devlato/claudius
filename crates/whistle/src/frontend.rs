//! The audio front end as the engine runs it ("Porting Whistle" §3), and the
//! silence gate's per-frame band energy (§8).
//!
//! 16 kHz mono f32 in [-1, 1]: bad samples to zero, the level gain, centred
//! 400-sample frames every 160 samples under a symmetric Hann window, a
//! 512-point real FFT's power, the mel filterbank the archive ships,
//! `ln(x + 2^-24)`, and per-clip per-channel normalisation with the n-1
//! standard deviation.

use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

use crate::layout::Manifest;

/// RMS block of the level gain, in samples.
pub const LEVEL_BLOCK: usize = 320;
/// The silence gate's band, in Hz.
pub const GATE_BAND: (f64, f64) = (250.0, 3500.0);

/// Log-mel features, `(frames, mels)`, and each frame's energy in the gate band.
pub struct Features {
    pub frames: usize,
    pub mels: usize,
    pub data: Vec<f32>,
    pub band_db: Vec<f32>,
}

pub struct FrontEnd {
    /// `(fft/2 + 1, mels)` row-major, as record 679 stores it.
    mel: Vec<f32>,
    bins: usize,
    mels: usize,
    win: usize,
    hop: usize,
    window: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    gate_bands: Vec<usize>,
}

/// The selection the engine uses for its percentiles: sort and index
/// `floor(q * (n - 1))`, no interpolation.
pub fn select(sorted: &[f32], q: f64) -> f32 {
    sorted[(q * (sorted.len() - 1) as f64).floor() as usize]
}

/// Non-finite samples and samples of magnitude 65536 or more read as zero;
/// then, unless `level` is off, the clip is scaled so its 99th-percentile
/// 320-sample block RMS is 0.1.
pub fn condition(pcm: &[f32], level: bool) -> Vec<f32> {
    let mut x: Vec<f32> = pcm.iter().map(|&v| if v.is_finite() && v.abs() < 65536.0 { v } else { 0.0 }).collect();
    let blocks = x.len() / LEVEL_BLOCK;
    if level && blocks > 0 {
        let mut rms: Vec<f64> = x
            .chunks_exact(LEVEL_BLOCK)
            .map(|b| (b.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / LEVEL_BLOCK as f64).sqrt())
            .collect();
        rms.sort_by(|a, b| a.total_cmp(b));
        let loud = rms[(0.99 * (blocks - 1) as f64).floor() as usize];
        if loud > 1e-6 {
            let gain = 0.1 / loud;
            for v in x.iter_mut() {
                *v = (*v as f64 * gain) as f32;
            }
        }
    }
    x
}

impl FrontEnd {
    pub fn new(mel: Vec<f32>, man: &Manifest) -> Self {
        let bins = man.fft / 2 + 1;
        assert_eq!(mel.len(), bins * man.mels, "mel filterbank shape");
        // np.hanning: symmetric, 0.5 - 0.5 cos(2πi / (N - 1)).
        let window = (0..man.win)
            .map(|i| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (man.win - 1) as f64).cos()) as f32)
            .collect();
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(man.fft);
        // Bands whose filter support lies inside the gate band.
        let hz = man.sample_rate as f64 / man.fft as f64;
        let gate_bands = (0..man.mels)
            .filter(|&m| {
                let nz: Vec<usize> = (0..bins).filter(|&k| mel[k * man.mels + m] > 0.0).collect();
                match (nz.first(), nz.last()) {
                    (Some(&lo), Some(&hi)) => lo as f64 * hz >= GATE_BAND.0 && hi as f64 * hz <= GATE_BAND.1,
                    _ => false,
                }
            })
            .collect();
        Self { mel, bins, mels: man.mels, win: man.win, hop: man.hop, window, fft, gate_bands }
    }

    pub fn gate_bands(&self) -> &[usize] {
        &self.gate_bands
    }

    /// Features for an already conditioned clip.
    pub fn features(&self, x: &[f32]) -> Features {
        let frames = x.len() / self.hop;
        let mels = self.mels;
        let mut data = vec![0.0f32; frames * mels];
        let mut band_db = vec![0.0f32; frames];
        let mut input = self.fft.make_input_vec();
        let mut spectrum = self.fft.make_output_vec();
        let mut power = vec![0.0f32; self.bins];
        let lead = self.win / 2;
        for t in 0..frames {
            input.fill(0.0);
            // Frame t reads samples hop*t - win/2 .. hop*t + win/2 - 1, zero outside.
            for (i, slot) in input[..self.win].iter_mut().enumerate() {
                let s = (self.hop * t + i) as isize - lead as isize;
                if s >= 0 && (s as usize) < x.len() {
                    *slot = x[s as usize] * self.window[i];
                }
            }
            self.fft.process(&mut input, &mut spectrum).expect("realfft buffer sizes are fixed at construction");
            for (p, c) in power.iter_mut().zip(&spectrum) {
                *p = c.re * c.re + c.im * c.im;
            }
            let row = &mut data[t * mels..(t + 1) * mels];
            for (k, &pk) in power.iter().enumerate() {
                if pk == 0.0 {
                    continue;
                }
                let filt = &self.mel[k * mels..(k + 1) * mels];
                for (r, &fv) in row.iter_mut().zip(filt) {
                    *r += pk * fv;
                }
            }
            // The floor is the features' own 2^-24; the engine's is not documented.
            let band: f64 = self.gate_bands.iter().map(|&m| row[m] as f64).sum();
            band_db[t] = (10.0 * (band + 2f64.powi(-24)).log10()) as f32;
            for r in row.iter_mut() {
                *r = (*r + 2.0f32.powi(-24)).ln();
            }
        }
        // Per-channel normalisation over the clip, n - 1 in the variance.
        if frames > 0 {
            let denom = frames.saturating_sub(1).max(1) as f64;
            for m in 0..mels {
                let mean = (0..frames).map(|t| data[t * mels + m] as f64).sum::<f64>() / frames as f64;
                let var = (0..frames)
                    .map(|t| {
                        let dv = data[t * mels + m] as f64 - mean;
                        dv * dv
                    })
                    .sum::<f64>()
                    / denom;
                let std = var.sqrt() as f32;
                let mean = mean as f32;
                for t in 0..frames {
                    let v = &mut data[t * mels + m];
                    *v = (*v - mean) / (std + 1e-5);
                }
            }
        }
        Features { frames, mels, data, band_db }
    }
}

/// The silence gate's measure: how far the 99th-percentile frame of gate-band
/// energy lies above the 10th, in dB. The clip is silence below 5 dB.
pub fn gate_spread(band_db: &[f32]) -> f32 {
    if band_db.is_empty() {
        return 0.0;
    }
    let mut s = band_db.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    select(&s, 0.99) - select(&s, 0.10)
}

pub const GATE_DB: f32 = 5.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_gain_targets_a_tenth_at_the_loud_block() {
        let mut pcm = vec![0.0f32; 3200];
        for (i, v) in pcm.iter_mut().enumerate() {
            *v = if i % 2 == 0 { 0.5 } else { -0.5 };
        }
        let x = condition(&pcm, true);
        assert!((x[0] - 0.1).abs() < 1e-6, "{}", x[0]);
    }

    #[test]
    fn bad_samples_read_as_zero() {
        let x = condition(&[f32::NAN, f32::INFINITY, 70000.0, 0.25], false);
        assert_eq!(x, [0.0, 0.0, 0.0, 0.25]);
    }
}
