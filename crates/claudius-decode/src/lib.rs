//! Audio files to samples. symphonia reads WAV, AIFF, FLAC, MP3, AAC / M4A, ALAC, CAF (Apple Loops) and Ogg Vorbis
//! in pure Rust, so claudius needs no other program to read sound. When a file uses something symphonia cannot
//! read and ffmpeg is installed, ffmpeg decodes it instead, reading that one file and nothing it refers to
//! (`ffmpeg_input`).

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as Sym;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// the most samples a decode keeps: 30 minutes of stereo at 48 kHz (690 MB as f32). A longer file is refused rather
/// than allowed to fill the memory.
pub const MAX_SAMPLES: usize = MAX_SECONDS as usize * 48_000 * 2;
pub const MAX_SECONDS: u32 = 30 * 60;

/// the error for a file with more audio than `MAX_SAMPLES`
#[derive(Debug)]
pub struct TooLong(pub PathBuf);

impl std::fmt::Display for TooLong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is too long: claudius decodes up to 30 minutes of stereo at 48 kHz", self.0.display())
    }
}

impl std::error::Error for TooLong {}

/// The demuxers ffmpeg may read input with: files that hold their own audio. Playlists (hls), lists of files
/// (concat) and image sequences are left out, because they open other files, which may lie outside the folders a
/// shared claudius keeps to.
const DEMUXERS: &str = "aac,ac3,aiff,amr,ape,asf,au,avi,caf,dsf,dts,eac3,flac,flv,matroska,mov,mp3,mpc,mpc8,mpeg,mpegts,ogg,shorten,tak,\
     tta,w64,wav,wv";

/// the arguments that make `path` the input of ffmpeg or ffprobe: a local file, read by one of `DEMUXERS`, and no
/// other file or URL it names
pub fn ffmpeg_input(path: &Path) -> Vec<OsString> {
    let mut file = OsString::from("file:");
    file.push(path);
    ["-protocol_whitelist", "file", "-format_whitelist", DEMUXERS, "-i"]
        .map(OsString::from)
        .into_iter()
        .chain([file])
        .collect()
}

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
    match &native {
        Ok(d) if d.frames() > 0 => return native,
        Err(e) if e.is::<TooLong>() => return native,
        _ => {}
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
    decode_native_upto(path, MAX_SAMPLES)
}

fn decode_native_upto(path: &Path, max_samples: usize) -> Result<Decoded> {
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
                if out.samples.len() + packet_samples.len() > max_samples {
                    return Err(TooLong(path.to_path_buf()).into());
                }
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
        if let Some(b) = priming.checked_add(valid).and_then(|frames| frames.checked_mul(out.channels))
            && b <= out.samples.len()
        {
            out.samples = out.samples[priming * out.channels..b].to_vec();
        }
    }
    Ok(out)
}

