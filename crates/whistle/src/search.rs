//! Decoding ("Porting Whistle" §6): `<s>`, the language tag (argmax over the
//! tags unless forced), then text tokens (ids below the tags) until `</s>`, a
//! repetition loop, or the length cap; greedily or by beam search.

use crate::Whistle;
use crate::decoder::{Memory, State};
use crate::nn::{Numerics, argmax, log_softmax};

pub const BOS: u32 = 2;
pub const EOS: u32 = 1;
/// A transcript ends on reaching this many text tokens.
pub const MAX_TEXT: usize = 319;
/// Longest repeated unit the loop guard looks for, and the copies that make a loop.
pub const LOOP_UNIT: usize = 8;
pub const LOOP_COPIES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Eos,
    Loop,
    Length,
}

#[derive(Debug, Clone)]
pub struct Decoded {
    /// Index into the model's languages.
    pub language: usize,
    /// Softmax over the tags at step 0.
    pub language_prob: f32,
    /// Text tokens, without `</s>`.
    pub tokens: Vec<u32>,
    pub stop: Stop,
    /// Decoder steps run, over every hypothesis.
    pub steps: usize,
    /// Score of the chosen hypothesis (beam search) or its mean log-probability.
    pub score: f32,
}

/// If the transcript ends with a unit of up to [`LOOP_UNIT`] tokens repeated
/// [`LOOP_COPIES`] times, cut it back to one copy and say so.
pub fn loop_cut(tokens: &mut Vec<u32>) -> bool {
    for unit in 1..=LOOP_UNIT {
        let need = LOOP_COPIES * unit;
        if tokens.len() < need {
            break;
        }
        let tail = &tokens[tokens.len() - need..];
        let first = &tail[..unit];
        if tail.chunks_exact(unit).all(|c| c == first) {
            let keep = tokens.len() - (LOOP_COPIES - 1) * unit;
            tokens.truncate(keep);
            return true;
        }
    }
    false
}

/// Feed `<s>` and the tag; returns the state, the logits after the tag, the
/// language and its probability.
fn prompt(w: &Whistle, mem: &Memory, forced: Option<usize>, num: Numerics) -> (State, Vec<f32>, usize, f32) {
    let dec = &w.model.dec;
    let tv = w.model.man.text_vocab;
    let nl = w.model.languages.len();
    let mut st = dec.state();
    let logits = dec.step(&mut st, mem, BOS, &w.model.perms, num);
    let tags = &logits[tv..tv + nl];
    let lang = forced.unwrap_or_else(|| argmax(tags));
    let probs = log_softmax(tags);
    let logits = dec.step(&mut st, mem, (tv + lang) as u32, &w.model.perms, num);
    (st, logits, lang, probs[lang].exp())
}

pub fn greedy(w: &Whistle, mem: &Memory, forced: Option<usize>, num: Numerics) -> Decoded {
    let tv = w.model.man.text_vocab;
    let (mut st, mut logits, language, language_prob) = prompt(w, mem, forced, num);
    let mut steps = 2;
    let mut tokens = Vec::new();
    let mut logp = 0.0f64;
    let stop = loop {
        let text = &logits[..tv];
        let tok = argmax(text) as u32;
        logp += log_softmax(text)[tok as usize] as f64;
        if tok == EOS {
            break Stop::Eos;
        }
        tokens.push(tok);
        if loop_cut(&mut tokens) {
            break Stop::Loop;
        }
        if tokens.len() >= MAX_TEXT {
            break Stop::Length;
        }
        logits = w.model.dec.step(&mut st, mem, tok, &w.model.perms, num);
        steps += 1;
    };
    Decoded { language, language_prob, score: (logp / (tokens.len() + 1) as f64) as f32, tokens, stop, steps }
}

struct Hyp {
    state: State,
    logits: Vec<f32>,
    tokens: Vec<u32>,
    /// Per-token log-probabilities, normalised over the text ids.
    logps: Vec<f32>,
    sum: f64,
}

struct Finished {
    tokens: Vec<u32>,
    stop: Stop,
    score: f64,
}

