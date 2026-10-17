//! Tempo, beats, downbeats and pulse clarity (earshot/timeline.py music_grid with _pulse_envs, _period and
//! _downbeat_phase): autocorrelation tempo with a soft 120 BPM prior on 10 ms log-mel flux, dynamic-programming
//! beats (Ellis 2007), a 4/4 downbeat guess from harmony changes and low-band onsets.

use serde::Serialize;

use crate::dsp::fft::{C64, Irfft, Rfft};
use crate::dsp::filter::uniform_filter1d_reflect;
use crate::dsp::{hanning, rfftfreq, round_to};

const SRF: f64 = crate::SR as f64;
pub const CLEAR_PULSE: f64 = 0.30;
pub const WEAK_PULSE: f64 = 0.15;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Grid {
    pub tempo: Option<f64>,
    pub tempo_alt: Option<f64>,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub pulse_clarity: Option<f64>,
    pub beat_regularity: Option<f64>,
    pub confidence: String,
    pub note: String,
}

impl Grid {
    fn none(note: &str) -> Grid {
        Grid {
            tempo: None,
            tempo_alt: None,
            beats: vec![],
            downbeats: vec![],
            pulse_clarity: None,
            beat_regularity: None,
            confidence: "none".into(),
            note: note.into(),
        }
    }
}

#[inline]
fn f32r(v: f64) -> f64 {
    v as f32 as f64
}

/// triangular HTK-mel filterbank (n_mels x n_fft / 2 + 1), float32
fn mel_fb(n_fft: usize, n_mels: usize, fmin: f64, fmax: f64) -> Vec<Vec<f64>> {
    let mel = |f: f64| 2595.0 * (1.0 + f / 700.0).log10();
    let (a, b) = (mel(fmin), mel(fmax));
    let edges: Vec<f64> = crate::librosa::linspace(a, b, n_mels + 2)
        .into_iter()
        .map(|m| 700.0 * (10f64.powf(m / 2595.0) - 1.0))
        .collect();
    let f = rfftfreq(n_fft, SRF);
    (0..n_mels)
        .map(|i| {
            let (lo, c, hi) = (edges[i], edges[i + 1], edges[i + 2]);
            f.iter().map(|&x| f32r(((x - lo) / (c - lo)).min((hi - x) / (hi - c)).max(0.0))).collect()
        })
        .collect()
}

/// numpy.pad(..., mode='reflect') of n on each side
fn pad_reflect(m: &[f32], p: usize) -> Vec<f32> {
    let n = m.len();
    let mut y = Vec::with_capacity(n + 2 * p);
    for i in (1..=p).rev() {
        y.push(m[crate::dsp::filter::reflect_index(i as i64, n).min(n - 1)]);
    }
    y.extend_from_slice(m);
    for i in 0..p {
        let j = n as i64 - 2 - i as i64;
        y.push(m[j.rem_euclid(n as i64) as usize]);
    }
    y
}

/// log-mel power spectrogram (dB), frame i centred at i * hop (timeline.log_mel): n_mels x frames
fn log_mel(m: &[f32], n_fft: usize, hop: usize, n_mels: usize, fmin: f64, fmax: f64) -> Vec<Vec<f64>> {
    let y: Vec<f32> = if m.len() > n_fft {
        pad_reflect(m, n_fft / 2)
    } else {
        let mut v = vec![0.0f32; n_fft / 2];
        v.extend_from_slice(m);
        v.extend(std::iter::repeat_n(0.0, n_fft / 2));
        v
    };
    let n = 1 + (y.len() - n_fft) / hop;
    let fb = mel_fb(n_fft, n_mels, fmin, fmax);
    let win: Vec<f32> = hanning(n_fft).into_iter().map(|v| v as f32).collect();
    let mut fft = Rfft::new(n_fft);
    let mut buf = vec![0.0; n_fft];
    let mut spec = vec![C64::new(0.0, 0.0); n_fft / 2 + 1];
    let mut out = vec![vec![0.0; n]; n_mels];
    for j in 0..n {
        for i in 0..n_fft {
            buf[i] = (y[j * hop + i] * win[i]) as f64;
        }
        fft.process(&mut buf, &mut spec);
        let p: Vec<f64> = spec.iter().map(|c| f32r(c.re * c.re + c.im * c.im)).collect();
        for (k, w) in fb.iter().enumerate() {
            let mut acc = 0.0;
            for (b, wb) in w.iter().enumerate() {
                if *wb != 0.0 {
                    acc += wb * p[b];
                }
            }
            out[k][j] = f32r(10.0 * (f32r(acc) + 1e-12).log10());
        }
    }
    out
}

