//! Arrangement-time edits on a region (beats): tape stop, stutter / beat repeat, reverse.

use super::Ctx;
use crate::audio::Stereo;

fn region(x: &Stereo, start: f64, len: f64, ctx: &Ctx) -> Option<(usize, usize)> {
    let spb = ctx.spb();
    let a = (start * spb).round().max(0.0) as usize;
    let b = (((start + len) * spb).round() as usize).min(x.len());
    if b > a + 16 { Some((a, b)) } else { None }
}

/// the region slows to a halt (speed falls 1 -> 0 along a quadratic), silence after
pub fn tape_stop(x: &mut Stereo, start: f64, len: f64, ctx: &Ctx) {
    let Some((a, b)) = region(x, start, len, ctx) else { return };
    let n = b - a;
    let src = x.slice(a, x.len());
    let mut pos = 0.0f64;
    for i in 0..n {
        let f = i as f64 / n as f64;
        let speed = (1.0 - f).powi(2);
        let i0 = pos as usize;
        let fr = (pos - i0 as f64) as f32;
        let (l, r) = if i0 + 1 < src.len() {
            (src.l[i0] + (src.l[i0 + 1] - src.l[i0]) * fr, src.r[i0] + (src.r[i0 + 1] - src.r[i0]) * fr)
        } else {
            (0.0, 0.0)
        };
        let fade = if f > 0.92 { ((1.0 - f) / 0.08) as f32 } else { 1.0 };
        x.l[a + i] = l * fade;
        x.r[a + i] = r * fade;
        pos += speed;
    }
}

/// beat repeat: the first `slice` beats of the region loop to its end, halving the slice every quarter of the
/// region when slice <= 0.5 (a build-up stutter)
pub fn stutter(x: &mut Stereo, start: f64, len: f64, slice: f64, ctx: &Ctx) {
    let Some((a, b)) = region(x, start, len, ctx) else { return };
    let spb = ctx.spb();
    let src = x.slice(a, b);
    let mut i = a;
    let mut cur = slice.max(1.0 / 32.0);
    let accelerate = slice <= 0.5;
    while i < b {
        let s = ((cur * spb) as usize).max(32);
        for k in 0..s.min(b - i) {
            let fade = if k < 32 {
                k as f32 / 32.0
            } else if s - k < 32 {
                (s - k) as f32 / 32.0
            } else {
                1.0
            };
            x.l[i + k] = src.l[k % src.len()] * fade;
            x.r[i + k] = src.r[k % src.len()] * fade;
        }
        i += s;
        if accelerate && (i - a) as f64 > (b - a) as f64 * 0.5 && cur > 1.0 / 16.0 {
            cur *= 0.5;
        }
    }
}

pub fn reverse(x: &mut Stereo, start: f64, len: f64, ctx: &Ctx) {
    let Some((a, b)) = region(x, start, len, ctx) else { return };
    x.l[a..b].reverse();
    x.r[a..b].reverse();
    for ch in x.channels_mut() {
        crate::audio::fade_slice(&mut ch[a..b], 96, 96);
    }
}
