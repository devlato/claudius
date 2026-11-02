//! Edits and commands: every `Command` of the API, run against the session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use claudius::compose::{self, Options};
use claudius::model::*;
use claudius::pattern::quote;
use claudius::rng::Rng;
use claudius::theory::rhythm::Hit;
use claudius::theory::scale::Key;
use claudius::{analysis, audio, config, export, genres, script, song};
use claudius_api::*;

use super::{App, no, now_seed, ok};
use crate::views;

/// what to sample, from what a command or a query says
pub(crate) fn sampling(
    count: Option<u32>,
    bars: Option<u32>,
    find: Option<String>,
    sections: Vec<String>,
) -> claudius::sampler::Options {
    claudius::sampler::Options {
        count: count.map_or(2, |c| c as usize),
        bars,
        find: find.filter(|f| !f.trim().is_empty()),
        sections: sections.into_iter().filter(|s| !s.trim().is_empty()).collect(),
    }
}

/// the parts notes played on a keyboard can make, as a take asked for may be
const PART_ROLES: &[&str] = &["keys", "lead", "chords", "pad", "arp", "bass"];

impl App {
    pub(super) fn commit(self: &Arc<Self>, bp: Option<Blueprint>, msg: String) -> Reply {
        let label = msg.lines().next().unwrap_or("").to_string();
        self.commit_as(bp, msg, label)
    }

    /// commit a new song; `label` is what undo will say it takes back
    pub(super) fn commit_as(self: &Arc<Self>, bp: Option<Blueprint>, msg: String, label: String) -> Reply {
        let v = self.update(|s| {
            let old = std::mem::replace(&mut s.bp, bp);
            s.undo.push((old, label.chars().take(120).collect()));
            if s.undo.len() > 200 {
                s.undo.remove(0);
            }
            s.redo.clear();
            // the meters keep the last render's readings until the next render lands (the UI marks them stale)
            s.view = s.bp.as_ref().map(|b| views::song(b, &s.loudness));
            s.say(msg.clone());
            s.version + 1
        });
        self.after_change();
        ok(msg, v)
    }

    pub(super) fn after_change(self: &Arc<Self>) {
        {
            let mut s = self.lock();
            s.song_version = s.version;
        }
        if self.auto_render.load(Ordering::Relaxed) {
            self.start_render(false, Duration::from_millis(200));
        }
    }

