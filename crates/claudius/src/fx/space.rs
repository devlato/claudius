//! Space: a convolution reverb whose impulse response is generated procedurally (decorrelated stereo noise with
//! frequency-dependent decay and early reflections), and a tempo-synced stereo / ping-pong delay.

use super::{Ctx, blend};
use crate::audio::Stereo;
use crate::dsp::fft::convolve;
use crate::dsp::filter::{self, coef};
use crate::dsp::osc;
use crate::rng::Rng;

fn impulse(decay: f32, damping: f32, predelay_ms: f32, sr: f64, rng: &mut Rng) -> Stereo {
    let decay = decay.clamp(0.1, 12.0) as f64;
    let len = ((decay * 1.15 + predelay_ms as f64 / 1000.0) * sr) as usize + 1;
    let pre = (predelay_ms.max(0.0) as f64 / 1000.0 * sr) as usize;
    let mut ir = Stereo::zeros(len);
    // three bands decaying at different rates: highs die faster with damping, lows a little slower
    let bands: [(f64, f64, f64); 3] =
        [(0.0, 500.0, 1.1), (500.0, 4000.0, 1.0), (4000.0, 0.0, 1.0 - 0.75 * damping.clamp(0.0, 1.0) as f64)];
    for (c, ch) in ir.channels_mut().into_iter().enumerate() {
        let mut r = rng.fork(&format!("ir{c}"));
        for (lo, hi, k) in bands {
            let mut nz = osc::noise(len - pre, &mut r, "white");
            if lo > 0.0 {
                filter::butter(&mut nz, true, lo, sr, 4);
            }
            if hi > 0.0 {
                filter::butter(&mut nz, false, hi, sr, 4);
            }
            let t60 = (decay * k).max(0.05);
            let fade_in = 0.004 * sr;
            for (i, v) in nz.iter().enumerate() {
                let t = i as f64 / sr;
                let env = (-6.9 * t / t60).exp() * (i as f64 / fade_in).min(1.0);
                ch[pre + i] += *v * env as f32;
            }
        }
        // early reflections: a few discrete taps in the first 70 ms
        let mut r2 = rng.fork(&format!("er{c}"));
        for k in 0..7 {
            let t = pre as f64 + (0.006 + 0.064 * r2.f64()) * sr;
            let g = 0.55 * (1.0 - k as f64 / 8.0) * if r2.chance(0.5) { 1.0 } else { -1.0 };
            let i = t as usize;
            if i < len {
                ch[i] += (g * 3.0) as f32;
            }
        }
    }
    let e: f64 = ir.l.iter().chain(&ir.r).map(|v| (*v as f64).powi(2)).sum::<f64>() / 2.0;
    ir.scale((1.0 / e.sqrt().max(1e-9)) as f32);
    ir
}

/// convolution reverb; mix = wet share (1.0 for a send bus)
#[allow(clippy::too_many_arguments)]
pub fn reverb(
    x: &mut Stereo,
    decay: f32,
    damping: f32,
    predelay_ms: f32,
    width: f32,
    mix: f32,
    lowcut: f32,
    ctx: &Ctx,
) {
    let mut rng = Rng::new(ctx.seed ^ 0x2e7b);
    let ir = impulse(decay, damping, predelay_ms, ctx.sr, &mut rng);
    let n = x.len();
    // a little cross-feed so a hard-panned source still fills both sides
    let wl = convolve(&x.l.iter().zip(&x.r).map(|(a, b)| 0.8 * a + 0.2 * b).collect::<Vec<_>>(), &ir.l);
    let wr = convolve(&x.r.iter().zip(&x.l).map(|(a, b)| 0.8 * a + 0.2 * b).collect::<Vec<_>>(), &ir.r);
    let mut wet = Stereo { l: wl[..n.min(wl.len())].to_vec(), r: wr[..n.min(wr.len())].to_vec() };
    wet.ensure_len(n);
    // a noise-built tail reads as hiss above ~9 kHz: real rooms and plates are darker up there
    let top = 9000.0 - 4000.0 * damping.clamp(0.0, 1.0) as f64;
    for ch in wet.channels_mut() {
        if lowcut > 20.0 {
            filter::butter(ch, true, lowcut as f64, ctx.sr, 2);
        }
        filter::butter(ch, false, top, ctx.sr, 2);
    }
    let w = width.clamp(0.0, 2.0);
    for i in 0..n {
        let m = 0.5 * (wet.l[i] + wet.r[i]);
        let s = 0.5 * (wet.l[i] - wet.r[i]) * w;
        wet.l[i] = (m + s) * 0.35;
        wet.r[i] = (m - s) * 0.35;
    }
    let dry = x.clone();
    *x = wet;
    blend(&dry, x, mix);
}

/// tempo-synced feedback delay (beats), repeats darkened by `damping`; output = dry + mix * repeats
pub fn delay(x: &mut Stereo, beats: f32, feedback: f32, damping: f32, pingpong: bool, mix: f32, ctx: &Ctx) {
    let d = ((beats as f64 * ctx.spb()).round() as usize).max(1);
    let n = x.len();
    let fb = feedback.clamp(0.0, 0.95);
    let damp = coef(0.00002 + damping.clamp(0.0, 1.0) as f64 * 0.0004, ctx.sr) as f32;
    let mut bl = vec![0.0f32; d];
    let mut br = vec![0.0f32; d];
    let (mut lpl, mut lpr) = (0.0f32, 0.0f32);
    let (mut hpl, mut hpr) = (0.0f32, 0.0f32);
    let hp = coef(1.0 / (2.0 * std::f64::consts::PI * 180.0), ctx.sr) as f32;
    let mut w = 0;
    for i in 0..n {
        let (dl, dr) = (bl[w], br[w]);
        let (inl, inr) = if pingpong { (0.5 * (x.l[i] + x.r[i]), 0.0) } else { (x.l[i], x.r[i]) };
        lpl += (1.0 - damp) * (dl - lpl);
        lpr += (1.0 - damp) * (dr - lpr);
        hpl = hp * hpl + (1.0 - hp) * lpl;
        hpr = hp * hpr + (1.0 - hp) * lpr;
        let (fl, fr) = (lpl - hpl, lpr - hpr);
        if pingpong {
            bl[w] = inl + fb * fr;
            br[w] = inr + fb * fl;
        } else {
            bl[w] = inl + fb * fl;
            br[w] = inr + fb * fr;
        }
        x.l[i] += dl * mix;
        x.r[i] += dr * mix;
        w += 1;
        if w >= d {
            w = 0;
        }
    }
}
