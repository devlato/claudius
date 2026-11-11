//! Needle 3 in candle: needle-core's forward pass over the dequantized `.cact` weights of a ladder rung, with LoRA
//! beside the five attention projections of every block.
//!
//! The model runs over a batch of token runs that start at the same absolute position. A run either starts at 0, or
//! continues a frozen [`Prefix`]: the tools part of the prompt, run once without LoRA, whose keys and values every
//! example attends over, and whose last few positions are the history of the Q/K/V and Engram convolutions.

use anyhow::{Result, ensure};
use candle_core::{D, DType, Device, Tensor, Var};
use needle_core::cq::CqWeight;
use needle_core::v3::V3Model;
use needle_core::v3::engram::{engram_indices, ngram_valid};
use rayon::prelude::*;

use crate::ops::{CausalSoftmax, Frozen};

pub const DM: usize = 768;
const LANES: usize = 4;
const HADA: usize = 1024;
const BLOCK: usize = 32;
/// Positions before the current one that the Q/K/V convolution reads: `qkv_conv_taps - 1`.
const CONV_HIST: usize = 2;
/// Raw Engram values before the current position the dilated value conv reads: `(conv_taps - 1) * dilation`.
const ENGRAM_REACH: usize = 9;
/// Output widths of q, k, v, gate and out, the projections LoRA sits beside.
pub const OUTS: [usize; 5] = [576, 96, 128, 768, 768];
/// the projections' names, as the adapters' file has them
pub const NAMES: [&str; 5] = ["q", "k", "v", "gate", "out"];

/// the Engram of a batch, per site: its keys, its convolved values, and its raw values
type Engram = (Vec<Tensor>, Vec<Tensor>, Vec<Vec<f32>>);

/// an adapter matrix as the app reads it: its name, its shape and its values, row-major
pub type Exported = (String, Vec<usize>, Vec<f32>);

fn dense(w: &CqWeight) -> Vec<f32> {
    let mut v = vec![0f32; w.out_feat * w.in_feat];
    w.dequantize_to(&mut v);
    v
}

fn inv_rms(x: &Tensor) -> candle_core::Result<Tensor> {
    x.sqr()?.mean_keepdim(D::Minus1)?.affine(1.0, 1e-6)?.sqrt()?.recip()
}

/// `x / rms(x)`, the scale-free norm of the mHC router and the Engram gate.
fn rms_unit(x: &Tensor) -> candle_core::Result<Tensor> {
    x.broadcast_mul(&inv_rms(x)?)
}

/// ZCRMSNorm, with `one_plus` holding `1 + scale`.
fn zcrms(x: &Tensor, one_plus: &Tensor) -> candle_core::Result<Tensor> {
    x.broadcast_mul(&inv_rms(x)?)?.broadcast_mul(one_plus)
}

fn sigmoid(x: &Tensor) -> candle_core::Result<Tensor> {
    candle_nn::ops::sigmoid(x)
}

fn softmax_last(x: &Tensor) -> candle_core::Result<Tensor> {
    let e = x.broadcast_sub(&x.max_keepdim(D::Minus1)?.detach())?.exp()?;
    e.broadcast_div(&e.sum_keepdim(D::Minus1)?)
}

fn logsumexp(x: &Tensor, dim: usize) -> candle_core::Result<Tensor> {
    let max = x.max_keepdim(dim)?.detach();
    x.broadcast_sub(&max)?.exp()?.sum_keepdim(dim)?.log()?.broadcast_add(&max)
}

/// Twenty log-domain Sinkhorn iterations over `[n, 4, 4]`: rows, then columns, then `exp`.
fn sinkhorn(m: &Tensor) -> candle_core::Result<Tensor> {
    let mut m = m.clone();
    for _ in 0..20 {
        m = m.broadcast_sub(&logsumexp(&m, 2)?)?;
        m = m.broadcast_sub(&logsumexp(&m, 1)?)?;
    }
    m.exp()
}

/// `aᵀ · Z · b` per row, with `Z` the row as a 32×32 row-major matrix.
fn kron(z: &Tensor, a: &Tensor, b: &Tensor) -> candle_core::Result<Tensor> {
    let n = z.dim(0)?;
    let t = z.reshape((n * BLOCK, BLOCK))?.matmul(b)?;
    let t = t.reshape((n, BLOCK, BLOCK))?.transpose(1, 2)?.contiguous()?.reshape((n * BLOCK, BLOCK))?;
    let o = t.matmul(a)?;
    o.reshape((n, BLOCK, BLOCK))?.transpose(1, 2)?.contiguous()?.reshape((n, HADA))
}

