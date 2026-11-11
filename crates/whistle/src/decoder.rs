//! The text decoder: eight Needle 3 blocks at width 512, each with a gated
//! cross-attention step over the encoder memory (`cactus/model/whistle.py`:
//! `DecoderBlock`, `CrossAttention`, `Whistle.decode`, `Whistle.logits`).
//!
//! The weights are needle-core's own `V3Layer`/`V3Mhc`/`V3EngramSite`, and the
//! step reuses its mHC, Engram, attention, RoPE and Hadamard MLP kernels.
//! `V3Model`'s per-layer loop has no place for cross-attention, so the loop is
//! written here, and its decode state is this module's own because
//! `needle_core::v3::V3Cache` is not `Clone`, which a beam search needs.

use std::collections::VecDeque;

use needle_core::cq::CqWeight;
use needle_core::kernels::rms_unit_to;
use needle_core::norm::zc_rms_norm_vec;
use needle_core::ops::sigmoid;
use needle_core::rope::RopeCache;
use needle_core::v3::engram::apply_site;
use needle_core::v3::mhc::{Gates, LaneSite};
use needle_core::v3::{
    AttnDims, HadaPerms, KvStore, MhcLayer, Ring, V3Config, V3EngramSite, V3Layer, V3Mhc, attend_step,
    causal_depthwise_conv, engram_indices, hadamard_mlp, mix_down, ngram_valid, norm_and_rope, quantize_rows,
    scatter_up,
};

use crate::nn::{Numerics, a8, a8_rows, prepare, project, zc_rows};

/// One decoder layer's cross-attention: 8 heads for queries, keys and values
/// alike, per-head norms on Q and K, no RoPE, a sigmoid output gate.
pub struct CrossLayer {
    pub norm: Vec<f32>,
    pub q_proj: CqWeight,
    pub k_proj: CqWeight,
    pub v_proj: CqWeight,
    pub gate_proj: CqWeight,
    pub out_proj: CqWeight,
    pub q_norm: Vec<f32>,
    pub k_norm: Vec<f32>,
    pub post_norm: Vec<f32>,
    /// Pre-sigmoid.
    pub gate: f32,
}

pub struct Decoder {
    pub cfg: V3Config,
    /// Tied: the input embedding (times `sqrt(d)`) and the output head.
    pub embedding: CqWeight,
    pub layers: Vec<V3Layer>,
    pub cross: Vec<CrossLayer>,
    pub mhc: V3Mhc,
    pub engrams: Vec<V3EngramSite>,
    pub final_norm: Vec<f32>,
    /// Pre-sigmoid weight of the sinusoid added to the encoder memory.
    pub pe_gate: f32,
    pub rope: RopeCache,
}

/// Keys and values for one layer, either int8 per head or f32.
#[derive(Clone, Default)]
pub struct Kv {
    k: Vec<i8>,
    ks: Vec<f32>,
    v: Vec<i8>,
    vs: Vec<f32>,
    kf: Vec<f32>,
    vf: Vec<f32>,
}

impl Kv {
    fn push(&mut self, k: &[f32], v: &[f32], qk: usize, vd: usize, kv8: bool) {
        if kv8 {
            let (kq, ksc) = quantize_rows(k, qk);
            let (vq, vsc) = quantize_rows(v, vd);
            self.k.extend_from_slice(&kq);
            self.ks.extend_from_slice(&ksc);
            self.v.extend_from_slice(&vq);
            self.vs.extend_from_slice(&vsc);
        } else {
            self.kf.extend_from_slice(k);
            self.vf.extend_from_slice(v);
        }
    }

    fn store(&self, kv8: bool) -> KvStore<'_> {
        if kv8 {
            KvStore::Int8 { k: &self.k, k_scale: &self.ks, v: &self.v, v_scale: &self.vs }
        } else {
            KvStore::F32 { k: &self.kf, v: &self.vf }
        }
    }
}

/// The encoder memory as each layer's cross-attention reads it, projected
/// once per clip and shared by every hypothesis.
pub struct Memory {
    pub frames: usize,
    pub layers: Vec<Kv>,
}

/// One hypothesis's decode state.
#[derive(Clone)]
pub struct State {
    pub pos: usize,
    /// The last `max(order)` tokens, for the Engram hash.
    hist: Vec<u32>,
    kv: Vec<Kv>,
    /// Raw (pre-convolution) q, k, v of the last `taps - 1` positions per
    /// layer, oldest first; zeros stand for positions before the start.
    tails: Vec<[Vec<f32>; 3]>,
    /// Raw Engram values per site, most recent last, `(taps - 1) * dilation` deep.
    eng: Vec<VecDeque<Vec<f32>>>,
}

