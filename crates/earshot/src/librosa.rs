//! Ports of the librosa 1.0 functions earshot's features use: onset_strength / onset_detect (with backtracking),
//! beat_track (+ tempo), chroma_cqt (+ estimate_tuning), effects.hpss, yin and rms, at 22.05 kHz.
//!
//! Computed in f64 with values rounded to f32 where librosa stores float32 (STFT, mel, dB), so frame decisions
//! (peaks, beats, troughs) match librosa except on exact float32 ties.

use crate::dsp::fft::{C64, Irfft, Rfft, next_fast_len};
use crate::dsp::filter::median_filter_reflect;
use crate::dsp::{hann_periodic, rfftfreq, stats};

pub const SR22: u32 = 22050;
const TINY32: f64 = 1.175_494_350_822_287_5e-38;
const TINY64: f64 = f64::MIN_POSITIVE;

#[inline]
fn f32r(v: f64) -> f64 {
    v as f32 as f64
}

/// centred STFT with zero padding (librosa.stft(center=True, pad_mode='constant')): frames x (n_fft / 2 + 1),
/// complex values rounded to complex64
pub fn stft(y: &[f64], n_fft: usize, hop: usize, window: &[f64]) -> Vec<Vec<C64>> {
    let pad = n_fft / 2;
    let n = y.len();
    let frames = 1 + n / hop;
    let mut fft = Rfft::new(n_fft);
    let mut buf = vec![0.0; n_fft];
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let start = (f * hop) as i64 - pad as i64;
        for i in 0..n_fft {
            let j = start + i as i64;
            buf[i] = if j >= 0 && (j as usize) < n { y[j as usize] * window[i] } else { 0.0 };
        }
        let mut spec = vec![C64::new(0.0, 0.0); n_fft / 2 + 1];
        fft.process(&mut buf, &mut spec);
        for c in &mut spec {
            *c = C64::new(f32r(c.re), f32r(c.im));
        }
        out.push(spec);
    }
    out
}

// ------------------------------------------------------------------------------------------------ mel / onsets
fn hz_to_mel_slaney(f: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = 6.4f64.ln() / 27.0;
    if f >= min_log_hz { min_log_mel + (f / min_log_hz).ln() / logstep } else { f / f_sp }
}

fn mel_to_hz_slaney(m: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = 6.4f64.ln() / 27.0;
    if m >= min_log_mel { min_log_hz * (logstep * (m - min_log_mel)).exp() } else { f_sp * m }
}

/// librosa.filters.mel(sr, n_fft, n_mels, fmin, fmax) (Slaney scale and norm), float32 weights: n_mels x bins
pub fn mel_filters(sr: f64, n_fft: usize, n_mels: usize, fmin: f64, fmax: f64) -> Vec<Vec<f64>> {
    let fft_f = rfftfreq(n_fft, sr);
    let (a, b) = (hz_to_mel_slaney(fmin), hz_to_mel_slaney(fmax));
    let mel_f: Vec<f64> = linspace(a, b, n_mels + 2).into_iter().map(mel_to_hz_slaney).collect();
    (0..n_mels)
        .map(|i| {
            let fd0 = mel_f[i + 1] - mel_f[i];
            let fd1 = mel_f[i + 2] - mel_f[i + 1];
            let enorm = 2.0 / (mel_f[i + 2] - mel_f[i]);
            fft_f
                .iter()
                .map(|&f| {
                    let lower = -(mel_f[i] - f) / fd0;
                    let upper = (mel_f[i + 2] - f) / fd1;
                    f32r(f32r(lower.min(upper).max(0.0)) * enorm)
                })
                .collect()
        })
        .collect()
}

/// numpy.linspace(a, b, n)
pub fn linspace(a: f64, b: f64, n: usize) -> Vec<f64> {
    if n == 1 {
        return vec![a];
    }
    let step = (b - a) / (n - 1) as f64;
    let mut v: Vec<f64> = (0..n).map(|i| a + i as f64 * step).collect();
    v[n - 1] = b;
    v
}

