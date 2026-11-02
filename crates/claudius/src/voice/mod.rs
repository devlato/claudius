//! A voice: its pitch every 10 ms (`pitch`), the notes it means (`intent`), and how its singer sings (`singer`). An
//! assessment reads a recording's notes as anyone sings them, learns the singer from how near they came, and reads the
//! notes again as that singer sings them.

pub mod check;
pub mod intent;
pub mod pitch;
pub mod singer;
pub mod tune;

use crate::audio::Stereo;
use crate::theory::scale::note_name;
use intent::{Meant, Scale};
use pitch::{FRAME_SECS, Frame};
use singer::{Heard, Voice};

/// a note as an assessment heard it
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Reading {
    /// where it starts and how long it lasts, seconds
    pub at: f64,
    pub secs: f64,
    /// the note meant, where it was sung (a fractional MIDI note), and how sure the reading is (0 to 1)
    pub note: i32,
    pub sung: f32,
    pub sure: f32,
}

impl Reading {
    /// how far it was sung from the note meant, cents (below 0: flat)
    pub fn cents(&self) -> f32 {
        (self.sung - self.note as f32) * 100.0
    }
}

/// how a recording is sung: the singer learnt from it, and its notes
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Assessment {
    pub voice: Voice,
    pub notes: Vec<Reading>,
}

impl Assessment {
    /// the share of its notes sung within `cents` of the note meant
    pub fn within(&self, cents: f32) -> f32 {
        let near = self.notes.iter().filter(|n| n.cents().abs() <= cents).count();
        near as f32 / self.notes.len().max(1) as f32
    }

    /// in words: how many notes, how many near the note, the voice, and the furthest off
    pub fn describe(&self) -> String {
        if self.notes.is_empty() {
            return "no notes to hear: it wants singing, one note at a time".into();
        }
        let mut off: Vec<&Reading> = self.notes.iter().filter(|n| n.cents().abs() > 25.0).collect();
        off.sort_by(|a, b| b.cents().abs().total_cmp(&a.cents().abs()));
        let furthest: Vec<String> = off
            .iter()
            .take(3)
            .map(|n| format!("{} at {:.1} s ({:+.0} cents)", note_name(n.note), n.at, n.cents()))
            .collect();
        let worst =
            if furthest.is_empty() { String::new() } else { format!("; furthest off: {}", furthest.join(", ")) };
        let n = self.notes.len();
        format!(
            "{n} note{}, {:.0}% within 25 cents of the note meant and {:.0}% within 50; {}{worst}",
            if n == 1 { "" } else { "s" },
            self.within(25.0) * 100.0,
            self.within(50.0) * 100.0,
            self.voice.describe()
        )
    }
}

/// How the voice in `x` sings, from its singing alone, in a song that makes `scale` likelier: its notes read as anyone
/// sings them, the singer learnt from how near they came, and the notes read again as that singer sings them (twice
/// over, each reading sharper than the one before).
pub fn assess(x: &Stereo, scale: &Scale) -> Assessment {
    let frames = pitch::track(x);
    let (voice, meant) = learn(&frames, scale);
    let notes = meant
        .iter()
        .map(|m| Reading { at: pitch::time(m.start), secs: m.secs(), note: m.note, sung: m.sung, sure: m.sure })
        .collect();
    Assessment { voice, notes }
}

/// the singer in `frames` and the notes they mean, learnt from the singing alone as `assess` learns them
pub fn learn(frames: &[Frame], scale: &Scale) -> (Voice, Vec<Meant>) {
    let mut voice = Voice::default();
    let mut meant = intent::decode(frames, &voice, scale);
    for _ in 0..2 {
        let heard = heard(frames, &meant);
        if heard.is_empty() {
            break;
        }
        voice = singer::fit(&heard);
        meant = intent::decode(frames, &voice, scale);
    }
    (voice, meant)
}

/// the notes meant, as heard against them: where each was held, how long it took to settle, its vibrato
pub fn heard(frames: &[Frame], meant: &[Meant]) -> Vec<Heard> {
    meant
        .iter()
        .map(|m| {
            let held: Vec<f32> = frames[m.start + m.settle..m.end].iter().filter_map(|f| f.pitch).collect();
            Heard {
                meant: m.note as f32,
                sung: m.sung,
                settle_ms: (m.settle as f64 * FRAME_SECS * 1000.0) as f32,
                long: held.len() >= singer::LONG,
                vibrato: singer::vibrato(&held),
            }
        })
        .collect()
}

/// A voice singing `notes` (a fractional MIDI pitch, beats, and a breath after it or not) at 120 BPM: three
/// harmonics, a little vibrato, breath noise, and a short glide into each note.
#[cfg(test)]
pub(crate) fn sing(notes: &[(f32, f64, bool)]) -> Stereo {
    use crate::config::SRF;
    let secs_per_beat = 0.5;
    let mut l = Vec::new();
    let mut phase = 0f64;
    let mut noise = 0x1234_5678u32;
    let mut prev = notes[0].0;
    for &(p, beats, breath) in notes {
        let n = (beats * secs_per_beat * SRF) as usize;
        for i in 0..n {
            let t = i as f64 / SRF;
            let glide = (t / 0.03).min(1.0) as f32;
            let pitch = prev + (p - prev) * glide + 0.15 * (std::f64::consts::TAU * 5.5 * t).sin() as f32;
            let hz = 440.0 * 2f64.powf((pitch as f64 - 69.0) / 12.0);
            phase += std::f64::consts::TAU * hz / SRF;
            // a breath is the note's last 60 ms
            let env = if breath && i > n - (0.06 * SRF) as usize { 0.0 } else { 1.0 };
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let hiss = (noise as f32 / u32::MAX as f32 - 0.5) * 0.004;
            let v = (phase.sin() + 0.5 * (2.0 * phase).sin() + 0.25 * (3.0 * phase).sin()) * 0.25 * env;
            l.push(v as f32 + hiss);
        }
        prev = p;
    }
    Stereo { l: l.clone(), r: l }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::{Key, Mode};

    #[test]
    fn a_singer_is_learnt_from_their_singing_alone() {
        // a run up and down C major from C4, every note from A4 up 35 cents flat (five of them), the rest in tune
        let up = [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 71.0, 72.0, 71.0, 69.0, 67.0, 65.0, 64.0, 62.0, 60.0];
        let melody: Vec<(f32, f64, bool)> =
            up.iter().map(|&p| (if p >= 69.0 { p - 0.35 } else { p }, 1.0, true)).collect();
        let a = assess(&sing(&melody), &Scale::key(&Key::new(0, Mode::Major)));
        let read: Vec<i32> = a.notes.iter().map(|n| n.note).collect();
        assert_eq!(read, up.map(|p| p as i32), "{a:?}");
        assert!(a.voice.bias[2] < -0.2 && a.voice.bias[0].abs() < 0.1, "flat at the top only: {:?}", a.voice);
        assert!((a.within(25.0) - 10.0 / 15.0).abs() < 1e-6 && a.within(50.0) == 1.0, "{a:?}");
        let words = a.describe();
        assert!(words.starts_with("15 notes, 67% within 25 cents of the note meant and 100% within 50"), "{words}");
        assert!(words.contains("furthest off: ") && words.contains("range C4-C5"), "{words}");
    }

    #[test]
    fn nothing_sung_is_said_so() {
        let quiet = Stereo::zeros(48_000);
        let a = assess(&quiet, &Scale::free());
        assert!(a.notes.is_empty() && a.voice == Voice::default());
        assert_eq!(a.describe(), "no notes to hear: it wants singing, one note at a time");
    }
}
