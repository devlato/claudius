//! Ogg Vorbis through vorbis_rs: libvorbis with the aoTuV tuning, compiled in.

use std::num::{NonZeroU8, NonZeroU32};

use anyhow::{Result, anyhow};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

use crate::{Pcm, Tags};

/// the Ogg stream serial: fixed, so the same audio always makes the same file
const SERIAL: i32 = 0x636c_6175;

/// quality `q` on Vorbis' 0..10 scale (7 is about 224 kbps)
pub(crate) fn encode(pcm: &Pcm, q: u8, tags: &Tags) -> Result<Vec<u8>> {
    let rate = NonZeroU32::new(pcm.rate).ok_or_else(|| anyhow!("no sample rate"))?;
    let channels = NonZeroU8::new(pcm.channels as u8).ok_or_else(|| anyhow!("no channels"))?;
    let mut builder = VorbisEncoderBuilder::new(rate, channels, Vec::new())?;
    builder
        .stream_serial(SERIAL)
        .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr { target_quality: q as f32 / 10.0 });
    builder.comment_tags(tags.fields())?;
    let mut e = builder.build()?;
    let planar: Vec<Vec<f32>> =
        (0..pcm.channels).map(|c| pcm.samples.iter().skip(c).step_by(pcm.channels).copied().collect()).collect();
    let n = planar[0].len();
    for start in (0..n).step_by(4096) {
        let end = (start + 4096).min(n);
        e.encode_audio_block(planar.iter().map(|ch| &ch[start..end]).collect::<Vec<_>>())?;
    }
    Ok(e.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorbis_decodes_to_the_same_music_and_the_same_bytes_twice() {
        let n = 48_000;
        let x: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = 0.4 * (i as f32 * 2.0 * std::f32::consts::PI * 523.25 / 48_000.0).sin();
                [v, v * 0.5]
            })
            .collect();
        let tags = Tags { title: "Paper Moon".into(), ..Default::default() };
        let pcm = Pcm { samples: &x, channels: 2, rate: 48_000 };
        let bytes = encode(&pcm, 7, &tags).unwrap();
        assert_eq!(bytes, encode(&pcm, 7, &tags).unwrap(), "deterministic");
        assert!(bytes.windows(16).any(|w| w == b"TITLE=Paper Moon"));
        let d = claudius_decode::decode_bytes(bytes, "ogg").unwrap();
        assert_eq!((d.rate, d.channels), (48_000, 2));
        assert!((d.samples.len() as i64 - x.len() as i64).abs() <= 2 * 2048, "{} for {}", d.samples.len(), x.len());
        let n = x.len().min(d.samples.len()) - 2 * 9600;
        let (mut sig, mut err) = (0.0f64, 0.0f64);
        for (a, b) in x.iter().zip(&d.samples).skip(9600).take(n) {
            sig += (*a as f64).powi(2);
            err += (*a as f64 - *b as f64).powi(2);
        }
        let snr = 10.0 * (sig / err).log10();
        assert!(snr > 20.0, "{snr:.1} dB");
    }
}
