//! The effects rack. Every effect is plain data (serde), so blueprints can store, mutate and diff effect chains;
//! automatable parameters are `Param`s (a constant, a curve through (beat, value) points, or a value for each
//! section). A `Parallel` effect runs chains of its own side by side, each as loud as its own amount.

mod dynamics;
mod modulation;
mod space;
mod tone;
mod transition;

use serde::{Deserialize, Serialize};

use crate::audio::Stereo;
use crate::dsp::env::{Param, Values};
use crate::dsp::filter::{BiquadKind, FilterMode};
use crate::voice::singer::Voice;
use crate::voice::tune::{TuneScale, Tuning};

pub use dynamics::{compress, limit, soft_clip, true_peak_db};
pub use space::{delay, reverb};

#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub sr: f64,
    pub bpm: f64,
    pub seed: u64,
    /// where the buffer begins in the whole it is part of, in samples, and how long that whole is: a window of the
    /// song (`render::render_window`) runs through an effect as that part of the whole song would, its automation,
    /// LFOs, gates and noise where they would be. Tuning a voice reads its harmony from the buffer's start: it plays on
    /// tracks, which a window does not cut.
    pub start: usize,
    pub total: usize,
}

impl Ctx {
    /// for a whole buffer of `total` samples
    pub fn whole(sr: f64, bpm: f64, seed: u64, total: usize) -> Ctx {
        Ctx { sr, bpm, seed, start: 0, total }
    }

    pub fn spb(&self) -> f64 {
        self.sr * 60.0 / self.bpm
    }

    /// `p`'s value at each of the buffer's `n` samples, where they are in the whole
    pub fn values(&self, p: &Param, n: usize, log: bool) -> Values {
        p.values_from(self.start, n, self.spb(), log)
    }
}

