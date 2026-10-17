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
    for style in bass::STYLES {
        let notes = bass::bassline(style, &chords, &key, &[0.0, 1.0, 2.0, 3.0], 40, &mut rng);
        assert!(!notes.is_empty(), "{style}");
        for c in &chords {
            if let Some(n) = notes.iter().find(|n| (n.t - c.start).abs() < 1e-6)
                && *style != "acid"
            {
                assert_eq!(n.p.rem_euclid(12) as u8, c.chord.bass_pc(), "{style} at {}", c.start);
            }
        }
    }
    let lead = melody::lead(&chords, &key, 0.0, 8, 0.5, true, (64, 84), &mut rng);
    assert!(lead.iter().all(|n| key.contains(n.p)));
    assert!(lead.iter().all(|n| (60..=88).contains(&n.p)));
}
