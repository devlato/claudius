//! Aesthetics: how a recording rates on Meta's Audiobox Aesthetics (Tjandra, Wu, Guo et al., "Meta Audiobox
//! Aesthetics: Unified Automatic Quality Assessment for Speech, Music, and Sound", 2025, arXiv:2502.05139), four
//! scores from 1 to 10 that listeners trained to rate each were asked for: how much they enjoy what they hear (content
//! enjoyment), how useful it would be to make something of (content usefulness), how many things it is made of
//! (production complexity), and how well it is recorded and mixed (production quality). On natural music its scores
//! agree with the listeners' about as well as the listeners agree with one another; on music a model made, less.
//!
//! The model and its weights are Meta's, CC-BY 4.0, and part of its code is Microsoft's WavLM, MIT. It runs as an ONNX
//! export of them (`FILE` in `REPO` at `REV`, pinned by its SHA-256), its MatMul weights quantized to int8 per channel,
//! which rates a whole song within 0.02 of the original. It hears 16 kHz mono ten seconds at a time and says each score
//! normalized, which `STATS` undoes. A recording is cut into back-to-back windows, the last as long as is left (padded,
//! it would be heard as silence), and its scores are the windows' averaged by their length.

use std::path::Path;

use anyhow::{Result, anyhow, bail};
use ort::session::Session;
use ort::value::Tensor;
use serde::{Deserialize, Serialize};

use crate::dsp::resample::resample;
use crate::models::{self, ox};

/// where the model is on the Hugging Face Hub
pub const REPO: &str = "Microck/audiobox-aesthetics-onnx";
pub const REV: &str = "5daff8012a45bf6fbfee30503666f7ba85e61c50";
pub const FILE: &str = "aes.int8.onnx";

/// what the model hears: 16 kHz mono, ten seconds at a time
pub const SR: u32 = 16_000;
pub const WINDOW: usize = 160_000;
/// the shortest last window worth hearing, when others come before it: less than a second weighs under a tenth
const SHORTEST: usize = SR as usize;
/// the fewest samples the model's feature extractor takes
const LEAST: usize = 400;

/// the mean and the standard deviation the model's scores were normalized by, content enjoyment, content
/// usefulness, production complexity and production quality, as its `config.json` gives them (`target_transform`)
pub const STATS: [(f64, f64); 4] = [(5.06865, 1.93029), (5.73633, 1.75669), (3.18591, 1.86637), (6.57505, 1.51466)];

/// How a recording rates, each score from 1 to 10.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scores {
    /// content enjoyment: how much a listener would enjoy it
    pub ce: f64,
    /// content usefulness: how useful it would be as material to make something of
    pub cu: f64,
    /// production complexity: how many things it is made of, how dense it is
    pub pc: f64,
    /// production quality: how clear, full and well balanced its recording and mix are
    pub pq: f64,
}

impl Scores {
    /// each score as a z against what the model was trained on (`STATS`): 0 for the average recording it heard, 1
    /// for one a standard deviation better
    pub fn z(&self) -> [f64; 4] {
        let v = [self.ce, self.cu, self.pc, self.pq];
        std::array::from_fn(|i| (v[i] - STATS[i].0) / STATS[i].1)
    }
}

/// What rates a window: the model's four outputs, normalized, for `window`, 16 kHz mono of `WINDOW` samples at most.
pub trait Net {
    fn rate(&mut self, window: &[f32]) -> Result<[f32; 4]>;
}

/// the model
pub struct Aesthetics {
    net: Session,
}

impl Aesthetics {
    /// the model, from `cache` (downloaded into it first, unless `offline`), on `threads` threads
    pub fn load(cache: &Path, offline: bool, threads: usize) -> Result<Aesthetics> {
        let path = models::fetch(cache, REPO, REV, FILE, offline)?;
        Ok(Aesthetics { net: models::session(&path, threads)? })
    }

    /// whether the model is in `cache`
    pub fn cached(cache: &Path) -> bool {
        models::model_path(cache, REPO, REV, FILE).is_file()
    }

    /// how `x`, mono at `sr` Hz, rates: every window of it (`rate_with`), or `most` spread evenly across it
    pub fn rate(&mut self, x: &[f32], sr: u32, most: Option<usize>) -> Result<Scores> {
        rate_with(self, x, sr, most)
    }
}

impl Net for Aesthetics {
    fn rate(&mut self, window: &[f32]) -> Result<[f32; 4]> {
        let t = ox(Tensor::from_array(([1, 1, window.len()], window.to_vec())))?;
        let res = ox(self.net.run(ort::inputs!["wav" => t]))?;
        let (_, y) = ox(res["out"].try_extract_tensor::<f32>())?;
        match y {
            [ce, cu, pc, pq] => Ok([*ce, *cu, *pc, *pq]),
            _ => Err(anyhow!("the model said {} scores, not 4", y.len())),
        }
    }
}

