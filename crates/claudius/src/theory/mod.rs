//! Music theory: keys and modes, chords and voicings, progressions, rhythm, bass and melody writing.

pub mod bass;
pub mod chord;
pub mod melody;
pub mod progression;
pub mod rhythm;
pub mod scale;

use serde::{Deserialize, Serialize};

use chord::Chord;

/// a note on the timeline: start and length in beats, pitch in semitones (60 = middle C), velocity 0..1
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub t: f64,
    pub d: f64,
    pub p: i32,
    pub v: f32,
}

/// a chord sounding from `start` for `len` beats
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChordSpan {
    pub start: f64,
    pub len: f64,
    pub chord: Chord,
}

pub fn chord_at(spans: &[ChordSpan], t: f64) -> Option<&ChordSpan> {
    spans.iter().find(|c| t >= c.start - 1e-9 && t < c.start + c.len - 1e-9).or_else(|| spans.last())
}
