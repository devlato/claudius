//! `cargo xtask site`: the website in one directory, as Cloudflare Pages serves it.

use std::path::Path;

use anyhow::Result;
use xshell::Shell;

use crate::{copy_tree, fresh_dir, root};

pub const DEFAULT_URL: &str = "https://claudius.pages.dev";
const REPO_URL: &str = "https://github.com/devlato/claudius";

/// the entries of site/ that are tooling (the smoke test, its packages, the scripts that make the demos), not pages;
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
];

/// Cloudflare Pages serves unknown extensions as application/octet-stream, which PowerShell's `irm | iex` cannot run
const HEADERS: &str = "\
/install.sh
  Content-Type: text/plain; charset=utf-8
/install.ps1
  Content-Type: text/plain; charset=utf-8
";

pub fn build(sh: &Shell, out: &Path, site_url: &str) -> Result<()> {
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

fn placeholder(version: &str) -> String {
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>claudius {version}</title>\n<h1>claudius {version}</h1>\n\
         <ul>\n<li><a href=\"docs/\">Documentation</a></li>\n<li><a href=\"studio/\">Studio</a></li>\n\
         <li><a href=\"{REPO_URL}\">Source</a></li>\n</ul>\n"
    )
}
