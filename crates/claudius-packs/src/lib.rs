//! Sample packs for claudius: a curated, licence-checked library of multi-sampled instruments and drum kits that
//! installs on demand, Strudel sample-map compatibility (`samples('github:user/repo')`), and a player that renders
//! notes with them at 48 kHz stereo.
//!
//! ```no_run
//! use claudius_packs::{NoteOn, instrument};
//! let piano = instrument("piano", None)?;
//! let notes = [NoteOn { start: 0, gate: 24_000, pitch: 60.0, velocity: 0.8 }];
//! let (left, right) = piano.render(&notes, 96_000);
//! let kick = instrument("bd", Some("tr909"))?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! Packs live in `$CLAUDIUS_PACKS`, else `<data dir>/claudius/packs` (macOS: `~/Library/Application Support/
//! claudius/packs`), one directory per pack with an `index.json` (metadata, licence, files, zones).

#![forbid(unsafe_code)]

mod catalog;
mod decode;
mod fetch;
mod install;
mod model;
mod notes;
mod player;
mod resample;
mod resolve;
mod sfz;
mod strudel;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

pub use install::InstallOptions;
pub use model::{Kind, PackIndex};
pub use player::{Instrument, InstrumentInfo, NoteOn, SR};
pub use resolve::InstrumentRef;

/// Parsing helpers (note names, file-name analysis, Strudel maps, SFZ) for callers that want them.
pub mod parse {
    pub use crate::notes::{NameInfo, analyze_name, drum_canonical, note_name, parse_note};
    pub use crate::sfz::{SfzFile, SfzRegion, parse as parse_sfz};
    pub use crate::strudel::{MapValue, SampleMap, map_url, parse_map};
}

/// A pack as claudius shows it: curated (installed or not) or added from a sample map.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PackInfo {
    pub name: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub licence: String,
    pub licence_url: String,
    pub attribution: String,
    /// the licence requires crediting (CC BY ...): include `attribution` in song credits
    pub attribution_required: bool,
    pub licence_note: String,
    pub source: String,
    pub installed: bool,
    /// samples download on first use
    pub lazy: bool,
    /// disk usage when installed, otherwise the approximate download
    pub size_mb: f32,
    pub default: bool,
    /// unclear licence: never installed unless explicitly accepted
    pub opt_in: bool,
    pub warning: Option<String>,
    /// added with `add_sample_map`
    pub user: bool,
    pub instruments: usize,
}

/// Where packs are installed: `$CLAUDIUS_PACKS`, else `<data dir>/claudius/packs`.
pub fn packs_dir() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDIUS_PACKS")
        && !p.trim().is_empty()
    {
        return PathBuf::from(p);
    }
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("claudius").join("packs")
}

/// Scratch space for archives being installed: `<cache dir>/claudius/packs`.
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("claudius").join("packs")
}

static INDEXES: Mutex<Option<Vec<Arc<PackIndex>>>> = Mutex::new(None);
static INSTRUMENTS: Mutex<Option<HashMap<InstrumentRef, Arc<Instrument>>>> = Mutex::new(None);

/// Forget cached indexes and instruments (after packs changed on disk, e.g. from another process).
pub fn refresh() {
    *INDEXES.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *INSTRUMENTS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn load_indexes() -> Vec<Arc<PackIndex>> {
    let dir = packs_dir();
    let mut out: Vec<Arc<PackIndex>> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path().join("index.json");
            if !p.is_file() {
                continue;
            }
            match std::fs::read(&p)
                .map_err(anyhow::Error::from)
                .and_then(|b| serde_json::from_slice::<PackIndex>(&b).map_err(Into::into))
            {
                Ok(ix) => out.push(Arc::new(ix)),
                Err(e) => eprintln!("claudius-packs: ignoring {}: {e:#}", p.display()),
            }
        }
    }
    // curated packs in registry order, then user packs by name
    let pos = |n: &str| catalog::REGISTRY.packs.iter().position(|p| p.name == n).unwrap_or(usize::MAX);
    out.sort_by(|a, b| pos(&a.name).cmp(&pos(&b.name)).then(a.name.cmp(&b.name)));
    out
}

