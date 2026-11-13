//! Recordings from the free online libraries (`claudius_packs::online`), for a person or an assistant to hear and use:
//! a search lists them (`/api/library/search`, `search_recordings`), and `library:<id>`, where a command takes a
//! recording's path, is one a search found, downloaded once into the samples folder with what it credits.

use std::path::PathBuf;
use std::sync::Arc;

use claudius::config;
use claudius_api::{Command, LibraryHit};
use claudius_packs::online::{self, Found, Policy, Query};

use super::App;

/// what a command names a recording a search found by
pub const SCHEME: &str = "library:";

/// a recording a search found, as the studio lists it: its audio fetched from this claudius
pub fn hit(f: &Found) -> LibraryHit {
    LibraryHit {
        id: f.id.clone(),
        title: f.title.clone(),
        creator: f.creator.clone(),
        licence: f.licence_name.clone(),
        seconds: f.seconds,
        page: f.page.clone(),
        audio: format!("/api/library/audio?id={}", escape(&f.id)),
        credit: f.credit(),
        found: None,
    }
}

/// `s` as a URL's query value: letters, digits and `-_.~` as they are, every other byte as `%XX`
fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl App {
    /// the libraries' recordings for `q`, at most `limit`, under the licences claudius takes by default; an error
    /// when none answered
    pub fn search_library(&self, q: &str, limit: usize) -> Result<Vec<LibraryHit>, String> {
        self.library_allowed()?;
        let q = q.trim();
        if q.is_empty() {
            return Err("say what to search for: \"a soul singer\", \"a jazz drum break\", \"rain\"".into());
        }
        let query =
            Query { text: q.to_string(), sources: Vec::new(), policy: Policy::default(), limit: limit.clamp(1, 50) };
        let got = online::search(&query).map_err(|e| format!("the search failed: {e:#}"))?;
        if got.found.is_empty() && !got.failed.is_empty() {
            let names: Vec<&str> = got.failed.iter().map(|(s, _)| s.name()).collect();
            return Err(format!("no library answered ({}): try again in a minute", names.join(", ")));
        }
        Ok(got.found.iter().map(hit).collect())
    }

    /// the file of recording `id`, which a search found: downloaded once into the samples folder, with a `.json`
    /// beside it naming what it credits
    pub fn library_file(&self, id: &str) -> Result<PathBuf, String> {
        self.library_allowed()?;
        let Some(f) = online::recall(id) else {
            return Err(format!("no recording '{id}' among the searches: search_recordings finds it first"));
        };
        online::fetch(&f, &Policy::default()).map_err(|e| format!("cannot download {id}: {e:#}"))
    }

    /// whether this claudius may keep what the libraries offer: one that may not write its samples folder says why
    fn library_allowed(&self) -> Result<(), String> {
        let dir = online::samples_dir();
        if config::write_allowed(&dir, self.roots.as_deref()) {
            Ok(())
        } else {
            Err(format!(
                "this claudius keeps no recordings from the libraries: {} is outside the folders it may write",
                dir.display()
            ))
        }
    }

    /// `c`, with a recording it names as `library:<id>` downloaded and named by its file instead
    pub(super) fn from_library(self: &Arc<Self>, c: Command) -> Result<Command, String> {
        let file = |path: String| -> Result<String, String> {
            match path.strip_prefix(SCHEME) {
                Some(id) => self.library_file(id).map(|p| p.display().to_string()),
                None => Ok(path),
            }
        };
        Ok(match c {
            Command::AddTake { path, from, name, role } => Command::AddTake { path: file(path)?, from, name, role },
            Command::SampleSong { path, count, bars, find, sections, how, every } => {
                Command::SampleSong { path: file(path)?, count, bars, find, sections, how, every }
            }
            Command::Analyze { path } => Command::Analyze { path: file(path)? },
            Command::SamplePieces { path, seed, count, bars, find } => {
                Command::SamplePieces { path: file(path)?, seed, count, bars, find }
            }
            Command::Remake { path, genre, seed, bpm, sampled, flip, rebuild } => {
                Command::Remake { path: file(path)?, genre, seed, bpm, sampled, flip, rebuild }
            }
            c => c,
        })
    }
}
