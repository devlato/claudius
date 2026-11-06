//! Planar stereo buffers, decoding (wav, aiff, flac, mp3, aac / m4a, alac, caf / Apple Loops, ogg), the song WAV
//! and the sample-level helpers every module shares; [`crate::export`] writes the other formats.

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{self, SR};

#[derive(Clone, Debug, Default)]
pub struct Stereo {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Stereo {
    pub fn zeros(n: usize) -> Self {
        Stereo { l: vec![0.0; n], r: vec![0.0; n] }
    }

    pub fn from_mono(m: &[f32]) -> Self {
        Stereo { l: m.to_vec(), r: m.to_vec() }
    }

    pub fn from_interleaved(x: &[f32]) -> Self {
        let n = x.len() / 2;
        let mut s = Stereo::zeros(n);
        for i in 0..n {
            s.l[i] = x[2 * i];
            s.r[i] = x[2 * i + 1];
        }
        s
    }

    pub fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.len() * 2);
        for i in 0..self.len() {
            out.push(self.l[i]);
            out.push(self.r[i]);
        }
        out
    }

    pub fn len(&self) -> usize {
        self.l.len()
    }

    pub fn is_empty(&self) -> bool {
        self.l.is_empty()
    }

    pub fn secs(&self) -> f64 {
        self.len() as f64 / config::SRF
    }

    pub fn mono(&self) -> Vec<f32> {
        self.l.iter().zip(&self.r).map(|(a, b)| 0.5 * (a + b)).collect()
    }

    pub fn ensure_len(&mut self, n: usize) {
        if self.l.len() < n {
            self.l.resize(n, 0.0);
            self.r.resize(n, 0.0);
        }
    }

    pub fn truncate(&mut self, n: usize) {
        self.l.truncate(n);
        self.r.truncate(n);
    }

    /// mix `other` into self starting at sample `pos` (may be negative), with per-channel gains; never grows self
    pub fn add_at(&mut self, other: &Stereo, pos: i64, gl: f32, gr: f32) {
        let n = self.len() as i64;
        let a = pos.max(0);
        let b = (pos + other.len() as i64).min(n);
        if b <= a {
            return;
        }
        let (a, b) = (a as usize, b as usize);
        let off = (a as i64 - pos) as usize;
        for (k, i) in (a..b).enumerate() {
            self.l[i] += other.l[off + k] * gl;
            self.r[i] += other.r[off + k] * gr;
        }
    }

    pub fn add_mono_at(&mut self, m: &[f32], pos: i64, gl: f32, gr: f32) {
        let n = self.len() as i64;
        let a = pos.max(0);
        let b = (pos + m.len() as i64).min(n);
        if b <= a {
            return;
        }
        let (a, b) = (a as usize, b as usize);
        let off = (a as i64 - pos) as usize;
        for (k, i) in (a..b).enumerate() {
            self.l[i] += m[off + k] * gl;
            self.r[i] += m[off + k] * gr;
        }
    }

    pub fn mix(&mut self, other: &Stereo, gain: f32) {
        self.add_at(other, 0, gain, gain);
    }

    pub fn scale(&mut self, g: f32) {
        self.l.iter_mut().for_each(|v| *v *= g);
        self.r.iter_mut().for_each(|v| *v *= g);
    }

    pub fn scale_lr(&mut self, gl: f32, gr: f32) {
        self.l.iter_mut().for_each(|v| *v *= gl);
        self.r.iter_mut().for_each(|v| *v *= gr);
    }

    pub fn peak(&self) -> f32 {
        self.l.iter().chain(&self.r).fold(0.0f32, |m, v| m.max(v.abs()))
    }

    pub fn rms(&self) -> f32 {
        if self.is_empty() {
            return 0.0;
        }
        let s: f64 = self.l.iter().chain(&self.r).map(|v| (*v as f64) * (*v as f64)).sum();
        (s / (2 * self.len()) as f64).sqrt() as f32
    }

    pub fn slice(&self, a: usize, b: usize) -> Stereo {
        let b = b.min(self.len());
        let a = a.min(b);
        Stereo { l: self.l[a..b].to_vec(), r: self.r[a..b].to_vec() }
    }

    pub fn reverse(&mut self) {
        self.l.reverse();
        self.r.reverse();
    }

    pub fn channels_mut(&mut self) -> [&mut Vec<f32>; 2] {
        [&mut self.l, &mut self.r]
    }

    pub fn map(&mut self, mut f: impl FnMut(&mut [f32])) {
        f(&mut self.l);
        f(&mut self.r);
    }

    /// raised-cosine fades, lengths in samples
    pub fn fade(&mut self, fade_in: usize, fade_out: usize) {
        let n = self.len();
        for ch in self.channels_mut() {
            fade_slice(ch, fade_in.min(n), fade_out.min(n));
        }
    }

    pub fn normalize(&mut self, peak_db: f32) {
        let p = self.peak();
        if p > 1e-9 {
            self.scale(db_to_gain(peak_db) / p);
        }
    }

    /// last sample index above `thresh_db` dBFS (+1), or 0 when the buffer is silent
    pub fn active_len(&self, thresh_db: f32) -> usize {
        let t = db_to_gain(thresh_db);
        (0..self.len()).rev().find(|&i| self.l[i].abs() > t || self.r[i].abs() > t).map_or(0, |i| i + 1)
    }
}

