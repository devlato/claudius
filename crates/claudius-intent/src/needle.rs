//! Reading requests with Needle 3, Cactus Compute's small tool-calling model (Apache-2.0), on the device: its weights
//! from `needle3.cact`, run on needle-core's own kernels at a depth of its ladder, with the adapters claudius was taught
//! with (`teach`) added beside the five projections of each block's attention (q, k, v, its gate and its output). The
//! tools are read once, without the adapters, as they were when the adapters were taught; each request rewinds to the
//! end of them, so it costs only its own words and the answer.
//!
//! The step is needle-core's `V3Model::decode_step`, operation for operation, with the adapters added; with none, it
//! gives needle-core's logits.

use needle_core::kernels::rms_unit_to;
use needle_core::math::{cos, powf, sin, sqrt};
use needle_core::norm::zc_rms_norm_vec;
use needle_core::ops::sigmoid;
use needle_core::v3::attention::{AttnDims, Ring, attend_step, norm_and_rope};
use needle_core::v3::cache::{KvPrecision, Qkv, V3Cache};
use needle_core::v3::engram::{apply_site, engram_indices, ngram_valid};
use needle_core::v3::kernels::hadamard_mlp;
use needle_core::v3::mhc::{Gates, LaneSite, MhcLayer, mix_down, scatter_up};
use needle_core::v3::model::V3Model;
use needle_infer::cact::CactV3;
use needle_infer::constrained::{ConstrainedDecoder, ToolDef, byte_table};
use needle_infer::prompt::{IM_END, TOOL_CALL_END, TOOL_CALL_START, build_prompt};
use needle_infer::sp_tokenizer::SpTokenizer;

/// the sequence's first token, and the end of a completion
const BOS: u32 = 2;
const EOS: u32 = 1;

/// One adapter: `y += scale · B·(A·x)`, `A` rank × in, `B` out × rank, both row-major.
#[derive(Clone, Debug, Default)]
pub struct Pair {
    pub a: Vec<f32>,
    pub b: Vec<f32>,
    pub rank: usize,
}

impl Pair {
    fn add(&self, x: &[f32], y: &mut [f32], scale: f32) {
        if self.rank == 0 {
            return;
        }
        let (n, r) = (x.len(), self.rank);
        let mut t = vec![0.0f32; r];
        for (i, ti) in t.iter_mut().enumerate() {
            *ti = self.a[i * n..(i + 1) * n].iter().zip(x).map(|(a, b)| a * b).sum();
        }
        for (o, yo) in y.iter_mut().enumerate() {
            let row = &self.b[o * r..(o + 1) * r];
            *yo += scale * row.iter().zip(&t).map(|(b, t)| b * t).sum::<f32>();
        }
    }
}

/// The adapters of one block, beside its attention's five projections.
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub q: Pair,
    pub k: Pair,
    pub v: Pair,
    pub gate: Pair,
    pub out: Pair,
}

/// What claudius was taught with: the adapters of each block of the depth they were taught at, and their scale.
#[derive(Clone, Debug, Default)]
pub struct Adapters {
    pub depth: usize,
    pub scale: f32,
    pub blocks: Vec<Block>,
}

