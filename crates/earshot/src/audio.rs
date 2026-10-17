//! Audio I/O (earshot/audio.py): any file ffmpeg can read, decoded to f32 at 48 kHz.
//!
//! Levels: the stereo decode keeps true sample levels (a mono source is duplicated to both channels at unity gain).
//! `load_mono` is ffmpeg's `-ac 1` downmix, which sums a stereo source at -3 dB per channel
//! (out = 0.70710677 * (L + R), measured for wav / flac / mp3 / aac at every bit depth): the models' input
//! convention. `downmix` reproduces it for in-memory stereo arrays.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::SR;

/// ffmpeg's stereo -> mono coefficient (1 / sqrt 2, as f32)
pub const DOWNMIX_GAIN: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn find_tool(name: &str, env: &str) -> PathBuf {
    if let Ok(p) = std::env::var(env)
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&paths) {
            let c = d.join(name);
            if c.is_file() {
                return c;
            }
        }
    }
    // GUI apps on macOS do not inherit the shell PATH
    for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let c = Path::new(d).join(name);
        if c.is_file() {
            return c;
        }
    }
    PathBuf::from(name)
}

/// the ffmpeg binary: $EARSHOT_FFMPEG, else PATH, else /opt/homebrew/bin, /usr/local/bin
pub fn ffmpeg() -> PathBuf {
    find_tool("ffmpeg", "EARSHOT_FFMPEG")
}

/// the ffprobe binary: $EARSHOT_FFPROBE, else PATH, else /opt/homebrew/bin, /usr/local/bin
pub fn ffprobe() -> PathBuf {
    find_tool("ffprobe", "EARSHOT_FFPROBE")
}

fn ffmpeg_reason(err: &str) -> String {
    let lines: Vec<String> = err
        .lines()
        .map(|l| {
            let l = l.trim();
            // drop a leading "[component @ 0x...]" prefix
            if l.starts_with('[')
                && let Some(i) = l.find(']')
                && l[..i].contains(" @ ")
            {
                return l[i + 1..].trim().to_string();
            }
            l.to_string()
        })
        .filter(|l| {
            let lo = l.to_lowercase();
            !l.is_empty() && !lo.starts_with("error opening output") && !lo.starts_with("conversion failed")
        })
        .collect();
    let s = lines.last().cloned().unwrap_or_else(|| "ffmpeg failed".into());
    s.chars().take(200).collect()
}

/// number of channels of the first audio stream (0 when unknown)
pub fn src_channels(path: &Path) -> usize {
    Command::new(ffprobe())
        .args(["-v", "error", "-select_streams", "a:0", "-show_entries", "stream=channels", "-of", "csv=p=0"])
        .arg(path)
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().and_then(|l| l.trim().parse().ok()))
        .unwrap_or(0)
}