/// (priming frames, valid frames) from a CAF file's packet table ('pakt' chunk), when it has one; it reads the chunk
/// headers and skips their bodies
pub fn caf_valid_frames(path: &Path) -> Option<(usize, usize)> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 8];
    file.read_exact(&mut header).ok()?;
    if &header[..4] != b"caff" {
        return None;
    }
    let mut chunk = [0u8; 12];
    while file.read_exact(&mut chunk).is_ok() {
        let size = i64::from_be_bytes(chunk[4..].try_into().ok()?);
        if &chunk[..4] == b"pakt" {
            let mut table = [0u8; 20];
            file.read_exact(&mut table).ok()?;
            let valid = i64::from_be_bytes(table[8..16].try_into().ok()?);
            let priming = i32::from_be_bytes(table[16..].try_into().ok()?);
            return (valid > 0 && priming >= 0).then_some((priming as usize, usize::try_from(valid).ok()?));
        }
        if size < 0 {
            return None;
        }
        file.seek(SeekFrom::Current(size)).ok()?;
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
    let mut child = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin"])
        .args(ffmpeg_input(path))
        // a second past the limit, so that a longer file is told from one exactly as long
        .args(["-vn", "-t", &(MAX_SECONDS + 1).to_string(), "-f", "f32le", "-acodec", "pcm_f32le", "-ac", "2"])
        .args(["-ar", &RATE.to_string(), "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot run {}", ffmpeg.display()))?;
    let mut stderr = child.stderr.take().expect("stderr is piped");
    // read beside stdout, so that ffmpeg never waits on a full pipe; the last line says what went wrong
    let errors = std::thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stderr.by_ref().take(1 << 16).read_to_end(&mut text);
        let _ = std::io::copy(&mut stderr, &mut std::io::sink());
        String::from_utf8_lossy(&text).into_owned()
    });
    let mut out = std::io::BufReader::with_capacity(1 << 16, child.stdout.take().expect("stdout is piped"));
    let mut samples = Vec::new();
    let mut bytes = [0u8; 4];
    while out.read_exact(&mut bytes).is_ok() {
        samples.push(f32::from_le_bytes(bytes));
    }
    let status = child.wait()?;
    let errors = errors.join().unwrap_or_default();
    if !status.success() || samples.is_empty() {
        bail!("{}", errors.lines().last().unwrap_or("no audio stream").trim());
    }
    if samples.len() > MAX_SAMPLES {
        return Err(TooLong(path.to_path_buf()).into());
    }
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

    #[test]
    fn refuses_more_audio_than_it_keeps() {
        let dir = std::env::temp_dir().join(format!("claudius-decode-long-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.wav");
        write_wav(&p, 44_100, 2, 4410);
        let e = decode_native_upto(&p, 1000).unwrap_err();
        assert!(e.is::<TooLong>() && e.to_string().contains("too long"), "{e}");
        assert_eq!(decode_native_upto(&p, 8820).unwrap().frames(), 4410);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reads_the_packet_table_of_a_caf() {
        let dir = std::env::temp_dir().join(format!("claudius-decode-caf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.caf");
        let mut b = b"caff\x00\x01\x00\x00".to_vec();
        b.extend_from_slice(b"desc");
        b.extend_from_slice(&32i64.to_be_bytes());
        b.extend_from_slice(&[0; 32]);
        // packets, valid frames, priming frames, remainder frames
        b.extend_from_slice(b"pakt");
        b.extend_from_slice(&24i64.to_be_bytes());
        b.extend_from_slice(&10i64.to_be_bytes());
        b.extend_from_slice(&44_100i64.to_be_bytes());
        b.extend_from_slice(&2112i32.to_be_bytes());
        b.extend_from_slice(&0i32.to_be_bytes());
        std::fs::write(&p, &b).unwrap();
        assert_eq!(caf_valid_frames(&p), Some((2112, 44_100)));
        // a data chunk of unknown size (-1) ends the search
        std::fs::write(&p, b"caff\x00\x01\x00\x00data\xff\xff\xff\xff\xff\xff\xff\xff").unwrap();
        assert_eq!(caf_valid_frames(&p), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ffmpeg_opens_no_file_a_playlist_names() {
        let Some(ff) = ffmpeg() else { return };
        let dir = std::env::temp_dir().join(format!("claudius-decode-playlist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("elsewhere.wav");
        write_wav(&wav, 48_000, 2, 48_000);
        let ts = dir.join("elsewhere.ts");
        let made = Command::new(&ff)
            .args(["-v", "error", "-nostdin", "-y", "-i"])
            .arg(&wav)
            .args(["-c:a", "aac", "-f", "mpegts"])
            .arg(&ts)
            .status();
        if !made.is_ok_and(|s| s.success()) {
            return;
        }
        assert!(decode_ffmpeg(&ff, &ts).is_ok_and(|d| d.frames() > 40_000), "the segment itself decodes");
        let playlist = dir.join("list.m3u8");
        let list = format!("#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:1.0,\n{}\n#EXT-X-ENDLIST\n", ts.display());
        std::fs::write(&playlist, list).unwrap();
        let e = decode(&playlist).unwrap_err().to_string();
        assert!(e.contains("cannot decode"), "{e}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
