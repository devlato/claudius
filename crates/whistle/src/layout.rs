//! The Whistle tensor canon over needle-infer's `CactV3`.
//!
//! A Whistle archive is a Needle 3 container whose decoder part (records
//! 0..=236 for the shipped file) is the Needle 3 canon, followed by the audio
//! manifest, the cross-attention, `pe_gate`, the encoder, the stem, the mel
//! filterbank and the tokenizer (`export.py`: `_tensors`, `_audio_tensors`).
//! The directory is nameless, so every slot is derived from the header and
//! the manifest and checked for dtype and shape before anything is read.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;

use needle_core::cq::CqWeight;
use needle_core::rope::RopeCache;
use needle_core::v3::{HadaMlp, HadaPerms, V3Config, V3EngramSite, V3Layer, V3Mhc, hada_blocks};
use needle_infer::cact::{CactV3, DT_CQ, DT_FP16, DT_FP32, DT_RAW};
use needle_infer::sp_tokenizer::SpTokenizer;
use needle_infer::v3::config_from_geometry;

use crate::decoder::{CrossLayer, Decoder};
use crate::encoder::{Dims, EncBlock, Encoder, Stem};

/// First value of `audio.manifest`.
pub const MANIFEST_MAGIC: f32 = 3246.0;
/// Values in `audio.manifest`.
pub const MANIFEST_LEN: usize = 13;
/// `HADA_COND_RANK` in `architecture.py`.
pub const HADA_COND_RANK: usize = 8;

const HADA: [&str; 13] = ["d1", "d2", "b2", "d3", "d4", "w1a", "w1b", "w2a", "w2b", "w3a", "w3b", "cond_v", "cond_u"];
const MHC: [&str; 9] = [
    "mhc_a_pre",
    "mhc_a_post",
    "mhc_a_res",
    "mhc_b_pre",
    "mhc_b_post",
    "mhc_b_res",
    "mhc_phi_pre",
    "mhc_phi_post",
    "mhc_phi_res",
];
const ENC_NORMS: [&str; 6] =
    ["pre_hada_norm_0", "ZCRMSNorm_0", "post_attn_norm", "conv_norm", "conv_out_norm", "pre_hada_norm"];

#[derive(Debug, Clone)]
pub struct LoadError(pub String);

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

