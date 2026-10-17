//! The long jobs: rendering and judging, mutating, evolving, saving. One runs at a time, on its own thread.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use claudius::compose::{self};
use claudius::model::*;
use claudius::render::Rendered;
use claudius::rng::Rng;
use claudius::{audio, evolve, genres, judge, render, song};
use claudius_api::*;
use rayon::prelude::*;

use super::{App, RenderOut, no, now_seed, ok};
use crate::views;

impl App {
    pub(super) fn start_job(self: &Arc<Self>, what: &str, job: impl FnOnce(Arc<App>) + Send + 'static) -> Reply {
        if self.blueprint().is_none() {
            return no("no song yet", self.version());
        }
        let started = self.try_update(|s| {
            if s.job {
                return Err(());
            }
            s.job = true;
            s.busy = Some(what.to_string());
            s.progress = Some(0.0);
            Ok(())
        });
        if started.is_err() {
            return no(
                format!(
                    "busy ({}); try again when it is done, or cancel it",
                    self.lock().busy.clone().unwrap_or_default()
                ),
                self.version(),
            );
        }
        self.spawn_job(job);
        ok(format!("{what}…"), self.version())
    }

    /// run a job that holds the job slot, and hand the slot on when it ends
    fn spawn_job(self: &Arc<Self>, job: impl FnOnce(Arc<App>) + Send + 'static) {
        self.cancel.store(false, Ordering::Relaxed);
        let app = self.clone();
        std::thread::spawn(move || {
            let a = app.clone();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || job(a))).is_err() {
                app.update(|s| s.say("the job failed (internal error)"));
            }
            app.finish_job();
        });
    }

    pub(super) fn start_render(self: &Arc<Self>, stems: bool, delay: Duration) {
        let busy = self.lock().job;
        if busy {
            let mut s = self.lock();
            s.rerender = Some(s.rerender.unwrap_or(false) || stems);
            return;
        }
        self.start_job("rendering", move |app| {
            std::thread::sleep(delay);
            app.render_job(stems, Vec::new())
        });
    }

    fn finish_job(self: &Arc<Self>) {
        enum Next {
            Save(Vec<PathBuf>),
            Render(bool),
            Nothing,
        }
        let next = self.update(|s| {
            // a waiting save takes the slot straight away, so nobody sees the session idle in between; its render
            // of the song as it is now also serves as the fresh render
            if !s.pending_saves.is_empty() {
                s.busy = Some("saving".into());
                s.progress = Some(0.0);
                return Next::Save(std::mem::take(&mut s.pending_saves));
            }
            s.job = false;
            s.busy = None;
            s.progress = None;
            let stale = s.render.as_ref().is_none_or(|r| r.view.version < s.song_version);
            match s.rerender.take() {
                Some(stems) if stale || stems => Next::Render(stems),
                _ if stale && s.bp.is_some() && self.auto_render.load(Ordering::Relaxed) => Next::Render(false),
                _ => Next::Nothing,
            }
        });
        match next {
            Next::Save(dirs) => self.spawn_job(move |app| app.render_job(true, dirs)),
            Next::Render(stems) => self.start_render(stems, Duration::from_millis(100)),
            Next::Nothing => {}
        }
    }

    fn progress(&self, what: &str, p: f32) {
        self.update(|s| {
            s.busy = Some(what.to_string());
            s.progress = Some(p.clamp(0.0, 1.0));
        });
    }

    fn ears_fn<'a>(
        &'a self,
        genre: &'a Option<genres::GenreProfile>,
    ) -> Option<impl Fn(&Project, &Rendered) -> Option<judge::EarsReport> + Sync + 'a> {
        let ears = self.ears.get()?.as_ref()?;
        Some(move |_: &Project, r: &Rendered| {
            ears.report(&r.master, genre.as_ref().map(|g| (g.query.as_str(), g.alts.as_slice())), None, false).ok()
        })
    }

    /// render and judge the song as it is now, and write a song folder into each of `saves`
    pub(super) fn render_job(self: &Arc<Self>, stems: bool, saves: Vec<PathBuf>) {
        let (bp, version) = {
            let s = self.lock();
            (s.bp.clone(), s.song_version)
        };
        let Some(bp) = bp else { return };
        let t = Instant::now();
        self.progress("rendering", 0.05);
        let project = compose::arrange(&bp);
        let r = match render::render(
            &project,
            &render::Options { stems: stems || !saves.is_empty(), ..Default::default() },
        ) {
            Ok(r) => r,
            Err(e) => {
                self.update(|s| s.say(format!("render failed: {e}")));
                return;
            }
        };
        self.progress("listening", 0.75);
        let g = genres::get(&bp.genre).ok();
        let ears = self.ears_fn(&g);
        let e = ears.as_ref().and_then(|f| f(&project, &r));
        let verdict = judge::judge(&project, &r, e);
        let wav = self.cache.join(format!("{version}.wav"));
        if let Err(e) = audio::save_wav(&wav, &r.master, false) {
            self.update(|s| s.say(format!("cannot write the render: {e}")));
            return;
        }
        let mut stem_files = BTreeMap::new();
        for (name, x) in &r.stems {
            let p = self.cache.join(format!("{version}")).join(format!("{}.wav", song::slug(name)));
            if audio::save_wav(&p, x, false).is_ok() {
                stem_files.insert(name.clone(), p);
            }
        }
        let spb = 60.0 / bp.bpm;
        let view = RenderView {
            version,
            url: format!("/api/audio/{version}.mp3"),
            seconds: r.master.secs(),
            lufs: r.lufs,
            true_peak_db: r.true_peak_db,
            bpm: bp.bpm,
            sections: project
                .sections
                .iter()
                .map(|s| (s.name.clone(), s.start_bar as f64 * 4.0 * spb, (s.start_bar + s.bars) as f64 * 4.0 * spb))
                .collect(),
            waveform: views::waveform(&r, 1000),
            stems: stem_files.keys().cloned().collect(),
        };
        // each track as it sits in the master: its own level, its gain staging, then the master's gain
        let master_gain = if r.premaster_lufs.is_finite() { r.lufs - r.premaster_lufs } else { 0.0 };
        let mut loudness = BTreeMap::new();
        for t in &r.tracks {
            if let Some(l) = t.level_lufs {
                loudness.insert(t.name.clone(), ((l + t.gain_db as f64 + master_gain) * 10.0).round() / 10.0);
            }
        }
        // the whole folder from this render, so the score in it is the song its audio plays
        let saved: Vec<_> =
            saves.iter().map(|dir| (dir, song::write_folder(dir, &bp, &project, &r, &verdict))).collect();
        // encode the browser's copy now, so the first request for it does not wait
        let _ = self.audio_path(version, "mp3");
        let vv = views::verdict(&verdict);
        let secs = t.elapsed().as_secs_f64();
        self.update(|s| {
            // a newer render may have landed while this one ran
            if s.render.as_ref().is_none_or(|old| old.view.version <= version) {
                s.render = Some(RenderOut { view, stems: stem_files });
                s.verdict = Some(vv.clone());
                s.loudness = loudness;
                if s.song_version == version
                    && let Some(b) = &s.bp
                {
                    s.view = Some(views::song(b, &s.loudness));
                }
            }
            s.say(format!("rendered in {secs:.1} s: {:.1} LUFS, verdict {:.1} — {}", r.lufs, vv.total, vv.summary));
            for (dir, result) in &saved {
                match result {
                    Ok(_) => s.say(format!("saved {}", dir.display())),
                    Err(e) => s.say(format!("could not save {}: {e:#}", dir.display())),
                }
            }
        });
        self.prune(version);
    }

    /// keep the last few renders on disk
    fn prune(&self, newest: u64) {
        let Ok(rd) = std::fs::read_dir(&self.cache) else { return };
        let mut versions: Vec<u64> =
            rd.filter_map(|e| e.ok()?.path().file_stem()?.to_str()?.split('.').next()?.parse().ok()).collect();
        versions.sort();
        versions.dedup();
        let keep: Vec<u64> = versions.iter().rev().take(10).copied().collect();
        for v in versions.into_iter().filter(|v| !keep.contains(v) && *v < newest) {
            let _ = std::fs::remove_file(self.cache.join(format!("{v}.wav")));
            let _ = std::fs::remove_file(self.cache.join(format!("{v}.mp3")));
            let _ = std::fs::remove_dir_all(self.cache.join(format!("{v}")));
        }
    }

    pub(super) fn mutate_job(self: &Arc<Self>, count: usize, keep_best: bool) {
        let Some(bp) = self.blueprint() else { return };
        let g = match genres::get(&bp.genre) {
            Ok(g) => g,
            Err(e) => {
                self.update(|s| s.say(e.to_string()));
                return;
            }
        };
        let genre = Some(g.clone());
        let f = self.ears_fn(&genre);
        let ears: Option<evolve::Ears> = f.as_ref().map(|f| f as evolve::Ears);
        let lib = self.lib.as_deref().map(|l| l as &dyn compose::Library);
        self.progress("mutating: judging the current song", 0.05);
        let base = match evolve::evaluate(&bp, 32, ears) {
            Ok(v) => v,
            Err(e) => {
                self.update(|s| s.say(format!("cannot judge the song: {e}")));
                return;
            }
        };
        let mut rng = Rng::new(now_seed());
        let kids: Vec<(Blueprint, String)> = (0..count)
            .map(|_| {
                let n = 1 + rng.below(2);
                let mut child = bp.clone();
                let mut what = Vec::new();
                for _ in 0..n {
                    let (c, w) = evolve::mutate(&child, &g, lib, &base.issues, &mut rng);
                    child = c;
                    what.push(w);
                }
                (child, what.join("; "))
            })
            .collect();
        self.progress(&format!("mutating: judging {count} variations"), 0.2);
        let done = std::sync::atomic::AtomicUsize::new(0);
        let scored: Vec<(Blueprint, String, f64)> = kids
            .into_par_iter()
            .filter_map(|(child, what)| {
                if self.cancel.load(Ordering::Relaxed) {
                    return None;
                }
                let v = evolve::evaluate(&child, 32, ears).ok()?;
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                self.update(|s| {
                    s.progress = Some(0.2 + 0.8 * n as f32 / count as f32);
                    s.say(format!("  {:5.1}  {what}", v.total));
                });
                Some((child, what, v.total))
            })
            .collect();
        let best = scored.iter().max_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
        match best {
            Some((child, what, score)) if keep_best && *score >= base.total + 0.1 => {
                let mut child = child.clone();
                child.lineage.push(format!("mutated: {what}"));
                let msg = format!(
                    "kept the best of {} variations: {what} (it scored {score:.1} on the preview against {:.1} for the song before)",
                    scored.len(),
                    base.total
                );
                self.commit(Some(child), msg);
            }
            Some((_, _, score)) => {
                self.update(|s| {
                    s.say(format!("no variation beat the current song ({:.1}; best {score:.1})", base.total))
                });
            }
            None => self.update(|s| s.say("no variations were judged")),
        }
    }

    pub(super) fn evolve_job(self: &Arc<Self>, generations: usize, children: usize) {
        let Some(bp) = self.blueprint() else { return };
        let g = match genres::get(&bp.genre) {
            Ok(g) => g,
            Err(e) => {
                self.update(|s| s.say(e.to_string()));
                return;
            }
        };
        let genre = Some(g.clone());
        let f = self.ears_fn(&genre);
        let ears: Option<evolve::Ears> = f.as_ref().map(|f| f as evolve::Ears);
        let lib = self.lib.as_deref().map(|l| l as &dyn compose::Library);
        let settings = evolve::Settings {
            generations,
            children,
            seed: now_seed(),
            cancel: Some(self.cancel.clone()),
            ..Default::default()
        };
        let log = |line: &str| {
            let generation = line
                .strip_prefix("generation ")
                .and_then(|r| r.split(':').next())
                .and_then(|n| n.trim().parse::<usize>().ok());
            self.update(|s| {
                s.say(line.to_string());
                if let Some(n) = generation {
                    s.busy = Some(format!("evolving: generation {n} of {generations} done"));
                    s.progress = Some(n as f32 / generations as f32);
                }
            });
        };
        self.progress(&format!("evolving: generation 1 of {generations}"), 0.02);
        let all = match evolve::run(&g, vec![bp.clone()], lib, ears, &settings, &log) {
            Ok(a) => a,
            Err(e) => {
                self.update(|s| s.say(format!("evolution failed: {e}")));
                return;
            }
        };
        let start = all.iter().find(|c| c.generation == 0).map_or(0.0, |c| c.score);
        if let Some(best) = all.first().filter(|b| b.generation > 0 && b.score > start) {
            let mut child = best.blueprint.clone();
            if child.lineage.last().is_none_or(|l| !l.contains(&best.change)) {
                child.lineage.push(format!("evolved: {}", best.change));
            }
            let msg =
                format!("evolved over {} candidates: {start:.1} -> {:.1} ({})", all.len(), best.score, best.change);
            self.commit(Some(child), msg);
        } else {
            self.update(|s| {
                s.say(format!(
                    "evolution found nothing better than the current song ({start:.1}) in {} candidates",
                    all.len()
                ))
            });
        }
    }
}
