//! `cargo xtask site`: the website in one directory, as Cloudflare serves it (site/wrangler.toml).

use std::path::Path;

use anyhow::{Context, Result, anyhow, ensure};
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

    // the pages a search engine should know of, for the robots.txt that names it
    let pages = pages(out)?;
    sh.write_file(out.join("sitemap.xml"), sitemap(site_url, &pages))?;

    eprintln!("site: {} (claudius {version} at {site_url}, {} pages in its sitemap)", out.display(), pages.len());
    Ok(())
}

/// The site's pages for its sitemap, each as the path its address ends in (`/`, `/radio/`, `/docs/guide/cli.html`),
/// sorted: every HTML page but those that are not pages to be found by searching (a page not found, the book printed
/// whole or its table of contents, the studio app, and the page a phone plays along on).
fn pages(out: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, out: &Path, found: &mut Vec<String>) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, out, found)?;
            } else if path.extension().is_some_and(|e| e == "html") {
                let rel = path.strip_prefix(out)?.to_string_lossy().replace('\\', "/");
                found.push(rel);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    walk(out, out, &mut found)?;
    let left_out = |rel: &str| {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        matches!(name, "404.html" | "print.html" | "toc.html")
            || rel.starts_with("studio/")
            || rel.starts_with("phone/")
    };
    let mut pages: Vec<String> = found
        .into_iter()
        .filter(|rel| !left_out(rel))
        .map(|rel| match rel.strip_suffix("index.html") {
            Some(dir) => format!("/{dir}"),
            None => format!("/{rel}"),
        })
        .collect();
    pages.sort();
    Ok(pages)
}

/// The sitemap of `pages` (as `pages` gives them) at `site_url`, as sitemaps.org has it
fn sitemap(site_url: &str, pages: &[String]) -> String {
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for page in pages {
        xml.push_str(&format!("  <url><loc>{}{}</loc></url>\n", escape(site_url), escape(page)));
    }
    xml.push_str("</urlset>\n");
    xml
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
    "tr909:bd",
    "tr909:sd",
    "tr909:lt",
    "tr909:mt",
    "tr909:ht",
    "tr909:hh",
    "tr909:oh",
    "tr909:cr",
    "tr909:rd",
    "tr909:cp",
    "tr909:rim",
    "uzu:bd",
    "uzu:sd",
    "uzu:hh",
    "uzu:oh",
    "uzu:cp",
    "uzu:rim",
    "uzu:lt",
    "uzu:mt",
    "uzu:ht",
    "uzu:cr",
    "uzu:rd",
    "uzu:cb",
    "uzu:sh",
    "uzu:tb",
    "uzu:misc",
    "synthdrums:kit",
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
    for &(file, size, sha) in source.files {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every drum kit a genre plays is in the web demo, so that the demo's songs play the drums the studio's play (and
    /// the genre benchmark measured) rather than stand-ins synthesized where a kit is missing.
    #[test]
    fn the_demo_has_every_drum_kit_a_genre_plays() {
        let web: std::collections::BTreeSet<&str> =
            WEB_INSTRUMENTS.iter().filter_map(|p| p.split(':').next()).collect();
        let mut missing = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(root().join("crates/claudius/data/genres")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let genre: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            for kit in genre["kits"].as_array().into_iter().flatten() {
                let pack = kit.get(0).and_then(serde_json::Value::as_str).unwrap_or_default();
                if !web.contains(pack) {
                    missing.insert(format!("{pack} ({})", path.file_stem().unwrap_or_default().to_string_lossy()));
                }
            }
        }
        assert!(missing.is_empty(), "drum kits genres play that the web demo lacks: {missing:?}");
    }

    /// The sitemap names the landing page, the radio and the book's pages, each at the site's address, and leaves out
    /// what is not a page to be found by searching.
    #[test]
    fn the_sitemap_names_the_pages_to_be_found_and_no_others() {
        let out = std::env::temp_dir().join(format!("claudius-sitemap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        for page in [
            "index.html",
            "404.html",
            "radio/index.html",
            "docs/index.html",
            "docs/guide/cli.html",
            "docs/print.html",
            "docs/toc.html",
            "docs/404.html",
            "studio/index.html",
            "phone/index.html",
        ] {
            let path = out.join(page);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "<!doctype html>").unwrap();
        }
        std::fs::write(out.join("docs/guide/notes.txt"), "not a page").unwrap();
        let pages = pages(&out).unwrap();
        assert_eq!(pages, ["/", "/docs/", "/docs/guide/cli.html", "/radio/"]);
        let xml = sitemap("https://claudius.devlato.com", &pages);
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns="), "{xml}");
        assert!(xml.contains("<url><loc>https://claudius.devlato.com/</loc></url>"), "{xml}");
        assert!(xml.contains("<url><loc>https://claudius.devlato.com/docs/guide/cli.html</loc></url>"), "{xml}");
        assert_eq!(xml.matches("<url>").count(), 4, "{xml}");
        let _ = std::fs::remove_dir_all(&out);
    }
}
