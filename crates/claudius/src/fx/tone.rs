//! Tone shaping: gain, EQ, resonant filters with automation, saturation (2x oversampled), bitcrushing, tape, vinyl.

use super::{Ctx, EqBand, SatKind, blend};
use crate::audio::{Stereo, db_to_gain};
use crate::dsp::env::Param;
use crate::dsp::filter::{self, Biquad, BiquadKind, FilterMode};
use crate::dsp::osc;
use crate::rng::Rng;

pub fn gain(x: &mut Stereo, db: &Param, ctx: &Ctx) {
    match db {
        Param::Const(d) => x.scale(db_to_gain(*d)),
        Param::Curve(_) => {
            let spb = ctx.spb();
            for i in 0..x.len() {
                let g = db_to_gain(db.at(i as f64 / spb, false));
                x.l[i] *= g;
                x.r[i] *= g;
            }
        }
    }
}

pub fn eq(x: &mut Stereo, bands: &[EqBand], ctx: &Ctx) {
    for b in bands {
        for ch in x.channels_mut() {
            Biquad::new(b.kind, b.freq as f64, ctx.sr, b.q as f64, b.gain_db as f64).process(ch);
        }
    }
}

pub fn filter(x: &mut Stereo, mode: FilterMode, cutoff: &Param, res: f32, drive: f32, mix: f32, ctx: &Ctx) {
    let dry = if mix < 0.999 { Some(x.clone()) } else { None };
    let spb = ctx.spb();
    let cut: Vec<f32> = match cutoff {
        Param::Const(c) => vec![*c; 1],
        Param::Curve(_) => cutoff.render(x.len(), spb, true),
    };
    let f = |i: usize| if cut.len() == 1 { cut[0] as f64 } else { cut[i] as f64 };
    for ch in x.channels_mut() {
        filter::run_filter(ch, &f, ctx.sr, res as f64, mode, drive as f64);
    }
    if let Some(d) = dry {
        blend(&d, x, mix);
    }
}

#[inline]
fn shape(kind: SatKind, v: f64) -> f64 {
    match kind {
        SatKind::Tanh => v.tanh(),
        SatKind::Tube => (v + 0.25).tanh() - 0.25f64.tanh(),
        SatKind::Tape => v / (1.0 + v.abs()),
        SatKind::Hard => v.clamp(-1.0, 1.0),
        SatKind::Fold => (v * std::f64::consts::FRAC_PI_2).sin(),
    }
}

/// waveshaper at 2x oversampling; drive maps full scale to full scale, so quieter material gets denser
pub fn saturate(x: &mut Stereo, kind: SatKind, drive_db: f32, mix: f32, sr: f64) {
    let g = db_to_gain(drive_db) as f64;
    let norm = shape(kind, g).abs().max(1e-6);
    let dry = if mix < 0.999 { Some(x.clone()) } else { None };
    let wave = |v: f32| (shape(kind, v as f64 * g) / norm) as f32;
    for ch in x.channels_mut() {
        // a zero after each sample, lowpassed, shaped, lowpassed and every other sample kept, a sample at a time
        let mut up = filter::Butter::<4>::new(false, 0.45 * sr, 2.0 * sr);
        let mut down = up;
        for v in ch.iter_mut() {
            let kept = down.tick(wave(up.tick(*v * 2.0)));
            down.tick(wave(up.tick(0.0)));
            *v = kept;
        }
        if kind == SatKind::Tube {
            filter::dc_block(ch, sr);
        }
    }
    if let Some(d) = dry {
        blend(&d, x, mix);
    }
}

