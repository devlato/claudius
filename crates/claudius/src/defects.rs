//! Defects a recording carries whatever its genre, heard on its master alone (`judge`'s defect checks): a recording
//! driven into clipping piles its samples up at its ceiling, and one cut to silence and back over and over has gaps in
//! it. Each measure here is what the critic weighs; how much it weighs is `judge`'s.

use crate::audio::Stereo;
use crate::config::SRF;

/// The share of a recording's samples, both channels, within a tenth of a decibel of its loudest: a recording driven
/// into a hard ceiling sits there for much of its length, where a mastered one only touches its ceiling now and then.
/// None for silence.
pub fn clipped_share(x: &Stereo) -> Option<f64> {
    let peak = x.l.iter().chain(&x.r).fold(0.0f32, |m, v| m.max(v.abs()));
    if peak < 1e-4 {
        return None;
    }
    let near = peak * 10f32.powf(-0.1 / 20.0);
    let n = x.l.len() + x.r.len();
    let top = x.l.iter().chain(&x.r).filter(|v| v.abs() >= near).count();
    Some(top as f64 / n as f64)
}

/// the frames the dropouts are found in, in seconds
const FRAME_S: f64 = 0.010;

/// How many times a minute the sound is cut to silence and back: a stretch of 80 ms or more, 40 dB or more under the
/// recording's own level (the median of its frames that sound), with the music within 10 dB of that level in the 50 ms
/// on either side of it. A song stops now and then, and a phrase rests; a recording cut over and over has gaps in it.
/// The first and last second are not counted (a song starts and ends), and a recording under three seconds is too
/// short to tell (none).
pub fn dropouts_per_minute(x: &Stereo) -> Option<f64> {
    let frame = (FRAME_S * SRF) as usize;
    let mono = x.mono();
    let db: Vec<f64> = mono
        .chunks(frame)
        .map(|c| 10.0 * (c.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / c.len() as f64 + 1e-20).log10())
        .collect();
    let edge = (1.0 / FRAME_S) as usize;
    if db.len() < 3 * edge {
        return None;
    }
    let mut sounding: Vec<f64> = db.iter().copied().filter(|d| *d > -70.0).collect();
    if sounding.is_empty() {
        return None;
    }
    sounding.sort_by(f64::total_cmp);
    let level = sounding[sounding.len() / 2];
    let (quiet, full) = (level - 40.0, level - 10.0);
    let (shortest, side) = ((0.080 / FRAME_S) as usize, (0.050 / FRAME_S) as usize);
    let (mut count, mut i) = (0usize, edge);
    while i < db.len() - edge {
        if db[i] >= quiet {
            i += 1;
            continue;
        }
        let start = i;
        while i < db.len() && db[i] < quiet {
            i += 1;
        }
        if i - start >= shortest && i < db.len() - edge {
            let before = db[start.saturating_sub(side)..start].iter().any(|d| *d >= full);
            let after = db[i..(i + side).min(db.len())].iter().any(|d| *d >= full);
            count += usize::from(before && after);
        }
    }
    Some(count as f64 / (db.len() as f64 * FRAME_S / 60.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(x: Vec<f32>) -> Stereo {
        Stereo { l: x.clone(), r: x }
    }

    /// `secs` of a chord with overtones, as instruments' are, under a slow swell
    fn music(secs: f64) -> Vec<f32> {
        (0..(secs * SRF) as usize)
            .map(|i| {
                let t = i as f64 / SRF;
                let note = |hz: f64| {
                    (1..=5).map(|h| (std::f64::consts::TAU * hz * h as f64 * t).sin() / h as f64).sum::<f64>()
                };
                (0.15 * (0.8 + 0.2 * (0.3 * t).sin()) * (note(220.0) + note(277.18) + note(329.63))) as f32
            })
            .collect()
    }

    #[test]
    fn a_recording_driven_into_its_ceiling_sits_there_and_a_clean_one_only_touches_it() {
        let clean = music(6.0);
        let peak = clean.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let clipped: Vec<f32> = clean.iter().map(|v| (v * 5.6 / peak).clamp(-1.0, 1.0) * 0.4).collect();
        let (c, k) = (clipped_share(&stereo(clean)).unwrap(), clipped_share(&stereo(clipped)).unwrap());
        assert!(c < 0.01 && k > 0.2, "clean {c}, clipped {k}");
        assert_eq!(clipped_share(&stereo(vec![0.0; 4_800])), None, "silence is not judged");
    }

    #[test]
    fn gaps_every_two_seconds_are_dropouts_and_a_song_that_stops_once_has_one() {
        let x = music(30.0);
        let gapped: Vec<f32> =
            x.iter().enumerate().map(|(i, v)| if (i as f64 / SRF) % 2.0 >= 1.75 { 0.0 } else { *v }).collect();
        let stops_once: Vec<f32> = x
            .iter()
            .enumerate()
            .map(|(i, v)| if (14.0..14.5).contains(&(i as f64 / SRF)) { 0.0 } else { *v })
            .collect();
        let gapped = dropouts_per_minute(&stereo(gapped)).unwrap();
        let once = dropouts_per_minute(&stereo(stops_once)).unwrap();
        let none = dropouts_per_minute(&stereo(x)).unwrap();
        assert!(gapped > 20.0, "gapped {gapped} a minute");
        assert!((once - 2.0).abs() < 1e-9, "one stop in half a minute: {once}");
        assert_eq!(none, 0.0);
        assert_eq!(dropouts_per_minute(&stereo(vec![0.1; 4_800])), None, "too short");
    }
}