/// librosa.onset.onset_strength(y, sr, hop_length) with the defaults (n_fft 2048, 128 Slaney mels up to sr / 2,
/// power_to_db with top_db 80, lag 1, mean over bands, centred)
pub fn onset_strength(y: &[f64], sr: f64, hop: usize) -> Vec<f64> {
    let n_fft = 2048;
    let win = hann_periodic(n_fft);
    let spec = stft(y, n_fft, hop, &win);
    let mel = mel_filters(sr, n_fft, 128, 0.0, sr / 2.0);
    let t = spec.len();
    // S (mels x frames) in dB
    let mut s = vec![vec![0.0f64; t]; 128];
    let mut maxdb = f64::NEG_INFINITY;
    for (j, fr) in spec.iter().enumerate() {
        let p: Vec<f64> = fr.iter().map(|c| f32r(f32r(c.norm()).powi(2))).collect();
        for (m, w) in mel.iter().enumerate() {
            let mut acc = 0.0;
            for (k, wk) in w.iter().enumerate() {
                if *wk != 0.0 {
                    acc += wk * p[k];
                }
            }
            let v = f32r(10.0 * f32r(acc).max(1e-10).log10());
            s[m][j] = v;
            if v > maxdb {
                maxdb = v;
            }
        }
    }
    let floor = maxdb - 80.0;
    for row in s.iter_mut() {
        for v in row.iter_mut() {
            if *v < floor {
                *v = f32r(floor);
            }
        }
    }
    // onset_env[j] = mean_m max(0, S[m, j+1] - S[m, j]), padded at the start by lag + n_fft // (2 hop)
    let pad = 1 + n_fft / (2 * hop);
    let mut env = vec![0.0; t];
    for j in 0..t.saturating_sub(1) {
        let mut acc = 0.0f32;
        for row in s.iter() {
            acc += f32r(row[j + 1] - row[j]).max(0.0) as f32;
        }
        let v = (acc / 128.0) as f64;
        if j + pad < t {
            env[j + pad] = v;
        }
    }
    env
}

/// librosa.util.peak_pick (greedy)
pub fn peak_pick(
    x: &[f64],
    pre_max: usize,
    post_max: usize,
    pre_avg: usize,
    post_avg: usize,
    delta: f64,
    wait: usize,
) -> Vec<usize> {
    let n = x.len();
    let mut peaks = Vec::new();
    if n == 0 {
        return peaks;
    }
    let maxr = |a: usize, b: usize| x[a..b].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let meanr = |a: usize, b: usize| x[a..b].iter().sum::<f64>() / (b - a) as f64;
    let first = x[0] >= maxr(0, post_max.min(n)) && x[0] >= meanr(0, post_avg.min(n)) + delta;
    let mut i = if first {
        peaks.push(0);
        wait + 1
    } else {
        1
    };
    while i < n {
        let maxn = maxr(i.saturating_sub(pre_max), (i + post_max).min(n));
        if x[i] != maxn {
            i += 1;
            continue;
        }
        let avgn = meanr(i.saturating_sub(pre_avg), (i + post_avg).min(n));
        if x[i] < avgn + delta {
            i += 1;
            continue;
        }
        peaks.push(i);
        i += wait + 1;
    }
    peaks
}

/// librosa.onset.onset_backtrack: each onset moves back to the preceding local minimum of `energy`
pub fn onset_backtrack(events: &[usize], energy: &[f64]) -> Vec<usize> {
    let mut minima = vec![0usize];
    for i in 1..energy.len().saturating_sub(1) {
        if energy[i] <= energy[i - 1] && energy[i] < energy[i + 1] {
            minima.push(i);
        }
    }
    minima.sort_unstable();
    minima.dedup();
    events
        .iter()
        .map(|&e| {
            // largest minimum <= e (match_events with right=False)
            let k = minima.partition_point(|&m| m <= e);
            minima[k.saturating_sub(1)]
        })
        .collect()
}

/// librosa.onset.onset_detect(y, sr, hop_length, units='time', backtrack=True, delta) -> onset times (s)
pub fn onset_detect_times(y: &[f64], sr: f64, hop: usize, delta: f64) -> Vec<f64> {
    let env = onset_strength(y, sr, hop);
    let mn = stats::min(&env);
    let mut e: Vec<f64> = env.iter().map(|v| f32r(v - mn)).collect();
    let mx = stats::max(&e);
    for v in e.iter_mut() {
        *v = f32r(*v / (mx + TINY32));
    }
    if !e.iter().any(|v| *v != 0.0) || !e.iter().all(|v| v.is_finite()) {
        return vec![];
    }
    let fl = |s: f64| (s * sr / hop as f64).floor();
    let pre_max = fl(0.03).ceil() as usize;
    let post_max = (0.0 * sr / hop as f64).floor() as usize + 1;
    let pre_avg = fl(0.10).ceil() as usize;
    let post_avg = fl(0.10) as usize + 1;
    let wait = fl(0.03).ceil() as usize;
    let peaks = peak_pick(&e, pre_max, post_max, pre_avg, post_avg, delta, wait);
    let bt = onset_backtrack(&peaks, &e);
    bt.into_iter().map(|f| f as f64 * hop as f64 / sr).collect()
}

