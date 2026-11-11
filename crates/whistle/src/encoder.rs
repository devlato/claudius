//! The speech encoder: the convolutional stem, eight macaron blocks in an mHC
//! lane stack, and the final norm (`cactus/model/whistle.py`: `Stem`,
//! `EncoderBlock`, `LaneStack`, `Whistle.encode`).
//!
//! needle-core supplies the CQ matmuls, the Hadamard MLP, ZCRMSNorm, the mHC
//! mix-down and scatter, RoPE and the attention kernel. New here: the stem's
//! strided 2-D convolutions, the conv module (GLU, a centred depthwise
//! convolution over time) and the bidirectional attention driver.

use needle_core::cq::CqWeight;
use needle_core::kernels::rms_unit_to;
use needle_core::norm::zc_rms_norm_vec;
use needle_core::ops::{sigmoid, silu};
use needle_core::rope::RopeCache;
use needle_core::v3::mhc::{Gates, LaneSite};
use needle_core::v3::{
    AttnDims, HadaMlp, HadaPerms, KvStore, MhcLayer, Ring, V3Mhc, attend_step, hadamard_mlp, mix_down, norm_and_rope,
    quantize_rows, scatter_up,
};

use crate::nn::{Numerics, a8, a8_rows, matmul, prepare, project, zc_rows};

/// Attention geometry, shared with the decoder and read from the header.
#[derive(Debug, Clone, Copy)]
pub struct Dims {
    pub d: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub qk: usize,
    pub v: usize,
    pub hada_n: usize,
    pub rope_theta: f32,
}

/// The convolutional stem: one 3x3 stride-2 convolution from one channel,
/// then `pw.len()` depthwise 3x3 stride-2 stages each followed by a pointwise
/// matrix, then a projection of the channel-major flattened frame.
pub struct Stem {
    /// `(9, channels)`, row `3 * time + mel`.
    pub w: Vec<f32>,
    pub dw: Vec<Vec<f32>>,
    pub pw: Vec<CqWeight>,
    pub out: CqWeight,
    pub channels: usize,
}

pub struct EncBlock {
    pub pre_hada_norm_0: Vec<f32>,
    pub attn_norm: Vec<f32>,
    pub post_attn_norm: Vec<f32>,
    pub conv_norm: Vec<f32>,
    pub conv_out_norm: Vec<f32>,
    pub pre_hada_norm: Vec<f32>,
    pub q_proj: CqWeight,
    pub k_proj: CqWeight,
    pub v_proj: CqWeight,
    pub gate_proj: CqWeight,
    pub out_proj: CqWeight,
    pub q_norm: Vec<f32>,
    pub k_norm: Vec<f32>,
    /// Pre-sigmoid.
    pub attn_gate: f32,
    pub pw1: CqWeight,
    pub pw2: CqWeight,
    /// `(kernel, d)`, applied as stored (cross-correlation).
    pub dw: Vec<f32>,
    pub mlp_0: HadaMlp,
    pub mlp: HadaMlp,
}

pub struct Encoder {
    pub dims: Dims,
    pub stem: Stem,
    pub blocks: Vec<EncBlock>,
    pub mhc: V3Mhc,
    pub final_norm: Vec<f32>,
    pub lanes: usize,
    pub kernel: usize,
}

/// One 3x3 stride-2 tap's input index, or `None` in the one-zero padding.
#[inline]
fn tap(out: usize, k: usize, len: usize) -> Option<usize> {
    let i = 2 * out + k;
    if i == 0 || i > len { None } else { Some(i - 1) }
}

