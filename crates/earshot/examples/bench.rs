//! Timings in release mode: model load, one CLAP window, one AST window, batches (dev tool).
//!
//!     cargo run --release --example bench [-- threads]

use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let threads: usize = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let t = Instant::now();
    let ears = earshot::Ears::load(earshot::EarsOptions { threads, ..Default::default() })?;
    println!("load (sessions + cached vocab): {:.2} s, threads {}", t.elapsed().as_secs_f64(), ears.threads);
    let mut rng = 12345u64;
    let mut noise = |n: usize| -> Vec<f32> {
        (0..n)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                ((rng >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.2
            })
            .collect()
    };
    let x10 = noise(480_000);
    let x1 = noise(48_000);
    // warm-up
    ears.clap.embed_windows(&[&x1], earshot::Pad::RepeatPad)?;
    for _ in 0..2 {
        let t = Instant::now();
        ears.clap.embed_windows(&[&x10], earshot::Pad::RepeatPad)?;
        println!("CLAP one 10 s window: {:.3} s", t.elapsed().as_secs_f64());
    }
    let t = Instant::now();
    let fe = earshot::clap::FrontEnd::default();
    for _ in 0..10 {
        std::hint::black_box(fe.features(&x10, earshot::Pad::RepeatPad));
    }
    println!("  of which the mel front end: {:.3} s", t.elapsed().as_secs_f64() / 10.0);
    let segs: Vec<&[f32]> = vec![&x10; 8];
    let t = Instant::now();
    ears.clap.embed_windows(&segs, earshot::Pad::RepeatPad)?;
    println!(
        "CLAP batch of 8 windows: {:.3} s ({:.3} s / window)",
        t.elapsed().as_secs_f64(),
        t.elapsed().as_secs_f64() / 8.0
    );
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let t = Instant::now();
    ears.embed_text_raw(&[&format!("a never seen before phrase {stamp}")])?;
    println!("CLAP first new text (loads the text encoder): {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    ears.embed_text_raw(&[&format!("another never seen phrase {stamp}")])?;
    println!("CLAP next new text: {:.3} s", t.elapsed().as_secs_f64());
    if let Some(a) = &ears.ast {
        let y = noise(491_520); // 10.24 s at 48 kHz
        a.probs(&[&x1], earshot::Reduce::Max)?;
        for b in [1usize, 2, 4] {
            let arrays: Vec<&[f32]> = vec![&y; b];
            let t = Instant::now();
            a.probs(&arrays, earshot::Reduce::Max)?;
            println!(
                "AST {b} window(s) of 10.24 s: {:.3} s ({:.3} s / window)",
                t.elapsed().as_secs_f64(),
                t.elapsed().as_secs_f64() / b as f64
            );
        }
    }
    Ok(())
}
