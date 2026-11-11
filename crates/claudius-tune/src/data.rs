//! The lessons as the trainer takes them: each rendered as Needle 3 is taught (`teach::Lesson::rendered`, whose prompt
//! is the app's), as the tokens after the tools, with the loss on the answer and the end.

use anyhow::{Result, ensure};
use claudius_intent::teach::{Lesson, TOOLS};
use needle_infer::prompt::build_prompt;
use needle_infer::sp_tokenizer::SpTokenizer;

use crate::train::Ex;

pub const BOS: u32 = 2;
pub const EOS: u32 = 1;
pub const IM_END: u32 = 5;

/// The prompt up to and including `</tools>\n`, which is the same for every lesson and is read once.
pub fn prefix_ids(tok: &SpTokenizer) -> Vec<u32> {
    let whole = build_prompt("", TOOLS, None);
    let head = whole.find("</tools>\n").map_or(whole.as_str(), |i| &whole[..i + "</tools>\n".len()]);
    let mut ids = vec![BOS];
    ids.extend(tok.encode(head));
    ids
}

/// `[BOS] + prompt + answer + [EOS]` after the tools, and its loss mask: 1 on every token of the answer and on EOS,
/// as upstream's `_encode`.
pub fn example(tok: &SpTokenizer, lesson: &Lesson, prefix: &[u32]) -> Result<Ex> {
    let (prompt, target) = lesson.rendered();
    let mut ids = vec![BOS];
    ids.extend(tok.encode(&prompt));
    ensure!(ids.starts_with(prefix), "'{}' does not start with the tools", lesson.query);
    let mut mask = vec![0.0; ids.len()];
    ids.extend(tok.encode(&target));
    ids.push(EOS);
    mask.resize(ids.len(), 1.0);
    Ok(Ex { suffix: ids[prefix.len()..].to_vec(), mask: mask[prefix.len()..].to_vec() })
}
