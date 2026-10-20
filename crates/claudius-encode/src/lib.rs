//! Audio files from samples. WAV, AIFF, FLAC, MP3 and Opus are written by Rust code in this process, and Ogg Vorbis by
//! libvorbis (aoTuV), built in. AAC and Apple Lossless come from the system's encoder: Core Audio's `afconvert` on
//! macOS, ffmpeg elsewhere when it is installed. A [`Spec`] names a format and its quality: `mp3:320`, `flac:16`.

#![forbid(unsafe_code)]

pub mod flac;
mod mp3;
mod opus;
mod pcm;
mod resample;
mod system;
mod vorbis;

use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail};

pub use resample::convert as resample;

/// interleaved samples, nominally -1..1
#[derive(Clone, Copy, Debug)]
pub struct Pcm<'a> {
    pub samples: &'a [f32],
    pub channels: usize,
    pub rate: u32,
}

/// what a file says about itself; empty fields are left out
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// a year, or a date as YYYY-MM-DD
    pub date: String,
    pub comment: String,
    /// the program that made the file
    pub software: String,
}

impl Tags {
    /// (Vorbis comment name, value) for every field that is set
    fn fields(&self) -> Vec<(&'static str, &str)> {
        [
            ("TITLE", &self.title),
            ("ARTIST", &self.artist),
            ("ALBUM", &self.album),
            ("DATE", &self.date),
            ("COMMENT", &self.comment),
            ("ENCODER", &self.software),
        ]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (k, v.as_str()))
        .collect()
    }
}

/// how deep a lossless sample is
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    I16,
    I24,
    F32,
}

