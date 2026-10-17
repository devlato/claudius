//! Objective measurements (earshot/features.py): what a meter, a spectrum analyser and an oscilloscope tell you.
//!
//! Every field of features.measure. The librosa parts (onsets, HPSS harmonic ratio, yin f0, tempo / beats,
//! chroma key) are ports of librosa 1.0 running on a 22.05 kHz copy made by the soxr-equivalent resampler.

use serde::Serialize;

use crate::dsp::filter::{lfilter, maximum_filter1d_nearest};
use crate::dsp::spectral::{stft_mag, upsample4_peak, welch};
use crate::dsp::stats::{self, percentile};
use crate::dsp::{db, resample, round_to};
use crate::librosa;

const SR: usize = crate::SR as usize;
const SRF: f64 = crate::SR as f64;

pub const BANDS: [(&str, f64, f64); 7] = [
    ("sub", 20.0, 60.0),
    ("low", 60.0, 250.0),
    ("lowmid", 250.0, 1000.0),
    ("mid", 1000.0, 4000.0),
    ("presence", 4000.0, 8000.0),
    ("air", 8000.0, 16000.0),
    ("top", 16000.0, 24000.0),
];
const KK_MAJ: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const KK_MIN: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
pub const NOTE: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/// energy share per band in dB (features.BANDS order)
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Bands {
    pub sub: f64,
    pub low: f64,
    pub lowmid: f64,
    pub mid: f64,
    pub presence: f64,
    pub air: f64,
    pub top: f64,
}

