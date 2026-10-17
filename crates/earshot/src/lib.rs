//! earshot for Rust: "ears" for code that cannot listen. A port of the Python library earshot
//! (<https://github.com/Leonxlnx/earshot>, MIT) with no Python at runtime: CLAP and AudioSet models run as fp32 ONNX
//! exports on ONNX Runtime, the DSP (mel front ends, loudness, onsets, beat grid ...) is reimplemented.
//!
//! ```no_run
//! let ears = earshot::Ears::load(earshot::EarsOptions::default())?;
//! let x = earshot::audio::load_mono(std::path::Path::new("kick.wav"))?;
//! let emb = ears.embed(&x)?;
//! let m = ears.match_query(&emb, "a punchy kick drum", &["a snare drum hit"])?;
//! println!("{} z {} {}", m.query, m.z, m.verdict);
//! # anyhow::Ok(())
//! ```

#![forbid(unsafe_code)]

pub mod ast;
pub mod audio;
pub mod clap;
pub mod compare;
pub mod describe;
pub mod dsp;
pub mod features;
pub mod grid;
pub mod librosa;
pub mod matching;
pub mod models;
pub mod realness;
pub mod search;
pub mod vocab;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub use crate::ast::Reduce;
pub use crate::clap::Pad;
pub use crate::compare::{Feats, Typicality};
pub use crate::features::Measure;
pub use crate::grid::Grid;
pub use crate::matching::{Alt, Match};
pub use crate::realness::Realness;
pub use crate::search::{FindOptions, FindResult, Trim};
pub use crate::vocab::{Score, Term};

/// earshot's working sample rate
pub const SR: u32 = 48000;

/// how to load the models
#[derive(Clone, Debug)]
pub struct EarsOptions {
    /// model + text-embedding cache; None = `$EARSHOT_HOME`, else `~/.cache/earshot-rs`
    pub cache_dir: Option<PathBuf>,
    /// ONNX Runtime intra-op threads per model; 0 = `$EARSHOT_THREADS`, else min(4, cores / 2)
    pub threads: usize,
    /// also load the AudioSet tagger (347 MB; needed for audioset() and describe())
    pub load_ast: bool,
    /// never download (models must already be cached); `EARSHOT_OFFLINE=1` forces it too
    pub offline: bool,
}

impl Default for EarsOptions {
    fn default() -> Self {
        EarsOptions { cache_dir: None, threads: 0, load_ast: true, offline: false }
    }
}

/// the loaded ears: CLAP (audio + text), the optional AudioSet tagger, the calibrated vocabulary and the realness
/// model. Send + Sync: the ONNX sessions sit behind mutexes, so several threads can share one `Ears` (calls into
/// the same model are serialised; each call uses `threads` cores).
pub struct Ears {
    pub clap: clap::Clap,
    pub ast: Option<ast::Ast>,
    pub vocab: vocab::Vocab,
    pub realness_model: realness::Model,
    text_cache: vocab::TextCache,
    prompts: Vec<Vec<f32>>,
    anchors: (Vec<f32>, Vec<f32>),
    pub cache_dir: PathBuf,
    pub threads: usize,
}

fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum()
}

fn mean_vec(vs: &[Vec<f32>]) -> Vec<f32> {
    let mut m = vec![0.0f64; vs[0].len()];
    for v in vs {
        for (a, b) in m.iter_mut().zip(v) {
            *a += *b as f64;
        }
    }
    m.iter().map(|v| (v / vs.len() as f64) as f32).collect()
}

