//! Procedural drum kit: one-shots rendered from code (pitch-enveloped sines, metallic square clusters, shaped noise),
//! in the spirit of the 808 / 909 circuits. All voices share one numeric parameter set so mutation can explore it.

use serde::{Deserialize, Serialize};

use crate::audio::Stereo;
use crate::dsp::env::{perc, pitch_drop};
use crate::dsp::filter::{self, Biquad, BiquadKind};
use crate::dsp::osc::{self, Osc, Wave};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DrumVoice {
    Kick,
    Snare,
    Clap,
    ClosedHat,
    OpenHat,
    Ride,
    Crash,
    Tom,
    Rim,
    Shaker,
    Cowbell,
    Snap,
    Perc,
}

impl DrumVoice {
    pub const ALL: [DrumVoice; 13] = [
        DrumVoice::Kick,
        DrumVoice::Snare,
        DrumVoice::Clap,
        DrumVoice::ClosedHat,
        DrumVoice::OpenHat,
        DrumVoice::Ride,
        DrumVoice::Crash,
        DrumVoice::Tom,
        DrumVoice::Rim,
        DrumVoice::Shaker,
        DrumVoice::Cowbell,
        DrumVoice::Snap,
        DrumVoice::Perc,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DrumVoice::Kick => "kick",
            DrumVoice::Snare => "snare",
            DrumVoice::Clap => "clap",
            DrumVoice::ClosedHat => "closed_hat",
            DrumVoice::OpenHat => "open_hat",
            DrumVoice::Ride => "ride",
            DrumVoice::Crash => "crash",
            DrumVoice::Tom => "tom",
            DrumVoice::Rim => "rim",
            DrumVoice::Shaker => "shaker",
            DrumVoice::Cowbell => "cowbell",
            DrumVoice::Snap => "snap",
            DrumVoice::Perc => "perc",
        }
    }

    /// the mixing role of a lane playing this voice
    pub fn role(self) -> &'static str {
        match self {
            DrumVoice::Kick => "kick",
            DrumVoice::Snare => "snare",
            DrumVoice::Clap => "clap",
            DrumVoice::ClosedHat => "hat",
            DrumVoice::OpenHat => "ohat",
            DrumVoice::Ride => "ride",
            DrumVoice::Crash => "crash",
            DrumVoice::Tom => "tom",
            DrumVoice::Shaker => "shaker",
            DrumVoice::Rim | DrumVoice::Cowbell | DrumVoice::Snap | DrumVoice::Perc => "perc",
        }
    }
}

/// tune: Hz of the body (kick / tom / snare / perc) or a ratio for metallic voices; decay: seconds;
/// tone: 0 dark .. 1 bright; punch: 0..1 hold before the decay; snap: transient / noise share; drive: dB;
/// pitch_env: semitones of the starting pitch sweep; pitch_time: its time constant (s); room: 0..1 short ambience
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumParams {
    pub tune: f32,
    pub decay: f32,
    pub tone: f32,
    pub punch: f32,
    pub snap: f32,
    pub drive: f32,
    pub pitch_env: f32,
    pub pitch_time: f32,
    pub room: f32,
}

/// (low, high, log scale) per parameter, per voice: the mutation space
pub struct Range(pub f32, pub f32, pub bool);

impl DrumParams {
    pub fn default_for(v: DrumVoice) -> Self {
        let p = |tune, decay, tone, punch, snap, drive, pitch_env, pitch_time, room| DrumParams {
            tune,
            decay,
            tone,
            punch,
            snap,
            drive,
            pitch_env,
            pitch_time,
            room,
        };
        match v {
            DrumVoice::Kick => p(50.0, 0.42, 0.45, 0.35, 0.35, 6.0, 36.0, 0.028, 0.0),
            DrumVoice::Snare => p(195.0, 0.2, 0.6, 0.2, 0.6, 3.0, 4.0, 0.02, 0.25),
            DrumVoice::Clap => p(1300.0, 0.22, 0.55, 0.0, 0.5, 2.0, 0.0, 0.01, 0.35),
            DrumVoice::ClosedHat => p(1.0, 0.05, 0.6, 0.0, 0.4, 1.0, 0.0, 0.01, 0.0),
            DrumVoice::OpenHat => p(1.0, 0.38, 0.6, 0.0, 0.4, 1.0, 0.0, 0.01, 0.1),
            DrumVoice::Ride => p(1.0, 2.2, 0.55, 0.0, 0.3, 0.0, 0.0, 0.01, 0.1),
            DrumVoice::Crash => p(1.0, 2.6, 0.6, 0.0, 0.6, 1.0, 0.0, 0.01, 0.2),
            DrumVoice::Tom => p(120.0, 0.32, 0.4, 0.2, 0.2, 3.0, 7.0, 0.04, 0.2),
            DrumVoice::Rim => p(1700.0, 0.045, 0.6, 0.0, 0.5, 2.0, 0.0, 0.01, 0.15),
            DrumVoice::Shaker => p(1.0, 0.07, 0.6, 0.0, 0.5, 0.0, 0.0, 0.01, 0.1),
            DrumVoice::Cowbell => p(1.0, 0.22, 0.5, 0.0, 0.2, 2.0, 0.0, 0.01, 0.1),
            DrumVoice::Snap => p(2200.0, 0.06, 0.6, 0.0, 0.6, 2.0, 0.0, 0.01, 0.3),
            DrumVoice::Perc => p(520.0, 0.14, 0.5, 0.0, 0.2, 2.0, 12.0, 0.012, 0.15),
        }
    }