impl Stem {
    /// `feats` is `(frames, mels)`; returns `(out_frames, d)` and `out_frames`.
    /// Positions past the clip read as zero, as the reference's length mask
    /// makes them.
    pub fn forward(&self, feats: &[f32], frames: usize, mels: usize, num: Numerics) -> (Vec<f32>, usize) {
        let c = self.channels;
        let (mut t_in, mut f_in) = (frames, mels);
        let (mut t, mut f) = (t_in.div_ceil(2), f_in.div_ceil(2));

        // Stage 0: one channel to `c`, laid out (time, mel, channel).
        let mut x = vec![0.0f32; t * f * c];
        for t_out in 0..t {
            for f_out in 0..f {
                let o = &mut x[(t_out * f + f_out) * c..(t_out * f + f_out + 1) * c];
                for kh in 0..3 {
                    let Some(it) = tap(t_out, kh, t_in) else { continue };
                    for kw in 0..3 {
                        let Some(i_f) = tap(f_out, kw, f_in) else { continue };
                        let v = feats[it * mels + i_f];
                        let k = &self.w[(3 * kh + kw) * c..(3 * kh + kw + 1) * c];
                        for (oc, &kc) in o.iter_mut().zip(k) {
                            *oc += kc * v;
                        }
                    }
                }
            }
        }

        for (dw, pw) in self.dw.iter().zip(&self.pw) {
            for v in x.iter_mut() {
                *v = silu(*v);
            }
            t_in = t;
            f_in = f;
            t = t_in.div_ceil(2);
            f = f_in.div_ceil(2);
            let mut y = vec![0.0f32; t * f * c];
            for t_out in 0..t {
                for f_out in 0..f {
                    let o = &mut y[(t_out * f + f_out) * c..(t_out * f + f_out + 1) * c];
                    for kh in 0..3 {
                        let Some(it) = tap(t_out, kh, t_in) else { continue };
                        for kw in 0..3 {
                            let Some(i_f) = tap(f_out, kw, f_in) else { continue };
                            let src = &x[(it * f_in + i_f) * c..(it * f_in + i_f + 1) * c];
                            let k = &dw[(3 * kh + kw) * c..(3 * kh + kw + 1) * c];
                            for ((oc, &kc), &s) in o.iter_mut().zip(k).zip(src) {
                                *oc += kc * s;
                            }
                        }
                    }
                }
            }
            let yh = prepare(pw, &y, t * f);
            x = project(pw, &yh, t * f);
        }

        // SiLU, flatten channel-major (index `channel * mels + mel`), project.
        let width = c * f;
        assert_eq!(width, self.out.in_feat, "stem output width");
        let mut flat = vec![0.0f32; t * width];
        for t_out in 0..t {
            for f_out in 0..f {
                for ch in 0..c {
                    flat[t_out * width + ch * f + f_out] = silu(x[(t_out * f + f_out) * c + ch]);
                }
            }
        }
        a8_rows(&mut flat, width, num.act8);
        let xh = prepare(&self.out, &flat, t);
        (project(&self.out, &xh, t), t)
    }
}

/// `x += scale * HadamardMLP(ZCRMSNorm(x))`, row by row.
pub fn mlp_residual(x: &mut [f32], norm: &[f32], mlp: &HadaMlp, perms: &HadaPerms, hada_n: usize, scale: f32) {
    let d = norm.len();
    let mut h = vec![0.0f32; d];
    let mut o = vec![0.0f32; d];
    for row in x.chunks_exact_mut(d) {
        h.copy_from_slice(row);
        zc_rms_norm_vec(&mut h, norm);
        hadamard_mlp(&h, mlp, perms, hada_n, &mut o);
        for (xi, &oi) in row.iter_mut().zip(&o) {
            *xi += scale * oi;
        }
    }
}

