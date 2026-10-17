//! Model files (fp32 ONNX exports on the Hugging Face Hub, pinned revisions), the cache directory, downloads and
//! ONNX Runtime sessions.
//!
//! Cache: `$EARSHOT_HOME` if set, else `~/.cache/earshot-rs`; files land in `models/<repo with / as -->/<revision>/`.
//! Downloads write `<file>.<pid>.part` and rename it into place, so an interrupted download never leaves a
//! truncated model behind, and every file is checked against its pinned SHA-256 before it is used.
//! `EARSHOT_OFFLINE=1` (or `EarsOptions::offline`) refuses to download.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, anyhow, bail};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;

pub const CLAP_REPO: &str = "Xenova/larger_clap_general";
pub const CLAP_REV: &str = "d78c3994912c441f0151a5cc44d86f6b7bf4c861";
pub const CLAP_AUDIO: &str = "onnx/audio_model.onnx";
pub const CLAP_TEXT: &str = "onnx/text_model.onnx";
pub const CLAP_TOKENIZER: &str = "tokenizer.json";
pub const AST_REPO: &str = "Xenova/ast-finetuned-audioset-10-10-0.4593";
pub const AST_REV: &str = "249a1fbf0286b40e7f1ed687a8ae396997bf7dc6";
pub const AST_MODEL: &str = "onnx/model.onnx";
pub const AST_CONFIG: &str = "config.json";

/// SHA-256 of every file at its pinned revision (the ONNX files' hashes are the Hub's LFS object ids)
pub const SHA256: &[(&str, &str, &str)] = &[
    (CLAP_REPO, CLAP_AUDIO, "6192254e7f5c9a921dfd095105216c31ef70f508f910f3b926b7fc373b41f770"),
    (CLAP_REPO, CLAP_TEXT, "386413c758300d8f2be107b9fefbc049945b1b1b302938b01039af0ad4984595"),
    (CLAP_REPO, CLAP_TOKENIZER, "dc239041d98de27ffc3975473a1a23e3db4c937b23c138c38bbc66588bd247e5"),
    (AST_REPO, AST_MODEL, "6ff353b1e809da25f900b54494fd4ac12a6fc7db1e25195e238f21be125bb4bd"),
    (AST_REPO, AST_CONFIG, "49d0dce922da23685787c73a874d9b1ffa97297f8cc4dc0a4e8bef219725e44d"),
];

fn expected_sha256(repo: &str, file: &str) -> Option<&'static str> {
    SHA256.iter().find(|(r, f, _)| *r == repo && *f == file).map(|(_, _, h)| *h)
}

/// approximate download sizes (bytes) for messages
pub const SIZES: &[(&str, u64)] =
    &[(CLAP_AUDIO, 281_749_092), (CLAP_TEXT, 501_513_769), (AST_MODEL, 346_754_297), (CLAP_TOKENIZER, 2_108_774)];

/// progress hook: (file name, bytes done, bytes total or 0)
pub type Progress = dyn Fn(&str, u64, u64) + Send + Sync;
static PROGRESS: OnceLock<Box<Progress>> = OnceLock::new();

/// install a download progress callback (once per process); without one, downloads print one line to stderr
pub fn set_progress(cb: Box<Progress>) {
    let _ = PROGRESS.set(cb);
}

