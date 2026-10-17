//! Evolution: mutate blueprints the way a producer revises a track, keep what the critic likes.
//!
//! Mutation operators work on every level of the blueprint: harmony (theory-aware substitutions, new progressions),
//! drums (hit edits, regenerated patterns, sounds swapped between synthesis and recordings), parts (a new writing
//! seed, style, preset, patch tweaks, register, density), mix (levels, sends, sidechain, pan, space), form (layers per
//! section, lengths, fills / gaps / builds, variants) and the global feel (tempo, key, swing). The critic's issues
//! bias which operators are tried, crossover recombines parents, and every child records what changed.

use std::collections::BTreeMap;

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;

use crate::compose::{self, Library};
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

const OPS: &[&str] = &[
    "harmony_sub",
    "harmony_new",
    "drum_edit",
    "drum_regen",
    "drum_sound",
    "drum_kit",
    "part_seed",
    "part_style",
    "part_preset",
    "patch_tweak",
    "part_register",
    "part_density",
    "mix_level",
    "mix_send",
    "mix_duck",
    "mix_pan",
    "space",
    "form_layer",
    "form_bars",
    "form_flags",
    "form_variant",
    "tempo",
    "key",
    "swing",
    "loop_swap",
];

/// operator weights, raised for the operators that address the critic's issues
fn weights(issues: &[Issue]) -> Vec<f64> {
    let mut w: BTreeMap<&str, f64> = OPS.iter().map(|o| (*o, 1.0)).collect();
    for i in issues.iter().take(4) {
        let t = i.target.as_str();
        let boost = 1.0 + 4.0 * i.severity;
        let hit: &[&str] = if t.starts_with("band:") || t.starts_with("width") || t == "production" {
            &["mix_level", "drum_sound", "drum_kit", "patch_tweak", "space", "mix_pan"]
        } else if t == "form" {
            &["form_layer", "form_flags", "form_variant", "form_bars"]
        } else if t.starts_with("part:") {
            &["part_seed", "part_style", "part_density", "part_register"]
        } else if t == "genre" {
            &["part_preset", "part_style", "drum_regen", "drum_sound", "drum_kit", "tempo", "harmony_new"]
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

/// the track a band issue points at (what to turn up or down)
fn band_track(target: &str, rng: &mut Rng) -> Option<(String, f32)> {
    let (band, sign) =
        target.strip_prefix("band:").map(|b| (&b[..b.len() - 1], if b.ends_with('+') { -1.0 } else { 1.0 }))?;
    let names: &[&str] = match band {
        "sub" | "low" => &["kick", "bass"],
        "lowmid" => &["pad", "chords", "keys"],
        "mid" => &["chords", "lead", "keys", "arp"],
        "presence" => &["clap", "snare", "hat", "perc", "shaker"],
        "air" => &["hat", "ohat", "shaker", "ride"],
        _ => return None,
    };
    Some((rng.pick(names).to_string(), sign))
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
                    hits.push(Hit { t: step, v: 0.5 + 0.4 * rng.f32() });
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
                .filter_map(|i| i.target.strip_prefix("part:"))
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
                    let s = spec?.presets.clone();
                    let np = rng.pick(&s).clone();
                    let sound = crate::compose::part_sound(&np, rng);
                    if sound == part.sound {
                        return None;
                    }
                    let what = format!("{}: sound {} -> {}", part.name, part.sound.describe(), sound.describe());
                    part.sound = sound;
                    Some(what)
                }
                "patch_tweak" => {
                    let Sound::Synth { patch, preset } = &mut part.sound else { return None };
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
            let band = issues.iter().find_map(|i| band_track(&i.target, rng));
            let (name, sign) = match band {
                Some((n, s)) if layer_names.contains(&n) && rng.chance(0.75) => (n, s),
                _ => (rng.pick(&layer_names).clone(), if rng.chance(0.5) { 1.0 } else { -1.0 }),
            };
            let m = track_mix(b, &name)?;
            let d = sign * rng.range(1.0, 3.0) as f32;
            m.level_db = (m.level_db + d).clamp(-12.0, 9.0);
            Some(format!("mix: {name} {d:+.1} dB (now {:+.1})", m.level_db))
        }
        "mix_send" => {
            let name = rng.pick(&layer_names).clone();
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
            if matches!(name.as_str(), "kick" | "bass" | "snare" | "clap") {
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
            let d: i32 = *rng.pick(&[-8, -4, 4, 8]);
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

pub fn evaluate(bp: &Blueprint, preview_bars: u32, ears: Option<Ears>) -> Result<Verdict> {
    let full = compose::arrange(bp);
    let (a, b) = preview_window(bp, preview_bars);
    let p = if a == 0 && b >= full.bars { full } else { full.slice(a, b) };
    let r = render::render(&p, &render::Options { stems: false, tail_secs: 1.0 })?;
    let e = ears.and_then(|f| f(&p, &r));
    Ok(judge::judge(&p, &r, e))
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
        let mut pool: Vec<&Candidate> = all.iter().collect();
        pool.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        let parents: Vec<Candidate> = pool.into_iter().take(s.population).cloned().collect();
        let mut jobs: Vec<(Blueprint, Option<String>, String)> = Vec::new();
        let mut tries = 0;
        while jobs.len() < s.children && tries < s.children * 10 {
            tries += 1;
            let pa = &parents[rng.below(parents.len()).min(rng.below(parents.len()))];
            let issues = pa.verdict.as_ref().map(|v| v.issues.clone()).unwrap_or_default();
            let (mut child, mut change) = if parents.len() > 1 && rng.chance(s.crossover) {
                let pb = &parents[rng.below(parents.len())];
                crossover(&pa.blueprint, &pb.blueprint, &mut rng)
            } else {
                (pa.blueprint.clone(), String::new())
            };
            let n_ops = 1 + rng.below(2);
            for _ in 0..n_ops {
                let (c, w) = mutate(&child, g, lib, &issues, &mut rng);
                child = c;
                change = if change.is_empty() { w } else { format!("{change}; {w}") };
            }
            let h = hash_str(&child.to_json());
            if seen.insert(h) {
                jobs.push((child, Some(pa.id.clone()), change));
            }
        }
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
