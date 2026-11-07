use claudius::rng::Rng;
use claudius::theory::chord::{Chord, Quality, Voicing, roman, roman_of, voice};
use claudius::theory::progression;
use claudius::theory::rhythm::{self, LaneTemplate};
use claudius::theory::scale::{Key, Mode};
use claudius::theory::{ChordSpan, bass, melody};

#[test]
fn keys_parse_and_name() {
    assert_eq!(Key::parse("Am").unwrap(), Key::new(9, Mode::Minor));
    assert_eq!(Key::parse("F# dorian").unwrap(), Key::new(6, Mode::Dorian));
    assert_eq!(Key::parse("Eb").unwrap(), Key::new(3, Mode::Major));
    assert_eq!(Key::new(10, Mode::Major).name(), "Bb major");
    assert_eq!(Key::new(2, Mode::Minor).name(), "D minor");
}

#[test]
fn roman_numerals_follow_the_key_mode() {
    let am = Key::new(9, Mode::Minor);
    let names: Vec<String> =
        ["i", "VI", "III", "VII", "iv7", "V7", "bII"].iter().map(|r| roman(r, &am).unwrap().symbol(false)).collect();
    assert_eq!(names, ["Am", "F", "C", "G", "Dm7", "E7", "A#"]);
    let c = Key::new(0, Mode::Major);
    assert_eq!(roman("V7/V", &c).unwrap().symbol(false), "D7");
    assert_eq!(roman("bVII", &c).unwrap().symbol(true), "Bb");
    assert_eq!(roman("iiø7", &c).unwrap().quality, Quality::HalfDim7);
    for rn in ["i7", "VImaj7", "iv7", "v7"] {
        let d = Key::new(2, Mode::Minor);
        assert_eq!(roman_of(&roman(rn, &d).unwrap(), &d), rn);
    }
}

#[test]
fn chord_symbols_round_trip() {
    for s in ["Am7", "F/A", "Cmaj7", "G7", "Bbm9", "Dsus4", "E5"] {
        assert_eq!(Chord::parse(s).unwrap().symbol(s.contains('b')), s);
    }
}

#[test]
fn voicings_stay_in_range_and_move_little() {
    let key = Key::new(9, Mode::Minor);
    let prog = progression::chords(&progression::parse("i VI III VII"), &key);
    let mut prev: Option<Vec<i32>> = None;
    let mut total = 0;
    for c in &prog {
        let v = voice(c, prev.as_deref(), 55, 76, 4, Voicing::Close, None);
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|p| (55..=76).contains(p)), "{v:?}");
        assert!(v.iter().all(|p| c.has_pc(p.rem_euclid(12) as u8)));
        if let Some(p) = &prev {
            total += v.iter().zip(p).map(|(a, b)| (a - b).abs()).sum::<i32>();
        }
        prev = Some(v);
    }
    assert!(total <= 24, "voice leading moved {total} semitones over 3 changes");
}

#[test]
fn power_chords_are_the_root_its_fifth_and_the_octave_and_move_by_their_roots() {
    let key = Key::new(4, Mode::Minor);
    let prog = progression::chords(&progression::parse("i VI III"), &key);
    let e = voice(&prog[0], None, 40, 64, 3, Voicing::Power, None);
    assert_eq!(e, [40, 47, 52], "E2 B2 E3");
    let c = voice(&prog[1], Some(&e), 40, 64, 3, Voicing::Power, None);
    assert_eq!(c, [48, 55, 60], "C3 is the only C with room for its fifth and octave");
    let g = voice(&prog[2], Some(&c), 40, 64, 2, Voicing::Power, None);
    assert_eq!(g, [43, 50], "two voices: no octave; G2 is nearer C3 than G3 is");
}

#[test]
fn markov_progressions_are_diatonic_loops() {
    let key = Key::new(0, Mode::Major);
    let mut rng = Rng::new(3);
    for _ in 0..20 {
        let p = progression::markov(&key, 4, 0, &mut rng);
        assert_eq!(p.len(), 4);
        for c in progression::chords(&p, &key) {
            assert!(c.pcs().iter().all(|pc| key.pcs().contains(pc)), "{p:?}");
        }
    }
}