    pub fn space(v: DrumVoice) -> [Range; 9] {
        let r = Range;
        match v {
            DrumVoice::Kick => [
                r(38.0, 68.0, true),
                r(0.18, 1.6, true),
                r(0.1, 0.9, false),
                r(0.0, 0.8, false),
                r(0.0, 0.8, false),
                r(0.0, 14.0, false),
                r(18.0, 48.0, false),
                r(0.01, 0.07, true),
                r(0.0, 0.2, false),
            ],
            DrumVoice::Snare => [
                r(150.0, 260.0, true),
                r(0.1, 0.4, true),
                r(0.2, 1.0, false),
                r(0.0, 0.6, false),
                r(0.3, 0.95, false),
                r(0.0, 9.0, false),
                r(0.0, 9.0, false),
                r(0.005, 0.05, true),
                r(0.0, 0.6, false),
            ],
            DrumVoice::Clap => [
                r(900.0, 2000.0, true),
                r(0.1, 0.45, true),
                r(0.2, 1.0, false),
                r(0.0, 0.3, false),
                r(0.2, 0.9, false),
                r(0.0, 8.0, false),
                r(0.0, 0.0, false),
                r(0.005, 0.02, true),
                r(0.0, 0.7, false),
            ],
            DrumVoice::Crash | DrumVoice::Ride => [
                r(0.8, 1.3, true),
                r(1.0, 4.0, true),
                r(0.3, 1.0, false),
                r(0.0, 0.1, false),
                r(0.1, 0.8, false),
                r(0.0, 4.0, false),
                r(0.0, 0.0, false),
                r(0.005, 0.02, true),
                r(0.0, 0.4, false),
            ],
            DrumVoice::Tom => [
                r(70.0, 220.0, true),
                r(0.15, 0.6, true),
                r(0.1, 0.8, false),
                r(0.0, 0.6, false),
                r(0.0, 0.5, false),
                r(0.0, 8.0, false),
                r(0.0, 14.0, false),
                r(0.01, 0.08, true),
                r(0.0, 0.5, false),
            ],
            DrumVoice::Perc => [
                r(200.0, 1500.0, true),
                r(0.05, 0.35, true),
                r(0.1, 0.9, false),
                r(0.0, 0.3, false),
                r(0.0, 0.6, false),
                r(0.0, 8.0, false),
                r(0.0, 24.0, false),
                r(0.005, 0.04, true),
                r(0.0, 0.5, false),
            ],
            DrumVoice::ClosedHat | DrumVoice::Shaker | DrumVoice::OpenHat | DrumVoice::Cowbell => {
                let (dlo, dhi) = match v {
                    DrumVoice::OpenHat => (0.18, 0.8),
                    DrumVoice::Cowbell => (0.1, 0.4),
                    _ => (0.025, 0.12),
                };
                [
                    r(0.8, 1.35, true),
                    r(dlo, dhi, true),
                    r(0.2, 1.0, false),
                    r(0.0, 0.2, false),
                    r(0.1, 0.9, false),
                    r(0.0, 6.0, false),
                    r(0.0, 0.0, false),
                    r(0.005, 0.02, true),
                    r(0.0, 0.3, false),
                ]
            }
            DrumVoice::Rim | DrumVoice::Snap => [
                r(1000.0, 3000.0, true),
                r(0.02, 0.12, true),
                r(0.2, 1.0, false),
                r(0.0, 0.2, false),
                r(0.2, 0.9, false),
                r(0.0, 8.0, false),
                r(0.0, 0.0, false),
                r(0.005, 0.02, true),
                r(0.0, 0.6, false),
            ],
        }
    }

