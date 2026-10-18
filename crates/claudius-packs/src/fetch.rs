//! Networking: one shared HTTP agent, GitHub tree / licence lookups, parallel downloads that are atomic per file
//! (`.part` + rename), resumable (HTTP range), size-verified and retried, archive extraction and lossless
//! WAV -> FLAC conversion.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{LazyLock, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .user_agent(concat!("claudius-packs/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_recv_body(Some(Duration::from_secs(900)))
        .build()
        .into()
});

fn github_token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
}

/// GET a small text resource (sample maps, SFZ files, API responses).
pub fn get_text(url: &str) -> Result<String> {
    let mut req = AGENT.get(url);
    if url.starts_with("https://api.github.com/") {
        req = req.header("Accept", "application/vnd.github+json");
        if let Some(t) = github_token() {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
    }
    let mut resp = req.call().with_context(|| format!("GET {url}"))?;
    let s = resp.body_mut().with_config().limit(64 << 20).read_to_string().with_context(|| format!("reading {url}"))?;
    Ok(s)
}

pub fn get_json(url: &str) -> Result<Value> {
    let t = get_text(url)?;
    serde_json::from_str(&t).with_context(|| format!("{url} is not JSON"))
}

/// raw file URL inside a GitHub repository at a revision
pub fn github_raw(repo: &str, rev: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo}/{rev}/{}", crate::strudel::encode_path(path))
}

/// every file in a GitHub repository at a revision, with sizes
pub fn github_tree(repo: &str, rev: &str) -> Result<Vec<(String, u64)>> {
    let v = get_json(&format!("https://api.github.com/repos/{repo}/git/trees/{rev}?recursive=1"))
        .with_context(|| format!("listing github:{repo}@{rev} (set GITHUB_TOKEN if the API rate limit was hit)"))?;
    if v["truncated"].as_bool() == Some(true) {
        bail!("github tree of {repo} is truncated");
    }
    let tree = v["tree"].as_array().ok_or_else(|| anyhow!("unexpected GitHub tree response for {repo}"))?;
    Ok(tree
        .iter()
        .filter(|e| e["type"] == "blob")
        .filter_map(|e| Some((e["path"].as_str()?.to_string(), e["size"].as_u64().unwrap_or(0))))
        .collect())
}

/// SPDX id of a GitHub repository's licence, when GitHub detects one
pub fn github_licence(owner: &str, repo: &str) -> Option<String> {
    let v = get_json(&format!("https://api.github.com/repos/{owner}/{repo}")).ok()?;
    let id = v["license"]["spdx_id"].as_str()?;
    if id.is_empty() || id == "NOASSERTION" { None } else { Some(id.to_string()) }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// raw.githubusercontent.com serves Git LFS pointers; media.githubusercontent.com serves the content
fn lfs_url(url: &str) -> Option<String> {
    url.strip_prefix("https://raw.githubusercontent.com/")
        .map(|rest| format!("https://media.githubusercontent.com/media/{rest}"))
}

fn is_lfs_pointer(p: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(p) else { return false };
    if meta.len() > 400 {
        return false;
    }
    std::fs::read(p).map(|b| b.starts_with(b"version https://git-lfs")).unwrap_or(false)
}

/// the most a download of unknown size may be; a server that sends more is cut off
const MAX_UNKNOWN_SIZE: u64 = 1 << 30;

/// one attempt: resume a previous `.part` when the server supports ranges
fn download_once(url: &str, part: &Path, expected: u64, bytes: &dyn Fn(u64)) -> Result<()> {
    let have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let have = if expected > 0 && have >= expected { 0 } else { have };
    let mut req = AGENT.get(url);
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(416)) => {
            let _ = std::fs::remove_file(part);
            AGENT.get(url).call().with_context(|| format!("GET {url}"))?
        }
        Err(e) => return Err(anyhow!(e).context(format!("GET {url}"))),
    };
    let resumed = resp.status().as_u16() == 206 && have > 0;
    let start = if resumed { have } else { 0 };
    let len = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    let total = len.map(|l| l + start);
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(part)
        .with_context(|| format!("cannot write {}", part.display()))?;
    let mut rd = resp.into_body().into_reader();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = start;
    let limit = if expected > 0 { expected } else { MAX_UNKNOWN_SIZE };
    loop {
        let k = rd.read(&mut buf).with_context(|| format!("download interrupted: {url}"))?;
        if k == 0 {
            break;
        }
        if done + k as u64 > limit {
            drop(out);
            let _ = std::fs::remove_file(part);
            bail!("{url} sends more than {limit} bytes");
        }
        out.write_all(&buf[..k])?;
        done += k as u64;
        bytes(k as u64);
    }
    out.flush()?;
    drop(out);
    if let Some(t) = total
        && done != t
    {
        bail!("{url}: truncated ({done} of {t} bytes)");
    }
    Ok(())
}

