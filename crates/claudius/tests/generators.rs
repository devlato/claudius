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
use claudius::model::{Blueprint, Sound};
use claudius::{genres, palette};

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
fn fusions_write_what_they_wrote_before() {
    without_installed_packs();
    for name in ["trip_hop+idm", "nu_disco+filter_house"] {
        let g = genres::get(name).unwrap();
        let bp = compose::generate(&g, 7, &Options::default(), None);
        assert_eq!(bp.genre, name);
        insta::assert_snapshot!(name.replace('+', "_with_"), summary(&bp));
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

#[test]
fn a_synth_bass_with_nothing_under_its_note_plays_a_sub_in_the_sub_octave() {
    use claudius::model::{Layer, Role, SUB_OCTAVE};
    let sine = |l: &Layer| matches!(&l.sound, Sound::Synth { preset, .. } if preset == "sub_bass" || preset == "808");
    let mut subs = 0;
    for genre in ["house", "deep_house", "dance_pop", "techno", "trance", "electro_house", "afro_house"] {
        let g = genres::get(genre).unwrap();
        for seed in 0..16 {
            let bp = compose::generate(&g, seed, &Options::default(), None);
            let bass =
                bp.parts.iter().find(|p| p.role == Role::Bass && g.parts.iter().any(|s| s.name == p.name)).unwrap();
            match &bass.sound {
                // those with nothing under their note always get one; the rest have their own
                Sound::Synth { preset, .. } => assert!(
                    !["organ_bass", "pluck_bass", "dist_bass", "wobble", "sub_bass"].contains(&preset.as_str()),
                    "{genre} {seed}: {preset} with no sub"
                ),
                Sound::Rack { layers } if layers.iter().any(compose::is_sub) => {
                    let sub = layers.iter().find(|l| compose::is_sub(l)).unwrap();
                    assert!(sine(sub) && sub.fold == Some(SUB_OCTAVE), "{genre} {seed}: {}", bass.sound.describe());
                    if let [under, over] = layers.as_slice() {
                        assert_eq!((under.band, over.band), (Some([0.0, 90.0]), Some([90.0, 0.0])), "{genre} {seed}");
                    }
                    subs += 1;
                }
                _ => {}
            }
        }
    }
    assert!(subs >= 20, "{subs}");
    // an 808 played in the sub octave already, and a recorded bass, stay as they are
    let trap = genres::get("trap").unwrap();
    let bp = compose::generate(&trap, 1, &Options::default(), None);
    let bass = bp.parts.iter().find(|p| p.role == Role::Bass).unwrap();
    let Sound::Rack { layers } = &bass.sound else { panic!("trap stacks its 808") };
    assert!(layers.iter().all(|l| !compose::is_sub(l)), "{}", bass.sound.describe());
}

#[test]
fn every_part_plays_a_synth_its_genre_has_a_place_for() {
    without_installed_packs();
    for g in genres::all().unwrap() {
        // what a genre stacks under a part by name (a sub under its bass) is its own
        let stacked: Vec<&str> = g.parts.iter().flat_map(|s| &s.layers).map(|l| l.sound.as_str()).collect();
        // and so is the sub claudius stacks under a bass (`compose::is_sub`)
        fn heard(s: &Sound) -> Vec<&Sound> {
            match s {
                Sound::Rack { layers } => layers.iter().filter(|l| !compose::is_sub(l)).map(|l| &l.sound).collect(),
                s => vec![s],
            }
        }
        for seed in 0..12 {
            let bp = compose::generate(&g, seed, &Options::default(), None);
            for (p, one) in bp.parts.iter().flat_map(|p| heard(&p.sound).into_iter().map(move |s| (p, s))) {
                let Sound::Synth { preset, .. } = one else { continue };
                if matches!(p.sound, Sound::Rack { .. }) && stacked.contains(&preset.as_str()) {
                    continue;
                }
                let own = compose::fitting_synths(&g, p.role);
                let mut rp = g.palette.get(p.role.name()).cloned().unwrap_or_else(|| palette::default_for(p.role));
                if rp.synths.is_empty() {
                    rp.synths = own.clone();
                }
                let drawn = palette::synths_drawn(p.role, &rp);
                let name = preset.as_str();
                assert!(
                    own.contains(preset) || drawn.contains(&name),
                    "{} seed {seed}: its {} plays {preset}, none of {own:?} {drawn:?}",
                    g.name,
                    p.name
                );
                // a bell, a whistle and a chip tune's square ring out over a mix: only where the genre names one
                if ["bell", "whistle", "chip"].contains(&name) {
                    assert!(own.contains(preset) || rp.synths.contains(preset), "{} seed {seed}: {preset}", g.name);
                }
            }
        }
    }
    // trip-hop's lead sings softly, or rings as a vibraphone or a Rhodes does: never a bell, a whistle or a chip
    let g = genres::get("trip_hop").unwrap();
    for seed in 0..24 {
        let bp = compose::generate(&g, seed, &Options::default(), None);
        if let Some(Sound::Synth { preset, .. }) = bp.parts.iter().find(|p| p.name == "lead").map(|p| &p.sound) {
            assert!(!["bell", "whistle", "chip"].contains(&preset.as_str()), "seed {seed}: {preset}");
        }
    }
}

#[test]
fn songs_of_one_genre_draw_many_sounds_and_no_song_plays_one_twice() {
    use std::collections::{BTreeMap, BTreeSet};
    without_installed_packs();
    for name in ["house", "lofi", "techno", "trap"] {
        let g = genres::get(name).unwrap();
        // the sounds each role played, and in how many songs: enough songs that what a role plays shows how many
        // sounds its genre has for it, more than the luck of the draw
        let mut by_role: BTreeMap<&str, (BTreeSet<String>, usize)> = BTreeMap::new();
        for seed in 0..48 {
            let bp = compose::generate(&g, seed, &Options::default(), None);
            for p in &bp.parts {
                let e = by_role.entry(p.role.name()).or_default();
                e.0.insert(p.sound.describe());
                e.1 += 1;
            }
            let ids: Vec<String> = bp.parts.iter().flat_map(|p| claudius::palette::identities(&p.sound)).collect();
            assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len(), "{name} seed {seed}: {ids:?}");
        }
        for (role, (sounds, songs)) in &by_role {
            // trap's bass is its 808
            if *songs >= 10 && !(name == "trap" && *role == "bass") {
                assert!(sounds.len() >= 4, "{name} {role} in {songs} songs: {sounds:?}");
            }
        }
        let most = by_role.values().map(|(s, _)| s.len()).max().unwrap_or(0);
        assert!(most >= 8, "{name}: {by_role:?}");
    }
}

#[test]
fn a_genre_that_stacks_its_bass_plays_it_as_a_rack_of_its_layers_and_doubles_it_as_one_sound() {
    use claudius::model::Role;
    use std::collections::BTreeSet;
    without_installed_packs();
    let mut doubled = 0;
    for name in ["trap", "dnb", "future_bass"] {
        let g = genres::get(name).unwrap();
        let spec = g.parts.iter().find(|p| p.role == Role::Bass).unwrap();
        assert!(!spec.layers.is_empty(), "{name} stacks its bass");
        for seed in 0..8 {
            let bp = compose::generate(&g, seed, &Options { complexity: Some(1.0), ..Default::default() }, None);
            let bass = bp.parts.iter().find(|p| p.name == spec.name).unwrap();
            let Sound::Rack { layers } = &bass.sound else { panic!("{name} {seed}: {}", bass.sound.describe()) };
            assert_eq!(layers.len(), spec.layers.len(), "{name} {seed}");
            for (s, l) in spec.layers.iter().zip(layers) {
                assert_eq!(
                    (l.band, l.level_db, l.duck, l.transpose, &l.fx),
                    (s.band, s.level, s.duck, s.transpose, &s.fx)
                );
                if s.sound != "$" {
                    assert_eq!(palette::identity(&l.sound), Some(format!("synth:{}", s.sound)), "{name} {seed}");
                }
            }
            // "$": the one sound it drew, in each layer that plays it
            let drawn: Vec<&Sound> =
                spec.layers.iter().zip(layers).filter(|(s, _)| s.sound == "$").map(|(_, l)| &l.sound).collect();
            assert!(drawn.windows(2).all(|w| w[0] == w[1]), "{name} {seed}");
            for d in bp.parts.iter().filter(|p| p.doubles.as_deref() == Some(bass.name.as_str())) {
                assert!(!matches!(d.sound, Sound::Rack { .. }), "{name} {seed}: {} stacks a second sub", d.name);
                doubled += 1;
            }
            // nor does another part play a sound the rack plays (its double may, where the genre has no other)
            let rack: BTreeSet<String> = palette::identities(&bass.sound).into_iter().collect();
            for p in bp.parts.iter().filter(|p| p.name != bass.name && p.doubles.as_deref() != Some(&bass.name)) {
                let shared: Vec<String> =
                    palette::identities(&p.sound).into_iter().filter(|i| rack.contains(i)).collect();
                assert!(shared.is_empty(), "{name} {seed}: {} plays {shared:?}, as the bass does", p.name);
            }
        }
    }
    assert!(doubled > 0, "of 24 songs, none doubled its bass");
}

#[test]
fn a_stacked_bass_has_its_place_by_each_sound_in_it() {
    use claudius::model::Role;
    without_installed_packs();
    let synth = |name: &str| Sound::Synth { preset: name.into(), patch: claudius::synth::presets::get(name).unwrap() };
    for name in ["trap", "future_bass"] {
        let g = genres::get(name).unwrap();
        let bass = compose::generate(&g, 1, &Options::default(), None).parts.into_iter().find(|p| p.name == "bass");
        let sound = bass.unwrap().sound;
        assert!(matches!(sound, Sound::Rack { .. }) && compose::has_place(&g, Role::Bass, &sound), "{name}");
    }
    // trap's 808 swapped for a reese, further afield, is the sound out of place, not the rack around it
    let g = genres::get("trap").unwrap();
    let bp = compose::generate(&g, 1, &Options::default(), None);
    let mut sound = bp.parts.into_iter().find(|p| p.name == "bass").unwrap().sound;
    sound.each_mut().into_iter().for_each(|s| *s = synth("reese"));
    let out = compose::out_of_place(&g, Role::Bass, &sound).map(Sound::describe);
    assert_eq!(out.as_deref(), Some("synth patch 'reese'"));
}

#[test]
fn a_second_bass_of_a_genre_that_stacks_stacks_no_sound_another_part_plays() {
    without_installed_packs();
    let g = genres::get("future_bass").unwrap();
    for seed in 0..4 {
        let mut bp = compose::generate(&g, seed, &Options::default(), None);
        claudius::generate::track(&mut bp, "bass", "future_bass", None, Some("bass2"), 7).unwrap();
        let ids = |name: &str| palette::identities(&bp.parts.iter().find(|p| p.name == name).unwrap().sound);
        let (first, second) = (ids("bass"), ids("bass2"));
        assert!(first.contains(&"synth:sub_bass".to_string()), "{seed}: {first:?}");
        assert!(second.iter().all(|i| !first.contains(i)), "{seed}: {first:?} and {second:?}");
    }
}

#[test]
fn some_songs_answer_their_loop_and_some_choruses_play_their_own_progression() {
    use claudius::model::SectionKind;
    without_installed_packs();
    for name in ["dance_pop", "lofi", "trap", "synthwave"] {
        let g = genres::get(name).unwrap();
        let (mut answered, mut choruses) = (0, 0);
        for seed in 0..40 {
            let bp = compose::generate(&g, seed, &Options::default(), None);
            let progs = &bp.harmony.progressions;
            let a = &progs["A"];
            if a.len() == 8 && a[..4] != a[4..] {
                answered += 1;
                assert!(
                    bp.harmony.bars_per_chord <= 2,
                    "{name} {seed}: eight chords at {} bars each",
                    bp.harmony.bars_per_chord
                );
            }
            if let Some(c) = progs.get("C") {
                choruses += 1;
                assert_ne!(c, a, "{name} {seed}");
                for s in &bp.form {
                    match s.kind {
                        SectionKind::Hook => assert_eq!(s.prog, "C", "{name} {seed}: {}", s.name),
                        SectionKind::Verse => assert_eq!(s.prog, "A", "{name} {seed}: {}", s.name),
                        _ => {}
                    }
                }
            }
        }
        assert!(answered >= 4, "{name}: {answered} of 40 songs answer their loop");
        assert!(choruses >= 6, "{name}: {choruses} of 40 songs have a chorus of its own");
    }
}

#[test]
fn eight_chords_loop_in_sixteen_bars_and_no_chorus_plays_what_the_verses_play() {
    without_installed_packs();
    for g in genres::all().unwrap() {
        for seed in 0..40 {
            let bp = compose::generate(&g, seed, &Options::default(), None);
            let (progs, bpc) = (&bp.harmony.progressions, bp.harmony.bars_per_chord);
            for (name, p) in progs {
                assert!(p.len() < 8 || bpc <= 2, "{} {seed}: {name} has {} chords at {bpc} bars", g.name, p.len());
            }
            if let Some(c) = progs.get("C") {
                for (name, p) in progs.iter().filter(|(n, _)| *n != "C") {
                    let half = &p[..p.len() / 2];
                    assert!(
                        c != p && (p.len() != 8 || c.as_slice() != half),
                        "{} {seed}: C {c:?} is {name}'s {p:?}",
                        g.name
                    );
                }
            }
        }
    }
}
