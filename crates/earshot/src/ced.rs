//! CED-Tiny, an AudioSet tagger of 5.5 million parameters distilled from an ensemble of transformers (Dinkel et al.,
//! "CED: Consistent Ensemble Distillation for Audio Tagging", ICASSP 2024), as its authors export it to ONNX
//! (`mispeech/ced-tiny`, Apache-2.0): 16 kHz audio of any length in, its mel front end inside, the 527 AudioSet logits
//! out. As accurate on AudioSet as AST (mAP 0.481 against 0.459) at a fiftieth of its size, and it hears a stretch at
//! its own length rather than padded with silence to 10.24 s.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;

use crate::dsp::resample;
use crate::models::ox;

/// the rate it hears at
pub const SR_CED: u32 = 16000;
/// the shortest it is given, in samples at `SR_CED`: a quarter of a second, a stretch shorter padded with silence to
/// it (its front end frames 32 ms)
const SHORTEST: usize = 4000;

pub struct Ced {
    session: Mutex<Session>,
    pub labels: Vec<String>,
}

impl Ced {
    /// The tagger from `cache` (downloaded there first unless `offline`), on `threads` cores.
    pub fn load(cache: &Path, offline: bool, threads: usize) -> Result<Ced> {
        use crate::models::{CED_CONFIG, CED_MODEL, CED_REPO, CED_REV, fetch, session};
        let model = fetch(cache, CED_REPO, CED_REV, CED_MODEL, offline)?;
        let config = fetch(cache, CED_REPO, CED_REV, CED_CONFIG, offline)?;
        let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config)?)?;
        let map = cfg["id2label"].as_object().ok_or_else(|| anyhow!("CED config.json has no id2label"))?;
        let mut labels = vec![String::new(); map.len()];
        for (k, v) in map {
            let i: usize = k.parse()?;
            if i < labels.len() {
                labels[i] = v.as_str().unwrap_or_default().to_string();
            }
        }
        Ok(Ced { session: Mutex::new(session(&model, threads)?), labels })
    }

    /// whether its files are in `cache`
    pub fn cached(cache: &Path) -> bool {
        use crate::models::{CED_CONFIG, CED_MODEL, CED_REPO, CED_REV, model_path};
        [CED_MODEL, CED_CONFIG].iter().all(|f| model_path(cache, CED_REPO, CED_REV, f).is_file())
    }

    /// The 527 AudioSet probabilities of each 48 kHz mono array, each heard at its own length.
    pub fn probs(&self, arrays: &[&[f32]]) -> Result<Vec<Vec<f32>>> {
        let mut sess = self.session.lock().map_err(|_| anyhow!("CED session poisoned"))?;
        let mut out = Vec::with_capacity(arrays.len());
        for x in arrays {
            let mut y = resample(x, crate::SR, SR_CED);
            if y.len() < SHORTEST {
                y.resize(SHORTEST, 0.0);
            }
            let n = y.len();
            let t = ox(Tensor::from_array(([1usize, n], y)))?;
            let res = ox(sess.run(ort::inputs!["waveform" => t]))?;
            let (_, logits) = ox(res["logits"].try_extract_tensor::<f32>())?;
            out.push(logits.iter().map(|&l| (1.0 / (1.0 + (-(l as f64)).exp())) as f32).collect());
        }
        Ok(out)
    }
}
