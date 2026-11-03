//! Samples prepared for songs: cut on their beat grids from recordings downloaded from online libraries (`claudius
//! samples`), each with the role it plays (a drum break, a phrase to build a song on, a texture under it) and listed
//! in `<samples>/prepared/index.json`, which the composer draws from. Each cut keeps its recording's credit beside it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// the part a sample plays in a song
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// a drum break, played as a loop, with its own hits cut out for the drum lanes
    Break,
    /// a phrase of a recording, which the song is built on: its key, its tempo, its chords
    Phrase,
    /// noise of a medium (vinyl, tape) or a place, under the whole song
    Texture,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Break, Role::Phrase, Role::Texture];

    pub fn name(self) -> &'static str {
        match self {
            Role::Break => "break",
            Role::Phrase => "phrase",
            Role::Texture => "texture",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prepared {
    pub path: String,
    pub role: Role,
    /// the genre it was gathered for
    pub genre: String,
    pub bpm: f64,
    pub beats: f64,
    #[serde(default)]
    pub key: Option<u8>,
    #[serde(default)]
    pub minor: bool,
    /// a break's own hits, cut out of it
    #[serde(default)]
    pub kick: Option<String>,
    #[serde(default)]
    pub snare: Option<String>,
    #[serde(default)]
    pub hat: Option<String>,
    /// a phrase's chords, a Roman numeral for each of its bars, in its key
    #[serde(default)]
    pub chords: Vec<String>,
    /// how strongly the ears heard what the genre listens for in it, when they listened
    #[serde(default)]
    pub z: Option<f64>,
    /// the online sound it was cut from (`freesound:629052`)
    pub source: String,
}

/// where prepared samples are written: `<samples>/prepared`
pub fn dir() -> PathBuf {
    claudius_packs::online::samples_dir().join("prepared")
}

fn read<T: serde::de::DeserializeOwned + Default>(p: &Path) -> T {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write<T: Serialize>(p: &Path, v: &T) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("cannot create {}", d.display()))?;
    }
    std::fs::write(p, serde_json::to_string_pretty(v)?).with_context(|| format!("writing {}", p.display()))
}

/// The prepared samples listed in a folder: its `index.json`, and the recordings found wanting in its
/// `rejected.json`.
pub struct Index {
    root: PathBuf,
}

impl Index {
    /// the samples folder's
    pub fn open() -> Index {
        Index::at(&dir())
    }

    pub fn at(root: &Path) -> Index {
        Index { root: root.to_path_buf() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// every prepared sample whose file is still there
    pub fn all(&self) -> Vec<Prepared> {
        let list: Vec<Prepared> = read(&self.root.join("index.json"));
        list.into_iter().filter(|p| Path::new(&p.path).is_file()).collect()
    }

    /// the prepared samples of `role` for `genre`
    pub fn of(&self, role: Role, genre: &str) -> Vec<Prepared> {
        self.all().into_iter().filter(|p| p.role == role && p.genre == genre).collect()
    }

    /// list a prepared sample, in place of one cut to the same file
    pub fn add(&self, p: Prepared) -> Result<()> {
        let mut list: Vec<Prepared> = read(&self.root.join("index.json"));
        list.retain(|q| q.path != p.path);
        list.push(p);
        write(&self.root.join("index.json"), &list)
    }

    /// whether `source` was downloaded for `role` and found wanting, so that a later gathering does not fetch it again
    pub fn rejected(&self, role: Role, source: &str) -> bool {
        let list: Vec<(Role, String)> = read(&self.root.join("rejected.json"));
        list.iter().any(|(r, s)| *r == role && s == source)
    }

    pub fn reject(&self, role: Role, source: &str) -> Result<()> {
        let mut list: Vec<(Role, String)> = read(&self.root.join("rejected.json"));
        if !list.iter().any(|(r, s)| *r == role && s == source) {
            list.push((role, source.to_string()));
        }
        write(&self.root.join("rejected.json"), &list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prepared_sample_is_listed_once_and_only_while_its_file_is_there() {
        let tmp = std::env::temp_dir().join(format!("claudius-prepared-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let ix = Index::at(&tmp);
        let file = tmp.join("break.wav");
        std::fs::write(&file, b"x").unwrap();
        let p = Prepared {
            path: file.to_string_lossy().into_owned(),
            role: Role::Break,
            genre: "trip_hop".into(),
            bpm: 92.0,
            beats: 8.0,
            key: None,
            minor: false,
            kick: None,
            snare: None,
            hat: None,
            chords: Vec::new(),
            z: Some(2.0),
            source: "freesound:1".into(),
        };
        ix.add(p.clone()).unwrap();
        ix.add(Prepared { bpm: 93.0, ..p.clone() }).unwrap();
        assert_eq!(ix.of(Role::Break, "trip_hop").len(), 1);
        assert_eq!(ix.of(Role::Break, "trip_hop")[0].bpm, 93.0);
        assert!(ix.of(Role::Phrase, "trip_hop").is_empty());
        std::fs::remove_file(&file).unwrap();
        assert!(ix.all().is_empty());
        assert!(!ix.rejected(Role::Phrase, "freesound:2"));
        ix.reject(Role::Phrase, "freesound:2").unwrap();
        assert!(ix.rejected(Role::Phrase, "freesound:2") && !ix.rejected(Role::Break, "freesound:2"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