static DISABLED: AtomicBool = AtomicBool::new(false);

/// Leave the installed packs out of this process (`claudius --synth-only`): nothing resolves to a sampled
/// instrument, so every sound is synthesized.
pub fn set_disabled(off: bool) {
    DISABLED.store(off, Ordering::Relaxed);
    refresh();
}

/// Are the packs left out of this process (`set_disabled(true)`)?
pub fn disabled() -> bool {
    DISABLED.load(Ordering::Relaxed)
}

/// Installed pack indexes (cached per process); none after `set_disabled(true)`.
pub fn installed() -> Vec<Arc<PackIndex>> {
    if DISABLED.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let mut g = INDEXES.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(load_indexes).clone()
}

fn dir_mb(p: &std::path::Path) -> f32 {
    fetch::walk(p).iter().map(|(_, s)| *s).sum::<u64>() as f32 / 1e6
}

fn info_from_index(ix: &PackIndex) -> PackInfo {
    PackInfo {
        name: ix.name.clone(),
        title: ix.title.clone(),
        description: ix.description.clone(),
        categories: ix.categories.clone(),
        licence: ix.licence.clone(),
        licence_url: ix.licence_url.clone(),
        attribution: ix.attribution.clone(),
        attribution_required: ix.attribution_required,
        licence_note: ix.licence_note.clone(),
        source: ix.source.clone(),
        installed: true,
        lazy: ix.lazy,
        size_mb: dir_mb(&packs_dir().join(&ix.name)),
        default: ix.default,
        opt_in: ix.opt_in,
        warning: ix.warning.clone(),
        user: ix.user,
        instruments: ix.instruments.len(),
    }
}

/// Every curated pack (installed or not) and every user-added pack.
pub fn packs() -> Vec<PackInfo> {
    let inst = installed();
    let mut out: Vec<PackInfo> = catalog::REGISTRY
        .packs
        .iter()
        .map(|p| match inst.iter().find(|i| i.name == p.name) {
            Some(ix) => info_from_index(ix),
            None => PackInfo {
                name: p.name.clone(),
                title: p.title.clone(),
                description: p.description.clone(),
                categories: p.categories.clone(),
                licence: p.licence.clone(),
                licence_url: p.licence_url.clone(),
                attribution: p.attribution.clone(),
                attribution_required: p.attribution_required,
                licence_note: p.licence_note.clone(),
                source: p.source.clone(),
                installed: false,
                lazy: p.lazy,
                size_mb: p.size_mb,
                default: p.default,
                opt_in: p.opt_in,
                warning: p.warning.clone(),
                user: false,
                instruments: p.instruments.len(),
            },
        })
        .collect();
    for ix in inst.iter().filter(|i| catalog::spec(&i.name).is_none()) {
        out.push(info_from_index(ix));
    }
    out
}

/// Install a pack by name with default options (opt-in packs are refused). The progress callback receives the
/// fraction done (0..1) and a message, on the calling thread.
pub fn install(name: &str, progress: &dyn Fn(f32, &str)) -> Result<()> {
    install_with(name, &InstallOptions::default(), progress)
}

pub fn install_with(name: &str, opts: &InstallOptions, progress: &dyn Fn(f32, &str)) -> Result<()> {
    let res = match catalog::spec(name) {
        Some(spec) => install::install_spec(spec, opts, progress).map(|_| ()),
        None => {
            // a user pack: fetch every sample now
            let ix = installed().into_iter().find(|p| p.name == name).ok_or_else(|| anyhow!("unknown pack {name}"))?;
            let dir = packs_dir().join(&ix.name);
            let tasks: Vec<fetch::Task> = ix
                .files
                .iter()
                .filter(|f| !f.url.is_empty())
                .map(|f| fetch::Task { url: f.url.clone(), dest: dir.join(&f.path), size: f.size, flac: f.flac })
                .collect();
            fetch::download_all(&tasks, if opts.threads > 0 { opts.threads } else { 8 }, &ix.name, progress).map(|_| ())
        }
    };
    refresh();
    res
}

