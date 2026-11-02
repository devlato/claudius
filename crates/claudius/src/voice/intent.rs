//! The notes a voice means, read in two steps. Where the notes are comes from its pitch every 10 ms, by a hidden Markov
//! model over the notes it may be singing (pYIN's note tracker in Tony, Mauch et al. 2015, works the same way): each
//! note has two states, its attack, where a voice scoops, slides and settles, and its hold, and silence is a state of
//! its own. Which note each one is comes from where it was held, once for the whole note: a singer puts a note off by
//! so much for the whole of it, not frame by frame, so a note's place is one reading of how they sing it (`Voice`'s
//! bias and scatter). Along the notes, what comes next follows the song: the tones of its key and of the chord under
//! the moment are likelier, and steps likelier than leaps. The wider a singer scatters, the more the song decides a
//! doubtful note; how sure each reading is comes out of the same chain (forward-backward).

use crate::theory::ChordSpan;
use crate::theory::scale::Key;

use super::pitch::{FRAME_SECS, Frame};
use super::singer::Voice;

/// the weight of a tone of the key, of a tone of the chord under the moment, and of any other tone
const IN_KEY: f32 = 1.0;
const IN_CHORD: f32 = 1.6;
const OUTSIDE: f32 = 0.12;
/// per frame: leaving a held note (half a second on average), and of that the share that falls silent; settling from
/// an attack onto the hold (50 ms); starting a note out of silence
const LEAVE: f32 = 0.02;
const TO_REST: f32 = 0.35;
const SETTLE: f32 = 0.2;
const ENTER: f32 = 0.03;
/// the same note sung again, against a step to the next
const AGAIN: f32 = 0.15;
/// a frame that is something else entirely (a harmonic taken for the note, a consonant's noise), anywhere in four
/// octaves; more of them where the frame is less clearly a pitch
const STRAY: f32 = 0.05;
/// a frame without a pitch inside a note (a consonant, a breath through it), or 30 dB under the loudest, which is a
/// silence rather than a consonant; and a frame with a pitch in silence
const UNVOICED_IN_NOTE: f32 = 0.1;
const QUIET_IN_NOTE: f32 = 0.02;
const VOICED_IN_REST: f32 = 0.1;
/// what a rise in loudness of more than 3 dB within 30 ms (a new syllable, a note sung anew) adds to an attack, per dB
/// up to 20
const ONSET: f32 = 0.15;
/// a frame is an attack's only so often: what its wider reach costs, so that a note held off its centre stays held
const ATTACK: f32 = 0.5;
/// a note shorter than this many frames is a slide or a slip, not a note
const SHORTEST: usize = 7;
/// along the notes: the same note again, against a step to the next (a melody repeats notes often)
const UNISON: f32 = 0.8;
/// after this long a silence (seconds) the note before says nothing of the next
const FORGET: f64 = 1.0;
/// how exactly a held note's place is measured, semitones
const MEASURED: f32 = 0.05;

/// what the song makes likelier from moment to moment: a weight for each pitch class, from each time (seconds) on
#[derive(Clone, Debug, PartialEq)]
pub struct Scale {
    pub at: Vec<(f64, [f32; 12])>,
}

impl Scale {
    /// any note as likely as any other
    pub fn free() -> Scale {
        Scale { at: vec![(0.0, [1.0; 12])] }
    }

    /// the tones of `key`
    pub fn key(key: &Key) -> Scale {
        Scale { at: vec![(0.0, key_weights(key))] }
    }

    /// the song's key and its chords (in beats), for a recording that starts on beat `from` of a song at `bpm`
    pub fn song(key: &Key, chords: &[ChordSpan], from: f64, bpm: f64) -> Scale {
        Scale { at: harmony(key, chords).into_iter().map(|(b, w)| (((b - from) * 60.0 / bpm).max(0.0), w)).collect() }
    }