// ------------------------------------------------------------------------------------------------ beat tracking
fn tempo(oenv: &[f64], sr: f64, hop: usize) -> f64 {
    let win_length = ((8.0 * sr) as i64 / hop as i64) as usize; // time_to_frames(8.0)
    let ac_window = hann_periodic(win_length);
    let n = oenv.len();
    let half = win_length / 2;
    // linear_ramp padding to 0 at both ends
    let mut padded = Vec::with_capacity(n + 2 * half);
    let first = oenv[0];
    let last = oenv[n - 1];
    for j in 0..half {
        padded.push(f32r(first * j as f64 / half as f64));
    }
    padded.extend_from_slice(oenv);
    for j in (0..half).rev() {
        padded.push(f32r(last * j as f64 / half as f64));
    }
    let n_pad = next_fast_len(2 * win_length - 1);
    let mut fft = Rfft::new(n_pad);
    let mut ifft = Irfft::new(n_pad);
    let mut buf = vec![0.0; n_pad];
    let mut spec = vec![C64::new(0.0, 0.0); n_pad / 2 + 1];
    let mut ac = vec![0.0; n_pad];
    let mut tg = vec![0.0f64; win_length];
    for f in 0..n {
        for v in buf.iter_mut() {
            *v = 0.0;
        }
        for i in 0..win_length {
            buf[i] = padded[f + i] * ac_window[i];
        }
        fft.process(&mut buf, &mut spec);
        for c in spec.iter_mut() {
            *c = C64::new(c.re * c.re + c.im * c.im, 0.0);
        }
        ifft.process(&spec, &mut ac);
        let mx = ac[..win_length].iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let d = if mx < TINY64 { 1.0 } else { mx };
        for i in 0..win_length {
            tg[i] += ac[i] / d;
        }
    }
    for v in tg.iter_mut() {
        *v /= n as f64;
    }
    let bpms: Vec<f64> =
        (0..win_length).map(|k| if k == 0 { f64::INFINITY } else { 60.0 * sr / (hop as f64 * k as f64) }).collect();
    let max_idx = bpms.iter().position(|&b| b < 320.0).unwrap_or(0);
    let mut best = 0;
    let mut best_v = f64::NEG_INFINITY;
    for k in 0..win_length {
        let lp = if k < max_idx { f64::NEG_INFINITY } else { -0.5 * (bpms[k].log2() - 120f64.log2()).powi(2) };
        let v = (1e6 * tg[k]).ln_1p() + lp;
        if v > best_v {
            best_v = v;
            best = k;
        }
    }
    bpms[best]
}

