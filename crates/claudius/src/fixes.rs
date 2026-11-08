//! Quick fixes: the critic's issues (`judge`) that one change fixes outright, without searching variations for it. A
//! recording that keeps a tempo of its own is stretched from the tempo it keeps to the song's; a track mixed far from
//! where its genre has it moves there; a band of the master too faint or too strong gets an equalizer's bell or shelf
//! on it. `of` lists the fixes a verdict's issues have, `apply` makes those asked for, as one edit of the song.

use std::collections::BTreeMap;

use crate::dsp::filter::BiquadKind;
use crate::fx::{Effect, EqBand};
use crate::model::{Blueprint, Sound};

/// a fix of the issue at `target` (where the verdict says its fix lives), and what it does, in words
#[derive(Clone, Debug, PartialEq)]
pub struct QuickFix {
    pub target: String,
    pub label: String,
    edit: Edit,
}

#[derive(Clone, Debug, PartialEq)]
enum Edit {
    /// the tempo the recording on loop `track` keeps, which it is stretched from to the song's
    Tempo { track: String, bpm: f64 },
    /// the track's level, in dB from where its role is mixed
    Level { track: String, db: f32 },
    /// a band of the master moved by `gain_db`, with a bell or (at the ends) a shelf at `freq`
    Band { kind: BiquadKind, freq: f32, gain_db: f32 },
}

/// the most a fix moves a track's level, or a band of the master, at once
const MOST_DB: f64 = 6.0;

/// where the equalizer moves each band of the verdict (`judge::BANDS`): a shelf under the sub and over the air, a bell
/// in the middle of the others, and the band in words
fn band_eq(name: &str) -> Option<(BiquadKind, f32, &'static str)> {
    Some(match name {
        "sub" => (BiquadKind::Lowshelf, 60.0, "sub, under 60 Hz"),
        "low" => (BiquadKind::Peak, 120.0, "lows, 60-250 Hz"),
        "lowmid" => (BiquadKind::Peak, 500.0, "low mids, 250 Hz-1 kHz"),
        "mid" => (BiquadKind::Peak, 2000.0, "mids, 1-4 kHz"),
        "presence" => (BiquadKind::Peak, 6000.0, "presence, 4-8 kHz"),
        "air" => (BiquadKind::Highshelf, 10000.0, "air, over 8 kHz"),
        _ => return None,
    })
}

/// The quick fixes the issues at `targets` have, by the verdict's `measures`, for the song `bp` as it was judged: one
/// for each target that has one, in their order.
pub fn of(targets: &[&str], measures: &BTreeMap<String, f64>, bp: &Blueprint) -> Vec<QuickFix> {
    let mut out: Vec<QuickFix> = Vec::new();
    for &target in targets {
        if out.iter().any(|f| f.target == target) {
            continue;
        }
        let fix = if let Some(track) = target.strip_prefix("time:") {
            tempo(track, measures, bp)
        } else if let Some(track) = target.strip_prefix("level:") {
            level(track.trim_end_matches(['+', '-']), measures, bp)
        } else if let Some(name) = target.strip_prefix("band:") {
            band(name.trim_end_matches(['+', '-']), measures)
        } else {
            None
        };
        if let Some((label, edit)) = fix {
            out.push(QuickFix { target: target.to_string(), label, edit });
        }
    }
    out
}

/// A loop stretched from the tempo it is said to keep to the song's plays at the song's times the tempo its recording
/// keeps over the one it is said to; heard at `kept`, it keeps `said * kept / song`, which it is then said to keep.
fn tempo(track: &str, m: &BTreeMap<String, f64>, bp: &Blueprint) -> Option<(String, Edit)> {
    let kept = *m.get(&format!("time_{track}_own_bpm"))?;
    let Sound::Loop { bpm: said, .. } = &bp.loops.iter().find(|l| l.name == track)?.sound else { return None };
    let own = said * kept / bp.bpm;
    if !own.is_finite() || !(20.0..=400.0).contains(&own) || (own / said - 1.0).abs() < 0.002 {
        return None;
    }
    let label = format!(
        "'{track}' stretched from the {own:.1} BPM its recording keeps, not the {said:.1} it was said to, to the \
         song's {:.0}",
        bp.bpm
    );
    Some((label, Edit::Tempo { track: track.to_string(), bpm: own }))
}

