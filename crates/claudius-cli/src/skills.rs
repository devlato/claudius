//! The skills for AI assistants (`skills/` in the repository), built into the binary so that they always match the
//! claudius that runs them: `claudius skills` writes them out, and `claudius connect` installs them where an
//! assistant reads skills.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// every file of every skill, by its path under `skills/`
pub const FILES: &[(&str, &str)] = &[
    ("claudius-songs/SKILL.md", include_str!("../../../skills/claudius-songs/SKILL.md")),
    ("claudius-songs/references/script.md", include_str!("../../../skills/claudius-songs/references/script.md")),
    ("claudius-songs/references/verdict.md", include_str!("../../../skills/claudius-songs/references/verdict.md")),
    ("claudius-songs/references/genres.md", include_str!("../../../skills/claudius-songs/references/genres.md")),
    ("claudius-songs/references/tools.md", include_str!("../../../skills/claudius-songs/references/tools.md")),
    ("claudius-remix/SKILL.md", include_str!("../../../skills/claudius-remix/SKILL.md")),
    ("claudius-setup/SKILL.md", include_str!("../../../skills/claudius-setup/SKILL.md")),
];

/// the skills, by folder name
pub fn names() -> Vec<&'static str> {
    let mut names: Vec<&str> = FILES.iter().filter_map(|(path, _)| path.split('/').next()).collect();
    names.dedup();
    names
}

/// write every skill into `dir/<name>/`, replacing an older copy of each; returns the skill folders
pub fn install(dir: &Path) -> Result<Vec<PathBuf>> {
    uninstall(dir)?;
    for (path, text) in FILES {
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
        }
        std::fs::write(&file, text).with_context(|| format!("cannot write {}", file.display()))?;
    }
    Ok(names().into_iter().map(|n| dir.join(n)).collect())
}

/// take the skills out of `dir` again (only claudius' own folders); returns how many there were
pub fn uninstall(dir: &Path) -> Result<usize> {
    let mut removed = 0;
    for name in names() {
        let folder = dir.join(name);
        if folder.is_dir() {
            std::fs::remove_dir_all(&folder).with_context(|| format!("cannot remove {}", folder.display()))?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// one zip per skill in `dir` (`claudius-songs.zip` ...), each with the skill's folder at its root, which is how the
/// Claude apps take an uploaded skill
pub fn zips(dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut out = Vec::new();
    for name in names() {
        let path = dir.join(format!("{name}.zip"));
        let file = std::fs::File::create(&path).with_context(|| format!("cannot write {}", path.display()))?;
        let mut zip = zip::ZipWriter::new(file);
        for (file_path, text) in FILES.iter().filter(|(p, _)| p.split('/').next() == Some(name)) {
            zip.start_file(*file_path, options)?;
            zip.write_all(text.as_bytes())?;
        }
        zip.finish()?;
        out.push(path);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("claudius-skills-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn every_file_in_the_skills_folder_is_built_in() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills");
        let mut found = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.parent() != Some(root.as_path()) {
                    // files at the top (the README) explain the skills; each skill is a folder
                    found.push(p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        found.sort();
        let mut built: Vec<String> = FILES.iter().map(|(p, _)| p.to_string()).collect();
        built.sort();
        assert_eq!(found, built, "skills/ and skills::FILES differ");
    }

    #[test]
    fn skills_install_replace_and_uninstall() {
        let dir = temp("install");
        std::fs::create_dir_all(dir.join("claudius-songs/old")).unwrap();
        std::fs::create_dir_all(dir.join("someone-elses-skill")).unwrap();
        let folders = install(&dir).unwrap();
        assert_eq!(folders.len(), names().len());
        assert!(dir.join("claudius-songs/SKILL.md").is_file() && !dir.join("claudius-songs/old").exists());
        assert_eq!(uninstall(&dir).unwrap(), names().len());
        assert!(dir.join("someone-elses-skill").is_dir() && !dir.join("claudius-songs").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_zip_holds_one_skill_folder() {
        let dir = temp("zips");
        let zips = zips(&dir).unwrap();
        assert_eq!(zips.len(), names().len());
        let archive = zip::ZipArchive::new(std::fs::File::open(dir.join("claudius-songs.zip")).unwrap()).unwrap();
        let mut entries: Vec<String> = archive.file_names().map(|n| n.unwrap().into_owned()).collect();
        entries.sort();
        assert!(entries.iter().all(|e| e.starts_with("claudius-songs/")), "{entries:?}");
        assert!(entries.iter().any(|e| e == "claudius-songs/SKILL.md"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_skill_has_a_name_and_description_that_fit() {
        for name in names() {
            let text = FILES.iter().find(|(p, _)| *p == format!("{name}/SKILL.md")).unwrap().1;
            let front = text.strip_prefix("---\n").and_then(|t| t.split_once("\n---\n")).unwrap().0;
            assert!(front.contains(&format!("name: {name}\n")), "{name}: the name differs from its folder");
            let description = front.split_once("description:").unwrap().1;
            let words: String = description.split_whitespace().filter(|w| *w != ">-").collect::<Vec<_>>().join(" ");
            assert!(!words.is_empty() && words.len() <= 1024, "{name}: description of {} characters", words.len());
            assert!(text.lines().count() < 500, "{name}: SKILL.md is over 500 lines");
        }
    }
}
