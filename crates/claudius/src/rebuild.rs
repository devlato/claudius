//! A rebuild: a recording made again as a song of its own parts, each played on claudius's sounds, so that every note
//! of it can be changed. Its form is its own sections, bar for bar, each on a progression of its own bars' chords (one
//! to each section, so that each section plays its own notes), at its own tempo, a band's speeding up and slowing down
//! taken out, and in its own swing.
//!
//! - Its drums, apart in its stems, are its kicks, snares and hats (`analysis::drums`), struck where it strikes them:
//!   each section's folded into a pattern of its own (the section's variant), a step struck only some of the times
//!   round played with that chance, and each lane as far behind or ahead of the beat as its hits on the eighths are.
//!   Without its stems, the drum habits heard in its busiest sections play wherever its kick is heard.
//! - Its bass, and the rest of it but its voice (its keys, guitars, synths), are the notes the ears hear them play
//!   (`transcribe`), on the sixteenths: the bass one note at a time, the rest chords and all, without the faint copies
//!   an instrument's overtones leave an octave, a twelfth or two octaves over a note it plays. Without the ears, the
//!   genre's bass and chord parts play on its chords, as they do in a song on its skeleton.
//! - Its voice, when it sings, stays on a track of its own (`remix::VOICE`), as a rework keeps it, and the melody it
//!   sings is written out on a lead of its own (`MELODY`), which plays in no section until it is put in one.
//!
//! The genre gives what the recording does not: the sounds its parts play on, its mix and its master.

use std::collections::BTreeMap;
use std::path::Path;

use crate::analysis::drums::{self, Kind};
use crate::analysis::melody;
use crate::analysis::song::Analysis;
use crate::compose::{self, Library, Options};
use crate::genres::GenreProfile;
use crate::model::{Blueprint, LoopPart, Part, Role, SectionSpec, Sound};
use crate::remix::{self, Bars, Source};
use crate::theory::Note;
use crate::theory::rhythm::{Hit, Trig};
use crate::transcribe::Heard;

/// the parts a rebuild writes out: the bass's notes, the rest's, and the melody the voice sings
pub const BASS: &str = "bass";
pub const KEYS: &str = "keys";
pub const MELODY: &str = "melody";

/// What was heard in a recording's stems: each `None` when it could not be (no stems, or no ears to hear notes with).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Played {
    /// its drums' hits (`analysis::drums::hear` of its drum stem), when a kit plays in it
    pub drums: Option<Vec<drums::Hit>>,
    /// the notes its bass plays, and the rest of it but its voice
    pub bass: Option<Vec<Heard>>,
    pub rest: Option<Vec<Heard>>,
    /// the melody its voice sings, in beats from its first bar, when it sings
    pub melody: Option<Vec<Note>>,
}

/// The notes of `heard` that are `x`'s own: within 18 dB of its loud ones (the tenth loudest). The rest is what other
/// instruments leave in a stem, which the ears hear as notes as surely as the stem's own, however quiet they are.
fn own(x: &crate::audio::Stereo, heard: Vec<Heard>) -> Vec<Heard> {
    let mono = x.mono();
    let db = |n: &Heard| {
        let at = |t: f64| ((t * crate::config::SRF) as usize).min(mono.len());
        let (a, b) = (at(n.start), at(n.end.min(n.start + 0.2)));
        let power = mono[a..b.max(a)].iter().map(|v| v * v).sum::<f32>() / (b - a).max(1) as f32;
        10.0 * power.max(1e-12).log10()
    };
    let levels: Vec<f32> = heard.iter().map(db).collect();
    let mut sorted = levels.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let Some(&loud) = sorted.get(sorted.len() / 10) else { return heard };
    heard.into_iter().zip(levels).filter(|(_, l)| *l >= loud - 18.0).map(|(n, _)| n).collect()
}

