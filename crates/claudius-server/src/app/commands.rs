//! Edits and commands: every `Command` of the API, run against the session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use claudius::compose::{self, Options};
use claudius::model::*;
use claudius::theory::scale::Key;
use claudius::{analysis, audio, config, export, genres, script, song, take};
use claudius_api::*;

use super::{App, no, now_seed, ok};
pub use claudius_session::listen::{Remade, analysis_text, groove_name, sampling};
use claudius_session::{edit, listen, views};

/// the largest project the studio may send to open, as large as a file it reads
pub const MAX_PROJECT: usize = 300 << 20;

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

    /// an edit of the song, as `claudius_session::edit` makes it: a script run on it, or a change in place
    fn edit(self: &Arc<Self>, c: &Command) -> Reply {
        let seed = now_seed();
        match edit::script(c, self.blueprint().as_ref(), seed) {
            Some(Ok(s)) => self.run_script(&s.code, s.label),
            Some(Err(e)) => no(e, self.version()),
            None => self
                .edit_with(|bp| edit::change(c, bp, seed).unwrap_or_else(|| Err("that is no edit of the song".into()))),
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
        let c = match self.from_library(c) {
            Ok(c) => c,
            Err(e) => return no(e, v),
        };
        match c {
            Command::NewSong { genre, seed, bpm, key, loops, sampled, sample_search, complexity } => {
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
                if let Some(c) = complexity.filter(|c| !(0.0..=1.0).contains(c)) {
                    return no(format!("a complexity is from 0 (sparse) to 1 (layered), not {c}"), v);
                }
                let seed = seed.unwrap_or_else(now_seed);
                let sample_search = sample_search.as_deref().and_then(claudius::sampling::words);
                let opts = Options {
                    bpm,
                    key,
                    family: loops && self.lib.is_some(),
                    sampled,
                    sample_search,
                    complexity,
                    ..Default::default()
                };
                let on = claudius::sampling::sampled(&g, opts.sampled, opts.sample_search.as_deref());
                if let Some(wanted) = self.samples_wanted(&g, &opts) {
                    if let Err(busy) = self.take_slot(&format!("sampling for {}", g.name)) {
                        return busy;
                    }
                    // a song is newer once it is taken on: one turned away as busy is not
                    let asked = self.new_songs.fetch_add(1, Ordering::Relaxed) + 1;
                    let msg = format!(
                        "sampling for a {} song first: {wanted}, from free libraries under CC0, public domain or CC BY",
                        g.name
                    );
                    self.update(|s| s.say(msg.clone()));
                    self.spawn_job(move |app| app.sampled_song_job(g, seed, opts, asked));
                    return ok(msg, self.version());
                }
                self.new_songs.fetch_add(1, Ordering::Relaxed);
                let lib = if on { &self.samples_lib } else { &self.lib };
                let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
                let bp = compose::generate(&g, seed, &opts, lib.as_deref().map(|l| l as &dyn compose::Library));
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
            c @ (Command::SetTempo { .. }
            | Command::SetKey { .. }
            | Command::SetSwing { .. }
            | Command::SetProgression { .. }
            | Command::SetDrumPattern { .. }
            | Command::SetPart { .. }
            | Command::AddTrack { .. }
            | Command::Harmonize { .. }
            | Command::SwapSound { .. }
            | Command::GenerateTrack { .. }
            | Command::VaryTrack { .. }
            | Command::RemoveTrack { .. }
            | Command::SetNotes { .. }
            | Command::SetMix { .. }
            | Command::SetLane { .. }
            | Command::SetTrig { .. }
            | Command::RunScript { .. }
            | Command::SetSection { .. }
            | Command::AddSection { .. }
            | Command::RemoveSection { .. }
            | Command::MoveSection { .. }
            | Command::ToggleLayer { .. }
            | Command::SetDrumStep { .. }
            | Command::SetFx { .. }
            | Command::AddHits { .. }) => self.edit(&c),
            Command::ExtractGroove { path, name, lanes, bpm } => match self
                .readable(Path::new(&path))
                .and_then(|_| audio::load(Path::new(&path)).map_err(|e| format!("cannot read {path}: {e}")))
            {
                Ok(x) => {
                    let name = name.unwrap_or_else(|| groove_name(&path));
                    self.groove_audio(&x, &path, &name, &lanes, bpm)
                }
                Err(e) => no(e, v),
            },
            Command::AddTake { path, from, name, role } => {
                self.add_take_file(Path::new(&path), &from, name.as_deref(), role.as_deref())
            }
            Command::SamplePieces { path, seed, count, bars, find } => {
                let opt = sampling(count, bars, find, Vec::new());
                if let Err(e) = self.readable(Path::new(&path)) {
                    return no(e, v);
                }
                match audio::load(Path::new(&path)) {
                    Ok(x) => {
                        let name =
                            Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
                        let credits = claudius::take::credited(Path::new(&path));
                        self.offer_pieces(std::sync::Arc::new(x), &name, credits, opt, seed.unwrap_or(0))
                    }
                    Err(e) => no(format!("cannot read {path}: {e}"), v),
                }
            }
            Command::RedrawPieces { seed } => self.redraw_pieces(seed),
            Command::KeepPieces { keep, sections } => self.keep_pieces(&keep, sections),
            Command::TrimTake { track, start, end } => {
                let dir = claudius::take::dir(self.lock().folder.as_deref());
                if !claudius::config::write_allowed(&dir, self.roots.as_deref()) {
                    return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
                }
                self.edit_with(|bp| claudius::take::trim(bp, &track, start, end, &dir))
            }
            Command::AddScratch { from, track, bar, bars, moves, name, role, recording } => {
                match edit::scratch(track, bar, bars, &moves, recording.as_deref()) {
                    Ok(s) => self.add_scratch(&from, &s, name.as_deref(), role.as_deref()),
                    Err(e) => no(e, self.version()),
                }
            }
            ref c @ Command::AddNotes { request, .. } => self.answer(request, self.edit(c)),
            Command::TranscribeTake { take, sounds, name, role, chromatic, keep_take, style } => self.edit_with(|bp| {
                let m = claudius::take::Melody { sounds, name, role, chromatic, keep_take, style };
                claudius::take::melody(bp, &take, &m)
            }),
            Command::TranscribeDrums { take, path, bars, variant, bpm, keep_take } => {
                let o = claudius::take::Drums { bars, variant, keep_take };
                if let Some(b) = bpm.filter(|b| !(40.0..=250.0).contains(b)) {
                    return no(format!("a beat's tempo is 40 to 250 BPM, not {b}"), v);
                }
                match (take, path) {
                    (Some(take), _) => self.edit_with(|bp| claudius::take::drums(bp, &take, &o)),
                    (None, Some(path)) => match self
                        .readable(Path::new(&path))
                        .and_then(|_| audio::load(Path::new(&path)).map_err(|e| format!("cannot read {path}: {e}")))
                    {
                        Ok(x) => {
                            let named = Path::new(&path)
                                .file_name()
                                .and_then(|f| claudius::analysis::groove::tempo_in_name(&f.to_string_lossy()));
                            self.edit_with(|bp| claudius::take::drums_from(bp, &x, &path, bpm.or(named), &o))
                        }
                        Err(e) => no(e, v),
                    },
                    (None, None) => no("transcribe_drums wants a take, or the path of a recording", v),
                }
            }
            Command::AssessVoice { take, path, key } => self.assess_voice(take, path, key),
            Command::Tune { track, strength, speed_ms, humanize, vibrato, scale, off } => {
                let voice = self.lock().voice.clone();
                let asked = edit::Tuning { strength, speed_ms, humanize, vibrato, scale: scale.as_deref(), off };
                self.edit_with(|bp| edit::tune(bp, &track, &asked, voice.as_ref()))
            }
            Command::Analyze { path } => match self
                .readable(Path::new(&path))
                .and_then(|_| audio::load(Path::new(&path)).map_err(|e| e.to_string()))
            {
                Ok(x) => self.analyze_audio(&x, &path),
                Err(e) => no(format!("cannot read {path}: {e}"), v),
            },
            Command::Remake { path, genre, seed, bpm, sampled, flip } => {
                if let Err(e) = self.readable(Path::new(&path)) {
                    return no(e, v);
                }
                match audio::load(Path::new(&path)) {
                    Ok(x) => self.remake_audio(&x, &path, genre, seed, bpm, Remade::of(sampled, flip)),
                    Err(e) => no(format!("cannot read {path}: {e}"), v),
                }
            }
            Command::SampleSong { path, count, bars, find, sections } => {
                let opt = sampling(count, bars, find, sections);
                if let Err(e) = self.readable(Path::new(&path)) {
                    return no(e, v);
                }
                match audio::load(Path::new(&path)) {
                    Ok(x) => {
                        let name =
                            Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
                        self.sample_audio(&x, &name, &opt, &claudius::take::credited(Path::new(&path)))
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
                // a new song still being made (its samples gathered) renders when it is
                if self.blueprint().is_none() && !self.lock().job {
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

    /// The groove of the drums in recording `x` (`from`: its path or its name), kept with the song as `name`, and the
    /// lanes `lanes` (every drum lane when none) put in it.
    /// The groove of `x` (heard in the file `from`, a path or a name) for the lanes `lanes`, at `bpm` when given,
    /// else at the tempo GarageBand's catalogue has for it as an Apple Loop, else at one written in its name.
    pub(super) fn groove_audio(
        self: &Arc<Self>,
        x: &audio::Stereo,
        from: &str,
        name: &str,
        lanes: &[String],
        bpm: Option<f64>,
    ) -> Reply {
        if let Some(b) = bpm.filter(|b| !(40.0..=250.0).contains(b)) {
            return no(format!("a groove's tempo is 40 to 250 BPM, not {b}"), self.version());
        }
        let file = Path::new(from);
        // the library's own loops (chopped from recordings) carry a tempo claudius heard, not one it knows
        let apple = self.lib.as_ref().and_then(|lib| {
            let stem = file.file_stem().map(|s| s.to_string_lossy());
            let l = lib
                .loops
                .iter()
                .filter(|l| l.genre != "User")
                .find(|l| l.path == from || stem.as_deref() == Some(l.name.as_str()))?;
            (l.tempo > 0.0).then_some(l.tempo)
        });
        let named = file.file_name().and_then(|f| claudius::analysis::groove::tempo_in_name(&f.to_string_lossy()));
        let heard = match claudius::analysis::groove::hear(x, bpm.or(apple).or(named)) {
            Ok(h) => h,
            Err(e) => return no(format!("{from}: {e}"), self.version()),
        };
        self.edit_with(|bp| listen::groove(bp, &heard, from, name, lanes))
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

    /// A new song from a recording (`name`: its path, or the name of the file it came in), built on it as `how`
    /// says, the recording kept with the songs' takes; a song on its skeleton only when the recording has nothing a
    /// rework or a flip can be built on.
    pub(super) fn remake_audio(
        self: &Arc<Self>,
        x: &audio::Stereo,
        path: &str,
        genre: Option<String>,
        seed: Option<u64>,
        bpm: Option<f64>,
        how: Remade,
    ) -> Reply {
        let v = self.version();
        let genre = genre.or_else(|| self.blueprint().map(|b| b.genre)).unwrap_or_else(|| "house".into());
        let g = match genres::get(&genre) {
            Ok(g) => g,
            Err(e) => return no(e.to_string(), v),
        };
        let dir = take::dir(Some(&self.songs));
        if how != Remade::Skeleton && !config::write_allowed(&dir, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
        }
        let stems = claudius::stems::of(x, &mut |_| {});
        let a = analysis::song::analyze_with(x, stems.as_deref(), None);
        let seed = seed.unwrap_or_else(now_seed);
        let lib = self.lib.as_deref().map(|l| l as &dyn compose::Library);
        let name = Path::new(path).file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        let _g = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let ears = self.ears.get().and_then(|e| e.as_ref());
        let src = listen::Heard { x, path, name: &name, a: &a, stems: stems.as_deref() };
        let (bp, verb, told) = listen::remake(&src, &g, seed, bpm, how, &listen::With { lib, dir: &dir, ears });
        self.lock().folder = None;
        self.commit(Some(bp), format!("{verb} {path} as {genre}{told}:\n{}", analysis_text(&a)))
    }

    fn load(self: &Arc<Self>, path: &Path) -> Reply {
        let v = self.version();
        let path = if path.is_dir() { path.join("blueprint.json") } else { path.to_path_buf() };
        if let Err(e) = self.readable(&path) {
            return no(e, v);
        }
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            let name = path.file_name().map_or_else(|| "project.zip".into(), |n| n.to_string_lossy().into_owned());
            return match std::fs::read(&path) {
                Ok(zip) => self.open_project(&name, &zip),
                Err(e) => no(format!("cannot read {}: {e}", path.display()), v),
            };
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return no(format!("cannot read {}: {e}", path.display()), v),
        };
        self.open_text(&path.display().to_string(), &text, Some(&path))
    }

    /// a project the demo saved (`song::pack_project`), the zip named `name`, unpacked into a song folder of its own
    /// under ~/Music/claudius and opened there, with the samples it holds
    pub fn open_project(self: &Arc<Self>, name: &str, zip: &[u8]) -> Reply {
        let v = self.version();
        let songs = self.songs.clone();
        if !config::write_allowed(&songs, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", songs.display()), v);
        }
        match song::unpack_project(zip, name, &songs) {
            Ok(dir) => {
                let r = self.load(&dir);
                if !r.ok {
                    return r;
                }
                Reply { message: format!("{} (from {name}, unpacked into {})", r.message, dir.display()), ..r }
            }
            Err(e) => no(format!("{e:#}"), v),
        }
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
            let opened = format!("{said} {name}");
            let mut r = self.run_script(text, Some(opened.clone()));
            if r.ok {
                // a song folder's song.rhai opens that song; a script elsewhere makes a new one
                let folder = folder.filter(|d| d.join("blueprint.json").is_file()).map(Path::to_path_buf);
                self.update(|s| s.folder = folder);
                // what was opened, before what its script says it did (a lane laid back, a groove taken)
                if !r.message.starts_with(&opened) {
                    r.message = format!("{opened}\n{}", r.message);
                }
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
        let dir = path.unwrap_or_else(|| super::release::default_folder(&self.songs, &bp));
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
        let dir = path.unwrap_or_else(|| super::release::default_folder(&self.songs, &bp));
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
