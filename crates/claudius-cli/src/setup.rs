//! `claudius setup`: the downloads that do not ship with the binary, the default sample packs and earshot's models.

use std::io::{IsTerminal, Write};

use anyhow::Result;

fn confirm(question: &str) -> Result<bool> {
    eprint!("{question} [Y/n] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(!matches!(answer.trim().to_lowercase().as_str(), "n" | "no"))
}

/// a progress line that rewrites itself on a terminal and prints every 10 % elsewhere
fn progress(label: &str, fraction: f32, last: &mut i32) {
    let pct = (fraction.clamp(0.0, 1.0) * 100.0) as i32;
    if std::io::stderr().is_terminal() {
        eprint!("\r  {label}: {pct:3}%   ");
        if pct >= 100 {
            eprintln!();
        }
    } else if pct / 10 != *last / 10 || pct == 100 && *last != 100 {
        eprintln!("  {label}: {pct}%");
    }
    *last = pct;
}

pub fn run(yes: bool, packs: bool, models: bool, tags: bool) -> Result<()> {
    let ask = !yes && std::io::stdin().is_terminal();
    if packs {
        let missing: Vec<_> = claudius_packs::packs().into_iter().filter(|p| p.default && !p.installed).collect();
        if missing.is_empty() {
            eprintln!("sample packs: the default packs are installed in {}", claudius_packs::packs_dir().display());
        } else {
            let gb = missing.iter().map(|p| p.size_mb as f64).sum::<f64>() / 1000.0;
            let names = missing.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ");
            if !ask || confirm(&format!("Download {} sample packs ({names}), about {gb:.1} GB?", missing.len()))? {
                let last = std::sync::Mutex::new(-1);
                claudius_packs::install_defaults(&|f, what| progress(what, f, &mut last.lock().unwrap()))?;
                eprintln!("sample packs: installed in {}", claudius_packs::packs_dir().display());
            }
        }
    }
    if models {
        download_models(ask, tags)?;
    }
    eprintln!("\nNext: `claudius studio` opens the studio, `claudius connect claude-desktop` adds it to Claude.");
    Ok(())
}

#[cfg(feature = "ears")]
fn download_models(ask: bool, tags: bool) -> Result<()> {
    let cache = earshot::models::default_cache_dir();
    if earshot::models::cached(&cache, tags) {
        eprintln!("ears: the models are in {}", cache.display());
    } else {
        let size = if tags { "about 1.1 GB" } else { "about 0.8 GB" };
        if !ask || confirm(&format!("Download earshot's listening models ({size}) into {}?", cache.display()))? {
            let last = std::sync::Mutex::new((String::new(), -1));
            earshot::models::set_progress(Box::new(move |label, done, total| {
                let mut l = last.lock().unwrap();
                if l.0 != label {
                    *l = (label.to_string(), -1);
                }
                progress(label, if total > 0 { done as f32 / total as f32 } else { 0.0 }, &mut l.1);
            }));
            eprintln!("ears: downloading, then reading the vocabulary once (about 30 s)");
            claudius::ears::Ears::load(tags)?;
            eprintln!("ears: ready");
        }
    }
    Ok(())
}

#[cfg(not(feature = "ears"))]
fn download_models(_ask: bool, _tags: bool) -> Result<()> {
    eprintln!("ears: this build of claudius has none, so there are no models to download");
    Ok(())
}