fn one() -> f32 {
    1.0
}
fn full() -> Param {
    Param::Const(1.0)
}
fn sixteenth() -> f32 {
    0.25
}
fn default_lowcut() -> f32 {
    250.0
}
fn default_q() -> f32 {
    0.707
}
fn retune_ms() -> f32 {
    40.0
}
fn humanize_cents() -> f32 {
    10.0
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EqBand {
    pub kind: BiquadKind,
    pub freq: f32,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default = "default_q")]
    pub q: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SatKind {
    Tanh,
    Tube,
    Tape,
    Hard,
    Fold,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fx", rename_all = "snake_case")]
pub enum Effect {
    Gain {
        db: Param,
    },
    Eq {
        bands: Vec<EqBand>,
    },
    Filter {
        mode: FilterMode,
        cutoff: Param,
        #[serde(default)]
        res: Param,
        #[serde(default = "one")]
        drive: f32,
        #[serde(default = "full")]
        mix: Param,
    },
    /// only the frequencies from `low_hz` to `high_hz` (0: no edge on that side), through crossovers that let bands
    /// meeting at an edge add up to the whole again: a rack layer's share of the spectrum, or one band of a sound in a
    /// parallel effect's chains, to treat on its own
    Band {
        #[serde(default)]
        low_hz: f32,
        #[serde(default)]
        high_hz: f32,
    },
    Compressor {
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        #[serde(default)]
        knee_db: f32,
        #[serde(default)]
        makeup_db: f32,
        #[serde(default = "full")]
        mix: Param,
    },
    Limiter {
        ceiling_db: f32,
        release_ms: f32,
    },
    Saturate {
        kind: SatKind,
        drive_db: Param,
        #[serde(default = "full")]
        mix: Param,
    },
    Bitcrush {
        bits: Param,
        downsample: f32,
        #[serde(default = "full")]
        mix: Param,
    },
    Reverb {
        decay: f32,
        #[serde(default)]
        damping: f32,
        #[serde(default)]
        predelay_ms: f32,
        #[serde(default = "one")]
        width: f32,
        mix: Param,
        #[serde(default = "default_lowcut")]
        lowcut: f32,
    },
    Delay {
        beats: f32,
        feedback: Param,
        #[serde(default)]
        damping: f32,
        #[serde(default)]
        pingpong: bool,
        mix: Param,
    },
    Chorus {
        rate_hz: f32,
        depth_ms: f32,
        delay_ms: f32,
        mix: Param,
    },
    Flanger {
        rate_hz: f32,
        depth_ms: f32,
        delay_ms: f32,
        feedback: f32,
        mix: Param,
    },
    Phaser {
        rate_hz: f32,
        stages: u32,
        depth: f32,
        feedback: f32,
        mix: Param,
    },
    Width {
        width: Param,
        #[serde(default)]
        mono_below_hz: f32,
    },
    Tremolo {
        beats: f32,
        depth: f32,
    },
    AutoPan {
        beats: f32,
        depth: f32,
    },
    Gate {
        steps: Vec<f32>,
        #[serde(default = "sixteenth")]
        step_beats: f32,
        depth: f32,
        #[serde(default)]
        smooth_ms: f32,
    },
    Transient {
        attack_db: f32,
        sustain_db: f32,
    },
    Tape {
        wow: f32,
        flutter: f32,
        drive_db: f32,
        lowpass_hz: f32,
    },
    Vinyl {
        crackle: f32,
        hiss_db: f32,
    },
    TapeStop {
        start: f64,
        len: f64,
    },
    Stutter {
        start: f64,
        len: f64,
        slice: f64,
    },
    Reverse {
        start: f64,
        len: f64,
    },
    /// the signal through each of `chains` at once, each as loud as its `amount`, over the signal itself as loud as
    /// `dry`: parallel compression, a distorted copy under the clean sound, a reverb that only the drop has
    Parallel {
        chains: Vec<Branch>,
        #[serde(default = "full")]
        dry: Param,
    },
    /// a voice moved towards the notes it means (`voice::tune`): `strength` of the way (0 to 1), brought onto each
    /// note over `speed_ms` after it starts (0: at once, the hard-tuned sound), keeping `humanize` cents of the
    /// singer's own wandering and `vibrato` of their vibrato (0 to 1), read against `scale`, as `voice` sings (a pitch
    /// check's; learnt from the voice itself when not given)
    Tune {
        #[serde(default = "full")]
        strength: Param,
        #[serde(default = "retune_ms")]
        speed_ms: f32,
        #[serde(default = "humanize_cents")]
        humanize: f32,
        #[serde(default = "one")]
        vibrato: f32,
        #[serde(default)]
        scale: TuneScale,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        voice: Option<Voice>,
        /// the weights of each pitch class from each beat on, from `scale` and the song's key and chords (`arrange`
        /// sets them; empty, any note, as for a voice tuned outside a song)
        #[serde(skip)]
        harmony: Vec<(f64, [f32; 12])>,
    },
}

/// every effect's name, as `fx` names it in a blueprint or a script
pub const EFFECT_NAMES: &[&str] = &[
    "gain",
    "eq",
    "filter",
    "band",
    "compressor",
    "limiter",
    "saturate",
    "bitcrush",
    "reverb",
    "delay",
    "chorus",
    "flanger",
    "phaser",
    "width",
    "tremolo",
    "auto_pan",
    "gate",
    "transient",
    "tape",
    "vinyl",
    "tape_stop",
    "stutter",
    "reverse",
    "parallel",
    "tune",
];

/// one of a `Parallel` effect's chains, and how loud it comes out
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub fx: Vec<Effect>,
    #[serde(default = "full")]
    pub amount: Param,
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Effect::Gain { .. } => "gain",
            Effect::Eq { .. } => "eq",
            Effect::Filter { .. } => "filter",
            Effect::Band { .. } => "band",
            Effect::Compressor { .. } => "compressor",
            Effect::Limiter { .. } => "limiter",
            Effect::Saturate { .. } => "saturate",
            Effect::Bitcrush { .. } => "bitcrush",
            Effect::Reverb { .. } => "reverb",
            Effect::Delay { .. } => "delay",
            Effect::Chorus { .. } => "chorus",
            Effect::Flanger { .. } => "flanger",
            Effect::Phaser { .. } => "phaser",
            Effect::Width { .. } => "width",
            Effect::Tremolo { .. } => "tremolo",
            Effect::AutoPan { .. } => "auto_pan",
            Effect::Gate { .. } => "gate",
            Effect::Transient { .. } => "transient",
            Effect::Tape { .. } => "tape",
            Effect::Vinyl { .. } => "vinyl",
            Effect::TapeStop { .. } => "tape_stop",
            Effect::Stutter { .. } => "stutter",
            Effect::Reverse { .. } => "reverse",
            Effect::Parallel { .. } => "parallel",
            Effect::Tune { .. } => "tune",
        }
    }

    /// every parameter that can follow a curve, those in a parallel effect's chains included
    pub fn params_mut(&mut self, f: &mut dyn FnMut(&mut Param)) {
        match self {
            Effect::Gain { db } => f(db),
            Effect::Filter { cutoff, res, mix, .. } => {
                f(cutoff);
                f(res);
                f(mix);
            }
            Effect::Saturate { drive_db: p, mix, .. }
            | Effect::Bitcrush { bits: p, mix, .. }
            | Effect::Delay { feedback: p, mix, .. } => {
                f(p);
                f(mix);
            }
            Effect::Compressor { mix, .. }
            | Effect::Reverb { mix, .. }
            | Effect::Chorus { mix, .. }
            | Effect::Flanger { mix, .. }
            | Effect::Phaser { mix, .. } => f(mix),
            Effect::Width { width, .. } => f(width),
            Effect::Tune { strength, .. } => f(strength),
            Effect::Parallel { chains, dry } => {
                f(dry);
                for b in chains {
                    f(&mut b.amount);
                    b.fx.iter_mut().for_each(|e| e.params_mut(f));
                }
            }
            _ => {}
        }
    }

    /// the same effect `beats` earlier, as for a project that starts that far into the song: its curves and the
    /// beats a transition starts on move
    pub fn shift(&mut self, beats: f64) {
        self.params_mut(&mut |p| *p = p.shifted(beats));
        self.starts_mut(&mut |start| *start -= beats);
    }

    /// the beat each transition starts on, and each change of a tuner's harmony, those in a parallel effect's chains
    /// included
    fn starts_mut(&mut self, f: &mut dyn FnMut(&mut f64)) {
        match self {
            Effect::TapeStop { start, .. } | Effect::Stutter { start, .. } | Effect::Reverse { start, .. } => f(start),
            Effect::Tune { harmony, .. } => harmony.iter_mut().for_each(|(beat, _)| f(beat)),
            Effect::Parallel { chains, .. } => {
                chains.iter_mut().flat_map(|b| b.fx.iter_mut()).for_each(|e| e.starts_mut(f))
            }
            _ => {}
        }
    }

    /// every tuner's harmony set from its scale, by `harmony` (those in a parallel effect's chains included)
    pub fn harmonize(&mut self, harmony: &dyn Fn(&TuneScale) -> Vec<(f64, [f32; 12])>) {
        match self {
            Effect::Tune { scale, harmony: h, .. } => *h = harmony(scale),
            Effect::Parallel { chains, .. } => {
                chains.iter_mut().flat_map(|b| b.fx.iter_mut()).for_each(|e| e.harmonize(harmony))
            }
            _ => {}
        }
    }

    /// how much of its output is heard, for an effect with a dry/wet `mix`
    fn mix_mut(&mut self) -> Option<&mut Param> {
        match self {
            Effect::Filter { mix, .. }
            | Effect::Compressor { mix, .. }
            | Effect::Saturate { mix, .. }
            | Effect::Bitcrush { mix, .. }
            | Effect::Reverb { mix, .. }
            | Effect::Delay { mix, .. }
            | Effect::Chorus { mix, .. }
            | Effect::Flanger { mix, .. }
            | Effect::Phaser { mix, .. } => Some(mix),
            _ => None,
        }
    }

    /// effects whose output rings on after the input stops (the renderer keeps room for their tails)
    pub fn tail_secs(&self, bpm: f64) -> f64 {
        match self {
            Effect::Reverb { decay, predelay_ms, .. } => *decay as f64 * 1.2 + *predelay_ms as f64 / 1000.0,
            Effect::Delay { beats, feedback, .. } => {
                let fb = feedback.peak().min(0.95);
                let reps = if fb > 0.01 { (-60.0 / (20.0 * (fb as f64).log10())).min(40.0) } else { 1.0 };
                *beats as f64 * 60.0 / bpm * (reps + 1.0)
            }
            Effect::Parallel { chains, .. } => chains.iter().map(|b| chain_tail_secs(&b.fx, bpm)).fold(0.0, f64::max),
            _ => 0.0,
        }
    }
}

