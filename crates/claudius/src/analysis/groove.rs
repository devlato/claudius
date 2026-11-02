//! A groove heard in a recording: where each sixteenth of the bar lands against the grid and how hard. The song
//! analysis finds the tempo and the first downbeat of a constant grid (music made in a DAW keeps one tempo); every
//! onset is set against the sixteenth nearest it, and each of the sixteen positions of the bar takes the median of
//! how far its onsets fall from it, and their mean loudness against the loop's. The beats themselves are put back
//! on the grid, so that a downbeat found a little early or late does not move the whole groove.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::theory::rhythm::Groove;

/// a groove heard, and what it was heard in: the tempo and the onsets on the grid
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub groove: Groove,
    pub bpm: f64,
    pub onsets: usize,
}

/// The groove of the drums in `x`, or why there is none to hear (too few onsets on the grid).
pub fn hear(x: &Stereo) -> Result<Heard, String> {
    let mono = x.mono();
    let a = super::song::analyze(x, None);
    let spb = 60.0 / a.bpm.max(1.0);
    let onsets = super::onsets::detect(&mono, 0.06, 0.03);
    let win = (0.02 * SRF) as usize;
    let mut at: Vec<Vec<(f32, f32)>> = vec![Vec::new(); 16];
    let mut count = 0;
    for &s in &onsets {
        let b = (s as f64 / SRF - a.first_downbeat) / spb;
        if b < -0.125 {
            continue;
        }
        let k = (b * 4.0).round();
        let off = (b - k / 4.0) as f32;
        let loud = mono[s..(s + win).min(mono.len())].iter().map(|v| v * v).sum::<f32>().sqrt();
        at[(k as i64).rem_euclid(16) as usize].push((off, loud));
        count += 1;
    }
    if count < 8 {
        return Err(format!("too few drum hits to hear a groove in ({count}): it wants a beat, a few bars of it"));
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
    Ok(Heard { groove: Groove { timing, accent }, bpm: a.bpm, onsets: count })
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
        let h = hear(&clicks(0.06)).unwrap();
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
        let e = hear(&Stereo::zeros(48_000 * 4)).unwrap_err();
        assert!(e.starts_with("too few drum hits"), "{e}");
    }
}
