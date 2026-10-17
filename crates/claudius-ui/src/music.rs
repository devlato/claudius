//! Just enough music theory to label things: keys, Roman numerals as chord names (degrees counted in the key's own
//! mode, as claudius does: "VI" in A minor is F major), and note names.

pub const SHARP: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
pub const FLAT: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
/// tonic choices offered by the key picker
pub const TONICS: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
pub const MODES: [(&str, [i32; 7]); 7] = [
    ("major", [0, 2, 4, 5, 7, 9, 11]),
    ("minor", [0, 2, 3, 5, 7, 8, 10]),
    ("dorian", [0, 2, 3, 5, 7, 9, 10]),
    ("phrygian", [0, 1, 3, 5, 7, 8, 10]),
    ("lydian", [0, 2, 4, 6, 7, 9, 11]),
    ("mixolydian", [0, 2, 4, 5, 7, 9, 10]),
    ("harmonic_minor", [0, 2, 3, 5, 7, 8, 11]),
];

pub fn parse_pc(s: &str) -> Option<i32> {
    let mut ch = s.trim().chars();
    let base = match ch.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let acc: i32 =
        ch.take_while(|c| matches!(c, '#' | 'b' | '♯' | '♭')).map(|c| if c == '#' || c == '♯' { 1 } else { -1 }).sum();
    Some((base + acc).rem_euclid(12))
}

pub fn note_name(midi: i32) -> String {
    format!("{}{}", SHARP[midi.rem_euclid(12) as usize], midi.div_euclid(12) - 1)
}

/// "A1", "C#4", "Eb-1" or a MIDI number
pub fn parse_note(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i32>() {
        return Some(n);
    }
    let split = s.char_indices().find(|(i, c)| *i > 0 && (c.is_ascii_digit() || *c == '-')).map(|(i, _)| i)?;
    let octave: i32 = s[split..].parse().ok()?;
    Some(parse_pc(&s[..split])? + (octave + 1) * 12)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub tonic: i32,
    /// index into `MODES`
    pub mode: usize,
}

impl Key {
    /// "D minor", "F# dorian", "A harmonic minor", "Bb", "C#m"
    pub fn parse(s: &str) -> Option<Key> {
        let s = s.trim();
        let split =
            s.char_indices().skip(1).find(|(_, c)| !matches!(c, '#' | 'b' | '♯' | '♭')).map_or(s.len(), |(i, _)| i);
        let tonic = parse_pc(&s[..split])?;
        let rest = s[split..].trim().to_ascii_lowercase().replace([' ', '-'], "_");
        let mode = match rest.as_str() {
            "" | "maj" | "major" | "ionian" => "major",
            "m" | "min" | "minor" | "aeolian" => "minor",
            "harmonic" | "harmonic_minor" => "harmonic_minor",
            other => other,
        };
        let mode = MODES.iter().position(|(n, _)| *n == mode)?;
        Some(Key { tonic, mode })
    }

    pub fn intervals(&self) -> [i32; 7] {
        MODES[self.mode].1
    }

    pub fn mode_name(&self) -> &'static str {
        MODES[self.mode].0
    }

    pub fn is_minor(&self) -> bool {
        matches!(self.mode_name(), "minor" | "dorian" | "phrygian" | "harmonic_minor")
    }

    /// keys whose signatures use flats (relative major on the flat side of the circle of fifths)
    pub fn flats(&self) -> bool {
        let major = if self.is_minor() { (self.tonic + 3) % 12 } else { self.tonic };
        matches!(major, 5 | 10 | 3 | 8 | 1 | 6)
    }

    pub fn pc_name(&self, pc: i32) -> &'static str {
        let i = pc.rem_euclid(12) as usize;
        if self.flats() { FLAT[i] } else { SHARP[i] }
    }

    /// as the server writes it ("D minor")
    pub fn name(&self) -> String {
        format!("{} {}", self.pc_name(self.tonic), self.mode_name())
    }

    pub fn degree_pc(&self, degree: usize) -> i32 {
        (self.tonic + self.intervals()[degree % 7]).rem_euclid(12)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    pub root: i32,
    /// intervals above the root
    pub tones: Vec<i32>,
    /// "Dm7", "Bbmaj7"
    pub name: String,
}

const QUALITIES: [(&str, &[i32]); 20] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("m7b5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("add9", &[0, 4, 7, 14]),
    ("madd9", &[0, 3, 7, 14]),
    ("9", &[0, 4, 7, 10, 14]),
    ("maj9", &[0, 4, 7, 11, 14]),
    ("m9", &[0, 3, 7, 10, 14]),
    ("m11", &[0, 3, 7, 10, 14, 17]),
    ("5", &[0, 7]),
];

