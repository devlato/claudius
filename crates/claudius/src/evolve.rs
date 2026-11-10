//! Evolution: mutate blueprints the way a producer revises a track, keep what the critic likes.
//!
//! Mutation operators work on every level of the blueprint: harmony (theory-aware substitutions, new progressions),
//! drums (hit edits, regenerated patterns, sounds swapped between synthesis and recordings, a step's trig), parts (a
//! new writing seed, style, preset, patch tweaks, register, density, a stacked sound's layers), the instruments (a part
//! or a layer more, as a more layered song of the genre plays one, or one that only colours the song fewer), mix
//! (levels, sends, sidechain, pan, space), effects (a setting moved, an insert effect added or taken off, an
//! equalizer move on the band the critic finds too much or too little of), form (layers per section, lengths, fills /
//! gaps / builds, variants, build-ups, and sections played again, left out or set apart, as `form::change` revises a
//! form), samples (another loop, a break or phrase re-chopped) and the global feel (tempo, key, swing). The critic's
//! issues bias which operators are tried, and a form's revision aims at its issue with the form; crossover recombines
//! parents, and every child records what changed.

use std::collections::BTreeMap;

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;

use crate::compose::{self, Library};
use crate::fx::Effect;
use crate::genres::GenreProfile;
use crate::judge::{self, EarsReport, Issue, Verdict};
use crate::model::*;
use crate::render::{self, Rendered};
use crate::rng::{Rng, hash_str};
use crate::synth::drums::DrumParams;
use crate::theory::progression;
use crate::theory::rhythm::{self, Hit};
use crate::theory::scale::Key;

fn drum_lane_spec<'a>(g: &'a GenreProfile, name: &str) -> Option<&'a crate::genres::LaneSpec> {
    g.drums.iter().find(|d| d.name == name)
}

fn part_spec<'a>(g: &'a GenreProfile, name: &str) -> Option<&'a crate::genres::PartSpec> {
    g.parts.iter().find(|p| p.name == name)
}

fn track_mix<'a>(bp: &'a mut Blueprint, name: &str) -> Option<&'a mut TrackMix> {
    if let Some(d) = bp.drums.iter_mut().find(|d| d.name == name) {
        return Some(&mut d.mix);
    }
    if let Some(p) = bp.parts.iter_mut().find(|p| p.name == name) {
        return Some(&mut p.mix);
    }
    bp.loops.iter_mut().find(|l| l.name == name).map(|l| &mut l.mix)
}

/// whether `name` is a track of `bp` with a mix of its own (`track_mix`)
fn has_mix(bp: &Blueprint, name: &str) -> bool {
    bp.drums.iter().any(|d| d.name == name)
        || bp.parts.iter().any(|p| p.name == name)
        || bp.loops.iter().any(|l| l.name == name)
}

/// the kick and the bass parts: the low end, which stays centred and dry
fn low_end(bp: &Blueprint, name: &str) -> bool {
    name == "kick" || bp.parts.iter().any(|p| p.name == name && p.role == Role::Bass)
}

const OPS: &[&str] = &[
    "harmony_sub",
    "harmony_new",
    "drum_edit",
    "drum_regen",
    "drum_sound",
    "drum_kit",
    "drum_trig",
    "part_seed",
    "part_style",
    "part_preset",
    "patch_tweak",
    "part_register",
    "part_density",
    "rack_layer",
    "track_add",
    "track_remove",
    "mix_level",
    "mix_send",
    "mix_duck",
    "mix_pan",
    "space",
    "fx_tweak",
    "fx_add",
    "fx_remove",
    "fx_eq",
    "form_layer",
    "form_bars",
    "form_flags",
    "form_variant",
    "form_change",
    "form_build",
    "tempo",
    "key",
    "swing",
    "loop_swap",
    "loop_chop",
];

/// operator weights, raised for the operators that address the critic's issues
fn weights(issues: &[Issue]) -> Vec<f64> {
    let mut w: BTreeMap<&str, f64> = OPS.iter().map(|o| (*o, 1.0)).collect();
    for i in issues.iter().take(4) {
        let t = i.target.as_str();
        let boost = 1.0 + 4.0 * i.severity;
        let hit: &[&str] = if t.starts_with("band:") {
            &["mix_level", "drum_sound", "drum_kit", "patch_tweak", "space", "mix_pan", "fx_eq", "fx_tweak"]
        } else if t.starts_with("width") {
            &["mix_level", "drum_sound", "drum_kit", "patch_tweak", "space", "mix_pan", "fx_add", "fx_remove"]
        } else if t == "production" {
            &["mix_level", "drum_sound", "drum_kit", "patch_tweak", "space", "mix_pan", "fx_tweak", "fx_remove"]
        } else if t == "form" {
            &["form_layer", "form_flags", "form_variant", "form_bars", "form_build", "track_add"]
        } else if t == "form:short" || t == "form:long" {
            &["form_change", "form_bars"]
        } else if t.starts_with("form:") {
            &["form_change"]
        } else if t.starts_with("part:") {
            &["part_seed", "part_style", "part_density", "part_register"]
        } else if t.starts_with("level:") {
            &["mix_level"]
        } else if t.starts_with("sound:") {
            &["part_preset", "patch_tweak", "rack_layer", "fx_tweak"]
        } else if t == "genre" {
            &[
                "part_preset",
                "part_style",
                "drum_regen",
                "drum_sound",
                "drum_kit",
                "tempo",
                "harmony_new",
                "track_add",
                "track_remove",
                "drum_trig",
            ]
        } else if t == "harmony" {
            &["harmony_sub", "harmony_new"]
        } else {
            &[]
        };
        for h in hit {
            *w.get_mut(h).unwrap() *= boost;
        }
    }
    OPS.iter().map(|o| w[o]).collect()
}

/// the track a level issue names, and which way to turn it
fn level_track(target: &str) -> Option<(String, f32)> {
    let t = target.strip_prefix("level:")?;
    let (name, sign) = (&t[..t.len().saturating_sub(1)], if t.ends_with('+') { -1.0 } else { 1.0 });
    (!name.is_empty()).then(|| (name.to_string(), sign))
}

/// the band a band issue names, and which way to turn it (-1: down)
fn band_of(target: &str) -> Option<(&str, f32)> {
    target.strip_prefix("band:").map(|b| (&b[..b.len() - 1], if b.ends_with('+') { -1.0 } else { 1.0 }))
}

/// the tracks that live in a band
fn band_names(band: &str) -> Option<&'static [&'static str]> {
    Some(match band {
        "sub" | "low" => &["kick", "bass"],
        "lowmid" => &["pad", "chords", "keys"],
        "mid" => &["chords", "lead", "keys", "arp"],
        "presence" => &["clap", "snare", "hat", "perc", "shaker"],
        "air" => &["hat", "ohat", "shaker", "ride"],
        _ => return None,
    })
}

/// the track a band issue points at (what to turn up or down)
fn band_track(target: &str, rng: &mut Rng) -> Option<(String, f32)> {
    let (band, sign) = band_of(target)?;
    Some((rng.pick(band_names(band)?).to_string(), sign))
}

/// a track's mix, to read
fn mix_of<'a>(bp: &'a Blueprint, name: &str) -> Option<&'a TrackMix> {
    let drum = bp.drums.iter().find(|d| d.name == name).map(|d| &d.mix);
    let part = || bp.parts.iter().find(|p| p.name == name).map(|p| &p.mix);
    drum.or_else(part).or_else(|| bp.loops.iter().find(|l| l.name == name).map(|l| &l.mix))
}

/// what a track is to the effects it may take: the kick or the bass (the low end), the other drums, harmony (chords,
/// keys, a pad) or a line (a lead, an arpeggio); none for a loop or a pattern
fn fx_role(bp: &Blueprint, name: &str) -> Option<&'static str> {
    if let Some(d) = bp.drums.iter().find(|d| d.name == name) {
        return Some(if d.role() == "kick" { "low" } else { "drums" });
    }
    Some(match bp.parts.iter().find(|p| p.name == name)?.role {
        Role::Bass => "low",
        Role::Chords | Role::Keys | Role::Pad => "harmony",
        Role::Lead | Role::Arp => "line",
    })
}

/// the effects `fx_add` adds, and `fx_remove` takes off again
const ADDED_FX: &[&str] =
    &["chorus", "phaser", "tremolo", "auto_pan", "delay", "saturate", "tape", "transient", "width"];

