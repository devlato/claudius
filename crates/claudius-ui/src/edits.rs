//! Song edits applied to a `SongView` the way the server applies them. The mock uses this as its engine; the studio
//! uses it to show an edit at once (optimistically) while the server's own state is on its way.

use claudius_api::{Command, DrumView, PartView, SectionView, SongView, TrackView};

use crate::music::Key;

pub const SECTION_KINDS: [&str; 8] = ["intro", "verse", "groove", "build", "drop", "break", "hook", "outro"];

/// step notation of one velocity: x hit, X accent, o ghost, . rest
pub use claudius_api::{step_char, step_velocity};

pub fn steps_to_string(steps: &[f32]) -> String {
    steps.iter().map(|&v| step_char(v)).collect()
}

/// start bars, total bars and seconds after the form or the tempo changed
pub fn relayout(song: &mut SongView) {
    let mut bar = 0;
    for s in &mut song.sections {
        s.start_bar = bar;
        bar += s.bars;
    }
    song.bars = bar;
    song.seconds = bar as f64 * 240.0 / song.bpm.max(1.0);
}

fn unique_name(song: &SongView, kind: &str) -> String {
    if !song.sections.iter().any(|s| s.name == kind) {
        return kind.to_string();
    }
    (2..)
        .map(|i| format!("{kind}{i}"))
        .find(|n| !song.sections.iter().any(|s| &s.name == n))
        .unwrap_or_else(|| kind.to_string())
}

/// a new section's layers: like the nearest section of the same kind, else like its neighbour
fn layers_for(song: &SongView, kind: &str, near: usize) -> Vec<String> {
    song.sections
        .iter()
        .find(|s| s.kind == kind)
        .or_else(|| song.sections.get(near))
        .map(|s| s.layers.clone())
        .unwrap_or_else(|| song.layers.clone())
}

fn energy_for(kind: &str) -> f32 {
    match kind {
        "intro" | "outro" => 0.4,
        "break" => 0.35,
        "verse" => 0.55,
        "build" => 0.6,
        "groove" => 0.65,
        "hook" => 0.85,
        "drop" => 0.95,
        _ => 0.6,
    }
}

