//! FLAC through flacenc, each encoding read back before it is kept. flacenc 0.5 picks a subframe's predictor by an
//! estimate of its cost, which misjudges some 24-bit audio: a song's master shrinks to 97 % of its samples instead of
//! 82 %, and a harmonica note of 600 KB became 438 MB that no decoder reads back. Counting the bits instead fixes
//! both, and fails on other audio (a tone of 40 harmonics grows 1,300 times). So both are tried and the smaller
//! that reads back is kept; when neither does, the samples are stored verbatim, which FLAC can always hold.

use std::io::Read;
use std::path::Path;

use anyhow::{Result, anyhow, bail};

use crate::Tags;

#[derive(Clone, Copy)]
enum Predict {
    /// flacenc's default: the order by an estimate of its cost
    Estimate,
    /// the order by the bits it really takes
    Count,
    /// no prediction: the samples as they are
    Verbatim,
}

/// Lossless FLAC of `samples` (interleaved integers `bits` deep), checked by decoding it: the smaller of the
/// encoder's two ways of choosing predictors that is no larger than the samples and reads back exactly, or the
/// samples stored verbatim.
pub fn encode(samples: &[i32], channels: usize, bits: usize, rate: u32, tags: &Tags) -> Result<Vec<u8>> {
    let limit = bound(samples.len(), bits) + comment_size(tags);
    let mut best: Option<Vec<u8>> = None;
    for predict in [Predict::Count, Predict::Estimate] {
        let Ok(s) = stream(samples, channels, bits, rate, predict) else { continue };
        let s = with_tags(s, tags);
        if s.len() <= limit && best.as_ref().is_none_or(|b| s.len() < b.len()) && reads_back(&s, samples, bits) {
            best = Some(s);
        }
    }
    if let Some(b) = best {
        return Ok(b);
    }
    let verbatim = with_tags(stream(samples, channels, bits, rate, Predict::Verbatim)?, tags);
    if !reads_back(&verbatim, samples, bits) {
        bail!("the audio does not read back from FLAC, even stored verbatim");
    }
    Ok(verbatim)
}

/// The most a FLAC of `samples` interleaved samples `bits` deep should take: the samples themselves, plus frame
/// headers and metadata. Only a broken encoding goes past it.
pub fn bound(samples: usize, bits: usize) -> usize {
    samples * bits / 8 + samples / 64 + 8192
}

/// A FLAC larger than the audio its header says it holds: one an earlier claudius wrote before conversions were
/// read back. Anything else (not a FLAC, missing, unreadable) is not.
pub fn bloated(path: &Path) -> bool {
    let Ok(len) = std::fs::metadata(path).map(|m| m.len()) else { return false };
    let mut head = [0u8; 42];
    let read = std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head));
    if read.is_err() || &head[..4] != b"fLaC" || head[4] & 0x7f != 0 {
        return false;
    }
    // STREAMINFO: 20 bits of rate, 3 of channels - 1, 5 of bits - 1, 36 of samples per channel
    let info = &head[8..];
    let channels = ((info[12] >> 1) & 7) as usize + 1;
    let bits = ((((info[12] & 1) << 4) | (info[13] >> 4)) + 1) as usize;
    let frames =
        (((info[13] & 0x0f) as u64) << 32) | u32::from_be_bytes([info[14], info[15], info[16], info[17]]) as u64;
    len as usize > bound(frames as usize * channels, bits) + 64 * 1024
}

fn stream(samples: &[i32], channels: usize, bits: usize, rate: u32, predict: Predict) -> Result<Vec<u8>> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let mut config = flacenc::config::Encoder::default();
    match predict {
        Predict::Estimate => {}
        Predict::Count => config.subframe_coding.fixed.order_sel = flacenc::config::OrderSel::BitCount,
        Predict::Verbatim => {
            config.subframe_coding.use_fixed = false;
            config.subframe_coding.use_lpc = false;
            config.stereo_coding.use_leftside = false;
            config.stereo_coding.use_rightside = false;
            config.stereo_coding.use_midside = false;
        }
    }
    let config = config.into_verified().map_err(|e| anyhow!("FLAC encoder settings: {e:?}"))?;
    let source = flacenc::source::MemSource::from_samples(samples, channels, bits, rate as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| anyhow!("cannot encode FLAC: {e:?}"))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|e| anyhow!("cannot write FLAC: {e:?}"))?;
    let mut bytes = sink.as_slice().to_vec();
    // STREAMINFO's minimum block size excludes the last block (FLAC format, section 8.2); flacenc counts it, which
    // makes a fixed-block-size stream look variable and some decoders (symphonia) refuse to open it
    if bytes.len() > 12 && &bytes[..4] == b"fLaC" && bytes[4] & 0x7f == 0 {
        let max = [bytes[10], bytes[11]];
        bytes[8..10].copy_from_slice(&max);
    }
    Ok(bytes)
}

