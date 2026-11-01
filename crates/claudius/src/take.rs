//! Takes: recordings made for a song (a vocal, a guitar, a melody sung into the microphone). A take is a loop part
//! whose `from` names the section it starts in: it plays once from there, and on through the later sections that play
//! it, at the tempo it was recorded at (warped, as a loop is, when the song's tempo changes). Its file is FLAC in the
//! song folder's `takes/`, or, until the song is saved, in `takes/` in the songs folder (~/Music/claudius).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::audio::Stereo;
use crate::config;
use crate::model::{Blueprint, LoopPart, Sound, TrackMix};

/// what a take can be in the mix: its role sets how loud the mixer makes it
pub const ROLES: &[&str] = &["vocal", "lead", "chords", "keys", "bass", "drums", "texture"];

/// the folder a song's takes go in: its own folder's `takes/`, or the songs folder's until it has one
pub fn dir(folder: Option<&Path>) -> PathBuf {
    folder.map_or_else(config::songs_dir, Path::to_path_buf).join("takes")
}

/// writes a recording into `dir` as 24-bit FLAC (mono when both sides are the same), named after its sound, so that
/// the same take is one file however often it is stored
pub fn store(audio: &Stereo, dir: &Path) -> Result<PathBuf> {
    let mono = audio.l == audio.r;
    let samples = if mono { audio.l.clone() } else { audio.interleaved() };
    let bytes: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    let path = dir.join(format!("{}.flac", &blake3::hash(&bytes).to_hex()[..16]));
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("cannot write {}", dir.display()))?;
    let spec = claudius_encode::Spec { format: claudius_encode::Format::Flac(claudius_encode::Depth::I24), rate: None };
    let pcm = claudius_encode::Pcm { samples: &samples, channels: if mono { 1 } else { 2 }, rate: config::SR };
    let tags =
        claudius_encode::Tags { software: format!("claudius {}", env!("CARGO_PKG_VERSION")), ..Default::default() };
    let flac = claudius_encode::encode(&spec, pcm, &tags)?;
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, flac).with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

/// Adds the recording at `path`, `secs` long, as a take that starts with section `from` and goes on through the
/// sections after it for as long as it lasts. Without a `name` it is take1, take2, ...; without a `role`, a lead.
/// Returns what was added, in words.
pub fn add(
    bp: &mut Blueprint,
    path: &Path,
    secs: f64,
    from: &str,
    name: Option<&str>,
    role: Option<&str>,
) -> Result<String, String> {
    let Some(first) = bp.form.iter().position(|s| s.name == from) else {
        let names: Vec<&str> = bp.form.iter().map(|s| s.name.as_str()).collect();
        return Err(format!("no section '{from}' (sections: {})", names.join(" ")));
    };
    let role = role.unwrap_or("lead");
    if !ROLES.contains(&role) {
        return Err(format!("a take is one of {}, not '{role}'", ROLES.join(" ")));
    }
    if secs <= 0.0 {
        return Err("the recording is empty".into());
    }
    let taken = bp.layer_names();
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) if !n.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') => {
            return Err(format!("'{n}' cannot name a track: use letters, digits, - and _"));
        }
        Some(n) if taken.iter().any(|t| t == n) => return Err(format!("the song already has a track called '{n}'")),
        Some(n) => n.to_string(),
        None => (1..).map(|k| format!("take{k}")).find(|n| !taken.contains(n)).unwrap_or_default(),
    };
    let beats = secs * bp.bpm / 60.0;
    bp.loops.push(LoopPart {
        name: name.clone(),
        role: role.into(),
        sound: Sound::Loop {
            path: path.display().to_string(),
            bpm: bp.bpm,
            beats,
            key: None,
            tonal: false,
            label: "take".into(),
        },
        transpose: 0,
        chop: Vec::new(),
        slice_beats: 0.0,
        mix: TrackMix { reverb: 0.12, bus: "music".into(), ..Default::default() },
        from: Some(from.to_string()),
    });
    let mut reached = Vec::new();
    let mut at = 0.0;
    for s in bp.form.iter_mut().skip(first) {
        if at >= beats - 1e-6 {
            break;
        }
        s.layers.push(name.clone());
        reached.push(s.name.clone());
        at += s.bars as f64 * 4.0;
    }
    let bars = beats / 4.0;
    bp.lineage.push(format!("recorded {name}: {bars:.1} bars from {from}"));
    Ok(format!("{name}: a take of {bars:.1} bars from the start of {from}, on in {}", reached.join(", ")))
}