#[test]
fn every_library_progression_reads_in_its_mode() {
    for style in progression::STYLES {
        for p in style.progs {
            for tonic in 0..12 {
                let key = Key::new(tonic, style.mode);
                for rn in progression::parse(p) {
                    assert!(roman(&rn, &key).is_some(), "{}: {rn} in {p} does not read in {}", style.name, key.name());
                }
            }
        }
    }
}

#[test]
fn an_answer_plays_the_loop_then_ends_it_another_way() {
    let mut answered = 0;
    for style in progression::STYLES {
        for p in style.progs.iter().map(|p| progression::parse(p)).filter(|p| p.len() == 4) {
            for seed in 0..8 {
                let key = Key::new(seed as u8 % 12, style.mode);
                let Some((a, what)) = progression::answer(&p, &key, &mut Rng::new(seed)) else { continue };
                answered += 1;
                assert_eq!(a.len(), 8, "{what}");
                assert_eq!(a[..4], p[..], "{what}");
                assert!(a.iter().all(|rn| roman(rn, &key).is_some()), "{a:?}");
                // the second time round ends another way, and changes at most one chord before that
                assert_ne!(a[6..], p[2..], "{} {p:?}: {what}", style.name);
                assert_eq!(a[4], p[0], "{what}");
                let changed = (4..8).filter(|&i| a[i] != p[i - 4]).count();
                assert!(changed <= 3, "{p:?} -> {a:?}: {what}");
            }
        }
    }
    assert!(answered > 300, "{answered} loops answered");
}

#[test]
fn colours_and_suspensions_of_diatonic_chords_keep_to_the_key() {
    // a chord coloured in place (extended or suspended, not borrowed) stays in the key, unless it became a
    // dominant, which may lean on a leading tone the mode does not have
    let in_key = |rn: &str, key: &Key| {
        let c = roman(rn, key).unwrap();
        c.quality == Quality::Dom7 || c.quality == Quality::Dom9 || c.pcs().iter().all(|pc| key.pcs().contains(pc))
    };
    let diatonic = |c: &Chord, key: &Key| c.pcs().iter().all(|pc| key.pcs().contains(pc));
    // the library's loops (borrowed and applied chords among them) and Markov ones, in every mode
    let mut loops: Vec<(Key, Vec<String>)> = progression::STYLES
        .iter()
        .flat_map(|s| s.progs.iter().map(move |p| (Key::new(4, s.mode), progression::parse(p))))
        .filter(|(_, p)| p.len() == 4)
        .collect();
    let mut rng = Rng::new(11);
    for mode in [Mode::Major, Mode::Minor, Mode::Dorian, Mode::Phrygian, Mode::Mixolydian] {
        let key = Key::new(4, mode);
        for _ in 0..30 {
            loops.push((key, progression::markov(&key, 4, rng.below(3) as u8, &mut rng)));
        }
    }
    for (key, p) in &loops {
        for _ in 0..4 {
            if let Some((a, what)) = progression::answer(p, key, &mut rng) {
                for i in 4..8 {
                    let (was, now) = (roman(&p[i - 4], key).unwrap(), roman(&a[i], key).unwrap());
                    let coloured = now.quality.triad() == was.quality.triad()
                        || matches!(now.quality, Quality::Sus2 | Quality::Sus4);
                    if now != was && now.root == was.root && coloured && diatonic(&was, key) {
                        assert!(in_key(&a[i], key), "{} {p:?} -> {a:?}: {what}", key.name());
                    }
                }
            }
            let (s, what) = progression::substitute(p, key, &mut rng);
            if what.contains("extended") || what.contains("suspension") {
                for (now, was) in s.iter().zip(p).filter(|(now, was)| now != was) {
                    if diatonic(&roman(was, key).unwrap(), key) {
                        assert!(in_key(now, key), "{} {p:?} -> {s:?}: {what}", key.name());
                    }
                }
            }
        }
    }
}