fn std_f32(v: &[f64]) -> f64 {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt()
}

/// beat-tracking novelty at 10 ms frames: broadband and low band (30-200 Hz) log-mel flux, local mean removed
fn pulse_envs(m: &[f32]) -> [Vec<f64>; 2] {
    let mut l = log_mel(m, 2048, 480, 64, 30.0, 8000.0);
    let mx = l.iter().flatten().copied().fold(f64::NEG_INFINITY, f64::max);
    let floor = f32r(mx - 80.0);
    for row in l.iter_mut() {
        for v in row.iter_mut() {
            if *v < floor {
                *v = floor;
            }
        }
    }
    let n = l[0].len();
    let d: Vec<Vec<f64>> =
        l.iter().map(|row| (0..n).map(|j| f32r(row[j] - row[j.saturating_sub(1)]).max(0.0)).collect()).collect();
    let band_mean = |rows: &[Vec<f64>]| -> Vec<f64> {
        (0..n)
            .map(|j| {
                let mut acc = 0.0f32;
                for r in rows {
                    acc += r[j] as f32;
                }
                (acc / rows.len() as f32) as f64
            })
            .collect()
    };
    let mk = |env: Vec<f64>| -> Vec<f64> {
        let u: Vec<f64> = uniform_filter1d_reflect(&env, 101).into_iter().map(f32r).collect();
        let e: Vec<f64> = env.iter().zip(&u).map(|(a, b)| f32r(a - b).max(0.0)).collect();
        let sd = f32r(std_f32(&e));
        e.iter().map(|v| f32r(v / f32r(sd + 1e-9))).collect()
    };
    [mk(band_mean(&d)), mk(band_mean(&d[..6]))]
}

struct Period {
    p: f64,
    clarity: f64,
    score: f64,
    ac: Vec<f64>,
}

/// beat period in 10 ms frames, pulse clarity, score and autocorrelation, with a soft 120 BPM prior
fn period(env: &[f64], min_bpm: f64, max_bpm: f64) -> Option<Period> {
    let n = env.len();
    let mean = env.iter().sum::<f64>() / n as f64;
    let e0: Vec<f64> = env.iter().map(|v| f32r(v - f32r(mean))).collect();
    let nf = 2 * n;
    let mut fft = Rfft::new(nf);
    let mut buf = vec![0.0; nf];
    buf[..n].copy_from_slice(&e0);
    let mut spec = vec![C64::new(0.0, 0.0); nf / 2 + 1];
    fft.process(&mut buf, &mut spec);
    let pw: Vec<C64> = spec.iter().map(|c| C64::new(f32r(f32r(c.norm()).powi(2)), 0.0)).collect();
    let mut ifft = Irfft::new(nf);
    let mut full = vec![0.0; nf];
    ifft.process(&pw, &mut full);
    let a0 = f32r(full[0]);
    let ac: Vec<f64> = full[..n].iter().map(|v| f32r(f32r(*v) / f32r(a0 + 1e-12))).collect();
    let lo = (6000.0 / max_bpm) as usize;
    let hi = ((6000.0 / min_bpm) as usize).min(ac.len() / 2);
    if hi <= lo {
        return None;
    }
    let lags: Vec<usize> = (lo..hi).collect();
    let sc: Vec<f64> = lags
        .iter()
        .map(|&l| {
            let prior = (-0.5 * ((6000.0 / l as f64 / 120.0).log2() / 0.9).powi(2)).exp();
            (ac[l] + 0.5 * ac[(2 * l).min(ac.len() - 1)]) * prior
        })
        .collect();
    let k = crate::dsp::stats::argmax(&sc);
    let mut p = lags[k] as f64;
    if k > 0 && k < lags.len() - 1 {
        let (y0, y1, y2) = (sc[k - 1], sc[k], sc[k + 1]);
        p += 0.5 * (y0 - y2) / (y0 - 2.0 * y1 + y2 + 1e-12);
    }
    let clarity = ac[(p.round_ties_even() as usize).min(n - 1)];
    Some(Period { p, clarity, score: sc[k], ac })
}