    /// which of its weights hold at `secs`
    fn entry(&self, secs: f64) -> usize {
        self.at.iter().rposition(|(t, _)| *t <= secs + 1e-9).unwrap_or(0)
    }
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

fn key_weights(key: &Key) -> [f32; 12] {
    let mut w = [OUTSIDE; 12];
    for pc in key.pcs() {
        w[pc as usize] = IN_KEY;
    }
    w
}

/// the weights of the song's key, and of its chords from the beat each starts on (a chord that ends before the
/// recording starts still stands until the next: `Scale::entry` takes the last one begun)
pub fn harmony(key: &Key, chords: &[ChordSpan]) -> Vec<(f64, [f32; 12])> {
    let base = key_weights(key);
    let mut at = vec![(f64::MIN, base)];
    for c in chords {
        let mut w = base;
        for pc in c.chord.pcs() {
            w[pc as usize % 12] = IN_CHORD;
        }
        at.push((c.start, w));
    }
    at
}

/// a note the voice means
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meant {
    /// its frames, `start..end`
    pub start: usize,
    pub end: usize,
    /// the note, MIDI
    pub note: i32,
    /// where it was sung: the median pitch of its frames once settled
    pub sung: f32,
    /// how sure the reading is, from 0 to 1
    pub sure: f32,
    /// the frames it took to settle on the note: until its pitch stays within 30 cents of where it is held
    pub settle: usize,
}

impl Meant {
    pub fn secs(&self) -> f64 {
        (self.end - self.start) as f64 * FRAME_SECS
    }
}

/// the notes in `frames`, as `voice` sings them, in a song that makes `scale` likelier
pub fn decode(frames: &[Frame], voice: &Voice, scale: &Scale) -> Vec<Meant> {
    let Some(model) = Model::new(frames, voice, scale) else { return Vec::new() };
    let path = model.viterbi();
    let mut notes = model.notes(&path);
    name(&mut notes, voice, scale);
    notes
}

/// Which note each held note is: a chain over the notes, each read from where it was held as this voice sings, with
/// the song's weights and the step from the note before; Viterbi names them, forward-backward says how sure each is.
/// A note's candidates are the notes within a tritone of where it was held that the song allows at all.
fn name(notes: &mut [Meant], voice: &Voice, scale: &Scale) {
    if notes.is_empty() {
        return;
    }
    let weight = |m: &Meant, n: i32| scale.at[scale.entry(m.start as f64 * FRAME_SECS)].1[n.rem_euclid(12) as usize];
    let cands: Vec<Vec<i32>> = notes
        .iter()
        .map(|m| {
            let round = m.sung.round() as i32;
            let allowed: Vec<i32> = (-6..=6).map(|d| round + d).filter(|&n| weight(m, n) > 0.0).collect();
            if allowed.is_empty() { (-2..=2).map(|d| round + d).collect() } else { allowed }
        })
        .collect();
    let heard = |m: &Meant, n: i32| {
        let s = voice.spread_at(n as f32).hypot(MEASURED);
        let z = (m.sung - n as f32 - voice.bias_at(n as f32)) / s;
        -0.5 * z * z - s.ln()
    };
    let step = |a: i32, b: i32| if a == b { UNISON } else { (-((a.abs_diff(b) - 1) as f32) / 5.0).exp() };
    let ln_weight = |m: &Meant, n: i32| weight(m, n).max(1e-9).ln();
    // ln of moving from candidate i of note j - 1 to candidate k of note j, normalised over k
    let moves = |j: usize| -> Vec<Vec<f32>> {
        let gap = (notes[j].start - notes[j - 1].end) as f64 * FRAME_SECS;
        cands[j - 1]
            .iter()
            .map(|&a| {
                let row: Vec<f32> = cands[j]
                    .iter()
                    .map(|&b| ln_weight(&notes[j], b) + if gap > FORGET { 0.0 } else { step(a, b).ln() })
                    .collect();
                let total = lse(&row);
                row.iter().map(|r| r - total).collect()
            })
            .collect()
    };
    let first: Vec<f32> = {
        let row: Vec<f32> = cands[0].iter().map(|&b| ln_weight(&notes[0], b)).collect();
        let total = lse(&row);
        row.iter().zip(&cands[0]).map(|(r, &b)| r - total + heard(&notes[0], b)).collect()
    };
    let steps: Vec<Vec<Vec<f32>>> = (1..notes.len()).map(moves).collect();
    // forward, in ln, with Viterbi's best and its way back alongside
    let mut alpha = vec![first.clone()];
    let mut best = vec![first];
    let mut back: Vec<Vec<usize>> = vec![vec![0; cands[0].len()]];
    for j in 1..notes.len() {
        let mv = &steps[j - 1];
        let (prev, here) = (cands[j - 1].len(), cands[j].len());
        let mut a = Vec::with_capacity(here);
        let mut b = Vec::with_capacity(here);
        let mut from = Vec::with_capacity(here);
        for k in 0..here {
            let e = heard(&notes[j], cands[j][k]);
            let into: Vec<f32> = (0..prev).map(|i| alpha[j - 1][i] + mv[i][k]).collect();
            let (arg, top) = (0..prev)
                .map(|i| (i, best[j - 1][i] + mv[i][k]))
                .max_by(|x, y| x.1.total_cmp(&y.1))
                .unwrap_or((0, f32::MIN));
            a.push(lse(&into) + e);
            b.push(top + e);
            from.push(arg);
        }
        alpha.push(a);
        best.push(b);
        back.push(from);
    }
    // backward
    let last = notes.len() - 1;
    let mut beta: Vec<Vec<f32>> = cands.iter().map(|c| vec![0f32; c.len()]).collect();
    for j in (0..last).rev() {
        let mv = &steps[j];
        beta[j] = (0..cands[j].len())
            .map(|i| {
                let out: Vec<f32> = (0..cands[j + 1].len())
                    .map(|k| mv[i][k] + heard(&notes[j + 1], cands[j + 1][k]) + beta[j + 1][k])
                    .collect();
                lse(&out)
            })
            .collect();
    }
    let mut k = (0..cands[last].len()).max_by(|&x, &y| best[last][x].total_cmp(&best[last][y])).unwrap_or(0);
    for j in (0..notes.len()).rev() {
        let post: Vec<f32> = (0..cands[j].len()).map(|i| alpha[j][i] + beta[j][i]).collect();
        let total = lse(&post);
        notes[j].note = cands[j][k];
        notes[j].sure = (post[k] - total).exp();
        k = back[j][k];
    }
}

/// ln of the sum of the exponentials of `v`
fn lse(v: &[f32]) -> f32 {
    let top = v.iter().copied().fold(f32::MIN, f32::max);
    top + v.iter().map(|x| (x - top).exp()).sum::<f32>().ln()
}

/// the model over one recording: its candidate notes and what each frame says of each
struct Model<'a> {
    frames: &'a [Frame],
    voice: &'a Voice,
    scale: &'a Scale,
    /// the lowest candidate note, and how many there are
    lo: i32,
    m: usize,
    /// each frame's pitch held over 150 ms (its median), which keeps a note's edges and loses its vibrato
    held: Vec<Option<f32>>,
    /// each frame's rise in loudness over the 30 ms before it, dB
    rise: Vec<f32>,
    /// 30 dB under the recording's loudest frame
    quiet: f32,
}