/// An insert effect of those a track of `role` takes, set as the genres set theirs: drive on the low end (nothing that
/// widens it), drive or a sharper attack on the other drums, a chorus, a phaser, a tremolo or tape on harmony (an
/// auto-pan or a widener too, when the mix is too narrow), and a delay, a chorus or drive on a line.
fn new_fx(role: &str, wider: bool, rng: &mut Rng) -> Effect {
    let tape = |rng: &mut Rng| Effect::Saturate {
        kind: if rng.chance(0.5) { crate::fx::SatKind::Tape } else { crate::fx::SatKind::Tube },
        drive_db: (rng.range(2.0, 6.0) as f32).into(),
        mix: (rng.range(0.3, 0.5) as f32).into(),
    };
    let chorus = |rng: &mut Rng| Effect::Chorus {
        rate_hz: rng.range(0.5, 0.8) as f32,
        depth_ms: rng.range(2.5, 4.0) as f32,
        delay_ms: rng.range(10.0, 14.0) as f32,
        mix: (rng.range(0.25, 0.45) as f32).into(),
    };
    match role {
        "low" => tape(rng),
        "drums" if rng.chance(0.5) => {
            Effect::Transient { attack_db: rng.range(2.0, 4.0) as f32, sustain_db: rng.range(-2.0, 0.0) as f32 }
        }
        "drums" => tape(rng),
        "harmony" if wider => match rng.below(3) {
            0 => chorus(rng),
            1 => Effect::AutoPan { beats: *rng.pick(&[1.0, 2.0]), depth: rng.range(0.2, 0.35) as f32 },
            _ => Effect::Width { width: (rng.range(1.2, 1.4) as f32).into(), mono_below_hz: 150.0 },
        },
        "harmony" => match rng.below(4) {
            0 => chorus(rng),
            1 => Effect::Phaser {
                rate_hz: rng.range(0.15, 0.3) as f32,
                stages: 4,
                depth: rng.range(0.5, 0.7) as f32,
                feedback: rng.range(0.2, 0.35) as f32,
                mix: (rng.range(0.25, 0.4) as f32).into(),
            },
            2 => Effect::Tremolo { beats: *rng.pick(&[0.5, 1.0]), depth: rng.range(0.1, 0.25) as f32 },
            _ => Effect::Tape {
                wow: rng.range(0.1, 0.35) as f32,
                flutter: rng.range(0.05, 0.25) as f32,
                drive_db: rng.range(1.0, 3.0) as f32,
                lowpass_hz: rng.range(10000.0, 16000.0) as f32,
            },
        },
        _ => match rng.below(3) {
            0 => Effect::Delay {
                beats: *rng.pick(&[0.375, 0.5, 0.75]),
                feedback: (rng.range(0.25, 0.4) as f32).into(),
                damping: 0.3,
                pingpong: true,
                mix: (rng.range(0.12, 0.22) as f32).into(),
            },
            1 => chorus(rng),
            _ => tape(rng),
        },
    }
}

/// One setting of an effect moved a little, as an engineer rides a knob, within what it takes: what it is now, or None
/// for an effect with none to move (or one whose setting moves by section or along a curve).
fn tweak(e: &mut Effect, rng: &mut Rng) -> Option<String> {
    fn fixed(p: &mut crate::dsp::env::Param) -> Option<&mut f32> {
        if let crate::dsp::env::Param::Const(v) = p { Some(v) } else { None }
    }
    let scale = |v: f32, lo: f32, hi: f32, rng: &mut Rng| (v * rng.range(0.75, 1.33) as f32).clamp(lo, hi);
    let nudge = |v: f32, by: f32, lo: f32, hi: f32, rng: &mut Rng| (v + (2.0 * rng.f32() - 1.0) * by).clamp(lo, hi);
    let name = e.name();
    let said = match e {
        Effect::Filter { cutoff, .. } if rng.chance(0.6) => {
            let c = fixed(cutoff)?;
            *c = scale(*c, 40.0, 18000.0, rng);
            format!("cutoff {:.0} Hz", *c)
        }
        Effect::Filter { res, .. } => {
            let r = fixed(res)?;
            *r = nudge(*r, 0.1, 0.0, 0.9, rng);
            format!("resonance {:.2}", *r)
        }
        Effect::Saturate { mix, .. } | Effect::Bitcrush { mix, .. } if rng.chance(0.5) => {
            let m = fixed(mix)?;
            *m = nudge(*m, 0.1, 0.05, 1.0, rng);
            format!("mix {:.2}", *m)
        }
        Effect::Saturate { drive_db, .. } => {
            let d = fixed(drive_db)?;
            *d = nudge(*d, 2.0, 0.0, 18.0, rng);
            format!("drive {:.1} dB", *d)
        }
        Effect::Chorus { rate_hz, mix, .. }
        | Effect::Flanger { rate_hz, mix, .. }
        | Effect::Phaser { rate_hz, mix, .. } => {
            if rng.chance(0.5) {
                *rate_hz = scale(*rate_hz, 0.05, 5.0, rng);
                format!("rate {:.2} Hz", *rate_hz)
            } else {
                let m = fixed(mix)?;
                *m = nudge(*m, 0.1, 0.05, 1.0, rng);
                format!("mix {:.2}", *m)
            }
        }
        Effect::Delay { feedback, mix, .. } => {
            if rng.chance(0.5) {
                let f = fixed(feedback)?;
                *f = nudge(*f, 0.1, 0.0, 0.8, rng);
                format!("feedback {:.2}", *f)
            } else {
                let m = fixed(mix)?;
                *m = nudge(*m, 0.05, 0.02, 0.5, rng);
                format!("mix {:.2}", *m)
            }
        }
        Effect::Reverb { decay, .. } => {
            *decay = scale(*decay, 0.3, 8.0, rng);
            format!("decay {:.1} s", *decay)
        }
        Effect::Tremolo { depth, .. } | Effect::AutoPan { depth, .. } => {
            *depth = nudge(*depth, 0.1, 0.0, 1.0, rng);
            format!("depth {:.2}", *depth)
        }
        Effect::Compressor { threshold_db, .. } => {
            *threshold_db = nudge(*threshold_db, 3.0, -40.0, 0.0, rng);
            format!("threshold {:.1} dB", *threshold_db)
        }
        Effect::Eq { bands } => {
            let i = rng.below(bands.len());
            let b = bands.get_mut(i)?;
            b.gain_db = nudge(b.gain_db, 1.5, -12.0, 12.0, rng);
            format!("{:.0} Hz {:+.1} dB", b.freq, b.gain_db)
        }
        Effect::Width { width, .. } => {
            let w = fixed(width)?;
            *w = nudge(*w, 0.15, 0.0, 2.0, rng);
            format!("width {:.2}", *w)
        }
        Effect::Tape { drive_db, .. } => {
            *drive_db = nudge(*drive_db, 1.0, 0.0, 9.0, rng);
            format!("drive {:.1} dB", *drive_db)
        }
        Effect::Gain { db } => {
            let d = fixed(db)?;
            *d = nudge(*d, 1.5, -24.0, 12.0, rng);
            format!("{:+.1} dB", *d)
        }
        _ => return None,
    };
    Some(format!("{name} {said}"))
}

pub fn mutate(
    bp: &Blueprint,
    g: &GenreProfile,
    lib: Option<&dyn Library>,
    issues: &[Issue],
    rng: &mut Rng,
) -> (Blueprint, String) {
    for _ in 0..12 {
        let mut b = bp.clone();
        let w = weights(issues);
        let op = OPS[rng.weighted(&w)];
        if let Some(what) = apply_op(&mut b, op, g, lib, issues, rng)
            && b != *bp
        {
            b.lineage.push(what.clone());
            return (b, what);
        }
    }
    let mut b = bp.clone();
    for p in b.parts.iter_mut() {
        p.seed = rng.next_u64();
    }
    b.lineage.push("rewrote every part".into());
    (b, "rewrote every part".into())
}

