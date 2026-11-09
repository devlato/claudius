//! Notes: what a recording of one instrument plays, note by note and chords and all, by Spotify's Basic Pitch (Bittner,
//! Bosch, Rubinstein, Meseguer-Brocal and Ewert, "A Lightweight Instrument-Agnostic Model for Polyphonic Note
//! Transcription and Multipitch Estimation", ICASSP 2022, arXiv:2203.09893). Its code and its model are Apache-2.0: the
//! ONNX export it ships (`FILE` in `REPO` at `REV`) is pinned by its SHA-256, and what turns what the model hears into
//! notes (`notes_of`) is ported from its `note_creation.py`.
//!
//! The model hears 22.05 kHz mono two seconds at a time (`WINDOW` samples) and says, for every 256 samples (a frame,
//! 11.6 ms) and each of the 88 keys of a piano, how likely a note sounds there (`note`) and starts there (`onset`), and
//! for each third of a semitone from A0 up how likely the pitch lies there (`contour`). Its windows overlap by 30
//! frames, and each keeps all but the 15 at either end, the frames it hears with the least sound around them.
//!
//! A note starts at a peak in time of a key's onset likelihood of `Options::onset` at least, or of a jump in its note
//! likelihood (an onset the model missed), and lasts while its note likelihood stays at `Options::frame` or over, a
//! gap of up to 11 frames forgiven; one no longer than `Options::min_note_ms` is none. Onsets are taken latest first,
//! and the likelihood a note takes, its own key's and the keys either side of it, is spent. Then, as Melodia does, the
//! strongest likelihood left over `Options::frame` is a note of its own, followed forward and back the same way, until
//! none is left.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::Path;

use anyhow::{Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;

use crate::dsp::resample::resample;
use crate::models::{self, ox};

/// where the model is: Spotify's repository on GitHub, at a commit, and the file in it
pub const REPO: &str = "spotify/basic-pitch";
pub const REV: &str = "fa5997af0a8210982619003269994a1be25eddf3";
pub const FILE: &str = "basic_pitch/saved_models/icassp_2022/nmp.onnx";

/// what the model hears: 22.05 kHz mono, in windows two seconds long less a frame
pub const SR: u32 = 22_050;
const HOP: usize = 256;
const WINDOW: usize = 43_844;
/// the frames the model says of a window, and of them the frames two windows share
const FRAMES: usize = 172;
const OVERLAP: usize = 30;
/// the frames of a window kept, and the samples a window starts after the one before
const KEPT: usize = FRAMES - OVERLAP;
const STEP: usize = WINDOW - OVERLAP * HOP;
/// the keys of a piano, the MIDI note of the lowest (A0), and the bins of the contour, three to a key
pub const KEYS: usize = 88;
pub const LOWEST: u8 = 21;
const CONTOUR: usize = 3 * KEYS;
/// the frames of a gap a note lasts through
const TOLERANCE: usize = 11;
/// windows through the model at once
const BATCH: usize = 8;

const INPUT: &str = "serving_default_input_2:0";
const NOTE_OUT: &str = "StatefulPartitionedCall:1";
const ONSET_OUT: &str = "StatefulPartitionedCall:2";
const CONTOUR_OUT: &str = "StatefulPartitionedCall:0";

/// What the model says of a recording, frame by frame (`frame_time`): for each key, how likely a note sounds there
/// (`note`) and starts there (`onset`), `KEYS` to a frame, and for each third of a semitone from A0 up how likely its
/// pitch lies there (`contour`), three times as many.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activations {
    pub frames: usize,
    pub note: Vec<f32>,
    pub onset: Vec<f32>,
    pub contour: Vec<f32>,
}

/// A note: from `start` to `end` seconds, its MIDI `pitch`, and its `amplitude`, the model's mean likelihood of it over
/// its frames, 0 to 1 (Basic Pitch's MIDI velocity is 127 times it).
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
    pub amplitude: f32,
}

