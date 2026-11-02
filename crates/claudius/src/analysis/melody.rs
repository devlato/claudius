//! A melody sung, hummed, whistled or played one note at a time, as notes: the notes it means (`voice::intent`, which
//! reads its pitch every 10 ms, and reads it in the song's key unless asked not to), on the song's grid, and in its key
//! unless asked not to be.

use crate::audio::Stereo;
use crate::theory::Note;
use crate::theory::scale::Key;
use crate::voice::intent::{self, Scale};
use crate::voice::pitch;
use crate::voice::singer::Voice;

/// what to make of a melody
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// beats per minute it was sung at
    pub bpm: f64,
    /// the grid, in beats (0.25: sixteenths)
    pub grid: f64,
    /// keep notes outside the key instead of moving them to its nearest tone
    pub chromatic: bool,
}

/// the melody in `x` as notes, in beats from its start
pub fn transcribe(x: &Stereo, key: &Key, o: &Options) -> Vec<Note> {
    let frames = pitch::track(x);
    let scale = if o.chromatic { Scale::free() } else { Scale::key(key) };
    let heard: Vec<Heard> = intent::decode(&frames, &Voice::default(), &scale)
        .iter()
        .map(|m| Heard {
            start: m.start,
            end: m.end,
            note: m.note,
            pitch: m.sung,
            db: frames[m.start..m.end].iter().map(|f| f.db).fold(-120f32, f32::max),
        })
        .collect();
    to_grid(&heard, key, o)
}

/// the pitch of every 10 ms of `x`, as a fractional MIDI note, or none where nothing holds one (for the sampler, which
/// looks for what is sung)
pub fn pitches(x: &Stereo) -> Vec<Option<f32>> {
    pitch::rough(x).iter().map(|f| f.pitch).collect()
}

/// a note as heard: start and end in frames, the note meant, where it was sung, its loudness in dB
#[derive(Clone, Copy, Debug, PartialEq)]
struct Heard {
    start: usize,
    end: usize,
    note: i32,
    pitch: f32,
    db: f32,
}

/// the notes on the grid, one at a time, in the key unless chromatic, louder notes harder
fn to_grid(heard: &[Heard], key: &Key, o: &Options) -> Vec<Note> {
    let beats = |frame: usize| pitch::time(frame) * o.bpm / 60.0;
    let grid = o.grid.max(1.0 / 16.0);
    let snap = |b: f64| (b / grid).round() * grid;
    let loudest = heard.iter().map(|h| h.db).fold(-120f32, f32::max);
    let mut notes: Vec<Note> = Vec::new();
    for h in heard {
        let p = if o.chromatic || key.contains(h.note) { h.note } else { in_key(h.pitch, key) };
        let t = snap(beats(h.start));
        let d = (snap(beats(h.end)) - t).max(grid);
        let v = (0.55 + (h.db - loudest) / 40.0).clamp(0.35, 1.0);
        match notes.last_mut() {
            // two notes snapped onto one step: the longer one has it
            Some(last) if last.t == t => {
                if d > last.d {
                    *last = Note { t, d, p, v };
                }
            }
            _ => notes.push(Note { t, d, p, v }),
        }
    }
    // one at a time: a note ends where the next begins
    for i in 1..notes.len() {
        let next = notes[i].t;
        let prev = &mut notes[i - 1];
        prev.d = prev.d.min(next - prev.t).max(grid.min(next - prev.t));
    }
    notes
}

/// the scale tone nearest a fractional pitch
fn in_key(p: f32, key: &Key) -> i32 {
    let (lo, hi) = (p.floor() as i32, p.ceil() as i32);
    match (key.contains(lo), key.contains(hi)) {
        (true, true) if lo == hi => lo,
        (true, true) => {
            if p - lo as f32 <= hi as f32 - p {
                lo
            } else {
                hi
            }
        }
        (true, false) => lo,
        (false, true) => hi,
        (false, false) => key.snap(p.round() as i32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::Mode;
    use crate::voice::sing;

    fn o() -> Options {
        Options { bpm: 120.0, grid: 0.25, chromatic: false }
    }

    #[test]
    fn a_sung_melody_comes_out_as_its_notes_on_the_grid() {
        let melody = [
            (60.0, 1.0, false),
            (62.0, 1.0, true),
            (64.0, 0.5, false),
            (67.0, 1.5, true),
            (65.0, 1.0, false),
            (64.0, 2.0, true),
        ];
        let key = Key::new(0, Mode::Major);
        let notes = transcribe(&sing(&melody), &key, &o());
        let pitches: Vec<i32> = notes.iter().map(|n| n.p).collect();
        assert_eq!(pitches, [60, 62, 64, 67, 65, 64], "{notes:?}");
        let mut at = 0.0;
        for (n, &(_, beats, _)) in notes.iter().zip(&melody) {
            assert!((n.t - at).abs() <= 0.25, "{} starts at {} where it was sung at {at}", n.p, n.t);
            at += beats;
        }
        assert!(notes.windows(2).all(|w| w[0].t + w[0].d <= w[1].t + 1e-9), "one note at a time");
    }

    #[test]
    fn the_same_pitch_sung_twice_is_two_notes() {
        let notes = transcribe(
            &sing(&[(57.0, 1.0, true), (57.0, 1.0, false), (52.0, 1.0, false)]),
            &Key::new(9, Mode::Minor),
            &o(),
        );
        let pitches: Vec<i32> = notes.iter().map(|n| n.p).collect();
        assert_eq!(pitches, [57, 57, 52], "{notes:?}");
    }

    #[test]
    fn a_flat_or_sharp_note_goes_to_the_key_unless_chromatic() {
        // F#4 sung a quarter tone flat, in C major, goes to the nearer of F and G: F
        let x = sing(&[(65.75, 2.0, false)]);
        let key = Key::new(0, Mode::Major);
        assert_eq!(transcribe(&x, &key, &o()).iter().map(|n| n.p).collect::<Vec<_>>(), [65]);
        let chromatic = Options { chromatic: true, ..o() };
        assert_eq!(transcribe(&x, &key, &chromatic).iter().map(|n| n.p).collect::<Vec<_>>(), [66]);
    }

    #[test]
    fn silence_and_noise_are_no_notes() {
        let quiet = Stereo { l: vec![0.0; 48_000], r: vec![0.0; 48_000] };
        assert!(transcribe(&quiet, &Key::new(0, Mode::Major), &o()).is_empty());
        let mut n = 0x9e37_79b9u32;
        let hiss: Vec<f32> = (0..48_000)
            .map(|_| {
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                (n as f32 / u32::MAX as f32 - 0.5) * 0.3
            })
            .collect();
        assert!(transcribe(&Stereo { l: hiss.clone(), r: hiss }, &Key::new(0, Mode::Major), &o()).is_empty());
    }
}
