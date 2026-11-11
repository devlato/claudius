//! Needle 3 as claudius runs it (`needle`): its step gives needle-core's own logits, and a request read after another,
//! the cache rewound to the end of the tools, reads as it would on a model that read nothing before it. The model is
//! `needle3.cact` from Cactus Compute's Hugging Face repository, named by `CLAUDIUS_NEEDLE_MODEL`; without it these
//! are skipped.

use std::time::Instant;

use claudius_intent::needle::Needle;
use claudius_intent::teach::TOOLS;
use needle_core::v3::cache::{KvPrecision, V3Cache};
use needle_infer::v3_engine::V3Engine;

fn model() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("CLAUDIUS_NEEDLE_MODEL")?).ok()
}

#[test]
fn the_step_gives_needle_cores_logits() {
    let Some(bytes) = model() else {
        eprintln!("CLAUDIUS_NEEDLE_MODEL names no model: skipped");
        return;
    };
    for depth in [8, 20] {
        let mut ours = Needle::from_bytes(bytes.clone(), depth).unwrap();
        let ids: Vec<u32> = ours.prompt_ids("undo the last 2 changes", TOOLS).into_iter().take(48).collect();
        let theirs = V3Engine::from_bytes_with_depth(bytes.clone(), depth).unwrap();
        let mut cache = V3Cache::with_precision(&theirs.model.cfg, 1024, KvPrecision::F32);
        let want: Vec<Vec<f32>> = ids.iter().map(|&id| theirs.model.decode_step(&mut cache, id)).collect();
        let got = ours.logits_of(&ids);
        let worst = got
            .iter()
            .zip(&want)
            .flat_map(|(g, w)| g.iter().zip(w).map(|(a, b)| (a - b).abs() / b.abs().max(1.0)))
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "depth {depth}: off by {worst}");
    }
}

#[test]
fn a_request_after_another_reads_as_it_would_on_a_model_that_read_nothing_before() {
    let Some(bytes) = model() else {
        eprintln!("CLAUDIUS_NEEDLE_MODEL names no model: skipped");
        return;
    };
    let mut n = Needle::from_bytes(bytes.clone(), 8).unwrap();
    let tools = n.prompt_ids("", TOOLS).len();
    let started = Instant::now();
    let first = n.complete("undo the last 2 changes", TOOLS, 80);
    let read = started.elapsed();
    let started = Instant::now();
    let second = n.complete("mute the drums", TOOLS, 80);
    let each = started.elapsed();
    let mut fresh = Needle::from_bytes(bytes, 8).unwrap();
    assert_eq!(second, fresh.complete("mute the drums", TOOLS, 80));
    eprintln!("{tools} tokens of tools and a request in {read:?}, a request after them in {each:?}");
    eprintln!("first: {first:?}\nsecond: {second:?}");
}
