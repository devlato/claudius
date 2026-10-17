//! Decode files with symphonia and with ffmpeg and compare them: `cargo run -p claudius-decode --example compare -- FILE...`

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

fn main() {
    for arg in std::env::args().skip(1) {
        let p = std::path::Path::new(&arg);
        let t = std::time::Instant::now();
        match claudius_decode::decode_native(p) {
            Ok(d) => {
                let secs = d.frames() as f64 / d.rate as f64;
                let (l, _) = d.stereo();
                println!(
                    "{}: {} Hz, {} ch, {:.3} s, rms {:.4}, decoded in {:.0} ms",
                    p.display(),
                    d.rate,
                    d.channels,
                    secs,
                    rms(&l),
                    t.elapsed().as_secs_f64() * 1e3
                );
                if let Some(ff) = claudius_decode::ffmpeg() {
                    let out = std::process::Command::new(ff)
                        .args(["-v", "error", "-i"])
                        .arg(p)
                        .args(["-f", "f32le", "-ac", "2", "-"])
                        .output()
                        .unwrap();
                    let x: Vec<f32> =
                        out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                    let fl: Vec<f32> = x.chunks_exact(2).map(|f| f[0]).collect();
                    println!("   ffmpeg: {:.3} s, rms {:.4}", fl.len() as f64 / d.rate as f64, rms(&fl));
                }
            }
            Err(e) => println!("{}: {e}", p.display()),
        }
    }
}
