//! A tuner: a voice moved, note by note, towards the notes it means (`intent`), as far as its singer needs (`singer`).
//! Where they always put a note off (their bias) is taken out. Of how they scatter around that, what is within
//! `humanize` cents of the note stays, as expression, and the rest goes: a precise singer is left nearly alone and a
//! loose one is brought in. A note is brought onto its pitch over `speed_ms` after it starts, so that a slow tuner
//! leaves the scoop into it and a fast one takes it out (0 is the hard-tuned sound). The vibrato rides on the note as
//! sung unless `vibrato` asks for less of it. PSOLA moves the voice and keeps its formants.

use serde::{Deserialize, Serialize};

use super::intent::{self, Meant, Scale};
use super::pitch::{self, FRAME_SECS, Frame};
use super::singer::Voice;
use crate::audio::Stereo;
use crate::config::SRF;
use crate::dsp::env::Param;
use crate::dsp::psola;
use crate::theory::scale::parse_pc;

/// a frame further than this (semitones) from where its note is held is a slide, a slip or an octave misheard: it
/// does not count towards the note's centre, and it moves as the frames around it do
const NEAR: f32 = 1.5;

/// what a tuner reads the notes meant against
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuneScale {
    /// the song's key and the chord under each note (a tuner outside a song: any note)
    #[default]
    Chords,
    /// the song's key
    Key,
    /// any note
    Chromatic,
    /// only these notes, in any octave ("C", "Eb", "F#" ...)
    Notes(Vec<String>),
}

/// how a tuner moves a voice
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    /// how much of the way to the notes meant, at each beat: 0 leaves the voice, 1 goes all the way
    pub strength: Param,
    /// how fast a note is brought onto its pitch after it starts, ms (0: at once)
    pub speed_ms: f32,
    /// the cents of a singer's own wandering around a note that stay
    pub humanize: f32,
    /// how much of the vibrato stays: 1 all of it, 0 none
    pub vibrato: f32,
    /// the singer; learnt from the voice itself when not given
    pub voice: Option<Voice>,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning { strength: Param::Const(1.0), speed_ms: 40.0, humanize: 10.0, vibrato: 1.0, voice: None }
    }
}

/// a note as the tuner left it
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Tuned {
    /// where it starts, seconds
    pub at: f64,
    /// the note meant, where it was sung and where it is sung now (fractional MIDI notes, the middle of the note)
    pub note: i32,
    pub sung: f32,
    pub now: f32,
}

impl TuneScale {
    /// the weights it gives each pitch class from each beat on, in a song in `key` over `chords`
    pub fn harmony(
        &self,
        key: &crate::theory::scale::Key,
        chords: &[crate::theory::ChordSpan],
    ) -> Vec<(f64, [f32; 12])> {
        match self {
            TuneScale::Chords => intent::harmony(key, chords),
            TuneScale::Key => intent::harmony(key, &[]),
            TuneScale::Chromatic => vec![(0.0, [1.0; 12])],
            TuneScale::Notes(names) => vec![(0.0, only(names))],
        }
    }

    /// the same outside a song: a key or chords say nothing there
    pub fn alone(&self) -> Vec<(f64, [f32; 12])> {
        match self {
            TuneScale::Notes(names) => vec![(0.0, only(names))],
            _ => vec![(0.0, [1.0; 12])],
        }
    }
}

/// the notes named and no other (any note, when none of them is a note)
fn only(names: &[String]) -> [f32; 12] {
    let mut w = [0.0; 12];
    for pc in names.iter().filter_map(|n| parse_pc(n.trim())) {
        w[pc as usize % 12] = 1.0;
    }
    if w.iter().all(|&v| v == 0.0) { [1.0; 12] } else { w }
}

impl std::str::FromStr for TuneScale {
    type Err = String;

    /// "chords", "key", "chromatic", or the notes to go to ("C D E G A", "Eb, F, Bb")
    fn from_str(s: &str) -> Result<TuneScale, String> {
        Ok(match s.trim() {
            "chords" | "" => TuneScale::Chords,
            "key" => TuneScale::Key,
            "chromatic" => TuneScale::Chromatic,
            notes => {
                let names: Vec<String> = notes.split([' ', ',']).filter(|n| !n.is_empty()).map(String::from).collect();
                if let Some(bad) = names.iter().find(|n| parse_pc(n).is_none()) {
                    return Err(format!(
                        "a tuner goes to chords, key, chromatic or notes such as \"C D E G A\", not '{bad}'"
                    ));
                }
                TuneScale::Notes(names)
            }
        })
    }
}

