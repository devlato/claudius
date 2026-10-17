//! Decoding to f32 at the file's own sample rate (the player folds rate conversion into its pitch step, so audio is
//! interpolated once), and a process-wide cache of decoded samples bounded by memory.
//!
//! WAV is read with hound (which also keeps its loop points); everything else (FLAC, MP3, OGG, AIFF ...) goes through
//! claudius-decode.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use anyhow::{Context, Result, bail};

#[derive(Debug)]
pub struct Sample {
    pub sr: u32,
    pub l: Vec<f32>,
    /// `None` for mono files (both channels read `l`)
    pub r: Option<Vec<f32>>,
    pub peak: f32,
    /// loop points from a WAV `smpl` chunk, in samples
    pub file_loop: Option<(u32, u32)>,
}

impl Sample {
    pub fn len(&self) -> usize {
        self.l.len()
    }

    pub fn is_empty(&self) -> bool {
        self.l.is_empty()
    }

    pub fn right(&self) -> &[f32] {
        self.r.as_deref().unwrap_or(&self.l)
    }

    pub fn bytes(&self) -> usize {
        (self.l.len() + self.r.as_ref().map_or(0, |r| r.len())) * 4
    }
}

pub fn decode(path: &Path) -> Result<Sample> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut s = if ext == "wav" || ext == "wave" {
        match decode_wav(path) {
            Ok(s) => s,
            Err(_) => decode_any(path)?,
        }
    } else {
        decode_any(path)?
    };
    if ext == "wav" || ext == "wave" {
        s.file_loop = wav_loop(path);
    }
    Ok(s)
}

fn decode_wav(path: &Path) -> Result<Sample> {
    let mut rd = hound::WavReader::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let spec = rd.spec();
    let ch = spec.channels as usize;
    if ch == 0 || ch > 2 {
        bail!("{} channels", ch);
    }
    let data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample.max(1) - 1)) as f32;
            rd.samples::<i32>().map(|v| v.map(|v| v as f32 * scale)).collect::<Result<_, _>>()?
        }
    };
    Ok(finish(spec.sample_rate, data, ch))
}

/// any other format (FLAC, MP3, OGG, AIFF ...) through claudius-decode, at the file's own rate
fn decode_any(path: &Path) -> Result<Sample> {
    let d = claudius_decode::decode(path)?;
    if d.channels > 2 {
        let st: Vec<f32> = d.samples.chunks_exact(d.channels).flat_map(|f| [f[0], f[1]]).collect();
        return Ok(finish(d.rate, st, 2));
    }
    Ok(finish(d.rate, d.samples, d.channels))
}

fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// split channels, measure the peak, trim the silent tail (> 80 dB below the peak) and fade the cut
fn finish(sr: u32, data: Vec<f32>, ch: usize) -> Sample {
    let (mut l, mut r) = if ch == 1 {
        (data, None)
    } else {
        let n = data.len() / 2;
        let mut l = Vec::with_capacity(n);
        let mut r = Vec::with_capacity(n);
        for f in 0..n {
            l.push(data[2 * f]);
            r.push(data[2 * f + 1]);
        }
        // identical channels: keep one
        if l == r { (l, None) } else { (l, Some(r)) }
    };
    let peak = l.iter().chain(r.iter().flatten()).fold(0.0f32, |m, v| m.max(v.abs()));
    let thresh = (peak * 1e-4).max(1e-7);
    let last = (0..l.len()).rev().find(|&i| l[i].abs() > thresh || r.as_ref().is_some_and(|r| r[i].abs() > thresh));
    let keep = last.map_or(0, |i| (i + 1 + sr as usize / 50).min(l.len()));
    if keep < l.len() {
        l.truncate(keep);
        if let Some(r) = r.as_mut() {
            r.truncate(keep);
        }
    }
    // a short fade at the very end so a sample that stops abruptly never clicks
    let fade = (sr as usize / 500).min(l.len());
    let n = l.len();
    for k in 0..fade {
        let g = 0.5 - 0.5 * (std::f32::consts::PI * k as f32 / fade as f32).cos();
        l[n - 1 - k] *= g;
        if let Some(r) = r.as_mut() {
            r[n - 1 - k] *= g;
        }
    }
    Sample { sr, l, r, peak, file_loop: None }
}