/// The causal depthwise convolution over `[b, s, c]`, three taps, reading `hist` (`[2, c]`, oldest first) before the
/// first position, or zeros when the run starts the sequence.
fn conv(z: &Tensor, taps: &Tensor, hist: Option<&Tensor>) -> candle_core::Result<Tensor> {
    let (b, s, c) = z.dims3()?;
    let h = match hist {
        Some(h) => h.unsqueeze(0)?.broadcast_as((b, CONV_HIST, c))?,
        None => Tensor::zeros((b, CONV_HIST, c), z.dtype(), z.device())?,
    };
    let ext = Tensor::cat(&[&h, z], 1)?;
    let acc = ext.narrow(1, 2, s)?.broadcast_mul(&taps.get(0)?)?;
    let acc = acc.add(&ext.narrow(1, 1, s)?.broadcast_mul(&taps.get(1)?)?)?;
    acc.add(&ext.narrow(1, 0, s)?.broadcast_mul(&taps.get(2)?)?)
}

/// Half-split RoPE over `[b, s, heads, 48]`, with `cos`/`sin` shaped `[1, s, 1, 24]`.
fn rope(x: &Tensor, cos: &Tensor, sin: &Tensor) -> candle_core::Result<Tensor> {
    let half = x.dim(3)? / 2;
    let (x1, x2) = (x.narrow(3, 0, half)?, x.narrow(3, half, half)?);
    let r1 = x1.broadcast_mul(cos)?.sub(&x2.broadcast_mul(sin)?)?;
    let r2 = x2.broadcast_mul(cos)?.add(&x1.broadcast_mul(sin)?)?;
    Tensor::cat(&[&r1, &r2], 3)
}

struct Mlp {
    d1: Tensor,
    d2: Tensor,
    b2: Tensor,
    d3: Tensor,
    d4: Tensor,
    w: [(Tensor, Tensor); 3],
    cond_v: Tensor,
    cond_u: Tensor,
}

struct Layer {
    norm_in: Tensor,
    /// q, k, v, gate, out, each `[in, out]`.
    proj: [Tensor; 5],
    taps: [Tensor; 3],
    q_norm: Tensor,
    k_norm: Tensor,
    post_norm: Tensor,
    pre_hada: Tensor,
    gate: f64,
    mlp: Mlp,
    a: [f64; 3],
    b_pre: Tensor,
    b_post: Tensor,
    b_res: Tensor,
    pre_off: Tensor,
    post_off: Tensor,
    phi_pre: Tensor,
    phi_post: Tensor,
    phi_res: Tensor,
    global: bool,
    site: Option<usize>,
}

struct Site {
    key: Tensor,
    value: Tensor,
    taps: Vec<f32>,
}

/// LoRA beside the five projections of every block: `x·W + scale·(x·A)·B`, A `[768, r]`, B `[r, out]`.
pub struct Lora {
    pub a: Vec<[Var; 5]>,
    pub b: Vec<[Var; 5]>,
    pub rank: usize,
    pub scale: f64,
}