/// a tuner's settings, in words
pub fn words(strength: &Param, speed_ms: f32, humanize: f32, vibrato: f32, scale: &TuneScale) -> String {
    let to = match scale {
        TuneScale::Chords => "the song's chords".to_string(),
        TuneScale::Key => "the song's key".to_string(),
        TuneScale::Chromatic => "the nearest notes".to_string(),
        TuneScale::Notes(n) => n.join(" "),
    };
    let how = if speed_ms <= 0.0 { "at once".to_string() } else { format!("over {speed_ms:.0} ms") };
    let strength = match strength {
        Param::Const(s) if (*s - 1.0).abs() < 1e-6 => String::new(),
        Param::Const(s) => format!(", {:.0}% of the way", s * 100.0),
        _ => ", as far as it moves".to_string(),
    };
    let vibrato = match vibrato {
        v if v >= 0.999 => "all of its vibrato".to_string(),
        v if v <= 0.001 => "no vibrato".to_string(),
        v => format!("{:.0}% of its vibrato", v * 100.0),
    };
    format!("tuned to {to}{strength}, each note brought on {how}, keeping {humanize:.0} cents of its own and {vibrato}")
}

/// Puts a tuner first in track `track`'s chain, or changes the one in it, or with `None` takes it out. Returns what it
/// did, in words.
pub fn put(bp: &mut crate::model::Blueprint, track: &str, tuner: Option<crate::fx::Effect>) -> Result<String, String> {
    use crate::fx::Effect;
    let mix = crate::script::mix_of(bp, track).ok_or_else(|| format!("no track '{track}'"))?;
    let at = mix.fx.iter().position(|e| matches!(e, Effect::Tune { .. }));
    match (tuner, at) {
        (None, None) => Ok(format!("{track} has no tuner")),
        (None, Some(i)) => {
            mix.fx.remove(i);
            bp.lineage.push(format!("{track} untuned"));
            Ok(format!("{track}: tuned no longer"))
        }
        (Some(t), at) => {
            let said = match &t {
                Effect::Tune { strength, speed_ms, humanize, vibrato, scale, .. } => {
                    words(strength, *speed_ms, *humanize, *vibrato, scale)
                }
                other => return Err(format!("a tuner is a tune effect, not {}", other.name())),
            };
            match at {
                Some(i) => mix.fx[i] = t,
                None => mix.fx.insert(0, t),
            }
            bp.lineage.push(format!("{track} {said}"));
            Ok(format!("{track}: {said}"))
        }
    }
}

/// Tunes the voice in `x` to the notes it means, read against `harmony` (weights from each beat on), at `bpm`. Returns
/// each note as it was and as it is now.
pub fn tune(x: &mut Stereo, harmony: &[(f64, [f32; 12])], t: &Tuning, bpm: f64) -> Vec<Tuned> {
    let scale = Scale { at: harmony.iter().map(|(b, w)| (b * 60.0 / bpm, *w)).collect() };
    let scale = if scale.at.is_empty() { Scale::free() } else { scale };
    let frames = pitch::track(x);
    let (voice, meant) = match &t.voice {
        Some(v) => (v.clone(), intent::decode(&frames, v, &scale)),
        None => super::learn(&frames, &scale),
    };
    if meant.is_empty() {
        return Vec::new();
    }
    let delta = corrections(&frames, &meant, &voice, t, bpm);
    let mono = x.mono();
    for (a, b) in stretches(&frames) {
        if delta[a..b].iter().all(|d| d.abs() < 0.002) {
            continue;
        }
        let (s0, s1) = ((pitch::time(a) * SRF) as usize, (pitch::time(b - 1) * SRF) as usize);
        let period = |s: usize| psola::period_of(pitch::hz(pitch_near(&frames, s) as f64));
        let marks = psola::marks(&mono, s0, s1, &period);
        let ratio = |s: usize| 2f64.powf(at(&delta, s) as f64 / 12.0);
        for ch in x.channels_mut() {
            psola::shift(ch, &marks, &ratio);
        }
    }
    meant
        .iter()
        .map(|m| {
            let moved: Vec<f32> =
                (m.start + m.settle..m.end).filter(|&k| frames[k].pitch.is_some()).map(|k| delta[k]).collect();
            let moved = moved.iter().sum::<f32>() / moved.len().max(1) as f32;
            Tuned { at: pitch::time(m.start), note: m.note, sung: m.sung, now: m.sung + moved }
        })
        .collect()
}

