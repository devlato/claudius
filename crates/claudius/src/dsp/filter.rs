//! Filters: RBJ-cookbook biquads (static EQ, Butterworth cascades), a zero-delay-feedback state variable filter and
//! a saturating 4-pole ladder whose cutoffs may move every sample.

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiquadKind {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peak,
    Lowshelf,
    Highshelf,
    Allpass,
}

impl Biquad {
    pub fn new(kind: BiquadKind, f0: f64, sr: f64, q: f64, gain_db: f64) -> Self {
        let f0 = f0.clamp(5.0, 0.49 * sr);
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * PI * f0 / sr;
        let (cw, sw) = (w.cos(), w.sin());
        let al = sw / (2.0 * q.max(1e-3));
        let (b, aa) = match kind {
            BiquadKind::Lowpass => ([(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0], [1.0 + al, -2.0 * cw, 1.0 - al]),
            BiquadKind::Highpass => {
                ([(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0], [1.0 + al, -2.0 * cw, 1.0 - al])
            }
            BiquadKind::Bandpass => ([al, 0.0, -al], [1.0 + al, -2.0 * cw, 1.0 - al]),
            BiquadKind::Notch => ([1.0, -2.0 * cw, 1.0], [1.0 + al, -2.0 * cw, 1.0 - al]),
            BiquadKind::Allpass => ([1.0 - al, -2.0 * cw, 1.0 + al], [1.0 + al, -2.0 * cw, 1.0 - al]),
            BiquadKind::Peak => ([1.0 + al * a, -2.0 * cw, 1.0 - al * a], [1.0 + al / a, -2.0 * cw, 1.0 - al / a]),
            BiquadKind::Lowshelf => {
                let sq = 2.0 * a.sqrt() * al;
                (
                    [
                        a * ((a + 1.0) - (a - 1.0) * cw + sq),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                        a * ((a + 1.0) - (a - 1.0) * cw - sq),
                    ],
                    [
                        (a + 1.0) + (a - 1.0) * cw + sq,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                        (a + 1.0) + (a - 1.0) * cw - sq,
                    ],
                )
            }
            BiquadKind::Highshelf => {
                let sq = 2.0 * a.sqrt() * al;
                (
                    [
                        a * ((a + 1.0) + (a - 1.0) * cw + sq),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                        a * ((a + 1.0) + (a - 1.0) * cw - sq),
                    ],
                    [
                        (a + 1.0) - (a - 1.0) * cw + sq,
                        2.0 * ((a - 1.0) - (a + 1.0) * cw),
                        (a + 1.0) - (a - 1.0) * cw - sq,
                    ],
                )
            }
        };
        Biquad {
            b0: b[0] / aa[0],
            b1: b[1] / aa[0],
            b2: b[2] / aa[0],
            a1: aa[1] / aa[0],
            a2: aa[2] / aa[0],
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y as f32
    }

    pub fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            *v = self.tick(*v);
        }
    }
}

/// Butterworth Q values for cascaded biquads of even order
fn butter_qs(order: usize) -> Vec<f64> {
    let n = order.max(2) / 2 * 2;
    (0..n / 2).map(|k| 1.0 / (2.0 * ((2 * k + 1) as f64 * PI / (2 * n) as f64).sin())).collect()
}

pub fn butter(x: &mut [f32], highpass: bool, f: f64, sr: f64, order: usize) {
    let kind = if highpass { BiquadKind::Highpass } else { BiquadKind::Lowpass };
    for q in butter_qs(order) {
        Biquad::new(kind, f, sr, q, 0.0).process(x);
    }
}

pub fn biquad(x: &mut [f32], kind: BiquadKind, f: f64, sr: f64, q: f64, gain_db: f64) {
    Biquad::new(kind, f, sr, q, gain_db).process(x);
}

#[inline]
pub fn cutoff_g(fc: f64, sr: f64) -> f64 {
    (PI * fc.clamp(10.0, 0.45 * sr) / sr).tan()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    Lp,
    Hp,
    Bp,
    Notch,
    Ladder,
}

/// Simper/Cytomic zero-delay-feedback state variable filter
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f64,
    ic2: f64,
}

impl Svf {
    /// res 0..1 maps to k = 1/Q from 2 (flat) towards 0 (singing)
    #[inline]
    pub fn k(res: f64) -> f64 {
        2.0 * (1.0 - 0.985 * res.clamp(0.0, 1.0))
    }

    #[inline]
    pub fn tick(&mut self, x: f32, g: f64, k: f64, mode: FilterMode) -> f32 {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v0 = x as f64;
        let v3 = v0 - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (match mode {
            FilterMode::Lp | FilterMode::Ladder => v2,
            FilterMode::Hp => v0 - k * v1 - v2,
            FilterMode::Bp => k * v1,
            FilterMode::Notch => v0 - k * v1,
        }) as f32
    }
}

/// 4-pole transistor ladder: TPT one-pole stages, tanh on the input, one-sample feedback; res ~1 self-oscillates
#[derive(Clone, Copy, Debug, Default)]
pub struct Ladder {
    s: [f64; 4],
    out: f64,
}

impl Ladder {
    #[inline]
    pub fn tick(&mut self, x: f32, g: f64, res: f64, drive: f64) -> f32 {
        let k = 4.0 * res.clamp(0.0, 1.05);
        let gg = g / (1.0 + g);
        let d = drive.max(0.1);
        let mut u = ((x as f64) * (1.0 + 0.5 * k) - k * self.out) * d;
        u = u.tanh() / d;
        for s in self.s.iter_mut() {
            let v = (u - *s) * gg;
            let y = v + *s;
            *s = y + v;
            u = y;
        }
        self.out = u;
        u as f32
    }
}

/// filter a buffer with a constant or per-sample cutoff
pub fn run_filter(x: &mut [f32], cutoff: &dyn Fn(usize) -> f64, sr: f64, res: f64, mode: FilterMode, drive: f64) {
    match mode {
        FilterMode::Ladder => {
            let mut f = Ladder::default();
            for (i, v) in x.iter_mut().enumerate() {
                *v = f.tick(*v, cutoff_g(cutoff(i), sr), res, drive);
            }
        }
        _ => {
            let mut f = Svf::default();
            let k = Svf::k(res);
            for (i, v) in x.iter_mut().enumerate() {
                *v = f.tick(*v, cutoff_g(cutoff(i), sr), k, mode);
            }
        }
    }
}

/// one-pole smoother coefficient for a time constant in seconds
#[inline]
pub fn coef(t: f64, sr: f64) -> f64 {
    if t <= 0.0 { 0.0 } else { (-1.0 / (t * sr)).exp() }
}

pub fn dc_block(x: &mut [f32], sr: f64) {
    butter(x, true, 12.0, sr, 2);
}
