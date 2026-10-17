//! Band-limited resampling that reproduces soxr's 'HQ' recipe (what earshot uses through python-soxr).
//!
//! Measured: soxr HQ's effective filter for a decimation is a linear-phase Kaiser-windowed sinc defined on the output
//! rate, with cut-off 0.47843591 cycles per output sample, beta 13.04 and a half-length of 96.83 output samples
//! (2:1 uses beta 13.05 / 97.29). Evaluated directly (zero signal outside the input, no delay, output length
//! round(n * out / in)) it matches python-soxr within 1e-7 for 48k -> 32k / 16k and 2:1, and within ~1e-4 of
//! full scale for 48k -> 22.05k (soxr interpolates its coefficients for that ratio). No soxr code is used.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::{bessel_i0, gcd, sinc};

const FC: f64 = 0.478_435_91;
const BETA: f64 = 13.040_070_6;
const HALF: f64 = 96.833_449_19;
const BETA_2: f64 = 13.051_475_82;
const HALF_2: f64 = 97.291_585_99;

/// polyphase coefficients for one reduced ratio up / down: phase p -> (first input offset, taps)
struct Bank {
    up: usize,
    down: usize,
    phases: Vec<(i64, Vec<f64>)>,
}

fn design(up: usize, down: usize) -> Bank {
    let (beta, half) = if up == 1 && down == 2 { (BETA_2, HALF_2) } else { (BETA, HALF) };
    // unit = one sample of the lower rate; s scales the gain (a decimator averages, an interpolator does not)
    let (unit_per_in, s) = if up < down { (up as f64 / down as f64, up as f64 / down as f64) } else { (1.0, 1.0) };
    let i0b = bessel_i0(beta);
    let reach = half / unit_per_in; // filter half-length in input samples
    let mut phases = Vec::with_capacity(up);
    for ph in 0..up {
        // output k with k * down = q * up + ph sits at input time q + ph / up
        let frac = ph as f64 / up as f64;
        let j0 = (frac - reach).ceil() as i64;
        let j1 = (frac + reach).floor() as i64;
        let taps = (j0..=j1)
            .map(|j| {
                let t = (frac - j as f64) * unit_per_in;
                let r = t / half;
                if r.abs() > 1.0 {
                    0.0
                } else {
                    s * 2.0 * FC * sinc(2.0 * FC * t) * bessel_i0(beta * (1.0 - r * r).sqrt()) / i0b
                }
            })
            .collect();
        phases.push((j0, taps));
    }
    Bank { up, down, phases }
}

type BankCache = Mutex<HashMap<(usize, usize), Arc<Bank>>>;

fn bank(up: usize, down: usize) -> Arc<Bank> {
    static CACHE: OnceLock<BankCache> = OnceLock::new();
    let c = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut g = c.lock().unwrap();
    g.entry((up, down)).or_insert_with(|| Arc::new(design(up, down))).clone()
}

/// resample `x` from `sr_from` to `sr_to` (soxr HQ equivalent); output length round(len * sr_to / sr_from)
pub fn resample(x: &[f32], sr_from: u32, sr_to: u32) -> Vec<f32> {
    resample_f64(x, sr_from, sr_to).into_iter().map(|v| v as f32).collect()
}

pub fn resample_f64<T: Copy + Into<f64>>(x: &[T], sr_from: u32, sr_to: u32) -> Vec<f64> {
    if sr_from == sr_to {
        return x.iter().map(|&v| v.into()).collect();
    }
    let g = gcd(sr_from as u64, sr_to as u64);
    let (up, down) = ((sr_to as u64 / g) as usize, (sr_from as u64 / g) as usize);
    let b = bank(up, down);
    let n = x.len();
    let olen = ((2 * n as u128 * up as u128 + down as u128) / (2 * down as u128)) as usize;
    let mut out = vec![0.0f64; olen];
    for (k, o) in out.iter_mut().enumerate() {
        let num = k as u128 * b.down as u128;
        let q = (num / b.up as u128) as i64;
        let ph = (num % b.up as u128) as usize;
        let (j0, taps) = &b.phases[ph];
        let start = q + j0;
        let lo = (-start).max(0) as usize;
        let hi = taps.len().min((n as i64 - start).max(0) as usize);
        let mut acc = 0.0;
        for t in lo..hi {
            acc += taps[t] * x[(start + t as i64) as usize].into();
        }
        *o = acc;
    }
    out
}
