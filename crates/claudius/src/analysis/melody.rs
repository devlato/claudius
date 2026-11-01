//! A melody sung, hummed, whistled or played one note at a time, as notes. The take is filtered and decimated to
//! 8 kHz (a voice's fundamental lies far below 4 kHz), and YIN (de Cheveigné and Kawahara 2002) finds its pitch every
//! 10 ms. A note is a run of voiced frames whose pitch holds; it ends where the voice stops, moves on to another pitch
//! or sings the same one again (its loudness rises anew). Notes come out on the song's grid, and in its key unless asked
//! not to be.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::theory::Note;
use crate::theory::scale::Key;

/// the rate pitch is found at, and the hop between frames (10 ms)
const RATE: f64 = 8_000.0;
const HOP: usize = 80;
/// YIN's window (50 ms), and the pitches it looks for: 50 Hz to 1 kHz
const WINDOW: usize = 400;
const TAU_MAX: usize = (RATE / 50.0) as usize;
const TAU_MIN: usize = (RATE / 1000.0) as usize;
/// how clearly periodic a frame has to be (YIN's absolute threshold)
const THRESHOLD: f32 = 0.15;

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

/// one frame's reading: its pitch (a fractional MIDI note) when the frame is voiced, and its loudness in dB
#[derive(Clone, Copy, Debug)]
struct Frame {
    pitch: Option<f32>,
    db: f32,
}

/// the melody in `x` as notes, in beats from its start
pub fn transcribe(x: &Stereo, key: &Key, o: &Options) -> Vec<Note> {
    let frames = frames(&decimate(x));
    let raw = segment(&frames);
    to_grid(&raw, key, o)
}

/// mono, low-passed twice at 3 kHz and decimated from 48 kHz to 8 kHz
fn decimate(x: &Stereo) -> Vec<f32> {
    let step = (SRF / RATE).round() as usize;
    let (b, a) = lowpass(3_000.0, SRF);
    let mut stages = [[0f32; 4]; 2];
    let mut out = Vec::with_capacity(x.l.len() / step + 1);
    for i in 0..x.l.len() {
        let mut v = 0.5 * (x.l[i] + x.r.get(i).copied().unwrap_or(x.l[i]));
        for s in stages.iter_mut() {
            let y = b[0] * v + b[1] * s[0] + b[2] * s[1] - a[0] * s[2] - a[1] * s[3];
            *s = [v, s[0], y, s[2]];
            v = y;
        }
        if i % step == 0 {
            out.push(v);
        }
    }
    out
}

/// an RBJ low-pass biquad: (b0, b1, b2), (a1, a2), normalised
fn lowpass(hz: f64, rate: f64) -> ([f32; 3], [f32; 2]) {
    let w = std::f64::consts::TAU * hz / rate;
    let alpha = w.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let (cos, a0) = (w.cos(), 1.0 + alpha);
    let b1 = (1.0 - cos) / a0;
    ([(b1 / 2.0) as f32, b1 as f32, (b1 / 2.0) as f32], [(-2.0 * cos / a0) as f32, ((1.0 - alpha) / a0) as f32])
}

/// the pitch and loudness of every 10 ms
fn frames(x: &[f32]) -> Vec<Frame> {
    let n = x.len().saturating_sub(WINDOW + TAU_MAX) / HOP;
    let mut out: Vec<Frame> = (0..n).map(|k| frame(&x[k * HOP..k * HOP + WINDOW + TAU_MAX])).collect();
    // a frame far quieter than the take's loudest is breath or the room, whatever pitch it seems to have
    let loudest = out.iter().map(|f| f.db).fold(-120f32, f32::max);
    for f in out.iter_mut() {
        if f.db < (loudest - 35.0).max(-60.0) {
            f.pitch = None;
        }
    }
    smooth(&mut out);
    out
}

fn frame(x: &[f32]) -> Frame {
    let w = &x[..WINDOW];
    let db = 10.0 * (w.iter().map(|v| v * v).sum::<f32>() / WINDOW as f32).max(1e-12).log10();
    // the difference function, and its cumulative mean normalised form
    let mut d = vec![0f32; TAU_MAX + 1];
    for (tau, dt) in d.iter_mut().enumerate().skip(1) {
        *dt = w.iter().zip(&x[tau..tau + WINDOW]).map(|(a, b)| (a - b) * (a - b)).sum();
    }
    let mut cmnd = vec![1f32; TAU_MAX + 1];
    let mut sum = 0f32;
    for tau in 1..=TAU_MAX {
        sum += d[tau];
        cmnd[tau] = if sum > 0.0 { d[tau] * tau as f32 / sum } else { 1.0 };
    }
    let mut tau = TAU_MIN;
    while tau < TAU_MAX && cmnd[tau] >= THRESHOLD {
        tau += 1;
    }
    if tau >= TAU_MAX {
        return Frame { pitch: None, db };
    }
    while tau + 1 < TAU_MAX && cmnd[tau + 1] < cmnd[tau] {
        tau += 1;
    }
    // between samples: the parabola through the minimum and its neighbours
    let (l, c, r) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
    let bend = l - 2.0 * c + r;
    let exact = tau as f32 + if bend.abs() > 1e-9 { 0.5 * (l - r) / bend } else { 0.0 };
    let hz = RATE as f32 / exact;
    Frame { pitch: Some(69.0 + 12.0 * (hz / 440.0).log2()), db }
}

