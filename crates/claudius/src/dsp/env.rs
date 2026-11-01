//! Envelopes: analog-style ADSR, percussive decays, and automation curves (piecewise in beats).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const K: f64 = 4.6; // exp(-4.6) ~ 1%: a segment counts as done when it is 99% of the way there

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Adsr {
    pub a: f64,
    pub d: f64,
    pub s: f64,
    pub r: f64,
}

impl Adsr {
    pub const fn new(a: f64, d: f64, s: f64, r: f64) -> Self {
        Adsr { a, d, s, r }
    }

    /// level `i` samples after note-on for a gate of `gate` samples (release appended after the gate)
    #[inline]
    pub fn level(&self, i: usize, gate: usize, sr: f64) -> f64 {
        let held = |t: f64| {
            let an = (self.a * sr).max(1.0);
            if t < an {
                (1.0 - (-K * t / an).exp()) / (1.0 - (-K).exp())
            } else {
                let dn = (self.d * sr).max(1.0);
                self.s + (1.0 - self.s) * (-K * (t - an) / dn).exp()
            }
        };
        if i < gate {
            held(i as f64)
        } else {
            let at_release = held(gate.max(1) as f64 - 1.0);
            let rn = (self.r * sr).max(1.0);
            at_release * (-6.9 * (i - gate) as f64 / rn).exp()
        }
    }

    pub fn release_samples(&self, sr: f64) -> usize {
        (self.r * sr).ceil() as usize + 1
    }

    /// whole envelope for a gate of `gate` samples, release tail included, ending exactly at zero
    pub fn render(&self, gate: usize, sr: f64) -> Vec<f32> {
        let n = gate + self.release_samples(sr);
        let mut e: Vec<f32> = (0..n).map(|i| self.level(i, gate, sr) as f32).collect();
        let k = n.min(96);
        for j in 0..k {
            e[n - 1 - j] *= j as f32 / k as f32;
        }
        e
    }
}

/// percussive envelope: linear attack, hold, exponential decay to -60 dB at t60; curve > 1 falls faster at first
pub fn perc(n: usize, sr: f64, attack: f64, hold: f64, t60: f64, curve: f64) -> Vec<f32> {
    let an = attack.max(1e-5);
    let mut e: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            let a = if t < an { t / an } else { 1.0 };
            let tt = (t - an - hold).max(0.0);
            (a * (-6.9 * (tt / t60.max(1e-4)).powf(curve)).exp()) as f32
        })
        .collect();
    let k = n.min(64);
    for j in 0..k {
        e[n - 1 - j] *= j as f32 / k as f32;
    }
    e
}

/// frequency multiplier starting `semis` semitones up and falling exponentially to 1 (time constant `time` s)
#[inline]
pub fn pitch_drop(t: f64, semis: f64, time: f64) -> f64 {
    2f64.powf(semis * (-t / time.max(1e-4)).exp() / 12.0)
}

/// a constant, an automation curve through (beat, value) points, or a value for each section
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Param {
    Const(f32),
    Curve(Vec<(f64, f32)>),
    Sections(BySection),
}

/// A value for each section, as the form lays them out when the song renders (`Param::resolve`): a section named in
/// `sections`, or of a kind named there, holds its one value or moves through its several across the section (evenly
/// spaced, the last at its end); the others hold `rest`. A section's own name counts before its kind.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BySection {
    #[serde(default)]
    pub rest: f32,
    pub sections: BTreeMap<String, Vec<f32>>,
}

/// a section as the form lays it out: its name and kind, and the beats it starts and ends on
#[derive(Clone, Copy, Debug)]
pub struct Span<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub start: f64,
    pub end: f64,
}

/// beats a value by section takes to move to the next section's, so that a step does not click
const STEP_BEATS: f64 = 1.0 / 32.0;

/// a parameter's value at each sample, without a buffer for one that holds still
pub enum Values {
    One(f32),
    Each(Vec<f32>),
}

