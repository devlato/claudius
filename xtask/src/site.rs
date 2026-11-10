//! `cargo xtask site`: the website in one directory, as Cloudflare serves it (site/wrangler.toml).

use std::path::Path;

use anyhow::{Result, anyhow, ensure};
use xshell::{Shell, cmd};

use crate::{copy_tree, fresh_dir, root};

/// where a local build says it is served; a deployed site gets its address from `--site-url` (the repository variable
/// `SITE_URL`)
pub const DEFAULT_URL: &str = "http://localhost:8000";
const REPO_URL: &str = "https://github.com/devlato/claudius";

/// the entries of site/ that are tooling (the smoke test, its packages, the scripts that make the demos, the Worker),
/// not pages;
/// the same list as PRIVATE in site/tests/serve.mjs
const TOOLING: &[&str] = &[
    ".DS_Store",
    ".gitignore",
    "README.md",
    "blob-report",
    "node_modules",
    "package-lock.json",
    "package.json",
    "playwright-report",
    "playwright.config.mjs",
    "test-results",
    "tests",
    "tools",
    "worker",
    "wrangler.toml",
];

/// Cloudflare serves unknown extensions as application/octet-stream, which PowerShell's `irm | iex` cannot run
const HEADERS: &str = "\
/install.sh
  Content-Type: text/plain; charset=utf-8
/install.ps1
  Content-Type: text/plain; charset=utf-8
";

pub fn build(sh: &Shell, out: &Path, site_url: &str, sounds: bool) -> Result<()> {
    let version = crate::workspace_version(sh)?;
    let site_url = site_url.trim_end_matches('/');
    fresh_dir(out)?;

    let site = root().join("site");
    if site.is_dir() {
        let tooling = |path: &Path| {
            path.parent() == Some(site.as_path())
                && path.file_name().is_some_and(|name| TOOLING.iter().any(|t| name == *t))
        };
        let fill = |bytes: Vec<u8>| match String::from_utf8(bytes) {
            Ok(text) => text
                .replace("{{version}}", &version)
                .replace("{{site_url}}", site_url)
                .replace("{{repo_url}}", REPO_URL)
                .into_bytes(),
            Err(binary) => binary.into_bytes(),
        };
        copy_tree(&site, out, &tooling, &fill)?;
    } else {
        eprintln!("warning: there is no site/, so the site gets a plain index.html instead of the landing page");
        sh.write_file(out.join("index.html"), placeholder(&version))?;
    }

    crate::docs(sh)?;
    copy_tree(&root().join("target/book"), &out.join("docs"), &|_| false, &|bytes| bytes)?;

    crate::web(sh, "/studio/", &out.join("studio"))?;
    // the page a phone plays along with the demo on: the one `claudius serve` serves, reaching the demo by the relay
    copy_tree(&root().join("crates/claudius-server/src/phone"), &out.join("phone"), &|_| false, &|bytes| bytes)?;
    if sounds {
        web_packs(sh, &out.join("studio").join("packs"))?;
        web_model(sh, &out.join("studio").join("models"))?;
    } else {
        eprintln!(
            "note: no instruments for the demo, and its palette finds actions by name only (--sounds makes them)"
        );
    }

    for script in ["install.sh", "install.ps1"] {
        sh.copy_file(root().join("scripts").join(script), out)?;
    }
    // Pages joins a header that two rules set with a comma, so site/_headers, when there is one, is left to say it
    let headers = out.join("_headers");
    if !headers.exists() {
        sh.write_file(&headers, HEADERS)?;
    } else if !sh.read_file(&headers)?.lines().any(|line| line.trim() == "/install.ps1") {
        eprintln!(
            "warning: site/_headers has no rule for /install.ps1; `irm | iex` needs it served as text:\n{HEADERS}"
        );
    }

    eprintln!("site: {} (claudius {version} at {site_url})", out.display());
    Ok(())
}

