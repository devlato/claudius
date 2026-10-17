//! CLAP (laion/larger_clap_general as the Xenova fp32 ONNX export): one embedding space for sounds and text.
//!
//! Front end identical to transformers' ClapFeatureExtractor with the model's preprocessor config (truncation
//! 'rand_trunc' never triggers because windows are cut to 10 s first): repeat-pad (tile int(480000 / n) times, then
//! zero-pad), centred reflect padding, periodic Hann 1024, hop 480, power 2, Slaney mel 50 - 14000 Hz (64 bands),
//! power_to_db with floor 1e-10 -> one [1001, 64] log-mel per 10 s window. earshot's clap._prep runs first: input under
//! 10 ms is zero-padded to 480 samples, pad 'zero' pads to 10 s with silence instead of tiling.

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;

use crate::dsp::fft::{C64, Rfft};
use crate::dsp::hann_periodic;
use crate::models::ox;

pub const WIN_S: f64 = 10.0;
pub const HOP_S: f64 = 5.0;
pub const N_WIN: usize = 480_000;
pub const N_FRAMES: usize = 1001;
pub const N_MELS: usize = 64;
pub const DIM: usize = 512;
const N_FFT: usize = 1024;
const HOP: usize = 480;

/// how a window shorter than 10 s is padded (vocab.PADS)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pad {
    /// tile the sound (CLAP's training convention; earshot's default)
    #[default]
    RepeatPad,
    /// silence after the sound
    Zero,
    /// silence up to 1 s, then tile: one hit per second (vocab 'spaced')
    Spaced,
}

impl Pad {
    pub fn parse(s: &str) -> Option<Pad> {
        match s {
            "repeatpad" => Some(Pad::RepeatPad),
            "zero" => Some(Pad::Zero),
            "spaced" => Some(Pad::Spaced),
            _ => None,
        }
    }
}

fn hz_to_mel_slaney(f: f64) -> f64 {
    let logstep = 27.0 / 6.4f64.ln();
    let m = 3.0 * f / 200.0;
    if f >= 1000.0 { 15.0 + (f / 1000.0).ln() * logstep } else { m }
}

fn mel_to_hz_slaney(m: f64) -> f64 {
    let logstep = 6.4f64.ln() / 27.0;
    let f = 200.0 * m / 3.0;
    if m >= 15.0 { 1000.0 * (logstep * (m - 15.0)).exp() } else { f }
}

/// transformers mel_filter_bank(513, 64, 50, 14000, 48000, norm='slaney', mel_scale='slaney') as sparse rows
fn slaney_filters() -> Vec<(usize, Vec<f64>)> {
    let nb = N_FFT / 2 + 1;
    let (lo, hi) = (hz_to_mel_slaney(50.0), hz_to_mel_slaney(14000.0));
    let mels = crate::librosa::linspace(lo, hi, N_MELS + 2);
    let ff: Vec<f64> = mels.iter().map(|&m| mel_to_hz_slaney(m)).collect();
    let fft_f = crate::librosa::linspace(0.0, 24000.0, nb);
    (0..N_MELS)
        .map(|i| {
            let d0 = ff[i + 1] - ff[i];
            let d1 = ff[i + 2] - ff[i + 1];
            let enorm = 2.0 / (ff[i + 2] - ff[i]);
            let w: Vec<f64> = fft_f
                .iter()
                .map(|&f| {
                    let down = -(ff[i] - f) / d0;
                    let up = (ff[i + 2] - f) / d1;
                    down.min(up).max(0.0) * enorm
                })
                .collect();
            let a = w.iter().position(|&v| v != 0.0).unwrap_or(0);
            let b = w.iter().rposition(|&v| v != 0.0).map_or(a, |p| p + 1);
            (a, w[a..b].to_vec())
        })
        .collect()
}

/// the CLAP audio front end (stateless, cheap to share)
pub struct FrontEnd {
    filters: Vec<(usize, Vec<f64>)>,
    window: Vec<f64>,
}

impl Default for FrontEnd {
    fn default() -> Self {
        FrontEnd { filters: slaney_filters(), window: hann_periodic(N_FFT) }
    }
}

