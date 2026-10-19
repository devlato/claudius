//! `cargo xtask release`: the version bump and changelog of a release PR, and the notes of a GitHub release.
//!
//! CHANGELOG.md follows Keep a Changelog: `## [Unreleased]` on top, then `## [X.Y.Z] - YYYY-MM-DD` sections, then
//! the link definitions. Preparing a release turns the commits since the last tag into notes with git-cliff
//! (cliff.toml), puts them under a new version heading together with anything written under Unreleased by hand, and
//! leaves Unreleased empty. It also writes the songs the version saves into crates/claudius/tests/compat, which every
//! later version has to open.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, ensure};
use xshell::{Shell, cmd};

const CHANGELOG: &str = "CHANGELOG.md";
const PLUGIN: &str = "skills/.claude-plugin/plugin.json";
const UNRELEASED: &str = "## [Unreleased]";
const REPO_URL: &str = "https://github.com/devlato/claudius";
const NEW_CHANGELOG: &str = "\
# Changelog

All notable changes to claudius are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
";

pub fn prepare(sh: &Shell, bump: &str) -> Result<()> {
    let manifest = sh.read_file("Cargo.toml")?;
    let (line, current) = crate::workspace_version_line(&manifest).context("no version in [workspace.package]")?;
    let next = next_version(&current, bump)?;
    let tag = format!("v{next}");
    ensure!(cmd!(sh, "git tag --list {tag}").read()?.is_empty(), "the tag {tag} already exists");

    let changelog = if sh.path_exists(CHANGELOG) { sh.read_file(CHANGELOG)? } else { NEW_CHANGELOG.to_string() };
    ensure!(changelog.lines().any(|l| l.starts_with(UNRELEASED)), "CHANGELOG.md has no `{UNRELEASED}` heading");
    ensure!(section(&changelog, &next).is_none(), "CHANGELOG.md already has a section for {next}");
    let mut notes = section(&changelog, "Unreleased").unwrap_or_default();
    match cmd!(sh, "git-cliff --unreleased --tag {tag} --strip all").read() {
        Ok(generated) => notes = merge(&notes, &generated),
        Err(e) => eprintln!("warning: git-cliff did not run ({e}), so the notes are only what is under Unreleased"),
    }
    if notes.is_empty() {
        eprintln!("warning: the notes for {next} are empty; write them in CHANGELOG.md on the release branch");
    }
    // the Claude Code plugin (skills/), which Claude Code offers as an update when its version changes
    let plugin = sh.read_file(PLUGIN)?;
    let old_plugin = format!("\"version\": \"{current}\"");
    ensure!(plugin.contains(&old_plugin), "{PLUGIN} does not say {old_plugin}");

    let metadata = cmd!(sh, "cargo metadata --no-deps --format-version 1 --offline").read()?;
    let members: Vec<(String, String)> =
        member_manifests(&metadata).into_iter().map(|path| Ok((sh.read_file(&path)?, path))).collect::<Result<_>>()?;
    let versioned: Vec<String> = members.iter().filter_map(|(text, _)| workspace_versioned(text)).collect();

    // every manifest this touches, with its old contents, to put back if Cargo.lock does not update
    let mut changed = vec![("Cargo.toml".to_string(), manifest.clone())];
    let bumped: String = manifest
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, text)| if i == line { format!("version = \"{next}\"\n") } else { text.to_string() })
        .collect();
    sh.write_file("Cargo.toml", bumped)?;
    for (text, path) in members {
        let bumped = bump_path_dependencies(&text, &versioned, &current, &next);
        if bumped != text {
            sh.write_file(&path, bumped)?;
            changed.push((path, text));
        }
    }
    if let Err(e) = cmd!(sh, "cargo update --workspace --offline").run() {
        for (path, text) in &changed {
            sh.write_file(path, text)?;
        }
        return Err(e).context("Cargo.lock would not update, so the manifests are back as they were");
    }
    sh.write_file(CHANGELOG, with_release(&changelog, &next, &today(), &notes))?;
    sh.write_file(PLUGIN, plugin.replacen(&old_plugin, &format!("\"version\": \"{next}\""), 1))?;
    // the songs this version saves, which every later claudius has to open (crates/claudius/tests/compat.rs)
    let songs = format!("crates/claudius/tests/compat/v{next}");
    cmd!(sh, "cargo run --quiet --offline -p claudius --no-default-features --example compat_fixtures -- {songs}")
        .run()
        .context("could not write the songs of this version for tests/compat")?;
    let root = crate::root();
    let manifests: Vec<String> = changed
        .iter()
        .map(|(path, _)| Path::new(path).strip_prefix(&root).unwrap_or(Path::new(path)).display().to_string())
        .collect();
    eprintln!(
        "{current} -> {next}: updated {}, Cargo.lock, CHANGELOG.md and {PLUGIN}, and wrote {songs}",
        manifests.join(", ")
    );
    println!("{next}");
    Ok(())
}