/// Download `url` to `dest` atomically (`dest.part`, verified, renamed), with retries and resume. With `flac`, a
/// WAV download is converted to lossless FLAC at `dest`.
pub fn download_file(url: &str, dest: &Path, expected: u64, flac: bool) -> Result<u64> {
    download_file_with(url, dest, expected, flac, &|_| {})
}

fn download_file_with(url: &str, dest: &Path, expected: u64, flac: bool, bytes: &dyn Fn(u64)) -> Result<u64> {
    if dest.is_file() {
        return Ok(0);
    }
    let dir = dest.parent().ok_or_else(|| anyhow!("bad path {}", dest.display()))?;
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let part = part_path(dest);
    let mut url = url.to_string();
    let mut last_err = None;
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500 * (1 << attempt)));
        }
        match download_once(&url, &part, expected, bytes) {
            Ok(()) => {
                if is_lfs_pointer(&part)
                    && let Some(u) = lfs_url(&url)
                {
                    let _ = std::fs::remove_file(&part);
                    url = u;
                    continue;
                }
                let got = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                if expected > 0 && got != expected {
                    let _ = std::fs::remove_file(&part);
                    last_err = Some(anyhow!("{url}: size {got} != expected {expected}"));
                    continue;
                }
                if flac {
                    let tmp = dest.with_extension("flac.tmp");
                    let res = transcode_flac(&part, &tmp);
                    let _ = std::fs::remove_file(&part);
                    if let Err(e) = res {
                        let _ = std::fs::remove_file(&tmp);
                        return Err(e);
                    }
                    std::fs::rename(&tmp, dest).with_context(|| format!("cannot move {} into place", tmp.display()))?;
                } else {
                    std::fs::rename(&part, dest)
                        .with_context(|| format!("cannot move {} into place", part.display()))?;
                }
                return Ok(got);
            }
            Err(e) => {
                // 4xx other than range problems will not get better
                let fatal = e.chain().any(|c| {
                    c.downcast_ref::<ureq::Error>()
                        .is_some_and(|u| matches!(u, ureq::Error::StatusCode(s) if (400..500).contains(s) && *s != 429))
                });
                last_err = Some(e);
                if fatal {
                    let _ = std::fs::remove_file(&part);
                    break;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("download failed: {url}")))
}

/// the SHA-256 of a file, in lowercase hex
pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        match file.read(&mut buf)? {
            0 => break,
            n => hasher.update(&buf[..n]),
        }
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The WAV with its fmt chunk cut to the 16 bytes PCM needs, when it is longer (some packs' files carry extra bytes
/// there, and hound refuses any size it does not expect); None when it is not such a file. The audio is untouched.
fn pcm_fmt_trimmed(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..12]);
    let (mut at, mut trimmed) = (12, false);
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let end = (at + 8 + size).min(bytes.len());
        let body = &bytes[at + 8..end];
        let pcm = body.len() >= 16 && matches!(u16::from_le_bytes([body[0], body[1]]), 1 | 3);
        if &bytes[at..at + 4] == b"fmt " && size > 16 && pcm {
            out.extend_from_slice(b"fmt ");
            out.extend_from_slice(&16u32.to_le_bytes());
            out.extend_from_slice(&body[..16]);
            trimmed = true;
        } else {
            out.extend_from_slice(&bytes[at..end]);
            if size % 2 == 1 && end < bytes.len() {
                out.push(bytes[end]);
            }
        }
        at = end + (size % 2);
    }
    let riff = u32::try_from(out.len() - 8).ok()?;
    out[4..8].copy_from_slice(&riff.to_le_bytes());
    trimmed.then_some(out)
}

