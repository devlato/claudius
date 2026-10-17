//! Chord progressions: a curated library by harmonic style (Roman numerals in the key's own mode), a functional-
//! harmony Markov generator, and theory-aware substitutions used as mutations.

use super::chord::{Chord, Quality, roman, roman_of};
use super::scale::{Key, Mode};
use crate::rng::Rng;

pub struct Style {
    pub name: &'static str,
    pub mode: Mode,
    pub progs: &'static [&'static str],
}

pub const STYLES: &[Style] = &[
    Style {
        name: "minor_epic",
        mode: Mode::Minor,
        progs: &[
            "i VI III VII",
            "VI VII i i",
            "i VII VI VII",
            "i III VII VI",
            "VI III VII i",
            "i VI VII v",
            "i iv VI VII",
        ],
    },
    Style {
        name: "minor_dark",
        mode: Mode::Minor,
        progs: &["i i VI VII", "i VI iv V", "i iv i v", "i VI i VII", "i v VI iv", "i VII VI V"],
    },
    Style {
        name: "minor_soul",
        mode: Mode::Minor,
        progs: &[
            "i7 iv7 VII7 IIImaj7",
            "i7 VImaj7 iv7 v7",
            "i9 iv9 i9 VImaj7",
            "iiø7 V7 i7 i7",
            "i7 iv7 v7 i7",
            "VImaj7 v7 iv7 i7",
        ],
    },
    Style {
        name: "dorian_vamp",
        mode: Mode::Dorian,
        progs: &["i7 IV7", "i7 IV9 i7 VIImaj7", "i9 IV7", "i7 ii7 i7 IV7"],
    },
    Style { name: "phrygian", mode: Mode::Phrygian, progs: &["i II", "i II i VII", "i i II i", "i vii i II"] },
    Style { name: "harmonic", mode: Mode::HarmonicMinor, progs: &["i iv V i", "i VI V V", "i iv VI V", "i VI iv V"] },
    Style { name: "minimal", mode: Mode::Minor, progs: &["i", "i i i VI", "i i iv iv", "i9 i9 i9 VII"] },
    Style {
        name: "major_pop",
        mode: Mode::Major,
        progs: &["I V vi IV", "vi IV I V", "I vi IV V", "IV I V vi", "I IV vi V", "I iii vi IV", "IV V vi I"],
    },
    Style {
        name: "major7_jazz",
        mode: Mode::Major,
        progs: &[
            "Imaj7 vi7 ii7 V7",
            "ii7 V7 Imaj7 Imaj7",
            "IVmaj7 iii7 ii7 Imaj7",
            "Imaj7 IVmaj7",
            "ii9 V9 Imaj9 vi9",
            "vi7 ii7 V7 Imaj7",
            "IVmaj7 V7 iii7 vi7",
        ],
    },
    Style {
        name: "minor_80s",
        mode: Mode::Minor,
        progs: &["i VI III VII", "i VII VI VII", "VI VII i i", "i III VII IV", "i VI VII i"],
    },
    Style { name: "mixolydian", mode: Mode::Mixolydian, progs: &["I VII IV I", "I VII", "I IV VII IV"] },
];

pub fn style(name: &str) -> Option<&'static Style> {
    STYLES.iter().find(|s| s.name == name)
}

pub fn parse(prog: &str) -> Vec<String> {
    prog.split_whitespace().map(String::from).collect()
}

pub fn chords(prog: &[String], key: &Key) -> Vec<Chord> {
    prog.iter()
        .map(|rn| {
            roman(rn, key)
                .unwrap_or_else(|| Chord::new(key.tonic, if key.mode.is_minor() { Quality::Min } else { Quality::Maj }))
        })
        .collect()
}

fn diatonic_quality(key: &Key, d: i32) -> Quality {
    let iv = key.mode.intervals();
    let r = iv[d.rem_euclid(7) as usize];
    let t = (iv[(d + 2).rem_euclid(7) as usize] - r).rem_euclid(12);
    let f = (iv[(d + 4).rem_euclid(7) as usize] - r).rem_euclid(12);
    match (t, f) {
        (4, 7) => Quality::Maj,
        (3, 7) => Quality::Min,
        (3, 6) => Quality::Dim,
        (4, 8) => Quality::Aug,
        _ => Quality::Maj,
    }
}

/// diatonic chord on degree d with `ext` (0 triad, 1 seventh, 2 ninth)
pub fn diatonic(key: &Key, d: i32, ext: u8) -> Chord {
    let q = diatonic_quality(key, d);
    let q = match (q, ext) {
        (Quality::Maj, 1)
            if (key.mode.intervals()[((d + 6).rem_euclid(7)) as usize]
                - key.mode.intervals()[d.rem_euclid(7) as usize])
                .rem_euclid(12)
                == 10 =>
        {
            Quality::Dom7
        }
        (Quality::Maj, 2)
            if (key.mode.intervals()[((d + 6).rem_euclid(7)) as usize]
                - key.mode.intervals()[d.rem_euclid(7) as usize])
                .rem_euclid(12)
                == 10 =>
        {
            Quality::Dom9
        }
        (q, e) => q.extended(e),
    };
    Chord::new(key.degree_pc(d), q)
}

