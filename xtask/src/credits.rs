//! The book's list of every library claudius is built of (`docs/src/credits/libraries.md`), written before the book is
//! built: each crate the workspace depends on, on every target, as a normal or a build dependency, with its versions,
//! its licence and where its source is, as `cargo tree` reads them from Cargo.lock. It is not kept in the repository,
//! so it is never out of date.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::root;

/// where the list goes
const PAGE: &str = "docs/src/credits/libraries.md";

/// a crate in the lock: its versions, its licence, and where its source is
#[derive(Default)]
struct Crate {
    versions: Vec<String>,
    licence: String,
    source: String,
}

/// `PAGE`, written afresh from Cargo.lock
pub fn libraries(sh: &Shell) -> Result<()> {
    let format = "{p}|{l}|{r}";
    let tree = cmd!(sh, "cargo tree --workspace --target all -e normal,build --prefix none --format {format} --locked")
        .quiet()
        .read()
        .context("cargo tree, for the list of libraries")?;
    let mut crates: BTreeMap<String, Crate> = BTreeMap::new();
    for line in tree.lines() {
        // a crate met again is marked so at the end of its line
        let mut cols = line.trim_end_matches(" (*)").splitn(3, '|');
        let (Some(package), Some(licence), Some(source)) = (cols.next(), cols.next(), cols.next()) else { continue };
        // the workspace's own crates, which are path dependencies: "claudius v0.8.0 (/Users/...)"
        if package.contains(" (/") || package.contains(" (file:") || package.contains(":\\") {
            continue;
        }
        let package = package.trim_end_matches(" (proc-macro)");
        let Some((name, version)) = package.split_once(" v") else { continue };
        let c = crates.entry(name.to_string()).or_default();
        if !c.versions.iter().any(|v| v == version) {
            c.versions.push(version.to_string());
        }
        (c.licence, c.source) = (licence.to_string(), source.to_string());
    }
    let mut page = format!(
        "<!-- written by `cargo xtask docs` from Cargo.lock each time the book is built: edits here are lost -->\n\n\
         # Libraries\n\n\
         Every Rust crate claudius is built of, {} of them: those of the app, the command line, the server and the \
         studio in the browser, on every platform it runs on, as normal and build dependencies. Each has the versions \
         Cargo.lock holds and the licence it comes under, and links to its source. claudius uses them as they are \
         published, changing none. The book writes this page from Cargo.lock each time it is built, so it never goes \
         out of date; the rest of what claudius is built on is in [Credits](../credits.md).\n\n\
         | Crate | Version | Licence |\n|---|---|---|\n",
        crates.len()
    );
    for (name, c) in &crates {
        let link = match c.source.trim() {
            "" => format!("https://crates.io/crates/{name}"),
            source => source.to_string(),
        };
        page.push_str(&format!("| [{name}]({link}) | {} | {} |\n", c.versions.join(", "), c.licence.trim()));
    }
    let path = root().join(PAGE);
    std::fs::create_dir_all(path.parent().expect("a folder")).context("the folder for the list of libraries")?;
    std::fs::write(&path, page).with_context(|| format!("cannot write {}", path.display()))
}