impl Depth {
    pub fn bits(self) -> u16 {
        match self {
            Depth::I16 => 16,
            Depth::I24 => 24,
            Depth::F32 => 32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Wav(Depth),
    Aiff(Depth),
    Flac(Depth),
    /// Apple Lossless in an .m4a
    Alac(Depth),
    /// constant bitrate, kbps
    Mp3(u32),
    /// variable bitrate at LAME's quality scale: V0 (best) to V9
    Mp3Vbr(u8),
    /// kbps, in an .m4a
    Aac(u32),
    /// kbps, in an .opus (Ogg)
    Opus(u32),
    /// Vorbis quality 0 to 10, in an .ogg
    Vorbis(u8),
}

/// MPEG-1 Layer III bitrates
const MP3_KBPS: [u32; 14] = [32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const RATES: [u32; 2] = [44_100, 48_000];

/// a format and its settings: what a file is to be
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub format: Format,
    /// the file's sample rate; `None` takes the format's own (see [`Spec::rate_for`])
    pub rate: Option<u32>,
}

impl Spec {
    /// The file's sample rate for audio at `source`: 44.1 kHz for 16-bit lossless files (CD audio), 48 kHz for Opus,
    /// which always runs at it, and the source's own otherwise.
    pub fn rate_for(&self, source: u32) -> u32 {
        if let Some(r) = self.rate {
            return r;
        }
        match self.format {
            Format::Opus(_) => 48_000,
            Format::Wav(Depth::I16)
            | Format::Aiff(Depth::I16)
            | Format::Flac(Depth::I16)
            | Format::Alac(Depth::I16) => 44_100,
            _ => source,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self.format {
            Format::Wav(_) => "wav",
            Format::Aiff(_) => "aiff",
            Format::Flac(_) => "flac",
            Format::Alac(_) | Format::Aac(_) => "m4a",
            Format::Mp3(_) | Format::Mp3Vbr(_) => "mp3",
            Format::Opus(_) => "opus",
            Format::Vorbis(_) => "ogg",
        }
    }

    /// the media type to serve the file as
    pub fn mime(&self) -> &'static str {
        match self.format {
            Format::Wav(_) => "audio/wav",
            Format::Aiff(_) => "audio/aiff",
            Format::Flac(_) => "audio/flac",
            Format::Alac(_) | Format::Aac(_) => "audio/mp4",
            Format::Mp3(_) | Format::Mp3Vbr(_) => "audio/mpeg",
            Format::Opus(_) | Format::Vorbis(_) => "audio/ogg",
        }
    }

    pub fn lossless(&self) -> bool {
        matches!(self.format, Format::Wav(_) | Format::Aiff(_) | Format::Flac(_) | Format::Alac(_))
    }

    /// what a menu calls it, for audio at `source` Hz: `MP3, 320 kbps`, `FLAC, 16-bit, 44.1 kHz`
    pub fn label(&self, source: u32) -> String {
        let khz = |r: u32| {
            if r.is_multiple_of(1000) { format!("{} kHz", r / 1000) } else { format!("{:.1} kHz", r as f64 / 1000.0) }
        };
        let rate = khz(self.rate_for(source));
        let depth = |d: Depth| match d {
            Depth::F32 => "32-bit float".to_string(),
            d => format!("{}-bit", d.bits()),
        };
        let rated = |s: String| if self.rate.is_some() { format!("{s}, {rate}") } else { s };
        match self.format {
            Format::Wav(d) => format!("WAV, {}, {rate}", depth(d)),
            Format::Aiff(d) => format!("AIFF, {}, {rate}", depth(d)),
            Format::Flac(d) => format!("FLAC, {}, {rate}", depth(d)),
            Format::Alac(d) => format!("Apple Lossless, {}, {rate}", depth(d)),
            Format::Mp3(k) => rated(format!("MP3, {k} kbps")),
            Format::Mp3Vbr(q) => rated(format!("MP3, VBR V{q}")),
            Format::Aac(k) => rated(format!("AAC, {k} kbps")),
            Format::Opus(k) => format!("Opus, {k} kbps"),
            Format::Vorbis(q) => rated(format!("Ogg Vorbis, quality {q}")),
        }
    }

    /// why this machine cannot write it, when it cannot: AAC and Apple Lossless need a system encoder
    pub fn unavailable(&self) -> Option<String> {
        match self.format {
            Format::Aac(_) | Format::Alac(_) if system::encoder().is_none() => Some(format!(
                "{} needs Core Audio (macOS) or ffmpeg, and this machine has neither: install ffmpeg, or export MP3 or Opus",
                self.label(48_000).split(',').next().unwrap_or_default()
            )),
            _ => None,
        }
    }
}

impl fmt::Display for Spec {
    /// the canonical spec: `mp3`, `flac:16`, `opus:96`, `wav@44100`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let depth = |d: Depth| match d {
            Depth::I24 => String::new(),
            d => format!(":{}", d.bits()),
        };
        match self.format {
            Format::Wav(d) => write!(f, "wav{}", depth(d))?,
            Format::Aiff(d) => write!(f, "aiff{}", depth(d))?,
            Format::Flac(d) => write!(f, "flac{}", depth(d))?,
            Format::Alac(d) => write!(f, "alac{}", depth(d))?,
            Format::Mp3(320) => write!(f, "mp3")?,
            Format::Mp3(k) => write!(f, "mp3:{k}")?,
            Format::Mp3Vbr(q) => write!(f, "mp3:v{q}")?,
            Format::Aac(256) => write!(f, "aac")?,
            Format::Aac(k) => write!(f, "aac:{k}")?,
            Format::Opus(160) => write!(f, "opus")?,
            Format::Opus(k) => write!(f, "opus:{k}")?,
            Format::Vorbis(7) => write!(f, "ogg")?,
            Format::Vorbis(q) => write!(f, "ogg:{q}")?,
        }
        if let Some(r) = self.rate {
            write!(f, "@{r}")?;
        }
        Ok(())
    }
}

fn number<T: FromStr>(what: &str, s: &str) -> Result<T> {
    s.parse::<T>().map_err(|_| anyhow::anyhow!("'{s}' is not a {what}"))
}

impl FromStr for Spec {
    type Err = anyhow::Error;

