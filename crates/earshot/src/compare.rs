//! Compare sounds with each other and with reference sets (earshot/compare.py): trim-proof measurements
//! (feats), typicality vs a reference set and A/B differences in words.

use serde::Serialize;

use crate::dsp::round_to;
use crate::features::{self, Measure};

const SRF: f64 = crate::SR as f64;
pub const MONO_DB: f64 = -40.0;
pub const CLOSE_DZ: f64 = 0.3;
const LOG: [&str; 5] = ["brightness_hz", "bandwidth_hz", "attack_ms", "decay_ms", "length_s"];
/// (key, threshold, kind) in order of how much they matter to the ear
pub const DIFFS: [(&str, f64, &str); 9] = [
    ("loudness_lufs", 1.5, "db"),
    ("brightness_hz", 1.15, "ratio"),
    ("attack_ms", 1.5, "ratio"),
    ("decay_ms", 1.5, "ratio"),
    ("low_end_db", 3.0, "db"),
    ("length_s", 1.25, "ratio"),
    ("noise_floor_dbfs", 6.0, "db"),
    ("width_db", 6.0, "db"),
    ("bandwidth_hz", 1.4, "ratio"),
];

fn words(k: &str) -> (&'static str, &'static str) {
    match k {
        "loudness_lufs" => ("louder", "quieter"),
        "brightness_hz" => ("brighter", "darker"),
        "bandwidth_hz" => ("more top end", "less top end"),
        "attack_ms" => ("a softer attack", "a sharper attack"),
        "decay_ms" => ("a longer tail", "a shorter tail"),
        "noise_floor_dbfs" => ("noisier", "cleaner"),
        "length_s" => ("longer", "shorter"),
        "low_end_db" => ("more low end", "less low end"),
        "width_db" => ("wider", "narrower"),
        "peak_dbfs" => ("hotter peaks", "lower peaks"),
        _ => ("more", "less"),
    }
}

fn label(k: &str) -> Option<&'static str> {
    Some(match k {
        "brightness_hz" => "brightness",
        "bandwidth_hz" => "bandwidth",
        "noise_floor_dbfs" => "noise floor",
        "width_db" => "side/mid",
        "low_end_db" => "energy below 250 Hz",
        "loudness_lufs" => "loudness",
        "length_s" => "sounding length",
        _ => return None,
    })
}

const WITH: [&str; 4] = ["attack_ms", "decay_ms", "low_end_db", "bandwidth_hz"];

/// the comparable, trim-proof measurements of a sound (compare.feats)
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Feats {
    pub loudness_lufs: Option<f64>,
    pub peak_dbfs: Option<f64>,
    pub brightness_hz: Option<f64>,
    pub bandwidth_hz: Option<f64>,
    pub attack_ms: Option<f64>,
    pub decay_ms: Option<f64>,
    pub length_s: Option<f64>,
    pub duration: Option<f64>,
    pub noise_floor_dbfs: Option<f64>,
    pub width_db: Option<f64>,
    pub low_end_db: Option<f64>,
}

impl Feats {
    pub fn get(&self, k: &str) -> Option<f64> {
        match k {
            "loudness_lufs" => self.loudness_lufs,
            "peak_dbfs" => self.peak_dbfs,
            "brightness_hz" => self.brightness_hz,
            "bandwidth_hz" => self.bandwidth_hz,
            "attack_ms" => self.attack_ms,
            "decay_ms" => self.decay_ms,
            "length_s" => self.length_s,
            "duration" => self.duration,
            "noise_floor_dbfs" => self.noise_floor_dbfs,
            "width_db" => self.width_db,
            "low_end_db" => self.low_end_db,
            _ => None,
        }
    }
}

/// gated integrated loudness of the sound padded to >= 0.4 s (leading / trailing silence does not change it)
pub fn active_lufs(l: &[f32], r: &[f32]) -> f64 {
    let n = (0.4 * SRF) as usize;
    if l.len() < n {
        let mut l2 = l.to_vec();
        let mut r2 = r.to_vec();
        l2.resize(n, 0.0);
        r2.resize(n, 0.0);
        return round_to(features::integrated_lufs(&l2, &r2), 1);
    }
    round_to(features::integrated_lufs(l, r), 1)
}

/// seconds from the first to the last 10 ms frame within floor_db of the loudest frame
pub fn active_length(mono: &[f32], floor_db: f64) -> f64 {
    let e = features::frame_rms(mono, (0.01 * SRF) as usize);
    let emax = 20.0 * crate::dsp::stats::max(&e).log10();
    let on: Vec<usize> = (0..e.len()).filter(|&i| 20.0 * e[i].log10() >= emax - floor_db).collect();
    let (a, b) = (on[0], on[on.len() - 1]);
    round_to((mono.len() as f64 / SRF).min((b - a + 1) as f64 * 0.01), 3)
}

/// compare.feats from a features.measure result and its stereo audio
pub fn feats(m: &Measure, l: &[f32], r: &[f32]) -> Feats {
    let mono = crate::audio::mean_mono(l, r);
    let nf = m.noise_floor_dbfs;
    let mut f = Feats {
        loudness_lufs: Some(active_lufs(l, r)),
        peak_dbfs: m.peak_dbfs,
        brightness_hz: m.brightness_hz.map(|v| v as f64),
        bandwidth_hz: m.bandwidth_hz.map(|v| v as f64),
        attack_ms: m.attack_ms,
        decay_ms: m.decay_ms,
        length_s: if mono.is_empty() { None } else { Some(active_length(&mono, 40.0)) },
        duration: Some(m.duration),
        noise_floor_dbfs: nf.filter(|_| m.duration >= 1.0).map(|v| v.max(-120.0)),
        width_db: Some(m.side_to_mid_db.map_or(MONO_DB, |v| v.max(MONO_DB))),
        low_end_db: None,
    };
    if let Some(b) = &m.bands_db {
        f.low_end_db = Some(round_to(10.0 * (10f64.powf(b.sub / 10.0) + 10f64.powf(b.low / 10.0)).log10(), 1));
    }
    f
}

