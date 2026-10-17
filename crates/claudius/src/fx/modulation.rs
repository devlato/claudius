//! Modulation and stereo: chorus, flanger, phaser, width (M/S with mono bass), tremolo, auto-pan, trance gate.

use super::{Ctx, blend};
use crate::audio::Stereo;
use crate::dsp::filter::{self, coef};

const TAU: f64 = std::f64::consts::TAU;

/// fractional read from a delay line written up to index `w` (linear interpolation)
#[inline]
fn read(buf: &[f32], w: usize, delay: f64) -> f32 {
    let size = buf.len();
    let mut r = w as f64 - delay.max(1.0);
    while r < 0.0 {
        r += size as f64;
    }
    let i0 = r as usize % size;
    let i1 = (i0 + 1) % size;
    let fr = (r - r.floor()) as f32;
    buf[i0] * (1.0 - fr) + buf[i1] * fr
}

fn mod_delay(x: &mut [f32], sr: f64, rate: f64, phase: f64, base_ms: f64, depth_ms: f64, fb: f32) -> Vec<f32> {
    let size = ((base_ms + depth_ms + 5.0) / 1000.0 * sr) as usize + 4;
    let mut buf = vec![0.0f32; size];
    let mut out = vec![0.0f32; x.len()];
    let mut w = 0usize;
    for i in 0..x.len() {
        let lfo = 0.5 + 0.5 * (TAU * (rate * i as f64 / sr + phase)).sin();
        let d = (base_ms + depth_ms * lfo) / 1000.0 * sr;
        let y = read(&buf, w, d);
        buf[w] = x[i] + fb * y;
        out[i] = y;
        w = (w + 1) % size;
    }
    out
}

pub fn chorus(x: &mut Stereo, rate: f32, depth_ms: f32, delay_ms: f32, mix: f32, sr: f64) {
    let dry = x.clone();
    let (r, dm, bm) = (rate as f64, depth_ms as f64, delay_ms as f64);
    let mut wet = Stereo::zeros(x.len());
    for (c, ch) in x.channels_mut().into_iter().enumerate() {
        let mut acc = vec![0.0f32; ch.len()];
        for v in 0..2 {
            let ph = c as f64 * 0.25 + v as f64 * 0.5;
            let y = mod_delay(ch, sr, r * (1.0 + 0.13 * v as f64), ph, bm * (1.0 + 0.3 * v as f64), dm, 0.0);
            acc.iter_mut().zip(&y).for_each(|(a, b)| *a += 0.5 * b);
        }
        if c == 0 { wet.l = acc } else { wet.r = acc }
    }
    *x = wet;
    blend(&dry, x, mix * 0.5);
}

pub fn flanger(x: &mut Stereo, rate: f32, depth_ms: f32, delay_ms: f32, feedback: f32, mix: f32, sr: f64) {
    let dry = x.clone();
    for (c, ch) in x.channels_mut().into_iter().enumerate() {
        let y = mod_delay(
            ch,
            sr,
            rate as f64,
            c as f64 * 0.2,
            delay_ms as f64,
            depth_ms as f64,
            feedback.clamp(-0.95, 0.95),
        );
        ch.copy_from_slice(&y);
    }
    blend(&dry, x, mix * 0.5);
}

pub fn phaser(x: &mut Stereo, rate: f32, stages: u32, depth: f32, feedback: f32, mix: f32, sr: f64) {
    let dry = x.clone();
    let stages = stages.clamp(2, 12) as usize;
    let (fmin, fmax) = (250.0f64, 250.0 + 3500.0 * depth.clamp(0.0, 1.0) as f64);
    for (c, ch) in x.channels_mut().into_iter().enumerate() {
        let mut z = vec![0.0f64; stages];
        let mut last = 0.0f64;
        for i in 0..ch.len() {
            let lfo = 0.5 + 0.5 * (TAU * (rate as f64 * i as f64 / sr + c as f64 * 0.25)).sin();
            let f = fmin * (fmax / fmin).powf(lfo);
            let t = (std::f64::consts::PI * f / sr).tan();
            let a = (t - 1.0) / (t + 1.0);
            let mut v = ch[i] as f64 + feedback.clamp(-0.9, 0.9) as f64 * last;
            for s in z.iter_mut() {
                let out = a * v + *s;
                *s = v - a * out;
                v = out;
            }
            last = v;
            ch[i] = v as f32;
        }
    }
    blend(&dry, x, mix * 0.5);
}

/// M/S width (0 = mono, 1 = as is, 2 = twice the side); side below `mono_below_hz` removed (mono bass)
pub fn width(x: &mut Stereo, width: f32, mono_below_hz: f32, sr: f64) {
    let n = x.len();
    let mut side: Vec<f32> = (0..n).map(|i| 0.5 * (x.l[i] - x.r[i]) * width).collect();
    if mono_below_hz > 20.0 {
        filter::butter(&mut side, true, mono_below_hz as f64, sr, 4);
    }
    for i in 0..n {
        let m = 0.5 * (x.l[i] + x.r[i]);
        x.l[i] = m + side[i];
        x.r[i] = m - side[i];
    }
}

pub fn tremolo(x: &mut Stereo, beats: f32, depth: f32, ctx: &Ctx) {
    let period = beats.max(0.01) as f64 * ctx.spb();
    for i in 0..x.len() {
        let g = 1.0 - depth * (0.5 - 0.5 * (TAU * i as f64 / period).cos()) as f32;
        x.l[i] *= g;
        x.r[i] *= g;
    }
}

pub fn autopan(x: &mut Stereo, beats: f32, depth: f32, ctx: &Ctx) {
    let period = beats.max(0.01) as f64 * ctx.spb();
    for i in 0..x.len() {
        let p = (TAU * i as f64 / period).sin() as f32 * depth;
        let (gl, gr) = crate::audio::balance_gains(p);
        x.l[i] *= gl;
        x.r[i] *= gr;
    }
}

/// rhythmic volume gate (trance gate / chopped pads): step gains cycle every steps.len() * step_beats beats
pub fn gate(x: &mut Stereo, steps: &[f32], step_beats: f32, depth: f32, smooth_ms: f32, ctx: &Ctx) {
    if steps.is_empty() {
        return;
    }
    let spb = ctx.spb();
    let a = coef((smooth_ms.max(0.5) / 1000.0) as f64, ctx.sr) as f32;
    let mut g = 1.0f32;
    for i in 0..x.len() {
        let step = ((i as f64 / spb) / step_beats.max(0.01) as f64) as usize % steps.len();
        let target = 1.0 - depth * (1.0 - steps[step].clamp(0.0, 1.0));
        g = a * g + (1.0 - a) * target;
        x.l[i] *= g;
        x.r[i] *= g;
    }
}
