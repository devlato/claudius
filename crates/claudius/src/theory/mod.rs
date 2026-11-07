//! Music theory: keys and modes, chords and voicings, progressions, rhythm, bass and melody writing.

pub mod bass;
pub mod chord;
pub mod harmonize;
pub mod idiom;
pub mod melody;
pub mod progression;
pub mod rhythm;
pub mod scale;

use serde::{Deserialize, Serialize};

use chord::Chord;

/// a note on the timeline: start and length in beats, pitch in semitones (60 = middle C), velocity 0..1, and for a
/// drum stroke what its step locks in the sound
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub t: f64,
    pub d: f64,
    pub p: i32,
    pub v: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<Lock>,
}

/// What a drum stroke changes from its lane's sound (a trig's locks, `rhythm::Trig`): where it lands off the beat
/// after swing (beats), its decay (times the lane's), its tone (-1 darker to 1 brighter), its pan (-1 to 1)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lock {
    pub nudge: f64,
    pub decay: f32,
    pub tone: f32,
    pub pan: f32,
}

impl Default for Lock {
    fn default() -> Lock {
        Lock { nudge: 0.0, decay: 1.0, tone: 0.0, pan: 0.0 }
    }
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
