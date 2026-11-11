//! A training step, the optimiser, the learning rate's schedule and greedy decoding over the candle model.

use anyhow::Result;
use candle_core::{DType, Tensor, Var};
use rayon::prelude::*;

use crate::data::{EOS, IM_END};
use crate::model::{Lora, Model, Prefix};

/// One example as the trainer sees it: the tokens after the tools prefix, and which of them carry the loss.
#[derive(Clone)]
pub struct Ex {
    pub suffix: Vec<u32>,
    pub mask: Vec<f32>,
}

impl Ex {
    pub fn ntarget(&self) -> usize {
        (1..self.suffix.len()).filter(|&i| self.mask[i] > 0.0).count()
    }
}

fn scalar(t: &Tensor) -> Result<f64> {
    Ok(t.to_dtype(DType::F64)?.to_scalar::<f64>()?)
}

/// A variable's gradient, detached: candle leaves a leaf's accumulated gradient attached to the forward graph, so
/// keeping it (in a sum, in the optimiser's moments) would keep every example's activations alive.
fn grad_of(grads: &candle_core::backprop::GradStore, v: &Var) -> Result<Tensor> {
    Ok(grads.get(v.as_tensor()).ok_or_else(|| anyhow::anyhow!("a LoRA matrix got no gradient"))?.detach())
}

/// how many examples' graphs the parallel step keeps alive at once (work stealing would otherwise keep all of them)
const CHUNK: usize = 8;

/// The batch's mean cross-entropy over its loss tokens and its gradient, one graph per example on the thread pool,
/// the per-example gradients summed: candle's CPU elementwise ops are single-threaded, and running the examples side
/// by side is what uses the cores.
pub fn grads(model: &Model, pre: &Prefix, lora: &Lora, exs: &[Ex]) -> Result<(f64, Vec<Tensor>)> {
    let total = exs.iter().map(Ex::ntarget).sum::<usize>() as f64;
    let vars = lora.vars();
    let mut loss = 0.0;
    let mut sum: Vec<Tensor> = Vec::new();
    for part in exs.chunks(CHUNK) {
        let parts = part
            .par_iter()
            .map(|e| -> Result<(f64, Vec<Tensor>)> {
                let bt = model.batch(
                    std::slice::from_ref(&e.suffix),
                    pre.len,
                    Some(pre),
                    Some(std::slice::from_ref(&e.mask)),
                    None,
                )?;
                let x = model.forward(&bt, Some(pre), Some(lora))?;
                let loss = model.loss_sum(&x, &bt)?.affine(1.0 / total, 0.0)?;
                let grads = loss.backward()?;
                let g = vars.iter().map(|v| grad_of(&grads, v)).collect::<Result<Vec<_>>>()?;
                Ok((scalar(&loss)?, g))
            })
            .collect::<Result<Vec<_>>>()?;
        for (l, g) in parts {
            loss += l;
            sum = if sum.is_empty() {
                g
            } else {
                sum.iter().zip(&g).map(|(a, b)| a.add(b)).collect::<candle_core::Result<_>>()?
            };
        }
    }
    Ok((loss, sum))
}