/// librosa.beat.beat_track(onset_envelope, sr, hop_length, units='time') -> (tempo, beat times)
pub fn beat_track(oenv: &[f64], sr: f64, hop: usize) -> (f64, Vec<f64>) {
    if !oenv.iter().any(|v| *v != 0.0) {
        return (0.0, vec![]);
    }
    let bpm = tempo(oenv, sr, hop);
    let frame_rate = sr / hop as f64;
    let fpb = (frame_rate * 60.0 / bpm).round_ties_even();
    let n = oenv.len();
    // normalise by the sample std (ddof 1), float32
    let sd = stats::std_ddof1(oenv);
    let on: Vec<f64> = oenv.iter().map(|v| f32r(v / (f32r(sd) + TINY32))).collect();
    // local score: Gaussian-window convolution, accumulated in float32
    let half = fpb as i64;
    let window: Vec<f64> = (-half..=half).map(|k| (-0.5 * (k as f64 * 32.0 / fpb).powi(2)).exp()).collect();
    let kk = window.len() as i64;
    let mut localscore = vec![0.0f64; n];
    for i in 0..n as i64 {
        let mut acc = 0.0f32;
        let lo = (i + kk / 2 - n as i64 + 1).max(0);
        let hi = (i + kk / 2).min(kk);
        for k in lo..hi {
            acc = (acc as f64 + window[k as usize] * on[(i + kk / 2 - k) as usize]) as f32;
        }
        localscore[i as usize] = acc as f64;
    }
    // dynamic programme (float64)
    let tightness = 100.0f64;
    let score_thresh = 0.01 * stats::max(&localscore);
    let mut backlink = vec![-1i64; n];
    let mut cumscore = vec![0.0f64; n];
    let mut first_beat = true;
    cumscore[0] = localscore[0];
    let lfpb = fpb.ln();
    for i in 0..n {
        let mut best_score = f64::NEG_INFINITY;
        let mut beat_location = -1i64;
        let start = i as i64 - (fpb / 2.0).round_ties_even() as i64;
        let stop = (i as f64 - 2.0 * fpb - 1.0) as i64;
        let mut loc = start;
        while loc > stop {
            if loc < 0 {
                break;
            }
            let d = ((i as i64 - loc) as f64).ln() - lfpb;
            let score = cumscore[loc as usize] - tightness * d * d;
            if score > best_score {
                best_score = score;
                beat_location = loc;
            }
            loc -= 1;
        }
        cumscore[i] = if beat_location >= 0 { localscore[i] + best_score } else { localscore[i] };
        if first_beat && localscore[i] < score_thresh {
            backlink[i] = -1;
        } else {
            backlink[i] = beat_location;
            first_beat = false;
        }
    }
    // last beat: the last local maximum of cumscore above half the median local maximum
    let mut is_max = vec![false; n];
    for i in 1..n.saturating_sub(1) {
        is_max[i] = cumscore[i] > cumscore[i - 1] && cumscore[i] >= cumscore[i + 1];
    }
    if n >= 2 {
        is_max[n - 1] = cumscore[n - 1] > cumscore[n - 2];
    }
    let maxima: Vec<f64> = (0..n).filter(|&i| is_max[i]).map(|i| cumscore[i]).collect();
    let thr = if maxima.is_empty() { f64::NAN } else { 0.5 * stats::median(&maxima) };
    let mut tail = n as i64 - 1;
    for i in (0..n).rev() {
        if is_max[i] && cumscore[i] >= thr {
            tail = i as i64;
            break;
        }
    }
    let mut beats = vec![false; n];
    let mut k = tail;
    while k >= 0 {
        beats[k as usize] = true;
        k = backlink[k as usize];
    }
    // trim weak leading / trailing frames
    let bvals: Vec<f64> = (0..n).filter(|&i| beats[i]).map(|i| localscore[i]).collect();
    let w = [0.0, 0.5, 1.0, 0.5, 0.0];
    let full_len = bvals.len() + w.len() - 1;
    let mut conv = vec![0.0; full_len];
    for (i, b) in bvals.iter().enumerate() {
        for (j, wj) in w.iter().enumerate() {
            conv[i + j] += b * wj;
        }
    }
    let smooth: Vec<f64> = conv[2.min(full_len)..(n + 2).min(full_len)].to_vec();
    let threshold = 0.5 * (smooth.iter().map(|v| v * v).sum::<f64>() / smooth.len().max(1) as f64).sqrt();
    let mut i = 0;
    while i < n && localscore[i] <= threshold {
        beats[i] = false;
        i += 1;
    }
    let mut i = n as i64 - 1;
    while i >= 0 && localscore[i as usize] <= threshold {
        beats[i as usize] = false;
        i -= 1;
    }
    let times = (0..n).filter(|&i| beats[i]).map(|i| i as f64 * hop as f64 / sr).collect();
    (bpm, times)
}

// ------------------------------------------------------------------------------------------------ chroma
const C1_HZ: f64 = 32.703_195_662_574_83;

fn localmax_axis(x: &[f64]) -> Vec<bool> {
    let n = x.len();
    let mut m = vec![false; n];
    for i in 1..n.saturating_sub(1) {
        m[i] = x[i] > x[i - 1] && x[i] >= x[i + 1];
    }
    if n >= 2 {
        m[n - 1] = x[n - 1] > x[n - 2];
    }
    m
}

/// librosa.estimate_tuning(y, sr, bins_per_octave) via piptrack (n_fft 2048, hop 512, 150 - 4000 Hz)
pub fn estimate_tuning(y: &[f64], sr: f64, bins_per_octave: usize) -> f64 {
    let n_fft = 2048;
    let win = hann_periodic(n_fft);
    let spec = stft(y, n_fft, n_fft / 4, &win);
    let freqs = rfftfreq(n_fft, sr);
    let nb = n_fft / 2 + 1;
    let mut pitches = Vec::new();
    let mut mags = Vec::new();
    for fr in &spec {
        let s: Vec<f64> = fr.iter().map(|c| f32r(c.norm())).collect();
        let ref_v = 0.1 * stats::max(&s);
        let masked: Vec<f64> = s.iter().map(|&v| if v > ref_v { v } else { 0.0 }).collect();
        let lm = localmax_axis(&masked);
        for k in 0..nb {
            if !(freqs[k] >= 150.0 && freqs[k] < 4000.0f64.min(sr / 2.0) && lm[k]) {
                continue;
            }
            let (avg, shift) = if k == 0 || k == nb - 1 {
                (if k == 0 { s[1] - s[0] } else { s[k] - s[k - 1] }, 0.0)
            } else {
                let a = s[k + 1] + s[k - 1] - 2.0 * s[k];
                let b = (s[k + 1] - s[k - 1]) / 2.0;
                ((s[k + 1] - s[k - 1]) / 2.0, if b.abs() >= a.abs() { 0.0 } else { -b / a })
            };
            let p = f32r((k as f64 + shift) * sr / n_fft as f64);
            let m = f32r(s[k] + 0.5 * avg * shift);
            pitches.push(p);
            mags.push(m);
        }
    }
    let pos: Vec<f64> = pitches.iter().zip(&mags).filter(|(p, _)| **p > 0.0).map(|(_, m)| *m).collect();
    let threshold = if pos.is_empty() { 0.0 } else { stats::median(&pos) };
    let sel: Vec<f64> =
        pitches.iter().zip(&mags).filter(|(p, m)| **p > 0.0 && **m >= threshold).map(|(p, _)| *p).collect();
    pitch_tuning(&sel, 0.01, bins_per_octave)
}

