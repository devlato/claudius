//! Band-limited oscillators (polyBLEP), unison stacks with analog drift, and noise colours.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wave {
    Sine,
    Saw,
    Square,
    Pulse,
    Tri,
    Noise,
}

#[inline]
fn blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let u = t / dt;
        u + u - u * u - 1.0
    } else if t > 1.0 - dt {
        let u = (t - 1.0) / dt;
        u * u + u + u + 1.0
    } else {
        0.0
    }
}

#[derive(Clone, Debug)]
pub struct Osc {
    pub wave: Wave,
    pub phase: f64,
    pub pw: f64,
    noise: u64,
}

impl Osc {
    pub fn new(wave: Wave, phase: f64) -> Self {
        Osc { wave, phase: phase.rem_euclid(1.0), pw: 0.5, noise: 0x2545_F491_4F6C_DD1D ^ (phase.to_bits()) }
    }

    /// one sample at `hz`; `pm` adds phase modulation in cycles (FM)
    #[inline]
    pub fn tick_pm(&mut self, hz: f64, sr: f64, pm: f64) -> f32 {
        let dt = (hz / sr).clamp(1e-9, 0.49);
        let t = (self.phase + pm).rem_euclid(1.0);
        let y = match self.wave {
            Wave::Sine => (2.0 * std::f64::consts::PI * t).sin(),
            Wave::Saw => 2.0 * t - 1.0 - blep(t, dt),
            Wave::Square => {
                let v = if t < 0.5 { 1.0 } else { -1.0 };
                v + blep(t, dt) - blep((t + 0.5).rem_euclid(1.0), dt)
            }
            Wave::Pulse => {
                let pw = self.pw.clamp(0.02, 0.98);
                let v = if t < pw { 1.0 } else { -1.0 };
                v + blep(t, dt) - blep((t + 1.0 - pw).rem_euclid(1.0), dt) - (2.0 * pw - 1.0)
            }
            Wave::Tri => 1.0 - 4.0 * (t - 0.5).abs(),
            Wave::Noise => {
                self.noise ^= self.noise << 13;
                self.noise ^= self.noise >> 7;
                self.noise ^= self.noise << 17;
                (self.noise >> 11) as f64 / (1u64 << 52) as f64 - 1.0
            }
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        y as f32
    }

    #[inline]
    pub fn tick(&mut self, hz: f64, sr: f64) -> f32 {
        self.tick_pm(hz, sr, 0.0)
    }
}

/// slow random pitch wander (cents), smooth: a new target every ~1/rate seconds, cosine-interpolated
#[derive(Clone, Debug)]
pub struct Drift {
    cents: f64,
    len: usize,
    i: usize,
    a: f64,
    b: f64,
    rng: Rng,
}

impl Drift {
    pub fn new(rng: Rng, cents: f64, rate_hz: f64, sr: f64) -> Self {
        let mut rng = rng;
        let a = rng.normal() * cents;
        let b = rng.normal() * cents;
        Drift { cents, len: ((sr / rate_hz.max(0.05)) as usize).max(16), i: 0, a, b, rng }
    }

    /// frequency multiplier for the next sample
    #[inline]
    pub fn next(&mut self) -> f64 {
        if self.cents <= 0.0 {
            return 1.0;
        }
        let f = self.i as f64 / self.len as f64;
        let w = f * f * (3.0 - 2.0 * f);
        let c = self.a + (self.b - self.a) * w;
        self.i += 1;
        if self.i >= self.len {
            self.i = 0;
            self.a = self.b;
            self.b = self.rng.normal() * self.cents;
        }
        // 2^(c/1200) to first order: exact to 1e-6 for the few cents of drift used
        1.0 + c * (std::f64::consts::LN_2 / 1200.0)
    }
}

/// a detuned unison stack (a supersaw for Wave::Saw) spread across the stereo field
#[derive(Clone, Debug)]
pub struct Unison {
    oscs: Vec<Osc>,
    ratios: Vec<f64>,
    gl: Vec<f32>,
    gr: Vec<f32>,
    drift: Vec<Drift>,
}

impl Unison {
    pub fn new(
        wave: Wave,
        voices: usize,
        detune_cents: f64,
        width: f64,
        pw: f64,
        drift_cents: f64,
        sr: f64,
        rng: &mut Rng,
    ) -> Self {
        let voices = voices.max(1);
        let mut oscs = Vec::with_capacity(voices);
        let mut ratios = Vec::with_capacity(voices);
        let mut gl = Vec::with_capacity(voices);
        let mut gr = Vec::with_capacity(voices);
        let mut drift = Vec::with_capacity(voices);
        let side = if voices > 1 { 0.78 } else { 1.0 };
        let mut norm = 0.0f64;
        for j in 0..voices {
            let o = if voices == 1 { 0.0 } else { -1.0 + 2.0 * j as f64 / (voices - 1) as f64 };
            let centre = voices % 2 == 1 && j == voices / 2;
            let lvl = if voices == 1 || centre { 1.0 } else { side };
            let mut osc = Osc::new(wave, rng.f64());
            osc.pw = pw;
            oscs.push(osc);
            ratios.push(2f64.powf(o * detune_cents / 1200.0));
            let th = (o * width + 1.0) * std::f64::consts::FRAC_PI_4;
            gl.push((th.cos() * lvl) as f32);
            gr.push((th.sin() * lvl) as f32);
            norm += lvl * lvl;
            drift.push(Drift::new(rng.fork(&format!("drift{j}")), drift_cents, 0.6 + 0.4 * rng.f64(), sr));
        }
        let k = (1.0 / (norm.sqrt() * std::f64::consts::FRAC_1_SQRT_2)) as f32;
        gl.iter_mut().for_each(|g| *g *= k);
        gr.iter_mut().for_each(|g| *g *= k);
        Unison { oscs, ratios, gl, gr, drift }
    }

    #[inline]
    pub fn tick(&mut self, hz: f64, sr: f64) -> (f32, f32) {
        self.tick_pm(hz, sr, 0.0)
    }

    /// `pm` = phase modulation in cycles applied to every voice (FM)
    #[inline]
    pub fn tick_pm(&mut self, hz: f64, sr: f64, pm: f64) -> (f32, f32) {
        let mut l = 0.0;
        let mut r = 0.0;
        for j in 0..self.oscs.len() {
            let f = hz * self.ratios[j] * self.drift[j].next();
            let y = self.oscs[j].tick_pm(f, sr, pm);
            l += y * self.gl[j];
            r += y * self.gr[j];
        }
        (l, r)
    }
}

/// unit-RMS noise; pink / brown shaped in the frequency domain
pub fn noise(n: usize, rng: &mut Rng, color: &str) -> Vec<f32> {
    let mut w: Vec<f32> = (0..n).map(|_| rng.normal() as f32).collect();
    if color != "white" && n > 64 {
        use realfft::RealFftPlanner;
        let mut planner = RealFftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(n);
        let inv = planner.plan_fft_inverse(n);
        let mut spec = fwd.make_output_vec();
        let mut buf = w.clone();
        fwd.process(&mut buf, &mut spec).ok();
        for (k, c) in spec.iter_mut().enumerate() {
            let f = (k.max(1)) as f32;
            let g = if color == "pink" { 1.0 / f.sqrt() } else { 1.0 / f };
            *c *= if k == 0 { 0.0 } else { g };
        }
        inv.process(&mut spec, &mut w).ok();
    }
    let rms = (w.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / n.max(1) as f64).sqrt() as f32;
    if rms > 1e-12 {
        w.iter_mut().for_each(|v| *v /= rms);
    }
    w
}
