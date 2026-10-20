//! One song end to end: blueprint -> project -> render, and the files a run leaves behind.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::audio;
use crate::compose;
use crate::judge::{self, EarsReport, Verdict};
use crate::model::{Blueprint, Project};
use crate::render::{self, Rendered, TrackReport};

#[derive(Serialize)]
pub struct Report<'a> {
    pub title: &'a str,
    pub genre: &'a str,
    pub bpm: f64,
    pub key: String,
    pub swing: f32,
    pub bars: u32,
    pub seconds: f64,
    pub progressions: Vec<(String, String)>,
    pub sections: Vec<String>,
    pub lufs: f64,
    pub true_peak_db: f32,
    pub premaster_lufs: f64,
    pub tracks: &'a [TrackReport],
    pub lineage: &'a [String],
}

pub fn report<'a>(bp: &'a Blueprint, p: &Project, r: &'a Rendered) -> Report<'a> {
    Report {
        title: &bp.title,
        genre: &bp.genre,
        bpm: bp.bpm,
        key: bp.key.name(),
        swing: bp.swing,
        bars: p.bars,
        seconds: (p.secs() * 10.0).round() / 10.0,
        progressions: bp.harmony.progressions.iter().map(|(k, v)| (k.clone(), v.join(" "))).collect(),
        sections: p.sections.iter().map(|s| format!("{} ({} bars, energy {:.2})", s.name, s.bars, s.energy)).collect(),
        lufs: (r.lufs * 10.0).round() / 10.0,
        true_peak_db: (r.true_peak_db * 10.0).round() / 10.0,
        premaster_lufs: (r.premaster_lufs * 10.0).round() / 10.0,
        tracks: &r.tracks,
        lineage: &bp.lineage,
    }
}

/// credits for the recorded instruments a project plays (the sample packs whose licences ask for attribution)
pub fn credits(p: &Project) -> String {
    let mut names: Vec<&str> = p
        .tracks
        .iter()
        .filter_map(|t| {
            if let crate::model::Sound::Instrument { name, .. } = &t.sound { Some(name.as_str()) } else { None }
        })
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() { String::new() } else { claudius_packs::credits_for(&names) }
}

pub fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub struct Written {
    pub dir: PathBuf,
    pub wav: PathBuf,
    pub mp3: PathBuf,
}

/// a song from its blueprint.json, checked: its size, and that its pattern tracks still read
pub fn read_blueprint(path: &Path) -> Result<Blueprint> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let bp = Blueprint::from_json(&text).map_err(|e| anyhow::anyhow!("cannot open {}: {e}", path.display()))?;
    bp.check_size().map_err(anyhow::Error::msg)?;
    crate::script::check_patterns(&bp).map_err(anyhow::Error::msg)?;
    Ok(bp)
}

/// render a blueprint and write song.wav, song.mp3, blueprint.json, song.rhai, project.json, report.json,
/// verdict.json, credits.txt when sampled instruments ask for credit (and stems/)
pub fn render_and_write(
    bp: &Blueprint,
    dir: &Path,
    stems: bool,
    ears: Option<&dyn Fn(&Project, &Rendered) -> Option<EarsReport>>,
) -> Result<(Project, Rendered, Written, Verdict)> {
    let project = compose::arrange(bp);
    let r = render::render(&project, &render::Options { stems, ..Default::default() })?;
    // the MP3 is encoded while the critic listens: both take seconds, and neither needs the other
    let (audio, e) = std::thread::scope(|s| {
        let audio = s.spawn(|| write_audio(dir, bp, &r));
        let e = ears.and_then(|f| f(&project, &r));
        (audio.join(), e)
    });
    let (wav, mp3) = audio.unwrap_or_else(|panic| std::panic::resume_unwind(panic))?;
    let verdict = judge::judge(&project, &r, e);
    write_documents(dir, bp, &project, &r, &verdict)?;
    Ok((project, r, Written { dir: dir.to_path_buf(), wav, mp3 }, verdict))
}