fn err<T>(msg: impl Into<String>) -> Result<T, LoadError> {
    Err(LoadError(msg.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    F16,
    F32,
    Cq,
    Raw,
}

impl Kind {
    fn code(self) -> u8 {
        match self {
            Kind::F16 => DT_FP16,
            Kind::F32 => DT_FP32,
            Kind::Cq => DT_CQ,
            Kind::Raw => DT_RAW,
        }
    }
}

pub struct Slot {
    pub name: String,
    pub kind: Kind,
    pub shape: Vec<usize>,
}

/// `audio.manifest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: usize,
    pub text_vocab: usize,
    pub enc_layers: usize,
    pub enc_lanes: usize,
    pub conv_kernel: usize,
    pub stem_channels: usize,
    pub stem_stages: usize,
    pub mels: usize,
    pub sample_rate: usize,
    pub fft: usize,
    pub win: usize,
    pub hop: usize,
}

impl Manifest {
    fn parse(v: &[f32]) -> Result<Self, LoadError> {
        if v.len() != MANIFEST_LEN || v[0] != MANIFEST_MAGIC {
            return err(format!(
                "audio.manifest is {v:?}, not a {MANIFEST_LEN}-value manifest led by {MANIFEST_MAGIC}"
            ));
        }
        let mut u = [0usize; MANIFEST_LEN];
        for (o, &x) in u.iter_mut().zip(v) {
            if x < 0.0 || x.fract() != 0.0 {
                return err(format!("audio.manifest value {x} is not a count"));
            }
            *o = x as usize;
        }
        let m = Self {
            version: u[1],
            text_vocab: u[2],
            enc_layers: u[3],
            enc_lanes: u[4],
            conv_kernel: u[5],
            stem_channels: u[6],
            stem_stages: u[7],
            mels: u[8],
            sample_rate: u[9],
            fft: u[10],
            win: u[11],
            hop: u[12],
        };
        if m.version != 1 {
            return err(format!("audio manifest version {} (this reader knows 1)", m.version));
        }
        if m.sample_rate != 16000 || m.win > m.fft || m.stem_stages == 0 || m.conv_kernel.is_multiple_of(2) {
            return err(format!("unsupported audio geometry {m:?}"));
        }
        Ok(m)
    }

    /// Mel bands left after the stem's stride-2 stages.
    pub fn stem_mels(&self) -> usize {
        (0..self.stem_stages).fold(self.mels, |f, _| f.div_ceil(2))
    }
}

/// What the check found, for the stage-1 report.
pub struct Report {
    pub records: usize,
    pub by_kind: BTreeMap<String, (usize, u64)>,
    pub manifest_index: usize,
    pub decoder_records: usize,
}

struct Canon {
    slots: Vec<Slot>,
    index: HashMap<String, usize>,
}

fn push(s: &mut Vec<Slot>, name: impl Into<String>, kind: Kind, shape: &[usize]) {
    s.push(Slot { name: name.into(), kind, shape: shape.to_vec() });
}

fn hada_slots(s: &mut Vec<Slot>, prefix: &str, d: usize, n: usize) {
    let (first, second) = hada_blocks(n);
    for name in &HADA[..5] {
        push(s, format!("{prefix}{name}"), Kind::F16, &[n]);
    }
    let blocks = [("w1a", first), ("w1b", second), ("w2a", first), ("w2b", second), ("w3a", first), ("w3b", second)];
    for (name, b) in blocks {
        push(s, format!("{prefix}{name}"), Kind::F16, &[b, b]);
    }
    push(s, format!("{prefix}cond_v"), Kind::F16, &[d, HADA_COND_RANK]);
    push(s, format!("{prefix}cond_u"), Kind::F16, &[HADA_COND_RANK, n]);
}

fn mhc_slots(s: &mut Vec<Slot>, prefix: &str, layers: usize, lanes: usize, d: usize) {
    let w = lanes * d;
    let shapes: [Vec<usize>; 9] = [
        vec![layers],
        vec![layers],
        vec![layers],
        vec![layers, lanes],
        vec![layers, lanes],
        vec![layers, lanes, lanes],
        vec![layers * lanes, w],
        vec![layers * lanes, w],
        vec![layers * lanes * lanes, w],
    ];
    for (i, (name, shape)) in MHC.iter().zip(shapes).enumerate() {
        let kind = if i < 6 { Kind::F16 } else { Kind::Cq };
        push(s, format!("{prefix}{name}"), kind, &shape);
    }
}

/// The Needle 3 decoder canon (`export._tensors`), through `final_norm`.
fn decoder_slots(c: &V3Config) -> Vec<Slot> {
    let mut s = Vec::new();
    let d = c.d_model;
    push(&mut s, "embedding", Kind::Cq, &[c.vocab_size, d]);
    for i in 0..c.num_layers {
        let p = format!("layer{i:02}.");
        push(&mut s, format!("{p}norm_in"), Kind::F16, &[d]);
        push(&mut s, format!("{p}q_proj"), Kind::Cq, &[c.q_dim(), d]);
        push(&mut s, format!("{p}k_proj"), Kind::Cq, &[c.k_dim(), d]);
        push(&mut s, format!("{p}v_proj"), Kind::Cq, &[c.v_dim(), d]);
        if c.qkv_conv_taps > 0 {
            let t = c.qkv_conv_taps;
            push(&mut s, format!("{p}q_taps"), Kind::F16, &[t, c.q_dim()]);
            push(&mut s, format!("{p}k_taps"), Kind::F16, &[t, c.k_dim()]);
            push(&mut s, format!("{p}v_taps"), Kind::F16, &[t, c.v_dim()]);
        }
        push(&mut s, format!("{p}q_norm"), Kind::F16, &[c.qk_head_dim]);
        push(&mut s, format!("{p}k_norm"), Kind::F16, &[c.qk_head_dim]);
        push(&mut s, format!("{p}gate_proj"), Kind::Cq, &[c.attn_out_dim(), d]);
        push(&mut s, format!("{p}out_proj"), Kind::Cq, &[d, c.attn_out_dim()]);
        push(&mut s, format!("{p}post_norm"), Kind::F16, &[d]);
        push(&mut s, format!("{p}attn_gate"), Kind::F16, &[1]);
        push(&mut s, format!("{p}pre_hada"), Kind::F16, &[d]);
        hada_slots(&mut s, &p, d, c.hada_n);
    }
    mhc_slots(&mut s, "", c.num_layers, c.mhc_lanes, d);
    push(&mut s, "hada_p1", Kind::F32, &[c.hada_n]);
    push(&mut s, "hada_p2", Kind::F32, &[c.hada_n]);
    let e = &c.engram;
    let e_in = e.num_tables() * e.sub_dim;
    for site in 0..e.sites.len() {
        let p = format!("engram{site}.");
        push(&mut s, format!("{p}tables"), Kind::Cq, &[e.num_tables() * e.slots, e.sub_dim]);
        push(&mut s, format!("{p}key_proj"), Kind::Cq, &[d, e_in]);
        push(&mut s, format!("{p}value_proj"), Kind::Cq, &[d, e_in]);
        push(&mut s, format!("{p}taps"), Kind::F16, &[e.conv_taps, d]);
    }
    push(&mut s, "final_norm", Kind::F16, &[d]);
    s
}

/// The speech part (`export._audio_tensors`) and the tokenizer.
fn audio_slots(c: &V3Config, m: &Manifest) -> Vec<Slot> {
    let mut s = Vec::new();
    let d = c.d_model;
    let (hq, hv) = (c.num_heads * c.qk_head_dim, c.num_heads * c.v_head_dim);
    push(&mut s, "audio.manifest", Kind::F32, &[MANIFEST_LEN]);
    for i in 0..c.num_layers {
        let p = format!("layer{i:02}.");
        push(&mut s, format!("{p}cross_norm"), Kind::F16, &[d]);
        push(&mut s, format!("{p}cross.q_proj"), Kind::Cq, &[hq, d]);
        push(&mut s, format!("{p}cross.k_proj"), Kind::Cq, &[hq, d]);
        push(&mut s, format!("{p}cross.v_proj"), Kind::Cq, &[hv, d]);
        push(&mut s, format!("{p}cross.gate_proj"), Kind::Cq, &[hv, d]);
        push(&mut s, format!("{p}cross.out_proj"), Kind::Cq, &[d, hv]);
        push(&mut s, format!("{p}cross_q_norm"), Kind::F16, &[c.qk_head_dim]);
        push(&mut s, format!("{p}cross_k_norm"), Kind::F16, &[c.qk_head_dim]);
        push(&mut s, format!("{p}post_cross_norm"), Kind::F16, &[d]);
        push(&mut s, format!("{p}cross_gate"), Kind::F16, &[1]);
    }
    push(&mut s, "pe_gate", Kind::F16, &[1]);
    for i in 0..m.enc_layers {
        let p = format!("encoder{i:02}.");
        for name in ENC_NORMS {
            push(&mut s, format!("{p}{name}"), Kind::F16, &[d]);
        }
        push(&mut s, format!("{p}q_proj"), Kind::Cq, &[c.q_dim(), d]);
        push(&mut s, format!("{p}k_proj"), Kind::Cq, &[c.k_dim(), d]);
        push(&mut s, format!("{p}v_proj"), Kind::Cq, &[c.v_dim(), d]);
        push(&mut s, format!("{p}gate_proj"), Kind::Cq, &[c.attn_out_dim(), d]);
        push(&mut s, format!("{p}out_proj"), Kind::Cq, &[d, c.attn_out_dim()]);
        push(&mut s, format!("{p}q_norm"), Kind::F16, &[c.qk_head_dim]);
        push(&mut s, format!("{p}k_norm"), Kind::F16, &[c.qk_head_dim]);
        push(&mut s, format!("{p}attn_gate"), Kind::F16, &[1]);
        push(&mut s, format!("{p}pw1"), Kind::Cq, &[2 * d, d]);
        push(&mut s, format!("{p}pw2"), Kind::Cq, &[d, d]);
        push(&mut s, format!("{p}dw"), Kind::F16, &[m.conv_kernel, d]);
        hada_slots(&mut s, &format!("{p}hadamard_mlp_0."), d, c.hada_n);
        hada_slots(&mut s, &format!("{p}hadamard_mlp."), d, c.hada_n);
    }
    mhc_slots(&mut s, "encoder.", m.enc_layers, m.enc_lanes, d);
    push(&mut s, "encoder.final_norm", Kind::F16, &[d]);
    let ch = m.stem_channels;
    push(&mut s, "stem.w", Kind::F16, &[9, ch]);
    for i in 1..m.stem_stages {
        push(&mut s, format!("stem.dw_{i}"), Kind::F16, &[9, ch]);
        push(&mut s, format!("stem.pw_{i}"), Kind::Cq, &[ch, ch]);
    }
    push(&mut s, "stem.out", Kind::Cq, &[d, ch * m.stem_mels()]);
    push(&mut s, "mel", Kind::F32, &[m.fft / 2 + 1, m.mels]);
    push(&mut s, "tokenizer", Kind::Raw, &[]);
    s
}

impl Canon {
    fn new(slots: Vec<Slot>) -> Self {
        let index = slots.iter().enumerate().map(|(i, s)| (s.name.clone(), i)).collect();
        Self { slots, index }
    }

    /// Every record against its slot: the count, the dtype, the shape.
    fn check(&self, cact: &CactV3, manifest_index: usize, decoder_records: usize) -> Result<Report, LoadError> {
        if cact.num_tensors() != self.slots.len() {
            return err(format!(
                "directory has {} records, the canon for this header and manifest has {}",
                cact.num_tensors(),
                self.slots.len()
            ));
        }
        let mut by_kind = BTreeMap::new();
        for (i, s) in self.slots.iter().enumerate() {
            let r = cact.record(i);
            if r.dtype != s.kind.code() {
                return err(format!("record {i} ({}) has dtype {}, want {:?}", s.name, r.dtype, s.kind));
            }
            let got = &r.shape[..r.ndim as usize];
            if got != s.shape.as_slice() {
                return err(format!("record {i} ({}) has shape {got:?}, want {:?}", s.name, s.shape));
            }
            let label = match s.kind {
                Kind::Cq => format!("CQ{}", r.bits),
                Kind::F16 => "FP16".into(),
                Kind::F32 => "FP32".into(),
                Kind::Raw => "RAW".into(),
            };
            let e = by_kind.entry(label).or_insert((0usize, 0u64));
            e.0 += 1;
            e.1 += r.nbytes;
        }
        Ok(Report { records: self.slots.len(), by_kind, manifest_index, decoder_records })
    }

    fn at(&self, name: &str) -> usize {
        *self.index.get(name).unwrap_or_else(|| panic!("no canon slot {name}"))
    }
}

struct Loader<'a> {
    cact: &'a CactV3,
    canon: &'a Canon,
}