fn pitch_tuning(freqs: &[f64], resolution: f64, bins_per_octave: usize) -> f64 {
    let f: Vec<f64> = freqs.iter().copied().filter(|v| *v > 0.0).collect();
    if f.is_empty() {
        return 0.0;
    }
    let nbins = (1.0 / resolution).ceil() as usize;
    let edges = linspace(-0.5, 0.5, nbins + 1);
    let mut counts = vec![0usize; nbins];
    for v in f {
        // float32 arithmetic, as numpy does on the float32 pitch array
        let octs = (v as f32 / (440.0f32 / 16.0)).log2();
        let mut r = (bins_per_octave as f32 * octs).rem_euclid(1.0) as f64;
        if r >= 0.5 {
            r = (r as f32 - 1.0) as f64;
        }
        if r < edges[0] || r > edges[nbins] {
            continue;
        }
        // numpy.histogram with explicit edges: [e_i, e_i+1), the last bin closed
        let i = (edges.partition_point(|&e| e <= r) - 1).min(nbins - 1);
        counts[i] += 1;
    }
    let mut best = 0;
    for i in 1..nbins {
        if counts[i] > counts[best] {
            best = i;
        }
    }
    edges[best]
}

struct SparseRow {
    idx: Vec<usize>,
    val: Vec<C64>,
}

/// the FFT of one octave of CQT wavelets (librosa constantq.__vqt_filter_fft), sparsified, scaled
fn vqt_filter_fft(sr: f64, freqs: &[f64], alpha: &[f64], scale: f64) -> (Vec<SparseRow>, usize) {
    let lengths: Vec<f64> = freqs.iter().zip(alpha).map(|(f, a)| (1.0 / a) * sr / f).collect();
    let max_len = lengths.iter().copied().fold(0.0, f64::max);
    let n_fft = 2f64.powf(max_len.log2().ceil()) as usize;
    let mut rows = Vec::with_capacity(freqs.len());
    for (&ilen, &freq) in lengths.iter().zip(freqs) {
        let start = (-ilen / 2.0).floor();
        let stop = (ilen / 2.0).floor();
        let count = (stop - start).ceil().max(0.0) as usize;
        let win = hann_periodic(count);
        let mut sig: Vec<C64> = (0..count)
            .map(|i| {
                let ang = (start + i as f64) * 2.0 * std::f64::consts::PI * freq / sr;
                C64::new(ang.cos(), ang.sin()) * win[i]
            })
            .collect();
        let l1: f64 = sig.iter().map(|c| c.norm()).sum();
        if l1 >= TINY64 {
            for c in sig.iter_mut() {
                *c /= l1;
            }
        }
        // pad_center to n_fft, cast to complex64, scale by length / n_fft (stays complex64)
        let lpad = (n_fft - count) / 2;
        let mut full = vec![C64::new(0.0, 0.0); n_fft];
        for (i, c) in sig.iter().enumerate() {
            let c32 = C64::new(f32r(c.re), f32r(c.im));
            let s = ilen / n_fft as f64;
            full[lpad + i] = C64::new(f32r(c32.re * s), f32r(c32.im * s));
        }
        // full complex FFT, keep the first n_fft / 2 + 1 bins (complex64)
        let spec = complex_fft(&full);
        let half: Vec<C64> = spec[..n_fft / 2 + 1].iter().map(|c| C64::new(f32r(c.re), f32r(c.im))).collect();
        // sparsify: drop the smallest bins holding 1% of the row's magnitude
        let mags: Vec<f64> = half.iter().map(|c| f32r(c.norm())).collect();
        let norm: f64 = mags.iter().sum();
        let mut sorted = mags.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut cum = 0.0;
        let mut ti = 0;
        for (i, m) in sorted.iter().enumerate() {
            cum += m / norm;
            if cum >= 0.01 {
                ti = i;
                break;
            }
        }
        let thresh = sorted[ti];
        let mut row = SparseRow { idx: vec![], val: vec![] };
        for (k, c) in half.iter().enumerate() {
            if mags[k] >= thresh {
                row.idx.push(k);
                row.val.push(C64::new(f32r(c.re * scale), f32r(c.im * scale)));
            }
        }
        rows.push(row);
    }
    (rows, n_fft)
}