/// What can be heard in `src`'s stems: none of it without them, and no notes of its bass or the rest without the ears.
/// A stem that holds next to nothing plays nothing.
pub fn played(src: &Source) -> Played {
    let Some(s) = src.stems else { return Played::default() };
    let a = src.a;
    let notes = |x: &crate::audio::Stereo, lo, hi, shortest| {
        if s.share(x) < remix::REST {
            Some(Vec::new())
        } else {
            crate::transcribe::notes(x, lo, hi, shortest).map(|n| own(x, n))
        }
    };
    let melody = s.sung().then(|| {
        let flat = remix::cut(&s.vocals, a, Bars { first: 0, count: a.bars });
        melody::transcribe(&flat, &a.key, &melody::Options { bpm: a.cut_bpm(), grid: 0.25, chromatic: false })
    });
    Played {
        drums: s.drummed().then(|| drums::hear(&s.drums)),
        // a bass's pickups, a sixteenth or less, are shorter than Basic Pitch's 128 ms at 120 BPM and faster
        bass: notes(&s.bass, 30.0, 500.0, 80.0),
        rest: notes(&s.other, 55.0, 4200.0, 100.0),
        melody,
    }
}

/// The recording `src` rebuilt in genre `g` (the module's doc says how), its voice kept in `dir` with the song's takes.
/// `opts` as for a new song (its tempo); `lib` for the sounds the genre plays its parts on.
pub fn rebuild(
    src: &Source,
    g: &GenreProfile,
    seed: u64,
    opts: &Options,
    lib: Option<&dyn Library>,
    dir: &Path,
) -> Result<Blueprint, String> {
    rebuild_with(src, &played(src), g, seed, opts, lib, dir)
}

/// `rebuild`, of what was heard in it already (`played`)
pub fn rebuild_with(
    src: &Source,
    heard: &Played,
    g: &GenreProfile,
    seed: u64,
    opts: &Options,
    lib: Option<&dyn Library>,
    dir: &Path,
) -> Result<Blueprint, String> {
    let (name, a) = (src.name, src.a);
    if a.bars < 8 || a.sections.is_empty() {
        return Err("the recording is too short, or its sections too unclear, to rebuild".into());
    }
    let bpm = opts.bpm.unwrap_or_else(|| a.cut_bpm());
    let o = Options { bpm: Some(bpm), key: Some(a.key), sample_search: None, family: false, ..opts.clone() };
    let mut bp = compose::generate(g, seed, &o, lib);
    (bp.bpm, bp.key, bp.swing) = (bpm, a.key, a.swing);
    let (form, progressions) = form(a);
    bp.form = form;
    bp.harmony.progressions = progressions;
    bp.harmony.bars_per_chord = 1;
    bp.loops.clear();
    bp.patterns.clear();
    let mut said = vec![format!("its {} sections bar for bar, on its chords", bp.form.len())];
    said.push(drum_lanes(&mut bp, a, heard.drums.as_deref()));
    let pick = |roles: &[Role]| bp.parts.iter().find(|p| roles.contains(&p.role)).cloned();
    let (bass, keys, lead) =
        (pick(&[Role::Bass]), pick(&[Role::Keys, Role::Chords, Role::Pad]), pick(&[Role::Lead, Role::Arp]));
    bp.parts.clear();
    said.extend(bass_part(&mut bp, a, bass, heard.bass.as_deref()));
    said.extend(keys_part(&mut bp, a, keys, heard.rest.as_deref()));
    if let Some(m) = heard.melody.as_deref().filter(|m| !m.is_empty())
        && let Some(mut p) = part(MELODY, Role::Lead, lead, "soft_lead", m)
    {
        p.notes = per_section(&bp.form, m);
        said.push(format!(
            "the melody it sings as {} notes on {MELODY}, which plays in no section until put in one",
            m.len()
        ));
        bp.parts.push(p);
    }
    if let Some(s) = src.stems.filter(|s| s.sung()) {
        let slug = crate::song::slug(Path::new(name).file_stem().map_or(name.into(), |s| s.to_string_lossy()).as_ref());
        let body = remix::cut(&s.vocals, a, Bars { first: 0, count: a.bars });
        let path = crate::take::store(&body, &dir.join(&slug)).map_err(|e| format!("{e:#}"))?;
        bp.loops.push(LoopPart {
            name: remix::VOICE.into(),
            role: "vocal".into(),
            sound: Sound::Loop {
                path: path.to_string_lossy().into_owned(),
                bpm: a.cut_bpm(),
                beats: (a.bars * 4) as f64,
                key: None,
                tonal: false,
                label: format!("{slug}, its voice"),
            },
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix: remix::voice_mix(),
            from: bp.form.first().map(|s| s.name.clone()),
            sampled: true,
        });
        for s in &mut bp.form {
            s.layers.push(remix::VOICE.into());
        }
        said.push(format!("its voice on a track of its own ({})", remix::VOICE));
    }
    let line = format!("rebuilt from '{name}': {}", said.join("; "));
    bp.lineage.insert(1.min(bp.lineage.len()), line);
    Ok(bp)
}

