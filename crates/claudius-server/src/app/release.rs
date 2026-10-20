//! The release the session prepares for the open song, as `claudius publish` does: what stores ask about it, its
//! artwork, what they would say of its latest render, and the release kit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use claudius::audio::Stereo;
use claudius::model::{Blueprint, Project};
use claudius::publish::{self, Level, Made, Release};
use claudius::{audio, compose, config, song};
use claudius_api::{ReleaseCheck, ReleaseDetails, ReleaseView, Reply};

use super::{App, Session, no, ok};

/// the largest artwork the studio may send: well past what any store takes
pub const MAX_ARTWORK: usize = 40 << 20;

/// the side of the artwork's preview
const PREVIEW: u32 = 512;

/// a song's release in the making
pub(super) struct Prepared {
    /// the song it is for: its genre, its seed and its folder, which a new song, a remake or an open changes
    song: (String, u64, Option<PathBuf>),
    details: Release,
    artwork: Option<Arc<Vec<u8>>>,
    /// the session's version when the artwork came, so that no two pictures share one
    artwork_version: u64,
    /// what the artwork is, or why it cannot be read
    artwork_label: Option<String>,
    preview: Option<(u64, Arc<Vec<u8>>)>,
    /// the checks, and the render they are of
    report: Option<(u64, publish::Report)>,
    kit: Option<PathBuf>,
    /// something was given for it, or it was read from release.json: saving the song keeps it
    given: bool,
}

impl Prepared {
    fn set_artwork(&mut self, bytes: Arc<Vec<u8>>, version: u64) {
        self.artwork_label = Some(match publish::inspect(&bytes) {
            Ok(a) => publish::describe(&a),
            Err(why) => format!("the file {why}"),
        });
        self.artwork = Some(bytes);
        self.artwork_version = version;
        self.preview = None;
    }
}

/// the open song lives in `dir` now (it was saved there): its release stays as it is
pub(super) fn moved(s: &mut Session, dir: PathBuf) {
    if let Some(p) = s.release.as_mut() {
        p.song.2 = Some(dir.clone());
    }
    s.folder = Some(dir);
}

/// where Save puts a song that has no folder yet
pub(super) fn default_folder(bp: &Blueprint) -> PathBuf {
    config::songs_dir().join(format!("{}-{}-{}", bp.genre, song::slug(&bp.title), bp.seed))
}

fn to_api(r: &Release) -> ReleaseDetails {
    ReleaseDetails {
        title: r.title.clone(),
        version: r.version.clone(),
        artist: r.artist.clone(),
        featuring: r.featuring.clone(),
        album: r.album.clone(),
        genre: r.genre.clone(),
        subgenre: r.subgenre.clone(),
        date: r.date.clone(),
        label: r.label.clone(),
        writers: r.writers.clone(),
        producers: r.producers.clone(),
        owner: r.owner.clone(),
        isrc: r.isrc.clone(),
        upc: r.upc.clone(),
        explicit: r.explicit,
        made: r.made.map(|m| m.name().to_string()),
    }
}

fn from_api(d: ReleaseDetails, artwork: Option<PathBuf>) -> Result<Release, String> {
    let made = d.made.as_deref().filter(|m| !m.trim().is_empty()).map(str::parse::<Made>).transpose();
    let names = |v: Vec<String>| -> Vec<String> {
        v.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    Ok(Release {
        title: d.title.trim().into(),
        version: d.version.trim().into(),
        artist: d.artist.trim().into(),
        featuring: names(d.featuring),
        album: d.album.trim().into(),
        genre: d.genre.trim().into(),
        subgenre: d.subgenre.trim().into(),
        date: d.date.trim().into(),
        label: d.label.trim().into(),
        writers: names(d.writers),
        producers: names(d.producers),
        owner: d.owner.trim().into(),
        isrc: d.isrc.trim().into(),
        upc: d.upc.trim().into(),
        explicit: d.explicit,
        made: made.map_err(|e| format!("{e:#}"))?,
        artwork,
    })
}

fn level(l: Level) -> &'static str {
    match l {
        Level::Problem => "problem",
        Level::Warning => "warning",
        Level::Note => "note",
    }
}

