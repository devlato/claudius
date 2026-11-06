//! `cargo xtask perf`: how many instructions each of the engine's main steps takes (the workloads of
//! `crates/claudius/examples/perf.rs`), counted by Valgrind's cachegrind, which counts the same on every run of the same
//! build where a clock on a shared machine wanders by a tenth. With `--base`, the base's build is counted too, and a
//! workload that takes more than `--threshold` percent more fails, unless the pull request says it may
//! (`CLAUDIUS_PERF_ACCEPT`, its description: a line `perf: render +20%`). Without Valgrind (on a Mac), each workload's
//! wall time is printed instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use xshell::{Shell, cmd};

use crate::root;

/// the profile the workloads are built with: the release's optimisations without its link-time ones, which only make
/// the build slower to count the same code
const PROFILE: &str = "perf";

/// builds the workloads of the checkout at `dir` and copies them to `target/perf-bin/<name>`, so that another build
/// does not overwrite them
fn build(sh: &Shell, dir: &Path, name: &str) -> Result<PathBuf> {
    let _dir = sh.push_dir(dir);
    let _target = sh.push_env("CARGO_TARGET_DIR", root().join("target"));
    cmd!(sh, "cargo build --profile {PROFILE} --locked -p claudius --example perf").run()?;
    let bin =
        root().join("target").join(PROFILE).join("examples").join(format!("perf{}", std::env::consts::EXE_SUFFIX));
    let kept = root().join("target/perf-bin").join(name);
    std::fs::create_dir_all(kept.parent().expect("a folder"))?;
    std::fs::copy(&bin, &kept).with_context(|| format!("cannot copy {}", bin.display()))?;
    Ok(kept)
}

/// the instructions a run counted, from cachegrind's summary on stderr (`==12345== I refs:      1,234,567`)
pub fn instructions(stderr: &str) -> Option<u64> {
    stderr.lines().find_map(|l| {
        let (_, after) = l.split_once("I")?;
        let after = after.trim_start().strip_prefix("refs:")?;
        after.trim().replace(',', "").parse().ok()
    })
}

/// the instructions `bin` takes to run `workload` (`--setup` only, or all of it), counted by cachegrind on one thread
fn counted(sh: &Shell, bin: &Path, workload: &str, setup_only: bool) -> Result<u64> {
    let setup = setup_only.then_some("--setup");
    let out = cmd!(
        sh,
        "valgrind --tool=cachegrind --cache-sim=no --cachegrind-out-file=/dev/null {bin} {workload} {setup...}"
    )
    .env("RAYON_NUM_THREADS", "1")
    .quiet()
    .ignore_status()
    .output()?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    ensure!(out.status.success(), "{workload} failed under valgrind:\n{stderr}");
    instructions(&stderr)
        .with_context(|| format!("no instruction count in valgrind's output for {workload}:\n{stderr}"))
}

/// what each workload of `bin` takes beyond its setup
fn count_all(sh: &Shell, bin: &Path) -> Result<BTreeMap<String, u64>> {
    let mut out = BTreeMap::new();
    for w in cmd!(sh, "{bin} --list").read()?.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (all, setup) = (counted(sh, bin, w, false)?, counted(sh, bin, w, true)?);
        eprintln!("{w}: {} instructions", all.saturating_sub(setup));
        out.insert(w.to_string(), all.saturating_sub(setup));
    }
    Ok(out)
}

/// The growth a pull request says a workload may take, from the lines of its description that read
/// `perf: <workload> +<percent>%` (anything may follow, such as why).
pub fn accepted(description: &str) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for line in description.lines() {
        let Some(rest) = line.trim().strip_prefix("perf:") else { continue };
        let mut words = rest.split_whitespace();
        let (Some(name), Some(by)) = (words.next(), words.next()) else { continue };
        if let Some(pct) = by.strip_prefix('+').and_then(|b| b.strip_suffix('%')).and_then(|b| b.parse::<f64>().ok()) {
            out.insert(name.to_string(), pct);
        }
    }
    out
}

/// One workload's line in the report, and whether it failed: the base's count, this build's, the change, and what the
/// pull request allows it.
pub struct Line {
    pub name: String,
    pub base: Option<u64>,
    pub head: u64,
    pub change: Option<f64>,
    pub allowed: f64,
    pub failed: bool,
}

/// each workload against the base's, failing those that grew by more than `threshold` percent or than the pull request
/// allows them, whichever is more
pub fn compare(
    head: &BTreeMap<String, u64>,
    base: Option<&BTreeMap<String, u64>>,
    threshold: f64,
    allowed: &BTreeMap<String, f64>,
) -> Vec<Line> {
    head.iter()
        .map(|(name, &h)| {
            let b = base.and_then(|b| b.get(name)).copied();
            let change = b.filter(|b| *b > 0).map(|b| (h as f64 / b as f64 - 1.0) * 100.0);
            let allowed = allowed.get(name).copied().unwrap_or(0.0).max(threshold);
            Line { name: name.clone(), base: b, head: h, change, allowed, failed: change.is_some_and(|c| c > allowed) }
        })
        .collect()
}