/// How `net` rates `x`, mono at `sr` Hz: its back-to-back windows of `WINDOW` samples at 16 kHz (the last as long as
/// is left, left out when under a second and others come before it), or `most` of them spread evenly across it, each
/// score the windows' averaged by their length, its normalization undone.
pub fn rate_with(net: &mut dyn Net, x: &[f32], sr: u32, most: Option<usize>) -> Result<Scores> {
    let y = resample(x, sr, SR);
    if y.len() < LEAST {
        bail!("{} ms is too short to rate", y.len() as u64 * 1000 / SR as u64);
    }
    let mut windows: Vec<&[f32]> = y.chunks(WINDOW).collect();
    if windows.len() > 1 && windows.last().is_some_and(|w| w.len() < SHORTEST) {
        windows.pop();
    }
    if let Some(n) = most.filter(|&n| n > 0 && n < windows.len()) {
        let last = windows.len() - 1;
        windows = (0..n).map(|i| windows[if n == 1 { last / 2 } else { i * last / (n - 1) }]).collect();
    }
    let (mut sum, mut weight) = ([0f64; 4], 0f64);
    for w in windows {
        let out = net.rate(w)?;
        let k = w.len() as f64 / WINDOW as f64;
        for (s, (o, (mean, std))) in sum.iter_mut().zip(out.iter().zip(STATS)) {
            *s += (*o as f64 * std + mean) * k;
        }
        weight += k;
    }
    let [ce, cu, pc, pq] = sum.map(|s| s / weight);
    Ok(Scores { ce, cu, pc, pq })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a net that rates a window by its length: each output its share of a whole window, the second its negative
    struct ByLength(Vec<usize>);

    impl Net for ByLength {
        fn rate(&mut self, window: &[f32]) -> Result<[f32; 4]> {
            self.0.push(window.len());
            let k = window.len() as f32 / WINDOW as f32;
            Ok([k, -k, 0.0, 1.0])
        }
    }

    #[test]
    fn its_normalization_is_undone() {
        let mut net = ByLength(Vec::new());
        let s = rate_with(&mut net, &vec![0.1; WINDOW], SR, None).unwrap();
        assert!((s.ce - (5.06865 + 1.93029)).abs() < 1e-6 && (s.cu - (5.73633 - 1.75669)).abs() < 1e-6);
        assert!((s.pc - 3.18591).abs() < 1e-6 && (s.pq - (6.57505 + 1.51466)).abs() < 1e-6);
        assert!((s.z()[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_recording_is_heard_in_back_to_back_windows_weighed_by_their_length() {
        // 25 s: two whole windows and one of five seconds, unpadded; at 48 kHz it is heard at 16
        let mut net = ByLength(Vec::new());
        let s = rate_with(&mut net, &vec![0.1; 25 * 48_000], 48_000, None).unwrap();
        assert_eq!(net.0, [WINDOW, WINDOW, WINDOW / 2]);
        // the first score is each window's share, weighed by that share: (1 + 1 + 0.25) / 2.5 of a deviation
        assert!((s.ce - (5.06865 + 1.93029 * 2.25 / 2.5)).abs() < 1e-4, "{}", s.ce);
        // a last half second is left out, unless it is all there is
        let mut net = ByLength(Vec::new());
        rate_with(&mut net, &vec![0.1; WINDOW + SR as usize / 2], SR, None).unwrap();
        assert_eq!(net.0, [WINDOW]);
        let mut net = ByLength(Vec::new());
        rate_with(&mut net, &vec![0.1; SR as usize / 2], SR, None).unwrap();
        assert_eq!(net.0, [SR as usize / 2]);
        assert!(rate_with(&mut net, &[0.1; 100], SR, None).is_err(), "too short");
    }

    #[test]
    fn a_long_recording_can_be_heard_in_a_few_windows_spread_across_it() {
        let mut x = vec![0.0f32; 60 * SR as usize];
        // each window marked by its index in its first sample
        for (i, w) in x.chunks_mut(WINDOW).enumerate() {
            w[0] = i as f32;
        }
        struct First(Vec<f32>);
        impl Net for First {
            fn rate(&mut self, window: &[f32]) -> Result<[f32; 4]> {
                self.0.push(window[0]);
                Ok([0.0; 4])
            }
        }
        let mut net = First(Vec::new());
        rate_with(&mut net, &x, SR, Some(3)).unwrap();
        assert_eq!(net.0, [0.0, 2.0, 5.0], "the first, the middle and the last");
        let mut net = First(Vec::new());
        rate_with(&mut net, &x, SR, Some(10)).unwrap();
        assert_eq!(net.0.len(), 6, "no more than there are");
    }
}
