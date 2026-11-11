//! Teaching Needle 3 to read claudius's requests, in Rust, on the CPU: adapters (LoRA, rank 16) beside the five
//! projections of each block's attention, trained on the lessons of `claudius_intent::teach` against the weights the
//! app runs (`needle3.cact`, dequantized), with the tools read once by the model as it is, and written as the app's
//! step reads them (`claudius_intent::needle::Adapters`).
//!
//! ```text
//! claudius-tune train MODEL.cact OUT.safetensors [--depth 8] [--epochs 6] [--each 40] [--lr 2e-4] [--seed 7]
//! claudius-tune parity MODEL.cact [--depth 8]
//! claudius-tune check MODEL.cact ADAPTERS.safetensors
//! claudius-tune halve IN.safetensors OUT.safetensors
//! ```

#![forbid(unsafe_code)]

mod data;
mod model;
mod ops;
mod train;

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail, ensure};
use candle_core::DType;
use claudius_intent::teach::{TOOLS, lessons};
use needle_infer::prompt::build_prompt;
use needle_infer::v3_engine::V3Engine;

use model::{Lora, Model, Rng};
use train::{Adam, Ex, clip, grads, loss, rate};

const RANK: usize = 16;
const ALPHA: f64 = 32.0;
const BATCH: usize = 16;

/// `--name value` from the arguments after the positional ones
fn flag<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T> {
    match args.iter().position(|a| a == name) {
        Some(i) => args
            .get(i + 1)
            .ok_or_else(|| anyhow!("{name} wants a value"))?
            .parse()
            .map_err(|_| anyhow!("{name}: not a value it takes")),
        None => Ok(default),
    }
}

fn load(path: &str, depth: usize) -> Result<(Model, needle_infer::sp_tokenizer::SpTokenizer)> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {path}"))?;
    let engine = V3Engine::from_bytes_with_depth(bytes, depth).map_err(|e| anyhow!("{e}"))?;
    Ok((Model::new(engine.model, DType::F32)?, engine.tokenizer))
}

