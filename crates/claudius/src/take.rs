//! Takes: recordings made for a song (a vocal, a guitar, a melody sung into the microphone). A take is a loop part
//! whose `from` names the section it starts in: it plays once from there, and on through the later sections that play
//! it, at the tempo it was recorded at (warped, as a loop is, when the song's tempo changes). Its file is FLAC in the
//! song folder's `takes/`, or, until the song is saved, in `takes/` in the songs folder (~/Music/claudius).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::analysis::melody;
use crate::audio::Stereo;
use crate::model::{Blueprint, LoopPart, Part, Project, Role, Sound, TrackMix};
use crate::rng::{Rng, hash_str};
use crate::synth::presets;
use crate::theory::Note;
use crate::theory::chord::Voicing;
use crate::{compose, config};

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

/// what a take's melody becomes
#[derive(Clone, Debug, Default)]
pub struct Melody {
    /// the sounds to play it with, one part each: synth presets or installed instruments ("pluck", "piano")
    pub sounds: Vec<String>,
    /// the first part's name (melody when not given; the others are it with 2, 3 ... after it)
    pub name: Option<String>,
    /// lead (the default), keys, chords, pad, arp or bass
    pub role: Option<String>,
    /// keep notes outside the song's key where they were sung, instead of on its nearest tone
    pub chromatic: bool,
    /// the take goes on playing beside its parts, instead of going quiet
    pub keep_take: bool,
}

/// Turns the melody of take `take` (sung, hummed, whistled or played one note at a time) into the notes of new
/// parts, one for each sound, in the sections the take plays in. The take goes quiet unless `keep_take`. Returns what
/// was made, in words.
pub fn melody(bp: &mut Blueprint, take: &str, m: &Melody) -> Result<String, String> {
    let names: Vec<String> = bp.loops.iter().filter(|l| l.from.is_some()).map(|l| l.name.clone()).collect();
    let Some(t) = bp.loops.iter().position(|l| l.name == take && l.from.is_some()) else {
        return Err(if names.is_empty() {
            "the song has no takes: record one first".to_string()
        } else {
            format!("no take called '{take}' (takes: {})", names.join(" "))
        });
    };
    let role = match m.role.as_deref().unwrap_or("lead") {
        "lead" => Role::Lead,
        "keys" => Role::Keys,
        "chords" => Role::Chords,
        "pad" => Role::Pad,
        "arp" => Role::Arp,
        "bass" => Role::Bass,
        other => return Err(format!("a part is lead, keys, chords, pad, arp or bass, not '{other}'")),
    };
    if m.sounds.is_empty() {
        return Err("play it with what? give a synth preset or an instrument, such as pluck or piano".into());
    }
    if let Some(s) = m.sounds.iter().find(|s| presets::get(s).is_none() && claudius_packs::resolve(s, None).is_none()) {
        return Err(format!("no synth preset or installed instrument is called '{s}'"));
    }
    let (path, bpm) = match &bp.loops[t].sound {
        Sound::Loop { path, bpm, .. } => (path.clone(), *bpm),
        _ => return Err(format!("{take} is not a recording")),
    };
    let audio = crate::audio::load_once(Path::new(&path)).map_err(|e| format!("cannot read {take}: {e:#}"))?;
    let o = melody::Options { bpm, grid: 0.25, chromatic: m.chromatic };
    let notes = melody::transcribe(&audio, &bp.key, &o);
    if notes.is_empty() {
        return Err(format!(
            "{take} has no melody claudius can hear: it wants one note at a time, sung or played clearly"
        ));
    }
    // where the take starts, and the sections it plays in from there, each with the stretch of the melody over it
    let starts: Vec<f64> =
        bp.form.iter().scan(0.0, |at, s| Some(std::mem::replace(at, *at + s.bars as f64 * 4.0))).collect();
    let from = bp.loops[t].from.clone().unwrap_or_default();
    let first = bp
        .form
        .iter()
        .position(|s| s.name == from)
        .or_else(|| bp.form.iter().position(|s| s.layers.contains(&bp.loops[t].name)));
    let Some(first) = first else { return Err(format!("{take} plays in no section")) };
    let end = notes.iter().map(|n| n.t + n.d).fold(0.0, f64::max) + starts[first];
    let mut by_prog: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    let mut sections = Vec::new();
    for (i, s) in bp.form.iter().enumerate().skip(first) {
        let (a, b) = (starts[i], starts[i] + s.bars as f64 * 4.0);
        if a >= end {
            break;
        }
        if !s.layers.contains(&bp.loops[t].name) {
            continue;
        }
        // every note sounding in the section: one held over from the section before goes on from its start
        let over: Vec<Note> = notes
            .iter()
            .filter_map(|n| {
                let (t, end) = (n.t + starts[first] - a, n.t + starts[first] - a + n.d);
                let (from, to) = (t.max(0.0), end.min(b - a));
                (to - from >= 1.0 / 16.0).then_some(Note { t: from, d: to - from, ..*n })
            })
            .collect();
        if !over.is_empty() {
            // a section with a progression already sung over plays what was sung over it first
            by_prog.entry(s.prog.clone()).or_insert(over);
            sections.push(i);
        }
    }
    let low = notes.iter().map(|n| n.p).min().unwrap_or(60);
    let high = notes.iter().map(|n| n.p).max().unwrap_or(72);
    let base = m.name.clone().unwrap_or_else(|| "melody".into());
    if !base.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("'{base}' cannot name a track: use letters, digits, - and _"));
    }
    let mut taken = bp.layer_names();
    let mut made = Vec::new();
    // the parts take the free names of base, base2, base3 ... in turn
    let mut names = (1..).map(|k| if k == 1 { base.clone() } else { format!("{base}{k}") });
    for sound in &m.sounds {
        let name = names.by_ref().find(|n| !taken.contains(n)).unwrap_or_default();
        let seed = hash_str(&name) ^ bp.seed;
        bp.parts.push(Part {
            name: name.clone(),
            role,
            sound: compose::part_sound(sound, &mut Rng::new(seed)),
            style: style_for(role).into(),
            register: (low, high),
            density: 0.5,
            voices: 1,
            voicing: Voicing::Close,
            rate: 0.0,
            seed,
            notes: by_prog.clone(),
            mix: TrackMix { reverb: 0.15, bus: "music".into(), ..Default::default() },
        });
        for &i in &sections {
            bp.form[i].layers.push(name.clone());
        }
        taken.push(name.clone());
        made.push(format!("{name} ({sound})"));
    }
    if !m.keep_take {
        bp.loops[t].mix.mute = true;
    }
    let names: Vec<String> = sections.iter().map(|&i| bp.form[i].name.clone()).collect();
    bp.lineage.push(format!("{take} into notes: {} notes for {}", notes.len(), made.join(", ")));
    Ok(format!(
        "{} note{} from {take}, {} to {}, played by {} in {}{}",
        notes.len(),
        if notes.len() == 1 { "" } else { "s" },
        crate::theory::scale::note_name(low),
        crate::theory::scale::note_name(high),
        made.join(", "),
        names.join(", "),
        if m.keep_take { "" } else { "; the take is muted" },
    ))
}