fn apply_op(
    b: &mut Blueprint,
    op: &str,
    g: &GenreProfile,
    lib: Option<&dyn Library>,
    issues: &[Issue],
    rng: &mut Rng,
) -> Option<String> {
    let layer_names = b.layer_names();
    match op {
        "harmony_sub" => {
            let name = if rng.chance(0.6) { "A" } else { "B" }.to_string();
            let prog = b.harmony.progressions.get(&name)?.clone();
            let (np, what) = progression::substitute(&prog, &b.key, rng);
            b.harmony.progressions.insert(name.clone(), np);
            Some(format!("harmony {name}: {what}"))
        }
        "harmony_new" => {
            let name = if rng.chance(0.5) { "A" } else { "B" }.to_string();
            // only styles of the key's family: major progressions read as other chords in a minor key
            let fitting: Vec<(String, f64)> = g
                .harmony
                .iter()
                .filter(|(s, _)| progression::style(s).is_some_and(|st| st.mode.is_minor() == b.key.mode.is_minor()))
                .cloned()
                .collect();
            if fitting.is_empty() {
                return None;
            }
            let style_name = rng.pick_weighted(&fitting).clone();
            let st = progression::style(&style_name)?;
            let p = progression::parse(rng.pick(st.progs));
            let old = b.harmony.progressions.insert(name.clone(), p.clone()).unwrap_or_default();
            Some(format!("harmony {name}: {} -> {} ({style_name})", old.join(" "), p.join(" ")))
        }
        "drum_edit" => {
            let li = rng.below(b.drums.len().max(1));
            let lane = b.drums.get_mut(li)?;
            let var = lane.patterns.keys().cloned().collect::<Vec<_>>();
            let vname = rng.pick(&var).clone();
            let bars = lane.pattern_bars as f64 * 4.0;
            let hits = lane.patterns.get_mut(&vname)?;
            let k = rng.below(4);
            let what = match k {
                0 => {
                    let step = rng.below((bars * 4.0) as usize) as f64 * 0.25;
                    if hits.iter().any(|h| (h.t - step).abs() < 1e-6) {
                        return None;
                    }
                    hits.push(Hit::new(step, 0.5 + 0.4 * rng.f32()));
                    hits.sort_by(|a, c| a.t.partial_cmp(&c.t).unwrap());
                    format!("added a {} hit at beat {:.2}", lane.name, step)
                }
                1 if hits.len() > 2 => {
                    let i = rng.below(hits.len());
                    let h = hits.remove(i);
                    format!("removed the {} hit at beat {:.2}", lane.name, h.t)
                }
                2 if !hits.is_empty() => {
                    let i = rng.below(hits.len());
                    let d = if rng.chance(0.5) { 0.25 } else { -0.25 };
                    hits[i].t = (hits[i].t + d).rem_euclid(bars);
                    hits.sort_by(|a, c| a.t.partial_cmp(&c.t).unwrap());
                    format!("nudged a {} hit by {d:+} beats", lane.name)
                }
                _ if !hits.is_empty() => {
                    let i = rng.below(hits.len());
                    hits[i].v = (hits[i].v + (rng.f32() - 0.5) * 0.4).clamp(0.2, 1.0);
                    format!("re-accented a {} hit", lane.name)
                }
                _ => return None,
            };
            Some(format!("drums ({vname}): {what}"))
        }
        "drum_regen" => {
            let li = rng.below(b.drums.len().max(1));
            let name = b.drums.get(li)?.name.clone();
            let spec = drum_lane_spec(g, &name)?;
            let t = spec.template.as_ref()?;
            let lane = &mut b.drums[li];
            lane.patterns.insert("main".into(), rhythm::generate(t, lane.pattern_bars, rng));
            Some(format!("drums: new {name} pattern"))
        }
        "drum_sound" => {
            let li = rng.below(b.drums.len().max(1));
            let name = b.drums.get(li)?.name.clone();
            let spec = drum_lane_spec(g, &name)?;
            let lane = &mut b.drums[li];
            if let (Some(l), true) = (lib, rng.chance(0.5)) {
                let hints = if spec.samples.is_empty() { compose::default_hints(&name) } else { spec.samples.clone() };
                let s = l.one_shot(&name, &hints, g, rng)?;
                let what = format!("{name}: {} -> {}", lane.sound.describe(), s.describe());
                lane.sound = s;
                return Some(what);
            }
            let mut params = match &lane.sound {
                Sound::Drum { params, .. } => *params,
                _ => DrumParams::default_for(spec.voice),
            };
            let space = DrumParams::space(spec.voice);
            let i = rng.below(9);
            let crate::synth::drums::Range(lo, hi, _) = space[i];
            let v = params.get(i) + (hi - lo) * (rng.f32() - 0.5) * 0.3;
            params.set(i, v.clamp(lo.min(hi), hi.max(lo)));
            lane.sound = Sound::Drum { voice: spec.voice, params };
            Some(format!("{name}: synth voice parameter {i} -> {:.3}", params.get(i)))
        }
        "drum_kit" => {
            // every lane onto another drum machine from the sample packs, or back to the synthesized voices
            let installed: Vec<String> = claudius_packs::installed().iter().map(|p| p.name.clone()).collect();
            let mut kits: Vec<String> =
                g.kits.iter().map(|(k, _)| k.clone()).filter(|k| installed.contains(k)).collect();
            for k in ["tr909", "tr808", "tr707", "muldjord", "uzu"] {
                if installed.iter().any(|x| x == k) && !kits.iter().any(|x| x == k) {
                    kits.push(k.to_string());
                }
            }
            let current = b.drums.iter().find_map(|d| {
                if let Sound::Instrument { name, .. } = &d.sound {
                    name.split(':').next().map(String::from)
                } else {
                    None
                }
            });
            if current.is_some() && rng.chance(0.25) {
                for d in b.drums.iter_mut().filter(|d| matches!(d.sound, Sound::Instrument { .. })) {
                    let spec = drum_lane_spec(g, &d.name)?;
                    d.sound = Sound::Drum { voice: spec.voice, params: DrumParams::default_for(spec.voice) };
                }
                return Some("drums: back to the synthesized voices".into());
            }
            let choices: Vec<String> = kits.into_iter().filter(|k| Some(k.as_str()) != current.as_deref()).collect();
            if choices.is_empty() {
                return None;
            }
            let kit = rng.pick(&choices).clone();
            let mut n = 0;
            for d in b.drums.iter_mut() {
                if let Some(s) = compose::kit_sound(&d.name, &kit) {
                    d.sound = s;
                    n += 1;
                }
            }
            (n > 0).then(|| format!("drums: played on the {kit} kit"))
        }
        "part_seed" | "part_style" | "part_preset" | "patch_tweak" | "part_register" | "part_density" => {
            let targeted: Vec<usize> = issues
                .iter()
                .filter_map(|i| i.target.strip_prefix("part:").or_else(|| i.target.strip_prefix("sound:")))
                .filter_map(|n| b.parts.iter().position(|p| p.name == n))
                .collect();
            let pi =
                if !targeted.is_empty() && rng.chance(0.7) { targeted[0] } else { rng.below(b.parts.len().max(1)) };
            let part = b.parts.get_mut(pi)?;
            let spec = part_spec(g, &part.name);
            match op {
                "part_seed" => {
                    part.seed = rng.next_u64();
                    Some(format!("{}: rewrote the part", part.name))
                }
                "part_style" => {
                    let s = spec?.styles.clone();
                    let ns = rng.pick(&s).clone();
                    if ns == part.style {
                        return None;
                    }
                    let what = format!("{}: style {} -> {ns}", part.name, part.style);
                    part.style = ns;
                    Some(what)
                }
                "part_preset" => {
                    let others = b.parts.iter().enumerate().filter(|(i, _)| *i != pi);
                    let used = others.flat_map(|(_, p)| crate::palette::identities(&p.sound)).collect();
                    let part = b.parts.get_mut(pi)?;
                    let sound = crate::compose::draw_sound(spec?, g, &crate::palette::instruments(), &used, rng);
                    if sound == part.sound {
                        return None;
                    }
                    let what = format!("{}: sound {} -> {}", part.name, part.sound.describe(), sound.describe());
                    part.sound = sound;
                    Some(what)
                }
                "patch_tweak" => {
                    // of a rack, the sound it is heard by, tweaked alike in each layer that plays it
                    let voice = match &part.sound {
                        Sound::Rack { layers } => crate::swap::voice(layers),
                        s => Some(s),
                    };
                    let Some(Sound::Synth { patch: was, preset }) = voice.cloned() else { return None };
                    let mut patch = was.clone();
                    let what = match rng.below(5) {
                        0 => {
                            let f = patch.filter.as_mut()?;
                            f.cutoff = (f.cutoff * rng.range(0.7, 1.4) as f32).clamp(120.0, 16000.0);
                            format!("cutoff {:.0} Hz", f.cutoff)
                        }
                        1 => {
                            let f = patch.filter.as_mut()?;
                            f.res = (f.res + (rng.f32() - 0.5) * 0.3).clamp(0.0, 0.9);
                            format!("resonance {:.2}", f.res)
                        }
                        2 => {
                            patch.aenv.r = (patch.aenv.r * rng.range(0.6, 1.6)).clamp(0.02, 3.0);
                            format!("release {:.2} s", patch.aenv.r)
                        }
                        3 => {
                            patch.drive = (patch.drive + (rng.f32() - 0.4) * 4.0).clamp(0.0, 12.0);
                            format!("drive {:.1} dB", patch.drive)
                        }
                        _ => {
                            let o = patch.osc.first_mut()?;
                            o.detune = (o.detune * rng.range(0.6, 1.5) as f32).clamp(0.0, 45.0);
                            format!("detune {:.0} cents", o.detune)
                        }
                    };
                    for s in part.sound.each_mut() {
                        if let Sound::Synth { preset: p, patch: q } = s
                            && *p == preset
                            && *q == was
                        {
                            *q = patch.clone();
                        }
                    }
                    Some(format!("{} ({preset}): {what}", part.name))
                }
                "part_register" => {
                    let d = *rng.pick(&[-12, -5, -3, 3, 5, 12]);
                    let (lo, hi) = (part.register.0 + d, part.register.1 + d);
                    if lo < 24 || hi > 100 {
                        return None;
                    }
                    part.register = (lo, hi);
                    Some(format!("{}: register moved {d:+} semitones", part.name))
                }
                _ => {
                    if part.role != Role::Lead {
                        return None;
                    }
                    part.density = (part.density + (rng.f32() - 0.5) * 0.3).clamp(0.1, 0.9);
                    Some(format!("{}: density {:.2}", part.name, part.density))
                }
            }
        }
        "mix_level" => {
            let level = issues.iter().find_map(|i| level_track(&i.target)).filter(|(n, _)| has_mix(b, n));
            let aimed = match level {
                Some(l) => Some(l),
                None => issues.iter().find_map(|i| band_track(&i.target, rng)).filter(|(n, _)| layer_names.contains(n)),
            };
            let (name, sign) = match aimed {
                Some(a) if rng.chance(0.75) => a,
                _ => (rng.pick(&layer_names).clone(), if rng.chance(0.5) { 1.0 } else { -1.0 }),
            };
            let m = track_mix(b, &name)?;
            let d = sign * rng.range(1.0, 3.0) as f32;
            m.level_db = (m.level_db + d).clamp(-12.0, 9.0);
            Some(format!("mix: {name} {d:+.1} dB (now {:+.1})", m.level_db))
        }
        "mix_send" => {
            let name = rng.pick(&layer_names).clone();
            if low_end(b, &name) {
                return None;
            }
            let m = track_mix(b, &name)?;
            if rng.chance(0.5) {
                m.reverb = (m.reverb + (rng.f32() - 0.5) * 0.2).clamp(0.0, 0.6);
                Some(format!("mix: {name} reverb send {:.2}", m.reverb))
            } else {
                m.delay = (m.delay + (rng.f32() - 0.5) * 0.2).clamp(0.0, 0.5);
                Some(format!("mix: {name} delay send {:.2}", m.delay))
            }
        }
        "mix_duck" => {
            let names: Vec<String> = b.parts.iter().map(|p| p.name.clone()).collect();
            let name = rng.pick(&names).clone();
            let m = track_mix(b, &name)?;
            m.duck = (m.duck + (rng.f32() - 0.5) * 0.3).clamp(0.0, 0.9);
            Some(format!("mix: {name} sidechain depth {:.2}", m.duck))
        }
        "mix_pan" => {
            let name = rng.pick(&layer_names).clone();
            if low_end(b, &name) || matches!(name.as_str(), "snare" | "clap") {
                return None;
            }
            let m = track_mix(b, &name)?;
            m.pan = (m.pan + (rng.f32() - 0.5) * 0.5).clamp(-0.7, 0.7);
            Some(format!("mix: {name} pan {:+.2}", m.pan))
        }
        "space" => {
            if rng.chance(0.5) {
                if let crate::fx::Effect::Reverb { decay, .. } = &mut b.mix.reverb {
                    *decay = (*decay * rng.range(0.75, 1.3) as f32).clamp(0.6, 6.0);
                    return Some(format!("reverb decay {:.1} s", decay));
                }
                None
            } else if let crate::fx::Effect::Delay { beats, .. } = &mut b.mix.delay {
                *beats = *rng.pick(&[0.375, 0.5, 0.75, 1.0, 1.5]);
                Some(format!("delay time {} beats", beats))
            } else {
                None
            }
        }
        "form_layer" => {
            let si = rng.below(b.form.len());
            let name = rng.pick(&layer_names).clone();
            let s = &mut b.form[si];
            if let Some(i) = s.layers.iter().position(|l| *l == name) {
                if s.layers.len() <= 2 {
                    return None;
                }
                s.layers.remove(i);
                Some(format!("form: {name} out of {}", s.name))
            } else {
                s.layers.push(name.clone());
                Some(format!("form: {name} into {}", s.name))
            }
        }
        "form_bars" => {
            let si = rng.below(b.form.len());
            let s = &mut b.form[si];
            // the way the critic would have the song's length go, when it says
            let ds: &[i32] = match issues.iter().find(|i| i.target == "form:short" || i.target == "form:long") {
                Some(i) if i.target == "form:short" => &[4, 8],
                Some(_) => &[-8, -4],
                None => &[-8, -4, 4, 8],
            };
            let d = *rng.pick(ds);
            let nb = s.bars as i32 + d;
            if !(4..=48).contains(&nb) {
                return None;
            }
            s.bars = nb as u32;
            Some(format!("form: {} now {} bars", s.name, s.bars))
        }
        "form_flags" => {
            let si = rng.below(b.form.len());
            let s = &mut b.form[si];
            let (name, flag): (&str, &mut bool) = match rng.below(4) {
                0 => ("fills", &mut s.fills),
                1 => ("gap", &mut s.gap),
                2 => ("impact", &mut s.impact),
                _ => ("filter sweep", &mut s.filter_in),
            };
            *flag = !*flag;
            Some(format!("form: {} {} {}", s.name, if *flag { "gets" } else { "loses" }, name))
        }
        "form_variant" => {
            let vars: Vec<String> = b
                .drums
                .iter()
                .flat_map(|d| d.patterns.keys().cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            let si = rng.below(b.form.len());
            let v = rng.pick(&vars).clone();
            let s = &mut b.form[si];
            if s.variant == v {
                return None;
            }
            let what = format!("form: {} drums {} -> {v}", s.name, s.variant);
            s.variant = v;
            Some(what)
        }
        "form_change" => {
            let aim = issues.iter().find_map(|i| crate::form::Aim::of(&i.target)).unwrap_or(crate::form::Aim::Any);
            let (form, what) = crate::form::change(&b.form, g, aim, rng)?;
            // a take plays from its section, which stays
            if b.loops.iter().filter_map(|l| l.from.as_ref()).any(|n| !form.iter().any(|s| s.name == *n)) {
                return None;
            }
            b.form = form;
            Some(format!("form: {what}"))
        }
        "form_build" => {
            let builds: Vec<usize> = (0..b.form.len()).filter(|&i| b.form[i].build).collect();
            let s = &mut b.form[*builds.get(rng.below(builds.len()))?];
            if rng.chance(0.5) {
                let style = *rng.pick(&BuildStyle::ALL);
                if style == s.build_style {
                    return None;
                }
                let what = format!("form: {} builds {} -> {}", s.name, s.build_style.name(), style.name());
                s.build_style = style;
                Some(what)
            } else {
                let amount = (s.build_amount + (rng.f32() - 0.5) * 0.6).clamp(0.0, 1.0);
                let what = format!("form: {} builds {:.2} -> {amount:.2} hard", s.name, s.build_amount);
                s.build_amount = amount;
                Some(what)
            }
        }
        "tempo" => {
            let d = (rng.int(1, 4) * if rng.chance(0.5) { 1 } else { -1 }) as f64;
            let nb = (b.bpm + d).clamp(g.bpm.0, g.bpm.1);
            if (nb - b.bpm).abs() < 0.5 {
                return None;
            }
            let what = format!("tempo {:.0} -> {nb:.0} BPM", b.bpm);
            b.bpm = nb;
            Some(what)
        }
        "key" => {
            let d = rng.int(1, 5) as i32 * if rng.chance(0.5) { 1 } else { -1 };
            let nk = Key::new(((b.key.tonic as i32 + d).rem_euclid(12)) as u8, b.key.mode);
            for l in b.loops.iter_mut() {
                if let Sound::Loop { tonal: true, .. } = l.sound {
                    let t = l.transpose + d;
                    l.transpose = if t > 6 {
                        t - 12
                    } else if t < -6 {
                        t + 12
                    } else {
                        t
                    };
                }
            }
            let what = format!("key {} -> {}", b.key.name(), nk.name());
            b.key = nk;
            Some(what)
        }
        "swing" => {
            let ns = (b.swing + (rng.f32() - 0.5) * 0.06).clamp(g.swing.0.min(0.5), g.swing.1.max(0.5) + 0.02);
            let what = format!("swing {:.3} -> {ns:.3}", b.swing);
            b.swing = ns;
            Some(what)
        }
        "loop_swap" => {
            let l = lib?;
            let li = rng.below(b.loops.len().max(1));
            let name = b.loops.get(li)?.name.clone();
            let spec = g.loops.iter().find(|s| s.name == name)?;
            let key = b.key;
            let (s, t) = l.find_loop(spec, g, b.bpm, &key, rng)?;
            let lp = &mut b.loops[li];
            let what = format!("{name}: {} -> {}", lp.sound.describe(), s.describe());
            lp.sound = s;
            lp.transpose = t;
            Some(what)
        }
        "loop_chop" => {
            // a sampled break or phrase re-chopped as the sampler chops one, in a genre that chops them: a slice
            // played in another's place (every fourth where it was), or one back in its own
            let chops = |l: &LoopPart| {
                let role = match l.name.as_str() {
                    crate::sampling::BREAK => Some(crate::library::prepared::Role::Break),
                    crate::sampling::PHRASE => Some(crate::library::prepared::Role::Phrase),
                    _ => None,
                };
                !l.chop.is_empty() || role.and_then(|r| crate::sampling::need(g, r)).is_some_and(|n| n.chop > 0.0)
            };
            let can: Vec<usize> = (0..b.loops.len()).filter(|&i| chops(&b.loops[i])).collect();
            let l = &mut b.loops[*can.get(rng.below(can.len()))?];
            let Sound::Loop { beats, .. } = &l.sound else { return None };
            let slice = if l.slice_beats > 0.0 { l.slice_beats } else { 0.5 };
            let slices = (beats / slice).round() as i32;
            if slices < 4 {
                return None;
            }
            if l.chop.is_empty() {
                l.chop = (0..slices).collect();
            }
            let free: Vec<usize> = (0..l.chop.len()).filter(|i| i % 4 != 0).collect();
            let i = *free.get(rng.below(free.len()))?;
            let what = if l.chop[i] != i as i32 && rng.chance(0.3) {
                l.chop[i] = i as i32;
                format!("slice {} back in its place", i + 1)
            } else {
                l.chop[i] = rng.below(slices as usize) as i32;
                format!("slice {} plays slice {}", i + 1, l.chop[i] + 1)
            };
            // every slice in its own place is the recording as it is
            if l.chop.iter().enumerate().all(|(k, &c)| c == k as i32) {
                l.chop.clear();
            }
            Some(format!("{}: {what}", l.name))
        }
        "drum_trig" => {
            // a step programmed as a drum machine's trig: a hat or a shaker playing by chance or rolling (where its
            // genre rolls them), a snare or a clap flammed or laid back, a percussion hit tuned, any but the kick's
            // brighter, darker, shorter or longer; or a step played plainly again
            let li = rng.below(b.drums.len().max(1));
            let role = b.drums.get(li)?.role();
            if role == "kick" {
                return None;
            }
            let rolls =
                drum_lane_spec(g, &b.drums[li].name).and_then(|s| s.template.as_ref()).is_some_and(|t| t.rolls > 0.0);
            let lane = &mut b.drums[li];
            let lane_name = lane.name.clone();
            let variants: Vec<String> = lane.patterns.keys().cloned().collect();
            let vname = variants.get(rng.below(variants.len()))?.clone();
            let hits = lane.patterns.get_mut(&vname)?;
            let i = rng.below(hits.len());
            let h = hits.get_mut(i)?;
            let at = h.t;
            let tops = matches!(role.as_str(), "hat" | "ohat" | "shaker" | "ride" | "perc");
            let back = matches!(role.as_str(), "snare" | "clap");
            let mut t = h.trig.unwrap_or_default();
            let what = match rng.below(4) {
                _ if h.trig.is_some() && rng.chance(0.25) => {
                    h.trig = None;
                    return Some(format!("drums ({vname}): the {lane_name} hit at beat {at:.2} played plainly"));
                }
                0 if tops && h.v < 0.8 => {
                    t.prob = rng.range(0.5, 0.9) as f32;
                    format!("plays {:.0}% of the time", t.prob * 100.0)
                }
                1 if tops && rolls => {
                    t.ratchet = *rng.pick(&[2, 3, 4]);
                    t.ramp = if rng.chance(0.5) { 0.5 } else { 0.0 };
                    format!("rolls in {}", t.ratchet)
                }
                0 | 1 if back && rng.chance(0.5) => {
                    t.flam = rng.range(12.0, 30.0) as f32;
                    format!("flammed {:.0} ms", t.flam)
                }
                0 | 1 if back => {
                    t.nudge = rng.range(0.05, 0.15) as f32;
                    format!("laid back {:.2} of a sixteenth", t.nudge)
                }
                2 if matches!(role.as_str(), "perc" | "tom") => {
                    t.pitch = rng.int(-5, 5) as i8;
                    format!("tuned {:+} semitones", t.pitch)
                }
                _ if rng.chance(0.5) => {
                    t.tone = (t.tone + (2.0 * rng.f32() - 1.0) * 0.3).clamp(-1.0, 1.0);
                    format!("tone {:+.2}", t.tone)
                }
                _ => {
                    t.decay = (t.decay * rng.range(0.7, 1.4) as f32).clamp(0.1, 4.0);
                    format!("decay {:.2} times its lane's", t.decay)
                }
            };
            h.trig = Some(t.clamped());
            Some(format!("drums ({vname}): the {lane_name} hit at beat {at:.2} {what}"))
        }
        "rack_layer" => {
            // one layer of a stacked part louder or quieter against the others, or a crossover between its layers
            // moved a third of an octave, both layers that meet there with it
            let racks: Vec<usize> =
                (0..b.parts.len()).filter(|&i| matches!(b.parts[i].sound, Sound::Rack { .. })).collect();
            let part = &mut b.parts[*racks.get(rng.below(racks.len()))?];
            let Sound::Rack { layers } = &mut part.sound else { return None };
            let edges: Vec<f32> = layers.iter().filter_map(|l| l.band).flatten().filter(|&f| f > 0.0).collect();
            let what = if !edges.is_empty() && rng.chance(0.5) {
                let f = edges[rng.below(edges.len())];
                let to = (f * 2f32.powf(if rng.chance(0.5) { 1.0 } else { -1.0 } / 3.0)).clamp(30.0, 16000.0);
                for l in layers.iter_mut() {
                    if let Some(band) = &mut l.band {
                        band.iter_mut().filter(|e| (**e - f).abs() < 1e-3).for_each(|e| *e = to);
                    }
                }
                if layers.iter().any(|l| l.band.is_some_and(|[lo, hi]| lo > 0.0 && hi > 0.0 && lo >= hi)) {
                    return None;
                }
                format!("crossover {f:.0} -> {to:.0} Hz")
            } else {
                let i = rng.below(layers.len());
                let l = layers.get_mut(i)?;
                l.level_db = (l.level_db + rng.range(-2.5, 2.5) as f32).clamp(-18.0, 6.0);
                format!("{} at {:+.1} dB", l.describe(), l.level_db)
            };
            Some(format!("{}: {what}", part.name))
        }
        "track_add" => {
            // a part its genre writes that the song left out, where its genre's forms play it, or a layer a more
            // layered song of its genre plays
            let missing: Vec<&crate::genres::PartSpec> =
                g.parts.iter().filter(|s| !b.parts.iter().any(|p| p.name == s.name)).collect();
            if missing.is_empty() || rng.chance(0.5) {
                return compose::one_more(b, g, lib, rng);
            }
            let spec = *rng.pick(&missing);
            let mut used = b.parts.iter().flat_map(|p| crate::palette::identities(&p.sound)).collect();
            let mut part = compose::new_part(spec, g, &crate::palette::instruments(), &mut used, rng);
            crate::colour::vary_part(&mut part, &rng.fork(&spec.name));
            let what = format!("{} in, on {}", spec.name, part.sound.describe());
            if !b.form.iter().any(|s| s.layers.contains(&spec.name)) {
                for s in b.form.iter_mut().filter(|s| matches!(s.kind, SectionKind::Hook | SectionKind::Drop)) {
                    let was = s.layers.clone();
                    s.layers.push(spec.name.clone());
                    crate::form::retune(s, &was, g);
                }
            }
            b.parts.push(part);
            Some(what)
        }
        "track_remove" => {
            // a part or a drum lane that only colours the song out of it: one its genre may leave out, or one a more
            // layered song plays, never one every song of its genre has
            let colour = |n: &String| !crate::form::sure(n, g);
            let names: Vec<String> = b
                .parts
                .iter()
                .map(|p| p.name.clone())
                .chain(b.drums.iter().map(|d| d.name.clone()))
                .filter(colour)
                .collect();
            let name = names.get(rng.below(names.len()))?.clone();
            b.parts.retain(|p| p.name != name);
            b.drums.retain(|d| d.name != name);
            for s in b.form.iter_mut().filter(|s| s.layers.contains(&name)) {
                let was = s.layers.clone();
                s.layers.retain(|l| *l != name);
                crate::form::retune(s, &was, g);
            }
            Some(format!("{name} out of the song"))
        }
        "fx_tweak" => {
            let names: Vec<&String> =
                layer_names.iter().filter(|n| mix_of(b, n).is_some_and(|m| !m.fx.is_empty())).collect();
            let name = (*names.get(rng.below(names.len()))?).clone();
            let m = track_mix(b, &name)?;
            let i = rng.below(m.fx.len());
            Some(format!("{name}: {}", tweak(&mut m.fx[i], rng)?))
        }
        "fx_add" => {
            let name = rng.pick(&layer_names).clone();
            let fx = new_fx(fx_role(b, &name)?, issues.iter().any(|i| i.target == "width-"), rng);
            let m = track_mix(b, &name)?;
            if m.fx.iter().any(|e| e.name() == fx.name()) {
                return None;
            }
            let what = format!("{name}: {} in", fx.name().replace('_', " "));
            m.fx.push(fx);
            Some(what)
        }
        "fx_remove" => {
            // one `fx_add` adds taken off again, a widening one when the song is too wide
            let wide = |n: &str| matches!(n, "chorus" | "auto_pan" | "width");
            let narrower = issues.iter().any(|i| i.target == "width+");
            let mut found: Vec<(String, usize)> = Vec::new();
            for n in &layer_names {
                for (i, e) in mix_of(b, n).map_or(&[][..], |m| &m.fx).iter().enumerate() {
                    if ADDED_FX.contains(&e.name()) && (!narrower || wide(e.name())) {
                        found.push((n.clone(), i));
                    }
                }
            }
            let (name, i) = found.get(rng.below(found.len()))?.clone();
            let e = track_mix(b, &name)?.fx.remove(i);
            Some(format!("{name}: {} out", e.name().replace('_', " ")))
        }
        "fx_eq" => {
            // the band the master has too much or too little of, cut or lifted on a track that lives in it, as an
            // engineer reaches for an equalizer: its band there moved, or one more band
            let (band, sign) = issues.iter().find_map(|i| band_of(&i.target))?;
            let names: Vec<&str> = band_names(band)?.iter().copied().filter(|n| has_mix(b, n)).collect();
            let name = names.get(rng.below(names.len()))?.to_string();
            let (kind, freq) = match band {
                "sub" => (crate::dsp::filter::BiquadKind::Lowshelf, 60.0),
                "low" => (crate::dsp::filter::BiquadKind::Peak, 120.0),
                "lowmid" => (crate::dsp::filter::BiquadKind::Peak, 400.0),
                "mid" => (crate::dsp::filter::BiquadKind::Peak, 2000.0),
                "presence" => (crate::dsp::filter::BiquadKind::Peak, 5500.0),
                _ => (crate::dsp::filter::BiquadKind::Highshelf, 10000.0),
            };
            let d = sign * rng.range(1.5, 3.0) as f32;
            let q = if kind == crate::dsp::filter::BiquadKind::Peak { 0.9 } else { 0.707 };
            let m = track_mix(b, &name)?;
            let near = |x: &crate::fx::EqBand| x.kind == kind && (x.freq / freq).log2().abs() < 0.5;
            match m.fx.iter_mut().find_map(|e| if let Effect::Eq { bands } = e { Some(bands) } else { None }) {
                Some(bands) => match bands.iter_mut().find(|x| near(x)) {
                    Some(x) => x.gain_db = (x.gain_db + d).clamp(-9.0, 9.0),
                    None => bands.push(crate::fx::EqBand { kind, freq, gain_db: d, q }),
                },
                None => m.fx.push(Effect::Eq { bands: vec![crate::fx::EqBand { kind, freq, gain_db: d, q }] }),
            }
            Some(format!("{name}: {d:+.1} dB at {freq:.0} Hz"))
        }
        _ => None,
    }
}

/// a child taking the drums of `a` and the parts of `b` (or the other way round)
pub fn crossover(a: &Blueprint, b: &Blueprint, rng: &mut Rng) -> (Blueprint, String) {
    let mut c = a.clone();
    let what = if rng.chance(0.5) {
        c.parts = b.parts.clone();
        c.harmony = b.harmony.clone();
        "crossover: drums and form of one parent, harmony and parts of the other"
    } else {
        c.drums = b.drums.clone();
        "crossover: drums of one parent, everything else of the other"
    };
    c.lineage.push(what.to_string());
    (c, what.to_string())
}

/// bars [from, to) to audition: the build-up into the first drop and the drop itself
pub fn preview_window(bp: &Blueprint, max_bars: u32) -> (u32, u32) {
    let mut bar = 0;
    let mut starts = Vec::new();
    for s in &bp.form {
        starts.push((bar, s));
        bar += s.bars;
    }
    if bar <= max_bars {
        return (0, bar);
    }
    if let Some(di) = starts.iter().position(|(_, s)| matches!(s.kind, SectionKind::Drop | SectionKind::Hook)) {
        let (drop_start, drop) = starts[di];
        let lead_in = if di > 0 { starts[di - 1].1.bars.min(16) } else { 0 };
        let from = drop_start - lead_in;
        return (from, (drop_start + drop.bars).min(from + max_bars));
    }
    (0, max_bars)
}

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub id: String,
    pub generation: usize,
    pub parent: Option<String>,
    pub change: String,
    pub score: f64,
    pub summary: String,
    #[serde(skip)]
    pub blueprint: Blueprint,
    #[serde(skip)]
    pub verdict: Option<Verdict>,
}