impl Loader<'_> {
    fn f(&self, name: &str) -> Result<Vec<f32>, LoadError> {
        self.cact.floats(self.canon.at(name)).map_err(|e| LoadError(format!("{name}: {e}")))
    }

    fn scalar(&self, name: &str) -> Result<f32, LoadError> {
        Ok(self.f(name)?[0])
    }

    fn q(&self, name: &str) -> Result<CqWeight, LoadError> {
        self.cact.cq(self.canon.at(name)).map_err(|e| LoadError(format!("{name}: {e}")))
    }

    fn mlp(&self, prefix: &str) -> Result<HadaMlp, LoadError> {
        let g = |n: &str| self.f(&format!("{prefix}{n}"));
        Ok(HadaMlp {
            d1: g("d1")?,
            d2: g("d2")?,
            b2: g("b2")?,
            d3: g("d3")?,
            d4: g("d4")?,
            w1: (g("w1a")?, g("w1b")?),
            w2: (g("w2a")?, g("w2b")?),
            w3: (g("w3a")?, g("w3b")?),
            cond_v: g("cond_v")?,
            cond_u: g("cond_u")?,
            cond_rank: HADA_COND_RANK,
        })
    }

    fn mhc(&self, prefix: &str) -> Result<V3Mhc, LoadError> {
        let g = |n: &str| self.f(&format!("{prefix}{n}"));
        let q = |n: &str| self.q(&format!("{prefix}{n}"));
        Ok(V3Mhc {
            a_pre: g("mhc_a_pre")?,
            a_post: g("mhc_a_post")?,
            a_res: g("mhc_a_res")?,
            b_pre: g("mhc_b_pre")?,
            b_post: g("mhc_b_post")?,
            b_res: g("mhc_b_res")?,
            phi_pre: q("mhc_phi_pre")?,
            phi_post: q("mhc_phi_post")?,
            phi_res: q("mhc_phi_res")?,
        })
    }
}

