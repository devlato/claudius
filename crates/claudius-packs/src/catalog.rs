//! The curated pack registry (`registry.json`, embedded): what each pack contains, its licence and attribution as
//! recorded from the source, and how to fetch and map it.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::model::Unison;

#[derive(Clone, Debug, Deserialize)]
pub struct Registry {
    pub version: u32,
    pub packs: Vec<PackSpec>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PackSpec {
    pub name: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub licence: String,
    pub licence_url: String,
    /// attribution text exactly as it should appear in credits
    pub attribution: String,
    #[serde(default)]
    pub attribution_required: bool,
    /// what the licence text says, where it was read, anything unusual
    #[serde(default)]
    pub licence_note: String,
    pub source: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub opt_in: bool,
    #[serde(default)]
    pub warning: Option<String>,
    /// approximate download in MB
    pub size_mb: f32,
    pub version: String,
    /// bank names (`909`, `RolandTR909` ...)
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub fetch: Option<FetchSpec>,
    /// convert WAV downloads to FLAC (lossless) to save disk
    #[serde(default)]
    pub flac: bool,
    /// install lazily (index only; samples on first use) unless asked otherwise
    #[serde(default)]
    pub lazy: bool,
    pub instruments: Vec<InstSpec>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FetchSpec {
    /// `owner/repo` on GitHub
    #[serde(default)]
    pub github: Option<String>,
    /// commit, tag or branch
    #[serde(default)]
    pub rev: Option<String>,
    /// an archive URL (zip / 7z / tar.*)
    #[serde(default)]
    pub archive: Option<String>,
    /// path prefix inside the archive to strip
    #[serde(default)]
    pub strip: Option<String>,
    /// the archive's size in bytes and SHA-256, which the download must match: unlike a GitHub source at a commit,
    /// an archive on a web host can change under the same URL
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct InstSpec {
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    /// "pitched" (default) or "drum"
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fetch: Option<FetchSpec>,

    // --- SFZ source
    #[serde(default)]
    pub sfz: Option<String>,
    /// the folder the SFZ's sample and include paths start from, when it is not the SFZ's own: a mapping file that
    /// a program in another folder includes (the erhu's maps, used without the program's unison voices)
    #[serde(default)]
    pub base: Option<String>,
    /// keep this many velocity layers (evenly spread) and widen them to cover the full range
    #[serde(default)]
    pub keep_layers: Option<usize>,
    /// only regions whose key range lies inside [lo, hi]
    #[serde(default)]
    pub keys: Option<[u8; 2]>,
    /// split a kit SFZ: drum name -> keys (each key is one variant)
    #[serde(default)]
    pub kit: Option<BTreeMap<String, Vec<u8>>>,
    /// one variant per distinct key range of the SFZ (unpitched percussion with several hits)
    #[serde(default)]
    pub kit_auto: bool,
    /// regex on sample paths to leave out (SFZ regions and file-name sources)
    #[serde(default)]
    pub exclude: Option<String>,
    /// shift roots and key ranges by octaves (mappings at written pitch, e.g. bass guitars one octave up)
    #[serde(default)]
    pub octave: Option<i32>,

    // --- file-name source
    /// glob on paths of the source (`*` and `?`; `**` crosses directories)
    #[serde(default)]
    pub files: Option<String>,
    /// every matched file is its own variant
    #[serde(default)]
    pub each: bool,
    /// file name to make variant 0
    #[serde(default)]
    pub first: Option<String>,
    /// regex with one group: the velocity layer number in the file name
    #[serde(default)]
    pub vel: Option<String>,
    /// explicit variants, each a file-name source
    #[serde(default)]
    pub variants: Option<Vec<InstSpec>>,

    // --- Strudel sample map source
    #[serde(default)]
    pub map: Option<String>,
    /// where the map's files are, at a commit, in place of its `_base` when that names a branch of another
    /// repository (a branch of the map's own repository is pinned to the map's commit by itself)
    #[serde(default)]
    pub map_base: Option<String>,
    #[serde(default)]
    pub map_keys: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_keys: Vec<String>,

    // --- composite of other instruments of the pack, split by key
    #[serde(default)]
    pub compose: Option<Vec<ComposePart>>,

    // --- playback
    #[serde(default)]
    pub one_shot: Option<bool>,
    #[serde(default)]
    pub release: Option<f32>,
    /// keep per-region release times from the SFZ instead of `release`
    #[serde(default)]
    pub keep_zone_release: bool,
    #[serde(default)]
    pub attack: Option<f32>,
    #[serde(default)]
    pub veltrack: Option<f32>,
    #[serde(default)]
    pub gain_db: Option<f32>,
    #[serde(default)]
    pub mono: Option<bool>,
    #[serde(default)]
    pub retrigger: Option<bool>,
    #[serde(default)]
    pub unison: Option<Unison>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ComposePart {
    pub from: String,
    #[serde(default)]
    pub lokey: Option<u8>,
    #[serde(default)]
    pub hikey: Option<u8>,
}

pub static REGISTRY: LazyLock<Registry> =
    LazyLock::new(|| serde_json::from_str(include_str!("registry.json")).expect("embedded registry.json is valid"));

pub fn spec(name: &str) -> Option<&'static PackSpec> {
    let n = name.to_ascii_lowercase();
    REGISTRY.packs.iter().find(|p| p.name == n || p.aliases.iter().any(|a| a.to_ascii_lowercase() == n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_consistent() {
        let r = &*REGISTRY;
        assert!(r.version >= 1);
        let mut names = std::collections::HashSet::new();
        let mut default_mb = 0.0;
        for p in &r.packs {
            assert!(names.insert(p.name.clone()), "duplicate pack {}", p.name);
            assert!(!p.licence.is_empty() && !p.attribution.is_empty() && !p.source.is_empty(), "{}", p.name);
            assert!(!(p.default && p.opt_in), "{} cannot be default and opt-in", p.name);
            if p.opt_in {
                assert!(p.warning.is_some(), "opt-in pack {} needs a warning", p.name);
            }
            if p.default {
                default_mb += p.size_mb;
            }
            let mut inst = std::collections::HashSet::new();
            for i in &p.instruments {
                let has_fetch = i.fetch.is_some() || p.fetch.is_some() || i.map.is_some() || i.compose.is_some();
                assert!(has_fetch, "{}:{} has no source", p.name, i.name);
                let sources =
                    [i.sfz.is_some(), i.files.is_some(), i.variants.is_some(), i.map.is_some(), i.compose.is_some()];
                assert_eq!(sources.iter().filter(|s| **s).count(), 1, "{}:{} needs exactly one source", p.name, i.name);
                if i.kit.is_none() && i.map.is_none() {
                    assert!(inst.insert(i.name.clone()), "duplicate instrument {}:{}", p.name, i.name);
                }
                if let Some(c) = &i.compose {
                    for part in c {
                        assert!(
                            p.instruments.iter().any(|x| x.name == part.from),
                            "{}:{} composes unknown {}",
                            p.name,
                            i.name,
                            part.from
                        );
                    }
                }
            }
        }
        assert!(default_mb < 3300.0, "default packs are {default_mb} MB");
    }

    #[test]
    fn every_download_is_pinned() {
        let commit = |rev: &str| rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit());
        for p in &REGISTRY.packs {
            let fetches = p.fetch.iter().chain(p.instruments.iter().filter_map(|i| i.fetch.as_ref()));
            for f in fetches {
                if f.github.is_some() {
                    assert!(f.rev.as_deref().is_some_and(commit), "{}: a GitHub source needs a commit", p.name);
                }
                if let Some(url) = &f.archive {
                    assert!(f.size.is_some_and(|s| s > 0), "{}: {url} needs its size", p.name);
                    assert!(f.sha256.as_deref().is_some_and(|h| h.len() == 64), "{}: {url} needs its SHA-256", p.name);
                }
            }
            for i in p.instruments.iter().filter(|i| i.map.is_some()) {
                assert!(
                    crate::strudel::pinned(i.map.as_deref().unwrap()),
                    "{}:{}: the map needs a commit",
                    p.name,
                    i.name
                );
                if let Some(base) = &i.map_base {
                    assert!(crate::strudel::pinned(base), "{}:{}: map_base needs a commit", p.name, i.name);
                }
            }
        }
    }
}
