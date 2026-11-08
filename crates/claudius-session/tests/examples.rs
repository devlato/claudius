//! Every example song makes its song from its script alone, without the sample packs, as the studio, the assistants
//! and the online demo open it: in its genre, under its title, two and a half to five minutes long, in whole phrases.
//! The packs are switched off for the whole process, which is why this is a test binary of its own.

use claudius::{compose, genres, script};
use claudius_api::examples;

#[test]
fn every_example_makes_its_song_without_the_packs() {
    claudius_packs::set_disabled(true);
    let mut failures = Vec::new();
    for e in examples::ALL {
        let made = script::run(e.script, None, None).map_err(|err| err.to_string()).and_then(|out| {
            let bp = out.bp.ok_or("no song")?;
            let p = compose::arrange(&bp);
            let secs = p.secs();
            let checks = [
                (genres::get(e.genre).is_ok(), format!("there is no genre {}", e.genre)),
                (bp.genre == e.genre, format!("the genre is {}, not {}", bp.genre, e.genre)),
                (bp.title == e.title, format!("the title is {:?}, not {:?}", bp.title, e.title)),
                (!p.tracks.is_empty(), "nothing plays".to_string()),
                ((150.0..=300.0).contains(&secs), format!("{secs:.0} s long")),
                (p.sections.iter().all(|s| s.bars % 4 == 0), "a section is not whole 4-bar phrases".into()),
            ];
            match checks.into_iter().find(|(ok, _)| !ok) {
                Some((_, why)) => Err(why),
                None => Ok(()),
            }
        });
        if let Err(why) = made {
            failures.push(format!("{}: {why}", e.name));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_docs_list_every_example() {
    let page = include_str!("../../../docs/src/cookbook/examples.md");
    for e in examples::ALL {
        assert!(
            page.contains(&format!("{} (`{}`)", e.title, e.name)),
            "docs/src/cookbook/examples.md lacks {}",
            e.name
        );
        assert!(page.contains(e.like), "docs/src/cookbook/examples.md does not say what {} is like", e.name);
    }
}