/// The recording's sections as the song's: each its own kind, length and energy, its drums' variant named after it,
/// on a progression of its own of its bars' chords (a chord heard in none of them carried on from the bar before).
fn form(a: &Analysis) -> (Vec<SectionSpec>, BTreeMap<String, Vec<String>>) {
    let mut form: Vec<SectionSpec> = Vec::new();
    let mut progs = BTreeMap::new();
    let tonic = if a.key.mode == crate::theory::scale::Mode::Minor { "i" } else { "I" };
    let mut last = a.numerals.iter().find(|c| !c.is_empty()).cloned().unwrap_or_else(|| tonic.into());
    for (i, s) in a.sections.iter().enumerate() {
        let chords: Vec<String> = (s.start_bar..s.start_bar + s.bars)
            .map(|b| {
                if let Some(c) = a.numerals.get(b).filter(|c| !c.is_empty()) {
                    last = c.clone();
                }
                last.clone()
            })
            .collect();
        let prog = remix::progression_name(i);
        progs.insert(prog.clone(), chords);
        let of_kind = form.iter().filter(|f| f.kind == s.kind).count();
        let name = if of_kind == 0 { s.kind.name().to_string() } else { format!("{}{}", s.kind.name(), of_kind + 1) };
        form.push(SectionSpec {
            name: name.clone(),
            kind: s.kind,
            bars: s.bars as u32,
            energy: s.energy,
            prog,
            layers: Vec::new(),
            variant: name,
            filter_in: false,
            build: false,
            build_style: Default::default(),
            build_amount: 0.5,
            gap: false,
            impact: false,
            fills: false,
        });
    }
    (form, progs)
}

/// where `t` seconds into the recording falls, in beats from its first bar's downbeat
fn beat_of(a: &Analysis, t: f64) -> f64 {
    let b = a.bar_at(t);
    if b < 0 {
        return 4.0 * (t - a.bar_start(0)) / a.bar_secs.max(1e-3);
    }
    let (from, to) = (a.bar_start(b as usize), a.bar_start(b as usize + 1));
    4.0 * (b as f64 + (t - from) / (to - from).max(1e-3))
}

/// the beat each section of `form` starts on, and the beat after the last
fn starts(form: &[SectionSpec]) -> Vec<f64> {
    let mut out = vec![0.0];
    for s in form {
        out.push(out[out.len() - 1] + s.bars as f64 * 4.0);
    }
    out
}

/// the genre's lane a kind of hit plays in (a snare's in its snare's lane, else its clap's), else a new one
fn lane_for(bp: &Blueprint, kind: Kind) -> String {
    let roles: &[&str] = match kind {
        Kind::Kick => &["kick"],
        Kind::Snare => &["snare", "clap"],
        Kind::Hat => &["hat"],
    };
    roles
        .iter()
        .find_map(|r| bp.drums.iter().find(|d| d.role() == *r))
        .map_or(kind.name().to_string(), |d| d.name.clone())
}

/// A kind's pattern as a lane's hits over its bars: each sixteenth struck at its velocity, one struck only some of the
/// times round with that chance.
fn hits_of(steps: &drums::Steps) -> Vec<Hit> {
    let chances = steps.chances();
    (0..steps.v.len())
        .filter(|&s| steps.v[s] > 0.0)
        .map(|s| {
            let prob = chances.iter().find(|(c, _)| *c == s).map(|&(_, p)| p);
            Hit { t: s as f64 / 4.0, v: steps.v[s], trig: prob.map(|prob| Trig { prob, ..Trig::default() }) }
        })
        .collect()
}

/// how far behind the beat (ahead, below zero) the hits on the eighths land, on average, in ms at `bpm`: none within
/// 3 ms, and 40 at most either way
fn lateness(beats: &[f64], bpm: f64) -> f32 {
    let off: Vec<f64> = beats.iter().map(|b| b * 2.0 - (b * 2.0).round()).filter(|d| d.abs() < 0.25).collect();
    if off.is_empty() {
        return 0.0;
    }
    let ms = off.iter().sum::<f64>() / off.len() as f64 / 2.0 * 60_000.0 / bpm;
    if ms.abs() < 3.0 { 0.0 } else { ms.clamp(-40.0, 40.0) as f32 }
}