/// The adapters, in half precision as the app reads them, with what they were taught at and the fingerprint of the
/// tools they were taught with.
fn save(lora: &Lora, depth: usize, tools: &str, out: &Path) -> Result<()> {
    let tensors = lora.exported()?;
    let half = |d: &[f32]| d.iter().flat_map(|&x| half::f16::from_f32(x).to_le_bytes()).collect::<Vec<u8>>();
    let bytes: Vec<(String, Vec<usize>, Vec<u8>)> = tensors.into_iter().map(|(n, s, d)| (n, s, half(&d))).collect();
    let views = bytes
        .iter()
        .map(|(n, s, b)| Ok((n.clone(), safetensors::tensor::TensorView::new(safetensors::Dtype::F16, s.clone(), b)?)))
        .collect::<Result<Vec<_>>>()?;
    let meta: HashMap<String, String> = [
        ("depth", depth.to_string()),
        ("rank", lora.rank.to_string()),
        ("scale", lora.scale.to_string()),
        ("tools", tools.to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let tmp = out.with_extension("part");
    safetensors::tensor::serialize_to_file(views, Some(meta), &tmp)?;
    std::fs::rename(&tmp, out)?;
    Ok(())
}

fn train(args: &[String]) -> Result<()> {
    let (Some(model_path), Some(out)) = (args.first(), args.get(1)) else {
        bail!(
            "claudius-tune train MODEL.cact OUT.safetensors [--depth 8] [--epochs 6] [--each 40] [--lr 2e-4] [--seed 7]"
        );
    };
    let depth: usize = flag(args, "--depth", 8)?;
    let epochs: usize = flag(args, "--epochs", 6)?;
    let each: usize = flag(args, "--each", 40)?;
    let peak: f64 = flag(args, "--lr", 2e-4)?;
    let seed: u64 = flag(args, "--seed", 7)?;
    let started = Instant::now();
    let (model, tok) = load(model_path, depth)?;
    let prefix = data::prefix_ids(&tok);
    let pre = model.prefix(&prefix)?;
    let all = lessons(seed, each);
    let (mut taught, mut held): (Vec<Ex>, Vec<Ex>) = (Vec::new(), Vec::new());
    for (i, l) in all.iter().enumerate() {
        let ex = data::example(&tok, l, &prefix)?;
        if i % 10 == 9 { held.push(ex) } else { taught.push(ex) }
    }
    let lora = Lora::new(depth, RANK, ALPHA, seed, 0.0, DType::F32)?;
    let vars = lora.vars();
    let mut opt = Adam::new(&vars, 1e-4)?;
    let per_epoch = taught.len().div_ceil(BATCH);
    let total = per_epoch * epochs;
    eprintln!(
        "depth {depth}: {} lessons ({} taught, {} held), {} tokens of tools, {} adapter parameters; {total} steps; ready in {:.1?}",
        all.len(),
        taught.len(),
        held.len(),
        prefix.len(),
        lora.params(),
        started.elapsed()
    );
    let mut rng = Rng(seed ^ 0x5eed);
    let mut step = 0;
    let t0 = Instant::now();
    for epoch in 1..=epochs {
        for i in (1..taught.len()).rev() {
            taught.swap(i, rng.below(i + 1));
        }
        let mut running = 0.0;
        for (b, batch) in taught.chunks(BATCH).enumerate() {
            let (l, mut g) = grads(&model, &pre, &lora, batch)?;
            let norm = clip(&mut g, 1.0)?;
            let lr = rate(step, total, peak);
            opt.step(&vars, &g, lr)?;
            step += 1;
            running += l;
            if step % 20 == 0 || b + 1 == per_epoch {
                let per = t0.elapsed() / step as u32;
                eprintln!(
                    "epoch {epoch} step {step}/{total}: loss {:.4} (grad norm {norm:.2}, lr {lr:.2e}); {per:.2?} a step, {:.0?} to go",
                    running / (b + 1) as f64,
                    per * (total - step) as u32
                );
            }
        }
        let val = loss(&model, &pre, &lora, &held)?;
        save(&lora, depth, &claudius_intent::needle::fingerprint(TOOLS), Path::new(out))?;
        eprintln!(
            "epoch {epoch}: held-out loss {val:.4}; adapters written to {out} ({:.1?} in all)",
            started.elapsed()
        );
    }
    // a few lessons it was not taught, read with the adapters
    for ex in held.iter().step_by((held.len() / 4).max(1)).take(4) {
        let asked = ex.mask.iter().position(|&m| m > 0.0).unwrap_or(ex.suffix.len());
        let read = train::generate(&model, &pre, Some(&lora), &ex.suffix[..asked], 100)?;
        eprintln!("  {:?}\n    -> {:?}", tok.decode(&ex.suffix[..asked]), tok.decode(&read));
    }
    Ok(())
}

/// The candle model against needle-core's own logits, every position of a prompt: what the adapters are taught on is
/// what the app runs.
fn parity(args: &[String]) -> Result<()> {
    let Some(model_path) = args.first() else { bail!("claudius-tune parity MODEL.cact [--depth 8]") };
    let depth: usize = flag(args, "--depth", 8)?;
    let (model, tok) = load(model_path, depth)?;
    let mut ids = vec![data::BOS];
    ids.extend(tok.encode(&build_prompt("turn the bass down by 3 db", TOOLS, None)));
    let want = model.core.forward_sequence(&ids);
    let lora = Lora::new(depth, RANK, ALPHA, 7, 0.0, DType::F32)?;
    let bt = model.batch(std::slice::from_ref(&ids), 0, None, None, None)?;
    let x = model.forward(&bt, None, Some(&lora))?;
    let got: Vec<f32> = model.logits(&x, None)?.flatten_all()?.to_vec1()?;
    ensure!(got.len() == want.len(), "{} logits against {}", got.len(), want.len());
    let rms = (want.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>() / want.len() as f64).sqrt();
    let worst = got.iter().zip(&want).map(|(a, b)| f64::from((a - b).abs())).fold(0.0, f64::max);
    println!("depth {depth}, {} positions: the most any logit differs is {:.2e} of their RMS", ids.len(), worst / rms);
    ensure!(worst / rms < 1e-3, "the candle model is not needle-core's");
    Ok(())
}

/// The adapters in a file, read back into the trainer's model, against the app's step with the same file: the logits
/// after a request, which must agree.
fn check(args: &[String]) -> Result<()> {
    let (Some(model_path), Some(file)) = (args.first(), args.get(1)) else {
        bail!("claudius-tune check MODEL.cact ADAPTERS.safetensors")
    };
    let bytes = std::fs::read(file)?;
    let adapters = claudius_intent::needle::Adapters::from_safetensors(bytes.clone()).map_err(|e| anyhow!(e))?;
    let depth = adapters.depth;
    let (model, tok) = load(model_path, depth)?;
    let lora = Lora::from_exported(&adapters, DType::F32)?;
    let mut needle =
        claudius_intent::needle::Needle::from_bytes(std::fs::read(model_path)?, depth).map_err(|e| anyhow!(e))?;
    needle.adapt(adapters).map_err(|e| anyhow!(e))?;
    let prefix = data::prefix_ids(&tok);
    let pre = model.prefix(&prefix)?;
    for query in ["undo", "make it faster", "mute the drums"] {
        let mut ids = vec![data::BOS];
        ids.extend(tok.encode(&build_prompt(query, TOOLS, None)));
        let suffix = ids[prefix.len()..].to_vec();
        let bt = model.batch(std::slice::from_ref(&suffix), pre.len, Some(&pre), None, None)?;
        let x = model.forward(&bt, Some(&pre), Some(&lora))?;
        let last = x.narrow(0, suffix.len() - 1, 1)?;
        let want: Vec<f32> = model.logits(&last, None)?.flatten_all()?.to_vec1()?;
        let got = needle.logits_after(query, TOOLS);
        let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        let top = |v: &[f32]| v.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(0, |(i, _)| i);
        println!("{query:?}: the most a logit differs is {worst:.3e}; top tokens {} and {}", top(&want), top(&got));
        let asked = &suffix;
        let theirs = tok.decode(&train::generate(&model, &pre, Some(&lora), asked, 80)?);
        let fresh = {
            let mut n = claudius_intent::needle::Needle::from_bytes(std::fs::read(model_path)?, depth)
                .map_err(|e| anyhow!(e))?;
            n.adapt(claudius_intent::needle::Adapters::from_safetensors(bytes.clone()).map_err(|e| anyhow!(e))?)
                .map_err(|e| anyhow!(e))?;
            n.complete(query, TOOLS, 80)
        };
        let after = needle.complete(query, TOOLS, 80);
        println!("  trainer: {theirs:?}\n  fresh:   {fresh:?}\n  after:   {after:?}");
    }
    Ok(())
}

/// Adapters written in full precision, as claudius-tune wrote them before, again in half.
fn halve(args: &[String]) -> Result<()> {
    let (Some(file), Some(out)) = (args.first(), args.get(1)) else {
        bail!("claudius-tune halve IN.safetensors OUT.safetensors")
    };
    let adapters = claudius_intent::needle::Adapters::from_safetensors(std::fs::read(file)?).map_err(|e| anyhow!(e))?;
    let lora = Lora::from_exported(&adapters, DType::F32)?;
    save(&lora, adapters.depth, &adapters.tools, Path::new(out))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("train") => train(&args[1..]),
        Some("parity") => parity(&args[1..]),
        Some("check") => check(&args[1..]),
        Some("halve") => halve(&args[1..]),
        _ => bail!(
            "claudius-tune train MODEL.cact OUT.safetensors [...] | parity MODEL.cact [--depth 8] | check MODEL.cact \
             ADAPTERS.safetensors | halve IN.safetensors OUT.safetensors"
        ),
    }
}
