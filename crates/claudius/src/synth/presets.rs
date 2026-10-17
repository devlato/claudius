//! Factory patches for the synth engine, tagged by the roles they suit. Blueprints copy a patch and may mutate it.

use super::voice::{FilterSpec, FmSpec, LfoSpec, OscSpec, Patch};
use crate::dsp::env::Adsr;
use crate::dsp::filter::FilterMode;
use crate::dsp::osc::Wave;

const fn a(a: f64, d: f64, s: f64, r: f64) -> Adsr {
    Adsr::new(a, d, s, r)
}

fn base(osc: Vec<OscSpec>, aenv: Adsr) -> Patch {
    Patch {
        osc,
        sub: 0.0,
        noise: 0.0,
        fm: None,
        filter: None,
        fenv: a(0.001, 0.2, 0.0, 0.2),
        aenv,
        pitch_env: 0.0,
        pitch_time: 0.03,
        lfo: None,
        glide: 0.0,
        mono: false,
        legato: false,
        drive: 0.0,
        analog: 2.0,
        velocity: 0.6,
    }
}

fn filt(mode: FilterMode, cutoff: f32, res: f32, env: f32, keytrack: f32) -> Option<FilterSpec> {
    Some(FilterSpec { mode, cutoff, res, env, keytrack, drive: 1.0, velocity: 0.3 })
}

fn o(w: Wave) -> OscSpec {
    OscSpec::new(w)
}

pub const ROLES: &[(&str, &[&str])] = &[
    ("sub_bass", &["bass"]),
    ("808", &["bass"]),
    ("reese", &["bass"]),
    ("acid", &["bass", "lead"]),
    ("saw_bass", &["bass"]),
    ("pluck_bass", &["bass"]),
    ("organ_bass", &["bass"]),
    ("wobble", &["bass"]),
    ("pluck", &["arp", "chords", "lead"]),
    ("supersaw", &["lead", "chords"]),
    ("pad", &["pad", "chords"]),
    ("warm_pad", &["pad"]),
    ("stab", &["chords"]),
    ("epiano", &["keys", "chords"]),
    ("keys", &["keys", "chords"]),
    ("bell", &["lead", "arp"]),
    ("organ", &["chords", "keys"]),
    ("strings", &["pad", "chords"]),
    ("square_lead", &["lead"]),
    ("chip", &["lead", "arp"]),
];

pub fn names() -> Vec<&'static str> {
    ROLES.iter().map(|(n, _)| *n).collect()
}

pub fn for_role(role: &str) -> Vec<&'static str> {
    ROLES.iter().filter(|(_, r)| r.contains(&role)).map(|(n, _)| *n).collect()
}

