//! ITU-R BS.1770 loudness: K-weighting, momentary / short-term block loudness, gated integrated LUFS, LRA, and the
//! "active level" of a stem (how loud it is while it plays) the auto-mixer balances tracks with.

use crate::audio::Stereo;
use crate::config::SRF;

/// K-weighting (48 kHz coefficients) of one channel, squared and accumulated: prefix sums of the weighted power
fn k_power_prefix(x: &[f32]) -> Vec<f64> {
    let (b1, a1) = (
        [1.535_124_859_586_97, -2.691_696_189_406_38, 1.198_392_810_852_85],
        [1.0, -1.690_659_293_182_41, 0.732_480_774_215_85],
    );
    let (b2, a2) = ([1.0, -2.0, 1.0], [1.0, -1.990_047_454_833_98, 0.990_072_250_366_21]);
    let mut out = Vec::with_capacity(x.len() + 1);
    out.push(0.0);
    let (mut z1, mut z2, mut w1, mut w2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut acc = 0.0f64;
    for &v in x {
        let v = v as f64;
        let y = b1[0] * v + z1;
        z1 = b1[1] * v - a1[1] * y + z2;
        z2 = b1[2] * v - a1[2] * y;
        let u = b2[0] * y + w1;
        w1 = b2[1] * y - a2[1] * u + w2;
        w2 = b2[2] * y - a2[2] * u;
        acc += u * u;
        out.push(acc);
    }
    out
}

/// (block centre seconds, LUFS) of sliding blocks: win 0.4 s = momentary, 3 s = short-term
pub fn blocks(x: &Stereo, win: f64, hop: f64) -> Vec<(f64, f64)> {
    let (pl, pr) = (k_power_prefix(&x.l), k_power_prefix(&x.r));
    let n = x.len();
    let w = (win * SRF) as usize;
    let h = ((hop * SRF) as usize).max(1);
    let mut out = Vec::new();
    if n == 0 {
        return out;
    }
    if n < w {
        let ms = (pl[n] + pr[n]) / n as f64;
        return vec![(n as f64 / SRF / 2.0, -0.691 + 10.0 * ms.max(1e-20).log10())];
    }
    let mut s = 0;
    while s + w <= n {
        let ms = (pl[s + w] - pl[s] + pr[s + w] - pr[s]) / w as f64;
        out.push(((s + w / 2) as f64 / SRF, -0.691 + 10.0 * ms.max(1e-20).log10()));
        s += h;
    }
    out
}

fn energy_mean(ls: &[f64]) -> f64 {
    if ls.is_empty() {
        return -70.0;
    }
    10.0 * (ls.iter().map(|l| 10f64.powf(l / 10.0)).sum::<f64>() / ls.len() as f64).log10()
}

/// gated integrated loudness (LUFS)
pub fn integrated(x: &Stereo) -> f64 {
    let b: Vec<f64> = blocks(x, 0.4, 0.1).into_iter().map(|(_, l)| l).filter(|l| *l > -70.0).collect();
    if b.is_empty() {
        return -70.0;
    }
    let rel = energy_mean(&b) - 10.0;
    let g: Vec<f64> = b.into_iter().filter(|l| *l > rel).collect();
    energy_mean(&g)
}

pub fn short_term_max(x: &Stereo) -> f64 {
    blocks(x, 3.0, 0.5).into_iter().map(|(_, l)| l).fold(-70.0, f64::max)
}

/// loudness range (LU): spread of short-term loudness (10th to 95th percentile, gated)
pub fn lra(x: &Stereo) -> Option<f64> {
    let mut s: Vec<f64> = blocks(x, 3.0, 0.1).into_iter().map(|(_, l)| l).filter(|l| *l > -70.0).collect();
    if s.len() < 5 {
        return None;
    }
    let rel = energy_mean(&s) - 20.0;
    s.retain(|l| *l > rel);
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| s[((s.len() - 1) as f64 * q).round() as usize];
    Some(p(0.95) - p(0.10))
}

/// how loud a stem is while it plays: the 80th percentile of its short-term loudness over the blocks where it is
/// audible (one-shot tracks with little activity fall back to momentary blocks)
pub fn active_level(x: &Stereo) -> Option<f64> {
    let mut st: Vec<f64> = blocks(x, 3.0, 0.5).into_iter().map(|(_, l)| l).filter(|l| *l > -50.0).collect();
    if st.len() < 3 {
        st = blocks(x, 0.4, 0.1).into_iter().map(|(_, l)| l).filter(|l| *l > -50.0).collect();
    }
    if st.is_empty() {
        return None;
    }
    st.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(st[((st.len() - 1) as f64 * 0.8).round() as usize])
}