/// the report, as a Markdown table
pub fn report(lines: &[Line], base: Option<&str>) -> String {
    let millions = |n: u64| format!("{:.1} M", n as f64 / 1e6);
    let mut s = String::from("## Instructions per workload\n\n");
    match base {
        Some(b) => s.push_str(&format!("Counted by cachegrind, against the base `{b}`.\n\n")),
        None => s.push_str("Counted by cachegrind.\n\n"),
    }
    s.push_str("| workload | base | this | change |\n|---|---:|---:|---:|\n");
    for l in lines {
        let change = match l.change {
            Some(c) if l.failed => format!("**{c:+.1} %** (more than {:.0} %)", l.allowed),
            Some(c) => format!("{c:+.1} %"),
            None => "new".into(),
        };
        s.push_str(&format!(
            "| {} | {} | {} | {change} |\n",
            l.name,
            l.base.map_or_else(|| "-".into(), millions),
            millions(l.head)
        ));
    }
    s
}

/// `cargo xtask perf [--base REV] [--threshold PCT]`, as the module's doc says
pub fn run(sh: &Shell, base: Option<String>, threshold: f64) -> Result<()> {
    let valgrind = cmd!(sh, "valgrind --version").quiet().ignore_stdout().ignore_stderr().run().is_ok();
    let head = build(sh, &root(), "head")?;
    if !valgrind {
        ensure!(base.is_none(), "valgrind is not installed: the instruction counts need it (on Linux)");
        eprintln!("valgrind is not installed, so here is each workload's wall time on this machine instead:");
        for w in cmd!(sh, "{head} --list").read()?.lines() {
            let took = cmd!(sh, "{head} {w}").env("RAYON_NUM_THREADS", "1").quiet().read_stderr()?;
            println!("{w}: {}", took.trim().trim_start_matches("perf: "));
        }
        return Ok(());
    }
    let counts = count_all(sh, &head)?;
    let base_counts = match &base {
        None => None,
        Some(rev) => {
            let dir = root().join("target/perf-base");
            if dir.exists() {
                cmd!(sh, "git worktree remove --force {dir}").quiet().run().ok();
            }
            cmd!(sh, "git worktree add --detach --force {dir} {rev}").quiet().run()?;
            let counted = match dir.join("crates/claudius/examples/perf.rs").exists() {
                true => build(sh, &dir, "base").and_then(|bin| count_all(sh, &bin)).map(Some),
                // the pull request that brings the workloads: this build counted again, which shows how far a count
                // wanders from one run to the next
                false => {
                    eprintln!("the base has no workloads: counting this build again, against itself");
                    count_all(sh, &head).map(Some)
                }
            };
            cmd!(sh, "git worktree remove --force {dir}").quiet().run().ok();
            counted?
        }
    };
    let allowed = accepted(&std::env::var("CLAUDIUS_PERF_ACCEPT").unwrap_or_default());
    let lines = compare(&counts, base_counts.as_ref(), threshold, &allowed);
    let text = report(&lines, base.as_deref().filter(|_| base_counts.is_some()));
    println!("{text}");
    if let Ok(summary) = std::env::var("GITHUB_STEP_SUMMARY") {
        std::fs::write(&summary, format!("{}{text}", std::fs::read_to_string(&summary).unwrap_or_default()))
            .with_context(|| format!("cannot write {summary}"))?;
    }
    let failed: Vec<&str> = lines.iter().filter(|l| l.failed).map(|l| l.name.as_str()).collect();
    if !failed.is_empty() {
        bail!(
            "{} took more instructions than {threshold:.0} % over the base: if it should, say so in the pull request's \
             description, a line such as `perf: {} +25%` and why",
            failed.join(", "),
            failed[0]
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_count_is_read_from_cachegrinds_summary() {
        let out = "==4182== Cachegrind, a high-precision tracing profiler\n==4182== \n==4182== I refs:        1,234,567,890\n";
        assert_eq!(instructions(out), Some(1_234_567_890));
        assert_eq!(instructions("==1== I   refs:      42\n"), Some(42));
        assert_eq!(instructions("perf: 12.0 ms\n"), None);
    }

    #[test]
    fn a_pull_request_says_which_workloads_may_take_more() {
        let body =
            "It renders twice the voices.\n\nperf: render +25% (twice the voices)\nperf: judge +5%\nperf: nonsense";
        let a = accepted(body);
        assert_eq!(a.get("render"), Some(&25.0));
        assert_eq!(a.get("judge"), Some(&5.0));
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn a_workload_fails_past_the_threshold_or_what_the_pull_request_allows() {
        let head: BTreeMap<String, u64> = [("render".into(), 120), ("judge".into(), 105), ("new".into(), 7)].into();
        let base: BTreeMap<String, u64> = [("render".into(), 100), ("judge".into(), 100)].into();
        let lines = compare(&head, Some(&base), 10.0, &BTreeMap::new());
        let failed = |lines: &[Line], n: &str| lines.iter().find(|l| l.name == n).unwrap().failed;
        assert!(failed(&lines, "render") && !failed(&lines, "judge") && !failed(&lines, "new"));
        let lines = compare(&head, Some(&base), 10.0, &[("render".to_string(), 25.0)].into());
        assert!(!failed(&lines, "render"));
        assert!(report(&lines, Some("abc")).contains("| render | 0.0 M | 0.0 M | +20.0 % |"));
        // without a base, nothing fails
        assert!(compare(&head, None, 10.0, &BTreeMap::new()).iter().all(|l| !l.failed));
    }
}
