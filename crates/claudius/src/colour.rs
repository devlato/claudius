//! A mix of its own for every song: its genre's mix, varied. The reverb's length, darkness, pre-delay, width and
//! low cut, and the delay's time (a sixteenth to a dotted quarter), feedback and bounce; each part's sends; and a
//! colour on some of the parts, by role: a chorus, phaser, flanger, slow autopan or more width on a pad; a chorus,
//! phaser, tremolo, autopan or tape on chords and keys; tape, a tube, a chorus, phaser or flanger on a lead; an autopan,
//! a bitcrusher, phaser or chorus on an arp; a tube, tape or a little fold on a bass. Some colours deepen in the hooks
//! and drops. Some songs high-pass their music in the breaks, and the master's saturation may take another flavour.

use crate::dsp::env::{BySection, Param};
use crate::dsp::filter::FilterMode;
use crate::fx::{Effect, SatKind};
use crate::model::{MixPlan, Part, Role};
use crate::rng::Rng;

/// `x` moved up or down by a factor of `spread` at most
fn by(x: f32, spread: f32, rng: &mut Rng) -> f32 {
    x * (rng.range(-1.0, 1.0) as f32 * spread.ln()).exp()
}

/// a value that holds `rest`, and `peak` in the hooks and drops
fn peaks(rest: f32, peak: f32) -> Param {
    Param::Sections(BySection { rest, sections: [("hook".into(), vec![peak]), ("drop".into(), vec![peak])].into() })
}

/// a colour for a part of `role`, with settings of its own
fn colour(role: Role, rng: &mut Rng) -> Effect {
    let choices: &[&str] = match role {
        Role::Pad => &["chorus", "phaser", "flanger", "auto_pan", "width"],
        Role::Chords | Role::Keys => &["chorus", "phaser", "tremolo", "auto_pan", "tape"],
        Role::Lead => &["tape", "tube", "chorus", "phaser", "flanger"],
        Role::Arp => &["auto_pan", "bitcrush", "phaser", "chorus"],
        Role::Bass => &["tube", "tape", "fold"],
    };
    let mut r = |lo: f64, hi: f64| rng.range(lo, hi) as f32;
    match choices[(r(0.0, choices.len() as f64) as usize).min(choices.len() - 1)] {
        "chorus" => Effect::Chorus {
            rate_hz: r(0.2, 0.9),
            depth_ms: r(1.5, 4.0),
            delay_ms: r(8.0, 16.0),
            mix: r(0.25, 0.55).into(),
        },
        "phaser" => {
            let rest = r(0.2, 0.4);
            Effect::Phaser {
                rate_hz: r(0.1, 0.6),
                stages: [4, 6, 8][r(0.0, 2.99) as usize],
                depth: r(0.5, 0.9),
                feedback: r(0.2, 0.55),
                mix: peaks(rest, rest + r(0.1, 0.25)),
            }
        }
        "flanger" => Effect::Flanger {
            rate_hz: r(0.05, 0.3),
            depth_ms: r(1.0, 3.0),
            delay_ms: r(1.0, 4.0),
            feedback: r(0.2, 0.55),
            mix: r(0.2, 0.45).into(),
        },
        "auto_pan" => Effect::AutoPan { beats: [1.0, 2.0, 4.0][r(0.0, 2.99) as usize], depth: r(0.3, 0.7) },
        "tremolo" => Effect::Tremolo { beats: [0.25, 0.5, 1.0][r(0.0, 2.99) as usize], depth: r(0.15, 0.4) },
        "width" => Effect::Width { width: r(1.2, 1.5).into(), mono_below_hz: 150.0 },
        "tape" => Effect::Tape {
            wow: r(0.1, 0.45),
            flutter: r(0.05, 0.25),
            drive_db: r(1.0, 4.0),
            lowpass_hz: r(9000.0, 15000.0),
        },
        "tube" => Effect::Saturate { kind: SatKind::Tube, drive_db: r(3.0, 8.0).into(), mix: r(0.3, 0.6).into() },
        "fold" => Effect::Saturate { kind: SatKind::Fold, drive_db: r(2.0, 5.0).into(), mix: r(0.1, 0.25).into() },
        _ => {
            let rest = r(0.15, 0.3);
            Effect::Bitcrush {
                bits: r(8.0, 12.0).round().into(),
                downsample: r(1.0, 3.0).round(),
                mix: peaks(rest, rest + 0.15),
            }
        }
    }
}