/// The mean cross-entropy of `exs` over their loss tokens, without gradients.
pub fn loss(model: &Model, pre: &Prefix, lora: &Lora, exs: &[Ex]) -> Result<f64> {
    let total = exs.iter().map(Ex::ntarget).sum::<usize>() as f64;
    let parts = exs
        .par_iter()
        .map(|e| -> Result<f64> {
            let bt = model.batch(
                std::slice::from_ref(&e.suffix),
                pre.len,
                Some(pre),
                Some(std::slice::from_ref(&e.mask)),
                None,
            )?;
            let x = model.forward(&bt, Some(pre), Some(lora))?;
            scalar(&model.loss_sum(&x, &bt)?.detach())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(parts.iter().sum::<f64>() / total.max(1.0))
}

/// Scale the gradients to a global norm of at most `max`; returns the norm before clipping.
pub fn clip(grads: &mut [Tensor], max: f64) -> Result<f64> {
    let mut sq = 0.0;
    for g in grads.iter() {
        sq += scalar(&g.sqr()?.sum_all()?)?;
    }
    let norm = sq.sqrt();
    if norm > max {
        for g in grads.iter_mut() {
            *g = g.affine(max / norm, 0.0)?;
        }
    }
    Ok(norm)
}

/// The learning rate at `step` of `total`: up from zero over the first twentieth, then down a cosine to zero, as
/// optax's `warmup_cosine_decay_schedule` that Cactus's own fine-tuning uses.
pub fn rate(step: usize, total: usize, peak: f64) -> f64 {
    let warmup = (total / 20).max(1);
    if step < warmup {
        return peak * step as f64 / warmup as f64;
    }
    let done = (step - warmup) as f64 / (total - warmup).max(1) as f64;
    peak * 0.5 * (1.0 + (std::f64::consts::PI * done.min(1.0)).cos())
}

/// AdamW with decoupled weight decay, as optax's `adamw` and candle's `AdamW`.
pub struct Adam {
    m: Vec<Tensor>,
    v: Vec<Tensor>,
    t: i32,
    b1: f64,
    b2: f64,
    eps: f64,
    wd: f64,
}

impl Adam {
    pub fn new(vars: &[Var], wd: f64) -> Result<Adam> {
        let zeros = || vars.iter().map(|v| v.as_tensor().zeros_like()).collect::<candle_core::Result<Vec<_>>>();
        Ok(Adam { m: zeros()?, v: zeros()?, t: 0, b1: 0.9, b2: 0.999, eps: 1e-8, wd })
    }

    pub fn step(&mut self, vars: &[Var], grads: &[Tensor], lr: f64) -> Result<()> {
        self.t += 1;
        let (c1, c2) = (1.0 - self.b1.powi(self.t), 1.0 - self.b2.powi(self.t));
        for (i, (var, g)) in vars.iter().zip(grads).enumerate() {
            let m = self.m[i].affine(self.b1, 0.0)?.add(&g.affine(1.0 - self.b1, 0.0)?)?;
            let v = self.v[i].affine(self.b2, 0.0)?.add(&g.sqr()?.affine(1.0 - self.b2, 0.0)?)?;
            let update = m.affine(1.0 / c1, 0.0)?.div(&v.affine(1.0 / c2, 0.0)?.sqrt()?.affine(1.0, self.eps)?)?;
            let theta = var.as_tensor().detach().affine(1.0 - lr * self.wd, 0.0)?.sub(&update.affine(lr, 0.0)?)?;
            var.set(&theta)?;
            self.m[i] = m;
            self.v[i] = v;
        }
        Ok(())
    }
}

/// Greedy decoding after the prefix: feed `prompt` (the request and the assistant header), then take the most likely
/// token until EOS or `<|im_end|>`. Recomputes the whole run each step, which is fine for a check.
pub fn generate(model: &Model, pre: &Prefix, lora: Option<&Lora>, prompt: &[u32], max_new: usize) -> Result<Vec<u32>> {
    let mut run = prompt.to_vec();
    let mut out = Vec::new();
    for _ in 0..max_new {
        let bt = model.batch(std::slice::from_ref(&run), pre.len, Some(pre), None, None)?;
        let x = model.forward(&bt, Some(pre), lora)?;
        let last = x.narrow(0, run.len() - 1, 1)?;
        let logits: Vec<f32> = model.logits(&last, None)?.flatten_all()?.to_dtype(DType::F32)?.to_vec1()?;
        let next = logits.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(EOS, |(i, _)| i as u32);
        out.push(next);
        if next == EOS || next == IM_END {
            break;
        }
        run.push(next);
    }
    Ok(out)
}
