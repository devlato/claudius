//! Pitch classes, modes and keys. Pitches are semitone numbers (60 = middle C); pitch classes are 0..11 (C = 0).

use serde::{Deserialize, Serialize};

pub const SHARP: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
pub const FLAT: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

pub fn pc_name(pc: u8, flats: bool) -> &'static str {
    if flats { FLAT[(pc % 12) as usize] } else { SHARP[(pc % 12) as usize] }
}

pub fn parse_pc(s: &str) -> Option<u8> {
    let s = s.trim();
    let mut ch = s.chars();
    let base = match ch.next()?.to_ascii_uppercase() {
        'C' => 0i32,
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
    Some((base + acc).rem_euclid(12) as u8)
}

pub fn note_name(p: i32) -> String {
    format!("{}{}", SHARP[p.rem_euclid(12) as usize], p.div_euclid(12) - 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    HarmonicMinor,
}

impl Mode {
    pub const ALL: [Mode; 7] =
        [Mode::Major, Mode::Minor, Mode::Dorian, Mode::Phrygian, Mode::Lydian, Mode::Mixolydian, Mode::HarmonicMinor];

    pub fn intervals(self) -> [i32; 7] {
        match self {
            Mode::Major => [0, 2, 4, 5, 7, 9, 11],
            Mode::Minor => [0, 2, 3, 5, 7, 8, 10],
            Mode::Dorian => [0, 2, 3, 5, 7, 9, 10],
            Mode::Phrygian => [0, 1, 3, 5, 7, 8, 10],
            Mode::Lydian => [0, 2, 4, 6, 7, 9, 11],
            Mode::Mixolydian => [0, 2, 4, 5, 7, 9, 10],
            Mode::HarmonicMinor => [0, 2, 3, 5, 7, 8, 11],
        }
    }

    pub fn is_minor(self) -> bool {
        matches!(self, Mode::Minor | Mode::Dorian | Mode::Phrygian | Mode::HarmonicMinor)
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Major => "major",
            Mode::Minor => "minor",
            Mode::Dorian => "dorian",
            Mode::Phrygian => "phrygian",
            Mode::Lydian => "lydian",
            Mode::Mixolydian => "mixolydian",
            Mode::HarmonicMinor => "harmonic_minor",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        Some(match s.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
            "" | "maj" | "major" | "ionian" => Mode::Major,
            "m" | "min" | "minor" | "aeolian" => Mode::Minor,
            "dorian" => Mode::Dorian,
            "phrygian" => Mode::Phrygian,
            "lydian" => Mode::Lydian,
            "mixolydian" => Mode::Mixolydian,
            "harmonic_minor" | "harmonic" => Mode::HarmonicMinor,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub tonic: u8,
    pub mode: Mode,
}

impl Key {
    pub fn new(tonic: u8, mode: Mode) -> Self {
        Key { tonic: tonic % 12, mode }
    }

    /// "Am", "A minor", "F# dorian", "Eb", "C#m"
    pub fn parse(s: &str) -> Option<Key> {
        let s = s.trim();
        let split =
            s.char_indices().skip(1).find(|(_, c)| !matches!(c, '#' | 'b' | '♯' | '♭')).map_or(s.len(), |(i, _)| i);
        let tonic = parse_pc(&s[..split])?;
        let rest = s[split..].trim();
        Some(Key { tonic, mode: Mode::parse(rest)? })
    }

    pub fn flats(&self) -> bool {
        // keys whose signatures use flats (relative major on the flat side of the circle of fifths)
        let major_tonic = if self.mode.is_minor() { (self.tonic + 3) % 12 } else { self.tonic };
        matches!(major_tonic, 5 | 10 | 3 | 8 | 1 | 6)
    }

    pub fn name(&self) -> String {
        format!("{} {}", pc_name(self.tonic, self.flats()), self.mode.name())
    }

    pub fn short(&self) -> String {
        let m = match self.mode {
            Mode::Major => "",
            Mode::Minor => "m",
            other => other.name(),
        };
        format!("{}{}", pc_name(self.tonic, self.flats()), if m.len() > 1 { format!(" {m}") } else { m.to_string() })
    }

    pub fn pcs(&self) -> [u8; 7] {
        let iv = self.mode.intervals();
        let mut out = [0u8; 7];
        for (o, i) in out.iter_mut().zip(iv) {
            *o = ((self.tonic as i32 + i).rem_euclid(12)) as u8;
        }
        out
    }

    /// pitch class of scale degree `d` (0-based; wraps)
    pub fn degree_pc(&self, d: i32) -> u8 {
        let iv = self.mode.intervals();
        ((self.tonic as i32 + iv[d.rem_euclid(7) as usize]).rem_euclid(12)) as u8
    }

    /// pitch of scale degree `d` counted from the tonic placed at or above `base`
    pub fn degree_pitch(&self, d: i32, base: i32) -> i32 {
        let t = base + (self.tonic as i32 - base).rem_euclid(12);
        let iv = self.mode.intervals();
        t + 12 * d.div_euclid(7) + iv[d.rem_euclid(7) as usize]
    }

    pub fn contains(&self, pitch: i32) -> bool {
        self.pcs().contains(&(pitch.rem_euclid(12) as u8))
    }

    /// diatonic index of a pitch (degree + 7 * octave) if it is in the scale
    pub fn index_of(&self, pitch: i32) -> Option<i32> {
        let pc = pitch.rem_euclid(12) as u8;
        let d = self.pcs().iter().position(|&p| p == pc)? as i32;
        let rel = pitch - self.tonic as i32;
        let oct = (rel - self.mode.intervals()[d as usize]).div_euclid(12);
        Some(d + 7 * oct)
    }

    pub fn from_index(&self, idx: i32) -> i32 {
        self.tonic as i32 + 12 * idx.div_euclid(7) + self.mode.intervals()[idx.rem_euclid(7) as usize]
    }

    /// nearest scale tone (ties go down)
    pub fn snap(&self, pitch: i32) -> i32 {
        for d in 0..7 {
            if self.contains(pitch - d) {
                return pitch - d;
            }
            if self.contains(pitch + d) {
                return pitch + d;
            }
        }
        pitch
    }

    /// move `steps` scale steps from a (snapped) pitch
    pub fn step(&self, pitch: i32, steps: i32) -> i32 {
        let p = self.snap(pitch);
        self.from_index(self.index_of(p).unwrap_or(0) + steps)
    }

    pub fn relative(&self) -> Key {
        match self.mode {
            Mode::Major => Key::new((self.tonic + 9) % 12, Mode::Minor),
            _ if self.mode.is_minor() => Key::new((self.tonic + 3) % 12, Mode::Major),
            _ => *self,
        }
    }

    /// pentatonic subset (major or minor flavour)
    pub fn pentatonic(&self) -> Vec<u8> {
        let iv: [i32; 5] = if self.mode.is_minor() { [0, 3, 5, 7, 10] } else { [0, 2, 4, 7, 9] };
        iv.iter().map(|i| ((self.tonic as i32 + i).rem_euclid(12)) as u8).collect()
    }

    /// semitones to move a part from `from` to this key along the shortest way (-6..=5)
    pub fn transpose_from(&self, from_tonic: u8) -> i32 {
        let d = (self.tonic as i32 - from_tonic as i32).rem_euclid(12);
        if d > 5 { d - 12 } else { d }
    }
}