/// Install every pack marked default.
pub fn install_defaults(progress: &dyn Fn(f32, &str)) -> Result<()> {
    let mut failed = Vec::new();
    for p in catalog::REGISTRY.packs.iter().filter(|p| p.default) {
        if let Err(e) = install(&p.name, progress) {
            eprintln!("claudius-packs: {}: {e:#}", p.name);
            failed.push(p.name.clone());
        }
    }
    if !failed.is_empty() {
        bail!("could not install: {} (run again to resume)", failed.join(", "));
    }
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let name = catalog::spec(name).map_or(name.to_string(), |s| s.name.clone());
    let r = install::remove_dir(&name);
    refresh();
    r
}

fn build_instrument(ix: &PackIndex, r: &InstrumentRef) -> Result<Instrument> {
    let def = ix
        .instruments
        .iter()
        .find(|d| d.name == r.instrument)
        .ok_or_else(|| anyhow!("no instrument {} in {}", r.instrument, ix.name))?;
    if r.variant >= def.variants.len() {
        bail!("{} has {} variants", r, def.variants.len());
    }
    let dir = packs_dir().join(&ix.name);
    let files = ix.files.iter().map(|f| dir.join(&f.path)).collect();
    let remotes = ix
        .files
        .iter()
        .map(|f| (!f.url.is_empty()).then(|| decode::Remote { url: f.url.clone(), size: f.size, flac: f.flac }))
        .collect();
    Ok(Instrument::new(&ix.name, def.clone(), r.variant, files, remotes, &ix.licence, &ix.attribution))
}

/// Every installed instrument (variant 0 of each; `InstrumentInfo::variants` says how many there are).
pub fn instruments() -> Vec<InstrumentInfo> {
    let mut out = Vec::new();
    for ix in installed() {
        for d in &ix.instruments {
            let r = InstrumentRef { pack: ix.name.clone(), instrument: d.name.clone(), variant: 0 };
            if let Ok(i) = build_instrument(&ix, &r) {
                out.push(i.info());
            }
        }
    }
    out
}

/// Resolve a name (`piano`, `bd` with bank `tr909`, `tr909:bd:2`, `vsco2:violins`, `RolandTR909_bd`, `kick` ...)
/// to an installed instrument.
pub fn resolve(name: &str, bank: Option<&str>) -> Option<InstrumentRef> {
    resolve::resolve_in(&installed(), name, bank)
}

/// The instrument for a resolved reference (cached per process).
pub fn instrument_ref(r: &InstrumentRef) -> Result<Arc<Instrument>> {
    {
        let g = INSTRUMENTS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = g.as_ref().and_then(|m| m.get(r)) {
            return Ok(i.clone());
        }
    }
    let ix = installed()
        .into_iter()
        .find(|p| p.name == r.pack)
        .ok_or_else(|| anyhow!("pack {} is not installed", r.pack))?;
    let inst = Arc::new(build_instrument(&ix, r)?);
    let mut g = INSTRUMENTS.lock().unwrap_or_else(|e| e.into_inner());
    Ok(g.get_or_insert_with(HashMap::new).entry(r.clone()).or_insert(inst).clone())
}

/// Resolve and load an instrument (cached per process; `Arc<Instrument>` is Send + Sync).
pub fn instrument(name: &str, bank: Option<&str>) -> Result<Arc<Instrument>> {
    let r = resolve(name, bank).ok_or_else(|| {
        let hint = if disabled() {
            " (the sample packs are off in this session)"
        } else if installed().is_empty() {
            " (no packs installed: claudius-packs install --defaults)"
        } else {
            ""
        };
        match bank {
            Some(b) => anyhow!("no installed instrument {name:?} in bank {b:?}{hint}"),
            None => anyhow!("no installed instrument {name:?}{hint}"),
        }
    })?;
    instrument_ref(&r)
}

