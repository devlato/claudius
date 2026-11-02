//! Trigs in scripts: `trig(lane, [variant,] step, #{ ... })` sets what a step does beyond striking (`rhythm::Trig`),
//! and the printed script writes them back.

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
            let n = super::print::number;
            let mut parts = Vec::new();
            if tr.prob != d.prob {
                parts.push(format!("prob: {}", n(tr.prob as f64)));
            }
            if tr.cond != d.cond {
                parts.push(format!("cond: {}", quote(&String::from(tr.cond))));
            }
            if tr.ratchet != d.ratchet {
                parts.push(format!("ratchet: {}", tr.ratchet));
            }
            if tr.ramp != d.ramp {
                parts.push(format!("ramp: {}", n(tr.ramp as f64)));
            }
            if tr.flam != d.flam {
                parts.push(format!("flam: {}", n(tr.flam as f64)));
            }
            if tr.nudge != d.nudge {
                parts.push(format!("nudge: {}", n(tr.nudge as f64)));
            }
            if tr.pitch != d.pitch {
                parts.push(format!("pitch: {}", tr.pitch));
            }
            if tr.decay != d.decay {
                parts.push(format!("decay: {}", n(tr.decay as f64)));
            }
            if tr.tone != d.tone {
                parts.push(format!("tone: {}", n(tr.tone as f64)));
            }
            if tr.pan != d.pan {
                parts.push(format!("pan: {}", n(tr.pan as f64)));
            }
            Some(format!("trig({}, {}, {}, #{{ {} }});", quote(lane), quote(variant), n(h.t * 4.0), parts.join(", ")))
        })
        .collect()
}
