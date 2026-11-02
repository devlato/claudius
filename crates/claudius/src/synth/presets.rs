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
    ("fm_bass", &["bass"]),
    ("rubber_bass", &["bass"]),
    ("moog_bass", &["bass"]),
    ("dist_bass", &["bass"]),
    ("deep_bass", &["bass"]),
    ("saw_lead", &["lead"]),
    ("pwm_lead", &["lead", "arp"]),
    ("whistle", &["lead"]),
    ("brass_lead", &["lead", "chords"]),
    ("fm_lead", &["lead", "arp"]),
    ("vox_lead", &["lead"]),
    ("solina", &["pad", "chords"]),
    ("glass_pad", &["pad"]),
    ("pwm_pad", &["pad", "chords"]),
    ("air_pad", &["pad"]),
    ("vowel_pad", &["pad"]),
    ("swell_pad", &["pad"]),
    ("tine_piano", &["keys", "chords"]),
    ("clav", &["keys", "chords"]),
    ("house_piano", &["keys", "chords"]),
    ("brass_stab", &["chords"]),
    ("rock_organ", &["chords", "keys"]),
    ("fm_marimba", &["arp", "lead"]),
    ("fm_kalimba", &["arp", "lead"]),
    ("harp_pluck", &["arp", "chords"]),
    ("mallet", &["arp", "keys", "lead"]),
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
        // a DX bass: a sine whose FM brightness falls away fast, over a sub
        "fm_bass" => Patch {
            sub: 0.35,
            fm: Some(FmSpec { ratio: 1.0, index: 2.8, decay: 0.12, sustain: 0.18, tine: 0.0 }),
            mono: true,
            drive: 2.0,
            analog: 0.5,
            velocity: 0.5,
            ..base(vec![o(Wave::Sine)], a(0.002, 0.35, 0.7, 0.08))
        },
        "rubber_bass" => Patch {
            sub: 0.5,
            filter: filt(FilterMode::Lp, 520.0, 0.32, 1.6, 0.3),
            fenv: a(0.002, 0.18, 0.2, 0.1),
            mono: true,
            legato: true,
            glide: 0.06,
            drive: 2.5,
            ..base(vec![o(Wave::Square).level(0.8)], a(0.004, 0.3, 0.75, 0.09))
        },
        "moog_bass" => Patch {
            filter: Some(FilterSpec {
                mode: FilterMode::Ladder,
                cutoff: 380.0,
                res: 0.35,
                env: 2.0,
                keytrack: 0.35,
                drive: 1.8,
                velocity: 0.6,
            }),
            fenv: a(0.002, 0.25, 0.25, 0.12),
            mono: true,
            legato: true,
            glide: 0.03,
            drive: 3.0,
            analog: 1.5,
            ..base(
                vec![o(Wave::Saw), o(Wave::Saw).semi(0.08), o(Wave::Square).octave(-1).level(0.5)],
                a(0.003, 0.3, 0.85, 0.08),
            )
        },
        "dist_bass" => Patch {
            sub: 0.3,
            filter: filt(FilterMode::Lp, 950.0, 0.22, 1.2, 0.3),
            fenv: a(0.002, 0.2, 0.4, 0.1),
            mono: true,
            drive: 9.0,
            ..base(
                vec![o(Wave::Saw).unison(2, 14.0, 0.3), o(Wave::Square).octave(-1).level(0.45)],
                a(0.003, 0.2, 0.9, 0.07),
            )
        },
        "deep_bass" => Patch {
            sub: 0.4,
            filter: filt(FilterMode::Lp, 650.0, 0.1, 0.6, 0.2),
            fenv: a(0.004, 0.3, 0.5, 0.15),
            mono: true,
            legato: true,
            drive: 1.5,
            analog: 1.0,
            velocity: 0.4,
            ..base(vec![o(Wave::Tri), o(Wave::Sine).octave(1).level(0.2)], a(0.006, 0.4, 0.85, 0.12))
        },
        "saw_lead" => Patch {
            filter: filt(FilterMode::Lp, 3400.0, 0.18, 0.9, 0.4),
            fenv: a(0.003, 0.35, 0.55, 0.2),
            lfo: Some(LfoSpec { rate: 5.4, sync: false, vibrato: 0.14, filter: 0.0, amp: 0.0, delay: 0.3 }),
            mono: true,
            legato: true,
            glide: 0.03,
            drive: 1.5,
            ..base(
                vec![o(Wave::Saw).unison(3, 12.0, 0.5), o(Wave::Saw).octave(-1).level(0.3)],
                a(0.004, 0.25, 0.85, 0.18),
            )
        },
        // two pulses of different widths, detuned, beating as a pulse-width sweep does
        "pwm_lead" => Patch {
            filter: filt(FilterMode::Lp, 4200.0, 0.12, 0.6, 0.4),
            fenv: a(0.002, 0.3, 0.6, 0.2),
            lfo: Some(LfoSpec { rate: 5.0, sync: false, vibrato: 0.1, filter: 0.0, amp: 0.0, delay: 0.35 }),
            mono: true,
            legato: true,
            glide: 0.025,
            ..base(
                vec![o(Wave::Pulse).pw(0.32), o(Wave::Pulse).pw(0.46).semi(0.07).level(0.8)],
                a(0.004, 0.3, 0.8, 0.16),
            )
        },
        "whistle" => Patch {
            noise: 0.04,
            filter: filt(FilterMode::Lp, 6000.0, 0.0, 0.0, 0.5),
            lfo: Some(LfoSpec { rate: 5.6, sync: false, vibrato: 0.2, filter: 0.0, amp: 0.05, delay: 0.25 }),
            mono: true,
            legato: true,
            glide: 0.05,
            analog: 1.0,
            velocity: 0.4,
            ..base(vec![o(Wave::Sine), o(Wave::Sine).octave(1).level(0.06)], a(0.035, 0.2, 0.9, 0.12))
        },
        // a brassy swell: the filter opens over the first tenth of a second
        "brass_lead" => Patch {
            filter: filt(FilterMode::Lp, 700.0, 0.12, 3.0, 0.4),
            fenv: a(0.07, 0.35, 0.6, 0.2),
            lfo: Some(LfoSpec { rate: 5.2, sync: false, vibrato: 0.1, filter: 0.0, amp: 0.0, delay: 0.4 }),
            drive: 1.5,
            velocity: 0.7,
            ..base(
                vec![o(Wave::Saw).unison(3, 9.0, 0.5), o(Wave::Saw).octave(-1).level(0.35)],
                a(0.03, 0.3, 0.85, 0.15),
            )
        },
        "fm_lead" => Patch {
            fm: Some(FmSpec { ratio: 2.0, index: 1.8, decay: 0.6, sustain: 0.55, tine: 0.0 }),
            lfo: Some(LfoSpec { rate: 5.5, sync: false, vibrato: 0.12, filter: 0.0, amp: 0.0, delay: 0.3 }),
            mono: true,
            legato: true,
            glide: 0.02,
            analog: 0.8,
            ..base(vec![o(Wave::Sine), o(Wave::Sine).octave(-1).level(0.25)], a(0.004, 0.4, 0.8, 0.2))
        },
        // a saw through a resonant band-pass that drifts slowly, as a voice's vowel does
        "vox_lead" => Patch {
            filter: Some(FilterSpec {
                mode: FilterMode::Bp,
                cutoff: 1100.0,
                res: 0.45,
                env: 0.4,
                keytrack: 0.4,
                drive: 1.2,
                velocity: 0.3,
            }),
            fenv: a(0.05, 0.5, 0.6, 0.3),
            lfo: Some(LfoSpec { rate: 0.35, sync: false, vibrato: 0.08, filter: 0.35, amp: 0.0, delay: 0.0 }),
            mono: true,
            legato: true,
            glide: 0.04,
            drive: 1.0,
            ..base(vec![o(Wave::Saw).unison(2, 8.0, 0.4), o(Wave::Pulse).pw(0.4).level(0.4)], a(0.04, 0.3, 0.9, 0.2))
        },
        "solina" => Patch {
            filter: filt(FilterMode::Lp, 2600.0, 0.08, 0.4, 0.2),
            fenv: a(0.3, 0.8, 0.8, 0.9),
            lfo: Some(LfoSpec { rate: 4.6, sync: false, vibrato: 0.06, filter: 0.0, amp: 0.0, delay: 0.2 }),
            analog: 3.0,
            velocity: 0.25,
            ..base(
                vec![o(Wave::Saw).unison(6, 18.0, 0.9), o(Wave::Saw).octave(1).unison(3, 12.0, 0.7).level(0.35)],
                a(0.25, 0.8, 0.9, 0.9),
            )
        },
        "glass_pad" => Patch {
            fm: Some(FmSpec { ratio: 3.0, index: 1.1, decay: 2.0, sustain: 0.4, tine: 0.0 }),
            filter: filt(FilterMode::Lp, 5000.0, 0.05, 0.0, 0.4),
            lfo: Some(LfoSpec { rate: 0.2, sync: false, vibrato: 0.03, filter: 0.0, amp: 0.08, delay: 0.0 }),
            analog: 2.0,
            velocity: 0.2,
            ..base(vec![o(Wave::Sine).unison(2, 6.0, 0.6), o(Wave::Tri).octave(1).level(0.2)], a(0.4, 1.5, 0.8, 2.4))
        },
        "pwm_pad" => Patch {
            filter: filt(FilterMode::Lp, 1600.0, 0.15, 0.5, 0.2),
            fenv: a(0.6, 1.2, 0.6, 1.2),
            lfo: Some(LfoSpec { rate: 0.18, sync: false, vibrato: 0.02, filter: 0.3, amp: 0.0, delay: 0.0 }),
            analog: 3.5,
            velocity: 0.2,
            ..base(
                vec![
                    o(Wave::Pulse).pw(0.3).unison(3, 14.0, 0.8),
                    o(Wave::Pulse).pw(0.45).unison(2, 9.0, 0.6).level(0.7),
                ],
                a(0.45, 1.0, 0.85, 1.5),
            )
        },
        "air_pad" => Patch {
            noise: 0.18,
            filter: filt(FilterMode::Lp, 2400.0, 0.2, 0.3, 0.3),
            fenv: a(1.0, 1.5, 0.7, 1.8),
            lfo: Some(LfoSpec { rate: 0.11, sync: false, vibrato: 0.0, filter: 0.4, amp: 0.0, delay: 0.0 }),
            analog: 3.0,
            velocity: 0.15,
            ..base(vec![o(Wave::Tri).unison(4, 12.0, 0.9), o(Wave::Sine).octave(1).level(0.3)], a(0.9, 1.2, 0.9, 2.2))
        },
        // a vowel-ish "ah": soft waves through a resonant low-pass, slow vibrato
        "vowel_pad" => Patch {
            filter: filt(FilterMode::Lp, 1250.0, 0.32, 0.2, 0.3),
            fenv: a(0.5, 1.0, 0.8, 1.2),
            lfo: Some(LfoSpec { rate: 4.8, sync: false, vibrato: 0.05, filter: 0.05, amp: 0.0, delay: 0.4 }),
            analog: 2.5,
            velocity: 0.2,
            ..base(
                vec![o(Wave::Tri).unison(4, 10.0, 0.8), o(Wave::Saw).unison(2, 7.0, 0.5).level(0.25)],
                a(0.45, 1.0, 0.9, 1.4),
            )
        },
        "swell_pad" => Patch {
            filter: filt(FilterMode::Lp, 700.0, 0.35, 0.6, 0.2),
            fenv: a(1.5, 2.0, 0.5, 1.5),
            lfo: Some(LfoSpec { rate: 0.07, sync: false, vibrato: 0.0, filter: 1.3, amp: 0.0, delay: 0.0 }),
            analog: 4.0,
            velocity: 0.2,
            ..base(vec![o(Wave::Saw).unison(5, 22.0, 0.9)], a(0.7, 1.0, 0.9, 1.8))
        },
        // the bright tine piano of the 80s: more FM, and more of it in the attack
        "tine_piano" => Patch {
            fm: Some(FmSpec { ratio: 1.0, index: 3.2, decay: 0.35, sustain: 0.1, tine: 2.2 }),
            filter: filt(FilterMode::Lp, 8000.0, 0.0, 0.0, 0.5),
            velocity: 0.75,
            analog: 0.8,
            ..base(vec![o(Wave::Sine), o(Wave::Sine).octave(1).level(0.2)], a(0.001, 1.8, 0.0, 0.3))
        },
        "clav" => Patch {
            filter: filt(FilterMode::Bp, 1700.0, 0.35, 1.6, 0.6),
            fenv: a(0.001, 0.12, 0.2, 0.08),
            drive: 2.0,
            velocity: 0.8,
            ..base(vec![o(Wave::Pulse).pw(0.18), o(Wave::Saw).level(0.4)], a(0.001, 0.35, 0.15, 0.06))
        },
        "house_piano" => Patch {
            fm: Some(FmSpec { ratio: 1.0, index: 4.0, decay: 0.18, sustain: 0.2, tine: 0.6 }),
            filter: filt(FilterMode::Lp, 7000.0, 0.0, 0.0, 0.4),
            velocity: 0.7,
            analog: 0.6,
            ..base(
                vec![o(Wave::Sine), o(Wave::Tri).octave(1).level(0.25), o(Wave::Sine).octave(2).level(0.08)],
                a(0.001, 1.1, 0.12, 0.25),
            )
        },
        "brass_stab" => Patch {
            filter: filt(FilterMode::Lp, 1100.0, 0.15, 2.4, 0.4),
            fenv: a(0.012, 0.2, 0.3, 0.12),
            drive: 1.5,
            velocity: 0.7,
            ..base(
                vec![o(Wave::Saw).unison(4, 10.0, 0.7), o(Wave::Saw).octave(-1).level(0.3)],
                a(0.008, 0.25, 0.35, 0.12),
            )
        },
        // drawbars through an overdriven amp, with the wobble of a spinning speaker
        "rock_organ" => Patch {
            drive: 6.0,
            analog: 1.0,
            lfo: Some(LfoSpec { rate: 6.2, sync: false, vibrato: 0.04, filter: 0.0, amp: 0.12, delay: 0.0 }),
            ..base(
                vec![
                    o(Wave::Sine),
                    o(Wave::Sine).octave(1).level(0.7),
                    o(Wave::Sine).octave(-1).level(0.6),
                    o(Wave::Sine).semi(19.0).level(0.4),
                    o(Wave::Square).octave(1).level(0.08),
                ],
                a(0.003, 0.05, 1.0, 0.06),
            )
        },
        "fm_marimba" => Patch {
            fm: Some(FmSpec { ratio: 4.0, index: 1.6, decay: 0.08, sustain: 0.0, tine: 0.0 }),
            analog: 0.3,
            velocity: 0.7,
            ..base(vec![o(Wave::Sine), o(Wave::Sine).octave(2).level(0.06)], a(0.001, 0.55, 0.0, 0.25))
        },
        "fm_kalimba" => Patch {
            fm: Some(FmSpec { ratio: 2.8, index: 2.2, decay: 0.06, sustain: 0.0, tine: 0.4 }),
            analog: 0.4,
            velocity: 0.6,
            ..base(vec![o(Wave::Sine), o(Wave::Tri).octave(1).level(0.1)], a(0.001, 0.9, 0.0, 0.5))
        },
        "harp_pluck" => Patch {
            filter: filt(FilterMode::Lp, 2600.0, 0.05, 1.6, 0.6),
            fenv: a(0.001, 0.3, 0.1, 0.4),
            velocity: 0.6,
            analog: 1.0,
            ..base(
                vec![o(Wave::Tri), o(Wave::Saw).level(0.18), o(Wave::Sine).octave(1).level(0.1)],
                a(0.001, 1.3, 0.0, 0.8),
            )
        },
        "mallet" => Patch {
            analog: 0.5,
            velocity: 0.6,
            ..base(
                vec![o(Wave::Sine), o(Wave::Sine).octave(2).level(0.12), o(Wave::Tri).octave(1).level(0.05)],
                a(0.002, 0.9, 0.0, 0.6),
            )
        },
        _ => return None,
    };
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SRF;
    use crate::synth::voice::{NoteEv, render};

    /// three notes in the register its first role plays in, a second each
    fn phrase(name: &str) -> Vec<NoteEv> {
        let low = for_role("bass").contains(&name);
        let root = if low { 36.0 } else { 60.0 };
        let s = SRF as usize;
        [0.0, 4.0, 7.0]
            .iter()
            .enumerate()
            .map(|(i, step)| NoteEv { start: i * s, gate: s * 9 / 10, pitch: root + step, vel: 0.8 })
            .collect()
    }

    #[test]
    fn every_preset_sounds_and_stays_finite() {
        for name in names() {
            let x = render(&get(name).unwrap(), &phrase(name), 5 * SRF as usize, SRF, 120.0, 11);
            let peak = x.l.iter().chain(&x.r).fold(0f32, |m, v| m.max(v.abs()));
            assert!(x.l.iter().chain(&x.r).all(|v| v.is_finite()), "{name}");
            assert!((0.02..8.0).contains(&peak), "{name}: peak {peak}");
        }
    }

    #[test]
    fn no_new_preset_hides_an_instrument_of_its_name() {
        // presets come before pack instruments, so a preset named as an instrument hides it: the first twenty were
        // here before, and the docs say what to write for the recording instead
        let instruments = claudius_packs::registry_names();
        for name in names().into_iter().skip(20) {
            assert!(!instruments.iter().any(|i| i == name), "{name} is an instrument too");
        }
    }

    #[test]
    fn every_role_has_presets_to_draw_from() {
        for role in ["bass", "chords", "keys", "pad", "lead", "arp"] {
            assert!(for_role(role).len() >= 6, "{role}: {:?}", for_role(role));
        }
    }
}