/// which of 4 beat phases starts the bar: harmony changes plus low-band onsets are strongest on downbeats
fn downbeat_phase(m: &[f32], beats: &[usize]) -> usize {
    let (n_fft, hop) = (4096usize, 480usize);
    let mut y = vec![0.0f32; n_fft / 2];
    y.extend_from_slice(m);
    y.extend(std::iter::repeat_n(0.0, n_fft / 2));
    let n = 1 + (y.len() - n_fft) / hop;
    let f = rfftfreq(n_fft, SRF);
    let pcs: Vec<Option<usize>> = f
        .iter()
        .map(|&fr| {
            if (55.0..=2000.0).contains(&fr) {
                Some((12.0 * (fr / 440.0).log2()).round_ties_even().rem_euclid(12.0) as usize)
            } else {
                None
            }
        })
        .collect();
    let win: Vec<f32> = hanning(n_fft).into_iter().map(|v| v as f32).collect();
    let mut fft = Rfft::new(n_fft);
    let mut buf = vec![0.0; n_fft];
    let mut spec = vec![C64::new(0.0, 0.0); n_fft / 2 + 1];
    let mut chroma = vec![[0.0f64; 12]; n];
    let mut low = vec![0.0f64; n];
    for j in 0..n {
        for i in 0..n_fft {
            buf[i] = (y[j * hop + i] * win[i]) as f64;
        }
        fft.process(&mut buf, &mut spec);
        let mut c = [0.0f32; 12];
        let mut lo = 0.0f32;
        for (b, s) in spec.iter().enumerate() {
            let mag = s.norm() as f32;
            if let Some(pc) = pcs[b] {
                c[pc] += mag;
            }
            if f[b] < 150.0 {
                lo += mag;
            }
        }
        for k in 0..12 {
            chroma[j][k] = c[k] as f64;
        }
        low[j] = f32r((lo as f64).ln_1p());
    }
    let b: Vec<usize> = beats.iter().map(|&x| x.min(n - 1)).collect();
    let mut seg: Vec<[f64; 12]> = Vec::with_capacity(b.len());
    for w in b.windows(2) {
        let (p, q) = (w[0], w[1]);
        if q > p {
            let mut s = [0.0f32; 12];
            for row in &chroma[p..q] {
                for k in 0..12 {
                    s[k] += row[k] as f32;
                }
            }
            let mut v = [0.0; 12];
            for k in 0..12 {
                v[k] = (s[k] / (q - p) as f32) as f64;
            }
            seg.push(v);
        } else {
            seg.push(chroma[p]);
        }
    }
    seg.push(chroma[*b.last().unwrap()]);
    for s in seg.iter_mut() {
        let nrm = f32r(s.iter().map(|v| v * v).sum::<f64>().sqrt());
        for v in s.iter_mut() {
            *v = f32r(*v / f32r(nrm + 1e-9));
        }
    }
    let mut harm = vec![0.0];
    for i in 1..seg.len() {
        let dot: f32 = (0..12).map(|k| (seg[i][k] * seg[i - 1][k]) as f32).sum();
        harm.push(1.0 - dot as f64);
    }
    let kick_all: Vec<f64> = (0..n).map(|i| f32r(low[i] - low[i.saturating_sub(1)]).max(0.0)).collect();
    let kick: Vec<f64> = b
        .iter()
        .map(|&i| kick_all[i.saturating_sub(3)..(i + 4).min(n)].iter().copied().fold(f64::NEG_INFINITY, f64::max))
        .collect();
    let (sh, sk) = (std_f32(&harm), f32r(std_f32(&kick)));
    let acc: Vec<f64> = harm.iter().zip(&kick).map(|(h, k)| h / (sh + 1e-9) + f32r(k / f32r(sk + 1e-9))).collect();
    let means: Vec<f64> = (0..4)
        .map(|p| {
            let v: Vec<f64> = acc.iter().skip(p).step_by(4).copied().collect();
            if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
        })
        .collect();
    let mut best = 0;
    for p in 1..4 {
        if means[p] > means[best] || (means[best].is_nan() && !means[p].is_nan()) {
            best = p;
        }
    }
    best
}