/// the style a part of `role` is written in when it plays a section its notes do not cover
fn style_for(role: Role) -> &'static str {
    match role {
        Role::Bass => "offbeat",
        Role::Chords | Role::Keys | Role::Pad => "sustain",
        Role::Arp => "up",
        Role::Lead => "motif",
    }
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
        moved(&mut l.sound, moves);
    }
}

/// the same in an arranged song
pub fn repoint_tracks(project: &mut Project, moves: &[(String, String)]) {
    for t in project.tracks.iter_mut() {
        moved(&mut t.sound, moves);
    }
}

fn moved(sound: &mut Sound, moves: &[(String, String)]) {
    if let Sound::Loop { path, .. } = sound
        && let Some((_, new)) = moves.iter().find(|(old, _)| old == path)
    {
        *path = new.clone();
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
    fn a_song_folder_the_command_line_writes_holds_its_takes() {
        let (home, out) = (scratch("cli-home"), scratch("cli-out"));
        let path = store(&tone(0.5), &home).unwrap();
        let mut bp = song();
        bp.form.truncate(1);
        bp.form[0].bars = 2;
        let from = bp.form[0].name.clone();
        add(&mut bp, &path, 0.5, &from, None, None).unwrap();
        crate::song::render_and_write(&bp, &out, false, None).unwrap();
        let copy = out.join("takes").join(path.file_name().unwrap());
        assert!(copy.is_file(), "the take is copied into the folder");
        let written = crate::song::read_blueprint(&out.join("blueprint.json")).unwrap();
        assert_eq!(written.files(), [copy.display().to_string()], "and the folder's song plays the copy");
        let project = std::fs::read_to_string(out.join("project.json")).unwrap();
        assert!(!project.contains(&path.display().to_string()), "the arranged song too");
        for d in [home, out] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// a song at 120 BPM (as `melody::sing` sings) with a take of `notes` from its second section
    fn sung(what: &str, notes: &[(f32, f64, bool)]) -> (Blueprint, PathBuf) {
        let dir = scratch(what);
        let voice = melody::sing(notes);
        let path = store(&voice, &dir).unwrap();
        let mut bp = song();
        bp.bpm = 120.0;
        bp.key = crate::theory::scale::Key::new(0, crate::theory::scale::Mode::Major);
        let from = bp.form[1].name.clone();
        add(&mut bp, &path, voice.len() as f64 / config::SRF, &from, None, None).unwrap();
        (bp, dir)
    }

    #[test]
    fn a_sung_take_becomes_parts_that_play_its_notes_where_it_was_sung() {
        let (mut bp, dir) = sung("melody", &[(60.0, 1.0, false), (64.0, 1.0, true), (67.0, 2.0, false)]);
        let m = Melody { sounds: vec!["pluck".into(), "bell".into()], ..Default::default() };
        let msg = melody(&mut bp, "take1", &m).unwrap();
        assert!(msg.starts_with("3 notes from take1, C4 to G4, played by melody (pluck), melody2 (bell)"), "{msg}");
        let take = bp.loops.iter().find(|l| l.name == "take1").unwrap();
        assert!(take.mix.mute, "the take goes quiet: its parts play it now");
        let prog = bp.form[1].prog.clone();
        for name in ["melody", "melody2"] {
            let p = bp.parts.iter().find(|p| p.name == name).unwrap();
            assert_eq!((p.role, p.register), (Role::Lead, (60, 67)));
            let pitches: Vec<i32> = p.notes[&prog].iter().map(|n| n.p).collect();
            assert_eq!(pitches, [60, 64, 67]);
            assert!(bp.form[1].layers.contains(&p.name) && !bp.form[0].layers.contains(&p.name));
        }
        let project = compose::arrange(&bp);
        let track = project.tracks.iter().find(|t| t.name == "melody").unwrap();
        let start = project.sections[1].start_bar as f64 * 4.0;
        let at: Vec<f64> = track.notes.iter().take(3).map(|n| n.t - start).collect();
        assert_eq!(at, [0.0, 1.0, 2.0], "where they were sung");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_note_held_into_the_next_section_goes_on_there_and_names_follow_in_turn() {
        // a short first section and a long note sung through its end
        let (mut bp, dir) = sung("held", &[(60.0, 6.0, false), (64.0, 2.0, false)]);
        // a one-bar section, then one with a progression of its own
        bp.form[1].bars = 1;
        let other = bp.harmony.progressions.values().next().cloned().unwrap();
        bp.harmony.progressions.insert("Z".into(), other);
        bp.form[2].prog = "Z".into();
        bp.form[2].layers.push("take1".into());
        let pluck = Melody { sounds: vec!["pluck".into()], keep_take: true, ..Default::default() };
        melody(&mut bp, "take1", &pluck).unwrap();
        let part = bp.parts.iter().find(|p| p.name == "melody").unwrap();
        let first = &part.notes[&bp.form[1].prog];
        assert_eq!((first[0].p, first[0].t, first[0].d), (60, 0.0, 4.0), "the note to the section's end");
        assert!(bp.form[2].layers.contains(&part.name), "and the part plays on into the next");
        let next: Vec<(i32, f64, f64)> = part.notes["Z"].iter().map(|n| (n.p, n.t, n.d)).collect();
        assert_eq!(next, [(60, 0.0, 2.0), (64, 2.0, 2.0)], "where the held note goes on from the section's start");
        // asked again for two sounds, the parts take the next free names in turn
        let two = Melody { sounds: vec!["pluck".into(), "bell".into()], keep_take: true, ..Default::default() };
        melody(&mut bp, "take1", &two).unwrap();
        let names: Vec<&str> =
            bp.parts.iter().filter(|p| p.name.starts_with("melody")).map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["melody", "melody2", "melody3"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_melody_longer_than_a_cycle_plays_once_in_its_section() {
        // six bars of whole notes, over a section of at least eight bars
        let notes: Vec<(f32, f64, bool)> =
            [60.0, 62.0, 64.0, 65.0, 67.0, 69.0].iter().map(|&p| (p, 4.0, true)).collect();
        let (mut bp, dir) = sung("long", &notes);
        bp.form[1].bars = 8;
        melody(&mut bp, "take1", &Melody { sounds: vec!["pluck".into()], ..Default::default() }).unwrap();
        let project = compose::arrange(&bp);
        let s = &project.sections[1];
        let (a, b) = (s.start_bar as f64 * 4.0, (s.start_bar + s.bars) as f64 * 4.0);
        let track = project.tracks.iter().find(|t| t.name == "melody").unwrap();
        let inside: Vec<i32> = track.notes.iter().filter(|n| n.t >= a && n.t < b).map(|n| n.p).collect();
        assert_eq!(inside, [60, 62, 64, 65, 67, 69], "the six notes once, not over one another");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_take_without_a_melody_or_a_sound_to_play_it_is_refused() {
        let (mut bp, dir) = sung("refused", &[(60.0, 2.0, false)]);
        let pluck = Melody { sounds: vec!["pluck".into()], ..Default::default() };
        assert!(melody(&mut bp, "nope", &pluck).unwrap_err().contains("no take called 'nope'"));
        let kazoo = Melody { sounds: vec!["kazoo9000".into()], ..Default::default() };
        assert!(melody(&mut bp, "take1", &kazoo).unwrap_err().contains("'kazoo9000'"));
        let quiet = store(&Stereo { l: vec![0.0; 48_000], r: vec![0.0; 48_000] }, &dir).unwrap();
        let from = bp.form[1].name.clone();
        add(&mut bp, &quiet, 1.0, &from, Some("hush"), None).unwrap();
        assert!(melody(&mut bp, "hush", &pluck).unwrap_err().contains("no melody"));
        let _ = std::fs::remove_dir_all(dir);
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