/// first loop of a WAV `smpl` chunk
pub fn wav_loop(path: &Path) -> Option<(u32, u32)> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return None;
    }
    use std::io::{Seek, SeekFrom};
    loop {
        let mut ch = [0u8; 8];
        f.read_exact(&mut ch).ok()?;
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        if &ch[0..4] == b"smpl" && size >= 36 {
            let mut body = vec![0u8; size.min(4096) as usize];
            f.read_exact(&mut body).ok()?;
            let n = u32le(&body, 28);
            if n >= 1 && body.len() >= 36 + 24 {
                let (s, e) = (u32le(&body, 36 + 8), u32le(&body, 36 + 12));
                if e > s {
                    return Some((s, e + 1));
                }
            }
            return None;
        }
        f.seek(SeekFrom::Current((size + (size & 1)) as i64)).ok()?;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// cache

struct Slot {
    cell: OnceLock<Arc<Sample>>,
    init: Mutex<()>,
    last: AtomicU64,
}

struct Cache {
    map: Mutex<HashMap<PathBuf, Arc<Slot>>>,
    used: AtomicUsize,
    clock: AtomicU64,
    budget: usize,
}

static CACHE: LazyLock<Cache> = LazyLock::new(|| {
    let mb = std::env::var("CLAUDIUS_PACKS_CACHE_MB").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(3072);
    Cache { map: Mutex::new(HashMap::new()), used: AtomicUsize::new(0), clock: AtomicU64::new(1), budget: mb << 20 }
});

/// How to obtain a file that is not on disk yet (lazily installed packs).
#[derive(Clone, Debug)]
pub struct Remote {
    pub url: String,
    pub size: u64,
    pub flac: bool,
}

/// Decoded sample for `path`, decoding (and downloading, for lazy packs) on first use. Thread-safe; concurrent
/// requests for the same file wait for one decode.
pub fn load(path: &Path, remote: Option<&Remote>) -> Result<Arc<Sample>> {
    let slot = {
        let mut m = CACHE.map.lock().unwrap_or_else(|e| e.into_inner());
        m.entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Slot { cell: OnceLock::new(), init: Mutex::new(()), last: AtomicU64::new(0) }))
            .clone()
    };
    slot.last.store(CACHE.clock.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
    if let Some(s) = slot.cell.get() {
        return Ok(s.clone());
    }
    let _g = slot.init.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = slot.cell.get() {
        return Ok(s.clone());
    }
    if !path.is_file() {
        match remote {
            Some(r) if !r.url.is_empty() => {
                crate::fetch::download_file(&r.url, path, r.size, r.flac)
                    .with_context(|| format!("fetching {}", r.url))?;
            }
            _ => bail!("sample missing: {} (reinstall the pack)", path.display()),
        }
    }
    let s = Arc::new(decode(path)?);
    let bytes = s.bytes();
    let _ = slot.cell.set(s.clone());
    let used = CACHE.used.fetch_add(bytes, Ordering::Relaxed) + bytes;
    if used > CACHE.budget {
        evict();
    }
    Ok(s)
}

/// drop least recently used samples that nobody is playing until the cache is at 80% of its budget
fn evict() {
    let mut m = CACHE.map.lock().unwrap_or_else(|e| e.into_inner());
    let mut entries: Vec<(u64, PathBuf, usize)> = m
        .iter()
        .filter_map(|(p, s)| {
            let smp = s.cell.get()?;
            // only the cache holds it
            (Arc::strong_count(smp) == 1).then(|| (s.last.load(Ordering::Relaxed), p.clone(), smp.bytes()))
        })
        .collect();
    entries.sort_by_key(|e| e.0);
    let target = CACHE.budget / 10 * 8;
    for (_, p, b) in entries {
        if CACHE.used.load(Ordering::Relaxed) <= target {
            break;
        }
        m.remove(&p);
        CACHE.used.fetch_sub(b, Ordering::Relaxed);
    }
}

/// forget cached samples under a directory (after a pack is removed or reinstalled)
pub fn forget_dir(dir: &Path) {
    let mut m = CACHE.map.lock().unwrap_or_else(|e| e.into_inner());
    let gone: Vec<PathBuf> = m.keys().filter(|p| p.starts_with(dir)).cloned().collect();
    for p in gone {
        if let Some(s) = m.remove(&p)
            && let Some(smp) = s.cell.get()
        {
            CACHE.used.fetch_sub(smp.bytes(), Ordering::Relaxed);
        }
    }
}

/// bytes of decoded audio held in memory
pub fn cache_bytes() -> usize {
    CACHE.used.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_round_trip_trim_and_loop() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&p, spec).unwrap();
        for i in 0..44100 {
            let v = if i < 22050 { ((i as f32 * 0.05).sin() * 0.5 * 8_388_607.0) as i32 } else { 0 };
            w.write_sample(v).unwrap();
            w.write_sample(-v).unwrap();
        }
        w.finalize().unwrap();
        let s = decode(&p).unwrap();
        assert_eq!(s.sr, 44100);
        assert!(s.r.is_some());
        assert!((s.peak - 0.5).abs() < 0.01);
        // silent second half trimmed to ~20 ms after the last audible sample
        assert!(s.len() < 22050 + 1000 && s.len() > 22050, "{}", s.len());
        assert!(s.l[s.len() - 1].abs() < 1e-6);
        assert_eq!(wav_loop(&p), None);
        let s2 = load(&p, None).unwrap();
        let s3 = load(&p, None).unwrap();
        assert!(Arc::ptr_eq(&s2, &s3));
    }

    #[test]
    fn mono_is_kept_mono() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("m.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&p, spec).unwrap();
        for i in 0..4800 {
            w.write_sample(((i as f32 * 0.1).sin() * 16000.0) as i16).unwrap();
        }
        w.finalize().unwrap();
        let s = decode(&p).unwrap();
        assert!(s.r.is_none());
        assert_eq!(s.right().len(), s.l.len());
    }
}