/// lossless WAV -> FLAC (keeps rate, channels and bit depth; float WAV becomes 24-bit)
pub fn transcode_flac(src: &Path, dst: &Path) -> Result<()> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let raw = std::fs::read(src).with_context(|| format!("cannot read {}", src.display()))?;
    let bytes = match hound::WavReader::new(std::io::Cursor::new(&raw[..])) {
        Ok(_) => raw,
        Err(e) => pcm_fmt_trimmed(&raw).ok_or_else(|| anyhow!(e).context(format!("cannot read {}", src.display())))?,
    };
    let mut rd = hound::WavReader::new(std::io::Cursor::new(&bytes[..]))
        .with_context(|| format!("cannot read {}", src.display()))?;
    let spec = rd.spec();
    let (samples, bits): (Vec<i32>, usize) = match spec.sample_format {
        hound::SampleFormat::Int if spec.bits_per_sample <= 24 => {
            (rd.samples::<i32>().collect::<Result<_, _>>()?, spec.bits_per_sample as usize)
        }
        hound::SampleFormat::Int => (rd.samples::<i32>().map(|v| v.map(|v| v >> 8)).collect::<Result<_, _>>()?, 24),
        hound::SampleFormat::Float => (
            rd.samples::<f32>()
                .map(|v| v.map(|v| (v.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32))
                .collect::<Result<_, _>>()?,
            24,
        ),
    };
    let config =
        flacenc::config::Encoder::default().into_verified().map_err(|e| anyhow!("FLAC encoder settings: {e:?}"))?;
    let source =
        flacenc::source::MemSource::from_samples(&samples, spec.channels as usize, bits, spec.sample_rate as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| anyhow!("cannot encode {} as FLAC: {e:?}", src.display()))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|e| anyhow!("cannot write FLAC: {e:?}"))?;
    let mut bytes = sink.as_slice().to_vec();
    // STREAMINFO's minimum block size excludes the last block (FLAC format, section 8.2); flacenc counts it, which
    // makes a fixed-block-size stream look variable and some decoders (symphonia) refuse to open it
    if bytes.len() > 12 && &bytes[..4] == b"fLaC" && bytes[4] & 0x7f == 0 {
        let max = [bytes[10], bytes[11]];
        bytes[8..10].copy_from_slice(&max);
    }
    std::fs::write(dst, bytes).with_context(|| format!("cannot write {}", dst.display()))?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Task {
    pub url: String,
    pub dest: PathBuf,
    pub size: u64,
    pub flac: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub files: usize,
    pub downloaded_bytes: u64,
    pub skipped: usize,
}

enum Event {
    Bytes(u64),
    Done,
    Failed(usize, String),
}

