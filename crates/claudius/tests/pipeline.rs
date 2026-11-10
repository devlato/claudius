use claudius::audio::Stereo;
use claudius::compose::{self, Options};
use claudius::config::SRF;
use claudius::dsp::env::Param;
use claudius::dsp::osc::{Osc, Wave};
use claudius::fx::{Effect, SatKind};
use claudius::theory::scale::{Key, Mode};
use claudius::{analysis, genres, judge, loudness, render};

#[test]
fn every_genre_composes_and_arranges() {
    for g in genres::all().unwrap() {
        let bp = compose::generate(&g, 5, &Options { short: true, ..Default::default() }, None);
        let p = compose::arrange(&bp);
        assert!(p.bars >= 16, "{}", g.name);
        let names = bp.layer_names();
        for s in &bp.form {
            assert!(
                s.layers.iter().all(|l| names.contains(l)
                    || g.loops.iter().any(|x| &x.name == l)
                    || g.parts.iter().any(|x| &x.name == l)),
                "{}: {:?}",
                g.name,
                s.layers
            );
        }
        assert!(p.tracks.iter().any(|t| !t.notes.is_empty()));
        let json = bp.to_json();
        let back: claudius::model::Blueprint = serde_json::from_str(&json).unwrap();
        assert_eq!(back, bp);
    }
}

#[test]
fn the_drums_end_their_phrases_as_the_genres_drummers_do_and_only_where_a_section_fills() {
    for (genre, seed) in [("boom_bap", 4), ("trap", 2), ("reggae", 5), ("dnb", 1), ("art_rock", 2)] {
        let bp = compose::generate(&genres::get(genre).unwrap(), seed, &Options::default(), None);
        assert_eq!(compose::drummer(&bp).name, claudius::theory::drummer::of(genre).name);
        let mut plain = bp.clone();
        plain.form.iter_mut().for_each(|s| s.fills = false);
        let (filled, plain) = (compose::arrange(&bp), compose::arrange(&plain));
        // the last bar of each four-bar phrase of a section that fills, and the beat before a gap
        let mut ends = Vec::new();
        let mut bar = 0;
        for s in bp.form.iter() {
            if s.fills {
                ends.extend(
                    (1..=s.bars).filter(|k| k.is_multiple_of(4) || *k == s.bars).map(|k| (bar + k) as f64 * 4.0),
                );
            }
            bar += s.bars;
        }
        let ending = |t: f64| ends.iter().any(|&e| t >= e - 5.0 - 1e-9 && t < e);
        let mut played = 0;
        for d in &bp.drums {
            let notes = |p: &claudius::model::Project| -> Vec<(f64, i32, f32)> {
                p.tracks.iter().find(|t| t.name == d.name).unwrap().notes.iter().map(|n| (n.t, n.p, n.v)).collect()
            };
            let (with, without) = (notes(&filled), notes(&plain));
            let away = |v: &[(f64, i32, f32)]| v.iter().filter(|n| !ending(n.0)).copied().collect::<Vec<_>>();
            assert_eq!(away(&with), away(&without), "{genre}: {} plays its loop away from the phrases' ends", d.name);
            played += usize::from(with != without);
        }
        assert!(played > 0, "{genre}: its drummer played nothing");
    }
}

#[test]
fn a_short_song_renders_to_target() {
    let g = genres::get("house").unwrap();
    let bp = compose::generate(&g, 2, &Options { short: true, ..Default::default() }, None);
    let p = compose::arrange(&bp).slice(0, 24);
    let r = render::render(&p, &render::Options::default()).unwrap();
    assert!(r.master.l.iter().chain(&r.master.r).all(|v| v.is_finite()));
    assert!(r.true_peak_db <= p.mix.master.ceiling_db + 0.05, "true peak {}", r.true_peak_db);
    assert!((r.lufs - p.mix.master.lufs as f64).abs() < 1.0, "lufs {} vs {}", r.lufs, p.mix.master.lufs);
    let v = judge::judge(&p, &r, None);
    assert!(v.total > 50.0, "{}", v.summary);
    // the same song crushed to three bits and clipped hard is noise, which no meter on the master hears as a fault
    let mut crushed = p.clone();
    crushed
        .mix
        .master
        .fx
        .insert(0, Effect::Bitcrush { bits: Param::Const(3.0), downsample: 6.0, mix: Param::Const(1.0) });
    crushed
        .mix
        .master
        .fx
        .insert(1, Effect::Saturate { kind: SatKind::Hard, drive_db: Param::Const(30.0), mix: Param::Const(1.0) });
    let r = render::render(&crushed, &render::Options::default()).unwrap();
    let noisy = judge::judge(&crushed, &r, None);
    assert!(noisy.total < 0.6 * v.total, "{} after {}", noisy.summary, v.summary);
    assert!(noisy.issues.iter().any(|i| i.what.starts_with("noisy") && i.target == "production"), "{}", noisy.summary);
}