/// `[sin(t·ω), cos(t·ω)]`, `ω_i = exp(-i · ln 10000 / (d/2 - 1))`.
fn sinusoid(t: usize, d: usize, out: &mut [f32]) {
    let half = d / 2;
    let step = -(10000.0f64.ln()) / (half as f64 - 1.0);
    for i in 0..half {
        let omega = ((step * i as f64) as f32).exp();
        let ang = t as f32 * omega;
        out[i] = ang.sin();
        out[half + i] = ang.cos();
    }
}

impl Decoder {
    fn dims(&self) -> (usize, usize, usize, usize, usize) {
        let c = &self.cfg;
        (c.d_model, c.num_heads, c.num_kv_heads, c.qk_head_dim, c.v_head_dim)
    }

    /// `mem = A8(enc + sigmoid(pe_gate) · sinusoid)`, then every layer's
    /// cross keys (normed per head) and values.
    pub fn memory(&self, enc: &[f32], t: usize, num: Numerics) -> Memory {
        let (d, nh, _, qk, vd) = self.dims();
        let g = sigmoid(self.pe_gate);
        let mut mem = enc.to_vec();
        let mut pe = vec![0.0f32; d];
        for r in 0..t {
            sinusoid(r, d, &mut pe);
            for (m, &p) in mem[r * d..(r + 1) * d].iter_mut().zip(&pe) {
                *m += g * p;
            }
        }
        a8_rows(&mut mem, d, num.act8);
        let mut layers = Vec::with_capacity(self.cross.len());
        if t == 0 {
            return Memory { frames: 0, layers };
        }
        let mh = prepare(&self.cross[0].k_proj, &mem, t);
        for cl in &self.cross {
            let mut k = project(&cl.k_proj, &mh, t);
            let v = project(&cl.v_proj, &mh, t);
            zc_rows(&mut k, &cl.k_norm);
            debug_assert_eq!(k.len(), t * nh * qk);
            let mut kv = Kv::default();
            kv.push(&k, &v, qk, vd, num.kv8);
            layers.push(kv);
        }
        Memory { frames: t, layers }
    }

    /// A fresh hypothesis, before `<s>`.
    pub fn state(&self) -> State {
        let c = &self.cfg;
        let taps = c.qkv_conv_taps.saturating_sub(1);
        let tails = (0..c.num_layers)
            .map(|_| [vec![0.0f32; taps * c.q_dim()], vec![0.0f32; taps * c.k_dim()], vec![0.0f32; taps * c.v_dim()]])
            .collect();
        State {
            pos: 0,
            hist: Vec::new(),
            kv: vec![Kv::default(); c.num_layers],
            tails,
            eng: vec![VecDeque::new(); c.engram.sites.len()],
        }
    }

