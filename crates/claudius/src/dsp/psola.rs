//! Pitch-synchronous overlap-add (TD-PSOLA, Moulines and Charpentier 1990): a voice moved in pitch by laying its
//! periods down closer together or further apart. Each grain is two periods of the voice cut around a pitch mark, and
//! keeps its own spectrum, so the voice keeps its formants: it sounds like the same person singing a little higher or
//! lower, not like a tape sped up. Where nothing holds a pitch (a consonant, a breath) the signal passes as it is.
//!
//! The marks are a period apart, each where the voice looks most like it did a period before, so every grain is cut
//! at the same point of its period. A grain's two halves are as long as the periods either side of its mark, so the
//! grains laid back where they were cut add up to the signal itself; moved, they are divided by how much of them
//! overlaps.

use crate::config::SRF;

/// The pitch marks of the voiced stretch of `x` from sample `start` to `end`, a period apart (`period` gives it, in
/// samples, at each sample), as sample indices.
pub fn marks(x: &[f32], start: usize, end: usize, period: &dyn Fn(usize) -> f64) -> Vec<usize> {
    let end = end.min(x.len());
    let p0 = period(start).round() as usize;
    if p0 < 2 || start + 3 * p0 >= end {
        return Vec::new();
    }
    // the first mark: the highest point of the period that starts half a period in, so that a grain fits before it
    let from = start + p0 / 2;
    let first = (from..from + p0).max_by(|&a, &b| x[a].total_cmp(&x[b])).unwrap_or(from);
    let mut out = vec![first];
    loop {
        let m = *out.last().unwrap_or(&first);
        let p = period(m);
        let half = (p / 2.0) as usize;
        let reach = (p / 6.0).max(1.0) as usize;
        let expect = m + p.round() as usize;
        if expect + reach + half >= end || m < half {
            break;
        }
        // where the next period looks most like this one
        let like = |c: usize| -> f64 {
            let (a, b) = (&x[m - half..m + half], &x[c - half..c + half]);
            let (mut ab, mut bb) = (0f64, 0f64);
            for (u, v) in a.iter().zip(b) {
                ab += (*u * *v) as f64;
                bb += (*v * *v) as f64;
            }
            ab / bb.max(1e-12).sqrt()
        };
        let next = (expect - reach..=expect + reach).max_by(|&a, &b| like(a).total_cmp(&like(b))).unwrap_or(expect);
        out.push(next);
    }
    out
}

/// `x` with the voiced stretch between `marks` moved in pitch by `ratio` at each sample (2 is an octave up), and the
/// first and last period crossfaded from and back to `x`
pub fn shift(x: &mut [f32], marks: &[usize], ratio: &dyn Fn(usize) -> f64) {
    if marks.len() < 4 {
        return;
    }
    let (a, b) = (marks[0], marks[marks.len() - 1]);
    let mut acc = vec![0f64; b - a + 1];
    let mut sum = vec![0f64; b - a + 1];
    // each output mark takes the grain at the input mark nearest it, and the next comes a period on, over the ratio
    let mut o = a as f64;
    let mut i = 0;
    while o < b as f64 {
        while i + 1 < marks.len() && (marks[i + 1] as f64 - o).abs() <= (marks[i] as f64 - o).abs() {
            i += 1;
        }
        let (before, after) = match i {
            0 => (marks[1] - marks[0], marks[1] - marks[0]),
            j if j + 1 == marks.len() => (marks[j] - marks[j - 1], marks[j] - marks[j - 1]),
            j => (marks[j] - marks[j - 1], marks[j + 1] - marks[j]),
        };
        let at = o.round() as i64;
        for d in -(before as i64)..(after as i64) {
            let (src, dst) = (marks[i] as i64 + d, at + d);
            if src < 0 || src as usize >= x.len() || dst < a as i64 || dst > b as i64 {
                continue;
            }
            let w = if d < 0 {
                hann_half(1.0 + d as f64 / before as f64)
            } else {
                hann_half(1.0 - d as f64 / after as f64)
            };
            let k = (dst - a as i64) as usize;
            acc[k] += w * x[src as usize] as f64;
            sum[k] += w;
        }
        let r = ratio(at.max(0) as usize).clamp(0.25, 4.0);
        o += after as f64 / r;
    }
    let fade_in = (marks[1] - a).max(1);
    let fade_out = (b - marks[marks.len() - 2]).max(1);
    for k in 0..acc.len() {
        let moved = if sum[k] > 1e-3 { acc[k] / sum[k] } else { x[a + k] as f64 };
        let edge = ((k as f64 / fade_in as f64).min((acc.len() - 1 - k) as f64 / fade_out as f64)).clamp(0.0, 1.0);
        x[a + k] = (x[a + k] as f64 * (1.0 - edge) + moved * edge) as f32;
    }
}