pub type Ears<'a> = &'a (dyn Fn(&Project, &Rendered) -> Option<EarsReport> + Sync);

/// the critic's verdict on `bp`: its mix, its arc and its time as its preview plays them (`preview_window`), its notes
/// and its form as the whole song has them
pub fn evaluate(bp: &Blueprint, preview_bars: u32, ears: Option<Ears>) -> Result<Verdict> {
    let whole = compose::arrange(bp);
    let (a, b) = preview_window(bp, preview_bars);
    let window = (a > 0 || b < whole.bars).then(|| whole.slice(a, b));
    let p = window.as_ref().unwrap_or(&whole);
    let r = render::render(p, &render::Options { stems: false, tail_secs: 1.0 })?;
    let e = ears.and_then(|f| f(p, &r));
    Ok(judge::judge_whole(p, &whole, &r, e))
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub generations: usize,
    pub population: usize,
    pub children: usize,
    pub preview_bars: u32,
    pub seed: u64,
    pub crossover: f64,
    /// set to stop after the generation in progress
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { generations: 5, population: 4, children: 8, preview_bars: 32, seed: 1, crossover: 0.2, cancel: None }
    }
}

/// (mu + lambda) evolution from `initial` blueprints; returns every candidate, best first
pub fn run(
    g: &GenreProfile,
    initial: Vec<Blueprint>,
    lib: Option<&dyn Library>,
    ears: Option<Ears>,
    s: &Settings,
    log: &(dyn Fn(&str) + Sync),
) -> Result<Vec<Candidate>> {
    let mut rng = Rng::new(s.seed ^ hash_str("evolve"));
    let mut all: Vec<Candidate> = Vec::new();
    let eval =
        |bp: Blueprint, id: String, generation: usize, parent: Option<String>, change: String| -> Result<Candidate> {
            let v = evaluate(&bp, s.preview_bars, ears)?;
            Ok(Candidate {
                id,
                generation,
                parent,
                change,
                score: v.total,
                summary: v.summary.clone(),
                blueprint: bp,
                verdict: Some(v),
            })
        };
    let first: Vec<Candidate> = initial
        .into_par_iter()
        .enumerate()
        .map(|(i, bp)| eval(bp, format!("g0-{i}"), 0, None, "initial".into()))
        .collect::<Result<_>>()?;
    for c in &first {
        log(&format!("  {} {:5.1}  {}", c.id, c.score, c.blueprint.title));
    }
    all.extend(first);
    let mut seen: std::collections::HashSet<u64> = all.iter().map(|c| hash_str(&c.blueprint.to_json())).collect();
    for generation in 1..=s.generations {
        if s.cancel.as_ref().is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
            log("stopped");
            break;
        }
        let jobs = children(&all, g, lib, s, &mut rng, &mut seen);
        let parents = best(&all, s.population);
        let kids: Vec<Candidate> = jobs
            .into_par_iter()
            .enumerate()
            .map(|(i, (bp, parent, change))| eval(bp, format!("g{generation}-{i}"), generation, parent, change))
            .collect::<Result<_>>()?;
        let best_before = parents.first().map_or(0.0, |p| p.score);
        for k in &kids {
            let mark = if k.score > best_before { "+" } else { " " };
            log(&format!("{mark} {} {:5.1}  from {}: {}", k.id, k.score, k.parent.as_deref().unwrap_or("-"), k.change));
        }
        all.extend(kids);
        let best = all.iter().map(|c| c.score).fold(f64::MIN, f64::max);
        log(&format!("generation {generation}: best {best:.1}"));
    }
    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    Ok(all)
}

