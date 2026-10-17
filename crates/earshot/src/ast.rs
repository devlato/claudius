//! AudioSet tagger (MIT/ast-finetuned-audioset-10-10-0.4593 as the Xenova fp32 ONNX export): 527 classes with
//! independent probabilities.
//!
//! Front end = the numpy path transformers' ASTFeatureExtractor takes when torchaudio is not installed (as in
//! earshot's environment): frames of 400 at hop 160 (no centring), DC removal, pre-emphasis 0.97, symmetric Hann,
//! 512-point FFT, power, 128 kaldi-mel bands (20 Hz - 8 kHz, triangles in mel space), natural log with floor
//! 1.1920929e-7, zero-padded / cut to 1024 frames, then (x + 4.2677393) / (2 * 4.5689974). Input is resampled to
//! 16 kHz and read in 10.24 s windows (half-window hop, the last flush with the end), reduced by max or mean.

use std::sync::Mutex;

use anyhow::{Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;

use crate::dsp::fft::{C64, Rfft};
use crate::dsp::{hanning, resample, stats};
use crate::models::ox;

pub const SR_AST: u32 = 16000;
pub const WIN_S: f64 = 10.24;
pub const N_CLASSES: usize = 527;
const N_MELS: usize = 128;
const MAX_FRAMES: usize = 1024;
const FRAME: usize = 400;
const HOP: usize = 160;
const N_FFT: usize = 512;
const MEAN: f32 = -4.2677393;
const STD: f32 = 4.5689974;
const MEL_FLOOR: f64 = 1.192092955078125e-07;

/// how window probabilities are combined for a long sound
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Reduce {
    #[default]
    Max,
    Mean,
}

fn kaldi_filters() -> Vec<(usize, Vec<f64>)> {
    let nb = N_FFT / 2 + 1;
    let mel = |f: f64| 1127.0 * (1.0 + f / 700.0).ln();
    let mels = crate::librosa::linspace(mel(20.0), mel(8000.0), N_MELS + 2);
    let bin_w = SR_AST as f64 / ((nb - 1) * 2) as f64;
    let fft_m: Vec<f64> = (0..nb).map(|k| mel(bin_w * k as f64)).collect();
    (0..N_MELS)
        .map(|i| {
            let d0 = mels[i + 1] - mels[i];
            let d1 = mels[i + 2] - mels[i + 1];
            let w: Vec<f64> = fft_m
                .iter()
                .map(|&f| {
                    let down = -(mels[i] - f) / d0;
                    let up = (mels[i + 2] - f) / d1;
                    down.min(up).max(0.0)
                })
                .collect();
            let a = w.iter().position(|&v| v != 0.0).unwrap_or(0);
            let b = w.iter().rposition(|&v| v != 0.0).map_or(a, |p| p + 1);
            (a, w[a..b].to_vec())
        })
        .collect()
}

pub struct FrontEnd {
    filters: Vec<(usize, Vec<f64>)>,
    window: Vec<f64>,
}

impl Default for FrontEnd {
    fn default() -> Self {
        FrontEnd { filters: kaldi_filters(), window: hanning(FRAME) }
    }
}

impl FrontEnd {
    /// normalised [1024, 128] input values of one 16 kHz window (row-major)
    pub fn features(&self, y: &[f32]) -> Vec<f32> {
        let mut w: Vec<f64> = y.iter().map(|&v| v as f64).collect();
        if w.len() < 1600 {
            w.resize(1600, 0.0);
        }
        let n_frames = 1 + (w.len() - FRAME) / HOP;
        let mut fft = Rfft::new(N_FFT);
        let mut buf = vec![0.0; N_FFT];
        let mut spec = vec![C64::new(0.0, 0.0); N_FFT / 2 + 1];
        let mut fr = vec![0.0; FRAME];
        let mut out = vec![0.0f32; MAX_FRAMES * N_MELS];
        for t in 0..n_frames.min(MAX_FRAMES) {
            fr.copy_from_slice(&w[t * HOP..t * HOP + FRAME]);
            let m = stats::mean(&fr);
            for v in fr.iter_mut() {
                *v -= m;
            }
            let prev: Vec<f64> = fr[..FRAME - 1].to_vec();
            for i in 1..FRAME {
                fr[i] -= 0.97 * prev[i - 1];
            }
            fr[0] *= 1.0 - 0.97;
            for v in buf.iter_mut() {
                *v = 0.0;
            }
            for i in 0..FRAME {
                buf[i] = fr[i] * self.window[i];
            }
            fft.process(&mut buf, &mut spec);
            let p: Vec<f64> = spec
                .iter()
                .map(|c| {
                    let (re, im) = (c.re as f32 as f64, c.im as f32 as f64);
                    re * re + im * im
                })
                .collect();
            for (k, (a, wts)) in self.filters.iter().enumerate() {
                let mut acc = 0.0;
                for (j, wj) in wts.iter().enumerate() {
                    acc += wj * p[a + j];
                }
                let v = acc.max(MEL_FLOOR).ln() as f32;
                out[t * N_MELS + k] = (v - MEAN) / (STD * 2.0);
            }
        }
        // zero-padded frames (log domain) normalise to a constant
        let pad = (0.0f32 - MEAN) / (STD * 2.0);
        for v in out[n_frames.min(MAX_FRAMES) * N_MELS..].iter_mut() {
            *v = pad;
        }
        out
    }
}

pub struct Ast {
    pub front: FrontEnd,
    session: Mutex<Session>,
    pub labels: Vec<String>,
    pub batch: usize,
}

impl Ast {
    pub fn new(session: Session, config_json: &std::path::Path) -> Result<Ast> {
        let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config_json)?)?;
        let map = cfg["id2label"].as_object().ok_or_else(|| anyhow!("AST config.json has no id2label"))?;
        let mut labels = vec![String::new(); map.len()];
        for (k, v) in map {
            let i: usize = k.parse()?;
            if i < labels.len() {
                labels[i] = v.as_str().unwrap_or_default().to_string();
            }
        }
        // one window per run: batching is slower on CPU (attention over 1214 patches is memory-bound: measured
        // 1.3 s for one window, 3.2 s per window in a batch of 4)
        Ok(Ast { front: FrontEnd::default(), session: Mutex::new(session), labels, batch: 1 })
    }

    fn forward(&self, segs: &[&[f32]]) -> Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(segs.len());
        for chunk in segs.chunks(self.batch.max(1)) {
            let mut data = Vec::with_capacity(chunk.len() * MAX_FRAMES * N_MELS);
            for s in chunk {
                data.extend(self.front.features(s));
            }
            let t = ox(Tensor::from_array(([chunk.len(), MAX_FRAMES, N_MELS], data)))?;
            let mut sess = self.session.lock().map_err(|_| anyhow!("AST session poisoned"))?;
            let res = ox(sess.run(ort::inputs!["input_values" => t]))?;
            let (_, logits) = ox(res["logits"].try_extract_tensor::<f32>())?;
            for row in logits.chunks(N_CLASSES) {
                out.push(row.iter().map(|&l| (1.0 / (1.0 + (-(l as f64)).exp())) as f32).collect());
            }
        }
        Ok(out)
    }

    /// 48 kHz mono arrays -> 527 class probabilities each (10.24 s windows reduced by max or mean)
    pub fn probs(&self, arrays: &[&[f32]], reduce: Reduce) -> Result<Vec<Vec<f32>>> {
        let n = (WIN_S * SR_AST as f64) as usize;
        let ys: Vec<Vec<f32>> = arrays.iter().map(|x| resample(x, crate::SR, SR_AST)).collect();
        let mut segs: Vec<&[f32]> = Vec::new();
        let mut owner = Vec::new();
        for (j, y) in ys.iter().enumerate() {
            if y.len() <= n {
                segs.push(y);
                owner.push(j);
            } else {
                let mut s = 0;
                while s + n <= y.len() {
                    segs.push(&y[s..s + n]);
                    owner.push(j);
                    s += n / 2;
                }
                if !(y.len() - n).is_multiple_of(n / 2) {
                    segs.push(&y[y.len() - n..]);
                    owner.push(j);
                }
            }
        }
        let p = self.forward(&segs)?;
        let mut res = vec![vec![0.0f32; N_CLASSES]; arrays.len()];
        let mut count = vec![0usize; arrays.len()];
        for (row, &j) in p.iter().zip(&owner) {
            for (a, b) in res[j].iter_mut().zip(row) {
                *a = match reduce {
                    Reduce::Max => {
                        if count[j] == 0 {
                            *b
                        } else {
                            a.max(*b)
                        }
                    }
                    Reduce::Mean => *a + *b,
                };
            }
            count[j] += 1;
        }
        if reduce == Reduce::Mean {
            for (r, c) in res.iter_mut().zip(&count) {
                for v in r.iter_mut() {
                    *v /= (*c).max(1) as f32;
                }
            }
        }
        Ok(res)
    }

    /// the k most probable labels (label, p rounded to 3 decimals) with p >= min_p
    pub fn top(&self, p: &[f32], k: usize, min_p: f64) -> Vec<(String, f64)> {
        let v: Vec<f64> = p.iter().map(|&x| x as f64).collect();
        stats::argsort_desc(&v)
            .into_iter()
            .take(k)
            .filter(|&i| v[i] >= min_p)
            .map(|i| (self.labels[i].clone(), crate::dsp::round_to(v[i], 3)))
            .collect()
    }

    pub fn prob_of(&self, p: &[f32], label: &str) -> Option<f64> {
        self.labels.iter().position(|l| l == label).map(|i| p[i] as f64)
    }
}