/// The song's kick, snare and hat lanes, each section's hits `heard` folded into a pattern of the section's own (four
/// bars, or as many as it has), the lanes struck in a section where they play in it; without its drums' hits, the
/// habits heard in its busiest sections, wherever its kick is heard. Every other lane goes. What was made, in words.
fn drum_lanes(bp: &mut Blueprint, a: &Analysis, heard: Option<&[drums::Hit]>) -> String {
    let lanes: Vec<(Kind, String)> = Kind::ALL.iter().map(|&k| (k, lane_for(bp, k))).collect();
    bp.drums.retain(|d| lanes.iter().any(|(_, l)| *l == d.name));
    // the recording's own sixteenths, in its own swing
    for d in &mut bp.drums {
        d.patterns.clear();
        (d.groove, d.genre, d.swing, d.length, d.speed) = (None, None, None, None, 1.0);
    }
    let lane = |k: Kind| lanes.iter().find(|(kk, _)| *kk == k).map(|(_, l)| l.clone()).unwrap_or_default();
    let Some(hits) = heard else {
        let habits = [(Kind::Kick, &a.kick_steps), (Kind::Snare, &a.snare_steps), (Kind::Hat, &a.hat_steps)];
        let mut played = Vec::new();
        for (kind, steps) in habits {
            let hits: Vec<Hit> = steps
                .iter()
                .enumerate()
                .filter(|(_, v)| **v >= 0.25)
                .map(|(s, v)| Hit::new(s as f64 / 4.0, v.clamp(0.3, 1.0)))
                .collect();
            if hits.is_empty() || crate::script::put_hits(bp, &lane(kind), "main", hits, 1, false).is_err() {
                continue;
            }
            played.push(lane(kind));
        }
        for (s, g) in bp.form.iter_mut().zip(&a.sections) {
            s.variant = "main".into();
            if g.kick {
                s.layers.extend(played.iter().cloned());
            }
        }
        bp.drums.retain(|d| played.contains(&d.name));
        return if played.is_empty() {
            "no drums heard in it".into()
        } else {
            "its drums' habits wherever its kick is heard, its stems not apart".into()
        };
    };
    let bpm = bp.bpm;
    let at: Vec<(f64, &drums::Hit)> = hits.iter().map(|h| (beat_of(a, h.t), h)).collect();
    let bounds = starts(&bp.form);
    let mut struck: BTreeMap<String, usize> = BTreeMap::new();
    for (i, s) in bp.form.clone().iter().enumerate() {
        let (from, to) = (bounds[i], bounds[i + 1]);
        // the section's hits, at the seconds they would strike at the song's tempo from its start
        let of: Vec<drums::Hit> = at
            .iter()
            .filter(|(b, _)| *b >= from - 0.125 && *b < to - 0.125)
            .map(|(b, h)| drums::Hit { t: (b - from).max(0.0) * 60.0 / bpm, ..**h })
            .collect();
        let bars = s.bars.clamp(1, 4);
        for steps in drums::fold(&of, bpm, 0.0, bars).iter().filter(|st| st.hits() > 0) {
            let l = lane(steps.kind);
            if crate::script::put_hits(bp, &l, &s.name, hits_of(steps), bars, false).is_ok() {
                bp.form[i].layers.push(l.clone());
                *struck.entry(l).or_default() += of.iter().filter(|h| h.kind == steps.kind).count();
            }
        }
    }
    for d in &mut bp.drums {
        let beats: Vec<f64> = at.iter().filter(|(_, h)| lane(h.kind) == d.name).map(|(b, _)| *b).collect();
        d.late_ms = lateness(&beats, bpm);
    }
    bp.drums.retain(|d| struck.contains_key(&d.name));
    if struck.is_empty() {
        return "no drums heard in it".into();
    }
    let each: Vec<String> = struck.iter().map(|(l, n)| format!("{n} on {l}")).collect();
    format!(
        "its drums struck as it strikes them, each section's folded into a pattern of its own ({})",
        each.join(", ")
    )
}

/// a note an instrument's overtone leaves: an octave, a twelfth, two octaves, two and a third or two and a fifth over
/// a note sounding with it from about as early, which the ears are half as sure of again at least
fn overtone(n: &Heard, of: &Heard) -> bool {
    let over = n.pitch - of.pitch;
    let with = (n.start - of.start).abs() < 0.06 && of.end >= n.start + 0.5 * (n.end - n.start);
    [12, 19, 24, 28, 31].contains(&over) && with && of.sure >= 1.5 * n.sure
}

