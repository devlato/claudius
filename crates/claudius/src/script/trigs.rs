//! The drum machine in scripts: `trig(lane, [variant,] step, #{ ... })` sets what a step does beyond striking
//! (`rhythm::Trig`), `lane(lane, #{ ... })` a lane's own time and feel, and `groove(name, ...)` a groove heard in a
//! recording or written as a table; the printed script writes them all back.

use rhai::Map;

use super::song::{check_keys, map_f32, map_str};
use super::{R, fail};
use crate::model::Blueprint;
use crate::theory::rhythm::{Cond, Hit, Trig};

/// what a trig's map may hold
pub(super) const KEYS: &[&str] =
    &["v", "prob", "cond", "ratchet", "ramp", "flam", "nudge", "pitch", "decay", "tone", "pan"];

/// Sets the trig of step `step` (sixteenths from the pattern's start; a fraction for a 32nd or a triplet) of lane
/// `lane`'s variant `variant` from `m`, on the trig there (an empty map clears it). A step without a hit gets one, of
/// velocity `v` (0.8 when not given). Returns what it did, in words.
pub(super) fn set(bp: &mut Blueprint, lane: &str, variant: &str, step: f64, m: &Map) -> R<String> {
    check_keys(m, "trig()", KEYS)?;
    let lanes: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    let Some(l) = bp.drums.iter_mut().find(|d| d.name == lane) else {
        return fail(format!("trig(): no drum lane '{lane}' (lanes: {})", lanes.join(" ")));
    };
    let len = l.pattern_bars as f64 * 16.0;
    if !(0.0..len).contains(&step) {
        return fail(format!("trig(): {lane}'s steps go from 0 to {} (sixteenths), not {step}", len - 1.0));
    }
    let variants: Vec<String> = l.patterns.keys().cloned().collect();
    let Some(hits) = l.patterns.get_mut(variant) else {
        return fail(format!("trig(): {lane} has no variant '{variant}' (variants: {})", variants.join(" ")));
    };
    let t = step * 0.25;
    let at = match hits.iter().position(|h| (h.t - t).abs() < 1e-6) {
        Some(i) => i,
        // nothing to clear on a step without a hit
        None if m.is_empty() => {
            return Ok(format!("{lane} {variant} step {}: no hit, no trig", super::print::number(step)));
        }
        None => {
            hits.push(Hit::new(t, 0.8));
            hits.sort_by(|a, b| a.t.total_cmp(&b.t));
            hits.iter().position(|h| (h.t - t).abs() < 1e-6).unwrap_or(0)
        }
    };
    let h = &mut hits[at];
    if let Some(v) = map_f32(m, "v") {
        if !(0.05..=1.0).contains(&v) {
            return fail(format!("trig()'s v is a velocity from 0.05 to 1, not {v}"));
        }
        h.v = v;
    }
    let mut tr = if m.is_empty() { Trig::default() } else { h.trig.unwrap_or_default() };
    let range = |k: &str, lo: f32, hi: f32| -> R<Option<f32>> {
        match map_f32(m, k) {
            Some(v) if !(lo..=hi).contains(&v) => fail(format!("trig()'s {k} is {lo} to {hi}, not {v}")),
            v => Ok(v),
        }
    };
    if let Some(v) = range("prob", 0.0, 1.0)? {
        tr.prob = v;
    }
    if let Some(c) = map_str(m, "cond") {
        tr.cond = Cond::try_from(c).map_err(|e| format!("trig(): {e}"))?;
    }
    if let Some(v) = range("ratchet", 1.0, 8.0)? {
        tr.ratchet = v.round() as u8;
    }
    if let Some(v) = range("ramp", -1.0, 1.0)? {
        tr.ramp = v;
    }
    if let Some(v) = range("flam", 0.0, 60.0)? {
        tr.flam = v;
    }
    if let Some(v) = range("nudge", -0.5, 0.5)? {
        tr.nudge = v;
    }
    if let Some(v) = range("pitch", -24.0, 24.0)? {
        tr.pitch = v.round() as i8;
    }
    if let Some(v) = range("decay", 0.1, 4.0)? {
        tr.decay = v;
    }
    if let Some(v) = range("tone", -1.0, 1.0)? {
        tr.tone = v;
    }
    if let Some(v) = range("pan", -1.0, 1.0)? {
        tr.pan = v;
    }
    h.trig = (tr != Trig::default()).then_some(tr);
    let step = super::print::number(step);
    Ok(match h.trig {
        Some(tr) => format!("{lane} {variant} step {step}: {}", words(&tr)),
        None => format!("{lane} {variant} step {step}: plays as the lane does"),
    })
}

