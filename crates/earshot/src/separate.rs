//! Stems: a recording apart, as its vocals, its drums, its bass and the rest (`other`), by Deezer's Spleeter (4 stems).
//! Its weights are MIT, as the paper releasing them says (Hennequin et al., JOSS 5(50):2154, 2020), trained on Deezer's
//! own licensed catalogue; the ONNX export (`REPO`, Apache-2.0) is pinned at a revision and each file by its SHA-256.
//!
//! Spleeter works on the magnitudes of the spectrum: for each stem a U-Net estimates the stem's magnitudes from the
//! recording's, up to 16 kHz (1536 of the 2049 bins of a 4096-point frame at 44.1 kHz). The four estimates share every
//! bin by their squares, a ratio mask that sums to one, so that the stems add up to the recording; above 16 kHz each
//! frame's mean mask carries on; and the masks go on the recording's own spectrum, its phase untouched, which an
//! overlap-add of the frames under the same periodic Hann window turns back into sound.

use std::path::Path;

use anyhow::{Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;
use realfft::RealFftPlanner;
use realfft::num_complex::Complex;

use crate::dsp::resample::resample;
use crate::models::{self, ox};

pub const REPO: &str = "Best-Practice/spleeter-4stems-onnx";
pub const REV: &str = "486643021f18a4d057d282fff1d11b89a30e9887";
/// the stems, in the order `separate` gives them
pub const STEMS: [&str; 4] = ["vocals", "drums", "bass", "other"];
/// each stem's net: the export that models 16 kHz rather than 11
pub const FILES: [&str; 4] = ["vocals.wide.onnx", "drums.wide.onnx", "bass.wide.onnx", "other.wide.onnx"];

/// what the nets were trained on: 44.1 kHz, frames of 4096 a hop of 1024 apart under a periodic Hann window, 512
/// frames to a split, and the first 1536 bins
pub const SR: u32 = 44_100;
const N_FFT: usize = 4096;
const HOP: usize = 1024;
const T: usize = 512;
const F: usize = 1536;
const BINS: usize = N_FFT / 2 + 1;
/// splits through the nets at once: about 24 seconds
const BATCH: usize = 2;

/// a stereo recording, left and right
pub type Stereo = (Vec<f32>, Vec<f32>);

/// What estimates the stems' magnitudes: for `splits` splits of the recording's (`[2, splits, 512, 1536]`, channel
/// first), each stem's of the same shape, in the order of `STEMS`.
pub trait Nets {
    fn estimate(&mut self, magnitudes: &[f32], splits: usize) -> Result<[Vec<f32>; 4]>;
}

/// Spleeter's four nets
pub struct Separator {
    nets: Vec<Session>,
}

impl Separator {
    /// the four nets, from `cache` (downloaded into it first, unless `offline`), on `threads` threads
    pub fn load(cache: &Path, offline: bool, threads: usize) -> Result<Separator> {
        let mut nets = Vec::with_capacity(FILES.len());
        for f in FILES {
            let path = models::fetch(cache, REPO, REV, f, offline)?;
            nets.push(models::session(&path, threads)?);
        }
        Ok(Separator { nets })
    }

    /// whether every net is in `cache`
    pub fn cached(cache: &Path) -> bool {
        FILES.iter().all(|f| models::model_path(cache, REPO, REV, f).is_file())
    }

    /// `x` at `sr` Hz as its four stems (`STEMS`), each as long as it at `sr`; `progress` hears the share done
    pub fn separate(&mut self, x: &Stereo, sr: u32, progress: &mut dyn FnMut(f64)) -> Result<[Stereo; 4]> {
        separate_with(self, x, sr, progress)
    }
}

impl Nets for Separator {
    fn estimate(&mut self, magnitudes: &[f32], splits: usize) -> Result<[Vec<f32>; 4]> {
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(4);
        for net in &mut self.nets {
            let t = ox(Tensor::from_array(([2, splits, T, F], magnitudes.to_vec())))?;
            let res = ox(net.run(ort::inputs!["x" => t]))?;
            let (_, y) = ox(res["y"].try_extract_tensor::<f32>())?;
            if y.len() != magnitudes.len() {
                return Err(anyhow!("a stem's net gave {} values for {}", y.len(), magnitudes.len()));
            }
            out.push(y.to_vec());
        }
        out.try_into().map_err(|_| anyhow!("four nets"))
    }
}

/// a periodic Hann window, as the nets were trained under (`np.hanning(n + 1)[:-1]`)
fn window() -> Vec<f32> {
    (0..N_FFT).map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N_FFT as f64).cos()) as f32).collect()
}