pub fn fade_slice(x: &mut [f32], fade_in: usize, fade_out: usize) {
    let n = x.len();
    if fade_in > 1 {
        for i in 0..fade_in {
            x[i] *= 0.5 - 0.5 * (std::f32::consts::PI * i as f32 / fade_in as f32).cos();
        }
    }
    if fade_out > 1 {
        for i in 0..fade_out {
            x[n - 1 - i] *= 0.5 - 0.5 * (std::f32::consts::PI * i as f32 / fade_out as f32).cos();
        }
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.abs().max(1e-12).log10()
}

/// -3 dB equal-power pan law, normalised so centre is unity on both channels
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let th = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (th.cos() * std::f32::consts::SQRT_2, th.sin() * std::f32::consts::SQRT_2)
}

/// balance for stereo sources: each side only ever attenuated
pub fn balance_gains(pan: f32) -> (f32, f32) {
    let (l, r) = pan_gains(pan);
    (l.min(1.0), r.min(1.0))
}

pub fn secs_to_samples(s: f64) -> usize {
    (s * config::SRF).round().max(0.0) as usize
}

fn cache_key(path: &Path) -> Result<String> {
    let md = std::fs::metadata(path).with_context(|| format!("no such file: {}", path.display()))?;
    let mtime =
        md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(blake3::hash(format!("{}|{}|{}|{}", abs.display(), md.len(), mtime, SR).as_bytes()).to_hex()[..20].to_string())
}

/// a file decoded (claudius-decode) and resampled to 48 kHz stereo, interleaved
fn decode_file(path: &Path) -> Result<Vec<f32>> {
    decoded(|| claudius_decode::decode(path))
}

/// what `decode` decoded, at the engine's rate and interleaved, timed as a decoding (`metrics`)
fn decoded(decode: impl FnOnce() -> Result<claudius_decode::Decoded>) -> Result<Vec<f32>> {
    let secs = |x: &Result<Vec<f32>>| x.as_ref().map_or(0.0, |x| x.len() as f64 / 2.0 / config::SRF);
    crate::metrics::timed(crate::metrics::Metric::Decode, secs, || decode().map(at_rate))
}

/// decoded audio resampled to 48 kHz stereo, interleaved
fn at_rate(d: claudius_decode::Decoded) -> Vec<f32> {
    let (mut l, mut r) = d.stereo();
    if d.rate != SR && d.rate > 0 {
        let step = d.rate as f64 / SR as f64;
        let out_len = (l.len() as f64 / step).round() as usize;
        l = resample_rate(&l, d.rate, SR, out_len);
        r = resample_rate(&r, d.rate, SR, out_len);
    }
    l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect()
}

/// decode any audio file to 48 kHz stereo without keeping a decoded copy: for files read once, like a render's WAV
pub fn load_once(path: &Path) -> Result<Stereo> {
    Ok(Stereo::from_interleaved(&decode_file(path)?))
}

/// decode audio in memory (a file's bytes, `ext` its kind: wav, flac, ...) to 48 kHz stereo
pub fn load_bytes(bytes: Vec<u8>, ext: &str) -> Result<Stereo> {
    Ok(Stereo::from_interleaved(&decoded(|| claudius_decode::decode_bytes(bytes, ext))?))
}

type Remembered = std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Stereo>>>;

fn remembered() -> &'static Remembered {
    static R: std::sync::OnceLock<Remembered> = std::sync::OnceLock::new();
    R.get_or_init(Default::default)
}

/// Keeps `x` (48 kHz stereo) as the audio at `path`, which `load` then hands out without a file: what a browser's page
/// fetched for the engine to play, where there are no files to read.
pub fn remember(path: &str, x: Stereo) {
    remembered().lock().unwrap_or_else(|e| e.into_inner()).insert(path.to_string(), std::sync::Arc::new(x));
    let mut v = versions().lock().unwrap_or_else(|e| e.into_inner());
    let next = v.values().max().copied().unwrap_or(0) + 1;
    v.insert(path.to_string(), next);
}

fn versions() -> &'static std::sync::Mutex<std::collections::HashMap<String, u64>> {
    static V: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> =
        std::sync::OnceLock::new();
    V.get_or_init(Default::default)
}

