use claudius::audio::Stereo;
use claudius::compose::{self, Options};
use claudius::config::SRF;
use claudius::dsp::osc::{Osc, Wave};
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
/// and four, hats on the eighths, and a bass on the root of each bar's chord (A, F, C, G round and round); and when
/// each of its beats falls, the first a second in.
fn band(from: f64, to: f64, bars: usize) -> (Stereo, Vec<f64>) {
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
    for b in 0..bars {
        let (t0, t1) = (at[4 * b], at[4 * b + 4]);
        let f = [55.0, 43.65, 65.41, 49.0][b % 4];
        let s = (t0 * SRF) as usize;
        for i in 0..(((t1 - t0) * SRF) as usize).min(n - s) {
            let tt = i as f64 / SRF;
            x[s + i] += ((std::f64::consts::TAU * f * tt).sin() * 0.25) as f32 * (1.0 - (-tt / 0.01).exp()) as f32;
        }
    }
    (Stereo::from_mono(&x), at)
}

#[test]
fn a_band_that_speeds_up_is_followed_beat_for_beat_and_one_that_keeps_time_is_on_its_grid() {
    // 92 to 100 BPM over 48 bars: a grid of one tempo would be more than two beats off at one end or the other
    let (x, at) = band(92.0, 100.0, 48);
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
    let (x, at) = band(96.0, 96.0, 24);
    let g = analysis::beats::track(&x.mono(), None);
    assert!(g.steady && (g.bpm - 96.0).abs() < 0.2, "{} BPM", g.bpm);
    assert!(at[8..at.len() - 8].iter().all(|&t| near(t, &g.beats) < 0.015));
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