/// The instruments the web studio's demo plays (`pack:instrument`, and `=` the names it also answers to), from packs
/// the site may pass on (CC0, public domain, CC BY). A song asks for a Rhodes most of all, and jRhodes' samples may not
/// be passed on for others to use as they like, so the Wurlitzer answers for it.
const WEB_INSTRUMENTS: &[&str] = &[
    "salamander:piano",
    "epianos:wurlitzer=rhodes",
    "epianos:cp80",
    "freepats:upright_piano",
    "freepats:drawbar_organ",
    "vsco2:strings",
    "vsco2:strings_pizz",
    "vsco2:violins",
    "vsco2:cellos",
    "vsco2:brass",
    "vsco2:trumpet",
    "vsco2:horn",
    "vsco2:flute",
    "vsco2:harp",
    "vcsl:tenor_sax",
    "vcsl:vibraphone",
    "vcsl:marimba",
    "vcsl:kalimba",
    "vcsl:glockenspiel",
    "vcsl:tubular_bells",
    "vcsl:congas",
    "vcsl:bongos",
    "vcsl:shaker",
    "vcsl:tambourine",
    "vcsl:claves",
    "freepats:classical_guitar",
    "freepats:clean_guitar",
    "freepats:jazz_guitar",
    "freepats:dist_guitar",
    "freepats:finger_bass",
    "freepats:pick_bass",
    "doublebass:upright_bass",
    "growlybass:bass",
    "voice:voice_ah",
    "voice:choir",
    "muldjord:bd",
    "muldjord:sd",
    "muldjord:hh",
    "muldjord:oh",
    "muldjord:lt",
    "muldjord:mt",
    "muldjord:ht",
    "muldjord:cr",
    "muldjord:rd",
    "tr808:bd",
    "tr808:sd",
    "tr808:lt",
    "tr808:mt",
    "tr808:ht",
    "tr808:lc",
    "tr808:mc",
    "tr808:hc",
    "tr808:rim",
    "tr808:cl",
    "tr808:cp",
    "tr808:sh",
    "tr808:cb",
    "tr808:cr",
    "tr808:oh",
    "tr808:hh",
    "tr707:bd",
    "tr707:sd",
    "tr707:lt",
    "tr707:mt",
    "tr707:ht",
    "tr707:hh",
    "tr707:oh",
    "tr707:cr",
    "tr707:rd",
    "tr707:cb",
    "tr707:cp",
    "tr707:rim",
    "tr707:tb",
];

/// The demo's instruments, as `claudius-packs web` keeps them for a browser (a few seconds of a note every few
/// semitones, as Ogg Vorbis), in `out`: their packs are installed into a folder of their own, lazily, so that only the
/// notes kept are downloaded.
fn web_packs(sh: &Shell, out: &Path) -> Result<()> {
    let _packs = sh.push_env("CLAUDIUS_PACKS", root().join("target/demo-sounds/packs"));
    let packs: std::collections::BTreeSet<&str> = WEB_INSTRUMENTS.iter().filter_map(|p| p.split(':').next()).collect();
    let features = "claudius-encode/vorbis";
    cmd!(sh, "cargo run --locked --release -p claudius-packs --features {features} -- install --lazy {packs...}")
        .run()?;
    cmd!(sh, "cargo run --locked --release -p claudius-packs --features {features} -- web {out} /studio/packs {WEB_INSTRUMENTS...}")
        .run()?;
    Ok(())
}

/// The embedding model the demo's palette finds actions by meaning with, as claudius keeps it (`<name>.clde`), in
/// `out`: downloaded once beside the demo's packs, from the revision claudius was checked with, each file checked
/// against its hash.
fn web_model(sh: &Shell, out: &Path) -> Result<()> {
    use sha2::{Digest, Sha256};
    let source = claudius_intent::model::POTION;
    let downloads = root().join("target/demo-sounds/models").join(source.name);
    sh.create_dir(&downloads)?;
    for (file, size, sha) in source.files {
        let dest = downloads.join(file);
        let checked = |path: &Path| {
            std::fs::read(path).is_ok_and(|bytes| {
                bytes.len() as u64 == size
                    && Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>() == sha
            })
        };
        if !checked(&dest) {
            let url = source.url(file);
            cmd!(sh, "curl -fsSL --retry 3 -o {dest} {url}").run()?;
            ensure!(checked(&dest), "{file} from {url} is not the file claudius was checked with");
        }
    }
    let tensors = std::fs::read(downloads.join("model.safetensors"))?;
    let model = claudius_intent::Model::from_model2vec(&tensors, &sh.read_file(downloads.join("vocab.txt"))?)
        .map_err(|why| anyhow!("{}: {why}", source.name))?;
    sh.create_dir(out)?;
    sh.write_file(out.join(format!("{}.clde", source.name)), model.to_bytes())?;
    let licence = format!("LICENSE-{}", source.name);
    sh.copy_file(root().join("crates/claudius-intent").join(&licence), out.join(format!("{licence}.txt")))?;
    Ok(())
}

fn placeholder(version: &str) -> String {
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>claudius {version}</title>\n<h1>claudius {version}</h1>\n\
         <ul>\n<li><a href=\"docs/\">Documentation</a></li>\n<li><a href=\"studio/\">Studio</a></li>\n\
         <li><a href=\"{REPO_URL}\">Source</a></li>\n</ul>\n"
    )
}
