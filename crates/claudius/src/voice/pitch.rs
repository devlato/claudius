//! A voice's pitch every 10 ms. The recording is filtered and decimated to 8 kHz (a voice's fundamental lies far below
//! 4 kHz), and YIN (de Cheveigné and Kawahara 2002) finds its period in each frame. The period is then found again at
//! the full rate, near where YIN found it: at 8 kHz a period is known to a twentieth of a sample, which near the top of
//! a voice is several cents, and a tuner and an assessment work in cents.

use crate::audio::Stereo;
use crate::config::SRF;

/// the rate YIN runs at, and the hop between frames (10 ms)
pub const RATE: f64 = 8_000.0;
pub const HOP: usize = 80;
/// how long a frame is, in seconds
pub const FRAME_SECS: f64 = HOP as f64 / RATE;
/// YIN's window (50 ms), and the pitches it looks for: 50 Hz to 1 kHz
const WINDOW: usize = 400;
const TAU_MAX: usize = (RATE / 50.0) as usize;
const TAU_MIN: usize = (RATE / 1000.0) as usize;
/// how clearly periodic a frame has to be (YIN's absolute threshold)
const THRESHOLD: f32 = 0.15;

/// one frame's reading
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// the pitch the frame holds, as a fractional MIDI note
    pub pitch: Option<f32>,
    /// how clearly it holds it: 1 less YIN's dip there, from 0 to 1
    pub clarity: f32,
    /// its loudness, dB
    pub db: f32,
}

/// the middle of frame `k`, in seconds from the start of the recording
pub fn time(k: usize) -> f64 {
    (k * HOP + WINDOW / 2) as f64 / RATE
}

/// the frame whose middle is nearest `secs`
pub fn frame_at(secs: f64) -> usize {
    ((secs * RATE - (WINDOW / 2) as f64) / HOP as f64).round().max(0.0) as usize
}

/// a fractional MIDI note's frequency, Hz
pub fn hz(pitch: f64) -> f64 {
    440.0 * 2f64.powf((pitch - 69.0) / 12.0)
}

/// the pitch and loudness of every 10 ms of `x`
pub fn track(x: &Stereo) -> Vec<Frame> {
    frames(x, true)
}

/// the same to a few cents, without finding each period again at the full rate (for a quick look at a whole song)
pub fn rough(x: &Stereo) -> Vec<Frame> {
    frames(x, false)
}

fn frames(x: &Stereo, exact: bool) -> Vec<Frame> {
    let mono = x.mono();
    let low = decimate(&mono);
    let n = low.len().saturating_sub(WINDOW + TAU_MAX) / HOP;
    let mut out: Vec<Frame> = (0..n).map(|k| yin(&low[k * HOP..k * HOP + WINDOW + TAU_MAX])).collect();
    // a frame far quieter than the recording's loudest is breath or the room, whatever pitch it seems to have
    let loudest = out.iter().map(|f| f.db).fold(-120f32, f32::max);
    for f in out.iter_mut() {
        if f.db < (loudest - 35.0).max(-60.0) {
            f.pitch = None;
        }
    }
    octaves(&mut out);
    if exact {
        for (k, f) in out.iter_mut().enumerate() {
            if let Some(p) = f.pitch {
                f.pitch = Some(refine(&mono, time(k), p));
            }
        }
    }
    out
}

/// low-passed twice at 3 kHz and decimated from 48 kHz to 8 kHz
fn decimate(x: &[f32]) -> Vec<f32> {
    let step = (SRF / RATE).round() as usize;
    let (b, a) = lowpass(3_000.0, SRF);
    let mut stages = [[0f32; 4]; 2];
    let mut out = Vec::with_capacity(x.len() / step + 1);
    for (i, &v) in x.iter().enumerate() {
        let mut v = v;
        for s in stages.iter_mut() {
            let y = b[0] * v + b[1] * s[0] + b[2] * s[1] - a[0] * s[2] - a[1] * s[3];
            *s = [v, s[0], y, s[2]];
            v = y;
        }
        if i % step == 0 {
            out.push(v);
        }
    }
    out
}

/// an RBJ low-pass biquad: (b0, b1, b2), (a1, a2), normalised
fn lowpass(hz: f64, rate: f64) -> ([f32; 3], [f32; 2]) {
    let w = std::f64::consts::TAU * hz / rate;
    let alpha = w.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let (cos, a0) = (w.cos(), 1.0 + alpha);
    let b1 = (1.0 - cos) / a0;
    ([(b1 / 2.0) as f32, b1 as f32, (b1 / 2.0) as f32], [(-2.0 * cos / a0) as f32, ((1.0 - alpha) / a0) as f32])
}

