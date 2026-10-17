//! Dynamics: a feed-forward compressor (optionally keyed by another signal), a lookahead brickwall limiter,
//! a transient shaper and a true-peak meter.

use std::collections::VecDeque;

use crate::audio::{Stereo, db_to_gain};
use crate::dsp::filter::{self, coef};

/// stereo-linked feed-forward compressor with a soft knee; gain reduction is smoothed in dB.
/// `key` (sidechain) replaces the detector input when given.
#[allow(clippy::too_many_arguments)]
pub fn compress(
    x: &mut Stereo,
    thr_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    makeup_db: f32,
    mix: f32,
    key: Option<&Stereo>,
    sr: f64,
) {
    let att = coef(attack_ms as f64 / 1000.0, sr);
    let rel = coef(release_ms as f64 / 1000.0, sr);
    let slope = 1.0 / ratio.max(1.0) as f64 - 1.0;
    let knee = knee_db.max(0.0) as f64;
    let thr = thr_db as f64;
    let mut env = 0.0f64;
    let makeup = makeup_db as f64;
    for i in 0..x.len() {
        let (dl, dr) = match key {
            Some(k) if i < k.len() => (k.l[i], k.r[i]),
            Some(_) => (0.0, 0.0),
            None => (x.l[i], x.r[i]),
        };
        let lvl = dl.abs().max(dr.abs()) as f64;
        let d = 20.0 * lvl.max(1e-9).log10();
        let over = d - thr;
        let gr = if knee > 0.0 && 2.0 * over.abs() <= knee {
            slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
        } else if over > 0.0 {
            slope * over
        } else {
            0.0
        };
        env = if gr < env { att * env + (1.0 - att) * gr } else { rel * env + (1.0 - rel) * gr };
        let g = 10f64.powf((env + makeup) / 20.0) as f32;
        let m = mix.clamp(0.0, 1.0);
        let gg = 1.0 - m + m * g;
        x.l[i] *= gg;
        x.r[i] *= gg;
    }
}

