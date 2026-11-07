//! The API's edits of a song. Most run as the script that says them (`script`), which is also how a person's script
//! reads in the song's history; the rest change the blueprint in place (`change`). A command that is neither, such
//! as a new song, a render, a take or a file, is no edit: both say None.

use claudius::fx::Effect;
use claudius::model::*;
use claudius::pattern::quote;
use claudius::rng::Rng;
use claudius::script;
use claudius::theory::rhythm::Hit;
use claudius_api::{Command, ScratchMove};

/// What a tuner is asked for (`tune`): what is given changes the track's tuner, or a new one's defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tuning<'a> {
    pub strength: Option<f32>,
    pub speed_ms: Option<f32>,
    pub humanize: Option<f32>,
    pub vibrato: Option<f32>,
    pub scale: Option<&'a str>,
    /// the tuner taken off instead
    pub off: bool,
}

/// `track` of `bp` tuned as `asked` asks, singing as `voice` (the person's, as their pitch check heard it) unless its
/// tuner has a voice of its own; what it did, or why not.
pub fn tune(
    bp: &mut Blueprint,
    track: &str,
    asked: &Tuning,
    voice: Option<&claudius::voice::singer::Voice>,
) -> Result<String, String> {
    let scale = asked.scale.map(str::parse::<claudius::voice::tune::TuneScale>).transpose()?;
    if asked.off {
        return claudius::voice::tune::put(bp, track, None);
    }
    let was = script::mix_of(bp, track).and_then(|m| m.fx.iter().find(|e| matches!(e, Effect::Tune { .. })).cloned());
    let mut t = was.unwrap_or_else(|| {
        serde_json::from_value(serde_json::json!({ "fx": "tune" })).expect("a tuner from its defaults")
    });
    if let Effect::Tune { strength: s, speed_ms: sp, humanize: h, vibrato: vb, scale: sc, .. } = &mut t {
        if let Some(x) = asked.strength {
            *s = x.clamp(0.0, 1.0).into();
        }
        if let Some(x) = asked.speed_ms {
            *sp = x.clamp(0.0, 1000.0);
        }
        if let Some(x) = asked.humanize {
            *h = x.clamp(0.0, 100.0);
        }
        if let Some(x) = asked.vibrato {
            *vb = x.clamp(0.0, 1.0);
        }
        if let Some(x) = scale {
            *sc = x;
        }
    }
    let mut as_checked = false;
    if let (Effect::Tune { voice: v @ None, .. }, Some(p)) = (&mut t, voice) {
        (*v, as_checked) = (Some(p.clone()), true);
    }
    claudius::voice::tune::put(bp, track, Some(t))
        .map(|said| if as_checked { format!("{said}, as your pitch check heard you sing") } else { said })
}

/// A script an edit runs on the song, and what undo calls it (None: the script's first line).
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub code: String,
    pub label: Option<String>,
}