/// the Cargo.toml of every workspace member, from `cargo metadata --no-deps` output
fn member_manifests(metadata: &str) -> Vec<String> {
    let key = "\"manifest_path\":\"";
    metadata.split(key).skip(1).filter_map(|rest| rest.split('"').next()).map(|p| p.replace("\\\\", "\\")).collect()
}

/// the package name of a member manifest that takes its version from the workspace
fn workspace_versioned(manifest: &str) -> Option<String> {
    let lines = || manifest.lines().map(|line| line.replace(' ', ""));
    lines().any(|line| line == "version.workspace=true" || line == "version={workspace=true}").then_some(())?;
    lines().find_map(|line| line.strip_prefix("name=").map(|name| name.trim_matches('"').to_string()))
}

/// raises `version = "<old>"` on path dependencies on `crates`, which is how `cargo add --path` writes them; left
/// alone, `^old` stops matching those crates after a minor bump
fn bump_path_dependencies(manifest: &str, crates: &[String], old: &str, new: &str) -> String {
    let (old, new) = (format!("version = \"{old}\""), format!("version = \"{new}\""));
    manifest
        .split_inclusive('\n')
        .map(|line| {
            let name = line.split('=').next().unwrap_or_default().trim();
            if crates.iter().any(|c| c == name) && line.contains("path = \"") {
                line.replace(&old, &new)
            } else {
                line.to_string()
            }
        })
        .collect()
}

pub fn notes(sh: &Shell, version: &str) -> Result<()> {
    let version = version.trim_start_matches('v');
    let changelog = sh.read_file(CHANGELOG)?;
    let notes = section(&changelog, version).with_context(|| format!("CHANGELOG.md has no section for {version}"))?;
    println!("{notes}");
    Ok(())
}

fn next_version(current: &str, bump: &str) -> Result<String> {
    let [major, minor, patch] = numbers(current)?;
    Ok(match bump {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{major}.{}.0", minor + 1),
        "patch" => format!("{major}.{minor}.{}", patch + 1),
        version => {
            let version = version.trim_start_matches('v');
            ensure!(numbers(version)? >= [major, minor, patch], "{version} is older than the current {current}");
            version.to_string()
        }
    })
}

/// major, minor and patch of a version like 1.2.3 or 1.2.3-rc.1
fn numbers(version: &str) -> Result<[u64; 3]> {
    let invalid = || anyhow!("`{version}` is not a version like 1.2.3 (or patch, minor, major)");
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<u64> = core.split('.').map(|n| n.parse().map_err(|_| invalid())).collect::<Result<_>>()?;
    parts.try_into().map_err(|_| invalid())
}

/// the text under `## [name]`, up to the next section or the link definitions
fn section(changelog: &str, name: &str) -> Option<String> {
    let heading = format!("## [{name}]");
    let mut lines = changelog.lines().skip_while(|line| !line.starts_with(&heading));
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|line| !line.starts_with("## ") && !is_link(line)).collect();
    Some(body.join("\n").trim().to_string())
}