impl FrontEnd {
    /// clap._prep + ClapFeatureExtractor for one window (mono 48 kHz, cut to 10 s): 1001 x 64 log-mel, row-major
    pub fn features(&self, x: &[f32], pad: Pad) -> Vec<f32> {
        // earshot clap._prep (pad 'spaced' reaches here as repeatpad, already spaced by the caller)
        let mut w: Vec<f64> = x.iter().take(N_WIN).map(|&v| v as f64).collect();
        if w.len() < 480 {
            w.resize(480, 0.0);
        }
        if w.len() < N_WIN && pad == Pad::Zero {
            w.resize(N_WIN, 0.0);
        }
        // ClapFeatureExtractor 'repeatpad'
        if w.len() < N_WIN {
            let reps = N_WIN / w.len();
            let one = w.clone();
            w.clear();
            for _ in 0..reps {
                w.extend_from_slice(&one);
            }
            w.resize(N_WIN, 0.0);
        }
        self.log_mel(&w)
    }

    /// spectrogram(waveform, periodic hann, 1024, hop 480, power 2, center reflect, slaney mel, dB): frames x mels
    pub fn log_mel(&self, w: &[f64]) -> Vec<f32> {
        let n = w.len();
        let pad = N_FFT / 2;
        let get = |j: i64| -> f64 {
            // numpy 'reflect' padding (edge sample not repeated)
            let idx = if j < 0 {
                (-j) as usize
            } else if j as usize >= n {
                2 * (n - 1) - j as usize
            } else {
                j as usize
            };
            w[idx]
        };
        let frames = 1 + (n + 2 * pad - N_FFT) / HOP;
        let mut fft = Rfft::new(N_FFT);
        let mut buf = vec![0.0; N_FFT];
        let mut spec = vec![C64::new(0.0, 0.0); N_FFT / 2 + 1];
        let mut out = vec![0.0f32; frames * N_MELS];
        for t in 0..frames {
            let start = (t * HOP) as i64 - pad as i64;
            for (i, (b, w)) in buf.iter_mut().zip(&self.window).enumerate() {
                *b = get(start + i as i64) * w;
            }
            fft.process(&mut buf, &mut spec);
            // stored as complex64, then |.|^2 in float64
            let p: Vec<f64> = spec
                .iter()
                .map(|c| {
                    let (re, im) = (c.re as f32 as f64, c.im as f32 as f64);
                    re * re + im * im
                })
                .collect();
            for (m, (a, wts)) in self.filters.iter().enumerate() {
                let mut acc = 0.0;
                for (k, wk) in wts.iter().enumerate() {
                    acc += wk * p[a + k];
                }
                out[t * N_MELS + m] = (10.0 * acc.max(1e-10).log10()) as f32;
            }
        }
        out
    }
}

pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    for x in v.iter_mut() {
        *x = (*x as f64 / (n + 1e-12)) as f32;
    }
}

/// where the text encoder comes from: loaded on the first text-cache miss (500 MB the vocabulary cache usually
/// makes unnecessary)
pub struct TextModel {
    pub path: std::path::PathBuf,
    pub threads: usize,
}

/// CLAP audio + text encoders (ONNX sessions behind mutexes: usable from several threads)
pub struct Clap {
    pub front: FrontEnd,
    audio: Mutex<Session>,
    text: Mutex<Option<Session>>,
    text_model: TextModel,
    tokenizer: tokenizers::Tokenizer,
    pub batch: usize,
}

