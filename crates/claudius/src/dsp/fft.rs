//! FFT helpers: overlap-add convolution (reverb impulse responses) and magnitude spectrograms (analysis).

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
