//! Chords: qualities, symbols ("Am7", "F/A"), Roman numerals relative to a key ("i", "bVI", "V7/V", "iiø7"),
//! and voicings chosen for smooth voice leading.

use serde::{Deserialize, Serialize};

use super::scale::{Key, parse_pc, pc_name};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Maj,
    Min,
    Dim,
    Aug,
    Sus2,
    Sus4,
    Dom7,
    Maj7,
    Min7,
    HalfDim7,
    Dim7,
    Maj6,
    Min6,
    Add9,
    MinAdd9,
    Dom9,
    Maj9,
    Min9,
    Min11,
    Power,
}

impl Quality {
    pub fn intervals(self) -> &'static [i32] {
        match self {
            Quality::Maj => &[0, 4, 7],
            Quality::Min => &[0, 3, 7],
            Quality::Dim => &[0, 3, 6],
            Quality::Aug => &[0, 4, 8],
            Quality::Sus2 => &[0, 2, 7],
            Quality::Sus4 => &[0, 5, 7],
            Quality::Dom7 => &[0, 4, 7, 10],
            Quality::Maj7 => &[0, 4, 7, 11],
            Quality::Min7 => &[0, 3, 7, 10],
            Quality::HalfDim7 => &[0, 3, 6, 10],
            Quality::Dim7 => &[0, 3, 6, 9],
            Quality::Maj6 => &[0, 4, 7, 9],
            Quality::Min6 => &[0, 3, 7, 9],
            Quality::Add9 => &[0, 4, 7, 14],
            Quality::MinAdd9 => &[0, 3, 7, 14],
            Quality::Dom9 => &[0, 4, 7, 10, 14],
            Quality::Maj9 => &[0, 4, 7, 11, 14],
            Quality::Min9 => &[0, 3, 7, 10, 14],
            Quality::Min11 => &[0, 3, 7, 10, 14, 17],
            Quality::Power => &[0, 7],
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Quality::Maj => "",
            Quality::Min => "m",
            Quality::Dim => "dim",
            Quality::Aug => "aug",
            Quality::Sus2 => "sus2",
            Quality::Sus4 => "sus4",
            Quality::Dom7 => "7",
            Quality::Maj7 => "maj7",
            Quality::Min7 => "m7",
            Quality::HalfDim7 => "m7b5",
            Quality::Dim7 => "dim7",
            Quality::Maj6 => "6",
            Quality::Min6 => "m6",
            Quality::Add9 => "add9",
            Quality::MinAdd9 => "madd9",
            Quality::Dom9 => "9",
            Quality::Maj9 => "maj9",
            Quality::Min9 => "m9",
            Quality::Min11 => "m11",
            Quality::Power => "5",
        }
    }

    const ALL: [Quality; 20] = [
        Quality::Maj,
        Quality::Min,
        Quality::Dim,
        Quality::Aug,
        Quality::Sus2,
        Quality::Sus4,
        Quality::Dom7,
        Quality::Maj7,
        Quality::Min7,
        Quality::HalfDim7,
        Quality::Dim7,
        Quality::Maj6,
        Quality::Min6,
        Quality::Add9,
        Quality::MinAdd9,
        Quality::Dom9,
        Quality::Maj9,
        Quality::Min9,
        Quality::Min11,
        Quality::Power,
    ];

    pub fn from_suffix(s: &str) -> Option<Quality> {
        let s = match s {
            "M" | "maj" => "",
            "min" | "-" => "m",
            "°" | "o" => "dim",
            "+" => "aug",
            "ø" | "ø7" | "m7♭5" => "m7b5",
            "°7" | "o7" => "dim7",
            "M7" | "Δ" | "Δ7" => "maj7",
            "-7" | "min7" => "m7",
            "M9" | "Δ9" => "maj9",
            "sus" => "sus4",
            other => other,
        };
        Quality::ALL.iter().copied().find(|q| q.suffix() == s)
    }

    pub fn is_minorish(self) -> bool {
        matches!(
            self,
            Quality::Min
                | Quality::Min7
                | Quality::Min6
                | Quality::MinAdd9
                | Quality::Min9
                | Quality::Min11
                | Quality::Dim
                | Quality::HalfDim7
                | Quality::Dim7
        )
    }

    /// richer / plainer variants of the same function
    pub fn extended(self, level: u8) -> Quality {
        match (self, level) {
            (Quality::Maj, 1) => Quality::Maj7,
            (Quality::Maj, 2) => Quality::Maj9,
            (Quality::Min, 1) => Quality::Min7,
            (Quality::Min, 2) => Quality::Min9,
            (Quality::Dim, 1 | 2) => Quality::HalfDim7,
            (Quality::Dom7, 2) => Quality::Dom9,
            (q, _) => q,
        }
    }

    pub fn triad(self) -> Quality {
        match self {
            Quality::Maj7 | Quality::Maj9 | Quality::Maj6 | Quality::Add9 | Quality::Dom7 | Quality::Dom9 => {
                Quality::Maj
            }
            Quality::Min7 | Quality::Min9 | Quality::Min11 | Quality::Min6 | Quality::MinAdd9 => Quality::Min,
            Quality::HalfDim7 | Quality::Dim7 => Quality::Dim,
            q => q,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chord {
    pub root: u8,
    pub quality: Quality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass: Option<u8>,
}

impl Chord {
    pub fn new(root: u8, quality: Quality) -> Self {
        Chord { root: root % 12, quality, bass: None }
    }

    pub fn symbol(&self, flats: bool) -> String {
        let mut s = format!("{}{}", pc_name(self.root, flats), self.quality.suffix());
        if let Some(b) = self.bass
            && b != self.root
        {
            s.push('/');
            s.push_str(pc_name(b, flats));
        }
        s
    }

    pub fn parse(s: &str) -> Option<Chord> {
        let s = s.trim();
        let (main, bass) = match s.split_once('/') {
            Some((m, b)) => (m, parse_pc(b)),
            None => (s, None),
        };
        let split = main
            .char_indices()
            .skip(1)
            .find(|(_, c)| !matches!(c, '#' | 'b' | '♯' | '♭'))
            .map_or(main.len(), |(i, _)| i);
        let root = parse_pc(&main[..split])?;
        let quality = Quality::from_suffix(&main[split..])?;
        Some(Chord { root, quality, bass })
    }

    /// intervals from the root (may exceed 12 for extensions)
    pub fn tones(&self) -> Vec<i32> {
        self.quality.intervals().to_vec()
    }

    pub fn pcs(&self) -> Vec<u8> {
        let mut v: Vec<u8> = self.tones().iter().map(|i| ((self.root as i32 + i).rem_euclid(12)) as u8).collect();
        v.dedup();
        v
    }

    pub fn has_pc(&self, pc: u8) -> bool {
        self.pcs().contains(&(pc % 12))
    }

    pub fn bass_pc(&self) -> u8 {
        self.bass.unwrap_or(self.root)
    }

    pub fn third_pc(&self) -> u8 {
        let t = self.quality.intervals().get(1).copied().unwrap_or(4);
        ((self.root as i32 + t) % 12) as u8
    }

    pub fn fifth_pc(&self) -> u8 {
        let t = self.quality.intervals().iter().copied().find(|i| (6..=8).contains(i)).unwrap_or(7);
        ((self.root as i32 + t) % 12) as u8
    }
}

fn numeral_degree(s: &str) -> Option<(i32, bool, usize)> {
    const NUMS: [&str; 7] = ["VII", "III", "VI", "IV", "II", "V", "I"];
    const DEG: [i32; 7] = [6, 2, 5, 3, 1, 4, 0];
    for (n, d) in NUMS.iter().zip(DEG) {
        if let Some(head) = s.get(..n.len())
            && head.eq_ignore_ascii_case(n)
        {
            let upper = head.chars().next().is_some_and(|c| c.is_ascii_uppercase());
            return Some((d, upper, n.len()));
        }
    }
    None
}

/// a Roman numeral in `key`: degrees are counted in the key's own mode (so "VI" in A minor is F major), case gives
/// major / minor, a b / # prefix shifts the root, a suffix sets the quality ("7", "maj7", "9", "°", "ø7", "sus4"),
/// "/x" makes it an applied chord ("V7/V").
pub fn roman(rn: &str, key: &Key) -> Option<Chord> {
    let rn = rn.trim();
    if let Some((head, target)) = rn.split_once('/') {
        if numeral_degree(target.trim_start_matches(['b', '#'])).is_some() {
            let t = roman(target, key)?;
            let tonic_key = Key::new(
                t.root,
                if t.quality.is_minorish() { super::scale::Mode::Minor } else { super::scale::Mode::Major },
            );
            return roman(head, &tonic_key);
        }
        let bass = parse_pc(target);
        return roman(head, key).map(|c| Chord { bass, ..c });
    }
    let mut shift = 0;
    let mut s = rn;
    while let Some(c) = s.chars().next() {
        match c {
            'b' | '♭' => shift -= 1,
            '#' | '♯' => shift += 1,
            _ => break,
        }
        s = &s[c.len_utf8()..];
    }
    let (deg, upper, used) = numeral_degree(s)?;
    let suffix = &s[used..];
    let root = ((key.degree_pc(deg) as i32 + shift).rem_euclid(12)) as u8;
    let quality = match suffix {
        "" => {
            if upper {
                Quality::Maj
            } else {
                Quality::Min
            }
        }
        "°" | "o" | "dim" => Quality::Dim,
        "ø" | "ø7" | "m7b5" => Quality::HalfDim7,
        "°7" | "o7" | "dim7" => Quality::Dim7,
        "+" | "aug" => Quality::Aug,
        "7" => {
            if upper {
                Quality::Dom7
            } else {
                Quality::Min7
            }
        }
        "9" => {
            if upper {
                Quality::Dom9
            } else {
                Quality::Min9
            }
        }
        "6" => {
            if upper {
                Quality::Maj6
            } else {
                Quality::Min6
            }
        }
        "add9" => {
            if upper {
                Quality::Add9
            } else {
                Quality::MinAdd9
            }
        }
        "11" => Quality::Min11,
        other => Quality::from_suffix(other)?,
    };
    Some(Chord::new(root, quality))
}

/// the chord and the bass note of a slash chord numeral ("VI/A"); None for plain and applied chords ("V7/V")
pub fn slash_bass(rn: &str) -> Option<(&str, u8)> {
    let (head, target) = rn.trim().split_once('/')?;
    if numeral_degree(target.trim_start_matches(['b', '#', '♭', '♯'])).is_some() {
        return None;
    }
    Some((head, parse_pc(target)?))
}

/// the Roman numeral of a chord in a key (diatonic degree when the root is in the scale, else b/# of the nearest),
/// with its bass note when that is not the root ("VI/A")
pub fn roman_of(c: &Chord, key: &Key) -> String {
    const UP: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
    let pcs = key.pcs();
    let (deg, acc) = match pcs.iter().position(|&p| p == c.root) {
        Some(d) => (d, ""),
        None => match pcs.iter().position(|&p| p == (c.root + 1) % 12) {
            Some(d) => (d, "b"),
            None => (pcs.iter().position(|&p| (p + 1) % 12 == c.root).unwrap_or(0), "#"),
        },
    };
    let minor = c.quality.is_minorish();
    let num = if minor { UP[deg].to_lowercase() } else { UP[deg].to_string() };
    let suf = match c.quality {
        Quality::Maj | Quality::Min => "",
        Quality::Dim => "°",
        Quality::HalfDim7 => "ø7",
        Quality::Dom7 | Quality::Min7 => "7",
        Quality::Dom9 | Quality::Min9 => "9",
        Quality::Maj6 | Quality::Min6 => "6",
        Quality::Add9 | Quality::MinAdd9 => "add9",
        q => q.suffix(),
    };
    match c.bass {
        Some(b) if b != c.root => format!("{acc}{num}{suf}/{}", pc_name(b, key.flats())),
        _ => format!("{acc}{num}{suf}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Voicing {
    Close,
    Open,
    Spread,
}

/// choose pitches for `chord` within [low, high] with `n` voices: candidates are scored by voice-leading distance
/// from `prev`, span for the style, and mud (close intervals in the low register)
pub fn voice(
    chord: &Chord,
    prev: Option<&[i32]>,
    low: i32,
    high: i32,
    n: usize,
    style: Voicing,
    rng: Option<&mut Rng>,
) -> Vec<i32> {
    let mut tones: Vec<u8> = chord.pcs();
    // drop the fifth, then the root, when there are more tones than voices (the bass covers the root)
    while tones.len() > n {
        let fifth = chord.fifth_pc();
        if let Some(i) = tones.iter().position(|&t| t == fifth) {
            tones.remove(i);
        } else if let Some(i) = tones.iter().position(|&t| t == chord.root) {
            tones.remove(i);
        } else {
            tones.pop();
        }
    }
    // fewer tones than voices: try doubling each tone (root first, the third last) and keep the best voicing
    let mut doubles: Vec<u8> = vec![chord.root, chord.fifth_pc(), chord.third_pc()];
    doubles.extend(tones.iter().copied());
    doubles.retain(|d| tones.contains(d));
    doubles.dedup();
    let wants: Vec<(Vec<u8>, f64)> = if tones.len() >= n {
        vec![(tones.clone(), 0.0)]
    } else {
        doubles
            .iter()
            .enumerate()
            .map(|(rank, &d)| {
                let mut w = tones.clone();
                while w.len() < n {
                    w.push(d);
                }
                (w, rank as f64 * 0.8)
            })
            .collect()
    };
    let mut best: Vec<(f64, Vec<i32>)> = Vec::new();
    for (want, cost) in &wants {
        voicings(want, prev, low, high, style, *cost, &mut best);
    }
    best.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    best.dedup_by(|a, b| a.1 == b.1);
    let close_second = best.len() > 1 && best[1].0 - best[0].0 < 1.0;
    let take_second = close_second && rng.is_some_and(|r| r.chance(0.3));
    if take_second {
        return best[1].1.clone();
    }
    best.first().map(|b| b.1.clone()).unwrap_or_else(|| {
        let mut v: Vec<i32> = tones.iter().map(|&pc| low + (pc as i32 - low).rem_euclid(12)).collect();
        v.sort();
        v.dedup();
        v
    })
}

/// every distinct voicing of `want` within [low, high], scored (lower is better) into `best`
fn voicings(
    want: &[u8],
    prev: Option<&[i32]>,
    low: i32,
    high: i32,
    style: Voicing,
    cost: f64,
    best: &mut Vec<(f64, Vec<i32>)>,
) {
    let options: Vec<Vec<i32>> =
        want.iter().map(|&pc| (low..=high).filter(|p| p.rem_euclid(12) as u8 == pc).collect()).collect();
    let (span_lo, span_hi) = match style {
        Voicing::Close => (3, 14),
        Voicing::Open => (10, 24),
        Voicing::Spread => (17, 34),
    };
    let mut idx = vec![0usize; options.len()];
    if options.iter().any(|o| o.is_empty()) {
        return;
    }
    loop {
        let mut v: Vec<i32> = idx.iter().zip(&options).map(|(i, o)| o[*i]).collect();
        v.sort();
        let distinct = v.windows(2).all(|w| w[1] > w[0]);
        if distinct {
            let span = v[v.len() - 1] - v[0];
            let mut score = cost;
            if span < span_lo {
                score += (span_lo - span) as f64 * 1.5;
            }
            if span > span_hi {
                score += (span - span_hi) as f64 * 1.5;
            }
            for w in v.windows(2) {
                if w[0] < 52 && w[1] - w[0] < 5 {
                    score += 3.0;
                }
                if w[1] - w[0] == 1 && w[0] < 64 {
                    score += 1.5;
                }
            }
            if let Some(p) = prev {
                if p.len() == v.len() {
                    score += v.iter().zip(p).map(|(a, b)| (a - b).abs() as f64).sum::<f64>() * 0.6;
                } else if !p.is_empty() {
                    let pm = p.iter().sum::<i32>() as f64 / p.len() as f64;
                    let vm = v.iter().sum::<i32>() as f64 / v.len() as f64;
                    score += (pm - vm).abs() * 0.6;
                }
            } else {
                let centre = (low + high) as f64 / 2.0;
                let vm = v.iter().sum::<i32>() as f64 / v.len() as f64;
                score += (vm - centre).abs() * 0.3;
            }
            best.push((score, v));
        }
        let mut k = 0;
        loop {
            if k == idx.len() {
                return;
            }
            idx[k] += 1;
            if idx[k] < options[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
    }
}
