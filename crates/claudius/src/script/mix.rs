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
        check_keys(
            &m,
            "part()",
            &["role", "style", "preset", "register", "density", "seed", "rate", "doubles", "harmonizes", "octave", "late"],
        )?;
        let late = match map_f32(&m, "late") {
            Some(v) if !(-super::trigs::MOST_LATE..=super::trigs::MOST_LATE).contains(&v) => {
                let most = super::trigs::MOST_LATE;
                return fail(format!("part()'s late is -{most} (ahead) to {most} ms (behind), not {v}"));
            }
            v => v,
        };
        with_bp(&c, |bp, _| {
            // "" leaves it playing its own notes
            let doubles = map_str(&m, "doubles").map(|d| Some(d).filter(|d| !d.is_empty()));
            if let Some(Some(d)) = &doubles {
                match bp.parts.iter().find(|p| p.name == *d) {
                    _ if d == name => return fail(format!("part '{name}' cannot double itself")),
                    None => return fail(format!("part '{name}' doubles '{d}', which is not a part of the song")),
                    Some(p) if p.doubles.is_some() => {
                        return fail(format!("part '{name}' doubles '{d}', which doubles another part: double that one"));
                    }
                    Some(_) => {}
                }
                if let Some(by) = bp.parts.iter().find(|p| p.doubles.as_deref() == Some(name)) {
                    return fail(format!("part '{name}' is doubled by '{}', so it cannot double another", by.name));
                }
            }
            // "" leaves it playing chords of its own
            let harmonizes = map_str(&m, "harmonizes").map(|h| Some(h).filter(|h| !h.is_empty()));
            if let Some(Some(h)) = &harmonizes {
                match bp.parts.iter().find(|p| p.name == *h) {
                    _ if h == name => return fail(format!("part '{name}' cannot harmonize itself")),
                    None => return fail(format!("part '{name}' harmonizes '{h}', which is not a part of the song")),
                    Some(p) if p.harmonizes.is_some() => {
                        return fail(format!(
                            "part '{name}' harmonizes '{h}', which harmonizes another part: harmonize the melody itself"
                        ));
                    }
                    Some(_) => {}
                }
                if let Some(by) = bp.parts.iter().find(|p| p.harmonizes.as_deref() == Some(name)) {
                    return fail(format!("part '{name}' is harmonized by '{}', so it cannot harmonize another", by.name));
                }
            }
            let doubling = match &doubles {
                Some(d) => d.is_some(),
                None => bp.parts.iter().any(|p| p.name == name && p.doubles.is_some()),
            };
            let harmonizing = match &harmonizes {
                Some(h) => h.is_some(),
                None => bp.parts.iter().any(|p| p.name == name && p.harmonizes.is_some()),
            };
            if doubling && harmonizing {
                return fail(format!("part '{name}' doubles another's notes or harmonizes them, not both"));
            }
            let octave = match m.get("octave") {
                None => None,
                Some(o) => match o.as_int() {
                    Ok(o) if (-2..=2).contains(&o) => Some(o as i32),
                    _ => return fail(format!("part(): an octave is a whole number from -2 to 2, not {o}")),
                },
            };
            let role = match map_str(&m, "role") {
                Some(r) => Some(Role::parse(&r).ok_or_else(|| {
                    format!("part(): a role is bass, chords, pad, keys, lead or arp, not '{r}'")
                })?),
                None => None,
            };
            match bp.parts.iter().find(|p| p.name == name) {
                None => add_part(bp, name, role)?,
                Some(p) if role.is_some_and(|r| r != p.role) => {
                    return fail(format!(
                        "part '{name}' is a {} part; remove(\"{name}\") first to make it another",
                        p.role.name()
                    ));
                }
                Some(_) => {}
            }
            let part = bp.parts.iter_mut().find(|p| p.name == name).expect("added above");
            if let Some(h) = harmonizes {
                if h.is_some() && !matches!(part.role, Role::Chords | Role::Pad | Role::Keys) {
                    return fail(format!(
                        "part '{name}' is a {} part: a part that harmonizes plays chords, as a chords, pad or keys part",
                        part.role.name()
                    ));
                }
                // chords of its own play in a chord part's rhythm, not under a melody's notes
                if h.is_none() && part.style == "block" {
                    part.style = "sustain".into();
                }
                part.harmonizes = h;
            }
            if let Some(s) = map_str(&m, "style") {
                let styles = if part.harmonizes.is_some() {
                    crate::theory::harmonize::STYLES.to_vec()
                } else {
                    compose::styles_for(part.role)
                };
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
            if let Some(d) = doubles {
                part.doubles = d;
            }
            if let Some(o) = octave {
                part.octave = o;
            }
            if let Some(ms) = late {
                part.late_ms = ms;
            }
            Ok(())
        })
    });
    // notes(part, prog, [[t, d, pitch, velocity], ...]): what a part plays over a progression, written out
    let c = ctx.clone();
    engine.register_fn("notes", move |name: &str, prog: &str, list: Array| -> R<()> {
        let notes = list.iter().enumerate().map(|(i, n)| read_note(i, n)).collect::<Result<Vec<_>, String>>()?;
        if notes.len() > MAX_WRITTEN {
            return fail(format!("notes(): {} notes in one progression; the limit is {MAX_WRITTEN}", notes.len()));
        }
        with_bp(&c, |bp, _| {
            check_progression(bp, "notes()", prog)?;
            let part = written_part(bp, "notes()", name)?;
            let mut notes = notes;
            notes.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.p.cmp(&b.p)));
            part.notes.insert(prog.to_string(), notes);
            Ok(())
        })
    });
    // clear_notes(part, prog): the theory engine writes that progression again
    let c = ctx.clone();
    engine.register_fn("clear_notes", move |name: &str, prog: &str| -> R<()> {
        with_bp(&c, |bp, _| {
            check_progression(bp, "clear_notes()", prog)?;
            written_part(bp, "clear_notes()", name)?.notes.remove(prog);
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
                let drum_sound = matches!(sound, Sound::Drum { .. } | Sound::Sample { .. } | Sound::Instrument { .. });
                if !bp.drums.iter().any(|d| d.name == lane) {
                    // a lane of any name, playing this sound, silent until drum() writes its steps
                    if bp.layer_names().iter().any(|n| n == lane) {
                        return fail(format!("'{lane}' is a track of the song that is not a drum lane"));
                    }
                    if !drum_sound {
                        return fail(format!("'{label}' is not a drum sound (a drum name, a kit sound, or a sample)"));
                    }
                    bp.drums.push(DrumLane {
                        late_ms: 0.0,
                        length: None,
                        speed: 1.0,
                        swing: None,
                        groove: None,
                        groove_amount: 1.0,
                        name: lane.to_string(),
                        sound,
                        patterns: [("main".to_string(), Vec::new())].into(),
                        pattern_bars: 1,
                        pitch: 0,
                        choke: None,
                        mix: TrackMix { bus: "drums".into(), ..Default::default() },
                    });
                    super::song::place_new(bp, lane);
                    return Ok(());
                }
                let l = bp.drums.iter_mut().find(|d| d.name == lane).expect("found above");
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
        check_keys(&m, "mix()", &["level", "pan", "reverb", "room", "delay", "duck", "drive", "mute", "solo", "bus"])?;
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
            if let Some(v) = map_f32(&m, "drive") {
                mix.drive_db = v.clamp(0.0, crate::model::MAX_DRIVE_DB);
            }
            if let Some(v) = m.get("mute").and_then(|d| d.as_bool().ok()) {
                mix.mute = v;
            }
            if let Some(v) = m.get("solo").and_then(|d| d.as_bool().ok()) {
                mix.solo = v;
            }
            if let Some(b) = map_str(&m, "bus") {
                if b.is_empty() || !b.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
                    return fail(format!("a bus is named with letters, digits, - and _, not '{b}'"));
                }
                mix.bus = b;
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
            // a part that doubled or harmonized it plays its own from now on
            for p in bp.parts.iter_mut() {
                if p.doubles.as_deref() == Some(track) {
                    p.doubles = None;
                }
                if p.harmonizes.as_deref() == Some(track) {
                    p.harmonizes = None;
                    if p.style == "block" {
                        p.style = "sustain".into();
                    }
                }
            }
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("master", move |m: Map| -> R<()> {
        check_keys(&m, "master()", &["lufs", "ceiling", "fx"])?;
        let fx = match m.get("fx") {
            Some(d) => Some(super::effects::chain_of(
                &d.clone().try_cast::<Array>().ok_or("master()'s fx is a list of effects")?,
                "master()'s fx",
            )?),
            None => None,
        };
        with_bp(&c, |bp, _| {
            if let Some(fx) = fx {
                bp.mix.master.fx = fx;
            }
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

/// the most notes one progression of a part may have written out
const MAX_WRITTEN: usize = 4096;

/// note `i` of a `notes()` list: `[t, d, pitch]` or `[t, d, pitch, velocity]`
fn read_note(i: usize, n: &rhai::Dynamic) -> Result<crate::theory::Note, String> {
    let bad = || {
        format!(
            "notes(): note {} is [start, length, pitch] or [start, length, pitch, velocity]: beats from the \
             progression's start, beats long, a MIDI pitch from 0 to 127 (60 is middle C) and a velocity from 0 to 1",
            i + 1
        )
    };
    let a = n.clone().try_cast::<Array>().ok_or_else(bad)?;
    let x: Vec<f64> = a.iter().map(num).collect::<Option<Vec<f64>>>().ok_or_else(bad)?;
    let (t, d, p, v) = match x.as_slice() {
        [t, d, p] => (*t, *d, *p, 0.8),
        [t, d, p, v] => (*t, *d, *p, *v),
        _ => return Err(bad()),
    };
    if t < 0.0 || d <= 0.0 || p != p.trunc() || !(0.0..=127.0).contains(&p) || !(0.0..=1.0).contains(&v) {
        return Err(bad());
    }
    Ok(crate::theory::Note { t, d, p: p as i32, v: v as f32, lock: None })
}

fn check_progression(bp: &Blueprint, f: &str, prog: &str) -> R<()> {
    if bp.harmony.progressions.contains_key(prog) {
        return Ok(());
    }
    let names: Vec<&str> = bp.harmony.progressions.keys().map(String::as_str).collect();
    fail(format!("{f}: no progression '{prog}' (progressions: {})", names.join(" ")))
}

fn written_part<'a>(bp: &'a mut Blueprint, f: &str, name: &str) -> Result<&'a mut Part, Box<rhai::EvalAltResult>> {
    if !bp.parts.iter().any(|p| p.name == name) {
        let names: Vec<&str> = bp.parts.iter().map(|p| p.name.as_str()).collect();
        return Err(format!(
            "{f}: no part '{name}' (parts: {})",
            if names.is_empty() { "none".into() } else { names.join(" ") }
        )
        .into());
    }
    Ok(bp.parts.iter_mut().find(|p| p.name == name).expect("found above"))
}

/// a part added to a song that does not have it yet: the genre's part of that name (a blank song's, say), or, with a
/// role, a part of the role written the way the genre writes it (its part of that role, renamed) or, when the genre
/// has none, in every style of the role
fn add_part(bp: &mut Blueprint, name: &str, role: Option<Role>) -> R<()> {
    // one track to a name: a drum lane, a loop or a pattern track of this name is not to be shadowed
    if bp.layer_names().iter().any(|n| n == name) {
        return fail(format!("'{name}' is a track of the song that is not a part: give the part another name"));
    }
    let g = genre_of(bp);
    let Some(genre) = g.as_ref() else { return fail(format!("part(): the song's genre '{}' is unknown", bp.genre)) };
    let spec = match (genre.parts.iter().find(|p| p.name == name), role) {
        (Some(s), Some(r)) if s.role != r => {
            return fail(format!(
                "{} has a part '{name}' that is a {}, not a {}; give yours another name",
                bp.genre,
                s.role.name(),
                r.name()
            ));
        }
        (Some(s), _) => s.clone(),
        (None, Some(r)) => match genre.parts.iter().find(|p| p.role == r) {
            Some(s) => {
                crate::genres::PartSpec { name: name.into(), optional: 0.0, doubles: None, octave: 0, ..s.clone() }
            }
            None => default_spec(name, r),
        },
        (None, None) => {
            let have: Vec<&str> = bp.parts.iter().map(|p| p.name.as_str()).collect();
            let theirs: Vec<&str> = genre.parts.iter().map(|p| p.name.as_str()).collect();
            let like = Role::parse(name).map_or("lead", |r| r.name());
            return fail(format!(
                "no part '{name}' (the song has: {}; {} parts: {}); a role makes one of your own \
                 (part(\"{name}\", #{{ role: \"{like}\" }})), and track() writes your own notes",
                if have.is_empty() { "none".into() } else { have.join(" ") },
                bp.genre,
                theirs.join(" ")
            ));
        }
    };
    let mut rng = crate::rng::Rng::new(bp.seed ^ crate::rng::hash_str(name));
    let mut used = bp.parts.iter().filter_map(|p| crate::palette::identity(&p.sound)).collect();
    bp.parts.push(compose::new_part(&spec, genre, &crate::palette::instruments(), &mut used, &mut rng));
    super::song::place_new(bp, name);
    Ok(())
}

/// a part of `role` for a genre that has none: its role's presets and styles, in the role's usual register
fn default_spec(name: &str, role: Role) -> crate::genres::PartSpec {
    let register = match role {
        Role::Bass => (28, 52),
        Role::Pad => (48, 72),
        Role::Chords | Role::Keys => (52, 76),
        Role::Lead | Role::Arp => (60, 86),
    };
    crate::genres::PartSpec {
        name: name.into(),
        role,
        presets: presets::drawable(role.name()).into_iter().map(String::from).collect(),
        // a bass of no genre plays the motifs, not another genre's idiom
        styles: compose::styles_for(role)
            .into_iter()
            .filter(|s| crate::theory::bass::idiom(s).is_none())
            .map(String::from)
            .collect(),
        register,
        density: (0.4, 0.7),
        voices: if matches!(role, Role::Chords | Role::Pad | Role::Keys) { 4 } else { 0 },
        voicing: crate::theory::chord::Voicing::Close,
        rates: if role == Role::Arp { vec![0.25] } else { Vec::new() },
        optional: 0.0,
        core: None,
        mix: crate::model::TrackMix::default(),
        doubles: None,
        octave: 0,
        late_ms: None,
    }
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