/// the notes heard but for those an overtone of another left
fn without_overtones(notes: &[Heard]) -> Vec<Heard> {
    notes.iter().filter(|n| !notes.iter().any(|m| overtone(n, m))).copied().collect()
}

/// One note at a time, as a bass plays: of notes struck together the one the ears are surest of (the lower of two
/// nearly as sure), each cut short where the next starts.
fn one_at_a_time(notes: &[Heard]) -> Vec<Heard> {
    let mut by_start = without_overtones(notes);
    by_start.sort_by(|x, y| x.start.total_cmp(&y.start));
    let mut out: Vec<Heard> = Vec::new();
    for n in by_start {
        match out.last_mut() {
            Some(last) if n.start - last.start < 0.05 => {
                let lower = n.pitch < last.pitch && n.sure >= 0.8 * last.sure;
                if n.sure > last.sure * 1.25 || lower {
                    *last = n;
                }
            }
            Some(last) if n.start < last.end => {
                last.end = n.start;
                out.push(n);
            }
            _ => out.push(n),
        }
    }
    out
}

/// notes heard as notes on the song's sixteenths, in beats from its first bar, as loud as the ears are sure of them
fn on_grid(a: &Analysis, heard: &[Heard]) -> Vec<Note> {
    let q = |b: f64| (b * 4.0).round() / 4.0;
    heard
        .iter()
        .map(|n| {
            let t = q(beat_of(a, n.start));
            let d = (q(beat_of(a, n.end)) - t).max(0.25);
            Note { t, d, p: n.pitch, v: (0.35 + 0.65 * n.sure).clamp(0.3, 1.0), lock: None }
        })
        .filter(|n| n.t >= 0.0)
        .collect()
}

/// `notes` (beats from the first bar) by the progression of the section each starts in, from the section's start,
/// none held past its end
fn per_section(form: &[SectionSpec], notes: &[Note]) -> BTreeMap<String, Vec<Note>> {
    let bounds = starts(form);
    let mut out: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    for (i, s) in form.iter().enumerate() {
        let (from, to) = (bounds[i], bounds[i + 1]);
        let mut of: Vec<Note> = notes
            .iter()
            .filter(|n| n.t >= from - 1e-9 && n.t < to - 1e-9)
            .map(|n| Note { t: n.t - from, d: n.d.min(to - n.t), ..*n })
            .collect();
        if !of.is_empty() {
            of.sort_by(|x, y| x.t.total_cmp(&y.t).then(x.p.cmp(&y.p)));
            out.insert(s.prog.clone(), of);
        }
    }
    out
}

/// A part of `role` called `name`, its register the notes it plays: the genre's part (`like`), its sound and its mix,
/// else one on the synth `preset`.
fn part(name: &str, role: Role, like: Option<Part>, preset: &str, notes: &[Note]) -> Option<Part> {
    let lo = notes.iter().map(|n| n.p).min().unwrap_or(48);
    let hi = notes.iter().map(|n| n.p).max().unwrap_or(72);
    let mut p = match like {
        Some(p) => p,
        None => Part {
            name: name.into(),
            role,
            sound: Sound::Synth { preset: preset.into(), patch: crate::synth::presets::get(preset)? },
            style: String::new(),
            register: (lo, hi),
            density: 0.5,
            voices: 4,
            voicing: crate::theory::chord::Voicing::Close,
            rate: 0.25,
            seed: 0,
            notes: BTreeMap::new(),
            mix: Default::default(),
            doubles: None,
            harmonizes: None,
            octave: 0,
            late_ms: 0.0,
        },
    };
    (p.name, p.role, p.register) = (name.into(), role, (lo, hi));
    (p.doubles, p.harmonizes, p.octave) = (None, None, 0);
    p.notes.clear();
    Some(p)
}

/// the sections of `bp` a part with notes written out for `notes`' progressions plays in
fn plays_in(bp: &mut Blueprint, part: &str, notes: &BTreeMap<String, Vec<Note>>) {
    for s in bp.form.iter_mut().filter(|s| notes.contains_key(&s.prog)) {
        s.layers.push(part.into());
    }
}

