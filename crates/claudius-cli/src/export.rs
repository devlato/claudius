//! `claudius export`: a song's master as files in other formats.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use claudius::config::SR;
use claudius::export::{self, Spec};
use claudius::{audio, song};

pub struct Request {
    pub song: Option<PathBuf>,
    pub formats: Vec<String>,
    pub out: Option<PathBuf>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub list: bool,
}

fn list() {
    println!("format[:quality][@rate], e.g. mp3:192, flac:16, wav@44100:\n");
    for p in export::presets() {
        let note = p.unavailable().map(|_| "  (needs ffmpeg here)").unwrap_or_default();
        println!("  {:<10} {}{note}", p.to_string(), p.label(SR));
    }
    println!(
        "\nalso: wav:32 (float), aiff:16, alac:16, mp3:128 to 320, mp3:v0 to v9, aac:64 to 320, opus:32 to 256, ogg:0 to 10"
    );
}

/// the master and the title of `path`: a song folder's song.wav and blueprint title, or an audio file and its name
fn source(path: &Path) -> Result<(PathBuf, String, PathBuf)> {
    if path.is_dir() {
        let wav = path.join("song.wav");
        if !wav.is_file() {
            bail!("{} has no song.wav: render the song first, or name an audio file", path.display());
        }
        let title = song::read_blueprint(&path.join("blueprint.json"))
            .map(|bp| bp.title)
            .unwrap_or_else(|_| path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        return Ok((wav, title, path.to_path_buf()));
    }
    if !path.is_file() {
        bail!("no such song or file: {}", path.display());
    }
    let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((path.to_path_buf(), title, path.parent().map(Path::to_path_buf).unwrap_or_default()))
}

pub fn run(r: Request) -> Result<()> {
    if r.list {
        list();
        return Ok(());
    }
    let Some(path) = r.song else {
        bail!("export what? name a song folder or an audio file (claudius export --list shows the formats)")
    };
    if r.formats.is_empty() {
        bail!("export as what? --format mp3 (or flac, wav:16, aac, opus ...; claudius export --list shows them)");
    }
    let specs: Vec<Spec> = r.formats.iter().map(|f| f.parse()).collect::<Result<_>>()?;
    if let Some(why) = specs.iter().find_map(Spec::unavailable) {
        bail!(why);
    }
    let (master_path, title, folder) = source(&path)?;
    let title = r.title.unwrap_or(title);
    let mut tags = export::tags(&title);
    tags.artist = r.artist.unwrap_or_default();
    tags.album = r.album.unwrap_or_default();
    let stem = match song::slug(&title) {
        s if s.is_empty() => "song".to_string(),
        s => s,
    };
    let files = match r.out {
        Some(o) if specs.len() == 1 && !o.is_dir() => vec![o],
        Some(o) => export::file_names(&o, &stem, &specs),
        None => export::file_names(&folder, &stem, &specs),
    };
    let master = audio::load_once(&master_path).with_context(|| format!("cannot read {}", master_path.display()))?;
    let t = std::time::Instant::now();
    let written: Vec<Result<(PathBuf, String)>> = std::thread::scope(|s| {
        let jobs: Vec<_> = specs
            .iter()
            .zip(&files)
            .map(|(spec, file)| {
                let (master, tags) = (&master, &tags);
                s.spawn(move || export::write(master, spec, tags, file).map(|_| (file.clone(), spec.label(SR))))
            })
            .collect();
        jobs.into_iter().map(|j| j.join().expect("export thread")).collect()
    });
    for w in written {
        let (file, label) = w?;
        let mb = std::fs::metadata(&file).map(|m| m.len() as f64 / 1e6).unwrap_or(0.0);
        println!("wrote {} ({label}, {mb:.1} MB)", file.display());
    }
    eprintln!("exported in {:.1} s", t.elapsed().as_secs_f64());
    Ok(())
}
