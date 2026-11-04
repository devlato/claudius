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
    /// the words a person searched for it with (`sampling::words`); none: the genre's own searches found it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    /// a phrase's second passage, from another section of its recording, which other sections of a song play
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second: Option<Passage>,
}

/// Another passage of a phrase's recording, as long as the phrase and at its tempo and key: its file, its chords (a
/// Roman numeral for each of its bars, in the phrase's key), and whether it is the louder of the two.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Passage {
    pub path: String,
    pub chords: Vec<String>,
    pub louder: bool,
}

impl Prepared {
    /// whether a song of `genre` draws on it: one asked for `asked`, a sample gathered for those words (for any
    /// genre); else one the genre's own searches found
    pub fn fits(&self, genre: &str, asked: Option<&str>) -> bool {
        match asked {
            Some(words) => self.asked.as_deref() == Some(words),
            None => self.asked.is_none() && self.genre == genre,
        }
    }
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

    /// every prepared sample whose file is still there (with its second passage while that is too)
    pub fn all(&self) -> Vec<Prepared> {
        let list: Vec<Prepared> = read(&self.root.join("index.json"));
        list.into_iter()
            .filter(|p| Path::new(&p.path).is_file())
            .map(|mut p| {
                p.second = p.second.filter(|s| Path::new(&s.path).is_file());
                p
            })
            .collect()
    }

    /// the prepared samples of `role` a song of `genre` draws on, asked for `asked` (`Prepared::fits`)
    pub fn of(&self, role: Role, genre: &str, asked: Option<&str>) -> Vec<Prepared> {
        self.all().into_iter().filter(|p| p.role == role && p.fits(genre, asked)).collect()
    }

    /// list a prepared sample, in place of one cut to the same file
    pub fn add(&self, p: Prepared) -> Result<()> {
        let mut list: Vec<Prepared> = read(&self.root.join("index.json"));
        list.retain(|q| q.path != p.path);
        list.push(p);
        write(&self.root.join("index.json"), &list)
    }

    /// the recordings found wanting: for `role`, from `source`, and for whose gathering (`sampling::whose`), or for
    /// every one when it could not be read or cut at all; an older list's `[role, source]` is the latter
    fn rejections(&self) -> Vec<(Role, String, Option<String>)> {
        let list: Vec<serde_json::Value> = read(&self.root.join("rejected.json"));
        list.into_iter()
            .filter_map(|v| {
                serde_json::from_value(v.clone())
                    .ok()
                    .or_else(|| serde_json::from_value::<(Role, String)>(v).ok().map(|(r, s)| (r, s, None)))
            })
            .collect()
    }

    /// whether `source` was downloaded for `role` and found wanting for the gathering `whose` (or for every one), so
    /// that a later gathering for it does not fetch it again
    pub fn rejected(&self, role: Role, source: &str, whose: &str) -> bool {
        self.rejections().iter().any(|(r, s, w)| *r == role && s == source && w.as_deref().is_none_or(|w| w == whose))
    }

    /// `source` found wanting for `role`: for the gathering `whose` (the ears heard other than what it asked for), or
    /// for every one (none: it could not be read or cut)
    pub fn reject(&self, role: Role, source: &str, whose: Option<&str>) -> Result<()> {
        let mut list = self.rejections();
        let entry = (role, source.to_string(), whose.map(str::to_string));
        if !list.contains(&entry) {
            list.push(entry);
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
            asked: None,
            second: None,
        };
        ix.add(p.clone()).unwrap();
        ix.add(Prepared { bpm: 93.0, ..p.clone() }).unwrap();
        assert_eq!(ix.of(Role::Break, "trip_hop", None).len(), 1);
        assert_eq!(ix.of(Role::Break, "trip_hop", None)[0].bpm, 93.0);
        // a second passage is listed while its file is there
        let other = tmp.join("phrase-2.wav");
        let second = Passage { path: other.to_string_lossy().into_owned(), chords: vec!["iv".into()], louder: true };
        ix.add(Prepared { second: Some(second.clone()), ..p.clone() }).unwrap();
        assert_eq!(ix.all()[0].second, None);
        std::fs::write(&other, b"x").unwrap();
        assert_eq!(ix.all()[0].second, Some(second));
        assert!(ix.of(Role::Phrase, "trip_hop", None).is_empty());
        // one asked for by its words is a song's that asks for them, of any genre, and not the genre's own
        ix.add(Prepared { role: Role::Phrase, asked: Some("70s soul".into()), ..p.clone() }).unwrap();
        assert_eq!(ix.of(Role::Phrase, "house", Some("70s soul")).len(), 1);
        assert!(
            ix.of(Role::Phrase, "trip_hop", None).is_empty() && ix.of(Role::Phrase, "house", Some("soul")).is_empty()
        );
        std::fs::remove_file(&file).unwrap();
        assert!(ix.all().is_empty());
        assert!(!ix.rejected(Role::Phrase, "freesound:2", "house"));
        // turned down for house's phrase: not for another genre's, nor for words of one's own
        ix.reject(Role::Phrase, "freesound:2", Some("house")).unwrap();
        assert!(ix.rejected(Role::Phrase, "freesound:2", "house"));
        assert!(!ix.rejected(Role::Phrase, "freesound:2", "lofi") && !ix.rejected(Role::Break, "freesound:2", "house"));
        // one that could not be cut at all is turned down for every gathering
        ix.reject(Role::Break, "freesound:3", None).unwrap();
        assert!(ix.rejected(Role::Break, "freesound:3", "house") && ix.rejected(Role::Break, "freesound:3", "lofi"));
        // as an older list kept them: for every gathering
        std::fs::write(tmp.join("rejected.json"), r#"[["texture", "freesound:4"]]"#).unwrap();
        assert!(ix.rejected(Role::Texture, "freesound:4", "trip_hop"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