/// compare.measure_path on in-memory stereo audio: feats(features.measure(x, light=True), x)
pub fn measure_feats(l: &[f32], r: &[f32]) -> Feats {
    feats(&features::measure(l, r, false, true), l, r)
}

pub fn log_value(k: &str, v: f64) -> f64 {
    if LOG.contains(&k) { v.max(1e-3).ln() } else { v }
}

fn hz(f: f64) -> String {
    if f >= 1000.0 { format!("{:.1} kHz", f / 1000.0) } else { format!("{f:.0} Hz") }
}

fn ms(v: f64) -> String {
    if v >= 2000.0 { format!("{:.1} s", v / 1000.0) } else { format!("{v:.0} ms") }
}

pub fn fmt(k: &str, v: f64) -> String {
    match k {
        "brightness_hz" | "bandwidth_hz" => hz(v),
        "attack_ms" | "decay_ms" => ms(v),
        "length_s" | "duration" => format!("{v:.2} s"),
        "width_db" if v <= MONO_DB => "mono".into(),
        _ => format!(
            "{v:.0} {}",
            match k {
                "loudness_lufs" => "LUFS",
                "noise_floor_dbfs" | "peak_dbfs" => "dBFS",
                _ => "dB",
            }
        ),
    }
}

pub fn phrase(k: &str, up: bool, a_txt: &str, b_txt: &str) -> String {
    let w = words(k);
    format!(
        "{}{} ({}{b_txt} vs {a_txt})",
        if WITH.contains(&k) { "with " } else { "" },
        if up { w.0 } else { w.1 },
        label(k).map(|l| format!("{l} ")).unwrap_or_default()
    )
}

/// how B differs from A in words (the `limit` most important differences big enough to hear): (keys, text)
pub fn differences(fa: &Feats, fb: &Feats, limit: usize) -> (Vec<String>, String) {
    let mut out: Vec<(&str, String)> = Vec::new();
    for (k, thr, kind) in DIFFS {
        let (Some(a), Some(b)) = (fa.get(k), fb.get(k)) else { continue };
        if kind == "db" {
            if (b - a).abs() < thr {
                continue;
            }
            let txt = if k == "loudness_lufs" {
                format!("{:.0} dB {}", (b - a).abs(), if b > a { words(k).0 } else { words(k).1 })
            } else {
                phrase(k, b > a, &fmt(k, a), &fmt(k, b))
            };
            out.push((k, txt));
        } else if a.min(b) > 0.0 && a.max(b) / a.min(b) >= thr && !(k == "attack_ms" && (a - b).abs() < 3.0) {
            out.push((k, phrase(k, b > a, &fmt(k, a), &fmt(k, b))));
        }
    }
    out.truncate(limit);
    let mut parts: Vec<&String> = out.iter().filter(|(_, p)| !p.starts_with("with ")).map(|(_, p)| p).collect();
    parts.extend(out.iter().filter(|(_, p)| p.starts_with("with ")).map(|(_, p)| p));
    if parts.is_empty() {
        return (
            vec![],
            "no measurable difference big enough to hear (level, brightness, envelope, low end, noise)".into(),
        );
    }
    let text = if parts.len() == 1 {
        parts[0].clone()
    } else {
        format!(
            "{} and {}",
            parts[..parts.len() - 1].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
            parts[parts.len() - 1]
        )
    };
    (out.iter().map(|(k, _)| k.to_string()).collect(), text)
}

/// how close a sound is to its k nearest references vs how close the references are to their own k nearest
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Typicality {
    pub mean_cos_knn: f64,
    pub z: f64,
    pub percentile: f64,
    pub k: usize,
    pub verdict: String,
}

pub fn fit_words(z: f64) -> &'static str {
    if z >= -1.0 {
        "fits the set"
    } else if z >= -2.0 {
        "somewhat unusual for the set"
    } else {
        "an outlier: unlike the references"
    }
}

/// (mean cos of x to its k nearest refs, z, percentile) unrounded (compare._typicality)
pub fn typicality_raw(emb: &[f32], refs: &[Vec<f32>], k: usize) -> (f64, f64, f64) {
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum::<f64>();
    let n = refs.len();
    let kk = k.min(n.saturating_sub(1)).max(1);
    let ref_s: Vec<f64> = (0..n)
        .map(|i| {
            let mut s: Vec<f64> =
                (0..n).map(|j| if i == j { f64::NEG_INFINITY } else { dot(&refs[i], &refs[j]) }).collect();
            s.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            s[..kk.min(s.len())].iter().sum::<f64>() / kk as f64
        })
        .collect();
    let mut sx: Vec<f64> = refs.iter().map(|r| dot(r, emb)).collect();
    sx.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let sx = sx[..kk.min(sx.len())].iter().sum::<f64>() / kk as f64;
    let mean = ref_s.iter().sum::<f64>() / n as f64;
    let sd = (ref_s.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n as f64).sqrt() + 1e-6;
    let pct = 100.0 * ref_s.iter().filter(|&&v| v <= sx).count() as f64 / n as f64;
    (sx, (sx - mean) / sd, pct)
}
