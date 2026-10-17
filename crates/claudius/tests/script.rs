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
}

#[test]
fn errors_say_what_to_do() {
    let e = script::run(r#"track("x", s("foo*4"))"#, None, None).err().unwrap();
    assert!(e.contains("unknown sound 'foo'") && e.contains("synths:"), "{e}");
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
