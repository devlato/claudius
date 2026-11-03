//! What keeps the engine running in a browser (a build without the `native` feature, for wasm32-unknown-unknown): std's
//! clock and temporary folder exist there only to panic, at run time, so the engine reads them through `clock` and
//! `config::temp_dir`, which know.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn the_engine_reads_no_clock_or_temporary_folder_a_browser_lacks() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for c in ["claudius", "claudius-packs"] {
        sources(&crates.join(c).join("src"), &mut files);
    }
    // the clocks themselves, and the pack tools' own programs, which no browser runs
    let allowed = |p: &Path| p.ends_with("src/clock.rs") || p.components().any(|c| c.as_os_str() == "bin");
    let mut found = Vec::new();
    for f in files.iter().filter(|f| !allowed(f)) {
        let text = std::fs::read_to_string(f).unwrap();
        // a file's tests come last and run on the computer only
        let code = text.split("#[cfg(test)]").next().unwrap_or_default();
        for (n, line) in code.lines().enumerate() {
            let clock = line.contains("std::time") && (line.contains("Instant") || line.contains("SystemTime"));
            let temp = line.contains("std::env::temp_dir") && !line.contains("if cfg!(target_arch = \"wasm32\")");
            if clock || temp {
                found.push(format!("{}:{}: {}", f.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "use claudius::clock and config::temp_dir instead:\n{}", found.join("\n"));
}