/// lookahead brickwall limiter (sample peaks): the gain needed at every sample is held over the lookahead window,
/// smoothed by a moving average of the same length (so it is fully down when the peak plays) and released with a
/// one-pole. Offline, so the lookahead delay is removed again.
pub fn limit(x: &mut Stereo, ceiling_db: f32, release_ms: f32, sr: f64) {
    let n = x.len();
    if n == 0 {
        return;
    }
    let ceil = db_to_gain(ceiling_db) as f64;
    let la = ((0.0015 * sr) as usize).max(8);
    let req: Vec<f64> = (0..n + la)
        .map(|i| {
            if i >= n {
                return 1.0;
            }
            let p = x.l[i].abs().max(x.r[i].abs()) as f64;
            if p > ceil { ceil / p } else { 1.0 }
        })
        .collect();
    // causal sliding minimum over [i - la, i]
    let mut mins = vec![1.0f64; n + la];
    let mut dq: VecDeque<usize> = VecDeque::new();
    for i in 0..n + la {
        while let Some(&b) = dq.back() {
            if req[b] >= req[i] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
        while let Some(&f) = dq.front() {
            if f + la < i {
                dq.pop_front();
            } else {
                break;
            }
        }
        mins[i] = req[*dq.front().unwrap()];
    }
    // moving average over the last la + 1 values
    let mut avg = vec![1.0f64; n + la];
    let mut acc = 0.0;
    for i in 0..n + la {
        acc += mins[i];
        if i > la {
            acc -= mins[i - la - 1];
        }
        let cnt = (i.min(la) + 1) as f64;
        avg[i] = (acc + (la + 1) as f64 - cnt) / (la + 1) as f64;
    }
    let rel = coef(release_ms.max(1.0) as f64 / 1000.0, sr);
    let mut cur = 1.0f64;
    let mut out = Stereo::zeros(n);
    for i in 0..n + la {
        let t = avg[i];
        cur = if t < cur { t } else { rel * cur + (1.0 - rel) * t };
        if i >= la {
            let j = i - la;
            out.l[j] = (x.l[j] as f64 * cur).clamp(-ceil, ceil) as f32;
            out.r[j] = (x.r[j] as f64 * cur).clamp(-ceil, ceil) as f32;
        }
    }
    *x = out;
}

/// soft clipper at 2x oversampling: linear below `knee` of the ceiling, then a tanh curve that never passes the
/// ceiling. Shaves drum transients the way mastering engineers clip before the limiter.
pub fn soft_clip(x: &mut Stereo, ceiling_db: f32, knee: f32, sr: f64) {
    let c = db_to_gain(ceiling_db) as f64;
    let t = c * knee.clamp(0.1, 0.99) as f64;
    let shape = |v: f64| {
        let a = v.abs();
        if a <= t { v } else { v.signum() * (t + (c - t) * ((a - t) / (c - t)).tanh()) }
    };
    let run = |ch: &mut Vec<f32>| {
        let n = ch.len();
        let mut up = vec![0.0f32; 2 * n];
        for i in 0..n {
            up[2 * i] = ch[i] * 2.0;
        }
        filter::butter(&mut up, false, 0.45 * sr, 2.0 * sr, 8);
        for v in up.iter_mut() {
            *v = shape(*v as f64) as f32;
        }
        filter::butter(&mut up, false, 0.45 * sr, 2.0 * sr, 8);
        for i in 0..n {
            ch[i] = up[2 * i];
        }
    };
    let [l, r] = x.channels_mut();
    rayon::join(|| run(l), || run(r));
}

/// true peak (dBTP) via 4x oversampling
pub fn true_peak_db(x: &Stereo) -> f32 {
    // 4x oversampled peak by windowed-sinc interpolation at the three in-between positions, evaluated only between
    // samples within ~4.4 dB of the sample peak (inter-sample overs ride on loud neighbours)
    const H: usize = 12;
    let kernels: Vec<[f32; 2 * H]> = (1..4)
        .map(|q| {
            let frac = q as f64 * 0.25;
            let mut k = [0.0f32; 2 * H];
            for (j, c) in k.iter_mut().enumerate() {
                let d = frac + (H as f64 - 1.0) - j as f64;
                let s =
                    if d.abs() < 1e-9 { 1.0 } else { (std::f64::consts::PI * d).sin() / (std::f64::consts::PI * d) };
                *c = (s * (0.5 + 0.5 * (std::f64::consts::PI * d / H as f64).cos())) as f32;
            }
            k
        })
        .collect();
    let peak = |ch: &Vec<f32>| {
        let sp = ch.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let thr = sp * 0.6;
        let n = ch.len();
        let mut best = sp;
        for i in H..n.saturating_sub(H) {
            if ch[i].abs() < thr && ch[i + 1].abs() < thr {
                continue;
            }
            let win = &ch[i + 1 - H..i + 1 + H];
            for k in &kernels {
                let acc: f32 = win.iter().zip(k.iter()).map(|(a, b)| a * b).sum();
                best = best.max(acc.abs());
            }
        }
        best
    };
    let (a, b) = rayon::join(|| peak(&x.l), || peak(&x.r));
    20.0 * a.max(b).max(1e-9).log10()
}

/// transient shaper: the difference of a fast and a slow envelope separates attacks from sustain
pub fn transient(x: &mut Stereo, attack_db: f32, sustain_db: f32, sr: f64) {
    let (fa, fr) = (coef(0.0008, sr), coef(0.03, sr));
    let (sa, sr2) = (coef(0.02, sr), coef(0.25, sr));
    let mut ef = 0.0f64;
    let mut es = 0.0f64;
    for i in 0..x.len() {
        let v = x.l[i].abs().max(x.r[i].abs()) as f64;
        ef = if v > ef { fa * ef + (1.0 - fa) * v } else { fr * ef + (1.0 - fr) * v };
        es = if v > es { sa * es + (1.0 - sa) * v } else { sr2 * es + (1.0 - sr2) * v };
        let d = ((ef + 1e-6) / (es + 1e-6)).ln();
        let g_db =
            if d > 0.0 { attack_db as f64 * (d * 1.5).min(1.0) } else { sustain_db as f64 * (-d * 1.5).min(1.0) };
        let g = 10f64.powf(g_db / 20.0) as f32;
        x.l[i] *= g;
        x.r[i] *= g;
    }
}