/// 48 kHz samples with `ch` channels (1 = ffmpeg's -ac 1 downmix): ffmpeg when it is installed (matching earshot's
/// Python reference to the sample), else claudius-decode plus earshot's own resampler
fn decode(path: &Path, ch: usize) -> Result<Vec<f32>> {
    if !path.exists() {
        bail!("no such file: {}", path.display());
    }
    if !ffmpeg().is_file() {
        return decode_without_ffmpeg(path, ch);
    }
    let out = Command::new(ffmpeg())
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ac", &ch.to_string(), "-ar", &SR.to_string(), "-"])
        .output()
        .with_context(|| format!("cannot run ffmpeg ({}): install ffmpeg or set EARSHOT_FFMPEG", ffmpeg().display()))?;
    let err = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() || out.stdout.is_empty() {
        if out.status.success() || err.contains("does not contain any stream") {
            bail!("{}: no audio stream", path.display());
        }
        bail!("cannot decode {}: {}", path.display(), ffmpeg_reason(&err));
    }
    Ok(out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

fn decode_without_ffmpeg(path: &Path, ch: usize) -> Result<Vec<f32>> {
    let d = claudius_decode::decode(path)?;
    let (l, r) = d.stereo();
    let (l, r) = if d.rate == SR {
        (l, r)
    } else {
        (crate::dsp::resample(&l, d.rate, SR), crate::dsp::resample(&r, d.rate, SR))
    };
    Ok(match ch {
        1 if d.channels == 1 => l,
        1 => downmix(&l, &r),
        _ => l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect(),
    })
}

/// stereo decode (L, R) at 48 kHz; a mono source is duplicated at unity gain
pub fn load_stereo(path: &Path) -> Result<(Vec<f32>, Vec<f32>)> {
    if !path.exists() {
        bail!("no such file: {}", path.display());
    }
    if !ffmpeg().is_file() {
        let x = decode_without_ffmpeg(path, 2)?;
        return Ok((x.iter().step_by(2).copied().collect(), x.iter().skip(1).step_by(2).copied().collect()));
    }
    if src_channels(path) == 1 {
        let m = decode(path, 1)?;
        return Ok((m.clone(), m));
    }
    let x = decode(path, 2)?;
    let n = x.len() / 2;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for f in x.chunks_exact(2) {
        l.push(f[0]);
        r.push(f[1]);
    }
    Ok((l, r))
}

/// mono decode at 48 kHz: ffmpeg's -ac 1 downmix (the models' input convention)
pub fn load_mono(path: &Path) -> Result<Vec<f32>> {
    decode(path, 1)
}

/// ffmpeg's stereo -> mono downmix of in-memory arrays: 0.70710677 * (L + R)
pub fn downmix(l: &[f32], r: &[f32]) -> Vec<f32> {
    l.iter().zip(r).map(|(&a, &b)| a * DOWNMIX_GAIN + b * DOWNMIX_GAIN).collect()
}

/// mean of the channels (numpy x.mean(axis=1), in f32): earshot's to_mono of a stereo array
pub fn mean_mono(l: &[f32], r: &[f32]) -> Vec<f32> {
    l.iter().zip(r).map(|(&a, &b)| (a + b) / 2.0).collect()
}

/// [start, end) seconds -> sample range, as audio.load does (round, clipped)
pub fn range(n: usize, start: Option<f64>, end: Option<f64>) -> (usize, usize) {
    let a = ((start.unwrap_or(0.0) * SR as f64).round().max(0.0) as usize).min(n);
    let b = match end {
        None => n,
        Some(e) => ((e * SR as f64).round().max(0.0) as usize).min(n),
    };
    (a, b.max(a))
}

/// [(t0 seconds, start sample, end sample)] windows of `win` seconds every `hop` seconds (audio.windows)
pub fn windows(n: usize, win: f64, hop: f64, pad_last: bool) -> Vec<(f64, usize, usize)> {
    let w = (win * SR as f64).round() as usize;
    let h = (hop * SR as f64).round() as usize;
    if n <= w {
        return vec![(0.0, 0, n)];
    }
    let mut out = Vec::new();
    let mut s = 0;
    while s + w <= n {
        out.push((s as f64 / SR as f64, s, s + w));
        s += h;
    }
    if pad_last && !(n - w).is_multiple_of(h) {
        out.push(((n - w) as f64 / SR as f64, n - w, n));
    }
    out
}

/// sample peak in dBFS (audio.db of max |x|)
pub fn peak_db(x: &[f32]) -> f64 {
    let p = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    crate::dsp::db(p as f64)
}

/// duration in seconds of the mono decode
pub fn duration(path: &Path) -> Result<f64> {
    Ok(load_mono(path)?.len() as f64 / SR as f64)
}

/// expand files / folders into a sorted list of media files (audio.list_media)
pub fn list_media(paths: &[PathBuf]) -> Vec<PathBuf> {
    const EXTS: &[&str] = &[
        "wav", "flac", "ogg", "oga", "opus", "mp3", "m4a", "aac", "aif", "aiff", "wma", "caf", "mp4", "mov", "m4v",
        "webm", "mkv", "avi",
    ];
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let mut ents: Vec<PathBuf> = match std::fs::read_dir(d) {
            Ok(r) => r.filter_map(|e| e.ok().map(|e| e.path())).collect(),
            Err(_) => return,
        };
        ents.sort();
        let (dirs, files): (Vec<PathBuf>, Vec<PathBuf>) = ents.into_iter().partition(|p| p.is_dir());
        for f in files {
            let ok = f.extension().and_then(|e| e.to_str()).map(|e| EXTS.contains(&e.to_lowercase().as_str()));
            if ok == Some(true) {
                out.push(f);
            }
        }
        for sub in dirs {
            walk(&sub, out);
        }
    }
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, &mut out);
        } else if p.is_file() {
            out.push(p.clone());
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    out
}
