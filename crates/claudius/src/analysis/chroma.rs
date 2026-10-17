//! Harmony from audio: chroma (pitch-class energy) frames, key estimation with Krumhansl-Kessler profiles, chord
//! recognition by template matching with a pull towards the key's diatonic chords.

use crate::config::SRF;
use crate::dsp::fft::spectrogram;
use crate::theory::chord::{Chord, Quality};
use crate::theory::scale::{Key, Mode};

pub const NFFT: usize = 8192;

/// chroma per frame (hop samples) from bins between `lo` and `hi` Hz, each frame normalised to sum 1; only bins
/// close to an equal-tempered pitch count (so noise and drums smear less into it)
pub fn chroma_band(mono: &[f32], hop: usize, lo: f64, hi: f64) -> Vec<[f32; 12]> {
    let spec = spectrogram(mono, NFFT, hop);
    let map: Vec<Option<(usize, f32)>> = (0..=NFFT / 2)
        .map(|k| {
            let f = k as f64 * SRF / NFFT as f64;
            if f < lo || f > hi {
                return None;
            }
            let semis = 12.0 * (f / 440.0).log2();
            let off = (semis - semis.round()).abs();
            let w = (1.0 - off * 2.0).max(0.0) as f32;
            Some((((semis.round() as i64) + 9).rem_euclid(12) as usize, w))
        })
        .collect();
    spec.iter()
        .map(|frame| {
            let mut c = [0.0f32; 12];
            for (k, m) in frame.iter().enumerate() {
                if let Some((pc, w)) = map[k] {
                    c[pc] += m * m * w;
                }
            }
            let s: f32 = c.iter().sum::<f32>().max(1e-12);
            c.iter_mut().for_each(|v| *v /= s);
            c
        })
        .collect()
}

pub fn chroma(mono: &[f32], hop: usize) -> Vec<[f32; 12]> {
    chroma_band(mono, hop, 110.0, 2000.0)
}

/// the keys whose profiles fit an average chroma best, best first
pub fn key_candidates(avg: &[f32; 12], n: usize) -> Vec<(Key, f64)> {
    let a: [f64; 12] = std::array::from_fn(|i| avg[i] as f64);
    let mut all = Vec::with_capacity(24);
    for t in 0..12 {
        for (prof, mode) in [(KK_MAJ, Mode::Major), (KK_MIN, Mode::Minor)] {
            let rot: [f64; 12] = std::array::from_fn(|i| prof[(i + 12 - t) % 12]);
            all.push((Key::new(t as u8, mode), corr(&a, &rot)));
        }
    }
    all.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
    all.truncate(n);
    all
}

/// the chord of a bar from its chroma and the bass chroma under it (the bass mostly plays the root)
pub fn chord_with_bass(c: &[f32; 12], bass: &[f32; 12], key: Option<&Key>) -> (Chord, f64) {
    let qualities = [Quality::Maj, Quality::Min, Quality::Min7, Quality::Dom7, Quality::Maj7, Quality::Dim];
    let mut best = (Chord::new(0, Quality::Maj), f64::MIN);
    for root in 0..12u8 {
        for q in qualities {
            let ch = Chord::new(root, q);
            let pcs = ch.pcs();
            let inside: f64 = pcs.iter().map(|p| c[*p as usize] as f64).sum();
            let expected = pcs.len() as f64 / 12.0;
            let mut s = inside - expected - 0.3 * (1.0 - inside) + 0.5 * bass[root as usize] as f64;
            if matches!(q, Quality::Min7 | Quality::Dom7 | Quality::Maj7) {
                s -= 0.05;
            }
            if let Some(k) = key
                && pcs.iter().all(|p| k.pcs().contains(p))
            {
                s += 0.08;
            }
            if s > best.1 {
                best = (ch, s);
            }
        }
    }
    best
}

const KK_MAJ: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const KK_MIN: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

fn corr(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let ma = a.iter().sum::<f64>() / 12.0;
    let mb = b.iter().sum::<f64>() / 12.0;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        sab += (a[i] - ma) * (b[i] - mb);
        saa += (a[i] - ma).powi(2);
        sbb += (b[i] - mb).powi(2);
    }
    sab / (saa.sqrt() * sbb.sqrt()).max(1e-12)
}

/// the best-fitting major / minor key of an average chroma, with its correlation
pub fn key(avg: &[f32; 12]) -> (Key, f64) {
    let a: [f64; 12] = std::array::from_fn(|i| avg[i] as f64);
    let mut best = (Key::new(0, Mode::Major), f64::MIN);
    for t in 0..12 {
        for (prof, mode) in [(KK_MAJ, Mode::Major), (KK_MIN, Mode::Minor)] {
            let rot: [f64; 12] = std::array::from_fn(|i| prof[(i + 12 - t) % 12]);
            let c = corr(&a, &rot);
            if c > best.1 {
                best = (Key::new(t as u8, mode), c);
            }
        }
    }
    best
}

/// the chord that best explains a chroma vector (triads and sevenths), diatonic chords of `key` preferred
pub fn chord(c: &[f32; 12], key: Option<&Key>) -> (Chord, f64) {
    let qualities = [Quality::Maj, Quality::Min, Quality::Min7, Quality::Dom7, Quality::Maj7, Quality::Dim];
    let mut best = (Chord::new(0, Quality::Maj), f64::MIN);
    for root in 0..12u8 {
        for q in qualities {
            let ch = Chord::new(root, q);
            let pcs = ch.pcs();
            let inside: f64 = pcs.iter().map(|p| c[*p as usize] as f64).sum();
            let outside: f64 = 1.0 - inside;
            // fewer-note chords explain less energy: normalise by the share a random chord of that size would get
            let expected = pcs.len() as f64 / 12.0;
            let mut s = inside - expected - 0.3 * outside + 0.6 * c[root as usize] as f64;
            if q == Quality::Min7 || q == Quality::Dom7 || q == Quality::Maj7 {
                s -= 0.04;
            }
            if let Some(k) = key
                && pcs.iter().all(|p| k.pcs().contains(p))
            {
                s += 0.06;
            }
            if s > best.1 {
                best = (ch, s);
            }
        }
    }
    best
}