/// the audio `remember` keeps at `path`, if any
pub fn remembered_audio(path: &str) -> Option<std::sync::Arc<Stereo>> {
    remembered().lock().unwrap_or_else(|e| e.into_inner()).get(path).cloned()
}

/// which of the audio `remember` has kept at `path` this is: a number that changes each time the path is given audio
/// anew (none: nothing kept there)
pub fn remembered_version(path: &str) -> Option<u64> {
    versions().lock().unwrap_or_else(|e| e.into_inner()).get(path).copied()
}

/// decode any audio file to 48 kHz stereo (cached as raw f32 under ~/.cache/claudius/decoded), or hand out what
/// `remember` keeps for its path
pub fn load(path: &Path) -> Result<Stereo> {
    if let Some(x) = remembered().lock().unwrap_or_else(|e| e.into_inner()).get(&*path.to_string_lossy()) {
        return Ok((**x).clone());
    }
    let key = cache_key(path)?;
    let cached = config::cache_dir("decoded").join(format!("{key}.f32"));
    if let Ok(bytes) = std::fs::read(&cached) {
        let x: Vec<f32> = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        return Ok(Stereo::from_interleaved(&x));
    }
    let x = decode_file(path)?;
    if config::FILES {
        let tmp = cached.with_extension(format!("{}.tmp", config::process_id()));
        let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &cached);
        }
    }
    Ok(Stereo::from_interleaved(&x))
}

