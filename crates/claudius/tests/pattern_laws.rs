//! Properties of mini-notation and patterns that must hold for any input, checked with generated cases.

use claudius::pattern::{Pattern, mini};
use claudius::script;
use proptest::prelude::*;

/// mini-notation built from the pieces people actually write
fn mini_source() -> impl Strategy<Value = String> {
    let atom = prop_oneof![
        Just("bd".to_string()),
        Just("sd".to_string()),
        Just("hh".to_string()),
        Just("~".to_string()),
        Just("cp:2".to_string()),
        (0..12i32).prop_map(|n| n.to_string()),
    ];
    let step = (
        atom,
        prop_oneof![
            Just(""),
            Just("*2"),
            Just("*3"),
            Just("/2"),
            Just("!"),
            Just("?"),
            Just("@2"),
            Just("(3,8)"),
            Just("(5,8,2)")
        ],
    )
        .prop_map(|(a, m)| format!("{a}{m}"));
    let seq = prop::collection::vec(step, 1..6).prop_map(|v| v.join(" "));
    seq.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|s| format!("[{s}]")),
            inner.clone().prop_map(|s| format!("<{s}>")),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("{a}, {b}")),
            (inner.clone(), inner).prop_map(|(a, b)| format!("{a} [{b}]*2")),
        ]
    })
}

fn pattern(code: &str) -> Pattern {
    script::eval_pattern(code).unwrap_or_else(|e| panic!("{code}: {e}"))
}

/// (onset, value) pairs, sorted; events that start together have no order of their own
#[derive(Debug)]
struct Onsets(Vec<(f64, String)>);

impl PartialEq for Onsets {
    /// the same values starting at the same times, give or take floating-point rounding; matched as a multiset, so
    /// events that start together may come in any order
    fn eq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        let mut used = vec![false; other.0.len()];
        self.0.iter().all(|(t, v)| {
            let hit = other.0.iter().enumerate().position(|(i, (u, w))| !used[i] && (t - u).abs() < 1e-9 && v == w);
            hit.map(|i| used[i] = true).is_some()
        })
    }
}

fn onsets(p: &Pattern, b: f64, e: f64) -> Onsets {
    let mut out: Vec<(f64, String)> = p.events(b, e).into_iter().map(|ev| (ev.b, format!("{:?}", ev.v))).collect();
    out.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| x.1.cmp(&y.1)));
    Onsets(out)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn the_parser_never_panics(src in ".{0,120}") {
        let _ = mini::parse(&src);
    }

    #[test]
    fn events_start_inside_the_span_and_in_order(src in mini_source(), from in 0u32..6) {
        let p = pattern(&format!("s({src:?})"));
        let (b, e) = (from as f64, from as f64 + 2.0);
        let evs = p.events(b, e);
        for w in evs.windows(2) {
            prop_assert!(w[0].b <= w[1].b);
        }
        for ev in &evs {
            prop_assert!(ev.b >= b - 1e-9 && ev.b < e - 1e-9, "{} not in [{b}, {e})", ev.b);
            prop_assert!(ev.e > ev.b && ev.e.is_finite());
        }
    }

    #[test]
    fn querying_in_pieces_gives_the_same_events(src in mini_source()) {
        let p = pattern(&format!("s({src:?})"));
        let mut pieces = onsets(&p, 0.0, 1.0).0;
        pieces.extend(onsets(&p, 1.0, 1.5).0);
        pieces.extend(onsets(&p, 1.5, 3.0).0);
        prop_assert_eq!(Onsets(pieces), onsets(&p, 0.0, 3.0));
    }

    #[test]
    fn fast_then_slow_is_the_original(src in mini_source(), k in 2u32..5) {
        let a = pattern(&format!("s({src:?})"));
        let b = pattern(&format!("s({src:?}).fast({k}).slow({k})"));
        prop_assert_eq!(onsets(&a, 0.0, 4.0), onsets(&b, 0.0, 4.0));
    }

    #[test]
    fn late_then_early_is_the_original(src in mini_source(), t in prop_oneof![Just(0.25), Just(0.125), Just(0.5)]) {
        let a = pattern(&format!("s({src:?})"));
        let b = pattern(&format!("s({src:?}).late({t}).early({t})"));
        prop_assert_eq!(onsets(&a, 1.0, 4.0), onsets(&b, 1.0, 4.0));
    }

    #[test]
    fn reversing_twice_is_the_original(src in mini_source()) {
        let a = pattern(&format!("s({src:?})"));
        let b = pattern(&format!("s({src:?}).rev().rev()"));
        prop_assert_eq!(onsets(&a, 0.0, 3.0), onsets(&b, 0.0, 3.0));
    }

    #[test]
    fn randomness_is_the_same_every_time(src in mini_source()) {
        let code = format!("s({src:?}).sometimes(|x| x.fast(2)).degradeBy(0.3)");
        prop_assert_eq!(onsets(&pattern(&code), 0.0, 8.0), onsets(&pattern(&code), 0.0, 8.0));
    }

    #[test]
    fn degrading_nothing_or_everything(src in mini_source()) {
        let a = pattern(&format!("s({src:?})"));
        prop_assert_eq!(onsets(&a, 0.0, 2.0), onsets(&pattern(&format!("s({src:?}).degradeBy(0)")), 0.0, 2.0));
        let silent = pattern(&format!("s({src:?}).degradeBy(1)"));
        prop_assert!(silent.events(0.0, 2.0).is_empty());
    }

    #[test]
    fn printed_patterns_read_back_the_same(src in mini_source()) {
        let code = format!("s({src:?}).every(3, |x| x.rev()).gain(\"1 0.7\")");
        let p = pattern(&code);
        let again = pattern(&p.repr);
        prop_assert_eq!(onsets(&p, 0.0, 6.0), onsets(&again, 0.0, 6.0));
    }
}
