//! `cargo xtask <command>`: the chores of this repository, in Rust so that they run the same on every OS and in CI.
//! `cargo xtask help` lists the commands.

mod dist;
mod release;
mod site;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use xshell::{Shell, cmd};

const HELP: &str = "\
usage: cargo xtask <command>

  ci                      the checks CI runs: formatting, clippy (also of the builds without the ears or the
                          studio window), the tests (with cargo-nextest when it is installed) and the examples
                          in the docs
  web [--public-url URL]  build the web studio with trunk into crates/claudius-ui/dist
  docs                    build the book with mdbook into target/book
  site [--out DIR] [--site-url URL]
                          assemble the website in DIR (target/site): site/ with its placeholders filled in,
                          the book at /docs/, the studio at /studio/, and /install.sh and /install.ps1
  dist [--target TRIPLE] [--headless] [--no-ears] [--web DIR]
                          build the release binaries for TRIPLE (default: this machine) and package them with
                          the web studio into target/dist. --headless leaves out the studio window (no ALSA,
                          X11 or GL) and names the archive ...-headless; --no-ears leaves out earshot and ONNX
                          Runtime, as the Intel Mac and Windows builds always do; --web packages a studio that
                          is already built
  release prepare <patch|minor|major|X.Y.Z>
                          bump the workspace version, Cargo.lock and CHANGELOG.md, then print the new version
  release notes <X.Y.Z>   print that version's section of CHANGELOG.md

Paths are relative to the root of the repository.
";

/// where `cargo xtask web` puts the studio, which is also where `claudius serve` in a checkout looks for it
const WEB_DIST: &str = "crates/claudius-ui/dist";

fn main() -> Result<()> {
    let sh = Shell::new()?;
    sh.change_dir(root());
    let mut args = Args(std::env::args().skip(1).collect());
    match args.next().as_deref() {
        Some("ci") => {
            args.finish()?;
            ci(&sh)
        }
        Some("web") => {
            let public_url = args.value("--public-url")?.unwrap_or_else(|| "./".into());
            args.finish()?;
            web(&sh, &public_url, &root().join(WEB_DIST))
        }
        Some("docs") => {
            args.finish()?;
            docs(&sh)
        }
        Some("site") => {
            let out = args.value("--out")?.unwrap_or_else(|| "target/site".into());
            let site_url = args.value("--site-url")?.unwrap_or_else(|| site::DEFAULT_URL.into());
            args.finish()?;
            site::build(&sh, &root().join(out), &site_url)
        }
        Some("dist") => {
            let target = args.value("--target")?;
            let web = args.value("--web")?.map(|dir| root().join(dir));
            let headless = args.flag("--headless");
            let no_ears = args.flag("--no-ears");
            args.finish()?;
            let target = match target {
                Some(target) => target,
                None => host_target(&sh)?,
            };
            dist::build(&sh, &target, web, headless, no_ears)
        }
        Some("release") => match args.next().as_deref() {
            Some("prepare") => {
                let bump = args.required("patch, minor, major or a version")?;
                args.finish()?;
                release::prepare(&sh, &bump)
            }
            Some("notes") => {
                let version = args.required("a version")?;
                args.finish()?;
                release::notes(&sh, &version)
            }
            _ => bail!("usage: cargo xtask release prepare <patch|minor|major|X.Y.Z>, or release notes <X.Y.Z>"),
        },
        None | Some("help" | "-h" | "--help") => {
            print!("{HELP}");
            Ok(())
        }
        Some(other) => bail!("unknown command `{other}`; `cargo xtask help` lists the commands"),
    }
}

fn ci(sh: &Shell) -> Result<()> {
    let mut steps = vec![
        ("formatting", cmd!(sh, "cargo fmt --all --check")),
        ("clippy", cmd!(sh, "cargo clippy --workspace --all-targets --locked -- -D warnings")),
        // the builds that leave features out, so that code behind cfg(feature = ...) keeps compiling
        (
            "clippy, studio without the ears",
            cmd!(sh, "cargo clippy --locked -p claudius-cli --no-default-features --features studio -- -D warnings"),
        ),
        (
            "clippy, engine without the ears",
            cmd!(sh, "cargo clippy --locked -p claudius -p claudius-server --no-default-features -- -D warnings"),
        ),
        ("check, headless", cmd!(sh, "cargo check --locked -p claudius-cli --no-default-features --features ears")),
    ];
    // the tests' renders and caches go in target/, which `cargo clean` empties, not in the cache of the person running
    // them, where each run would leave its sessions' renders
    let home = root().join("target/claudius-home");
    if cmd!(sh, "cargo nextest --version").quiet().ignore_stdout().ignore_stderr().run().is_ok() {
        let not_docs_examples = "not binary(docs_examples)";
        steps.push((
            "tests",
            cmd!(sh, "cargo nextest run --workspace --locked --no-fail-fast -E {not_docs_examples}")
                .env("CLAUDIUS_HOME", &home),
        ));
        steps.push(("doctests", cmd!(sh, "cargo test --workspace --locked --doc").env("CLAUDIUS_HOME", &home)));
        steps.push((
            "docs examples",
            cmd!(sh, "cargo test -p claudius --locked --test docs_examples").env("CLAUDIUS_HOME", &home),
        ));
    } else {
        eprintln!("cargo-nextest is not installed, so the tests (the docs examples too) run with cargo test");
        steps.push(("tests", cmd!(sh, "cargo test --workspace --locked --no-fail-fast").env("CLAUDIUS_HOME", &home)));
    }

    let mut failed = Vec::new();
    for (name, step) in steps {
        eprintln!("\n== {name}");
        if let Err(e) = step.run() {
            eprintln!("{e}");
            failed.push(name);
        }
    }
    ensure!(failed.is_empty(), "failed: {}", failed.join(", "));
    eprintln!("\nci: all passed");
    Ok(())
}

