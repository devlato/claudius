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
            "i VII VI V",
            "VI VII i v",
            "i VI III VII i VI iv V",
        ],
    },
    Style {
        name: "minor_dark",
        mode: Mode::Minor,
        progs: &[
            "i i VI VII",
            "i VI iv V",
            "i iv i v",
            "i VI i VII",
            "i v VI iv",
            "i VII VI V",
            "i VI bII V",
            "i iv V7 i",
        ],
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
            "i7 IV9 VImaj7 V7",
            "VImaj7 VII7 i7 i7",
            "iv9 v7 i7 VImaj7",
        ],
    },
    Style {
        name: "dorian_vamp",
        mode: Mode::Dorian,
        progs: &["i7 IV7", "i7 IV9 i7 VIImaj7", "i9 IV7", "i7 ii7 i7 IV7", "i7 IV7 i7 v7", "i7 VIImaj7 IV7 IV7"],
    },
    Style {
        name: "phrygian",
        mode: Mode::Phrygian,
        progs: &["i II", "i II i VII", "i i II i", "i vii i II", "i II VII i", "i II III II"],
    },
    Style {
        name: "harmonic",
        mode: Mode::HarmonicMinor,
        progs: &["i iv V i", "i VI V V", "i iv VI V", "i VI iv V", "i VI ii° V", "iv V i VI"],
    },
    Style {
        name: "minimal",
        mode: Mode::Minor,
        progs: &["i", "i i i VI", "i i iv iv", "i9 i9 i9 VII", "i i VII VII", "i i VI VI"],
    },
    Style {
        name: "major_pop",
        mode: Mode::Major,
        progs: &[
            "I V vi IV",
            "vi IV I V",
            "I vi IV V",
            "IV I V vi",
            "I IV vi V",
            "I iii vi IV",
            "IV V vi I",
            "I iii IV iv",
            "I bVII IV I",
            "I V/vi vi IV",
            "I bVI bVII I",
            "IV V iii vi",
            "I V vi iii IV I IV V",
        ],
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
            "Imaj7 VI7 ii7 V7",
            "IVmaj7 iv7 iii7 VI7",
            "ii9 bII7 Imaj9 Imaj9",
        ],
    },
    Style {
        name: "minor_80s",
        mode: Mode::Minor,
        progs: &[
            "i VI III VII",
            "i VII VI VII",
            "VI VII i i",
            "i III VII IV",
            "i VI VII i",
            "VI VII i III",
            "i VII VI iv",
        ],
    },
    Style {
        name: "mixolydian",
        mode: Mode::Mixolydian,
        progs: &["I VII IV I", "I VII", "I IV VII IV", "I VII IV IV", "I v IV I"],
    },
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

/// diatonic chord on degree d with `ext` (0 triad, 1 seventh, 2 ninth), with the key's own notes: the seventh the
/// key has above the root (a dominant seventh on the fifth of a major key, a diminished one on the seventh of a
/// harmonic minor), a ninth only where the key has one (the fifth of a minor key stays a seventh), and a triad where
/// no seventh fits (the tonic of a harmonic minor, whose seventh is major over a minor third)
pub fn diatonic(key: &Key, d: i32, ext: u8) -> Chord {
    let iv = key.mode.intervals();
    let seventh = (iv[(d + 6).rem_euclid(7) as usize] - iv[d.rem_euclid(7) as usize]).rem_euclid(12);
    let ninth = key.pcs().contains(&((key.degree_pc(d) + 2) % 12));
    let with_seventh = match (diatonic_quality(key, d), seventh) {
        (Quality::Maj, 10) => Some(Quality::Dom7),
        (Quality::Maj, 11) => Some(Quality::Maj7),
        (Quality::Min, 10) => Some(Quality::Min7),
        (Quality::Dim, 10) => Some(Quality::HalfDim7),
        (Quality::Dim, 9) => Some(Quality::Dim7),
        _ => None,
    };
    let q = match (ext, with_seventh) {
        (0, _) | (_, None) => diatonic_quality(key, d),
        (1, Some(s)) => s,
        (_, Some(Quality::Dom7)) if ninth => Quality::Dom9,
        (_, Some(Quality::Maj7)) if ninth => Quality::Maj9,
        (_, Some(Quality::Min7)) if ninth => Quality::Min9,
        (_, Some(s)) => s,
    };
    Chord::new(key.degree_pc(d), q)
}

/// `c` richer by `level` (1 a seventh, 2 a ninth) with the key's own notes (a dominant seventh on the fifth of a major
/// key, never a major seventh), or `None` for a chord the key does not have: a borrowed or applied chord has no
/// seventh of the key's to add
fn richer(c: &Chord, key: &Key, level: u8) -> Option<Chord> {
    let d = key.pcs().iter().position(|&p| p == c.root)? as i32;
    (diatonic(key, d, 0).quality == c.quality.triad()).then(|| diatonic(key, d, level))
}

