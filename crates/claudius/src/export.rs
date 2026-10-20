//! Songs as files: the 48 kHz master in any format claudius writes ([`claudius_encode`]), resampled when the file is
//! at 44.1 kHz, and tagged with the song's title.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
pub use claudius_encode::{Format, Spec, Tags, presets};

use crate::audio::Stereo;
use crate::config::SR;

/// The MP3 every song folder has, for listening and sharing.
pub const FOLDER_MP3: &str = "mp3:256";

/// the tags of a song called `title`, made by this claudius
pub fn tags(title: &str) -> Tags {
    Tags { title: title.to_string(), software: format!("claudius {}", env!("CARGO_PKG_VERSION")), ..Default::default() }
}

pub fn encode(master: &Stereo, spec: &Spec, tags: &Tags) -> Result<Vec<u8>> {
    let x = master.interleaved();
    claudius_encode::encode(spec, claudius_encode::Pcm { samples: &x, channels: 2, rate: SR }, tags)
}

/// Write `master` as `spec` to `path`, which appears only once it is complete.
pub fn write(master: &Stereo, spec: &Spec, tags: &Tags, path: &Path) -> Result<()> {
    let bytes = encode(master, spec, tags).with_context(|| format!("cannot write {}", spec.label(SR)))?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.part", std::process::id()));
    std::fs::write(&tmp, bytes).with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))
}

/// The files to export `specs` to, named after `stem` in `dir`: `stem.mp3`, and when several specs share an
/// extension, the later ones after their spec as well (`stem-mp3-192.mp3`).
pub fn file_names(dir: &Path, stem: &str, specs: &[Spec]) -> Vec<PathBuf> {
    let mut seen: Vec<&str> = Vec::new();
    specs
        .iter()
        .map(|s| {
            let ext = s.extension();
            let name = if seen.contains(&ext) {
                format!("{stem}-{}.{ext}", s.to_string().replace([':', '@'], "-"))
            } else {
                format!("{stem}.{ext}")
            };
            seen.push(ext);
            dir.join(name)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_apart() {
        let specs: Vec<Spec> = ["mp3", "flac", "mp3:192", "wav:16", "wav"].iter().map(|s| s.parse().unwrap()).collect();
        let names: Vec<String> =
            file_names(Path::new("x"), "glass-garden", &specs).iter().map(|p| p.display().to_string()).collect();
        assert_eq!(
            names,
            [
                "x/glass-garden.mp3",
                "x/glass-garden.flac",
                "x/glass-garden-mp3-192.mp3",
                "x/glass-garden.wav",
                "x/glass-garden-wav.wav"
            ]
        );
    }

    #[test]
    fn a_master_becomes_a_cd_file_at_44_1_khz() {
        let n = SR as usize / 2;
        let m = Stereo {
            l: (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
            r: (0..n).map(|i| (i as f32 * 0.07).sin() * 0.5).collect(),
        };
        let bytes = encode(&m, &"wav:16".parse().unwrap(), &tags("Glass Garden")).unwrap();
        let d = claudius_decode::decode_bytes(bytes, "wav").unwrap();
        assert_eq!((d.rate, d.channels, d.samples.len() / 2), (44_100, 2, n * 44_100 / SR as usize));
    }
}