/// complex DFT via two real FFTs
fn complex_fft(x: &[C64]) -> Vec<C64> {
    let n = x.len();
    let mut fft = Rfft::new(n);
    let mut re: Vec<f64> = x.iter().map(|c| c.re).collect();
    let mut im: Vec<f64> = x.iter().map(|c| c.im).collect();
    let mut sr = vec![C64::new(0.0, 0.0); n / 2 + 1];
    let mut si = vec![C64::new(0.0, 0.0); n / 2 + 1];
    fft.process(&mut re, &mut sr);
    fft.process(&mut im, &mut si);
    let get = |s: &Vec<C64>, k: usize| if k <= n / 2 { s[k] } else { s[n - k].conj() };
    (0..n).map(|k| get(&sr, k) + C64::new(0.0, 1.0) * get(&si, k)).collect()
}

/// librosa.feature.chroma_cqt(y, sr, hop_length) with the defaults (C1, 7 octaves, 36 bins per octave, tuning
/// estimated, inf-norm per frame) -> chroma (12 x frames)
pub fn chroma_cqt(y: &[f64], sr: f64, hop: usize) -> Vec<Vec<f64>> {
    let bpo = 36usize;
    let n_bins = 7 * bpo;
    let tuning = estimate_tuning(y, sr, bpo);
    let fmin = C1_HZ * 2f64.powf(tuning / bpo as f64);
    let ratios: Vec<f64> = (0..bpo).map(|k| 2f64.powf(k as f64 / bpo as f64)).collect();
    let mut freqs: Vec<f64> = (0..7).flat_map(|o| ratios.iter().map(move |r| 2f64.powi(o) * r)).take(n_bins).collect();
    freqs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let freqs: Vec<f64> = freqs.iter().map(|r| r * fmin).collect();
    // relative bandwidths
    let logf: Vec<f64> = freqs.iter().map(|f| f.log2()).collect();
    let mut bpo_v = vec![0.0; n_bins];
    bpo_v[0] = 1.0 / (logf[1] - logf[0]);
    bpo_v[n_bins - 1] = 1.0 / (logf[n_bins - 1] - logf[n_bins - 2]);
    for i in 1..n_bins - 1 {
        bpo_v[i] = 2.0 / (logf[i + 1] - logf[i - 1]);
    }
    let alpha: Vec<f64> = bpo_v.iter().map(|b| (2f64.powf(2.0 / b) - 1.0) / (2f64.powf(2.0 / b) + 1.0)).collect();
    let lengths: Vec<f64> = freqs.iter().zip(&alpha).map(|(f, a)| (1.0 / a) * sr / f).collect();
    let n_octaves = n_bins.div_ceil(bpo);
    let n_filters = bpo.min(n_bins);
    let mut my_y: Vec<f64> = y.to_vec();
    let mut my_sr = sr;
    let mut my_hop = hop;
    let mut resp: Vec<Vec<Vec<f64>>> = Vec::new(); // per octave: bins x frames (magnitudes)
    for i in 0..n_octaves {
        let (a, b) = (n_bins - n_filters * (i + 1), n_bins - n_filters * i);
        let (rows, n_fft) = vqt_filter_fft(my_sr, &freqs[a..b], &alpha[a..b], (sr / my_sr).sqrt());
        let ones = vec![1.0; n_fft];
        let d = stft(&my_y, n_fft, my_hop, &ones);
        let mut oct = vec![vec![0.0; d.len()]; rows.len()];
        for (r, row) in rows.iter().enumerate() {
            for (t, fr) in d.iter().enumerate() {
                let mut acc = C64::new(0.0, 0.0);
                for (k, v) in row.idx.iter().zip(&row.val) {
                    acc += v * fr[*k];
                }
                oct[r][t] = f32r(C64::new(f32r(acc.re), f32r(acc.im)).norm());
            }
        }
        resp.push(oct);
        if i < n_octaves - 1 {
            let f_max_next = freqs[a - 1];
            if my_hop.is_multiple_of(2) && f_max_next <= my_sr / 5.0 {
                my_hop /= 2;
                my_sr /= 2.0;
                let target = my_y.len().div_ceil(2);
                let y32: Vec<f32> = my_y.iter().map(|v| *v as f32).collect();
                let mut r = crate::dsp::resample::resample_f64(&y32, 2, 1);
                r.resize(target, 0.0);
                my_y = r.into_iter().map(|v| f32r(f32r(v) / 0.5f64.sqrt())).collect();
            }
        }
    }
    let max_col = resp.iter().map(|o| o.first().map_or(0, |r| r.len())).min().unwrap_or(0);
    let mut c = vec![vec![0.0; max_col]; n_bins];
    let mut end = n_bins;
    for oct in &resp {
        let n_oct = oct.len();
        for (j, row) in oct.iter().enumerate() {
            let dst = end - n_oct + j;
            for t in 0..max_col {
                c[dst][t] = f32r(row[t] / lengths[dst].sqrt());
            }
        }
        end -= n_oct;
    }
    // cq_to_chroma (fmin = C1, so no roll), then inf-norm per frame
    let n_merge = bpo / 12;
    let mut chroma = vec![vec![0.0; max_col]; 12];
    for (bin, row) in c.iter().enumerate() {
        // repeat(eye(12), 3) rolled by -(3 // 2) = -1 along the input axis, tiled over octaves
        let pos = (bin % bpo + n_merge / 2) % bpo;
        let ch = pos / n_merge;
        for t in 0..max_col {
            chroma[ch][t] += row[t];
        }
    }
    for t in 0..max_col {
        let mx = (0..12).map(|k| chroma[k][t].abs()).fold(0.0, f64::max);
        let d = if mx < TINY32 { 1.0 } else { mx };
        for row in chroma.iter_mut() {
            row[t] = f32r(row[t] / d);
        }
    }
    chroma
}

