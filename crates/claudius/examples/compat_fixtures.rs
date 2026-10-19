//! Writes the songs a release keeps in tests/compat, which every later claudius has to open:
//!
//!   cargo run -p claudius --no-default-features --example compat_fixtures -- crates/claudius/tests/compat/v<version>
//!
//! A song in each built-in genre, and one edited by a script, as blueprint.json has them (`<name>.json`), with the
//! script each prints as song.rhai (`<name>.rhai`). `cargo xtask release prepare` runs it for the version it releases.

use std::path::Path;

use anyhow::{Context, Result};
use claudius::compose::{self, Options};
use claudius::model::Blueprint;
use claudius::{genres, script};

/// edits that reach most of the blueprint: tempo, key, swing, a progression, a part, the mix, a drum lane, pattern
/// tracks and a section
const EDITS: &str = r#"
tempo(122);
key("A minor");
swing(0.56);
prog("A", "i7 VImaj7 iv7 v7");
part("chords", #{ preset: "epiano" });
mix("lead", #{ level: -2, reverb: 0.35 });
drum("kick", "main", "X...X...X...X...");
track("rim", s("~ rim ~ rim").gain(0.6).pan(0.2));
track("bass2", note("<a1 f1 d1 e1>").struct("x ~ x ~ x ~ x ~").s("sub_bass").legato(0.8));
track("keys2", chord("prog").voicing().struct("x ~ ~ x ~ ~ x ~").s("epiano").room(0.25));
section("drop", #{ bars: 16 });
"#;

fn write(dir: &Path, name: &str, bp: &Blueprint) -> Result<()> {
    std::fs::write(dir.join(format!("{name}.json")), bp.to_json())?;
    std::fs::write(dir.join(format!("{name}.rhai")), script::to_script(bp))?;
    Ok(())
}

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).context("usage: compat_fixtures <folder>")?;
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    let opts = Options { short: true, ..Default::default() };
    for name in genres::builtin_names() {
        let bp = compose::generate(&genres::get(name)?, 7, &opts, None);
        write(dir, name, &bp)?;
        if name == "house" {
            let edited = script::run(EDITS, Some(bp), None).map_err(anyhow::Error::msg)?;
            write(dir, "edits", &edited.bp.context("the edits left no song")?)?;
        }
    }
    eprintln!("compat_fixtures: wrote the songs of this claudius into {}", dir.display());
    Ok(())
}
