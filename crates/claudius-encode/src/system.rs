//! AAC and Apple Lossless by the system's encoder, in an .m4a: Core Audio's `afconvert` on macOS (Apple's AAC
//! encoder, the best there is), ffmpeg elsewhere when it is installed. The Rust AAC encoders are not fast enough yet:
//! the best one takes longer than the song to encode it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};

use crate::{Depth, Pcm, Tags, pcm};

pub(crate) enum Encoder {
    Afconvert(PathBuf),
    Ffmpeg(PathBuf),
}

/// the encoder this machine has: afconvert on macOS, otherwise ffmpeg when it is installed
pub(crate) fn encoder() -> Option<Encoder> {
    let afconvert = Path::new("/usr/bin/afconvert");
    if cfg!(target_os = "macos") && afconvert.is_file() {
        return Some(Encoder::Afconvert(afconvert.to_path_buf()));
    }
    claudius_decode::ffmpeg().map(Encoder::Ffmpeg)
}

pub(crate) fn aac(pcm: &Pcm, kbps: u32, tags: &Tags) -> Result<Vec<u8>> {
    if ![44_100, 48_000].contains(&pcm.rate) {
        bail!("AAC is written at 44.1 or 48 kHz, not {} Hz", pcm.rate);
    }
    // constrained VBR at the highest quality, as iTunes Plus is made
    let bps = (kbps * 1000).to_string();
    run(
        pcm,
        Depth::I24,
        tags,
        &["-f", "m4af", "-d", "aac", "-b", &bps, "-s", "2", "-q", "127"],
        &["-c:a", "aac", "-b:a", &format!("{kbps}k")],
    )
}

pub(crate) fn alac(pcm: &Pcm, depth: Depth, tags: &Tags) -> Result<Vec<u8>> {
    run(pcm, depth, tags, &["-f", "m4af", "-d", "alac"], &["-c:a", "alac"])
}

static SCRATCH: AtomicUsize = AtomicUsize::new(0);

/// Write `pcm` as a WAV `depth` deep into a scratch folder, have the encoder turn it into an .m4a, and return that.
/// ffmpeg also writes the tags; afconvert cannot.
fn run(pcm: &Pcm, depth: Depth, tags: &Tags, afconvert: &[&str], ffmpeg: &[&str]) -> Result<Vec<u8>> {
    let Some(encoder) = encoder() else {
        bail!("no AAC encoder here: Core Audio is on macOS only, and ffmpeg is not installed")
    };
    let dir = std::env::temp_dir().join(format!(
        "claudius-encode-{}-{}",
        std::process::id(),
        SCRATCH.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let result = (|| -> Result<Vec<u8>> {
        let (input, output) = (dir.join("in.wav"), dir.join("out.m4a"));
        std::fs::write(&input, pcm::wav(pcm, depth)?)?;
        let mut cmd;
        match &encoder {
            Encoder::Afconvert(p) => {
                cmd = Command::new(p);
                cmd.args(afconvert).arg(&input).arg(&output);
            }
            Encoder::Ffmpeg(p) => {
                cmd = Command::new(p);
                cmd.args(["-v", "error", "-nostdin", "-y"]).args(claudius_decode::ffmpeg_input(&input)).args(ffmpeg);
                for (k, v) in tags.fields() {
                    let key = if k == "ENCODER" { "encoder".to_string() } else { k.to_ascii_lowercase() };
                    cmd.arg("-metadata").arg(format!("{key}={v}"));
                }
                cmd.arg(&output);
            }
        }
        let out = cmd.output().context("cannot run the system's encoder")?;
        if !out.status.success() {
            bail!("the system's encoder failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(std::fs::read(&output)?)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aac_and_apple_lossless_where_there_is_an_encoder() {
        if encoder().is_none() {
            #[cfg(target_os = "macos")]
            panic!("every Mac has afconvert");
            #[cfg(not(target_os = "macos"))]
            return;
        }
        let n = 48_000;
        let x: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = 0.4 * (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / 48_000.0).sin();
                [v, -v]
            })
            .collect();
        let pcm = Pcm { samples: &x, channels: 2, rate: 48_000 };
        let aac = aac(&pcm, 256, &Tags::default()).unwrap();
        assert_eq!(&aac[4..8], b"ftyp");
        let d = claudius_decode::decode_bytes(aac, "m4a").unwrap();
        assert_eq!((d.rate, d.channels), (48_000, 2));
        assert!(d.samples.len() >= x.len(), "{} for {}", d.samples.len(), x.len());
        let alac = alac(&pcm, Depth::I24, &Tags::default()).unwrap();
        let d = claudius_decode::decode_bytes(alac, "m4a").unwrap();
        let ints = pcm::quantize(&x, Depth::I24);
        let back: Vec<i32> = d.samples.iter().map(|v| (v * 8_388_608.0).round() as i32).collect();
        assert_eq!(back, ints, "Apple Lossless is lossless");
    }
}