/// whether `c` can be suspended to `q` (sus2 or sus4) with the key's own notes
fn suspends(c: &Chord, key: &Key, q: Quality) -> bool {
    let pcs = key.pcs();
    let added = if q == Quality::Sus4 { 5 } else { 2 };
    matches!(c.quality.triad(), Quality::Maj | Quality::Min)
        && pcs.contains(&((c.root + added) % 12))
        && pcs.contains(&((c.root + 7) % 12))
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
            let up = c.quality == c.quality.triad();
            let q = if up {
                match richer(&c, key, if rng.chance(0.4) { 2 } else { 1 }) {
                    Some(r) => r.quality,
                    None => return (out, format!("kept chord {}: nothing richer in the key", k + 1)),
                }
            } else {
                c.quality.triad()
            };
            out[k] = roman_of(&Chord::new(c.root, q), key);
            format!("chord {}: {} -> {} ({})", k + 1, prog[k], out[k], if up { "extended" } else { "plain triad" })
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
            if c.quality.triad() == Quality::Dim {
                return (out, "kept a diminished chord".into());
            }
            let qs: Vec<Quality> =
                [Quality::Sus4, Quality::Sus2].into_iter().filter(|&q| suspends(&c, key, q)).collect();
            if qs.is_empty() {
                return (out, format!("kept chord {}: no suspension in the key", k + 1));
            }
            out[k] = roman_of(&Chord::new(c.root, qs[rng.below(qs.len())]), key);
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

/// A four-chord loop answered, as a player bends it the second time round: the loop and then the loop again, ending
/// another way back to its first chord (its dominant; on the tonic, a chord borrowed from the parallel mode or the
/// flat sixth and seventh; a suspension of its last chord), and half the time with one chord in between coloured (a
/// seventh, a ninth, a suspension). `None` for a loop of another length. Returns the eight chords and what the answer
/// did.
pub fn answer(prog: &[String], key: &Key, rng: &mut Rng) -> Option<(Vec<String>, String)> {
    if prog.len() != 4 {
        return None;
    }
    let cs = chords(prog, key);
    let (first, last) = (cs[0], cs[3]);
    let t = key.tonic;
    // a borrowed chord as the mode it comes from has it, a seventh in a loop of sevenths
    let ext = u8::from(cs.iter().any(|c| c.quality != c.quality.triad()));
    let from = |mode: Mode, d: i32| diatonic(&Key::new(t, mode), d, ext);
    let mut ways: Vec<(f64, Vec<Chord>, &str)> = Vec::new();
    if first.quality.triad() != Quality::Dim {
        ways.push((2.0, vec![Chord::new((first.root + 7) % 12, Quality::Dom7)], "its dominant"));
    }
    if first.root == t {
        if key.mode.is_minor() {
            ways.push((1.0, vec![from(Mode::Dorian, 3)], "the major fourth, borrowed"));
            ways.push((1.0, vec![from(Mode::Minor, 5), from(Mode::Minor, 6)], "the sixth and the seventh"));
            ways.push((0.5, vec![from(Mode::Phrygian, 1)], "the flat second"));
        } else {
            ways.push((1.0, vec![from(Mode::Minor, 6)], "the flat seventh, borrowed"));
            ways.push((1.0, vec![from(Mode::Minor, 3)], "the minor fourth, borrowed"));
            ways.push((1.0, vec![from(Mode::Minor, 5), from(Mode::Minor, 6)], "the flat sixth and seventh"));
        }
    }
    if suspends(&last, key, Quality::Sus4) {
        ways.push((1.0, vec![Chord::new(last.root, Quality::Sus4)], "a suspension"));
    }
    // only a new way: a tail it already ends on is no answer
    ways.retain(|(_, tail, _)| tail.as_slice() != &cs[4 - tail.len()..]);
    if ways.is_empty() {
        return None;
    }
    let weights: Vec<f64> = ways.iter().map(|w| w.0).collect();
    let (_, tail, how) = &ways[rng.weighted(&weights)];
    let mut second = prog.to_vec();
    for (i, c) in tail.iter().enumerate() {
        second[4 - tail.len() + i] = roman_of(c, key);
    }
    let mut what = format!("answered, ending on {how} ({})", second[4 - tail.len()..].join(" "));
    // a chord between the first and the new ending, coloured with the key's own notes
    let inner: Vec<usize> = (1..4 - tail.len()).filter(|&i| cs[i].quality.triad() != Quality::Dim).collect();
    if !inner.is_empty() && rng.chance(0.5) {
        let i = inner[rng.below(inner.len())];
        let c = cs[i];
        let triad = c.quality == c.quality.triad();
        let mut colours: Vec<Chord> = [Quality::Sus4, Quality::Sus2]
            .into_iter()
            .filter(|&q| triad && suspends(&c, key, q))
            .map(|q| Chord::new(c.root, q))
            .collect();
        colours.extend(richer(&c, key, if triad { 1 } else { 2 }));
        if let Some(&coloured) = colours.get(rng.below(colours.len().max(1)))
            && coloured != c
        {
            second[i] = roman_of(&coloured, key);
            what.push_str(&format!(", chord {} coloured ({})", i + 5, second[i]));
        }
    }
    let mut out = prog.to_vec();
    out.extend(second);
    Some((out, what))
}
