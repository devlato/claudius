//! Every `js` block in the book (docs/src) and the README runs, so the docs cannot drift from the language. The fence's
//! info string decides how: `js` must succeed without sample packs, `js packs` runs when they are installed, `js error`
//! must fail, `js no-run` is not run. Plain `js` blocks run with the packs switched off even where they are installed,
//! so that a block that needs them fails here as it would on CI. Blocks run against house seed 9 unless they start
//! their own song. Lines starting with `~` are hidden on the page (mdBook) but run here.

use std::path::{Path, PathBuf};

use claudius::compose::{self, Options};
use claudius::{genres, script};

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            markdown_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

struct Block {
    line: usize,
    flags: Vec<String>,
    code: String,
}

fn js_blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut current: Option<Block> = None;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```") {
            match current.take() {
                Some(b) => out.push(b),
                None => {
                    let mut words = info.split_whitespace();
                    if words.next() == Some("js") {
                        current =
                            Some(Block { line: i + 1, flags: words.map(String::from).collect(), code: String::new() });
                    } else if !info.trim().is_empty() || trimmed == "```" {
                        // a block in another language: skip to its end
                        current = Some(Block { line: 0, flags: vec!["skip".into()], code: String::new() });
                    }
                }
            }
            continue;
        }
        if let Some(b) = current.as_mut() {
            let code_line = line.strip_prefix('~').unwrap_or(line);
            b.code.push_str(code_line);
            b.code.push('\n');
        }
    }
    out.into_iter().filter(|b| !b.flags.iter().any(|f| f == "skip")).collect()
}

#[test]
fn every_example_in_the_docs_runs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/src");
    let mut files = Vec::new();
    markdown_files(&root, &mut files);
    files.sort();
    files.push(root.join("../../README.md"));
    let house = genres::get("house").unwrap();
    claudius_packs::set_disabled(true);
    let base = compose::generate(&house, 9, &Options::default(), None);
    claudius_packs::set_disabled(false);
    let packs = !claudius_packs::installed().is_empty();
    let base_with_packs = compose::generate(&house, 9, &Options::default(), None);
    let (mut ran, mut failures) = (0, Vec::new());
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let shown = file.strip_prefix(&root).unwrap_or(file).display().to_string();
        for b in js_blocks(&text) {
            let has = |f: &str| b.flags.iter().any(|x| x == f);
            if has("no-run") || (has("packs") && !packs) {
                continue;
            }
            ran += 1;
            claudius_packs::set_disabled(!has("packs"));
            let start = if has("packs") { &base_with_packs } else { &base };
            let result = script::run(&b.code, Some(start.clone()), None).and_then(|out| {
                if let Some(bp) = &out.bp {
                    let p = compose::arrange(bp);
                    if p.tracks.is_empty() {
                        return Err("the song has no tracks that play".into());
                    }
                }
                Ok(out)
            });
            match (result, has("error")) {
                (Ok(_), false) | (Err(_), true) => {}
                (Ok(_), true) => {
                    failures.push(format!("docs/src/{shown}:{}: expected an error, the example ran", b.line))
                }
                (Err(e), false) => failures.push(format!("docs/src/{shown}:{}: {e}\n{}", b.line, b.code.trim_end())),
            }
        }
    }
    claudius_packs::set_disabled(false);
    assert!(failures.is_empty(), "{} of {ran} examples failed:\n\n{}", failures.len(), failures.join("\n\n"));
}

#[test]
fn the_block_reader_handles_flags_and_hidden_lines() {
    let md = "text\n```js packs\n~blank(\"lofi\");\ntempo(80);\n```\n```sh\nls\n```\n```js no-run\nfoo\n```\n";
    let b = js_blocks(md);
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].flags, ["packs"]);
    assert_eq!(b[0].code, "blank(\"lofi\");\ntempo(80);\n");
    assert_eq!(b[1].flags, ["no-run"]);
}