impl Adapters {
    /// The adapters in a safetensors file as the trainer writes them: `b{block}.{proj}.a` and `.b` for each of the
    /// five projections, and `depth`, `rank` and `scale` in its metadata.
    pub fn from_safetensors(bytes: Vec<u8>) -> Result<Adapters, String> {
        let st = needle_infer::safetensors::SafeTensors::from_bytes(bytes).map_err(|e| e.to_string())?;
        let meta = |k: &str| st.get_metadata(k).and_then(|v| v.parse::<f32>().ok()).ok_or(format!("no {k}"));
        let (depth, rank, scale) = (meta("depth")? as usize, meta("rank")? as usize, meta("scale")?);
        let pair = |b: usize, p: &str| -> Result<Pair, String> {
            let get = |m: &str| st.get_f32(&format!("b{b}.{p}.{m}")).ok_or(format!("no b{b}.{p}.{m}"));
            Ok(Pair { a: get("a")?, b: get("b")?, rank })
        };
        let blocks = (0..depth)
            .map(|b| {
                Ok(Block {
                    q: pair(b, "q")?,
                    k: pair(b, "k")?,
                    v: pair(b, "v")?,
                    gate: pair(b, "gate")?,
                    out: pair(b, "out")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Adapters { depth, scale, blocks })
    }
}

/// The cache as the tools leave it, to rewind to: where they end, the tokens the n-gram hash reads, and the last
/// pre-convolution vectors of each block's q, k and v and of each Engram site.
struct Tools {
    json: String,
    ids: Vec<u32>,
    pos: usize,
    tokens: Vec<u32>,
    conv: Vec<[Vec<f32>; 3]>,
    engram: Vec<Vec<f32>>,
}

/// the last pre-convolution vectors a step saw, kept while the tools are read
#[derive(Default)]
struct Tails {
    conv: Vec<[Vec<Vec<f32>>; 3]>,
    engram: Vec<Vec<Vec<f32>>>,
}

/// Needle 3 at a depth, with its tokenizer, its cache, the tools it has read, and its adapters.
pub struct Needle {
    model: V3Model,
    tok: SpTokenizer,
    cache: V3Cache,
    tools: Option<Tools>,
    adapters: Option<Adapters>,
    marks: (Option<u32>, Option<u32>, Option<u32>),
}

impl Needle {
    /// The model in the bytes of a `needle3.cact`, at `depth` blocks of its ladder (2 to 20).
    pub fn from_bytes(bytes: Vec<u8>, depth: usize) -> Result<Needle, String> {
        let cact = CactV3::from_bytes(bytes).map_err(|e| format!("{e:?}"))?;
        let model = needle_infer::v3::model_from_cact_at_depth(&cact, depth).map_err(|e| format!("{e:?}"))?;
        let blob = cact.tokenizer_blob().ok_or("no tokenizer in the file")?;
        let tok = SpTokenizer::from_blob(blob).map_err(|e| format!("{e:?}"))?;
        let cache = V3Cache::with_precision(&model.cfg, 1024, KvPrecision::F32);
        let marks = (tok.id_of(TOOL_CALL_START), tok.id_of(TOOL_CALL_END), tok.id_of(IM_END));
        Ok(Needle { model, tok, cache, tools: None, adapters: None, marks })
    }

    /// how many blocks it runs
    pub fn depth(&self) -> usize {
        self.model.cfg.num_layers
    }

    /// The adapters claudius was taught with, which must be of this depth.
    pub fn adapt(&mut self, adapters: Adapters) -> Result<(), String> {
        if adapters.depth != self.depth() || adapters.blocks.len() != self.depth() {
            return Err(format!("adapters taught at depth {}, the model runs {}", adapters.depth, self.depth()));
        }
        self.adapters = Some(adapters);
        Ok(())
    }

    /// the tokens of the prompt for `query` with `tools`, the first one the sequence's start
    pub fn prompt_ids(&self, query: &str, tools: &str) -> Vec<u32> {
        let mut ids = vec![BOS];
        ids.extend(self.tok.encode(&build_prompt(query, tools, None)));
        ids
    }

    /// The completion for `query` with `tools`: the reasoning, and the call between its markers, which the grammar
    /// holds to the tools. The tools are read once; a request after them is read with the adapters.
    pub fn complete(&mut self, query: &str, tools: &str, most: usize) -> String {
        let ids = self.prompt_ids(query, tools);
        let read = self.tools.as_ref().is_some_and(|t| t.json == tools && ids.starts_with(&t.ids));
        // the local blocks keep a window of 1024 positions; a rewind past it would find the tools overwritten
        let fits = |t: &Tools| ids.len() + most < 1024 && t.ids.len() < ids.len();
        if !read || !self.tools.as_ref().is_some_and(fits) {
            self.read_tools(&ids, tools);
        }
        let Some(t) = &self.tools else { return String::new() };
        let from = t.ids.len();
        self.rewind();
        let mut logits = Vec::new();
        for &id in &ids[from..] {
            logits = self.step(id, true, None);
        }
        self.generate(logits, tools, most)
    }

    /// reads the tools part of a prompt (everything up to the request) without the adapters, keeping what a rewind needs
    fn read_tools(&mut self, ids: &[u32], tools: &str) {
        let whole = build_prompt("", tools, None);
        let head = &whole[..whole.find("</tools>\n").map_or(whole.len(), |i| i + "</tools>\n".len())];
        let mut prefix = vec![BOS];
        prefix.extend(self.tok.encode(head));
        if !ids.starts_with(&prefix) {
            prefix.truncate(1);
        }
        let cfg = self.model.cfg.clone();
        self.cache = V3Cache::with_precision(&cfg, 1024, KvPrecision::F32);
        let mut tails = Tails {
            conv: (0..cfg.num_layers).map(|_| Default::default()).collect(),
            engram: vec![Vec::new(); cfg.engram.sites.len()],
        };
        for &id in &prefix {
            self.step(id, false, Some(&mut tails));
        }
        let taps = cfg.qkv_conv_taps.saturating_sub(1);
        let reach = cfg.engram.conv_taps.saturating_sub(1) * cfg.engram.conv_dilation;
        let last = |v: &Vec<Vec<f32>>, n: usize, dim: usize| -> Vec<f32> {
            // oldest first; positions before the sequence read as zero
            let mut out = vec![0.0f32; n * dim];
            let have = v.len().min(n);
            for (i, row) in v[v.len() - have..].iter().enumerate() {
                out[(n - have + i) * dim..(n - have + i + 1) * dim].copy_from_slice(row);
            }
            out
        };
        let dims = [cfg.q_dim(), cfg.k_dim(), cfg.v_dim()];
        let conv = tails.conv.iter().map(|c| [0, 1, 2].map(|i| last(&c[i], taps, dims[i]))).collect();
        let engram = tails.engram.iter().map(|e| last(e, reach, cfg.d_model)).collect();
        self.tools = Some(Tools {
            json: tools.to_string(),
            ids: prefix,
            pos: self.cache.pos(),
            tokens: self.cache.token_history().to_vec(),
            conv,
            engram,
        });
    }

    /// the cache back to where the tools end
    fn rewind(&mut self) {
        let Some(t) = &self.tools else { return };
        self.cache.set_pos(t.pos);
        self.cache.seed_tokens(&t.tokens);
        for (li, c) in t.conv.iter().enumerate() {
            for (which, raw) in [Qkv::Q, Qkv::K, Qkv::V].into_iter().zip(c) {
                if !raw.is_empty() {
                    self.cache.seed_conv_tail(li, which, raw);
                }
            }
        }
        for (s, raw) in t.engram.iter().enumerate() {
            if !raw.is_empty() {
                self.cache.seed_engram_tail(s, raw);
            }
        }
    }

    /// greedy decoding from `logits`, the call held to `tools` by the grammar, until the turn ends or `most` tokens
    fn generate(&mut self, mut logits: Vec<f32>, tools: &str, most: usize) -> String {
        let defs = ToolDef::from_json(tools);
        let mut grammar = ConstrainedDecoder::new(&defs, byte_table(&self.tok)).with_unique_arg_keys();
        let (start, end, im_end) = self.marks;
        let rows = self.model.cfg.logit_rows();
        let (mut out, mut inside) = (Vec::new(), false);
        for _ in 0..most {
            if inside {
                for (l, m) in logits.iter_mut().zip(grammar.logit_mask(rows)) {
                    *l += m;
                }
            }
            let next = logits.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(EOS, |(i, _)| i as u32);
            if next == EOS || Some(next) == im_end {
                break;
            }
            out.push(next);
            if Some(next) == start {
                inside = true;
            } else if Some(next) == end {
                inside = false;
            } else if inside {
                grammar.update(next);
            }
            logits = self.step(next, true, None);
        }
        self.tok.decode(&out)
    }

    /// One position: `V3Model::decode_step`, with the adapters added when `adapted`, and the pre-convolution vectors
    /// kept in `tails` while the tools are read.
    fn step(&mut self, token: u32, adapted: bool, mut tails: Option<&mut Tails>) -> Vec<f32> {
        let m = &self.model;
        let cfg = &m.cfg;
        let cache = &mut self.cache;
        let adapters = self.adapters.as_ref().filter(|_| adapted);
        let (d, n) = (cfg.d_model, cfg.mhc_lanes);
        let pos = cache.pos();
        cache.push_token(token);

        // the Engram for this position, from the last tokens
        let e = &cfg.engram;
        let hist = cache.token_history().to_vec();
        let here = hist.len() - 1;
        let idx = engram_indices(&hist, &e.orders, e.heads, e.slots as u32, e.seed_heads);
        let tables = e.num_tables();
        let mut ek = vec![0.0f32; e.sites.len() * d];
        let mut ev = vec![0.0f32; e.sites.len() * d];
        let mut fetched = vec![0.0f32; tables * e.sub_dim];
        let mut row = vec![0.0f32; e.sub_dim];
        for (s, site) in m.engrams.iter().enumerate() {
            for t in 0..tables {
                let dst = &mut fetched[t * e.sub_dim..(t + 1) * e.sub_dim];
                if ngram_valid(pos, t, &e.orders, e.heads) {
                    site.tables.dequantize_row(t * e.slots + idx[here * tables + t] as usize, &mut row);
                    dst.copy_from_slice(&row);
                } else {
                    dst.fill(0.0);
                }
            }
            site.key_proj.matvec(&fetched, &mut ek[s * d..(s + 1) * d]);
            site.value_proj.matvec(&fetched, &mut ev[s * d..(s + 1) * d]);
            let mut cur = ev[s * d..(s + 1) * d].to_vec();
            if let Some(t) = tails.as_deref_mut() {
                t.engram[s].push(cur.clone());
            }
            cache.engram_conv_step(s, &site.taps, &mut cur);
            ev[s * d..(s + 1) * d].copy_from_slice(&cur);
        }

        // the lanes start from the token's embedding
        let mut emb = vec![0.0f32; d];
        m.embedding.dequantize_row((token as usize).min(cfg.vocab_size - 1), &mut emb);
        let scale = sqrt(d as f32);
        emb.iter_mut().for_each(|x| *x *= scale);
        let mut lanes = emb.repeat(n);

        let dims = AttnDims {
            seq: 1,
            num_heads: cfg.num_heads,
            num_kv_heads: cfg.num_kv_heads,
            qk_head_dim: cfg.qk_head_dim,
            v_head_dim: cfg.v_head_dim,
        };
        let half = cfg.qk_head_dim / 2;
        let (rc, rs): (Vec<f32>, Vec<f32>) = (0..half)
            .map(|i| {
                let angle = pos as f32 * (1.0 / powf(cfg.rope_theta, (2 * i) as f32 / cfg.qk_head_dim as f32));
                (cos(angle), sin(angle))
            })
            .unzip();

        let mut nx = vec![0.0f32; n * d];
        let mut nx_prep = vec![0.0f32; m.mhc.phi_pre.prepared_len()];
        let mut hpre = vec![0.0f32; n];
        let mut hpost = vec![0.0f32; n];
        let mut hres = vec![0.0f32; n * n];
        let mut scratch = vec![0.0f32; n * d];
        let mut u = vec![0.0f32; d];
        let mut q = vec![0.0f32; cfg.q_dim()];
        let mut k = vec![0.0f32; cfg.k_dim()];
        let mut v = vec![0.0f32; cfg.v_dim()];
        let mut attn = vec![0.0f32; cfg.attn_out_dim()];
        let mut gate = vec![0.0f32; cfg.attn_out_dim()];
        let mut proj = vec![0.0f32; d];
        let mut mlp_out = vec![0.0f32; d];

        for li in 0..cfg.num_layers {
            let layer = &m.layers[li];
            let lora = adapters.map(|a| (&a.blocks[li], a.scale));
            let mhc = MhcLayer {
                a_pre: m.mhc.a_pre[li],
                a_post: m.mhc.a_post[li],
                a_res: m.mhc.a_res[li],
                b_pre: &m.mhc.b_pre[li * n..(li + 1) * n],
                b_post: &m.mhc.b_post[li * n..(li + 1) * n],
                b_res: &m.mhc.b_res[li * n * n..(li + 1) * n * n],
            };
            rms_unit_to(&lanes, &mut nx);
            m.mhc.phi_pre.prepare_input(&nx, &mut nx_prep);
            m.mhc.phi_pre.matvec_rows_prepared(&nx_prep, li * n, &mut hpre);
            mix_down(&lanes, &mut hpre, &mhc, li, n, d, &mut u);

            let mut bx = u.clone();
            if let Some(site) = cfg.engram.site_of(li) {
                apply_site(&mut bx, &ek[site * d..(site + 1) * d], &ev[site * d..(site + 1) * d], d);
            }

            let mut h = bx.clone();
            zc_rms_norm_vec(&mut h, &layer.norm_in);
            layer.q_proj.matvec(&h, &mut q);
            layer.k_proj.matvec(&h, &mut k);
            layer.v_proj.matvec(&h, &mut v);
            if let Some((b, s)) = lora {
                b.q.add(&h, &mut q, s);
                b.k.add(&h, &mut k, s);
                b.v.add(&h, &mut v, s);
            }
            if cfg.qkv_conv_taps > 0 {
                if let Some(t) = tails.as_deref_mut() {
                    t.conv[li][0].push(q.clone());
                    t.conv[li][1].push(k.clone());
                    t.conv[li][2].push(v.clone());
                }
                cache.conv_step(li, Qkv::Q, &layer.q_taps, &mut q);
                cache.conv_step(li, Qkv::K, &layer.k_taps, &mut k);
                cache.conv_step(li, Qkv::V, &layer.v_taps, &mut v);
            }
            norm_and_rope(&mut q, &layer.q_norm, &rc, &rs, 1, cfg.num_heads, cfg.qk_head_dim);
            norm_and_rope(&mut k, &layer.k_norm, &rc, &rs, 1, cfg.num_kv_heads, cfg.qk_head_dim);
            cache.write_kv(li, pos, &k, &v);
            let (lo, hi) = cache.span(li, pos);
            let slots = cache.slots(li);
            attend_step(&q, Ring { kv: cache.kv(li), slots, lo, hi }, dims, &mut attn);

            layer.gate_proj.matvec(&h, &mut gate);
            if let Some((b, s)) = lora {
                b.gate.add(&h, &mut gate, s);
            }
            for (ai, &gi) in attn.iter_mut().zip(gate.iter()) {
                *ai *= sigmoid(gi);
            }
            layer.out_proj.matvec(&attn, &mut proj);
            if let Some((b, s)) = lora {
                b.out.add(&attn, &mut proj, s);
            }
            zc_rms_norm_vec(&mut proj, &layer.post_norm);
            let agate = sigmoid(layer.attn_gate);
            for (si, &pi) in bx.iter_mut().zip(proj.iter()) {
                *si += agate * pi;
            }

            proj.copy_from_slice(&bx);
            zc_rms_norm_vec(&mut proj, &layer.pre_hada);
            hadamard_mlp(&proj, &layer.mlp, &m.perms, cfg.hada_n, &mut mlp_out);
            for (si, &mi) in bx.iter_mut().zip(mlp_out.iter()) {
                *si += mi;
            }
            let y: Vec<f32> = bx.iter().zip(&u).map(|(b, u)| b - u).collect();

            rms_unit_to(&lanes, &mut nx);
            m.mhc.phi_post.prepare_input(&nx, &mut nx_prep);
            m.mhc.phi_post.matvec_rows_prepared(&nx_prep, li * n, &mut hpost);
            m.mhc.phi_res.prepare_input(&nx, &mut nx_prep);
            m.mhc.phi_res.matvec_rows_prepared(&nx_prep, li * n * n, &mut hres);
            scatter_up(
                &mut lanes,
                Gates { post: &mut hpost, res: &mut hres },
                &y,
                &mhc,
                LaneSite { layer: li, lanes: n, d_model: d },
                &mut scratch,
            );
        }
        cache.advance();

        let mut x = vec![0.0f32; d];
        for (c, xc) in x.iter_mut().enumerate() {
            *xc = (0..n).map(|lane| lanes[lane * d + c]).sum::<f32>() / n as f32;
        }
        zc_rms_norm_vec(&mut x, &m.final_norm);
        let mut prep = vec![0.0f32; m.embedding.prepared_len()];
        let mut full = vec![0.0f32; cfg.vocab_size];
        m.embedding.prepare_input(&x, &mut prep);
        m.embedding.matvec_prepared(&prep, &mut full);
        full.truncate(cfg.logit_rows());
        full
    }

    /// the logits after each of `ids`, from an empty cache and without the adapters: for comparing with needle-core
    pub fn logits_of(&mut self, ids: &[u32]) -> Vec<Vec<f32>> {
        self.cache = V3Cache::with_precision(&self.model.cfg, 1024, KvPrecision::F32);
        self.tools = None;
        ids.iter().map(|&id| self.step(id, false, None)).collect()
    }
}
