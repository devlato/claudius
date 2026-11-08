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

/// what the recorded sounds `p` plays come from, each file's as `credited` says (`take::credited`): the recordings,
/// and the sample packs' instruments, of the tracks that play in it (a track with no notes and no clips is not heard)
pub fn sources(p: &Project, credited: impl Fn(&Path) -> crate::take::Sources) -> crate::take::Sources {
    use crate::model::Sound;
    let mut s = crate::take::Sources::default();
    for t in p.tracks.iter().filter(|t| !t.notes.is_empty() || !t.clips.is_empty()) {
        // a rack's every layer
        for sound in t.sound.each() {
            match sound {
                Sound::Instrument { name, .. } => {
                    s.add(crate::take::Sources { instruments: vec![name.clone()], ..Default::default() })
                }
                Sound::Sample { path, .. } | Sound::Loop { path, .. } => s.add(credited(Path::new(path.as_str()))),
                _ => {}
            }
        }
    }
    s
}

/// credits for the recorded sounds a project plays: the sample packs whose licences ask for attribution, and the
/// samples downloaded from online libraries, those its takes were cut from too
pub fn credits(p: &Project) -> String {
    credits_of(&sources(p, crate::take::credited))
}

/// the credits for `s`, as a song's credits.txt lists them: the sample packs, then the recordings
pub fn credits_of(s: &crate::take::Sources) -> String {
    let mut names: Vec<&str> = s.instruments.iter().map(String::as_str).collect();
    names.sort();
    names.dedup();
    let mut out = if names.is_empty() { String::new() } else { claudius_packs::credits_for(&names) };
    let samples = claudius_packs::online::credits_of(&s.recordings);
    if !samples.is_empty() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&samples);
    }
    out
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

/// A song as a project to open in claudius (`unpack_project`): a zip of its `blueprint.json` and `song.rhai`, and of
/// `files`, each at its path in it (`samples/...`, which the blueprint names them by), stored as they are.
pub fn pack_project(bp: &Blueprint, files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    // a fixed date: a browser has no clock zip may ask
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    let documents =
        [("blueprint.json", bp.to_json().into_bytes()), ("song.rhai", crate::script::to_script(bp).into_bytes())];
    for (name, bytes) in documents.iter().map(|(n, b)| (*n, b)).chain(files.iter().map(|(n, b)| (n.as_str(), b))) {
        zip.start_file(name, opts)?;
        zip.write_all(bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// the most a project unpacks to, and the most files it holds: a zip that says more is not opened
const MOST_UNPACKED: u64 = 2 << 30;
const MOST_FILES: usize = 10_000;

/// A project (`pack_project`) unpacked into a song folder of its own under `songs`, named after `name` (the zip's
/// file name): the folder, which opens as any song's. A project's folder zipped again, as Finder and Explorer zip one
/// (its files under a folder of its name, with Finder's `__MACOSX/` beside it), unpacks as the project in it. A file it
/// holds outside itself (`../`) is refused, and so is a zip with no blueprint.json, or one that unpacks to more than
/// 2 GB or 10,000 files.
pub fn unpack_project(bytes: &[u8], name: &str, songs: &Path) -> Result<PathBuf> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).with_context(|| format!("{name} is not a zip"))?;
    if zip.len() > MOST_FILES {
        anyhow::bail!("{name} holds {} files, more than a project does ({MOST_FILES}): it is not opened", zip.len());
    }
    // what Finder keeps of a file's attributes, which no project holds
    let finders =
        |p: &Path| p.starts_with("__MACOSX") || p.file_name().is_some_and(|f| f.to_string_lossy().starts_with("._"));
    let mut entries = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        match entry.enclosed_name() {
            Some(rel) if finders(&rel) => {}
            Some(rel) => entries.push((i, rel, entry.is_dir(), entry.size())),
            None => {
                let file = entry.name().map(|n| n.to_string()).unwrap_or_default();
                anyhow::bail!("{name} holds a file outside itself ({file}): it is not opened");
            }
        }
    }
    let blueprint = |rel: &Path, dir: bool, depth: usize| {
        !dir && rel.components().count() == depth && rel.ends_with("blueprint.json")
    };
    let root = match entries.iter().find(|(_, rel, dir, _)| blueprint(rel, *dir, 1)) {
        Some(_) => PathBuf::new(),
        None => entries
            .iter()
            .find(|(_, rel, dir, _)| blueprint(rel, *dir, 2))
            .and_then(|(_, rel, _, _)| rel.parent().map(Path::to_path_buf))
            .with_context(|| format!("{name} is not a claudius project: it holds no blueprint.json"))?,
    };
    let (mut files, mut total) = (Vec::with_capacity(entries.len()), 0u64);
    for (i, rel, is_dir, size) in entries {
        let Ok(rel) = rel.strip_prefix(&root) else { continue };
        if rel.as_os_str().is_empty() {
            continue;
        }
        total = total.saturating_add(size);
        files.push((i, rel.to_path_buf(), is_dir, size));
    }
    if total > MOST_UNPACKED {
        anyhow::bail!("{name} unpacks to {} MB, more than a project does: it is not opened", total >> 20);
    }
    let stem = slug(Path::new(name).file_stem().map_or(name.into(), |s| s.to_string_lossy()).as_ref());
    let stem = if stem.is_empty() { "project".to_string() } else { stem };
    let dir = (1..)
        .map(|n| songs.join(if n == 1 { stem.clone() } else { format!("{stem}-{n}") }))
        .find(|d| !d.exists())
        .expect("a name no folder has");
    let unpacked = files.into_iter().try_for_each(|(i, rel, is_dir, size)| -> Result<()> {
        use std::io::Read;
        let to = dir.join(rel);
        let folder = if is_dir { Some(to.as_path()) } else { to.parent() };
        if let Some(folder) = folder {
            std::fs::create_dir_all(folder).with_context(|| format!("cannot write {}", folder.display()))?;
        }
        if !is_dir {
            let mut out = std::fs::File::create(&to).with_context(|| format!("cannot write {}", to.display()))?;
            // no more than the zip says the file holds, whatever its compressed bytes would make
            let mut entry = zip.by_index(i)?.take(size);
            std::io::copy(&mut entry, &mut out).with_context(|| format!("cannot write {}", to.display()))?;
        }
        Ok(())
    });
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(dir)
}

/// a song from its blueprint.json, checked: its size, and that its pattern tracks still read; takes that moved with the
/// song folder are found in it
pub fn read_blueprint(path: &Path) -> Result<Blueprint> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut bp = Blueprint::from_json(&text).map_err(|e| anyhow::anyhow!("cannot open {}: {e}", path.display()))?;
    if let Some(folder) = path.parent() {
        crate::take::find_moved(&mut bp, folder);
    }
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

/// everything in the folder but the audio; the song's takes are copied into its `takes/`, and what the folder says
/// plays the copies
fn write_documents(dir: &Path, bp: &Blueprint, project: &Project, r: &Rendered, verdict: &Verdict) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut bp = bp.clone();
    let moves = crate::take::gather(&mut bp, dir)?;
    let mut project = project.clone();
    crate::take::repoint_tracks(&mut project, &moves);
    let (bp, project) = (&bp, &project);
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
        settled: None,
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
            bands: Default::default(),
            master: Master { fx: Vec::new(), lufs: r.lufs as f32, ceiling_db: -1.0 },
        },
    }
}