pub fn apply(fx: &Effect, x: &mut Stereo, ctx: &Ctx) {
    // A mix that moves: the effect all wet, then from the signal towards that by the mix, a sample at a time. Each
    // effect's output moves in a straight line from the signal as its mix goes from 0 to 1, so this is the effect at
    // each moment's mix.
    let mut wet = fx.clone();
    if let Some(mix) = wet.mix_mut().filter(|m| !m.is_const()) {
        let m = ctx.values(&std::mem::replace(mix, Param::Const(1.0)), x.len(), false);
        let dry = x.clone();
        run(&wet, x, ctx);
        for i in 0..x.len().min(dry.len()) {
            let k = m.at(i);
            x.l[i] = dry.l[i] + (x.l[i] - dry.l[i]) * k;
            x.r[i] = dry.r[i] + (x.r[i] - dry.r[i]) * k;
        }
        return;
    }
    run(fx, x, ctx);
}

/// an effect whose mix, if it has one, holds still
fn run(fx: &Effect, x: &mut Stereo, ctx: &Ctx) {
    let k = Param::first;
    match fx {
        Effect::Gain { db } => tone::gain(x, db, ctx),
        Effect::Eq { bands } => tone::eq(x, bands, ctx),
        Effect::Filter { mode, cutoff, res, drive, mix } => tone::filter(x, *mode, cutoff, res, *drive, k(mix), ctx),
        Effect::Band { low_hz, high_hz } => tone::band(x, *low_hz, *high_hz, ctx.sr),
        Effect::Compressor { threshold_db, ratio, attack_ms, release_ms, knee_db, makeup_db, mix } => {
            dynamics::compress(
                x,
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                *makeup_db,
                k(mix),
                None,
                ctx.sr,
            )
        }
        Effect::Limiter { ceiling_db, release_ms } => dynamics::limit(x, *ceiling_db, *release_ms, ctx.sr),
        Effect::Saturate { kind, drive_db, mix } => tone::saturate(x, *kind, drive_db, k(mix), ctx),
        Effect::Bitcrush { bits, downsample, mix } => tone::bitcrush(x, bits, *downsample, k(mix), ctx),
        Effect::Reverb { decay, damping, predelay_ms, width, mix, lowcut } => {
            space::reverb(x, *decay, *damping, *predelay_ms, *width, k(mix), *lowcut, ctx)
        }
        Effect::Delay { beats, feedback, damping, pingpong, mix } => {
            space::delay(x, *beats, feedback, *damping, *pingpong, k(mix), ctx)
        }
        Effect::Chorus { rate_hz, depth_ms, delay_ms, mix } => {
            modulation::chorus(x, *rate_hz, *depth_ms, *delay_ms, k(mix), ctx)
        }
        Effect::Flanger { rate_hz, depth_ms, delay_ms, feedback, mix } => {
            modulation::flanger(x, *rate_hz, *depth_ms, *delay_ms, *feedback, k(mix), ctx)
        }
        Effect::Phaser { rate_hz, stages, depth, feedback, mix } => {
            modulation::phaser(x, *rate_hz, *stages, *depth, *feedback, k(mix), ctx)
        }
        Effect::Width { width, mono_below_hz } => modulation::width(x, width, *mono_below_hz, ctx),
        Effect::Tremolo { beats, depth } => modulation::tremolo(x, *beats, *depth, ctx),
        Effect::AutoPan { beats, depth } => modulation::autopan(x, *beats, *depth, ctx),
        Effect::Gate { steps, step_beats, depth, smooth_ms } => {
            modulation::gate(x, steps, *step_beats, *depth, *smooth_ms, ctx)
        }
        Effect::Transient { attack_db, sustain_db } => dynamics::transient(x, *attack_db, *sustain_db, ctx.sr),
        Effect::Tape { wow, flutter, drive_db, lowpass_hz } => {
            tone::tape(x, *wow, *flutter, *drive_db, *lowpass_hz, ctx)
        }
        Effect::Vinyl { crackle, hiss_db } => tone::vinyl(x, *crackle, *hiss_db, ctx),
        Effect::TapeStop { start, len } => transition::tape_stop(x, *start, *len, ctx),
        Effect::Stutter { start, len, slice } => transition::stutter(x, *start, *len, *slice, ctx),
        Effect::Reverse { start, len } => transition::reverse(x, *start, *len, ctx),
        Effect::Parallel { chains, dry } => parallel(x, chains, dry, ctx),
        Effect::Tune { strength, speed_ms, humanize, vibrato, scale, voice, harmony } => {
            let alone;
            let harmony = if harmony.is_empty() {
                alone = scale.alone();
                &alone
            } else {
                harmony
            };
            let t = Tuning {
                strength: strength.clone(),
                speed_ms: *speed_ms,
                humanize: *humanize,
                vibrato: *vibrato,
                voice: voice.clone(),
            };
            crate::voice::tune::tune(x, harmony, &t, ctx.bpm);
        }
    }
}

