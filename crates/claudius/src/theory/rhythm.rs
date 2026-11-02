//! Rhythm: drum hits, per-genre lane templates (step probabilities on the 1/16 grid), euclidean rhythms, trap
//! rolls, fills, swing, and the syncopation / density measures the critic uses.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;

/// one drum hit: position in beats within its pattern, velocity 0..1, and what the step does beyond striking
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub t: f64,
    pub v: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trig: Option<Trig>,
}

impl Hit {
    pub const fn new(t: f64, v: f32) -> Hit {
        Hit { t, v, trig: None }
    }
}

/// What a step does beyond striking, as a drum machine's trig does: when it plays (a chance each time round, a
/// condition), how it strikes (a ratchet of strokes within its sixteenth, a flam), where (nudged off the grid), and
/// what it changes in the sound for that stroke (pitch, decay, tone, pan).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trig {
    /// the chance it plays each time round, 0 to 1
    #[serde(skip_serializing_if = "is_one")]
    pub prob: f32,
    #[serde(skip_serializing_if = "Cond::is_always")]
    pub cond: Cond,
    /// strokes within its sixteenth (1 once, 2 two 32nds, 3 a triplet, up to 8), and how their velocity moves
    /// across them (1 swells to the last, -1 fades from the first)
    #[serde(skip_serializing_if = "is_one_u8")]
    pub ratchet: u8,
    #[serde(skip_serializing_if = "is_zero")]
    pub ramp: f32,
    /// a grace stroke this many ms before it, at half its velocity (0: none)
    #[serde(skip_serializing_if = "is_zero")]
    pub flam: f32,
    /// where it lands off the grid, in sixteenths: -0.5 early to 0.5 late
    #[serde(skip_serializing_if = "is_zero")]
    pub nudge: f32,
    /// its pitch in semitones; its decay, times the lane's; its tone, -1 darker to 1 brighter; its pan, -1 to 1
    #[serde(skip_serializing_if = "is_zero_i8")]
    pub pitch: i8,
    #[serde(skip_serializing_if = "is_one")]
    pub decay: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub tone: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub pan: f32,
}

impl Default for Trig {
    fn default() -> Trig {
        Trig {
            prob: 1.0,
            cond: Cond::Always,
            ratchet: 1,
            ramp: 0.0,
            flam: 0.0,
            nudge: 0.0,
            pitch: 0,
            decay: 1.0,
            tone: 0.0,
            pan: 0.0,
        }
    }
}

impl Trig {
    /// every setting within its range
    pub fn clamped(self) -> Trig {
        Trig {
            prob: self.prob.clamp(0.0, 1.0),
            cond: self.cond,
            ratchet: self.ratchet.clamp(1, 8),
            ramp: self.ramp.clamp(-1.0, 1.0),
            flam: self.flam.clamp(0.0, 60.0),
            nudge: self.nudge.clamp(-0.5, 0.5),
            pitch: self.pitch.clamp(-24, 24),
            decay: self.decay.clamp(0.1, 4.0),
            tone: self.tone.clamp(-1.0, 1.0),
            pan: self.pan.clamp(-1.0, 1.0),
        }
    }

    /// The strokes it makes for a hit at `t` (beats) of velocity `v` at `bpm`, its step `step` beats long (a sixteenth
    /// at the lane's own speed): (beat, velocity), the flam's grace stroke first.
    pub fn strokes(&self, t: f64, v: f32, bpm: f64, step: f64) -> Vec<(f64, f32)> {
        let mut out = Vec::new();
        if self.flam > 0.0 {
            out.push((t - self.flam as f64 / 1000.0 * bpm / 60.0, v * 0.5));
        }
        let n = self.ratchet.max(1);
        for k in 0..n {
            let across = if n == 1 { 0.0 } else { k as f32 / (n - 1) as f32 };
            // a swell starts soft and ends at the velocity; a fade the other way round
            let level = 1.0 - self.ramp.abs() * if self.ramp > 0.0 { 1.0 - across } else { across } * 0.75;
            out.push((t + step * k as f64 / n as f64, (v * level).clamp(0.05, 1.0)));
        }
        out
    }
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_one_u8(v: &u8) -> bool {
    *v == 1
}
fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

/// A groove: where each sixteenth of the bar lands against the grid and how hard, as a drummer's feel or a drum
/// machine's groove template has it. `timing` is beats (below 0 early), `accent` a factor on the velocity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Groove {
    pub timing: [f32; 16],
    pub accent: [f32; 16],
}

