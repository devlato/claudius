//! How a person sings: where they put a note against where it is, at the bottom of their range, in the middle and at the
//! top; how widely their held notes scatter around that; how long they take to settle on a note; their vibrato. A
//! `Voice` is learnt from notes heard against the notes meant (`fit`), and says how likely they are to sing a note
//! within so many cents of it (`hit`): what reads their singing as the notes they mean, and what decides how far a
//! tuner moves them.

use serde::{Deserialize, Serialize};

use crate::theory::scale::note_name;

/// the scatter of anyone's held notes, one standard deviation in semitones, where nothing better is known
pub const ANYONE: f32 = 0.3;
/// a note sung this far (semitones) from the note meant is another note, missed, instead of that one sung off
pub const MISSED: f32 = 0.75;
/// how many frames (of 10 ms) a held note needs for its vibrato to be heard
pub const LONG: usize = 40;
/// how many notes' worth of anyone's singing a fit leans on where it has few notes of its own: for where they sing,
/// and for how widely
const LEAN_BIAS: f64 = 0.1;
const LEAN: f32 = 0.5;

/// a person's singing
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Voice {
    /// the lowest and highest notes meant, MIDI
    pub low: f32,
    pub high: f32,
    /// how far off they sing, in semitones (below 0: flat), at the bottom of the range, the middle and the top
    pub bias: [f32; 3],
    /// how widely their held notes scatter around that, one standard deviation in semitones, at the same three places
    pub spread: [f32; 3],
    /// how long they take to settle on a note, ms
    pub settle_ms: f32,
    /// their vibrato on long notes: how fast (Hz) and how wide (semitones either way); 0 where they hold notes still
    pub vibrato_hz: f32,
    pub vibrato_st: f32,
    /// the notes this was learnt from, how many of them were missed, and how many were long enough to hear a vibrato in
    pub notes: usize,
    pub missed: usize,
    #[serde(default)]
    pub long: usize,
}

/// anyone: in tune on the whole, 30 cents either way, across an octave either side of middle C
impl Default for Voice {
    fn default() -> Voice {
        Voice {
            low: 48.0,
            high: 72.0,
            bias: [0.0; 3],
            spread: [ANYONE; 3],
            settle_ms: 60.0,
            vibrato_hz: 0.0,
            vibrato_st: 0.0,
            notes: 0,
            missed: 0,
            long: 0,
        }
    }
}

/// the version of the voice file claudius writes (`settings`): 1, the first with one; claudius before 0.6 wrote 0
pub const VOICE_VERSION: u64 = 1;

impl Voice {
    /// The voice a settings file holds, as any claudius wrote it (`settings::Fields`): each field read on its own, and
    /// anyone's where it does not read; none when the file holds no voice, or none heard in any notes.
    pub fn from_settings(text: &str) -> Option<Voice> {
        let f = crate::settings::Fields::parse(text);
        let d = Voice::default();
        let notes = f.get::<usize>("notes")?;
        Some(Voice {
            low: f.or("low", d.low),
            high: f.or("high", d.high),
            bias: f.or("bias", d.bias),
            spread: f.or("spread", d.spread),
            settle_ms: f.or("settle_ms", d.settle_ms),
            vibrato_hz: f.or("vibrato_hz", d.vibrato_hz),
            vibrato_st: f.or("vibrato_st", d.vibrato_st),
            notes,
            missed: f.or("missed", d.missed),
            long: f.or("long", d.long),
        })
    }

    /// the settings file of the voice, keeping what the file as it was (`earlier`) holds that this claudius does not
    pub fn to_settings(&self, earlier: &str) -> String {
        crate::settings::Fields::parse(earlier).with(self, VOICE_VERSION).to_text()
    }
}

/// a note heard against the note meant: where it was sung (the median of its held part), how long it took to settle,
/// whether it was held long enough to hear a vibrato in, and its vibrato (Hz, semitones either way) where it has one
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    pub meant: f32,
    pub sung: f32,
    pub settle_ms: f32,
    pub long: bool,
    pub vibrato: Option<(f32, f32)>,
}

