//! Parts and the mix: `part`, `drum_sound`, `mix`, `mute`, `solo`, `remove` and `master`.

use std::cell::RefCell;
use std::rc::Rc;

use rhai::{Array, Engine, Map};

use crate::compose;
use crate::model::*;
use crate::synth::presets;

use super::song::{check_keys, genre_of, map_f32, map_str, with_bp};
use super::sounds::resolve_sound;
use super::{Ctx, R, fail, num};

/// parts and the mix: `part`, `mix`, `mute`, `solo`, `remove`, `master`
pub(super) fn register_mix(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    let c = ctx.clone();
    engine.register_fn("part", move |name: &str, m: Map| -> R<()> {
        check_keys(&m, "part()", &["style", "preset", "register", "density", "seed", "rate"])?;
        with_bp(&c, |bp, _| {
            if !bp.parts.iter().any(|p| p.name == name) {
                add_genre_part(bp, name)?;
            }
            let part = bp.parts.iter_mut().find(|p| p.name == name).expect("added above");
            if let Some(s) = map_str(&m, "style") {
                let styles = compose::styles_for(part.role);
                if !styles.contains(&s.as_str()) {
                    let role = part.role.name();
                    let article = if role.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
                    return fail(format!("{article} {role} part has no style '{s}' (styles: {})", styles.join(" ")));
                }
                part.style = s;
            }
            if let Some(pr) = map_str(&m, "preset") {
                // the same preset keeps its (possibly varied) patch
                let same = match &part.sound {
                    Sound::Synth { preset, .. } => *preset == pr,
                    Sound::Instrument { name, label } => *name == pr || *label == pr,
                    _ => false,
                };
                if !same {
                    part.sound = if let Some(patch) = presets::get(&pr) {
                        Sound::Synth { preset: pr, patch }
                    } else if let Some(r) = claudius_packs::resolve(&pr, None) {
                        Sound::Instrument { name: r.to_string(), label: pr }
                    } else {
                        return fail(format!("unknown preset '{pr}' (synths: {}; or an installed instrument such as piano, rhodes, string_ensemble)", presets::names().join(" ")));
                    };
                }
            }
            if let Some(r) = m.get("register").and_then(|d| d.clone().try_cast::<Array>())
                && r.len() == 2
                    && let (Some(a), Some(b)) = (num(&r[0]), num(&r[1])) {
                        part.register = (a as i32, b as i32);
                    }
            if let Some(x) = map_f32(&m, "density") {
                part.density = x.clamp(0.0, 1.0);
            }
            if let Some(x) = m.get("seed").and_then(|d| d.as_int().ok()) {
                part.seed = x as u64;
            }
            if let Some(x) = m.get("rate").and_then(num) {
                part.rate = x.clamp(1.0 / 32.0, 4.0);
            }
            Ok(())
        })
    });
    for (name, banked) in [("drum_sound", false), ("drum_sound", true)] {
        let c = ctx.clone();
        let set = move |lane: &str, label: &str, bank: Option<&str>| -> R<()> {
            let (lib, roots) = {
                let cx = c.borrow();
                (cx.lib.clone(), cx.roots.clone())
            };
            with_bp(&c, |bp, _| {
                let g = genre_of(bp);
                let sound = resolve_sound(label, bank, lib.as_deref(), g.as_ref(), bp.seed, roots.as_deref())?;
                let lanes = match bp.drums.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(" ") {
                    l if l.is_empty() => "none yet".to_string(),
                    l => l,
                };
                let l = bp
                    .drums
                    .iter_mut()
                    .find(|d| d.name == lane)
                    .ok_or_else(|| format!("no drum lane '{lane}' (lanes: {lanes}); drum() makes one"))?;
                // the sound it already plays stays as it is (labels and cut points included)
                let same = match (&l.sound, &sound) {
                    (Sound::Instrument { name: a, .. }, Sound::Instrument { name: b, .. }) => a == b,
                    (Sound::Sample { path: a, .. }, Sound::Sample { path: b, .. }) => a == b,
                    // the same voice keeps its tuning, unless an index asks for a variation ("bd:2")
                    (Sound::Drum { voice: a, .. }, Sound::Drum { voice: b, .. }) => a == b && !label.contains(':'),
                    _ => false,
                };
                match sound {
                    _ if same => {}
                    Sound::Drum { .. } | Sound::Sample { .. } | Sound::Instrument { .. } => l.sound = sound,
                    _ => return fail(format!("'{label}' is not a drum sound (a drum name, a kit sound, or a sample)")),
                }
                Ok(())
            })
        };
        if banked {
            engine.register_fn(name, move |lane: &str, label: &str, bank: &str| set(lane, label, Some(bank)));
        } else {
            engine.register_fn(name, move |lane: &str, label: &str| set(lane, label, None));
        }
    }
    let c = ctx.clone();
    engine.register_fn("mix", move |name: &str, m: Map| -> R<()> {
        check_keys(&m, "mix()", &["level", "pan", "reverb", "room", "delay", "duck", "mute", "solo"])?;
        with_bp(&c, |bp, _| {
            let mix = mix_of(bp, name).ok_or_else(|| format!("no track '{name}'"))?;
            if let Some(v) = map_f32(&m, "level") {
                mix.level_db = v.clamp(-40.0, 12.0);
            }
            if let Some(v) = map_f32(&m, "pan") {
                mix.pan = v.clamp(-1.0, 1.0);
            }
            if let Some(v) = map_f32(&m, "reverb").or_else(|| map_f32(&m, "room")) {
                mix.reverb = v.clamp(0.0, 1.0);
            }
            if let Some(v) = map_f32(&m, "delay") {
                mix.delay = v.clamp(0.0, 1.0);
            }
            if let Some(v) = map_f32(&m, "duck") {
                mix.duck = v.clamp(0.0, 1.0);
            }
            if let Some(v) = m.get("mute").and_then(|d| d.as_bool().ok()) {
                mix.mute = v;
            }
            if let Some(v) = m.get("solo").and_then(|d| d.as_bool().ok()) {
                mix.solo = v;
            }
            Ok(())
        })
    });
    for (name, on) in [("mute", true), ("unmute", false)] {
        let c = ctx.clone();
        engine.register_fn(name, move |track: &str| -> R<()> {
            with_bp(&c, |bp, _| {
                mix_of(bp, track).ok_or_else(|| format!("no track '{track}'"))?.mute = on;
                Ok(())
            })
        });
    }
    for (name, on) in [("solo", true), ("unsolo", false)] {
        let c = ctx.clone();
        engine.register_fn(name, move |track: &str| -> R<()> {
            with_bp(&c, |bp, _| {
                mix_of(bp, track).ok_or_else(|| format!("no track '{track}'"))?.solo = on;
                Ok(())
            })
        });
    }
    let c = ctx.clone();
    engine.register_fn("remove", move |track: &str| -> R<()> {
        with_bp(&c, |bp, _| {
            let before = bp.drums.len() + bp.parts.len() + bp.loops.len() + bp.patterns.len();
            bp.drums.retain(|d| d.name != track);
            bp.parts.retain(|p| p.name != track);
            bp.loops.retain(|l| l.name != track);
            bp.patterns.retain(|p| p.name != track);
            if before == bp.drums.len() + bp.parts.len() + bp.loops.len() + bp.patterns.len() {
                return fail(format!("no track '{track}'"));
            }
            for s in bp.form.iter_mut() {
                s.layers.retain(|l| l != track);
            }
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("master", move |m: Map| -> R<()> {
        check_keys(&m, "master()", &["lufs", "ceiling"])?;
        with_bp(&c, |bp, _| {
            if let Some(v) = map_f32(&m, "lufs") {
                bp.mix.master.lufs = v.clamp(-24.0, -5.0);
            }
            if let Some(v) = map_f32(&m, "ceiling") {
                bp.mix.master.ceiling_db = v.clamp(-3.0, -0.1);
            }
            Ok(())
        })
    });
}

/// a part the genre has, added to a song that does not have it yet (a blank song, say)
fn add_genre_part(bp: &mut Blueprint, name: &str) -> R<()> {
    let g = genre_of(bp);
    let Some(spec) = g.as_ref().and_then(|g| g.parts.iter().find(|p| p.name == name)) else {
        let have: Vec<&str> = bp.parts.iter().map(|p| p.name.as_str()).collect();
        let genre: Vec<&str> =
            g.as_ref().map(|g| g.parts.iter().map(|p| p.name.as_str()).collect()).unwrap_or_default();
        return fail(format!(
            "no part '{name}' (the song has: {}; {} parts: {}); track() writes your own",
            if have.is_empty() { "none".into() } else { have.join(" ") },
            bp.genre,
            genre.join(" ")
        ));
    };
    let mut rng = crate::rng::Rng::new(bp.seed ^ crate::rng::hash_str(name));
    bp.parts.push(compose::new_part(spec, &mut rng));
    let any = bp.form.iter().any(|s| !s.layers.is_empty());
    for s in bp.form.iter_mut() {
        if !any || s.energy >= 0.6 {
            s.layers.push(name.to_string());
        }
    }
    Ok(())
}

pub fn mix_of<'a>(bp: &'a mut Blueprint, name: &str) -> Option<&'a mut TrackMix> {
    if let Some(d) = bp.drums.iter_mut().find(|d| d.name == name) {
        return Some(&mut d.mix);
    }
    if let Some(p) = bp.parts.iter_mut().find(|p| p.name == name) {
        return Some(&mut p.mix);
    }
    if let Some(l) = bp.loops.iter_mut().find(|l| l.name == name) {
        return Some(&mut l.mix);
    }
    if let Some(p) = bp.patterns.iter_mut().find(|p| p.name == name) {
        return Some(&mut p.mix);
    }
    if name == "fx" || name == "transitions" {
        return Some(&mut bp.transitions.mix);
    }
    None
}
