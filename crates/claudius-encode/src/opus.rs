//! Opus through opus-pure (pure Rust, byte-exact with libopus 1.6 where the format allows), in Ogg: an .opus file.

use anyhow::{Result, bail};
use opus_pure::{Application, MAX_PACKET_BYTES, OggOpusWriter, OpusEncoder, OpusHead, OpusTags};

use crate::{Pcm, Tags};

/// Gapless: the encoder's lookahead is fed and its pre-skip recorded, and the last packet's duration ends the file
/// on the last real sample, so the file decodes to exactly the samples given.
pub(crate) fn encode(pcm: &Pcm, kbps: u32, tags: &Tags) -> Result<Vec<u8>> {
    if pcm.rate != 48_000 {
        bail!("Opus is written at 48 kHz, not {} Hz", pcm.rate);
    }
    let rate = pcm.rate as i32;
    let channels = pcm.channels;
    let frame = (rate / 50) as usize;
    let mut encoder = OpusEncoder::new(rate, channels, Application::Audio)?;
    encoder.bitrate_bps = kbps as i32 * 1000;
    let head = OpusHead::for_encoder(&encoder, pcm.rate);
    let mut comments = OpusTags::new();
    for (k, v) in tags.fields() {
        comments.push(k, v)?;
    }
    let total = pcm.samples.len() / channels;
    let frames = (total + head.pre_skip as usize).div_ceil(frame);
    let final_granule = u64::from(head.pre_skip) + total as u64;
    let mut w = OggOpusWriter::with_tags(Vec::new(), head, comments)?;
    let mut packet = vec![0u8; MAX_PACKET_BYTES];
    let per = frame * channels;
    let mut block = vec![0.0f32; per];
    for i in 0..frames {
        let s = (i * per).min(pcm.samples.len());
        let e = (s + per).min(pcm.samples.len());
        block[..e - s].copy_from_slice(&pcm.samples[s..e]);
        block[e - s..].fill(0.0);
        let n = encoder.encode(&block, frame, &mut packet)?;
        if i + 1 == frames {
            let rest = final_granule - w.granule() as u64;
            w.write_packet_with_duration(&packet[..n], rest as u32)?;
        } else {
            w.write_packet(&packet[..n])?;
        }
    }
    Ok(w.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// what opus-pure's decode example does: decode every packet, keep what the head and the granules say is audio
    fn decode(bytes: Vec<u8>) -> Vec<f32> {
        use opus_pure::{MAX_PACKET_SAMPLES, OggOpusReader, Trim};
        let mut reader = OggOpusReader::new(std::io::Cursor::new(bytes)).unwrap();
        let head = reader.head().clone();
        let channels = head.channel_count as usize;
        let mut decoder = head.decoder(48_000).unwrap();
        let mut trim = Trim::new(&head, 48_000, channels).unwrap();
        let mut block = vec![0.0f32; MAX_PACKET_SAMPLES * channels];
        let mut out = Vec::new();
        for packet in reader.packets() {
            let packet = packet.unwrap();
            let n = decoder.decode(&packet.data, MAX_PACKET_SAMPLES, &mut block).unwrap();
            out.extend_from_slice(trim.keep(&packet, &block[..n * channels]));
        }
        out
    }

    #[test]
    fn opus_decodes_to_exactly_as_many_samples() {
        let n = 48_000 + 123;
        let x: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = 0.4 * (i as f32 * 2.0 * std::f32::consts::PI * 330.0 / 48_000.0).sin();
                [v, -v]
            })
            .collect();
        let tags = Tags { title: "Low Tide".into(), ..Default::default() };
        let bytes = encode(&Pcm { samples: &x, channels: 2, rate: 48_000 }, 160, &tags).unwrap();
        assert_eq!(&bytes[..4], b"OggS");
        assert!(bytes.windows(14).any(|w| w == b"TITLE=Low Tide"));
        let pcm = decode(bytes);
        assert_eq!(pcm.len(), x.len(), "gapless");
        let (mut sig, mut err) = (0.0f64, 0.0f64);
        for i in 4800..pcm.len() - 4800 {
            sig += (x[i] as f64).powi(2);
            err += (x[i] as f64 - pcm[i] as f64).powi(2);
        }
        let snr = 10.0 * (sig / err).log10();
        assert!(snr > 15.0, "{snr:.1} dB");
    }
}
