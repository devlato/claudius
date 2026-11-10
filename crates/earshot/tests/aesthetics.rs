//! Audiobox Aesthetics rating what it hears: a model check, as the parity checks with models are.
//!
//!     cargo test -p earshot --release --test aesthetics -- --ignored --nocapture

use earshot::aesthetics::{Aesthetics, Scores};
use earshot::models;

const SR: u32 = 44_100;

/// `secs` of a few bars of music: a soft kick on the beat, a bass on the root and a chord of three plucked strings, at
/// 120 BPM, on I-vi-IV-V in C
fn music(secs: f64) -> Vec<f32> {
    let n = (secs * SR as f64) as usize;
    let chords = [[60, 64, 67], [57, 60, 64], [53, 57, 60], [55, 59, 62]];
    let hz = |p: i32| 440.0 * 2f64.powf((p as f64 - 69.0) / 12.0);
    (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let beat = t * 2.0;
            let (bar, in_beat) = ((beat / 4.0) as usize % 4, beat.fract() / 2.0);
            let kick = (std::f64::consts::TAU * (50.0 + 80.0 * (-in_beat * 40.0).exp()) * in_beat).sin()
                * (-in_beat * 12.0).exp();
            let pluck = |p: i32| {
                (1..=5).map(|h| (std::f64::consts::TAU * hz(p) * h as f64 * t).sin() / h as f64).sum::<f64>()
                    * (-in_beat * 6.0).exp()
            };
            let chord: f64 = chords[bar].iter().map(|&p| pluck(p)).sum::<f64>() / 3.0;
            let bass = (std::f64::consts::TAU * hz(chords[bar][0] - 24) * t).sin() * (-in_beat * 3.0).exp();
            (0.35 * kick + 0.2 * chord + 0.25 * bass) as f32
        })
        .collect()
}

/// `secs` of white noise, clipped
fn noise(secs: f64) -> Vec<f32> {
    let mut s = 0x2545_f491_4f6c_dd1du64;
    (0..(secs * SR as f64) as usize)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0).clamp(-0.6, 0.6)
        })
        .collect()
}

#[test]
#[ignore]
fn music_rates_above_noise_on_every_score_but_its_usefulness() {
    let mut a = Aesthetics::load(&models::default_cache_dir(), false, 2).unwrap();
    let said = |s: &Scores| format!("CE {:.2} CU {:.2} PC {:.2} PQ {:.2}", s.ce, s.cu, s.pc, s.pq);
    let m = a.rate(&music(25.0), SR, None).unwrap();
    let n = a.rate(&noise(25.0), SR, None).unwrap();
    eprintln!("music: {}\nnoise: {}", said(&m), said(&n));
    for s in [m, n] {
        for v in [s.ce, s.cu, s.pc, s.pq] {
            assert!((0.0..=11.0).contains(&v), "{}", said(&s));
        }
    }
    assert!(m.ce > n.ce + 1.0 && m.pq > n.pq + 1.0, "music {}, noise {}", said(&m), said(&n));
}
