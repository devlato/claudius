//! Listening vocabulary and score calibration (earshot/vocab.py): raw CLAP cosines -> numbers that mean the same
//! for every sound and query.
//!
//! ```text
//! u = cos(a, q) - m.q               term bias removed (m = mean reference embedding, data/vocab_stats.json)
//! z = (u - mean_t u_t) / std_t u_t  per-sound z over the 322 vocabulary terms t
//! ```
//!
//! percentile = share of vocabulary terms the query beats for this sound, rank_in_vocab = 1 + terms above it.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::dsp::round_to;

pub const VOCAB_JSON: &str = include_str!("../data/vocab.json");
pub const STATS_JSON: &str = include_str!("../data/vocab_stats.json");
pub const MAX_WIN: usize = 6;
pub const WIN: f64 = 10.0;
pub const HOP: f64 = 5.0;
pub const SPACED_S: f64 = 1.0;
/// categories whose tags were never validated (reported as hints, never with a verdict)
pub const UNVALIDATED: [&str; 4] = ["music_genre", "music_mood", "instruments", "voice"];
pub const CLAP_MODEL_ID: &str = "laion/larger_clap_general";

#[derive(Deserialize)]
struct VocabFile {
    templates: Vec<String>,
    categories: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct StatsFile {
    model: String,
    logit_scale: f64,
    mean: Vec<f32>,
}

/// the vocabulary file: prompt templates and terms per category (file order)
#[derive(Clone, Debug)]
pub struct VocabData {
    pub templates: Vec<String>,
    pub cats: Vec<(String, Vec<String>)>,
    pub names: Vec<String>,
    pub cat_idx: Vec<usize>,
}

impl VocabData {
    pub fn embedded() -> VocabData {
        let f: VocabFile = serde_json::from_str(VOCAB_JSON).expect("data/vocab.json");
        let cats: Vec<(String, Vec<String>)> = f
            .categories
            .into_iter()
            .map(|(c, v)| {
                let ts = v.as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect());
                (c, ts.unwrap_or_default())
            })
            .collect();
        let mut names = Vec::new();
        let mut cat_idx = Vec::new();
        for (i, (_, ts)) in cats.iter().enumerate() {
            for t in ts {
                names.push(t.clone());
                cat_idx.push(i);
            }
        }
        VocabData { templates: f.templates, cats, names, cat_idx }
    }

    pub fn category_of(&self, term_index: usize) -> &str {
        &self.cats[self.cat_idx[term_index]].0
    }
}

/// reference statistics: the mean reference embedding and CLAP's softmax temperature
#[derive(Clone, Debug)]
pub struct Stats {
    pub mean: Vec<f32>,
    pub logit_scale: f64,
}

impl Stats {
    pub fn embedded() -> Option<Stats> {
        let s: StatsFile = serde_json::from_str(STATS_JSON).ok()?;
        (s.model == CLAP_MODEL_ID).then_some(Stats { mean: s.mean, logit_scale: s.logit_scale })
    }
}

/// calibrated score of one query for one sound (vocab.scores)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Score {
    pub query: String,
    pub cosine: f64,
    pub z: f64,
    pub percentile: f64,
    pub rank_in_vocab: usize,
    pub vocab_size: usize,
}

/// a vocabulary term scored for one sound (vocab.top_terms)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Term {
    pub term: String,
    pub category: String,
    pub z: f64,
    pub cosine: f64,
}

/// CLAP windows [(s, e)] covering [start, end): one span if <= 10 s, else 10 s windows hop 5 s (last one flush with
/// the end), at most max_win of them evenly spaced (vocab.spans_for)
pub fn spans_for(start: f64, end: f64, max_win: usize) -> Vec<(f64, f64)> {
    let sr = crate::SR as f64;
    let n = ((end - start) * sr).round_ties_even() as i64;
    let (w, h) = ((WIN * sr) as i64, (HOP * sr) as i64);
    if n <= w {
        return vec![(start, end)];
    }
    let mut st: Vec<i64> = (0..).map(|k| k * h).take_while(|s| *s <= n - w).collect();
    if (n - w) % h != 0 {
        st.push(n - w);
    }
    if max_win > 0 && st.len() > max_win {
        let l = crate::librosa::linspace(0.0, (st.len() - 1) as f64, max_win);
        st = l.iter().map(|i| st[i.round_ties_even() as usize]).collect();
    }
    st.iter().map(|&s| (start + s as f64 / sr, start + (s + w) as f64 / sr)).collect()
}

fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum()
}

/// per-sound calibration over the vocabulary terms
pub struct Calib {
    pub cos_t: Vec<f64>,
    pub u_t: Vec<f64>,
    pub z_t: Vec<f64>,
    pub mu: f64,
    pub sd: f64,
}

/// one query's calibrated values for one sound
pub struct QueryCalib {
    pub cos: f64,
    pub u: f64,
    pub z: f64,
    pub pct: f64,
    pub beats: usize,
}

/// the vocabulary embedded with CLAP: term matrix E (322 x 512), reference mean m and term biases
pub struct Vocab {
    pub data: VocabData,
    pub e: Vec<Vec<f32>>,
    pub m: Option<Vec<f32>>,
    pub mu_e: Vec<f64>,
    pub logit_scale: f64,
}

impl Vocab {
    pub fn new(data: VocabData, e: Vec<Vec<f32>>, stats: Option<Stats>) -> Vocab {
        let m = stats.as_ref().map(|s| s.mean.clone());
        let mu_e = e.iter().map(|t| m.as_ref().map_or(0.0, |m| dot(t, m))).collect();
        let logit_scale = stats.map_or(100.0, |s| s.logit_scale);
        Vocab { data, e, m, mu_e, logit_scale }
    }

    /// term bias = mean cosine of the description over the reference recordings (= q . m)
    pub fn bias(&self, q: &[f32]) -> f64 {
        self.m.as_ref().map_or(0.0, |m| dot(q, m))
    }

    pub fn calibrate(&self, a: &[f32]) -> Calib {
        let cos_t: Vec<f64> = self.e.iter().map(|t| dot(a, t)).collect();
        let u_t: Vec<f64> = cos_t.iter().zip(&self.mu_e).map(|(c, b)| c - b).collect();
        let mu = u_t.iter().sum::<f64>() / u_t.len() as f64;
        let sd = (u_t.iter().map(|u| (u - mu) * (u - mu)).sum::<f64>() / u_t.len() as f64).sqrt() + 1e-9;
        let z_t = u_t.iter().map(|u| (u - mu) / sd).collect();
        Calib { cos_t, u_t, z_t, mu, sd }
    }

    pub fn query(&self, c: &Calib, a: &[f32], q: &[f32]) -> QueryCalib {
        let cos = dot(a, q);
        let u = cos - self.bias(q);
        let z = (u - c.mu) / c.sd;
        let below = c.u_t.iter().filter(|&&t| t < u).count();
        let beats = c.u_t.iter().filter(|&&t| t > u).count();
        QueryCalib { cos, u, z, pct: below as f64 / c.u_t.len() as f64, beats }
    }

    /// cosine, z, percentile among vocab terms and rank_in_vocab of each query embedding for one sound
    pub fn scores(&self, a: &[f32], queries: &[(&str, &[f32])]) -> Vec<Score> {
        let c = self.calibrate(a);
        queries
            .iter()
            .map(|(name, q)| {
                let r = self.query(&c, a, q);
                Score {
                    query: name.to_string(),
                    cosine: round_to(r.cos, 3),
                    z: round_to(r.z, 2),
                    percentile: round_to(100.0 * r.pct, 1),
                    rank_in_vocab: r.beats + 1,
                    vocab_size: self.data.names.len(),
                }
            })
            .collect()
    }