    /// Feed `token` at the state's position; returns the logits for the next
    /// token over the whole tied vocabulary.
    pub fn step(&self, st: &mut State, mem: &Memory, token: u32, perms: &HadaPerms, num: Numerics) -> Vec<f32> {
        let c = &self.cfg;
        let (d, nh, kvh, qk, vd) = self.dims();
        let n = c.mhc_lanes;
        let pos = st.pos;
        assert!(pos < c.max_seq_len, "decoder position {pos} past {}", c.max_seq_len);

        // Engram keys and values for this position.
        let e = &c.engram;
        let max_order = *e.orders.iter().max().unwrap_or(&1);
        st.hist.push(token);
        if st.hist.len() > max_order {
            st.hist.remove(0);
        }
        let tables = e.num_tables();
        let idx = engram_indices(&st.hist, &e.orders, e.heads, e.slots as u32, e.seed_heads);
        let here = st.hist.len() - 1;
        let reach = e.conv_taps.saturating_sub(1) * e.conv_dilation;
        let mut ek = vec![0.0f32; e.sites.len() * d];
        let mut ev = vec![0.0f32; e.sites.len() * d];
        let mut fetched = vec![0.0f32; tables * e.sub_dim];
        for (s, site) in self.engrams.iter().enumerate() {
            for tb in 0..tables {
                let dst = &mut fetched[tb * e.sub_dim..(tb + 1) * e.sub_dim];
                if ngram_valid(pos, tb, &e.orders, e.heads) {
                    let slot = idx[here * tables + tb] as usize;
                    site.tables.dequantize_row(tb * e.slots + slot, dst);
                } else {
                    dst.fill(0.0);
                }
            }
            if num.act8 {
                a8(&mut fetched);
            }
            site.key_proj.matvec(&fetched, &mut ek[s * d..(s + 1) * d]);
            let mut raw = vec![0.0f32; d];
            site.value_proj.matvec(&fetched, &mut raw);
            let out = &mut ev[s * d..(s + 1) * d];
            for (o, (&t0, &r)) in out.iter_mut().zip(site.taps[..d].iter().zip(&raw)) {
                *o = t0 * r;
            }
            let tail = &mut st.eng[s];
            for j in 1..e.conv_taps {
                let back = j * e.conv_dilation;
                if pos < back || pos < j * max_order {
                    continue;
                }
                let src = &tail[tail.len() - back];
                for (o, (&tj, &sv)) in out.iter_mut().zip(site.taps[j * d..(j + 1) * d].iter().zip(src)) {
                    *o += tj * sv;
                }
            }
            tail.push_back(raw);
            if tail.len() > reach {
                tail.pop_front();
            }
        }

        // The lane stream starts from the scaled embedding.
        let mut emb = vec![0.0f32; d];
        self.embedding.dequantize_row((token as usize).min(c.vocab_size - 1), &mut emb);
        let scale = (d as f32).sqrt();
        for x in emb.iter_mut() {
            *x *= scale;
        }
        let mut lanes = vec![0.0f32; n * d];
        for l in 0..n {
            lanes[l * d..(l + 1) * d].copy_from_slice(&emb);
        }

        let half = qk / 2;
        let rc = &self.rope.cos[pos * half..(pos + 1) * half];
        let rs = &self.rope.sin[pos * half..(pos + 1) * half];
        let self_dims = AttnDims { seq: 1, num_heads: nh, num_kv_heads: kvh, qk_head_dim: qk, v_head_dim: vd };
        let cross_dims = AttnDims { num_kv_heads: nh, ..self_dims };
        let taps_n = c.qkv_conv_taps;

        let p = self.mhc.phi_pre.prepared_len();
        let mut nx = vec![0.0f32; n * d];
        let mut nxh = vec![0.0f32; p];
        let mut hpre = vec![0.0f32; n];
        let mut hpost = vec![0.0f32; n];
        let mut hres = vec![0.0f32; n * n];
        let mut u = vec![0.0f32; d];
        let mut y = vec![0.0f32; d];
        let mut scratch = vec![0.0f32; n * d];
        let mut hh = vec![0.0f32; self.layers[0].q_proj.prepared_len()];
        let mut q = vec![0.0f32; c.q_dim()];
        let mut k = vec![0.0f32; c.k_dim()];
        let mut v = vec![0.0f32; c.v_dim()];
        let mut gate = vec![0.0f32; c.attn_out_dim()];
        let mut att = vec![0.0f32; c.attn_out_dim()];
        let mut proj = vec![0.0f32; d];
        let mut cq = vec![0.0f32; nh * qk];
        let mut mlp = vec![0.0f32; d];

        for li in 0..c.num_layers {
            let layer = &self.layers[li];
            let cross = &self.cross[li];
            let m = MhcLayer {
                a_pre: self.mhc.a_pre[li],
                a_post: self.mhc.a_post[li],
                a_res: self.mhc.a_res[li],
                b_pre: &self.mhc.b_pre[li * n..(li + 1) * n],
                b_post: &self.mhc.b_post[li * n..(li + 1) * n],
                b_res: &self.mhc.b_res[li * n * n..(li + 1) * n * n],
            };
            rms_unit_to(&lanes, &mut nx);
            if num.act8 {
                a8(&mut nx);
            }
            self.mhc.phi_pre.prepare_input(&nx, &mut nxh);
            self.mhc.phi_pre.matvec_rows_prepared(&nxh, li * n, &mut hpre);
            mix_down(&lanes, &mut hpre, &m, li, n, d, &mut u);

            let mut bx = u.clone();
            if let Some(site) = e.site_of(li) {
                apply_site(&mut bx, &ek[site * d..(site + 1) * d], &ev[site * d..(site + 1) * d], d);
            }

            // Causal self-attention with the 3-tap convolution over Q, K, V.
            let mut h = bx.clone();
            zc_rms_norm_vec(&mut h, &layer.norm_in);
            if num.act8 {
                a8(&mut h);
            }
            layer.q_proj.prepare_input(&h, &mut hh);
            layer.q_proj.matvec_prepared(&hh, &mut q);
            layer.k_proj.matvec_prepared(&hh, &mut k);
            layer.v_proj.matvec_prepared(&hh, &mut v);
            layer.gate_proj.matvec_prepared(&hh, &mut gate);
            if taps_n > 0 {
                let tails = &mut st.tails[li];
                for (which, (raw, taps)) in
                    [(&mut q, &layer.q_taps), (&mut k, &layer.k_taps), (&mut v, &layer.v_taps)].into_iter().enumerate()
                {
                    let dim = raw.len();
                    let tail = &mut tails[which];
                    let mut buf = Vec::with_capacity(taps_n * dim);
                    buf.extend_from_slice(tail);
                    buf.extend_from_slice(raw);
                    tail.copy_from_slice(&buf[dim..]);
                    causal_depthwise_conv(&mut buf, taps, taps_n, dim, taps_n);
                    raw.copy_from_slice(&buf[(taps_n - 1) * dim..]);
                }
            }
            norm_and_rope(&mut q, &layer.q_norm, rc, rs, 1, nh, qk);
            norm_and_rope(&mut k, &layer.k_norm, rc, rs, 1, kvh, qk);
            a8_rows(&mut q, qk, num.act8);
            st.kv[li].push(&k, &v, qk, vd, num.kv8);
            attend_step(&q, Ring { kv: st.kv[li].store(num.kv8), slots: pos + 1, lo: 0, hi: pos }, self_dims, &mut att);
            for (a, &g) in att.iter_mut().zip(&gate) {
                *a *= sigmoid(g);
            }
            if num.act8 {
                a8(&mut att);
            }
            layer.out_proj.matvec(&att, &mut proj);
            zc_rms_norm_vec(&mut proj, &layer.post_norm);
            let ag = sigmoid(layer.attn_gate);
            for (b, &pv) in bx.iter_mut().zip(&proj) {
                *b += ag * pv;
            }

            // Gated cross-attention over the encoder memory.
            if mem.frames > 0 {
                let mut x = bx.clone();
                zc_rms_norm_vec(&mut x, &cross.norm);
                if num.act8 {
                    a8(&mut x);
                }
                cross.q_proj.prepare_input(&x, &mut hh);
                cross.q_proj.matvec_prepared(&hh, &mut cq);
                cross.gate_proj.matvec_prepared(&hh, &mut gate);
                for hq in cq.chunks_exact_mut(qk) {
                    zc_rms_norm_vec(hq, &cross.q_norm);
                }
                a8_rows(&mut cq, qk, num.act8);
                attend_step(
                    &cq,
                    Ring { kv: mem.layers[li].store(num.kv8), slots: mem.frames, lo: 0, hi: mem.frames - 1 },
                    cross_dims,
                    &mut att,
                );
                for (a, &g) in att.iter_mut().zip(&gate) {
                    *a *= sigmoid(g);
                }
                if num.act8 {
                    a8(&mut att);
                }
                cross.out_proj.matvec(&att, &mut proj);
                zc_rms_norm_vec(&mut proj, &cross.post_norm);
                let cg = sigmoid(cross.gate);
                for (b, &pv) in bx.iter_mut().zip(&proj) {
                    *b += cg * pv;
                }
            }

            let mut h2 = bx.clone();
            zc_rms_norm_vec(&mut h2, &layer.pre_hada);
            hadamard_mlp(&h2, &layer.mlp, perms, c.hada_n, &mut mlp);
            for (b, &mv) in bx.iter_mut().zip(&mlp) {
                *b += mv;
            }

            for i in 0..d {
                y[i] = bx[i] - u[i];
            }
            self.mhc.phi_post.matvec_rows_prepared(&nxh, li * n, &mut hpost);
            self.mhc.phi_res.matvec_rows_prepared(&nxh, li * n * n, &mut hres);
            scatter_up(
                &mut lanes,
                Gates { post: &mut hpost, res: &mut hres },
                &y,
                &m,
                LaneSite { layer: li, lanes: n, d_model: d },
                &mut scratch,
            );
        }
        st.pos += 1;

        let mut x = vec![0.0f32; d];
        for (i, xi) in x.iter_mut().enumerate() {
            let mut a = 0.0f32;
            for l in 0..n {
                a += lanes[l * d + i];
            }
            *xi = a / n as f32;
        }
        zc_rms_norm_vec(&mut x, &self.final_norm);
        if num.act8 {
            a8(&mut x);
        }
        let mut logits = vec![0.0f32; c.vocab_size];
        self.embedding.matvec(&x, &mut logits);
        logits
    }
}