/// The script command `cmd` runs on the song `bp` (None while there is none), `seed` the one a part reseeded with
/// takes; None when `cmd` is no edit a script makes, and an error when it asks for what cannot be.
pub fn script(cmd: &Command, bp: Option<&Blueprint>, seed: u64) -> Option<Result<Script, String>> {
    let run = |code: String, label: String| Some(Ok(Script { code, label: Some(label) }));
    match cmd {
        Command::SetTempo { bpm } => run(format!("tempo({bpm})"), format!("tempo {bpm:.0}")),
        Command::SetKey { key } => run(format!("key({})", quote(key)), format!("key {key}")),
        Command::SetSwing { swing } => run(format!("swing({swing})"), format!("swing {swing:.2}")),
        Command::SetProgression { name, chords } => run(
            format!("prog({}, {})", quote(name), quote(&chords.join(" "))),
            format!("progression {name}: {}", chords.join(" ")),
        ),
        Command::SetDrumPattern { lane, variant, steps } => run(
            format!("drum({}, {}, {})", quote(lane), quote(variant), quote(steps)),
            format!("{lane} ({variant}) pattern"),
        ),
        Command::SetPart { name, style, preset, register, density, reseed, late_ms } => {
            let mut fields = Vec::new();
            if let Some(ms) = late_ms {
                fields.push(format!("late: {ms:?}"));
            }
            if let Some(s) = style {
                fields.push(format!("style: {}", quote(s)));
            }
            if let Some(p) = preset {
                fields.push(format!("preset: {}", quote(p)));
            }
            if let Some((a, b)) = register {
                fields.push(format!("register: [{a}, {b}]"));
            }
            if let Some(d) = density {
                fields.push(format!("density: {d}"));
            }
            if *reseed {
                fields.push(format!("seed: {}", Rng::new(seed).next_u64() as i64));
            }
            run(
                format!("part({}, #{{ {} }})", quote(name), fields.join(", ")),
                format!("{name}: {}", fields.join(", ")),
            )
        }
        Command::AddTrack { name, kind, role, sound } => Some(add_track(name, kind, role, sound, bp)),
        Command::RemoveTrack { name } => run(format!("remove({})", quote(name)), format!("removed {name}")),
        Command::Harmonize { part, style, sound, name } => Some(harmonize(part, style, sound, name, bp)),
        Command::SwapSound { track, to, only } => Some(swap(track, to, only, bp)),
        Command::SetNotes { part, prog, notes } => Some(Ok(match notes {
            Some(notes) => {
                // as the script reads them back exactly: whole numbers whole, the rest as Rust prints them
                let n =
                    |x: f64| if x == x.trunc() && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x}") };
                let list: Vec<String> =
                    notes.iter().map(|x| format!("[{}, {}, {}, {}]", n(x.t), n(x.d), x.p, x.v)).collect();
                Script {
                    code: format!("notes({}, {}, [{}])", quote(part), quote(prog), list.join(", ")),
                    label: Some(format!("{part} over {prog}: {} notes written out", notes.len())),
                }
            }
            None => Script {
                code: format!("clear_notes({}, {})", quote(part), quote(prog)),
                label: Some(format!("{part} over {prog}: the theory engine's again")),
            },
        })),
        Command::SetMix { track, level_db, pan, reverb, delay, duck, drive_db, muted, solo, bus } => {
            let mut fields = Vec::new();
            for (k, x) in [
                ("level", level_db),
                ("pan", pan),
                ("reverb", reverb),
                ("delay", delay),
                ("duck", duck),
                ("drive", drive_db),
            ] {
                if let Some(x) = x {
                    fields.push(format!("{k}: {x}"));
                }
            }
            if let Some(m) = muted {
                fields.push(format!("mute: {m}"));
            }
            if let Some(m) = solo {
                fields.push(format!("solo: {m}"));
            }
            if let Some(b) = bus {
                fields.push(format!("bus: {}", quote(b)));
            }
            run(
                format!("mix({}, #{{ {} }})", quote(track), fields.join(", ")),
                format!("mix {track}: {}", fields.join(", ")),
            )
        }
        Command::SetLane { lane, length, speed, swing, groove, groove_amount, late_ms } => {
            let mut set = Vec::new();
            if let Some(v) = length {
                set.push(format!("length: {v}"));
            }
            for (k, v) in [("speed", speed), ("swing", swing), ("groove_amount", groove_amount), ("late", late_ms)] {
                if let Some(v) = v {
                    set.push(format!("{k}: {v:?}"));
                }
            }
            if let Some(g) = groove {
                set.push(format!("groove: {}", quote(g)));
            }
            run(format!("lane({}, #{{ {} }});", quote(lane), set.join(", ")), format!("{lane}'s time"))
        }
        // the trig given in place of the one there: cleared, then set with what differs from the defaults
        Command::SetTrig { lane, variant, step, trig } => {
            let (lane_q, variant_q) = (quote(lane), quote(variant));
            let mut code = format!("trig({lane_q}, {variant_q}, {step}, #{{}});");
            if let Some(set) = trig.as_ref().map(|t| t.script()).filter(|s| !s.is_empty()) {
                code.push_str(&format!(" trig({lane_q}, {variant_q}, {step}, #{{ {set} }});"));
            }
            run(code, format!("{lane} {variant} step {step}"))
        }
        Command::RunScript { code } => Some(Ok(Script { code: code.clone(), label: None })),
        _ => None,
    }
}

