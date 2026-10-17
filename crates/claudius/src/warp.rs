//! Loop warping: fit a recorded loop to the project tempo and key. Signalsmith Stretch does time-stretch and
//! pitch-shift in one pass; a tempo change under 1.5 % with no transpose is plain varispeed (no artifacts at all).
//! Loops are stretched three copies end to end and the middle one is kept, so the result wraps seamlessly.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use signalsmith_stretch::Stretch;

use crate::audio::{self, Stereo, resample_step};
use crate::config::{self, SR, SRF};

type Cache = Mutex<HashMap<String, Arc<Stereo>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// the loop at `path` (`beats` long at `src_bpm`) warped to `dst_bpm` and transposed by `semis`
pub fn warp(path: &str, src_bpm: f64, beats: f64, dst_bpm: f64, semis: i32) -> Result<Arc<Stereo>> {
    let key = format!("{path}|{src_bpm:.3}|{beats:.3}|{dst_bpm:.3}|{semis}");
    if let Some(x) = cache().lock().unwrap().get(&key) {
        return Ok(x.clone());
    }
    let disk = config::cache_dir("stretched").join(format!("{}.f32", &blake3::hash(key.as_bytes()).to_hex()[..20]));
    if let Ok(bytes) = std::fs::read(&disk) {
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
    let out = if semis == 0 && (ratio - 1.0).abs() < 0.015 {
        let step = src_len as f64 / dst_len.max(1) as f64;
        Stereo { l: resample_step(&x.l, step, dst_len), r: resample_step(&x.r, step, dst_len) }
    } else {
        let mut tripled = Stereo::zeros(src_len * 3);
        for k in 0..3 {
            tripled.add_at(&x, (k * src_len) as i64, 1.0, 1.0);
        }
        let mut st = Stretch::preset_default(2, SR);
        st.set_transpose_factor_semitones(semis as f32, None);
        let input = tripled.interleaved();
        let mut output = vec![0.0f32; dst_len * 3 * 2];
        st.exact(&input, &mut output);
        Stereo::from_interleaved(&output).slice(dst_len, 2 * dst_len)
    };
    let bytes: Vec<u8> = out.interleaved().iter().flat_map(|v| v.to_le_bytes()).collect();
    let tmp = disk.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &disk);
    }
    let out = Arc::new(out);
    cache().lock().unwrap().insert(key, out.clone());
    Ok(out)
}