/// Applies a song edit. `None` when `cmd` is not a plain song edit (jobs, undo, new song, scripts ...).
pub fn apply(song: &mut SongView, cmd: &Command) -> Option<Result<String, String>> {
    Some(match cmd {
        Command::SetTempo { bpm } => {
            if !(40.0..=240.0).contains(bpm) {
                return Some(Err(format!("tempo {bpm} BPM is outside 40..240")));
            }
            song.bpm = *bpm;
            relayout(song);
            Ok(format!("tempo {bpm:.1} BPM"))
        }
        Command::SetKey { key } => match Key::parse(key) {
            Some(k) => {
                song.key = k.name();
                Ok(format!("key {}", song.key))
            }
            None => Err(format!("unknown key '{key}' (try \"F# minor\" or \"Eb dorian\")")),
        },
        Command::SetSwing { swing } => {
            song.swing = swing.clamp(0.5, 0.75);
            Ok(format!("swing {:.0}%", song.swing * 100.0))
        }
        Command::SetProgression { name, chords } => {
            if chords.is_empty() {
                return Some(Err("a progression needs at least one chord".into()));
            }
            match song.progressions.iter_mut().find(|(n, _)| n == name) {
                Some((_, c)) => *c = chords.clone(),
                None => song.progressions.push((name.clone(), chords.clone())),
            }
            Ok(format!("progression {name}: {}", chords.join(" ")))
        }
        Command::SetSection {
            index,
            name,
            bars,
            energy,
            layers,
            prog,
            variant,
            filter_in,
            build,
            gap,
            impact,
            fills,
        } => {
            let Some(s) = song.sections.get_mut(*index) else {
                return Some(Err(format!("no section {index}")));
            };
            if let Some(v) = name {
                s.name = v.clone();
            }
            if let Some(v) = bars {
                s.bars = (*v).clamp(1, 256);
            }
            if let Some(v) = energy {
                s.energy = v.clamp(0.0, 1.0);
            }
            if let Some(v) = layers {
                s.layers = v.clone();
            }
            if let Some(v) = variant {
                s.variant = v.clone();
            }
            if let Some(v) = prog {
                s.prog = v.clone();
            }
            let flags = [
                (filter_in, &mut s.filter_in),
                (build, &mut s.build),
                (gap, &mut s.gap),
                (impact, &mut s.impact),
                (fills, &mut s.fills),
            ];
            for (new, old) in flags {
                if let Some(v) = new {
                    *old = *v;
                }
            }
            let msg = format!("section {} ({} bars)", s.name, s.bars);
            relayout(song);
            Ok(msg)
        }
        Command::AddSection { after, kind, bars } => {
            // like the server: a copy of the `after` section (layers, prog, variant, energy...) of the new kind
            let at = (*after + 1).min(song.sections.len());
            let near = (*after).min(song.sections.len().saturating_sub(1));
            let like = song.sections.get(near).cloned().unwrap_or_else(|| SectionView {
                energy: energy_for(kind),
                layers: layers_for(song, kind, near),
                prog: "A".into(),
                variant: "main".into(),
                ..Default::default()
            });
            let section = SectionView {
                name: unique_name(song, kind),
                kind: kind.clone(),
                start_bar: 0,
                bars: (*bars).clamp(1, 256),
                build: kind == "build",
                impact: kind == "drop",
                gap: false,
                ..like
            };
            let msg = format!("added {} ({} bars)", section.name, section.bars);
            song.sections.insert(at, section);
            relayout(song);
            Ok(msg)
        }
        Command::RemoveSection { index } => {
            if song.sections.len() <= 1 {
                return Some(Err("a song needs at least one section".into()));
            }
            if *index >= song.sections.len() {
                return Some(Err(format!("no section {index}")));
            }
            let s = song.sections.remove(*index);
            relayout(song);
            Ok(format!("removed {}", s.name))
        }
        Command::MoveSection { from, to } => {
            if *from >= song.sections.len() {
                return Some(Err(format!("no section {from}")));
            }
            let s = song.sections.remove(*from);
            let msg = format!("moved {} to position {}", s.name, to + 1);
            song.sections.insert((*to).min(song.sections.len()), s);
            relayout(song);
            Ok(msg)
        }
        Command::ToggleLayer { section, layer } => {
            let order: Vec<String> = song.layers.clone();
            let Some(s) = song.sections.get_mut(*section) else {
                return Some(Err(format!("no section {section}")));
            };
            let on = if let Some(i) = s.layers.iter().position(|l| l == layer) {
                s.layers.remove(i);
                false
            } else {
                s.layers.push(layer.clone());
                s.layers.sort_by_key(|l| order.iter().position(|o| o == l).unwrap_or(usize::MAX));
                true
            };
            Ok(format!("{layer} {} in {}", if on { "on" } else { "off" }, s.name))
        }
        Command::SetDrumStep { lane, variant, step, velocity } => {
            let Some(d) = song.drums.iter_mut().find(|d| &d.lane == lane) else {
                return Some(Err(format!("no drum lane '{lane}'")));
            };
            let len = (d.pattern_bars.max(1) * 16) as usize;
            if d.variants.iter().all(|(n, _)| n != variant) {
                let base = d.variants.iter().find(|(n, _)| n == "main").or(d.variants.first()).map(|(_, s)| s.clone());
                d.variants.push((variant.clone(), base.unwrap_or_else(|| vec![0.0; len])));
            }
            let steps = &mut d.variants.iter_mut().find(|(n, _)| n == variant)?.1;
            steps.resize(len, 0.0);
            steps[*step % len] = velocity.clamp(0.0, 1.0);
            Ok(format!(
                "{lane} {variant} step {}: {}",
                step % len + 1,
                if *velocity > 0.0 { format!("{velocity:.2}") } else { "off".into() }
            ))
        }
        // a lane's own time and feel, as the server keeps them
        Command::SetLane { lane, length, speed, swing, groove, groove_amount } => {
            let Some(d) = song.drums.iter_mut().find(|d| &d.lane == lane) else {
                return Some(Err(format!("no drum lane '{lane}'")));
            };
            if let Some(l) = length {
                d.length = (*l >= 1).then_some(*l);
            }
            if let Some(v) = speed {
                d.speed = v.clamp(0.125, 4.0);
            }
            if let Some(v) = swing {
                d.swing = (*v != 0.0).then_some(*v);
            }
            if let Some(g) = groove {
                d.groove = (!g.is_empty() && g != "none").then(|| g.clone());
            }
            if let Some(v) = groove_amount {
                d.groove_amount = v.clamp(0.0, 1.0);
            }
            Ok(format!("{lane}: its own time"))
        }
        // a step's trig, as the server keeps it; a step without a hit gets one
        Command::SetTrig { lane, variant, step, trig } => {
            let Some(d) = song.drums.iter_mut().find(|d| &d.lane == lane) else {
                return Some(Err(format!("no drum lane '{lane}'")));
            };
            let len = (d.pattern_bars.max(1) * 16) as usize;
            if *step >= len {
                return Some(Err(format!("{lane}'s steps go from 0 to {}", len - 1)));
            }
            if let Some((_, steps)) = d.variants.iter_mut().find(|(n, _)| n == variant) {
                steps.resize(len, 0.0);
                if steps[*step] <= 0.0 && trig.is_some() {
                    steps[*step] = 0.8;
                }
            }
            if d.trigs.iter().all(|(n, _)| n != variant) {
                d.trigs.push((variant.clone(), Vec::new()));
            }
            let trigs = &mut d.trigs.iter_mut().find(|(n, _)| n == variant)?.1;
            trigs.retain(|(s, _)| s != step);
            if let Some(t) = trig.clone().filter(|t| *t != claudius_api::TrigView::default()) {
                trigs.push((*step, t));
                trigs.sort_by_key(|(s, _)| *s);
            }
            Ok(format!("{lane} {variant} step {step}: trig set"))
        }
        Command::SetDrumPattern { lane, variant, steps } => {
            let Some(d) = song.drums.iter_mut().find(|d| &d.lane == lane) else {
                return Some(Err(format!("no drum lane '{lane}'")));
            };
            let len = (d.pattern_bars.max(1) * 16) as usize;
            let parsed: Option<Vec<f32>> =
                steps.chars().filter(|c| !c.is_whitespace() && *c != '|').map(step_velocity).collect();
            let Some(parsed) = parsed.filter(|p| !p.is_empty()) else {
                return Some(Err(format!("bad step notation '{steps}' (x hit, X accent, o ghost, . rest)")));
            };
            let full: Vec<f32> = (0..len).map(|i| parsed[i % parsed.len()]).collect();
            match d.variants.iter_mut().find(|(n, _)| n == variant) {
                Some((_, s)) => *s = full,
                None => d.variants.push((variant.clone(), full)),
            }
            Ok(format!(
                "{lane} {variant}: {}",
                steps_to_string(&d.variants.iter().find(|(n, _)| n == variant)?.1[..16.min(len)])
            ))
        }
        Command::SetPart { name, style, preset, register, density, reseed } => {
            let Some(p) = song.parts.iter_mut().find(|p| &p.name == name) else {
                return Some(Err(format!("no part '{name}'")));
            };
            let mut what = Vec::new();
            if let Some(v) = style {
                p.style = v.clone();
                what.push(format!("style {v}"));
            }
            if let Some(v) = preset {
                p.preset = v.clone();
                what.push(format!("preset {v}"));
            }
            if let Some((lo, hi)) = register {
                p.register = ((*lo).min(*hi), (*lo).max(*hi));
                what.push(format!(
                    "register {}..{}",
                    crate::music::note_name(p.register.0),
                    crate::music::note_name(p.register.1)
                ));
            }
            if let Some(v) = density {
                p.density = v.clamp(0.0, 1.0);
                what.push(format!("density {:.2}", p.density));
            }
            if *reseed {
                what.push("rewritten with a new seed".into());
            }
            if let Some(v) = preset
                && let Some(t) = song.tracks.iter_mut().find(|t| &t.name == name)
            {
                t.sound = format!("synth patch '{v}'");
            }
            Ok(format!("{name}: {}", what.join(", ")))
        }
        Command::AddTrack { name, kind, role, sound } => {
            // like the server: one word, a name of its own, playing in the sections with energy 0.6 or more
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Some(Err(format!("a track's name is one word of letters, digits, _ and -, not '{name}'")));
            }
            if song.layers.contains(name) {
                return Some(Err(format!("the song already has a track '{name}': give the new one another name")));
            }
            let (track, bus) = match kind.as_str() {
                "drum" => {
                    song.drums.push(DrumView {
                        lane: name.clone(),
                        pattern_bars: 1,
                        variants: vec![("main".into(), vec![0.0; 16])],
                        speed: 1.0,
                        ..Default::default()
                    });
                    (
                        TrackView {
                            kind: "drum".into(),
                            role: name.clone(),
                            sound: sound.clone().unwrap_or_default(),
                            ..Default::default()
                        },
                        "drums",
                    )
                }
                "part" => {
                    let role = role.clone().unwrap_or_else(|| "lead".into());
                    let register = match role.as_str() {
                        "bass" => (28, 52),
                        "pad" => (48, 72),
                        "chords" | "keys" => (52, 76),
                        _ => (60, 86),
                    };
                    let preset =
                        sound.clone().unwrap_or_else(|| if role == "bass" { "saw_bass" } else { "pluck" }.into());
                    song.parts.push(PartView {
                        name: name.clone(),
                        role: role.clone(),
                        style: String::new(),
                        styles: Vec::new(),
                        preset: preset.clone(),
                        presets: Vec::new(),
                        register,
                        density: 0.55,
                        cycles: Vec::new(),
                    });
                    let bus = if role == "bass" { "bass" } else { "music" };
                    (TrackView { kind: "part".into(), role, sound: preset, ..Default::default() }, bus)
                }
                other => {
                    return Some(Err(format!(
                        "a track is a drum lane (kind \"drum\") or a part (\"part\"), not '{other}'"
                    )));
                }
            };
            song.tracks.push(TrackView { name: name.clone(), bus: bus.into(), ..track });
            song.layers.push(name.clone());
            for s in song.sections.iter_mut().filter(|s| s.energy >= 0.6) {
                s.layers.push(name.clone());
            }
            Ok(format!("new track {name}"))
        }
        Command::SetNotes { part, prog, notes } => {
            let Some(p) = song.parts.iter_mut().find(|p| &p.name == part) else {
                return Some(Err(format!("no part '{part}'")));
            };
            if !song.progressions.iter().any(|(name, _)| name == prog) {
                return Some(Err(format!("no progression '{prog}'")));
            }
            let at = match p.cycles.iter().position(|c| &c.prog == prog) {
                Some(i) => i,
                None => {
                    // a progression the part has no cycle for yet (the demo writes its parts as they play, a chord a
                    // bar)
                    let beats = song
                        .progressions
                        .iter()
                        .find(|(name, _)| name == prog)
                        .map_or(16.0, |(_, chords)| (chords.len().max(1) * 4) as f64);
                    p.cycles.push(claudius_api::CycleView { prog: prog.clone(), beats, ..Default::default() });
                    p.cycles.len() - 1
                }
            };
            let c = &mut p.cycles[at];
            match notes {
                Some(n) => {
                    c.notes = n.clone();
                    c.written = true;
                    Ok(format!("{part} over {prog}: {} notes written out", n.len()))
                }
                None => {
                    c.written = false;
                    Ok(format!("{part} over {prog}: the theory engine's again"))
                }
            }
        }
        Command::RemoveTrack { name } => {
            if !song.layers.contains(name) {
                return Some(Err(format!("no track '{name}'")));
            }
            song.layers.retain(|l| l != name);
            song.tracks.retain(|t| &t.name != name);
            song.drums.retain(|d| &d.lane != name);
            song.parts.retain(|p| &p.name != name);
            for s in song.sections.iter_mut() {
                s.layers.retain(|l| l != name);
            }
            Ok(format!("removed {name}"))
        }
        Command::SetMix { track, level_db, pan, reverb, delay, duck, drive_db, muted, solo, bus } => {
            let Some(t) = song.tracks.iter_mut().find(|t| &t.name == track) else {
                return Some(Err(format!("no track '{track}'")));
            };
            if let Some(v) = level_db {
                t.level_db = v.clamp(-60.0, 24.0);
            }
            if let Some(v) = pan {
                t.pan = v.clamp(-1.0, 1.0);
            }
            if let Some(v) = reverb {
                t.reverb = v.clamp(0.0, 1.0);
            }
            if let Some(v) = delay {
                t.delay = v.clamp(0.0, 1.0);
            }
            if let Some(v) = duck {
                t.duck = v.clamp(0.0, 1.0);
            }
            if let Some(v) = drive_db {
                t.drive_db = v.clamp(0.0, 36.0);
            }
            if let Some(v) = muted {
                t.muted = *v;
            }
            if let Some(v) = solo {
                t.solo = *v;
            }
            if let Some(b) = bus {
                t.bus = b.clone();
            }
            Ok(format!("mix {track}: {:+.1} dB, pan {:+.2}{}", t.level_db, t.pan, if t.muted { ", muted" } else { "" }))
        }
        // the demo keeps the chain as it is given: its own synth does not play the effects
        Command::SetFx { target, fx } => {
            let chain = fx.clone();
            match target.split_once(':') {
                Some(("track", name)) => match song.tracks.iter_mut().find(|t| t.name == name) {
                    Some(t) => t.fx = chain,
                    None => return Some(Err(format!("no track '{name}'"))),
                },
                Some(("bus", name)) => match song.mix.buses.iter_mut().find(|(b, _)| b == name) {
                    Some((_, c)) => *c = chain,
                    None => song.mix.buses.push((name.to_string(), chain)),
                },
                Some(("send", "reverb")) if chain.len() == 1 => song.mix.reverb = chain[0].clone(),
                Some(("send", "delay")) if chain.len() == 1 => song.mix.delay = chain[0].clone(),
                None if target == "master" => song.mix.master = chain,
                _ => return Some(Err(format!("'{target}' is no chain of effects"))),
            }
            Ok(format!("{target}: {} effects", fx.len()))
        }
        // a tuner first in the track's chain, or the one there changed, as the server puts it
        Command::Tune { track, strength, speed_ms, humanize, vibrato, scale, off } => {
            let Some(t) = song.tracks.iter_mut().find(|t| t.name == *track) else {
                return Some(Err(format!("no track '{track}'")));
            };
            let at = t.fx.iter().position(|e| e.get("fx").and_then(|f| f.as_str()) == Some("tune"));
            if *off {
                if let Some(i) = at {
                    t.fx.remove(i);
                }
                return Some(Ok(format!("{track}: tuned no longer")));
            }
            let mut e = at.map(|i| t.fx[i].clone()).unwrap_or_else(|| {
                serde_json::from_str(
                    claudius_api::EFFECT_TEMPLATES.iter().find(|(n, _)| *n == "tune").map_or("{}", |t| t.1),
                )
                .unwrap_or_default()
            });
            for (k, v) in [("strength", strength), ("speed_ms", speed_ms), ("humanize", humanize), ("vibrato", vibrato)]
            {
                if let Some(v) = v {
                    e[k] = serde_json::json!(v);
                }
            }
            if let Some(s) = scale {
                e["scale"] = match s.trim() {
                    "chords" | "key" | "chromatic" => serde_json::json!(s.trim()),
                    notes => {
                        serde_json::json!({ "notes": notes.split([' ', ',']).filter(|n| !n.is_empty()).collect::<Vec<_>>() })
                    }
                };
            }
            match at {
                Some(i) => t.fx[i] = e,
                None => t.fx.insert(0, e),
            }
            Ok(format!("{track}: tuned"))
        }
        _ => return None,
    })
}
