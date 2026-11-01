//! FFT helpers: overlap-add convolution (reverb impulse responses), magnitude spectrograms (analysis), and the
//! sustained part of a sound without its drums (`harmonic`, for the sampler).

use realfft::RealFftPlanner;
use realfft::num_complex::Complex;

/// x convolved with h, length len(x) + len(h) - 1 (uniform overlap-add)
pub fn convolve(x: &[f32], h: &[f32]) -> Vec<f32> {
    let (n, m) = (x.len(), h.len());
    if n == 0 || m == 0 {
        return Vec::new();
    }
    let block = m.next_power_of_two().max(8192);
    let nfft = (block + m - 1).next_power_of_two();
    let mut planner = RealFftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(nfft);
    let inv = planner.plan_fft_inverse(nfft);
    let mut hb = vec![0.0f32; nfft];
    hb[..m].copy_from_slice(h);
    let mut hs = fwd.make_output_vec();
    let _ = fwd.process(&mut hb, &mut hs);
    let mut out = vec![0.0f32; n + m - 1];
    let mut xb = vec![0.0f32; nfft];
    let mut xs = fwd.make_output_vec();
    let mut yb = vec![0.0f32; nfft];
    let scale = 1.0 / nfft as f32;
    let mut pos = 0;
    while pos < n {
        let len = block.min(n - pos);
        xb.iter_mut().for_each(|v| *v = 0.0);
        xb[..len].copy_from_slice(&x[pos..pos + len]);
        let _ = fwd.process(&mut xb, &mut xs);
        for (a, b) in xs.iter_mut().zip(&hs) {
            *a *= *b;
        }
        let last = xs.len() - 1;
        xs[0] = Complex::new(xs[0].re, 0.0);
        xs[last] = Complex::new(xs[last].re, 0.0);
        let _ = inv.process(&mut xs, &mut yb);
        let end = (pos + len + m - 1).min(out.len());
        for i in pos..end {
            out[i] += yb[i - pos] * scale;
        }
        pos += block;
    }
    out
}

/// The part of `x` that sustains, without what strikes: a median across time of its spectrogram is what holds a
/// pitch, a median across frequency what hits all at once, and a soft mask of the first against both keeps the
/// sustained part (harmonic-percussive separation). The same length as `x`.
pub fn harmonic(x: &[f32]) -> Vec<f32> {
    const N: usize = 2048;
    const HOP: usize = N / 4;
    const ACROSS: usize = 8;
    let pad = vec![0.0f32; N];
    let padded: Vec<f32> = pad.iter().chain(x).chain(&pad).copied().collect();
    let win: Vec<f32> =
        (0..N).map(|i| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / N as f64).cos()) as f32).collect();
    let mut planner = RealFftPlanner::<f32>::new();
    let (fwd, inv) = (planner.plan_fft_forward(N), planner.plan_fft_inverse(N));
    let frames = (padded.len() - N) / HOP + 1;
    let mut buf = vec![0.0f32; N];
    let mut spectra = Vec::with_capacity(frames);
    for f in 0..frames {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = padded[f * HOP + i] * win[i];
        }
        let mut spec = fwd.make_output_vec();
        let _ = fwd.process(&mut buf, &mut spec);
        spectra.push(spec);
    }
    let bins = N / 2 + 1;
    let mag: Vec<Vec<f32>> = spectra.iter().map(|s| s.iter().map(|c| c.norm()).collect()).collect();
    let median = |v: &mut Vec<f32>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    let mut out = vec![0.0f32; padded.len()];
    let mut norm = vec![0.0f32; padded.len()];
    let mut held = Vec::with_capacity(2 * ACROSS + 1);
    for (f, spec) in spectra.iter_mut().enumerate() {
        for k in 0..bins {
            held.clear();
            held.extend((f.saturating_sub(ACROSS)..(f + ACROSS + 1).min(frames)).map(|g| mag[g][k]));
            let h = median(&mut held);
            held.clear();
            held.extend(mag[f][k.saturating_sub(ACROSS)..(k + ACROSS + 1).min(bins)].iter().copied());
            let p = median(&mut held);
            let mask = h * h / (h * h + p * p + 1e-12);
            spec[k] *= mask;
        }
        let last = bins - 1;
        spec[0].im = 0.0;
        spec[last].im = 0.0;
        let _ = inv.process(spec, &mut buf);
        for i in 0..N {
            out[f * HOP + i] += buf[i] * win[i] / N as f32;
            norm[f * HOP + i] += win[i] * win[i];
        }
    }
    out.iter().zip(&norm).skip(N).take(x.len()).map(|(v, n)| if *n > 1e-6 { v / n } else { 0.0 }).collect()
}

/// magnitude spectrogram: frames x (n_fft / 2 + 1), periodic Hann window, hop in samples, no centring
pub fn spectrogram(x: &[f32], n_fft: usize, hop: usize) -> Vec<Vec<f32>> {
    if x.len() < n_fft {
        return Vec::new();
    }
    let win: Vec<f32> =
        (0..n_fft).map(|i| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n_fft as f64).cos()) as f32).collect();
    let mut planner = RealFftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(n_fft);
    let mut buf = vec![0.0f32; n_fft];
    let mut spec = fwd.make_output_vec();
    let frames = 1 + (x.len() - n_fft) / hop;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let a = f * hop;
        for i in 0..n_fft {
            buf[i] = x[a + i] * win[i];
        }
        let _ = fwd.process(&mut buf, &mut spec);
        out.push(spec.iter().map(|c| c.norm()).collect());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sustained_part_keeps_a_tone_and_drops_clicks() {
        let sr = 48_000.0;
        let tone: Vec<f32> =
            (0..96_000).map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr).sin() as f32 * 0.4).collect();
        let mut clicks = vec![0.0f32; 96_000];
        for k in (0..96_000).step_by(12_000) {
            clicks[k] = 0.9;
            clicks[k + 1] = -0.9;
        }
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let kept = harmonic(&tone);
        assert_eq!(kept.len(), tone.len());
        assert!((rms(&kept[4_096..90_000]) / rms(&tone[4_096..90_000]) - 1.0).abs() < 0.1, "the tone stays");
        assert!(rms(&harmonic(&clicks)) < rms(&clicks) * 0.25, "the clicks go");
    }
}