/// a median of five over each voiced frame's neighbours, which takes out a lone octave jump
fn smooth(frames: &mut [Frame]) {
    let pitches: Vec<Option<f32>> = frames.iter().map(|f| f.pitch).collect();
    for (i, f) in frames.iter_mut().enumerate() {
        if f.pitch.is_none() {
            continue;
        }
        let mut near: Vec<f32> =
            pitches[i.saturating_sub(2)..(i + 3).min(pitches.len())].iter().flatten().copied().collect();
        near.sort_by(f32::total_cmp);
        f.pitch = Some(near[near.len() / 2]);
    }
}

/// a note as heard: start and end in frames, its pitch, its loudness in dB
#[derive(Clone, Copy, Debug, PartialEq)]
struct Heard {
    start: usize,
    end: usize,
    pitch: f32,
    db: f32,
}

fn median(v: &[f32]) -> f32 {
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    s[s.len() / 2]
}

/// notes where the pitch holds: a note ends after 3 unvoiced frames, when the pitch has left it for 3 frames, or when
/// the voice grows 6 dB louder again within 30 ms (the same pitch sung anew); notes under 70 ms are dropped
fn segment(frames: &[Frame]) -> Vec<Heard> {
    let mut out = Vec::new();
    let mut cur: Vec<(usize, f32, f32)> = Vec::new();
    let mut gap = 0;
    let mut away = 0;
    let close = |cur: &mut Vec<(usize, f32, f32)>, out: &mut Vec<Heard>, upto: usize| {
        let take: Vec<_> = cur.drain(..upto.min(cur.len())).collect();
        if take.len() >= 7 {
            let pitch = median(&take.iter().map(|t| t.1).collect::<Vec<_>>());
            let db = take.iter().map(|t| t.2).fold(-120f32, f32::max);
            out.push(Heard { start: take[0].0, end: take[take.len() - 1].0 + 1, pitch, db });
        }
    };
    for (i, f) in frames.iter().enumerate() {
        let Some(p) = f.pitch else {
            gap += 1;
            if gap >= 3 && !cur.is_empty() {
                let n = cur.len();
                close(&mut cur, &mut out, n);
                away = 0;
            }
            continue;
        };
        gap = 0;
        if !cur.is_empty() {
            let held = median(&cur.iter().map(|t| t.1).collect::<Vec<_>>());
            away = if (p - held).abs() > 0.7 { away + 1 } else { 0 };
            let dip = frames[i.saturating_sub(3)..i].iter().map(|f| f.db).fold(f32::MAX, f32::min);
            let again = f.db - dip > 6.0 && cur.len() >= 8;
            if away >= 3 {
                // the frames that left belong to the next note
                let keep = cur.len().saturating_sub(away - 1);
                close(&mut cur, &mut out, keep);
                away = 0;
            } else if again {
                let n = cur.len();
                close(&mut cur, &mut out, n);
                away = 0;
            }
        }
        cur.push((i, p, f.db));
    }
    let n = cur.len();
    close(&mut cur, &mut out, n);
    out
}

/// the notes on the grid, one at a time, in the key unless chromatic, louder notes harder
fn to_grid(heard: &[Heard], key: &Key, o: &Options) -> Vec<Note> {
    // a frame's pitch is that of the middle of its window
    let beats = |frame: usize| (frame * HOP + WINDOW / 2) as f64 / RATE * o.bpm / 60.0;
    let grid = o.grid.max(1.0 / 16.0);
    let snap = |b: f64| (b / grid).round() * grid;
    let loudest = heard.iter().map(|h| h.db).fold(-120f32, f32::max);
    let mut notes: Vec<Note> = Vec::new();
    for h in heard {
        let p = if o.chromatic { h.pitch.round() as i32 } else { in_key(h.pitch, key) };
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

/// a voice singing `notes` (MIDI pitch, beats, and a breath after it or not) at 120 BPM: three harmonics, a little
/// vibrato, breath noise, and a short glide into each note
#[cfg(test)]
pub(crate) fn sing(notes: &[(f32, f64, bool)]) -> Stereo {
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
    use crate::theory::scale::Mode;

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
    fn falling_off_a_note_into_a_breath_and_scooping_into_the_next_is_two_notes() {
        // C4 falling off for its last two frames, a breath, then E4 scooped into from below
        let voiced = |p: f32| Frame { pitch: Some(p), db: -20.0 };
        let mut frames: Vec<Frame> = vec![voiced(60.0); 10];
        frames.extend([voiced(58.0), voiced(58.0)]);
        frames.extend([Frame { pitch: None, db: -70.0 }; 3]);
        frames.push(voiced(61.0));
        frames.extend(vec![voiced(64.0); 10]);
        let pitches: Vec<i32> = segment(&frames).iter().map(|h| h.pitch.round() as i32).collect();
        assert_eq!(pitches, [60, 64]);
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