/// How sure the model must be of a note, and of which notes: Basic Pitch's own defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// the onset likelihood a note starts at, at its peak
    pub onset: f32,
    /// the note likelihood a note lasts at
    pub frame: f32,
    /// the shortest note, ms
    pub min_note_ms: f64,
    /// the lowest and the highest pitch, Hz (none: the whole piano)
    pub min_hz: Option<f64>,
    pub max_hz: Option<f64>,
    /// starts too where the note likelihood jumps, as well as where the model hears onsets
    pub infer_onsets: bool,
    /// notes too of the likelihood left after the onsets' notes (Melodia's trick)
    pub melodia: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            onset: 0.5,
            frame: 0.3,
            min_note_ms: 127.7,
            min_hz: None,
            max_hz: None,
            infer_onsets: true,
            melodia: true,
        }
    }
}

/// What hears windows of sound: for `n` windows of `WINDOW` samples one after another, the note, onset and contour
/// likelihoods of each (`[n, FRAMES, KEYS]`, `[n, FRAMES, KEYS]` and `[n, FRAMES, 3 * KEYS]`).
pub trait Net {
    fn hear(&mut self, windows: &[f32], n: usize) -> Result<[Vec<f32>; 3]>;
}

/// Basic Pitch's model
pub struct Transcriber {
    net: Session,
}

impl Transcriber {
    /// the model, from `cache` (downloaded into it first, unless `offline`), on `threads` threads
    pub fn load(cache: &Path, offline: bool, threads: usize) -> Result<Transcriber> {
        let path = models::fetch_github(cache, REPO, REV, FILE, offline)?;
        Ok(Transcriber { net: models::session(&path, threads)? })
    }

    /// whether the model is in `cache`
    pub fn cached(cache: &Path) -> bool {
        models::model_path(cache, REPO, REV, FILE).is_file()
    }

    /// what the model says of `x`, mono at `sr` Hz
    pub fn activations(&mut self, x: &[f32], sr: u32) -> Result<Activations> {
        activations_with(self, x, sr)
    }

    /// the notes `x`, mono at `sr` Hz, plays, by the start
    pub fn notes(&mut self, x: &[f32], sr: u32, o: &Options) -> Result<Vec<Note>> {
        Ok(notes_of(&self.activations(x, sr)?, o))
    }
}

impl Net for Transcriber {
    fn hear(&mut self, windows: &[f32], n: usize) -> Result<[Vec<f32>; 3]> {
        let t = ox(Tensor::from_array(([n, WINDOW, 1], windows.to_vec())))?;
        let res = ox(self.net.run(ort::inputs![INPUT => t]))?;
        let take = |name: &str| -> Result<Vec<f32>> {
            let (_, y) = ox(res[name].try_extract_tensor::<f32>())?;
            Ok(y.to_vec())
        };
        Ok([take(NOTE_OUT)?, take(ONSET_OUT)?, take(CONTOUR_OUT)?])
    }
}

/// the time of frame `f` of `Activations`, in seconds from the start of the recording: the frames of each window
/// stand a frame apart, and the windows `STEP` samples apart
pub fn frame_time(f: usize) -> f64 {
    ((f / KEPT) * STEP + (f % KEPT) * HOP) as f64 / SR as f64
}

/// What `net` says of `x`, mono at `sr` Hz (`Transcriber::activations`): the recording, half an overlap of silence
/// before it, heard window by window, each window's frames but those it shares kept, as many frames as the recording
/// lasts.
pub fn activations_with(net: &mut dyn Net, x: &[f32], sr: u32) -> Result<Activations> {
    let y = resample(x, sr, SR);
    let mut padded = vec![0.0f32; OVERLAP * HOP / 2];
    padded.extend_from_slice(&y);
    let windows = padded.len().div_ceil(STEP).max(1);
    let frames = (y.len() as f64 / STEP as f64 * KEPT as f64) as usize;
    let mut a = Activations {
        frames,
        note: vec![0.0; frames * KEYS],
        onset: vec![0.0; frames * KEYS],
        contour: vec![0.0; frames * CONTOUR],
    };
    for w0 in (0..windows).step_by(BATCH) {
        let n = (w0 + BATCH).min(windows) - w0;
        let mut input = vec![0.0f32; n * WINDOW];
        for (j, chunk) in input.chunks_mut(WINDOW).enumerate() {
            let from = (w0 + j) * STEP;
            let to = (from + WINDOW).min(padded.len());
            if from < to {
                chunk[..to - from].copy_from_slice(&padded[from..to]);
            }
        }
        let heard = net.hear(&input, n)?;
        for (out, width) in heard.iter().zip([KEYS, KEYS, CONTOUR]) {
            if out.len() != n * FRAMES * width {
                return Err(anyhow!("the model said {} values of {n} windows, not {}", out.len(), n * FRAMES * width));
            }
        }
        for j in 0..n {
            for k in 0..KEPT {
                let f = (w0 + j) * KEPT + k;
                if f >= frames {
                    break;
                }
                let src = j * FRAMES + OVERLAP / 2 + k;
                for (dst, (out, width)) in
                    [&mut a.note, &mut a.onset, &mut a.contour].into_iter().zip(heard.iter().zip([KEYS, KEYS, CONTOUR]))
                {
                    dst[f * width..(f + 1) * width].copy_from_slice(&out[src * width..(src + 1) * width]);
                }
            }
        }
    }
    Ok(a)
}

