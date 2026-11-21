//! Opus, which symphonia finds in Ogg, WebM (Matroska) and MP4 but does not decode, decoded by opus-pure (pure Rust,
//! byte-exact with libopus where the format allows) and trimmed as the container says, as symphonia's own decoders
//! trim: an .opus file decodes to exactly the samples it was made from, its encoder's pre-skip and its last packet's
//! padding left out.

use std::path::Path;

use anyhow::{Result, anyhow, bail};
use opus_pure::{MAX_PACKET_SAMPLES, OpusDecoder, OpusMSDecoder};
use symphonia::core::codecs::audio::AudioCodecParameters;
use symphonia::core::errors::Error as Sym;
use symphonia::core::formats::FormatReader;

use crate::{Decoded, TooLong};

/// mono and stereo, or the several streams of a surround one
enum Decoder {
    One(OpusDecoder),
    Many(OpusMSDecoder),
}

impl Decoder {
    /// The decoder the stream's identification header asks for, with the gain it declares, and its channel count.
    /// Ogg and WebM carry the header as an OpusHead, its numbers little-endian; an MP4 carries a dOps box (version 0,
    /// where an OpusHead's is 1), laid out the same with its numbers big-endian, which symphonia hands on behind the
    /// OpusHead magic.
    fn new(head: &[u8]) -> Result<(Decoder, usize)> {
        if head.len() < 19 || &head[..8] != b"OpusHead" {
            bail!("the Opus stream has no identification header");
        }
        let channels = usize::from(head[9]);
        if channels == 0 {
            bail!("the Opus stream declares no channels");
        }
        let gain = [head[16], head[17]];
        let gain = i32::from(if head[8] == 0 { i16::from_be_bytes(gain) } else { i16::from_le_bytes(gain) });
        let family = head[18];
        let decoder = if family == 0 {
            let mut d = OpusDecoder::new(48_000, channels)?;
            d.gain_q8 = gain;
            Decoder::One(d)
        } else {
            let mut d = OpusMSDecoder::new(48_000, channels, family)?;
            for s in d.streams_mut() {
                s.gain_q8 = gain;
            }
            Decoder::Many(d)
        };
        Ok((decoder, channels))
    }

    /// one packet into `out`, interleaved; the frames it decoded
    fn decode(&mut self, packet: &[u8], out: &mut [f32]) -> opus_pure::Result<usize> {
        match self {
            Decoder::One(d) => d.decode(packet, MAX_PACKET_SAMPLES, out),
            Decoder::Many(d) => d.decode(packet, MAX_PACKET_SAMPLES, out),
        }
    }
}

/// Track `track_id` of `format`, an Opus stream with codec parameters `params`, decoded at 48 kHz, its usual rate;
/// a damaged packet is skipped, as players do. `name` is what a too-long error names.
pub(crate) fn decode(
    format: &mut dyn FormatReader,
    track_id: u32,
    params: &AudioCodecParameters,
    max_samples: usize,
    name: &Path,
) -> Result<Decoded> {
    let head = params.extra_data.as_deref().ok_or_else(|| anyhow!("the Opus stream has no identification header"))?;
    let (mut decoder, channels) = Decoder::new(head)?;
    let mut block = vec![0.0f32; MAX_PACKET_SAMPLES * channels];
    let mut out = Decoded { rate: 48_000, channels, samples: Vec::new() };
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
        let Ok(n) = decoder.decode(&packet.data, &mut block) else { continue };
        // the encoder's pre-skip at the start and the padding of the last packet, as the container marks them
        let start = usize::try_from(packet.trim_start.get()).unwrap_or(usize::MAX).min(n);
        let end = n.saturating_sub(usize::try_from(packet.trim_end.get()).unwrap_or(usize::MAX)).max(start);
        let kept = &block[start * channels..end * channels];
        if out.samples.len() + kept.len() > max_samples {
            return Err(TooLong(name.to_path_buf()).into());
        }
        out.samples.extend_from_slice(kept);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use opus_pure::{Application, MAX_PACKET_BYTES, OggOpusWriter, OpusEncoder, OpusHead};

    use super::*;

    /// `x` (interleaved) as an .opus file, gapless as claudius-encode writes one: pre-skip recorded, the last
    /// packet's duration ending on the last real sample
    fn opus_file(x: &[f32], channels: usize) -> Vec<u8> {
        let mut encoder = OpusEncoder::new(48_000, channels, Application::Audio).unwrap();
        encoder.bitrate_bps = 128_000;
        let head = OpusHead::for_encoder(&encoder, 48_000);
        let (frame, total) = (960, x.len() / channels);
        let frames = (total + head.pre_skip as usize).div_ceil(frame);
        let last = u64::from(head.pre_skip) + total as u64;
        let mut w = OggOpusWriter::new(Vec::new(), head).unwrap();
        let (mut packet, mut block) = (vec![0u8; MAX_PACKET_BYTES], vec![0.0f32; frame * channels]);
        for i in 0..frames {
            let s = (i * frame * channels).min(x.len());
            let e = (s + frame * channels).min(x.len());
            block[..e - s].copy_from_slice(&x[s..e]);
            block[e - s..].fill(0.0);
            let n = encoder.encode(&block, frame, &mut packet).unwrap();
            if i + 1 == frames {
                let rest = last - w.granule() as u64;
                w.write_packet_with_duration(&packet[..n], rest as u32).unwrap();
            } else {
                w.write_packet(&packet[..n]).unwrap();
            }
        }
        w.finish().unwrap()
    }

    #[test]
    fn an_opus_file_decodes_to_exactly_its_samples_without_ffmpeg() {
        let n = 48_000 + 321;
        let x: Vec<f32> = (0..n).map(|i| 0.5 * (i as f32 * std::f32::consts::TAU * 220.0 / 48_000.0).sin()).collect();
        let d = crate::decode_bytes(opus_file(&x, 1), "opus").unwrap();
        assert_eq!((d.rate, d.channels, d.frames()), (48_000, 1, n), "the pre-skip and the padding left out");
        // and it is the same sound, in time: no pre-skip left in front of it
        let (mut sig, mut err) = (0.0f64, 0.0f64);
        for i in 4_800..n - 4_800 {
            sig += f64::from(x[i]).powi(2);
            err += (f64::from(x[i]) - f64::from(d.samples[i])).powi(2);
        }
        let snr = 10.0 * (sig / err).log10();
        assert!(snr > 15.0, "{snr:.1} dB");
    }

    #[test]
    fn the_gain_a_header_declares_is_read_in_its_own_byte_order() {
        // an OpusHead (version 1, little-endian) and an MP4's dOps behind the magic (version 0, big-endian), each
        // with 1 dB of gain (256 in Q7.8), stereo, mapping family 0
        let mut ogg = b"OpusHead".to_vec();
        ogg.extend([1, 2, 0x38, 0x01, 0x80, 0xbb, 0, 0, 0x00, 0x01, 0]);
        let mut mp4 = b"OpusHead".to_vec();
        mp4.extend([0, 2, 0x01, 0x38, 0, 0, 0xbb, 0x80, 0x01, 0x00, 0]);
        for head in [ogg, mp4] {
            let (Decoder::One(d), 2) = Decoder::new(&head).unwrap() else { panic!("a stereo decoder") };
            assert_eq!(d.gain_q8, 256);
        }
        assert!(Decoder::new(b"OpusTags").is_err());
    }
}