/// the transitions under one set of weights: from each hold to each attack, and from silence to each attack (ln)
struct Moves {
    hold: Vec<f32>,
    rest: Vec<f32>,
}

impl<'a> Model<'a> {
    fn new(frames: &'a [Frame], voice: &'a Voice, scale: &'a Scale) -> Option<Model<'a>> {
        let mut voiced: Vec<f32> = frames.iter().filter_map(|f| f.pitch).collect();
        if voiced.len() < SHORTEST {
            return None;
        }
        voiced.sort_by(f32::total_cmp);
        let pick = |q: f32| voiced[((voiced.len() - 1) as f32 * q) as usize];
        let lo = (pick(0.02).floor() as i32 - 2).max(24);
        let hi = (pick(0.98).ceil() as i32 + 2).clamp(lo + 1, lo + 60);
        let held = (0..frames.len())
            .map(|t| {
                frames[t].pitch?;
                let mut near: Vec<f32> =
                    frames[t.saturating_sub(7)..(t + 8).min(frames.len())].iter().filter_map(|f| f.pitch).collect();
                near.sort_by(f32::total_cmp);
                Some(near[near.len() / 2])
            })
            .collect();
        let rise = (0..frames.len())
            .map(|t| {
                let before = frames[t.saturating_sub(3)..t].iter().map(|f| f.db).fold(f32::MAX, f32::min);
                if t == 0 { 0.0 } else { (frames[t].db - before).clamp(0.0, 20.0) }
            })
            .collect();
        let quiet = frames.iter().map(|f| f.db).fold(f32::MIN, f32::max) - 30.0;
        Some(Model { frames, voice, scale, lo, m: (hi - lo + 1) as usize, held, rise, quiet })
    }

    fn rest(&self) -> usize {
        2 * self.m
    }

    /// ln of what frame `t` says of note `k` (its attack or its hold)
    fn heard(&self, t: usize, k: usize, attack: bool) -> f32 {
        let f = &self.frames[t];
        let Some(p) = f.pitch else {
            return if f.db < self.quiet { QUIET_IN_NOTE.ln() } else { UNVOICED_IN_NOTE.ln() };
        };
        let note = (self.lo + k as i32) as f32;
        let centre = note + self.voice.bias_at(note);
        let s = self.voice.spread_at(note).hypot(0.03);
        let (x, s) = if attack { (p, (2.5 * s).max(0.8)) } else { (self.held[t].unwrap_or(p), s) };
        let stray = (STRAY + 0.5 * (1.0 - f.clarity)).min(0.5);
        let z = (x - centre) / s;
        let near = (1.0 - stray) * (-0.5 * z * z).exp() / (s * std::f32::consts::TAU.sqrt());
        let onset = ONSET * (self.rise[t] - 3.0).max(0.0);
        let share = if attack { ATTACK.ln() + onset } else { -onset };
        (1.0 - UNVOICED_IN_NOTE).ln() + (near + stray / 48.0).ln() + share
    }

    /// ln of what frame `t` says of silence
    fn silent(&self, t: usize) -> f32 {
        if self.frames[t].pitch.is_some() { (VOICED_IN_REST / 48.0).ln() } else { (1.0 - VOICED_IN_REST).ln() }
    }

    /// what frame `t` says of every state: note k's attack at 2k, its hold at 2k + 1, silence last
    fn emissions(&self, t: usize) -> Vec<f32> {
        let mut e = Vec::with_capacity(2 * self.m + 1);
        for k in 0..self.m {
            e.push(self.heard(t, k, true));
            e.push(self.heard(t, k, false));
        }
        e.push(self.silent(t));
        e
    }

    /// how likely note `k` is next after note `from` (or out of silence) under weights `w`, before normalising
    fn next(&self, w: &[f32; 12], from: Option<usize>, k: usize) -> f32 {
        let pc = (self.lo + k as i32).rem_euclid(12) as usize;
        let step = match from {
            None => 1.0,
            Some(f) if f == k => AGAIN,
            Some(f) => (-((f.abs_diff(k) - 1) as f32) / 4.0).exp(),
        };
        step * w[pc]
    }

    fn moves(&self, w: &[f32; 12]) -> Moves {
        let m = self.m;
        let mut hold = vec![0f32; m * m];
        for i in 0..m {
            let row: Vec<f32> = (0..m).map(|k| self.next(w, Some(i), k)).collect();
            let sum: f32 = row.iter().sum();
            for k in 0..m {
                hold[i * m + k] = (LEAVE * (1.0 - TO_REST) * row[k] / sum).ln();
            }
        }
        let row: Vec<f32> = (0..m).map(|k| self.next(w, None, k)).collect();
        let sum: f32 = row.iter().sum();
        Moves { hold, rest: row.iter().map(|r| (ENTER * r / sum).ln()).collect() }
    }

    /// the likeliest path of states, one a frame
    fn viterbi(&self) -> Vec<usize> {
        let (m, n, rest) = (self.m, self.frames.len(), self.rest());
        let states = 2 * m + 1;
        let mut moves: Vec<Option<Moves>> = (0..self.scale.at.len()).map(|_| None).collect();
        let (stay_attack, settle, stay_hold) = ((1.0 - SETTLE).ln(), SETTLE.ln(), (1.0 - LEAVE).ln());
        let (stay_rest, fall_silent) = ((1.0 - ENTER).ln(), (LEAVE * TO_REST).ln());
        let e = self.emissions(0);
        let mut delta: Vec<f32> = (0..states)
            .map(|s| {
                if s == rest {
                    0.5f32.ln()
                } else if s.is_multiple_of(2) {
                    (0.5 / m as f32).ln()
                } else {
                    f32::MIN
                }
            })
            .zip(&e)
            .map(|(a, b)| a + b)
            .collect();
        let mut back = vec![0u16; n * states];
        let mut next = vec![f32::MIN; states];
        for t in 1..n {
            let entry = self.scale.entry(t as f64 * FRAME_SECS);
            let mv = moves[entry].get_or_insert_with(|| self.moves(&self.scale.at[entry].1));
            let e = self.emissions(t);
            let row = &mut back[t * states..(t + 1) * states];
            for k in 0..m {
                // into note k's attack: it goes on, or a held note moves to it, or it starts out of silence
                let (mut best, mut from) = (delta[2 * k] + stay_attack, 2 * k);
                let r = delta[rest] + mv.rest[k];
                if r > best {
                    (best, from) = (r, rest);
                }
                for i in 0..m {
                    let c = delta[2 * i + 1] + mv.hold[i * m + k];
                    if c > best {
                        (best, from) = (c, 2 * i + 1);
                    }
                }
                next[2 * k] = best + e[2 * k];
                row[2 * k] = from as u16;
                // into its hold: from its attack, or it goes on
                let (a, h) = (delta[2 * k] + settle, delta[2 * k + 1] + stay_hold);
                (next[2 * k + 1], row[2 * k + 1]) =
                    if a > h { (a + e[2 * k + 1], 2 * k as u16) } else { (h + e[2 * k + 1], (2 * k + 1) as u16) };
            }
            let (mut best, mut from) = (delta[rest] + stay_rest, rest);
            for i in 0..m {
                let c = delta[2 * i + 1] + fall_silent;
                if c > best {
                    (best, from) = (c, 2 * i + 1);
                }
            }
            next[rest] = best + e[rest];
            row[rest] = from as u16;
            std::mem::swap(&mut delta, &mut next);
        }
        let mut s = (0..states).max_by(|&a, &b| delta[a].total_cmp(&delta[b])).unwrap_or(rest);
        let mut path = vec![rest; n];
        for t in (0..n).rev() {
            path[t] = s;
            s = back[t * states + s] as usize;
        }
        path
    }

    /// the notes along `path`: a note starts where the path enters its attack, and ends where it leaves the note
    fn notes(&self, path: &[usize]) -> Vec<Meant> {
        let rest = self.rest();
        let mut out: Vec<Meant> = Vec::new();
        let mut t = 0;
        while t < path.len() {
            if path[t] == rest {
                t += 1;
                continue;
            }
            let k = path[t] / 2;
            let start = t;
            t += 1;
            // the note goes on while the path stays on it, until an attack after its hold (the note sung again)
            let attack = |s: usize| s != rest && s.is_multiple_of(2);
            let again = |t: usize| attack(path[t]) && !attack(path[t - 1]);
            while t < path.len() && path[t] != rest && path[t] / 2 == k && !again(t) {
                t += 1;
            }
            if t - start < SHORTEST {
                continue;
            }
            let voiced: Vec<(usize, f32)> = (start..t).filter_map(|u| self.frames[u].pitch.map(|p| (u, p))).collect();
            if voiced.is_empty() {
                continue;
            }
            // settled: from the first of three frames in a row within 30 cents of where the note is held
            let middle = median(voiced.iter().map(|v| v.1).collect());
            let settled =
                voiced.windows(3).find(|w| w.iter().all(|v| (v.1 - middle).abs() < 0.3)).map_or(start, |w| w[0].0);
            let sung = median(voiced.iter().filter(|v| v.0 >= settled).map(|v| v.1).collect());
            out.push(Meant { start, end: t, note: self.lo + k as i32, sung, sure: 1.0, settle: settled - start });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::{Chord, Quality};
    use crate::theory::scale::Mode;
    use crate::voice::{pitch, sing};

    fn notes(meant: &[Meant]) -> Vec<i32> {
        meant.iter().map(|m| m.note).collect()
    }

    #[test]
    fn a_sung_melody_is_read_as_its_notes_and_sure_of_them() {
        let x =
            sing(&[(60.0, 1.0, false), (62.0, 1.0, true), (64.0, 0.5, false), (67.0, 1.5, true), (65.0, 1.0, false)]);
        let meant = decode(&pitch::track(&x), &Voice::default(), &Scale::key(&Key::new(0, Mode::Major)));
        assert_eq!(notes(&meant), [60, 62, 64, 67, 65], "{meant:?}");
        assert!(meant.iter().all(|m| m.sure > 0.9 && (m.sung - m.note as f32).abs() < 0.1), "{meant:?}");
        // each where it was sung, give or take a frame or two of its scoop
        let starts: Vec<f64> = meant.iter().map(|m| pitch::time(m.start)).collect();
        for (got, want) in starts.iter().zip([0.0, 0.5, 1.0, 1.25, 2.0]) {
            assert!((got - want).abs() < 0.06, "{starts:?}");
        }
    }

    #[test]
    fn a_singer_flat_at_the_top_is_read_as_the_notes_they_mean_once_their_voice_is_known() {
        // G4, A4, B4, C5, B4, C5 in C major, each 60 cents flat: nearer the note below than the note meant
        let melody: Vec<(f32, f64, bool)> =
            [67.0, 69.0, 71.0, 72.0, 71.0, 72.0].iter().map(|&p| (p - 0.6, 1.0, true)).collect();
        let frames = pitch::track(&sing(&melody));
        let c = Scale::key(&Key::new(0, Mode::Major));
        // as anyone's: the key puts right the notes whose neighbour below is outside it, but not the Cs over a B
        assert_eq!(notes(&decode(&frames, &Voice::default(), &c)), [67, 69, 71, 71, 71, 71]);
        let flat = Voice { low: 60.0, high: 72.0, bias: [-0.6; 3], spread: [0.1; 3], ..Voice::default() };
        let meant = decode(&frames, &flat, &c);
        assert_eq!(notes(&meant), [67, 69, 71, 72, 71, 72]);
        assert!(meant.iter().all(|m| m.sure > 0.95), "{meant:?}");
    }

    #[test]
    fn a_doubtful_note_goes_to_the_keys_tone_and_then_to_the_chords() {
        // 60 cents above F4, 40 below F#4
        let frames = pitch::track(&sing(&[(65.6, 2.0, false)]));
        let anyone = Voice::default();
        let c = Key::new(0, Mode::Major);
        assert_eq!(notes(&decode(&frames, &anyone, &Scale::key(&c))), [65], "C major has F");
        assert_eq!(notes(&decode(&frames, &anyone, &Scale::key(&Key::new(7, Mode::Major)))), [66], "G major has F#");
        // over a D major chord in C major, F# is the chord's
        let d = [ChordSpan { start: 0.0, len: 8.0, chord: Chord::new(2, Quality::Maj) }];
        let over_d = decode(&frames, &anyone, &Scale::song(&c, &d, 0.0, 120.0));
        assert_eq!(notes(&over_d), [66]);
        assert!(over_d[0].sure > 0.5 && over_d[0].sure < 0.9, "a doubtful note is no sure reading: {over_d:?}");
        assert!(over_d[0].settle < 10, "held, not an attack all along: {over_d:?}");
    }

    #[test]
    fn the_scale_follows_the_chords_from_where_the_recording_starts() {
        let c = Key::new(0, Mode::Major);
        let chords = [
            ChordSpan { start: 0.0, len: 4.0, chord: Chord::new(0, Quality::Maj) },
            ChordSpan { start: 4.0, len: 4.0, chord: Chord::new(2, Quality::Maj) },
        ];
        // a recording from beat 2 at 120 BPM: C major for its first second, D major after
        let s = Scale::song(&c, &chords, 2.0, 120.0);
        assert_eq!(s.at[s.entry(0.5)].1[0], IN_CHORD);
        assert_eq!(s.at[s.entry(1.5)].1[6], IN_CHORD, "F# in D major");
        assert_eq!(s.at[s.entry(1.5)].1[1], OUTSIDE);
    }

    #[test]
    fn falling_off_a_note_into_a_breath_and_scooping_into_the_next_is_two_notes() {
        // C4 falling off for its last two frames, a breath, then E4 scooped into from below
        let voiced = |p: f32| Frame { pitch: Some(p), clarity: 0.95, db: -20.0 };
        let mut frames: Vec<Frame> = vec![voiced(60.0); 10];
        frames.extend([voiced(58.0), voiced(58.0)]);
        frames.extend([Frame { pitch: None, clarity: 0.1, db: -70.0 }; 3]);
        frames.push(voiced(61.0));
        frames.extend(vec![voiced(64.0); 10]);
        assert_eq!(notes(&decode(&frames, &Voice::default(), &Scale::free())), [60, 64]);
    }

    #[test]
    fn nothing_held_is_no_notes() {
        assert!(decode(&[], &Voice::default(), &Scale::free()).is_empty());
        let noise = vec![Frame { pitch: None, clarity: 0.2, db: -30.0 }; 50];
        assert!(decode(&noise, &Voice::default(), &Scale::free()).is_empty());
    }
}