/// Everything a transcription needs, loaded and checked.
pub struct Loaded {
    pub cfg: V3Config,
    pub man: Manifest,
    pub dec: Decoder,
    pub enc: Encoder,
    pub mel: Vec<f32>,
    pub perms: HadaPerms,
    pub tok: SpTokenizer,
    pub languages: Vec<String>,
    pub report: Report,
}

/// Parse, check and load a Whistle `.cact`.
pub fn load(bytes: Vec<u8>) -> Result<Loaded, LoadError> {
    let cact = CactV3::from_bytes(bytes).map_err(|e| LoadError(format!("cact: {e}")))?;
    let cfg = config_from_geometry(&cact.geom).map_err(|e| LoadError(format!("header: {e}")))?;

    let mut slots = decoder_slots(&cfg);
    let decoder_records = slots.len();
    let mi = decoder_records;
    if mi >= cact.num_tensors() {
        return err("no record after the decoder: a text model, not Whistle");
    }
    let r = cact.record(mi);
    if r.dtype != DT_FP32 || r.ndim != 1 || r.shape[0] != MANIFEST_LEN {
        return err(format!("record {mi} is not an audio manifest: a text model, not Whistle"));
    }
    let man = Manifest::parse(&cact.floats(mi).map_err(|e| LoadError(e.to_string()))?)?;
    if cfg.num_heads * cfg.v_head_dim != cfg.d_model {
        return err("encoder heads are d_model / v_head_dim, which disagrees with the header");
    }
    slots.extend(audio_slots(&cfg, &man));
    let canon = Canon::new(slots);
    let report = canon.check(&cact, mi, decoder_records)?;
    let ld = Loader { cact: &cact, canon: &canon };

    // Decoder: needle-core's own layer, mHC and Engram types.
    let mut layers = Vec::with_capacity(cfg.num_layers);
    let mut cross = Vec::with_capacity(cfg.num_layers);
    for i in 0..cfg.num_layers {
        let p = format!("layer{i:02}.");
        let g = |n: &str| ld.f(&format!("{p}{n}"));
        let q = |n: &str| ld.q(&format!("{p}{n}"));
        let taps = cfg.qkv_conv_taps > 0;
        layers.push(V3Layer {
            norm_in: g("norm_in")?,
            q_proj: q("q_proj")?,
            k_proj: q("k_proj")?,
            v_proj: q("v_proj")?,
            q_taps: if taps { g("q_taps")? } else { Vec::new() },
            k_taps: if taps { g("k_taps")? } else { Vec::new() },
            v_taps: if taps { g("v_taps")? } else { Vec::new() },
            q_norm: g("q_norm")?,
            k_norm: g("k_norm")?,
            gate_proj: q("gate_proj")?,
            out_proj: q("out_proj")?,
            post_norm: g("post_norm")?,
            attn_gate: ld.scalar(&format!("{p}attn_gate"))?,
            pre_hada: g("pre_hada")?,
            mlp: ld.mlp(&p)?,
        });
        cross.push(CrossLayer {
            norm: g("cross_norm")?,
            q_proj: q("cross.q_proj")?,
            k_proj: q("cross.k_proj")?,
            v_proj: q("cross.v_proj")?,
            gate_proj: q("cross.gate_proj")?,
            out_proj: q("cross.out_proj")?,
            q_norm: g("cross_q_norm")?,
            k_norm: g("cross_k_norm")?,
            post_norm: g("post_cross_norm")?,
            gate: ld.scalar(&format!("{p}cross_gate"))?,
        });
    }
    let mut engrams = Vec::with_capacity(cfg.engram.sites.len());
    for s in 0..cfg.engram.sites.len() {
        let p = format!("engram{s}.");
        engrams.push(V3EngramSite {
            tables: ld.q(&format!("{p}tables"))?,
            key_proj: ld.q(&format!("{p}key_proj"))?,
            value_proj: ld.q(&format!("{p}value_proj"))?,
            taps: ld.f(&format!("{p}taps"))?,
        });
    }
    let to_perm = |v: Vec<f32>| -> Vec<u32> { v.into_iter().map(|x| x as u32).collect() };
    let perms = HadaPerms { p1: to_perm(ld.f("hada_p1")?), p2: to_perm(ld.f("hada_p2")?) };
    for p in [&perms.p1, &perms.p2] {
        let mut sorted = p.clone();
        sorted.sort_unstable();
        if sorted.iter().enumerate().any(|(i, &v)| v as usize != i) {
            return err("hada_p1/hada_p2 are not permutations");
        }
    }
    let dec = Decoder {
        cfg: cfg.clone(),
        embedding: ld.q("embedding")?,
        layers,
        cross,
        mhc: ld.mhc("")?,
        engrams,
        final_norm: ld.f("final_norm")?,
        pe_gate: ld.scalar("pe_gate")?,
        rope: RopeCache::new(cfg.max_seq_len, cfg.qk_head_dim, cfg.rope_theta),
    };

    // Encoder.
    let dims = Dims {
        d: cfg.d_model,
        heads: cfg.num_heads,
        kv_heads: cfg.num_kv_heads,
        qk: cfg.qk_head_dim,
        v: cfg.v_head_dim,
        hada_n: cfg.hada_n,
        rope_theta: cfg.rope_theta,
    };
    let mut blocks = Vec::with_capacity(man.enc_layers);
    for i in 0..man.enc_layers {
        let p = format!("encoder{i:02}.");
        let g = |n: &str| ld.f(&format!("{p}{n}"));
        let q = |n: &str| ld.q(&format!("{p}{n}"));
        blocks.push(EncBlock {
            pre_hada_norm_0: g("pre_hada_norm_0")?,
            attn_norm: g("ZCRMSNorm_0")?,
            post_attn_norm: g("post_attn_norm")?,
            conv_norm: g("conv_norm")?,
            conv_out_norm: g("conv_out_norm")?,
            pre_hada_norm: g("pre_hada_norm")?,
            q_proj: q("q_proj")?,
            k_proj: q("k_proj")?,
            v_proj: q("v_proj")?,
            gate_proj: q("gate_proj")?,
            out_proj: q("out_proj")?,
            q_norm: g("q_norm")?,
            k_norm: g("k_norm")?,
            attn_gate: ld.scalar(&format!("{p}attn_gate"))?,
            pw1: q("pw1")?,
            pw2: q("pw2")?,
            dw: g("dw")?,
            mlp_0: ld.mlp(&format!("{p}hadamard_mlp_0."))?,
            mlp: ld.mlp(&format!("{p}hadamard_mlp."))?,
        });
    }
    let mut dw = Vec::new();
    let mut pw = Vec::new();
    for i in 1..man.stem_stages {
        dw.push(ld.f(&format!("stem.dw_{i}"))?);
        pw.push(ld.q(&format!("stem.pw_{i}"))?);
    }
    let enc = Encoder {
        dims,
        stem: Stem { w: ld.f("stem.w")?, dw, pw, out: ld.q("stem.out")?, channels: man.stem_channels },
        blocks,
        mhc: ld.mhc("encoder.")?,
        final_norm: ld.f("encoder.final_norm")?,
        lanes: man.enc_lanes,
        kernel: man.conv_kernel,
    };
    let mel = ld.f("mel")?;

    let blob = cact.raw_tensor(canon.at("tokenizer")).map_err(|e| LoadError(format!("tokenizer: {e}")))?;
    let tok = SpTokenizer::from_blob(blob).map_err(|e| LoadError(format!("tokenizer: {e}")))?;
    if tok.vocab_size() != cfg.vocab_size || man.text_vocab >= cfg.vocab_size {
        return err(format!(
            "tokenizer has {} pieces, header vocab {}, manifest text vocab {}",
            tok.vocab_size(),
            cfg.vocab_size,
            man.text_vocab
        ));
    }
    let mut languages = Vec::new();
    for id in man.text_vocab..cfg.vocab_size {
        let piece = tok.piece(id as u32).unwrap_or_default();
        match piece.strip_prefix("<|").and_then(|p| p.strip_suffix("|>")) {
            Some(code) => languages.push(code.to_string()),
            None => return err(format!("piece {id} is {piece:?}, not a language tag")),
        }
    }

    Ok(Loaded { cfg, man, dec, enc, mel, perms, tok, languages, report })
}