fn add_track(
    name: &str,
    kind: &str,
    role: &Option<String>,
    sound: &Option<String>,
    bp: Option<&Blueprint>,
) -> Result<Script, String> {
    // a name is one word: sections list their tracks separated by spaces
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("a track's name is one word of letters, digits, _ and -, not '{name}'"));
    }
    if bp.is_some_and(|bp| bp.layer_names().iter().any(|l| l == name)) {
        return Err(format!("the song already has a track '{name}': give the new one another name"));
    }
    let code = match kind {
        // the sound makes a lane of any name; without one, drum() makes a lane named for a drum
        "drum" => match sound {
            Some(s) => format!("drum_sound({}, {});", quote(name), quote(s)),
            None => format!("drum({}, \"main\", \"................\");", quote(name)),
        },
        "part" => {
            let mut fields = Vec::new();
            if let Some(r) = role {
                fields.push(format!("role: {}", quote(r)));
            }
            if let Some(s) = sound {
                fields.push(format!("preset: {}", quote(s)));
            }
            format!("part({}, #{{ {} }})", quote(name), fields.join(", "))
        }
        other => return Err(format!("a track is a drum lane (kind \"drum\") or a part (\"part\"), not '{other}'")),
    };
    Ok(Script { code, label: Some(format!("new track {name}")) })
}

/// Chords under part `melody` as the script says them: a part that harmonizes it, on in the sections it plays in.
fn harmonize(
    melody: &str,
    style: &Option<String>,
    sound: &Option<String>,
    name: &Option<String>,
    bp: Option<&Blueprint>,
) -> Result<Script, String> {
    let Some(bp) = bp else { return Err("no song yet: start one with a new song".into()) };
    let Some(m) = bp.parts.iter().find(|p| p.name == melody) else {
        let parts: Vec<&str> = bp.parts.iter().map(|p| p.name.as_str()).collect();
        return Err(format!("no part '{melody}' to harmonize (parts: {})", parts.join(" ")));
    };
    let style = style.clone().unwrap_or_else(|| "block".into());
    let styles = claudius::theory::harmonize::styles();
    if !styles.contains(&style.as_str()) {
        return Err(format!("chords under a melody play in one of {}, not '{style}'", styles.join(" ")));
    }
    let taken = bp.layer_names();
    let name = match name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) if taken.iter().any(|t| t == n) => {
            return Err(format!("the song already has a track '{n}': give the chords another name"));
        }
        Some(n) => n.to_string(),
        None => std::iter::once(format!("{melody}_chords"))
            .chain((2..).map(|k| format!("{melody}_chords{k}")))
            .find(|n| !taken.contains(n))
            .unwrap_or_default(),
    };
    // block chords sound as the melody does, a chord under each note; the rhythms keep under its range
    let block = style == "block";
    let role = if style == "sustain" { "pad" } else { "keys" };
    let sound = sound.clone().or_else(|| match &m.sound {
        Sound::Synth { preset, .. } if block => Some(preset.clone()),
        Sound::Instrument { name, .. } if block => Some(name.clone()),
        _ => None,
    });
    let (lo, hi) = m.register;
    let register = if block { ((lo - 14).max(36), (hi - 1).max(lo)) } else { ((lo - 12).max(36), (lo + 7).max(48)) };
    let preset = sound.map(|s| format!(", preset: {}", quote(&s))).unwrap_or_default();
    let mut code = format!(
        "part({}, #{{ role: {}, harmonizes: {}, style: {}{preset}, register: [{}, {}] }});",
        quote(&name),
        quote(role),
        quote(melody),
        quote(&style),
        register.0,
        register.1
    );
    let mut plays = Vec::new();
    for (i, s) in bp.form.iter().enumerate() {
        let on = s.layers.iter().any(|l| l == melody);
        code.push_str(&format!("\nlayer({i}, {}, {on});", quote(&name)));
        if on {
            plays.push(s.name.as_str());
        }
    }
    let how = if block { "a chord under each of its notes".to_string() } else { format!("{style} chords") };
    let label = format!(
        "{name}: {how} under {melody}, in {}",
        if plays.is_empty() { "no section yet".into() } else { plays.join(", ") }
    );
    Ok(Script { code, label: Some(label) })
}