/// SplitMix64 with Box–Muller, so an adapter's initialisation is reproducible.
pub struct Rng(pub u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn normal(&mut self) -> f64 {
        let u = self.uniform().max(1e-300);
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * self.uniform()).cos()
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

impl Lora {
    /// A ~ N(0, 1)/rank as upstream's `init_lora`; B ~ N(0, b_std), which is zero for training.
    pub fn new(layers: usize, rank: usize, alpha: f64, seed: u64, b_std: f64, dt: DType) -> Result<Lora> {
        let dev = Device::Cpu;
        let mut rng = Rng(seed);
        let mut var = |rows: usize, cols: usize, std: f64| -> Result<Var> {
            let v: Vec<f32> = (0..rows * cols).map(|_| (rng.normal() * std) as f32).collect();
            Ok(Var::from_tensor(&Tensor::from_vec(v, (rows, cols), &dev)?.to_dtype(dt)?)?)
        };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for _ in 0..layers {
            let mut la = Vec::new();
            let mut lb = Vec::new();
            for out in OUTS {
                la.push(var(DM, rank, 1.0 / rank as f64)?);
                lb.push(var(rank, out, b_std)?);
            }
            a.push(la.try_into().map_err(|_| anyhow::anyhow!("five projections"))?);
            b.push(lb.try_into().map_err(|_| anyhow::anyhow!("five projections"))?);
        }
        Ok(Lora { a, b, rank, scale: alpha / rank as f64 })
    }

    /// The adapters the app reads (`exported`), back as the trainer's: A and B transposed again.
    pub fn from_exported(adapters: &claudius_intent::needle::Adapters, dt: DType) -> Result<Lora> {
        let dev = Device::Cpu;
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let mut rank = 0;
        for block in &adapters.blocks {
            let pairs = [&block.q, &block.k, &block.v, &block.gate, &block.out];
            let mut la = Vec::new();
            let mut lb = Vec::new();
            for (pair, out) in pairs.into_iter().zip(OUTS) {
                rank = pair.rank;
                let at = Tensor::from_vec(pair.a.clone(), (pair.rank, DM), &dev)?.t()?.contiguous()?.to_dtype(dt)?;
                let bt = Tensor::from_vec(pair.b.clone(), (out, pair.rank), &dev)?.t()?.contiguous()?.to_dtype(dt)?;
                la.push(Var::from_tensor(&at)?);
                lb.push(Var::from_tensor(&bt)?);
            }
            a.push(la.try_into().map_err(|_| anyhow::anyhow!("five projections"))?);
            b.push(lb.try_into().map_err(|_| anyhow::anyhow!("five projections"))?);
        }
        Ok(Lora { a, b, rank, scale: f64::from(adapters.scale) })
    }

    pub fn vars(&self) -> Vec<Var> {
        self.a.iter().chain(&self.b).flat_map(|v| v.iter().cloned()).collect()
    }

    pub fn params(&self) -> usize {
        self.vars().iter().map(|v| v.elem_count()).sum()
    }

    /// Every adapter as the app's step reads it (`claudius_intent::needle::Pair`): named `b{block}.{proj}.a` and
    /// `.b`, `a` rank × in and `b` out × rank, row-major, which are this one's A and B transposed.
    pub fn exported(&self) -> Result<Vec<Exported>> {
        let mut out = Vec::new();
        for (block, (a, b)) in self.a.iter().zip(&self.b).enumerate() {
            for (i, name) in NAMES.iter().enumerate() {
                for (m, t) in [("a", a[i].as_tensor()), ("b", b[i].as_tensor())] {
                    let t = t.t()?.contiguous()?.to_dtype(DType::F32)?;
                    let shape = t.dims().to_vec();
                    out.push((format!("b{block}.{name}.{m}"), shape, t.flatten_all()?.to_vec1::<f32>()?));
                }
            }
        }
        Ok(out)
    }
}

/// The frozen tools prefix: per block its keys (post-conv, normed, rotated) and values (post-conv), and the
/// history the convolutions of the next positions read.
pub struct Prefix {
    pub len: usize,
    pub tokens: Vec<u32>,
    k: Vec<Tensor>,
    v: Vec<Tensor>,
    /// The last two pre-conv q, k and v projections, per block.
    tail: Vec<[Tensor; 3]>,
    /// The last nine pre-conv Engram values, per site.
    engram_tail: Vec<Vec<f32>>,
}

#[derive(Default)]
struct Capture {
    k: Vec<Tensor>,
    v: Vec<Tensor>,
    tail: Vec<[Tensor; 3]>,
}

/// One batch of runs padded to a common length `s`, starting at absolute position `start`, with everything that does
/// not depend on the adapter already computed.
pub struct Batch {
    pub b: usize,
    pub s: usize,
    pub start: usize,
    emb: Tensor,
    ek: Vec<Tensor>,
    ev: Vec<Tensor>,
    raw: Vec<Vec<f32>>,
    cos: Tensor,
    sin: Tensor,
    /// `[n, 1]`, 1 where LoRA applies: lets a full-sequence run leave the prefix positions without LoRA.
    lora_rows: Option<Tensor>,
    /// Rows whose next token carries the loss, and those tokens.
    pub rows: Option<Tensor>,
    pub targets: Option<Tensor>,
}

pub struct Model {
    pub core: V3Model,
    pub dt: DType,
    dev: Device,
    emb: Tensor,
    emb_t: Tensor,
    layers: Vec<Layer>,
    sites: Vec<Site>,
    final_norm: Tensor,
    p1: Tensor,
    p2: Tensor,
}

impl Model {
    /// Dequantize the rung `core` holds (from `model_from_cact_at_depth`) into candle tensors of `dt`.
    pub fn new(core: V3Model, dt: DType) -> Result<Model> {
        let dev = Device::Cpu;
        let cfg = core.cfg.clone();
        ensure!(cfg.d_model == DM && cfg.hada_n == HADA && cfg.mhc_lanes == LANES, "unexpected geometry");
        ensure!(cfg.qkv_conv_taps == CONV_HIST + 1, "unexpected conv width");
        ensure!((cfg.engram.conv_taps - 1) * cfg.engram.conv_dilation == ENGRAM_REACH, "unexpected engram conv");
        let vt =
            |v: Vec<f32>, shape: &[usize]| -> Result<Tensor> { Ok(Tensor::from_vec(v, shape, &dev)?.to_dtype(dt)?) };
        let lin = |w: &CqWeight| -> Result<Tensor> {
            Ok(Tensor::from_vec(dense(w), (w.out_feat, w.in_feat), &dev)?.t()?.contiguous()?.to_dtype(dt)?)
        };
        let plus1 = |v: &[f32]| -> Result<Tensor> { vt(v.iter().map(|x| 1.0 + x).collect(), &[v.len()]) };
        let all = |w: &CqWeight| -> Result<Tensor> { Ok(Tensor::from_vec(dense(w), (w.out_feat, w.in_feat), &dev)?) };
        let (phi_pre, phi_post, phi_res) = (all(&core.mhc.phi_pre)?, all(&core.mhc.phi_post)?, all(&core.mhc.phi_res)?);
        let n = LANES;
        let mut layers = Vec::new();
        for (li, l) in core.layers.iter().enumerate() {
            let lane = li % n;
            let rows = |m: &Tensor, w: usize| -> Result<Tensor> {
                Ok(m.narrow(0, li * w, w)?.t()?.contiguous()?.to_dtype(dt)?)
            };
            let taps = |t: &[f32]| vt(t.to_vec(), &[CONV_HIST + 1, t.len() / (CONV_HIST + 1)]);
            let m = &l.mlp;
            let pair = |p: &(Vec<f32>, Vec<f32>)| -> Result<(Tensor, Tensor)> {
                Ok((vt(p.0.clone(), &[BLOCK, BLOCK])?, vt(p.1.clone(), &[BLOCK, BLOCK])?))
            };
            let mlp = Mlp {
                d1: vt(m.d1.clone(), &[HADA])?,
                d2: vt(m.d2.clone(), &[HADA])?,
                b2: vt(m.b2.clone(), &[HADA])?,
                d3: vt(m.d3.clone(), &[HADA])?,
                d4: vt(m.d4.clone(), &[HADA])?,
                w: [pair(&m.w1)?, pair(&m.w2)?, pair(&m.w3)?],
                cond_v: vt(m.cond_v.clone(), &[DM, m.cond_rank])?,
                cond_u: vt(m.cond_u.clone(), &[m.cond_rank, HADA])?,
            };
            let mhc = &core.mhc;
            layers.push(Layer {
                norm_in: plus1(&l.norm_in)?,
                proj: [lin(&l.q_proj)?, lin(&l.k_proj)?, lin(&l.v_proj)?, lin(&l.gate_proj)?, lin(&l.out_proj)?],
                taps: [taps(&l.q_taps)?, taps(&l.k_taps)?, taps(&l.v_taps)?],
                q_norm: plus1(&l.q_norm)?,
                k_norm: plus1(&l.k_norm)?,
                post_norm: plus1(&l.post_norm)?,
                pre_hada: plus1(&l.pre_hada)?,
                gate: (1.0 / (1.0 + libm::expf(-l.attn_gate))) as f64,
                mlp,
                a: [mhc.a_pre[li] as f64, mhc.a_post[li] as f64, mhc.a_res[li] as f64],
                b_pre: vt(mhc.b_pre[li * n..(li + 1) * n].to_vec(), &[n])?,
                b_post: vt(mhc.b_post[li * n..(li + 1) * n].to_vec(), &[n])?,
                b_res: vt(mhc.b_res[li * n * n..(li + 1) * n * n].to_vec(), &[n * n])?,
                pre_off: vt((0..n).map(|i| if i == lane { 4.0 } else { -4.0 }).collect(), &[n])?,
                post_off: vt((0..n).map(|i| if i == lane { 0.0 } else { -4.0 }).collect(), &[n])?,
                phi_pre: rows(&phi_pre, n)?,
                phi_post: rows(&phi_post, n)?,
                phi_res: rows(&phi_res, n * n)?,
                global: cfg.is_global(li),
                site: cfg.engram.site_of(li),
            });
        }
        let sites = core
            .engrams
            .iter()
            .map(|s| -> Result<Site> {
                // Kept in f32 whatever `dt` is: the Engram is adapter-independent and computed before the graph.
                let t = |w: &CqWeight| -> Result<Tensor> {
                    Ok(Tensor::from_vec(dense(w), (DM, DM), &dev)?.t()?.contiguous()?)
                };
                Ok(Site { key: t(&s.key_proj)?, value: t(&s.value_proj)?, taps: s.taps.clone() })
            })
            .collect::<Result<Vec<_>>>()?;
        let e = Tensor::from_vec(dense(&core.embedding), (core.embedding.out_feat, DM), &dev)?;
        let emb_t = e.narrow(0, 0, cfg.logit_rows())?.t()?.contiguous()?.to_dtype(dt)?;
        let emb = e.to_dtype(dt)?;
        let perm = |p: &[u32]| -> Result<Tensor> { Ok(Tensor::from_vec(p.to_vec(), p.len(), &dev)?) };
        let (p1, p2, final_norm) = (perm(&core.perms.p1)?, perm(&core.perms.p2)?, plus1(&core.final_norm)?);
        Ok(Model { core, dt, dev, emb, emb_t, layers, sites, final_norm, p1, p2 })
    }

    /// Engram keys (rms-normalised) and convolved values, `[n, 768]` per site, for runs padded to `s` at absolute
    /// positions `start..`. With a prefix, its last tokens are the n-gram history and its raw values the conv's.
    fn engram(&self, seqs: &[Vec<u32>], s: usize, start: usize, pre: Option<&Prefix>) -> Result<Engram> {
        let e = &self.core.cfg.engram;
        let nt = e.num_tables();
        let max_order = e.orders.iter().copied().max().unwrap_or(1);
        let hist: &[u32] = match pre {
            Some(p) => &p.tokens[p.tokens.len() - (max_order - 1)..],
            None => &[],
        };
        let n = seqs.len() * s;
        let mut idx = vec![0u32; n * nt];
        for (b, run) in seqs.iter().enumerate() {
            let mut t = hist.to_vec();
            t.extend_from_slice(run);
            t.resize(hist.len() + s, 0);
            let all = engram_indices(&t, &e.orders, e.heads, e.slots as u32, e.seed_heads);
            idx[b * s * nt..(b + 1) * s * nt].copy_from_slice(&all[hist.len() * nt..]);
        }
        let (mut ks, mut vs, mut raws) = (Vec::new(), Vec::new(), Vec::new());
        for (si, site) in self.core.engrams.iter().enumerate() {
            let mut fetched = vec![0f32; n * DM];
            fetched.par_chunks_mut(DM).enumerate().for_each(|(r, dst)| {
                let p = start + r % s;
                let mut row = vec![0f32; e.sub_dim];
                for t in 0..nt {
                    if ngram_valid(p, t, &e.orders, e.heads) {
                        site.tables.dequantize_row(t * e.slots + idx[r * nt + t] as usize, &mut row);
                        dst[t * e.sub_dim..(t + 1) * e.sub_dim].copy_from_slice(&row);
                    }
                }
            });
            let f = Tensor::from_vec(fetched, (n, DM), &self.dev)?;
            let k = f.matmul(&self.sites[si].key)?;
            let raw: Vec<f32> = f.matmul(&self.sites[si].value)?.flatten_all()?.to_vec1()?;
            let taps = &self.sites[si].taps;
            let mut v = vec![0f32; n * DM];
            v.par_chunks_mut(DM).enumerate().for_each(|(r, dst)| {
                let (b, i) = (r / s, r % s);
                let p = start + i;
                for j in 0..e.conv_taps {
                    let back = j * e.conv_dilation;
                    if back > p || p < j * max_order {
                        continue;
                    }
                    let src = if i >= back {
                        &raw[(b * s + i - back) * DM..(b * s + i - back + 1) * DM]
                    } else {
                        let m = ENGRAM_REACH - (back - i);
                        match pre {
                            Some(pre) => &pre.engram_tail[si][m * DM..(m + 1) * DM],
                            None => continue,
                        }
                    };
                    for c in 0..DM {
                        dst[c] += taps[j * DM + c] * src[c];
                    }
                }
            });
            ks.push(rms_unit(&k)?.to_dtype(self.dt)?);
            vs.push(Tensor::from_vec(v, (n, DM), &self.dev)?.to_dtype(self.dt)?);
            raws.push(raw);
        }
        Ok((ks, vs, raws))
    }

    /// Everything about `seqs` that does not depend on the adapter. `masks` (aligned with `seqs`) marks the tokens that
    /// carry the loss; `lora_from` restricts LoRA to absolute positions at or after it.
    pub fn batch(
        &self,
        seqs: &[Vec<u32>],
        start: usize,
        pre: Option<&Prefix>,
        masks: Option<&[Vec<f32>]>,
        lora_from: Option<usize>,
    ) -> Result<Batch> {
        ensure!(start == pre.map_or(0, |p| p.len), "a run starts at 0 or right after its prefix");
        let (b, s) = (seqs.len(), seqs.iter().map(Vec::len).max().unwrap_or(0));
        let n = b * s;
        let mut toks = vec![0u32; n];
        for (i, run) in seqs.iter().enumerate() {
            toks[i * s..i * s + run.len()].copy_from_slice(run);
        }
        let ids = Tensor::from_vec(toks, n, &self.dev)?;
        let emb = self.emb.index_select(&ids, 0)?.affine((DM as f32).sqrt() as f64, 0.0)?;
        let (ek, ev, raw) = self.engram(seqs, s, start, pre)?;
        let cfg = &self.core.cfg;
        let half = cfg.qk_head_dim / 2;
        let (mut cos, mut sin) = (vec![0f32; s * half], vec![0f32; s * half]);
        for i in 0..half {
            let freq = 1.0 / libm::powf(cfg.rope_theta, (2 * i) as f32 / cfg.qk_head_dim as f32);
            for t in 0..s {
                let angle = (start + t) as f32 * freq;
                cos[t * half + i] = libm::cosf(angle);
                sin[t * half + i] = libm::sinf(angle);
            }
        }
        let cos = Tensor::from_vec(cos, (1, s, 1, half), &self.dev)?.to_dtype(self.dt)?;
        let sin = Tensor::from_vec(sin, (1, s, 1, half), &self.dev)?.to_dtype(self.dt)?;
        let lora_rows = match lora_from {
            Some(from) => {
                let m: Vec<f32> = (0..n).map(|r| if start + r % s >= from { 1.0 } else { 0.0 }).collect();
                Some(Tensor::from_vec(m, (n, 1), &self.dev)?.to_dtype(self.dt)?)
            }
            None => None,
        };
        let (mut rows, mut targets) = (Vec::new(), Vec::new());
        if let Some(masks) = masks {
            for (i, (run, mask)) in seqs.iter().zip(masks).enumerate() {
                for t in 0..run.len() - 1 {
                    if mask[t + 1] > 0.0 {
                        rows.push((i * s + t) as u32);
                        targets.push(run[t + 1]);
                    }
                }
            }
        }
        let ntarget = rows.len();
        let (rows, targets) = if masks.is_some() {
            (Some(Tensor::from_vec(rows, ntarget, &self.dev)?), Some(Tensor::from_vec(targets, ntarget, &self.dev)?))
        } else {
            (None, None)
        };
        Ok(Batch { b, s, start, emb, ek, ev, raw, cos, sin, lora_rows, rows, targets })
    }

    fn proj(&self, x: &Tensor, l: &Layer, which: usize, li: usize, lora: Option<&Lora>, bt: &Batch) -> Result<Tensor> {
        let y = x.contiguous()?.apply_op1(Frozen(l.proj[which].clone()))?;
        let Some(lo) = lora else { return Ok(y) };
        let d = x.matmul(lo.a[li][which].as_tensor())?.matmul(lo.b[li][which].as_tensor())?.affine(lo.scale, 0.0)?;
        let d = match &bt.lora_rows {
            Some(r) => d.broadcast_mul(r)?,
            None => d,
        };
        Ok(y.add(&d)?)
    }

    /// The final-normed hidden state of every position, `[b * s, 768]`.
    pub fn forward(&self, bt: &Batch, pre: Option<&Prefix>, lora: Option<&Lora>) -> Result<Tensor> {
        self.forward_impl(bt, pre, lora, None)
    }

    fn forward_impl(
        &self,
        bt: &Batch,
        pre: Option<&Prefix>,
        lora: Option<&Lora>,
        mut cap: Option<&mut Capture>,
    ) -> Result<Tensor> {
        let cfg = &self.core.cfg;
        let (b, s, n) = (bt.b, bt.s, bt.b * bt.s);
        let (hq, hkv, dqk, dv) = (cfg.num_heads, cfg.num_kv_heads, cfg.qk_head_dim, cfg.v_head_dim);
        let rep = hq / hkv;
        let p = pre.map_or(0, |x| x.len);
        let scale = (1.0f32 / (dqk as f32).sqrt()) as f64;
        let mut lanes = bt.emb.unsqueeze(1)?.broadcast_as((n, LANES, DM))?.contiguous()?;
        for (li, l) in self.layers.iter().enumerate() {
            let nx = rms_unit(&lanes.reshape((n, LANES * DM))?)?;
            let hpre = sigmoid(
                &nx.matmul(&l.phi_pre)?.affine(l.a[0], 0.0)?.broadcast_add(&l.b_pre)?.broadcast_add(&l.pre_off)?,
            )?;
            let u = hpre.unsqueeze(2)?.broadcast_mul(&lanes)?.sum(1)?;
            let mut bx = u.clone();
            if let Some(si) = l.site {
                let dot = rms_unit(&u)?.mul(&bt.ek[si])?.sum_keepdim(1)?;
                let alpha = sigmoid(&dot.affine((1.0f32 / (DM as f32).sqrt()) as f64, 0.0)?)?;
                bx = bx.add(&alpha.broadcast_mul(&bt.ev[si])?)?;
            }
            let h = zcrms(&bx, &l.norm_in)?;
            let q = self.proj(&h, l, 0, li, lora, bt)?;
            let k = self.proj(&h, l, 1, li, lora, bt)?;
            let v = self.proj(&h, l, 2, li, lora, bt)?;
            let g = self.proj(&h, l, 3, li, lora, bt)?;
            if let Some(c) = cap.as_deref_mut() {
                let last = |t: &Tensor| t.narrow(0, n - CONV_HIST, CONV_HIST)?.contiguous();
                c.tail.push([last(&q)?, last(&k)?, last(&v)?]);
            }
            let hist = |i: usize| pre.map(|x| &x.tail[li][i]);
            let q = conv(&q.reshape((b, s, hq * dqk))?, &l.taps[0], hist(0))?;
            let k = conv(&k.reshape((b, s, hkv * dqk))?, &l.taps[1], hist(1))?;
            let v = conv(&v.reshape((b, s, hkv * dv))?, &l.taps[2], hist(2))?;
            let q = rope(&zcrms(&q.reshape((b, s, hq, dqk))?, &l.q_norm)?, &bt.cos, &bt.sin)?;
            let k = rope(&zcrms(&k.reshape((b, s, hkv, dqk))?, &l.k_norm)?, &bt.cos, &bt.sin)?;
            // Query head h reads key/value head h / 6: group the six queries of a kv head along the rows.
            let q = q.reshape((b, s, hkv, rep, dqk))?.permute((0, 2, 3, 1, 4))?.reshape((b, hkv, rep * s, dqk))?;
            let k = k.permute((0, 2, 1, 3))?.contiguous()?;
            let v = v.reshape((b, s, hkv, dv))?.permute((0, 2, 1, 3))?.contiguous()?;
            if let Some(c) = cap.as_deref_mut() {
                c.k.push(k.clone());
                c.v.push(v.clone());
            }
            let (kf, vf) = match pre {
                Some(x) => (
                    Tensor::cat(&[&x.k[li].broadcast_as((b, hkv, p, dqk))?, &k], 2)?.contiguous()?,
                    Tensor::cat(&[&x.v[li].broadcast_as((b, hkv, p, dv))?, &v], 2)?.contiguous()?,
                ),
                None => (k, v),
            };
            let window = (!l.global && cfg.sliding_window > 0).then_some(cfg.sliding_window);
            let w = q.matmul(&kf.t()?)?.apply_op1(CausalSoftmax { s, start: bt.start, window, scale })?;
            let o = w.matmul(&vf)?;
            let o = o.reshape((b, hkv, rep, s, dv))?.permute((0, 3, 1, 2, 4))?.reshape((n, hq * dv))?;
            let o = o.mul(&sigmoid(&g)?)?;
            let a = zcrms(&self.proj(&o, l, 4, li, lora, bt)?, &l.post_norm)?;
            let bx = bx.add(&a.affine(l.gate, 0.0)?)?;
            let bx = bx.add(&self.mlp(&zcrms(&bx, &l.pre_hada)?, &l.mlp)?)?;
            let y = bx.sub(&u)?;
            let hpost = sigmoid(
                &nx.matmul(&l.phi_post)?.affine(l.a[1], 0.0)?.broadcast_add(&l.b_post)?.broadcast_add(&l.post_off)?,
            )?
            .affine(2.0, 0.0)?;
            let res =
                nx.matmul(&l.phi_res)?.affine(l.a[2], 0.0)?.broadcast_add(&l.b_res)?.reshape((n, LANES, LANES))?;
            lanes = sinkhorn(&res)?.matmul(&lanes)?.add(&hpost.unsqueeze(2)?.broadcast_mul(&y.unsqueeze(1)?)?)?;
        }
        Ok(zcrms(&lanes.mean(1)?, &self.final_norm)?)
    }

    fn mlp(&self, x: &Tensor, w: &Mlp) -> Result<Tensor> {
        let n = x.dim(0)?;
        let cond = softmax_last(&x.matmul(&w.cond_v)?)?.matmul(&w.cond_u)?.affine(1.0, 1.0)?;
        let z = Tensor::cat(&[x, &Tensor::zeros((n, HADA - DM), x.dtype(), x.device())?], 1)?;
        let z = z.broadcast_mul(&w.d1)?;
        let z = kron(&z, &w.w[0].0, &w.w[0].1)?.index_select(&self.p1, 1)?;
        let z = cond.broadcast_mul(&w.d2)?.mul(&z)?.broadcast_add(&w.b2)?.silu()?;
        let z = kron(&z, &w.w[1].0, &w.w[1].1)?.index_select(&self.p2, 1)?;
        let z = z.broadcast_mul(&w.d3)?;
        let z = kron(&z, &w.w[2].0, &w.w[2].1)?;
        Ok(z.broadcast_mul(&w.d4)?.narrow(1, 0, DM)?)
    }

    /// Run the tools prefix once, without LoRA, and keep what the rest of every sequence reads from it.
    pub fn prefix(&self, tokens: &[u32]) -> Result<Prefix> {
        let bt = self.batch(&[tokens.to_vec()], 0, None, None, None)?;
        let mut cap = Capture::default();
        self.forward_impl(&bt, None, None, Some(&mut cap))?;
        let p = tokens.len();
        let engram_tail = bt.raw.iter().map(|r| r[(p - ENGRAM_REACH) * DM..].to_vec()).collect();
        Ok(Prefix { len: p, tokens: tokens.to_vec(), k: cap.k, v: cap.v, tail: cap.tail, engram_tail })
    }

    /// Logits of the given rows of `x` (all rows when `None`), `[rows, logit_rows]`.
    pub fn logits(&self, x: &Tensor, rows: Option<&Tensor>) -> Result<Tensor> {
        let x = match rows {
            Some(r) => x.index_select(r, 0)?,
            None => x.clone(),
        };
        Ok(x.contiguous()?.apply_op1(Frozen(self.emb_t.clone()))?)
    }

    /// The summed cross-entropy of the batch's loss-carrying tokens.
    pub fn loss_sum(&self, x: &Tensor, bt: &Batch) -> Result<Tensor> {
        let logits = self.logits(x, bt.rows.as_ref())?;
        let targets = bt.targets.as_ref().ok_or_else(|| anyhow::anyhow!("a batch with targets"))?;
        let picked = logits.gather(&targets.unsqueeze(1)?, 1)?;
        Ok(logsumexp(&logits, 1)?.sub(&picked)?.sum_all()?)
    }
}