/// the `n` best of `all`, best first
fn best(all: &[Candidate], n: usize) -> Vec<Candidate> {
    let mut pool: Vec<&Candidate> = all.iter().collect();
    pool.sort_by(|a, b| b.score.total_cmp(&a.score));
    pool.into_iter().take(n).cloned().collect()
}

/// The next generation's children of the best of `all` (`s.population` of them): each one or two mutations of a
/// parent, or of a crossover of two, aimed at its verdict's issues; none that `seen` has seen. With its parent and
/// what changed.
fn children(
    all: &[Candidate],
    g: &GenreProfile,
    lib: Option<&dyn Library>,
    s: &Settings,
    rng: &mut Rng,
    seen: &mut std::collections::HashSet<u64>,
) -> Vec<(Blueprint, Option<String>, String)> {
    let parents = best(all, s.population);
    let mut jobs: Vec<(Blueprint, Option<String>, String)> = Vec::new();
    let mut tries = 0;
    while !parents.is_empty() && jobs.len() < s.children && tries < s.children * 10 {
        tries += 1;
        let pa = &parents[rng.below(parents.len()).min(rng.below(parents.len()))];
        let issues = pa.verdict.as_ref().map(|v| v.issues.clone()).unwrap_or_default();
        let (mut child, mut change) = if parents.len() > 1 && rng.chance(s.crossover) {
            let pb = &parents[rng.below(parents.len())];
            crossover(&pa.blueprint, &pb.blueprint, rng)
        } else {
            (pa.blueprint.clone(), String::new())
        };
        let n_ops = 1 + rng.below(2);
        for _ in 0..n_ops {
            let (c, w) = mutate(&child, g, lib, &issues, rng);
            child = c;
            change = if change.is_empty() { w } else { format!("{change}; {w}") };
        }
        let h = hash_str(&child.to_json());
        if seen.insert(h) {
            jobs.push((child, Some(pa.id.clone()), change));
        }
    }
    jobs
}