pub fn bitcrush(x: &mut Stereo, bits: f32, downsample: f32, mix: f32) {
    let dry = if mix < 0.999 { Some(x.clone()) } else { None };
    let levels = 2f32.powf(bits.clamp(1.0, 24.0) - 1.0);
    let step = downsample.max(1.0) as f64;
    for ch in x.channels_mut() {
        let mut acc = 0.0f64;
        let mut held = 0.0f32;
        for v in ch.iter_mut() {
            acc += 1.0;
            if acc >= step {
                acc -= step;
                held = (*v * levels).round() / levels;
            }
            *v = held;
        }
    }
    if let Some(d) = dry {
        blend(&d, x, mix);
    }
}

/// tape: wow (slow pitch wander) + flutter (fast), soft saturation, gentle lowpass
pub fn tape(x: &mut Stereo, wow: f32, flutter: f32, drive_db: f32, lowpass_hz: f32, ctx: &Ctx) {
    let sr = ctx.sr;
    let base = 0.004 * sr;
    let n = x.len();
    let mut rng = Rng::new(ctx.seed ^ 0x7a9e);
    let mut drift = osc::Drift::new(rng.fork("wow"), 1.0, 0.5, sr);
    let ph = rng.f64();
    let d: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            let w = (drift.next().log2() * 1200.0) * wow as f64 * 0.0015 * sr;
            let f = (2.0 * std::f64::consts::PI * (6.3 * t + ph)).sin() * flutter as f64 * 0.00012 * sr;
            (base + w + f).max(1.0)
        })
        .collect();
    for ch in x.channels_mut() {
        let src = ch.clone();
        for i in 0..n {
            let p = i as f64 - d[i];
            if p < 0.0 {
                ch[i] = 0.0;
                continue;
            }
            let i0 = p.floor() as usize;
            let fr = (p - i0 as f64) as f32;
            let a = src[i0];
            let b = if i0 + 1 < n { src[i0 + 1] } else { a };
            ch[i] = a + (b - a) * fr;
        }
    }
    // the modulated delay line above starts `base` samples late: realign
    let shift = base.round() as usize;
    for ch in x.channels_mut() {
        let len = ch.len();
        ch.rotate_left(shift.min(len));
        for v in ch[len.saturating_sub(shift)..].iter_mut() {
            *v = 0.0;
        }
    }
    if drive_db > 0.0 {
        saturate(x, SatKind::Tape, drive_db, 1.0, sr);
    }
    if lowpass_hz > 0.0 {
        for ch in x.channels_mut() {
            filter::butter(ch, false, lowpass_hz as f64, sr, 2);
        }
    }
}

/// vinyl surface: sparse crackles (short filtered impulses) and hiss
pub fn vinyl(x: &mut Stereo, crackle: f32, hiss_db: f32, ctx: &Ctx) {
    let sr = ctx.sr;
    let n = x.len();
    let mut rng = Rng::new(ctx.seed ^ 0x51ab);
    let mut hiss = osc::noise(n, &mut rng, "pink");
    filter::butter(&mut hiss, true, 1500.0, sr, 2);
    filter::butter(&mut hiss, false, 9000.0, sr, 2);
    let hg = db_to_gain(hiss_db);
    let rate = (crackle as f64 * 18.0).max(0.0);
    let mut clicks = vec![0.0f32; n];
    let mut t = 0usize;
    while rate > 0.0 && t < n {
        t += (-(rng.f64().max(1e-9)).ln() / rate * sr) as usize + 1;
        if t >= n {
            break;
        }
        let amp = (rng.f64().powi(3) * 0.35) as f32;
        let sign = if rng.chance(0.5) { 1.0 } else { -1.0 };
        for k in 0..((0.0006 * sr) as usize) {
            if t + k < n {
                clicks[t + k] += sign * amp * (-(k as f32) / (0.00012 * sr as f32)).exp();
            }
        }
    }
    let mut bq = Biquad::new(BiquadKind::Bandpass, 2500.0, sr, 0.7, 0.0);
    bq.process(&mut clicks);
    for i in 0..n {
        let v = hiss[i] * hg + clicks[i];
        x.l[i] += v;
        x.r[i] += v * 0.92;
    }
}