fn click_track(bpm: f64, secs: f64) -> Stereo {
    let n = (secs * SRF) as usize;
    let mut x = vec![0.0f32; n];
    let beat = 60.0 / bpm;
    let mut k = 0;
    loop {
        let t = k as f64 * beat;
        if t >= secs {
            break;
        }
        let s = (t * SRF) as usize;
        let mut o = Osc::new(Wave::Sine, 0.0);
        for i in 0..((0.08 * SRF) as usize).min(n - s) {
            let tt = i as f64 / SRF;
            let f = 50.0 + 120.0 * (-tt / 0.02).exp();
            x[s + i] += o.tick(f, SRF) * (-tt / 0.05).exp() as f32 * if k % 4 == 0 { 1.0 } else { 0.7 };
        }
        k += 1;
    }
    Stereo::from_mono(&x)
}

/// A band whose tempo rises from `from` to `to` BPM over `bars` bars of 4/4: a kick on one and three, a snare on two
/// and four, hats on the eighths, and a bass on the root of each bar's chord (A, F, C, G round and round); when each
/// of its beats falls, the first a second in; and its drums and its bass apart.
fn band(from: f64, to: f64, bars: usize) -> (Stereo, Vec<f64>, Stereo, Stereo) {
    let beats = bars * 4;
    let mut at = vec![1.0];
    for k in 0..beats {
        let bpm = from + (to - from) * k as f64 / beats as f64;
        at.push(at[k] + 60.0 / bpm);
    }
    let n = ((at[beats] + 1.0) * SRF) as usize;
    let mut x = vec![0.0f32; n];
    let mut rng = claudius::rng::Rng::new(11);
    let hit = |x: &mut Vec<f32>, t: f64, secs: f64, f: &dyn Fn(f64) -> f32| {
        let s = (t * SRF) as usize;
        for i in 0..((secs * SRF) as usize).min(n - s) {
            x[s + i] += f(i as f64 / SRF);
        }
    };
    for k in 0..beats {
        let (t, beat) = (at[k], at[k + 1] - at[k]);
        if k % 2 == 0 {
            let mut o = Osc::new(Wave::Sine, 0.0);
            let s = (t * SRF) as usize;
            for i in 0..((0.15 * SRF) as usize).min(n - s) {
                let tt = i as f64 / SRF;
                x[s + i] += o.tick(50.0 + 120.0 * (-tt / 0.02).exp(), SRF) * (-tt / 0.06).exp() as f32 * 0.9;
            }
        } else {
            let noise: Vec<f32> = (0..(0.12 * SRF) as usize).map(|_| rng.f32() - 0.5).collect();
            hit(&mut x, t, 0.12, &|tt| noise[(tt * SRF) as usize] * (-tt / 0.04).exp() as f32 * 0.6);
        }
        for h in [t, t + beat / 2.0] {
            let noise: Vec<f32> = (0..(0.03 * SRF) as usize + 1).map(|_| rng.f32() - 0.5).collect();
            hit(&mut x, h, 0.03, &|tt| {
                let i = (tt * SRF) as usize;
                (noise[i + 1] - noise[i]) * (-tt / 0.008).exp() as f32 * 0.3
            });
        }
    }
    // the bass, a bar a note, which changes on each downbeat
    let mut bass = vec![0.0f32; n];
    for b in 0..bars {
        let (t0, t1) = (at[4 * b], at[4 * b + 4]);
        let f = [55.0, 43.65, 65.41, 49.0][b % 4];
        let s = (t0 * SRF) as usize;
        for i in 0..(((t1 - t0) * SRF) as usize).min(n - s) {
            let tt = i as f64 / SRF;
            bass[s + i] += ((std::f64::consts::TAU * f * tt).sin() * 0.25) as f32 * (1.0 - (-tt / 0.01).exp()) as f32;
        }
    }
    let (drums, bass) = (Stereo::from_mono(&x), Stereo::from_mono(&bass));
    let mut mix = drums.clone();
    mix.add_at(&bass, 0, 1.0, 1.0);
    (mix, at, drums, bass)
}