/// Add any Strudel sample map as a user pack (installed lazily): a URL to a JSON map, or `github:user/repo` /
/// `github:user/repo/branch/path` (reads `strudel.json` there). The licence comes from the map (`_license`) or the
/// GitHub repository, otherwise it is "unknown".
pub fn add_sample_map(url_or_github: &str) -> Result<PackInfo> {
    let ix = install::add_map(url_or_github)?;
    refresh();
    Ok(info_from_index(&ix))
}

fn credit_line(ix: &PackIndex) -> String {
    let mut s = format!("- {}", ix.attribution);
    let squash = |t: &str| t.to_ascii_lowercase().replace(['-', ' '], "");
    if !ix.licence.is_empty() && !squash(&ix.attribution).contains(&squash(&ix.licence)) {
        s.push_str(&format!(" [{}]", ix.licence));
    }
    if !ix.licence_url.is_empty() {
        s.push_str(&format!(" {}", ix.licence_url));
    }
    s
}

/// Attribution lines for every installed pack whose licence requires credit (CC BY ...), for song credits.
pub fn credits() -> String {
    let lines: Vec<String> = installed().iter().filter(|p| p.attribution_required).map(|p| credit_line(p)).collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("Sounds:\n{}\n", lines.join("\n"))
}

/// Credits for the packs behind specific instrument names (only what a song actually used): required
/// attributions first, then courtesy credits, then a warning for anything with an unknown licence.
pub fn credits_for(names: &[&str]) -> String {
    let inst = installed();
    let mut used: BTreeMap<String, Arc<PackIndex>> = BTreeMap::new();
    for n in names {
        let (name, bank) = match n.split_once('@') {
            Some((a, b)) => (a, Some(b)),
            None => (*n, None),
        };
        if let Some(r) = resolve::resolve_in(&inst, name, bank)
            && let Some(ix) = inst.iter().find(|p| p.name == r.pack)
        {
            used.insert(ix.name.clone(), ix.clone());
        }
    }
    let mut req: Vec<String> = Vec::new();
    let mut courtesy: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for ix in used.values() {
        if ix.licence == "unknown" {
            unknown.push(ix.name.clone());
        }
        if ix.attribution_required { req.push(credit_line(ix)) } else { courtesy.push(credit_line(ix)) }
    }
    let mut s = String::new();
    if !req.is_empty() || !courtesy.is_empty() {
        s.push_str("Sounds:\n");
        for l in req.iter().chain(&courtesy) {
            s.push_str(l);
            s.push('\n');
        }
    }
    if !unknown.is_empty() {
        s.push_str(&format!("Warning: licence unknown for {}\n", unknown.join(", ")));
    }
    s
}

/// Friendly names and what they currently resolve to (installed packs only).
pub fn aliases() -> Vec<(String, InstrumentRef)> {
    let inst = installed();
    let mut out = Vec::new();
    for (names, _) in resolve::ALIASES {
        for n in *names {
            if let Some(r) = resolve::resolve_in(&inst, n, None) {
                out.push((n.to_string(), r));
            }
        }
    }
    for (drum, syn) in notes::DRUM_NAMES {
        for n in std::iter::once(drum).chain(syn.iter()) {
            if let Some(r) = resolve::resolve_in(&inst, n, None) {
                out.push((n.to_string(), r));
            }
        }
    }
    out
}

/// Version of the embedded curated registry.
pub fn registry_version() -> u32 {
    catalog::REGISTRY.version
}

/// Bytes of decoded audio currently cached in memory (bounded by `CLAUDIUS_PACKS_CACHE_MB`, default 3072).
pub fn cache_bytes() -> usize {
    decode::cache_bytes()
}

/// Read an installed pack's index directly.
pub fn pack_index(name: &str) -> Result<Arc<PackIndex>> {
    installed().into_iter().find(|p| p.name == name).with_context(|| format!("pack {name} is not installed"))
}