impl Values {
    #[inline]
    pub fn at(&self, i: usize) -> f32 {
        match self {
            Values::One(v) => *v,
            Values::Each(v) => v.get(i).or(v.last()).copied().unwrap_or(0.0),
        }
    }
}

impl Default for Param {
    fn default() -> Self {
        Param::Const(0.0)
    }
}

impl From<f32> for Param {
    fn from(v: f32) -> Self {
        Param::Const(v)
    }
}

impl Param {
    pub fn is_const(&self) -> bool {
        matches!(self, Param::Const(_))
    }

    pub fn first(&self) -> f32 {
        match self {
            Param::Const(v) => *v,
            Param::Curve(p) => p.first().map_or(0.0, |x| x.1),
            Param::Sections(by) => by.rest,
        }
    }

    /// the largest value it takes
    pub fn peak(&self) -> f32 {
        match self {
            Param::Const(v) => *v,
            Param::Curve(p) => p.iter().map(|x| x.1).fold(f32::MIN, f32::max).max(self.first()),
            Param::Sections(by) => by.sections.values().flatten().copied().fold(by.rest, f32::max),
        }
    }

    /// Values by section as a curve through the sections laid out as `spans`; a constant or a curve as it is. Each
    /// section's values take a moment (`STEP_BEATS`) to follow the last section's, and a curve that holds still all
    /// through is a constant.
    pub fn resolve(&self, spans: &[Span]) -> Param {
        let Param::Sections(by) = self else { return self.clone() };
        let mut pts: Vec<(f64, f32)> = Vec::new();
        for s in spans {
            let vals = by.sections.get(s.name).or_else(|| by.sections.get(s.kind)).filter(|v| !v.is_empty());
            let vals = vals.map_or(std::slice::from_ref(&by.rest), Vec::as_slice);
            let len = (s.end - s.start).max(0.0);
            let at: Vec<(f64, f32)> = match vals {
                [v] => vec![(s.start, *v), (s.end, *v)],
                many => {
                    let k = (many.len() - 1) as f64;
                    many.iter().enumerate().map(|(i, v)| (s.start + len * i as f64 / k, *v)).collect()
                }
            };
            match pts.last() {
                None => pts.extend(at),
                Some(&(_, last)) if last == at[0].1 => pts.extend(&at[1..]),
                Some(_) => {
                    pts.push((s.start + STEP_BEATS.min(len / 4.0), at[0].1));
                    pts.extend(&at[1..]);
                }
            }
        }
        match pts.first() {
            None => Param::Const(by.rest),
            Some(&(_, v)) if pts.iter().all(|p| p.1 == v) => Param::Const(v),
            Some(_) => Param::Curve(pts),
        }
    }

    /// its value at each of `n` samples from beat 0, `samples_per_beat` a beat
    pub fn values(&self, n: usize, samples_per_beat: f64, log: bool) -> Values {
        match self {
            Param::Const(v) => Values::One(*v),
            _ => Values::Each(self.render(n, samples_per_beat, log)),
        }
    }

    /// the same parameter `beats` earlier: a curve's points move, a constant stays
    pub fn shifted(&self, beats: f64) -> Param {
        match self {
            Param::Curve(pts) => Param::Curve(pts.iter().map(|(t, v)| (t - beats, *v)).collect()),
            other => other.clone(),
        }
    }

    /// value at `beat`; log=true interpolates geometrically (cutoff frequencies); values by section, until they are
    /// resolved, hold their `rest`
    pub fn at(&self, beat: f64, log: bool) -> f32 {
        match self {
            Param::Const(v) => *v,
            Param::Sections(by) => by.rest,
            Param::Curve(pts) => {
                if pts.is_empty() {
                    return 0.0;
                }
                if beat <= pts[0].0 {
                    return pts[0].1;
                }
                for w in pts.windows(2) {
                    let (b0, v0) = w[0];
                    let (b1, v1) = w[1];
                    if beat <= b1 {
                        let f = if b1 > b0 { ((beat - b0) / (b1 - b0)) as f32 } else { 1.0 };
                        return if log && v0 > 0.0 && v1 > 0.0 {
                            (v0.ln() + (v1.ln() - v0.ln()) * f).exp()
                        } else {
                            v0 + (v1 - v0) * f
                        };
                    }
                }
                pts[pts.len() - 1].1
            }
        }
    }