#[test]
fn a_band_that_speeds_up_is_followed_beat_for_beat_and_one_that_keeps_time_is_on_its_grid() {
    // 92 to 100 BPM over 48 bars: a grid of one tempo would be more than two beats off at one end or the other
    let (x, at, _, _) = band(92.0, 100.0, 48);
    let g = analysis::beats::track(&x.mono(), None);
    assert!(!g.steady, "it speeds up: {} BPM", g.bpm);
    let near = |t: f64, beats: &[f64]| beats.iter().map(|b| (b - t).abs()).fold(f64::MAX, f64::min);
    let inner = &at[8..at.len() - 8];
    let close = inner.iter().filter(|&&t| near(t, &g.beats) < 0.025).count();
    assert!(close as f64 >= 0.95 * inner.len() as f64, "{close} of {} beats within 25 ms", inner.len());
    // the bars start where the band's do, at the start, in the middle and at the end
    let a = analysis::song::analyze(&x, None);
    assert!(!a.steady && a.bars >= 46, "{} bars", a.bars);
    let bar = a.bar_at(at[4] + 0.01) as usize;
    for b in [2, 24, 44] {
        let (heard, played) = (a.bar_start(bar + b - 1), at[4 * b]);
        assert!((heard - played).abs() < 0.03, "bar {b}: {heard:.3} s, played at {played:.3} s");
    }
    // the same band in time keeps its grid
    let (x, at, _, _) = band(96.0, 96.0, 24);
    let g = analysis::beats::track(&x.mono(), None);
    assert!(g.steady && (g.bpm - 96.0).abs() < 0.2, "{} BPM", g.bpm);
    assert!(at[8..at.len() - 8].iter().all(|&t| near(t, &g.beats) < 0.015));
}

#[test]
fn with_its_stems_apart_the_kit_is_heard_in_its_drums_and_a_recording_with_none_has_no_drum_habits() {
    use claudius::stems::Stems;
    let (x, _, drums, bass) = band(96.0, 96.0, 16);
    let silent = Stereo::zeros(x.len());
    let apart = Stems { vocals: silent.clone(), drums, bass: bass.clone(), other: silent.clone() };
    let a = analysis::song::analyze_with(&x, Some(&apart), None);
    assert!((a.bpm - 96.0).abs() < 0.5 && a.steady, "{} BPM", a.bpm);
    // the kick on one and three, the snare on two and four
    assert!(a.kick_steps[0] > 0.8 && a.kick_steps[8] > 0.8 && a.kick_steps[4] < 0.5, "{:?}", a.kick_steps);
    assert!(a.snare_steps[4] > 0.8 && a.snare_steps[12] > 0.8, "{:?}", a.snare_steps);
    assert!(a.sections.iter().all(|s| s.kick && s.bass), "{:?}", a.sections);
    // the bass alone, with its stems saying so: no kit, so no drum habits, and no kick or hats anywhere
    let alone = Stems { vocals: silent.clone(), drums: silent.clone(), bass: bass.clone(), other: silent };
    let a = analysis::song::analyze_with(&bass, Some(&alone), None);
    assert!(a.kick_steps.iter().chain(&a.snare_steps).chain(&a.hat_steps).all(|v| *v == 0.0));
    assert!(a.sections.iter().all(|s| !s.kick && !s.hats && s.bass), "{:?}", a.sections);
}