/// `$EARSHOT_HOME`, else `~/.cache/earshot-rs`
pub fn default_cache_dir() -> PathBuf {
    if let Ok(h) = std::env::var("EARSHOT_HOME")
        && !h.is_empty()
    {
        return PathBuf::from(h);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    home.unwrap_or_else(|| PathBuf::from(".")).join(".cache").join("earshot-rs")
}

/// `EARSHOT_OFFLINE=1`
pub fn env_offline() -> bool {
    std::env::var("EARSHOT_OFFLINE").map(|v| v == "1").unwrap_or(false)
}

/// ONNX Runtime intra-op threads: `EARSHOT_THREADS`, else min(4, cores / 2) (earshot's torch default)
pub fn default_threads() -> usize {
    if let Some(n) = std::env::var("EARSHOT_THREADS").ok().and_then(|v| v.parse::<usize>().ok()) {
        return n.max(1);
    }
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    (cores / 2).clamp(1, 4)
}

pub fn model_path(cache: &Path, repo: &str, rev: &str, file: &str) -> PathBuf {
    cache.join("models").join(repo.replace('/', "--")).join(rev).join(file)
}

/// the local path of a pinned model file, downloading it first unless offline
pub fn fetch(cache: &Path, repo: &str, rev: &str, file: &str, offline: bool) -> Result<PathBuf> {
    let dst = model_path(cache, repo, rev, file);
    if dst.is_file() {
        return Ok(dst);
    }
    if offline || env_offline() {
        bail!(
            "{} is not in the cache ({}) and offline mode is on (EARSHOT_OFFLINE=1 / EarsOptions::offline): run once online to download it",
            file,
            dst.display()
        );
    }
    download(&format!("https://huggingface.co/{repo}/resolve/{rev}/{file}"), &dst, file, expected_sha256(repo, file))?;
    Ok(dst)
}

/// is every model file `ast` needs (CLAP, and the AudioSet tagger when `ast`) in the cache?
pub fn cached(cache: &Path, ast: bool) -> bool {
    let mut files = vec![
        (CLAP_REPO, CLAP_REV, CLAP_AUDIO),
        (CLAP_REPO, CLAP_REV, CLAP_TEXT),
        (CLAP_REPO, CLAP_REV, CLAP_TOKENIZER),
    ];
    if ast {
        files.extend([(AST_REPO, AST_REV, AST_MODEL), (AST_REPO, AST_REV, AST_CONFIG)]);
    }
    files.iter().all(|(r, v, f)| model_path(cache, r, v, f).is_file())
}

fn download(url: &str, dst: &Path, label: &str, sha256: Option<&str>) -> Result<()> {
    use sha2::{Digest, Sha256};
    // one download per file at a time within this process
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if dst.is_file() {
        return Ok(());
    }
    let dir = dst.parent().ok_or_else(|| anyhow!("bad cache path {}", dst.display()))?;
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let tmp = dst.with_file_name(format!(
        "{}.{}.part",
        dst.file_name().and_then(|s| s.to_str()).unwrap_or("download"),
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let resp = ureq::get(url).call().with_context(|| format!("download failed: {url}"))?;
        let total = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        if PROGRESS.get().is_none() {
            eprintln!("earshot: downloading {label} ({:.0} MB) from {url}", total as f64 / 1e6);
        }
        let mut reader = resp.into_body().into_reader();
        let mut out = std::fs::File::create(&tmp).with_context(|| format!("cannot write {}", tmp.display()))?;
        let mut buf = vec![0u8; 1 << 20];
        let mut done = 0u64;
        let mut hash = Sha256::new();
        loop {
            let k = reader.read(&mut buf).with_context(|| format!("download interrupted: {url}"))?;
            if k == 0 {
                break;
            }
            out.write_all(&buf[..k])?;
            hash.update(&buf[..k]);
            done += k as u64;
            if let Some(cb) = PROGRESS.get() {
                cb(label, done, total);
            }
        }
        out.sync_all()?;
        if total > 0 && done != total {
            bail!("download of {url} truncated ({done} of {total} bytes)");
        }
        let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if let Some(want) = sha256
            && got != want
        {
            bail!("{label} from {url} does not match its pinned SHA-256 (got {got}, expected {want}); not using it");
        }
        std::fs::rename(&tmp, dst).with_context(|| format!("cannot move {} into place", tmp.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// map ort's error (which carries a non-Send recovery value for builders) into anyhow
pub fn ox<T, R>(r: std::result::Result<T, ort::Error<R>>) -> Result<T> {
    r.map_err(|e| anyhow!("onnxruntime: {}", e.message()))
}

fn environment() -> Result<&'static ort::environment::Environment> {
    static ENV: OnceLock<ort::environment::Environment> = OnceLock::new();
    if let Some(e) = ENV.get() {
        return Ok(e);
    }
    let e = ox(ort::init().with_name("earshot").build())?;
    Ok(ENV.get_or_init(|| e))
}

/// an inference session on the CPU with `threads` intra-op threads. Graph optimisations stay off: with them the
/// AST run peaks at 3.3 GB instead of 0.7 GB (and CLAP at 0.6 instead of 0.46 GB), at no gain in speed
pub fn session(path: &Path, threads: usize) -> Result<Session> {
    let env = environment()?;
    let b = ox(Session::builder(env))?;
    let b = ox(b.with_optimization_level(GraphOptimizationLevel::Disable))?;
    let b = ox(b.with_intra_threads(threads.max(1)))?;
    let mut b = ox(b.with_inter_threads(1))?;
    ox(b.commit_from_file(path)).with_context(|| format!("cannot load {}", path.display()))
}