/// tempo (autocorrelation with a soft 120 BPM prior, broadband or low-band flux, whichever pulses more clearly),
/// beats, a downbeat guess and pulse clarity of a mono 48 kHz array (min / max 60 / 200 BPM)
pub fn music_grid(mono: &[f32]) -> Grid {
    music_grid_range(mono, 60.0, 200.0)
}

pub fn music_grid_range(m: &[f32], min_bpm: f64, max_bpm: f64) -> Grid {
    if m.len() < 5 * crate::SR as usize {
        return Grid::none("too short for a beat grid");
    }
    let envs = pulse_envs(m);
    let mut best: Option<(Period, &Vec<f64>)> = None;
    for env in envs.iter() {
        if let Some(p) = period(env, min_bpm, max_bpm)
            && best.as_ref().is_none_or(|(b, _)| p.score > b.score)
        {
            best = Some((p, env));
        }
    }
    let Some((per, env)) = best else {
        return Grid::none("too short for a beat grid");
    };
    let p = per.p;
    let wlo = ((p / 2.0) as usize).max(1);
    let whi = (2.0 * p) as usize;
    let win: Vec<usize> = (wlo..=whi).collect();
    let pen: Vec<f64> = win.iter().map(|&w| 100.0 * (w as f64 / p).ln().powi(2)).collect();
    let n = env.len();
    let mut score: Vec<f32> = env.iter().map(|&v| v as f32).collect();
    let mut back = vec![-1i64; n];
    for t in (p / 2.0) as usize..n {
        let mut best_c = f64::NEG_INFINITY;
        let mut best_prev = -1i64;
        for (k, &w) in win.iter().enumerate() {
            if w > t {
                continue;
            }
            let prev = t - w;
            let c = score[prev] as f64 - pen[k];
            if c > best_c {
                best_c = c;
                best_prev = prev as i64;
            }
        }
        if best_prev < 0 {
            continue;
        }
        if best_c > 0.0 {
            score[t] = (score[t] as f64 + best_c) as f32;
            back[t] = best_prev;
        }
    }
    let ip = p as usize;
    let tail = &score[n - ip..];
    let mut am = 0;
    for i in 1..tail.len() {
        if tail[i] > tail[am] {
            am = i;
        }
    }
    let mut t = (n - ip + am) as i64;
    let mut beats = Vec::new();
    while t >= 0 {
        beats.push(t as usize);
        t = back[t as usize];
    }
    beats.reverse();
    if beats.len() < 4 {
        return Grid::none("no steady pulse found");
    }
    let ibi: Vec<f64> = beats.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    let mi = ibi.iter().sum::<f64>() / ibi.len() as f64;
    let reg = (ibi.iter().map(|v| (v - mi) * (v - mi)).sum::<f64>() / ibi.len() as f64).sqrt() / mi;
    let clarity = per.clarity;
    let conf = if clarity >= CLEAR_PULSE && reg < 0.1 {
        "clear"
    } else if clarity >= WEAK_PULSE {
        "weak"
    } else {
        "none"
    };
    let ph = downbeat_phase(m, &beats);
    let bt: Vec<f64> = beats.iter().map(|&b| b as f64 * 0.01).collect();
    let h = (p / 2.0).round_ties_even() as usize;
    let alt = if 12000.0 / p <= max_bpm && per.ac[h.min(per.ac.len() - 1)] >= 0.5 * clarity {
        Some(round_to(12000.0 / p, 1))
    } else {
        None
    };
    Grid {
        tempo: Some(round_to(6000.0 / p, 1)),
        tempo_alt: alt,
        beats: bt.iter().map(|b| round_to(*b, 3)).collect(),
        downbeats: bt.iter().skip(ph).step_by(4).map(|b| round_to(*b, 3)).collect(),
        pulse_clarity: Some(round_to(clarity, 3)),
        beat_regularity: Some(round_to(reg, 3)),
        confidence: conf.into(),
        note: "beats at 10 ms resolution; downbeats are a guess (4/4 assumed); tempo may be half/double the felt tempo"
            .into(),
    }
}
