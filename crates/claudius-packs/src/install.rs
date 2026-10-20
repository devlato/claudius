//! Building a pack's index from its registry entry (SFZ files, file-name rules, Strudel maps, kits, key-split
//! composites) and installing it: downloads, archives, FLAC conversion, the index file.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;

use crate::catalog::{FetchSpec, InstSpec, PackSpec};
use crate::fetch;
use crate::model::{FileEntry, INDEX_FORMAT, InstrumentDef, Kind, LoopMode, PackIndex, Variant, Zone};
use crate::notes;
use crate::sfz::{self, SfzFile, SfzRegion};
use crate::strudel::{self, MapValue};

mod from_map;
mod from_names;
mod from_sfz;

use from_map::*;
use from_names::*;
use from_sfz::*;

#[derive(Clone, Debug, Default)]
pub struct InstallOptions {
    /// write the index only; download each sample the first time it is played
    pub lazy: bool,
    /// parallel downloads
    pub threads: usize,
    /// install even when the pack is opt-in (unclear licence)
    pub accept_opt_in: bool,
}

/// Where a pack's files come from.
enum Source {
    GitHub {
        repo: String,
        rev: String,
        listing: Option<Vec<(String, u64)>>,
    },
    /// extracted archive; `root` already has the strip prefix applied
    Archive {
        root: PathBuf,
    },
}

impl Source {
    fn read_text(&self, path: &str) -> Result<String> {
        match self {
            Source::GitHub { repo, rev, .. } => fetch::get_text(&fetch::github_raw(repo, rev, path)),
            Source::Archive { root } => {
                let p = root.join(path);
                std::fs::read(&p)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .with_context(|| format!("cannot read {}", p.display()))
            }
        }
    }

    fn list(&mut self) -> Result<Vec<(String, u64)>> {
        match self {
            Source::GitHub { repo, rev, listing } => {
                if listing.is_none() {
                    *listing = Some(fetch::github_tree(repo, rev)?);
                }
                Ok(listing.clone().unwrap_or_default())
            }
            Source::Archive { root } => Ok(fetch::walk(root)),
        }
    }

    fn size_of(&self, path: &str) -> u64 {
        match self {
            Source::GitHub { listing: Some(l), .. } => l.iter().find(|(p, _)| p == path).map_or(0, |(_, s)| *s),
            Source::Archive { root } => std::fs::metadata(root.join(path)).map(|m| m.len()).unwrap_or(0),
            _ => 0,
        }
    }
}

struct Ctx<'a> {
    pack: &'a PackSpec,
    files: Vec<FileEntry>,
    file_ids: HashMap<String, u32>,
    /// for archive members: file id -> extracted path to copy from
    copies: HashMap<u32, PathBuf>,
    built: Vec<InstrumentDef>,
    sources: HashMap<String, Source>,
    temp_dirs: Vec<PathBuf>,
    flac: bool,
}

