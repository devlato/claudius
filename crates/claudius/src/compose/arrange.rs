//! `arrange`: a blueprint expanded into the tracks, notes and clips the renderer plays.

use super::*;
use crate::theory::Lock;
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
            let centre = (part.register.0 + part.register.1) / 2;
            bass::bassline(&part.style, &spans, &bp.key, &kicks, centre, &mut rng)
        }
        Role::Chords | Role::Pad | Role::Keys => {
            melody::chord_part(&spans, &part.style, part.voicing, part.register, part.voices.max(2), &mut rng)
        }
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
/// bars at least, and for notes written out as many cycles as they run over
fn cycle_period(bp: &Blueprint, part: &Part, prog: &str, notes: &[Note]) -> f64 {
    let mut p = cycle_beats(&cycle_spans(bp, prog));
    if part.role == Role::Lead {
        while p < 16.0 {
            p *= 2.0;
        }
    }
    if part.notes.contains_key(prog) {
        let (cycle, end) = (p, notes.iter().map(|x| x.t + x.d).fold(0.0, f64::max));
        while p + 1e-9 < end {
            p += cycle;
        }
    }
    p
}

/// the kick's main pattern and its length in bars, which a bass follows
fn kick_of(bp: &Blueprint) -> (Vec<Hit>, u32) {
    let kick = bp.drums.iter().find(|d| d.role() == "kick");
    (kick.and_then(|k| k.patterns.get("main")).cloned().unwrap_or_default(), kick.map_or(4, |k| k.pattern_bars))
}

/// A part's cycle over each progression its sections play, in the order they first play it: the notes, the beats they
/// come round in, and whether they are written out (else the theory engine writes them).
pub fn cycles(bp: &Blueprint, part: &Part) -> Vec<(String, Vec<Note>, f64, bool)> {
    let (kick_main, kick_bars) = kick_of(bp);
    let mut progs: Vec<&str> = Vec::new();
    for s in bp.form.iter().filter(|s| s.layers.contains(&part.name)) {
        if !progs.contains(&s.prog.as_str()) {
            progs.push(&s.prog);
        }
    }
    progs
        .into_iter()
        .map(|prog| {
            let notes = part_cycle(bp, part, prog, &kick_main, kick_bars);
            let period = cycle_period(bp, part, prog, &notes);
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
    let fill_lane = ["snare", "clap"]
        .iter()
        .find_map(|n| bp.drums.iter().zip(&roles).find(|(_, r)| *r == n))
        .map(|(d, _)| d.name.clone());

    let mut tracks: Vec<Track> = Vec::new();
    // drum lanes
    for (lane, role) in bp.drums.iter().zip(&roles) {
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
                    if s.fills {
                        let mut r = Rng::new(bp.seed ^ hash_str(&lane.name) ^ si as u64);
                        let mut phrase = a + 32.0;
                        while phrase <= b + 1e-9 {
                            let fill_at = phrase - if r.chance(0.5) { 1.0 } else { 2.0 };
                            if is_fill_lane {
                                seg.retain(|n| n.t < fill_at || n.t >= phrase);
                                let kind = *r.pick(&["roll", "groove", "triplets", "groove"]);
                                for h in rhythm::fill(kind, phrase - fill_at, &mut r) {
                                    seg.push(Note {
                                        t: fill_at + h.t,
                                        d: 0.25,
                                        p: 60 + lane.pitch,
                                        v: h.v * 0.85,
                                        lock: None,
                                    });
                                }
                            } else if is_kick && r.chance(0.35) {
                                seg.retain(|n| n.t < phrase - 0.5 || n.t >= phrase);
                            }
                            phrase += 32.0;
                        }
                    }
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
            name: lane.name.clone(),
            role: role.clone(),
            sound: lane.sound.clone(),
            notes,
            clips: Vec::new(),
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
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
            let info = &sections[si];
            let (a, b) = (info.start_bar as f64 * 4.0, (info.start_bar + info.bars) as f64 * 4.0);
            let (cyc, period) = cycles
                .entry(s.prog.clone())
                .or_insert_with(|| {
                    let n = part_cycle(bp, part, &s.prog, &kick_main, kick_bars);
                    let p = cycle_period(bp, part, &s.prog, &n);
                    (n, p)
                })
                .clone();
            notes.extend(tile(&cyc, period, a, b));
        }
        tracks.push(Track {
            name: part.name.clone(),
            role: part.role.name().to_string(),
            sound: part.sound.clone(),
            notes,
            clips: Vec::new(),
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
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
            name: lp.name.clone(),
            role: lp.role.clone(),
            sound: lp.sound.clone(),
            notes: Vec::new(),
            clips,
            transpose: lp.transpose,
            chop: lp.chop.clone(),
            slice_beats: lp.slice_beats,
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
                name: name.into(),
                role: role.into(),
                sound,
                notes,
                clips: Vec::new(),
                transpose: 0,
                chop: Vec::new(),
                slice_beats: 0.0,
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