impl Voice {
    /// how much the bottom, the middle and the top of the range count at `note`
    fn weights(&self, note: f32) -> [f32; 3] {
        let u = 2.0 * ((note - self.low) / (self.high - self.low).max(1.0)).clamp(0.0, 1.0);
        if u <= 1.0 { [1.0 - u, u, 0.0] } else { [0.0, 2.0 - u, u - 1.0] }
    }

    /// how far off they sing `note`, semitones
    pub fn bias_at(&self, note: f32) -> f32 {
        let w = self.weights(note);
        (0..3).map(|i| w[i] * self.bias[i]).sum()
    }

    /// how widely they scatter around it, semitones
    pub fn spread_at(&self, note: f32) -> f32 {
        let w = self.weights(note);
        (0..3).map(|i| w[i] * self.spread[i]).sum::<f32>().max(0.02)
    }

    /// the chance that they sing `note` within `cents` of it
    pub fn hit(&self, note: f32, cents: f32) -> f32 {
        let (m, s, c) = (self.bias_at(note), self.spread_at(note), cents / 100.0);
        phi((c - m) / s) - phi((-c - m) / s)
    }

    /// the middle of the range
    pub fn middle(&self) -> f32 {
        0.5 * (self.low + self.high)
    }

    /// in words: the range, where they sing sharp or flat, how widely, how sure a note is, the settling, the vibrato
    pub fn describe(&self) -> String {
        let cents = |st: f32| (st * 100.0).round() as i32;
        let places = [("at the bottom", self.low), ("in the middle", self.middle()), ("at the top", self.high)];
        let off: Vec<String> = (0..3)
            .filter(|&i| self.bias[i].abs() >= 0.08)
            .map(|i| {
                let way = if self.bias[i] < 0.0 { "flat" } else { "sharp" };
                format!("{way} by {} cents {} ({})", cents(self.bias[i].abs()), places[i].0, name(places[i].1))
            })
            .collect();
        let tune = if off.is_empty() { "in tune on the whole".to_string() } else { off.join(", ") };
        let scatter = cents(self.spread.iter().sum::<f32>() / 3.0);
        let notes: Vec<f32> = (self.low.round() as i32..=self.high.round() as i32).map(|n| n as f32).collect();
        let sure = notes.iter().map(|&n| self.hit(n, 25.0)).sum::<f32>() / notes.len().max(1) as f32;
        let vibrato = if self.vibrato_st >= 0.05 {
            format!(
                "; a vibrato of {:.1} Hz, {} cents either way, on long notes",
                self.vibrato_hz,
                cents(self.vibrato_st)
            )
        } else if self.long > 0 {
            "; long notes held still".to_string()
        } else {
            String::new()
        };
        format!(
            "range {}-{}; {tune}; held notes scatter {scatter} cents either way, so a note lands within 25 cents {:.0}% \
             of the time; settles on a note in {:.0} ms{vibrato}",
            name(self.low),
            name(self.high),
            sure * 100.0,
            self.settle_ms
        )
    }
}

fn name(note: f32) -> String {
    note_name(note.round() as i32)
}

/// Learns a voice from notes heard against the notes meant. Notes a semitone or so away are missed and do not count
/// towards where the voice sings; where it has few notes, a fit leans on anyone's singing.
pub fn fit(heard: &[Heard]) -> Voice {
    let mut v = Voice::default();
    if heard.is_empty() {
        return v;
    }
    v.low = heard.iter().map(|h| h.meant).fold(f32::MAX, f32::min);
    v.high = heard.iter().map(|h| h.meant).fold(f32::MIN, f32::max);
    let kept: Vec<&Heard> = heard.iter().filter(|h| (h.sung - h.meant).abs() < MISSED).collect();
    (v.notes, v.missed) = (heard.len(), heard.len() - kept.len());
    // where they sing: least squares over the three places, each leaning a little on in-tune singing
    let mut a = [[0f64; 3]; 3];
    let mut b = [0f64; 3];
    for h in &kept {
        let w = v.weights(h.meant);
        for i in 0..3 {
            b[i] += (w[i] * (h.sung - h.meant)) as f64;
            for j in 0..3 {
                a[i][j] += (w[i] * w[j]) as f64;
            }
        }
    }
    for (i, row) in a.iter_mut().enumerate() {
        row[i] += LEAN_BIAS;
    }
    v.bias = solve(a, b).map(|x| x as f32);
    // how widely: what is left around that at each place, leaning on anyone's scatter for half a note
    for i in 0..3 {
        let (mut sum, mut weight) = (LEAN * ANYONE * ANYONE, LEAN);
        for h in &kept {
            let w = v.weights(h.meant)[i];
            let r = h.sung - h.meant - v.bias_at(h.meant);
            sum += w * r * r;
            weight += w;
        }
        v.spread[i] = (sum / weight).sqrt().max(0.02);
    }
    let mut settle: Vec<f32> = kept.iter().map(|h| h.settle_ms).collect();
    if !settle.is_empty() {
        settle.sort_by(f32::total_cmp);
        v.settle_ms = settle[settle.len() / 2];
    }
    v.long = kept.iter().filter(|h| h.long).count();
    let mut vib: Vec<(f32, f32)> = kept.iter().filter_map(|h| h.vibrato).collect();
    if !vib.is_empty() {
        vib.sort_by(|x, y| x.1.total_cmp(&y.1));
        (v.vibrato_hz, v.vibrato_st) = vib[vib.len() / 2];
    }
    v
}