impl Ears {
    /// load (downloading on first use unless offline) the models and embed the vocabulary (cached on disk)
    pub fn load(opts: EarsOptions) -> Result<Ears> {
        let cache = opts.cache_dir.clone().unwrap_or_else(models::default_cache_dir);
        let threads = if opts.threads == 0 { models::default_threads() } else { opts.threads };
        let offline = opts.offline || models::env_offline();
        use models::*;
        let audio_p = fetch(&cache, CLAP_REPO, CLAP_REV, CLAP_AUDIO, offline)?;
        let text_p = fetch(&cache, CLAP_REPO, CLAP_REV, CLAP_TEXT, offline)?;
        let tok_p = fetch(&cache, CLAP_REPO, CLAP_REV, CLAP_TOKENIZER, offline)?;
        let ast_p = if opts.load_ast {
            Some((
                fetch(&cache, AST_REPO, AST_REV, AST_MODEL, offline)?,
                fetch(&cache, AST_REPO, AST_REV, AST_CONFIG, offline)?,
            ))
        } else {
            None
        };
        // the audio sessions load in parallel; the text encoder waits for the first text-cache miss
        let (a, s) = std::thread::scope(|sc| {
            let a = sc.spawn(|| session(&audio_p, threads));
            let s = ast_p.as_ref().map(|(p, _)| sc.spawn(|| session(p, threads)));
            (a.join(), s.map(|h| h.join()))
        });
        let a = a.map_err(|_| anyhow::anyhow!("CLAP audio loader panicked"))??;
        let clap = clap::Clap::new(a, clap::TextModel { path: text_p, threads }, &tok_p)?;
        let ast = match (s, &ast_p) {
            (Some(s), Some((_, cfg))) => {
                Some(ast::Ast::new(s.map_err(|_| anyhow::anyhow!("AST loader panicked"))??, cfg)?)
            }
            _ => None,
        };
        let text_cache = vocab::TextCache::open(Some(cache.join("text").join(format!("clap-{}.bin", &CLAP_REV[..12]))));
        let data = vocab::VocabData::embedded();
        let realness_model = realness::Model::embedded();
        let mut ears = Ears {
            clap,
            ast,
            vocab: vocab::Vocab::new(data.clone(), vec![], None),
            realness_model,
            text_cache,
            prompts: vec![],
            anchors: (vec![], vec![]),
            cache_dir: cache,
            threads,
        };
        let names: Vec<&str> = data.names.iter().map(|s| s.as_str()).collect();
        let e = ears.embed_text_with(&names, &data.templates)?;
        ears.vocab = vocab::Vocab::new(data, e, vocab::Stats::embedded());
        let pt = ears.realness_model.prompt_texts();
        ears.prompts = ears.embed_text_raw(&pt.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
        let ev = ears.embed_text(&search::EVENT_ANCHORS)?;
        let tx = ears.embed_text(&search::TEXTURE_ANCHORS)?;
        ears.anchors = (mean_vec(&ev), mean_vec(&tx));
        ears.text_cache.save();
        Ok(ears)
    }

    // ------------------------------------------------------------------------------------------- embeddings
    /// raw CLAP text embeddings (unit vectors, no templates), cached on disk
    pub fn embed_text_raw(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut out: Vec<Option<Vec<f32>>> = texts.iter().map(|t| self.text_cache.get(t)).collect();
        let mut todo: Vec<&str> = texts.iter().zip(&out).filter(|(_, o)| o.is_none()).map(|(t, _)| *t).collect();
        todo.sort();
        todo.dedup();
        if !todo.is_empty() {
            let e = self.clap.embed_text_raw(&todo)?;
            for (t, v) in todo.iter().zip(e) {
                self.text_cache.insert(t.to_string(), v);
            }
            self.text_cache.save();
            for (o, t) in out.iter_mut().zip(texts) {
                if o.is_none() {
                    *o = self.text_cache.get(t);
                }
            }
        }
        out.into_iter().map(|o| o.context("text embedding missing")).collect()
    }

    fn embed_text_with(&self, texts: &[&str], templates: &[String]) -> Result<Vec<Vec<f32>>> {
        let all: Vec<String> =
            texts.iter().flat_map(|t| templates.iter().map(move |tp| tp.replacen("{}", t, 1))).collect();
        let e = self.embed_text_raw(&all.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
        Ok(e.chunks(templates.len()).map(clap::mean_norm).collect())
    }

    /// text embeddings as vocab.embed_text: each phrase through the 5 templates of data/vocab.json, averaged and
    /// renormalised (512-d unit vectors)
    pub fn embed_text(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.embed_text_with(texts, &self.vocab.data.templates)
    }

    /// vocab.embed_spans on an in-memory 48 kHz mono array: every span (seconds) is one CLAP pass (<= 10 s),
    /// padded to 10 s with `pad`
    pub fn embed_spans(&self, x: &[f32], spans: &[(f64, f64)], pad: Pad) -> Result<Vec<Vec<f32>>> {
        let sr = SR as f64;
        let mut owned: Vec<Vec<f32>> = Vec::new();
        let mut idx: Vec<(usize, usize)> = Vec::new();
        for &(s, e) in spans {
            let a = ((s * sr).round_ties_even().max(0.0) as usize).min(x.len());
            let b = ((e * sr).round_ties_even().max(0.0) as usize).min(x.len()).max(a);
            idx.push((a, b));
        }
        if pad == Pad::Spaced {
            for &(a, b) in &idx {
                let mut seg = x[a..b].to_vec();
                if (seg.len() as f64) < vocab::SPACED_S * sr {
                    seg.resize((vocab::SPACED_S * sr) as usize, 0.0);
                }
                owned.push(seg);
            }
            let refs: Vec<&[f32]> = owned.iter().map(|v| v.as_slice()).collect();
            return self.clap.embed_audio(&refs, Pad::RepeatPad);
        }
        let refs: Vec<&[f32]> = idx.iter().map(|&(a, b)| &x[a..b]).collect();
        self.clap.embed_audio(&refs, pad)
    }

    /// vocab.embed_files semantics with a padding mode and window cap (MAX_WIN = 6 by default)
    pub fn embed_with(&self, mono: &[f32], pad: Pad, max_win: usize) -> Result<Vec<f32>> {
        Ok(self.embed_many_with(&[mono], pad, max_win)?.remove(0))
    }

    /// 512-d unit embedding of a 48 kHz mono array with vocab.embed_files semantics: <= 10 s = one repeat-padded
    /// CLAP window, longer = mean of 10 s windows (hop 5 s, last flush with the end, at most 6 evenly spaced)
    pub fn embed(&self, mono: &[f32]) -> Result<Vec<f32>> {
        self.embed_with(mono, Pad::RepeatPad, vocab::MAX_WIN)
    }

    /// embed() for many arrays, batched across them
    pub fn embed_many(&self, monos: &[&[f32]]) -> Result<Vec<Vec<f32>>> {
        self.embed_many_with(monos, Pad::RepeatPad, vocab::MAX_WIN)
    }

    pub fn embed_many_with(&self, monos: &[&[f32]], pad: Pad, max_win: usize) -> Result<Vec<Vec<f32>>> {
        let sr = SR as f64;
        let mut segs: Vec<Vec<f32>> = Vec::new();
        let mut owner = Vec::new();
        for (j, x) in monos.iter().enumerate() {
            let dur = x.len() as f64 / sr;
            for (s, e) in vocab::spans_for(0.0, dur.max(0.01), max_win) {
                let a = ((s * sr).round_ties_even() as usize).min(x.len());
                let b = ((e * sr).round_ties_even() as usize).min(x.len()).max(a);
                let mut seg = x[a..b].to_vec();
                if pad == Pad::Spaced && (seg.len() as f64) < vocab::SPACED_S * sr {
                    seg.resize((vocab::SPACED_S * sr) as usize, 0.0);
                }
                segs.push(seg);
                owner.push(j);
            }
        }
        let refs: Vec<&[f32]> = segs.iter().map(|v| v.as_slice()).collect();
        let e = self.clap.embed_audio(&refs, if pad == Pad::Spaced { Pad::RepeatPad } else { pad })?;
        let mut groups: Vec<Vec<Vec<f32>>> = vec![Vec::new(); monos.len()];
        for (v, j) in e.into_iter().zip(owner) {
            groups[j].push(v);
        }
        Ok(groups.iter().map(|g| clap::mean_norm(g)).collect())
    }

    // ------------------------------------------------------------------------------------------- calibrated text
    /// cosine, z, percentile, rank_in_vocab and vocab_size of each query for one audio embedding
    pub fn scores(&self, emb: &[f32], queries: &[&str]) -> Result<Vec<Score>> {
        let q = self.embed_text(queries)?;
        let qs: Vec<(&str, &[f32])> = queries.iter().zip(&q).map(|(t, v)| (*t, v.as_slice())).collect();
        Ok(self.vocab.scores(emb, &qs))
    }

    /// calibrated match of one description: z, verdict, forced choice vs `alts`, closest vocabulary terms
    pub fn match_query(&self, emb: &[f32], query: &str, alts: &[&str]) -> Result<Match> {
        Ok(self.match_queries(emb, &[query], alts)?.remove(0))
    }

    /// match.score_queries for several queries (sorted by z, best first), each with the closest vocabulary terms
    pub fn match_queries(&self, emb: &[f32], queries: &[&str], alts: &[&str]) -> Result<Vec<Match>> {
        if queries.is_empty() || queries.iter().any(|q| q.trim().is_empty()) {
            bail!("no query given: pass one or more non-empty descriptions");
        }
        let q = self.embed_text(queries)?;
        let a = self.embed_text(alts)?;
        let qs: Vec<(&str, &[f32])> = queries.iter().zip(&q).map(|(t, v)| (*t, v.as_slice())).collect();
        let al: Vec<(&str, &[f32])> = alts.iter().zip(&a).map(|(t, v)| (*t, v.as_slice())).collect();
        let mut rows = matching::score_queries(&self.vocab, emb, &qs, &al);
        rows.sort_by(|x, y| y.z.partial_cmp(&x.z).unwrap_or(std::cmp::Ordering::Equal));
        let near = self.top_terms(emb, 4, &matching::SKIP_CATS);
        for r in rows.iter_mut() {
            r.closest_vocab = near.clone();
        }
        Ok(rows)
    }

    /// best calibrated vocabulary terms for one sound, leaving out the given categories
    pub fn top_terms(&self, emb: &[f32], k: usize, exclude_categories: &[&str]) -> Vec<Term> {
        self.vocab.top_terms(emb, k, None, exclude_categories)
    }

    /// files / sounds sorted by the calibrated z of `query` (stable on ties), with their scores
    pub fn rank(&self, embs: &[Vec<f32>], query: &str) -> Result<Vec<(usize, Score)>> {
        let q = self.embed_text(&[query])?.remove(0);
        let mut r: Vec<(usize, Score)> = embs
            .iter()
            .enumerate()
            .map(|(i, e)| (i, self.vocab.scores(e, &[(query, q.as_slice())]).remove(0)))
            .collect();
        r.sort_by(|a, b| b.1.z.partial_cmp(&a.1.z).unwrap_or(std::cmp::Ordering::Equal));
        Ok(r)
    }

    /// ('event' | 'texture' | 'either', score): CLAP text similarity to one-shot vs continuous anchor phrases
    pub fn query_kind(&self, query: &str) -> Result<(String, f64)> {
        let q = self.embed_text(&[query])?.remove(0);
        let d = dot(&q, &self.anchors.0) - dot(&q, &self.anchors.1);
        let k = if d >= search::KIND_EVENT {
            "event"
        } else if d <= search::KIND_TEXTURE {
            "texture"
        } else {
            "either"
        };
        Ok((k.into(), dsp::round_to(d, 3)))
    }

    /// (z, cosine) of every candidate span of `x` for a query embedding (search._score)
    pub fn score_spans(&self, x: &[f32], cands: &[search::Cand], qe: &[f32]) -> Result<(Vec<f64>, Vec<f64>)> {
        if cands.is_empty() {
            return Ok((vec![], vec![]));
        }
        let spans: Vec<(f64, f64)> = cands.iter().map(|c| (c.0, c.1)).collect();
        let e = self.embed_spans(x, &spans, Pad::RepeatPad)?;
        let mut z = Vec::with_capacity(e.len());
        let mut c = Vec::with_capacity(e.len());
        for a in &e {
            let cal = self.vocab.calibrate(a);
            let r = self.vocab.query(&cal, a, qe);
            z.push(r.z);
            c.push(r.cos);
        }
        Ok((z, c))
    }

    /// best non-overlapping trims of a 48 kHz mono recording for `query` (search.find, kind 'auto'); peaks are read
    /// from the mono array (find_file reads them from the stereo decode, like earshot)
    pub fn find(&self, mono: &[f32], query: &str, top: usize) -> Result<Vec<Trim>> {
        Ok(search::find_with(self, mono, None, query, top, &FindOptions::default(), "sound")?.segments)
    }

    /// the full search.find result for a file (mono decode for the models, stereo decode for the peaks)
    pub fn find_file(&self, path: &Path, query: &str, top: usize, opt: &FindOptions) -> Result<FindResult> {
        let mono = audio::load_mono(path)?;
        let (l, r) = audio::load_stereo(path)?;
        let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        search::find_with(self, &mono, Some((&l, &r)), query, top, opt, &name)
    }

    // ------------------------------------------------------------------------------------------- realness / AST
    /// real recording / pro-produced vs code-synthesized (realness.realness on a 48 kHz mono array). `emb` is
    /// reused only where it equals what realness computes: a sound of 6 s or less that needs no level lift
    pub fn realness(&self, mono: &[f32], emb: Option<&[f32]>) -> Result<Realness> {
        self.realness_named(mono, emb, "sound")
    }

    pub fn realness_named(&self, mono: &[f32], emb: Option<&[f32]>, name: &str) -> Result<Realness> {
        let m = &self.realness_model;
        let (x, gain, rdb, pdb) = realness::lift_quiet(mono);
        let dur = x.len() as f64 / SR as f64;
        let spans = realness::clips(x.len());
        let mut feats: Vec<Option<realness::SignalFeatures>> = Vec::with_capacity(spans.len());
        for &(_, _, a, b) in &spans {
            let c = &x[a..b];
            feats.push(if realness::audible(c) { realness::signal_features(c) } else { None });
        }
        let live: Vec<usize> = (0..spans.len()).filter(|&i| feats[i].is_some()).collect();
        let embs: Vec<Vec<f32>> = match emb {
            Some(e) if spans.len() == 1 && gain == 0.0 && live.len() == 1 => vec![e.to_vec()],
            _ => {
                let clips: Vec<&[f32]> = live.iter().map(|&i| &x[spans[i].2..spans[i].3]).collect();
                if clips.is_empty() { vec![] } else { self.clap.embed_audio(&clips, Pad::RepeatPad)? }
            }
        };
        let mut rows: Vec<Option<realness::ClipScore>> = vec![None; spans.len()];
        for (j, &i) in live.iter().enumerate() {
            let f = feats[i].take().unwrap();
            rows[i] = Some(realness::score_clip(m, f, &embs[j], &self.prompts));
        }
        Ok(realness::assemble(m, name, dur, 0.0, &spans, &rows, gain, rdb, pdb))
    }

    /// 527 AudioSet probabilities of a 48 kHz mono array (10.24 s windows, max over windows)
    pub fn audioset(&self, mono: &[f32]) -> Result<Vec<f32>> {
        self.audioset_reduce(mono, Reduce::Max)
    }

    pub fn audioset_reduce(&self, mono: &[f32], reduce: Reduce) -> Result<Vec<f32>> {
        let a = self.ast.as_ref().context("the AudioSet tagger is not loaded (EarsOptions::load_ast = false)")?;
        Ok(a.probs(&[mono], reduce)?.remove(0))
    }

    /// AudioSet class names in output order (empty when the tagger is not loaded)
    pub fn audioset_labels(&self) -> &[String] {
        self.ast.as_ref().map_or(&[], |a| &a.labels)
    }

    // ------------------------------------------------------------------------------------------- files
    /// describe.describe for a file: calibrated tags, AudioSet labels and measurements
    pub fn describe(&self, path: &Path) -> Result<describe::Description> {
        describe::describe(self, path, 8)
    }

    /// match.match for a file (several queries allowed: the best is first)
    pub fn match_file(&self, path: &Path, queries: &[&str], alts: &[&str]) -> Result<describe::FileMatch> {
        describe::match_file(self, path, queries, alts)
    }

    /// realness.realness for a file
    pub fn realness_file(&self, path: &Path) -> Result<Realness> {
        let x = audio::load_mono(path)?;
        let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.realness_named(&x, None, &name)
    }

    /// embedding of a file (mono decode, embed() semantics)
    pub fn embed_file(&self, path: &Path) -> Result<Vec<f32>> {
        self.embed(&audio::load_mono(path)?)
    }
}

/// how close `emb` is to its k nearest references compared with how close each reference is to its own k nearest
/// (compare._typicality): mean_cos_knn, z (>= -1 fits the set, < -2 outlier), percentile, verdict words
pub fn typicality(emb: &[f32], refs: &[Vec<f32>], k: usize) -> Typicality {
    let (sx, z, pct) = compare::typicality_raw(emb, refs, k);
    Typicality {
        mean_cos_knn: dsp::round_to(sx, 3),
        z: dsp::round_to(z, 2),
        percentile: dsp::round_to(pct, 1),
        k,
        verdict: compare::fit_words(z).into(),
    }
}

/// stereo measurements of a file (features.measure on the stereo decode)
pub fn measure_file(path: &Path, music: bool, light: bool) -> Result<Measure> {
    let (l, r) = audio::load_stereo(path)?;
    Ok(features::measure(&l, &r, music, light))
}
