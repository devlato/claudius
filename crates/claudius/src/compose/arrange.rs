//! `arrange`: a blueprint expanded into the tracks, notes and clips the renderer plays.

use super::*;
use crate::theory::Lock;
use crate::theory::drummer;
use crate::theory::harmonize;
use crate::theory::rhythm::Groove;

/// chord spans of one cycle of progression `name` starting at beat 0
pub fn cycle_spans(bp: &Blueprint, name: &str) -> Vec<ChordSpan> {
    let prog =
        bp.harmony.progressions.get(name).or_else(|| bp.harmony.progressions.get("A")).cloned().unwrap_or_default();
    let chords = progression::chords(&prog, &bp.key);
    let len = bp.harmony.bars_per_chord.max(1) as f64 * 4.0;
    chords.iter().enumerate().map(|(i, c)| ChordSpan { start: i as f64 * len, len, chord: *c }).collect()
}

fn cycle_beats(spans: &[ChordSpan]) -> f64 {
    spans.last().map_or(16.0, |s| s.start + s.len)
}

/// the notes of one cycle of a part over progression `prog`
pub fn part_cycle(bp: &Blueprint, part: &Part, prog: &str, kick_main: &[Hit], kick_bars: u32) -> Vec<Note> {
    if let Some(n) = part.notes.get(prog) {
        return n.clone();
    }
    let spans = cycle_spans(bp, prog);
    let beats = cycle_beats(&spans);
    let mut rng = Rng::new(part.seed ^ hash_str(prog));
    match part.role {
        Role::Bass => {
            let period = (kick_bars.max(1) * 4) as f64;
            let mut kicks = Vec::new();
            let mut off = 0.0;
            while off < beats {
                kicks.extend(kick_main.iter().map(|h| h.t + off).filter(|t| *t < beats));
                off += period;
            }
            bass::write(&part.style, &spans, &bp.key, &kicks, part.register, part.seed, hash_str(prog))
        }
        Role::Chords | Role::Pad | Role::Keys => match crate::theory::comp::idiom(&part.style) {
            Some(i) => crate::theory::comp::comp(
                i,
                &spans,
                part.voicing,
                part.register,
                part.voices.max(2),
                part.seed,
                hash_str(prog),
            ),
            None => melody::chord_part(&spans, &part.style, part.voicing, part.register, part.voices.max(2), &mut rng),
        },
        Role::Lead => {
            let bars = (beats / 4.0).round() as u32;
            let sync = rng.chance(0.5);
            melody::lead(&spans, &bp.key, 0.0, bars.clamp(4, 8), part.density, sync, part.register, &mut rng)
                .into_iter()
                .filter(|n| n.t < beats.max(16.0))
                .collect()
        }
        Role::Arp => melody::arp(
            &spans,
            0.0,
            beats,
            &part.style,
            if part.rate > 0.0 { part.rate } else { 0.25 },
            2,
            part.register,
            0.8,
            &mut rng,
        ),
    }
}

/// the beats a part's cycle over `prog` comes round in: its progression's, doubled for a lead until a phrase of four
/// bars at least, and for notes written out as many cycles as it takes for each to start within them (a note may ring
/// on into the next time round, as the theory engine's do)
fn cycle_period(bp: &Blueprint, part: &Part, prog: &str, notes: &[Note]) -> f64 {
    let mut p = cycle_beats(&cycle_spans(bp, prog));
    if part.role == Role::Lead {
        while p < 16.0 {
            p *= 2.0;
        }
    }
    if part.notes.contains_key(prog) {
        let (cycle, last) = (p, notes.iter().map(|x| x.t).fold(f64::NEG_INFINITY, f64::max));
        while p <= last + 1e-9 {
            p += cycle;
        }
    }
    p
}