/// the key a frequency is nearest, of the piano's 88 (A0 is 0), or past either end of it
fn key_of(hz: f64) -> i64 {
    (12.0 * (hz / 440.0).log2() + 69.0 - LOWEST as f64).round() as i64
}

/// where the note likelihood jumps, the jump over each of the two frames before it, the smaller (none in the first two
/// frames), scaled so that the largest is as likely as the likeliest onset, onsets where it is likelier than the model
/// heard
fn infer_onsets(onsets: &mut [f32], frames: &[f32], n: usize) {
    let mut jump = vec![0.0f32; n * KEYS];
    for t in 2..n {
        for k in 0..KEYS {
            let v = frames[t * KEYS + k];
            jump[t * KEYS + k] = (v - frames[(t - 1) * KEYS + k]).min(v - frames[(t - 2) * KEYS + k]).max(0.0);
        }
    }
    let likeliest = onsets.iter().fold(0.0f32, |m, &v| m.max(v));
    let largest = jump.iter().fold(0.0f32, |m, &v| m.max(v));
    if largest > 0.0 {
        for (o, j) in onsets.iter_mut().zip(&jump) {
            *o = o.max(likeliest * j / largest);
        }
    }
}

/// the likelihood at frame `t` of key `k` and the keys either side of it, spent
fn spend(left: &mut [f32], t: usize, k: usize) {
    left[t * KEYS + k] = 0.0;
    if k + 1 < KEYS {
        left[t * KEYS + k + 1] = 0.0;
    }
    if k > 0 {
        left[t * KEYS + k - 1] = 0.0;
    }
}

/// a likelihood left, the strongest first and, of equals, the earliest and lowest, as numpy's argmax takes it
#[derive(PartialEq)]
struct Left(f32, usize);

impl Eq for Left {}

impl Ord for Left {
    fn cmp(&self, other: &Left) -> Ordering {
        self.0.total_cmp(&other.0).then(other.1.cmp(&self.1))
    }
}