/// `a x = b` for three unknowns, by Cramer's rule (`a` is a sum of squares plus a ridge, so never singular)
fn solve(a: [[f64; 3]; 3], b: [f64; 3]) -> [f64; 3] {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    std::array::from_fn(|k| {
        let mut m = a;
        for (row, &v) in m.iter_mut().zip(&b) {
            row[k] = v;
        }
        det(m) / d
    })
}

/// The vibrato in a held note's pitch, one reading every 10 ms: its rate (Hz) and how wide it goes (semitones either
/// way), where the pitch swings regularly 4 to 8 times a second and by 5 cents or more. The note's drift is a line
/// taken out first.
pub fn vibrato(pitch: &[f32]) -> Option<(f32, f32)> {
    let n = pitch.len();
    if n < LONG {
        return None;
    }
    // the line through the pitch, taken out
    let mean_t = (n - 1) as f32 / 2.0;
    let mean_p = pitch.iter().sum::<f32>() / n as f32;
    let (mut sxy, mut sxx) = (0f32, 0f32);
    for (t, &p) in pitch.iter().enumerate() {
        sxy += (t as f32 - mean_t) * (p - mean_p);
        sxx += (t as f32 - mean_t) * (t as f32 - mean_t);
    }
    let slope = sxy / sxx;
    let x: Vec<f32> = pitch.iter().enumerate().map(|(t, &p)| p - mean_p - slope * (t as f32 - mean_t)).collect();
    let energy: f32 = x.iter().map(|v| v * v).sum();
    let depth = (2.0 * energy / n as f32).sqrt();
    if depth < 0.05 {
        return None;
    }
    let r = |lag: usize| x[..n - lag].iter().zip(&x[lag..]).map(|(a, b)| a * b).sum::<f32>() / energy;
    let lags: Vec<(usize, f32)> = (12..=25).map(|l| (l, r(l))).collect();
    let &(lag, peak) = lags.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
    if peak < 0.3 || lag == 12 || lag == 25 {
        return None;
    }
    let (l, c, rr) = (r(lag - 1), peak, r(lag + 1));
    let bend = l - 2.0 * c + rr;
    let exact = lag as f32 + if bend.abs() > 1e-9 { 0.5 * (l - rr) / bend } else { 0.0 };
    Some((100.0 / exact, depth))
}

/// the standard normal's cumulative distribution
fn phi(z: f32) -> f32 {
    0.5 * (1.0 + erf(z / std::f32::consts::SQRT_2))
}