/// a Markdown link definition, like `[0.2.0]: https://...`
fn is_link(line: &str) -> bool {
    line.starts_with('[') && line.contains("]: ")
}

/// git-cliff's `### Added`, `### Fixed`, ... sections added to the notes written by hand: under the hand-written
/// heading of the same name when there is one, at the end when there is not
fn merge(by_hand: &str, generated: &str) -> String {
    let mut lines: Vec<String> = by_hand.lines().map(String::from).collect();
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in generated.lines().filter(|line| !line.trim().is_empty() && !line.starts_with("## ")) {
        match sections.last_mut() {
            Some((_, entries)) if !line.starts_with("### ") => entries.push(line.to_string()),
            _ => sections.push((line.to_string(), Vec::new())),
        }
    }
    for (heading, entries) in sections {
        match lines.iter().position(|line| *line == heading) {
            Some(at) => {
                let next =
                    lines[at + 1..].iter().position(|line| line.starts_with("#")).map_or(lines.len(), |i| at + 1 + i);
                let mut end = next;
                while end > at + 1 && lines[end - 1].trim().is_empty() {
                    end -= 1;
                }
                lines.splice(end..end, entries);
            }
            None => {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.extend([heading, String::new()]);
                lines.extend(entries);
            }
        }
    }
    lines.join("\n").trim().to_string()
}

/// the changelog with an empty Unreleased section, then `version` with `notes`, then the older releases, and the
/// compare links updated
fn with_release(changelog: &str, version: &str, date: &str, notes: &str) -> String {
    let previous = changelog
        .lines()
        .filter_map(|line| line.strip_prefix("## [")?.split(']').next())
        .find(|name| *name != "Unreleased");
    let compare = match previous {
        Some(previous) => format!("[{version}]: {REPO_URL}/compare/v{previous}...v{version}"),
        None => format!("[{version}]: {REPO_URL}/releases/tag/v{version}"),
    };
    let unreleased_link = format!("[unreleased]: {REPO_URL}/compare/v{version}...HEAD");

    let mut out: Vec<String> = Vec::new();
    let mut lines = changelog.lines().peekable();
    for line in lines.by_ref() {
        out.push(line.to_string());
        if line.starts_with(UNRELEASED) {
            break;
        }
    }
    while lines.next_if(|line| !line.starts_with("## ") && !is_link(line)).is_some() {}
    out.extend(["".into(), format!("## [{version}] - {date}"), "".into()]);
    if !notes.is_empty() {
        out.extend([notes.to_string(), "".into()]);
    }
    let mut linked = false;
    for line in lines {
        if line.to_lowercase().starts_with("[unreleased]: ") {
            out.extend([unreleased_link.clone(), compare.clone()]);
            linked = true;
        } else {
            out.push(line.to_string());
        }
    }
    if !linked {
        while out.last().is_some_and(|line| line.is_empty()) {
            out.pop();
        }
        out.extend(["".into(), unreleased_link, compare]);
    }
    out.join("\n") + "\n"
}

fn today() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).expect("the clock is after 1970").as_secs();
    date(secs as i64 / 86_400)
}

