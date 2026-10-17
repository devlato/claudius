//! Band-limited interpolation for pitch shifting and sample-rate conversion in one pass: a Kaiser-windowed sinc
//! (16 zero crossings per side). When reading faster than one source sample per output sample (pitching up), the
//! kernel is widened by the step so content above the output Nyquist is filtered out instead of aliasing.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// zero crossings per side at full bandwidth
const HALF: usize = 16;
/// fine table resolution (points per source sample)
const RES: usize = 1024;
/// fraction of the source Nyquist kept when not pitching up (leaves room for the transition band)
pub const BASE_CUTOFF: f32 = 0.92;
const BETA: f64 = 8.6;
const PHASES: usize = 256;

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..64 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-16 {
            break;
        }
    }
    sum
}

/// windowed sinc h(t), t in [0, HALF], sampled at 1/RES
static TABLE: LazyLock<Vec<f32>> = LazyLock::new(|| {
    let n = HALF * RES + 2;
    let i0b = bessel_i0(BETA);
    (0..n)
        .map(|i| {
            let t = i as f64 / RES as f64;
            if t >= HALF as f64 {
                return 0.0;
            }
            let sinc = if t == 0.0 { 1.0 } else { (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t) };
            let u = t / HALF as f64;
            let w = bessel_i0(BETA * (1.0 - u * u).max(0.0).sqrt()) / i0b;
            (sinc * w) as f32
        })
        .collect()
});

#[inline]
fn kernel(x: f32) -> f32 {
    let t = x.abs() * RES as f32;
    let i = t as usize;
    if i + 1 >= TABLE.len() {
        return 0.0;
    }
    let f = t - i as f32;
    TABLE[i] + f * (TABLE[i + 1] - TABLE[i])
}

/// polyphase rows for one cutoff: row p holds the weights for fractional position p / PHASES
struct Poly {
    lo: isize,
    taps: usize,
    rows: Vec<f32>,
}

/// cutoffs on a 1/16-octave grid below BASE_CUTOFF: grid point k is BASE_CUTOFF * 2^(-k/16)
const GRID: f64 = 16.0;

fn build_poly(k: u32) -> Poly {
    let c = (BASE_CUTOFF as f64 * 2f64.powf(-(k as f64) / GRID)) as f32;
    let reach = (HALF as f32 / c).ceil() as isize;
    let lo = -reach + 1;
    let taps = (2 * reach) as usize;
    let mut rows = vec![0.0f32; (PHASES + 1) * taps];
    for p in 0..=PHASES {
        let f = p as f32 / PHASES as f32;
        let row = &mut rows[p * taps..(p + 1) * taps];
        let mut sum = 0.0;
        for (i, w) in row.iter_mut().enumerate() {
            let j = lo + i as isize;
            *w = c * kernel((j as f32 - f) * c);
            sum += *w;
        }
        if sum.abs() > 1e-9 {
            row.iter_mut().for_each(|w| *w /= sum);
        }
    }
    Poly { lo, taps, rows }
}

static POLYS: LazyLock<Mutex<HashMap<u32, Arc<Poly>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn poly(k: u32) -> Arc<Poly> {
    let mut m = POLYS.lock().unwrap_or_else(|e| e.into_inner());
    m.entry(k).or_insert_with(|| Arc::new(build_poly(k))).clone()
}

/// Interpolator for one playback step (source samples per output sample).
pub struct Interp {
    poly: Arc<Poly>,
    lo: isize,
    taps: usize,
    weights: Vec<f32>,
}

impl Interp {
    pub fn new(step: f64) -> Interp {
        // pitching up: cut at the output Nyquist, rounded down to the grid so nothing above it can alias
        let k = if step > 1.0 { (step.log2() * GRID - 1e-9).ceil().clamp(0.0, 8.0 * GRID) as u32 } else { 0 };
        let poly = poly(k);
        let (lo, taps) = (poly.lo, poly.taps);
        Interp { poly, lo, taps, weights: vec![0.0; taps] }
    }

    #[inline]
    fn fill(&mut self, f: f32) {
        let x = f * PHASES as f32;
        let p = (x as usize).min(PHASES - 1);
        let fr = x - p as f32;
        let t = self.taps;
        let rows = &self.poly.rows;
        let (r0, r1) = (&rows[p * t..(p + 1) * t], &rows[(p + 1) * t..(p + 2) * t]);
        for k in 0..t {
            self.weights[k] = r0[k] + fr * (r1[k] - r0[k]);
        }
    }