impl EncBlock {
    /// Bidirectional grouped-query self-attention over all `t` frames, with
    /// RoPE on Q and K after their per-head norms.
    fn attention(&self, h: &[f32], t: usize, dims: &Dims, rope: &RopeCache, num: Numerics) -> Vec<f32> {
        let (d, nh, kvh, qk, vd) = (dims.d, dims.heads, dims.kv_heads, dims.qk, dims.v);
        let mut x = h.to_vec();
        a8_rows(&mut x, d, num.act8);
        let xh = prepare(&self.q_proj, &x, t);
        let mut q = project(&self.q_proj, &xh, t);
        let mut k = project(&self.k_proj, &xh, t);
        let v = project(&self.v_proj, &xh, t);
        let gate = project(&self.gate_proj, &xh, t);
        let half = qk / 2;
        let (cos, sin) = (&rope.cos[..t * half], &rope.sin[..t * half]);
        norm_and_rope(&mut q, &self.q_norm, cos, sin, t, nh, qk);
        norm_and_rope(&mut k, &self.k_norm, cos, sin, t, kvh, qk);
        a8_rows(&mut q, qk, num.act8);

        let dims1 = AttnDims { seq: 1, num_heads: nh, num_kv_heads: kvh, qk_head_dim: qk, v_head_dim: vd };
        let ow = nh * vd;
        let mut out = vec![0.0f32; t * ow];
        let (kq, ks) = quantize_rows(&k, qk);
        let (vq, vs) = quantize_rows(&v, vd);
        for r in 0..t {
            let kv = if num.kv8 {
                KvStore::Int8 { k: &kq, k_scale: &ks, v: &vq, v_scale: &vs }
            } else {
                KvStore::F32 { k: &k, v: &v }
            };
            attend_step(
                &q[r * nh * qk..(r + 1) * nh * qk],
                Ring { kv, slots: t, lo: 0, hi: t - 1 },
                dims1,
                &mut out[r * ow..(r + 1) * ow],
            );
        }
        for (o, &g) in out.iter_mut().zip(&gate) {
            *o *= sigmoid(g);
        }
        a8_rows(&mut out, ow, num.act8);
        let oh = prepare(&self.out_proj, &out, t);
        project(&self.out_proj, &oh, t)
    }

    /// The macaron block over `t` frames in place.
    fn forward(&self, x: &mut [f32], t: usize, enc: &Encoder, rope: &RopeCache, perms: &HadaPerms, num: Numerics) {
        let d = enc.dims.d;
        mlp_residual(x, &self.pre_hada_norm_0, &self.mlp_0, perms, enc.dims.hada_n, 0.5);

        let mut h = x.to_vec();
        zc_rows(&mut h, &self.attn_norm);
        let mut a = self.attention(&h, t, &enc.dims, rope, num);
        zc_rows(&mut a, &self.post_attn_norm);
        let g = sigmoid(self.attn_gate);
        for (xi, &ai) in x.iter_mut().zip(&a) {
            *xi += g * ai;
        }

        // Conv module: pw1, GLU, depthwise over time ("SAME", not flipped),
        // norm, SiLU, pw2, then a plain residual.
        let mut h = x.to_vec();
        zc_rows(&mut h, &self.conv_norm);
        a8_rows(&mut h, d, num.act8);
        let hh = prepare(&self.pw1, &h, t);
        let p = project(&self.pw1, &hh, t);
        let mut glu = vec![0.0f32; t * d];
        for r in 0..t {
            let row = &p[r * 2 * d..(r + 1) * 2 * d];
            for ch in 0..d {
                glu[r * d + ch] = row[ch] * sigmoid(row[d + ch]);
            }
        }
        let half = enc.kernel / 2;
        let mut dc = vec![0.0f32; t * d];
        for r in 0..t {
            let o = &mut dc[r * d..(r + 1) * d];
            for k in 0..enc.kernel {
                let src = r + k;
                if src < half || src - half >= t {
                    continue;
                }
                let s = &glu[(src - half) * d..(src - half + 1) * d];
                let w = &self.dw[k * d..(k + 1) * d];
                for ((oc, &wc), &sc) in o.iter_mut().zip(w).zip(s) {
                    *oc += wc * sc;
                }
            }
        }
        zc_rows(&mut dc, &self.conv_out_norm);
        for v in dc.iter_mut() {
            *v = silu(*v);
        }
        a8_rows(&mut dc, d, num.act8);
        let dh = prepare(&self.pw2, &dc, t);
        let p2 = project(&self.pw2, &dh, t);
        for (xi, &pi) in x.iter_mut().zip(&p2) {
            *xi += pi;
        }

        mlp_residual(x, &self.pre_hada_norm, &self.mlp, perms, enc.dims.hada_n, 0.5);
    }
}