/// the signal as loud as `dry`, and through each chain as loud as its amount, all at once
fn parallel(x: &mut Stereo, chains: &[Branch], dry: &Param, ctx: &Ctx) {
    let n = x.len();
    let input = x.clone();
    let d = ctx.values(dry, n, false);
    for i in 0..n {
        let g = d.at(i);
        x.l[i] *= g;
        x.r[i] *= g;
    }
    for b in chains {
        let mut y = input.clone();
        apply_chain(&b.fx, &mut y, ctx);
        let a = ctx.values(&b.amount, n, false);
        for i in 0..n.min(y.len()) {
            let g = a.at(i);
            x.l[i] += y.l[i] * g;
            x.r[i] += y.r[i] * g;
        }
    }
}

pub fn apply_chain(chain: &[Effect], x: &mut Stereo, ctx: &Ctx) {
    for fx in chain {
        apply(fx, x, ctx);
    }
}

pub fn chain_tail_secs(chain: &[Effect], bpm: f64) -> f64 {
    chain.iter().map(|f| f.tail_secs(bpm)).fold(0.0, f64::max)
}

/// dry/wet blend helper: x = dry * (1 - mix) + wet * mix
pub(crate) fn blend(dry: &Stereo, wet: &mut Stereo, mix: f32) {
    if mix >= 0.999 {
        return;
    }
    let n = wet.len().min(dry.len());
    for i in 0..n {
        wet.l[i] = dry.l[i] * (1.0 - mix) + wet.l[i] * mix;
        wet.r[i] = dry.r[i] * (1.0 - mix) + wet.r[i] * mix;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize) -> Stereo {
        let l: Vec<f32> = (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        Stereo { l: l.clone(), r: l }
    }

    const CTX: Ctx = Ctx { sr: 1_000.0, bpm: 60.0, seed: 1, start: 0, total: 0 };

    #[test]
    fn every_effect_is_named_as_a_blueprint_names_it() {
        for name in EFFECT_NAMES {
            let e = serde_json::from_value::<Effect>(serde_json::json!({ "fx": name })).map(|e| e.name().to_string());
            match e {
                Ok(n) => assert_eq!(n, *name),
                Err(e) => assert!(e.to_string().contains("missing field"), "{name}: {e}"),
            }
        }
    }

    #[test]
    fn a_band_keeps_its_frequencies_and_two_bands_meeting_at_an_edge_add_up_to_the_whole() {
        let sr = 48_000.0;
        let ctx = Ctx { sr, bpm: 120.0, seed: 1, start: 0, total: 0 };
        let sine = |hz: f64| {
            let l: Vec<f32> = (0..24_000).map(|i| (std::f64::consts::TAU * hz * i as f64 / sr).sin() as f32).collect();
            Stereo { l: l.clone(), r: l }
        };
        // the level of the second half, once the filters have settled, in dB
        let db = |x: &Stereo| {
            let tail = &x.l[12_000..];
            20.0 * ((tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt() * 2f32.sqrt()).log10()
        };
        let through = |hz: f64, low: f32, high: f32| {
            let mut x = sine(hz);
            apply(&Effect::Band { low_hz: low, high_hz: high }, &mut x, &ctx);
            db(&x)
        };
        assert!(through(400.0, 100.0, 1_000.0).abs() < 0.5, "inside the band: {}", through(400.0, 100.0, 1_000.0));
        for hz in [25.0, 8_000.0] {
            assert!(through(hz, 100.0, 1_000.0) < -20.0, "{hz} Hz: {}", through(hz, 100.0, 1_000.0));
        }
        assert!(through(30.0, 0.0, 0.0).abs() < 0.1, "no edges: the whole");
        // under 100 Hz and over it, added up: as loud as the signal at the edge and around it
        for hz in [50.0, 100.0, 200.0] {
            let (mut low, mut high) = (sine(hz), sine(hz));
            apply(&Effect::Band { low_hz: 0.0, high_hz: 100.0 }, &mut low, &ctx);
            apply(&Effect::Band { low_hz: 100.0, high_hz: 0.0 }, &mut high, &ctx);
            let sum = Stereo {
                l: low.l.iter().zip(&high.l).map(|(a, b)| a + b).collect(),
                r: low.r.iter().zip(&high.r).map(|(a, b)| a + b).collect(),
            };
            assert!(db(&sum).abs() < 0.3, "{hz} Hz: the two bands add up to {} dB", db(&sum));
        }
    }

    #[test]
    fn a_parallel_effect_is_the_signal_and_each_chain_as_loud_as_their_amounts() {
        let x = tone(2_000);
        let gain = |db: f32| Effect::Gain { db: db.into() };
        let branch = |fx, amount: Param| Branch { fx: vec![fx], amount };
        // half the signal, and the signal 6 dB down at a quarter: 0.5 + 0.25 * 0.501
        let mut y = x.clone();
        let p = Effect::Parallel { chains: vec![branch(gain(-6.0), 0.25.into())], dry: 0.5.into() };
        apply(&p, &mut y, &CTX);
        let k = 0.5 + 0.25 * crate::audio::db_to_gain(-6.0);
        assert!(y.l.iter().zip(&x.l).all(|(a, b)| (a - b * k).abs() < 1e-6));
        // an amount that moves: nothing of the chain at beat 0, all of it from beat 1 (a thousand samples a beat)
        let mut y = x.clone();
        let moving = Param::Curve(vec![(0.0, 0.0), (1.0, 1.0)]);
        apply(&Effect::Parallel { chains: vec![branch(gain(0.0), moving)], dry: 0.0.into() }, &mut y, &CTX);
        assert_eq!(y.l[0], 0.0);
        assert!((y.l[500] - x.l[500] * 0.5).abs() < 1e-6 && (y.l[1_500] - x.l[1_500]).abs() < 1e-6);
    }

    #[test]
    fn a_mix_that_moves_is_the_effect_at_each_moments_mix() {
        let x = tone(3_000);
        let crush = |mix: Param| Effect::Bitcrush { bits: 3.0.into(), downsample: 1.0, mix };
        let held = |m: f32| {
            let mut y = x.clone();
            apply(&crush(m.into()), &mut y, &CTX);
            y
        };
        let mut moving = x.clone();
        apply(&crush(Param::Curve(vec![(0.0, 0.0), (2.0, 1.0)])), &mut moving, &CTX);
        // at beat 1 the mix is a half, at beat 2 and after it is all wet
        let (half, full) = (held(0.5), held(1.0));
        assert!((moving.l[1_000] - half.l[1_000]).abs() < 1e-6);
        assert!((moving.l[2_500] - full.l[2_500]).abs() < 1e-6 && (moving.l[0] - x.l[0]).abs() < 1e-6);
    }

    #[test]
    fn a_width_that_moves_is_the_width_at_each_moment() {
        // left and right apart; mono from beat 1 on, through the filter that keeps the lows in the middle
        let l: Vec<f32> = (0..3_000).map(|i| (i as f32 * 0.3).sin() * 0.5).collect();
        let x = Stereo { r: l.iter().map(|v| -v).collect(), l };
        let mut y = x.clone();
        let w = Effect::Width { width: Param::Curve(vec![(0.99, 1.0), (1.0, 0.0)]), mono_below_hz: 40.0 };
        apply(&w, &mut y, &CTX);
        assert!((1_000..3_000).all(|i| (y.l[i] - y.r[i]).abs() < 1e-6), "mono as soon as the width is 0");
        assert!((200..900).any(|i| (y.l[i] - y.r[i]).abs() > 0.1), "and wide before");
    }

    #[test]
    fn a_projects_slice_moves_the_curves_in_a_parallel_effects_chains_too() {
        let mut p = Effect::Parallel {
            chains: vec![Branch {
                fx: vec![Effect::Stutter { start: 8.0, len: 1.0, slice: 0.25 }],
                amount: Param::Curve(vec![(4.0, 0.0), (8.0, 1.0)]),
            }],
            dry: 1.0.into(),
        };
        p.shift(4.0);
        let Effect::Parallel { chains, .. } = &p else { unreachable!() };
        assert_eq!(chains[0].amount, Param::Curve(vec![(0.0, 0.0), (4.0, 1.0)]));
        assert_eq!(chains[0].fx[0], Effect::Stutter { start: 4.0, len: 1.0, slice: 0.25 });
    }
}