/// The engine's beam search: every live beam proposes its `proposals` best
/// text tokens; all proposals are ranked together by running log-probability
/// and taken until `beams` are live, a proposal that ends the transcript
/// finishing instead with its log-probability over its token count; stop at
/// `beams` finished transcripts or no live beam.
pub fn beam(
    w: &Whistle,
    mem: &Memory,
    forced: Option<usize>,
    beams: usize,
    proposals: usize,
    num: Numerics,
) -> Decoded {
    let tv = w.model.man.text_vocab;
    let (state, logits, language, language_prob) = prompt(w, mem, forced, num);
    let mut steps = 2;
    let mut live = vec![Hyp { state, logits, tokens: Vec::new(), logps: Vec::new(), sum: 0.0 }];
    let mut finished: Vec<Finished> = Vec::new();

    while !live.is_empty() && finished.len() < beams {
        // (score, beam, token, token logp)
        let mut props: Vec<(f64, usize, u32, f32)> = Vec::new();
        for (bi, h) in live.iter().enumerate() {
            let lp = log_softmax(&h.logits[..tv]);
            let mut ids: Vec<usize> = (0..tv).collect();
            let k = proposals.min(tv);
            ids.select_nth_unstable_by(k - 1, |&a, &b| lp[b].total_cmp(&lp[a]));
            for &id in &ids[..k] {
                props.push((h.sum + lp[id] as f64, bi, id as u32, lp[id]));
            }
        }
        props.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

        let mut chosen: Vec<(usize, u32, f32)> = Vec::new();
        for &(_, bi, tok, lp) in &props {
            if chosen.len() == beams {
                break;
            }
            let h = &live[bi];
            if tok == EOS {
                let n = h.tokens.len() + 1;
                finished.push(Finished {
                    tokens: h.tokens.clone(),
                    stop: Stop::Eos,
                    score: (h.sum + lp as f64) / n as f64,
                });
                continue;
            }
            let mut toks = h.tokens.clone();
            toks.push(tok);
            if loop_cut(&mut toks) {
                let mut logps = h.logps.clone();
                logps.push(lp);
                logps.truncate(toks.len());
                let s: f64 = logps.iter().map(|&v| v as f64).sum();
                finished.push(Finished { score: s / toks.len() as f64, tokens: toks, stop: Stop::Loop });
                continue;
            }
            if toks.len() >= MAX_TEXT {
                finished.push(Finished {
                    tokens: toks,
                    stop: Stop::Length,
                    score: (h.sum + lp as f64) / MAX_TEXT as f64,
                });
                continue;
            }
            chosen.push((bi, tok, lp));
        }

        let mut next = Vec::with_capacity(chosen.len());
        for (bi, tok, lp) in chosen {
            let h = &live[bi];
            let mut state = h.state.clone();
            let logits = w.model.dec.step(&mut state, mem, tok, &w.model.perms, num);
            steps += 1;
            let mut tokens = h.tokens.clone();
            tokens.push(tok);
            let mut logps = h.logps.clone();
            logps.push(lp);
            next.push(Hyp { state, logits, tokens, logps, sum: h.sum + lp as f64 });
        }
        live = next;
    }

    let best = finished.into_iter().max_by(|a, b| a.score.total_cmp(&b.score)).unwrap_or(Finished {
        tokens: Vec::new(),
        stop: Stop::Length,
        score: f64::NEG_INFINITY,
    });
    Decoded { language, language_prob, tokens: best.tokens, stop: best.stop, steps, score: best.score as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_four_times_repeated_unit_is_cut_to_one_copy() {
        let mut t = vec![9, 1, 2, 1, 2, 1, 2, 1, 2];
        assert!(loop_cut(&mut t));
        assert_eq!(t, vec![9, 1, 2]);
    }

    #[test]
    fn three_copies_are_not_a_loop() {
        let mut t = vec![5, 5, 5];
        assert!(!loop_cut(&mut t));
        let mut t = vec![5, 5, 5, 5];
        assert!(loop_cut(&mut t));
        assert_eq!(t, vec![5]);
    }
}