/// how far to move each frame, in semitones: within a note, its centre towards the note as far as the singer needs,
/// brought in over `speed_ms` from the note's start, and its vibrato as `vibrato` asks; between notes, from the one
/// to the next; a light smoothing over all of it
fn corrections(frames: &[Frame], meant: &[Meant], voice: &Voice, t: &Tuning, bpm: f64) -> Vec<f32> {
    let n = frames.len();
    let mut d: Vec<Option<f32>> = vec![None; n];
    let tau = t.speed_ms / 1000.0 / FRAME_SECS as f32;
    let h = (t.humanize / 100.0).max(0.0);
    for m in meant {
        let target = m.note as f32;
        let (bias, spread) = (voice.bias_at(target), voice.spread_at(target));
        let keep = if spread <= h { 1.0 } else { (h / spread).powi(2) };
        let held = |f: &Frame| f.pitch.filter(|p| (p - m.sung).abs() <= NEAR);
        for k in m.start..m.end {
            let Some(p) = held(&frames[k]) else { continue };
            // the note's centre here: its pitch over about one swing of a vibrato
            let near: Vec<f32> =
                frames[k.saturating_sub(9).max(m.start)..(k + 10).min(m.end)].iter().filter_map(held).collect();
            let centre = near.iter().sum::<f32>() / near.len() as f32;
            let off = centre - target - bias;
            let toward = -(bias + (1.0 - keep) * off);
            let strength = t.strength.at(pitch::time(k) * bpm / 60.0, false).clamp(0.0, 1.0);
            let ramp = if tau <= 0.0 { 1.0 } else { 1.0 - (-((k - m.start) as f32) / tau).exp() };
            d[k] = Some(strength * ramp * toward + (t.vibrato.clamp(0.0, 1.0) - 1.0) * (p - centre));
        }
    }
    // between notes: a straight line from the last move to the next
    let known: Vec<usize> = (0..n).filter(|&k| d[k].is_some()).collect();
    let mut out = vec![0f32; n];
    for k in 0..n {
        out[k] = match d[k] {
            Some(v) => v,
            None => {
                let after = known.partition_point(|&j| j < k);
                match (after.checked_sub(1).map(|i| known[i]), known.get(after)) {
                    (Some(a), Some(&b)) => {
                        let f = (k - a) as f32 / (b - a) as f32;
                        d[a].unwrap_or(0.0) * (1.0 - f) + d[b].unwrap_or(0.0) * f
                    }
                    (Some(a), None) => d[a].unwrap_or(0.0),
                    (None, Some(&b)) => d[b].unwrap_or(0.0),
                    (None, None) => 0.0,
                }
            }
        };
    }
    (0..n)
        .map(|k| {
            out[k.saturating_sub(1)..(k + 2).min(n)].iter().sum::<f32>() / ((k + 2).min(n) - k.saturating_sub(1)) as f32
        })
        .collect()
}

/// the voiced stretches of `frames`, as frame ranges: voiced frames with gaps of at most two between them, of five
/// frames or more
fn stretches(frames: &[Frame]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < frames.len() {
        if frames[k].pitch.is_none() {
            k += 1;
            continue;
        }
        let start = k;
        let mut last = k;
        while k < frames.len() && k - last <= 2 {
            if frames[k].pitch.is_some() {
                last = k;
            }
            k += 1;
        }
        if last + 1 - start >= 5 {
            out.push((start, last + 1));
        }
        k = last + 1;
    }
    out
}

/// the pitch of the voiced frame nearest sample `s`
fn pitch_near(frames: &[Frame], s: usize) -> f32 {
    let k = pitch::frame_at(s as f64 / SRF).min(frames.len().saturating_sub(1));
    (0..=3)
        .flat_map(|d| [k.saturating_sub(d), (k + d).min(frames.len() - 1)])
        .find_map(|j| frames[j].pitch)
        .unwrap_or(57.0)
}