fn yin(x: &[f32]) -> Frame {
    let w = &x[..WINDOW];
    let db = 10.0 * (w.iter().map(|v| v * v).sum::<f32>() / WINDOW as f32).max(1e-12).log10();
    // under -60 dB a frame holds no pitch whatever YIN finds (`frames` would take it away): spare it the search
    if db < -60.0 {
        return Frame { pitch: None, clarity: 0.0, db };
    }
    // the difference function, and its cumulative mean normalised form
    let mut d = vec![0f32; TAU_MAX + 1];
    for (tau, dt) in d.iter_mut().enumerate().skip(1) {
        *dt = w.iter().zip(&x[tau..tau + WINDOW]).map(|(a, b)| (a - b) * (a - b)).sum();
    }
    let mut cmnd = vec![1f32; TAU_MAX + 1];
    let mut sum = 0f32;
    for tau in 1..=TAU_MAX {
        sum += d[tau];
        cmnd[tau] = if sum > 0.0 { d[tau] * tau as f32 / sum } else { 1.0 };
    }
    let mut tau = TAU_MIN;
    while tau < TAU_MAX && cmnd[tau] >= THRESHOLD {
        tau += 1;
    }
    if tau >= TAU_MAX {
        let dip = cmnd[TAU_MIN..TAU_MAX].iter().copied().fold(1f32, f32::min);
        return Frame { pitch: None, clarity: (1.0 - dip).clamp(0.0, 1.0), db };
    }
    while tau + 1 < TAU_MAX && cmnd[tau + 1] < cmnd[tau] {
        tau += 1;
    }
    // between samples: the parabola through the minimum and its neighbours
    let (l, c, r) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
    let bend = l - 2.0 * c + r;
    let exact = tau as f32 + if bend.abs() > 1e-9 { 0.5 * (l - r) / bend } else { 0.0 };
    let hz = RATE as f32 / exact;
    Frame { pitch: Some(69.0 + 12.0 * (hz / 440.0).log2()), clarity: (1.0 - c).clamp(0.0, 1.0), db }
}

/// a voiced frame more than a third from the median of its five neighbours is an octave or a harmonic jumped to: it
/// takes the median, and the rest keep their own pitch, vibrato and all
fn octaves(frames: &mut [Frame]) {
    let pitches: Vec<Option<f32>> = frames.iter().map(|f| f.pitch).collect();
    for (i, f) in frames.iter_mut().enumerate() {
        let Some(p) = f.pitch else { continue };
        let mut near: Vec<f32> =
            pitches[i.saturating_sub(2)..(i + 3).min(pitches.len())].iter().flatten().copied().collect();
        near.sort_by(f32::total_cmp);
        let median = near[near.len() / 2];
        if (p - median).abs() > 4.0 {
            f.pitch = Some(median);
        }
    }
}

/// The pitch near `p` found again at the full rate in `x`, around `secs`: the period within 3% of the one YIN found
/// that differs least from itself over 20 ms, between samples by a parabola. `p` itself where `x` is too short.
fn refine(x: &[f32], secs: f64, p: f32) -> f32 {
    let period = SRF / hz(p as f64);
    let w = (0.02 * SRF) as usize;
    let (lo, hi) = ((period * 0.97).floor() as usize, (period * 1.03).ceil() as usize + 1);
    let start = (secs * SRF) as i64 - (w / 2) as i64;
    if start < 0 || lo < 2 || start as usize + w + hi + 1 > x.len() {
        return p;
    }
    let s = start as usize;
    let diff = |tau: usize| -> f64 {
        x[s..s + w]
            .iter()
            .zip(&x[s + tau..s + tau + w])
            .map(|(a, b)| {
                let e = (a - b) as f64;
                e * e
            })
            .sum()
    };
    let d: Vec<f64> = (lo - 1..=hi).map(diff).collect();
    let Some(i) = (1..d.len() - 1).min_by(|&a, &b| d[a].total_cmp(&d[b])) else { return p };
    // the least difference at the edge of the search: the pitch moves faster than the window, and YIN's stands
    if i == 1 || i == d.len() - 2 {
        return p;
    }
    let (l, c, r) = (d[i - 1], d[i], d[i + 1]);
    let bend = l - 2.0 * c + r;
    let exact = (lo - 1 + i) as f64 + if bend.abs() > 1e-12 { 0.5 * (l - r) / bend } else { 0.0 };
    (69.0 + 12.0 * (SRF / exact / 440.0).log2()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a tone at `pitch` with two harmonics, `secs` long
    fn tone(pitch: f64, secs: f64) -> Stereo {
        let f = hz(pitch);
        let l: Vec<f32> = (0..(secs * SRF) as usize)
            .map(|i| {
                let ph = std::f64::consts::TAU * f * i as f64 / SRF;
                (0.3 * ph.sin() + 0.15 * (2.0 * ph).sin() + 0.08 * (3.0 * ph).sin()) as f32
            })
            .collect();
        Stereo { l: l.clone(), r: l }
    }

    #[test]
    fn a_held_pitch_is_read_to_a_cent_from_low_to_high() {
        for pitch in [43.27, 57.0, 69.5, 76.83, 81.1] {
            let frames = track(&tone(pitch, 0.6));
            let held: Vec<f32> = frames[5..frames.len() - 5].iter().map(|f| f.pitch.unwrap()).collect();
            let worst = held.iter().map(|p| (p - pitch as f32).abs()).fold(0f32, f32::max);
            assert!(worst < 0.01, "{pitch}: off by {worst} semitones");
            assert!(frames[10].clarity > 0.9, "{:?}", frames[10]);
        }
    }

    #[test]
    fn a_frame_is_placed_at_its_middle() {
        assert_eq!(frame_at(time(37)), 37);
        assert!((time(1) - time(0) - FRAME_SECS).abs() < 1e-12);
        assert!((hz(69.0) - 440.0).abs() < 1e-9 && (hz(81.0) - 880.0).abs() < 1e-9);
    }

    #[test]
    fn an_octave_jumped_to_for_a_frame_goes_back_and_vibrato_stays() {
        let f = |p: f32| Frame { pitch: Some(p), clarity: 0.9, db: -20.0 };
        let mut frames: Vec<Frame> = [60.0, 60.3, 72.1, 60.2, 59.8, 60.1].iter().map(|&p| f(p)).collect();
        octaves(&mut frames);
        let got: Vec<f32> = frames.iter().map(|f| f.pitch.unwrap()).collect();
        assert_eq!(got, [60.0, 60.3, 60.2, 60.2, 59.8, 60.1]);
    }
}
