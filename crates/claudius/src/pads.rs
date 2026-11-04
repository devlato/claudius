//! The song's sounds to play by hand: each drum lane's hit and notes of a sound, rendered through the song's mix and
//! master, so that a phone (or another controller with a player of its own) plays them as the song would.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::model::{Blueprint, Project, TrackMix};
use crate::rng::{Rng, hash_str};
use crate::synth::presets;
use crate::theory::Note;
use crate::{compose, render};

/// the seconds each hit or note is given, its ringing included
pub const SLOT_SECS: f64 = 1.5;
/// how long a note is held
pub const HELD_SECS: f64 = 1.0;
/// the most notes rendered at once
pub const MAX_NOTES: usize = 25;

/// Every drum lane's hit at full strength, as the song mixes and masters it, in the lanes' order. A lane muted, or left
/// out while others are soloed, is heard all the same.
pub fn hits(bp: &Blueprint) -> Result<Vec<(String, Stereo)>, String> {
    let lanes: Vec<(String, i32)> = bp.drums.iter().map(|d| (d.name.clone(), d.pitch)).collect();
    if lanes.is_empty() {
        return Err("the song has no drum lanes".into());
    }
    let mut p = arranged(bp);
    p.tracks.retain(|t| lanes.iter().any(|(n, _)| *n == t.name));
    let slot = beats(&p, SLOT_SECS);
    for t in &mut p.tracks {
        let (i, (_, pitch)) = lanes.iter().enumerate().find(|(_, (n, _))| *n == t.name).expect("a lane");
        t.notes = vec![Note { t: i as f64 * slot, d: 0.25, p: 60 + pitch, v: 1.0, lock: None }];
        t.clips.clear();
    }
    let x = play(p, lanes.len())?;
    Ok(lanes.into_iter().map(|(n, _)| n).zip(x).collect())
}

/// Notes of `sound` (a synth preset or an installed instrument) at MIDI `pitches`, each held `HELD_SECS`, as a part of
/// the song playing it would sound.
pub fn notes(bp: &Blueprint, sound: &str, pitches: &[i32]) -> Result<Vec<Stereo>, String> {
    if presets::get(sound).is_none() && claudius_packs::resolve(sound, None).is_none() {
        return Err(format!("no synth preset or installed instrument is called '{sound}'"));
    }
    if pitches.is_empty() || pitches.len() > MAX_NOTES || pitches.iter().any(|p| !(12..=120).contains(p)) {
        return Err(format!("1 to {MAX_NOTES} notes, from MIDI 12 to 120"));
    }
    let mut p = arranged(bp);
    let Some(mut t) = p.tracks.first().cloned() else { return Err("the song has no tracks".into()) };
    let (slot, held) = (beats(&p, SLOT_SECS), beats(&p, HELD_SECS));
    t.name = "keys".into();
    t.role = "keys".into();
    t.sound = compose::part_sound(sound, &mut Rng::new(hash_str(sound) ^ bp.seed));
    t.notes =
        pitches.iter().enumerate().map(|(i, &p)| Note { t: i as f64 * slot, d: held, p, v: 0.8, lock: None }).collect();
    t.clips.clear();
    t.transpose = 0;
    t.chop.clear();
    t.slice_beats = 0.0;
    t.choke = None;
    t.mix = TrackMix { reverb: 0.15, bus: "music".into(), ..Default::default() };
    p.tracks = vec![t];
    play(p, pitches.len())
}

/// The keys of a phone or a controller: notes of `sound` from MIDI `low` to `high`, every `step` semitones, one after
/// another `SLOT_SECS` apart in one recording.
pub fn keys(bp: &Blueprint, sound: &str, low: i32, high: i32, step: usize) -> Result<Stereo, String> {
    let span = high.checked_sub(low).and_then(|s| usize::try_from(s).ok());
    let fits = (1..=12).contains(&step) && (12..=120).contains(&low) && (12..=120).contains(&high);
    if !fits || span.is_none_or(|s| s / step + 1 > MAX_NOTES) {
        return Err(format!("keys are 1 to {MAX_NOTES} notes from MIDI 12 to 120, 1 to 12 semitones apart"));
    }
    let pitches: Vec<i32> = (low..=high).step_by(step).collect();
    let notes = notes(bp, sound, &pitches)?;
    let slot = (SLOT_SECS * SRF) as usize;
    let mut all = Stereo::zeros(slot * notes.len());
    for (i, n) in notes.iter().enumerate() {
        all.l[i * slot..i * slot + n.len()].copy_from_slice(&n.l);
        all.r[i * slot..i * slot + n.len()].copy_from_slice(&n.r);
    }
    Ok(all)
}

