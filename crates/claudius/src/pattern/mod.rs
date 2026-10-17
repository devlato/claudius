//! Patterns, Strudel / TidalCycles style: a pattern is queried for the events that start inside a span of cycles
//! (one cycle = one bar). Built from mini-notation (`mini`) and transformed with methods (fast, every, sometimes,
//! struct, ...). A `Pattern` also remembers the code that built it (`repr`), so songs print back as scripts.

pub mod mini;
pub mod notes;

use std::cell::Cell;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

const EPS: f64 = 1e-9;

/// work one query may do (cycles visited plus events made) before it stops; a pattern that needs more is far too
/// dense to be music, and without a cap `s("bd*1000000000")` would take forever
pub const MAX_WORK: u64 = 4_000_000;
/// events one query may make, which bounds its memory; a track holds at most 1024 notes a bar over 64 bars, a
/// quarter of this
pub const MAX_EVENTS: u64 = 1 << 18;
/// the most `*`, `/`, `fast`, `slow` and `ply` may speed up or slow down a pattern
pub const MAX_SPEED: f64 = 256.0;

thread_local! {
    static WORK: Cell<u64> = const { Cell::new(u64::MAX) };
    static EVENTS: Cell<u64> = const { Cell::new(u64::MAX) };
}

/// charge one unit of work to the query in progress; false once its budget is spent
fn spend() -> bool {
    WORK.with(|w| match w.get() {
        0 => false,
        n => {
            w.set(n - 1);
            true
        }
    })
}

/// count one new event; false once the query has made too many (and then the work stops too)
fn make() -> bool {
    EVENTS.with(|n| match n.get() {
        0 => {
            WORK.with(|w| w.set(0));
            false
        }
        k => {
            n.set(k - 1);
            true
        }
    })
}