/// A track mixed further from its role's level than its genre has it moves there, by half decibels.
fn level(track: &str, m: &BTreeMap<String, f64>, bp: &Blueprint) -> Option<(String, Edit)> {
    let off = *m.get(&format!("level_{track}_off_db"))?;
    let now = mix_level(bp, track)?;
    let by = ((-off).clamp(-MOST_DB, MOST_DB) * 2.0).round() / 2.0;
    if by.abs() < 0.5 {
        return None;
    }
    let all = (by + off).abs() < 0.26;
    let label = format!(
        "'{track}' {:.1} dB {}, {} the level its genre mixes it at",
        by.abs(),
        if by > 0.0 { "up" } else { "down" },
        if all { "to" } else { "toward" }
    );
    Some((label, Edit::Level { track: track.to_string(), db: (f64::from(now) + by) as f32 }))
}

/// the level of the track called `name`, in dB from where its role is mixed
fn mix_level(bp: &Blueprint, name: &str) -> Option<f32> {
    let mut bp = bp.clone();
    crate::script::mix_of(&mut bp, name).map(|m| m.level_db)
}

/// A band of the master out of its range moves into it: by how far out it is and half a decibel more.
fn band(name: &str, m: &BTreeMap<String, f64>) -> Option<(String, Edit)> {
    let off = *m.get(&format!("band_{name}_off_db"))?;
    let (kind, freq, words) = band_eq(name)?;
    let gain = -off.signum() * (off.abs() + 0.5).min(MOST_DB);
    let eq = if kind == BiquadKind::Peak { "a bell" } else { "a shelf" };
    let at = if freq >= 1000.0 { format!("{} kHz", freq / 1000.0) } else { format!("{freq} Hz") };
    let label = format!(
        "the master's {words} {gain:+.1} dB, {eq} at {at} on its equalizer",
        gain = (gain * 10.0).round() / 10.0
    );
    Some((label, Edit::Band { kind, freq, gain_db: gain as f32 }))
}

/// Makes `fixes` (some of those `of` listed for the song as it is) to it; what was done, in words.
pub fn apply(bp: &mut Blueprint, fixes: &[QuickFix]) -> Result<String, String> {
    if fixes.is_empty() {
        return Err("nothing to fix that way: no issue of the verdict asked for has a quick fix".into());
    }
    for f in fixes {
        match &f.edit {
            Edit::Tempo { track, bpm } => {
                let gone = || format!("the song has no loop '{track}' now");
                let l = bp.loops.iter_mut().find(|l| l.name == *track).ok_or_else(gone)?;
                if let Sound::Loop { bpm: said, .. } = &mut l.sound {
                    *said = *bpm;
                }
            }
            Edit::Level { track, db } => {
                crate::script::mix_of(bp, track)
                    .ok_or_else(|| format!("the song has no track '{track}' now"))?
                    .level_db = *db;
            }
            Edit::Band { kind, freq, gain_db } => master_eq(bp, *kind, *freq, *gain_db),
        }
    }
    let labels: Vec<&str> = fixes.iter().map(|f| f.label.as_str()).collect();
    Ok(format!("quick fix{}: {}", if fixes.len() == 1 { "" } else { "es" }, labels.join("; ")))
}