#[test]
fn a_house_track_keeps_its_grid_and_its_first_bar_with_its_bass_and_hats_on_the_offbeats() {
    // a kick on every beat and the bass and the hats between them: the beats the tracker follows slip onto the offbeats
    // and their slope reads more than 1 % fast, which a grid fitted within half a percent of it cannot reach
    let bp = compose::generate(&genres::get("house").unwrap(), 5, &Options::default(), None);
    let p = compose::arrange(&bp).slice(0, 32);
    let r = render::render(&p, &render::Options { stems: true, ..Default::default() }).unwrap();
    let sum = |of: &dyn Fn(&str) -> bool| {
        let mut x = Stereo::zeros(r.master.len());
        for (_, s) in r.stems.iter().filter(|(name, _)| of(name)) {
            x.add_at(s, 0, 1.0, 1.0);
        }
        x
    };
    let drum = |name: &str| {
        bp.drums.iter().any(|d| d.name == name) || p.tracks.iter().any(|t| t.name == name && t.role == "fx")
    };
    // the drums apart as Spleeter leaves them, with some of the bass in them
    let mut drums = sum(&drum);
    drums.add_at(&sum(&|n| n == "bass"), 0, 0.6, 0.6);
    let apart = claudius::stems::Stems {
        vocals: Stereo::zeros(r.master.len()),
        drums,
        bass: sum(&|n| n == "bass"),
        other: sum(&|n| !drum(n) && n != "bass"),
    };
    let a = analysis::song::analyze_with(&r.master, Some(&apart), None);
    assert!(a.steady && (a.bpm - bp.bpm).abs() < 0.3, "{} BPM, steady {}", a.bpm, a.steady);
    assert!(a.bar_start(0) < 0.05, "its first bar starts at {:.3} s", a.bar_start(0));
    assert!((a.bar_secs - 240.0 / bp.bpm).abs() < 0.01, "bars of {:.3} s", a.bar_secs);
}

#[test]
fn the_beat_tracker_finds_the_tempo() {
    for bpm in [92.0, 124.0, 140.0, 174.0] {
        let x = click_track(bpm, 30.0);
        let g = analysis::beats::track(&x.mono(), None);
        let err =
            ((g.bpm - bpm) / bpm).abs().min(((g.bpm * 2.0 - bpm) / bpm).abs()).min(((g.bpm / 2.0 - bpm) / bpm).abs());
        assert!(err < 0.01, "{bpm} BPM read as {}", g.bpm);
    }
}

#[test]
fn a_songs_tempo_is_heard_at_its_own_and_neither_a_third_off_it_nor_at_twice_a_slow_one() {
    // the tempos these were heard at before: hip hop's syncopated kick at five thirds of it, drum and bass at half,
    // a slow lofi song at twice, house as it is
    for (genre, seed) in [("hip_hop", 3), ("dnb", 3), ("lofi", 3), ("house", 1)] {
        let bp = compose::generate(&genres::get(genre).unwrap(), seed, &Options::default(), None);
        let (a, b) = claudius::evolve::preview_window(&bp, 48);
        let p = compose::arrange(&bp).slice(a, b);
        let r = render::render(&p, &render::Options { stems: false, tail_secs: 1.0 }).unwrap();
        let heard = analysis::song::analyze(&r.master, None);
        assert!((heard.bpm / bp.bpm - 1.0).abs() < 0.01, "{genre} at {} BPM heard at {}", bp.bpm, heard.bpm);
    }
}

#[test]
fn chroma_finds_the_key_of_a_cadence() {
    // I IV V I in G major as sustained sine triads
    let key = Key::new(7, Mode::Major);
    let triads = [[55, 59, 62], [60, 64, 67], [62, 66, 69], [55, 59, 62]];
    let n = (2.0 * SRF) as usize;
    let mut x = Vec::new();
    for t in triads {
        let mut oscs: Vec<Osc> = t.iter().map(|_| Osc::new(Wave::Sine, 0.0)).collect();
        for _ in 0..n {
            x.push(
                oscs.iter_mut()
                    .zip(t.iter())
                    .map(|(o, p)| o.tick(claudius::audio::midi_to_hz(*p as f64), SRF))
                    .sum::<f32>()
                    / 3.0,
            );
        }
    }
    let c = analysis::chroma::chroma(&x, 4096);
    let mut avg = [0.0f32; 12];
    for f in &c {
        for i in 0..12 {
            avg[i] += f[i];
        }
    }
    let (k, _) = analysis::chroma::key(&avg);
    assert_eq!(k, key);
}

#[test]
fn loudness_matches_bs1770_for_a_sine() {
    // BS.1770: a -3 dBFS 1 kHz sine on both channels is -3.0 LUFS
    let n = (5.0 * SRF) as usize;
    let mut o = Osc::new(Wave::Sine, 0.0);
    let m: Vec<f32> = (0..n).map(|_| o.tick(1000.0, SRF) * std::f32::consts::FRAC_1_SQRT_2).collect();
    let l = loudness::integrated(&Stereo::from_mono(&m));
    assert!((l + 3.0).abs() < 0.2, "{l}");
}