    /// `format[:quality][@rate]`: `wav`, `wav:16`, `wav:32` (float), `aiff`, `flac:16`, `alac`, `mp3` (320 kbps),
    /// `mp3:192`, `mp3:v0`, `aac:192`, `opus:96`, `ogg:6`, and a rate after `@` (`flac@44100`, `mp3@44.1`)
    fn from_str(s: &str) -> Result<Spec> {
        let s = s.trim().to_ascii_lowercase();
        let (body, rate) = match s.split_once('@') {
            Some((b, r)) => {
                let r = r.trim_end_matches("khz").trim_end_matches('k');
                let hz = match r {
                    "44.1" => 44_100,
                    "48" => 48_000,
                    r => number::<u32>("sample rate", r)?,
                };
                if !RATES.contains(&hz) {
                    bail!("a file is written at 44100 or 48000 Hz, not {hz}");
                }
                (b, Some(hz))
            }
            None => (s.as_str(), None),
        };
        let (name, q) = match body.split_once(':') {
            Some((n, q)) => (n, Some(q)),
            None => (body, None),
        };
        let depth = |float: bool| -> Result<Depth> {
            match q {
                None | Some("24") => Ok(Depth::I24),
                Some("16") => Ok(Depth::I16),
                Some("32") | Some("32f") | Some("float") if float => Ok(Depth::F32),
                Some(q) => bail!("{name} is 16 or 24 bits{}, not '{q}'", if float { " (or 32, float)" } else { "" }),
            }
        };
        let kbps = |default: u32, lo: u32, hi: u32| -> Result<u32> {
            let k = match q {
                None => default,
                Some(q) => number::<u32>("bitrate in kbps", q.trim_end_matches("kbps").trim_end_matches('k'))?,
            };
            if !(lo..=hi).contains(&k) {
                bail!("{name} runs from {lo} to {hi} kbps, not {k}");
            }
            Ok(k)
        };
        let format = match name {
            "wav" | "wave" => Format::Wav(depth(true)?),
            "aif" | "aiff" => Format::Aiff(depth(false)?),
            "flac" => Format::Flac(depth(false)?),
            "alac" => Format::Alac(depth(false)?),
            "mp3" => match q {
                Some(v) if v.starts_with('v') => {
                    let q = number::<u8>("VBR quality (V0 to V9)", &v[1..])?;
                    if q > 9 {
                        bail!("MP3 VBR runs from V0 (best) to V9, not V{q}");
                    }
                    Format::Mp3Vbr(q)
                }
                _ => {
                    let k = kbps(320, 32, 320)?;
                    if !MP3_KBPS.contains(&k) {
                        bail!("MP3 takes {} kbps, not {k}", MP3_KBPS.map(|k| k.to_string()).join(", "));
                    }
                    Format::Mp3(k)
                }
            },
            "aac" | "m4a" => Format::Aac(kbps(256, 64, 320)?),
            "opus" => Format::Opus(kbps(160, 32, 256)?),
            "ogg" | "vorbis" => {
                let q = match q {
                    None => 7,
                    Some(q) => number::<u8>("Vorbis quality (0 to 10)", q.trim_start_matches('q'))?,
                };
                if q > 10 {
                    bail!("Vorbis quality runs from 0 to 10, not {q}");
                }
                Format::Vorbis(q)
            }
            other => bail!("unknown format '{other}': wav, aiff, flac, alac, mp3, aac, opus or ogg"),
        };
        if matches!(format, Format::Opus(_)) && rate.is_some_and(|r| r != 48_000) {
            bail!("Opus is always 48 kHz");
        }
        Ok(Spec { format, rate })
    }
}

/// the formats a menu offers, most used first within lossless and lossy
pub fn presets() -> Vec<Spec> {
    ["wav", "wav:16", "aiff", "flac", "flac:16", "alac", "mp3", "mp3:v0", "mp3:192", "aac", "opus", "opus:96", "ogg"]
        .iter()
        .map(|s| s.parse().expect("preset"))
        .collect()
}