/// a trig in words: "50%, 1:2, ratchet 3 swelling, flam 20 ms, 0.1 late, +2 st, decay x0.5, darker, pan right"
pub fn words(tr: &Trig) -> String {
    let mut w = Vec::new();
    if tr.prob < 1.0 {
        w.push(format!("{:.0}% of the time", tr.prob * 100.0));
    }
    if !tr.cond.is_always() {
        w.push(format!("only {}", String::from(tr.cond)));
    }
    if tr.ratchet > 1 {
        let how = match tr.ramp {
            r if r > 0.0 => " swelling",
            r if r < 0.0 => " fading",
            _ => "",
        };
        w.push(format!("{} strokes{how}", tr.ratchet));
    }
    if tr.flam > 0.0 {
        w.push(format!("a flam {:.0} ms before", tr.flam));
    }
    if tr.nudge != 0.0 {
        w.push(format!("{:.2} of a step {}", tr.nudge.abs(), if tr.nudge < 0.0 { "early" } else { "late" }));
    }
    if tr.pitch != 0 {
        w.push(format!("{:+} semitones", tr.pitch));
    }
    if tr.decay != 1.0 {
        w.push(format!("decay x{}", super::print::number(tr.decay as f64)));
    }
    if tr.tone != 0.0 {
        w.push(if tr.tone < 0.0 { "darker".into() } else { "brighter".into() });
    }
    if tr.pan != 0.0 {
        w.push(format!("pan {}", super::print::number(tr.pan as f64)));
    }
    w.join(", ")
}

/// the hits `new`, each with the trig of the hit of `old` on its step, if that had one: a pattern written again keeps
/// what its steps that still strike do
pub(super) fn carry(old: Option<&Vec<Hit>>, new: Vec<Hit>) -> Vec<Hit> {
    let Some(old) = old else { return new };
    new.into_iter()
        .map(|h| match old.iter().find(|o| (o.t - h.t).abs() < 1e-6 && o.trig.is_some()) {
            Some(o) => Hit { trig: o.trig, ..h },
            None => h,
        })
        .collect()
}

/// the `trig()` lines that set a lane variant's trigs again
pub(super) fn lines(lane: &str, variant: &str, hits: &[Hit]) -> Vec<String> {
    use crate::pattern::quote;
    hits.iter()
        .filter_map(|h| {
            let tr = h.trig?;
            let d = Trig::default();
            let (n, f) = (super::print::number, super::print::f32s);
            let mut parts = Vec::new();
            if tr.prob != d.prob {
                parts.push(format!("prob: {}", f(tr.prob)));
            }
            if tr.cond != d.cond {
                parts.push(format!("cond: {}", quote(&String::from(tr.cond))));
            }
            if tr.ratchet != d.ratchet {
                parts.push(format!("ratchet: {}", tr.ratchet));
            }
            if tr.ramp != d.ramp {
                parts.push(format!("ramp: {}", f(tr.ramp)));
            }
            if tr.flam != d.flam {
                parts.push(format!("flam: {}", f(tr.flam)));
            }
            if tr.nudge != d.nudge {
                parts.push(format!("nudge: {}", f(tr.nudge)));
            }
            if tr.pitch != d.pitch {
                parts.push(format!("pitch: {}", tr.pitch));
            }
            if tr.decay != d.decay {
                parts.push(format!("decay: {}", f(tr.decay)));
            }
            if tr.tone != d.tone {
                parts.push(format!("tone: {}", f(tr.tone)));
            }
            if tr.pan != d.pan {
                parts.push(format!("pan: {}", f(tr.pan)));
            }
            Some(format!("trig({}, {}, {}, #{{ {} }});", quote(lane), quote(variant), n(h.t * 4.0), parts.join(", ")))
        })
        .collect()
}

/// what `lane()`'s map may hold
pub(super) const LANE_KEYS: &[&str] = &["length", "speed", "swing", "groove", "groove_amount"];

/// Sets drum lane `lane`'s own time and feel from `m`: its `length` in sixteenths (0: its pattern's bars), its
/// `speed`, its `swing` (0: the song's), its `groove` (a name from `rhythm::GROOVES` or the song's grooves; "" none)
/// and how much of it, `groove_amount`. Returns what it did, in words.
pub(super) fn set_lane(bp: &mut Blueprint, lane: &str, m: &Map) -> R<String> {
    use crate::theory::rhythm::GROOVES;
    check_keys(m, "lane()", LANE_KEYS)?;
    let lanes: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    let grooves: Vec<String> = GROOVES.iter().map(|g| g.to_string()).chain(bp.grooves.keys().cloned()).collect();
    let Some(l) = bp.drums.iter_mut().find(|d| d.name == lane) else {
        return fail(format!("lane(): no drum lane '{lane}' (lanes: {})", lanes.join(" ")));
    };
    let range = |k: &str, lo: f32, hi: f32| -> R<Option<f32>> {
        match map_f32(m, k) {
            Some(v) if !(lo..=hi).contains(&v) => fail(format!("lane()'s {k} is {lo} to {hi}, not {v}")),
            v => Ok(v),
        }
    };
    if let Some(v) = range("length", 0.0, 256.0)? {
        let pattern = l.pattern_bars.max(1) * 16;
        if v.round() as u32 > pattern {
            return fail(format!(
                "lane(): {lane}'s length is up to its pattern's {pattern} steps (a longer pattern first, with drum()), not {v}"
            ));
        }
        l.length = (v >= 1.0).then_some(v.round() as u32);
    }
    if let Some(v) = range("speed", 0.125, 4.0)? {
        l.speed = v;
    }
    if let Some(v) = map_f32(m, "swing") {
        if v != 0.0 && !(0.5..=0.8).contains(&v) {
            return fail(format!("lane()'s swing is 0.5 (straight) to 0.8, or 0 for the song's, not {v}"));
        }
        l.swing = (v != 0.0).then_some(v);
    }
    if let Some(g) = map_str(m, "groove") {
        let g = g.trim().to_string();
        if g.is_empty() || g == "none" {
            l.groove = None;
        } else if grooves.contains(&g) {
            l.groove = Some(g);
        } else {
            return fail(format!("lane(): no groove '{g}' (grooves: {})", grooves.join(" ")));
        }
    }
    if let Some(v) = range("groove_amount", 0.0, 1.0)? {
        l.groove_amount = v;
    }
    Ok(format!("{lane}: {}", lane_words(l)))
}