/// Another sound for `track`, or for every part and drum lane (`only`: "drums" or "parts"), as the script says it:
/// a part's `preset` and a lane's `drum_sound`, for each track it changes.
fn swap(
    track: &Option<String>,
    to: &Option<String>,
    only: &Option<String>,
    bp: Option<&Blueprint>,
) -> Result<Script, String> {
    use claudius::swap;
    let Some(bp) = bp else { return Err("no song yet: start one with a new song".into()) };
    let to = to.as_deref().map(str::trim).filter(|t| !t.is_empty());
    if let Some(t) = track.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        return swap_track(bp, t, to);
    }
    let (drums, parts) = match only.as_deref().map(str::trim) {
        None | Some("") => (true, true),
        Some("drums") => (true, false),
        Some("parts") => (false, true),
        Some(other) => {
            return Err(format!("a swap of the whole song swaps the \"drums\" or the \"parts\", not '{other}'"));
        }
    };
    if to.is_some() && parts {
        return Err("to names a kit when only the drums swap (only \"drums\"), or a sound for one track".into());
    }
    let (mut code, mut said) = (Vec::new(), Vec::new());
    if drums && !bp.drums.is_empty() {
        let kits = swap::kits(bp);
        let kit = match to {
            Some(k) if kits.iter().any(|x| x == k) => Some(k.to_string()),
            Some(k) => return Err(format!("no kit '{k}' (kits: {})", kits.join(" "))),
            None => swap::next_kit(bp),
        };
        if let Some(kit) = kit {
            for (lane, name) in swap::on_kit(bp, &kit) {
                code.push(format!("drum_sound({}, {});", quote(&lane), quote(&name)));
            }
            said.push(if kit == swap::SYNTHESIZED {
                "the drums synthesized".to_string()
            } else {
                format!("the drums on {kit}")
            });
        }
    }
    if parts {
        let mut taken = std::collections::BTreeSet::new();
        for p in bp.parts.iter().filter(|p| !follows(bp, p)) {
            let Some(c) = swap::next(bp, &p.name, &taken) else { continue };
            taken.insert(swap::identity(&c.name));
            code.extend(sounding(bp, p, &c.name));
            said.push(format!("{} on {}", p.name, c.name));
        }
    }
    if code.is_empty() {
        return Err("nothing in the song has another sound to swap to".into());
    }
    Ok(Script { code: code.join("\n"), label: Some(format!("new sounds: {}", said.join(", "))) })
}

fn swap_track(bp: &Blueprint, t: &str, to: Option<&str>) -> Result<Script, String> {
    use claudius::swap;
    let next = || {
        swap::next(bp, t, &Default::default())
            .map(|c| c.name)
            .ok_or_else(|| format!("{t} has no other sound to swap to"))
    };
    if let Some(p) = bp.parts.iter().find(|p| p.name == t) {
        let from = swap::playing(bp, t).unwrap_or_else(|| p.sound.describe());
        let to = match to {
            Some(x) => x.to_string(),
            None => next()?,
        };
        return Ok(Script { code: sounding(bp, p, &to).join("\n"), label: Some(format!("{t}: {from} → {to}")) });
    }
    if let Some(d) = bp.drums.iter().find(|d| d.name == t) {
        let from = swap::playing(bp, t).unwrap_or_else(|| d.sound.describe());
        let to = match to {
            Some(x) => x.to_string(),
            None => next()?,
        };
        let code = format!("drum_sound({}, {});", quote(t), quote(&to));
        return Ok(Script { code, label: Some(format!("{t}: {from} → {to}")) });
    }
    if let Some(l) = bp.loops.iter().find(|l| l.name == t) {
        let what = if l.from.is_some() { "a take, a recording" } else { "a loop from the library" };
        return Err(format!("{t} is {what}, with no other sound to swap to: swap a part or a drum lane"));
    }
    if bp.patterns.iter().any(|p| p.name == t) {
        return Err(format!("{t} is a pattern track, whose sounds its pattern names (.s(\"...\")): edit its pattern"));
    }
    let tracks: Vec<&str> =
        bp.parts.iter().map(|p| p.name.as_str()).chain(bp.drums.iter().map(|d| d.name.as_str())).collect();
    Err(format!("no part or drum lane '{t}' to swap the sound of (they are: {})", tracks.join(" ")))
}

