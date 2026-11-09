//! Basic Pitch hearing notes played: a model check, as the parity checks with models are.
//!
//!     cargo test -p earshot --release --test notes -- --ignored --nocapture

use earshot::models;
use earshot::notes::{Options, Transcriber};

const SR: u32 = 44_100;

/// a plucked string at MIDI `pitch`, `secs` long: six harmonics, each dying away faster than the one below it
fn pluck(pitch: u8, secs: f64) -> Vec<f32> {
    let hz = 440.0 * 2f64.powf((pitch as f64 - 69.0) / 12.0);
    (0..(secs * SR as f64) as usize)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let tone: f64 = (1..=6)
                .map(|h| (std::f64::consts::TAU * hz * h as f64 * t).sin() * (-t * (2.0 + h as f64)).exp() / h as f64)
                .sum();
            (0.3 * tone * (t * 400.0).min(1.0)) as f32
        })
        .collect()
}

fn add(into: &mut Vec<f32>, at: f64, x: &[f32]) {
    let from = (at * SR as f64) as usize;
    if into.len() < from + x.len() {
        into.resize(from + x.len(), 0.0);
    }
    for (o, v) in into[from..].iter_mut().zip(x) {
        *o += v;
    }
}

#[test]
#[ignore]
fn an_arpeggio_and_a_chord_are_heard_as_the_notes_played() {
    let mut x = Vec::new();
    let tune = [(0.5, 60), (1.0, 64), (1.5, 67), (2.0, 72)];
    for (at, pitch) in tune {
        add(&mut x, at, &pluck(pitch, 0.45));
    }
    for pitch in [53, 57, 60] {
        add(&mut x, 3.0, &pluck(pitch, 1.5));
    }
    add(&mut x, 5.0, &vec![0.0; SR as usize]);
    let mut t = Transcriber::load(&models::default_cache_dir(), false, 2).unwrap();
    let notes = t.notes(&x, SR, &Options::default()).unwrap();
    for n in &notes {
        eprintln!("{:6.3} {:6.3} {:3} {:.2}", n.start, n.end, n.pitch, n.amplitude);
    }
    let heard = |at: f64, pitch: u8| notes.iter().any(|n| n.pitch == pitch && (n.start - at).abs() < 0.06);
    for (at, pitch) in tune.into_iter().chain([(3.0, 53), (3.0, 57), (3.0, 60)]) {
        assert!(heard(at, pitch), "{pitch} at {at} s is not among {notes:?}");
    }
    assert!(notes.len() <= 9, "{} notes for the 7 played: {notes:?}", notes.len());
}
