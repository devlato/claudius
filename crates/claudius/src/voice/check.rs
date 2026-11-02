//! A pitch check: short phrases played to sing back, and how near the singing came to each note. The phrases walk a
//! major scale in a range, mostly by step and now and then by a leap; each is played, and the singer answers it in
//! the gap after it, in whatever octave suits them. The answers are read as the notes they are (anyone's voice, in no
//! key: what was sung, not what should have been), lined up with the notes played, an octave or two away if that is
//! where the singer is, and the singer is learnt from how near each came (`singer::fit`).

use super::intent::{self, Meant, Scale};
use super::pitch::{self, FRAME_SECS};
use super::singer::{self, Heard, Voice};
use crate::audio::Stereo;
use crate::config::SRF;
use crate::rng::Rng;
use crate::theory::scale::{Key, Mode, note_name};

/// how many phrases, and how many notes each
const PHRASES: usize = 6;
const NOTES: usize = 4;
/// the tempo, a note to a beat; and how a phrase goes: its notes, a beat's rest, as long again to answer, a beat's rest
pub const BPM: f64 = 90.0;
const PHRASE_BEATS: f64 = 10.0;
const ANSWER_FROM: f64 = 5.0;
/// a note this far from the one before (semitones) is reached by a leap
const LEAP: i32 = 3;

/// the phrases to play, and where the answers go
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub key: Key,
    /// each phrase's notes: MIDI pitches, a beat each from the phrase's start
    pub phrases: Vec<Vec<i32>>,
}

/// a note asked for, and how it was sung back
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Answer {
    /// the note played, and the note in the singer's octave
    pub played: i32,
    pub meant: i32,
    /// where it was sung (a fractional MIDI note), when it was
    pub sung: Option<f32>,
    /// reached by a leap from the note before
    pub leap: bool,
}

impl Answer {
    /// how far from the note it was sung, cents
    pub fn cents(&self) -> Option<f32> {
        self.sung.map(|s| (s - self.meant as f32) * 100.0)
    }
}

/// what a pitch check heard: the singer, and every note asked for
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Checked {
    pub voice: Voice,
    pub answers: Vec<Answer>,
    /// how many octaves from the notes played the singer answered, most of the time (below 0: lower)
    pub octaves: i32,
}

impl Check {
    /// Phrases in a major key, between MIDI notes `low` and `high`, from `seed`.
    pub fn new(low: i32, high: i32, seed: u64) -> Check {
        let (low, high) = (low.clamp(36, 84), high.clamp(36, 84).max(low.clamp(36, 84) + 7));
        let mut rng = Rng::new(seed);
        let key = Key::new(rng.below(12) as u8, Mode::Major);
        let tones: Vec<i32> = (low..=high).filter(|&p| key.contains(p)).collect();
        let phrases = (0..PHRASES)
            .map(|_| {
                let mut at = rng.below(tones.len() / 2 + 1) + tones.len() / 4;
                (0..NOTES)
                    .map(|k| {
                        if k > 0 {
                            let step = if rng.chance(0.7) { 1 + rng.below(2) } else { 3 + rng.below(2) } as i64;
                            let way = if rng.chance(0.5) { 1 } else { -1 };
                            let next = at as i64 + way * step;
                            let next =
                                if next < 0 || next >= tones.len() as i64 { at as i64 - way * step } else { next };
                            at = next.clamp(0, tones.len() as i64 - 1) as usize;
                        }
                        tones[at]
                    })
                    .collect()
            })
            .collect();
        Check { key, phrases }
    }

    /// how long a beat is, seconds
    pub fn beat(&self) -> f64 {
        60.0 / BPM
    }

    /// the notes played, each as (MIDI pitch, beat it starts on, beats it lasts)
    pub fn notes(&self) -> Vec<(i32, f64, f64)> {
        self.phrases
            .iter()
            .enumerate()
            .flat_map(|(i, ph)| ph.iter().enumerate().map(move |(k, &p)| (p, i as f64 * PHRASE_BEATS + k as f64, 0.9)))
            .collect()
    }

    /// where each phrase's answer goes, (first beat, last beat): from the beat after its rest to the next phrase
    pub fn answers(&self) -> Vec<(f64, f64)> {
        (0..self.phrases.len())
            .map(|i| (i as f64 * PHRASE_BEATS + ANSWER_FROM, (i + 1) as f64 * PHRASE_BEATS))
            .collect()
    }

    /// how long the whole check lasts, beats
    pub fn beats(&self) -> f64 {
        self.phrases.len() as f64 * PHRASE_BEATS
    }