/// Download many files with `threads` workers. Files already present are skipped, so an interrupted install
/// resumes. The progress callback runs on the calling thread.
pub fn download_all(tasks: &[Task], threads: usize, label: &str, progress: &dyn Fn(f32, &str)) -> Result<Stats> {
    let mut stats = Stats::default();
    let todo: Vec<&Task> = tasks.iter().filter(|t| !t.dest.is_file()).collect();
    stats.skipped = tasks.len() - todo.len();
    stats.files = tasks.len();
    if todo.is_empty() {
        progress(1.0, &format!("{label}: all {} files present", tasks.len()));
        return Ok(stats);
    }
    let total_bytes: u64 = todo.iter().map(|t| t.size).sum();
    let known = todo.iter().all(|t| t.size > 0);
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let (tx, rx) = mpsc::channel::<Event>();
    let mut failures: Vec<String> = Vec::new();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1).min(todo.len()) {
            let tx = tx.clone();
            let (next, stop, todo) = (&next, &stop, &todo);
            s.spawn(move || {
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= todo.len() {
                        break;
                    }
                    let t = todo[i];
                    let txb = tx.clone();
                    let r = download_file_with(&t.url, &t.dest, t.size, t.flac, &move |n| {
                        let _ = txb.send(Event::Bytes(n));
                    });
                    let _ = tx.send(match r {
                        Ok(_) => Event::Done,
                        Err(e) => Event::Failed(i, format!("{e:#}")),
                    });
                }
            });
        }
        drop(tx);
        let mut done = 0usize;
        let mut bytes = 0u64;
        let mut last = Instant::now() - Duration::from_secs(1);
        for ev in rx {
            match ev {
                Event::Bytes(n) => bytes += n,
                Event::Done => done += 1,
                Event::Failed(i, e) => {
                    done += 1;
                    failures.push(format!("{}: {e}", todo[i].url));
                    if failures.len() > 20 {
                        stop.store(true, Ordering::Relaxed);
                    }
                }
            }
            if last.elapsed() > Duration::from_millis(150) || done == todo.len() {
                last = Instant::now();
                let frac = if known && total_bytes > 0 {
                    bytes as f32 / total_bytes as f32
                } else {
                    done as f32 / todo.len() as f32
                };
                progress(frac.min(1.0), &format!("{label}: {done}/{} files, {:.1} MB", todo.len(), bytes as f64 / 1e6));
            }
        }
        stats.downloaded_bytes = bytes;
    });
    if !failures.is_empty() {
        bail!("{} download(s) failed (run install again to resume):\n  {}", failures.len(), failures.join("\n  "));
    }
    Ok(stats)
}

/// Extract an archive (zip, 7z, tar.*) with the system tools: bsdtar reads all of them; GNU systems fall back to
/// unzip / 7z. The symbolic links an archive holds are deleted afterwards: one could point anywhere on the computer,
/// and the files of a pack are copied out of `dir`.
pub fn extract_archive(archive: &Path, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let name = archive.to_string_lossy().to_ascii_lowercase();
    let tar = Command::new("tar").arg("-xf").arg(archive).arg("-C").arg(dir).output();
    if matches!(&tar, Ok(o) if o.status.success()) {
        return remove_links(dir);
    }
    if name.ends_with(".zip") {
        let o = Command::new("unzip").args(["-q", "-o"]).arg(archive).arg("-d").arg(dir).output();
        if matches!(&o, Ok(o) if o.status.success()) {
            return remove_links(dir);
        }
    }
    if name.ends_with(".7z") {
        for tool in ["7z", "7za", "7zz"] {
            let o = Command::new(tool).arg("x").arg("-y").arg(format!("-o{}", dir.display())).arg(archive).output();
            if matches!(&o, Ok(o) if o.status.success()) {
                return remove_links(dir);
            }
        }
    }
    let msg = match tar {
        Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
        Err(e) => e.to_string(),
    };
    bail!("cannot extract {}: {msg}", archive.display())
}

/// delete every symbolic link under `dir`, without following any
fn remove_links(dir: &Path) -> Result<()> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let kind = e.file_type()?;
            if kind.is_symlink() {
                // Windows removes a link to a folder as a folder
                std::fs::remove_file(e.path())
                    .or_else(|_| std::fs::remove_dir(e.path()))
                    .with_context(|| format!("cannot remove the link {}", e.path().display()))?;
            } else if kind.is_dir() {
                stack.push(e.path());
            }
        }
    }
    Ok(())
}

