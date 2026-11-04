//! Loop warping: fit a recorded loop to the project tempo and key. Signalsmith Stretch does time-stretch and
//! pitch-shift in one pass; a tempo change under 1.5 % with no transpose, or one that moves the pitch by just the
//! semitones the loop is transposed (a sampler's varispeed: a record sped up four semitones into house), is plain
//! resampling (no artifacts at all).
//! Loops are stretched three copies end to end and the middle one is kept, so the result wraps seamlessly; a take,
//! played once, is stretched between silences, so neither of its ends hears the other. A build without the `native`
//! feature (a browser's, which cannot build Signalsmith's C++) varispeeds every loop, as a turntable would: it fits the
//! tempo and keeps its own key.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;

use crate::audio::{self, Stereo, resample_step};
use crate::config::{self, SRF};

type Cache = Mutex<HashMap<String, Arc<Stereo>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// the loop at `path` (`beats` long at `src_bpm`) warped to `dst_bpm` and transposed by `semis`; `seamless` for a loop,
/// which wraps, and not for a take
pub fn warp(path: &str, src_bpm: f64, beats: f64, dst_bpm: f64, semis: i32, seamless: bool) -> Result<Arc<Stereo>> {
    let key = format!("{path}|{src_bpm:.3}|{beats:.3}|{dst_bpm:.3}|{semis}{}", if seamless { "" } else { "|once" });
    if let Some(x) = cache().lock().unwrap().get(&key) {
        return Ok(x.clone());
    }
    let disk = config::cache_dir("stretched").join(format!("{}.f32", &blake3::hash(key.as_bytes()).to_hex()[..20]));
    if config::FILES
        && let Ok(bytes) = std::fs::read(&disk)
    {
        let x: Vec<f32> = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let s = Arc::new(Stereo::from_interleaved(&x));
        cache().lock().unwrap().insert(key, s.clone());
        return Ok(s);
    }
    let mut x = audio::load(Path::new(path))?;
    let src_len = ((beats * 60.0 / src_bpm.max(1.0)) * SRF).round() as usize;
    x.ensure_len(src_len);
    x.truncate(src_len);
    let dst_len = ((beats * 60.0 / dst_bpm) * SRF).round() as usize;
    let ratio = dst_len as f64 / src_len.max(1) as f64;
    let out = if resamples(ratio, semis) {
        let step = src_len as f64 / dst_len.max(1) as f64;
        Stereo { l: resample_step(&x.l, step, dst_len), r: resample_step(&x.r, step, dst_len) }
    } else {
        let mut tripled = Stereo::zeros(src_len * 3);
        for k in 0..3 {
            if seamless || k == 1 {
                tripled.add_at(&x, (k * src_len) as i64, 1.0, 1.0);
            }
        }
        stretch(&tripled, dst_len * 3, semis).slice(dst_len, 2 * dst_len)
    };
    if config::FILES {
        let bytes: Vec<u8> = out.interleaved().iter().flat_map(|v| v.to_le_bytes()).collect();
        let tmp = disk.with_extension(format!("{}.tmp", config::process_id()));
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &disk);
        }
    }
    let out = Arc::new(out);
    cache().lock().unwrap().insert(key, out.clone());
    Ok(out)
}

/// whether a loop made `ratio` times as long and transposed by `semis` is only resampled: a tempo change under 1.5 %
/// with no transpose, or one that moves its pitch by just the semitones asked (within 5 cents)
fn resamples(ratio: f64, semis: i32) -> bool {
    (12.0 * (1.0 / ratio).log2() - semis as f64).abs() < 0.05 || (semis == 0 && (ratio - 1.0).abs() < 0.015)
}

/// `x` stretched to `len` samples and transposed by `semis`, in one pass
#[cfg(feature = "native")]
fn stretch(x: &Stereo, len: usize, semis: i32) -> Stereo {
    let mut st = signalsmith_stretch::Stretch::preset_default(2, crate::config::SR);
    st.set_transpose_factor_semitones(semis as f32, None);
    let mut output = vec![0.0f32; len * 2];
    st.exact(x.interleaved(), &mut output);
    Stereo::from_interleaved(&output)
}

/// `x` varispeeded to `len` samples, in its own key
#[cfg(not(feature = "native"))]
fn stretch(x: &Stereo, len: usize, _semis: i32) -> Stereo {
    let step = x.len() as f64 / len.max(1) as f64;
    Stereo { l: resample_step(&x.l, step, len), r: resample_step(&x.r, step, len) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_sped_up_by_as_many_semitones_as_it_is_transposed_is_only_resampled() {
        // a record at 98 sped up four semitones plays at 123.48: a quarter shorter, as a sampler plays it
        let four_up = 98.0 / (98.0 * 2f64.powf(4.0 / 12.0));
        assert!(resamples(four_up, 4));
        // the same tempo kept in its key, or moved by a semitone it was not asked: a stretch
        assert!(!resamples(four_up, 0) && !resamples(four_up, 3));
        assert!(!resamples(1.0, 2));
        // a tempo nudged under 1.5 %, as before
        assert!(resamples(1.01, 0) && !resamples(1.05, 0));
    }
}
