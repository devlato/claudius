//! Sample-rate conversion for a file at another rate than its audio: a polyphase windowed sinc (Kaiser, β 9.5, about
//! 95 dB of stopband) reaching 80 input samples either side, cutting off at 95 % of the lower rate's Nyquist
//! frequency (21 kHz from 48 to 44.1 kHz).

const HALF: usize = 80;
const BETA: f64 = 9.5;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// the modified Bessel function I0, by its series
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-12 * sum {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        k += 1.0;
    }
    sum
}

/// interleaved `x` at `from` Hz, at `to` Hz instead
pub fn convert(x: &[f32], channels: usize, from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() || channels == 0 {
        return x.to_vec();
    }
    let g = gcd(from, to);
    let (up, down) = ((to / g) as usize, (from / g) as usize);
    let frames = x.len() / channels;
    let out_frames = (frames as u64 * to as u64 / from as u64) as usize;
    // the cutoff in cycles per input sample
    let fc = 0.5 * 0.95 * (to.min(from) as f64 / from as f64);
    let taps = 2 * HALF;
    let i0b = bessel_i0(BETA);
    // table[phase][j]: the weight of input sample (base + 1 - HALF + j) for an output that sits phase/up past base
    let table: Vec<Vec<f64>> = (0..up)
        .map(|phase| {
            let mut w: Vec<f64> = (0..taps)
                .map(|j| {
                    let t = phase as f64 / up as f64 + (HALF - 1) as f64 - j as f64;
                    let r = t / HALF as f64;
                    if r.abs() >= 1.0 {
                        return 0.0;
                    }
                    let a = 2.0 * std::f64::consts::PI * fc * t;
                    let sinc = if a.abs() < 1e-12 { 1.0 } else { a.sin() / a };
                    sinc * bessel_i0(BETA * (1.0 - r * r).sqrt()) / i0b
                })
                .collect();
            let sum: f64 = w.iter().sum();
            w.iter_mut().for_each(|v| *v /= sum);
            w
        })
        .collect();
    let one = |c: usize| -> Vec<f32> {
        (0..out_frames)
            .map(|n| {
                let pos = n * down;
                let (base, phase) = (pos / up, pos % up);
                let first = base as isize + 1 - HALF as isize;
                let w = &table[phase];
                let mut acc = 0.0f64;
                for (j, wj) in w.iter().enumerate() {
                    let k = first + j as isize;
                    if k >= 0 && (k as usize) < frames {
                        acc += x[k as usize * channels + c] as f64 * wj;
                    }
                }
                acc as f32
            })
            .collect()
    };
    let planes: Vec<Vec<f32>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..channels).map(|c| s.spawn(move || one(c))).collect();
        handles.into_iter().map(|h| h.join().expect("resampler thread")).collect()
    });
    let mut out = Vec::with_capacity(out_frames * channels);
    for n in 0..out_frames {
        for p in &planes {
            out.push(p[n]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(f: f64, rate: u32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (2.0 * std::f64::consts::PI * f * i as f64 / rate as f64).sin() as f32 * 0.5).collect()
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    #[test]
    fn a_tone_keeps_its_pitch_and_level() {
        let x = sine(1000.0, 48_000, 48_000);
        let y = convert(&x, 1, 48_000, 44_100);
        assert_eq!(y.len(), 44_100);
        let want = sine(1000.0, 44_100, 44_100);
        // the filter is symmetric, so the output lines up with the input: compare away from the edges
        let err: Vec<f32> = y[400..44_000].iter().zip(&want[400..44_000]).map(|(a, b)| a - b).collect();
        let snr = 20.0 * (rms(&want[400..44_000]) / rms(&err)).log10();
        assert!(snr > 90.0, "{snr:.1} dB");
    }

    #[test]
    fn what_cannot_be_kept_is_filtered_out() {
        // 23 kHz exists at 48 kHz and not at 44.1 kHz: it must go, not fold down to 21.1 kHz
        let x = sine(23_000.0, 48_000, 48_000);
        let y = convert(&x, 1, 48_000, 44_100);
        let level = 20.0 * (rms(&y[400..44_000]) / rms(&x)).log10();
        assert!(level < -80.0, "{level:.1} dB left");
        // 20 kHz passes
        let x = sine(20_000.0, 48_000, 48_000);
        let level = 20.0 * (rms(&convert(&x, 1, 48_000, 44_100)[400..44_000]) / rms(&x)).log10();
        assert!(level.abs() < 0.2, "{level:.2} dB");
    }

    #[test]
    fn channels_stay_apart() {
        let (l, r) = (sine(500.0, 48_000, 4800), sine(700.0, 48_000, 4800));
        let x: Vec<f32> = l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect();
        let y = convert(&x, 2, 48_000, 44_100);
        let yl: Vec<f32> = y.iter().step_by(2).copied().collect();
        assert_eq!(yl, convert(&l, 1, 48_000, 44_100));
    }
}
