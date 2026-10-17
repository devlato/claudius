//! Transition effects rendered in code: risers, downlifters, impacts, reverse crashes, noise sweeps.

use crate::audio::Stereo;
use crate::dsp::env::perc;
use crate::dsp::filter::{self, FilterMode, Svf, cutoff_g};
use crate::dsp::osc::{self, Osc, Unison, Wave};
use crate::rng::Rng;
use crate::synth::drums::{self, DrumParams, DrumVoice};

/// noise swept up through a resonant bandpass plus a detuned saw climbing two octaves, swelling into the drop
pub fn riser(secs: f64, sr: f64, seed: u64, root_hz: f64) -> Stereo {
    let n = (secs * sr) as usize;
    let mut rng = Rng::new(seed ^ 0x51);
    let mut out = Stereo::zeros(n);
    let k = Svf::k(0.35);
    for (c, ch) in out.channels_mut().into_iter().enumerate() {
        let nz = osc::noise(n, &mut rng.fork(&format!("n{c}")), "white");
        let mut f = Svf::default();
        for i in 0..n {
            let t = i as f64 / n as f64;
            let fc = 250.0 * (9000.0f64 / 250.0).powf(t);
            ch[i] = f.tick(nz[i], cutoff_g(fc, sr), k, FilterMode::Bp) * (t.powf(1.6) * 0.8) as f32;
        }
    }
    let mut saw = Unison::new(Wave::Saw, 3, 25.0, 0.8, 0.5, 3.0, sr, &mut rng);
    let mut lp = [Svf::default(); 2];
    for i in 0..n {
        let t = i as f64 / n as f64;
        let f = root_hz * 4f64.powf(t * t);
        let (l, r) = saw.tick(f, sr);
        let g = cutoff_g(800.0 * 10f64.powf(t), sr);
        let amp = (t * t * 0.28) as f32;
        out.l[i] += lp[0].tick(l, g, 1.6, FilterMode::Lp) * amp;
        out.r[i] += lp[1].tick(r, g, 1.6, FilterMode::Lp) * amp;
    }
    out.fade(0, ((0.005 * sr) as usize).min(n));
    out.normalize(-1.0);
    out
}

/// the reverse of a riser: falling noise and a sinking tone after a drop
pub fn downlifter(secs: f64, sr: f64, seed: u64) -> Stereo {
    let n = (secs * sr) as usize;
    let rng = Rng::new(seed ^ 0x52);
    let mut out = Stereo::zeros(n);
    for (c, ch) in out.channels_mut().into_iter().enumerate() {
        let nz = osc::noise(n, &mut rng.fork(&format!("d{c}")), "white");
        let mut f = Svf::default();
        for i in 0..n {
            let t = i as f64 / n as f64;
            let fc = 9000.0 * (150.0f64 / 9000.0).powf(t.sqrt());
            ch[i] = f.tick(nz[i], cutoff_g(fc, sr), 1.2, FilterMode::Lp) * ((1.0 - t).powf(1.5) * 0.7) as f32;
        }
    }
    let mut s = Osc::new(Wave::Sine, 0.0);
    for i in 0..n {
        let t = i as f64 / n as f64;
        let v = s.tick(400.0 * (50.0f64 / 400.0).powf(t), sr) * ((1.0 - t).powi(2) * 0.4) as f32;
        out.l[i] += v;
        out.r[i] += v;
    }
    out.fade((0.002 * sr) as usize, ((0.02 * sr) as usize).min(n));
    out.normalize(-1.0);
    out
}

/// sub boom + noise burst: the hit that marks a drop
pub fn impact(sr: f64, seed: u64) -> Stereo {
    let n = (2.6 * sr) as usize;
    let rng = Rng::new(seed ^ 0x53);
    let env = perc(n, sr, 0.0008, 0.02, 2.0, 1.0);
    let mut s = Osc::new(Wave::Sine, 0.0);
    let mut body: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            s.tick(32.0 + 70.0 * (-t / 0.09).exp(), sr) * env[i]
        })
        .collect();
    for v in body.iter_mut() {
        *v = ((*v as f64 * 2.5).tanh() / 2.5f64.tanh()) as f32;
    }
    let mut out = Stereo::from_mono(&body);
    for (c, ch) in out.channels_mut().into_iter().enumerate() {
        let mut nz = osc::noise(n, &mut rng.fork(&format!("i{c}")), "white");
        filter::butter(&mut nz, false, 2800.0, sr, 2);
        let ne = perc(n, sr, 0.0005, 0.0, 0.6, 1.2);
        for i in 0..n {
            ch[i] += nz[i] * ne[i] * 0.25;
        }
    }
    out.normalize(-1.0);
    out
}

/// a crash cymbal played backwards: swells for `secs` and ends exactly on the downbeat
pub fn reverse_crash(secs: f64, sr: f64, seed: u64) -> Stereo {
    let p = DrumParams { decay: (secs as f32 * 1.4).max(1.2), ..DrumParams::default_for(DrumVoice::Crash) };
    let mut c = drums::render(DrumVoice::Crash, &p, sr, seed);
    let n = ((secs * sr) as usize).min(c.len());
    c.truncate(n);
    c.reverse();
    c.fade(((0.05 * sr) as usize).min(n), 32);
    c
}

/// white noise through a sweeping highpass (up = brighter towards the end)
pub fn sweep(secs: f64, sr: f64, seed: u64, up: bool) -> Stereo {
    let n = (secs * sr) as usize;
    let rng = Rng::new(seed ^ 0x54);
    let mut out = Stereo::zeros(n);
    for (c, ch) in out.channels_mut().into_iter().enumerate() {
        let nz = osc::noise(n, &mut rng.fork(&format!("s{c}")), "white");
        let mut f = Svf::default();
        for i in 0..n {
            let t = i as f64 / n as f64;
            let tt = if up { t } else { 1.0 - t };
            let fc = 200.0 * (12000.0f64 / 200.0).powf(tt);
            let amp = if up { t } else { 1.0 - t };
            ch[i] = f.tick(nz[i], cutoff_g(fc, sr), 1.4, FilterMode::Hp) * (amp * 0.6) as f32;
        }
    }
    out.normalize(-1.0);
    out
}