    /// per-sample values for n samples starting at sample 0 = beat 0
    pub fn render(&self, n: usize, samples_per_beat: f64, log: bool) -> Vec<f32> {
        match self {
            Param::Const(v) => vec![*v; n],
            _ => (0..n).map(|i| self.at(i as f64 / samples_per_beat, log)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(name: &'static str, kind: &'static str, start: f64, end: f64) -> Span<'static> {
        Span { name, kind, start, end }
    }

    #[test]
    fn values_by_section_follow_the_form_by_name_then_kind() {
        let form =
            [span("intro", "intro", 0.0, 16.0), span("rise", "build", 16.0, 32.0), span("drop", "drop", 32.0, 64.0)];
        let by = |rest: f32, pairs: &[(&str, &[f32])]| {
            let sections = pairs.iter().map(|(k, v)| (k.to_string(), v.to_vec())).collect();
            Param::Sections(BySection { rest, sections })
        };
        // a build by its kind moves from 0.1 to 0.6 across it; the drop holds 0.2, and takes a moment to get there
        let p = by(0.1, &[("build", &[0.1, 0.6]), ("drop", &[0.2])]).resolve(&form);
        let at = |b: f64| p.at(b, false);
        assert_eq!((at(0.0), at(15.9), at(16.0)), (0.1, 0.1, 0.1));
        assert!((at(24.0) - 0.35).abs() < 1e-6, "{}", at(24.0));
        let near = |b: f64, v: f32| (at(b) - v).abs() < 1e-6;
        assert!(near(32.0, 0.6) && near(32.0 + STEP_BEATS, 0.2) && near(63.0, 0.2) && near(80.0, 0.2));
        assert!((at(32.0 + STEP_BEATS / 2.0) - 0.4).abs() < 1e-6);
        // its own name counts before its kind; several values spread across the section, the last at its end
        let p = by(0.0, &[("build", &[1.0]), ("rise", &[0.0, 1.0, 0.0])]).resolve(&form);
        assert_eq!((p.at(16.0, false), p.at(24.0, false), p.at(31.99, false) < 0.01), (0.0, 1.0, true));
        // what holds still all through is a constant; a curve and a constant stay as they are
        assert_eq!(by(0.5, &[("verse", &[0.9])]).resolve(&form), Param::Const(0.5));
        let curve = Param::Curve(vec![(0.0, 1.0), (4.0, 2.0)]);
        assert_eq!(curve.resolve(&form), curve);
        assert_eq!(by(0.3, &[("drop", &[0.2, 0.9])]).peak(), 0.9);
        assert_eq!(curve.shifted(4.0), Param::Curve(vec![(-4.0, 1.0), (0.0, 2.0)]));
    }

    #[test]
    fn values_by_section_read_and_write_as_plain_json() {
        let p: Param =
            serde_json::from_str(r#"{"rest": 0.1, "sections": {"drop": [0.5], "build": [0.1, 0.6]}}"#).unwrap();
        assert!(matches!(&p, Param::Sections(by) if by.rest == 0.1 && by.sections["build"] == [0.1, 0.6]));
        assert_eq!(serde_json::from_str::<Param>(&serde_json::to_string(&p).unwrap()).unwrap(), p);
        assert_eq!(serde_json::from_str::<Param>("0.25").unwrap(), Param::Const(0.25));
        assert_eq!(
            serde_json::from_str::<Param>("[[0, 1], [4, 0.5]]").unwrap(),
            Param::Curve(vec![(0.0, 1.0), (4.0, 0.5)])
        );
    }
}