/// the grooves claudius has by name, as `groove` makes them
pub const GROOVES: &[&str] =
    &["straight", "swing54", "swing58", "swing62", "swing66", "swing71", "shuffle", "dilla", "push", "lazy"];

/// The groove named `name`, one of `GROOVES`. The swings put each second sixteenth of an eighth that far into the
/// eighth (54 % to 71 %, as a drum machine's swing setting; 66 % is a triplet); `shuffle` moves the off-beat eighths
/// and sixteenths onto triplets; `dilla` drags the off-beats and the backbeat unevenly and leans on the
/// downbeats; `push` plays everything but the beats a little early, `lazy` a little late.
pub fn groove(name: &str) -> Option<Groove> {
    let swing = |pct: f32| {
        let late = 0.5 * pct / 100.0 - 0.25;
        let timing = std::array::from_fn(|i| if i % 2 == 1 { late } else { 0.0 });
        Groove { timing, accent: [1.0; 16] }
    };
    Some(match name {
        "straight" => Groove { timing: [0.0; 16], accent: [1.0; 16] },
        "swing54" => swing(54.0),
        "swing58" => swing(58.0),
        "swing62" => swing(62.0),
        "swing66" => swing(66.0),
        "swing71" => swing(71.0),
        "shuffle" => Groove {
            timing: std::array::from_fn(|i| match i % 4 {
                1 => 1.0 / 3.0 - 0.25,
                2 => 2.0 / 3.0 - 0.5,
                3 => 5.0 / 6.0 - 0.75,
                _ => 0.0,
            }),
            accent: std::array::from_fn(|i| if i % 4 == 0 { 1.0 } else { 0.85 }),
        },
        "dilla" => Groove {
            timing: [
                0.0, 0.07, 0.02, 0.06, 0.03, 0.08, 0.025, 0.05, 0.0, 0.065, 0.015, 0.07, 0.035, 0.075, 0.02, 0.055,
            ],
            accent: [1.0, 0.7, 0.85, 0.75, 0.95, 0.7, 0.8, 0.72, 0.98, 0.7, 0.85, 0.74, 0.95, 0.72, 0.82, 0.7],
        },
        "push" => Groove {
            timing: std::array::from_fn(|i| if i % 4 == 0 { 0.0 } else { -0.02 }),
            accent: std::array::from_fn(|i| if i % 2 == 1 { 1.08 } else { 1.0 }),
        },
        "lazy" => Groove {
            timing: std::array::from_fn(|i| if i % 4 == 0 { 0.0 } else { 0.03 }),
            accent: std::array::from_fn(|i| if i % 4 == 0 { 1.0 } else { 0.92 }),
        },
        _ => return None,
    })
}

impl Groove {
    /// Where a stroke at beat `t` of the song (bar lines every 4 beats) lands and how hard, `amount` of the groove (0
    /// to 1): only strokes on a sixteenth take it.
    pub fn apply(&self, t: f64, v: f32, amount: f32) -> (f64, f32) {
        let q = t * 4.0;
        if (q - q.round()).abs() > 1e-6 {
            return (t, v);
        }
        let i = (q.round() as i64).rem_euclid(16) as usize;
        let a = amount.clamp(0.0, 1.0);
        (t + (a * self.timing[i]) as f64, (v * (1.0 + a * (self.accent[i] - 1.0))).clamp(0.05, 1.0))
    }

