use claudius::compose::{self, Options};
use claudius::{genres, render, script};

#[test]
fn patterns_from_code() {
    let p = script::eval_pattern(r#"s("bd*4, [~ sd]*2, hh*8")"#).unwrap();
    assert_eq!(p.events(0.0, 1.0).len(), 14);
    let p = script::eval_pattern(r#"s("bd(3,8)")"#).unwrap();
    let on: Vec<f64> = p.events(0.0, 1.0).iter().map(|e| e.b).collect();
    assert_eq!(on, vec![0.0, 0.375, 0.75]);
}

#[test]
fn closures_keep_their_source() {
    let code = r#"s("bd sd").every(2, |x| x.fast(2)).gain(0.8)"#;
    let p = script::eval_pattern(code).unwrap();
    assert_eq!(p.repr, code);
    assert_eq!(p.events(0.0, 1.0).len(), 4);
    assert_eq!(p.events(1.0, 2.0).len(), 2);
    let code = r#"n("0 2 4 6").scale("A:minor").off(0.125, |x| x.add(7)).sometimes(|x| x.ply(2))"#;
    assert_eq!(script::eval_pattern(code).unwrap().repr, code);
}

#[test]
fn songs_print_as_scripts_that_change_nothing() {
    for (genre, seed) in [("house", 3), ("trap", 5), ("techno", 11), ("lofi", 2), ("dnb", 4)] {
        let g = genres::get(genre).unwrap();
        let bp = compose::generate(&g, seed, &Options::default(), None);
        let code = script::to_script(&bp);
        let got =
            script::run(&code, Some(bp.clone()), None).unwrap_or_else(|e| panic!("{genre}: {e}\n{code}")).bp.unwrap();
        if got != bp {
            let (a, b) = (bp.to_json(), got.to_json());
            let line = a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(0);
            let ctx = |s: &str| s.lines().skip(line.saturating_sub(6)).take(10).collect::<Vec<_>>().join("\n");
            panic!(
                "{genre}: the script changed the song near line {line}\nbefore:\n{}\nafter:\n{}\n\n{code}",
                ctx(&a),
                ctx(&b)
            );
        }
    }
}

#[test]
fn layered_and_sparse_songs_print_as_scripts_that_change_nothing() {
    for (genre, seed, complexity) in
        [("neo_soul", 2, 1.0), ("house", 3, 0.9), ("boom_bap", 1, 1.0), ("techno", 11, 0.0)]
    {
        let g = genres::get(genre).unwrap();
        let bp = compose::generate(&g, seed, &Options { complexity: Some(complexity), ..Default::default() }, None);
        let code = script::to_script(&bp);
        let got =
            script::run(&code, Some(bp.clone()), None).unwrap_or_else(|e| panic!("{genre}: {e}\n{code}")).bp.unwrap();
        assert!(got == bp, "{genre} at {complexity}: the script changed the song\n{code}");
    }
}

#[test]
fn a_part_doubles_another_and_says_why_it_cannot() {
    let run = |code: &str| script::run(code, None, None);
    let made = r#"song("house", 3); part("chords_double", #{ role: "chords", doubles: "chords", octave: 1, preset: "bell" });"#;
    let bp = run(made).unwrap().bp.unwrap();
    let double = bp.parts.iter().find(|p| p.name == "chords_double").unwrap();
    assert_eq!((double.doubles.as_deref(), double.octave), (Some("chords"), 1));
    let again = script::run(&script::to_script(&bp), Some(bp.clone()), None).unwrap().bp.unwrap();
    assert!(again == bp, "printed and run again, the same");
    let start = r#"song("house", 3); part("x", #{ role: "keys", doubles: "chords" });"#;
    for (code, says) in [
        (r#"part("lead", #{ doubles: "nope" });"#, "part 'lead' doubles 'nope', which is not a part of the song"),
        (r#"part("lead", #{ doubles: "lead" });"#, "part 'lead' cannot double itself"),
        (r#"part("lead", #{ doubles: "chords", octave: 3 });"#, "an octave is a whole number from -2 to 2, not 3"),
        (
            r#"part("y", #{ role: "keys", doubles: "x" });"#,
            "part 'y' doubles 'x', which doubles another part: double that one",
        ),
        (r#"part("chords", #{ doubles: "lead" });"#, "part 'chords' is doubled by 'x', so it cannot double another"),
    ] {
        let e = run(&format!("{start} {code}")).err().unwrap_or_else(|| panic!("{code} ran")).to_string();
        assert!(e.contains(says), "{code}: {e}");
    }
    let own = run(&format!(r#"{start} part("x", #{{ doubles: "" }});"#)).unwrap().bp.unwrap();
    assert_eq!(own.parts.iter().find(|p| p.name == "x").unwrap().doubles, None, "doubles: \"\" plays its own again");
}

#[test]
fn the_mixers_drive_roughens_a_track_at_the_loudness_its_level_gives_it() {
    use claudius::model::Blueprint;
    let g = genres::get("trap").unwrap();
    let clean = compose::generate(&g, 1, &Options::default(), None);
    let driven = script::run(r#"mix("bass", #{ drive: 24 });"#, Some(clean.clone()), None).unwrap().bp.unwrap();
    let drive = |bp: &Blueprint| bp.parts.iter().find(|p| p.name == "bass").unwrap().mix.drive_db;
    assert_eq!((drive(&clean), drive(&driven)), (0.0, 24.0));
    let code = script::to_script(&driven);
    assert!(code.lines().any(|l| l.starts_with(r#"mix("bass", #{"#) && l.contains("drive: 24")), "{code}");
    assert_eq!(drive(&script::run(&code, Some(driven.clone()), None).unwrap().bp.unwrap()), 24.0);
    // two bars where the bass plays: its stem as loud either way, and driven, more of it where small speakers play
    let stem = |bp: &Blueprint| {
        let at = bp.form.iter().position(|s| s.layers.iter().any(|l| l == "bass")).unwrap();
        let p = compose::arrange(bp);
        let s = &p.sections[at];
        let r =
            render::render(&p.slice(s.start_bar, s.start_bar + 2), &render::Options { stems: true, tail_secs: 0.0 })
                .unwrap();
        r.stems.into_iter().find(|(n, _)| n == "bass").unwrap().1
    };
    let (a, b) = (stem(&clean), stem(&driven));
    let (la, lb) = (claudius::loudness::active_level(&a).unwrap(), claudius::loudness::active_level(&b).unwrap());
    assert!((la - lb).abs() < 0.5, "as loud: {la:.1} and {lb:.1} LUFS");
    let body = |x| claudius::judge::band_shares(x)[2];
    assert!(body(&b) > body(&a) + 3.0, "250 Hz-1 kHz: {:.1} dB driven, {:.1} dB clean", body(&b), body(&a));
}

#[test]
fn pattern_tracks_play_in_their_sections() {
    let code = r#"
        blank("house");
        tempo(120);
        track("drums", s("bd*4, [~ sd]*2, hh*8"));
        track("bass", note("<a1 f1 c2 g1>").struct("x ~ x x").s("808"));
        track("keys", chord("<Am7 Fmaj7>").voicing().s("keys").room(0.3));
    "#;
    let bp = script::run(code, None, None).unwrap().bp.unwrap();
    let p = compose::arrange(&bp);
    let count = |n: &str| p.tracks.iter().find(|t| t.name == n).map_or(0, |t| t.notes.len());
    assert_eq!(count("drums.bd"), 32);
    assert_eq!(count("drums.sd"), 16);
    assert_eq!(count("drums.hh"), 64);
    assert_eq!(count("bass"), 24);
    assert_eq!(count("keys"), 32);
    let keys = p.tracks.iter().find(|t| t.name == "keys").unwrap();
    assert!(
        keys.notes.iter().all(|n| (52..=74).contains(&n.p)),
        "{:?}",
        keys.notes.iter().map(|n| n.p).collect::<Vec<_>>()
    );
    assert_eq!(p.tracks.iter().find(|t| t.name == "drums.bd").unwrap().role, "kick");
    assert_eq!(p.tracks.iter().find(|t| t.name == "bass").unwrap().role, "bass");
    // and the song reads back as the same script
    let again = script::run(&script::to_script(&bp), Some(bp.clone()), None).unwrap().bp.unwrap();
    assert_eq!(again, bp);
    let r = render::render(&p.slice(0, 4), &render::Options::default()).unwrap();
    assert!(r.lufs > -20.0 && r.lufs < -5.0, "{}", r.lufs);
}

#[test]
fn edits_on_a_generated_song() {
    let g = genres::get("house").unwrap();
    let bp = compose::generate(&g, 9, &Options::default(), None);
    let code = r#"
        tempo(126); key("F minor");
        drum("kick", "x...x...x...x...");
        track("perc", s("~ rim ~ [rim rim]").gain(0.7).pan(-0.3));
        mute("hat");
        section("drop", #{ energy: 1.0 });
    "#;
    let out = script::run(code, Some(bp.clone()), None).unwrap().bp.unwrap();
    assert_eq!(out.bpm, 126.0);
    assert_eq!(out.key.name(), "F minor");
    let kick = out.drums.iter().find(|d| d.name == "kick").unwrap();
    assert_eq!(kick.patterns["main"].len() as u32, 4 * kick.pattern_bars);
    assert!(out.drums.iter().find(|d| d.name == "hat").unwrap().mix.mute);
    let p = compose::arrange(&out);
    assert!(p.tracks.iter().all(|t| t.name != "hat"));
    assert!(p.tracks.iter().any(|t| t.name == "perc" && !t.notes.is_empty()));
    assert!(out.form.iter().filter(|s| s.energy >= 0.6).all(|s| s.layers.contains(&"perc".to_string())));
}

#[test]
fn a_mode_change_moves_the_chords_with_it() {
    let out = script::run(
        r#"blank("house"); key("E major"); prog("A", "Imaj7 vi7 ii7 V7 bVII"); key("F minor");"#,
        None,
        None,
    )
    .unwrap()
    .bp
    .unwrap();
    // the borrowed bVII keeps its sound, a whole step below the tonic, which in minor is the diatonic VII
    assert_eq!(out.harmony.progressions["A"], ["i7", "VImaj7", "iiø7", "v7", "VII"]);
    // back in major every chord is diatonic in minor, so each takes major's chord on its degree: VII becomes vii°
    let back = script::run(r#"key("F major")"#, Some(out), None).unwrap().bp.unwrap();
    assert_eq!(back.harmony.progressions["A"], ["Imaj7", "vi7", "ii7", "V7", "vii°"]);
}

#[test]
fn the_reference_examples_run() {
    let g = genres::get("house").unwrap();
    let bp = compose::generate(&g, 9, &Options::default(), None);
    let mut blocks = 0;
    let packs = !claudius_packs::installed().is_empty();
    for block in script::REFERENCE.split("```js").skip(1) {
        let code = block.split("```").next().unwrap();
        if code.starts_with(" packs") && !packs {
            continue;
        }
        let code = code.trim_start_matches(" packs");
        let out = script::run(code, Some(bp.clone()), None).unwrap_or_else(|e| panic!("{e}\n{code}"));
        let p = compose::arrange(&out.bp.unwrap());
        assert!(p.tracks.iter().any(|t| !t.notes.is_empty()), "{code}");
        blocks += 1;
    }
    assert!(blocks >= 3);
}

#[test]
fn sampled_instruments_and_drum_machines() {
    if claudius_packs::installed().is_empty() {
        eprintln!("no sample packs installed: skipped");
        return;
    }
    use claudius::model::Sound;
    let bp = script::run(
        r#"blank("lofi"); track("d", s("bd sd, hh*4").bank("tr909")); track("k", note("c4 e4 g4").s("piano"));"#,
        None,
        None,
    )
    .unwrap()
    .bp
    .unwrap();
    let d = bp.patterns.iter().find(|p| p.name == "d").unwrap();
    assert!(
        d.sounds.values().all(|s| matches!(s, Sound::Instrument { name, .. } if name.starts_with("tr909:"))),
        "{:?}",
        d.sounds
    );
    let k = bp.patterns.iter().find(|p| p.name == "k").unwrap();
    assert!(
        matches!(k.sounds.values().next(), Some(Sound::Instrument { name, .. }) if name == "salamander:piano"),
        "{:?}",
        k.sounds
    );
    let r = render::render(&compose::arrange(&bp).slice(0, 2), &render::Options::default()).unwrap();
    assert!(r.lufs > -20.0, "{}", r.lufs);

    let g = genres::get("house").unwrap();
    let song = compose::generate(&g, 3, &Options::default(), None);
    let out = script::run(r#"part("chords", #{ preset: "piano" })"#, Some(song), None).unwrap().bp.unwrap();
    let chords = out.parts.iter().find(|p| p.name == "chords").unwrap();
    assert!(matches!(&chords.sound, Sound::Instrument { name, .. } if name == "salamander:piano"));
    let again = script::run(&script::to_script(&out), Some(out.clone()), None).unwrap().bp.unwrap();
    assert_eq!(again, out);
}

#[test]
fn scripts_cannot_run_away() {
    let err =
        |code: &str| script::run(code, None, None).err().unwrap_or_else(|| panic!("expected an error from {code}"));
    assert!(err(r#"track("x", s("bd*100000000"))"#).contains("up to 256"));
    assert!(err(r#"track("x", s("bd").fast(1000000000))"#).contains("up to 256"));
    // each factor within its limit, too dense together: stopped before it fills the memory
    assert!(err(r#"track("x", s("[bd*256]*256"))"#).contains("too dense"));
    assert!(err(r#"track("x", s("bd*256").fast(256).ply(256))"#).contains("too dense"));
    // silence makes no events but still costs work
    assert!(err(r#"track("x", s("[[~ ~]*256 ~]*256"))"#).contains("steps of work"));
    assert!(err(r#"track("x", s("bd").euclid(3, 1000000))"#).contains("1 to 256 steps"));
    assert!(err(r#"track("x", s("bd").ply(1.5))"#).contains("whole number"));
    assert!(err(&format!("track(\"x\", s(\"{}bd{}\"))", "[".repeat(80), "]".repeat(80))).contains("nested more than"));
    assert!(err(&format!("track(\"x\", s(\"{}\"))", "bd ".repeat(2000))).contains("limited to 4096"));
    assert!(err(r#"track("x", s("bd(3,100000)"))"#).contains("at most 256 steps"));
    assert!(err(r#"track("x", s("bd!100000"))"#).contains("at most 256 times"));
    assert!(err("loop {}").contains("operations") || err("loop {}").contains("seconds"));
    assert!(err(r#"import "other" as o;"#).to_lowercase().contains("module"));
    assert!(err(r#"eval("1 + 1")"#).contains("eval"));
    assert!(
        err(r#"blank("house"); form([sec("drop", 256), sec("drop", 256), sec("drop", 256)])"#).contains("limit is 512")
    );
    assert!(err(r#"blank("house"); tempo(40); form([sec("drop", 256), sec("drop", 200)])"#).contains("minutes"));
    // a render holds every track, so the number of tracks is limited, and more so in a long song
    assert!(err(r#"blank("house"); for i in 0..65 { track("t" + i, s("bd*4")); }"#).contains("limit is 64"));
    assert!(
        err(r#"blank("house"); tempo(60); form([sec("drop", 300)]); for i in 0..20 { track("t" + i, s("bd")); }"#)
            .contains("track-minutes")
    );
}

#[test]
fn a_form_too_long_to_count_is_still_too_long() {
    let mut bp =
        script::run(r#"blank("house"); form([sec("drop", 8), sec("drop", 8)])"#, None, None).unwrap().bp.unwrap();
    bp.form[0].bars = u32::MAX;
    bp.form[1].bars = 2;
    assert!(bp.check_size().unwrap_err().contains("limit is 512"));
}

#[test]
fn errors_say_what_to_do() {
    // a name no pack plays: `foo` is a bank of TidalCycles' Dirt-Samples, which a machine may have installed
    let e = script::run(r#"track("x", s("nosuchsound*4"))"#, None, None).err().unwrap();
    assert!(e.contains("unknown sound 'nosuchsound'") && e.contains("synths:"), "{e}");
    let e = script::run(r#"track("x", s("bd [sd"))"#, None, None).err().unwrap();
    assert!(e.contains(']'), "{e}");
    let e = script::run(r#"key("H major")"#, None, None).err().unwrap();
    assert!(e.contains("cannot read key"), "{e}");
    let e = script::run(r#"part("nope", #{ style: "x" })"#, None, None).err().unwrap();
    assert!(e.contains("no part 'nope'"), "{e}");
}

fn onsets(code: &str, b: f64, e: f64) -> Vec<f64> {
    script::eval_pattern(code).unwrap().events(b, e).iter().map(|ev| (ev.b * 1000.0).round() / 1000.0).collect()
}

fn placed(code: &str) -> Vec<(String, i32)> {
    use claudius::pattern::notes;
    let p = script::eval_pattern(code).unwrap();
    let progressions = Default::default();
    let ctx = notes::Context {
        key: claudius::theory::scale::Key::parse("C major").unwrap(),
        progressions: &progressions,
        bars_per_chord: 1,
        prog: "A",
    };
    notes::place(&p, 0, 2, 0.0, &ctx).unwrap().into_iter().map(|x| (x.sound, x.note.p)).collect()
}

#[test]
fn euclidean_rhythms_match_tidal() {
    assert_eq!(onsets(r#"s("bd(3,8)")"#, 0.0, 1.0), vec![0.0, 0.375, 0.75]);
    // x.xx.xx.
    assert_eq!(onsets(r#"s("bd(5,8)")"#, 0.0, 1.0), vec![0.0, 0.25, 0.375, 0.625, 0.75]);
    // rotated two steps left: xx.xx.x.
    assert_eq!(onsets(r#"s("bd(5,8,2)")"#, 0.0, 1.0), vec![0.0, 0.125, 0.375, 0.5, 0.75]);
}

#[test]
fn structure_finds_values_that_started_earlier() {
    // one note over two bars: the second bar's beats still find it
    assert_eq!(onsets(r#"note("c3").slow(2).struct("x x x x")"#, 1.0, 2.0), vec![1.0, 1.25, 1.5, 1.75]);
    assert_eq!(onsets(r#"s("hh*4").mask("<1 0>/2")"#, 1.0, 2.0).len(), 4);
}

#[test]
fn sometimes_decides_once_per_event() {
    // every hat plays exactly once: either as it was or nudged late, never both or neither
    let code = r#"s("hh*16").sometimes(|x| x.late(0.01))"#;
    for c in 0..8 {
        let on = onsets(code, c as f64, c as f64 + 1.0);
        assert_eq!(on.len(), 16, "cycle {c}: {on:?}");
        let moved = on.iter().filter(|t| ((*t * 16.0) - (*t * 16.0).round()).abs() > 1e-6).count();
        assert!(moved > 0 && moved < 16, "cycle {c}: {moved} moved");
    }
}

#[test]
fn notes_read_names_numbers_and_transpositions() {
    assert_eq!(
        placed(r#"s("tr808:sd bd:3 tr808:bd:2")"#).iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        ["tr808:sd", "bd:3", "tr808:bd:2", "tr808:sd", "bd:3", "tr808:bd:2"]
    );
    // transpose is semitones, add is scale degrees
    assert_eq!(placed(r#"n("0").scale("C:major").transpose(1)"#)[0].1, 49);
    assert_eq!(placed(r#"n("0").scale("C:major").add(1)"#)[0].1, 50);
    // a chord's root, or its slash bass, in the bass register
    assert_eq!(placed(r#"chord("<Am F/A>").root()"#).iter().map(|x| x.1).collect::<Vec<_>>(), [45, 45]);
}

#[test]
fn words_that_are_not_notes_or_chords_are_errors() {
    let err = |code: &str| script::run(code, None, None).err().unwrap_or_default();
    assert!(err(r#"blank("house"); track("x", note("c3 zz"))"#).contains("cannot read 'zz' as a note"));
    assert!(err(r#"blank("house"); track("x", chord("Am H7"))"#).contains("cannot read 'H7' as a chord"));
    assert!(err(r#"blank("house"); track("x", n("0 e"))"#).contains("scale degrees"));
}

#[test]
fn slash_chords_keep_their_bass_through_key_changes() {
    let out = script::run(r#"blank("house"); key("A minor"); prog("A", "Am F/A C G")"#, None, None).unwrap();
    assert_eq!(out.bp.as_ref().unwrap().harmony.progressions["A"], ["i", "VI/A", "III", "VII"]);
    let out = script::run(r#"key("C minor")"#, out.bp, None).unwrap();
    assert_eq!(out.bp.unwrap().harmony.progressions["A"], ["i", "VI/C", "III", "VII"]);
}

#[test]
fn drum_lanes_named_by_alias_play_their_role() {
    let out =
        script::run(r#"blank("house"); drum("bd", "x...x...x...x..."); drum("sd", "....x.......x...")"#, None, None)
            .unwrap();
    let project = compose::arrange(&out.bp.unwrap());
    let role = |n: &str| project.tracks.iter().find(|t| t.name == n).map(|t| t.role.clone());
    assert_eq!(role("bd").as_deref(), Some("kick"));
    assert_eq!(role("sd").as_deref(), Some("snare"));
}

#[test]
fn a_seed_makes_the_same_song_with_or_without_a_library() {
    struct Empty;
    impl compose::Library for Empty {
        fn one_shot(
            &self,
            _: &str,
            _: &[String],
            _: &genres::GenreProfile,
            _: &mut claudius::rng::Rng,
        ) -> Option<claudius::model::Sound> {
            None
        }
        fn find_loop(
            &self,
            _: &claudius::genres::LoopSpec,
            _: &genres::GenreProfile,
            _: f64,
            _: &claudius::theory::scale::Key,
            _: &mut claudius::rng::Rng,
        ) -> Option<(claudius::model::Sound, i32)> {
            None
        }
    }
    for genre in ["house", "trap", "lofi"] {
        let g = genres::get(genre).unwrap();
        let opts = Options { samples: 0.75, ..Default::default() };
        let without = compose::generate(&g, 9, &opts, None);
        let with = compose::generate(&g, 9, &opts, Some(&Empty));
        assert!(without == with, "{genre}: an empty library changed the song");
    }
}

#[test]
fn options_and_names_are_checked() {
    let err = |code: &str| script::run(code, None, None).err().unwrap_or_else(|| panic!("{code} ran"));
    assert!(err(r#"song("house", #{ bpm: 300 })"#).contains("between 40 and 240"));
    assert!(err(r#"song("house", #{ key: "H minor" })"#).contains("cannot read key 'H minor'"));
    assert!(err(r#"song("house", #{ tempo: 120 })"#).contains("no option 'tempo'"));
    assert!(err(r#"song("house", 1); part("bass", #{ stlye: "pulse" })"#).contains("no option 'stlye'"));
    assert!(err(r#"song("house", 1); part("bass", #{ style: "stabs" })"#).contains("no style 'stabs'"));
    assert!(err(r#"song("house", 1); section(0, #{ prog: "Z" })"#).contains("progression 'Z'"));
    assert!(err(r#"song("house", 1); layer(0, "nope", true)"#).contains("no track 'nope'"));
    assert!(err(r#"blank("house"); part("violin", #{ style: "sustain" })"#).contains("no part 'violin'"));
    assert!(err(r#"blank("house"); drum("kick", "x..."); drum_sound("kick", "supersaw")"#).contains("not a drum"));
}

#[test]
fn blank_songs_print_and_rebuild_exactly() {
    let code = r#"
        blank("house");
        tempo(122);
        prog("A", "i VI III VII");
        bars_per_chord(2);
        drum("kick", "x...x...x...x...");
        drum("clap", "....x.......x...");
        drum_sound("clap", "sd");
        drum("hat", "x.x.x.x.x.x.x.x.");
        drum_sound("hat", "hh:3");
        part("bass", #{ style: "pulse" });
        part("chords", #{ style: "stabs" });
    "#;
    let bp = script::run(code, None, None).unwrap().bp.unwrap();
    assert_eq!(bp.harmony.bars_per_chord, 2);
    assert_eq!(bp.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["bass", "chords"]);
    let printed = script::to_script(&bp);
    assert!(printed.contains("// blank(\"house\")"), "{printed}");
    assert!(printed.contains("drum_sound(\"clap\", \"sd\")"), "{printed}");
    assert!(printed.contains("drum_sound(\"hat\", \"hh:3\")"), "{printed}");
    // the printed script rebuilds the song from nothing, and changes nothing when run on it
    let rebuilt = script::run(&printed, None, None).unwrap().bp.unwrap();
    assert!(rebuilt == bp, "rebuilt differs\n{printed}\n{}", script::to_script(&rebuilt));
    let again = script::run(&printed, Some(bp.clone()), None).unwrap().bp.unwrap();
    assert!(again == bp);
}

#[test]
fn written_notes_print_play_and_clear_back_to_the_theory_engine() {
    let g = genres::get("house").unwrap();
    let bp = compose::generate(&g, 3, &Options::default(), None);
    let code = r#"notes("lead", "A", [[0, 0.5, 64], [0.5, 0.25, 67, 0.6], [1, 1, 71, 1]]);"#;
    let out = script::run(code, Some(bp.clone()), None).unwrap().bp.unwrap();
    let written = &out.parts.iter().find(|p| p.name == "lead").unwrap().notes["A"];
    assert_eq!(written.iter().map(|n| (n.p, n.v)).collect::<Vec<_>>(), [(64, 0.8), (67, 0.6), (71, 1.0)]);
    // printed, it is the same song
    let printed = script::to_script(&out);
    assert!(
        printed.contains(r#"notes("lead", "A", [[0, 0.5, 64, 0.8], [0.5, 0.25, 67, 0.6], [1, 1, 71, 1]]);"#),
        "{printed}"
    );
    assert!(script::run(&printed, Some(out.clone()), None).unwrap().bp.unwrap() == out);
    // the lead plays them
    let p = compose::arrange(&out);
    let lead = p.tracks.iter().find(|t| t.name == "lead").unwrap();
    assert!(lead.notes.iter().any(|n| n.p == 67 && n.v == 0.6), "{:?}", &lead.notes[..lead.notes.len().min(8)]);
    // cleared, the theory engine writes the progression again
    assert!(script::run(r#"clear_notes("lead", "A");"#, Some(out), None).unwrap().bp.unwrap() == bp);
    for (code, says) in [
        (r#"notes("lead", "Z", [])"#, "no progression 'Z'"),
        (r#"notes("nope", "A", [])"#, "no part 'nope'"),
        (r#"notes("lead", "A", [[0, 0, 60]])"#, "note 1 is [start, length, pitch]"),
        (r#"notes("lead", "A", [[0, 1, 60], [1, 1, 200]])"#, "note 2 is"),
    ] {
        let Err(e) = script::run(code, Some(bp.clone()), None) else { panic!("{code} ran") };
        assert!(e.to_string().contains(says), "{code}: {e}");
    }
}

#[test]
fn written_notes_keep_their_loop_when_they_ring_past_it_and_stay_listed_where_the_part_plays_nowhere() {
    let g = genres::get("house").unwrap();
    let bp = compose::generate(&g, 3, &Options::default(), None);
    let cycles = |code: &str| {
        let out = script::run(code, Some(bp.clone()), None).unwrap().bp.unwrap();
        let lead = out.parts.iter().find(|p| p.name == "lead").unwrap();
        compose::cycles(&out, lead)
            .into_iter()
            .map(|(prog, _, beats, written)| (prog, beats, written))
            .collect::<Vec<_>>()
    };
    let beats = cycles("").iter().find(|c| c.0 == "A").unwrap().1;
    // a note in the last beat that rings on into the next time round, as the theory engine's do, keeps the loop
    let ringing = cycles(&format!(r#"notes("lead", "A", [[0, 1, 64], [{}, 2, 67]]);"#, beats - 1.0));
    assert!(ringing.contains(&("A".into(), beats, true)), "{ringing:?}");
    // one that starts in the next time round makes the loop two of them
    let longer = cycles(&format!(r#"notes("lead", "A", [[0, 1, 64], [{beats}, 1, 67]]);"#));
    assert!(longer.contains(&("A".into(), beats * 2.0, true)), "{longer:?}");
    // notes written out over a progression the lead plays nowhere are listed after those it plays
    let elsewhere = cycles(r#"prog("Z", "i iv"); notes("lead", "Z", [[0, 1, 64]]);"#);
    assert!(elsewhere.last().is_some_and(|c| c.0 == "Z" && c.2), "{elsewhere:?}");
    assert!(elsewhere.iter().any(|c| c.0 == "A" && !c.2), "{elsewhere:?}");
}

#[test]
fn parts_of_your_own_print_with_their_role_and_rebuild_exactly() {
    // techno has a lead (lead2 is written like it) and no keys (keys gets the role's own styles and presets)
    let code = r#"
        blank("techno");
        drum("kick", "x...x...x...x...");
        drum_sound("bongo", "tom");
        drum("bongo", "..x...x...x.x...");
        part("keys", #{ role: "keys", preset: "epiano" });
        part("lead2", #{ role: "lead" });
    "#;
    let bp = script::run(code, None, None).unwrap().bp.unwrap();
    let roles: Vec<(&str, &str)> = bp.parts.iter().map(|p| (p.name.as_str(), p.role.name())).collect();
    assert_eq!(roles, [("keys", "keys"), ("lead2", "lead")]);
    assert!(bp.drums.iter().any(|d| d.name == "bongo"), "drum_sound() makes a lane of any name");
    let printed = script::to_script(&bp);
    assert!(printed.contains(r#"part("keys", #{ role: "keys", "#), "{printed}");
    // a lane not named for a drum: its sound first, which makes it
    let (sound, steps) = (printed.find(r#"drum_sound("bongo""#), printed.find(r#"drum("bongo""#));
    assert!(sound.zip(steps).is_some_and(|(a, b)| a < b), "{printed}");
    // built from nothing, as the printed header says: blank("techno") uncommented
    let from_nothing = printed.replacen("// blank(", "blank(", 1);
    let rebuilt = script::run(&from_nothing, None, None).unwrap().bp.unwrap();
    assert!(rebuilt == bp, "rebuilt differs\n{printed}\n{}", script::to_script(&rebuilt));
    let again = script::run(&printed, Some(bp.clone()), None).unwrap().bp.unwrap();
    assert!(again == bp);
    // the genre's own parts print without one, and another role than a part's is an error
    let house = script::run(r#"song("house", 1);"#, None, None).unwrap().bp.unwrap();
    assert!(!script::to_script(&house).contains("role:"));
    let Err(err) = script::run(r#"song("house", 1); part("lead", #{ role: "bass" });"#, None, None) else {
        panic!("a lead made a bass")
    };
    assert!(err.to_string().contains("is a lead part"), "{err}");
    let Err(err) = script::run(r#"song("house", 1); part("x", #{ role: "drums" });"#, None, None) else {
        panic!("a part of role drums")
    };
    assert!(err.to_string().contains("not 'drums'"), "{err}");
    // a drum lane's name is the lane's
    let Err(err) = script::run(r#"song("house", 1); part("kick", #{ role: "bass" });"#, None, None) else {
        panic!("a part named after a drum lane")
    };
    assert!(err.to_string().contains("'kick' is a track of the song that is not a part"), "{err}");
}

#[test]
fn a_tempo_or_key_of_your_own_changes_only_that() {
    let g = genres::get("house").unwrap();
    let plain = compose::generate(&g, 9, &Options::default(), None);
    // another tonic in the song's own mode (another mode moves the chords, as the next test shows)
    let key = claudius::theory::scale::Key::new((plain.key.tonic + 5) % 12, plain.key.mode);
    let mine = compose::generate(&g, 9, &Options { bpm: Some(128.0), key: Some(key), ..Default::default() }, None);
    assert_eq!((mine.bpm, mine.key), (128.0, key));
    assert_eq!(mine.title, plain.title);
    assert_eq!(mine.form, plain.form);
    assert_eq!(mine.harmony.progressions["A"], plain.harmony.progressions["A"]);
    let hits = |bp: &claudius::model::Blueprint| bp.drums.iter().map(|d| d.patterns.clone()).collect::<Vec<_>>();
    assert_eq!(hits(&mine), hits(&plain));
}

#[test]
fn a_key_in_another_mode_gets_the_progressions_in_that_mode() {
    let g = genres::get("house").unwrap();
    let plain = compose::generate(&g, 9, &Options::default(), None);
    let key = claudius::theory::scale::Key::parse("C major");
    let major = compose::generate(&g, 9, &Options { key, ..Default::default() }, None);
    // the same chords on the same degrees, moved to major, as key("C major") would
    let remoded: Vec<String> =
        plain.harmony.progressions["A"].iter().map(|rn| script::remode(rn, &plain.key, &major.key)).collect();
    assert_eq!(major.harmony.progressions["A"], remoded);
    assert!(plain.key.mode != major.key.mode || major.harmony.progressions["A"] == plain.harmony.progressions["A"]);
    // every progression, answered loops and choruses of their own too, whatever the mode
    for (name, seeds) in [("house", 0..12), ("dance_pop", 0..12)] {
        let g = genres::get(name).unwrap();
        for seed in seeds {
            let plain = compose::generate(&g, seed, &Options::default(), None);
            for k in ["C major", "A minor", "D dorian"] {
                let key = claudius::theory::scale::Key::parse(k);
                let mine = compose::generate(&g, seed, &Options { key, ..Default::default() }, None);
                assert_eq!(
                    mine.harmony.progressions.keys().collect::<Vec<_>>(),
                    plain.harmony.progressions.keys().collect::<Vec<_>>()
                );
                for (p, chords) in &plain.harmony.progressions {
                    let moved: Vec<String> =
                        chords.iter().map(|rn| script::remode(rn, &plain.key, &mine.key)).collect();
                    assert_eq!(mine.harmony.progressions[p], moved, "{name} {seed} in {k}: progression {p}");
                }
            }
        }
    }
}

#[test]
fn a_new_part_plays_where_the_form_already_lists_it() {
    // the form names the pad before there is one, as generated forms name the genre's optional parts
    let code = r#"
        blank("house");
        track("kick", s("bd*4"));
        form([sec("intro", 8, "kick pad"), sec("drop", 16, "kick")]);
        part("pad", #{ style: "sustain" });
        track("lead", n("0 2 4 7").s("pluck"));
    "#;
    let bp = script::run(code, None, None).unwrap().bp.unwrap();
    let layers = |i: usize| bp.form[i].layers.clone();
    assert_eq!(layers(0), ["kick", "pad"], "the pad stays where the form put it, once");
    assert_eq!(layers(1), ["kick", "lead"], "a track the form doesn't name goes to the high-energy sections");
}

#[test]
fn a_change_of_mode_keeps_borrowed_chords_as_they_sound() {
    let out = script::run(r#"blank("house"); key("F major"); prog("A", "I bIII IV V"); key("F minor")"#, None, None);
    // diatonic chords take the new mode on their degree; the borrowed A flat stays A flat, now spelled III
    assert_eq!(out.unwrap().bp.unwrap().harmony.progressions["A"], ["i", "III", "iv", "v"]);
}

#[test]
fn effect_chains_with_parallel_branches_and_values_by_section_print_arrange_and_play() {
    use claudius::dsp::env::Param;
    use claudius::fx::Effect;
    use claudius::model::SectionKind;
    let code = r#"
        song("house", 3);
        fx("lead", [
          #{ fx: "saturate", kind: "tape", drive_db: sections(2, #{ drop: [2, 12] }) },
          #{ fx: "parallel", dry: 1, chains: [
            #{ amount: sections(0, #{ build: [0, 0.6], drop: 0.2 }), fx: [#{ fx: "reverb", decay: 4, mix: 1 }] },
            #{ amount: 0.3, fx: [
              #{ fx: "bitcrush", bits: [[0, 12], [64, 4]], downsample: 2 },
              #{ fx: "filter", mode: "hp", cutoff: 900 },
            ] },
          ] },
        ]);
        mix("chords", #{ bus: "glue" });
        bus("glue", [#{ fx: "compressor", threshold_db: -18, ratio: 4, attack_ms: 5, release_ms: 80, mix: 0.5 }]);
        send_fx(#{ delay: #{ fx: "delay", beats: 0.75, feedback: sections(0.3, #{ "break": 0.7 }), mix: 1 } });
        master(#{ fx: [#{ fx: "limiter", ceiling_db: -1, release_ms: 50 }] });
    "#;
    let bp = script::run(code, None, None).unwrap().bp.unwrap();
    let lead = bp.parts.iter().find(|p| p.name == "lead").unwrap();
    assert!(matches!(&lead.mix.fx[1], Effect::Parallel { chains, .. } if chains.len() == 2), "{:?}", lead.mix.fx);
    assert_eq!(bp.parts.iter().find(|p| p.name == "chords").unwrap().mix.bus, "glue");
    // the song prints with them, and reads back as the same song
    let printed = script::to_script(&bp);
    for line in ["sections(0, #{ build: [0, 0.6], drop: 0.2 })", r#"sections(0.3, #{ "break": 0.7 })"#, r#"bus("glue""#]
    {
        assert!(printed.contains(line), "{line} is missing from\n{printed}");
    }
    let again = script::run(&printed, Some(bp.clone()), None).unwrap().bp.unwrap();
    assert!(again == bp, "the printed script changed the song\n{printed}");
    // and, its first line uncommented, makes the song again from its seed with its chains and its routing
    let made = script::run(&printed.replacen("// song(", "song(", 1), None, None).unwrap().bp.unwrap();
    let chords = |b: &claudius::model::Blueprint| b.parts.iter().find(|p| p.name == "chords").unwrap().mix.clone();
    assert_eq!((chords(&made).bus, made.mix.buses.get("glue")), ("glue".to_string(), bp.mix.buses.get("glue")));
    assert_eq!(made.parts.iter().find(|p| p.name == "lead").unwrap().mix.fx, lead.mix.fx);
    // arranged, values by section are curves through the form as it is laid out
    let p = compose::arrange(&bp);
    let drop = p.sections.iter().find(|s| s.kind == SectionKind::Drop).unwrap();
    let (a, b) = (drop.start_bar as f64 * 4.0, (drop.start_bar + drop.bars) as f64 * 4.0);
    let lead = p.tracks.iter().find(|t| t.name == "lead").unwrap();
    let Effect::Saturate { drive_db, .. } = &lead.mix.fx[0] else { panic!("{:?}", lead.mix.fx) };
    assert!(matches!(drive_db, Param::Curve(_)));
    assert_eq!((drive_db.at(a - 1.0, false), drive_db.at(b, false)), (2.0, 12.0), "{drive_db:?}");
    assert!((drive_db.at((a + b) / 2.0, false) - 7.0).abs() < 0.01, "half way across the drop");
    let Effect::Delay { feedback, .. } = &p.mix.delay else { panic!() };
    let brk = p.sections.iter().find(|s| s.kind == SectionKind::Break).unwrap();
    assert_eq!(feedback.at((brk.start_bar + 1) as f64 * 4.0, false), 0.7);
    // and it plays: two bars of the drop, through the parallel chains, the glue bus and the master's limiter
    let r = render::render(&p.slice(drop.start_bar, drop.start_bar + 2), &render::Options::default()).unwrap();
    assert!(r.lufs > -20.0 && r.lufs < -5.0, "{}", r.lufs);
}

#[test]
fn effects_a_script_gets_wrong_say_what_they_are() {
    let fails =
        |code: &str| script::run(&format!("song(\"house\", 3); {code}"), None, None).err().map(|e| e.to_string());
    let says = |code: &str, words: &str| {
        let e = fails(code).unwrap_or_else(|| panic!("{code} ran"));
        assert!(e.contains(words), "{code}: {e}");
    };
    says(r#"fx("lead", [#{ fx: "revrb", decay: 2 }]);"#, "unknown variant `revrb`");
    says(r#"fx("lead", [#{ decay: 2 }]);"#, "is a map such as #{ fx: \"reverb\", ... } (effects: gain eq filter");
    says(r#"fx("lead", [#{ fx: "gain", db: "loud" }]);"#, "a setting that can move is a number, a curve");
    says(r#"fx("nobody", []);"#, "no track 'nobody'");
    says(r#"fx("lead", [#{ fx: "gain", db: sections(0, #{ chorus2: 1 }) }]);"#, "no section or kind 'chorus2'");
    says(r#"fx("lead", [#{ fx: "gain", db: sections(0, #{ drop: [] }) }]);"#, "'drop' is a number, or the numbers");
    says(r#"bus("my bus", []);"#, "a bus is named with letters, digits, - and _");
    says(r#"mix("lead", #{ bus: "" });"#, "a bus is named with letters");
    says(r#"send_fx(#{ room: #{ fx: "reverb" } });"#, "send_fx()");
    // a kind's other names count as the kind
    assert!(fails(r#"fx("lead", [#{ fx: "gain", db: sections(0, #{ chorus: -3, breakdown: -6 }) }]);"#).is_none());
}

#[test]
fn trigs_play_by_chance_and_condition_ratchet_flam_and_lock_the_sound() {
    use claudius::theory::rhythm::{Cond, Trig};
    let code = r#"
        blank("house"); tempo(120);
        drum("kick", "x...............");
        drum("hat", "x.x.x.x.x.x.x.x.");
        form([sec("drop", 16, "hat kick")]);
        trig("hat", 2, #{ ratchet: 3, ramp: 1 });
        trig("hat", 4, #{ prob: 0.5 });
        trig("hat", 6, #{ cond: "last" });
        trig("hat", 8, #{ nudge: 0.25, pitch: 3, pan: -1, decay: 0.5 });
        trig("hat", 10, #{ flam: 20 });
        trig("hat", 13, #{ v: 0.6, cond: "1:2" });
    "#;
    let out = script::run(code, None, None).unwrap();
    assert!(out.log.iter().any(|l| l == "hat main step 2: 3 strokes swelling"), "{:?}", out.log);
    let bp = out.bp.unwrap();
    let hat = bp.drums.iter().find(|d| d.name == "hat").unwrap();
    let main = &hat.patterns["main"];
    assert_eq!(main.len(), 9, "step 13 is a hit now: {main:?}");
    assert_eq!(main.iter().find(|h| h.t == 3.25).and_then(|h| h.trig).map(|t| t.cond), Some(Cond::Every(1, 2)));
    let p = compose::arrange(&bp);
    let notes = &p.tracks.iter().find(|t| t.name == "hat").unwrap().notes;
    let at = |beat: f64| notes.iter().filter(|n| (n.t % 4.0 - beat).abs() < 1e-6).count();
    assert_eq!((at(0.0), at(3.0), at(3.5)), (16, 16, 16), "plain steps every bar");
    assert_eq!((at(0.5), at(0.5 + 1.0 / 12.0), at(0.5 + 2.0 / 12.0)), (16, 16, 16), "three strokes in a sixteenth");
    let chance = at(1.0);
    assert!((4..=12).contains(&chance), "about half the bars: {chance}");
    assert_eq!(
        chance,
        compose::arrange(&bp)
            .tracks
            .iter()
            .find(|t| t.name == "hat")
            .unwrap()
            .notes
            .iter()
            .filter(|n| (n.t % 4.0 - 1.0).abs() < 1e-6)
            .count(),
        "the same each render"
    );
    assert_eq!(
        notes.iter().filter(|n| (n.t % 4.0 - 1.5).abs() < 1e-6).map(|n| n.t).collect::<Vec<_>>(),
        [61.5],
        "only the last bar"
    );
    assert_eq!((at(3.25), at(2.5 - 0.04), at(2.5)), (8, 16, 16), "1:2, and a flam 20 ms ahead at 120 BPM");
    let locked: Vec<_> = notes.iter().filter(|n| (n.t % 4.0 - 2.0).abs() < 1e-6).collect();
    assert_eq!(locked.len(), 16);
    assert!(
        locked
            .iter()
            .all(|n| n.p == 63 && n.lock.is_some_and(|l| l.nudge == 0.0625 && l.pan == -1.0 && l.decay == 0.5))
    );
    // the song prints with them and reads back as the same song
    let printed = script::to_script(&bp);
    for line in [
        r#"trig("hat", "main", 2, #{ ratchet: 3, ramp: 1 });"#,
        r#"trig("hat", "main", 8, #{ nudge: 0.25, pitch: 3, decay: 0.5, pan: -1 });"#,
    ] {
        assert!(printed.contains(line), "{line} is missing from\n{printed}");
    }
    assert!(script::run(&printed, Some(bp.clone()), None).unwrap().bp.unwrap() == bp, "{printed}");
    // the pattern written again keeps the trigs of the steps that still strike; an empty map clears one
    let again =
        script::run(r#"drum("hat", "x.x.x...x.x.x.x."); trig("hat", 2, #{});"#, Some(bp.clone()), None).unwrap();
    let hats = &again.bp.unwrap().drums.into_iter().find(|d| d.name == "hat").unwrap().patterns["main"];
    assert_eq!(hats.iter().filter(|h| h.trig.is_some()).count(), 3, "{hats:?}");
    assert!(hats.iter().find(|h| h.t == 2.0).and_then(|h| h.trig).is_some_and(|t| t.pitch == 3));
    assert!(hats.iter().find(|h| h.t == 0.5).is_some_and(|h| h.trig.is_none()));
    let cleared = script::run(r#"trig("hat", 3, #{});"#, Some(bp.clone()), None).unwrap().bp.unwrap();
    assert_eq!(cleared.drums, bp.drums, "clearing a step without a hit adds none");
    for (bad, says) in [
        (r#"trig("hat", 2, #{ ratchets: 3 })"#, "trig() has no option 'ratchets'"),
        (r#"trig("hat", 2, #{ ratchet: 12 })"#, "trig()'s ratchet is 1 to 8, not 12"),
        (r#"trig("hat", 64, #{ prob: 0.5 })"#, "hat's steps go from 0 to 15"),
        (r#"trig("hat", "fill", 2, #{ prob: 0.5 })"#, "hat has no variant 'fill' (variants: main)"),
        (r#"trig("bongo", 2, #{ prob: 0.5 })"#, "no drum lane 'bongo'"),
        (r#"trig("hat", 2, #{ cond: "often" })"#, "not 'often'"),
    ] {
        let e = script::run(bad, Some(bp.clone()), None).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains(says), "{bad}: {e}");
    }
    let _ = Trig::default();
}

#[test]
fn a_nudge_moves_the_stroke_late_and_a_pan_sends_it_to_one_side() {
    let song = |extra: &str| {
        let code = format!(
            r#"blank("house"); tempo(120); swing(0.5); drum("kick", "....x..........."); form([sec("drop", 1, "kick")]); {extra}"#
        );
        let bp = script::run(&code, None, None).unwrap().bp.unwrap();
        let r = render::render(&compose::arrange(&bp), &render::Options { stems: true, tail_secs: 0.5 }).unwrap();
        r.stems.into_iter().find(|(n, _)| n == "kick").unwrap().1
    };
    let onset =
        |x: &claudius::audio::Stereo| x.l.iter().zip(&x.r).position(|(l, r)| l.abs().max(r.abs()) > 0.01).unwrap();
    // on beat 2, where the jitter humanize adds moves both alike
    let (plain, late) = (song(""), song(r#"trig("kick", 4, #{ nudge: 0.5 });"#));
    // half a sixteenth at 120 BPM: 62.5 ms, 3000 samples
    let moved = onset(&late) as i64 - onset(&plain) as i64;
    assert!((moved - 3000).abs() < 2, "{moved}");
    let left = song(r#"trig("kick", 4, #{ pan: -1 });"#);
    let (l, r) = (left.l.iter().map(|v| v * v).sum::<f32>(), left.r.iter().map(|v| v * v).sum::<f32>());
    assert!(r < 0.01 * l, "all of it on the left: {l} {r}");
}

#[test]
fn a_lane_and_a_part_play_behind_or_ahead_of_the_beat_as_the_script_says_and_read_back() {
    let code = r#"
        blank("boom_bap"); tempo(90);
        drum("kick", "x.......x.......");
        drum("snare", "....x.......x...");
        part("bass", #{ role: "bass", late: 12 });
        form([sec("verse", 4, "kick snare bass")]);
        lane("snare", #{ late: 22 });
        lane("kick", #{ late: -4 });
    "#;
    let out = script::run(code, None, None).unwrap();
    assert!(out.log.iter().any(|l| l == "snare: 22 ms behind the beat"), "{:?}", out.log);
    let bp = out.bp.unwrap();
    let late = |name: &str| bp.drums.iter().find(|d| d.name == name).unwrap().late_ms;
    assert_eq!((late("snare"), late("kick")), (22.0, -4.0));
    assert_eq!(bp.parts.iter().find(|p| p.name == "bass").unwrap().late_ms, 12.0);
    // the arranged tracks carry it to the render
    let p = compose::arrange(&bp);
    let track = |name: &str| p.tracks.iter().find(|t| t.name == name).unwrap().late_ms;
    assert_eq!((track("snare"), track("bass")), (22.0, 12.0));
    // and the song's script says it again
    let again = script::to_script(&bp);
    assert!(again.contains(r#"lane("snare", #{ late: 22 });"#) && again.contains("late: 12 }"), "{again}");
    let back = script::run(&again, None, None).unwrap().bp.unwrap();
    assert_eq!((back.drums.iter().find(|d| d.name == "kick").unwrap().late_ms, back.parts[0].late_ms), (-4.0, 12.0));
    let far = script::run(r#"lane("snare", #{ late: 200 });"#, Some(bp.clone()), None).err().unwrap().to_string();
    assert!(far.contains("late is -80 to 80"), "{far}");
    let far = script::run(r#"part("bass", #{ late: -90 });"#, Some(bp), None).err().unwrap().to_string();
    assert!(far.contains("-80 (ahead) to 80 ms (behind)"), "{far}");
}

#[test]
fn a_lane_keeps_its_own_length_speed_swing_and_groove() {
    let code = r#"
        blank("house"); tempo(120);
        drum("kick", "x...x...x...x...");
        drum("hat", "x.x.x.x.x.x.x.x.");
        drum("perc", "x...............");
        drum("shaker", "xxxxxxxxxxxxxxxx");
        form([sec("drop", 4, "kick hat perc shaker")]);
        lane("hat", #{ length: 12 });
        lane("perc", #{ speed: 0.75 });
        lane("shaker", #{ speed: 2, swing: 0.66 });
        lane("kick", #{ groove: "swing66", groove_amount: 0.5 });
    "#;
    let out = script::run(code, None, None).unwrap();
    assert!(out.log.iter().any(|l| l == "hat: 12 steps round"), "{:?}", out.log);
    let bp = out.bp.unwrap();
    let p = compose::arrange(&bp);
    let times = |name: &str| -> Vec<f64> {
        p.tracks.iter().find(|t| t.name == name).unwrap().notes.iter().map(|n| n.t).collect()
    };
    // 12 steps round against the bar: the hat's six hits come round every 3 beats, not every 4
    let hat = times("hat");
    assert_eq!(&hat[..8], &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5]);
    assert_eq!(hat.len(), 32, "16 beats at two a beat");
    let perc = times("perc");
    assert_eq!(&perc[..3], &[0.0, 16.0 / 3.0, 32.0 / 3.0], "a bar at 3/4 speed lasts 16/3 beats");
    let shaker = times("shaker");
    assert_eq!((shaker.len(), shaker[1]), (128, 0.125), "32nds at double speed");
    assert_eq!(p.tracks.iter().find(|t| t.name == "shaker").unwrap().swing, Some(0.66));
    // the kick on the beats does not move; half of swing66 moves nothing on a beat
    assert_eq!(&times("kick")[..4], &[0.0, 1.0, 2.0, 3.0]);
    // a lane in a groove takes its timing from it, not from the song's swing; one in neither keeps the song's
    let swing = |name: &str| p.tracks.iter().find(|t| t.name == name).unwrap().swing;
    assert_eq!((swing("kick"), swing("hat")), (Some(0.5), None));
    let groove = |code: &str| {
        let bp = script::run(code, Some(bp.clone()), None).unwrap().bp.unwrap();
        compose::arrange(&bp)
            .tracks
            .into_iter()
            .find(|t| t.name == "hat")
            .unwrap()
            .notes
            .iter()
            .map(|n| n.t)
            .take(4)
            .collect::<Vec<f64>>()
    };
    // the off-beat eighths of a straight hat are no sixteenth off-beats: a swing groove moves the sixteenths, a
    // shuffle the eighths
    assert_eq!(groove(r#"lane("hat", #{ length: 0, groove: "swing66" });"#), [0.0, 0.5, 1.0, 1.5]);
    let shuffled = groove(r#"lane("hat", #{ length: 0, groove: "shuffle" });"#);
    assert!((shuffled[1] - 2.0 / 3.0).abs() < 1e-6, "{shuffled:?}");
    let half = groove(r#"lane("hat", #{ length: 0, groove: "shuffle", groove_amount: 0.5 });"#);
    assert!((half[1] - (0.5 + 1.0 / 12.0)).abs() < 1e-6, "{half:?}");
    // a step's strokes go with it: a ratchet on a swung sixteenth keeps its order and its spacing
    let code = r#"lane("hat", #{ length: 0, groove: "swing71" }); trig("hat", 1, #{ ratchet: 4 });"#;
    let rolled = script::run(code, Some(bp.clone()), None).unwrap().bp.unwrap();
    let hat = compose::arrange(&rolled).tracks.into_iter().find(|t| t.name == "hat").unwrap();
    let roll: Vec<f64> = hat.notes.iter().filter(|n| n.d < 0.1 && n.t < 1.0).map(|n| n.t).collect();
    assert_eq!(roll.len(), 4, "{roll:?}");
    assert!((roll[0] - (0.25 + 0.105)).abs() < 1e-6, "the step swung: {roll:?}");
    assert!(roll.windows(2).all(|w| (w[1] - w[0] - 0.0625).abs() < 1e-9), "{roll:?}");
    // a groove written as a table, and one heard in a recording: kept with the song, printed as tables
    let dir = std::env::temp_dir().join(format!("claudius-groove-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("swung.wav");
    let spb = 0.6;
    let n = (8.0 * 4.0 * spb * 48_000.0) as usize + 4_800;
    let mut l = vec![0f32; n];
    for k in 0..8 * 16 {
        let b = k as f64 / 4.0 + if k % 2 == 1 { 0.06 } else { 0.0 };
        let at = (0.1 * 48_000.0 + b * spb * 48_000.0) as usize;
        for i in 0..240.min(n - at) {
            l[at + i] += 0.5 * (-(i as f32) / 40.0).exp() * if i % 2 == 0 { 1.0 } else { -1.0 };
        }
    }
    claudius::audio::save_wav(&file, &claudius::audio::Stereo { l: l.clone(), r: l }, true).unwrap();
    let code = format!(
        r#"groove("loop", {0}); groove("half", {0}, 50); groove("mine", #{{ timing: [0, 0.05, 0, 0.05, 0, 0.05, 0, 0.05, 0, 0.05, 0, 0.05, 0, 0.05, 0, 0.05] }}); lane("hat", #{{ groove: "loop" }}); lane("perc", #{{ groove: "mine" }});"#,
        claudius::pattern::quote(&file.display().to_string())
    );
    let out = script::run(&code, Some(bp.clone()), None).unwrap();
    assert!(
        out.log
            .iter()
            .any(|l| l.contains("heard in") && l.contains("at 100 BPM: the off-beat sixteenths at 62% swing")),
        "{:?}",
        out.log
    );
    assert!(
        out.log.iter().any(|l| l.starts_with("groove half,") && l.contains("at 50 BPM")),
        "a tempo given is kept: {:?}",
        out.log
    );
    let grooved = out.bp.unwrap();
    assert!((grooved.grooves["loop"].timing[1] - 0.06).abs() < 0.012, "{:?}", grooved.grooves["loop"]);
    let printed = script::to_script(&grooved);
    assert!(printed.contains(r#"groove("mine", #{ timing: [0, 0.05, 0, 0.05"#), "{printed}");
    assert!(printed.contains(r#"lane("hat", #{ length: 12, groove: "loop" });"#), "{printed}");
    assert!(script::run(&printed, Some(grooved.clone()), None).unwrap().bp.unwrap() == grooved, "{printed}");
    std::fs::remove_dir_all(&dir).unwrap();
    for (bad, says) in [
        (r#"lane("hat", #{ groove: "jazz" })"#, "lane(): no groove 'jazz' (grooves: straight swing54"),
        (r#"lane("hat", #{ speed: 8 })"#, "lane()'s speed is 0.125 to 4, not 8"),
        (r#"lane("hat", #{ length: 24 })"#, "lane(): hat's length is up to its pattern's 16 steps"),
        (r#"lane("hat", #{ length: -3 })"#, "lane()'s length is a number of steps, or 0 for its pattern's, not -3"),
        (r#"lane("hat", #{ swing: 0.3 })"#, "lane()'s swing is 0.5 (straight) to 0.8, or 0 for the song's"),
        (r#"lane("hat", #{ steps: 12 })"#, "lane() has no option 'steps'"),
        (r#"groove("x", #{ timing: [0, 0.1] })"#, "groove()'s timing is a list of 16 numbers, one a sixteenth, not 2"),
        (r#"groove("x", "/nowhere/loop.wav")"#, "groove(): /nowhere/loop.wav"),
        (r#"groove("x", "/nowhere/loop.wav", 400)"#, "groove()'s tempo is 40 to 250 BPM, not 400"),
    ] {
        let e = script::run(bad, Some(bp.clone()), None).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains(says), "{bad}: {e}");
    }
}

/// `bars` bars of a kit at `bpm`, from `lead` seconds in: a kick on 1 and 3, a snare on 2 and 4, a hat on every off-beat
fn beat(bars: usize, bpm: f64, lead: f64) -> claudius::audio::Stereo {
    use claudius::dsp::filter::butter;
    let sr = 48_000.0;
    let spb = 60.0 / bpm;
    let n = ((lead + bars as f64 * 4.0 * spb) * sr) as usize + 24_000;
    let mut l = vec![0f32; n];
    let mut rng = claudius::rng::Rng::new(5);
    let fade = |i: usize, len: usize| ((len - i) as f32 / 480.0).min(1.0);
    let mut noise = |lo: f64, hi: f64, len: usize| {
        let mut x: Vec<f32> = (0..len).map(|_| rng.f32() * 2.0 - 1.0).collect();
        butter(&mut x, true, lo, sr, 4);
        butter(&mut x, false, hi, sr, 4);
        x
    };
    for b in 0..bars * 4 {
        let at = |beat: f64| ((lead + (b as f64 + beat) * spb) * sr) as usize;
        if b % 2 == 0 {
            let (len, mut ph) = (14_400, 0f32);
            for i in 0..len {
                let t = i as f32 / sr as f32;
                ph += std::f32::consts::TAU * (50.0 + 100.0 * (-t / 0.03).exp()) / sr as f32;
                l[at(0.0) + i] += 0.9 * ph.sin() * (-t / 0.15).exp() * fade(i, len);
            }
        } else {
            let x = noise(1000.0, 6000.0, 9_600);
            for (i, v) in x.iter().enumerate() {
                l[at(0.0) + i] += 0.6 * v * (-(i as f32 / sr as f32) / 0.08).exp() * fade(i, x.len());
            }
        }
        let x = noise(7000.0, 20_000.0, 4_800);
        for (i, v) in x.iter().enumerate() {
            l[at(0.5) + i] += 0.35 * v * (-(i as f32 / sr as f32) / 0.03).exp() * fade(i, x.len());
        }
    }
    claudius::audio::Stereo { l: l.clone(), r: l }
}

#[test]
fn a_beat_in_a_recording_becomes_the_drum_lanes() {
    let dir = std::env::temp_dir().join(format!("claudius-drums-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("beat 100bpm.wav");
    claudius::audio::save_wav(&file, &beat(4, 100.0, 0.3), true).unwrap();
    let path = claudius::pattern::quote(&file.display().to_string());
    let out = script::run(&format!(r#"song("house", 3); drums_from({path}, #{{ bars: 1 }});"#), None, None).unwrap();
    let said = out.log.iter().find(|l| l.contains("into drums")).cloned().unwrap_or_default();
    assert!(
        said.contains(
            "1 bar as their main patterns: kick 2 steps into kick, snare 2 steps into clap, hat 4 steps into hat"
        ),
        "{:?}",
        out.log
    );
    let bp = out.bp.unwrap();
    let steps = |lane: &str| -> Vec<u32> {
        let mut s: Vec<u32> = bp.drums.iter().find(|d| d.name == lane).unwrap().patterns["main"]
            .iter()
            .map(|h| (h.t * 4.0).round() as u32)
            .filter(|s| *s < 16)
            .collect();
        s.dedup();
        s
    };
    assert_eq!((steps("kick"), steps("clap"), steps("hat")), (vec![0, 8], vec![4, 12], vec![2, 6, 10, 14]));
    // a step heard every time round plays every time: a chance it had goes, what else its trig does stays
    let code = format!(
        r#"song("house", 3); trig("kick", 0, #{{ prob: 0.5, ratchet: 2 }}); drums_from({path}, #{{ bars: 1 }});"#
    );
    let again = script::run(&code, None, None).unwrap().bp.unwrap();
    let first = again.drums.iter().find(|d| d.name == "kick").unwrap().patterns["main"]
        .iter()
        .find(|h| h.t == 0.0)
        .unwrap()
        .trig;
    assert_eq!(first.map(|t| (t.prob, t.ratchet)), Some((1.0, 2)), "{first:?}");
    // the printed script writes the lanes as patterns: it needs no recording to read back
    let printed = script::to_script(&bp);
    assert!(!printed.contains("drums_from"), "{printed}");
    assert!(script::run(&printed, Some(bp.clone()), None).unwrap().bp.unwrap() == bp, "{printed}");
    let silent = dir.join("silence.wav");
    claudius::audio::save_wav(&silent, &claudius::audio::Stereo::zeros(96_000), true).unwrap();
    let quiet = claudius::pattern::quote(&silent.display().to_string());
    for (bad, says) in [
        (format!("drums_from({path}, #{{ bars: 3 }})"), "a beat's pattern is 1, 2 or 4 bars, not 3".to_string()),
        (format!("drums_from({path}, #{{ beats: 2 }})"), "drums_from() has no option 'beats'".to_string()),
        (format!("drums_from({path}, #{{ bpm: 400 }})"), "drums_from()'s bpm is 40 to 250, not 400".to_string()),
        (format!("drums_from({quiet})"), "too few drum hits".to_string()),
        (r#"drums_from("/nowhere/beat.wav")"#.to_string(), "drums_from(): /nowhere/beat.wav".to_string()),
    ] {
        let e = script::run(&bad, Some(bp.clone()), None).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains(&says), "{bad}: {e}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
