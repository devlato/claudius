//! Audio files to samples. symphonia reads WAV, AIFF, FLAC, MP3, AAC / M4A, ALAC, CAF (Apple Loops) and Ogg Vorbis
//! in pure Rust, so claudius needs no other program to read sound. When a file uses something symphonia cannot
//! read and ffmpeg is installed, ffmpeg decodes it instead.

#![forbid(unsafe_code)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as Sym;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// decoded audio at the file's own sample rate, frames interleaved
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    pub rate: u32,
    pub channels: usize,
    pub samples: Vec<f32>,
}

impl Decoded {
    pub fn frames(&self) -> usize {
        self.samples.len().checked_div(self.channels).unwrap_or(0)
    }

    /// the first two channels, deinterleaved (a mono file plays on both)
    pub fn stereo(&self) -> (Vec<f32>, Vec<f32>) {
        match self.channels {
            0 => (Vec::new(), Vec::new()),
            1 => (self.samples.clone(), self.samples.clone()),
            c => {
                let l = self.samples.chunks_exact(c).map(|f| f[0]).collect();
                let r = self.samples.chunks_exact(c).map(|f| f[1]).collect();
                (l, r)
            }
        }
    }
}

/// decode a file; symphonia first, ffmpeg when symphonia cannot and ffmpeg is installed
pub fn decode(path: &Path) -> Result<Decoded> {
    if !path.is_file() {
        bail!("no such file: {}", path.display());
    }
    let native = decode_native(path);
    if let Ok(d) = &native
        && d.frames() > 0
    {
        return native;
    }
    let reason = match &native {
        Ok(_) => "it holds no audio".to_string(),
        Err(e) => e.to_string(),
    };
    match ffmpeg() {
        Some(ff) => decode_ffmpeg(&ff, path)
            .with_context(|| format!("cannot decode {} ({reason}; ffmpeg failed too)", path.display())),
        None => Err(anyhow!("cannot decode {}: {reason} (installing ffmpeg adds more formats)", path.display())),
    }
}

/// decode with symphonia only
pub fn decode_native(path: &Path) -> Result<Decoded> {
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format =
        symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| anyhow!("no audio track"))?;
    let track_id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else {
        bail!("the audio track has no codec parameters")
    };
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(&params, &AudioDecoderOptions::default())?;
    let mut out = Decoded {
        rate: params.sample_rate.unwrap_or(0),
        channels: params.channels.as_ref().map_or(0, |c| c.count()),
        samples: Vec::new(),
    };
    let mut packet_samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) | Err(Sym::ResetRequired) => break,
            Err(Sym::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                if out.rate == 0 {
                    out.rate = audio.spec().rate();
                }
                if out.channels == 0 {
                    out.channels = audio.spec().channels().count();
                }
                audio.copy_to_vec_interleaved(&mut packet_samples);
                out.samples.extend_from_slice(&packet_samples);
            }
            // a damaged packet: skip it, as players do
            Err(Sym::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    if out.rate == 0 || out.channels == 0 {
        bail!("cannot tell the sample rate or channel count");
    }
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("caf"))
        && let Some((priming, valid)) = caf_valid_frames(path)
    {
        // AAC in CAF starts with encoder priming and ends with padding; a loop must be exactly its bars
        let (a, b) = (priming * out.channels, (priming + valid) * out.channels);
        if b <= out.samples.len() {
            out.samples = out.samples[a..b].to_vec();
        }
    }
    Ok(out)
}

/// (priming frames, valid frames) from a CAF file's packet table ('pakt' chunk), when it has one
pub fn caf_valid_frames(path: &Path) -> Option<(usize, usize)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.get(..4)? != b"caff" {
        return None;
    }
    let mut at = 8;
    while at + 12 <= bytes.len() {
        let kind = &bytes[at..at + 4];
        let size = i64::from_be_bytes(bytes[at + 4..at + 12].try_into().ok()?);
        let body = at + 12;
        if kind == b"pakt" {
            let valid = i64::from_be_bytes(bytes.get(body + 8..body + 16)?.try_into().ok()?);
            let priming = i32::from_be_bytes(bytes.get(body + 16..body + 20)?.try_into().ok()?);
            return (valid > 0 && priming >= 0).then_some((priming as usize, valid as usize));
        }
        if size < 0 {
            return None;
        }
        at = body.checked_add(size as usize)?;
    }
    None
}

/// the ffmpeg program: $CLAUDIUS_FFMPEG (`none` turns ffmpeg off), PATH, then the usual install places (GUI apps on
/// macOS miss the shell PATH)
pub fn ffmpeg() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CLAUDIUS_FFMPEG").filter(|p| !p.is_empty()) {
        return (p != "none").then(|| PathBuf::from(p)).filter(|p| p.is_file());
    }
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let path_dirs = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    let usual = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from);
    path_dirs.into_iter().chain(usual).map(|d| d.join(name)).find(|p| p.is_file())
}

fn decode_ffmpeg(ffmpeg: &Path, path: &Path) -> Result<Decoded> {
    const RATE: u32 = 48_000;
    let out = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ac", "2", "-ar", &RATE.to_string(), "-"])
        .output()
        .with_context(|| format!("cannot run {}", ffmpeg.display()))?;
    if !out.status.success() || out.stdout.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{}", err.lines().last().unwrap_or("no audio stream").trim());
    }
    let samples = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    Ok(Decoded { rate: RATE, channels: 2, samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path, rate: u32, channels: u16, frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            for c in 0..channels {
                let x = ((i as f64 * 0.05 + c as f64).sin() * 0.5 * 8_388_607.0) as i32;
                w.write_sample(x).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn reads_wav_at_its_own_rate() {
        let dir = std::env::temp_dir().join(format!("claudius-decode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.wav");
        write_wav(&p, 44_100, 2, 4410);
        let d = decode_native(&p).unwrap();
        assert_eq!((d.rate, d.channels, d.frames()), (44_100, 2, 4410));
        let (l, r) = d.stereo();
        assert!((l[20] - ((20.0f64 * 0.05).sin() * 0.5) as f32).abs() < 1e-5);
        assert!((r[20] - ((20.0f64 * 0.05 + 1.0).sin() * 0.5) as f32).abs() < 1e-5);
        write_wav(&p, 22_050, 1, 100);
        let d = decode(&p).unwrap();
        assert_eq!((d.rate, d.channels, d.frames()), (22_050, 1, 100));
        assert_eq!(d.stereo().0, d.stereo().1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn explains_what_it_cannot_read() {
        let dir = std::env::temp_dir().join(format!("claudius-decode-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("not-audio.mp3");
        std::fs::write(&p, b"this is text, not sound").unwrap();
        let e = decode(&p).unwrap_err().to_string();
        assert!(e.contains("cannot decode"), "{e}");
        assert!(decode(&dir.join("missing.wav")).unwrap_err().to_string().contains("no such file"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