/// Encode `pcm` as `spec`, resampled to the file's rate first when it differs.
pub fn encode(spec: &Spec, pcm: Pcm, tags: &Tags) -> Result<Vec<u8>> {
    if !(1..=2).contains(&pcm.channels) {
        bail!("claudius writes mono and stereo files, not {} channels", pcm.channels);
    }
    if let Some(why) = spec.unavailable() {
        bail!(why);
    }
    let rate = spec.rate_for(pcm.rate);
    let resampled;
    let pcm = if rate == pcm.rate {
        pcm
    } else {
        resampled = resample::convert(pcm.samples, pcm.channels, pcm.rate, rate);
        Pcm { samples: &resampled, channels: pcm.channels, rate }
    };
    match spec.format {
        Format::Wav(d) => pcm::wav(&pcm, d),
        Format::Aiff(d) => pcm::aiff(&pcm, d),
        Format::Flac(d) => {
            flac::encode(&pcm::quantize(pcm.samples, d), pcm.channels, d.bits() as usize, pcm.rate, tags)
        }
        Format::Alac(d) => system::alac(&pcm, d, tags),
        Format::Mp3(k) => mp3::encode(&pcm, k, None, tags),
        Format::Mp3Vbr(q) => mp3::encode(&pcm, 320, Some(q), tags),
        Format::Aac(k) => system::aac(&pcm, k, tags),
        Format::Opus(k) => opus::encode(&pcm, k, tags),
        Format::Vorbis(q) => vorbis::encode(&pcm, q, tags),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_read_and_print_the_same() {
        for (s, want) in [
            ("wav", "wav"),
            ("WAV:24", "wav"),
            ("wav:16", "wav:16"),
            ("wav:32", "wav:32"),
            ("aif", "aiff"),
            ("flac:16@48000", "flac:16@48000"),
            ("alac", "alac"),
            ("mp3", "mp3"),
            ("mp3:320", "mp3"),
            ("mp3:192k", "mp3:192"),
            ("mp3:V0", "mp3:v0"),
            ("m4a", "aac"),
            ("aac:128kbps", "aac:128"),
            ("opus:96", "opus:96"),
            ("vorbis", "ogg"),
            ("ogg:q5", "ogg:5"),
            ("mp3@44.1", "mp3@44100"),
            ("wav@48k", "wav@48000"),
        ] {
            let spec: Spec = s.parse().unwrap();
            assert_eq!(spec.to_string(), want, "{s}");
            assert_eq!(want.parse::<Spec>().unwrap(), spec, "{want}");
        }
    }

    #[test]
    fn bad_specs_say_what_is_allowed() {
        for (s, says) in [
            ("mp4", "unknown format 'mp4'"),
            ("mp3:300", "MP3 takes 32, 40"),
            ("mp3:v12", "V0 (best) to V9"),
            ("flac:32", "flac is 16 or 24 bits, not '32'"),
            ("opus@44100", "Opus is always 48 kHz"),
            ("wav@96000", "44100 or 48000 Hz"),
            ("ogg:11", "0 to 10"),
            ("aac:500", "64 to 320 kbps"),
        ] {
            let e = s.parse::<Spec>().unwrap_err().to_string();
            assert!(e.contains(says), "{s}: {e}");
        }
    }

    #[test]
    fn rates_and_labels() {
        let rate = |s: &str| s.parse::<Spec>().unwrap().rate_for(48_000);
        assert_eq!(
            (rate("wav"), rate("wav:16"), rate("flac:16@48000"), rate("mp3"), rate("opus")),
            (48_000, 44_100, 48_000, 48_000, 48_000)
        );
        let label = |s: &str| s.parse::<Spec>().unwrap().label(48_000);
        assert_eq!(label("wav:16"), "WAV, 16-bit, 44.1 kHz");
        assert_eq!(label("flac"), "FLAC, 24-bit, 48 kHz");
        assert_eq!(label("wav:32"), "WAV, 32-bit float, 48 kHz");
        assert_eq!(label("mp3"), "MP3, 320 kbps");
        assert_eq!(label("mp3:v0@44100"), "MP3, VBR V0, 44.1 kHz");
        assert_eq!(label("opus:96"), "Opus, 96 kbps");
        assert_eq!(label("alac"), "Apple Lossless, 24-bit, 48 kHz");
        assert!(presets().iter().all(|p| p.to_string().parse::<Spec>().unwrap() == *p));
    }
}
