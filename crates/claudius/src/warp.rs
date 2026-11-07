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
    let key = format!(
        "{path}|{src_bpm:.3}|{beats:.3}|{dst_bpm:.3}|{semis}{}|{:?}",
        if seamless { "" } else { "|once" },
        audio::remembered_version(path)
    );
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

/// `x` with each of its bars stretched to `bar_secs`: bar `i` from `starts[i]` to `starts[i + 1]` seconds into it, so
/// that a band's speeding up and slowing down comes out and it keeps one tempo, each bar as it was played inside it;
/// what plays before the first bar and after the last stays as it was. Bars as long as one another within a
/// millisecond are left as they are, and so is `x`.
pub fn flatten(x: &Stereo, starts: &[f64], bar_secs: f64) -> Stereo {
    let even = starts.windows(2).all(|w| ((w[1] - w[0]) - bar_secs).abs() < 0.001);
    if starts.len() < 2 || even || bar_secs <= 0.0 {
        return x.clone();
    }
    // the stretches: (from, to) in samples of `x`, and how long each comes out
    let at = |t: f64| ((t * SRF).round().max(0.0) as usize).min(x.len());
    let (head, tail) = (at(starts[0]), at(starts[starts.len() - 1]));
    let bar = (bar_secs * SRF).round() as usize;
    let mut parts: Vec<(usize, usize, usize)> = vec![(0, head, head)];
    parts.extend(starts.windows(2).map(|w| (at(w[0]), at(w[1]), bar)));
    parts.push((tail, x.len(), x.len() - tail));
    parts.retain(|&(a, b, n)| b > a && n > 0);
    stretched(x, &parts)
}

/// `x` through `parts`, each `(from, to, out)` a stretch of it from sample `from` to `to` made `out` samples long, which
/// follow one another from its start to its end, in one stream as Signalsmith's `exact` does a single stretch: seeked
/// on the input's first `input_latency` samples, fed the rest of it that far ahead (silence after its end), the first
/// `output_latency` samples of the output folded back and taken off, and the last flushed
#[cfg(feature = "native")]
fn stretched(x: &Stereo, parts: &[(usize, usize, usize)]) -> Stereo {
    let total: usize = parts.iter().map(|p| p.2).sum();
    let mut st = signalsmith_stretch::Stretch::preset_default(2, crate::config::SR);
    let (inl, outl) = (st.input_latency(), st.output_latency());
    if total < 2 * outl {
        return varispeeded(x, parts);
    }
    let input = x.interleaved();
    let skip = (2 * inl).min(input.len());
    let mut ahead = vec![0.0f32; input.len()];
    ahead[..input.len() - skip].copy_from_slice(&input[skip..]);
    let rate = (parts[0].1 - parts[0].0) as f64 / parts[0].2.max(1) as f64;
    st.seek(&input[..skip], rate);
    let mut out = vec![0.0f32; 2 * total];
    let mut written = 0usize;
    for &(a, b, n) in parts {
        st.process(&ahead[2 * a..2 * b], &mut out[2 * written..2 * (written + n)]);
        written += n;
    }
    for i in 0..outl.min(total - outl) {
        for c in 0..2 {
            out[2 * (i + outl) + c] -= out[2 * (outl - 1 - i) + c];
        }
    }
    out.copy_within(2 * outl.., 0);
    st.flush(&mut out[2 * (total - outl)..]);
    Stereo::from_interleaved(&out)
}

/// `x` through `parts`, each varispeeded to its length, as a turntable would
fn varispeeded(x: &Stereo, parts: &[(usize, usize, usize)]) -> Stereo {
    let mut out = Stereo::zeros(parts.iter().map(|p| p.2).sum());
    let mut at = 0usize;
    for &(a, b, n) in parts {
        let piece = x.slice(a, b);
        let step = piece.len() as f64 / n.max(1) as f64;
        let p = Stereo { l: resample_step(&piece.l, step, n), r: resample_step(&piece.r, step, n) };
        out.add_at(&p, at as i64, 1.0, 1.0);
        at += n;
    }
    out
}

/// `x` through `parts`, each varispeeded to its length, as a turntable would
#[cfg(not(feature = "native"))]
fn stretched(x: &Stereo, parts: &[(usize, usize, usize)]) -> Stereo {
    varispeeded(x, parts)
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
    fn a_band_speeding_up_is_flattened_to_one_tempo_bar_by_bar() {
        // four bars of a click on each beat, 2.6, 2.5, 2.4 and 2.3 s long, after half a second of silence
        let lens = [2.6, 2.5, 2.4, 2.3];
        let mut starts = vec![0.5];
        for l in lens {
            starts.push(starts.last().unwrap() + l);
        }
        let mut x = Stereo::zeros(((starts[4] + 0.5) * SRF) as usize);
        for b in 0..4 {
            for k in 0..4 {
                let at = ((starts[b] + k as f64 * lens[b] / 4.0) * SRF) as usize;
                for i in 0..48 {
                    x.l[at + i] = 0.8;
                    x.r[at + i] = 0.8;
                }
            }
        }
        let flat = flatten(&x, &starts, 2.45);
        let bar = (2.45 * SRF).round() as usize;
        assert_eq!(flat.len(), (0.5 * SRF) as usize + 4 * bar + x.len() - (starts[4] * SRF).round() as usize);
        // each click where a grid of 2.45 s bars puts it, within 3 ms
        let loud: Vec<usize> =
            (1..flat.len()).filter(|&i| flat.l[i].abs() > 0.3 && flat.l[i - 1].abs() <= 0.3).collect();
        let mut onsets: Vec<usize> = Vec::new();
        for i in loud {
            if onsets.last().is_none_or(|&o| i - o > (0.1 * SRF) as usize) {
                onsets.push(i);
            }
        }
        assert_eq!(onsets.len(), 16, "{onsets:?}");
        for (k, &o) in onsets.iter().enumerate() {
            let want = 0.5 + k as f64 * 2.45 / 4.0;
            assert!(
                (o as f64 / SRF - want).abs() < 0.003,
                "click {k} at {:.4} s, the grid's at {want:.4}",
                o as f64 / SRF
            );
        }
        // bars as long as one another are left as they were
        let even = [0.5, 2.95, 5.4];
        assert_eq!(flatten(&x, &even, 2.45).l, x.l);
    }

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