/// days since 1970-01-01 as a YYYY-MM-DD date in the proleptic Gregorian calendar
fn date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumps() {
        assert_eq!(next_version("0.3.7", "patch").unwrap(), "0.3.8");
        assert_eq!(next_version("0.3.7", "minor").unwrap(), "0.4.0");
        assert_eq!(next_version("0.3.7", "major").unwrap(), "1.0.0");
        assert_eq!(next_version("0.3.7", "v0.5.0").unwrap(), "0.5.0");
        assert_eq!(next_version("0.1.0", "0.1.0").unwrap(), "0.1.0");
        assert!(next_version("0.3.7", "0.3.6").is_err());
        assert!(next_version("0.3.7", "next").is_err());
    }

    #[test]
    fn raises_path_dependencies_on_workspace_crates() {
        let manifest = "[dependencies]\nfoo = { version = \"0.1.0\", path = \"../foo\" }\n\
                        own = { version = \"0.1.0\", path = \"../own\" }\nserde = \"0.1.0\"\n";
        assert_eq!(
            bump_path_dependencies(manifest, &["foo".into()], "0.1.0", "0.2.0"),
            "[dependencies]\nfoo = { version = \"0.2.0\", path = \"../foo\" }\n\
             own = { version = \"0.1.0\", path = \"../own\" }\nserde = \"0.1.0\"\n"
        );
        let foo = "[package]\nname = \"foo\"\nversion.workspace = true\n\n[[bin]]\nname = \"f\"\n";
        assert_eq!(workspace_versioned(foo).as_deref(), Some("foo"));
        assert_eq!(workspace_versioned("[package]\nname = \"own\"\nversion = \"0.1.0\"\n"), None);
        let metadata =
            r#"{"packages":[{"name":"a","manifest_path":"C:\\w\\a\\Cargo.toml"},{"manifest_path":"/w/b/Cargo.toml"}]}"#;
        assert_eq!(member_manifests(metadata), [r"C:\w\a\Cargo.toml", "/w/b/Cargo.toml"]);
    }

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(11_016), "2000-02-29");
        assert_eq!(date(20_742), "2026-10-16");
    }

    #[test]
    fn moves_unreleased_into_the_release() {
        let before = "# Changelog\n\n## [Unreleased]\n\nHand-written.\n\n## [0.1.0] - 2026-01-02\n\n- First.\n\n\
                      [unreleased]: https://github.com/devlato/claudius/compare/v0.1.0...HEAD\n\
                      [0.1.0]: https://github.com/devlato/claudius/releases/tag/v0.1.0\n";
        let notes =
            merge(&section(before, "Unreleased").unwrap(), "## [0.2.0] - 2026-10-16\n\n### Added\n\n- A thing.\n");
        let after = with_release(before, "0.2.0", "2026-10-16", &notes);
        assert_eq!(
            after,
            "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - 2026-10-16\n\nHand-written.\n\n### Added\n\n- A thing.\n\n\
             ## [0.1.0] - 2026-01-02\n\n- First.\n\n\
             [unreleased]: https://github.com/devlato/claudius/compare/v0.2.0...HEAD\n\
             [0.2.0]: https://github.com/devlato/claudius/compare/v0.1.0...v0.2.0\n\
             [0.1.0]: https://github.com/devlato/claudius/releases/tag/v0.1.0\n"
        );
        assert_eq!(section(&after, "0.2.0").unwrap(), "Hand-written.\n\n### Added\n\n- A thing.");
        assert_eq!(section(&after, "Unreleased").unwrap(), "");
    }

    #[test]
    fn generated_entries_join_the_hand_written_section_of_the_same_name() {
        let by_hand = "The first public release.\n\n### Added\n\n- A beatmaker that composes,\n  arranges and mixes.\n";
        let generated = "## [0.1.0] - 2026-10-16\n\n### Added\n\n- script: Add euclid()\n\n### Fixed\n\n- A bug\n";
        assert_eq!(
            merge(by_hand, generated),
            "The first public release.\n\n### Added\n\n- A beatmaker that composes,\n  arranges and mixes.\n\
             - script: Add euclid()\n\n### Fixed\n\n- A bug"
        );
        assert_eq!(merge("", generated), "### Added\n\n- script: Add euclid()\n\n### Fixed\n\n- A bug");
        assert_eq!(merge("Only words.", ""), "Only words.");
    }

    #[test]
    fn first_release() {
        let after = with_release(NEW_CHANGELOG, "0.1.0", "2026-10-16", "First public release.");
        assert!(after.ends_with(
            "## [Unreleased]\n\n## [0.1.0] - 2026-10-16\n\nFirst public release.\n\n\
             [unreleased]: https://github.com/devlato/claudius/compare/v0.1.0...HEAD\n\
             [0.1.0]: https://github.com/devlato/claudius/releases/tag/v0.1.0\n"
        ));
    }
}