fn quality(suffix: &str) -> Option<(&'static str, &'static [i32])> {
    let s = match suffix {
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
    QUALITIES.iter().find(|(q, _)| *q == s).copied()
}

fn numeral_degree(s: &str) -> Option<(usize, bool, usize)> {
    const NUMS: [(&str, usize); 7] = [("VII", 6), ("III", 2), ("VI", 5), ("IV", 3), ("II", 1), ("V", 4), ("I", 0)];
    for (n, d) in NUMS {
        if let Some(head) = s.get(..n.len())
            && head.eq_ignore_ascii_case(n)
        {
            let upper = head.chars().next().is_some_and(|c| c.is_ascii_uppercase());
            return Some((d, upper, n.len()));
        }
    }
    None
}

/// a Roman numeral in `key` ("i7", "VImaj7", "bVII", "V7/V", "iiø7")
pub fn chord(numeral: &str, key: &Key) -> Option<Chord> {
    let rn = numeral.trim();
    if let Some((head, target)) = rn.split_once('/') {
        let t = chord(target, key)?;
        let minorish = t.tones.get(1) == Some(&3);
        let local = Key { tonic: t.root, mode: if minorish { 1 } else { 0 } };
        let c = chord(head, &local)?;
        return Some(Chord { name: key_spelled(&c, key), ..c });
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
    let (degree, upper, used) = numeral_degree(s)?;
    let suffix = &s[used..];
    let root = (key.degree_pc(degree) + shift).rem_euclid(12);
    let (q, tones) = match suffix {
        "" => quality(if upper { "" } else { "m" })?,
        "°" | "o" | "dim" => quality("dim")?,
        "ø" | "ø7" | "m7b5" => quality("m7b5")?,
        "°7" | "o7" | "dim7" => quality("dim7")?,
        "+" | "aug" => quality("aug")?,
        "7" => quality(if upper { "7" } else { "m7" })?,
        "9" => quality(if upper { "9" } else { "m9" })?,
        "6" => quality(if upper { "6" } else { "m6" })?,
        "add9" => quality(if upper { "add9" } else { "madd9" })?,
        "11" => quality("m11")?,
        other => quality(other)?,
    };
    let c = Chord { root, tones: tones.to_vec(), name: String::new() };
    let name = format!("{}{}", key.pc_name(root), q);
    Some(Chord { name, ..c })
}

fn key_spelled(c: &Chord, key: &Key) -> String {
    let q = QUALITIES.iter().find(|(_, t)| *t == c.tones.as_slice()).map_or("", |(q, _)| q);
    format!("{}{}", key.pc_name(c.root), q)
}

/// the diatonic chords of a key as Roman numerals (triads, or sevenths)
pub fn diatonic(key: &Key, sevenths: bool) -> Vec<String> {
    const UP: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
    let iv = key.intervals();
    (0..7)
        .map(|d| {
            let at = |k: usize| (iv[(d + k) % 7] - iv[d]).rem_euclid(12);
            let (third, fifth, seventh) = (at(2), at(4), at(6));
            let minor = third == 3;
            let num = if minor { UP[d].to_lowercase() } else { UP[d].to_string() };
            let suffix = match (third, fifth, sevenths.then_some(seventh)) {
                (3, 6, Some(10)) => "ø7",
                (3, 6, Some(9)) => "°7",
                (3, 6, _) => "°",
                (4, 8, _) => "+",
                (4, 7, Some(11)) => "maj7",
                (_, 7, Some(10)) => "7",
                _ => "",
            };
            format!("{num}{suffix}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerals_follow_the_mode() {
        let d_minor = Key::parse("D minor").unwrap();
        let names: Vec<String> =
            ["i7", "VImaj7", "iv7", "v7"].iter().map(|n| chord(n, &d_minor).unwrap().name).collect();
        assert_eq!(names, ["Dm7", "Bbmaj7", "Gm7", "Am7"]);
        assert_eq!(chord("V7/V", &Key::parse("C major").unwrap()).unwrap().root, 2);
        assert_eq!(Key::parse("f# dorian").unwrap().name(), "F# dorian");
        assert_eq!(Key::parse("A harmonic minor").unwrap().mode_name(), "harmonic_minor");
        assert_eq!(diatonic(&Key::parse("A minor").unwrap(), true)[1], "iiø7");
        assert_eq!(note_name(33), "A1");
        assert_eq!(parse_note("A1"), Some(33));
        assert_eq!(parse_note("c#4"), Some(61));
        assert_eq!(parse_note("64"), Some(64));
    }
}