impl PartialOrd for Left {
    fn partial_cmp(&self, other: &Left) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The notes `a` says, as `o` asks, by the start and then the pitch (Basic Pitch's `output_to_notes_polyphonic`).
pub fn notes_of(a: &Activations, o: &Options) -> Vec<Note> {
    let n = a.frames;
    if n < 2 {
        return Vec::new();
    }
    let (mut frames, mut onsets) = (a.note.clone(), a.onset.clone());
    let lo = o.min_hz.map_or(0, |hz| key_of(hz).clamp(0, KEYS as i64) as usize);
    let hi = o.max_hz.map_or(KEYS, |hz| key_of(hz).clamp(0, KEYS as i64) as usize);
    for t in 0..n {
        for k in (0..lo).chain(hi..KEYS) {
            frames[t * KEYS + k] = 0.0;
            onsets[t * KEYS + k] = 0.0;
        }
    }
    if o.infer_onsets {
        infer_onsets(&mut onsets, &frames, n);
    }
    let shortest = (o.min_note_ms / 1000.0 * SR as f64 / HOP as f64).round() as usize;
    let at = |t: usize, k: usize| t * KEYS + k;
    let mean =
        |from: usize, to: usize, k: usize| (from..to).map(|t| frames[at(t, k)]).sum::<f32>() / (to - from) as f32;
    let note = |from: usize, to: usize, k: usize| Note {
        start: frame_time(from),
        end: frame_time(to),
        pitch: LOWEST + k as u8,
        amplitude: mean(from, to, k),
    };

    let mut starts = Vec::new();
    for t in 1..n - 1 {
        for k in 0..KEYS {
            let v = onsets[at(t, k)];
            if v >= o.onset && v > onsets[at(t - 1, k)] && v > onsets[at(t + 1, k)] {
                starts.push((t, k));
            }
        }
    }
    let mut left = frames.clone();
    let mut notes = Vec::new();
    for &(from, k) in starts.iter().rev() {
        let (mut i, mut gap) = (from + 1, 0);
        while i < n - 1 && gap < TOLERANCE {
            gap = if left[at(i, k)] < o.frame { gap + 1 } else { 0 };
            i += 1;
        }
        let to = i - gap;
        if to - from <= shortest {
            continue;
        }
        for t in from..to {
            spend(&mut left, t, k);
        }
        notes.push(note(from, to, k));
    }

    if o.melodia {
        let mut strongest: BinaryHeap<Left> =
            left.iter().enumerate().filter(|(_, v)| **v > o.frame).map(|(i, v)| Left(*v, i)).collect();
        while let Some(Left(v, i)) = strongest.pop() {
            if left[i] != v {
                continue;
            }
            let (mid, k) = (i / KEYS, i % KEYS);
            left[i] = 0.0;
            let (mut i, mut gap) = (mid + 1, 0);
            while i < n - 1 && gap < TOLERANCE {
                gap = if left[at(i, k)] < o.frame { gap + 1 } else { 0 };
                spend(&mut left, i, k);
                i += 1;
            }
            let to = i - 1 - gap;
            let (mut i, mut gap) = (mid as i64 - 1, 0);
            while i > 0 && gap < TOLERANCE {
                gap = if left[at(i as usize, k)] < o.frame { gap + 1 } else { 0 };
                spend(&mut left, i as usize, k);
                i -= 1;
            }
            let from = (i + 1 + gap as i64) as usize;
            if to <= from || to - from <= shortest {
                continue;
            }
            notes.push(note(from, to, k));
        }
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `frames` frames of silence, and what `held` holds: key, first frame, frame after the last, its note likelihood,
    /// and whether the model hears it start
    fn heard(frames: usize, held: &[(usize, usize, usize, f32, bool)]) -> Activations {
        let mut a = Activations {
            frames,
            note: vec![0.0; frames * KEYS],
            onset: vec![0.0; frames * KEYS],
            contour: vec![0.0; frames * CONTOUR],
        };
        for &(k, from, to, v, starts) in held {
            for t in from..to {
                a.note[t * KEYS + k] = v;
            }
            if starts {
                a.onset[from * KEYS + k] = 0.9;
            }
        }
        a
    }

    fn played(notes: &[Note]) -> Vec<(u8, usize, usize)> {
        let frame =
            |s: f64| (0..10_000).min_by(|a, b| (frame_time(*a) - s).abs().total_cmp(&(frame_time(*b) - s).abs()));
        notes.iter().map(|n| (n.pitch, frame(n.start).unwrap(), frame(n.end).unwrap())).collect()
    }

    #[test]
    fn a_key_held_is_a_note_from_its_onset_to_where_it_lets_go() {
        let notes = notes_of(&heard(100, &[(39, 10, 60, 0.8, true)]), &Options::default());
        assert_eq!(played(&notes), [(60, 10, 60)]);
        assert!((notes[0].amplitude - 0.8).abs() < 1e-6);
    }

    #[test]
    fn a_chord_is_its_notes_at_once_and_a_dip_of_fewer_than_eleven_frames_does_not_end_one() {
        let mut a = heard(120, &[(39, 10, 80, 0.7, true), (43, 10, 80, 0.7, true), (46, 10, 80, 0.7, true)]);
        for t in 40..45 {
            a.note[t * KEYS + 43] = 0.1;
        }
        let o = Options { infer_onsets: false, ..Options::default() };
        assert_eq!(played(&notes_of(&a, &o)), [(60, 10, 80), (64, 10, 80), (67, 10, 80)]);
        assert_eq!(
            played(&notes_of(&a, &Options::default())),
            [(60, 10, 80), (64, 10, 40), (67, 10, 80), (64, 45, 80)],
            "where its likelihood jumps back, the key is struck again"
        );
    }

    #[test]
    fn a_note_too_short_is_none_and_one_heard_without_an_onset_starts_where_its_likelihood_jumps() {
        let short = notes_of(&heard(100, &[(39, 10, 18, 0.8, true)]), &Options::default());
        assert!(short.is_empty(), "{short:?}");
        let a = heard(100, &[(39, 10, 60, 0.8, true), (51, 20, 70, 0.6, false)]);
        assert_eq!(played(&notes_of(&a, &Options::default())), [(60, 10, 60), (72, 20, 70)]);
        let unjumped = Options { infer_onsets: false, ..Options::default() };
        assert_eq!(
            played(&notes_of(&a, &unjumped)),
            [(60, 10, 60), (72, 20, 69)],
            "Melodia's trick still finds it, to the last frame it is likely in"
        );
        let neither = Options { melodia: false, ..unjumped };
        assert_eq!(played(&notes_of(&a, &neither)), [(60, 10, 60)]);
    }

    #[test]
    fn a_note_spends_the_keys_beside_it_and_the_range_asked_for_keeps_out_the_rest() {
        let a = heard(100, &[(39, 10, 60, 0.8, true), (40, 10, 60, 0.5, false)]);
        let o = Options { infer_onsets: false, ..Options::default() };
        assert_eq!(
            played(&notes_of(&a, &o)),
            [(60, 10, 60)],
            "a semitone above it is its own likelihood spilling over"
        );
        let both = heard(100, &[(15, 10, 60, 0.8, true), (60, 10, 60, 0.8, true)]);
        let low = Options { max_hz: Some(200.0), ..Options::default() };
        assert_eq!(played(&notes_of(&both, &low)), [(36, 10, 60)]);
        let high = Options { min_hz: Some(200.0), ..Options::default() };
        assert_eq!(played(&notes_of(&both, &high)), [(81, 10, 60)]);
    }

    /// a model that hears every window as its own number, in every bin
    struct Numbered(usize);

    impl Net for Numbered {
        fn hear(&mut self, _: &[f32], n: usize) -> Result<[Vec<f32>; 3]> {
            let mut out = [Vec::new(), Vec::new(), Vec::new()];
            for j in 0..n {
                let w = (self.0 + j) as f32;
                out[0].extend(std::iter::repeat_n(w, FRAMES * KEYS));
                out[1].extend(std::iter::repeat_n(w, FRAMES * KEYS));
                out[2].extend(std::iter::repeat_n(w, FRAMES * CONTOUR));
            }
            self.0 += n;
            Ok(out)
        }
    }

    #[test]
    fn the_windows_are_heard_in_turn_and_their_frames_kept_one_after_another_as_long_as_the_recording() {
        let secs = 30.0;
        let x = vec![0.0f32; (secs * SR as f64) as usize];
        let a = activations_with(&mut Numbered(0), &x, SR).unwrap();
        assert_eq!(a.frames, (x.len() as f64 / STEP as f64 * KEPT as f64) as usize);
        assert!((frame_time(a.frames) - secs).abs() < 0.02, "{} frames end at {}", a.frames, frame_time(a.frames));
        for f in [0, KEPT - 1, KEPT, 5 * KEPT + 7, a.frames - 1] {
            assert_eq!(a.note[f * KEYS], (f / KEPT) as f32, "frame {f}");
            assert_eq!(a.contour[f * CONTOUR + CONTOUR - 1], (f / KEPT) as f32, "frame {f}");
        }
        assert!((frame_time(KEPT) - STEP as f64 / SR as f64).abs() < 1e-9);
        assert!((1..a.frames).all(|f| frame_time(f) > frame_time(f - 1)));
    }
}