impl Clap {
    pub fn new(audio: Session, text: TextModel, tokenizer_json: &std::path::Path) -> Result<Clap> {
        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_json)
            .map_err(|e| anyhow!("cannot load {}: {e}", tokenizer_json.display()))?;
        Ok(Clap {
            front: FrontEnd::default(),
            audio: Mutex::new(audio),
            text: Mutex::new(None),
            text_model: text,
            tokenizer,
            batch: 8,
        })
    }

    /// whether the text encoder has been loaded yet
    pub fn text_loaded(&self) -> bool {
        self.text.lock().map(|t| t.is_some()).unwrap_or(false)
    }

    /// drop the text encoder (it reloads on the next text-cache miss, ~1.5 s)
    pub fn unload_text(&self) {
        if let Ok(mut t) = self.text.lock() {
            *t = None;
        }
    }

    /// unit-norm embeddings of windows that are already at most 10 s long (one CLAP pass each), batched
    pub fn embed_windows(&self, segs: &[&[f32]], pad: Pad) -> Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(segs.len());
        for chunk in segs.chunks(self.batch.max(1)) {
            let mut data = Vec::with_capacity(chunk.len() * N_FRAMES * N_MELS);
            for s in chunk {
                data.extend(self.front.features(s, pad));
            }
            let t = ox(Tensor::from_array(([chunk.len(), 1, N_FRAMES, N_MELS], data)))?;
            let mut sess = self.audio.lock().map_err(|_| anyhow!("CLAP audio session poisoned"))?;
            let res = ox(sess.run(ort::inputs!["input_features" => t]))?;
            let (_, e) = ox(res["audio_embeds"].try_extract_tensor::<f32>())?;
            for row in e.chunks(DIM) {
                let mut v = row.to_vec();
                normalize(&mut v);
                out.push(v);
            }
        }
        Ok(out)
    }

    /// clap.embed_audio: arrays longer than 10 s become 10 s windows (hop 5 s, last one flush with the end),
    /// averaged and renormalised; per_window = the window embeddings of each array instead
    pub fn embed_audio_windows(&self, arrays: &[&[f32]], pad: Pad) -> Result<Vec<Vec<Vec<f32>>>> {
        let mut segs: Vec<&[f32]> = Vec::new();
        let mut owner = Vec::new();
        for (j, x) in arrays.iter().enumerate() {
            if x.len() <= N_WIN {
                segs.push(x);
                owner.push(j);
            } else {
                for (_, a, b) in crate::audio::windows(x.len(), WIN_S, HOP_S, true) {
                    segs.push(&x[a..b]);
                    owner.push(j);
                }
            }
        }
        let e = self.embed_windows(&segs, pad)?;
        let mut groups = vec![Vec::new(); arrays.len()];
        for (v, j) in e.into_iter().zip(owner) {
            groups[j].push(v);
        }
        Ok(groups)
    }

    /// clap.embed_audio (mean of the windows, renormalised)
    pub fn embed_audio(&self, arrays: &[&[f32]], pad: Pad) -> Result<Vec<Vec<f32>>> {
        Ok(self.embed_audio_windows(arrays, pad)?.into_iter().map(|g| mean_norm(&g)).collect())
    }

    /// token ids as the CLAP (RoBERTa) tokenizer produces them, with <s> ... </s>
    pub fn tokenize(&self, text: &str) -> Result<Vec<i64>> {
        let enc = self.tokenizer.encode(text, true).map_err(|e| anyhow!("tokenizer: {e}"))?;
        Ok(enc.get_ids().iter().map(|&i| i as i64).collect())
    }

    /// raw text embeddings (clap.embed_text: unit vectors, no templates). Texts are batched by token length, so
    /// no padding is needed (the ONNX text model has no attention-mask input).
    pub fn embed_text_raw(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let ids: Vec<Vec<i64>> = texts.iter().map(|t| self.tokenize(t)).collect::<Result<_>>()?;
        let mut order: Vec<usize> = (0..texts.len()).collect();
        order.sort_by_key(|&i| ids[i].len());
        let mut out = vec![Vec::new(); texts.len()];
        let mut i = 0;
        while i < order.len() {
            let len = ids[order[i]].len();
            let mut j = i;
            while j < order.len() && ids[order[j]].len() == len && j - i < 64 {
                j += 1;
            }
            let group = &order[i..j];
            let mut data = Vec::with_capacity(group.len() * len);
            for &g in group {
                data.extend_from_slice(&ids[g]);
            }
            let t = ox(Tensor::from_array(([group.len(), len], data)))?;
            let mut guard = self.text.lock().map_err(|_| anyhow!("CLAP text session poisoned"))?;
            if guard.is_none() {
                *guard = Some(crate::models::session(&self.text_model.path, self.text_model.threads)?);
            }
            let sess = guard.as_mut().unwrap();
            let res = ox(sess.run(ort::inputs!["input_ids" => t])).context("CLAP text model")?;
            let (_, e) = ox(res["text_embeds"].try_extract_tensor::<f32>())?;
            for (&g, row) in group.iter().zip(e.chunks(DIM)) {
                let mut v = row.to_vec();
                normalize(&mut v);
                out[g] = v;
            }
            i = j;
        }
        Ok(out)
    }
}

/// mean of unit vectors, renormalised (x / (|x| + 1e-12))
pub fn mean_norm(vs: &[Vec<f32>]) -> Vec<f32> {
    let d = vs.first().map_or(DIM, |v| v.len());
    let mut m = vec![0.0f64; d];
    for v in vs {
        for (a, b) in m.iter_mut().zip(v) {
            *a += *b as f64;
        }
    }
    let k = vs.len().max(1) as f64;
    let mut out: Vec<f32> = m.iter().map(|v| (v / k) as f32).collect();
    normalize(&mut out);
    out
}