fn sanitize(p: &str) -> String {
    let p = sfz::normalize_path(&p.replace('\\', "/"));
    p.chars()
        .map(|c| if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect()
}

impl Ctx<'_> {
    fn source(&mut self, f: &FetchSpec) -> Result<String> {
        let key = format!("{:?}|{:?}|{:?}", f.github, f.rev, f.archive);
        if self.sources.contains_key(&key) {
            return Ok(key);
        }
        let src = if let Some(repo) = &f.github {
            Source::GitHub { repo: repo.clone(), rev: f.rev.clone().unwrap_or_else(|| "HEAD".into()), listing: None }
        } else if let Some(url) = &f.archive {
            let cache = crate::cache_dir();
            let name = url.rsplit('/').next().unwrap_or("archive").to_string();
            let archive = cache.join("archives").join(sanitize(&name));
            fetch::download_file(url, &archive, f.size.unwrap_or(0), false)
                .with_context(|| format!("downloading {url}"))?;
            if let Some(want) = &f.sha256 {
                check_archive(&archive, want).with_context(|| format!("{url} is not the archive claudius knows"))?;
            }
            let dir = cache.join("extract").join(format!("{}-{}", self.pack.name, std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            fetch::extract_archive(&archive, &dir)?;
            self.temp_dirs.push(dir.clone());
            self.temp_dirs.push(archive.clone());
            let root = match &f.strip {
                Some(s) => dir.join(s.trim_end_matches('/')),
                None => dir,
            };
            Source::Archive { root }
        } else {
            bail!("pack {}: fetch needs github or archive", self.pack.name)
        };
        self.sources.insert(key.clone(), src);
        Ok(key)
    }

    /// register a file of a source; returns its index
    fn add_source_file(&mut self, src_key: &str, path: &str) -> u32 {
        let src = &self.sources[src_key];
        let flac = self.flac && path.to_ascii_lowercase().ends_with(".wav");
        let mut local = format!("files/{}", sanitize(path));
        if let Source::GitHub { repo, .. } = src {
            // several repositories can feed one pack (FreePats): keep them apart
            if self.pack.fetch.is_none() {
                local = format!("files/{}/{}", repo.replace('/', "_"), sanitize(path));
            }
        }
        if flac {
            local = format!("{}.flac", &local[..local.len() - 4]);
        }
        if let Some(&id) = self.file_ids.get(&local) {
            return id;
        }
        let (url, size, copy) = match src {
            Source::GitHub { repo, rev, .. } => (fetch::github_raw(repo, rev, path), src.size_of(path), None),
            Source::Archive { root } => (String::new(), 0, Some(root.join(path))),
        };
        let id = self.files.len() as u32;
        self.files.push(FileEntry { path: local.clone(), url, size, flac });
        self.file_ids.insert(local, id);
        if let Some(c) = copy {
            self.copies.insert(id, c);
        }
        id
    }

    fn add_url_file(&mut self, url: &str) -> u32 {
        let rest = url.split_once("://").map_or(url, |(_, r)| r);
        let rest = rest.split(['?', '#']).next().unwrap_or(rest);
        let local = format!("files/{}", sanitize(&strudel::decode_path(rest)));
        if let Some(&id) = self.file_ids.get(&local) {
            return id;
        }
        let id = self.files.len() as u32;
        self.files.push(FileEntry { path: local.clone(), url: url.to_string(), size: 0, flac: false });
        self.file_ids.insert(local, id);
        id
    }
}

fn category_release(cat: &str, kind: Kind) -> f32 {
    if kind == Kind::Drum {
        return 0.08;
    }
    match cat {
        "piano" => 0.35,
        "electric piano" => 0.3,
        "organ" => 0.1,
        "strings" => 0.5,
        "brass" => 0.3,
        "woodwinds" => 0.2,
        "mallets" | "harp" => 0.5,
        "bass" => 0.15,
        "guitar" => 0.3,
        "pad" => 1.0,
        "voice" => 0.4,
        "choir" => 0.6,
        _ => 0.3,
    }
}

fn base_def(spec: &InstSpec, name: &str, kind: Kind, variants: Vec<Variant>) -> InstrumentDef {
    let category =
        spec.category.clone().unwrap_or_else(|| if kind == Kind::Drum { "drums".into() } else { "misc".into() });
    InstrumentDef {
        name: name.to_string(),
        release: spec.release.unwrap_or_else(|| category_release(&category, kind)),
        category,
        kind,
        description: spec.description.clone().unwrap_or_default(),
        one_shot: spec.one_shot.unwrap_or(kind == Kind::Drum),
        attack: spec.attack.unwrap_or(0.0),
        gain_db: spec.gain_db.unwrap_or(0.0),
        veltrack: spec.veltrack.unwrap_or(if kind == Kind::Drum { 0.5 } else { 0.7 }),
        vel_curve: Vec::new(),
        mono: spec.mono.unwrap_or(false),
        retrigger: spec.retrigger.unwrap_or(false),
        unison: spec.unison.clone(),
        aliases: spec.aliases.clone(),
        variants,
    }
}

fn spec_kind(spec: &InstSpec) -> Kind {
    match spec.kind.as_deref() {
        Some("drum") => Kind::Drum,
        _ => Kind::Pitched,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// composites

fn build_compose(ctx: &Ctx, spec: &InstSpec) -> Result<InstrumentDef> {
    let parts = spec.compose.as_ref().expect("compose");
    let mut zones: Vec<Zone> = Vec::new();
    let mut kind = Kind::Pitched;
    for part in parts {
        let src = ctx
            .built
            .iter()
            .find(|d| d.name == part.from)
            .ok_or_else(|| anyhow!("{}: unknown instrument {}", spec.name, part.from))?;
        kind = src.kind;
        let lo = part.lokey.unwrap_or(0);
        let hi = part.hikey.unwrap_or(127);
        for z in src.variants.first().map(|v| v.zones.as_slice()).unwrap_or(&[]) {
            let mut z = z.clone();
            z.lokey = z.lokey.max(lo);
            z.hikey = z.hikey.min(hi);
            if z.lokey <= z.hikey {
                zones.push(z);
            }
        }
    }
    if zones.is_empty() {
        bail!("{}: composite has no zones", spec.name);
    }
    let mut d = base_def(spec, &spec.name, kind, vec![Variant { name: String::new(), zones }]);
    if spec.veltrack.is_none()
        && let Some(src) = ctx.built.iter().find(|d| d.name == parts[0].from)
    {
        d.veltrack = src.veltrack;
    }
    Ok(d)
}

// ---------------------------------------------------------------------------------------------------------------
// install / remove

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn index_from_spec(
    pack: &PackSpec,
    lazy: bool,
    progress: &dyn Fn(f32, &str),
) -> Result<(PackIndex, HashMap<u32, PathBuf>, Vec<PathBuf>)> {
    let mut ctx = Ctx {
        pack,
        files: Vec::new(),
        file_ids: HashMap::new(),
        copies: HashMap::new(),
        built: Vec::new(),
        sources: HashMap::new(),
        temp_dirs: Vec::new(),
        flac: pack.flac,
    };
    let result = (|| -> Result<()> {
        let n = pack.instruments.len().max(1);
        for (i, spec) in pack.instruments.iter().enumerate() {
            progress(0.0, &format!("{}: mapping {} ({}/{n})", pack.name, spec.name, i + 1));
            let defs = if spec.compose.is_some() {
                vec![build_compose(&ctx, spec)?]
            } else if spec.map.is_some() {
                build_map(&mut ctx, spec)?
            } else {
                let f = spec
                    .fetch
                    .as_ref()
                    .or(pack.fetch.as_ref())
                    .ok_or_else(|| anyhow!("{}:{} has no source", pack.name, spec.name))?;
                let key = ctx.source(f)?;
                if spec.sfz.is_some() {
                    build_sfz(&mut ctx, &key, spec)?
                } else {
                    vec![build_files(&mut ctx, &key, spec)?]
                }
            };
            for d in defs {
                if d.variants.iter().all(|v| v.zones.is_empty()) {
                    continue;
                }
                ctx.built.retain(|x| x.name != d.name);
                ctx.built.push(d);
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        for d in &ctx.temp_dirs {
            let _ = std::fs::remove_dir_all(d);
            let _ = std::fs::remove_file(d);
        }
        return Err(e);
    }
    // drop files no zone uses any more (velocity layers cut, kit keys not mapped ...) and renumber the rest
    let mut used = vec![false; ctx.files.len()];
    for d in &ctx.built {
        for v in &d.variants {
            for z in &v.zones {
                used[z.file as usize] = true;
            }
        }
    }
    let mut remap = vec![u32::MAX; ctx.files.len()];
    let mut files = Vec::new();
    for (i, f) in ctx.files.into_iter().enumerate() {
        if used[i] {
            remap[i] = files.len() as u32;
            files.push(f);
        }
    }
    for d in &mut ctx.built {
        for v in &mut d.variants {
            for z in &mut v.zones {
                z.file = remap[z.file as usize];
            }
        }
    }
    let copies: HashMap<u32, PathBuf> =
        ctx.copies.into_iter().filter(|(i, _)| used[*i as usize]).map(|(i, p)| (remap[i as usize], p)).collect();
    let index = PackIndex {
        format: INDEX_FORMAT,
        name: pack.name.clone(),
        title: pack.title.clone(),
        description: pack.description.clone(),
        categories: pack.categories.clone(),
        licence: pack.licence.clone(),
        licence_url: pack.licence_url.clone(),
        attribution: pack.attribution.clone(),
        attribution_required: pack.attribution_required,
        licence_note: pack.licence_note.clone(),
        source: pack.source.clone(),
        version: pack.version.clone(),
        default: pack.default,
        opt_in: pack.opt_in,
        warning: pack.warning.clone(),
        user: false,
        lazy: lazy && copies.is_empty(),
        installed_at: now(),
        aliases: pack.aliases.clone(),
        files,
        instruments: ctx.built,
    };
    Ok((index, copies, ctx.temp_dirs))
}

/// delete files under the pack directory that the index does not reference (older versions, cut layers)
fn prune(dir: &Path, index: &PackIndex) -> (usize, u64) {
    let keep: std::collections::HashSet<&str> = index.files.iter().map(|f| f.path.as_str()).collect();
    let mut removed = (0usize, 0u64);
    for (rel, size) in fetch::walk(&dir.join("files")) {
        let rel = format!("files/{rel}");
        // keep partial downloads of files still wanted, so an interrupted install resumes
        let wanted = keep.contains(rel.as_str()) || rel.strip_suffix(".part").is_some_and(|r| keep.contains(r));
        if !wanted && std::fs::remove_file(dir.join(&rel)).is_ok() {
            removed.0 += 1;
            removed.1 += size;
        }
    }
    removed
}

pub(crate) fn write_index(dir: &Path, index: &PackIndex) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("index.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(index)?)?;
    std::fs::rename(&tmp, dir.join("index.json"))?;
    Ok(())
}

/// Install (or complete / update) a registry pack.
pub fn install_spec(pack: &PackSpec, opts: &InstallOptions, progress: &dyn Fn(f32, &str)) -> Result<PackIndex> {
    if pack.opt_in && !opts.accept_opt_in {
        bail!(
            "{} is opt-in: {} Install it explicitly (InstallOptions::accept_opt_in, or `claudius-packs install {} --accept-licence`).",
            pack.name,
            pack.warning.as_deref().unwrap_or("its licence is unclear."),
            pack.name
        );
    }
    let dir = crate::packs_dir().join(&pack.name);
    let lazy = opts.lazy || pack.lazy;
    let (index, copies, temp) = index_from_spec(pack, lazy, progress)?;
    let mut downloaded = 0u64;
    let res = (|| -> Result<()> {
        std::fs::create_dir_all(&dir)?;
        let (n, bytes) = prune(&dir, &index);
        if n > 0 {
            progress(0.0, &format!("{}: removed {n} stale files ({:.1} MB)", pack.name, bytes as f64 / 1e6));
        }
        let damaged = index
            .files
            .iter()
            .filter(|f| {
                f.flac && fetch::bloated_flac(&dir.join(&f.path)) && std::fs::remove_file(dir.join(&f.path)).is_ok()
            })
            .count();
        if damaged > 0 {
            progress(0.0, &format!("{}: converting {damaged} damaged FLAC files again", pack.name));
        }
        // archive members: copy (or convert) out of the extraction directory, in parallel
        if !copies.is_empty() {
            use rayon::prelude::*;
            let list: Vec<(&u32, &PathBuf)> = copies.iter().collect();
            let n = list.len();
            let mut done = 0;
            for chunk in list.chunks(64) {
                chunk.par_iter().try_for_each(|(id, src)| -> Result<()> {
                    let fe = &index.files[**id as usize];
                    let dest = dir.join(&fe.path);
                    if dest.is_file() {
                        return Ok(());
                    }
                    std::fs::create_dir_all(dest.parent().expect("parent"))?;
                    let tmp = dest.with_extension("tmp");
                    if fe.flac {
                        fetch::transcode_flac(src, &tmp)?;
                    } else {
                        std::fs::copy(src, &tmp).with_context(|| format!("copying {}", src.display()))?;
                    }
                    std::fs::rename(&tmp, &dest)?;
                    Ok(())
                })?;
                done += chunk.len();
                progress(done as f32 / n as f32, &format!("{}: unpacking {done}/{n}", pack.name));
            }
        }
        if !index.lazy {
            let tasks: Vec<fetch::Task> = index
                .files
                .iter()
                .filter(|f| !f.url.is_empty())
                .map(|f| fetch::Task { url: f.url.clone(), dest: dir.join(&f.path), size: f.size, flac: f.flac })
                .collect();
            if !tasks.is_empty() {
                let threads = if opts.threads > 0 { opts.threads } else { 8 };
                downloaded += fetch::download_all(&tasks, threads, &pack.name, progress)?.downloaded_bytes;
            }
        }
        write_index(&dir, &index)?;
        Ok(())
    })();
    for d in &temp {
        if let Ok(m) = std::fs::metadata(d)
            && m.is_file()
        {
            downloaded += m.len();
        }
        let _ = std::fs::remove_dir_all(d);
        let _ = std::fs::remove_file(d);
    }
    // leave no empty scratch directories behind
    let cache = crate::cache_dir();
    for sub in ["archives", "extract"] {
        let _ = std::fs::remove_dir(cache.join(sub));
    }
    let _ = std::fs::remove_dir(&cache);
    res?;
    crate::decode::forget_dir(&dir);
    progress(
        1.0,
        &format!(
            "{}: installed ({} instruments, {} files, {:.1} MB downloaded)",
            pack.name,
            index.instruments.len(),
            index.files.len(),
            downloaded as f64 / 1e6
        ),
    );
    Ok(index)
}

/// The folder of the pack called `name`. A name is one plain folder name, so nothing built from it can reach outside
/// the packs folder: not `..`, not `a/b`, not an absolute path (which `Path::join` would take as the whole path).
fn pack_dir(name: &str) -> Result<PathBuf> {
    let mut parts = Path::new(name).components();
    match (parts.next(), parts.next()) {
        (Some(std::path::Component::Normal(_)), None) if !name.contains(['/', '\\']) => {
            Ok(crate::packs_dir().join(name))
        }
        _ => bail!("'{name}' is not a pack name"),
    }
}

/// an error, and the file deleted, when it is not the one whose SHA-256 the registry pins (its host may have changed it,
/// or it was altered on the way); a copy in the cache is checked the same way
fn check_archive(path: &Path, sha256: &str) -> Result<()> {
    let got = fetch::sha256_file(path)?;
    if !got.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(path);
        bail!("its SHA-256 is {got}, and the registry pins {sha256}");
    }
    Ok(())
}

/// Remove an installed pack's folder entirely.
pub fn remove_dir(name: &str) -> Result<()> {
    let dir = pack_dir(name)?;
    // an installed pack has its index; a folder without one is an interrupted install of a pack the registry knows
    let known = dir.join("index.json").is_file() || crate::catalog::spec(name).is_some();
    if !known || !dir.is_dir() {
        bail!("pack {name} is not installed");
    }
    crate::decode::forget_dir(&dir);
    std::fs::remove_dir_all(&dir).with_context(|| format!("cannot remove {}", dir.display()))?;
    Ok(())
}

/// A user pack from any Strudel sample map (URL or `github:user/repo[/branch[/path]]`), installed lazily.
pub fn add_map(spec_str: &str) -> Result<PackIndex> {
    let url = strudel::map_url(spec_str)?;
    let json = fetch::get_text(&url).with_context(|| format!("fetching {url}"))?;
    let map = strudel::parse_map(&json, &url)?;
    let gh = strudel::github_repo(&url);
    let name = match &gh {
        Some((o, r)) => format!("{}-{}", o, r),
        None => {
            let s = url.trim_end_matches(".json").rsplit('/').take(2).collect::<Vec<_>>();
            s.into_iter().rev().collect::<Vec<_>>().join("-")
        }
    }
    .to_ascii_lowercase()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
    .collect::<String>();
    if crate::catalog::spec(&name).is_some() {
        bail!("{name} is a curated pack name; install it with install()");
    }
    let licence = map
        .licence
        .clone()
        .or_else(|| gh.as_ref().and_then(|(o, r)| fetch::github_licence(o, r)))
        .unwrap_or_else(|| "unknown".to_string());
    let attribution = map.attribution.clone().unwrap_or_else(|| match &gh {
        Some((o, r)) => format!("{r} by {o} (github.com/{o}/{r})"),
        None => url.clone(),
    });
    let mut files: Vec<FileEntry> = Vec::new();
    let mut ids: HashMap<String, u32> = HashMap::new();
    let spec = InstSpec { name: name.clone(), ..Default::default() };
    let mut add = |u: &str| {
        let rest = u.split_once("://").map_or(u, |(_, r)| r);
        let local = format!("files/{}", sanitize(&strudel::decode_path(rest)));
        *ids.entry(local.clone()).or_insert_with(|| {
            files.push(FileEntry { path: local, url: u.to_string(), size: 0, flac: false });
            (files.len() - 1) as u32
        })
    };
    let instruments = instruments_from_map(&map, &spec, &mut add);
    if instruments.is_empty() {
        bail!("{url}: no sounds in the sample map");
    }
    let lic_lower = licence.to_ascii_lowercase();
    let index = PackIndex {
        format: INDEX_FORMAT,
        name: name.clone(),
        title: gh.as_ref().map_or(name.clone(), |(o, r)| format!("{o}/{r}")),
        description: format!("Strudel sample map {spec_str} ({} sounds)", instruments.len()),
        categories: vec!["samples".into()],
        licence: licence.clone(),
        licence_url: String::new(),
        attribution,
        attribution_required: lic_lower.contains("by") && !lic_lower.contains("cc0"),
        licence_note: if licence == "unknown" {
            "No licence declared by the map or its repository.".into()
        } else {
            String::new()
        },
        source: spec_str.to_string(),
        version: format!("added {}", now()),
        default: false,
        opt_in: false,
        warning: (licence == "unknown")
            .then(|| "Licence unknown: check the source before publishing music with these samples.".to_string()),
        user: true,
        lazy: true,
        installed_at: now(),
        aliases: gh.map(|(_, r)| vec![r.to_ascii_lowercase()]).unwrap_or_default(),
        files,
        instruments,
    };
    write_index(&crate::packs_dir().join(&name), &index)?;
    Ok(index)
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_archive_must_be_the_one_the_registry_pins() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("pack.zip");
        std::fs::write(&p, b"abc").unwrap();
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        super::check_archive(&p, abc).unwrap();
        super::check_archive(&p, &abc.to_uppercase()).unwrap();
        let e = super::check_archive(&p, &"0".repeat(64)).unwrap_err().to_string();
        assert!(e.contains(abc), "{e}");
        assert!(!p.exists(), "a file that does not match is deleted");
    }

    #[test]
    fn pack_names_stay_inside_the_packs_folder() {
        for bad in ["..", ".", "", "a/b", "../x", "/tmp/victim", "a\\b"] {
            assert!(super::pack_dir(bad).is_err(), "{bad:?} was accepted");
        }
        assert!(super::pack_dir("tr808").unwrap().ends_with("tr808"));
        assert!(super::remove_dir("../packs").is_err());
    }

    use super::*;

    #[test]
    fn globs() {
        let re = glob_regex("bd8/*.WAV").unwrap();
        assert!(re.is_match("bd8/BD0010.WAV"));
        assert!(!re.is_match("bd8/x/BD0010.WAV"));
        assert!(!re.is_match("sd8/SD0010.WAV"));
        let re = glob_regex("BassDrum/BassDrum909-accent*-tune100-attack100-decay100.wav").unwrap();
        assert!(re.is_match("BassDrum/BassDrum909-accent070-tune100-attack100-decay100.wav"));
        assert!(!re.is_match("BassDrum/BassDrum909-tune100-attack100-decay100.wav"));
        assert!(glob_regex("**/*.wav").unwrap().is_match("a/b/c.wav"));
    }

    #[test]
    fn velocity_layer_reduction() {
        // 16 Salamander-style layers on one key
        let bounds = [1, 27, 35, 37, 44, 47, 51, 57, 65, 73, 81, 89, 97, 105, 113, 121, 128];
        let mut zones: Vec<Zone> = (0..16)
            .map(|i| Zone {
                file: i as u32,
                lovel: bounds[i] as u8,
                hivel: (bounds[i + 1] - 1) as u8,
                ..Default::default()
            })
            .collect();
        keep_layers(&mut zones, 5);
        let got: Vec<(u32, u8, u8)> = zones.iter().map(|z| (z.file, z.lovel, z.hivel)).collect();
        assert_eq!(got, vec![(0, 0, 43), (4, 44, 64), (8, 65, 88), (11, 89, 120), (15, 121, 127)]);
    }

    #[test]
    fn zones_from_file_names() {
        let names = [
            "VlnEns_Pizz_A2_v1_rr1.wav",
            "VlnEns_Pizz_A2_v1_rr2.wav",
            "VlnEns_Pizz_A2_v2_rr1.wav",
            "VlnEns_Pizz_A2_v2_rr2.wav",
            "VlnEns_Pizz_C4_v1_rr1.wav",
            "VlnEns_Pizz_C4_v2_rr1.wav",
        ];
        let ids: Vec<(u32, String)> = names.iter().enumerate().map(|(i, n)| (i as u32, n.to_string())).collect();
        let z = zones_from_names(&ids, None, Kind::Pitched);
        // two layers, two round robins on A2, key split between A2 (45) and C4 (60)
        assert_eq!((z[0].lovel, z[0].hivel, z[0].seq_len, z[0].seq_pos), (0, 64, 2, 1));
        assert_eq!((z[1].seq_len, z[1].seq_pos), (2, 2));
        assert_eq!((z[2].lovel, z[2].hivel), (65, 127));
        assert_eq!((z[0].lokey, z[0].hikey), (0, 52));
        assert_eq!((z[4].lokey, z[4].hikey), (53, 127));
        // accent layers by regex
        let names: Vec<(u32, String)> =
            (0..11).map(|i| (i, format!("BassDrum909-accent{:03}-tune100.wav", i * 10))).collect();
        let re = Regex::new(r"accent(\d+)").unwrap();
        let z = zones_from_names(&names, Some(&re), Kind::Drum);
        assert_eq!(z.len(), 11);
        assert_eq!((z[0].lovel, z[0].hivel), (0, 12));
        assert_eq!((z[10].lovel, z[10].hivel), (116, 127));
        assert!(z.iter().all(|z| z.root == 60.0 && z.seq_len == 0));
    }

    #[test]
    fn strudel_map_instruments() {
        let json = r#"{"_base": "https://x.org/s/", "bd": ["bd/a.wav", "bd/b.wav"], "piano": {"C4": "C4.mp3", "A4": "A4.mp3", "C3": ["C3a.mp3", "C3b.mp3"]}}"#;
        let map = strudel::parse_map(json, "https://x.org/strudel.json").unwrap();
        let mut urls: Vec<String> = Vec::new();
        let mut add = |u: &str| {
            urls.push(u.to_string());
            (urls.len() - 1) as u32
        };
        let defs = instruments_from_map(&map, &InstSpec::default(), &mut add);
        let bd = defs.iter().find(|d| d.name == "bd").unwrap();
        assert_eq!(bd.kind, Kind::Drum);
        assert_eq!(bd.variants.len(), 2);
        assert!(bd.one_shot);
        let piano = defs.iter().find(|d| d.name == "piano").unwrap();
        assert_eq!(piano.kind, Kind::Pitched);
        assert_eq!(piano.variants.len(), 2);
        let z = &piano.variants[0].zones;
        let c4 = z.iter().find(|z| z.root == 60.0).unwrap();
        assert_eq!((c4.lokey, c4.hikey), (55, 64));
        assert!(urls.iter().any(|u| u == "https://x.org/s/C3b.mp3"));
    }

    #[test]
    fn sfz_regions_to_zones() {
        let src = r#"
<control> default_path=Samples/ set_hdcc72=0.5
<global> ampeg_release_oncc72=2 sw_default=c0
<master> sw_last=c0
<group> lovel=1 hivel=64
<region> sample=C4v1.flac lokey=58 hikey=61 pitch_keycenter=60 tune=-10 volume=3
<master> sw_last=c#0
<group>
<region> sample=C4retuned.flac key=60
<global> trigger=release
<region> sample=rel.flac key=60
"#;
        let file = sfz::parse(src, &mut |_| Ok(String::new())).unwrap();
        let sw = file.regions.iter().find_map(|r| r.key("sw_default"));
        let plays: Vec<bool> = file.regions.iter().map(|r| region_plays(r, &file, sw)).collect();
        assert_eq!(plays, vec![true, false, false]);
        let z = sfz_zone(&file.regions[0], &file, 7, true);
        assert_eq!((z.lokey, z.hikey, z.lovel, z.hivel), (58, 61, 1, 64));
        assert!((z.root - 60.1).abs() < 1e-4);
        assert!((z.gain_db - 3.0).abs() < 1e-4);
        assert_eq!(z.release, None);
        assert_eq!(sfz::join_path("", &file.regions[0].default_path, "C4v1.flac"), "Samples/C4v1.flac");
    }
}