/// Copies the takes the song plays from outside `folder` into its `takes/` and points the song at the copies, so
/// that a saved song folder holds its recordings. Returns each take's old path and new one.
pub fn gather(bp: &mut Blueprint, folder: &Path) -> Result<Vec<(String, String)>> {
    let dir = folder.join("takes");
    let mut moves = Vec::new();
    for l in bp.loops.iter_mut().filter(|l| l.from.is_some()) {
        let Sound::Loop { path, .. } = &mut l.sound else { continue };
        let src = PathBuf::from(&*path);
        let Some(file) = src.file_name() else { continue };
        let dst = dir.join(file);
        // a take is named after its sound: a file there of that name is this take, whether copied before or never moved
        if !dst.is_file() {
            std::fs::create_dir_all(&dir).with_context(|| format!("cannot write {}", dir.display()))?;
            std::fs::copy(&src, &dst)
                .with_context(|| format!("cannot copy the take {} into {}", src.display(), dir.display()))?;
        }
        let new = dst.display().to_string();
        if *path != new {
            moves.push((std::mem::replace(path, new.clone()), new));
        }
    }
    Ok(moves)
}

/// points the takes that play a file in `moves` (old path, new path) at its new place
pub fn repoint(bp: &mut Blueprint, moves: &[(String, String)]) {
    for l in bp.loops.iter_mut().filter(|l| l.from.is_some()) {
        if let Sound::Loop { path, .. } = &mut l.sound
            && let Some((_, new)) = moves.iter().find(|(old, _)| old == path)
        {
            *path = new.clone();
        }
    }
}