#[test]
fn diatonic_sevenths_and_ninths_are_the_keys_own() {
    let q = |mode: Mode, d: i32, ext: u8| progression::diatonic(&Key::new(9, mode), d, ext).quality;
    // C major's fifth: a dominant seventh and ninth, never a major seventh
    assert_eq!((q(Mode::Major, 4, 1), q(Mode::Major, 4, 2)), (Quality::Dom7, Quality::Dom9));
    assert_eq!(q(Mode::Major, 0, 1), Quality::Maj7);
    // a minor key's fifth has no ninth in the key: it stays a seventh
    assert_eq!(q(Mode::Minor, 4, 2), Quality::Min7);
    assert_eq!(q(Mode::Minor, 0, 2), Quality::Min9);
    // harmonic minor: the tonic's seventh would be major over a minor third, so it stays a triad, and the leading
    // note's seventh chord is fully diminished
    assert_eq!(q(Mode::HarmonicMinor, 0, 1), Quality::Min);
    assert_eq!(q(Mode::HarmonicMinor, 6, 1), Quality::Dim7);
    assert_eq!(q(Mode::HarmonicMinor, 4, 1), Quality::Dom7);
    for mode in
        [Mode::Major, Mode::Minor, Mode::Dorian, Mode::Phrygian, Mode::Lydian, Mode::Mixolydian, Mode::HarmonicMinor]
    {
        let key = Key::new(2, mode);
        for d in 0..7 {
            for ext in 0..3 {
                let c = progression::diatonic(&key, d, ext);
                assert!(c.pcs().iter().all(|pc| key.pcs().contains(pc)), "{} degree {d} ext {ext}: {c:?}", key.name());
            }
        }
    }
}

#[test]
fn euclid_spreads_onsets() {
    let e = rhythm::euclid(3, 8, 0);
    assert_eq!(e.iter().filter(|x| **x).count(), 3);
    assert_eq!(rhythm::euclid(4, 16, 0), (0..16).map(|i| i % 4 == 0).collect::<Vec<_>>());
}

#[test]
fn templates_keep_anchors() {
    let t = LaneTemplate {
        steps: vec![1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
        vel: (0.9, 1.0),
        ghost: 0.0,
        rolls: 0.0,
        vary: 0.0,
    };
    let hits = rhythm::generate(&t, 4, &mut Rng::new(1));
    assert_eq!(hits.len(), 16);
    assert!(hits.iter().all(|h| (h.t % 1.0).abs() < 1e-9));
}

#[test]
fn basslines_land_on_roots_and_leads_stay_in_key() {
    let key = Key::new(2, Mode::Minor);
    let chords: Vec<ChordSpan> = progression::chords(&progression::parse("i7 VImaj7 iv7 v7"), &key)
        .into_iter()
        .enumerate()
        .map(|(i, c)| ChordSpan { start: i as f64 * 4.0, len: 4.0, chord: c })
        .collect();
    let mut rng = Rng::new(9);
    for style in bass::styles() {
        let notes = bass::write(style, &chords, &key, &[0.0, 1.0, 2.0, 3.0], (28, 52), 9, 0);
        assert!(!notes.is_empty(), "{style}");
        for c in &chords {
            if let Some(n) = notes.iter().find(|n| (n.t - c.start).abs() < 1e-6 && n.v > 0.5)
                && style != "acid"
            {
                assert_eq!(n.p.rem_euclid(12) as u8, c.chord.bass_pc(), "{style} at {}", c.start);
            }
        }
    }
    let lead = melody::lead(&chords, &key, 0.0, 8, 0.5, true, (64, 84), &mut rng);
    assert!(lead.iter().all(|n| key.contains(n.p)));
    assert!(lead.iter().all(|n| (60..=88).contains(&n.p)));
}