/// whether part `p` plays block chords under its melody on the melody's own sound, which it keeps as the melody swaps
fn follows(bp: &Blueprint, p: &Part) -> bool {
    let melody = p.harmonizes.as_deref().and_then(|m| bp.parts.iter().find(|q| q.name == m));
    p.style == "block"
        && melody.is_some_and(|m| claudius::palette::identity(&m.sound) == claudius::palette::identity(&p.sound))
}

/// the script that puts part `p` on sound `name`, and the block chords that follow it with it
fn sounding(bp: &Blueprint, p: &Part, name: &str) -> Vec<String> {
    let line = |part: &str| format!("part({}, #{{ preset: {} }});", quote(part), quote(name));
    let chords = bp.parts.iter().filter(|h| h.harmonizes.as_deref() == Some(p.name.as_str()) && follows(bp, h));
    std::iter::once(line(&p.name)).chain(chords.map(|h| line(&h.name))).collect()
}

/// What command `cmd` changes in the song `bp` itself, and what to say of it; None when `cmd` is no such edit. `seed`
/// is the one a track generated or varied takes, when the command names none.
pub fn change(cmd: &Command, bp: &mut Blueprint, seed: u64) -> Option<Result<String, String>> {
    Some(match cmd {
        Command::GenerateTrack { what, genre, replace, name, seed: given } => {
            let genre = genre.clone().filter(|g| !g.trim().is_empty()).unwrap_or_else(|| bp.genre.clone());
            claudius::generate::track(bp, what, &genre, replace.as_deref(), name.as_deref(), given.unwrap_or(seed))
        }
        Command::VaryTrack { track, take } => claudius::generate::vary(bp, track, *take, seed),
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
            if let Some(p) = prog
                && !bp.harmony.progressions.contains_key(p)
            {
                return Some(Err(format!(
                    "no progression '{p}' (there are {})",
                    bp.harmony.progressions.keys().cloned().collect::<Vec<_>>().join(", ")
                )));
            }
            let n = bp.form.len();
            let Some(s) = bp.form.get_mut(*index) else {
                return Some(Err(format!("no section {index} (there are {n})")));
            };
            if let Some(x) = name {
                s.name = x.clone();
            }
            if let Some(x) = bars {
                s.bars = (*x).clamp(1, 256);
            }
            if let Some(x) = energy {
                s.energy = x.clamp(0.0, 1.0);
            }
            if let Some(x) = layers {
                s.layers = x.clone();
            }
            if let Some(x) = prog {
                s.prog = x.clone();
            }
            if let Some(x) = variant {
                s.variant = x.clone();
            }
            for (src, dst) in [
                (filter_in, &mut s.filter_in),
                (build, &mut s.build),
                (gap, &mut s.gap),
                (impact, &mut s.impact),
                (fills, &mut s.fills),
            ] {
                if let Some(x) = src {
                    *dst = *x;
                }
            }
            Ok(format!("section {index} ({})", s.name))
        }
        Command::AddSection { after, kind, bars } => add_section(bp, *after, kind, *bars),
        Command::RemoveSection { index } => {
            if bp.form.len() <= 1 {
                Err("a song needs at least one section".into())
            } else if *index >= bp.form.len() {
                Err(format!("no section {index}"))
            } else {
                let s = bp.form.remove(*index);
                Ok(format!("removed {}", s.name))
            }
        }
        Command::MoveSection { from, to } => {
            if *from >= bp.form.len() || *to >= bp.form.len() {
                Err(format!("sections are 0..{}", bp.form.len()))
            } else {
                let s = bp.form.remove(*from);
                let name = s.name.clone();
                bp.form.insert(*to, s);
                Ok(format!("moved {name} to {to}"))
            }
        }
        Command::ToggleLayer { section, layer } => match bp.form.get_mut(*section) {
            None => Err(format!("no section {section}")),
            Some(s) => match s.layers.iter().position(|l| l == layer) {
                Some(i) => {
                    s.layers.remove(i);
                    Ok(format!("{layer} off in {}", s.name))
                }
                None => {
                    s.layers.push(layer.clone());
                    Ok(format!("{layer} on in {}", s.name))
                }
            },
        },
        Command::SetDrumStep { lane, variant, step, velocity } => drum_step(bp, lane, variant, *step, *velocity),
        Command::SetFx { target, fx } => set_fx(bp, target, fx),
        Command::AddNotes { from, notes, sounds, name, role, style, request: _ } => {
            let notes: Vec<claudius::theory::Note> =
                notes.iter().map(|n| claudius::theory::Note { t: n.t, d: n.d, p: n.p, v: n.v, lock: None }).collect();
            let played = claudius::take::Played {
                sounds: sounds.clone(),
                name: name.clone(),
                role: role.clone(),
                style: style.clone(),
            };
            claudius::take::played(bp, from, &notes, &played)
        }
        Command::AddHits { from, hits, bars, variant, grid, start, beats } => {
            let struck: Vec<claudius::take::Struck> =
                hits.iter().map(|h| claudius::take::Struck { lane: h.lane.clone(), t: h.t, v: h.v }).collect();
            let o = claudius::take::Pads {
                bars: *bars,
                variant: variant.clone(),
                grid: *grid,
                start: *start,
                beats: *beats,
            };
            claudius::take::hits(bp, from, &struck, &o)
        }
        _ => return None,
    })
}