/// The song's bass: the notes its bass plays, one at a time, on the genre's bass's sound; without the ears, the genre's
/// bass on its chords wherever its bass is heard. What was made, in words.
fn bass_part(bp: &mut Blueprint, a: &Analysis, genre: Option<Part>, heard: Option<&[Heard]>) -> Option<String> {
    let Some(heard) = heard else {
        let mut p = genre?;
        p.name = BASS.into();
        for (s, _) in bp.form.iter_mut().zip(&a.sections).filter(|(_, g)| g.bass) {
            s.layers.push(BASS.into());
        }
        bp.parts.push(p);
        return Some(format!("{}'s bass on its chords, its notes not heard", bp.genre.replace('_', " ")));
    };
    let notes = on_grid(a, &one_at_a_time(heard));
    if notes.is_empty() {
        return None;
    }
    let mut p = part(BASS, Role::Bass, genre, "sub_bass", &notes)?;
    p.notes = per_section(&bp.form, &notes);
    plays_in(bp, BASS, &p.notes);
    bp.parts.push(p);
    Some(format!("its bass as {} notes on {BASS}", notes.len()))
}

/// The rest of it but its voice and its bass: the notes it plays, chords and all, on the genre's keys' (or chords')
/// sound; without the ears, the genre's chord part on its chords wherever its harmony is heard. What was made, in words.
fn keys_part(bp: &mut Blueprint, a: &Analysis, genre: Option<Part>, heard: Option<&[Heard]>) -> Option<String> {
    let Some(heard) = heard else {
        let mut p = genre?;
        p.name = KEYS.into();
        for (s, _) in bp.form.iter_mut().zip(&a.sections).filter(|(_, g)| g.harmony) {
            s.layers.push(KEYS.into());
        }
        bp.parts.push(p);
        return Some(format!("{}'s chords on its chords, its notes not heard", bp.genre.replace('_', " ")));
    };
    let notes = on_grid(a, &without_overtones(heard));
    if notes.is_empty() {
        return None;
    }
    let mut p = part(KEYS, Role::Keys, genre, "epiano", &notes)?;
    p.notes = per_section(&bp.form, &notes);
    plays_in(bp, KEYS, &p.notes);
    bp.parts.push(p);
    Some(format!("the rest of it as {} notes on {KEYS}", notes.len()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::analysis::song::SectionGuess;
    use crate::audio::Stereo;
    use crate::config::SRF;
    use crate::model::SectionKind;
    use crate::stems::Stems;
    use crate::theory::scale::{Key, Mode};

    const BAR: f64 = 2.5;
    const BEAT: f64 = BAR / 4.0;

    /// eight bars at 96 BPM in A minor, `i VI III VII` a bar each, a verse and then a hook, its kick on 1 and 3
    fn analysis() -> Analysis {
        let numerals: Vec<String> = ["i", "VI", "III", "VII"].iter().cycle().take(8).map(|s| s.to_string()).collect();
        let section = |start_bar, energy, kind| SectionGuess {
            start_bar,
            bars: 4,
            energy,
            kind,
            kick: true,
            bass: true,
            hats: true,
            harmony: true,
        };
        let on = |steps: &[usize]| (0..16).map(|s| if steps.contains(&s) { 1.0 } else { 0.0 }).collect();
        Analysis {
            bpm: 96.0,
            key: Key::new(9, Mode::Minor),
            key_confidence: 0.9,
            swing: 0.5,
            pulse_clarity: 0.8,
            first_downbeat: 0.0,
            bar_secs: BAR,
            bars: 8,
            chords: Vec::new(),
            numerals: numerals.clone(),
            progression_a: numerals[..4].to_vec(),
            progression_b: Vec::new(),
            bars_per_chord: 1,
            sections: vec![section(0, 0.5, SectionKind::Verse), section(4, 0.9, SectionKind::Hook)],
            bar_chroma: Vec::new(),
            kick_steps: on(&[0, 8]),
            snare_steps: on(&[4, 12]),
            hat_steps: on(&[0, 2, 4, 6, 8, 10, 12, 14]),
            bass_style: "sustain".into(),
            steady: true,
            bar_starts: Vec::new(),
        }
    }

    /// a kick on 1 and 3 (in the hook on the "and" of 3 too) struck 10 ms late, a snare on 2 and 4, a hat on the
    /// eighths
    fn kit() -> Vec<drums::Hit> {
        let mut out = Vec::new();
        for bar in 0..8 {
            let at = |beat: f64| bar as f64 * BAR + beat * BEAT;
            let kicks: &[f64] = if bar < 4 { &[0.0, 2.0] } else { &[0.0, 2.0, 2.5] };
            out.extend(kicks.iter().map(|&k| drums::Hit { t: at(k) + 0.01, kind: Kind::Kick, level: 1.0 }));
            out.extend([1.0, 3.0].map(|s| drums::Hit { t: at(s), kind: Kind::Snare, level: 0.8 }));
            out.extend((0..8).map(|h| drums::Hit { t: at(h as f64 * 0.5), kind: Kind::Hat, level: 0.5 }));
        }
        out.sort_by(|x, y| x.t.total_cmp(&y.t));
        out
    }

    fn note(start: f64, secs: f64, pitch: i32, sure: f32) -> Heard {
        Heard { start, end: start + secs, pitch, sure }
    }

    fn heard() -> Played {
        // A2 on every downbeat for a beat, its octave faintly over it; A minor's triad held through every bar, E's
        // octave faintly over it; and a tune sung in the hook
        let bass =
            (0..8).flat_map(|b| [note(b as f64 * BAR, BEAT, 45, 0.8), note(b as f64 * BAR + 0.01, BEAT, 57, 0.3)]);
        let keys = (0..8).flat_map(|b| {
            let t = b as f64 * BAR;
            [note(t, 2.4, 57, 0.7), note(t, 2.4, 60, 0.7), note(t, 2.4, 64, 0.7), note(t, 2.4, 76, 0.3)]
        });
        let tune = (0..8).map(|i| Note { t: 16.0 + i as f64 * 2.0, d: 1.0, p: 69 + i % 3, v: 0.8, lock: None });
        Played {
            drums: Some(kit()),
            bass: Some(bass.collect()),
            rest: Some(keys.collect()),
            melody: Some(tune.collect()),
        }
    }

    /// stems of eight bars in which only a voice sings
    fn sung() -> Stems {
        let n = (8.0 * BAR * SRF) as usize;
        let voice: Vec<f32> =
            (0..n).map(|i| 0.3 * (std::f64::consts::TAU * 440.0 * i as f64 / SRF).sin() as f32).collect();
        Stems {
            vocals: Stereo::from_mono(&voice),
            drums: Stereo::zeros(n),
            bass: Stereo::zeros(n),
            other: Stereo::zeros(n),
        }
    }

    fn dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("claudius-rebuild-{name}-{}", std::process::id()))
    }

    #[test]
    fn a_rebuild_plays_each_section_as_the_recording_does_on_a_progression_of_its_own() {
        let (a, s) = (analysis(), sung());
        let x = s.vocals.clone();
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: Some(&s) };
        let dir = dir("parts");
        let g = crate::genres::get("house").unwrap();
        let bp = rebuild_with(&src, &heard(), &g, 3, &Options::default(), None, &dir).unwrap();
        assert_eq!(bp.bpm, 96.0);
        let form: Vec<(&str, &str, &str)> =
            bp.form.iter().map(|s| (s.name.as_str(), s.prog.as_str(), s.variant.as_str())).collect();
        assert_eq!(form, [("verse", "A", "verse"), ("hook", "B", "hook")]);
        assert_eq!(bp.harmony.progressions["B"], ["i", "VI", "III", "VII"]);

        // the kick on 1 and 3, and on the "and" of 3 too in the hook, 10 ms behind the beat
        let kick = bp.drums.iter().find(|d| d.role() == "kick").unwrap();
        let steps =
            |v: &str| kick.patterns[v].iter().map(|h| (h.t * 4.0).round() as usize % 16).collect::<BTreeSet<_>>();
        assert_eq!((steps("verse"), steps("hook")), ([0, 8].into(), [0, 8, 10].into()));
        assert!((kick.late_ms - 10.0).abs() < 1.0, "{}", kick.late_ms);
        assert_eq!(bp.drums.len(), 3, "{:?}", bp.drums.iter().map(|d| &d.name).collect::<Vec<_>>());

        // A2 on every downbeat and the triad in every bar, their overtones gone, from the song's first bar on
        let p = compose::arrange(&bp);
        let track = |n: &str| p.tracks.iter().find(|t| t.name == n).unwrap_or_else(|| panic!("{n} plays"));
        let bass: Vec<(i64, i32)> = track(BASS).notes.iter().map(|n| (n.t.round() as i64, n.p)).collect();
        assert_eq!(bass, (0..8).map(|b| (b * 4, 45)).collect::<Vec<_>>());
        let keys: BTreeSet<i32> = track(KEYS).notes.iter().map(|n| n.p).collect();
        assert_eq!((keys, track(KEYS).notes.len()), ([57, 60, 64].into(), 24));

        // the tune written out in the hook, playing nowhere; the voice itself on a track of its own in both
        let melody = bp.parts.iter().find(|p| p.name == MELODY).unwrap();
        assert_eq!(
            melody.notes["B"].iter().map(|n| n.t).collect::<Vec<_>>(),
            (0..8).map(|i| i as f64 * 2.0).collect::<Vec<_>>()
        );
        for s in &bp.form {
            assert!(!s.layers.iter().any(|l| l == MELODY), "{:?}", s.layers);
            assert!([BASS, KEYS, remix::VOICE].iter().all(|l| s.layers.iter().any(|x| x == l)), "{:?}", s.layers);
        }
        assert!(bp.loops.iter().any(|l| l.name == remix::VOICE && l.role == "vocal"));
        assert!(
            bp.lineage[1].starts_with("rebuilt from 'my song.wav': its 2 sections bar for bar"),
            "{:?}",
            bp.lineage
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_the_ears_the_genres_parts_play_on_its_chords_and_without_its_stems_its_drums_habits() {
        let a = analysis();
        let x = Stereo::zeros((8.0 * BAR * SRF) as usize);
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let dir = dir("habits");
        let bp = rebuild_with(
            &src,
            &Played::default(),
            &crate::genres::get("house").unwrap(),
            3,
            &Options::default(),
            None,
            &dir,
        )
        .unwrap();
        let kick = bp.drums.iter().find(|d| d.role() == "kick").unwrap();
        let steps: BTreeSet<usize> = kick.patterns["main"].iter().map(|h| (h.t * 4.0).round() as usize % 16).collect();
        assert_eq!(steps, [0, 8].into(), "every bar");
        assert!(bp.form.iter().all(|s| s.variant == "main"));
        for name in [BASS, KEYS] {
            assert!(bp.parts.iter().any(|p| p.name == name && p.notes.is_empty()), "{name} writes its own notes");
            assert!(bp.form.iter().all(|s| s.layers.iter().any(|l| l == name)));
        }
        let p = compose::arrange(&bp);
        let bass = &p.tracks.iter().find(|t| t.name == BASS).unwrap().notes;
        assert!(bass.iter().any(|n| n.t < 16.0) && bass.iter().any(|n| n.t >= 16.0), "{bass:?}");
        assert!(bp.loops.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_far_under_its_stems_loud_ones_is_another_instruments_left_in_it() {
        let n = (2.0 * SRF) as usize;
        let x: Vec<f32> = (0..n)
            .map(|i| {
                let gain = if i < n / 2 { 0.5 } else { 0.01 };
                gain * (std::f64::consts::TAU * 110.0 * i as f64 / SRF).sin() as f32
            })
            .collect();
        let kept = own(&Stereo::from_mono(&x), vec![note(0.0, 1.0, 45, 0.8), note(1.0, 1.0, 45, 0.8)]);
        assert_eq!(kept.iter().map(|n| n.start).collect::<Vec<_>>(), [0.0]);
    }

    #[test]
    fn a_bass_plays_the_surest_of_notes_struck_together_one_at_a_time_and_an_overtone_is_no_note() {
        let notes =
            [note(0.0, 1.0, 45, 0.8), note(0.02, 1.0, 52, 0.5), note(0.01, 1.0, 57, 0.3), note(0.5, 1.0, 47, 0.7)];
        let one: Vec<(i32, f64, f64)> = one_at_a_time(&notes).iter().map(|n| (n.pitch, n.start, n.end)).collect();
        assert_eq!(one, [(45, 0.0, 0.5), (47, 0.5, 1.5)]);
        let louder = [note(0.0, 1.0, 45, 0.4), note(0.01, 1.0, 57, 0.8)];
        assert_eq!(without_overtones(&louder).len(), 2, "an octave louder than the note under it is played");
        assert_eq!(lateness(&[0.016, 2.016, 4.0 + 0.016], 96.0).round(), 10.0);
        assert_eq!(lateness(&[0.002, 2.0], 96.0), 0.0);
    }
}