    /// Interpolated stereo value at fractional source position `pos`; samples outside the buffer are silence.
    #[inline]
    pub fn at(&mut self, l: &[f32], r: &[f32], pos: f64) -> (f32, f32) {
        let i0 = pos.floor();
        let f = (pos - i0) as f32;
        let i0 = i0 as isize;
        self.fill(f);
        let start = i0 + self.lo;
        let n = l.len() as isize;
        let (mut al, mut ar) = (0.0f32, 0.0f32);
        if start >= 0 && start + self.taps as isize <= n {
            let s = start as usize;
            let (ls, rs) = (&l[s..s + self.taps], &r[s..s + self.taps]);
            for k in 0..self.taps {
                let w = self.weights[k];
                al += w * ls[k];
                ar += w * rs[k];
            }
        } else {
            for k in 0..self.taps {
                let i = start + k as isize;
                if i >= 0 && i < n {
                    let w = self.weights[k];
                    al += w * l[i as usize];
                    ar += w * r[i as usize];
                }
            }
        }
        (al, ar)
    }

    /// Like `at`, but indices at or beyond `loop_end` continue from `loop_start` (seamless sustain loops).
    #[inline]
    pub fn at_looped(&mut self, l: &[f32], r: &[f32], pos: f64, loop_start: usize, loop_end: usize) -> (f32, f32) {
        let i0 = pos.floor();
        let f = (pos - i0) as f32;
        let i0 = i0 as isize;
        self.fill(f);
        let start = i0 + self.lo;
        let n = l.len() as isize;
        let len = (loop_end - loop_start).max(1) as isize;
        let (mut al, mut ar) = (0.0f32, 0.0f32);
        for k in 0..self.taps {
            let mut i = start + k as isize;
            if i >= loop_end as isize {
                i = loop_start as isize + (i - loop_start as isize).rem_euclid(len);
            }
            if i >= 0 && i < n {
                let w = self.weights[k];
                al += w * l[i as usize];
                ar += w * r[i as usize];
            }
        }
        (al, ar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, sr: f64, n: usize) -> Vec<f32> {
        (0..n).map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sr).sin() as f32).collect()
    }

    fn resample(x: &[f32], step: f64, n_out: usize) -> Vec<f32> {
        let mut it = Interp::new(step);
        (0..n_out).map(|i| it.at(x, x, i as f64 * step).0).collect()
    }

    /// frequency from zero crossings over the steady middle part
    fn freq(y: &[f32], sr: f64) -> f64 {
        let y = &y[y.len() / 4..3 * y.len() / 4];
        let mut first = None;
        let mut last = 0usize;
        let mut count = 0usize;
        for i in 1..y.len() {
            if y[i - 1] < 0.0 && y[i] >= 0.0 {
                if first.is_none() {
                    first = Some(i);
                } else {
                    count += 1;
                }
                last = i;
            }
        }
        count as f64 * sr / (last - first.unwrap()) as f64
    }

    fn rms(y: &[f32]) -> f32 {
        let y = &y[y.len() / 4..3 * y.len() / 4];
        (y.iter().map(|v| v * v).sum::<f32>() / y.len() as f32).sqrt()
    }

    #[test]
    fn pitch_and_rate_are_exact() {
        // 440 Hz at 44.1 kHz, played 7 semitones up at 48 kHz
        let x = sine(440.0, 44100.0, 44100);
        let step = 2f64.powf(7.0 / 12.0) * 44100.0 / 48000.0;
        let y = resample(&x, step, 24000);
        let f = freq(&y, 48000.0);
        let want = 440.0 * 2f64.powf(7.0 / 12.0);
        assert!((f - want).abs() < 0.5, "{f} vs {want}");
        assert!((rms(&y) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01, "passband gain {}", rms(&y));
        // down an octave
        let y = resample(&x, 0.5, 48000);
        assert!((freq(&y, 44100.0) - 220.0).abs() < 0.3);
    }

    #[test]
    fn pitching_up_does_not_alias() {
        // 15 kHz at 44.1 kHz up an octave would be 30 kHz: above the 22.05 kHz output Nyquist, must vanish
        let x = sine(15000.0, 44100.0, 44100);
        let y = resample(&x, 2.0, 20000);
        assert!(rms(&y) < 0.003, "aliasing rms {}", rms(&y));
        // 5 kHz up an octave = 10 kHz, kept
        let x = sine(5000.0, 44100.0, 44100);
        let y = resample(&x, 2.0, 20000);
        assert!((rms(&y) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02, "rms {}", rms(&y));
    }

    #[test]
    fn identity_and_dc() {
        // band-limited content passes unchanged at step 1
        let x: Vec<f32> = (0..2000)
            .map(|i| {
                let t = i as f64 / 44100.0;
                let w = 2.0 * std::f64::consts::PI;
                (0.4 * (w * 1000.0 * t).sin() + 0.3 * (w * 3300.0 * t).sin() + 0.2 * (w * 7700.0 * t).sin()) as f32
            })
            .collect();
        let mut it = Interp::new(1.0);
        for i in 100..1900 {
            let (v, _) = it.at(&x, &x, i as f64);
            assert!((v - x[i]).abs() < 2e-3, "{v} vs {}", x[i]);
        }
        let dc = vec![1.0f32; 4000];
        let mut it = Interp::new(3.3);
        let (v, _) = it.at(&dc, &dc, 2000.37);
        assert!((v - 1.0).abs() < 1e-3);
    }
}
