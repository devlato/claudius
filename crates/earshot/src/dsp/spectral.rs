//! scipy.signal.welch, scipy.signal.stft (magnitudes) and resample_poly(x, 4, 1) as used by earshot.

use super::fft::{C64, Rfft};
use super::{hann_periodic, kaiser_symmetric, rfftfreq, sinc};

/// scipy.signal.welch(x, fs, nperseg) with the defaults earshot uses: periodic Hann, noverlap = nperseg // 2,
/// constant detrend, 'density' scaling, one-sided, mean average. Returns (f, Pxx).
pub fn welch(x: &[f64], fs: f64, nperseg: usize) -> (Vec<f64>, Vec<f64>) {
    let nperseg = nperseg.min(x.len()).max(1);
    let win = hann_periodic(nperseg);
    let noverlap = nperseg / 2;
    let step = nperseg - noverlap;
    let nseg = if x.len() >= nperseg { (x.len() - noverlap) / step } else { 0 };
    let nbins = nperseg / 2 + 1;
    let scale = 1.0 / (fs * win.iter().map(|w| w * w).sum::<f64>());
    let mut fft = Rfft::new(nperseg);
    let mut buf = vec![0.0; nperseg];
    let mut spec = vec![C64::new(0.0, 0.0); nbins];
    let mut acc = vec![0.0; nbins];
    for s in 0..nseg {
        let seg = &x[s * step..s * step + nperseg];
        let m = seg.iter().sum::<f64>() / nperseg as f64;
        for (i, v) in seg.iter().enumerate() {
            buf[i] = (v - m) * win[i];
        }
        fft.process(&mut buf, &mut spec);
        for (a, c) in acc.iter_mut().zip(&spec) {
            *a += (c.re * c.re + c.im * c.im) * scale;
        }
    }
    let last = if nperseg.is_multiple_of(2) { nbins - 1 } else { nbins };
    for (k, a) in acc.iter_mut().enumerate() {
        if k >= 1 && k < last {
            *a *= 2.0;
        }
        if nseg > 0 {
            *a /= nseg as f64;
        }
    }
    (rfftfreq(nperseg, fs), acc)
}

/// |scipy.signal.stft(x, fs, nperseg, noverlap, boundary=None)| (padded=True, 'spectrum' scaling) as frames x bins
pub fn stft_mag(x: &[f64], nperseg: usize, noverlap: usize) -> Vec<Vec<f64>> {
    let step = nperseg - noverlap;
    let mut y = x.to_vec();
    if y.len() < nperseg {
        return vec![];
    }
    let nadd = ((-((y.len() - nperseg) as i64)).rem_euclid(step as i64) as usize) % nperseg;
    y.extend(std::iter::repeat_n(0.0, nadd));
    let win = hann_periodic(nperseg);
    let scale = 1.0 / win.iter().sum::<f64>();
    let n = (y.len() - nperseg) / step + 1;
    let mut fft = Rfft::new(nperseg);
    let mut buf = vec![0.0; nperseg];
    let mut spec = vec![C64::new(0.0, 0.0); nperseg / 2 + 1];
    (0..n)
        .map(|f| {
            for i in 0..nperseg {
                buf[i] = y[f * step + i] * win[i];
            }
            fft.process(&mut buf, &mut spec);
            spec.iter().map(|c| c.norm() * scale).collect()
        })
        .collect()
}

/// max |scipy.signal.resample_poly(x, 4, 1)| (Kaiser beta 5, 81 taps, zero padding): the true-peak reading
pub fn upsample4_peak(x: &[f32]) -> f64 {
    let up = 4usize;
    let half = 10 * up;
    let ntaps = 2 * half + 1;
    let win = kaiser_symmetric(ntaps, 5.0);
    let mut h: Vec<f64> = (0..ntaps).map(|i| 0.25 * sinc(0.25 * (i as f64 - half as f64)) * win[i]).collect();
    let s: f64 = h.iter().sum();
    for v in &mut h {
        *v = (*v / s) as f32 as f64 * up as f64; // firwin output is cast to the input dtype (float32) before *= up
    }
    let n = x.len();
    let mut peak = 0.0f64;
    // y[m] = sum_i x[i] h[m + half - up i], m in [0, up n)
    for m in 0..up * n {
        let t = m + half;
        let i_hi = (t / up).min(n.saturating_sub(1));
        let i_lo = t.saturating_sub(ntaps - 1).div_ceil(up);
        let mut acc = 0.0f64;
        let mut i = i_lo;
        while i <= i_hi {
            acc += x[i] as f64 * h[t - up * i];
            i += 1;
        }
        let a = (acc as f32).abs() as f64;
        if a > peak {
            peak = a;
        }
    }
    peak
}