/// write a rendered song's folder: song.wav, song.mp3, blueprint.json, song.rhai, project.json,
/// report.json, verdict.json, credits.txt when sampled instruments ask for credit, and stems/ when they were rendered
/// (32-bit float, since a track can peak above full scale before the master's limiter)
pub fn write_folder(dir: &Path, bp: &Blueprint, project: &Project, r: &Rendered, verdict: &Verdict) -> Result<Written> {
    let (wav, mp3) = write_audio(dir, bp, r)?;
    write_documents(dir, bp, project, r, verdict)?;
    Ok(Written { dir: dir.to_path_buf(), wav, mp3 })
}

/// song.wav (24-bit) and song.mp3
fn write_audio(dir: &Path, bp: &Blueprint, r: &Rendered) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let wav = dir.join("song.wav");
    audio::save_wav(&wav, &r.master, false)?;
    let mp3 = dir.join("song.mp3");
    let spec: crate::export::Spec = crate::export::FOLDER_MP3.parse()?;
    crate::export::write(&r.master, &spec, &crate::export::tags(&bp.title), &mp3)?;
    Ok((wav, mp3))
}

/// everything in the folder but the audio
fn write_documents(dir: &Path, bp: &Blueprint, project: &Project, r: &Rendered, verdict: &Verdict) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("verdict.json"), serde_json::to_string_pretty(verdict)?)?;
    std::fs::write(dir.join("blueprint.json"), bp.to_json())?;
    std::fs::write(dir.join("song.rhai"), crate::script::to_script(bp))?;
    std::fs::write(dir.join("project.json"), serde_json::to_string_pretty(project)?)?;
    std::fs::write(dir.join("report.json"), serde_json::to_string_pretty(&report(bp, project, r))?)?;
    let c = credits(project);
    if !c.is_empty() {
        std::fs::write(dir.join("credits.txt"), c)?;
    }
    for (name, s) in &r.stems {
        audio::save_wav(&dir.join("stems").join(format!("{}.wav", slug(name))), s, true)?;
    }
    Ok(())
}

/// judge an audio file; with the project.json written next to it, the arrangement and theory checks run too
pub fn judge_file(path: &Path, ears: Option<&dyn Fn(&Project, &Rendered) -> Option<EarsReport>>) -> Result<Verdict> {
    let master = audio::load(path)?;
    let project: Option<Project> = path
        .parent()
        .map(|d| d.join("project.json"))
        .filter(|p| p.exists())
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    let r = Rendered {
        lufs: crate::loudness::integrated(&master),
        true_peak_db: crate::fx::true_peak_db(&master),
        premaster_lufs: f64::NAN,
        stems: Vec::new(),
        tracks: Vec::new(),
        master,
    };
    let p = project.unwrap_or_else(|| empty_project(&r));
    let e = ears.and_then(|f| f(&p, &r));
    Ok(judge::judge(&p, &r, e))
}

fn empty_project(r: &Rendered) -> Project {
    use crate::model::{Master, MixPlan};
    use crate::theory::scale::{Key, Mode};
    let fx = crate::fx::Effect::Gain { db: crate::dsp::env::Param::Const(0.0) };
    Project {
        title: String::new(),
        genre: String::new(),
        bpm: 120.0,
        key: Key::new(0, Mode::Minor),
        swing: 0.5,
        humanize: 0.0,
        bars: (r.master.secs() / 2.0) as u32,
        seed: 0,
        sections: Vec::new(),
        chords: Vec::new(),
        tracks: Vec::new(),
        mix: MixPlan {
            reverb: fx.clone(),
            delay: fx,
            buses: Default::default(),
            duck_release: 0.3,
            levels: Default::default(),
            master: Master { fx: Vec::new(), lufs: r.lufs as f32, ceiling_db: -1.0 },
        },
    }
}