/// the scratch a command asks for, on `bars` bars (1 when not given) of `track` from bar `bar`, or on the record recorded
/// off a microphone that `recording` holds (a WAV file in base64)
pub fn scratch(
    track: String,
    bar: u32,
    bars: Option<u32>,
    moves: &[ScratchMove],
    recording: Option<&str>,
) -> Result<claudius::scratch::Take, String> {
    use base64::Engine;
    let recording = recording
        .map(|b| base64::engine::general_purpose::STANDARD.decode(b.trim()))
        .transpose()
        .map_err(|_| "the recording is no WAV file in base64".to_string())?;
    let moves = moves.iter().map(|m| claudius::scratch::Move { t: m.t, at: m.at, fader: m.fader }).collect();
    Ok(claudius::scratch::Take { track, bar, bars: bars.unwrap_or(1), moves, recording })
}

fn add_section(bp: &mut Blueprint, after: usize, kind: &str, bars: u32) -> Result<String, String> {
    let k = section_kind(kind)?;
    let like = bp.form.get(after.min(bp.form.len().saturating_sub(1))).cloned();
    let mut spec = like.unwrap_or_else(|| SectionSpec {
        name: String::new(),
        kind: k,
        bars,
        energy: 0.7,
        prog: "A".into(),
        layers: bp.layer_names(),
        variant: "main".into(),
        filter_in: false,
        build: false,
        gap: false,
        impact: false,
        fills: false,
    });
    spec.kind = k;
    spec.bars = bars.clamp(1, 256);
    spec.build = k == SectionKind::Build;
    spec.impact = matches!(k, SectionKind::Drop);
    spec.gap = false;
    let mut i = 1;
    spec.name = kind.to_string();
    while bp.form.iter().any(|s| s.name == spec.name) {
        i += 1;
        spec.name = format!("{kind}{i}");
    }
    let at = (after + 1).min(bp.form.len());
    let name = spec.name.clone();
    bp.form.insert(at, spec);
    Ok(format!("added {name} ({bars} bars) at {at}"))
}

fn drum_step(bp: &mut Blueprint, lane: &str, variant: &str, step: usize, velocity: f32) -> Result<String, String> {
    let names: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    let l = bp
        .drums
        .iter_mut()
        .find(|d| d.name == lane)
        .ok_or(format!("no drum lane '{lane}' (lanes: {})", names.join(" ")))?;
    let steps = (l.pattern_bars * 16) as usize;
    if step >= steps {
        return Err(format!("{lane} has {steps} steps"));
    }
    let main = l.patterns.get("main").cloned().unwrap_or_default();
    let hits = l.patterns.entry(variant.to_string()).or_insert(main);
    let t = step as f64 * 0.25;
    // a step that still strikes keeps what its trig does
    let trig = hits.iter().find(|h| (h.t - t).abs() <= 1e-6).and_then(|h| h.trig);
    hits.retain(|h| (h.t - t).abs() > 1e-6);
    if velocity > 0.0 {
        hits.push(Hit { trig, ..Hit::new(t, velocity.min(1.0)) });
        hits.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    }
    Ok(format!(
        "{lane} ({variant}) step {step}: {}",
        if velocity > 0.0 { format!("{velocity:.2}") } else { "off".into() }
    ))
}

