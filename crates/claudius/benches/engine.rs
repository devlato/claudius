//! How long the engine's main steps take: `cargo bench -p claudius` (results in target/criterion).

use claudius::compose::{self, Options};
use claudius::{genres, render, script};
use criterion::{Criterion, criterion_group, criterion_main};

const EDIT: &str = r#"tempo(126); key("F minor"); drum("kick", "x...x...x...x..x"); mix("pad", #{ level: -4 });"#;

fn engine(c: &mut Criterion) {
    let g = genres::get("house").unwrap();
    let bp = compose::generate(&g, 1, &Options { short: true, ..Default::default() }, None);
    c.bench_function("generate a house song", |b| b.iter(|| compose::generate(&g, 1, &Options::default(), None)));
    c.bench_function("arrange a house song", |b| b.iter(|| compose::arrange(&bp)));
    let eight = compose::arrange(&bp).slice(0, 8);
    c.bench_function("render 8 bars of house", |b| {
        b.iter(|| render::render(&eight, &render::Options::default()).unwrap())
    });
    let dense = script::eval_pattern(r#"s("[bd*4, [~ cp]*2, hh*16?0.2]").every(4, |x| x.fast(2))"#).unwrap();
    c.bench_function("query a dense pattern for 64 bars", |b| b.iter(|| dense.events(0.0, 64.0)));
    c.bench_function("run a script edit", |b| b.iter(|| script::run(EDIT, Some(bp.clone()), None).unwrap()));
    c.bench_function("print a song as a script", |b| b.iter(|| script::to_script(&bp)));
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = engine
}
criterion_main!(benches);