/// points the takes whose file is gone (the song folder moved, or came from another computer) at the file of that
/// name in `folder`'s `takes/`; returns how many it found
pub fn find_moved(bp: &mut Blueprint, folder: &Path) -> usize {
    let mut found = 0;
    for l in bp.loops.iter_mut().filter(|l| l.from.is_some()) {
        let Sound::Loop { path, .. } = &mut l.sound else { continue };
        if Path::new(&*path).is_file() {
            continue;
        }
        let Some(file) = Path::new(&*path).file_name() else { continue };
        let here = folder.join("takes").join(file);
        if here.is_file() {
            *path = here.display().to_string();
            found += 1;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::{self, Options};
    use crate::genres;

    fn song() -> Blueprint {
        let g = genres::get("house").unwrap();
        compose::generate(&g, 3, &Options { short: true, samples: 0.0, loops: false, ..Default::default() }, None)
    }

    fn scratch(what: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("claudius-take-{what}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn tone(secs: f64) -> Stereo {
        let n = (secs * config::SRF) as usize;
        let l: Vec<f32> = (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        Stereo { l: l.clone(), r: l }
    }

    #[test]
    fn a_take_starts_with_its_section_and_goes_on_as_long_as_it_lasts() {
        let mut bp = song();
        let from = bp.form[1].name.clone();
        let first = bp.form[1].bars as f64 * 4.0 * 60.0 / bp.bpm;
        let msg = add(&mut bp, Path::new("/takes/a.flac"), first + 1.0, &from, None, None).unwrap();
        assert!(msg.starts_with("take1: a take of"), "{msg}");
        let on: Vec<bool> = bp.form.iter().map(|s| s.layers.iter().any(|l| l == "take1")).collect();
        assert_eq!(&on[..3], &[false, true, true], "its section and the one it reaches into");
        assert!(on[3..].iter().all(|o| !o));
        let take = bp.loops.iter().find(|l| l.name == "take1").unwrap();
        assert_eq!((take.role.as_str(), take.from.as_deref()), ("lead", Some(from.as_str())));
        assert!(add(&mut bp, Path::new("/takes/b.flac"), 1.0, &from, Some("take1"), None).is_err(), "names are unique");
        assert!(add(&mut bp, Path::new("/takes/b.flac"), 1.0, &from, Some("my take"), None).is_err());
        assert!(add(&mut bp, Path::new("/takes/b.flac"), 1.0, "nowhere", None, None).is_err());
        assert!(add(&mut bp, Path::new("/takes/b.flac"), 1.0, &from, None, Some("kazoo")).is_err());
        assert!(add(&mut bp, Path::new("/takes/b.flac"), 1.0, &from, Some("vox"), Some("vocal")).is_ok());
    }

    #[test]
    fn a_take_is_one_clip_from_its_start_and_never_starts_over() {
        let mut bp = song();
        let from = bp.form[1].name.clone();
        let beats = (bp.form[1].bars + bp.form[2].bars) as f64 * 4.0;
        let secs = beats * 60.0 / bp.bpm;
        add(&mut bp, Path::new("/takes/a.flac"), secs, &from, None, None).unwrap();
        // on in the first section too, which comes before the take starts, and in the last, after a gap
        bp.form[0].layers.push("take1".into());
        let last = bp.form.len() - 1;
        bp.form[last].layers.push("take1".into());
        let p = compose::arrange(&bp);
        let t = p.tracks.iter().find(|t| t.name == "take1").unwrap();
        let start = p.sections[1].start_bar as f64 * 4.0;
        assert_eq!(t.clips.len(), 2, "{:?}", t.clips);
        assert!(t.clips.iter().all(|c| c.once));
        assert_eq!((t.clips[0].t, t.clips[0].d, t.clips[0].offset), (start, beats, 0.0));
        let end = p.sections[last].start_bar as f64 * 4.0;
        assert_eq!((t.clips[1].t, t.clips[1].offset), (end, end - start), "where the take is by then");
    }

    #[test]
    fn a_take_plays_once_and_is_silent_after_its_end() {
        let dir = scratch("render");
        let path = store(&tone(1.0), &dir).unwrap();
        // nothing but the take: no other tracks, no transitions, and a dry take, so nothing rings on after it
        let mut bp = song();
        bp.loops.clear();
        bp.patterns.clear();
        bp.parts.clear();
        bp.drums.clear();
        for s in bp.form.iter_mut() {
            (s.impact, s.build, s.gap) = (false, false, false);
        }
        let from = bp.form[0].name.clone();
        add(&mut bp, &path, 1.0, &from, None, None).unwrap();
        bp.loops[0].mix.reverb = 0.0;
        let p = compose::arrange(&bp);
        let r = crate::render::render(&p, &crate::render::Options::default()).unwrap();
        let second = config::SR as usize;
        let after =
            &r.master.l[second + 4_800..(p.sections[0].bars as f64 * 4.0 * 60.0 / p.bpm * config::SRF) as usize];
        let during = &r.master.l[..second - 4_800];
        let peak = |x: &[f32]| x.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak(during) > 0.05, "{}", peak(during));
        assert!(peak(after) < 0.02 * peak(during), "after its end: {} against {}", peak(after), peak(during));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_saved_song_keeps_its_takes_and_finds_them_when_moved() {
        let (home, saved, moved) = (scratch("home"), scratch("saved"), scratch("moved"));
        let path = store(&tone(0.5), &home).unwrap();
        assert_eq!(store(&tone(0.5), &home).unwrap(), path, "the same take is one file");
        let mut bp = song();
        let from = bp.form[0].name.clone();
        add(&mut bp, &path, 0.5, &from, None, None).unwrap();
        let mut copy = bp.clone();
        let moves = gather(&mut copy, &saved).unwrap();
        assert_eq!(moves.len(), 1);
        assert!(saved.join("takes").join(path.file_name().unwrap()).is_file());
        repoint(&mut bp, &moves);
        assert_eq!(bp, copy, "the song points at the copy too");
        assert!(gather(&mut copy, &saved).unwrap().is_empty(), "saved again: nothing to copy");
        std::fs::rename(&saved, &moved).unwrap();
        assert_eq!(find_moved(&mut copy, &moved), 1);
        let Sound::Loop { path: now, .. } = &copy.loops.iter().find(|l| l.from.is_some()).unwrap().sound else {
            unreachable!()
        };
        assert!(Path::new(now).starts_with(&moved) && Path::new(now).is_file());
        for d in [home, moved] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