/// whether the FLAC `bytes` decode to exactly `samples` (interleaved, `bits` deep)
fn reads_back(bytes: &[u8], samples: &[i32], bits: usize) -> bool {
    let Ok(d) = claudius_decode::decode_bytes(bytes.to_vec(), "flac") else { return false };
    let scale = (1i64 << (bits - 1)) as f32;
    d.samples.len() == samples.len() && d.samples.iter().zip(samples).all(|(v, s)| (v * scale).round() as i32 == *s)
}

/// the VORBIS_COMMENT block of `tags`, header included; empty when there are none
fn comment_block(tags: &Tags) -> Vec<u8> {
    let fields = tags.fields();
    if fields.is_empty() {
        return Vec::new();
    }
    let vendor = b"claudius";
    let mut body = Vec::new();
    body.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    body.extend_from_slice(vendor);
    body.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for (k, v) in fields {
        let c = format!("{k}={v}");
        body.extend_from_slice(&(c.len() as u32).to_le_bytes());
        body.extend_from_slice(c.as_bytes());
    }
    let mut block = vec![0x80 | 4];
    block.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    block.extend(body);
    block
}

fn comment_size(tags: &Tags) -> usize {
    comment_block(tags).len()
}

/// `bytes` with a VORBIS_COMMENT block after its last metadata block, which stops being the last
fn with_tags(mut bytes: Vec<u8>, tags: &Tags) -> Vec<u8> {
    let block = comment_block(tags);
    if block.is_empty() || bytes.len() < 8 || &bytes[..4] != b"fLaC" {
        return bytes;
    }
    let mut at = 4;
    while at + 4 <= bytes.len() {
        let last = bytes[at] & 0x80 != 0;
        let len = u32::from_be_bytes([0, bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
        if last {
            bytes[at] &= 0x7f;
            let end = at + 4 + len;
            bytes.splice(end..end, block);
            return bytes;
        }
        at += 4 + len;
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// loud 24-bit audio rich in harmonics, which flacenc's default settings encode into more bytes than its samples
    fn harmonics() -> Vec<i32> {
        (0..44_100)
            .map(|i| {
                let t = i as f64 / 44_100.0;
                let sum: f64 = (1..=40).map(|h| (2.0 * std::f64::consts::PI * 784.0 * h as f64 * t).sin()).sum();
                (sum * 7_000_000.0 / 40.0) as i32
            })
            .collect()
    }

    fn decoded(bytes: Vec<u8>, bits: u32) -> Vec<i32> {
        let d = claudius_decode::decode_bytes(bytes, "flac").unwrap();
        d.samples.iter().map(|v| (v * (1u32 << (bits - 1)) as f32).round() as i32).collect()
    }

    #[test]
    fn no_flac_is_larger_than_its_samples() {
        let samples = harmonics();
        let bytes = encode(&samples, 1, 24, 44_100, &Tags::default()).unwrap();
        assert!(
            bytes.len() <= bound(samples.len(), 24),
            "{} bytes for {} bytes of samples",
            bytes.len(),
            samples.len() * 3
        );
        assert_eq!(decoded(bytes, 24), samples, "lossless");
    }

    #[test]
    fn tags_go_after_streaminfo_and_the_audio_stays_the_same() {
        let samples: Vec<i32> = (0..20_000).map(|i| ((i as f64 * 0.01).sin() * 20_000.0) as i32).collect();
        let tags = Tags {
            title: "Glass Garden".into(),
            artist: "Ana".into(),
            software: "claudius 0.3.0".into(),
            ..Default::default()
        };
        let bytes = encode(&samples, 2, 16, 48_000, &tags).unwrap();
        assert_eq!(bytes[4] & 0x80, 0, "STREAMINFO is no longer the last block");
        let text = String::from_utf8_lossy(&bytes[..512]).into_owned();
        assert!(
            text.contains("TITLE=Glass Garden")
                && text.contains("ARTIST=Ana")
                && text.contains("ENCODER=claudius 0.3.0")
        );
        assert!(!text.contains("ALBUM="), "empty fields are left out");
        assert_eq!(decoded(bytes, 16), samples);
    }

    #[test]
    fn a_damaged_conversion_is_recognised() {
        let dir = tempfile::tempdir().unwrap();
        let flac = dir.path().join("out.flac");
        let samples: Vec<i32> = (0..20_000).map(|i| ((i as f64 * 0.01).sin() * 3_000_000.0) as i32).collect();
        std::fs::write(&flac, encode(&samples, 1, 24, 44_100, &Tags::default()).unwrap()).unwrap();
        assert!(!bloated(&flac));
        // what flacenc wrote for some 24-bit notes: frames far larger than the samples they hold
        let mut bytes = std::fs::read(&flac).unwrap();
        bytes.extend(std::iter::repeat_n(0xffu8, 200_000));
        std::fs::write(&flac, &bytes).unwrap();
        assert!(bloated(&flac));
        std::fs::write(dir.path().join("x.wav"), b"RIFF....WAVE").unwrap();
        assert!(!bloated(&dir.path().join("x.wav")), "only FLAC files are judged");
        assert!(!bloated(&dir.path().join("missing.flac")));
    }
}
