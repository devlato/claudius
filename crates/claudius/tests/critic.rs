//! The critic hears what breaks a song. A song it passes, with one thing broken that a listener hears at once (a track
//! far too loud or too quiet, a sound its genre has no place for), scores lower, and the critic names the fault
//! where its fix lives.

use std::sync::Once;

use claudius::compose::{self, Options};
use claudius::model::{Blueprint, SectionKind, Sound};
use claudius::synth::presets;
use claudius::{genres, judge, render};

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

/// the critic's verdict, without the ears, on the first drop of `bp` (or its first hook) and the eight bars before
fn verdict(bp: &Blueprint) -> judge::Verdict {
    let p = compose::arrange(bp);
    let first = |k: SectionKind| p.sections.iter().find(|s| s.kind == k);
    let drop = first(SectionKind::Drop).or_else(|| first(SectionKind::Hook)).expect("a drop or a hook");
    let slice = p.slice(drop.start_bar.saturating_sub(8), drop.start_bar + drop.bars.min(16));
    let r = render::render(&slice, &render::Options { stems: false, tail_secs: 0.0 }).unwrap();
    judge::judge(&slice, &r, None)
}

fn targets(v: &judge::Verdict) -> Vec<&str> {
    v.issues.iter().map(|i| i.target.as_str()).collect()
}

fn song_of(genre: &str, seed: u64) -> Blueprint {
    without_installed_packs();
    compose::generate(&genres::get(genre).unwrap(), seed, &Options::default(), None)
}

fn song() -> Blueprint {
    song_of("house", 3)
}

fn level(bp: &mut Blueprint, track: &str, db: f32) {
    if let Some(d) = bp.drums.iter_mut().find(|d| d.name == track) {
        d.mix.level_db = db;
    } else {
        bp.parts.iter_mut().find(|p| p.name == track).expect("the track").mix.level_db = db;
    }
}

#[test]
fn a_song_as_its_genre_makes_it_has_no_track_out_of_balance_and_no_sound_out_of_place() {
    // trap and R&B set their claps under the snare, deep house its transitions under the drums: as they mean to
    for (genre, seed) in [("house", 3), ("trap", 3), ("rnb", 3), ("deep_house", 3), ("hip_hop", 5)] {
        let v = verdict(&song_of(genre, seed));
        let wrong: Vec<&str> =
            targets(&v).into_iter().filter(|t| t.starts_with("level:") || t.starts_with("sound:")).collect();
        assert!(wrong.is_empty(), "{genre}: {wrong:?}: {}", v.summary);
    }
}

#[test]
fn a_track_far_too_loud_or_too_quiet_is_named_and_costs_the_mix() {
    let base = verdict(&song());
    for (track, db, target) in
        [("chords", 9.0, "level:chords+"), ("bass", -9.0, "level:bass-"), ("clap", 9.0, "level:clap+")]
    {
        let mut bp = song();
        level(&mut bp, track, db);
        let v = verdict(&bp);
        assert!(targets(&v).contains(&target), "{track} {db:+} dB: {}", v.summary);
        assert!(
            v.scores["mix"] < base.scores["mix"] - 0.1,
            "{track}: mix {} from {}",
            v.scores["mix"],
            base.scores["mix"]
        );
        assert!(v.total < base.total - 3.0, "{track}: {} from {}", v.total, base.total);
    }
    // a level a producer might choose is no fault
    let mut bp = song();
    level(&mut bp, "chords", 2.5);
    assert!(!targets(&verdict(&bp)).iter().any(|t| t.starts_with("level:")));
}

#[test]
fn a_sound_its_genre_has_no_place_for_is_named_and_costs_the_song() {
    let base = verdict(&song());
    let mut bp = song();
    let chords = bp.parts.iter_mut().find(|p| p.name == "chords").expect("house plays chords");
    chords.sound = Sound::Synth { preset: "whistle".into(), patch: presets::get("whistle").unwrap() };
    let v = verdict(&bp);
    assert!(targets(&v).contains(&"sound:chords"), "{}", v.summary);
    assert!(
        v.scores["theory"] < base.scores["theory"] - 0.1,
        "theory {} from {}",
        v.scores["theory"],
        base.scores["theory"]
    );
    assert!(v.total < base.total - 3.0, "{} from {}", v.total, base.total);
}

#[test]
fn a_recording_out_of_the_songs_time_is_named_and_cuts_the_verdict_whatever_else_is_right() {
    let p = compose::arrange(&song());
    let slice = p.slice(0, 16);
    let r = render::render(&slice, &render::Options { stems: false, tail_secs: 0.0 }).unwrap();
    let base = judge::judge(&slice, &r, None);
    // the same render with a recording on its first track that keeps the song's time, and then one that is 3 % slow
    let keeps = |time: render::KeptTime| {
        let mut r = render::Rendered { tracks: r.tracks.clone(), master: r.master.clone(), stems: Vec::new(), ..r };
        r.tracks[0].time = Some(time);
        judge::judge(&slice, &r, None)
    };
    let on = keeps(render::KeptTime { on_grid: 0.5, own: 0.5, own_bpm: slice.bpm, drift_ms: 5.0 });
    assert!((on.total - base.total).abs() < 1e-9 && on.scores["time"] == 1.0, "{}", on.summary);
    let off = keeps(render::KeptTime { on_grid: 0.02, own: 0.4, own_bpm: slice.bpm * 0.97, drift_ms: 60.0 });
    let name = &r.tracks[0].name;
    assert_eq!(off.issues[0].target, format!("time:{name}"), "{}", off.summary);
    assert!((off.total - judge::OUT_OF_TIME * base.total).abs() < 1e-9, "{} from {}", off.total, base.total);
}