/// An evolution made one candidate at a time (`step`), for a host with no threads that answers between them (the
/// browser's demo): the generations `run` makes, judged on the same window, without the ears. A mutation is one
/// generation of children of the song alone, none crossed (`Settings { generations: 1, population: 1, crossover:
/// 0.0, .. }`).
pub struct Stepwise {
    g: GenreProfile,
    s: Settings,
    rng: Rng,
    all: Vec<Candidate>,
    waiting: std::collections::VecDeque<(Blueprint, Option<String>, String, usize)>,
    generation: usize,
    seen: std::collections::HashSet<u64>,
}

impl Stepwise {
    pub fn new(g: &GenreProfile, initial: Vec<Blueprint>, s: Settings) -> Stepwise {
        let rng = Rng::new(s.seed ^ hash_str("evolve"));
        let seen = initial.iter().map(|bp| hash_str(&bp.to_json())).collect();
        let waiting = initial.into_iter().map(|bp| (bp, None, "initial".to_string(), 0)).collect();
        Stepwise { g: g.clone(), s, rng, all: Vec::new(), waiting, generation: 0, seen }
    }

    /// Judges the next candidate, after breeding the next generation when the last is judged: what the log says of
    /// it, or None once every generation has been judged.
    pub fn step(&mut self, lib: Option<&dyn Library>) -> Result<Option<String>> {
        if self.waiting.is_empty() {
            if self.generation >= self.s.generations {
                return Ok(None);
            }
            self.generation += 1;
            let jobs = children(&self.all, &self.g, lib, &self.s, &mut self.rng, &mut self.seen);
            if jobs.is_empty() {
                return Ok(None);
            }
            self.waiting.extend(jobs.into_iter().map(|(bp, parent, change)| (bp, parent, change, self.generation)));
        }
        let Some((bp, parent, change, generation)) = self.waiting.pop_front() else { return Ok(None) };
        let v = evaluate(&bp, self.s.preview_bars, None)?;
        let n = self.all.iter().filter(|c| c.generation == generation).count();
        let best_before = self.all.iter().map(|c| c.score).fold(f64::MIN, f64::max);
        let id = format!("g{generation}-{n}");
        let said = match &parent {
            None => format!("  {id} {:5.1}  {}", v.total, bp.title),
            Some(p) => {
                let mark = if v.total > best_before { "+" } else { " " };
                format!("{mark} {id} {:5.1}  from {p}: {change}", v.total)
            }
        };
        self.all.push(Candidate {
            id,
            generation,
            parent,
            change,
            score: v.total,
            summary: v.summary.clone(),
            blueprint: bp,
            verdict: Some(v),
        });
        Ok(Some(said))
    }