    /// what is played: each note a clear tone, its first three harmonics, a 10 ms attack and an 80 ms release
    pub fn guide(&self) -> Stereo {
        let mut out = Stereo::zeros((self.beats() * self.beat() * SRF) as usize);
        for (p, t, d) in self.notes() {
            let (at, len) = ((t * self.beat() * SRF) as usize, (d * self.beat() * SRF) as usize);
            let hz = pitch::hz(p as f64);
            for i in 0..len.min(out.len().saturating_sub(at)) {
                let s = i as f64 / SRF;
                let env = (s / 0.01).min(1.0).min((len - i) as f64 / SRF / 0.08);
                let ph = std::f64::consts::TAU * hz * s;
                let v = (ph.sin() + 0.35 * (2.0 * ph).sin() + 0.15 * (3.0 * ph).sin()) * 0.25 * env;
                out.l[at + i] += v as f32;
                out.r[at + i] += v as f32;
            }
        }
        out
    }

    /// How `sung` answers the phrases: a recording from the first beat of the check on.
    pub fn hear(&self, sung: &Stereo) -> Checked {
        let frames = pitch::track(sung);
        let beat = self.beat();
        let mut answers: Vec<Answer> = Vec::new();
        let mut heard: Vec<Heard> = Vec::new();
        let mut octaves: Vec<i32> = Vec::new();
        for (ph, (a, b)) in self.phrases.iter().zip(self.answers()) {
            // the answer's frames, from a little before its first beat (an eager singer) to the next phrase
            let (f0, f1) = (pitch::frame_at((a * beat - 0.2).max(0.0)), pitch::frame_at(b * beat).min(frames.len()));
            let found: Vec<Meant> = if f0 < f1 {
                intent::decode(&frames[f0..f1], &Voice::default(), &Scale::free())
                    .into_iter()
                    .map(|m| Meant { start: m.start + f0, end: m.end + f0, ..m })
                    .collect()
            } else {
                Vec::new()
            };
            let (shift, pairs) = [-24, -12, 0, 12]
                .iter()
                .map(|&k| (k, align(ph, &found, k)))
                .min_by(|x, y| x.1.0.total_cmp(&y.1.0))
                .map(|(k, (_, p))| (k, p))
                .unwrap_or((0, vec![None; ph.len()]));
            if pairs.iter().any(Option::is_some) {
                octaves.push(shift / 12);
            }
            for (k, (&played, pair)) in ph.iter().zip(&pairs).enumerate() {
                let meant = played + shift;
                let m = pair.map(|j| &found[j]);
                if let Some(m) = m {
                    let held: Vec<f32> = frames[m.start + m.settle..m.end].iter().filter_map(|f| f.pitch).collect();
                    heard.push(Heard {
                        meant: meant as f32,
                        sung: m.sung,
                        settle_ms: (m.settle as f64 * FRAME_SECS * 1000.0) as f32,
                        long: held.len() >= singer::LONG,
                        vibrato: singer::vibrato(&held),
                    });
                }
                let leap = k > 0 && (played - ph[k - 1]).abs() >= LEAP;
                answers.push(Answer { played, meant, sung: m.map(|m| m.sung), leap });
            }
        }
        octaves.sort();
        let octaves = octaves.get(octaves.len() / 2).copied().unwrap_or(0);
        Checked { voice: singer::fit(&heard), answers, octaves }
    }
}

