//! Whistle, Cactus Compute's 16.9 MB speech model, in Rust on needle-core's kernels: 16 kHz speech of up to 30
//! seconds to text, in seven languages, on any processor.
//!
//! A port of Cactus's Apache-2.0 JAX reference (`cactus-compute/cactus` at `bafcf73`: `cactus/model/whistle.py`,
//! `architecture.py`, `export.py`, `quantize.py`) and of the engine's behaviour Cactus documents in "Porting
//! Whistle": the log-mel front end (`frontend`), the encoder of eight macaron blocks over a convolution stem
//! (`encoder`), the decoder of eight layers attending to it (`decoder`), and greedy and beam search over its tokens
//! (`search`). The weights are read from the model's archive (`whistle.cact`) through needle-infer into
//! needle-core's types (`layout`).

#![forbid(unsafe_code)]

pub mod decoder;
pub mod encoder;
pub mod frontend;
pub mod layout;
pub mod nn;
pub mod search;

use std::time::Instant;

use frontend::FrontEnd;
pub use layout::{LoadError, Loaded};
pub use nn::Numerics;
pub use search::Stop;

/// The longest clip one encoder pass takes (375 encoder frames).
pub const MAX_SECONDS: usize = 30;

/// The loaded model and its front end.
pub struct Whistle {
    pub model: Loaded,
    pub front: FrontEnd,
}

#[derive(Debug, Clone)]
pub struct Options {
    /// A language code to force, or `None` to detect it.
    pub language: Option<String>,
    /// 1 is the greedy decode.
    pub beams: usize,
    /// Text proposals per beam and step.
    pub proposals: usize,
    pub numerics: Numerics,
    /// The engine's level gain.
    pub level: bool,
    /// The engine's silence gate.
    pub gate: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { language: None, beams: 5, proposals: 6, numerics: Numerics::default(), level: true, gate: true }
    }
}

/// Milliseconds per stage.
#[derive(Debug, Clone, Copy, Default)]
pub struct Timings {
    pub front_end: f64,
    pub encoder: f64,
    pub cross_kv: f64,
    pub decode: f64,
    pub steps: usize,
}

#[derive(Debug, Clone)]
pub struct Transcript {
    pub text: String,
    /// Empty when the silence gate stopped the clip.
    pub language: String,
    pub language_prob: f32,
    pub tokens: Vec<u32>,
    pub stop: Option<Stop>,
    pub feature_frames: usize,
    pub encoder_frames: usize,
    /// p99 - p10 of the frames' 250-3500 Hz energy, in dB.
    pub gate_spread_db: f32,
    pub silent: bool,
    pub timings: Timings,
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

impl Whistle {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LoadError> {
        let model = layout::load(bytes)?;
        let front = FrontEnd::new(model.mel.clone(), &model.man);
        Ok(Self { model, front })
    }

    pub fn language_index(&self, code: &str) -> Option<usize> {
        self.model.languages.iter().position(|l| l == code)
    }

    /// The speech embedding: encoder output, `(frames, d)`.
    pub fn embed(&self, pcm: &[f32], opts: &Options) -> (Vec<f32>, usize) {
        let x = frontend::condition(pcm, opts.level);
        let f = self.front.features(&x);
        self.model.enc.forward(&f.data, f.frames, f.mels, &self.model.perms, opts.numerics)
    }

    /// 16 kHz mono samples in [-1, 1] to text, up to [`MAX_SECONDS`].
    pub fn transcribe(&self, pcm: &[f32], opts: &Options) -> Result<Transcript, String> {
        if pcm.len() > MAX_SECONDS * self.model.man.sample_rate {
            return Err(format!(
                "{:.1} s of audio: one pass takes at most {MAX_SECONDS} s, and windowing longer audio is not implemented",
                pcm.len() as f64 / self.model.man.sample_rate as f64
            ));
        }
        let forced = match &opts.language {
            Some(code) => Some(
                self.language_index(code)
                    .ok_or_else(|| format!("language {code} is not one of {}", self.model.languages.join(" ")))?,
            ),
            None => None,
        };
        let mut timings = Timings::default();

        let t0 = Instant::now();
        let x = frontend::condition(pcm, opts.level);
        let f = self.front.features(&x);
        timings.front_end = ms(t0);
        let gate_spread_db = frontend::gate_spread(&f.band_db);
        let silent = opts.gate && gate_spread_db < frontend::GATE_DB;
        let mut out = Transcript {
            gate_spread_db,
            text: String::new(),
            language: String::new(),
            language_prob: 0.0,
            tokens: Vec::new(),
            stop: None,
            feature_frames: f.frames,
            encoder_frames: 0,
            silent,
            timings,
        };
        if silent || f.frames == 0 {
            return Ok(out);
        }

        let t1 = Instant::now();
        let (enc, frames) = self.model.enc.forward(&f.data, f.frames, f.mels, &self.model.perms, opts.numerics);
        out.timings.encoder = ms(t1);
        out.encoder_frames = frames;

        let t2 = Instant::now();
        let mem = self.model.dec.memory(&enc, frames, opts.numerics);
        out.timings.cross_kv = ms(t2);

        let t3 = Instant::now();
        let d = if opts.beams <= 1 {
            search::greedy(self, &mem, forced, opts.numerics)
        } else {
            search::beam(self, &mem, forced, opts.beams, opts.proposals, opts.numerics)
        };
        out.timings.decode = ms(t3);
        out.timings.steps = d.steps;
        out.text = self.model.tok.decode(&d.tokens).trim().to_string();
        out.language = self.model.languages[d.language].clone();
        out.language_prob = d.language_prob;
        out.tokens = d.tokens;
        out.stop = Some(d.stop);
        Ok(out)
    }
}