    /// best calibrated vocabulary terms for one sound; cats = only these categories, exclude = not these
    pub fn top_terms(&self, a: &[f32], k: usize, cats: Option<&[&str]>, exclude: &[&str]) -> Vec<Term> {
        let c = self.calibrate(a);
        let order = crate::dsp::stats::argsort_desc(&c.z_t);
        order
            .into_iter()
            .filter(|&i| {
                let cat = self.data.category_of(i);
                cats.is_none_or(|cs| cs.contains(&cat)) && !exclude.contains(&cat)
            })
            .take(k)
            .map(|i| Term {
                term: self.data.names[i].clone(),
                category: self.data.category_of(i).to_string(),
                z: round_to(c.z_t[i], 2),
                cosine: round_to(c.cos_t[i], 3),
            })
            .collect()
    }

    /// category of the vocabulary term whose text is closest to a query embedding
    pub fn nearest_category(&self, q: &[f32]) -> String {
        let s: Vec<f64> = self.e.iter().map(|t| dot(q, t)).collect();
        self.data.category_of(crate::dsp::stats::argmax(&s)).to_string()
    }
}

// ------------------------------------------------------------------------------------------------ text cache
const MAGIC: &[u8; 8] = b"ESTXT01\n";

/// raw CLAP text embeddings cached on disk (one file per model revision), merged on save
pub struct TextCache {
    path: Option<PathBuf>,
    map: Mutex<HashMap<String, Vec<f32>>>,
    dirty: Mutex<bool>,
}

fn read_cache(path: &Path) -> HashMap<String, Vec<f32>> {
    let mut out = HashMap::new();
    let Ok(mut f) = std::fs::File::open(path) else { return out };
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() || buf.len() < 12 || &buf[..8] != MAGIC {
        return out;
    }
    let rd = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
    let count = rd(&buf, 8);
    let mut p = 12;
    for _ in 0..count {
        if p + 8 > buf.len() {
            break;
        }
        let tl = rd(&buf, p);
        let dim = rd(&buf, p + 4);
        p += 8;
        if p + tl + 4 * dim > buf.len() {
            break;
        }
        let text = String::from_utf8_lossy(&buf[p..p + tl]).into_owned();
        p += tl;
        let v: Vec<f32> =
            buf[p..p + 4 * dim].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        p += 4 * dim;
        out.insert(text, v);
    }
    out
}

impl TextCache {
    pub fn open(path: Option<PathBuf>) -> TextCache {
        let map = path.as_deref().map(read_cache).unwrap_or_default();
        TextCache { path, map: Mutex::new(map), dirty: Mutex::new(false) }
    }

    pub fn get(&self, text: &str) -> Option<Vec<f32>> {
        self.map.lock().ok()?.get(text).cloned()
    }

    pub fn insert(&self, text: String, v: Vec<f32>) {
        if let Ok(mut m) = self.map.lock() {
            m.insert(text, v);
            *self.dirty.lock().unwrap() = true;
        }
    }

    /// merge with what other processes saved meanwhile, then replace atomically; never fails the caller
    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let mut dirty = self.dirty.lock().unwrap();
        if !*dirty {
            return;
        }
        let mut merged = read_cache(path);
        {
            let m = self.map.lock().unwrap();
            for (k, v) in m.iter() {
                merged.insert(k.clone(), v.clone());
            }
        }
        let mut keys: Vec<&String> = merged.keys().collect();
        keys.sort();
        let mut out = Vec::with_capacity(16 + merged.len() * (40 + 2048));
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(keys.len() as u32).to_le_bytes());
        for k in keys {
            let v = &merged[k];
            out.extend_from_slice(&(k.len() as u32).to_le_bytes());
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(k.as_bytes());
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        let ok = std::fs::File::create(&tmp).and_then(|mut f| f.write_all(&out).and_then(|_| f.sync_all())).is_ok()
            && std::fs::rename(&tmp, path).is_ok();
        if !ok {
            let _ = std::fs::remove_file(&tmp);
        }
        *self.map.lock().unwrap() = merged;
        *dirty = false;
    }
}