/// a speed factor within reach of music
pub fn check_speed(f: f64, what: &str) -> Result<f64, String> {
    if f > 0.0 && f <= MAX_SPEED {
        Ok(f)
    } else {
        Err(format!("{what} takes a number above 0 and up to {MAX_SPEED}, not {}", fmt_num(f)))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Num(f64),
    Word(String),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Num(n) => *n != 0.0,
            Value::Word(w) => !matches!(w.as_str(), "0" | "false" | "f" | "~" | "-"),
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Word(w) => w.parse().ok(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CtrlKey {
    Gain,
    Velocity,
    Legato,
    /// added to the value: scale degrees for `n()`, semitones for `note()`
    Add,
    /// always semitones
    Transpose,
    Index,
}

/// per-event controls set by methods like .gain() / .legato() / .add()
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ctrl {
    pub gain: Option<f64>,
    pub velocity: Option<f64>,
    pub legato: Option<f64>,
    pub add: f64,
    pub transpose: f64,
    pub index: Option<i64>,
}

/// one event: whole span [b, e) in cycles, a value and its controls
#[derive(Clone, Debug, PartialEq)]
pub struct Ev {
    pub b: f64,
    pub e: f64,
    pub v: Value,
    pub ctrl: Ctrl,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Silence,
    Atom(Value),
    /// subdivide each cycle by weights
    Seq(Vec<(Node, f64)>),
    Stack(Vec<Node>),
    /// one child per cycle, in turn
    Alt(Vec<Node>),
    Fast(Box<Node>, f64),
    Late(Box<Node>, f64),
    Rev(Box<Node>),
    /// play the second node on every n-th cycle (cycle % n == offset)
    Every(u32, u32, Box<Node>, Box<Node>),
    Degrade(Box<Node>, f64, u64),
    Choose(Vec<Node>, u64),
    Ply(Box<Node>, u32),
    /// events (timing) of the second node, values of the first
    Struct(Box<Node>, Box<Node>),
    Mask(Box<Node>, Box<Node>),
    Ctrl(Box<Node>, CtrlKey, Box<Node>),
    /// per event: the second node's events instead with probability p
    /// the original events not chosen, plus the transformed chosen ones (the transform got `Keep` of the original)
    Sometimes(Box<Node>, Box<Node>, f64, u64),
    /// the events `Degrade` with the same probability and seed would drop: what `sometimes` hands its transform
    Keep(Box<Node>, f64, u64),
}

fn hash01(seed: u64, t: f64) -> f64 {
    let mut x = seed ^ ((t * 1_000_003.0).round() as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^= x >> 33;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn cycles(b: f64, e: f64) -> std::ops::Range<i64> {
    (b + EPS).floor() as i64..(e - EPS).ceil().max((b + EPS).floor() + 1.0) as i64
}

/// the longest an event of `n` can last, in cycles (an upper bound)
fn reach(n: &Node) -> f64 {
    match n {
        Node::Silence => 0.0,
        Node::Atom(_) => 1.0,
        Node::Seq(items) => {
            let w: f64 = items.iter().map(|i| i.1).sum::<f64>().max(EPS);
            items.iter().map(|(c, wi)| reach(c) * wi / w).fold(0.0, f64::max)
        }
        Node::Stack(items) | Node::Alt(items) | Node::Choose(items, _) => items.iter().map(reach).fold(0.0, f64::max),
        Node::Fast(c, f) => reach(c) / f.max(EPS),
        Node::Every(_, _, a, b) | Node::Sometimes(a, b, ..) => reach(a).max(reach(b)),
        Node::Struct(_, c) => reach(c),
        Node::Late(c, _)
        | Node::Rev(c)
        | Node::Degrade(c, ..)
        | Node::Keep(c, ..)
        | Node::Ply(c, _)
        | Node::Mask(c, _)
        | Node::Ctrl(c, ..) => reach(c),
    }
}

/// the event of `n` sounding at time t (whole span contains t), if any; it may have started cycles earlier
fn sample_at(n: &Node, t: f64) -> Option<Ev> {
    let c = (t + EPS).floor();
    let back = (reach(n).ceil() - 1.0).clamp(0.0, 64.0);
    let mut evs = Vec::new();
    query(n, c - back, c + 1.0, &mut evs);
    evs.into_iter().filter(|ev| ev.b <= t + EPS && t < ev.e - EPS).max_by(|a, b| a.b.total_cmp(&b.b))
}

/// events of `n` whose onset lies in [b, e)
pub fn query(n: &Node, b: f64, e: f64, out: &mut Vec<Ev>) {
    if e <= b + EPS || !spend() {
        return;
    }
    match n {
        Node::Silence => {}
        Node::Atom(v) => {
            for c in cycles(b, e) {
                if !spend() {
                    return;
                }
                let s = c as f64;
                if s >= b - EPS && s < e - EPS {
                    if !make() {
                        return;
                    }
                    out.push(Ev { b: s, e: s + 1.0, v: v.clone(), ctrl: Ctrl::default() });
                }
            }
        }
        Node::Seq(items) => {
            let w: f64 = items.iter().map(|i| i.1).sum::<f64>().max(EPS);
            for c in cycles(b, e) {
                if !spend() {
                    return;
                }
                let cf = c as f64;
                let mut off = 0.0;
                for (child, wi) in items {
                    let start = cf + off / w;
                    let len = wi / w;
                    off += wi;
                    let (qb, qe) = (b.max(start), e.min(start + len));
                    if qe <= qb + EPS {
                        continue;
                    }
                    let mut tmp = Vec::new();
                    query(child, cf + (qb - start) / len, cf + (qe - start) / len, &mut tmp);
                    for mut ev in tmp {
                        ev.b = start + (ev.b - cf) * len;
                        ev.e = start + (ev.e - cf) * len;
                        out.push(ev);
                    }
                }
            }
        }
        Node::Stack(items) => {
            for i in items {
                query(i, b, e, out);
            }
        }
        Node::Alt(items) => {
            if items.is_empty() {
                return;
            }
            let k = items.len() as i64;
            for c in cycles(b, e) {
                if !spend() {
                    return;
                }
                let local = c.div_euclid(k) as f64;
                let cf = c as f64;
                let (qb, qe) = (b.max(cf), e.min(cf + 1.0));
                if qe <= qb + EPS {
                    continue;
                }
                let mut tmp = Vec::new();
                query(&items[c.rem_euclid(k) as usize], local + (qb - cf), local + (qe - cf), &mut tmp);
                for mut ev in tmp {
                    ev.b = cf + (ev.b - local);
                    ev.e = cf + (ev.e - local);
                    out.push(ev);
                }
            }
        }
        Node::Fast(child, f) => {
            if *f <= EPS {
                return;
            }
            let mut tmp = Vec::new();
            query(child, b * f, e * f, &mut tmp);
            for mut ev in tmp {
                ev.b /= f;
                ev.e /= f;
                out.push(ev);
            }
        }
        Node::Late(child, t) => {
            let mut tmp = Vec::new();
            query(child, b - t, e - t, &mut tmp);
            for mut ev in tmp {
                ev.b += t;
                ev.e += t;
                out.push(ev);
            }
        }
        Node::Rev(child) => {
            for c in cycles(b, e) {
                if !spend() {
                    return;
                }
                let cf = c as f64;
                let mut tmp = Vec::new();
                query(child, cf, cf + 1.0, &mut tmp);
                for mut ev in tmp {
                    let (nb, ne) = (2.0 * cf + 1.0 - ev.e, 2.0 * cf + 1.0 - ev.b);
                    ev.b = nb.max(cf);
                    ev.e = ne;
                    if ev.b >= b - EPS && ev.b < e - EPS {
                        out.push(ev);
                    }
                }
            }
        }
        Node::Every(n, offset, a, alt) => {
            for c in cycles(b, e) {
                if !spend() {
                    return;
                }
                let cf = c as f64;
                let pick = if c.rem_euclid((*n).max(1) as i64) == *offset as i64 { alt } else { a };
                query(pick, b.max(cf), e.min(cf + 1.0), out);
            }
        }
        Node::Degrade(child, p, seed) => {
            let mut tmp = Vec::new();
            query(child, b, e, &mut tmp);
            out.extend(tmp.into_iter().filter(|ev| hash01(*seed, ev.b) >= *p));
        }
        Node::Choose(items, seed) => {
            if items.is_empty() {
                return;
            }
            for c in cycles(b, e) {
                if !spend() {
                    return;
                }
                let cf = c as f64;
                let i = (hash01(*seed, cf) * items.len() as f64) as usize % items.len();
                query(&items[i], b.max(cf), e.min(cf + 1.0), out);
            }
        }
        Node::Ply(child, k) => {
            let mut tmp = Vec::new();
            query(child, b, e, &mut tmp);
            for ev in tmp {
                let k = (*k).max(1) as f64;
                let len = (ev.e - ev.b) / k;
                for j in 0..k as usize {
                    if !spend() || !make() {
                        return;
                    }
                    out.push(Ev { b: ev.b + j as f64 * len, e: ev.b + (j + 1) as f64 * len, ..ev.clone() });
                }
            }
        }
        Node::Struct(values, structure) => {
            let mut tmp = Vec::new();
            query(structure, b, e, &mut tmp);
            for ev in tmp.into_iter().filter(|ev| ev.v.truthy()) {
                if let Some(v) = sample_at(values, ev.b) {
                    if !make() {
                        return;
                    }
                    out.push(Ev { b: ev.b, e: ev.e, v: v.v, ctrl: v.ctrl });
                }
            }
        }
        Node::Mask(child, mask) => {
            let mut tmp = Vec::new();
            query(child, b, e, &mut tmp);
            out.extend(tmp.into_iter().filter(|ev| sample_at(mask, ev.b).is_some_and(|m| m.v.truthy())));
        }
        Node::Ctrl(child, key, param) => {
            let mut tmp = Vec::new();
            query(child, b, e, &mut tmp);
            for mut ev in tmp {
                if let Some(x) = sample_at(param, ev.b).and_then(|p| p.v.num()) {
                    match key {
                        CtrlKey::Gain => ev.ctrl.gain = Some(x),
                        CtrlKey::Velocity => ev.ctrl.velocity = Some(x),
                        CtrlKey::Legato => ev.ctrl.legato = Some(x),
                        CtrlKey::Add => ev.ctrl.add += x,
                        CtrlKey::Transpose => ev.ctrl.transpose += x,
                        CtrlKey::Index => ev.ctrl.index = Some(x as i64),
                    }
                }
                out.push(ev);
            }
        }
        Node::Sometimes(child, alt, p, seed) => {
            let mut a = Vec::new();
            query(child, b, e, &mut a);
            out.extend(a.into_iter().filter(|ev| hash01(*seed, ev.b) >= *p));
            query(alt, b, e, out);
        }
        Node::Keep(child, p, seed) => {
            let mut tmp = Vec::new();
            query(child, b, e, &mut tmp);
            out.extend(tmp.into_iter().filter(|ev| hash01(*seed, ev.b) < *p));
        }
    }
}

/// what a pattern's values mean
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// sound names: s("bd sd")
    Sound,
    /// notes: note("c3 e3") or MIDI numbers
    Note,
    /// scale degrees: n("0 2 4").scale("A:minor") (the song's key when no scale is given)
    Degree,
    /// chord symbols or Roman numerals ("Am7", "i7", "prog:A" = the song's progression)
    Chord,
    /// plain numbers / structure
    Plain,
}

/// track-level settings a pattern carries (Strudel sets these per event; claudius mixes per track)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackOpts {
    pub sound: Option<String>,
    pub bank: Option<String>,
    pub level_db: Option<f32>,
    pub pan: Option<f32>,
    pub reverb: Option<f32>,
    pub delay: Option<f32>,
    pub duck: Option<f32>,
    pub lpf: Option<f32>,
    pub hpf: Option<f32>,
    pub role: Option<String>,
    pub scale: Option<String>,
    pub octave: Option<i32>,
    pub voicing: Option<(i32, i32)>,
    pub arp: Option<String>,
    /// chords play only their root (the slash bass when there is one): a bass line that follows the harmony
    #[serde(default)]
    pub root: bool,
    pub crush: Option<f32>,
    pub drive: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub node: Arc<Node>,
    pub kind: Kind,
    pub opts: TrackOpts,
    /// the script expression that builds this pattern
    pub repr: String,
}

/// a value as it was written in mini-notation
pub fn mini_word(v: &Value) -> String {
    match v {
        Value::Word(w) => w.clone(),
        Value::Num(n) => fmt_num(*n),
    }
}

pub fn fmt_num(x: f64) -> String {
    if (x - x.round()).abs() < 1e-9 {
        format!("{}", x.round() as i64)
    } else {
        let s = format!("{x:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Pattern {
    pub fn new(node: Node, kind: Kind, repr: String) -> Self {
        Pattern { node: Arc::new(node), kind, opts: TrackOpts::default(), repr }
    }

    /// the stand-in a closure is called with to learn its code ("x")
    pub fn placeholder() -> Self {
        Pattern::new(Node::Silence, Kind::Plain, "x".into())
    }

    pub fn with(&self, node: Node, repr_suffix: &str) -> Pattern {
        Pattern {
            node: Arc::new(node),
            kind: self.kind,
            opts: self.opts.clone(),
            repr: format!("{}{}", self.repr, repr_suffix),
        }
    }

    pub fn node(&self) -> Node {
        (*self.node).clone()
    }

    /// the events whose onset lies in [b, e), sorted; an error when the pattern is too dense to play
    pub fn try_events(&self, b: f64, e: f64) -> Result<Vec<Ev>, String> {
        let outer = (WORK.with(|w| w.replace(MAX_WORK)), EVENTS.with(|n| n.replace(MAX_EVENTS)));
        let mut out = Vec::new();
        query(&self.node, b, e, &mut out);
        let left = (WORK.with(|w| w.replace(outer.0)), EVENTS.with(|n| n.replace(outer.1)));
        if left.1 == 0 {
            return Err(format!("this pattern is too dense to play: it makes more than {MAX_EVENTS} events"));
        }
        if left.0 == 0 {
            return Err(format!("this pattern is too dense to play: it needs more than {MAX_WORK} steps of work"));
        }
        out.sort_by(|x, y| x.b.total_cmp(&y.b));
        Ok(out)
    }

    /// like `try_events`, empty when the pattern is too dense
    pub fn events(&self, b: f64, e: f64) -> Vec<Ev> {
        self.try_events(b, e).unwrap_or_default()
    }

    /// distinct sound names the pattern uses within the first `cycles` cycles (s() words, or the track's sound)
    pub fn sound_names(&self, cycles: u32) -> Vec<String> {
        if self.kind != Kind::Sound {
            return self.opts.sound.clone().into_iter().collect();
        }
        let mut names: Vec<String> = self
            .events(0.0, cycles as f64)
            .into_iter()
            .filter_map(|e| match e.v {
                Value::Word(w) => Some(w.split(':').next().unwrap_or("").to_string()),
                Value::Num(_) => None,
            })
            .filter(|w| !w.is_empty())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

pub fn parse(src: &str) -> Result<Node, String> {
    mini::parse(src)
}