/// one cycle of what `part` plays over progression `prog`, and the beats it comes round in: chords under the part it
/// harmonizes (`harmonized`), the notes of the part it doubles, moved by its octaves (`Part::source`), or its own
fn sounding(bp: &Blueprint, part: &Part, prog: &str, kick_main: &[Hit], kick_bars: u32) -> (Vec<Note>, f64) {
    if let Some(melody) = harmonized(bp, part, prog) {
        let (line, period) = sounding(bp, melody, prog, kick_main, kick_bars);
        let spans = cycle_spans(bp, prog);
        let song = harmonize::Song { spans: &spans, cycle: cycle_beats(&spans), key: &bp.key };
        let play = harmonize::Play {
            style: &part.style,
            voicing: part.voicing,
            register: part.register,
            voices: part.voices.max(2),
            seed: part.seed,
            stream: hash_str(prog),
        };
        let mut rng = Rng::new(part.seed ^ hash_str(prog));
        return (harmonize::harmony(&line, period, &song, &play, &mut rng), period);
    }
    let (source, semis) = part.source(bp);
    let notes = part_cycle(bp, source, prog, kick_main, kick_bars);
    let period = cycle_period(bp, source, prog, &notes);
    (notes.into_iter().map(|n| Note { p: (n.p + semis).clamp(0, 127), ..n }).collect(), period)
}

/// the part whose melody `part` harmonizes over `prog`: none when it writes its own notes out there, or when that part
/// is gone or harmonizes another itself
fn harmonized<'a>(bp: &'a Blueprint, part: &Part, prog: &str) -> Option<&'a Part> {
    let name = part.harmonizes.as_deref().filter(|_| !part.notes.contains_key(prog))?;
    bp.parts.iter().find(|p| p.name == name && p.name != part.name && p.harmonizes.is_none())
}

/// the kick's main pattern and its length in bars, which a bass follows
fn kick_of(bp: &Blueprint) -> (Vec<Hit>, u32) {
    let kick = bp.drums.iter().find(|d| d.role() == "kick");
    (kick.and_then(|k| k.patterns.get("main")).cloned().unwrap_or_default(), kick.map_or(4, |k| k.pattern_bars))
}

/// A part's cycle over each progression its sections play, in the order they first play it, then over those it has
/// notes written out for and plays nowhere now: the notes, the beats they come round in, and whether they are written
/// out (else the theory engine writes them).
pub fn cycles(bp: &Blueprint, part: &Part) -> Vec<(String, Vec<Note>, f64, bool)> {
    let (kick_main, kick_bars) = kick_of(bp);
    let mut progs: Vec<&str> = Vec::new();
    for s in bp.form.iter().filter(|s| s.layers.contains(&part.name)) {
        if !progs.contains(&s.prog.as_str()) {
            progs.push(&s.prog);
        }
    }
    for prog in part.notes.keys() {
        if !progs.contains(&prog.as_str()) && bp.harmony.progressions.contains_key(prog) {
            progs.push(prog);
        }
    }
    progs
        .into_iter()
        .map(|prog| {
            let (notes, period) = sounding(bp, part, prog, &kick_main, kick_bars);
            (prog.to_string(), notes, period, part.notes.contains_key(prog))
        })
        .collect()
}

fn tile(notes: &[Note], period: f64, start: f64, end: f64) -> Vec<Note> {
    let mut out = Vec::new();
    if notes.is_empty() || period <= 0.0 {
        return out;
    }
    let mut off = start;
    while off < end - 1e-9 {
        for n in notes {
            let t = n.t + off;
            if t >= start && t < end - 1e-9 {
                out.push(Note { t, d: n.d.min(end - t), ..*n });
            }
        }
        off += period;
    }
    out
}

