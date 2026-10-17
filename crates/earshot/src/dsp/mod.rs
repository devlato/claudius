//! Signal-processing building blocks that reproduce the numpy / scipy / soxr behaviour earshot relies on.

pub mod fft;
pub mod filter;
pub mod resample;
pub mod spectral;
pub mod stats;

pub use resample::resample;

/// numpy.hanning(m): symmetric Hann window, computed with numpy's formula
pub fn hanning(m: usize) -> Vec<f64> {
    match m {
        0 => vec![],
        1 => vec![1.0],
        _ => {
            let d = (m - 1) as f64;
            (0..m)
                .map(|i| {
                    let n = 1.0 - m as f64 + 2.0 * i as f64;
                    0.5 + 0.5 * (std::f64::consts::PI * n / d).cos()
                })
                .collect()
        }
    }
}

/// periodic Hann window of length m (scipy get_window('hann', m) / torch.hann_window): np.hanning(m + 1)[:-1]
pub fn hann_periodic(m: usize) -> Vec<f64> {
    let mut w = hanning(m + 1);
    w.pop();
    w
}

/// modified Bessel function of the first kind, order 0 (power series, double precision)
pub fn bessel_i0(x: f64) -> f64 {
    let q = x * x / 4.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut k = 1.0;
    loop {
        term *= q / (k * k);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
        k += 1.0;
    }
    sum
}

/// symmetric Kaiser window (scipy get_window(('kaiser', beta), m, fftbins=False))
pub fn kaiser_symmetric(m: usize, beta: f64) -> Vec<f64> {
    if m == 1 {
        return vec![1.0];
    }
    let a = (m - 1) as f64 / 2.0;
    let d = bessel_i0(beta);
    (0..m)
        .map(|n| {
            let r = (n as f64 - a) / a;
            bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / d
        })
        .collect()
}

/// sin(pi x) / (pi x)
pub fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let y = std::f64::consts::PI * x;
        y.sin() / y
    }
}

/// numpy.fft.rfftfreq(n, 1 / sr)
pub fn rfftfreq(n: usize, sr: f64) -> Vec<f64> {
    (0..n / 2 + 1).map(|k| k as f64 * sr / n as f64).collect()
}

/// 20 log10(max(v, floor)): earshot audio.db
pub fn db(v: f64) -> f64 {
    20.0 * v.max(1e-12).log10()
}

pub fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Python's round(v, d): correctly rounded decimal of the exact binary value, ties to even
pub fn round_to(v: f64, d: usize) -> f64 {
    if !v.is_finite() {
        return v;
    }
    format!("{v:.d$}").parse().unwrap_or(v)
}