/// a part's sends moved, and now and then a colour of its role on it, from `sends` and `colours`
fn sends_and_colour(p: &mut Part, sends: &mut Rng, colours: &mut Rng) {
    p.mix.reverb = by(p.mix.reverb, 1.5, sends).clamp(0.0, 0.8);
    p.mix.delay = by(p.mix.delay, 1.7, sends).clamp(0.0, 0.6);
    if colours.chance(0.4) {
        let e = colour(p.role, colours);
        // one of each kind: a part with a phaser of its genre's keeps that one
        if !p.mix.fx.iter().any(|f| f.name() == e.name()) {
            p.mix.fx.push(e);
        }
    }
}

/// one part's sends and colour as `vary` gives a song's, from draws of its own (`rng`): a part a song adds to those its
/// genre writes, so that theirs stay as they were
pub fn vary_part(p: &mut Part, rng: &Rng) {
    sends_and_colour(p, &mut rng.fork("sends"), &mut rng.fork("colours"));
}

/// `mix` and the parts' own mixes varied for one song (the module's doc says how). Each change draws from a fork of
/// `rng` of its own, so that changing one leaves the others' draws as they were.
pub fn vary(mix: &mut MixPlan, parts: &mut [Part], rng: &Rng) {
    let mut r = rng.fork("reverb");
    if let Effect::Reverb { decay, damping, predelay_ms, width, lowcut, .. } = &mut mix.reverb {
        *decay = by(*decay, 1.5, &mut r).clamp(0.6, 8.0);
        *damping = (*damping + r.range(-0.15, 0.15) as f32).clamp(0.05, 0.95);
        *predelay_ms = (*predelay_ms + r.range(-10.0, 25.0) as f32).clamp(0.0, 80.0);
        *width = by(*width, 1.2, &mut r).clamp(0.6, 1.5);
        *lowcut = by(*lowcut, 1.4, &mut r).clamp(80.0, 800.0);
    }
    let mut r = rng.fork("delay");
    if let Effect::Delay { beats, feedback, pingpong, .. } = &mut mix.delay {
        if r.chance(0.5) {
            *beats = *r.pick(&[0.25, 0.375, 0.5, 0.75, 1.0, 1.5]);
        }
        if let Param::Const(f) = feedback {
            *f = by(*f, 1.3, &mut r).clamp(0.1, 0.7);
        }
        if r.chance(0.3) {
            *pingpong = !*pingpong;
        }
    }
    let (mut sends, mut colours) = (rng.fork("sends"), rng.fork("colours"));
    for p in parts.iter_mut() {
        sends_and_colour(p, &mut sends, &mut colours);
    }
    // the music high-passed in the breaks, as if the room emptied
    let mut r = rng.fork("room");
    if r.chance(0.3) {
        let cutoff = r.range(250.0, 500.0) as f32;
        mix.buses.entry("music".into()).or_default().push(Effect::Filter {
            mode: FilterMode::Hp,
            cutoff: Param::Sections(BySection { rest: 20.0, sections: [("break".into(), vec![cutoff])].into() }),
            res: 0.1.into(),
            drive: 1.0,
            mix: 1.0.into(),
        });
    }
    // the master's saturation in another flavour
    let mut r = rng.fork("master");
    for e in mix.master.fx.iter_mut() {
        if let Effect::Saturate { kind, .. } = e
            && r.chance(0.5)
        {
            *kind = *r.pick(&[SatKind::Tape, SatKind::Tube, SatKind::Tanh]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_of_a_genre_mix_differently() {
        let g = crate::genres::get("house").unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..16 {
            let bp = crate::compose::generate(&g, seed, &Default::default(), None);
            let Effect::Reverb { decay, .. } = bp.mix.reverb else { panic!("a reverb") };
            let Effect::Delay { beats, .. } = bp.mix.delay else { panic!("a delay") };
            let colours: Vec<&str> = bp.parts.iter().flat_map(|p| p.mix.fx.iter().map(|e| e.name())).collect();
            assert!((0.6..=8.0).contains(&decay), "{decay}");
            seen.insert(format!("{decay:.1} {beats} {colours:?}"));
            for p in &bp.parts {
                let mut names: Vec<&str> = p.mix.fx.iter().map(|e| e.name()).collect();
                let all = names.len();
                names.sort();
                names.dedup();
                assert_eq!(names.len(), all, "one of each effect on {}: {:?}", p.name, p.mix.fx);
            }
        }
        assert_eq!(seen.len(), 16, "{seen:?}");
    }
}