/// a lane's own time and feel in words: "12 steps round, at 0.75 speed, swing 0.6, in the dilla groove (80%)", or
/// that it keeps the song's
pub fn lane_words(l: &crate::model::DrumLane) -> String {
    let mut w = Vec::new();
    if let Some(n) = l.length {
        w.push(format!("{n} steps round"));
    }
    if l.speed != 1.0 {
        w.push(format!("at {} speed", super::print::number(l.speed as f64)));
    }
    if let Some(s) = l.swing {
        w.push(format!("swing {}", super::print::number(s as f64)));
    }
    if let Some(g) = &l.groove {
        let amount = if l.groove_amount < 1.0 { format!(" ({:.0}%)", l.groove_amount * 100.0) } else { String::new() };
        w.push(format!("in the {g} groove{amount}"));
    }
    if w.is_empty() { "on the song's time and feel".into() } else { w.join(", ") }
}

/// a groove as a script writes it: `#{ timing: [16 beats], accent: [16 factors] }`
pub(super) fn groove_of(m: &Map) -> R<crate::theory::rhythm::Groove> {
    check_keys(m, "groove()", &["timing", "accent"])?;
    let list = |k: &str, lo: f32, hi: f32, base: f32| -> R<[f32; 16]> {
        let Some(v) = m.get(k) else { return Ok([base; 16]) };
        let Some(a) = v.clone().try_cast::<rhai::Array>() else {
            return fail(format!("groove()'s {k} is a list of 16 numbers"));
        };
        if a.len() != 16 {
            return fail(format!("groove()'s {k} is a list of 16 numbers, one a sixteenth, not {}", a.len()));
        }
        let mut out = [base; 16];
        for (o, d) in out.iter_mut().zip(&a) {
            let x = super::num(d).ok_or_else(|| format!("groove()'s {k} is a list of numbers"))? as f32;
            if !(lo..=hi).contains(&x) {
                return fail(format!("groove()'s {k} holds {lo} to {hi}, not {x}"));
            }
            *o = x;
        }
        Ok(out)
    };
    Ok(crate::theory::rhythm::Groove {
        timing: list("timing", -0.25, 0.25, 0.0)?,
        accent: list("accent", 0.0, 2.0, 1.0)?,
    })
}

/// the `groove()` lines that set the song's grooves again, as tables (they need no recording to read back)
pub(super) fn groove_lines(bp: &Blueprint) -> Vec<String> {
    use crate::pattern::quote;
    let n = |v: &f32| super::print::f32s(*v);
    bp.grooves
        .iter()
        .map(|(name, g)| {
            let timing: Vec<String> = g.timing.iter().map(n).collect();
            let accent: Vec<String> = g.accent.iter().map(n).collect();
            format!("groove({}, #{{ timing: [{}], accent: [{}] }});", quote(name), timing.join(", "), accent.join(", "))
        })
        .collect()
}

/// the `lane()` line that sets a lane's own time and feel again, when it has any
pub(super) fn lane_line(l: &crate::model::DrumLane) -> Option<String> {
    use crate::pattern::quote;
    let n = super::print::f32s;
    let mut parts = Vec::new();
    if let Some(len) = l.length {
        parts.push(format!("length: {len}"));
    }
    if l.speed != 1.0 {
        parts.push(format!("speed: {}", n(l.speed)));
    }
    if let Some(s) = l.swing {
        parts.push(format!("swing: {}", n(s)));
    }
    if let Some(g) = &l.groove {
        parts.push(format!("groove: {}", quote(g)));
    }
    if l.groove_amount != 1.0 {
        parts.push(format!("groove_amount: {}", n(l.groove_amount)));
    }
    (!parts.is_empty()).then(|| format!("lane({}, #{{ {} }});", quote(&l.name), parts.join(", ")))
}