/// `x` at `sr` Hz as the four stems `nets` estimate, each as long as it at `sr` (`Separator::separate`)
pub fn separate_with(nets: &mut dyn Nets, x: &Stereo, sr: u32, progress: &mut dyn FnMut(f64)) -> Result<[Stereo; 4]> {
    let len = x.0.len();
    let input = [resample(&x.0, sr, SR), resample(&x.1, sr, SR)];
    let n = input[0].len();
    // the frames start a frame less a hop before the recording, so that its first samples are in as many frames as
    // the rest and the overlap-add has the window's whole weight there
    let pad = N_FFT - HOP;
    let frames = (pad + n).div_ceil(HOP).max(1);
    let total = (frames - 1) * HOP + N_FFT;
    let padded: Vec<Vec<f32>> = input
        .iter()
        .map(|c| {
            let mut p = vec![0.0f32; total];
            p[pad..pad + c.len()].copy_from_slice(c);
            p
        })
        .collect();
    let w = window();
    let mut planner = RealFftPlanner::<f32>::new();
    let (fwd, inv) = (planner.plan_fft_forward(N_FFT), planner.plan_fft_inverse(N_FFT));
    let (mut buf, mut sp) = (fwd.make_input_vec(), fwd.make_output_vec());
    let mut out = vec![vec![vec![0.0f32; total]; 2]; 4];
    let mut weight = vec![0.0f32; total];
    let splits = frames.div_ceil(T);
    for b0 in (0..splits).step_by(BATCH) {
        let b1 = (b0 + BATCH).min(splits);
        let s = b1 - b0;
        let (f0, f1) = (b0 * T, (b1 * T).min(frames));
        // these frames' spectra, both channels
        let mut spec = vec![vec![Complex::<f32>::default(); (f1 - f0) * BINS]; 2];
        for (c, ch) in padded.iter().enumerate() {
            for f in f0..f1 {
                let a = f * HOP;
                for (i, v) in buf.iter_mut().enumerate() {
                    *v = ch[a + i] * w[i];
                }
                fwd.process(&mut buf, &mut sp).map_err(|e| anyhow!("{e}"))?;
                spec[c][(f - f0) * BINS..(f - f0 + 1) * BINS].copy_from_slice(&sp);
            }
        }
        // the nets hear the magnitudes of the first bins, silence past the last frame
        let at = |c: usize, f: usize| ((c * s + (f - f0) / T) * T + (f - f0) % T) * F;
        let mut mag = vec![0.0f32; 2 * s * T * F];
        for (c, sc) in spec.iter().enumerate() {
            for f in f0..f1 {
                let (m, z) = (at(c, f), (f - f0) * BINS);
                for j in 0..F {
                    mag[m + j] = sc[z + j].norm();
                }
            }
        }
        let est = nets.estimate(&mag, s)?;
        let mut mask = vec![[0.0f32; 4]; F];
        for (c, sc) in spec.iter().enumerate() {
            for f in f0..f1 {
                let (m, z) = (at(c, f), (f - f0) * BINS);
                // each stem's share of a bin, by the squares of the four estimates, which add up to one
                let mut mean = [0.0f32; 4];
                for (j, mj) in mask.iter_mut().enumerate() {
                    let sq: [f32; 4] = std::array::from_fn(|k| est[k][m + j] * est[k][m + j]);
                    let all = sq.iter().sum::<f32>() + 1e-10;
                    for k in 0..4 {
                        mj[k] = (sq[k] + 1e-10 / 4.0) / all;
                        mean[k] += mj[k] / F as f32;
                    }
                }
                let a = f * HOP;
                for (k, stem) in out.iter_mut().enumerate() {
                    for (j, v) in sp.iter_mut().enumerate() {
                        *v = sc[z + j] * if j < F { mask[j][k] } else { mean[k] };
                    }
                    sp[0].im = 0.0;
                    sp[BINS - 1].im = 0.0;
                    inv.process(&mut sp, &mut buf).map_err(|e| anyhow!("{e}"))?;
                    for (i, v) in buf.iter().enumerate() {
                        stem[c][a + i] += v / N_FFT as f32 * w[i];
                    }
                }
                if c == 0 {
                    for (i, wi) in w.iter().enumerate() {
                        weight[a + i] += wi * wi;
                    }
                }
            }
        }
        progress(b1 as f64 / splits as f64);
    }
    // each sample over the weight the windows gave it, without the pad, and back at `sr`, as long as `x`
    let back = |c: &[f32]| -> Vec<f32> {
        let even: Vec<f32> = (pad..pad + n).map(|i| if weight[i] > 1e-8 { c[i] / weight[i] } else { 0.0 }).collect();
        let mut y = resample(&even, SR, sr);
        y.resize(len, 0.0);
        y
    };
    Ok(std::array::from_fn(|k| (back(&out[k][0]), back(&out[k][1]))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// nets that hear vocals in the bins under 1 kHz, drums above 4 kHz, bass under 200 Hz and the rest between
    struct Bands;

    impl Nets for Bands {
        fn estimate(&mut self, magnitudes: &[f32], _: usize) -> Result<[Vec<f32>; 4]> {
            let hz = |j: usize| j as f64 * SR as f64 / N_FFT as f64;
            let pick = |keep: &dyn Fn(f64) -> bool| -> Vec<f32> {
                magnitudes.iter().enumerate().map(|(i, m)| if keep(hz(i % F)) { *m } else { 0.0 }).collect()
            };
            Ok([
                pick(&|f| (200.0..1000.0).contains(&f)),
                pick(&|f| f >= 4000.0),
                pick(&|f| f < 200.0),
                pick(&|f| (1000.0..4000.0).contains(&f)),
            ])
        }
    }

    #[test]
    fn the_stems_add_up_to_the_recording_and_each_holds_what_its_net_heard() {
        // three seconds at 48 kHz: a 100 Hz bass, a 440 Hz voice and a 6 kHz hiss, louder on the left
        let sr = 48_000u32;
        let n = 3 * sr as usize;
        let tone = |f: f64, a: f32, i: usize| a * (std::f64::consts::TAU * f * i as f64 / sr as f64).sin() as f32;
        let l: Vec<f32> = (0..n).map(|i| tone(100.0, 0.3, i) + tone(440.0, 0.3, i) + tone(6000.0, 0.1, i)).collect();
        let r: Vec<f32> = l.iter().map(|v| v * 0.5).collect();
        let mut heard = Vec::new();
        let stems = separate_with(&mut Bands, &(l.clone(), r.clone()), sr, &mut |p| heard.push(p)).unwrap();
        assert_eq!(heard.last().copied(), Some(1.0));
        for (k, (sl, sr_)) in stems.iter().enumerate() {
            assert_eq!((sl.len(), sr_.len()), (n, n), "{}", STEMS[k]);
        }
        // the four add up to the recording (away from its first and last few milliseconds, where resampling rings)
        let edge = 2400;
        let power = |v: &[f32]| v[edge..n - edge].iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
        let sum: Vec<f32> = (0..n).map(|i| stems.iter().map(|s| s.0[i]).sum::<f32>() - l[i]).collect();
        let residual = 10.0 * (power(&sum) / power(&l)).log10();
        assert!(residual < -50.0, "the stems add up to the recording within {residual:.1} dB");
        // each stem holds its band: the voice's 440 Hz in the vocals, the bass's 100 Hz in the bass, the hiss in the drums
        let level = |v: &[f32], f: f64| {
            let (mut c, mut s) = (0.0, 0.0);
            for (i, x) in v.iter().enumerate().take(n - edge).skip(edge) {
                let p = std::f64::consts::TAU * f * i as f64 / sr as f64;
                (c, s) = (c + *x as f64 * p.cos(), s + *x as f64 * p.sin());
            }
            2.0 * (c * c + s * s).sqrt() / (n - 2 * edge) as f64
        };
        let ((voc, _), (drm, _), (bas, _), (oth, _)) = (&stems[0], &stems[1], &stems[2], &stems[3]);
        assert!(level(voc, 440.0) > 0.25 && level(oth, 440.0) < 0.03, "{} {}", level(voc, 440.0), level(oth, 440.0));
        assert!(level(bas, 100.0) > 0.25 && level(voc, 100.0) < 0.03);
        assert!(level(drm, 6000.0) > 0.08 && level(voc, 6000.0) < 0.01);
        // the right channel is its own: half as loud
        assert!((level(&stems[0].1, 440.0) / level(voc, 440.0) - 0.5).abs() < 0.02);
    }
}