/// a report as lines, problems marked `--`, warnings `!` and notes `..`, as `claudius publish` prints them
pub fn report_lines(checks: &[ReleaseCheck]) -> String {
    checks
        .iter()
        .map(|c| {
            let mark = match c.level.as_str() {
                "problem" => "--",
                "warning" => " !",
                _ => "..",
            };
            format!("{mark} {}: {}", c.about, c.message)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl App {
    /// the release of the open song, read from its folder's release.json (or begun from the songs next to it)
    /// when the song is new to it
    pub(super) fn prepared<'a>(&self, s: &'a mut Session) -> Option<&'a mut Prepared> {
        let bp = s.bp.as_ref()?;
        let key = (bp.genre.clone(), bp.seed, s.folder.clone());
        if s.release.as_ref().is_none_or(|p| p.song != key) {
            let dir = s.folder.clone().unwrap_or_else(|| default_folder(bp));
            let mut details = publish::load(&dir, bp);
            // the song's own title, which follows the song as it changes, unless one was given
            if details.title == bp.title && !dir.join(publish::FILE).exists() {
                details.title.clear();
            }
            let artwork = publish::artwork_file(&dir, &details)
                .filter(|f| config::path_allowed(f, self.roots.as_deref()))
                .and_then(|f| std::fs::read(f).ok());
            let mut p = Prepared {
                song: key,
                details,
                artwork: None,
                artwork_version: 0,
                artwork_label: None,
                preview: None,
                report: None,
                kit: None,
                given: dir.join(publish::FILE).is_file(),
            };
            if let Some(a) = artwork {
                p.set_artwork(Arc::new(a), s.version);
            }
            s.release = Some(p);
        }
        s.release.as_mut()
    }

    pub(super) fn release_view(&self, s: &mut Session) -> Option<ReleaseView> {
        let title = s.bp.as_ref()?.title.clone();
        let p = self.prepared(s)?;
        let report = p.report.as_ref();
        Some(ReleaseView {
            details: to_api(&p.details),
            song_title: title,
            artwork: p.artwork_label.clone(),
            artwork_version: p.artwork_version,
            checks: report
                .map(|(_, r)| {
                    r.checks
                        .iter()
                        .map(|c| ReleaseCheck {
                            level: level(c.level).into(),
                            about: c.about.name().into(),
                            message: c.message.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            checked: report.map_or(0, |(v, _)| *v),
            ready: report.is_some_and(|(_, r)| r.ready()),
            kit: p.kit.as_ref().map(|k| k.display().to_string()),
            made_options: Made::ALL.iter().map(|m| (m.name().to_string(), m.describe().to_string())).collect(),
        })
    }

    pub(super) fn set_release(self: &Arc<Self>, details: ReleaseDetails, artwork: Option<PathBuf>) -> Reply {
        let v = self.version();
        let bytes = match &artwork {
            Some(p) => {
                if let Err(e) = self.readable(p) {
                    return no(e, v);
                }
                match std::fs::read(p) {
                    Ok(b) if b.len() > MAX_ARTWORK => return no(format!("{} is over 40 MB", p.display()), v),
                    Ok(b) => Some(Arc::new(b)),
                    Err(e) => return no(format!("cannot read {}: {e}", p.display()), v),
                }
            }
            None => None,
        };
        let set = self.try_update(|s| {
            let version = s.version + 1;
            let p = self.prepared(s).ok_or_else(|| "no song to release: make one first".to_string())?;
            p.details = from_api(details, artwork.clone().or_else(|| p.details.artwork.clone()))?;
            p.given = true;
            if let Some(b) = bytes {
                p.set_artwork(b, version);
            }
            Ok::<_, String>(())
        });
        if let Err(e) = set {
            return no(e, v);
        }
        self.recheck();
        ok(self.release_summary(), self.version())
    }

    /// the artwork, as the studio sends it
    pub fn set_artwork_bytes(self: &Arc<Self>, bytes: Vec<u8>) -> Reply {
        let v = self.version();
        if bytes.len() > MAX_ARTWORK {
            return no("the artwork is over 40 MB", v);
        }
        let set = self.try_update(|s| {
            let version = s.version + 1;
            let p = self.prepared(s).ok_or_else(|| "no song to release: make one first".to_string())?;
            p.set_artwork(Arc::new(bytes), version);
            p.details.artwork = None;
            p.given = true;
            Ok::<_, String>(p.artwork_label.clone().unwrap_or_default())
        });
        match set {
            Ok(label) => {
                self.recheck();
                ok(format!("artwork: {label}"), self.version())
            }
            Err(e) => no(e, v),
        }
    }

    /// the artwork as it was given
    pub fn artwork(&self) -> Option<Arc<Vec<u8>>> {
        self.lock().release.as_ref()?.artwork.clone()
    }

    /// the artwork at most 512 pixels on a side, as PNG (made once for each artwork)
    pub fn artwork_preview(&self) -> Option<Arc<Vec<u8>>> {
        let (bytes, version) = {
            let s = self.lock();
            let p = s.release.as_ref()?;
            if let Some((v, png)) = &p.preview
                && *v == p.artwork_version
            {
                return Some(png.clone());
            }
            (p.artwork.clone()?, p.artwork_version)
        };
        let png = Arc::new(publish::preview(&bytes, PREVIEW)?);
        let mut s = self.lock();
        if let Some(p) = s.release.as_mut().filter(|p| p.artwork_version == version) {
            p.preview = Some((version, png.clone()));
        }
        Some(png)
    }

    fn release_summary(&self) -> String {
        let mut s = self.lock();
        let Some(view) = self.release_view(&mut s) else { return "no song".into() };
        if view.checked == 0 {
            return "release details kept; they are checked once the song is rendered".into();
        }
        let problems = view.checks.iter().filter(|c| c.level == "problem").count();
        let state = if problems == 0 {
            "ready for a distributor".to_string()
        } else {
            format!("{problems} problem(s) to fix before a distributor takes it")
        };
        format!("{state}\n{}", report_lines(&view.checks))
    }

    /// check the release again, against the latest render when it is of the song as it is now
    pub(super) fn recheck(self: &Arc<Self>) {
        let (bp, version) = {
            let s = self.lock();
            let Some(bp) = s.bp.clone() else { return };
            let current = s.render.as_ref().map(|r| r.view.version).filter(|v| *v >= s.song_version);
            let Some(version) = current else { return };
            (bp, version)
        };
        let Ok(master) = audio::load_once(&self.cache.join(format!("{version}.wav"))) else { return };
        self.check_render(&bp, version, &compose::arrange(&bp), &master);
    }

    /// what stores would say of render `version` of `bp`, kept with the release, once one is in the making: until
    /// something is given for it, a render has no checks to pay for
    pub(super) fn check_render(&self, bp: &Blueprint, version: u64, project: &Project, master: &Stereo) {
        let (details, artwork, artwork_version) = {
            let s = self.lock();
            let Some(p) = s.release.as_ref().filter(|p| p.given) else { return };
            (p.details.clone(), p.artwork.clone(), p.artwork_version)
        };
        let report =
            publish::check(&details, &publish::Song { bp, project, master }, artwork.as_deref().map(|a| &a[..]));
        self.update(|s| {
            // what changed meanwhile is checked again by whatever changed it
            if let Some(p) = s.release.as_mut().filter(|p| p.details == details && p.artwork_version == artwork_version)
            {
                p.report = Some((version, report));
            }
        });
    }

    pub(super) fn write_kit(self: &Arc<Self>, path: Option<PathBuf>) -> Reply {
        let v = self.version();
        let Some(bp) = self.blueprint() else { return no("no song to release", v) };
        let (version, details, artwork, folder) = {
            let mut s = self.lock();
            let current = s.render.as_ref().map(|r| r.view.version).filter(|rv| *rv >= s.song_version);
            let folder = s.folder.clone();
            let Some(p) = self.prepared(&mut s) else { return no("no song to release", v) };
            (current, p.details.clone(), p.artwork.clone(), folder)
        };
        let Some(version) = version else {
            return no("the song has changed since its last render: render it, then write the kit", v);
        };
        let Some(artwork) = artwork else {
            return no("add the artwork first: stores need a square JPEG or PNG, 3000 × 3000", v);
        };
        let song_dir = folder.unwrap_or_else(|| default_folder(&bp));
        let dir = path.unwrap_or_else(|| song_dir.join("release"));
        for d in [&dir, &song_dir] {
            if !config::write_allowed(d, self.roots.as_deref()) {
                return no(format!("{} is outside the folders this claudius may write", d.display()), v);
            }
        }
        let master = match audio::load_once(&self.cache.join(format!("{version}.wav"))) {
            Ok(m) => m,
            Err(e) => return no(format!("cannot read the render: {e:#}"), v),
        };
        let project = compose::arrange(&bp);
        let song = publish::Song { bp: &bp, project: &project, master: &master };
        let report = publish::check(&details, &song, Some(&artwork));
        if !report.ready() {
            let first = report.checks.iter().find(|c| c.level == Level::Problem).map(|c| c.message.clone());
            let n = report.problems();
            self.update(|s| {
                if let Some(p) = s.release.as_mut() {
                    p.report = Some((version, report));
                }
            });
            return no(format!("{n} problem(s) to fix first: {}", first.unwrap_or_default()), self.version());
        }
        let files = match publish::write_kit(&dir, &details, &song, &artwork, &report) {
            Ok(f) => f,
            Err(e) => return no(format!("cannot write the release kit: {e:#}"), v),
        };
        // the details and the artwork go with the song, for the next time it is opened
        let kept = keep(&song_dir, &details, Some(&artwork));
        let names: Vec<String> =
            files.iter().filter_map(|(f, _)| f.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
        let msg = format!("wrote the release kit to {}: {}", dir.display(), names.join(", "));
        self.update(|s| {
            if let Some(p) = s.release.as_mut() {
                p.report = Some((version, report));
                p.kit = Some(dir.clone());
                if let Ok(Some(name)) = &kept {
                    p.details.artwork = Some(name.clone());
                }
            }
            s.say(msg.clone());
            if let Err(e) = &kept {
                s.say(format!("could not keep the release details with the song: {e:#}"));
            }
        });
        ok(msg, self.version())
    }

    /// keep the release's details and artwork in the folder a song is saved to
    pub(super) fn keep_release(&self, dir: &Path) {
        let (details, artwork) = {
            let s = self.lock();
            let Some(p) = s.release.as_ref().filter(|p| p.given) else { return };
            (p.details.clone(), p.artwork.clone())
        };
        let kept = keep(dir, &details, artwork.as_deref().map(|a| &a[..]));
        self.update(|s| match &kept {
            Ok(name) => {
                if let Some(p) = s.release.as_mut() {
                    p.details.artwork = name.clone();
                }
            }
            Err(e) => s.say(format!("could not keep the release details with the song: {e:#}")),
        });
    }
}

/// release.json in a song's folder, with the artwork beside it when there is one; returns the artwork's name there
fn keep(dir: &Path, details: &Release, artwork: Option<&[u8]>) -> anyhow::Result<Option<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let name = match artwork {
        Some(a) => Some(publish::save_artwork(dir, a)?),
        None => details.artwork.clone(),
    };
    publish::save(dir, &Release { artwork: name.clone(), ..details.clone() })?;
    Ok(name)
}