    pub fn get(&self, i: usize) -> f32 {
        [
            self.tune,
            self.decay,
            self.tone,
            self.punch,
            self.snap,
            self.drive,
            self.pitch_env,
            self.pitch_time,
            self.room,
        ][i]
    }

    pub fn set(&mut self, i: usize, v: f32) {
        match i {
            0 => self.tune = v,
            1 => self.decay = v,
            2 => self.tone = v,
            3 => self.punch = v,
            4 => self.snap = v,
            5 => self.drive = v,
            6 => self.pitch_env = v,
            7 => self.pitch_time = v,
            _ => self.room = v,
        }
    }
}

const METAL: [f64; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

fn drive(x: &mut [f32], db: f32) {
    if db <= 0.01 {
        return;
    }
    let g = 10f64.powf(db as f64 / 20.0);
    let norm = g.tanh();
    for v in x.iter_mut() {
        *v = ((*v as f64 * g).tanh() / norm) as f32;
    }
}

fn metallic(n: usize, sr: f64, ratio: f64, rng: &mut Rng) -> Vec<f32> {
    let mut oscs: Vec<Osc> = METAL.iter().map(|_| Osc::new(Wave::Square, rng.f64())).collect();
    (0..n).map(|_| oscs.iter_mut().zip(METAL.iter()).map(|(o, f)| o.tick(f * ratio, sr)).sum::<f32>() / 6.0).collect()
}

fn room(x: &mut Stereo, amount: f32, sr: f64, rng: &mut Rng) {
    if amount <= 0.01 {
        return;
    }
    let n = (0.18 * sr) as usize;
    let mut tail = Stereo::zeros(x.len() + n);
    for (c, ch) in tail.channels_mut().into_iter().enumerate() {
        let mut ir = osc::noise(n, &mut rng.fork(&format!("room{c}")), "white");
        let e = perc(n, sr, 0.002, 0.0, 0.16, 1.0);
        ir.iter_mut().zip(&e).for_each(|(v, g)| *v *= g * 0.06);
        filter::butter(&mut ir, false, 6000.0, sr, 2);
        let src = if c == 0 { &x.l } else { &x.r };
        let w = crate::dsp::fft::convolve(src, &ir);
        ch[..w.len().min(x.len() + n)].copy_from_slice(&w[..w.len().min(x.len() + n)]);
    }
    let len = x.len();
    x.ensure_len(len + n);
    x.mix(&tail, amount);
}

/// render one drum hit (stereo, peak -1 dBFS)
pub fn render(v: DrumVoice, p: &DrumParams, sr: f64, seed: u64) -> Stereo {
    let mut rng = Rng::new(seed ^ crate::rng::hash_str(v.name()));
    let tone = p.tone.clamp(0.0, 1.0) as f64;
    let mut out = match v {
        DrumVoice::Kick => {
            let n = ((p.decay as f64 * 1.3 + 0.05) * sr) as usize;
            let env = perc(n, sr, 0.0004, p.punch as f64 * 0.045, p.decay as f64, 1.25);
            let mut body = Osc::new(Wave::Sine, 0.0);
            let mut x: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f64 / sr;
                    body.tick(p.tune as f64 * pitch_drop(t, p.pitch_env as f64, p.pitch_time as f64), sr) * env[i]
                })
                .collect();
            let cn = (0.006 * sr) as usize;
            let mut click = osc::noise(cn, &mut rng, "white");
            filter::butter(&mut click, true, 2500.0 + 3000.0 * tone, sr, 2);
            let ce = perc(cn, sr, 0.0002, 0.0, 0.004, 1.0);
            let mut blip = Osc::new(Wave::Sine, 0.25);
            for i in 0..cn.min(n) {
                x[i] += (click[i] * 0.25 + blip.tick(1800.0 + 1500.0 * tone, sr) * 0.4) * ce[i] * p.snap;
            }
            drive(&mut x, p.drive);
            filter::butter(&mut x, false, 1800.0 + 9000.0 * tone, sr, 2);
            filter::butter(&mut x, true, 28.0, sr, 2);
            Stereo::from_mono(&x)
        }
        DrumVoice::Snare => {
            let n = ((p.decay as f64 * 1.6 + 0.05) * sr) as usize;
            let te = perc(n, sr, 0.0005, p.punch as f64 * 0.01, (p.decay as f64 * 0.55).max(0.04), 1.2);
            let (mut a, mut b) = (Osc::new(Wave::Sine, 0.0), Osc::new(Wave::Sine, 0.1));
            let mut body: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f64 / sr;
                    let m = pitch_drop(t, p.pitch_env as f64, p.pitch_time as f64);
                    (a.tick(p.tune as f64 * m, sr) + 0.6 * b.tick(p.tune as f64 * 1.62 * m, sr)) * te[i]
                })
                .collect();
            let mut nz = osc::noise(n, &mut rng, "white");
            filter::butter(&mut nz, true, 1200.0, sr, 2);
            Biquad::new(BiquadKind::Peak, 5500.0, sr, 0.8, 5.0).process(&mut nz);
            filter::butter(&mut nz, false, 6000.0 + 9000.0 * tone, sr, 2);
            let ne = perc(n, sr, 0.0008, 0.0, p.decay as f64, 1.35);
            let s = p.snap.clamp(0.0, 1.0);
            for i in 0..n {
                body[i] = body[i] * (1.0 - 0.6 * s) + nz[i] * ne[i] * 0.45 * s;
            }
            drive(&mut body, p.drive);
            filter::butter(&mut body, true, 90.0, sr, 2);
            Stereo::from_mono(&body)
        }
        DrumVoice::Clap => {
            let n = ((p.decay as f64 * 1.5 + 0.08) * sr) as usize;
            let mut x = Stereo::zeros(n);
            for (c, ch) in x.channels_mut().into_iter().enumerate() {
                let mut r = rng.fork(&format!("clap{c}"));
                let nz = osc::noise(n, &mut r, "white");
                let offs = [0.0, 0.0085 + 0.002 * r.f64(), 0.018 + 0.003 * r.f64(), 0.028 + 0.003 * r.f64()];
                for (k, o) in offs.iter().enumerate() {
                    let start = (o * sr) as usize;
                    let last = k == offs.len() - 1;
                    let t60 = if last { p.decay as f64 } else { 0.012 };
                    let len = n - start;
                    let e = perc(len, sr, 0.0004, 0.0, t60, if last { 1.4 } else { 1.0 });
                    let g = if last { 1.0 } else { 0.75 };
                    for i in 0..len {
                        ch[start + i] += nz[start + i] * e[i] * g;
                    }
                }
                Biquad::new(BiquadKind::Bandpass, p.tune as f64, sr, 1.1, 0.0).process(ch);
                Biquad::new(BiquadKind::Highshelf, 3500.0, sr, 0.7, tone * 10.0 - 4.0).process(ch);
                filter::butter(ch, true, 500.0, sr, 2);
                drive(ch, p.drive);
            }
            x
        }
        DrumVoice::ClosedHat | DrumVoice::OpenHat | DrumVoice::Shaker => {
            let n = ((p.decay as f64 * 1.4 + 0.02) * sr) as usize;
            let attack = if v == DrumVoice::Shaker { 0.008 } else { 0.0003 };
            let e = perc(n, sr, attack, 0.0, p.decay as f64, if v == DrumVoice::ClosedHat { 1.5 } else { 1.15 });
            let metal = if v == DrumVoice::Shaker { vec![0.0; n] } else { metallic(n, sr, p.tune as f64, &mut rng) };
            let nz = osc::noise(n, &mut rng, "white");
            let s = if v == DrumVoice::Shaker { 1.0 } else { p.snap.clamp(0.0, 1.0) };
            let mut x: Vec<f32> = (0..n).map(|i| (metal[i] * (1.0 - s) * 1.6 + nz[i] * s * 0.35) * e[i]).collect();
            filter::butter(&mut x, true, 2800.0 + 3200.0 * tone, sr, 4);
            Biquad::new(BiquadKind::Peak, 8000.0, sr, 0.7, 2.0 + 3.0 * tone).process(&mut x);
            Biquad::new(BiquadKind::Highshelf, 12_000.0, sr, 0.7, -2.0).process(&mut x);
            drive(&mut x, p.drive);
            Stereo::from_mono(&x)
        }
        DrumVoice::Ride | DrumVoice::Crash => {
            let n = ((p.decay as f64 * 1.2 + 0.05) * sr) as usize;
            let mut x = Stereo::zeros(n);
            for (c, ch) in x.channels_mut().into_iter().enumerate() {
                let mut r = rng.fork(&format!("cym{c}"));
                let metal = metallic(n, sr, p.tune as f64 * if v == DrumVoice::Ride { 1.0 } else { 1.17 }, &mut r);
                let metal2 = metallic(n, sr, p.tune as f64 * 1.49, &mut r);
                let nz = osc::noise(n, &mut r, "white");
                let e = perc(n, sr, if v == DrumVoice::Crash { 0.002 } else { 0.0005 }, 0.0, p.decay as f64, 1.0);
                let es = perc(n, sr, 0.0005, 0.0, (p.decay as f64 * 0.12).max(0.05), 1.0);
                let s = p.snap.clamp(0.0, 1.0);
                for i in 0..n {
                    ch[i] = ((metal[i] + metal2[i]) * (1.0 - s) * 0.8 * e[i]
                        + nz[i] * s * 0.3 * (0.6 * e[i] + 0.4 * es[i]))
                        * 1.0;
                }
                filter::butter(ch, true, if v == DrumVoice::Ride { 3000.0 } else { 2500.0 } + 2000.0 * tone, sr, 2);
                Biquad::new(BiquadKind::Highshelf, 9000.0, sr, 0.7, tone * 6.0 - 2.0).process(ch);
                drive(ch, p.drive);
            }
            x
        }
        DrumVoice::Tom | DrumVoice::Perc => {
            let n = ((p.decay as f64 * 1.4 + 0.03) * sr) as usize;
            let e = perc(n, sr, 0.0005, p.punch as f64 * 0.02, p.decay as f64, 1.2);
            let mut car = Osc::new(Wave::Sine, 0.0);
            let mut modu = Osc::new(Wave::Sine, 0.0);
            let fm = if v == DrumVoice::Perc { 1.0 + 2.5 * tone } else { 0.0 };
            let mut x: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f64 / sr;
                    let f = p.tune as f64 * pitch_drop(t, p.pitch_env as f64, p.pitch_time as f64);
                    let idx = fm * (-t / 0.03).exp();
                    let pm = modu.tick(f * 2.41, sr) as f64 * idx / std::f64::consts::TAU;
                    car.tick_pm(f, sr, pm) * e[i]
                })
                .collect();
            let mut nz = osc::noise(n, &mut rng, "white");
            filter::butter(&mut nz, true, 1500.0, sr, 2);
            let ne = perc(n, sr, 0.0003, 0.0, 0.03, 1.0);
            for i in 0..n {
                x[i] += nz[i] * ne[i] * 0.15 * p.snap;
            }
            drive(&mut x, p.drive);
            Stereo::from_mono(&x)
        }
        DrumVoice::Rim | DrumVoice::Snap => {
            let n = ((p.decay as f64 * 2.0 + 0.02) * sr) as usize;
            let e = perc(n, sr, 0.0002, 0.0, p.decay as f64, 1.3);
            let mut nz = osc::noise(n, &mut rng, "white");
            Biquad::new(BiquadKind::Bandpass, p.tune as f64, sr, 1.6, 0.0).process(&mut nz);
            let (mut a, mut b) = (Osc::new(Wave::Sine, 0.0), Osc::new(Wave::Sine, 0.3));
            let s = p.snap.clamp(0.0, 1.0);
            let tonal = if v == DrumVoice::Rim { 1.0 - s } else { 0.15 };
            let mut x: Vec<f32> = (0..n)
                .map(|i| (nz[i] * s * 1.2 + (a.tick(500.0, sr) + b.tick(1720.0, sr)) * 0.5 * tonal) * e[i])
                .collect();
            drive(&mut x, p.drive);
            filter::butter(&mut x, true, 300.0, sr, 2);
            Stereo::from_mono(&x)
        }
        DrumVoice::Cowbell => {
            let n = ((p.decay as f64 * 1.6 + 0.02) * sr) as usize;
            let e = perc(n, sr, 0.0005, 0.0, p.decay as f64, 1.2);
            let (mut a, mut b) = (Osc::new(Wave::Square, 0.0), Osc::new(Wave::Square, 0.0));
            let r = p.tune as f64;
            let mut x: Vec<f32> =
                (0..n).map(|i| (a.tick(540.0 * r, sr) + b.tick(800.0 * r, sr)) * 0.5 * e[i]).collect();
            Biquad::new(BiquadKind::Bandpass, 1400.0 * r, sr, 1.8, 0.0).process(&mut x);
            drive(&mut x, p.drive);
            Stereo::from_mono(&x)
        }
    };
    room(&mut out, p.room, sr, &mut rng);
    let len = out.len();
    out.fade(0, ((0.004 * sr) as usize).min(len));
    out.normalize(-1.0);
    out
}
