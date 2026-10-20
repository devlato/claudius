//! Uncompressed files: WAV through hound, AIFF written here, and the integer samples both (and FLAC) store.

use std::io::Cursor;

use anyhow::Result;

use crate::{Depth, Pcm};

/// Samples as integers `depth` deep, full scale clipped: 24-bit rounded, 16-bit with triangular (TPDF) dither of
/// one LSB, from a fixed seed, so the same audio always becomes the same file.
pub(crate) fn quantize(x: &[f32], depth: Depth) -> Vec<i32> {
    match depth {
        Depth::I24 | Depth::F32 => x.iter().map(|v| (v.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32).collect(),
        Depth::I16 => {
            let mut state = 0x9E37_79B9_7F4A_7C15u64;
            let mut uniform = move || {
                // xorshift64*: the top 24 bits, in [0, 1)
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u32 << 24) as f32
            };
            x.iter()
                .map(|v| {
                    let dither = uniform() - uniform();
                    (v * 32_767.0 + dither).round().clamp(-32_768.0, 32_767.0) as i32
                })
                .collect()
        }
    }
}

pub(crate) fn wav(pcm: &Pcm, depth: Depth) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: pcm.channels as u16,
        sample_rate: pcm.rate,
        bits_per_sample: depth.bits(),
        sample_format: if depth == Depth::F32 { hound::SampleFormat::Float } else { hound::SampleFormat::Int },
    };
    let mut out = Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut out, spec)?;
    match depth {
        Depth::F32 => {
            for v in pcm.samples {
                w.write_sample(*v)?;
            }
        }
        Depth::I16 => {
            for v in quantize(pcm.samples, depth) {
                w.write_sample(v as i16)?;
            }
        }
        Depth::I24 => {
            for v in quantize(pcm.samples, depth) {
                w.write_sample(v)?;
            }
        }
    }
    w.finalize()?;
    Ok(out.into_inner())
}

/// a sample rate as the 80-bit IEEE 754 extended float AIFF stores in its COMM chunk
fn extended(rate: u32) -> [u8; 10] {
    let mut out = [0u8; 10];
    if rate == 0 {
        return out;
    }
    let shift = 31 - rate.leading_zeros();
    out[..2].copy_from_slice(&(16_383 + shift as u16).to_be_bytes());
    out[2..].copy_from_slice(&((rate as u64) << (63 - shift)).to_be_bytes());
    out
}

/// AIFF: a FORM of a COMM chunk (channels, frames, bits, rate) and an SSND chunk of big-endian samples
pub(crate) fn aiff(pcm: &Pcm, depth: Depth) -> Result<Vec<u8>> {
    let bytes = depth.bits() as usize / 8;
    let samples = quantize(pcm.samples, depth);
    let data = samples.len() * bytes;
    let ssnd = 8 + data;
    let pad = ssnd % 2;
    let form = 4 + (8 + 18) + (8 + ssnd + pad);
    let mut out = Vec::with_capacity(8 + form);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(form as u32).to_be_bytes());
    out.extend_from_slice(b"AIFF");
    out.extend_from_slice(b"COMM");
    out.extend_from_slice(&18u32.to_be_bytes());
    out.extend_from_slice(&(pcm.channels as u16).to_be_bytes());
    out.extend_from_slice(&((samples.len() / pcm.channels) as u32).to_be_bytes());
    out.extend_from_slice(&depth.bits().to_be_bytes());
    out.extend_from_slice(&extended(pcm.rate));
    out.extend_from_slice(b"SSND");
    out.extend_from_slice(&(ssnd as u32).to_be_bytes());
    out.extend_from_slice(&[0u8; 8]);
    for v in samples {
        out.extend_from_slice(&v.to_be_bytes()[4 - bytes..]);
    }
    if pad == 1 {
        out.push(0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, channels: usize) -> Vec<f32> {
        (0..n * channels)
            .map(|i| ((i / channels) as f32 * 0.031).sin() * 0.7 * if i % channels == 0 { 1.0 } else { -0.5 })
            .collect()
    }

    #[test]
    fn aiff_sample_rates_are_the_extended_floats_apple_writes() {
        assert_eq!(extended(44_100), [0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
        assert_eq!(extended(48_000), [0x40, 0x0E, 0xBB, 0x80, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn wav_and_aiff_read_back_as_written() {
        for channels in [1usize, 2] {
            let x = tone(4801, channels);
            let pcm = Pcm { samples: &x, channels, rate: 48_000 };
            for (depth, bits) in [(Depth::I16, 16u32), (Depth::I24, 24)] {
                let ints = quantize(&x, depth);
                for (name, bytes) in [("wav", wav(&pcm, depth).unwrap()), ("aiff", aiff(&pcm, depth).unwrap())] {
                    let d = claudius_decode::decode_bytes(bytes, name).unwrap();
                    assert_eq!((d.rate, d.channels), (48_000, channels), "{name} {bits}");
                    let back: Vec<i32> =
                        d.samples.iter().map(|v| (v * (1u32 << (bits - 1)) as f32).round() as i32).collect();
                    assert_eq!(back, ints, "{name} {bits}-bit, {channels} channels: lossless");
                }
            }
            let d = claudius_decode::decode_bytes(wav(&pcm, Depth::F32).unwrap(), "wav").unwrap();
            assert_eq!(d.samples, x, "float WAV keeps every sample");
        }
    }

    #[test]
    fn sixteen_bits_are_dithered_the_same_way_every_time() {
        let x = vec![0.25f32 / 32_767.0; 10_000];
        let a = quantize(&x, Depth::I16);
        assert_eq!(a, quantize(&x, Depth::I16));
        // a quarter of an LSB survives as the average of the dithered samples, instead of rounding to silence
        let mean = a.iter().map(|v| *v as f64).sum::<f64>() / a.len() as f64;
        assert!((mean - 0.25).abs() < 0.05, "{mean}");
        assert!(a.iter().all(|v| v.abs() <= 1));
    }
}