    /// the candidates judged, and how many there will be: every generation's children (or fewer, when the song's
    /// mutations run out)
    pub fn progress(&self) -> (usize, usize) {
        let initial =
            self.all.iter().filter(|c| c.generation == 0).count() + self.waiting.iter().filter(|w| w.3 == 0).count();
        (self.all.len(), initial + self.s.generations * self.s.children)
    }

    /// the generation being judged
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// every candidate judged, best first
    pub fn into_candidates(self) -> Vec<Candidate> {
        let mut all = self.all;
        all.sort_by(|a, b| b.score.total_cmp(&a.score));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_leaves_the_low_end_centred_and_dry() {
        // dubstep's basses are a sub and a wobble: neither is called bass
        let g = crate::genres::get("dubstep").unwrap();
        let bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let low = |b: &Blueprint| -> Vec<(f32, f32, f32)> {
            let kick = b.drums.iter().filter(|d| d.name == "kick").map(|d| &d.mix);
            let basses = b.parts.iter().filter(|p| p.role == Role::Bass).map(|p| &p.mix);
            kick.chain(basses).map(|m| (m.pan, m.reverb, m.delay)).collect()
        };
        assert_eq!(low(&bp).len(), 3, "a kick, a sub and a wobble");
        for seed in 0..200 {
            let mut b = bp.clone();
            let mut rng = Rng::new(seed);
            for op in ["mix_send", "mix_pan"] {
                apply_op(&mut b, op, &g, None, &[], &mut rng);
            }
            assert_eq!(low(&b), low(&bp), "seed {seed}");
        }
    }

    #[test]
    fn a_stacked_bass_is_tweaked_in_the_sound_it_is_heard_by_alike_in_each_layer_that_plays_it() {
        let g = crate::genres::get("trap").unwrap();
        let bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let patches = |b: &Blueprint| -> Vec<crate::synth::voice::Patch> {
            let bass = b.parts.iter().find(|p| p.name == "bass").unwrap();
            assert!(matches!(bass.sound, Sound::Rack { .. }), "trap stacks its 808");
            let each = bass.sound.each().into_iter();
            each.filter_map(|s| if let Sound::Synth { patch, .. } = s { Some(patch.clone()) } else { None }).collect()
        };
        let aimed = [Issue { severity: 1.0, what: String::new(), target: "sound:bass".into() }];
        let tweaked = (0..40)
            .map(|seed| {
                let mut b = bp.clone();
                apply_op(&mut b, "patch_tweak", &g, None, &aimed, &mut Rng::new(seed)).map(|_| b)
            })
            .find_map(|b| b.filter(|b| patches(b) != patches(&bp)))
            .expect("a tweak of the 808");
        let (was, now) = (patches(&bp), patches(&tweaked));
        assert_eq!(was.len(), 2);
        assert!(now[0] == now[1] && now[0] != was[0], "the 808 and its grit layer change together");
    }

    #[test]
    fn a_preview_is_judged_with_the_whole_songs_form() {
        let g = crate::genres::get("techno").unwrap();
        let mut bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let whole = compose::arrange(&bp);
        let v = evaluate(&bp, 32, None).unwrap();
        assert!(preview_window(&bp, 32).1 - preview_window(&bp, 32).0 < whole.bars, "the preview is part of the song");
        assert_eq!(v.measures["length_s"], whole.secs().round());
        // its first three sections alone are a short song, whatever its preview plays
        bp.form.truncate(3);
        let v = evaluate(&bp, 32, None).unwrap();
        assert!(v.issues.iter().any(|i| i.target == "form:short"), "{:?}", v.issues);
    }

    #[test]
    fn a_song_the_critic_finds_short_is_lengthened_and_never_shortened() {
        let g = crate::genres::get("techno").unwrap();
        let mut bp = compose::generate(&g, 1, &compose::Options::default(), None);
        bp.form.truncate(3);
        let bars = |b: &Blueprint| b.form.iter().map(|s| s.bars).sum::<u32>();
        let short = [Issue { severity: 0.6, what: String::new(), target: "form:short".into() }];
        let mut rng = Rng::new(5);
        let mut longer = 0;
        for _ in 0..150 {
            let (child, what) = mutate(&bp, &g, None, &short, &mut rng);
            assert!(bars(&child) >= bars(&bp), "{what}");
            longer += usize::from(bars(&child) > bars(&bp));
        }
        assert!(longer >= 10, "{longer} of 150 mutations lengthen it");
    }

    /// every track a song plays, by name
    fn tracks(b: &Blueprint) -> Vec<String> {
        b.parts.iter().map(|p| p.name.clone()).chain(b.drums.iter().map(|d| d.name.clone())).collect()
    }

    #[test]
    fn a_track_comes_in_one_at_a_time_and_goes_only_if_its_genre_may_do_without_it() {
        for genre in ["house", "trap", "lofi", "techno", "neo_soul"] {
            let g = crate::genres::get(genre).unwrap();
            let bp = compose::generate(&g, 2, &compose::Options::default(), None);
            for seed in 0..12 {
                let mut b = bp.clone();
                let mut rng = Rng::new(seed);
                if let Some(what) = apply_op(&mut b, "track_add", &g, None, &[], &mut rng) {
                    let new: Vec<String> = tracks(&b).into_iter().filter(|t| !tracks(&bp).contains(t)).collect();
                    assert_eq!(new.len(), 1, "{genre} {seed}: {what}: {new:?}");
                    assert!(
                        b.form.iter().any(|s| s.layers.contains(&new[0])),
                        "{genre} {seed}: {what} plays somewhere"
                    );
                    compose::arrange(&b);
                }
                let mut b = bp.clone();
                if let Some(what) = apply_op(&mut b, "track_remove", &g, None, &[], &mut Rng::new(seed)) {
                    let gone: Vec<String> = tracks(&bp).into_iter().filter(|t| !tracks(&b).contains(t)).collect();
                    assert_eq!(gone.len(), 1, "{genre} {seed}: {what}");
                    assert!(!crate::form::sure(&gone[0], &g), "{genre} {seed}: {what}, which every {genre} song has");
                    assert!(b.form.iter().all(|s| !s.layers.contains(&gone[0])), "{genre} {seed}: {what}");
                    assert!(b.form.iter().all(|s| s.layers.iter().any(|l| crate::form::sure(l, &g))));
                }
            }
        }
    }

    #[test]
    fn a_trig_leaves_the_kick_alone_and_rolls_only_where_the_genre_rolls() {
        let set = |genre: &str| -> Vec<(String, crate::theory::rhythm::Trig)> {
            let g = crate::genres::get(genre).unwrap();
            let bp = compose::generate(&g, 1, &compose::Options::default(), None);
            (0..300)
                .filter_map(|seed| {
                    let mut b = bp.clone();
                    apply_op(&mut b, "drum_trig", &g, None, &[], &mut Rng::new(seed))?;
                    let lane = b.drums.iter().zip(&bp.drums).find(|(x, y)| x.patterns != y.patterns)?.0;
                    let hit = lane.patterns.values().flatten().find_map(|h| h.trig)?;
                    Some((lane.role(), hit))
                })
                .collect()
        };
        let trap = set("trap");
        assert!(trap.iter().all(|(role, _)| role != "kick"));
        assert!(trap.iter().any(|(role, t)| role == "hat" && t.ratchet > 1), "trap rolls its hats");
        assert!(trap.iter().any(|(role, t)| matches!(role.as_str(), "snare" | "clap") && t.flam > 0.0));
        let house = set("house");
        assert!(house.iter().any(|(_, t)| t.prob < 1.0), "a hat by chance");
        assert!(house.iter().all(|(role, t)| t.ratchet == 1 || role != "hat"), "house's hats do not roll");
    }

    #[test]
    fn an_effect_comes_in_as_its_track_takes_one_and_the_low_end_stays_narrow() {
        let g = crate::genres::get("house").unwrap();
        let bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let wide = ["chorus", "phaser", "auto_pan", "width", "delay", "tremolo"];
        let mut added = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let mut b = bp.clone();
            let Some(what) = apply_op(&mut b, "fx_add", &g, None, &[], &mut Rng::new(seed)) else { continue };
            let name = what.split(':').next().unwrap();
            let fx = &mix_of(&b, name).unwrap().fx;
            let new = fx.last().unwrap().name();
            added.insert(new);
            assert!(ADDED_FX.contains(&new), "{what}");
            if low_end(&b, name) {
                assert!(!wide.contains(&new), "{what}: the low end stays mono and dry");
            }
            // and taken off again, the track's chain is what it was
            let mut c = b.clone();
            for s in 0..50 {
                if apply_op(&mut c, "fx_remove", &g, None, &[], &mut Rng::new(s)).is_some_and(|w| w.starts_with(name))
                    && mix_of(&c, name).unwrap().fx.len() < fx.len()
                {
                    break;
                }
                c = b.clone();
            }
        }
        assert!(added.len() >= 6, "{added:?}");
    }