pub fn get(name: &str) -> Option<Patch> {
    let p = match name {
        "sub_bass" => Patch {
            drive: 4.0,
            mono: true,
            legato: true,
            analog: 0.4,
            velocity: 0.3,
            ..base(vec![o(Wave::Sine)], a(0.004, 0.2, 1.0, 0.06))
        },
        "808" => Patch {
            filter: filt(FilterMode::Lp, 1900.0, 0.0, 0.0, 0.0),
            pitch_env: 5.0,
            pitch_time: 0.035,
            drive: 9.0,
            mono: true,
            legato: true,
            glide: 0.07,
            analog: 0.0,
            velocity: 0.2,
            ..base(vec![o(Wave::Sine)], a(0.002, 1.2, 0.55, 0.12))
        },
        "reese" => Patch {
            sub: 0.5,
            filter: Some(FilterSpec {
                mode: FilterMode::Ladder,
                cutoff: 700.0,
                res: 0.25,
                env: 0.4,
                keytrack: 0.2,
                drive: 1.5,
                velocity: 0.0,
            }),
            fenv: a(0.01, 0.4, 0.6, 0.3),
            lfo: Some(LfoSpec { rate: 0.35, sync: false, vibrato: 0.0, filter: 0.35, amp: 0.0, delay: 0.0 }),
            mono: true,
            legato: true,
            glide: 0.03,
            drive: 4.0,
            ..base(vec![o(Wave::Saw).unison(2, 22.0, 0.5), o(Wave::Saw).octave(-1).level(0.5)], a(0.01, 0.3, 0.9, 0.12))
        },
        "acid" => Patch {
            filter: Some(FilterSpec {
                mode: FilterMode::Ladder,
                cutoff: 320.0,
                res: 0.82,
                env: 3.2,
                keytrack: 0.3,
                drive: 2.0,
                velocity: 1.0,
            }),
            fenv: a(0.002, 0.16, 0.0, 0.06),
            mono: true,
            legato: true,
            glide: 0.045,
            drive: 3.0,
            analog: 1.0,
            ..base(vec![o(Wave::Saw)], a(0.002, 0.25, 0.75, 0.04))
        },
        "saw_bass" => Patch {
            sub: 0.4,
            filter: filt(FilterMode::Lp, 420.0, 0.25, 2.2, 0.4),
            fenv: a(0.002, 0.22, 0.15, 0.1),
            mono: true,
            drive: 3.0,
            ..base(
                vec![o(Wave::Saw).unison(2, 8.0, 0.2), o(Wave::Square).octave(-1).level(0.4)],
                a(0.003, 0.3, 0.8, 0.08),
            )
        },
        "pluck_bass" => Patch {
            filter: filt(FilterMode::Lp, 300.0, 0.3, 3.0, 0.3),
            fenv: a(0.001, 0.12, 0.0, 0.08),
            mono: true,
            drive: 2.0,
            ..base(vec![o(Wave::Square).level(0.7), o(Wave::Saw).level(0.5)], a(0.002, 0.25, 0.2, 0.1))
        },
        "organ_bass" => Patch {
            filter: filt(FilterMode::Lp, 1500.0, 0.0, 0.0, 0.0),
            mono: true,
            drive: 3.0,
            analog: 0.5,
            ..base(
                vec![o(Wave::Sine), o(Wave::Sine).octave(1).level(0.5), o(Wave::Sine).octave(2).level(0.15)],
                a(0.003, 0.15, 0.9, 0.06),
            )
        },
        "wobble" => Patch {
            sub: 0.6,
            filter: Some(FilterSpec {
                mode: FilterMode::Ladder,
                cutoff: 220.0,
                res: 0.55,
                env: 0.0,
                keytrack: 0.2,
                drive: 2.0,
                velocity: 0.0,
            }),
            lfo: Some(LfoSpec { rate: 0.5, sync: true, vibrato: 0.0, filter: 3.6, amp: 0.0, delay: 0.0 }),
            mono: true,
            legato: true,
            drive: 6.0,
            ..base(
                vec![o(Wave::Saw).unison(3, 18.0, 0.4), o(Wave::Square).octave(-1).level(0.5)],
                a(0.005, 0.2, 1.0, 0.1),
            )
        },
        "pluck" => Patch {
            filter: Some(FilterSpec {
                mode: FilterMode::Lp,
                cutoff: 450.0,
                res: 0.2,
                env: 4.2,
                keytrack: 0.5,
                drive: 1.0,
                velocity: 0.6,
            }),
            fenv: a(0.001, 0.22, 0.0, 0.2),
            drive: 1.0,
            ..base(
                vec![o(Wave::Saw).unison(3, 14.0, 0.7), o(Wave::Square).octave(1).level(0.2)],
                a(0.001, 0.45, 0.0, 0.35),
            )
        },
        "supersaw" => Patch {
            filter: filt(FilterMode::Lp, 5200.0, 0.1, 0.6, 0.3),
            fenv: a(0.005, 0.5, 0.5, 0.4),
            lfo: Some(LfoSpec { rate: 5.2, sync: false, vibrato: 0.12, filter: 0.0, amp: 0.0, delay: 0.35 }),
            analog: 3.0,
            ..base(
                vec![o(Wave::Saw).unison(7, 32.0, 0.9), o(Wave::Saw).octave(-1).unison(3, 18.0, 0.6).level(0.45)],
                a(0.006, 0.4, 0.85, 0.3),
            )
        },
        "pad" => Patch {
            filter: filt(FilterMode::Lp, 1800.0, 0.12, 0.8, 0.2),
            fenv: a(0.9, 1.5, 0.55, 1.4),
            lfo: Some(LfoSpec { rate: 0.13, sync: false, vibrato: 0.0, filter: 0.25, amp: 0.0, delay: 0.0 }),
            analog: 4.0,
            velocity: 0.2,
            ..base(
                vec![o(Wave::Saw).unison(5, 20.0, 0.8), o(Wave::Tri).octave(1).unison(2, 8.0, 0.5).level(0.25)],
                a(0.55, 1.0, 0.85, 1.6),
            )
        },
        "warm_pad" => Patch {
            filter: filt(FilterMode::Lp, 1100.0, 0.2, 0.5, 0.2),
            fenv: a(1.2, 2.0, 0.6, 1.5),
            lfo: Some(LfoSpec { rate: 0.2, sync: false, vibrato: 0.05, filter: 0.2, amp: 0.0, delay: 0.0 }),
            analog: 4.0,
            velocity: 0.2,
            ..base(vec![o(Wave::Tri).unison(3, 10.0, 0.7), o(Wave::Pulse).pw(0.3).level(0.3)], a(0.8, 1.2, 0.9, 2.0))
        },
        "stab" => Patch {
            filter: Some(FilterSpec {
                mode: FilterMode::Lp,
                cutoff: 1300.0,
                res: 0.2,
                env: 2.4,
                keytrack: 0.3,
                drive: 1.0,
                velocity: 0.5,
            }),
            fenv: a(0.001, 0.14, 0.12, 0.12),
            drive: 1.5,
            ..base(vec![o(Wave::Saw).unison(3, 12.0, 0.6), o(Wave::Square).level(0.4)], a(0.001, 0.22, 0.25, 0.14))
        },
        "epiano" => Patch {
            fm: Some(FmSpec { ratio: 1.0, index: 2.2, decay: 0.55, sustain: 0.15, tine: 1.2 }),
            filter: filt(FilterMode::Lp, 6000.0, 0.0, 0.0, 0.5),
            velocity: 0.7,
            analog: 1.0,
            ..base(vec![o(Wave::Sine), o(Wave::Sine).octave(1).level(0.15)], a(0.002, 2.2, 0.0, 0.35))
        },
        "keys" => Patch {
            fm: Some(FmSpec { ratio: 2.0, index: 1.4, decay: 0.25, sustain: 0.05, tine: 0.4 }),
            filter: filt(FilterMode::Lp, 3500.0, 0.0, 0.6, 0.6),
            fenv: a(0.001, 0.6, 0.0, 0.3),
            velocity: 0.7,
            analog: 1.5,
            ..base(vec![o(Wave::Sine), o(Wave::Tri).octave(1).level(0.2)], a(0.002, 1.6, 0.0, 0.5))
        },
        "bell" => Patch {
            fm: Some(FmSpec { ratio: 3.5, index: 3.6, decay: 1.0, sustain: 0.1, tine: 0.0 }),
            analog: 0.5,
            ..base(vec![o(Wave::Sine)], a(0.001, 2.8, 0.0, 1.5))
        },
        "organ" => Patch {
            drive: 2.0,
            analog: 0.8,
            ..base(
                vec![
                    o(Wave::Sine),
                    o(Wave::Sine).octave(1).level(0.6),
                    o(Wave::Sine).octave(-1).level(0.5),
                    o(Wave::Sine).semi(19.0).level(0.3),
                    o(Wave::Sine).octave(2).level(0.25),
                ],
                a(0.004, 0.05, 1.0, 0.07),
            )
        },
        "strings" => Patch {
            filter: filt(FilterMode::Lp, 3200.0, 0.05, 0.3, 0.2),
            fenv: a(0.3, 0.8, 0.8, 0.8),
            lfo: Some(LfoSpec { rate: 5.0, sync: false, vibrato: 0.08, filter: 0.0, amp: 0.0, delay: 0.5 }),
            velocity: 0.3,
            ..base(vec![o(Wave::Saw).unison(5, 14.0, 0.8)], a(0.32, 0.6, 0.9, 0.7))
        },
        "square_lead" => Patch {
            filter: filt(FilterMode::Lp, 2800.0, 0.25, 1.0, 0.5),
            fenv: a(0.002, 0.3, 0.4, 0.2),
            lfo: Some(LfoSpec { rate: 5.5, sync: false, vibrato: 0.15, filter: 0.0, amp: 0.0, delay: 0.25 }),
            mono: true,
            legato: true,
            glide: 0.035,
            drive: 2.0,
            ..base(vec![o(Wave::Square), o(Wave::Saw).octave(1).level(0.25)], a(0.004, 0.2, 0.85, 0.15))
        },
        "chip" => {
            Patch { analog: 0.0, velocity: 0.3, ..base(vec![o(Wave::Pulse).pw(0.25)], a(0.001, 0.12, 0.6, 0.05)) }
        }
        _ => return None,
    };
    Some(p)
}