/// 16-bit PCM WAV in memory, for a player that fetches a sound (a phone's pads)
pub fn wav_bytes(x: &Stereo) -> Vec<u8> {
    let spec =
        hound::WavSpec { channels: 2, sample_rate: SR, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut out = std::io::Cursor::new(Vec::with_capacity(44 + x.len() * 4));
    {
        let mut w = hound::WavWriter::new(&mut out, spec).expect("a WAV header in memory");
        for i in 0..x.len() {
            for v in [x.l[i], x.r[i]] {
                w.write_sample((v.clamp(-1.0, 1.0) * 32_767.0).round() as i16).expect("memory takes the samples");
            }
        }
        w.finalize().expect("a WAV in memory");
    }
    out.into_inner()
}

/// 24-bit PCM WAV (or 32-bit float with `float = true`)
pub fn save_wav(path: &Path, x: &Stereo, float: bool) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: if float { 32 } else { 24 },
        sample_format: if float { hound::SampleFormat::Float } else { hound::SampleFormat::Int },
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for i in 0..x.len() {
        for v in [x.l[i], x.r[i]] {
            if float {
                w.write_sample(v)?;
            } else {
                w.write_sample((v.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)?;
            }
        }
    }
    w.finalize()?;
    Ok(())
}

const SINC_HALF: i64 = 16;

/// the weight of the input sample `d` samples before the position read, for a filter cutting at `fc` (of the input's
/// Nyquist) with `half` taps a side: a sinc under a Hann window
fn sinc_weight(d: f64, fc: f64, half: i64) -> f64 {
    let t = d * fc;
    let sinc = if t.abs() < 1e-9 { 1.0 } else { (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t) };
    let wpos = d / half as f64;
    let win = if wpos.abs() >= 1.0 { 0.0 } else { 0.5 + 0.5 * (std::f64::consts::PI * wpos).cos() };
    sinc * win
}

/// read `x` at fractional positions start + i * step (windowed-sinc, anti-aliased when step > 1): the core of
/// sampler pitch shifting and varispeed
pub fn resample_step(x: &[f32], step: f64, out_len: usize) -> Vec<f32> {
    let n = x.len() as i64;
    let fc = (1.0 / step).min(1.0);
    let half = (SINC_HALF as f64 / fc).ceil() as i64;
    let mut out = vec![0.0f32; out_len];
    for (i, o) in out.iter_mut().enumerate() {
        let p = i as f64 * step;
        let base = p.floor() as i64;
        let mut acc = 0.0f64;
        let mut wsum = 0.0f64;
        for k in (base - half + 1)..=(base + half) {
            if k < 0 || k >= n {
                continue;
            }
            let w = sinc_weight(p - k as f64, fc, half);
            acc += x[k as usize] as f64 * w;
            wsum += w;
        }
        *o = if wsum.abs() > 1e-9 { (acc / wsum) as f32 } else { 0.0 };
    }
    out
}

/// the most positions between input samples `resample_rate` works out the weights of before it reads with
/// `resample_step` instead
const MOST_PHASES: i64 = 4096;

/// `x`, recorded at `from` Hz, at `to` Hz: `resample_step`'s filter, with its weights worked out once for each of the
/// positions between input samples that the conversion comes back to (`to / gcd(from, to)` of them, 160 from 44.1 to
/// 48 kHz), so that each sample out is a sum of its taps, and with each position counted in whole numbers, exactly
pub fn resample_rate(x: &[f32], from: u32, to: u32, out_len: usize) -> Vec<f32> {
    let g = gcd(from as u64, to as u64).max(1);
    let (num, den) = ((from as u64 / g) as i64, (to as u64 / g) as i64);
    if den == 0 || den > MOST_PHASES {
        return resample_step(x, from as f64 / to as f64, out_len);
    }
    let fc = (den as f64 / num as f64).min(1.0);
    let half = (SINC_HALF as f64 / fc).ceil() as i64;
    let taps = 2 * half as usize;
    // the weights of the taps from `half - 1` samples before a position to `half` after it, for each phase r / den
    let mut weights = vec![0.0f64; den as usize * taps];
    let mut sums = vec![0.0f64; den as usize];
    for r in 0..den as usize {
        let frac = r as f64 / den as f64;
        for j in 0..taps {
            let w = sinc_weight(frac + (half - 1 - j as i64) as f64, fc, half);
            weights[r * taps + j] = w;
            sums[r] += w;
        }
    }
    let n = x.len() as i64;
    let mut out = vec![0.0f32; out_len];
    for (i, o) in out.iter_mut().enumerate() {
        let q = i as i64 * num;
        let (base, r) = ((q / den), (q % den) as usize);
        let first = base - half + 1;
        let w = &weights[r * taps..(r + 1) * taps];
        let (acc, wsum) = if first >= 0 && base + half < n {
            let taps = &x[first as usize..first as usize + taps];
            (taps.iter().zip(w).map(|(v, w)| *v as f64 * w).sum::<f64>(), sums[r])
        } else {
            (0..taps)
                .filter(|&j| (0..n).contains(&(first + j as i64)))
                .fold((0.0, 0.0), |(a, s), j| (a + x[(first + j as i64) as usize] as f64 * w[j], s + w[j]))
        };
        *o = if wsum.abs() > 1e-9 { (acc / wsum) as f32 } else { 0.0 };
    }
    out
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// play `x` `semis` semitones higher / lower (shorter / longer, like a classic sampler)
pub fn repitch(x: &Stereo, semis: f64) -> Stereo {
    if semis.abs() < 1e-6 {
        return x.clone();
    }
    let step = 2f64.powf(semis / 12.0);
    let out_len = ((x.len() as f64) / step).floor() as usize;
    Stereo { l: resample_step(&x.l, step, out_len), r: resample_step(&x.r, step, out_len) }
}

const AUDIO_EXTS: &[&str] = &["wav", "aif", "aiff", "flac", "mp3", "m4a", "aac", "caf", "ogg", "opus"];

/// audio files among `paths` (folders are searched recursively), sorted
pub fn list_audio(paths: &[String]) -> Vec<String> {
    fn walk(p: &Path, out: &mut Vec<String>) {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
                entries.sort();
                for e in entries {
                    walk(&e, out);
                }
            }
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    let mut out = Vec::new();
    for p in paths {
        walk(Path::new(p), &mut out);
    }
    out
}

pub fn midi_to_hz(p: f64) -> f64 {
    440.0 * 2f64.powf((p - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// two seconds at `rate` of a chord, a sweep up to near its Nyquist, and noise
    fn recording(rate: u32) -> Vec<f32> {
        let mut rng = crate::rng::Rng::new(5);
        let sr = rate as f64;
        (0..2 * rate as usize)
            .map(|i| {
                let t = i as f64 / sr;
                let chord: f64 = [220.0, 277.18, 329.63].iter().map(|f| (std::f64::consts::TAU * f * t).sin()).sum();
                let sweep = (std::f64::consts::TAU * (200.0 + 0.45 * sr * t / 2.0) * t).sin();
                (0.2 * chord + 0.2 * sweep) as f32 + 0.05 * (rng.f32() - 0.5)
            })
            .collect()
    }

    #[test]
    fn a_rate_converts_as_the_resampler_reads_it_to_within_a_rounding() {
        for from in [44_100, 32_000, 96_000, 22_050] {
            let x = recording(from);
            let out_len = (x.len() as f64 * SR as f64 / from as f64).round() as usize;
            let read = resample_step(&x, from as f64 / SR as f64, out_len);
            let converted = resample_rate(&x, from, SR, out_len);
            let worst = read.iter().zip(&converted).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            eprintln!("{from} Hz: within {worst:e} of reading it a step at a time");
            assert!(worst < 1e-6, "{from} Hz: {worst}");
        }
        // a rate with no few positions between samples to come back to is read as before
        let x = recording(44_057);
        let out_len = (x.len() as f64 * SR as f64 / 44_057.0).round() as usize;
        assert_eq!(resample_rate(&x, 44_057, SR, out_len), resample_step(&x, 44_057.0 / SR as f64, out_len));
    }
}
