//! Memory / speed of ONNX Runtime session settings (dev tool), one configuration per process:
//!
//!     cargo run --release --example mem -- <clap|ast> <arena 0|1> <device-initializers 0|1> <prepack 0|1>

use std::time::Instant;

use earshot::models::{self, ox};
use ort::session::Session;
use ort::value::Tensor;

fn rss_mb() -> f64 {
    let out =
        std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (which, arena, devinit, prepack) = (a[0].as_str(), a[1] == "1", a[2] == "1", a[3] == "1");
    let pattern = a.get(4).is_none_or(|v| v == "1");
    let level = match a.get(5).map(|s| s.as_str()) {
        Some("0") => ort::session::builder::GraphOptimizationLevel::Disable,
        Some("1") => ort::session::builder::GraphOptimizationLevel::Level1,
        Some("2") => ort::session::builder::GraphOptimizationLevel::Level2,
        _ => ort::session::builder::GraphOptimizationLevel::Level3,
    };
    let threads: usize = a.get(6).and_then(|v| v.parse().ok()).unwrap_or(4);
    let cache = models::default_cache_dir();
    let (repo, rev, file) = if which == "ast" {
        (models::AST_REPO, models::AST_REV, models::AST_MODEL)
    } else {
        (models::CLAP_REPO, models::CLAP_REV, models::CLAP_AUDIO)
    };
    let p = models::fetch(&cache, repo, rev, file, true)?;
    let env = ox(ort::init().with_name("mem").build())?;
    let base = rss_mb();
    let t = Instant::now();
    let b = ox(Session::builder(&env))?;
    let b = ox(b.with_execution_providers([ort::ep::CPU::default().with_arena_allocator(arena).build()]))?;
    let b = if devinit { ox(b.with_device_allocated_initializers())? } else { b };
    let b = ox(b.with_prepacking(prepack))?;
    let b = ox(b.with_memory_pattern(pattern))?;
    let b = ox(b.with_optimization_level(level))?;
    let mut b = ox(b.with_intra_threads(threads))?;
    let mut s = ox(b.commit_from_file(&p))?;
    let load = t.elapsed().as_secs_f64();
    let after_load = rss_mb();
    let mut times = vec![];
    for _ in 0..3 {
        let t = Instant::now();
        if which == "ast" {
            let x = ox(Tensor::from_array(([1usize, 1024, 128], vec![0.1f32; 1024 * 128])))?;
            let r = ox(s.run(ort::inputs!["input_values" => x]))?;
            let _ = ox(r["logits"].try_extract_tensor::<f32>())?;
        } else {
            let x = ox(Tensor::from_array(([1usize, 1, 1001, 64], vec![-20.0f32; 1001 * 64])))?;
            let r = ox(s.run(ort::inputs!["input_features" => x]))?;
            let _ = ox(r["audio_embeds"].try_extract_tensor::<f32>())?;
        }
        times.push(t.elapsed().as_secs_f64());
    }
    println!(
        "{which} arena={arena} devinit={devinit} prepack={prepack} pattern={pattern} level={level:?} threads={threads}: load {load:.2} s, rss after load {:.0} MB, after runs {:.0} MB, runs {:?}",
        after_load - base,
        rss_mb() - base,
        times.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>()
    );
    Ok(())
}
