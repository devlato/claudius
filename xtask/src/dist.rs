//! `cargo xtask dist`: the release archive for one target, laid out the way `claudius` looks for its files:
//!
//! ```text
//! claudius-<version>-<target>/
//!   bin/claudius, bin/claudius-packs
//!   share/claudius/web/        the web studio (found at <exe dir>/../share/claudius/web)
//!   README.md, LICENSE, NOTICE, CHANGELOG.md
//! ```
//!
//! What goes into `claudius` are the two features of claudius-cli, both on by default: `studio`, the native window,
//! and `ears`, earshot's models through ONNX Runtime. A `--headless` archive leaves the window out, so the binary
//! links no ALSA, X11 or GL, and is named `claudius-<version>-<target>-headless`. The ears are left out with
//! `--no-ears`, and always on the targets in `NO_EARS`; those archives keep their usual names.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use xshell::{Shell, cmd};

use crate::{WEB_DIST, copy_tree, fresh_dir, root};

const BINARIES: &[&str] = &["claudius", "claudius-packs"];
const DOCS: &[&str] = &["README.md", "LICENSE", "NOTICE", "CHANGELOG.md"];

/// Targets built without the ears. ort has no ONNX Runtime for Intel Macs. On Windows its ONNX Runtime always
/// links DirectML and DirectX 12 (DXCORE needs Windows 10 2004), and nobody has run that yet.
const NO_EARS: &[&str] = &["x86_64-apple-darwin", "x86_64-pc-windows-msvc"];

pub fn build(sh: &Shell, target: &str, web: Option<PathBuf>, headless: bool, no_ears: bool) -> Result<()> {
    let version = crate::workspace_version(sh)?;
    let windows = target.contains("windows");
    let ears = !no_ears && !NO_EARS.contains(&target);
    let web = match web {
        Some(dir) => dir,
        None => {
            crate::web(sh, "./", &root().join(WEB_DIST))?;
            root().join(WEB_DIST)
        }
    };
    ensure!(web.join("index.html").is_file(), "{} is not a build of the web studio (no index.html)", web.display());

    let mut features = Vec::new();
    if headless || !ears {
        let wanted: Vec<&str> = [(!headless).then_some("claudius-cli/studio"), ears.then_some("claudius-cli/ears")]
            .into_iter()
            .flatten()
            .collect();
        features.push("--no-default-features".to_string());
        if !wanted.is_empty() {
            features.extend(["--features".to_string(), wanted.join(",")]);
        }
    }
    eprintln!(
        "dist: {target}, {}, {}",
        if headless { "no studio window" } else { "with the studio window" },
        if ears { "with the ears" } else { "no ears" }
    );
    cmd!(sh, "cargo build --release --locked --target {target} --package claudius-cli --package claudius-packs --bins {features...}")
        .run()?;
    let built = crate::target_dir().join(target).join("release");

    let name = format!("claudius-{version}-{target}{}", if headless { "-headless" } else { "" });
    let dist = root().join("target/dist");
    let stage = dist.join(&name);
    fresh_dir(&stage)?;
    let bin = stage.join("bin");
    std::fs::create_dir_all(&bin)?;
    let exe = if windows { ".exe" } else { "" };
    for binary in BINARIES {
        let file = format!("{binary}{exe}");
        std::fs::copy(built.join(&file), bin.join(&file)).with_context(|| format!("copying {file}"))?;
    }
    if windows {
        // with the ears, ort puts the ONNX Runtime and DirectML DLLs the executables load next to them
        for entry in std::fs::read_dir(&built)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("dll")) {
                std::fs::copy(&path, bin.join(path.file_name().expect("files have names")))?;
            }
        }
    }
    copy_tree(&web, &stage.join("share/claudius/web"), &|_| false, &|bytes| bytes)?;
    for doc in DOCS {
        let path = root().join(doc);
        if path.exists() {
            std::fs::copy(&path, stage.join(doc))?;
        } else if *doc != "NOTICE" {
            eprintln!("warning: there is no {doc}, so the archive goes without it");
        }
    }

    let archive = if windows { format!("{name}.zip") } else { format!("{name}.tar.gz") };
    {
        let _dir = sh.push_dir(&dist);
        // keeps macOS's tar from adding ._ files with extended attributes
        let _no_xattrs = sh.push_env("COPYFILE_DISABLE", "1");
        sh.remove_path(&archive)?;
        if windows {
            // Windows' own tar.exe (bsdtar) writes zip files; the GNU tar that comes with Git for Windows cannot
            let tar = match std::env::var_os("SystemRoot") {
                Some(system) => Path::new(&system).join("System32").join("tar.exe"),
                None => PathBuf::from("tar"),
            };
            cmd!(sh, "{tar} -a -c -f {archive} {name}").run()?;
        } else {
            // macOS keeps extended attributes such as com.apple.provenance in tar files, and GNU tar warns about them
            cmd!(sh, "tar --no-xattrs -czf {archive} {name}").run()?;
        }
    }
    let line = format!("{}  {archive}\n", sha256(&dist.join(&archive))?);
    sh.write_file(dist.join(format!("{archive}.sha256")), &line)?;
    write_sums(sh, &dist)?;
    eprintln!("dist: {}", dist.join(&archive).display());
    Ok(())
}

fn sha256(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        match file.read(&mut buf)? {
            0 => break,
            n => hasher.update(&buf[..n]),
        }
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// SHA256SUMS for every archive in target/dist, so that the directory can stand in for a release
fn write_sums(sh: &Shell, dist: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(dist)?.map(|entry| entry.map(|e| e.path())).collect::<Result<_, _>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "sha256"));
    files.sort();
    let mut sums = String::new();
    for file in files {
        sums += &sh.read_file(file)?;
    }
    sh.write_file(dist.join("SHA256SUMS"), sums)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_of_a_file_is_the_usual_hex() {
        let p = std::env::temp_dir().join(format!("xtask-sha256-{}", std::process::id()));
        std::fs::write(&p, b"abc").unwrap();
        let sum = super::sha256(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(sum, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