    /// edit the current song with a function; no change, no new version
    pub(super) fn edit_with(self: &Arc<Self>, f: impl FnOnce(&mut Blueprint) -> Result<String, String>) -> Reply {
        let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut bp) = self.blueprint() else { return no("no song yet: start one with new_song", self.version()) };
        let before = bp.clone();
        match f(&mut bp).and_then(|msg| bp.check_size().map(|_| msg)) {
            Ok(msg) if bp != before => self.commit(Some(bp), msg),
            Ok(_) => ok("nothing changed", self.version()),
            Err(e) => no(e, self.version()),
        }
    }

    /// replaces a chain of effects: a track's, a bus's, the master's, or the one effect a send feeds
    fn set_fx(self: &Arc<Self>, target: &str, fx: Vec<serde_json::Value>) -> Reply {
        let chain: Result<Vec<claudius::fx::Effect>, String> = fx
            .into_iter()
            .enumerate()
            .map(|(i, v)| serde_json::from_value(v).map_err(|e| format!("{target}'s effect {}: {e}", i + 1)))
            .collect();
        let chain = match chain {
            Ok(c) => c,
            Err(e) => return no(e, self.version()),
        };
        self.edit_with(|bp| {
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
                    let [one]: [claudius::fx::Effect; 1] =
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
        })
    }

    /// run a script on the current song (or on none: the script starts one)
    pub fn run_script(self: &Arc<Self>, code: &str, what: Option<String>) -> Reply {
        let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let before = self.blueprint();
        match script::run_with(code, before.clone(), self.lib.clone(), self.roots.clone()) {
            Ok(out) => {
                let mut log = out.log.join("\n");
                if out.bp == before {
                    if log.is_empty() {
                        log = "ran; nothing changed".into();
                    }
                    let v = self.update(|s| {
                        s.say(log.clone());
                        s.version + 1
                    });
                    return ok(log, v);
                }
                let mut bp = out.bp;
                let label = what.clone().unwrap_or_else(|| {
                    let first =
                        code.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("//")).unwrap_or("");
                    format!(
                        "script: {}{}",
                        first.chars().take(80).collect::<String>(),
                        if code.lines().count() > 1 { " …" } else { "" }
                    )
                });
                if let Some(b) = bp.as_mut() {
                    b.lineage.push(label.clone());
                }
                let msg = if log.is_empty() { label.clone() } else { log };
                self.commit_as(bp, msg, label)
            }
            Err(e) => {
                let v = self.update(|s| {
                    s.say(format!("script error: {e}"));
                    s.version + 1
                });
                no(e, v)
            }
        }
    }

    pub fn command(self: &Arc<Self>, c: Command) -> Reply {
        let v = self.version();
        match c {
            Command::NewSong { genre, seed, bpm, key, loops } => {
                let g = match genres::get(&genre) {
                    Ok(g) => g,
                    Err(e) => {
                        return no(
                            format!(
                                "{e}; genres: {}",
                                self.genres.iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", ")
                            ),
                            v,
                        );
                    }
                };
                let key = match key.map(|k| Key::parse(&k).ok_or(k)).transpose() {
                    Ok(k) => k,
                    Err(k) => return no(format!("cannot read key '{k}' (try \"A minor\", \"F#m\", \"Eb\")"), v),
                };
                let seed = seed.unwrap_or_else(now_seed);
                let opts = Options { bpm, key, family: loops && self.lib.is_some(), ..Default::default() };
                let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
                let bp = compose::generate(&g, seed, &opts, self.lib.as_deref().map(|l| l as &dyn compose::Library));
                let msg = format!(
                    "new song: {} — {} at {:.0} BPM in {} (seed {seed})",
                    bp.title,
                    bp.genre,
                    bp.bpm,
                    bp.key.name()
                );
                self.lock().folder = None;
                self.commit(Some(bp), msg)
            }
            Command::RequestTake { from, role, name, note } => self.request_take(from, role, name, note),
            Command::RequestPitchCheck { low, high, note } => self.request_pitch_check(low, high, note),
            Command::CancelPitchCheck { id } => match self.try_update(|s| match &mut s.pitch_check {
                Some((_, c)) if c.id == id && c.status == TakeStatus::Waiting => {
                    c.status = TakeStatus::Cancelled;
                    s.say(format!("the pitch check will not be sung (check {id})"));
                    Ok(())
                }
                Some((_, c)) if c.id == id => Err(format!("pitch check {id} is answered already")),
                _ => Err(format!("no pitch check {id}")),
            }) {
                Ok(()) => ok(format!("pitch check {id}: not now, then"), self.version()),
                Err(e) => no(e, v),
            },
            Command::CancelTakeRequest { id } => match self.try_update(|s| match &mut s.take_request {
                Some(r) if r.id == id && r.status == TakeStatus::Waiting => {
                    r.status = TakeStatus::Cancelled;
                    let said = format!("the take asked for from {} will not be recorded (request {id})", r.from);
                    s.say(said);
                    Ok(())
                }
                Some(r) if r.id == id => Err(format!("request {id} is answered already")),
                _ => Err(format!("no take request {id}")),
            }) {
                Ok(()) => ok(format!("request {id}: no take, then"), self.version()),
                Err(e) => no(e, v),
            },
            Command::Load { path } => self.load(Path::new(&path)),
            Command::OpenText { name, text } => self.open_text(&name, &text, None),
            Command::Save { path } => self.save(path.map(PathBuf::from)),
            Command::Export { formats, path } => self.export(&formats, path.map(PathBuf::from)),
            Command::SetRelease { details, artwork } => self.set_release(*details, artwork.map(PathBuf::from)),
            Command::WriteKit { path } => self.write_kit(path.map(PathBuf::from)),
            Command::SetTempo { bpm } => self.run_script(&format!("tempo({bpm})"), Some(format!("tempo {bpm:.0}"))),
            Command::SetKey { key } => self.run_script(&format!("key({})", quote(&key)), Some(format!("key {key}"))),
            Command::SetSwing { swing } => {
                self.run_script(&format!("swing({swing})"), Some(format!("swing {swing:.2}")))
            }
            Command::SetProgression { name, chords } => self.run_script(
                &format!("prog({}, {})", quote(&name), quote(&chords.join(" "))),
                Some(format!("progression {name}: {}", chords.join(" "))),
            ),
            Command::SetDrumPattern { lane, variant, steps } => self.run_script(
                &format!("drum({}, {}, {})", quote(&lane), quote(&variant), quote(&steps)),
                Some(format!("{lane} ({variant}) pattern")),
            ),
            Command::SetPart { name, style, preset, register, density, reseed } => {
                let mut fields = Vec::new();
                if let Some(s) = &style {
                    fields.push(format!("style: {}", quote(s)));
                }
                if let Some(p) = &preset {
                    fields.push(format!("preset: {}", quote(p)));
                }
                if let Some((a, b)) = register {
                    fields.push(format!("register: [{a}, {b}]"));
                }
                if let Some(d) = density {
                    fields.push(format!("density: {d}"));
                }
                if reseed {
                    fields.push(format!("seed: {}", Rng::new(now_seed()).next_u64() as i64));
                }
                self.run_script(
                    &format!("part({}, #{{ {} }})", quote(&name), fields.join(", ")),
                    Some(format!("{name}: {}", fields.join(", "))),
                )
            }
            Command::SetMix { track, level_db, pan, reverb, delay, duck, muted, solo, bus } => {
                let mut fields = Vec::new();
                for (k, x) in [("level", level_db), ("pan", pan), ("reverb", reverb), ("delay", delay), ("duck", duck)]
                {
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
                    fields.push(format!("bus: {}", quote(&b)));
                }
                self.run_script(
                    &format!("mix({}, #{{ {} }})", quote(&track), fields.join(", ")),
                    Some(format!("mix {track}: {}", fields.join(", "))),
                )
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
            } => self.edit_with(|bp| {
                if let Some(p) = &prog
                    && !bp.harmony.progressions.contains_key(p)
                {
                    return Err(format!(
                        "no progression '{p}' (there are {})",
                        bp.harmony.progressions.keys().cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
                let n = bp.form.len();
                let s = bp.form.get_mut(index).ok_or(format!("no section {index} (there are {n})"))?;
                if let Some(x) = name {
                    s.name = x;
                }
                if let Some(x) = bars {
                    s.bars = x.clamp(1, 256);
                }
                if let Some(x) = energy {
                    s.energy = x.clamp(0.0, 1.0);
                }
                if let Some(x) = layers {
                    s.layers = x;
                }
                if let Some(x) = prog {
                    s.prog = x;
                }
                if let Some(x) = variant {
                    s.variant = x;
                }
                for (src, dst) in [
                    (filter_in, &mut s.filter_in),
                    (build, &mut s.build),
                    (gap, &mut s.gap),
                    (impact, &mut s.impact),
                    (fills, &mut s.fills),
                ] {
                    if let Some(x) = src {
                        *dst = x;
                    }
                }
                Ok(format!("section {index} ({})", s.name))
            }),
            Command::AddSection { after, kind, bars } => self.edit_with(|bp| {
                let k = section_kind(&kind)?;
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
                let base = kind.clone();
                let mut i = 1;
                spec.name = base.clone();
                while bp.form.iter().any(|s| s.name == spec.name) {
                    i += 1;
                    spec.name = format!("{base}{i}");
                }
                let at = (after + 1).min(bp.form.len());
                let name = spec.name.clone();
                bp.form.insert(at, spec);
                Ok(format!("added {name} ({bars} bars) at {at}"))
            }),
            Command::RemoveSection { index } => self.edit_with(|bp| {
                if bp.form.len() <= 1 {
                    return Err("a song needs at least one section".into());
                }
                if index >= bp.form.len() {
                    return Err(format!("no section {index}"));
                }
                let s = bp.form.remove(index);
                Ok(format!("removed {}", s.name))
            }),
            Command::MoveSection { from, to } => self.edit_with(|bp| {
                if from >= bp.form.len() || to >= bp.form.len() {
                    return Err(format!("sections are 0..{}", bp.form.len()));
                }
                let s = bp.form.remove(from);
                let name = s.name.clone();
                bp.form.insert(to, s);
                Ok(format!("moved {name} to {to}"))
            }),
            Command::ToggleLayer { section, layer } => self.edit_with(|bp| {
                let s = bp.form.get_mut(section).ok_or(format!("no section {section}"))?;
                if let Some(i) = s.layers.iter().position(|l| *l == layer) {
                    s.layers.remove(i);
                    Ok(format!("{layer} off in {}", s.name))
                } else {
                    s.layers.push(layer.clone());
                    Ok(format!("{layer} on in {}", s.name))
                }
            }),
            Command::SetDrumStep { lane, variant, step, velocity } => self.edit_with(|bp| {
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
                let hits = l.patterns.entry(variant.clone()).or_insert(main);
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
            }),
            Command::SetTrig { lane, variant, step, trig } => {
                let set = trig.map(|t| t.script()).unwrap_or_default();
                let code = format!("trig({}, {}, {step}, #{{ {set} }});", quote(&lane), quote(&variant));
                self.run_script(&code, Some(format!("{lane} {variant} step {step}")))
            }
            Command::RunScript { code } => self.run_script(&code, None),
            Command::AddTake { path, from, name, role } => {
                self.add_take_file(Path::new(&path), &from, name.as_deref(), role.as_deref())
            }
            Command::AddNotes { from, notes, sounds, name, role, style, request } => self.answer(
                request,
                self.edit_with(|bp| {
                    let notes: Vec<claudius::theory::Note> = notes
                        .iter()
                        .map(|n| claudius::theory::Note { t: n.t, d: n.d, p: n.p, v: n.v, lock: None })
                        .collect();
                    claudius::take::played(bp, &from, &notes, &claudius::take::Played { sounds, name, role, style })
                }),
            ),
            Command::TranscribeTake { take, sounds, name, role, chromatic, keep_take, style } => self.edit_with(|bp| {
                let m = claudius::take::Melody { sounds, name, role, chromatic, keep_take, style };
                claudius::take::melody(bp, &take, &m)
            }),
            Command::AssessVoice { take, path, key } => self.assess_voice(take, path, key),
            Command::Tune { track, strength, speed_ms, humanize, vibrato, scale, off } => {
                let profile = self.lock().voice.clone();
                let scale = match scale.as_deref().map(str::parse::<claudius::voice::tune::TuneScale>).transpose() {
                    Ok(s) => s,
                    Err(e) => return no(e, v),
                };
                self.edit_with(|bp| {
                    if off {
                        return claudius::voice::tune::put(bp, &track, None);
                    }
                    let was = script::mix_of(bp, &track)
                        .and_then(|m| m.fx.iter().find(|e| matches!(e, claudius::fx::Effect::Tune { .. })).cloned());
                    let mut t = was.unwrap_or_else(|| {
                        serde_json::from_value(serde_json::json!({ "fx": "tune" })).expect("a tuner from its defaults")
                    });
                    if let claudius::fx::Effect::Tune {
                        strength: s,
                        speed_ms: sp,
                        humanize: h,
                        vibrato: vb,
                        scale: sc,
                        ..
                    } = &mut t
                    {
                        if let Some(x) = strength {
                            *s = x.clamp(0.0, 1.0).into();
                        }
                        if let Some(x) = speed_ms {
                            *sp = x.clamp(0.0, 1000.0);
                        }
                        if let Some(x) = humanize {
                            *h = x.clamp(0.0, 100.0);
                        }
                        if let Some(x) = vibrato {
                            *vb = x.clamp(0.0, 1.0);
                        }
                        if let Some(x) = scale {
                            *sc = x;
                        }
                    }
                    // the person's voice, as their pitch check heard it, unless the tuner has one of its own
                    let mut as_checked = false;
                    if let (claudius::fx::Effect::Tune { voice: v @ None, .. }, Some(p)) = (&mut t, profile.as_ref()) {
                        (*v, as_checked) = (Some(p.clone()), true);
                    }
                    claudius::voice::tune::put(bp, &track, Some(t)).map(|said| {
                        if as_checked { format!("{said}, as your pitch check heard you sing") } else { said }
                    })
                })
            }
            Command::Analyze { path } => match self
                .readable(Path::new(&path))
                .and_then(|_| audio::load(Path::new(&path)).map_err(|e| e.to_string()))
            {
                Ok(x) => self.analyze_audio(&x, &path),
                Err(e) => no(format!("cannot read {path}: {e}"), v),
            },
            Command::Remake { path, genre, seed, bpm } => {
                if let Err(e) = self.readable(Path::new(&path)) {
                    return no(e, v);
                }
                match audio::load(Path::new(&path)) {
                    Ok(x) => self.remake_audio(&x, &path, genre, seed, bpm),
                    Err(e) => no(format!("cannot read {path}: {e}"), v),
                }
            }
            Command::SetFx { target, fx } => self.set_fx(&target, fx),
            Command::SampleSong { path, count, bars, find, sections } => {
                let opt = sampling(count, bars, find, sections);
                if let Err(e) = self.readable(Path::new(&path)) {
                    return no(e, v);
                }
                match audio::load(Path::new(&path)) {
                    Ok(x) => {
                        let name =
                            Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
                        self.sample_audio(&x, &name, &opt)
                    }
                    Err(e) => no(format!("cannot read {path}: {e}"), v),
                }
            }
            Command::Mutate { count, keep_best } => {
                self.start_job("mutating", move |app| app.mutate_job(count.clamp(1, 32), keep_best))
            }
            Command::Evolve { generations, children } => {
                self.start_job("evolving", move |app| app.evolve_job(generations.clamp(1, 20), children.clamp(2, 32)))
            }
            Command::InstallPacks { names } => self.install_packs(names),
            Command::Render { stems } => {
                if self.blueprint().is_none() {
                    return no("no song yet", v);
                }
                {
                    let s = self.lock();
                    if !stems && !s.job && s.render.as_ref().is_some_and(|r| r.view.version >= s.song_version) {
                        return ok("the render is up to date", v);
                    }
                }
                if self.lock().job {
                    self.lock().rerender = Some(stems);
                    return ok("rendering after the job in progress", v);
                }
                self.start_render(stems, Duration::ZERO);
                ok("rendering", self.version())
            }
            Command::Undo => self.history(true),
            Command::Redo => self.history(false),
            Command::Cancel => {
                self.cancel.store(true, Ordering::Relaxed);
                ok("stopping after the current step", v)
            }
        }
    }

    fn history(self: &Arc<Self>, undo: bool) -> Reply {
        let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let r = self.try_update(|s| {
            let Some((prev, label)) = (if undo { s.undo.pop() } else { s.redo.pop() }) else {
                return Err(if undo { "nothing to undo" } else { "nothing to redo" });
            };
            let cur = std::mem::replace(&mut s.bp, prev);
            if undo {
                s.redo.push((cur, label.clone()));
            } else {
                s.undo.push((cur, label.clone()));
            }
            s.view = s.bp.as_ref().map(|b| views::song(b, &s.loudness));
            let msg = format!("{}: {label}", if undo { "undid" } else { "redid" });
            s.say(msg.clone());
            Ok((msg, s.version + 1))
        });
        match r {
            Ok((msg, v)) => {
                self.after_change();
                ok(msg, v)
            }
            Err(e) => no(e, self.version()),
        }
    }

    /// asks the person for a take of section `from`: the studio opens its Record window for it
    fn request_take(self: &Arc<Self>, from: String, role: Option<String>, name: Option<String>, note: String) -> Reply {
        let v = self.version();
        let Some(song) = self.lock().view.clone() else { return no("no song yet: make one first", v) };
        if !song.sections.iter().any(|s| s.name == from) {
            let names: Vec<&str> = song.sections.iter().map(|s| s.name.as_str()).collect();
            return no(format!("no section '{from}' (the song has {})", names.join(", ")), v);
        }
        if let Some(r) = role.as_deref().filter(|r| !claudius::take::ROLES.contains(r) && !PART_ROLES.contains(r)) {
            let all: Vec<&str> = claudius::take::ROLES.iter().chain(PART_ROLES).copied().collect();
            return no(format!("a take is {}, not '{r}'", all.join(", ")), v);
        }
        let id = self.update(|s| {
            s.asked += 1;
            let id = s.asked;
            s.say(format!("asked the person for a take from {from} (request {id})"));
            s.take_request = Some(TakeRequest { id, from: from.clone(), role, name, note, ..Default::default() });
            id
        });
        ok(format!("asked the person for a take from {from} (request {id})"), self.version())
    }

    /// asks the person for a pitch check of phrases between MIDI notes `low` and `high`: the studio opens its Record
    /// window for it
    fn request_pitch_check(self: &Arc<Self>, low: Option<i32>, high: Option<i32>, note: String) -> Reply {
        let (low, high) = (low.unwrap_or(57), high.unwrap_or(69));
        if !(36..=84).contains(&low) || !(36..=84).contains(&high) || high < low + 7 {
            return no(
                "a pitch check's notes go from low to high, MIDI 36 to 84, at least a fifth apart",
                self.version(),
            );
        }
        let check = claudius::voice::check::Check::new(low, high, now_seed());
        let notes = check.notes().iter().map(|&(p, t, d)| claudius_api::NoteView { t, d, p, v: 0.8 }).collect();
        let (answers, beats) = (check.answers(), check.beats());
        let id = self.update(|s| {
            s.asked += 1;
            let id = s.asked;
            s.say(format!("asked the person for a pitch check (check {id})"));
            let view = claudius_api::PitchCheck {
                id,
                bpm: claudius::voice::check::BPM,
                notes,
                answers,
                beats,
                note,
                ..Default::default()
            };
            s.pitch_check = Some((check, view));
            id
        });
        ok(
            format!("asked the person for a pitch check (check {id}): six phrases of four notes to sing back"),
            self.version(),
        )
    }

    /// `r`, which answers take request `request` (the take sent for it) when claudius kept the take
    pub(super) fn answer(&self, request: Option<u64>, r: Reply) -> Reply {
        let Some(id) = request.filter(|_| r.ok) else { return r };
        let answered = self.try_update(|s| match &mut s.take_request {
            Some(t) if t.id == id && t.status == TakeStatus::Waiting => {
                (t.status, t.answer) = (TakeStatus::Taken, Some(r.message.clone()));
                Ok(())
            }
            _ => Err(()),
        });
        Reply { version: if answered.is_ok() { self.version() } else { r.version }, ..r }
    }

    /// how a take of the song, or a recording at `path` (in `key` when given), is sung
    fn assess_voice(self: &Arc<Self>, take: Option<String>, path: Option<String>, key: Option<String>) -> Reply {
        let v = self.version();
        let heard = match (take, path) {
            (Some(take), _) => match self.blueprint() {
                Some(bp) => claudius::take::assess(&bp, &take).map(|a| (take, a)),
                None => Err("no song yet: start one with new_song".into()),
            },
            (None, Some(path)) => {
                let scale = match key.as_deref().map(|k| Key::parse(k).ok_or(k)) {
                    Some(Ok(k)) => claudius::voice::intent::Scale::key(&k),
                    Some(Err(k)) => return no(format!("cannot read the key '{k}' (try \"A minor\" or \"F#m\")"), v),
                    None => claudius::voice::intent::Scale::free(),
                };
                self.readable(Path::new(&path))
                    .and_then(|_| audio::load(Path::new(&path)).map_err(|e| format!("cannot read {path}: {e}")))
                    .map(|x| (path, claudius::voice::assess(&x, &scale)))
            }
            (None, None) => Err("name a take of the song or an audio file to hear".into()),
        };
        match heard {
            Ok((name, a)) => {
                let v = self.update(|s| {
                    s.say(format!("heard how {name} is sung"));
                    s.version + 1
                });
                ok(a.describe(), v)
            }
            Err(e) => no(e, v),
        }
    }

    /// what claudius hears in a recording (`name`: its path, or the name of the file it came in)
    pub(super) fn analyze_audio(self: &Arc<Self>, x: &audio::Stereo, name: &str) -> Reply {
        let text = analysis_text(&analysis::song::analyze(x, None));
        let v = self.update(|s| {
            s.say(format!("analyzed {name}"));
            s.version + 1
        });
        ok(text, v)
    }

    /// a new song on a recording's skeleton (`name`: its path, or the name of the file it came in)
    pub(super) fn remake_audio(
        self: &Arc<Self>,
        x: &audio::Stereo,
        path: &str,
        genre: Option<String>,
        seed: Option<u64>,
        bpm: Option<f64>,
    ) -> Reply {
        let v = self.version();
        let genre = genre.or_else(|| self.blueprint().map(|b| b.genre)).unwrap_or_else(|| "house".into());
        let g = match genres::get(&genre) {
            Ok(g) => g,
            Err(e) => return no(e.to_string(), v),
        };
        let a = analysis::song::analyze(x, None);
        let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let bp = analysis::song::to_blueprint(
            &a,
            &g,
            seed.unwrap_or_else(now_seed),
            &Options { bpm, ..Options::default() },
            self.lib.as_deref().map(|l| l as &dyn compose::Library),
            path,
        );
        let moved = if bpm.is_none() && bp.bpm != a.bpm.round() {
            format!(
                "\nat {:.0} BPM: the recording's {:.0} is outside {genre}'s {:.0}-{:.0}",
                bp.bpm, a.bpm, g.bpm.0, g.bpm.1
            )
        } else {
            String::new()
        };
        self.lock().folder = None;
        self.commit(Some(bp), format!("remade {path} as {genre}{moved}:\n{}", analysis_text(&a)))
    }

    fn load(self: &Arc<Self>, path: &Path) -> Reply {
        let v = self.version();
        let path = if path.is_dir() { path.join("blueprint.json") } else { path.to_path_buf() };
        if let Err(e) = self.readable(&path) {
            return no(e, v);
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return no(format!("cannot read {}: {e}", path.display()), v),
        };
        self.open_text(&path.display().to_string(), &text, Some(&path))
    }

    /// A song from a file's text: a blueprint (when `name` ends in `.json`) or a script. `file` is where claudius read
    /// it, when it did: a song folder's file opens that song, and saving writes there. A file that came by its text
    /// (`open_text`, from a studio that read it) makes a song of its own.
    pub(super) fn open_text(self: &Arc<Self>, name: &str, text: &str, file: Option<&Path>) -> Reply {
        let v = self.version();
        let folder = file.and_then(Path::parent);
        let said = if file.is_some() { "loaded" } else { "opened" };
        if Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
            let mut bp = match Blueprint::from_json(text) {
                Ok(bp) => bp,
                Err(e) => return no(format!("cannot open {name}: {e}"), v),
            };
            if let Some(folder) = folder {
                claudius::take::find_moved(&mut bp, folder);
            }
            let outside = bp.files().into_iter().find(|f| !config::path_allowed(Path::new(f), self.roots.as_deref()));
            let checked = bp.check_size().and_then(|_| script::check_patterns(&bp)).and_then(|_| match outside {
                Some(f) => Err(format!("the song plays {f}, which is outside the folders this claudius may read")),
                None => Ok(()),
            });
            match checked {
                Ok(()) => {
                    self.lock().folder = folder.map(Path::to_path_buf);
                    self.commit(Some(bp), format!("{said} {name}"))
                }
                Err(e) => no(format!("cannot open {name}: {e}"), v),
            }
        } else {
            let r = self.run_script(text, Some(format!("{said} {name}")));
            if r.ok {
                // a song folder's song.rhai opens that song; a script elsewhere makes a new one
                let folder = folder.filter(|d| d.join("blueprint.json").is_file()).map(Path::to_path_buf);
                self.update(|s| s.folder = folder);
            }
            r
        }
    }

    /// the latest render, when it is of the song as it is now, as files in `formats`, in `path` or the song's folder
    fn export(self: &Arc<Self>, formats: &[String], path: Option<PathBuf>) -> Reply {
        let v = self.version();
        if formats.is_empty() {
            return no("export as what? give formats such as mp3, flac:16 or opus (State.exports lists them)", v);
        }
        let specs: Vec<export::Spec> = match formats.iter().map(|f| f.parse()).collect() {
            Ok(s) => s,
            Err(e) => return no(format!("{e}"), v),
        };
        if let Some(why) = specs.iter().find_map(export::Spec::unavailable) {
            return no(why, v);
        }
        let Some(bp) = self.blueprint() else { return no("no song to export", v) };
        let rendered = {
            let s = self.lock();
            s.render.as_ref().map(|r| r.view.version).filter(|rv| *rv >= s.song_version)
        };
        let Some(version) = rendered else {
            return no("the song has changed since its last render: render it, then export", v);
        };
        let dir = path
            .unwrap_or_else(|| config::songs_dir().join(format!("{}-{}-{}", bp.genre, song::slug(&bp.title), bp.seed)));
        if !config::write_allowed(&dir, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
        }
        let master = match audio::load_once(&self.cache.join(format!("{version}.wav"))) {
            Ok(m) => m,
            Err(e) => return no(format!("cannot read the render: {e:#}"), v),
        };
        let stem = match song::slug(&bp.title) {
            s if s.is_empty() => "song".to_string(),
            s => s,
        };
        let files = export::file_names(&dir, &stem, &specs);
        let tags = export::tags(&bp.title);
        let written: Vec<anyhow::Result<()>> = std::thread::scope(|scope| {
            let jobs: Vec<_> = specs
                .iter()
                .zip(&files)
                .map(|(spec, file)| {
                    let (master, tags) = (&master, &tags);
                    scope.spawn(move || export::write(master, spec, tags, file))
                })
                .collect();
            jobs.into_iter().map(|j| j.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the encoder stopped")))).collect()
        });
        for (file, w) in files.iter().zip(&written) {
            if let Err(e) = w {
                return no(format!("cannot export {}: {e:#}", file.display()), v);
            }
        }
        let list: Vec<String> =
            files.iter().zip(&specs).map(|(f, s)| format!("{} ({})", f.display(), s.label(config::SR))).collect();
        self.update(|s| s.say(format!("exported {}", list.join(", "))));
        ok(format!("exported {}", list.join(", ")), self.version())
    }

    fn save(self: &Arc<Self>, path: Option<PathBuf>) -> Reply {
        let v = self.version();
        let Some(mut bp) = self.blueprint() else { return no("no song to save", v) };
        let dir = path
            .unwrap_or_else(|| config::songs_dir().join(format!("{}-{}-{}", bp.genre, song::slug(&bp.title), bp.seed)));
        if !config::write_allowed(&dir, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return no(format!("cannot write {}: {e}", dir.display()), v);
        }
        // the recordings go with the song: its takes are copied into the folder, and from now on it plays the copies
        match claudius::take::gather(&mut bp, &dir) {
            Ok(moves) if !moves.is_empty() => self.update(|s| {
                if let Some(b) = s.bp.as_mut() {
                    claudius::take::repoint(b, &moves);
                }
            }),
            Ok(_) => {}
            Err(e) => return no(format!("{e:#}"), v),
        }
        // the score now; the audio, stems and verdict once it is rendered
        for (file, text) in [("blueprint.json", bp.to_json()), ("song.rhai", script::to_script(&bp))] {
            if let Err(e) = std::fs::write(dir.join(file), text) {
                return no(format!("cannot write {}: {e}", dir.join(file).display()), v);
            }
        }
        // the release in the making, too, and the song lives in this folder from now on
        self.keep_release(&dir);
        self.update(|s| super::release::moved(s, dir.clone()));
        loop {
            let dir2 = dir.clone();
            if self.start_job("saving", move |app| app.render_job(true, vec![dir2])).ok {
                return ok(
                    format!(
                        "saving to {} (blueprint.json and song.rhai now, the audio after the render)",
                        dir.display()
                    ),
                    self.version(),
                );
            }
            // a job is running: the save renders when it is done (unless it ended just now: then try again)
            let queued = self.update(|s| {
                s.job.then(|| {
                    s.pending_saves.retain(|d| *d != dir);
                    s.pending_saves.push(dir.clone());
                    s.busy.clone().unwrap_or_else(|| "a job".into())
                })
            });
            if let Some(busy) = queued {
                return ok(
                    format!(
                        "saving to {}: blueprint.json and song.rhai now, the audio once the current job ({busy}) is done",
                        dir.display()
                    ),
                    self.version(),
                );
            }
        }
    }
}

fn section_kind(k: &str) -> Result<SectionKind, String> {
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

pub fn analysis_text(a: &analysis::song::Analysis) -> String {
    let mut out = format!(
        "{:.1} BPM (pulse clarity {:.2}), {} (confidence {:.2}), swing {:.2}, {} bars from {:.2} s\n",
        a.bpm,
        a.pulse_clarity,
        a.key.name(),
        a.key_confidence,
        a.swing,
        a.bars,
        a.first_downbeat
    );
    out.push_str(&format!(
        "progression A: {}  |  B: {}  ({} bar(s) per chord)\n",
        a.progression_a.join(" "),
        a.progression_b.join(" "),
        a.bars_per_chord
    ));
    for s in &a.sections {
        let mut l = Vec::new();
        for (on, n) in [(s.kick, "kick"), (s.bass, "bass"), (s.hats, "hats")] {
            if on {
                l.push(n);
            }
        }
        out.push_str(&format!(
            "bar {:3} +{:2}  {:7} energy {:.2}  {}\n",
            s.start_bar,
            s.bars,
            s.kind.name(),
            s.energy,
            l.join(" ")
        ));
    }
    let grid = claudius::analysis::song::grid;
    out.push_str(&format!(
        "kick  {}\nsnare {}\nhats  {}\nbass style: {}\n",
        grid(&a.kick_steps),
        grid(&a.snare_steps),
        grid(&a.hat_steps),
        a.bass_style
    ));
    out
}
