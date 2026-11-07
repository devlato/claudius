//! FLAC through flacenc, frame by frame, each encoding read back before it is kept. flacenc 0.5 picks a subframe's
//! predictor by an estimate of its cost, which misjudges some 24-bit audio: a song's master shrinks to 97 % of its
//! samples instead of 82 %, and a harmonica note of 600 KB became 438 MB that no decoder reads back. Counting the
//! bits instead fixes both, and fails on other audio (a tone of 40 harmonics grows 1,300 times). Some frames both
//! misjudge: 4,096 samples, half silence and half noise, take 1.3 GB either way. So each frame is encoded both ways
//! and the smaller kept, or the frame's samples verbatim when both are larger than that, before anything is written:
//! written whole, twenty seconds of drums came to 6.7 GB. A stream that does not read back is stored verbatim
//! throughout, which FLAC can always hold.

use std::io::Read;
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use flacenc::component::{BitRepr, Frame, Stream, StreamInfo};
use flacenc::error::{Verified, Verify};
use flacenc::source::{Context, Fill, FrameBuf};

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

/// Lossless FLAC of `samples` (interleaved integers `bits` deep), checked by decoding it: each frame in the
/// smaller of the encoder's two ways of choosing predictors, or verbatim when both take more than its samples; or,
/// when that does not read back exactly, the samples stored verbatim.
pub fn encode(samples: &[i32], channels: usize, bits: usize, rate: u32, tags: &Tags) -> Result<Vec<u8>> {
    let limit = bound(samples.len(), bits) + comment_size(tags);
    if let Ok(s) = stream(samples, channels, bits, rate, &[Predict::Count, Predict::Estimate]) {
        let s = with_tags(s, tags);
        if s.len() <= limit && reads_back(&s, samples, bits) {
            return Ok(s);
        }
    }
    let verbatim = with_tags(stream(samples, channels, bits, rate, &[])?, tags);
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

fn config(predict: Predict) -> Result<Verified<flacenc::config::Encoder>> {
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
    config.into_verified().map_err(|e| anyhow!("FLAC encoder settings: {e:?}"))
}

/// The FLAC of `samples` (interleaved, `bits` deep), each frame in whichever of `ways` takes the fewest bits, unless
/// that is more than the frame's samples, when it holds them verbatim; with no `ways`, every frame does.
fn stream(samples: &[i32], channels: usize, bits: usize, rate: u32, ways: &[Predict]) -> Result<Vec<u8>> {
    let ways = ways.iter().map(|&w| config(w)).collect::<Result<Vec<_>>>()?;
    let verbatim = config(Predict::Verbatim)?;
    let block = verbatim.block_size;
    let info = StreamInfo::new(rate as usize, channels, bits).map_err(|e| anyhow!("FLAC stream: {e:?}"))?;
    let chunks: Vec<&[i32]> = samples.chunks(block * channels).collect();
    let frames = each(&chunks, |n, chunk| {
        let mut buf = FrameBuf::with_size(channels, block).map_err(|e| anyhow!("FLAC frame: {e:?}"))?;
        buf.fill_interleaved(chunk).map_err(|e| anyhow!("FLAC frame: {e:?}"))?;
        let frame = |config| {
            flacenc::encode_fixed_size_frame(config, &buf, n, &info).map_err(|e| anyhow!("cannot encode FLAC: {e:?}"))
        };
        // the samples, a frame header and its checksum, which verbatim takes
        let plain = chunk.len() * bits + 8 * (16 + channels + 2);
        let mut best: Option<Frame> = None;
        for config in &ways {
            let f = frame(config)?;
            if f.count_bits() <= plain && best.as_ref().is_none_or(|b| f.count_bits() < b.count_bits()) {
                best = Some(f);
            }
        }
        best.map_or_else(|| frame(&verbatim), Ok)
    })?;
    let mut stream = Stream::new(rate as usize, channels, bits).map_err(|e| anyhow!("FLAC stream: {e:?}"))?;
    stream.stream_info_mut().set_block_sizes(block, block).map_err(|e| anyhow!("FLAC stream: {e:?}"))?;
    let mut context = Context::new(bits, channels);
    for (chunk, frame) in chunks.iter().zip(frames) {
        context.fill_interleaved(chunk).map_err(|e| anyhow!("FLAC stream: {e:?}"))?;
        stream.add_frame(frame);
    }
    stream.stream_info_mut().set_md5_digest(&context.md5_digest());
    stream.stream_info_mut().set_total_samples(context.total_samples());
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

/// `f` of each of `items` and its place, in order, on as many threads as the computer runs at once; in a browser,
/// whose WebAssembly starts none (one would panic, and take the demo's engine with it), on this one
fn each<T: Sync, R: Send>(items: &[T], f: impl Fn(usize, &T) -> Result<R> + Sync) -> Result<Vec<R>> {
    let threads =
        if cfg!(target_arch = "wasm32") { 1 } else { std::thread::available_parallelism().map_or(1, |n| n.get()) };
    let per = items.len().div_ceil(threads).max(1);
    if per >= items.len() {
        return items.iter().enumerate().map(|(n, x)| f(n, x)).collect();
    }
    std::thread::scope(|s| {
        let f = &f;
        let parts: Vec<_> = items
            .chunks(per)
            .enumerate()
            .map(|(k, part)| s.spawn(move || part.iter().enumerate().map(|(i, x)| f(k * per + i, x)).collect()))
            .collect();
        let mut out = Vec::with_capacity(items.len());
        for part in parts {
            let part: Result<Vec<R>> = part.join().map_err(|_| anyhow!("a FLAC encoding thread stopped"))?;
            out.extend(part?);
        }
        Ok(out)
    })
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
    fn a_frame_both_ways_misjudge_is_kept_verbatim_and_the_rest_compressed() {
        // a second of a quiet tone, then a frame half silence and half noise, which flacenc makes 1.3 GB of either way
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut noise = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f64 / (1u64 << 24) as f64 * 2.0 - 1.0) * 5_000_000.0
        };
        let tone = (0..48_000).map(|i| ((i as f64 * 0.02).sin() * 300_000.0) as i32);
        let silence_then_noise = (0..4096).map(|i| if i < 2048 { 0 } else { noise() as i32 });
        let samples: Vec<i32> = tone.chain(silence_then_noise).collect();
        let bytes = encode(&samples, 1, 24, 48_000, &Tags::default()).unwrap();
        assert!(
            bytes.len() < samples.len() * 3 / 2,
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
