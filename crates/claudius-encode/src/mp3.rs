//! MP3 through rusty_mp3 (pure Rust; within about 0.1 PEAQ ODG of LAME at 160 kbps and above), with an ID3v2.4 tag.

use anyhow::{Result, bail};

use crate::{Pcm, Tags};

/// constant bitrate `kbps`, or LAME's VBR quality `vbr` (0 = best) when it is given
pub(crate) fn encode(pcm: &Pcm, kbps: u32, vbr: Option<u8>, tags: &Tags) -> Result<Vec<u8>> {
    if ![32_000, 44_100, 48_000].contains(&pcm.rate) {
        bail!("MP3 is written at 32, 44.1 or 48 kHz, not {} Hz", pcm.rate);
    }
    let mut e = rusty_mp3::Mp3Encoder::new(rusty_mp3::Mp3EncoderConfig {
        bitrate_kbps: kbps,
        vbr_quality: vbr.map(|q| rusty_mp3::vbr_quality_index(q as f32)),
    });
    for chunk in pcm.samples.chunks(pcm.channels * 1152 * 16) {
        e.push_pcm_f32(chunk, pcm.channels as u16, pcm.rate)?;
    }
    // the Xing/Info frame, which players read the length and seek table from, is prepended by finish(): drain after
    e.finish();
    let mut out = id3(tags);
    loop {
        match e.next_packet() {
            Ok(p) => out.extend(p),
            Err(rusty_mp3::Error::Eof) | Err(rusty_mp3::Error::Again) => break,
            Err(err) => return Err(err.into()),
        }
    }
    Ok(out)
}

/// a 28-bit size as ID3v2 writes it: seven bits a byte
fn synchsafe(n: usize) -> [u8; 4] {
    [((n >> 21) & 0x7f) as u8, ((n >> 14) & 0x7f) as u8, ((n >> 7) & 0x7f) as u8, (n & 0x7f) as u8]
}

/// An ID3v2.4 tag of UTF-8 text frames; nothing when no field is set.
fn id3(tags: &Tags) -> Vec<u8> {
    let mut frames = Vec::new();
    let mut frame = |id: &[u8; 4], body: Vec<u8>| {
        frames.extend_from_slice(id);
        frames.extend_from_slice(&synchsafe(body.len()));
        frames.extend_from_slice(&[0, 0]);
        frames.extend(body);
    };
    for (id, v) in [
        (b"TIT2", &tags.title),
        (b"TPE1", &tags.artist),
        (b"TALB", &tags.album),
        (b"TDRC", &tags.date),
        (b"TSSE", &tags.software),
    ] {
        if !v.is_empty() {
            let mut body = vec![3u8];
            body.extend_from_slice(v.as_bytes());
            frame(id, body);
        }
    }
    if !tags.comment.is_empty() {
        // encoding, language, an empty description, the text
        let mut body = vec![3u8, b'e', b'n', b'g', 0];
        body.extend_from_slice(tags.comment.as_bytes());
        frame(b"COMM", body);
    }
    if frames.is_empty() {
        return frames;
    }
    let mut out = b"ID3\x04\x00\x00".to_vec();
    out.extend_from_slice(&synchsafe(frames.len()));
    out.extend(frames);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp3_decodes_to_the_same_music() {
        let rate = 48_000;
        let n = rate as usize * 2;
        let x: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f32 / rate as f32;
                let v = 0.3 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                    + 0.2 * (2.0 * std::f32::consts::PI * 660.0 * t).sin();
                [v, 0.8 * v]
            })
            .collect();
        let tags = Tags { title: "Night Bus".into(), software: "claudius".into(), ..Default::default() };
        for (kbps, vbr) in [(320, None), (192, None), (320, Some(0))] {
            let bytes = encode(&Pcm { samples: &x, channels: 2, rate }, kbps, vbr, &tags).unwrap();
            assert_eq!(&bytes[..3], b"ID3");
            assert!(bytes.windows(9).any(|w| w == b"Night Bus"));
            let tag = 10 + u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
            let info = &bytes[tag..tag + 400];
            assert!(
                info.windows(4).any(|w| w == b"Info" || w == b"Xing"),
                "{kbps} {vbr:?}: the Info frame comes first"
            );
            let d = claudius_decode::decode_bytes(bytes.clone(), "mp3").unwrap();
            assert_eq!((d.rate, d.channels), (rate, 2));
            let frames = d.samples.len() / 2;
            assert!(frames >= n && frames < n + 3 * 1152, "{frames} frames for {n}");
            // the decoder's delay: the shift that lines the two up best, then how close they are
            let corr = |lag: usize| -> f64 {
                (rate as usize..rate as usize + 9600).map(|i| x[2 * i] as f64 * d.samples[2 * (i + lag)] as f64).sum()
            };
            let lag = (0..2400).max_by(|a, b| corr(*a).partial_cmp(&corr(*b)).unwrap()).unwrap();
            let (mut sig, mut err) = (0.0f64, 0.0f64);
            for i in rate as usize..rate as usize + 24_000 {
                for c in 0..2 {
                    let a = x[2 * i + c] as f64;
                    sig += a * a;
                    err += (a - d.samples[2 * (i + lag) + c] as f64).powi(2);
                }
            }
            let snr = 10.0 * (sig / err).log10();
            assert!(snr > 20.0, "{kbps} kbps {vbr:?}: {snr:.1} dB");
        }
    }

    #[test]
    fn no_tags_no_id3() {
        assert!(id3(&Tags::default()).is_empty());
        assert_eq!(synchsafe(0x0fff_ffff), [0x7f; 4]);
        assert_eq!(synchsafe(257), [0, 0, 2, 1]);
    }
}