// ------------------------------------------------------------------------------------------------ hpss / yin / rms
/// librosa.effects.hpss(y) energies: (sum of harmonic^2, sum of percussive^2)
pub fn hpss_energies(y: &[f64]) -> (f64, f64) {
    let n_fft = 2048;
    let hop = 512;
    let win = hann_periodic(n_fft);
    let d = stft(y, n_fft, hop, &win);
    let t = d.len();
    let nb = n_fft / 2 + 1;
    let mag: Vec<Vec<f64>> = d.iter().map(|fr| fr.iter().map(|c| f32r(c.norm())).collect()).collect(); // t x nb
    // harmonic: median along time for each bin; percussive: median along frequency for each frame
    let mut harm = vec![vec![0.0; nb]; t];
    for k in 0..nb {
        let row: Vec<f64> = (0..t).map(|j| mag[j][k]).collect();
        let f = median_filter_reflect(&row, 31);
        for j in 0..t {
            harm[j][k] = f[j];
        }
    }
    let perc: Vec<Vec<f64>> = mag.iter().map(|fr| median_filter_reflect(fr, 31)).collect();
    let mask = |x: f64, r: f64| -> f64 {
        let z = x.max(r);
        if z < TINY32 {
            return 0.5;
        }
        let m = f32r((x / z).powi(2));
        let rm = f32r((r / z).powi(2));
        f32r(m / (m + rm))
    };
    let mut hs = Vec::with_capacity(t);
    let mut ps = Vec::with_capacity(t);
    for j in 0..t {
        let mut h = Vec::with_capacity(nb);
        let mut p = Vec::with_capacity(nb);
        for k in 0..nb {
            let mh = mask(harm[j][k], perc[j][k]);
            let mp = mask(perc[j][k], harm[j][k]);
            h.push(d[j][k] * mh);
            p.push(d[j][k] * mp);
        }
        hs.push(h);
        ps.push(p);
    }
    let yh = istft(&hs, n_fft, hop, y.len());
    let yp = istft(&ps, n_fft, hop, y.len());
    let eh: f64 = yh.iter().map(|v| (*v as f32 as f64).powi(2)).sum();
    let ep: f64 = yp.iter().map(|v| (*v as f32 as f64).powi(2)).sum();
    (eh, ep)
}

/// librosa.istft(stft, hop_length, center=True, length) with a periodic Hann window
pub fn istft(frames: &[Vec<C64>], n_fft: usize, hop: usize, length: usize) -> Vec<f64> {
    let win = hann_periodic(n_fft);
    let padded_len = length + 2 * (n_fft / 2);
    let n_frames = frames.len().min(padded_len.div_ceil(hop));
    let full = n_fft + hop * n_frames.saturating_sub(1);
    let mut y = vec![0.0; full];
    let mut wss = vec![0.0; full];
    let mut ifft = Irfft::new(n_fft);
    let mut buf = vec![0.0; n_fft];
    for (f, fr) in frames.iter().take(n_frames).enumerate() {
        ifft.process(fr, &mut buf);
        for i in 0..n_fft {
            y[f * hop + i] += buf[i] * win[i];
            wss[f * hop + i] += win[i] * win[i];
        }
    }
    let start = n_fft / 2;
    (0..length)
        .map(|i| {
            let j = start + i;
            if j >= full {
                return 0.0;
            }
            if wss[j] > TINY64 { y[j] / wss[j] } else { y[j] }
        })
        .collect()
}

