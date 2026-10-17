//! The synth engine: subtractive and 2-operator FM voices with unison stacks, filter and amp envelopes, an LFO,
//! glide and drive. Polyphonic (every note its own voice) or monophonic with legato slides (303 / 808 style).

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::audio::{Stereo, midi_to_hz};
use crate::dsp::env::{Adsr, pitch_drop};
use crate::dsp::filter::{FilterMode, Ladder, Svf, cutoff_g};
use crate::dsp::osc::{Osc, Unison, Wave};
use crate::rng::Rng;

fn one() -> f32 {
    1.0
}
fn one_u() -> u32 {
    1
}
fn half() -> f32 {
    0.5
}
fn default_pitch_time() -> f32 {
    0.03
}
fn default_analog() -> f32 {
    2.0
}
fn default_velocity() -> f32 {
    0.6
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OscSpec {
    pub wave: Wave,
    #[serde(default = "one")]
    pub level: f32,
    #[serde(default)]
    pub octave: i32,
    #[serde(default)]
    pub semi: f32,
    #[serde(default = "one_u")]
    pub voices: u32,
    #[serde(default)]
    pub detune: f32,
    #[serde(default = "half")]
    pub width: f32,
    #[serde(default = "half")]
    pub pw: f32,
}

impl OscSpec {
    pub fn new(wave: Wave) -> Self {
        OscSpec { wave, level: 1.0, octave: 0, semi: 0.0, voices: 1, detune: 0.0, width: 0.5, pw: 0.5 }
    }
    pub fn level(mut self, v: f32) -> Self {
        self.level = v;
        self
    }
    pub fn octave(mut self, v: i32) -> Self {
        self.octave = v;
        self
    }
    pub fn semi(mut self, v: f32) -> Self {
        self.semi = v;
        self
    }
    pub fn unison(mut self, voices: u32, detune: f32, width: f32) -> Self {
        self.voices = voices;
        self.detune = detune;
        self.width = width;
        self
    }
    pub fn pw(mut self, v: f32) -> Self {
        self.pw = v;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterSpec {
    pub mode: FilterMode,
    pub cutoff: f32,
    #[serde(default)]
    pub res: f32,
    /// envelope depth in octaves
    #[serde(default)]
    pub env: f32,
    /// 0..1: how much the cutoff follows the played pitch
    #[serde(default)]
    pub keytrack: f32,
    #[serde(default = "one")]
    pub drive: f32,
    /// octaves of cutoff per unit velocity above / below 0.7
    #[serde(default)]
    pub velocity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FmSpec {
    pub ratio: f32,
    pub index: f32,
    pub decay: f32,
    #[serde(default)]
    pub sustain: f32,
    /// level of a fast-decaying ratio-14 "tine" (electric piano bark)
    #[serde(default)]
    pub tine: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LfoSpec {
    /// Hz, or beats per cycle when `sync`
    pub rate: f32,
    #[serde(default)]
    pub sync: bool,
    #[serde(default)]
    pub vibrato: f32,
    #[serde(default)]
    pub filter: f32,
    #[serde(default)]
    pub amp: f32,
    #[serde(default)]
    pub delay: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub osc: Vec<OscSpec>,
    #[serde(default)]
    pub sub: f32,
    #[serde(default)]
    pub noise: f32,
    #[serde(default)]
    pub fm: Option<FmSpec>,
    #[serde(default)]
    pub filter: Option<FilterSpec>,
    pub fenv: Adsr,
    pub aenv: Adsr,
    #[serde(default)]
    pub pitch_env: f32,
    #[serde(default = "default_pitch_time")]
    pub pitch_time: f32,
    #[serde(default)]
    pub lfo: Option<LfoSpec>,
    #[serde(default)]
    pub glide: f32,
    #[serde(default)]
    pub mono: bool,
    #[serde(default)]
    pub legato: bool,
    #[serde(default)]
    pub drive: f32,
    #[serde(default = "default_analog")]
    pub analog: f32,
    #[serde(default = "default_velocity")]
    pub velocity: f32,
}

/// a note for the engine: sample positions, pitch in semitones (60 = middle C), velocity 0..1
#[derive(Clone, Copy, Debug)]
pub struct NoteEv {
    pub start: usize,
    pub gate: usize,
    pub pitch: f64,
    pub vel: f32,
}

struct Core<'a> {
    p: &'a Patch,
    sr: f64,
    stacks: Vec<(Unison, f64, f32)>,
    sub: Osc,
    modu: Osc,
    tine: Osc,
    rng: Rng,
    svf: [Svf; 2],
    lad: [Ladder; 2],
    drive_g: f64,
    drive_n: f64,
    g: f64,
}

impl<'a> Core<'a> {
    fn new(p: &'a Patch, sr: f64, rng: &mut Rng) -> Self {
        let stacks = p
            .osc
            .iter()
            .map(|o| {
                let u = Unison::new(
                    o.wave,
                    o.voices as usize,
                    o.detune as f64,
                    o.width as f64,
                    o.pw as f64,
                    p.analog as f64,
                    sr,
                    rng,
                );
                (u, 2f64.powf(o.octave as f64 + o.semi as f64 / 12.0), o.level)
            })
            .collect();
        let drive_g = 10f64.powf(p.drive.max(0.0) as f64 / 20.0);
        Core {
            p,
            sr,
            stacks,
            sub: Osc::new(Wave::Sine, 0.0),
            modu: Osc::new(Wave::Sine, 0.0),
            tine: Osc::new(Wave::Sine, 0.0),
            rng: rng.fork("noise"),
            svf: [Svf::default(); 2],
            lad: [Ladder::default(); 2],
            drive_g,
            drive_n: drive_g.tanh(),
            g: cutoff_g(p.filter.as_ref().map_or(1000.0, |f| f.cutoff as f64), sr),
        }
    }

    /// filter coefficient for the next block, from the cutoff offset in octaves
    fn set_cutoff(&mut self, cut_oct: f64) {
        if let Some(fs) = &self.p.filter {
            self.g = cutoff_g(fs.cutoff as f64 * 2f64.powf(cut_oct), self.sr);
        }
    }

    #[inline]
    fn tick(&mut self, f: f64, t: f64, vel: f32) -> (f32, f32) {
        let sr = self.sr;
        let mut pm = 0.0;
        if let Some(fm) = &self.p.fm {
            let env = fm.sustain as f64 + (1.0 - fm.sustain as f64) * (-t / fm.decay.max(1e-3) as f64).exp();
            let idx = fm.index as f64 * env * (0.55 + 0.45 * vel as f64);
            pm = self.modu.tick(f * fm.ratio as f64, sr) as f64 * idx / TAU;
            if fm.tine > 0.0 {
                pm += self.tine.tick(f * 14.0, sr) as f64 * fm.tine as f64 * (-t / 0.035).exp() / TAU;
            }
        }
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for (u, ratio, level) in self.stacks.iter_mut() {
            let (a, b) = u.tick_pm(f * *ratio, sr, pm);
            l += a * *level;
            r += b * *level;
        }
        if self.p.sub > 0.0 {
            let s = self.sub.tick(f * 0.5, sr) * self.p.sub;
            l += s;
            r += s;
        }
        if self.p.noise > 0.0 {
            let n = self.rng.normal() as f32 * 0.25 * self.p.noise;
            l += n;
            r += n;
        }
        if let Some(fs) = &self.p.filter {
            let g = self.g;
            if fs.mode == FilterMode::Ladder {
                l = self.lad[0].tick(l, g, fs.res as f64, fs.drive as f64);
                r = self.lad[1].tick(r, g, fs.res as f64, fs.drive as f64);
            } else {
                let k = Svf::k(fs.res as f64);
                l = self.svf[0].tick(l, g, k, fs.mode);
                r = self.svf[1].tick(r, g, k, fs.mode);
            }
        }
        if self.p.drive > 0.0 {
            l = ((l as f64 * self.drive_g).tanh() / self.drive_n) as f32;
            r = ((r as f64 * self.drive_g).tanh() / self.drive_n) as f32;
        }
        (l, r)
    }
}

fn lfo_value(p: &Patch, abs_sample: usize, t_note: f64, sr: f64, bpm: f64) -> (f64, f64) {
    match &p.lfo {
        None => (0.0, 1.0),
        Some(l) => {
            let v = if l.sync {
                let beats = abs_sample as f64 / (sr * 60.0 / bpm);
                (TAU * beats / l.rate.max(0.01) as f64).sin()
            } else {
                (TAU * l.rate as f64 * t_note).sin()
            };
            let ramp = if l.delay > 0.0 { (t_note / l.delay as f64).min(1.0) } else { 1.0 };
            (v * ramp, 1.0 - l.amp as f64 * (0.5 + 0.5 * v))
        }
    }
}

fn mods(p: &Patch, pitch: f64, vel: f32) -> (f64, f64) {
    match &p.filter {
        None => (0.0, 0.0),
        Some(fs) => (fs.keytrack as f64 * (pitch - 60.0) / 12.0, fs.velocity as f64 * (vel as f64 - 0.7) * 2.0),
    }
}

/// control-rate block: envelopes, pitch and filter coefficients are computed every CR samples (amplitude is
/// interpolated across the block, so there is no zipper noise)
const CR: usize = 16;

fn render_note(p: &Patch, n: &NoteEv, k: usize, len: usize, sr: f64, bpm: f64, seed: u64) -> Stereo {
    let rel = p.aenv.release_samples(sr);
    let mut rng = Rng::new(seed ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (n.pitch * 1000.0) as u64);
    let mut core = Core::new(p, sr, &mut rng);
    let f0 = midi_to_hz(n.pitch);
    let (kt, vf) = mods(p, n.pitch, n.vel);
    let env_oct = p.filter.as_ref().map_or(0.0, |f| f.env as f64);
    let lfo_f = p.lfo.as_ref().map_or(0.0, |l| l.filter as f64);
    let vib = p.lfo.as_ref().map_or(0.0, |l| l.vibrato as f64);
    let vel_gain = 1.0 - p.velocity.clamp(0.0, 1.0) * (1.0 - n.vel.clamp(0.0, 1.0));
    let total = (n.gate + rel).min(len.saturating_sub(n.start));
    let mut buf = Stereo::zeros(total);
    let amp_at = |i: usize| -> f64 {
        let t = i as f64 / sr;
        let (_, amp_lfo) = lfo_value(p, n.start + i, t, sr, bpm);
        p.aenv.level(i, n.gate, sr) * amp_lfo
    };
    let mut a0 = amp_at(0);
    let mut b = 0;
    while b < total {
        let e = (b + CR).min(total);
        let t = b as f64 / sr;
        let (lv, _) = lfo_value(p, n.start + b, t, sr, bpm);
        let pd = if p.pitch_env != 0.0 { pitch_drop(t, p.pitch_env as f64, p.pitch_time as f64) } else { 1.0 };
        let f = f0 * pd * if vib != 0.0 { 2f64.powf(vib * lv / 12.0) } else { 1.0 };
        let cut = env_oct * if env_oct != 0.0 { p.fenv.level(b, n.gate, sr) } else { 0.0 } + kt + vf + lfo_f * lv;
        core.set_cutoff(cut);
        let a1 = amp_at(e);
        for i in b..e {
            let w = (i - b) as f64 / (e - b) as f64;
            let (l, r) = core.tick(f, (i as f64) / sr, n.vel);
            let a = ((a0 + (a1 - a0) * w) as f32) * vel_gain;
            buf.l[i] = l * a;
            buf.r[i] = r * a;
        }
        a0 = a1;
        b = e;
    }
    let k = total.min(64);
    for j in 0..k {
        let g = j as f32 / k as f32;
        buf.l[total - 1 - j] *= g;
        buf.r[total - 1 - j] *= g;
    }
    buf
}

/// render a list of notes into a buffer of `len` samples (polyphonic notes in parallel)
pub fn render(p: &Patch, notes: &[NoteEv], len: usize, sr: f64, bpm: f64, seed: u64) -> Stereo {
    use rayon::prelude::*;
    if notes.is_empty() {
        return Stereo::zeros(len);
    }
    if p.mono {
        return render_mono(p, notes, len, sr, bpm, seed);
    }
    let mut out = Stereo::zeros(len);
    let idx: Vec<usize> = (0..notes.len()).filter(|&k| notes[k].start < len).collect();
    for chunk in idx.chunks(96) {
        let bufs: Vec<(usize, Stereo)> =
            chunk.par_iter().map(|&k| (notes[k].start, render_note(p, &notes[k], k, len, sr, bpm, seed))).collect();
        for (start, b) in &bufs {
            out.add_at(b, *start as i64, 1.0, 1.0);
        }
    }
    out
}

/// analog-style envelope generator (state machine) for the mono voice
struct EnvGen {
    level: f64,
    stage: u8,
    a: f64,
    d: f64,
    s: f64,
    r: f64,
}

impl EnvGen {
    fn new(e: &Adsr, sr: f64) -> Self {
        EnvGen {
            level: 0.0,
            stage: 0,
            a: (-(5f64.ln()) / (e.a.max(1e-4) * sr)).exp(),
            d: (-4.6 / (e.d.max(1e-4) * sr)).exp(),
            s: e.s,
            r: (-6.9 / (e.r.max(1e-4) * sr)).exp(),
        }
    }
    fn trigger(&mut self) {
        self.stage = 1;
    }
    fn release(&mut self) {
        if self.stage != 0 {
            self.stage = 3;
        }
    }
    #[inline]
    fn tick(&mut self) -> f64 {
        match self.stage {
            1 => {
                self.level += (1.25 - self.level) * (1.0 - self.a);
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = 2;
                }
            }
            2 => self.level = self.s + (self.level - self.s) * self.d,
            3 => self.level *= self.r,
            _ => {}
        }
        self.level
    }
}

fn render_mono(p: &Patch, notes: &[NoteEv], len: usize, sr: f64, bpm: f64, seed: u64) -> Stereo {
    let mut notes = notes.to_vec();
    notes.sort_by_key(|n| n.start);
    let mut rng = Rng::new(seed);
    let mut core = Core::new(p, sr, &mut rng);
    let mut ae = EnvGen::new(&p.aenv, sr);
    let mut fe = EnvGen::new(&p.fenv, sr);
    let mut out = Stereo::zeros(len);
    let rel = p.aenv.release_samples(sr);
    let first = notes[0].start;
    let end = notes.iter().map(|n| n.start + n.gate).max().unwrap_or(0) + rel;
    let env_oct = p.filter.as_ref().map_or(0.0, |f| f.env as f64);
    let lfo_f = p.lfo.as_ref().map_or(0.0, |l| l.filter as f64);
    let vib = p.lfo.as_ref().map_or(0.0, |l| l.vibrato as f64);
    let glide_c = if p.glide > 0.0 { (-1.0 / (p.glide as f64 * sr)).exp() } else { 0.0 };
    let mut k: Option<usize> = None;
    let mut cur = notes[0].pitch;
    let mut sliding = false;
    let mut t_trig = 0usize;
    for s in first..end.min(len) {
        while k.map_or(0, |i| i + 1) < notes.len() && s >= notes[k.map_or(0, |i| i + 1)].start {
            let next = k.map_or(0, |i| i + 1);
            let legato =
                k.is_some_and(|i| notes[i].start + notes[i].gate > notes[next].start) && (p.legato || p.glide > 0.0);
            if legato {
                sliding = p.glide > 0.0;
            } else {
                ae.trigger();
                fe.trigger();
                cur = notes[next].pitch;
                sliding = false;
                t_trig = s;
            }
            k = Some(next);
        }
        let Some(i) = k else { continue };
        let n = notes[i];
        if s >= n.start + n.gate {
            ae.release();
            fe.release();
        }
        if sliding {
            cur = n.pitch + (cur - n.pitch) * glide_c;
        } else {
            cur = n.pitch;
        }
        let t = (s - t_trig) as f64 / sr;
        let (lv, amp_lfo) = lfo_value(p, s, t, sr, bpm);
        let fe_level = fe.tick();
        if (s - first).is_multiple_of(CR) {
            let (kt, vf) = mods(p, cur, n.vel);
            core.set_cutoff(env_oct * fe_level + kt + vf + lfo_f * lv);
        }
        let pd = if p.pitch_env != 0.0 { pitch_drop(t, p.pitch_env as f64, p.pitch_time as f64) } else { 1.0 };
        let f = midi_to_hz(cur) * pd * if vib != 0.0 { 2f64.powf(vib * lv / 12.0) } else { 1.0 };
        let (l, r) = core.tick(f, t, n.vel);
        let vel_gain = 1.0 - p.velocity.clamp(0.0, 1.0) * (1.0 - n.vel.clamp(0.0, 1.0));
        let a = (ae.tick() * amp_lfo) as f32 * vel_gain;
        out.l[s] = l * a;
        out.r[s] = r * a;
    }
    out
}