    /// in words: how late the off-beat sixteenths land (as a swing), how the backbeat sits, where the accents are
    pub fn describe(&self) -> String {
        let mean = |idx: &[usize], v: &[f32; 16]| idx.iter().map(|&i| v[i]).sum::<f32>() / idx.len() as f32;
        let off = mean(&[1, 3, 5, 7, 9, 11, 13, 15], &self.timing);
        let swing = (off + 0.25) / 0.5 * 100.0;
        let back = mean(&[4, 12], &self.timing);
        let feel = match back {
            b if b > 0.01 => format!("the backbeat {:.0}% of a sixteenth behind", b / 0.25 * 100.0),
            b if b < -0.01 => format!("the backbeat {:.0}% of a sixteenth ahead", -b / 0.25 * 100.0),
            _ => "the backbeat on the beat".to_string(),
        };
        let mut loud: Vec<usize> = (0..16).collect();
        loud.sort_by(|a, b| self.accent[*b].total_cmp(&self.accent[*a]));
        let names = ["1", "1e", "1&", "1a", "2", "2e", "2&", "2a", "3", "3e", "3&", "3a", "4", "4e", "4&", "4a"];
        let accents: Vec<&str> = loud.iter().take(3).map(|&i| names[i]).collect();
        format!("the off-beat sixteenths at {swing:.0}% swing, {feel}, the accents on {}", accents.join(", "))
    }
}

/// When a step plays, counted in times round its pattern from the start of each section: every time, the `a`-th of
/// every `b` ("1:2" every other time, "4:4" the fourth), only the first or the last time round in a section, or every
/// time but those. Written as a string: "always", "1:2", "first", "last", "!first", "!last".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Cond {
    #[default]
    Always,
    Every(u8, u8),
    First,
    Last,
    NotFirst,
    NotLast,
}

impl Cond {
    pub fn is_always(&self) -> bool {
        *self == Cond::Always
    }

    /// whether it plays the `k`-th time round (from 0) of `n` in a section
    pub fn plays(&self, k: usize, n: usize) -> bool {
        match *self {
            Cond::Always => true,
            Cond::Every(a, b) => k % b.max(1) as usize == (a.max(1) - 1) as usize % b.max(1) as usize,
            Cond::First => k == 0,
            Cond::Last => k + 1 == n,
            Cond::NotFirst => k != 0,
            Cond::NotLast => k + 1 != n,
        }
    }
}

impl TryFrom<String> for Cond {
    type Error = String;

    fn try_from(s: String) -> Result<Cond, String> {
        let s = s.trim().to_lowercase();
        Ok(match s.as_str() {
            "" | "always" => Cond::Always,
            "first" => Cond::First,
            "last" => Cond::Last,
            "!first" | "not first" => Cond::NotFirst,
            "!last" | "not last" => Cond::NotLast,
            other => {
                let parsed = other
                    .split_once(':')
                    .and_then(|(a, b)| Some((a.trim().parse::<u8>().ok()?, b.trim().parse::<u8>().ok()?)));
                match parsed {
                    Some((a, b)) if (1..=16).contains(&b) && (1..=b).contains(&a) => Cond::Every(a, b),
                    _ => {
                        return Err(format!(
                            "a step plays always, a:b (1:2, 3:4 ...: the a-th time round of every b, b up to 16), \
                             first, last, !first or !last, not '{other}'"
                        ));
                    }
                }
            }
        })
    }
}

impl From<Cond> for String {
    fn from(c: Cond) -> String {
        match c {
            Cond::Always => "always".into(),
            Cond::Every(a, b) => format!("{a}:{b}"),
            Cond::First => "first".into(),
            Cond::Last => "last".into(),
            Cond::NotFirst => "!first".into(),
            Cond::NotLast => "!last".into(),
        }
    }
}

/// a lane's habits: probability of a hit on each 1/16 step (16 = one bar, 32 = two bars); values >= 1 always hit.
/// `vel` is the velocity range, `ghost` the chance of soft extra hits on empty steps, `rolls` the chance per beat of a
/// trap roll (1/32 or triplet bursts), `vary` how often the last bar of a phrase deviates
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaneTemplate {
    pub steps: Vec<f32>,
    #[serde(default = "default_vel")]
    pub vel: (f32, f32),
    #[serde(default)]
    pub ghost: f32,
    #[serde(default)]
    pub rolls: f32,
    #[serde(default)]
    pub vary: f32,
}

fn default_vel() -> (f32, f32) {
    (0.7, 1.0)
}