/// librosa.yin(y, fmin, fmax, sr, frame_length) with hop frame_length / 4, centred, zero padded
pub fn yin(y: &[f64], fmin: f64, fmax: f64, sr: f64, frame_length: usize) -> Vec<f64> {
    let hop = frame_length / 4;
    let pad = frame_length / 2;
    let n = y.len();
    let n_frames = 1 + n / hop;
    let min_period = (sr / fmax).floor() as usize;
    let max_period = ((sr / fmin).ceil() as usize).min(frame_length - 1);
    let n_pad = next_fast_len(2 * frame_length - 1);
    let mut fft = Rfft::new(n_pad);
    let mut ifft = Irfft::new(n_pad);
    let mut buf = vec![0.0; n_pad];
    let mut spec = vec![C64::new(0.0, 0.0); n_pad / 2 + 1];
    let mut acf = vec![0.0; n_pad];
    let mut out = Vec::with_capacity(n_frames);
    for f in 0..n_frames {
        let frame: Vec<f64> = (0..frame_length)
            .map(|i| {
                let j = (f * hop + i) as i64 - pad as i64;
                if j >= 0 && (j as usize) < n { y[j as usize] } else { 0.0 }
            })
            .collect();
        for v in buf.iter_mut() {
            *v = 0.0;
        }
        buf[..frame_length].copy_from_slice(&frame);
        fft.process(&mut buf, &mut spec);
        for c in spec.iter_mut() {
            *c = C64::new(f32r(c.re * c.re + c.im * c.im), 0.0);
        }
        ifft.process(&spec, &mut acf);
        // cumulative energy (float32)
        let mut cum = vec![0.0f64; frame_length];
        let mut acc = 0.0f32;
        for i in 0..frame_length {
            acc += (frame[i] * frame[i]) as f32;
            cum[i] = acc as f64;
        }
        // difference function as librosa computes it: 2 (r0 - rk) - cumsum(x^2)[k - 1], with cumsum[0] := 0
        let mut d = vec![0.0f64; max_period + 1];
        for k in 1..=max_period {
            let prev = if k == 1 { 0.0 } else { cum[k - 1] };
            d[k] = f32r(2.0 * (f32r(acf[0]) - f32r(acf[k])) - prev);
        }
        // cumulative mean normalisation (float32 cumsum divided by an int64 range: float64)
        let mut cm = vec![0.0f64; max_period + 1];
        let mut s = 0.0f32;
        for k in 1..=max_period {
            s += d[k] as f32;
            cm[k] = s as f64 / k as f64;
        }
        let yinf: Vec<f64> = (min_period..=max_period).map(|k| d[k] / (cm[k] + TINY64)).collect();
        let m = yinf.len();
        let mut shifts = vec![0.0; m];
        for i in 1..m.saturating_sub(1) {
            let a = yinf[i + 1] + yinf[i - 1] - 2.0 * yinf[i];
            let b = (yinf[i + 1] - yinf[i - 1]) / 2.0;
            shifts[i] = if b.abs() >= a.abs() { 0.0 } else { -b / a };
        }
        let mut trough = vec![false; m];
        for i in 1..m.saturating_sub(1) {
            trough[i] = yinf[i] < yinf[i - 1] && yinf[i] <= yinf[i + 1];
        }
        if m >= 2 {
            trough[m - 1] = yinf[m - 1] < yinf[m - 2];
            trough[0] = yinf[0] < yinf[1];
        }
        let thr: Vec<bool> = (0..m).map(|i| trough[i] && yinf[i] < 0.1).collect();
        let period = match thr.iter().position(|&b| b) {
            Some(p) => p,
            None => stats::argmin(&yinf),
        };
        let p = min_period as f64 + period as f64 + shifts[period];
        out.push(sr / p);
    }
    out
}

/// librosa.feature.rms(y, frame_length, hop_length) (centred, zero padded)
pub fn rms(y: &[f64], frame_length: usize, hop: usize) -> Vec<f64> {
    let pad = frame_length / 2;
    let n = y.len();
    let n_frames = 1 + n / hop;
    (0..n_frames)
        .map(|f| {
            let mut acc = 0.0f32;
            for i in 0..frame_length {
                let j = (f * hop + i) as i64 - pad as i64;
                if j >= 0 && (j as usize) < n {
                    acc += (y[j as usize] * y[j as usize]) as f32;
                }
            }
            (acc / frame_length as f32).sqrt() as f64
        })
        .collect()
}