/// builds the web studio into `out`; `public_url` is where it will be served from ("./" works anywhere)
fn web(sh: &Shell, public_url: &str, out: &Path) -> Result<()> {
    need(sh, "trunk")?;
    let _dir = sh.push_dir("crates/claudius-ui");
    // trunk reads NO_COLOR as a boolean, so the usual NO_COLOR=1 is an error
    let _no_color = std::env::var_os("NO_COLOR").map(|_| sh.push_env("NO_COLOR", "true"));
    cmd!(sh, "trunk build --release --locked --public-url {public_url} --dist {out}").run()?;
    Ok(())
}

fn docs(sh: &Shell) -> Result<()> {
    need(sh, "mdbook")?;
    // docs/book.toml sends the output to target/book
    cmd!(sh, "mdbook build docs").run()?;
    Ok(())
}

fn need(sh: &Shell, tool: &str) -> Result<()> {
    cmd!(sh, "{tool} --version")
        .quiet()
        .ignore_stdout()
        .ignore_stderr()
        .run()
        .with_context(|| format!("{tool} is not installed; docs/src/contributing/setup.md lists the tools"))
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask/ is inside the repository").to_path_buf()
}

/// cargo's build directory: $CARGO_TARGET_DIR, or target/
fn target_dir() -> PathBuf {
    root().join(std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| "target".into()))
}

fn host_target(sh: &Shell) -> Result<String> {
    let info = cmd!(sh, "rustc -vV").read()?;
    info.lines().find_map(|line| line.strip_prefix("host: ")).map(String::from).context("`rustc -vV` names no host")
}

fn workspace_version(sh: &Shell) -> Result<String> {
    let manifest = sh.read_file("Cargo.toml")?;
    let (_, version) = workspace_version_line(&manifest).context("Cargo.toml has no version in [workspace.package]")?;
    Ok(version)
}

/// the index and value of the `version = "..."` line in `[workspace.package]`
fn workspace_version_line(manifest: &str) -> Option<(usize, String)> {
    let mut in_package = false;
    for (i, line) in manifest.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
        } else if in_package
            && let Some(value) = line.strip_prefix("version").and_then(|rest| rest.trim_start().strip_prefix('='))
        {
            return Some((i, value.trim().trim_matches('"').to_string()));
        }
    }
    None
}

/// copies the files under `from` into `to`, leaving out the paths `skip` picks and passing contents through `edit`
fn copy_tree(from: &Path, to: &Path, skip: &dyn Fn(&Path) -> bool, edit: &dyn Fn(Vec<u8>) -> Vec<u8>) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let path = entry?.path();
        if skip(&path) {
            continue;
        }
        let dest = to.join(path.file_name().expect("directory entries have names"));
        if path.is_dir() {
            copy_tree(&path, &dest, skip, edit)?;
        } else {
            std::fs::write(&dest, edit(std::fs::read(&path)?))
                .with_context(|| format!("writing {}", dest.display()))?;
        }
    }
    Ok(())
}

/// replaces `dir` with an empty directory; anything outside target/ is only replaced when it is already empty
fn fresh_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        let target = root().join("target");
        let ours = target.exists() && dir.canonicalize()?.starts_with(target.canonicalize()?);
        let empty = std::fs::read_dir(dir)?.next().is_none();
        ensure!(ours || empty, "not deleting {}: it is outside target/ and not empty", dir.display());
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    Ok(())
}

/// positional arguments in order, and `--name value` or `--name=value` options anywhere
struct Args(Vec<String>);

impl Args {
    fn next(&mut self) -> Option<String> {
        (!self.0.is_empty()).then(|| self.0.remove(0))
    }

    /// whether `name` was given, removing it
    fn flag(&mut self, name: &str) -> bool {
        let found = self.0.iter().position(|arg| arg == name);
        found.map(|i| self.0.remove(i)).is_some()
    }

    fn required(&mut self, what: &str) -> Result<String> {
        self.next().with_context(|| format!("expected {what}"))
    }

    fn value(&mut self, name: &str) -> Result<Option<String>> {
        let inline = format!("{name}=");
        let Some(i) = self.0.iter().position(|arg| arg == name || arg.starts_with(&inline)) else {
            return Ok(None);
        };
        let arg = self.0.remove(i);
        if let Some(value) = arg.strip_prefix(&inline) {
            return Ok(Some(value.to_string()));
        }
        ensure!(i < self.0.len(), "{name} needs a value");
        Ok(Some(self.0.remove(i)))
    }

    fn finish(self) -> Result<()> {
        ensure!(self.0.is_empty(), "unexpected arguments: {}", self.0.join(" "));
        Ok(())
    }
}
