//! One-shots cut from recordings: slice at onsets, score every hit (level, clearance before the next hit, no tail
//! of the previous one bleeding in, decay that suits the lane) and keep the best, trimmed to its decay.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::analysis::onsets;
use crate::audio::{self, Stereo};
use crate::config::{self, SRF};
use crate::rng::Rng;

/// (minimum, maximum) seconds a hit of this lane should last, and whether longer decays are better
fn shape(lane: &str) -> (f64, f64, bool) {
    match lane {
        "kick" => (0.15, 0.9, true),
        "snare" | "clap" => (0.1, 0.6, true),
        "hat" => (0.03, 0.25, false),
        "ohat" => (0.15, 0.9, true),
        "ride" | "crash" => (0.6, 3.5, true),
        "shaker" => (0.04, 0.3, false),
        _ => (0.05, 0.6, true),
    }
}

fn env_db(x: &[f32], a: usize, b: usize) -> f32 {
    if b <= a {
        return -120.0;
    }
    let ms = x[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32;
    10.0 * ms.max(1e-12).log10()
}

/// the best hit of `lane` inside a recording, or None when nothing clean is found
pub fn cut(x: &Stereo, lane: &str, rng: &mut Rng) -> Option<Stereo> {
    let mono = x.mono();
    let n = mono.len();
    let ons = onsets::detect(&mono, 0.06, 0.03);
    if ons.is_empty() {
        return None;
    }
    let (min_s, max_s, longer_better) = shape(lane);
    let ms = |s: f64| (s * SRF) as usize;
    let w = ms(0.005);
    let mut scored: Vec<(f32, usize, usize)> = Vec::new();
    for (i, &o) in ons.iter().enumerate() {
        let next = ons.get(i + 1).copied().unwrap_or(n);
        let room = next.saturating_sub(o).saturating_sub(ms(0.003));
        if room < ms(min_s) {
            continue;
        }
        let peak = env_db(&mono, o, (o + ms(0.03)).min(n));
        if peak < -45.0 {
            continue;
        }
        // decay end: the 5 ms envelope 40 dB under the peak, or the next hit, or the lane's maximum
        let limit = (o + room).min(o + ms(max_s));
        let mut end = limit;
        let mut s = o + ms(0.01);
        while s + w < limit {
            if env_db(&mono, s, s + w) < peak - 40.0 {
                end = s + w;
                break;
            }
            s += w;
        }
        let before = if o > ms(0.02) { env_db(&mono, o - ms(0.02), o - ms(0.005)) } else { -120.0 };
        let bleed = (before - (peak - 30.0)).max(0.0);
        let len_s = (end - o) as f64 / SRF;
        let len_score = if longer_better {
            (len_s / max_s).min(1.0) as f32 * 6.0
        } else {
            (1.0 - len_s / max_s).max(0.0) as f32 * 6.0
        };
        let cut_off = if end == next.saturating_sub(ms(0.003)) && longer_better { 3.0 } else { 0.0 };
        scored.push((peak + len_score - bleed - cut_off + rng.f32() * 1.5, o, end));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let (_, a, b) = *scored.first()?;
    let pre = ms(0.0015);
    let mut s = x.slice(a.saturating_sub(pre), b);
    let len = s.len();
    s.fade(pre.min(len), ms(0.012).min(len / 2));
    s.normalize(-1.0);
    Some(s)
}

/// cut (once) and cache the best `lane` hit of a recording; returns the WAV path
pub fn cached(path: &Path, lane: &str, seed: u64) -> Result<Option<PathBuf>> {
    let key = blake3::hash(format!("{}|{lane}|{seed}|v1", path.display()).as_bytes()).to_hex()[..20].to_string();
    let out = config::cache_dir("samples").join(format!("{lane}-{key}.wav"));
    if out.exists() {
        return Ok(Some(out));
    }
    let x = audio::load(path)?;
    let mut rng = Rng::new(seed);
    match cut(&x, lane, &mut rng) {
        Some(s) => {
            audio::save_wav(&out, &s, true)?;
            Ok(Some(out))
        }
        None => Ok(None),
    }
}