/// The best way the notes `found` answer the phrase `played` sung `shift` semitones away, as for each note played the
/// one found for it, if any, and what that costs: a note sung for each played costs how far it was off (a semitone
/// and a half at most), a note played and not sung 1.2, a note sung that answers none 0.8.
fn align(played: &[i32], found: &[Meant], shift: i32) -> (f32, Vec<Option<usize>>) {
    let (n, m) = (played.len(), found.len());
    let off = |i: usize, j: usize| (found[j].sung - (played[i] + shift) as f32).abs().min(1.5);
    let mut cost = vec![vec![0f32; m + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=m {
            cost[i][j] = match (i, j) {
                (0, 0) => 0.0,
                (0, j) => cost[0][j - 1] + 0.8,
                (i, 0) => cost[i - 1][0] + 1.2,
                (i, j) => (cost[i - 1][j - 1] + off(i - 1, j - 1)).min(cost[i - 1][j] + 1.2).min(cost[i][j - 1] + 0.8),
            };
        }
    }
    let mut pairs = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        if (cost[i][j] - (cost[i - 1][j - 1] + off(i - 1, j - 1))).abs() < 1e-6 {
            pairs[i - 1] = Some(j - 1);
            (i, j) = (i - 1, j - 1);
        } else if (cost[i][j] - (cost[i - 1][j] + 1.2)).abs() < 1e-6 {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    (cost[n][m], pairs)
}

impl Checked {
    /// the notes sung back, and the share of them within `cents` of the note
    fn share(&self, cents: f32, of: impl Fn(&Answer) -> bool) -> (usize, f32) {
        let sung: Vec<f32> = self.answers.iter().filter(|a| of(a)).filter_map(Answer::cents).collect();
        let near = sung.iter().filter(|c| c.abs() <= cents).count();
        (sung.len(), near as f32 / sung.len().max(1) as f32)
    }

    /// in words: how many notes were sung back, how near, in which octave, steps against leaps, the voice
    pub fn describe(&self) -> String {
        let asked = self.answers.len();
        let (sung, near) = self.share(25.0, |_| true);
        if sung == 0 {
            return format!("{asked} notes played and none sung back: sing each phrase in the gap after it");
        }
        let (_, within) = self.share(50.0, |_| true);
        let octave = match self.octaves {
            0 => String::new(),
            k => format!(
                " ({} octave{} {} the notes played)",
                k.abs(),
                if k.abs() == 1 { "" } else { "s" },
                if k < 0 { "below" } else { "above" }
            ),
        };
        let (steps, by_step) = self.share(25.0, |a| !a.leap);
        let (leaps, by_leap) = self.share(25.0, |a| a.leap);
        let intervals = if steps > 0 && leaps > 0 {
            format!("; by step {:.0}% within 25 cents, by leap {:.0}%", by_step * 100.0, by_leap * 100.0)
        } else {
            String::new()
        };
        let mut off: Vec<&Answer> = self.answers.iter().filter(|a| a.cents().is_some_and(|c| c.abs() > 25.0)).collect();
        off.sort_by(|a, b| b.cents().unwrap_or(0.0).abs().total_cmp(&a.cents().unwrap_or(0.0).abs()));
        let furthest: Vec<String> = off
            .iter()
            .take(3)
            .map(|a| format!("{} ({:+.0} cents)", note_name(a.meant), a.cents().unwrap_or(0.0)))
            .collect();
        let furthest =
            if furthest.is_empty() { String::new() } else { format!("; furthest off: {}", furthest.join(", ")) };
        format!(
            "{sung} of {asked} notes sung back{octave}, {:.0}% within 25 cents of the note and {:.0}% within 50{intervals}; \
             {}{furthest}",
            near * 100.0,
            within * 100.0,
            self.voice.describe()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the check sung back an octave below, each note `off` semitones away, a beat each in the gap after its phrase
    fn answered(c: &Check, off: &dyn Fn(i32) -> f32) -> Stereo {
        let beat = c.beat();
        let mut out = Stereo::zeros((c.beats() * beat * SRF) as usize);
        for (ph, (a, _)) in c.phrases.iter().zip(c.answers()) {
            // `sing` counts in beats of half a second: a beat here is `beat / 0.5` of them, its last 60 ms a breath
            let melody: Vec<(f32, f64, bool)> =
                ph.iter().map(|&p| ((p - 12) as f32 + off(p), beat / 0.5, true)).collect();
            out.add_at(&crate::voice::sing(&melody), (a * beat * SRF) as i64, 1.0, 1.0);
        }
        out
    }

    #[test]
    fn the_phrases_keep_to_the_range_and_mostly_step() {
        let c = Check::new(55, 67, 7);
        assert_eq!((c.phrases.len(), c.phrases.iter().map(Vec::len).sum::<usize>()), (6, 24));
        assert!(c.phrases.iter().flatten().all(|&p| (55..=67).contains(&p) && c.key.contains(p)), "{c:?}");
        let moves: Vec<i32> = c.phrases.iter().flat_map(|ph| ph.windows(2).map(|w| (w[1] - w[0]).abs())).collect();
        assert!(moves.iter().filter(|&&m| m <= 4).count() * 2 > moves.len(), "{moves:?}");
        assert_eq!(c, Check::new(55, 67, 7), "the same seed, the same check");
        let (notes, answers) = (c.notes(), c.answers());
        assert_eq!((notes[4].1, answers[0], c.beats()), (10.0, (5.0, 10.0), 60.0));
        assert_eq!(c.guide().len(), (60.0 * c.beat() * SRF) as usize);
    }

    #[test]
    fn a_singer_an_octave_down_and_flat_at_the_top_is_heard_so() {
        let c = Check::new(60, 72, 3);
        // in tune below G4 as played, 30 cents flat from there up
        let x = answered(&c, &|p| if p >= 67 { -0.3 } else { 0.0 });
        let checked = c.hear(&x);
        assert_eq!(checked.octaves, -1, "{checked:?}");
        let sung = checked.answers.iter().filter(|a| a.sung.is_some()).count();
        assert!(sung >= 22, "nearly every note heard: {checked:?}");
        for a in checked.answers.iter().filter(|a| a.sung.is_some()) {
            let want = if a.played >= 67 { -30.0 } else { 0.0 };
            assert!((a.cents().unwrap() - want).abs() < 8.0, "{a:?}");
        }
        assert!(checked.voice.bias[2] < -0.15 && checked.voice.bias[0].abs() < 0.08, "{:?}", checked.voice);
        let words = checked.describe();
        assert!(words.contains("of 24 notes sung back (1 octave below the notes played)"), "{words}");
    }

    #[test]
    fn nothing_sung_back_is_said_so() {
        let c = Check::new(60, 72, 1);
        let checked = c.hear(&Stereo::zeros((c.beats() * c.beat() * SRF) as usize));
        assert!(checked.answers.iter().all(|a| a.sung.is_none()) && checked.voice == Voice::default());
        assert_eq!(checked.describe(), "24 notes played and none sung back: sing each phrase in the gap after it");
    }
}