/// the move at sample `s`, between the frames either side of it
fn at(delta: &[f32], s: usize) -> f32 {
    let u = ((s as f64 / SRF - pitch::time(0)) / FRAME_SECS).max(0.0);
    let k = (u.floor() as usize).min(delta.len().saturating_sub(1));
    let next = (k + 1).min(delta.len() - 1);
    let f = (u - k as f64).clamp(0.0, 1.0) as f32;
    delta[k] * (1.0 - f) + delta[next] * f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::{Key, Mode};
    use crate::voice::{intent::Scale, sing};

    /// the middle of each note heard in `x`, read with `voice` in C major
    fn heard(x: &Stereo, voice: &Voice) -> Vec<(i32, f32)> {
        let frames = pitch::track(x);
        intent::decode(&frames, voice, &Scale::key(&Key::new(0, Mode::Major)))
            .iter()
            .map(|m| (m.note, m.sung))
            .collect()
    }

    fn c_major() -> Vec<(f64, [f32; 12])> {
        TuneScale::Key.harmony(&Key::new(0, Mode::Major), &[])
    }

    #[test]
    fn a_flat_singer_is_brought_onto_the_notes_and_a_precise_one_is_left_alone() {
        // C4, E4, G4, C5 sung 35 cents flat, each with a breath after it
        let melody: Vec<(f32, f64, bool)> = [60.0, 64.0, 67.0, 72.0].iter().map(|&p| (p - 0.35, 2.0, true)).collect();
        let mut x = sing(&melody);
        let tuned = tune(&mut x, &c_major(), &Tuning::default(), 120.0);
        assert_eq!(tuned.iter().map(|t| t.note).collect::<Vec<_>>(), [60, 64, 67, 72]);
        assert!(tuned.iter().all(|t| (t.now - t.note as f32).abs() < 0.08), "{tuned:?}");
        let anyone = Voice::default();
        for (note, sung) in heard(&x, &anyone) {
            assert!((sung - note as f32).abs() < 0.06, "{note} is heard at {sung}");
        }
        // sung in tune, each note stays within a couple of cents of where it was
        let melody: Vec<(f32, f64, bool)> = [60.0, 64.0, 67.0, 72.0].iter().map(|&p| (p, 2.0, true)).collect();
        let (before, mut y) = (sing(&melody), sing(&melody));
        tune(&mut y, &c_major(), &Tuning::default(), 120.0);
        for ((n, was), (m, now)) in heard(&before, &anyone).iter().zip(heard(&y, &anyone)) {
            assert!(*n == m && (now - was).abs() < 0.03, "{n}: {was} -> {now}");
        }
    }

    #[test]
    fn a_hard_tuner_takes_out_the_vibrato_too_and_a_natural_one_keeps_it() {
        let melody = [(64.2, 4.0, false)];
        let swing = |x: &Stereo| {
            let p: Vec<f32> = pitch::track(x)[30..150].iter().filter_map(|f| f.pitch).collect();
            let mean = p.iter().sum::<f32>() / p.len() as f32;
            (p.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / p.len() as f32).sqrt()
        };
        let mut natural = sing(&melody);
        tune(&mut natural, &c_major(), &Tuning::default(), 120.0);
        let mut hard = sing(&melody);
        let robot = Tuning { speed_ms: 0.0, humanize: 0.0, vibrato: 0.0, ..Tuning::default() };
        tune(&mut hard, &c_major(), &robot, 120.0);
        assert!(swing(&natural) > 0.08, "the vibrato stays: {}", swing(&natural));
        assert!(swing(&hard) < 0.03, "held dead still: {}", swing(&hard));
    }

    #[test]
    fn strength_by_section_and_notes_to_go_to() {
        let melody = [(64.4, 2.0, true), (64.4, 2.0, true)];
        let mut x = sing(&melody);
        // the first second untouched, the second tuned all the way
        let t = Tuning { strength: Param::Curve(vec![(0.0, 0.0), (1.99, 0.0), (2.0, 1.0)]), ..Tuning::default() };
        let tuned = tune(&mut x, &c_major(), &t, 120.0);
        assert!((tuned[0].now - 64.4).abs() < 0.03 && (tuned[1].now - 64.0).abs() < 0.08, "{tuned:?}");
        // told only D and F#, an E sung 40 cents sharp goes to the nearer of them
        let mut y = sing(&[(64.4, 2.0, false)]);
        let only = TuneScale::Notes(vec!["D".into(), "F#".into()]).alone();
        let tuned = tune(&mut y, &only, &Tuning { humanize: 0.0, ..Tuning::default() }, 120.0);
        assert_eq!(tuned.iter().map(|t| t.note).collect::<Vec<_>>(), [66], "{tuned:?}");
        assert_eq!(TuneScale::Notes(vec!["H".into()]).alone()[0].1, [1.0; 12], "no note named: any note");
    }

    #[test]
    fn a_frame_misheard_an_octave_off_moves_as_its_note_does() {
        // E4 held 30 cents sharp, three frames of it heard an octave up
        let f = |p: f32| Frame { pitch: Some(p), clarity: 0.9, db: -20.0 };
        let mut frames: Vec<Frame> = (0..40).map(|k| f(if (20..23).contains(&k) { 76.3 } else { 64.3 })).collect();
        frames.push(Frame { pitch: None, clarity: 0.0, db: -80.0 });
        let meant = [Meant { start: 0, end: 40, note: 64, sung: 64.3, sure: 1.0, settle: 0 }];
        let hard = Tuning { speed_ms: 0.0, humanize: 0.0, vibrato: 0.0, ..Tuning::default() };
        let d = corrections(&frames, &meant, &Voice::default(), &hard, 120.0);
        assert!(d[..40].iter().all(|v| (v + 0.3).abs() < 1e-4), "{d:?}");
    }

    #[test]
    fn nothing_sung_is_nothing_tuned() {
        let mut x = Stereo::zeros(48_000);
        assert!(tune(&mut x, &[], &Tuning::default(), 120.0).is_empty());
        assert!(x.l.iter().all(|&v| v == 0.0));
    }
}
