//! What each genre's generator writes for a fixed seed, as a snapshot. A change to theory, genre profiles or the
//! composer shows up here as a readable diff in review ("the house hats moved", "trap's progression changed"), and
//! `cargo insta review` accepts it once it is intended.
//!
//! The snapshot is a summary (form, harmony, drum steps, parts, mix levels rounded), so that floating-point noise
//! across platforms cannot fail it. Sample packs and the library are left out: the snapshots describe the
//! generators, not what is installed on the machine running the tests.

use std::fmt::Write as _;
use std::sync::Once;

use claudius::compose::{self, Options};
use claudius::genres;
use claudius::model::{Blueprint, Sound};

fn without_installed_packs() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let empty = std::env::temp_dir().join(format!("claudius-no-packs-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        // SAFETY: set once, before any thread of this test binary reads the environment
        unsafe { std::env::set_var("CLAUDIUS_PACKS", &empty) };
        claudius_packs::refresh();
    });
}

fn sound(s: &Sound) -> String {
    match s {
        Sound::Drum { voice, .. } => format!("drum {}", voice.name()),
        Sound::Synth { preset, .. } => format!("synth {preset}"),
        other => other.describe(),
    }
}

fn summary(bp: &Blueprint) -> String {
    let mut out = String::new();
    writeln!(out, "{} at {:.1} BPM in {}, swing {:.2}", bp.genre, bp.bpm, bp.key.name(), bp.swing).unwrap();
    for (k, v) in &bp.harmony.progressions {
        writeln!(out, "prog {k}: {}", v.join(" ")).unwrap();
    }
    writeln!(out, "bars per chord: {}", bp.harmony.bars_per_chord).unwrap();
    for s in &bp.form {
        let flags: Vec<&str> =
            [(s.build, "build"), (s.gap, "gap"), (s.impact, "impact"), (s.fills, "fills"), (s.filter_in, "filter_in")]
                .into_iter()
                .filter_map(|(on, f)| on.then_some(f))
                .collect();
        writeln!(
            out,
            "section {} ({}) {} bars, energy {:.2}, prog {}, {} [{}]: {}",
            s.name,
            s.kind.name(),
            s.bars,
            s.energy,
            s.prog,
            s.variant,
            flags.join(" "),
            s.layers.join(" ")
        )
        .unwrap();
    }
    for d in &bp.drums {
        for (v, hits) in &d.patterns {
            let (steps, _) = claudius::script::hits_to_steps(hits, d.pattern_bars);
            writeln!(out, "drum {} {v} ({}): {steps}", d.name, sound(&d.sound)).unwrap();
        }
    }
    for p in &bp.parts {
        writeln!(
            out,
            "part {} ({}): {} {}, register {}..{}, density {:.2}",
            p.name,
            p.role.name(),
            p.style,
            sound(&p.sound),
            p.register.0,
            p.register.1,
            p.density
        )
        .unwrap();
    }
    for (name, m) in bp.drums.iter().map(|d| (&d.name, &d.mix)).chain(bp.parts.iter().map(|p| (&p.name, &p.mix))) {
        writeln!(
            out,
            "mix {name}: {:+.1} dB, pan {:+.2}, reverb {:.2}, delay {:.2}, duck {:.2}, bus {}",
            m.level_db, m.pan, m.reverb, m.delay, m.duck, m.bus
        )
        .unwrap();
    }
    out
}

#[test]
fn every_genre_writes_what_it_wrote_before() {
    without_installed_packs();
    for g in genres::all().unwrap() {
        let bp = compose::generate(&g, 42, &Options::default(), None);
        insta::assert_snapshot!(g.name.clone(), summary(&bp));
    }
}

#[test]
fn the_same_seed_gives_the_same_song() {
    without_installed_packs();
    for g in genres::all().unwrap() {
        let a = compose::generate(&g, 7, &Options::default(), None);
        let b = compose::generate(&g, 7, &Options::default(), None);
        assert_eq!(a, b, "{}", g.name);
        let p1 = compose::arrange(&a);
        let p2 = compose::arrange(&b);
        assert_eq!(p1, p2, "{}", g.name);
    }
}