/// the error function, to 1.5e-7 (Abramowitz and Stegun 7.1.26)
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs() as f64);
    let poly =
        t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let y = 1.0 - poly * (-(x as f64) * x as f64).exp();
    (if x >= 0.0 { y } else { -y }) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_voice_file_any_claudius_wrote_is_read() {
        // as claudius 0.4 wrote it, before it heard long notes; and 0.5, with them; neither with a version
        let v04 = r#"{"low": 50.0, "high": 74.0, "bias": [-0.2, -0.1, 0.0], "spread": [0.3, 0.25, 0.3],
            "settle_ms": 80.0, "vibrato_hz": 5.5, "vibrato_st": 0.2, "notes": 24, "missed": 1}"#;
        let v04 = Voice::from_settings(v04).unwrap();
        assert_eq!((v04.low, v04.bias[0], v04.notes, v04.long), (50.0, -0.2, 24, 0));
        let v05 = Voice::from_settings(&serde_json::to_string(&Voice { long: 6, ..v04.clone() }).unwrap()).unwrap();
        assert_eq!(v05.long, 6);
        // a later one's, with a field changed and one this claudius does not know: kept when written again
        let later = r#"{"version": 4, "low": "C3", "high": 76.0, "notes": 30, "timbre": {"bright": 0.4}}"#;
        let v = Voice::from_settings(later).unwrap();
        assert_eq!((v.low, v.high, v.notes), (Voice::default().low, 76.0, 30));
        let written = v.to_settings(later);
        assert!(written.contains("\"timbre\"") && written.contains(&format!("\"version\": {VOICE_VERSION}")));
        assert_eq!(Voice::from_settings(&written), Some(v));
        assert_eq!(Voice::from_settings("{}"), None, "no voice heard");
        assert_eq!(Voice::from_settings("not a voice"), None);
    }

    fn heard(meant: f32, sung: f32) -> Heard {
        Heard { meant, sung, settle_ms: 80.0, long: false, vibrato: None }
    }

    #[test]
    fn a_voice_flat_at_the_top_is_learnt_as_flat_there_and_steady_below() {
        // C3 to C4: in tune at the bottom, 30 cents flat at the top, scattering 5 cents either way, and one note missed
        let mut notes = Vec::new();
        for (k, meant) in (48..=60).enumerate() {
            let flat = -0.3 * (meant - 48) as f32 / 12.0;
            let jitter = [0.05, -0.05, 0.0][k % 3];
            notes.push(heard(meant as f32, meant as f32 + flat + jitter));
        }
        notes.push(heard(55.0, 57.0));
        let v = fit(&notes);
        assert_eq!((v.low, v.high, v.notes, v.missed), (48.0, 60.0, 14, 1));
        assert!(v.bias[0].abs() < 0.03 && (v.bias[1] + 0.15).abs() < 0.03 && (v.bias[2] + 0.28).abs() < 0.04, "{v:?}");
        assert!(v.spread.iter().all(|&s| s > 0.03 && s < 0.12), "{v:?}");
        // far likelier to land within 25 cents at the bottom than at the top
        assert!(v.hit(48.0, 25.0) > 0.95 && v.hit(60.0, 25.0) < 0.5, "{} {}", v.hit(48.0, 25.0), v.hit(60.0, 25.0));
        let words = v.describe();
        assert!(
            words.contains("range C3-C4; flat by 15 cents in the middle (F#3), flat by 27 cents at the top (C4)"),
            "{words}"
        );
        assert!(!words.contains("at the bottom"), "{words}");
    }

    #[test]
    fn with_no_notes_a_voice_is_anyones_and_few_notes_lean_on_it() {
        assert_eq!(fit(&[]), Voice::default());
        let one = fit(&[heard(60.0, 60.0)]);
        assert!(one.spread[1] > 0.15, "one note in tune is not proof of a perfect voice: {one:?}");
        let anyone = Voice::default();
        assert!((anyone.hit(60.0, 25.0) - 0.595).abs() < 0.01, "{}", anyone.hit(60.0, 25.0));
        assert!((erf(1.0) - 0.842_700_8).abs() < 1e-6 && (phi(0.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_regular_swing_is_vibrato_and_a_drift_is_not() {
        // 5.5 Hz, 30 cents either way, over a slow drift of a quarter tone
        let swing: Vec<f32> = (0..80)
            .map(|t| {
                let s = t as f32 * 0.01;
                60.0 + 0.25 * s + 0.3 * (std::f32::consts::TAU * 5.5 * s).sin()
            })
            .collect();
        let (hz, depth) = vibrato(&swing).expect("a vibrato");
        assert!((hz - 5.5).abs() < 0.2 && (depth - 0.3).abs() < 0.03, "{hz} {depth}");
        let drift: Vec<f32> = (0..80).map(|t| 60.0 + 0.004 * t as f32).collect();
        assert_eq!(vibrato(&drift), None);
        assert_eq!(vibrato(&swing[..30]), None, "too short to tell");
    }
}