/// replaces a chain of effects: a track's, a bus's, the master's, or the one effect a send feeds
fn set_fx(bp: &mut Blueprint, target: &str, fx: &[serde_json::Value]) -> Result<String, String> {
    let chain = fx
        .iter()
        .enumerate()
        .map(|(i, v)| serde_json::from_value(v.clone()).map_err(|e| format!("{target}'s effect {}: {e}", i + 1)))
        .collect::<Result<Vec<Effect>, String>>()?;
    let n = chain.len();
    match target.split_once(':') {
        Some(("track", name)) => {
            script::mix_of(bp, name).ok_or_else(|| format!("no track '{name}'"))?.fx = chain;
        }
        Some(("bus", name)) => {
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(format!("a bus is named with letters, digits, - and _, not '{name}'"));
            }
            bp.mix.buses.insert(name.to_string(), chain);
        }
        Some(("send", which @ ("reverb" | "delay"))) => {
            let [one]: [Effect; 1] =
                chain.try_into().map_err(|_| format!("the {which} send feeds one effect, not {n}"))?;
            if which == "reverb" {
                bp.mix.reverb = one;
            } else {
                bp.mix.delay = one;
            }
        }
        None if target == "master" => bp.mix.master.fx = chain,
        _ => {
            return Err(format!(
                "'{target}' is no chain of effects: track:<name>, bus:<name>, master, send:reverb or send:delay"
            ));
        }
    }
    Ok(format!("{target}: {n} effect{}", if n == 1 { "" } else { "s" }))
}