    #[test]
    fn a_band_the_master_has_too_much_of_is_cut_on_a_track_that_lives_in_it() {
        let g = crate::genres::get("house").unwrap();
        let bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let muddy = [Issue { severity: 0.5, what: String::new(), target: "band:lowmid+".into() }];
        let mut cut = 0;
        for seed in 0..20 {
            let mut b = bp.clone();
            let Some(what) = apply_op(&mut b, "fx_eq", &g, None, &muddy, &mut Rng::new(seed)) else { continue };
            let name = what.split(':').next().unwrap();
            assert!(["pad", "chords", "keys"].contains(&name), "{what}");
            let bands: Vec<crate::fx::EqBand> = mix_of(&b, name)
                .unwrap()
                .fx
                .iter()
                .filter_map(|e| if let Effect::Eq { bands } = e { Some(bands.clone()) } else { None })
                .flatten()
                .collect();
            assert!(bands.iter().any(|x| x.freq == 400.0 && x.gain_db < 0.0), "{what}: {bands:?}");
            cut += 1;
        }
        assert!(cut >= 10);
        assert!(apply_op(&mut bp.clone(), "fx_eq", &g, None, &[], &mut Rng::new(1)).is_none(), "only when asked");
    }

    #[test]
    fn an_effects_setting_moves_and_the_chain_stays_as_long() {
        let g = crate::genres::get("synthwave").unwrap();
        let bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let chains = |b: &Blueprint| -> Vec<usize> {
            b.layer_names().iter().map(|n| mix_of(b, n).map_or(0, |m| m.fx.len())).collect()
        };
        let moved = (0..40)
            .filter(|&seed| {
                let mut b = bp.clone();
                apply_op(&mut b, "fx_tweak", &g, None, &[], &mut Rng::new(seed)).is_some() && b != bp && {
                    assert_eq!(chains(&b), chains(&bp));
                    true
                }
            })
            .count();
        assert!(moved >= 20, "{moved}");
    }

    #[test]
    fn a_racks_crossover_moves_for_both_its_layers_at_once() {
        let g = crate::genres::get("trap").unwrap();
        let mut bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let bass = bp.parts.iter_mut().find(|p| p.role == Role::Bass).unwrap();
        let one = bass.sound.each()[0].clone();
        let band = |lo: f32, hi: f32| Layer { band: Some([lo, hi]), ..Layer::of(one.clone()) };
        bass.sound = Sound::Rack { layers: vec![band(0.0, 100.0), band(100.0, 2000.0), band(2000.0, 0.0)] };
        let bands = |b: &Blueprint| -> Vec<[f32; 2]> {
            let bass = b.parts.iter().find(|p| p.role == Role::Bass).unwrap();
            let Sound::Rack { layers } = &bass.sound else { panic!("a rack") };
            layers.iter().filter_map(|l| l.band).collect()
        };
        let mut moved = 0;
        for seed in 0..60 {
            let mut b = bp.clone();
            let Some(what) = apply_op(&mut b, "rack_layer", &g, None, &[], &mut Rng::new(seed)) else { continue };
            if what.contains("crossover") {
                let now = bands(&b);
                assert!(now[0][1] == now[1][0] && now[1][1] == now[2][0], "{what}: where two layers met, they meet");
                assert!(now != bands(&bp), "{what}");
                moved += 1;
            }
        }
        assert!(moved >= 10, "{moved}");
    }

    #[test]
    fn a_song_changed_track_by_track_still_arranges_and_renders() {
        let ops = [
            "drum_trig",
            "rack_layer",
            "track_add",
            "track_remove",
            "fx_tweak",
            "fx_add",
            "fx_remove",
            "fx_eq",
            "loop_chop",
        ];
        let issues = [Issue { severity: 0.5, what: String::new(), target: "band:presence-".into() }];
        for genre in ["trap", "house", "lofi", "future_bass"] {
            let g = crate::genres::get(genre).unwrap();
            let mut b = compose::generate(&g, 3, &compose::Options { short: true, ..Default::default() }, None);
            let mut rng = Rng::new(9);
            let changed =
                (0..90).filter(|k| apply_op(&mut b, ops[k % ops.len()], &g, None, &issues, &mut rng).is_some());
            assert!(changed.count() >= 45, "{genre}");
            let p = compose::arrange(&b);
            let r =
                render::render(&p.slice(0, 8.min(p.bars)), &render::Options { stems: false, tail_secs: 0.5 }).unwrap();
            assert!(r.master.l.iter().chain(&r.master.r).all(|v| v.is_finite()), "{genre}");
        }
    }

    #[test]
    fn a_break_is_rechopped_with_every_fourth_slice_where_it_was() {
        let g = crate::genres::get("boom_bap").unwrap();
        let mut bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let sound = Sound::Loop {
            path: "demo:break".into(),
            bpm: 92.0,
            beats: 8.0,
            key: None,
            tonal: false,
            label: String::new(),
        };
        bp.loops.push(LoopPart {
            name: crate::sampling::BREAK.into(),
            role: "drums".into(),
            sound,
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.5,
            retime: false,
            mix: TrackMix::default(),
            from: None,
            sampled: false,
        });
        let mut b = bp.clone();
        let mut rng = Rng::new(4);
        for _ in 0..20 {
            apply_op(&mut b, "loop_chop", &g, None, &[], &mut rng);
        }
        let chop = &b.loops.last().unwrap().chop;
        assert_eq!(chop.len(), 16, "half-beat slices of eight beats");
        assert!(chop.iter().enumerate().any(|(i, &c)| c != i as i32), "re-chopped");
        assert!(chop.iter().enumerate().all(|(i, &c)| i % 4 != 0 || c == i as i32), "{chop:?}");
        // a genre that plays its records whole leaves them whole
        let house = crate::genres::get("house").unwrap();
        let mut whole = bp.clone();
        whole.loops.last_mut().unwrap().name = "texture".into();
        assert!(apply_op(&mut whole, "loop_chop", &house, None, &[], &mut Rng::new(4)).is_none());
    }

    #[test]
    fn a_build_up_changes_its_style_or_how_hard_it_builds() {
        let g = crate::genres::get("trance").unwrap();
        let bp = compose::generate(&g, 1, &compose::Options::default(), None);
        let builds = |b: &Blueprint| -> Vec<(BuildStyle, f32)> {
            b.form.iter().filter(|s| s.build).map(|s| (s.build_style, s.build_amount)).collect()
        };
        assert!(!builds(&bp).is_empty(), "trance builds");
        let changed = (0..20).filter(|&seed| {
            let mut b = bp.clone();
            apply_op(&mut b, "form_build", &g, None, &[], &mut Rng::new(seed)).is_some() && builds(&b) != builds(&bp)
        });
        assert!(changed.count() >= 8);
    }
}
