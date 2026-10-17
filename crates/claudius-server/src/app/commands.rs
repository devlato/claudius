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
use claudius::{analysis, audio, config, genres, script, song};
use claudius_api::*;

use super::{App, no, now_seed, ok};
use crate::views;

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
    fn edit_with(self: &Arc<Self>, f: impl FnOnce(&mut Blueprint) -> Result<String, String>) -> Reply {
        let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut bp) = self.blueprint() else { return no("no song yet: start one with new_song", self.version()) };
        let before = bp.clone();
        match f(&mut bp).and_then(|msg| bp.check_size().map(|_| msg)) {
            Ok(msg) if bp != before => self.commit(Some(bp), msg),
            Ok(_) => ok("nothing changed", self.version()),
            Err(e) => no(e, self.version()),
        }
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
                self.commit(Some(bp), msg)
            }
            Command::Load { path } => self.load(Path::new(&path)),
            Command::Save { path } => self.save(path.map(PathBuf::from)),
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
            Command::SetMix { track, level_db, pan, reverb, delay, duck, muted, solo } => {
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
                hits.retain(|h| (h.t - t).abs() > 1e-6);
                if velocity > 0.0 {
                    hits.push(Hit { t, v: velocity.min(1.0) });
                    hits.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
                }
                Ok(format!(
                    "{lane} ({variant}) step {step}: {}",
                    if velocity > 0.0 { format!("{velocity:.2}") } else { "off".into() }
                ))
            }),
            Command::RunScript { code } => self.run_script(&code, None),
            Command::Analyze { path } => match self
                .readable(Path::new(&path))
                .and_then(|_| audio::load(Path::new(&path)).map_err(|e| e.to_string()))
            {
                Ok(x) => {
                    let a = analysis::song::analyze(&x, None);
                    let text = analysis_text(&a);
                    let v = self.update(|s| {
                        s.say(format!("analyzed {path}"));
                        s.version + 1
                    });
                    ok(text, v)
                }
                Err(e) => no(format!("cannot read {path}: {e}"), v),
            },
            Command::Remake { path, genre, seed } => {
                if let Err(e) = self.readable(Path::new(&path)) {
                    return no(e, v);
                }
                let x = match audio::load(Path::new(&path)) {
                    Ok(x) => x,
                    Err(e) => return no(format!("cannot read {path}: {e}"), v),
                };
                let genre = genre.or_else(|| self.blueprint().map(|b| b.genre)).unwrap_or_else(|| "house".into());
                let g = match genres::get(&genre) {
                    Ok(g) => g,
                    Err(e) => return no(e.to_string(), v),
                };
                let a = analysis::song::analyze(&x, None);
                let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
                let bp = analysis::song::to_blueprint(
                    &a,
                    &g,
                    seed.unwrap_or_else(now_seed),
                    &Options::default(),
                    self.lib.as_deref().map(|l| l as &dyn compose::Library),
                    &path,
                );
                self.commit(Some(bp), format!("remade {path} as {genre}:\n{}", analysis_text(&a)))
            }
            Command::Mutate { count, keep_best } => {
                self.start_job("mutating", move |app| app.mutate_job(count.clamp(1, 32), keep_best))
            }
            Command::Evolve { generations, children } => {
                self.start_job("evolving", move |app| app.evolve_job(generations.clamp(1, 20), children.clamp(2, 32)))
            }
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
        if path.extension().is_some_and(|e| e == "json") {
            let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
            let bp = match serde_json::from_str::<Blueprint>(&text) {
                Ok(bp) => bp,
                Err(e) => return no(format!("{} is not a claudius blueprint: {e}", path.display()), v),
            };
            let outside = bp.files().into_iter().find(|f| !config::path_allowed(Path::new(f), self.roots.as_deref()));
            let checked = bp.check_size().and_then(|_| script::check_patterns(&bp)).and_then(|_| match outside {
                Some(f) => Err(format!("the song plays {f}, which is outside the folders this claudius may read")),
                None => Ok(()),
            });
            match checked {
                Ok(()) => self.commit(Some(bp), format!("loaded {}", path.display())),
                Err(e) => no(format!("cannot open {}: {e}", path.display()), v),
            }
        } else {
            self.run_script(&text, Some(format!("loaded {}", path.display())))
        }
    }

    fn save(self: &Arc<Self>, path: Option<PathBuf>) -> Reply {
        let v = self.version();
        let Some(bp) = self.blueprint() else { return no("no song to save", v) };
        let dir = path
            .unwrap_or_else(|| config::songs_dir().join(format!("{}-{}-{}", bp.genre, song::slug(&bp.title), bp.seed)));
        if !config::write_allowed(&dir, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return no(format!("cannot write {}: {e}", dir.display()), v);
        }
        // the score now; the audio, stems and verdict once it is rendered
        for (file, text) in [("blueprint.json", bp.to_json()), ("song.rhai", script::to_script(&bp))] {
            if let Err(e) = std::fs::write(dir.join(file), text) {
                return no(format!("cannot write {}: {e}", dir.join(file).display()), v);
            }
        }
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
