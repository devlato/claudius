//! The effects rack. Every effect is plain data (serde), so blueprints can store, mutate and diff effect chains;
//! automatable parameters are `Param`s (a constant or a curve through (beat, value) points).

mod dynamics;
mod modulation;
mod space;
mod tone;
mod transition;

use serde::{Deserialize, Serialize};

use crate::audio::Stereo;
use crate::dsp::env::Param;
use crate::dsp::filter::{BiquadKind, FilterMode};

pub use dynamics::{compress, limit, soft_clip, true_peak_db};
pub use space::{delay, reverb};

#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub sr: f64,
    pub bpm: f64,
    pub seed: u64,
}

impl Ctx {
    pub fn spb(&self) -> f64 {
        self.sr * 60.0 / self.bpm
    }
}

fn one() -> f32 {
    1.0
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
        res: f32,
        #[serde(default = "one")]
        drive: f32,
        #[serde(default = "one")]
        mix: f32,
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
        #[serde(default = "one")]
        mix: f32,
    },
    Limiter {
        ceiling_db: f32,
        release_ms: f32,
    },
    Saturate {
        kind: SatKind,
        drive_db: f32,
        #[serde(default = "one")]
        mix: f32,
    },
    Bitcrush {
        bits: f32,
        downsample: f32,
        #[serde(default = "one")]
        mix: f32,
    },
    Reverb {
        decay: f32,
        #[serde(default)]
        damping: f32,
        #[serde(default)]
        predelay_ms: f32,
        #[serde(default = "one")]
        width: f32,
        mix: f32,
        #[serde(default = "default_lowcut")]
        lowcut: f32,
    },
    Delay {
        beats: f32,
        feedback: f32,
        #[serde(default)]
        damping: f32,
        #[serde(default)]
        pingpong: bool,
        mix: f32,
    },
    Chorus {
        rate_hz: f32,
        depth_ms: f32,
        delay_ms: f32,
        mix: f32,
    },
    Flanger {
        rate_hz: f32,
        depth_ms: f32,
        delay_ms: f32,
        feedback: f32,
        mix: f32,
    },
    Phaser {
        rate_hz: f32,
        stages: u32,
        depth: f32,
        feedback: f32,
        mix: f32,
    },
    Width {
        width: f32,
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
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Effect::Gain { .. } => "gain",
            Effect::Eq { .. } => "eq",
            Effect::Filter { .. } => "filter",
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
            Effect::AutoPan { .. } => "autopan",
            Effect::Gate { .. } => "gate",
            Effect::Transient { .. } => "transient",
            Effect::Tape { .. } => "tape",
            Effect::Vinyl { .. } => "vinyl",
            Effect::TapeStop { .. } => "tape_stop",
            Effect::Stutter { .. } => "stutter",
            Effect::Reverse { .. } => "reverse",
        }
    }

    /// effects whose output rings on after the input stops (the renderer keeps room for their tails)
    pub fn tail_secs(&self, bpm: f64) -> f64 {
        match self {
            Effect::Reverb { decay, predelay_ms, .. } => *decay as f64 * 1.2 + *predelay_ms as f64 / 1000.0,
            Effect::Delay { beats, feedback, .. } => {
                let reps = if *feedback > 0.01 { (-60.0 / (20.0 * (*feedback as f64).log10())).min(40.0) } else { 1.0 };
                *beats as f64 * 60.0 / bpm * (reps + 1.0)
            }
            _ => 0.0,
        }
    }
}

pub fn apply(fx: &Effect, x: &mut Stereo, ctx: &Ctx) {
    match fx {
        Effect::Gain { db } => tone::gain(x, db, ctx),
        Effect::Eq { bands } => tone::eq(x, bands, ctx),
        Effect::Filter { mode, cutoff, res, drive, mix } => tone::filter(x, *mode, cutoff, *res, *drive, *mix, ctx),
        Effect::Compressor { threshold_db, ratio, attack_ms, release_ms, knee_db, makeup_db, mix } => {
            dynamics::compress(
                x,
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                *makeup_db,
                *mix,
                None,
                ctx.sr,
            )
        }
        Effect::Limiter { ceiling_db, release_ms } => dynamics::limit(x, *ceiling_db, *release_ms, ctx.sr),
        Effect::Saturate { kind, drive_db, mix } => tone::saturate(x, *kind, *drive_db, *mix, ctx.sr),
        Effect::Bitcrush { bits, downsample, mix } => tone::bitcrush(x, *bits, *downsample, *mix),
        Effect::Reverb { decay, damping, predelay_ms, width, mix, lowcut } => {
            space::reverb(x, *decay, *damping, *predelay_ms, *width, *mix, *lowcut, ctx)
        }
        Effect::Delay { beats, feedback, damping, pingpong, mix } => {
            space::delay(x, *beats, *feedback, *damping, *pingpong, *mix, ctx)
        }
        Effect::Chorus { rate_hz, depth_ms, delay_ms, mix } => {
            modulation::chorus(x, *rate_hz, *depth_ms, *delay_ms, *mix, ctx.sr)
        }
        Effect::Flanger { rate_hz, depth_ms, delay_ms, feedback, mix } => {
            modulation::flanger(x, *rate_hz, *depth_ms, *delay_ms, *feedback, *mix, ctx.sr)
        }
        Effect::Phaser { rate_hz, stages, depth, feedback, mix } => {
            modulation::phaser(x, *rate_hz, *stages, *depth, *feedback, *mix, ctx.sr)
        }
        Effect::Width { width, mono_below_hz } => modulation::width(x, *width, *mono_below_hz, ctx.sr),
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