/// the rising half of a Hann window at `t` from 0 to 1
fn hann_half(t: f64) -> f64 {
    0.5 - 0.5 * (std::f64::consts::PI * t.clamp(0.0, 1.0)).cos()
}

/// a period at `hz`, in samples
pub fn period_of(hz: f64) -> f64 {
    SRF / hz.max(20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Stereo;
    use crate::voice::pitch;

    /// a voice-like tone at `hz`, `secs` long: five harmonics with a formant near 900 Hz
    fn voice(hz: f64, secs: f64) -> Vec<f32> {
        (0..(secs * SRF) as usize)
            .map(|i| {
                let ph = std::f64::consts::TAU * hz * i as f64 / SRF;
                (1..=5)
                    .map(|h| {
                        let f = hz * h as f64;
                        let formant = 1.0 / (1.0 + ((f - 900.0) / 400.0).powi(2));
                        (formant * (h as f64 * ph).sin()) as f32 * 0.2
                    })
                    .sum()
            })
            .collect()
    }

    fn pitch_of(x: &[f32]) -> f32 {
        let frames = pitch::track(&Stereo::from_mono(x));
        let mut p: Vec<f32> = frames[10..frames.len() - 10].iter().filter_map(|f| f.pitch).collect();
        p.sort_by(f32::total_cmp);
        p[p.len() / 2]
    }

    #[test]
    fn grains_laid_back_where_they_were_cut_are_the_signal_itself() {
        let x = voice(196.0, 0.5);
        let marks = marks(&x, 0, x.len(), &|_| period_of(196.0));
        assert!(marks.len() > 80, "{}", marks.len());
        assert!(marks.windows(2).all(|w| (w[1] - w[0]).abs_diff(245) <= 1), "a period apart: {:?}", &marks[..5]);
        let mut y = x.clone();
        shift(&mut y, &marks, &|_| 1.0);
        let worst = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(worst < 1e-5, "{worst}");
    }

    #[test]
    fn a_voice_moved_up_a_tone_is_a_tone_higher_and_as_long() {
        let x = voice(196.0, 0.6);
        let marks = marks(&x, 0, x.len(), &|_| period_of(196.0));
        let mut y = x.clone();
        shift(&mut y, &marks, &|_| 2f64.powf(2.0 / 12.0));
        assert_eq!(y.len(), x.len());
        let (before, after) = (pitch_of(&x), pitch_of(&y));
        assert!((after - before - 2.0).abs() < 0.03, "{before} -> {after}");
        // the same loudness, give or take
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        assert!((rms(&y) / rms(&x) - 1.0).abs() < 0.15, "{} {}", rms(&x), rms(&y));
    }

    #[test]
    fn a_stretch_too_short_for_grains_is_left_alone() {
        let x = voice(196.0, 0.005);
        assert!(marks(&x, 0, x.len(), &|_| period_of(196.0)).is_empty());
        let mut y = x.clone();
        shift(&mut y, &[0, 10, 20], &|_| 2.0);
        assert_eq!(x, y);
    }
}