/// a section kind by its name, or one of the names it also goes by (`breakdown`, `chorus`, `loop`)
pub fn section_kind(k: &str) -> Result<SectionKind, String> {
    Ok(match k {
        "intro" => SectionKind::Intro,
        "verse" => SectionKind::Verse,
        "build" => SectionKind::Build,
        "drop" => SectionKind::Drop,
        "break" | "breakdown" => SectionKind::Break,
        "hook" | "chorus" => SectionKind::Hook,
        "groove" | "loop" => SectionKind::Groove,
        "outro" => SectionKind::Outro,
        other => {
            return Err(format!("unknown section kind '{other}' (intro verse build drop break hook groove outro)"));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cmd(v: &serde_json::Value) -> Command {
        serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"))
    }

    #[test]
    fn a_scratch_carries_the_recording_it_was_sent_as_its_bytes() {
        let moves = [ScratchMove { t: 0.0, at: 0.0, fader: 1.0 }];
        let s = scratch("mic".into(), 0, None, &moves, Some("UklGRg==")).unwrap();
        assert_eq!((s.recording.as_deref(), s.bars), (Some(&b"RIFF"[..]), 1));
        assert_eq!(scratch("kick".into(), 2, Some(2), &moves, None).unwrap().recording, None);
        let e = scratch("mic".into(), 0, None, &moves, Some("not base64!")).unwrap_err();
        assert!(e.contains("no WAV file in base64"), "{e}");
    }

    fn song() -> Blueprint {
        let g = claudius::genres::get("house").unwrap();
        claudius::compose::generate(&g, 3, &claudius::compose::Options::default(), None)
    }

    #[test]
    fn every_edit_is_made_one_way_and_nothing_else_is_an_edit() {
        let bp = song();
        let edits = [
            json!({"cmd": "set_tempo", "bpm": 120}),
            json!({"cmd": "set_key", "key": "A minor"}),
            json!({"cmd": "set_swing", "swing": 0.1}),
            json!({"cmd": "set_progression", "name": "A", "chords": ["i", "VI"]}),
            json!({"cmd": "set_drum_pattern", "lane": "kick", "variant": "main", "steps": "x..."}),
            json!({"cmd": "set_part", "name": "bass", "density": 0.5, "reseed": true}),
            json!({"cmd": "swap_sound", "track": "bass"}),
            json!({"cmd": "generate_track", "what": "bass", "genre": "reggae"}),
            json!({"cmd": "vary_track", "track": "bass"}),
            json!({"cmd": "add_track", "name": "lead2", "kind": "part", "role": "lead"}),
            json!({"cmd": "remove_track", "name": "arp"}),
            json!({"cmd": "set_notes", "part": "bass", "prog": "A", "notes": [{"t": 0.0, "d": 1.0, "p": 36}]}),
            json!({"cmd": "set_mix", "track": "kick", "level_db": -3.0}),
            json!({"cmd": "set_lane", "lane": "kick", "length": 12}),
            json!({"cmd": "set_trig", "lane": "kick", "variant": "main", "step": 0, "trig": {"prob": 0.5}}),
            json!({"cmd": "run_script", "code": "tempo(100);"}),
            json!({"cmd": "set_section", "index": 0, "energy": 0.5}),
            json!({"cmd": "add_section", "after": 0, "kind": "break", "bars": 8}),
            json!({"cmd": "remove_section", "index": 1}),
            json!({"cmd": "move_section", "from": 0, "to": 1}),
            json!({"cmd": "toggle_layer", "section": 0, "layer": "kick"}),
            json!({"cmd": "set_drum_step", "lane": "kick", "variant": "main", "step": 0, "velocity": 0.8}),
            json!({"cmd": "set_fx", "target": "master", "fx": []}),
        ];
        for v in &edits {
            let c = cmd(v);
            let (by_script, by_change) = (script(&c, Some(&bp), 1).is_some(), change(&c, &mut bp.clone(), 1).is_some());
            assert!(by_script != by_change, "{v}: a script {by_script}, a change {by_change}");
        }
        for v in [json!({"cmd": "new_song", "genre": "house"}), json!({"cmd": "render"}), json!({"cmd": "undo"})] {
            let c = cmd(&v);
            assert!(script(&c, Some(&bp), 1).is_none() && change(&c, &mut bp.clone(), 1).is_none(), "{v} is no edit");
        }
    }

    #[test]
    fn a_mix_edit_is_the_line_a_person_would_write_and_runs_as_one() {
        let bp = song();
        let set = json!({"cmd": "set_mix", "track": "bass", "level_db": 2.0, "drive_db": 12.0, "muted": true});
        let s = script(&cmd(&set), Some(&bp), 1).unwrap().unwrap();
        assert_eq!(s.code, r#"mix("bass", #{ level: 2, drive: 12, mute: true })"#);
        let mut out = claudius::script::run(&s.code, Some(bp), None).unwrap().bp.unwrap();
        let m = script::mix_of(&mut out, "bass").unwrap();
        assert_eq!((m.level_db, m.drive_db, m.mute), (2.0, 12.0, true));
    }

    #[test]
    fn a_track_named_as_one_the_song_has_or_not_as_one_word_is_refused() {
        let bp = song();
        let add = |name: &str| script(&cmd(&json!({"cmd": "add_track", "name": name, "kind": "part"})), Some(&bp), 1);
        assert!(add("bass").unwrap().unwrap_err().contains("already has a track 'bass'"));
        assert!(add("two words").unwrap().unwrap_err().contains("one word"));
        assert_eq!(add("lead2").unwrap().unwrap().code, r#"part("lead2", #{  })"#);
    }

    #[test]
    fn a_layer_toggled_twice_is_as_it_was() {
        let bp = song();
        let toggle = cmd(&json!({"cmd": "toggle_layer", "section": 0, "layer": "kick"}));
        let mut b = bp.clone();
        let first = change(&toggle, &mut b, 1).unwrap().unwrap();
        assert_ne!(b.form[0].layers, bp.form[0].layers, "{first}");
        change(&toggle, &mut b, 1).unwrap().unwrap();
        let (mut was, mut is) = (bp.form[0].layers.clone(), b.form[0].layers.clone());
        was.sort();
        is.sort();
        assert_eq!(is, was);
    }
}