/// the song with every track heard: none muted, none soloed
pub(crate) fn heard(bp: &Blueprint) -> Blueprint {
    let mut bp = bp.clone();
    let mixes = bp.drums.iter_mut().map(|d| &mut d.mix);
    let mixes = mixes.chain(bp.parts.iter_mut().map(|p| &mut p.mix)).chain(bp.loops.iter_mut().map(|l| &mut l.mix));
    for m in mixes {
        (m.mute, m.solo) = (false, false);
    }
    bp
}

/// the song arranged with every track heard, and without its swing and humanizing, so that every slot starts on time
fn arranged(bp: &Blueprint) -> Project {
    let mut p = compose::arrange(&heard(bp));
    (p.swing, p.humanize) = (0.0, 0.0);
    p
}

fn beats(p: &Project, secs: f64) -> f64 {
    secs * p.bpm / 60.0
}

/// `p` rendered over `n` slots and cut into them, each without the silence after it
fn play(p: Project, n: usize) -> Result<Vec<Stereo>, String> {
    let bars = (n as f64 * beats(&p, SLOT_SECS) / 4.0).ceil().max(1.0) as u32;
    let mut p = p.slice(0, bars);
    p.bars = p.bars.max(bars);
    let r =
        render::render(&p, &render::Options { stems: false, tail_secs: SLOT_SECS }).map_err(|e| format!("{e:#}"))?;
    let len = (SLOT_SECS * SRF) as usize;
    Ok((0..n)
        .map(|i| {
            let a = (i * len).min(r.master.len());
            let mut x = r.master.slice(a, (a + len).min(r.master.len()));
            let end = (0..x.len()).rev().find(|&j| x.l[j].abs().max(x.r[j].abs()) > 1e-4).map_or(0, |j| j + 1);
            x = x.slice(0, (end + 480).min(x.len()));
            let n = x.len();
            x.fade(0, 240.min(n));
            x
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::Options;
    use crate::genres;

    fn song() -> Blueprint {
        let g = genres::get("house").unwrap();
        compose::generate(&g, 3, &Options { short: true, samples: 0.0, loops: false, ..Default::default() }, None)
    }

    fn peak(x: &Stereo) -> f32 {
        x.l.iter().chain(&x.r).fold(0.0, |m, s| m.max(s.abs()))
    }

    #[test]
    fn every_lane_has_its_hit_from_its_start() {
        let mut bp = song();
        bp.drums[0].mix.mute = true;
        let hits = hits(&bp).unwrap();
        let lanes: Vec<&str> = bp.drums.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(hits.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), lanes);
        for (lane, x) in &hits {
            assert!(peak(x) > 0.05, "{lane} sounds, muted or not: {}", peak(x));
            assert!(x.secs() <= SLOT_SECS + 1e-9);
            let first = (0..x.len()).find(|&i| x.l[i].abs().max(x.r[i].abs()) > 0.01).unwrap();
            assert!(first < 480, "{lane} strikes from its start, not {first} samples in");
        }
    }

    #[test]
    fn a_sound_plays_its_notes_at_their_pitches() {
        let bp = song();
        let x = notes(&bp, "epiano", &[48, 60, 72]).unwrap();
        assert_eq!(x.len(), 3);
        assert!(x.iter().all(|n| peak(n) > 0.05));
        for (n, want) in x.iter().zip([48.0, 60.0, 72.0]) {
            let heard: Vec<f32> = crate::analysis::melody::pitches(n).into_iter().flatten().collect();
            let mid = heard[heard.len() / 2];
            assert!((mid - want).abs() < 0.5, "MIDI {want} sounds as {mid}");
        }
        assert!(notes(&bp, "kazoo9", &[60]).unwrap_err().contains("no synth preset"));
        assert!(notes(&bp, "epiano", &[]).is_err());
    }

    #[test]
    fn keys_lay_their_notes_a_slot_apart_and_stay_in_bounds() {
        let bp = song();
        let k = keys(&bp, "epiano", 59, 65, 3).unwrap();
        assert!((k.secs() - 3.0 * SLOT_SECS).abs() < 1e-6, "three notes, a slot each: {} s", k.secs());
        for (low, high, step) in [(i32::MIN, i32::MAX, 3), (60, 59, 3), (12, 120, 1), (60, 72, 0), (0, 12, 3)] {
            assert!(keys(&bp, "epiano", low, high, step).is_err(), "{low} {high} {step}");
        }
    }
}