/// metrical weight of a 1/16 step within a bar (downbeat strongest)
pub fn step_weight(step: usize) -> f32 {
    match step % 16 {
        0 => 1.0,
        8 => 0.92,
        4 | 12 => 0.85,
        2 | 6 | 10 | 14 => 0.72,
        _ => 0.6,
    }
}

/// one bar (or the template's length) of hits sampled from the template
fn sample_bar(t: &LaneTemplate, rng: &mut Rng, vary: bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (s, &p) in t.steps.iter().enumerate() {
        let p = if vary && p < 1.0 { (p * 1.6).min(0.95) * if rng.chance(0.5) { 1.0 } else { 0.4 } } else { p };
        let hit = p >= 1.0 || rng.chance(p as f64);
        if hit {
            let v = t.vel.0 + (t.vel.1 - t.vel.0) * (0.35 + 0.65 * step_weight(s)) * (0.85 + 0.15 * rng.f32());
            hits.push(Hit::new(s as f64 * 0.25, v.clamp(0.05, 1.0)));
        } else if t.ghost > 0.0 && rng.chance(t.ghost as f64) {
            hits.push(Hit::new(s as f64 * 0.25, (t.vel.0 * 0.45).clamp(0.05, 1.0)));
        }
    }
    hits
}

/// a looping pattern of `bars` bars: the template's main bar repeated, the phrase's last bar sometimes varied,
/// rolls added on top
pub fn generate(t: &LaneTemplate, bars: u32, rng: &mut Rng) -> Vec<Hit> {
    let bar_steps = 16usize;
    let tmpl_bars = (t.steps.len().max(1) as f64 / bar_steps as f64).ceil().max(1.0) as u32;
    let main = sample_bar(t, rng, false);
    let mut out = Vec::new();
    let mut b = 0;
    while b < bars {
        let last = b + tmpl_bars >= bars;
        let part =
            if last && t.vary > 0.0 && rng.chance(t.vary as f64) { sample_bar(t, rng, true) } else { main.clone() };
        for h in &part {
            out.push(Hit { t: h.t + b as f64 * 4.0, ..*h });
        }
        b += tmpl_bars;
    }
    if t.rolls > 0.0 {
        out = add_rolls(&out, bars as f64 * 4.0, t.rolls, rng);
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    out.dedup_by(|a, b| (a.t - b.t).abs() < 1e-6);
    out
}

/// trap hi-hat rolls: some beats are replaced by bursts of 1/32 notes or 1/16 triplets with a velocity ramp
pub fn add_rolls(hits: &[Hit], len_beats: f64, prob: f32, rng: &mut Rng) -> Vec<Hit> {
    let mut out: Vec<Hit> = Vec::new();
    let beats = len_beats as usize;
    let mut rolled = vec![false; beats];
    for (b, r) in rolled.iter_mut().enumerate() {
        // rolls mostly on the second half of a bar, never on the downbeat
        let pos = b % 4;
        let p = prob as f64
            * if pos == 0 {
                0.1
            } else if pos == 3 {
                1.6
            } else {
                0.8
            };
        *r = rng.chance(p.min(0.9));
    }
    for h in hits {
        if !rolled[(h.t as usize).min(beats.saturating_sub(1))] {
            out.push(*h);
        }
    }
    for (b, r) in rolled.iter().enumerate() {
        if !*r {
            continue;
        }
        let (div, len) = match rng.below(4) {
            0 => (8.0, 1.0),
            1 => (6.0, 1.0),
            2 => (8.0, 0.5),
            _ => (12.0, 1.0),
        };
        let start = b as f64 + if len < 1.0 { 0.5 } else { 0.0 };
        let n = (div * len) as usize;
        let up = rng.chance(0.6);
        for k in 0..n {
            let f = k as f32 / n.max(1) as f32;
            let v = if up { 0.45 + 0.5 * f } else { 0.95 - 0.45 * f };
            out.push(Hit::new(start + k as f64 / div, v));
        }
    }
    out
}

/// Bjorklund: k onsets spread as evenly as possible over n steps, rotated by `rot`
pub fn euclid(k: usize, n: usize, rot: usize) -> Vec<bool> {
    if n == 0 {
        return Vec::new();
    }
    let mut v = bjorklund(k.min(n), n);
    v.rotate_right(rot % n);
    v
}

/// Bjorklund's algorithm, as Tidal and Strudel run it: (3,8) is x..x..x., (5,8) is x.xx.xx.
fn bjorklund(k: usize, n: usize) -> Vec<bool> {
    let mut a: Vec<Vec<bool>> = vec![vec![true]; k];
    let mut b: Vec<Vec<bool>> = vec![vec![false]; n - k];
    while a.len().min(b.len()) > 1 {
        // pair as many of the first group with the second as there are of the smaller; the leftovers of the
        // larger become the new second group
        let pairs = a.len().min(b.len());
        let rest = if a.len() > b.len() { a.split_off(pairs) } else { b.split_off(pairs) };
        for (x, y) in a.iter_mut().zip(b.drain(..)) {
            x.extend(y);
        }
        b = rest;
    }
    a.into_iter().chain(b).flatten().collect()
}

/// a euclidean pattern spread over one bar per cycle, repeated for `bars`
pub fn euclid_hits(k: usize, n: usize, rot: usize, bars: u32, vel: f32) -> Vec<Hit> {
    let pat = euclid(k, n, rot);
    let step = 4.0 / n.max(1) as f64;
    let mut out = Vec::new();
    for b in 0..bars {
        for (i, on) in pat.iter().enumerate() {
            if *on {
                out.push(Hit::new(b as f64 * 4.0 + i as f64 * step, vel * (0.75 + 0.25 * step_weight(i))));
            }
        }
    }
    out
}

/// snare build-up: quarters, eighths, sixteenths, then thirty-seconds, rising in velocity
pub fn snare_roll(len_beats: f64) -> Vec<Hit> {
    let mut out = Vec::new();
    let quarters = (len_beats * 0.5).floor();
    let mut t = 0.0;
    let phases = [(1.0, quarters), (0.5, len_beats * 0.25), (0.25, len_beats * 0.125), (0.125, len_beats * 0.125)];
    for (step, span) in phases {
        let end = t + span;
        while t < end - 1e-9 && t < len_beats {
            let v = 0.4 + 0.6 * (t / len_beats) as f32;
            out.push(Hit::new(t, v));
            t += step;
        }
    }
    out
}

/// a fill for the last `len_beats` of a phrase
pub fn fill(kind: &str, len_beats: f64, rng: &mut Rng) -> Vec<Hit> {
    match kind {
        "roll" => {
            let n = (len_beats * 4.0) as usize;
            (0..n).map(|i| Hit::new(i as f64 * 0.25, 0.5 + 0.5 * i as f32 / n as f32)).collect()
        }
        "triplets" => {
            let n = (len_beats * 3.0) as usize;
            (0..n).map(|i| Hit::new(i as f64 / 3.0, 0.6 + 0.4 * i as f32 / n as f32)).collect()
        }
        _ => {
            let mut out = Vec::new();
            let n = (len_beats * 4.0) as usize;
            for i in 0..n {
                if i % 2 == 1 && rng.chance(0.5) || rng.chance(0.55) {
                    out.push(Hit::new(i as f64 * 0.25, 0.55 + 0.45 * rng.f32()));
                }
            }
            out
        }
    }
}

/// swung position of a beat time: off-beat 1/16ths move later (swing 0.5 = straight, 0.66 = triplet feel)
pub fn swing(t: f64, amount: f32) -> f64 {
    if amount <= 0.501 {
        return t;
    }
    let pair = (t / 0.5).floor() * 0.5;
    let within = t - pair;
    if (within - 0.25).abs() < 0.01 { pair + 0.5 * amount as f64 } else { t }
}

/// Longuet-Higgins & Lee style syncopation per bar: onsets on weak positions followed by silence on stronger ones
pub fn syncopation(hits: &[Hit], len_beats: f64) -> f64 {
    let steps = (len_beats * 4.0).round() as usize;
    if steps == 0 {
        return 0.0;
    }
    let mut on = vec![false; steps];
    for h in hits {
        let s = (h.t * 4.0).round() as usize;
        if s < steps {
            on[s] = true;
        }
    }
    let lvl = |s: usize| -> i32 {
        match s % 16 {
            0 => 0,
            8 => -1,
            4 | 12 => -2,
            2 | 6 | 10 | 14 => -3,
            _ => -4,
        }
    };
    let mut total = 0.0;
    for s in 0..steps {
        if !on[s] {
            continue;
        }
        let mut n = s + 1;
        while n < steps && !on[n] && lvl(n) <= lvl(s) {
            n += 1;
        }
        if n < steps && !on[n] && lvl(n) > lvl(s) {
            total += (lvl(n) - lvl(s)) as f64;
        }
    }
    total / (len_beats / 4.0).max(1.0)
}

pub fn density(hits: &[Hit], len_beats: f64) -> f64 {
    hits.len() as f64 / len_beats.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_condition_counts_the_times_round_in_a_section() {
        let plays = |c: &str, n: usize| -> Vec<usize> {
            let c = Cond::try_from(c.to_string()).unwrap();
            (0..n).filter(|&k| c.plays(k, n)).collect()
        };
        assert_eq!(plays("1:2", 6), [0, 2, 4]);
        assert_eq!(plays("2:2", 6), [1, 3, 5]);
        assert_eq!(plays("4:4", 8), [3, 7]);
        assert_eq!((plays("first", 4), plays("last", 4)), (vec![0], vec![3]));
        assert_eq!((plays("!first", 3), plays("!last", 3)), (vec![1, 2], vec![0, 1]));
        assert_eq!(plays("always", 2), [0, 1]);
        for bad in ["5:4", "0:2", "1:17", "sometimes"] {
            let e = Cond::try_from(bad.to_string()).unwrap_err();
            assert!(e.contains(&format!("not '{bad}'")), "{e}");
        }
        assert_eq!(String::from(Cond::Every(3, 4)), "3:4");
    }

    #[test]
    fn a_ratchet_strikes_within_its_sixteenth_and_a_flam_comes_before() {
        let swell = Trig { ratchet: 4, ramp: 1.0, ..Trig::default() };
        let s = swell.strokes(1.0, 0.8, 120.0, 0.25);
        let at: Vec<f64> = s.iter().map(|x| x.0).collect();
        assert_eq!(at, [1.0, 1.0625, 1.125, 1.1875]);
        assert!(s.windows(2).all(|w| w[0].1 < w[1].1) && (s[3].1 - 0.8).abs() < 1e-6, "{s:?}");
        let fade = Trig { ratchet: 3, ramp: -1.0, ..Trig::default() }.strokes(0.0, 1.0, 120.0, 0.25);
        assert!(fade.windows(2).all(|w| w[0].1 > w[1].1) && fade[0].1 == 1.0, "{fade:?}");
        // 20 ms at 120 BPM is 0.04 beats, at half the velocity
        let flam = Trig { flam: 20.0, ..Trig::default() }.strokes(2.0, 0.8, 120.0, 0.25);
        assert_eq!(flam.len(), 2);
        assert!((flam[0].0 - 1.96).abs() < 1e-9 && flam[0].1 == 0.4 && flam[1] == (2.0, 0.8), "{flam:?}");
    }

    #[test]
    fn a_hit_without_a_trig_is_written_as_before_and_a_trig_only_with_what_it_changes() {
        assert_eq!(serde_json::to_string(&Hit::new(0.5, 0.8)).unwrap(), r#"{"t":0.5,"v":0.8}"#);
        let h =
            Hit { trig: Some(Trig { prob: 0.5, cond: Cond::Every(1, 2), ..Trig::default() }), ..Hit::new(1.0, 1.0) };
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, r#"{"t":1.0,"v":1.0,"trig":{"prob":0.5,"cond":"1:2"}}"#);
        assert_eq!(serde_json::from_str::<Hit>(&json).unwrap(), h);
        let wide = Trig { ratchet: 20, nudge: 2.0, pan: -3.0, ..Trig::default() }.clamped();
        assert_eq!((wide.ratchet, wide.nudge, wide.pan), (8, 0.5, -1.0));
    }
}