/// the master's band at `freq` moved by `gain_db` on the equalizer at the end of its chain, which a fix adds when its
/// chain ends otherwise; a band moved again moves further, to 8 dB either way at most
fn master_eq(bp: &mut Blueprint, kind: BiquadKind, freq: f32, gain_db: f32) {
    let fx = &mut bp.mix.master.fx;
    if !matches!(fx.last(), Some(Effect::Eq { .. })) {
        fx.push(Effect::Eq { bands: Vec::new() });
    }
    let Some(Effect::Eq { bands }) = fx.last_mut() else { return };
    match bands.iter_mut().find(|b| b.kind == kind && b.freq == freq) {
        Some(b) => b.gain_db = (b.gain_db + gain_db).clamp(-8.0, 8.0),
        None => bands.push(EqBand { kind, freq, gain_db, q: if kind == BiquadKind::Peak { 0.9 } else { 0.7 } }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LoopPart, TrackMix};

    /// a short house song with a loop 'sample', said to keep the song's 145 BPM
    fn song() -> Blueprint {
        let mut bp = crate::script::blank("house").unwrap();
        bp.bpm = 145.0;
        bp.loops.push(LoopPart {
            name: "sample".into(),
            role: "sample".into(),
            sound: Sound::Loop {
                path: "/x/sample.wav".into(),
                bpm: 145.0,
                beats: 16.0,
                key: None,
                tonal: false,
                label: String::new(),
            },
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix: TrackMix { level_db: -2.0, ..TrackMix::default() },
            from: None,
            sampled: true,
        });
        bp
    }

    fn measures(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn a_loop_heard_off_the_songs_tempo_is_stretched_from_the_tempo_it_keeps() {
        let mut bp = song();
        let m = measures(&[("time_sample_own_bpm", 140.0)]);
        let fixes = of(&["time:sample"], &m, &bp);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].label.contains("from the 140.0 BPM its recording keeps, not the 145.0"), "{}", fixes[0].label);
        apply(&mut bp, &fixes).unwrap();
        let Sound::Loop { bpm, beats, .. } = &bp.loops[0].sound else { unreachable!() };
        assert!((bpm - 140.0).abs() < 1e-9 && *beats == 16.0, "its length in beats stays: {bpm} {beats}");
        // said to keep 120 and heard at 140 against 145, its recording keeps 120 * 140 / 145
        if let Sound::Loop { bpm, .. } = &mut bp.loops[0].sound {
            *bpm = 120.0;
        }
        let fixes = of(&["time:sample"], &m, &bp);
        let Edit::Tempo { bpm, .. } = &fixes[0].edit else { unreachable!() };
        assert!((bpm - 120.0 * 140.0 / 145.0).abs() < 1e-9);
        // heard at the song's own tempo, there is nothing to fix
        assert!(of(&["time:sample"], &measures(&[("time_sample_own_bpm", 145.0)]), &bp).is_empty());
    }

    #[test]
    fn a_track_mixed_far_from_its_level_moves_there_and_a_band_out_of_its_range_into_it() {
        let mut bp = song();
        let m = measures(&[("level_sample_off_db", -4.2), ("band_presence_off_db", -3.0), ("band_sub_off_db", 1.0)]);
        let fixes = of(&["level:sample-", "band:presence-", "band:sub+", "genre", "level:sample-"], &m, &bp);
        let targets: Vec<&str> = fixes.iter().map(|f| f.target.as_str()).collect();
        assert_eq!(targets, ["level:sample-", "band:presence-", "band:sub+"], "one each, none for the genre");
        assert_eq!(fixes[0].label, "'sample' 4.0 dB up, to the level its genre mixes it at");
        let far = of(&["level:sample+"], &measures(&[("level_sample_off_db", 9.0)]), &bp);
        assert_eq!(far[0].label, "'sample' 6.0 dB down, toward the level its genre mixes it at", "6 dB at once");
        assert_eq!(fixes[1].label, "the master's presence, 4-8 kHz +3.5 dB, a bell at 6 kHz on its equalizer");
        let said = apply(&mut bp, &fixes).unwrap();
        assert!(said.starts_with("quick fixes: 'sample' 4.0 dB up"), "{said}");
        assert_eq!(bp.loops[0].mix.level_db, 2.0);
        let Some(Effect::Eq { bands }) = bp.mix.master.fx.last() else { panic!("{:?}", bp.mix.master.fx) };
        assert_eq!(bands.len(), 2);
        assert!(bands.iter().any(|b| b.kind == BiquadKind::Peak && b.freq == 6000.0 && b.gain_db == 3.5));
        assert!(bands.iter().any(|b| b.kind == BiquadKind::Lowshelf && b.gain_db == -1.5));
        // fixed again, the same band moves further on the same equalizer
        apply(&mut bp, &fixes[1..2]).unwrap();
        let Some(Effect::Eq { bands }) = bp.mix.master.fx.last() else { unreachable!() };
        assert!(bands.iter().any(|b| b.freq == 6000.0 && b.gain_db == 7.0) && bands.len() == 2);
        assert!(apply(&mut bp, &[]).unwrap_err().starts_with("nothing to fix"));
    }
}
