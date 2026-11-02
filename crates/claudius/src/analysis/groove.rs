//! A groove heard in a recording: where each sixteenth of the bar lands against the grid and how hard.
//!
//! The grid is the drums' own. A tempo that is known (given, or written in the file's name, as "92bpm") is only
//! tuned. Otherwise the song analysis guesses one, which on a short loop is often off by 3:4 or 4:3 or a few
//! percent, and a groove measured against the wrong sixteenths means nothing; so the onsets choose. The tempos tried
//! are the guess and its octaves and, for a short loop, the whole bars it lasts, which give its tempo exactly; then
//! the ratios a guess is off by, which win only when they fit clearly better. Each is tuned to a hundredth of a
//! percent to where the hits sit nearest its sixteenths, a swing allowed, and the slowest that fits about as well as
//! the best is the tempo (a grid twice as fine fits at least as well). The beats are the sixteenths the loud hits
//! fall on, and the downbeat the beat with the most kick under it.
//!
//! Every onset is then set against the sixteenth nearest it, and each of the sixteen positions of the bar takes the
//! median of how far its onsets fall from it, and their mean loudness against the loop's. The beats themselves are
//! put back on the grid, so that a downbeat found a little early or late does not move the whole groove.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::dsp::filter::{BiquadKind, biquad};
use crate::theory::rhythm::Groove;

/// a groove heard, and what it was heard in: the tempo and the onsets on the grid
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub groove: Groove,
    pub bpm: f64,
    pub onsets: usize,
}

/// an onset: when (seconds), how loud, and how much kick is in it (its level under 120 Hz)
struct Onset {
    t: f64,
    loud: f32,
    low: f32,
}