/// all files under `dir`, as paths relative to it with `/` separators; symbolic links are left out, not followed
pub fn walk(dir: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let p = e.path();
            if kind.is_dir() {
                stack.push(p);
            } else if kind.is_file()
                && let Ok(rel) = p.strip_prefix(dir)
            {
                let rel = rel.to_string_lossy().replace('\\', "/");
                out.push((rel, e.metadata().map(|m| m.len()).unwrap_or(0)));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod flac_tests {
    #[test]
    fn a_wav_with_a_long_fmt_chunk_still_becomes_flac() {
        let dir = tempfile::tempdir().unwrap();
        let (wav, flac) = (dir.path().join("in.wav"), dir.path().join("out.flac"));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        let samples: Vec<i16> = (0..2000).map(|i| ((i as f64 * 0.07).sin() * 12_000.0) as i16).collect();
        for s in &samples {
            w.write_sample(*s).unwrap();
        }
        w.finalize().unwrap();
        // the fmt chunk as some recorders write it: 20 bytes, the last 4 of them extra
        let b = std::fs::read(&wav).unwrap();
        let fmt = b.windows(4).position(|w| w == b"fmt ").unwrap();
        let mut odd = b[..fmt + 4].to_vec();
        odd.extend_from_slice(&20u32.to_le_bytes());
        odd.extend_from_slice(&b[fmt + 8..fmt + 24]);
        odd.extend_from_slice(&[0, 0, 0, 0]);
        odd.extend_from_slice(&b[fmt + 24..]);
        let riff = (odd.len() - 8) as u32;
        odd[4..8].copy_from_slice(&riff.to_le_bytes());
        std::fs::write(&wav, &odd).unwrap();
        assert!(hound::WavReader::open(&wav).is_err(), "hound refuses the file as it is");
        super::transcode_flac(&wav, &flac).unwrap();
        let d = claudius_decode::decode_native(&flac).unwrap();
        assert_eq!((d.rate, d.channels, d.samples.len()), (44_100, 2, samples.len()));
        let back: Vec<i16> = d.samples.iter().map(|x| (x * 32768.0).round() as i16).collect();
        assert_eq!(back, samples, "lossless");
    }

    #[cfg(unix)]
    #[test]
    fn links_in_an_archive_lead_nowhere() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir_all(outside.join("docs")).unwrap();
        std::fs::write(outside.join("secret.txt"), b"key").unwrap();
        let dir = root.path().join("unpacked");
        std::fs::create_dir_all(dir.join("BassDrum")).unwrap();
        std::fs::write(dir.join("BassDrum/kick.wav"), b"RIFF").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), dir.join("BassDrum/snare.wav")).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("loot")).unwrap();
        assert_eq!(super::walk(&dir), vec![("BassDrum/kick.wav".to_string(), 4)], "the walk follows no link");
        super::remove_links(&dir).unwrap();
        assert!(!dir.join("BassDrum/snare.wav").exists() && !dir.join("loot").exists());
        assert!(dir.join("BassDrum/kick.wav").is_file());
        assert_eq!(std::fs::read(outside.join("secret.txt")).unwrap(), b"key", "what a link pointed at stays");
        assert!(outside.join("docs").is_dir());
    }

    #[test]
    fn wav_to_flac_is_lossless() {
        let dir = std::env::temp_dir().join(format!("claudius-flac-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (wav, flac) = (dir.join("in.wav"), dir.join("out.flac"));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&wav, spec).unwrap();
        let values: Vec<i16> = (0..20_000).map(|i| ((i as f64 * 0.013).sin() * 20_000.0) as i16).collect();
        for v in &values {
            w.write_sample(*v).unwrap();
        }
        w.finalize().unwrap();
        super::transcode_flac(&wav, &flac).unwrap();
        let d = claudius_decode::decode_native(&flac).unwrap();
        assert_eq!((d.rate, d.channels, d.samples.len()), (44_100, 2, values.len()));
        for (a, b) in d.samples.iter().zip(&values) {
            assert!((a * 32768.0 - *b as f32).abs() < 0.5, "{a} vs {b}");
        }
        assert!(std::fs::metadata(&flac).unwrap().len() < std::fs::metadata(&wav).unwrap().len());
        std::fs::remove_dir_all(&dir).ok();
    }
}