impl Encoder {
    /// Features `(frames, mels)` to the speech embedding `(t, d)`, one row per
    /// 80 ms frame.
    pub fn forward(
        &self,
        feats: &[f32],
        frames: usize,
        mels: usize,
        perms: &HadaPerms,
        num: Numerics,
    ) -> (Vec<f32>, usize) {
        let (x0, t) = self.stem.forward(feats, frames, mels, num);
        if t == 0 {
            return (Vec::new(), 0);
        }
        let (d, n) = (self.dims.d, self.lanes);
        let rope = RopeCache::new(t, self.dims.qk, self.dims.rope_theta);

        let mut lanes = vec![0.0f32; t * n * d];
        for r in 0..t {
            for l in 0..n {
                lanes[(r * n + l) * d..(r * n + l + 1) * d].copy_from_slice(&x0[r * d..(r + 1) * d]);
            }
        }

        let p = self.mhc.phi_pre.prepared_len();
        let mut nxh = vec![0.0f32; t * p];
        let mut nx = vec![0.0f32; n * d];
        let mut hpre = vec![0.0f32; t * n];
        let mut hpost = vec![0.0f32; t * n];
        let mut hres = vec![0.0f32; t * n * n];
        let mut u = vec![0.0f32; t * d];
        let mut y = vec![0.0f32; d];
        let mut scratch = vec![0.0f32; n * d];

        for (li, blk) in self.blocks.iter().enumerate() {
            let m = MhcLayer {
                a_pre: self.mhc.a_pre[li],
                a_post: self.mhc.a_post[li],
                a_res: self.mhc.a_res[li],
                b_pre: &self.mhc.b_pre[li * n..(li + 1) * n],
                b_post: &self.mhc.b_post[li * n..(li + 1) * n],
                b_res: &self.mhc.b_res[li * n * n..(li + 1) * n * n],
            };
            for r in 0..t {
                rms_unit_to(&lanes[r * n * d..(r + 1) * n * d], &mut nx);
                if num.act8 {
                    a8(&mut nx);
                }
                self.mhc.phi_pre.prepare_input(&nx, &mut nxh[r * p..(r + 1) * p]);
            }
            matmul(&self.mhc.phi_pre, &nxh, t, li * n, n, &mut hpre);
            matmul(&self.mhc.phi_post, &nxh, t, li * n, n, &mut hpost);
            matmul(&self.mhc.phi_res, &nxh, t, li * n * n, n * n, &mut hres);
            for r in 0..t {
                mix_down(
                    &lanes[r * n * d..(r + 1) * n * d],
                    &mut hpre[r * n..(r + 1) * n],
                    &m,
                    li,
                    n,
                    d,
                    &mut u[r * d..(r + 1) * d],
                );
            }

            let mut x = u.clone();
            blk.forward(&mut x, t, self, &rope, perms, num);

            for r in 0..t {
                for c in 0..d {
                    y[c] = x[r * d + c] - u[r * d + c];
                }
                scatter_up(
                    &mut lanes[r * n * d..(r + 1) * n * d],
                    Gates { post: &mut hpost[r * n..(r + 1) * n], res: &mut hres[r * n * n..(r + 1) * n * n] },
                    &y,
                    &m,
                    LaneSite { layer: li, lanes: n, d_model: d },
                    &mut scratch,
                );
            }
        }

        let mut out = vec![0.0f32; t * d];
        for r in 0..t {
            let o = &mut out[r * d..(r + 1) * d];
            for (c, oc) in o.iter_mut().enumerate() {
                let mut a = 0.0f32;
                for l in 0..n {
                    a += lanes[(r * n + l) * d + c];
                }
                *oc = a / n as f32;
            }
            zc_rms_norm_vec(o, &self.final_norm);
        }
        (out, t)
    }
}