/// The groove of the drums in `x` (at `bpm` when the tempo is known), or why there is none to hear (too few onsets).
pub fn hear(x: &Stereo, bpm: Option<f64>) -> Result<Heard, String> {
    let mono = x.mono();
    let mut low = mono.clone();
    biquad(&mut low, BiquadKind::Lowpass, 120.0, SRF, std::f64::consts::FRAC_1_SQRT_2, 0.0);
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt();
    let (win, kick) = ((0.02 * SRF) as usize, (0.04 * SRF) as usize);
    let hits: Vec<Onset> = super::onsets::detect(&mono, 0.06, 0.03)
        .into_iter()
        .map(|s| Onset {
            t: s as f64 / SRF,
            loud: rms(&mono[s..(s + win).min(mono.len())]),
            low: rms(&low[s..(s + kick).min(low.len())]),
        })
        .collect();
    if hits.len() < 8 {
        return Err(format!(
            "too few drum hits to hear a groove in ({}): it wants a beat, a few bars of it",
            hits.len()
        ));
    }
    let bpm = match bpm {
        Some(b) => tune(&hits, b, 0.03).0,
        None => estimate(&hits, mono.len() as f64 / SRF, super::song::analyze(x, None).bpm),
    };
    let downbeat = downbeat(&hits, bpm);
    let spb = 60.0 / bpm;
    let mut at: Vec<Vec<(f32, f32)>> = vec![Vec::new(); 16];
    for h in &hits {
        let b = (h.t - downbeat) / spb;
        let k = (b * 4.0).round();
        at[(k as i64).rem_euclid(16) as usize].push(((b - k / 4.0) as f32, h.loud));
    }
    let median = |v: &mut Vec<f32>| {
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    let mut timing = [0f32; 16];
    let mut loud = [0f32; 16];
    for i in 0..16 {
        if at[i].len() >= 2 {
            timing[i] = median(&mut at[i].iter().map(|x| x.0).collect());
        }
        if !at[i].is_empty() {
            loud[i] = at[i].iter().map(|x| x.1).sum::<f32>() / at[i].len() as f32;
        }
    }
    // the beats back on the grid
    let beats: Vec<f32> = [0, 4, 8, 12].iter().filter(|&&i| at[i].len() >= 2).map(|&i| timing[i]).collect();
    let shift = if beats.is_empty() { 0.0 } else { beats.iter().sum::<f32>() / beats.len() as f32 };
    for (i, t) in timing.iter_mut().enumerate() {
        if at[i].len() >= 2 {
            *t = (*t - shift).clamp(-0.12, 0.12);
        }
    }
    let heard: Vec<f32> = loud.iter().copied().filter(|&l| l > 0.0).collect();
    let mean = heard.iter().sum::<f32>() / heard.len().max(1) as f32;
    let accent = std::array::from_fn(|i| if loud[i] > 0.0 { (loud[i] / mean.max(1e-9)).clamp(0.4, 1.3) } else { 1.0 });
    Ok(Heard { groove: Groove { timing, accent }, bpm, onsets: hits.len() })
}

/// How near the hits sit to the sixteenths of the grid at `bpm`, a swing allowed: the mean squared distance, in
/// sixteenths, of each from its nearest, the on-beat and the off-beat sixteenths each measured from where they
/// land on average (weighted by how loud each is), and the time of a sixteenth (the grid's phase, where the hits'
/// distances balance out). A swung loop fits its own grid as well as the grid of triplets at half as fast again;
/// triplets fit only theirs, as they fall on both sides of the sixteenths.
fn fit(hits: &[Onset], bpm: f64) -> (f64, f64) {
    let six = 15.0 / bpm;
    let w = std::f64::consts::TAU / six;
    let (re, im) = hits
        .iter()
        .fold((0.0, 0.0), |(re, im), h| (re + h.loud as f64 * (w * h.t).cos(), im + h.loud as f64 * (w * h.t).sin()));
    let at = im.atan2(re).rem_euclid(std::f64::consts::TAU) / w;
    let off = |h: &Onset| {
        let d = (h.t - at) / six;
        ((d.round() as i64).rem_euclid(2) as usize, d - d.round())
    };
    let (mut sum, mut sq, mut total) = ([0.0; 2], [0.0; 2], [0.0; 2]);
    for h in hits {
        let (odd, d) = off(h);
        sum[odd] += h.loud as f64 * d;
        sq[odd] += h.loud as f64 * d * d;
        total[odd] += h.loud as f64;
    }
    // the squared distances around each kind's mean
    let spread = (0..2).map(|i| sq[i] - sum[i] * sum[i] / total[i].max(1e-12)).sum::<f64>();
    (spread / (total[0] + total[1]).max(1e-12), at)
}

/// the tempo within `span` (a fraction) of `bpm` whose sixteenths the hits sit nearest, in steps of a hundredth of
/// a percent, and how near
fn tune(hits: &[Onset], bpm: f64, span: f64) -> (f64, f64) {
    let n = (span / 0.0001).round() as i64;
    (-n..=n)
        .map(|i| bpm * (1.0 + i as f64 * 0.0001))
        .map(|b| (b, fit(hits, b).0))
        .fold((bpm, f64::MAX), |best, c| if c.1 < best.1 { c } else { best })
}

/// a downbeat for the grid at `bpm`: the beats are the sixteenths (of four) the loud hits fall on, and the
/// downbeat the beat with the most kick under it
fn downbeat(hits: &[Onset], bpm: f64) -> f64 {
    let (_, at) = fit(hits, bpm);
    let six = 15.0 / bpm;
    let (mut loud, mut kick) = ([0f64; 4], [0f64; 16]);
    for h in hits {
        let k = (((h.t - at) / six).round() as i64).rem_euclid(16) as usize;
        loud[k % 4] += (h.loud * h.loud) as f64;
        kick[k] += (h.low * h.low) as f64;
    }
    let beat = (0..4).fold(0, |best, j| if loud[j] > loud[best] { j } else { best });
    let one = (0..4).map(|j| beat + 4 * j).fold(beat, |best, k| if kick[k] > kick[best] { k } else { best });
    at + one as f64 * six
}

/// The tempo of the hits, unknown (the module's doc says how it is found); `guess` is the song analysis'.
fn estimate(hits: &[Onset], dur: f64, guess: f64) -> f64 {
    let range = 55.0..=200.0;
    // a loop lasts whole bars (half a bar, one, two, four ...); three, six or twelve are less likely
    let (mut likely, mut less): (Vec<(f64, f64)>, Vec<(f64, f64)>) = (Vec::new(), Vec::new());
    let mut bars = Vec::new();
    if dur <= 40.0 {
        bars.extend([2.0, 4.0, 8.0, 16.0, 32.0, 64.0].map(|n| 60.0 * n / dur));
        less.extend([12.0, 24.0, 48.0].map(|n| (60.0 * n / dur, 0.003)));
    }
    likely.extend(bars.iter().map(|&b| (b, 0.003)));
    // over a bar or two, a guess tuned freely fits anything near: one near a loop's length is that length
    let near = |b: f64| {
        dur <= 12.0
            && bars.iter().any(|&l| {
                let r = b / l;
                (r / 2f64.powi(r.log2().round() as i32) - 1.0).abs() < 0.03
            })
    };
    likely.extend([1.0, 0.5, 2.0].map(|r| (guess * r, 0.025)).into_iter().filter(|t| !near(t.0)));
    less.extend([0.75, 4.0 / 3.0, 2.0 / 3.0, 1.5].map(|r| (guess * r, 0.025)).into_iter().filter(|t| !near(t.0)));
    // of each, the slowest that fits about as well as the best
    let pick = |tried: Vec<(f64, f64)>| {
        let mut tuned: Vec<(f64, f64)> =
            tried.into_iter().filter(|(b, _)| range.contains(b)).map(|(b, span)| tune(hits, b, span)).collect();
        tuned.sort_by(|a, b| a.0.total_cmp(&b.0));
        let best = tuned.iter().map(|t| t.1).fold(f64::MAX, f64::min);
        tuned.into_iter().find(|t| t.1 <= best + 0.006)
    };
    match (pick(likely), pick(less)) {
        (Some(l), Some(o)) if o.1 < 0.5 * l.1 - 0.002 => o.0,
        (Some(l), _) => l.0,
        (None, o) => o.map_or(guess, |o| o.0),
    }
}

/// A tempo written in a file's name, as "Funk Break 92bpm.wav", "loop_85_BPM" or "BPM-120 kit": a number from 40
/// to 250 next to "bpm".
pub fn tempo_in_name(name: &str) -> Option<f64> {
    let s = name.to_ascii_lowercase();
    let sep = |c: char| c == ' ' || c == '_' || c == '-' || c == '.';
    let number = |t: &str| t.parse::<f64>().ok().filter(|b| (40.0..=250.0).contains(b));
    s.match_indices("bpm").find_map(|(i, _)| {
        let before = s[..i].trim_end_matches(sep);
        let digits = before.len() - before.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.').len();
        let after = s[i + 3..].trim_start_matches(sep);
        let lead = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').len();
        number(before[before.len() - digits..].trim_matches('.')).or_else(|| number(after[..lead].trim_matches('.')))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// eight bars at 100 BPM of a click on every sixteenth: the off-beat sixteenths `late` beats behind, the
    /// downbeats twice as loud
    fn clicks(late: f64) -> Stereo {
        let spb = 0.6;
        let n = (8.0 * 4.0 * spb * SRF) as usize + 4_800;
        let mut l = vec![0f32; n];
        for k in 0..8 * 16 {
            let b = k as f64 / 4.0 + if k % 2 == 1 { late } else { 0.0 };
            let at = (0.1 * SRF + b * spb * SRF) as usize;
            let gain = if k % 16 == 0 {
                0.9
            } else if k % 4 == 0 {
                0.6
            } else {
                0.4
            };
            for i in 0..240.min(n - at) {
                let env = (-(i as f32) / 40.0).exp();
                l[at + i] += gain * env * if i % 2 == 0 { 1.0 } else { -1.0 };
            }
        }
        Stereo { l: l.clone(), r: l }
    }

    #[test]
    fn a_swung_loop_is_heard_as_its_swing_and_its_accents() {
        let h = hear(&clicks(0.06), None).unwrap();
        assert!((h.bpm - 100.0).abs() < 1.0, "{}", h.bpm);
        for i in [1, 3, 9, 15] {
            assert!((h.groove.timing[i] - 0.06).abs() < 0.012, "{i}: {:?}", h.groove.timing);
        }
        for i in [0, 2, 4, 8] {
            assert!(h.groove.timing[i].abs() < 0.012, "{i}: {:?}", h.groove.timing);
        }
        assert!(
            h.groove.accent[0] > h.groove.accent[4] && h.groove.accent[4] > h.groove.accent[1],
            "{:?}",
            h.groove.accent
        );
        // 0.06 beats late of the sixteenth is 62% swing
        assert!(h.groove.describe().starts_with("the off-beat sixteenths at 62% swing"), "{}", h.groove.describe());
    }

    #[test]
    fn silence_has_no_groove() {
        let e = hear(&Stereo::zeros(48_000 * 4), None).unwrap_err();
        assert!(e.starts_with("too few drum hits"), "{e}");
    }

    #[test]
    fn a_known_tempo_is_tuned_and_keeps_its_octave() {
        let x = clicks(0.06);
        let h = hear(&x, Some(101.5)).unwrap();
        assert!((h.bpm - 100.0).abs() < 0.05, "{}", h.bpm);
        assert!((h.groove.timing[1] - 0.06).abs() < 0.012, "{:?}", h.groove.timing);
        let h = hear(&x, Some(200.0)).unwrap();
        assert!((h.bpm / 200.0 - 1.0).abs() < 0.01, "the octave it is given: {}", h.bpm);
    }

    #[test]
    fn a_tempo_is_read_in_a_files_name() {
        for (name, bpm) in [
            ("Funk Break 92bpm.wav", Some(92.0)),
            ("loop_85_BPM.aif", Some(85.0)),
            ("BPM-120 kit.wav", Some(120.0)),
            ("Lazy (87.5 bpm).flac", Some(87.5)),
            ("break 2.wav", None),
            ("1999 bpm.wav", None),
        ] {
            assert_eq!(tempo_in_name(name), bpm, "{name}");
        }
    }
}