/// A drum lane's pattern over a section, from beat `a` to `b`, a time round lasting the lane's `period` (its own
/// length at its own speed): each time round, every hit as its trig says (`rhythm::Trig`), and every stroke in the
/// lane's groove. A condition counts the times round from the section's start; a chance is drawn afresh each time
/// round, the same in every render of the song (its seed, with the lane and section `si`); a ratchet or a flam is
/// its strokes, and the locks go with each.
fn lane_notes(
    bp: &Blueprint,
    lane: &DrumLane,
    hits: &[Hit],
    groove: Option<&Groove>,
    (a, b): (f64, f64),
    si: usize,
) -> Vec<Note> {
    let (seed, bpm) = (bp.seed, bp.bpm);
    let mut out = Vec::new();
    let period = lane.period();
    if hits.is_empty() || period <= 0.0 {
        return out;
    }
    let speed = lane.speed.max(0.125) as f64;
    let cut = lane.steps() as f64 * 0.25;
    let hits: Vec<Hit> = hits.iter().filter(|h| h.t < cut - 1e-9).map(|h| Hit { t: h.t / speed, ..*h }).collect();
    let step = 0.25 / speed;
    let feel = |t: f64, v: f32| groove.map_or((t, v), |g| g.apply(t, v, lane.groove_amount));
    let rounds = ((b - a) / period - 1e-9).ceil().max(1.0) as usize;
    let lane_seed = seed ^ hash_str(&lane.name) ^ (si as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for k in 0..rounds {
        let off = a + k as f64 * period;
        for (i, h) in hits.iter().enumerate() {
            let t = off + h.t;
            if t >= b - 1e-9 {
                continue;
            }
            let Some(tr) = h.trig else {
                let (t, v) = feel(t, h.v);
                out.push(Note { t, d: step, p: 60 + lane.pitch, v, lock: None });
                continue;
            };
            if !tr.cond.plays(k, rounds) {
                continue;
            }
            if tr.prob < 1.0 && !Rng::new(lane_seed ^ ((k as u64) << 24) ^ i as u64).chance(tr.prob as f64) {
                continue;
            }
            let lock = Lock { nudge: tr.nudge as f64 * 0.25, decay: tr.decay, tone: tr.tone, pan: tr.pan };
            let lock = (lock != Lock::default()).then_some(lock);
            let p = 60 + lane.pitch + tr.pitch as i32;
            // the step takes the groove, and its strokes go with it: a ratchet keeps its order
            let (t, v) = feel(t, h.v);
            for (st, v) in tr.strokes(t, v, bpm, step) {
                if st >= 0.0 && st < b - 1e-9 {
                    out.push(Note { t: st, d: step / tr.ratchet.max(1) as f64, p, v, lock });
                }
            }
        }
    }
    out
}

/// every tuner's harmony, from its scale and the song's key and chords, in every chain the song plays through
fn harmonize(
    tracks: &mut [Track],
    mix: &mut MixPlan,
    key: &crate::theory::scale::Key,
    chords: &[crate::theory::ChordSpan],
) {
    let harmony = |s: &crate::voice::tune::TuneScale| s.harmony(key, chords);
    let chains = tracks.iter_mut().map(|t| &mut t.mix.fx).chain(mix.buses.values_mut()).chain([&mut mix.master.fx]);
    for chain in chains {
        chain.iter_mut().for_each(|e| e.harmonize(&harmony));
    }
    mix.reverb.harmonize(&harmony);
    mix.delay.harmonize(&harmony);
}

/// expand a blueprint into the arranged project
/// values by section as curves through the form as it is laid out (`Param::resolve`), in every chain the song plays
/// through
fn resolve_sections(tracks: &mut [Track], mix: &mut MixPlan, sections: &[SectionInfo]) {
    let spans: Vec<crate::dsp::env::Span> = sections
        .iter()
        .map(|s| crate::dsp::env::Span {
            name: &s.name,
            kind: s.kind.name(),
            start: s.start_bar as f64 * 4.0,
            end: (s.start_bar + s.bars) as f64 * 4.0,
        })
        .collect();
    let mut resolve = |e: &mut Effect| e.params_mut(&mut |p| *p = p.resolve(&spans));
    let chains = tracks.iter_mut().map(|t| &mut t.mix.fx).chain(mix.buses.values_mut()).chain([&mut mix.master.fx]);
    for chain in chains {
        chain.iter_mut().for_each(&mut resolve);
    }
    resolve(&mut mix.reverb);
    resolve(&mut mix.delay);
}

/// the lane that plays the song's backbeat, and its rolls and fills: its snare, else its clap
fn backbeat<'a>(bp: &'a Blueprint, roles: &[String]) -> Option<&'a DrumLane> {
    ["snare", "clap"].iter().find_map(|n| bp.drums.iter().zip(roles).find(|(_, r)| *r == n)).map(|(d, _)| d)
}

/// The drummer who ends the song's phrases: the one of the genre its backbeat was written as, else the song's.
pub fn drummer(bp: &Blueprint) -> &'static drummer::Drummer {
    let roles: Vec<String> = bp.drums.iter().map(|d| d.role()).collect();
    drummer::of(backbeat(bp, &roles).and_then(|d| d.genre.as_deref()).unwrap_or(&bp.genre))
}