/// functional-harmony transitions between scale degrees (tonic -> subdominant -> dominant -> tonic, plus the
/// stepwise and third moves pop and EDM lean on)
fn transitions(minor: bool) -> [[f64; 7]; 7] {
    if minor {
        [
            [0.5, 0.2, 1.0, 1.6, 0.9, 2.2, 1.8],
            [0.3, 0.0, 0.4, 0.3, 2.0, 0.3, 0.6],
            [0.9, 0.2, 0.2, 1.0, 0.5, 1.6, 1.6],
            [1.4, 0.2, 0.6, 0.2, 1.4, 1.2, 1.4],
            [2.0, 0.1, 0.5, 0.6, 0.2, 1.4, 0.6],
            [1.4, 0.3, 1.6, 1.4, 1.2, 0.2, 2.2],
            [2.2, 0.1, 1.6, 0.6, 0.6, 1.6, 0.2],
        ]
    } else {
        [
            [0.3, 0.8, 0.4, 2.0, 2.0, 2.0, 0.1],
            [0.4, 0.1, 0.3, 0.6, 2.4, 0.4, 0.3],
            [0.5, 0.2, 0.1, 1.2, 0.4, 1.8, 0.1],
            [1.8, 0.6, 0.4, 0.2, 2.0, 1.0, 0.2],
            [2.2, 0.2, 0.4, 0.8, 0.2, 1.8, 0.1],
            [0.8, 1.2, 0.4, 2.2, 1.2, 0.2, 0.1],
            [1.8, 0.1, 0.6, 0.2, 0.4, 0.6, 0.1],
        ]
    }
}

/// a `len`-chord loop from the Markov chain: starts on the tonic or the sixth, never repeats a chord twice in a
/// row, and ends on a chord that leads back to the start
pub fn markov(key: &Key, len: usize, ext: u8, rng: &mut Rng) -> Vec<String> {
    let t = transitions(key.mode.is_minor());
    for _ in 0..64 {
        let start = if rng.chance(0.75) { 0 } else { 5 };
        let mut ds = vec![start];
        while ds.len() < len {
            let cur = *ds.last().unwrap() as usize;
            let mut w = t[cur];
            w[cur] *= 0.1;
            ds.push(rng.weighted(&w) as i32);
        }
        let last = *ds.last().unwrap() as usize;
        if len > 1 && t[last][start as usize] < 1.0 {
            continue;
        }
        return ds.iter().map(|&d| roman_of(&diatonic(key, d, ext), key)).collect();
    }
    vec![roman_of(&diatonic(key, 0, ext), key)]
}

/// one theory-aware substitution; returns the new progression and what was done
pub fn substitute(prog: &[String], key: &Key, rng: &mut Rng) -> (Vec<String>, String) {
    let mut out = prog.to_vec();
    if out.is_empty() {
        return (out, "nothing to substitute".into());
    }
    let k = rng.below(out.len());
    let chords = chords(prog, key);
    let c = chords[k];
    let deg = key.pcs().iter().position(|&p| p == c.root).map(|d| d as i32);
    let ext = match c.quality {
        Quality::Maj | Quality::Min | Quality::Dim => 0,
        Quality::Dom9 | Quality::Maj9 | Quality::Min9 | Quality::Min11 => 2,
        _ => 1,
    };
    let op = rng.below(7);
    let what = match (op, deg) {
        (0, Some(d)) => {
            let nd = if rng.chance(0.5) { d + 5 } else { d + 2 };
            out[k] = roman_of(&diatonic(key, nd, ext), key);
            format!("chord {}: {} -> {} (third relation)", k + 1, prog[k], out[k])
        }
        (1, _) => {
            let up = if c.quality == c.quality.triad() { 1 } else { 0 };
            let q = if up == 1 { c.quality.extended(if rng.chance(0.4) { 2 } else { 1 }) } else { c.quality.triad() };
            out[k] = roman_of(&Chord::new(c.root, q), key);
            format!("chord {}: {} -> {} ({})", k + 1, prog[k], out[k], if up == 1 { "extended" } else { "plain triad" })
        }
        (2, _) if out.len() >= 2 => {
            // the chord before becomes the dominant of this one
            let prev = (k + out.len() - 1) % out.len();
            let dom = Chord::new((c.root + 7) % 12, Quality::Dom7);
            let old = out[prev].clone();
            out[prev] = roman_of(&dom, key);
            format!("chord {}: {} -> {} (secondary dominant of {})", prev + 1, old, out[prev], prog[k])
        }
        (3, Some(_)) => {
            // modal interchange: borrow the chord with the opposite third from the parallel mode
            let q = if c.quality.is_minorish() { Quality::Maj } else { Quality::Min };
            out[k] = roman_of(&Chord::new(c.root, q.extended(ext)), key);
            format!("chord {}: {} -> {} (borrowed from the parallel mode)", k + 1, prog[k], out[k])
        }
        (4, _) => {
            let q = if rng.chance(0.5) { Quality::Sus4 } else { Quality::Sus2 };
            if c.quality.triad() == Quality::Dim {
                return (out, "kept a diminished chord".into());
            }
            out[k] = roman_of(&Chord::new(c.root, q), key);
            format!("chord {}: {} -> {} (suspension)", k + 1, prog[k], out[k])
        }
        (5, _) if out.len() >= 2 => {
            let j = (k + 1) % out.len();
            out.swap(k, j);
            format!("swapped chords {} and {}", k + 1, j + 1)
        }
        _ => {
            let r = rng.below(out.len().max(1));
            out.rotate_left(r);
            format!("rotated the progression by {r}")
        }
    };
    (out, what)
}