impl Bands {
    pub fn get(&self, k: &str) -> Option<f64> {
        Some(match k {
            "sub" => self.sub,
            "low" => self.low,
            "lowmid" => self.lowmid,
            "mid" => self.mid,
            "presence" => self.presence,
            "air" => self.air,
            "top" => self.top,
            _ => return None,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, f64)> + '_ {
        BANDS.iter().map(move |(k, _, _)| (*k, self.get(k).unwrap()))
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KeyGuess {
    pub key: String,
    pub r: f64,
}

/// every field of earshot's features.measure (None = the key Python leaves out or sets to None)
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Measure {
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_dbfs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub true_peak_dbtp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rms_dbfs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lufs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_term_max_lufs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lra_lu: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipped_samples: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noise_floor_dbfs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_range_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digital_silence_frac: Option<f64>,
    /// None for mono-identical channels (Python keeps the key with None)
    pub side_to_mid_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attack_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_at: Option<f64>,
    pub attack_ms: Option<f64>,
    pub decay_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub centroid_hz: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rolloff95_hz: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bands_db: Option<Bands>,
    pub bandwidth_hz: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flatness: Option<f64>,
    pub brightness_hz: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tonal_peak_ratio_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmonic_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f0_hz: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onsets: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onset_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onsets_per_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beats: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat_ibi_std_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Vec<KeyGuess>>,
}

// ------------------------------------------------------------------------------------------------ loudness
const KW_B1: [f64; 3] = [1.53512485958697, -2.69169618940638, 1.19839281085285];
const KW_A1: [f64; 3] = [1.0, -1.69065929318241, 0.73248077421585];
const KW_B2: [f64; 3] = [1.0, -2.0, 1.0];
const KW_A2: [f64; 3] = [1.0, -1.99004745483398, 0.99007225036621];

/// BS.1770 K-weighting at 48 kHz
pub fn k_weight(x: &[f32]) -> Vec<f64> {
    let x64: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    lfilter(&KW_B2, &KW_A2, &lfilter(&KW_B1, &KW_A1, &x64))
}

fn cumsum_sq(y: &[f64]) -> Vec<f64> {
    let mut p = Vec::with_capacity(y.len() + 1);
    let mut acc = 0.0;
    p.push(0.0);
    for v in y {
        acc += v * v;
        p.push(acc);
    }
    p
}

/// (t_mid, LUFS) of sliding blocks: win 0.4 = momentary, 3.0 = short-term
pub fn loudness_curve(l: &[f32], r: &[f32], win: f64, hop: f64) -> (Vec<f64>, Vec<f64>) {
    let pl = cumsum_sq(&k_weight(l));
    let pr = cumsum_sq(&k_weight(r));
    let n = l.len();
    let (w, h) = ((win * SRF) as usize, (hop * SRF) as usize);
    if n < w {
        let d = n.max(1) as f64;
        let ms = (pl[n] - pl[0]) / d + (pr[n] - pr[0]) / d;
        return (vec![n as f64 / SRF / 2.0], vec![-0.691 + 10.0 * (ms + 1e-20).log10()]);
    }
    let mut t = Vec::new();
    let mut lv = Vec::new();
    let mut s = 0;
    while s + w <= n {
        let ms = (pl[s + w] - pl[s]) / w as f64 + (pr[s + w] - pr[s]) / w as f64;
        t.push((s as f64 + w as f64 / 2.0) / SRF);
        lv.push(-0.691 + 10.0 * (ms + 1e-20).log10());
        s += h;
    }
    (t, lv)
}

fn power_mean_db(v: &[f64]) -> f64 {
    let p: Vec<f64> = v.iter().map(|x| 10f64.powf(x / 10.0)).collect();
    10.0 * stats::mean(&p).log10()
}

/// gated integrated loudness (LUFS)
pub fn integrated_lufs(l: &[f32], r: &[f32]) -> f64 {
    let (_, lv) = loudness_curve(l, r, 0.4, 0.1);
    let g: Vec<f64> = lv.into_iter().filter(|v| *v > -70.0).collect();
    if g.is_empty() {
        return -70.0;
    }
    let rel = power_mean_db(&g) - 10.0;
    let g2: Vec<f64> = g.into_iter().filter(|v| *v > rel).collect();
    if g2.is_empty() { rel + 10.0 } else { power_mean_db(&g2) }
}

/// loudness range (LU) of the 3 s short-term loudness, None when too short
pub fn lra(l: &[f32], r: &[f32]) -> Option<f64> {
    let (_, s) = loudness_curve(l, r, 3.0, 0.1);
    let s: Vec<f64> = s.into_iter().filter(|v| *v > -70.0).collect();
    if s.len() < 5 {
        return None;
    }
    let rel = power_mean_db(&s) - 20.0;
    let s: Vec<f64> = s.into_iter().filter(|v| *v > rel).collect();
    if s.len() > 4 { Some(percentile(&s, 95.0) - percentile(&s, 10.0)) } else { None }
}

/// true peak (dBTP, 4x oversampled like scipy resample_poly)
pub fn true_peak_dbtp(l: &[f32], r: &[f32]) -> f64 {
    db(upsample4_peak(l).max(upsample4_peak(r)))
}

// ------------------------------------------------------------------------------------------------ envelopes
/// RMS per non-overlapping frame of `win` samples (features.frame_rms)
pub fn frame_rms(m: &[f32], win: usize) -> Vec<f64> {
    let n = m.len() / win.max(1);
    if n == 0 {
        let sq: Vec<f64> = m.iter().map(|&v| (v as f64) * (v as f64)).collect();
        let mean = if sq.is_empty() { f64::NAN } else { stats::mean(&sq) };
        return vec![(mean + 1e-20).sqrt()];
    }
    let mut buf = vec![0.0; win];
    (0..n)
        .map(|i| {
            for (k, v) in m[i * win..(i + 1) * win].iter().enumerate() {
                buf[k] = (*v as f64) * (*v as f64);
            }
            (stats::mean(&buf) + 1e-20).sqrt()
        })
        .collect()
}

/// RMS envelope, one value per millisecond, each over a centred `win_ms` window
pub fn smooth_env(m: &[f32], win_ms: f64) -> Vec<f64> {
    let n = m.len();
    let h = SR / 1000;
    let w2 = ((win_ms * SRF / 2000.0) as usize).max(1);
    let mut c = Vec::with_capacity(n + 1);
    c.push(0.0f64);
    let mut acc = 0.0;
    for &v in m {
        acc += (v as f64) * (v as f64);
        c.push(acc);
    }
    let mut out = Vec::new();
    let mut s = 0;
    while s < n {
        let mid = s + h / 2;
        let a = mid.saturating_sub(w2).min(n);
        let b = (mid + w2).min(n);
        out.push(((c[b] - c[a]) / (b - a).max(1) as f64 + 1e-20).sqrt());
        s += h;
    }
    out
}

pub struct EnvTimes {
    pub attack_at: f64,
    pub peak_at: f64,
    pub attack_ms: Option<f64>,
    pub decay_ms: Option<f64>,
}

/// attack_at / peak_at (s), attack_ms and decay_ms (features.envelope_times)
pub fn envelope_times(m: &[f32]) -> EnvTimes {
    let e5 = smooth_env(m, 5.0);
    let e10 = smooth_env(m, 10.0);
    let mx5 = stats::max(&e5);
    let i0 = e5.iter().position(|&v| v >= mx5 * 0.1).unwrap_or(0);
    let loc = &e5[i0..(i0 + 300).min(e5.len())];
    let ip = stats::argmax(&e10);
    let after = &e10[ip..];
    let ahead = maximum_filter1d_nearest(after, 21, -10);
    let lim = e10[ip] * 10f64.powf(-30.0 / 20.0);
    let below = ahead.iter().position(|&v| v < lim);
    let attack_ms = if loc.is_empty() {
        None
    } else {
        let lm = stats::max(loc);
        Some(loc.iter().position(|&v| v >= 0.9 * lm).unwrap_or(0) as f64)
    };
    EnvTimes {
        attack_at: round_to(i0 as f64 / 1000.0, 3),
        peak_at: round_to(ip as f64 / 1000.0, 3),
        attack_ms,
        decay_ms: below.map(|b| b as f64),
    }
}

// ------------------------------------------------------------------------------------------------ spectrum
/// median magnitude-weighted spectral centroid over frames within 30 dB of the loudest (perceived brightness)
pub fn brightness_hz(m: &[f32]) -> Option<i64> {
    if m.len() < 1024 {
        return None;
    }
    let x: Vec<f64> = m.iter().map(|&v| v as f64).collect();
    let frames = stft_mag(&x, 1024, 768);
    let f = crate::dsp::rfftfreq(1024, SRF);
    let e: Vec<f64> = frames.iter().map(|fr| fr.iter().sum()).collect();
    let thr = stats::max(&e) * 10f64.powf(-30.0 / 20.0);
    let c: Vec<f64> = frames
        .iter()
        .zip(&e)
        .filter(|(_, e)| **e >= thr)
        .map(|(fr, _)| {
            let num: f64 = fr.iter().zip(&f).map(|(a, f)| a * f).sum();
            let den: f64 = fr.iter().sum();
            num / (den + 1e-20)
        })
        .collect();
    if c.is_empty() {
        return None;
    }
    Some(stats::median(&c) as i64)
}

/// Krumhansl-Kessler key estimate from the mean chroma: the 3 best keys with their correlation
pub fn key_estimate(m: &[f32]) -> Vec<KeyGuess> {
    let y: Vec<f64> = resample(m, crate::SR, librosa::SR22).into_iter().map(|v| v as f64).collect();
    let chroma = librosa::chroma_cqt(&y, librosa::SR22 as f64, 512);
    let c: Vec<f64> = chroma.iter().map(|row| stats::mean(row)).collect();
    let mut best: Vec<(f64, String)> = Vec::new();
    for (mode, prof) in [("major", KK_MAJ), ("minor", KK_MIN)] {
        for k in 0..12 {
            let rolled: Vec<f64> = (0..12).map(|i| prof[(i + 12 - k) % 12]).collect();
            best.push((stats::corrcoef(&c, &rolled), format!("{} {}", NOTE[k], mode)));
        }
    }
    best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| b.1.cmp(&a.1)));
    best.into_iter().take(3).map(|(r, key)| KeyGuess { key, r: round_to(r, 3) }).collect()
}

/// onset times (s) of a mono 48 kHz array: librosa onset_detect at 22.05 kHz, hop 256, backtracked
pub fn onsets(m: &[f32]) -> Vec<f64> {
    onsets_delta(m, 0.07)
}

pub fn onsets_delta(m: &[f32], delta: f64) -> Vec<f64> {
    let y: Vec<f64> = resample(m, crate::SR, librosa::SR22).into_iter().map(|v| v as f64).collect();
    librosa::onset_detect_times(&y, librosa::SR22 as f64, 256, delta).into_iter().map(|t| round_to(t, 3)).collect()
}

fn pairwise_f32(v: &[f32]) -> f32 {
    // numpy's float32 pairwise summation
    const BLOCK: usize = 128;
    let n = v.len();
    if n < 8 {
        v.iter().fold(0.0f32, |a, b| a + b)
    } else if n <= BLOCK {
        let mut r = [0.0f32; 8];
        r.copy_from_slice(&v[..8]);
        let mut i = 8;
        while i + 8 <= n {
            for k in 0..8 {
                r[k] += v[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += v[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_f32(&v[..n2]) + pairwise_f32(&v[n2..])
    }
}

fn mean_f32(v: &[f32]) -> f32 {
    if v.is_empty() { f32::NAN } else { pairwise_f32(v) / v.len() as f32 }
}

/// objective measurements of a 48 kHz stereo array; light = skip the slow parts (onsets, HPSS, pitch); music =
/// also tempo, beats and key (files of 5 s or more)
pub fn measure(l: &[f32], r: &[f32], music: bool, light: bool) -> Measure {
    let n = l.len().min(r.len());
    let (l, r) = (&l[..n], &r[..n]);
    let m: Vec<f32> = crate::audio::mean_mono(l, r);
    let mut res = Measure { duration: round_to(n as f64 / SRF, 3), ..Default::default() };
    if n < (0.005 * SRF) as usize {
        res.error = Some("too short to measure".into());
        return res;
    }
    let pk = l.iter().chain(r).fold(0.0f32, |a, v| a.max(v.abs())) as f64;
    res.peak_dbfs = Some(round_to(db(pk), 1));
    res.true_peak_dbtp = Some(round_to(true_peak_dbtp(l, r), 1));
    let sq: Vec<f64> = l.iter().zip(r).flat_map(|(a, b)| [(*a as f64).powi(2), (*b as f64).powi(2)]).collect();
    res.rms_dbfs = Some(round_to(db(stats::mean(&sq).sqrt()), 1));
    if n >= (0.4 * SRF) as usize {
        res.lufs = Some(round_to(integrated_lufs(l, r), 1));
        if n >= 3 * SR {
            let (_, s) = loudness_curve(l, r, 3.0, 0.5);
            res.short_term_max_lufs = Some(round_to(stats::max(&s), 1));
        }
        if n >= 6 * SR
            && let Some(v) = lra(l, r)
        {
            res.lra_lu = Some(round_to(v, 1));
        }
    }
    res.clipped_samples = Some(l.iter().chain(r).filter(|v| v.abs() >= 0.999).count() as u64);
    res.dc = Some(round_to(mean_f32(&m) as f64, 5));
    let r20 = frame_rms(&m, (0.02 * SRF) as usize);
    let nf = round_to(db(percentile(&r20, 10.0)), 1);
    res.noise_floor_dbfs = Some(nf);
    res.dynamic_range_db = Some(round_to(db(stats::max(&r20)) - nf, 1));
    res.digital_silence_frac = Some(round_to(m.iter().filter(|v| v.abs() < 1e-6).count() as f64 / n as f64, 4));
    let mid: Vec<f32> = l.iter().zip(r).map(|(a, b)| ((a + b) / 2.0) * ((a + b) / 2.0)).collect();
    let side: Vec<f32> = l.iter().zip(r).map(|(a, b)| ((a - b) / 2.0) * ((a - b) / 2.0)).collect();
    let sm = mean_f32(&side).sqrt();
    res.side_to_mid_db = if sm as f64 > 1e-7 {
        Some(round_to(db(sm as f64) - db((mean_f32(&mid).sqrt() + 1e-12) as f64), 1))
    } else {
        None
    };
    let l64: Vec<f64> = l.iter().map(|&v| v as f64).collect();
    let r64: Vec<f64> = r.iter().map(|&v| v as f64).collect();
    if stats::std(&l64) > 1e-9 && stats::std(&r64) > 1e-9 {
        res.correlation = Some(round_to(stats::corrcoef(&l64, &r64), 3));
    }
    let et = envelope_times(&m);
    res.attack_at = Some(et.attack_at);
    res.peak_at = Some(et.peak_at);
    res.attack_ms = et.attack_ms;
    res.decay_ms = et.decay_ms;
    // whole-file spectrum
    let seg: Vec<f64> = m.iter().map(|&v| v as f64).collect();
    let lg = (1024.max(seg.len().min(65536)) as f64).log2().ceil() as u32;
    let nfft = 1usize << lg.clamp(10, 16);
    let (f, mut p) = welch(&seg, SRF, nfft.min(seg.len()));
    for v in p.iter_mut() {
        *v += 1e-30;
    }
    let tot = stats::pairwise_sum(&p);
    let fp: Vec<f64> = f.iter().zip(&p).map(|(a, b)| a * b).collect();
    res.centroid_hz = Some((stats::pairwise_sum(&fp) / tot) as i64);
    let mut cum = 0.0;
    let mut ri = p.len();
    for (i, v) in p.iter().enumerate() {
        cum += v;
        if cum / tot >= 0.95 {
            ri = i;
            break;
        }
    }
    res.rolloff95_hz = Some(f[ri.min(f.len() - 1)] as i64);
    let band = |a: f64, b: f64| -> f64 {
        let s: Vec<f64> = f.iter().zip(&p).filter(|(fr, _)| **fr >= a && **fr < b).map(|(_, v)| *v).collect();
        round_to(10.0 * (stats::pairwise_sum(&s) / tot + 1e-12).log10(), 1)
    };
    res.bands_db = Some(Bands {
        sub: band(20.0, 60.0),
        low: band(60.0, 250.0),
        lowmid: band(250.0, 1000.0),
        mid: band(1000.0, 4000.0),
        presence: band(4000.0, 8000.0),
        air: band(8000.0, 16000.0),
        top: band(16000.0, 24000.0),
    });
    let edges: Vec<f64> = (0..31).map(|i| 20.0 * 2f64.powf(i as f64 / 3.0)).collect();
    let lv: Vec<f64> = edges
        .windows(2)
        .map(|e| {
            let s: Vec<f64> = f.iter().zip(&p).filter(|(fr, _)| **fr >= e[0] && **fr < e[1]).map(|(_, v)| *v).collect();
            if s.is_empty() { -300.0 } else { 10.0 * stats::mean(&s).log10() }
        })
        .collect();
    let lmax = stats::max(&lv);
    res.bandwidth_hz = lv.iter().rposition(|&v| v >= lmax - 60.0).map(|i| edges[i + 1] as i64);
    let logs: Vec<f64> = p.iter().map(|v| v.ln()).collect();
    res.flatness = Some(round_to(stats::mean(&logs).exp() / stats::mean(&p), 4));
    res.brightness_hz = brightness_hz(&m);
    res.tonal_peak_ratio_db = Some(round_to(10.0 * (stats::max(&p) / stats::median(&p)).log10(), 1));
    if light {
        return res;
    }
    let y22: Vec<f64> = resample(&m, crate::SR, librosa::SR22).into_iter().map(|v| v as f64).collect();
    let yt = &y22[..y22.len().min(30 * librosa::SR22 as usize)];
    if yt.len() >= 2048 {
        let (eh, ep) = librosa::hpss_energies(yt);
        let hr = round_to(eh / (eh + ep + 1e-20), 3);
        res.harmonic_ratio = Some(hr);
        let f0 = librosa::yin(yt, 50.0, 2000.0, librosa::SR22 as f64, 2048);
        let rms = librosa::rms(yt, 2048, 512);
        let k = rms.len().min(f0.len());
        let rmax = stats::max(&rms[..k]);
        let voiced: Vec<f64> = (0..k).filter(|&i| rms[i] > rmax * 0.1).map(|i| f0[i]).collect();
        if !voiced.is_empty() && hr >= 0.8 {
            res.f0_hz = Some(round_to(stats::median(&voiced), 1));
        }
    }
    let on: Vec<f64> = librosa::onset_detect_times(&y22, librosa::SR22 as f64, 256, 0.07)
        .into_iter()
        .map(|t| round_to(t, 3))
        .collect();
    res.onset_count = Some(on.len());
    res.onsets_per_s = Some(round_to(on.len() as f64 / (n as f64 / SRF), 2));
    res.onsets = Some(on.into_iter().take(200).collect());
    if music && n >= 5 * SR {
        let oenv = librosa::onset_strength(&y22, librosa::SR22 as f64, 256);
        let (tempo, beats) = librosa::beat_track(&oenv, librosa::SR22 as f64, 256);
        res.tempo = Some(round_to(tempo, 1));
        if beats.len() > 4 {
            let d: Vec<f64> = beats.windows(2).map(|w| w[1] - w[0]).collect();
            res.beat_ibi_std_ms = Some(round_to(stats::std(&d) * 1000.0, 1));
        }
        res.beats = Some(beats.iter().map(|b| round_to(*b, 3)).collect());
        res.key = Some(key_estimate(&m));
    }
    res
}

/// earshot's one-line text summary of a measurement
pub fn text(r: &Measure, name: &str) -> String {
    if let Some(e) = &r.error {
        return format!("{name}: {e}");
    }
    let pk = r.peak_dbfs.unwrap_or(0.0);
    if pk < -70.0 {
        let what = if r.digital_silence_frac.unwrap_or(0.0) >= 0.999 { "digitally silent" } else { "near-silent" };
        return format!(
            "{name}: {:.3} s, {what} (peak {pk:.0} dBFS): nothing to measure; the loudness, spectrum and envelope values of silence are meaningless.",
            r.duration
        );
    }
    let mut parts = vec![format!(
        "{name}: {:.3} s, peak {} dBFS (true peak {} dBTP)",
        r.duration,
        pyf(pk),
        pyf(r.true_peak_dbtp.unwrap_or(0.0))
    )];
    if let Some(l) = r.lufs {
        parts.push(format!("{} LUFS", pyf(l)) + &r.lra_lu.map(|v| format!(", LRA {} LU", pyf(v))).unwrap_or_default());
    }
    let mut env = r.attack_ms.map(|a| format!("attack {} ms", pyg(a))).unwrap_or_default();
    if !env.is_empty() {
        env.push_str(", ");
    }
    env.push_str(&match r.decay_ms {
        Some(d) => format!("decay {} ms", pyg(d)),
        None => "no -30 dB decay (sustained or cut off)".into(),
    });
    parts.push(format!(
        "noise floor {} dBFS, {env}, brightness {} Hz, bandwidth {} Hz",
        pyf(r.noise_floor_dbfs.unwrap_or(0.0)),
        r.brightness_hz.map_or("None".into(), |v| v.to_string()),
        r.bandwidth_hz.map_or("None".into(), |v| v.to_string())
    ));
    if let Some(c) = r.clipped_samples.filter(|c| *c > 0) {
        parts.push(format!("{c} clipped samples"));
    }
    if let Some(t) = r.tempo.filter(|t| *t != 0.0) {
        let k = r.key.as_ref().and_then(|k| k.first()).map_or("?".to_string(), |k| k.key.clone());
        parts.push(format!("tempo {} BPM, key {k}", pyf(t)));
    }
    if let Some(b) = &r.bands_db {
        parts.push(
            "bands ".to_string()
                + &b.iter().map(|(k, v)| format!("{k} {}", pyf(v))).collect::<Vec<_>>().join(" ")
                + " dB",
        );
    }
    parts.join("; ") + "."
}

/// Python str() of a float (shortest repr, at least one decimal)
pub fn pyf(v: f64) -> String {
    if v.is_finite() && v == v.trunc() && v.abs() < 1e16 { format!("{v:.1}") } else { format!("{v}") }
}

/// Python '{:g}' of a float
pub fn pyg(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e6 { format!("{}", v as i64) } else { format!("{v}") }
}