pub fn arrange(bp: &Blueprint) -> Project {
    let mut sections = Vec::new();
    let mut chords = Vec::new();
    let mut bar = 0u32;
    for s in &bp.form {
        sections.push(SectionInfo {
            name: s.name.clone(),
            kind: s.kind,
            start_bar: bar,
            bars: s.bars,
            energy: s.energy,
        });
        let spans = cycle_spans(bp, &s.prog);
        let period = cycle_beats(&spans);
        let (a, b) = (bar as f64 * 4.0, (bar + s.bars) as f64 * 4.0);
        let mut off = a;
        while off < b - 1e-9 {
            for c in &spans {
                let t = c.start + off;
                if t < b - 1e-9 {
                    chords.push(ChordSpan { start: t, len: c.len.min(b - t), chord: c.chord });
                }
            }
            off += period;
        }
        bar = bar.saturating_add(s.bars);
    }
    let total_bars = bar;
    let roles: Vec<String> = bp.drums.iter().map(|d| d.role()).collect();
    let (kick_main, kick_bars) = kick_of(bp);
    let fill_lane = backbeat(bp, &roles).map(|d| d.name.clone());
    // each section's phrases end the same way on every lane that plays in it
    let drummer = drummer(bp);
    let endings: Vec<Vec<drummer::Ending>> = bp
        .form
        .iter()
        .enumerate()
        .map(|(si, s)| {
            if !s.fills {
                return Vec::new();
            }
            let lanes: Vec<(String, String)> = bp
                .drums
                .iter()
                .zip(&roles)
                .map(|(d, r)| if s.layers.contains(&d.name) { (d.name.clone(), r.clone()) } else { Default::default() })
                .collect();
            let mut r = Rng::new(bp.seed ^ hash_str("drummer") ^ si as u64);
            let (start, bars) = (sections[si].start_bar as f64 * 4.0, sections[si].bars);
            // the section's drums stop at its end, or at the gap before the next; a build's roll ends it
            let end = start + bars as f64 * 4.0;
            let last = (!s.build).then_some(if s.gap { end - 1.0 } else { end });
            drummer::endings(drummer, &lanes, start, bars, last, &mut r)
        })
        .collect();

    let mut tracks: Vec<Track> = Vec::new();
    // drum lanes
    for (li, (lane, role)) in bp.drums.iter().zip(&roles).enumerate() {
        let is_kick = role == "kick";
        let groove = lane.groove.as_deref().and_then(|g| bp.grooves.get(g).cloned().or_else(|| rhythm::groove(g)));
        let mut notes = Vec::new();
        for (si, s) in bp.form.iter().enumerate() {
            let info = &sections[si];
            let (a, b) = (info.start_bar as f64 * 4.0, (info.start_bar + info.bars) as f64 * 4.0);
            let on = s.layers.contains(&lane.name);
            let is_fill_lane = fill_lane.as_deref() == Some(lane.name.as_str());
            let roll_len = if s.build { (s.bars.min(8) as f64 * 4.0 / 2.0).clamp(4.0, 16.0) } else { 0.0 };
            if on {
                let pat = lane.patterns.get(&s.variant).or_else(|| lane.patterns.get("main"));
                if let Some(p) = pat {
                    let mut seg = lane_notes(bp, lane, p, groove.as_ref(), (a, b), si);
                    // builds: the fill lane turns into a snare roll; the kick drops out for the last bar
                    if s.build && is_fill_lane {
                        seg.retain(|n| n.t < b - roll_len);
                    }
                    if s.build && is_kick {
                        seg.retain(|n| n.t < b - 4.0);
                    }
                    // each phrase's ending: what the drummer plays on this lane there, or the lane left out
                    for e in &endings[si] {
                        let mine: Vec<&drummer::Play> = e.plays.iter().filter(|p| p.lane == li).collect();
                        for p in &mine {
                            seg.retain(|n| n.t < p.from - 1e-9 || n.t >= e.at - 1e-9);
                        }
                        if e.stops.contains(&li) {
                            seg.retain(|n| n.t < e.at - e.span - 1e-9 || n.t >= e.at - 1e-9);
                        }
                        for p in mine {
                            for st in &p.strokes {
                                let (t, v) =
                                    groove.as_ref().map_or((st.t, st.v), |g| g.apply(st.t, st.v, lane.groove_amount));
                                seg.push(Note { t, d: p.grid, p: 60 + lane.pitch + st.pitch, v: v * 0.9, lock: None });
                            }
                        }
                    }
                    seg.sort_by(|x, y| x.t.total_cmp(&y.t));
                    notes.extend(seg);
                }
            }
            if s.build && is_fill_lane {
                for h in rhythm::snare_roll(roll_len) {
                    notes.push(Note { t: b - roll_len + h.t, d: 0.25, p: 60 + lane.pitch, v: h.v, lock: None });
                }
            }
        }
        tracks.push(Track {
            late_ms: lane.late_ms,
            name: lane.name.clone(),
            role: role.clone(),
            sound: lane.sound.clone(),
            notes,
            clips: Vec::new(),
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            choke: lane.choke.clone(),
            // a lane in a groove takes its timing from it: straight, unless it has a swing of its own
            swing: lane.swing.or(groove.as_ref().map(|_| 0.5)),
            mix: TrackMix {
                bus: if lane.mix.bus == "music" { "drums".into() } else { lane.mix.bus.clone() },
                ..lane.mix.clone()
            },
        });
    }
    // melodic parts: one cycle per progression, tiled over the sections that use it
    for part in &bp.parts {
        let mut cycles: BTreeMap<String, (Vec<Note>, f64)> = BTreeMap::new();
        let mut notes = Vec::new();
        for (si, s) in bp.form.iter().enumerate() {
            if !s.layers.contains(&part.name) {
                continue;
            }
            // a harmony plays where its melody does
            if harmonized(bp, part, &s.prog).is_some_and(|m| !s.layers.contains(&m.name)) {
                continue;
            }
            let info = &sections[si];
            let (a, b) = (info.start_bar as f64 * 4.0, (info.start_bar + info.bars) as f64 * 4.0);
            let (cyc, period) = cycles
                .entry(s.prog.clone())
                .or_insert_with(|| sounding(bp, part, &s.prog, &kick_main, kick_bars))
                .clone();
            notes.extend(tile(&cyc, period, a, b));
        }
        tracks.push(Track {
            late_ms: part.late_ms,
            name: part.name.clone(),
            role: part.role.name().to_string(),
            sound: part.sound.clone(),
            notes,
            clips: Vec::new(),
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            choke: None,
            swing: None,
            mix: part.mix.clone(),
        });
    }
    // loops: one clip per section they play in, each from the loop's start; a take goes on from the section it starts
    // in (or the first that plays it, when that one is gone), as one clip while the sections playing it follow each
    // other
    for lp in &bp.loops {
        let start = lp.from.as_ref().map(|from| {
            let first = bp.form.iter().position(|s| s.name == *from);
            first
                .or_else(|| bp.form.iter().position(|s| s.layers.contains(&lp.name)))
                .map_or(0, |i| sections[i].start_bar)
        });
        let mut clips: Vec<Clip> = Vec::new();
        for (si, s) in bp.form.iter().enumerate() {
            if !s.layers.contains(&lp.name) {
                continue;
            }
            let info = &sections[si];
            let (t, d) = (info.start_bar as f64 * 4.0, info.bars as f64 * 4.0);
            let clip = |offset: f64, once: bool| Clip { t, d, offset, gain_db: 0.0, fade_in: 0.0, fade_out: 0.0, once };
            match start {
                None => clips.push(clip(0.0, false)),
                Some(bar) if info.start_bar < bar => {}
                Some(bar) => match clips.last_mut() {
                    Some(c) if (c.t + c.d - t).abs() < 1e-9 => c.d += d,
                    _ => clips.push(clip(t - bar as f64 * 4.0, true)),
                },
            }
        }
        tracks.push(Track {
            late_ms: 0.0,
            name: lp.name.clone(),
            role: lp.role.clone(),
            sound: lp.sound.clone(),
            notes: Vec::new(),
            clips,
            transpose: lp.transpose,
            chop: lp.chop.clone(),
            slice_beats: lp.slice_beats,
            retime: lp.retime,
            choke: None,
            swing: None,
            mix: lp.mix.clone(),
        });
    }
    // pattern tracks: every section plays its pattern from the pattern's first cycle; one track per sound in it
    for pt in &bp.patterns {
        let Ok(pat) = crate::script::eval_pattern(&pt.code) else { continue };
        let mut by_sound: BTreeMap<String, Vec<Note>> = BTreeMap::new();
        for (si, s) in bp.form.iter().enumerate() {
            if !s.layers.contains(&pt.name) {
                continue;
            }
            let info = &sections[si];
            let ctx = notes::Context {
                key: bp.key,
                progressions: &bp.harmony.progressions,
                bars_per_chord: bp.harmony.bars_per_chord,
                prog: &s.prog,
            };
            for p in notes::place(&pat, 0, info.bars, info.start_bar as f64 * 4.0, &ctx).unwrap_or_default() {
                by_sound.entry(p.sound).or_default().push(p.note);
            }
        }
        let many = pt.sounds.len() > 1;
        for (label, notes) in by_sound {
            let Some(sound) = pt.sounds.get(&label).cloned() else { continue };
            let mut ps: Vec<i32> = notes.iter().map(|n| n.p).collect();
            ps.sort();
            let median = ps.get(ps.len() / 2).copied().unwrap_or(60) as f64;
            tracks.push(Track {
                late_ms: 0.0,
                name: if many { format!("{}.{label}", pt.name) } else { pt.name.clone() },
                role: if pt.role.is_empty() {
                    crate::script::role_of(&label, pat.kind, median)
                } else {
                    pt.role.clone()
                },
                sound,
                notes,
                clips: Vec::new(),
                transpose: 0,
                chop: Vec::new(),
                slice_beats: 0.0,
                retime: false,
                choke: None,
                swing: None,
                mix: pt.mix.clone(),
            });
        }
    }
    // transitions
    let tr = &bp.transitions;
    let fxmix = TrackMix { bus: "fx".into(), ..tr.mix.clone() };
    let mut crash = Vec::new();
    let mut riser = Vec::new();
    let mut impact = Vec::new();
    let mut down = Vec::new();
    let mut reverse = Vec::new();
    for (si, s) in bp.form.iter().enumerate() {
        let info = &sections[si];
        let (a, b) = (info.start_bar as f64 * 4.0, (info.start_bar + info.bars) as f64 * 4.0);
        let next_impact = bp.form.get(si + 1).is_some_and(|n| n.impact);
        if s.impact {
            crash.push(Note { t: a, d: 4.0, p: 60, v: 1.0, lock: None });
            if tr.use_impact {
                impact.push(Note { t: a, d: 8.0, p: 60, v: 1.0, lock: None });
            }
            if tr.use_downlifter {
                down.push(Note { t: a, d: 8.0, p: 60, v: 0.8, lock: None });
            }
        }
        if matches!(s.kind, SectionKind::Drop | SectionKind::Hook) {
            let mut t = a + 32.0;
            while t < b - 1e-9 {
                crash.push(Note { t, d: 4.0, p: 60, v: 0.7, lock: None });
                t += 32.0;
            }
        }
        if s.build && tr.riser.is_some() {
            let len = (tr.riser_bars.min(s.bars) as f64 * 4.0).max(4.0);
            riser.push(Note { t: b - len, d: len, p: 60, v: 1.0, lock: None });
        }
        if next_impact && tr.use_reverse_crash {
            reverse.push(Note { t: b - 2.0, d: 2.0, p: 60, v: 0.8, lock: None });
        }
    }
    let mut push_fx = |name: &str, sound: Sound, notes: Vec<Note>, role: &str| {
        if !notes.is_empty() {
            tracks.push(Track {
                late_ms: 0.0,
                name: name.into(),
                role: role.into(),
                sound,
                notes,
                clips: Vec::new(),
                transpose: 0,
                chop: Vec::new(),
                slice_beats: 0.0,
                retime: false,
                choke: None,
                swing: None,
                mix: fxmix.clone(),
            });
        }
    };
    push_fx("crash", tr.crash.clone(), crash, "crash");
    push_fx("riser", tr.riser.clone().unwrap_or(Sound::Fx { fx: "riser".into() }), riser, "riser");
    push_fx("impact", tr.impact.clone().unwrap_or(Sound::Fx { fx: "impact".into() }), impact, "impact");
    push_fx("downlifter", Sound::Fx { fx: "downlifter".into() }, down, "downlifter");
    push_fx("reverse_crash", Sound::Fx { fx: "reverse_crash".into() }, reverse, "reverse");
    tracks.retain(|t| !t.mix.mute);
    if tracks.iter().any(|t| t.mix.solo) {
        tracks.retain(|t| t.mix.solo);
    }

    // gaps: a beat of silence before the next section
    for (si, s) in bp.form.iter().enumerate() {
        if !s.gap {
            continue;
        }
        let info = &sections[si];
        let b = (info.start_bar + info.bars) as f64 * 4.0;
        for t in tracks.iter_mut() {
            if matches!(t.role.as_str(), "riser" | "reverse") {
                continue;
            }
            t.notes.retain(|n| !(n.t >= b - 1.0 && n.t < b));
            for n in t.notes.iter_mut() {
                if n.t < b - 1.0 && n.t + n.d > b - 1.0 {
                    n.d = b - 1.0 - n.t;
                }
            }
        }
    }

    // bus automation: lowpass opening in intros (closing in outros) on music + drums, highpass sweep in builds
    let mut lp: Vec<(f64, f32)> = vec![(0.0, 20000.0)];
    let mut hp: Vec<(f64, f32)> = vec![(0.0, 10.0)];
    for (si, s) in bp.form.iter().enumerate() {
        let info = &sections[si];
        let (a, b) = (info.start_bar as f64 * 4.0, (info.start_bar + info.bars) as f64 * 4.0);
        if s.filter_in {
            lp.extend([(a, 350.0), (b - 0.01, 20000.0)]);
        }
        if s.kind == SectionKind::Outro && si + 1 == bp.form.len() {
            lp.extend([(a + (b - a) * 0.4, 20000.0), (b, 500.0)]);
        }
        if s.build {
            hp.extend([(a, 10.0), (b - 0.01, 900.0), (b, 10.0)]);
        }
    }
    let mut mix = bp.mix.clone();
    let automate = |chain: &mut Vec<Effect>, lp: &[(f64, f32)], hp: &[(f64, f32)]| {
        if lp.len() > 1 {
            chain.insert(
                0,
                Effect::Filter {
                    mode: FilterMode::Lp,
                    cutoff: Param::Curve(lp.to_vec()),
                    res: 0.15.into(),
                    drive: 1.0,
                    mix: 1.0.into(),
                },
            );
        }
        if hp.len() > 1 {
            chain.insert(
                0,
                Effect::Filter {
                    mode: FilterMode::Hp,
                    cutoff: Param::Curve(hp.to_vec()),
                    res: 0.1.into(),
                    drive: 1.0,
                    mix: 1.0.into(),
                },
            );
        }
    };
    automate(mix.buses.entry("music".into()).or_default(), &lp, &hp);
    automate(mix.buses.entry("bass".into()).or_default(), &lp, &hp);
    // width on the music bus, mono low end on the bass bus, a clipper shaving drum transients
    let has = |chain: &[Effect], name: &str| chain.iter().any(|e| e.name() == name);
    let music = mix.buses.entry("music".into()).or_default();
    if !has(music, "width") {
        music.push(Effect::Width { width: 1.25.into(), mono_below_hz: 160.0 });
    }
    let bassbus = mix.buses.entry("bass".into()).or_default();
    if !has(bassbus, "width") {
        bassbus.push(Effect::Width { width: 1.0.into(), mono_below_hz: 140.0 });
    }
    let drumbus = mix.buses.entry("drums".into()).or_default();
    if !has(drumbus, "saturate") {
        drumbus.push(Effect::Saturate { kind: crate::fx::SatKind::Tanh, drive_db: 2.5.into(), mix: 0.5.into() });
    }
    resolve_sections(&mut tracks, &mut mix, &sections);
    harmonize(&mut tracks, &mut mix, &bp.key, &chords);

    Project {
        title: bp.title.clone(),
        genre: bp.genre.clone(),
        bpm: bp.bpm,
        key: bp.key,
        swing: bp.swing,
        humanize: bp.humanize,
        bars: total_bars,
        seed: bp.seed,
        sections,
        chords,
        tracks,
        mix,
    }
}
