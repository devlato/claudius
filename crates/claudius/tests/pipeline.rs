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
