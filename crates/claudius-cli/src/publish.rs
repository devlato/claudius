//! `claudius publish`: a song checked as stores check a release, and a release kit for a distributor.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use claudius::model::Project;
use claudius::publish::{self, Level, Release, Report};
use claudius::{audio, song};

use crate::args::ReleaseArgs;

pub struct Request {
    pub song: PathBuf,
    pub artwork: Option<PathBuf>,
    pub details: ReleaseArgs,
    pub check: bool,
    pub out: Option<PathBuf>,
    pub json: bool,
}

fn apply(r: &mut Release, d: ReleaseArgs, artwork: Option<PathBuf>) -> Result<()> {
    let set = |field: &mut String, v: Option<String>| {
        if let Some(v) = v {
            *field = v.trim().to_string();
        }
    };
    set(&mut r.artist, d.artist);
    set(&mut r.title, d.title);
    set(&mut r.version, d.version);
    set(&mut r.album, d.album);
    set(&mut r.genre, d.genre);
    set(&mut r.subgenre, d.subgenre);
    set(&mut r.date, d.date);
    set(&mut r.label, d.label);
    set(&mut r.owner, d.owner);
    set(&mut r.isrc, d.isrc);
    set(&mut r.upc, d.upc);
    for (list, given) in [(&mut r.featuring, d.featuring), (&mut r.writers, d.writers), (&mut r.producers, d.producers)]
    {
        if !given.is_empty() {
            *list = given.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
    }
    if let Some(e) = d.explicit {
        r.explicit = e;
    }
    if let Some(m) = d.made {
        r.made = Some(m.parse()?);
    }
    if let Some(a) = artwork {
        r.artwork = Some(std::path::absolute(&a).unwrap_or(a));
    }
    Ok(())
}

fn print(r: &Release, title: &str, report: &Report) {
    let by = if r.artist.trim().is_empty() { String::new() } else { format!(" by {}", r.artist.trim()) };
    println!("{title}{by}");
    if let Some(a) = &report.artwork {
        println!("  {:<8} {}", "artwork", publish::describe(a));
    }
    println!(
        "  {:<8} {}, {:.1} LUFS, {:.1} dB true peak",
        "audio",
        publish::length(report.seconds),
        report.lufs,
        report.true_peak_db
    );
    if let Some(m) = r.made {
        println!("  {:<8} {}", "made", m.describe());
    }
    if !report.checks.is_empty() {
        println!();
    }
    for c in &report.checks {
        let mark = match c.level {
            Level::Problem => "--",
            Level::Warning => " !",
            Level::Note => "..",
        };
        println!("  {mark}  {:<8} {}", c.about.name(), c.message);
    }
}

pub fn run(r: Request) -> Result<()> {
    let dir = r.song.as_path();
    let need = |file: &str| -> Result<PathBuf> {
        let p = dir.join(file);
        if !p.is_file() {
            bail!(
                "{} has no {file}: publish the folder of a song claudius rendered (claudius render {} writes it \
                 again)",
                dir.display(),
                dir.join("blueprint.json").display()
            );
        }
        Ok(p)
    };
    let bp = song::read_blueprint(&need("blueprint.json")?)?;
    let project: Project = serde_json::from_str(&std::fs::read_to_string(need("project.json")?)?)
        .with_context(|| format!("cannot read {}", dir.join("project.json").display()))?;
    let wav = need("song.wav")?;
    let mut release = publish::load(dir, &bp);
    apply(&mut release, r.details, r.artwork)?;
    publish::save(dir, &release)?;
    let cover = match publish::artwork_file(dir, &release) {
        Some(p) => Some(std::fs::read(&p).with_context(|| format!("cannot read the artwork {}", p.display()))?),
        None => None,
    };
    let master = audio::load_once(&wav).with_context(|| format!("cannot read {}", wav.display()))?;
    let song = publish::Song { bp: &bp, project: &project, master: &master };
    let report = publish::check(&release, &song, cover.as_deref());
    if r.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print(&release, release.title(&bp), &report);
    }
    if !report.ready() {
        bail!(
            "{} problem(s), marked --, to fix before a distributor takes it; what you gave is kept in {}",
            report.problems(),
            dir.join(publish::FILE).display()
        );
    }
    let (Some(cover), false) = (cover, r.check) else {
        if !r.json {
            println!("\nready for a distributor: claudius publish {} writes the release kit", dir.display());
        }
        return Ok(());
    };
    let out = r.out.unwrap_or_else(|| dir.join("release"));
    let files = publish::write_kit(&out, &release, &song, &cover, &report)?;
    if !r.json {
        println!("\nwrote the release kit to {}:", out.display());
        let width = files.iter().filter_map(|(f, _)| f.file_name()).map(|n| n.len()).max().unwrap_or(0);
        for (f, what) in &files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            println!("  {name:<width$}  {what}");
        }
    }
    Ok(())
}
