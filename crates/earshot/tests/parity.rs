//! Parity with the Python earshot (fixtures from parity/make_fixtures.py).
//!
//!     cargo test --release -- --nocapture                 # fast checks (no models)
//!     cargo test --release -- --ignored --nocapture       # model checks (CLAP, AST, calibration, realness, find)
//!
//! Inputs that are missing on this machine (Apple Loops) are skipped.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::Value;

use earshot::{Ears, EarsOptions, audio, features};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// the Python reference decoded every file with ffmpeg; without it, earshot decodes, downmixes and resamples on its
/// own, which is close but not sample-exact
fn exact_decoding() -> bool {
    audio::ffmpeg().is_file()
}

fn fixtures() -> Vec<(Value, PathBuf)> {
    let dir = crate_dir().join("parity").join("fixtures");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("parity/fixtures missing: run parity/make_fixtures.py")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "json") && !p.file_name().unwrap().to_string_lossy().starts_with('_')
        })
        .collect();
    v.sort();
    v.into_iter()
        .filter_map(|p| {
            let j: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let rel = j["path"].as_str().unwrap();
            let path = if Path::new(rel).is_absolute() { PathBuf::from(rel) } else { crate_dir().join(rel) };
            if !path.exists() {
                eprintln!("skip {} (missing)", path.display());
                None
            } else if !exact_decoding() && claudius_decode::decode(&path).is_ok_and(|d| d.rate != 48000) {
                eprintln!("skip {} (resampled, and without ffmpeg not the way the reference was)", path.display());
                None
            } else {
                Some((j, path))
            }
        })
        .collect()
}

fn common() -> Value {
    serde_json::from_str(&std::fs::read_to_string(crate_dir().join("parity/fixtures/_common.json")).unwrap()).unwrap()
}

fn cross() -> Value {
    serde_json::from_str(&std::fs::read_to_string(crate_dir().join("parity/fixtures/_cross.json")).unwrap()).unwrap()
}

fn f64s(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()).unwrap_or_default()
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let d: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    d / (na * nb)
}

/// collects failures so a run reports every mismatch, then panics once
struct Report {
    name: &'static str,
    fails: Vec<String>,
}

impl Report {
    fn new(name: &'static str) -> Report {
        Report { name, fails: vec![] }
    }
    fn check(&mut self, ok: bool, msg: String) {
        if ok {
            println!("  ok   {msg}");
        } else {
            println!("  FAIL {msg}");
            self.fails.push(msg);
        }
    }
    fn close(&mut self, what: &str, a: Option<f64>, b: Option<f64>, tol: f64) {
        match (a, b) {
            (Some(a), Some(b)) => self.check(
                (a - b).abs() <= tol + 1e-12,
                format!("{what}: rust {a} py {b} (|d| {:.4} <= {tol})", (a - b).abs()),
            ),
            (None, None) => println!("  ok   {what}: both absent"),
            _ => self.check(false, format!("{what}: rust {a:?} py {b:?}")),
        }
    }
    fn finish(&self) {
        assert!(self.fails.is_empty(), "{}: {} failures:\n{}", self.name, self.fails.len(), self.fails.join("\n"));
    }
}

fn opt(v: &Value) -> Option<f64> {
    v.as_f64()
}

// ------------------------------------------------------------------------------------------------ fast checks
#[test]
fn decode_matches_ffmpeg_path() {
    let mut r = Report::new("decode");
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let (l, rr) = audio::load_stereo(&path).unwrap();
        let m = audio::load_mono(&path).unwrap();
        let st = &j["decode"]["stereo"];
        let mo = &j["decode"]["mono"];
        r.check(l.len() * 2 == st["n"].as_u64().unwrap() as usize, format!("{name}: stereo length {}", l.len()));
        r.check(m.len() == mo["n"].as_u64().unwrap() as usize, format!("{name}: mono length {}", m.len()));
        let s: f64 = l.iter().zip(&rr).map(|(a, b)| *a as f64 + *b as f64).sum();
        let sq: f64 = m.iter().map(|a| (*a as f64).powi(2)).sum();
        r.close(&format!("{name}: stereo sum"), Some(s), opt(&st["sum"]), 1e-6 * st["n"].as_f64().unwrap());
        r.close(
            &format!("{name}: mono sumsq"),
            Some(sq),
            opt(&mo["sumsq"]),
            1e-6 * mo["sumsq"].as_f64().unwrap().abs() + 1e-9,
        );
        // the in-memory downmix reproduces ffmpeg's -ac 1 for stereo sources
        if audio::src_channels(&path) == 2 {
            let d = audio::downmix(&l, &rr);
            let err = d.iter().zip(&m).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            r.check(err < 1e-6, format!("{name}: downmix vs ffmpeg -ac 1 max err {err:.2e}"));
        }
    }
    r.finish();
}

#[test]
fn resampler_matches_soxr_hq() {
    let mut r = Report::new("resample");
    let c = common();
    for (src, outs) in c["resample"].as_object().unwrap() {
        let x = audio::load_mono(&crate_dir().join("parity/media").join(src)).unwrap();
        let x = &x[..x.len().min(96000)];
        for (sr, o) in outs.as_object().unwrap() {
            let y = earshot::dsp::resample(x, 48000, sr.parse().unwrap());
            r.check(
                y.len() == o["stats"]["n"].as_u64().unwrap() as usize,
                format!("{src} 48k -> {sr}: length {}", y.len()),
            );
            let h = y.len() / 2;
            let mut maxd = 0.0f64;
            for (part, off) in [("head", 0usize), ("mid", h), ("tail", y.len() - 256)] {
                for (i, w) in f64s(&o[part]).iter().enumerate() {
                    maxd = maxd.max((y[off + i] as f64 - w).abs());
                }
            }
            let peak = o["stats"]["max"].as_f64().unwrap().abs().max(o["stats"]["min"].as_f64().unwrap().abs());
            let ss: f64 = y.iter().map(|v| (*v as f64).powi(2)).sum();
            let want = o["stats"]["sumsq"].as_f64().unwrap();
            // 48k -> 22.05k: soxr interpolates its coefficients for this ratio (~1e-4 of full scale)
            let tol = if sr == "22050" { 2e-3 } else { 2e-6 } * peak.max(1e-3);
            r.check(
                maxd <= tol,
                format!("{src} 48k -> {sr}: max |d| vs soxr {maxd:.2e} (peak {peak:.3}, tol {tol:.1e})"),
            );
            r.check(
                ((ss - want) / want).abs() < 1e-3,
                format!("{src} 48k -> {sr}: energy rust {ss:.6} soxr {want:.6}"),
            );
        }
    }
    r.finish();
}

#[test]
fn units_match_python_definitions() {
    let mut r = Report::new("units");
    use earshot::vocab::spans_for;
    r.check(spans_for(0.0, 4.0, 6) == vec![(0.0, 4.0)], "spans_for <= 10 s is one span".into());
    let s = spans_for(0.0, 15.0, 6);
    r.check(s == vec![(0.0, 10.0), (5.0, 15.0)], format!("spans_for 15 s {s:?}"));
    let s = spans_for(0.0, 60.0, 6);
    let starts: Vec<f64> = s.iter().map(|x| x.0).collect();
    // range(0, 50 s, 5 s) = 11 starts -> linspace(0, 10, 6) rounded half-even = [0, 2, 4, 6, 8, 10]
    r.check(starts == vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0], format!("spans_for 60 s starts {starts:?}"));
    let s = spans_for(0.0, 23.0, 6);
    r.check(s.last() == Some(&(13.0, 23.0)) && s.len() == 4, format!("spans_for 23 s last flush {s:?}"));
    for (z, v) in [(3.0, "strong"), (2.99, "plausible"), (2.5, "plausible"), (2.0, "weak"), (1.99, "no match")] {
        r.check(earshot::matching::verdict(z) == v, format!("verdict({z}) = {v}"));
    }
    r.check(earshot::matching::zf(2.96) == "+2.9", format!("zf(2.96) = {}", earshot::matching::zf(2.96)));
    r.check(earshot::matching::zf(-0.04) == "-0.1", format!("zf(-0.04) = {}", earshot::matching::zf(-0.04)));
    r.check(earshot::dsp::round_to(2.675, 2) == 2.67, "round(2.675, 2) = 2.67 like Python".into());
    r.check(earshot::dsp::round_to(0.125, 2) == 0.12, "round(0.125, 2) = 0.12 (ties to even)".into());
    let p = earshot::matching::softmax(&[0.3, 0.1], 38.75);
    r.check((p[0] - 0.99957).abs() < 1e-4, format!("softmax {p:?}"));
    let w = earshot::search::window_candidates(12.0, None, None, 0.0);
    // win = clip(12 / 6, 1, 5) = 2 s, hop 1 s: starts 0 .. 10 s
    r.check(
        w.len() == 11 && (w[0].1 - 2.0).abs() < 1e-9,
        format!("window_candidates(12 s): {} windows of 2 s", w.len()),
    );
    // the downmix coefficient ffmpeg uses for stereo -> mono
    let d = audio::downmix(&[1.0, 0.5], &[1.0, -0.5]);
    r.check((d[0] - 2f32.sqrt()).abs() < 1e-6 && d[1].abs() < 1e-7, format!("downmix {d:?}"));
    r.finish();
}

#[test]
fn ears_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Ears>();
}

#[test]
fn offline_mode_refuses_to_download() {
    let dir = std::env::temp_dir().join(format!("earshot-offline-{}", std::process::id()));
    let err = earshot::models::fetch(&dir, earshot::models::AST_REPO, earshot::models::AST_REV, "config.json", true)
        .unwrap_err();
    assert!(err.to_string().contains("offline"), "{err}");
    assert!(!dir.exists(), "offline fetch must not create the cache");
}

#[test]
#[ignore]
fn download_is_atomic_and_cached() {
    let dir = std::env::temp_dir().join(format!("earshot-dl-{}", std::process::id()));
    let p = earshot::models::fetch(&dir, earshot::models::AST_REPO, earshot::models::AST_REV, "config.json", false)
        .unwrap();
    let cfg: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert_eq!(cfg["id2label"].as_object().unwrap().len(), 527);
    let leftovers: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "no partial files left behind");
    // second call: served from the cache even when offline
    let p2 =
        earshot::models::fetch(&dir, earshot::models::AST_REPO, earshot::models::AST_REV, "config.json", true).unwrap();
    assert_eq!(p, p2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn shared_ears_across_threads() {
    let e = ears();
    let x = audio::load_mono(&crate_dir().join("parity/media/kick.wav")).unwrap();
    let want = e.embed(&x).unwrap();
    let got: Vec<Vec<f32>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..4).map(|_| s.spawn(|| e.embed(&x).unwrap())).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for g in got {
        let c = cosine(
            &g.iter().map(|v| *v as f64).collect::<Vec<_>>(),
            &want.iter().map(|v| *v as f64).collect::<Vec<_>>(),
        );
        assert!(c > 0.99999, "thread result cosine {c}");
    }
}

#[test]
fn clap_front_end_matches_feature_extractor() {
    let mut r = Report::new("clap_mel");
    let fe = earshot::clap::FrontEnd::default();
    // without ffmpeg the downmix rounds differently in the quietest bins
    let (tol, sum_tol) = if exact_decoding() { (2e-3, 1e-5) } else { (2e-2, 1e-4) };
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let m = audio::load_mono(&path).unwrap();
        let dur = m.len() as f64 / 48000.0;
        let sp = earshot::vocab::spans_for(0.0, dur.max(0.01), 6);
        let a = ((sp[0].0 * 48000.0).round_ties_even() as usize).min(m.len());
        let b = ((sp[0].1 * 48000.0).round_ties_even() as usize).min(m.len());
        let f = fe.features(&m[a..b], earshot::Pad::RepeatPad);
        let cm = &j["clap_mel"];
        let rows = [("row0", 0usize), ("row500", 500), ("row1000", 1000)];
        let mut maxd = 0.0f64;
        for (k, i) in rows {
            let want = f64s(&cm[k]);
            for (m_, w) in want.iter().enumerate() {
                maxd = maxd.max((f[i * 64 + m_] as f64 - w).abs());
            }
        }
        for (t, w) in f64s(&cm["col10"]).iter().enumerate() {
            maxd = maxd.max((f[t * 50 * 64 + 10] as f64 - w).abs());
        }
        let sum: f64 = f.iter().map(|v| *v as f64).sum();
        let ws = cm["stats"]["sum"].as_f64().unwrap();
        r.check(maxd < tol, format!("{name}: CLAP log-mel max |d| over sampled rows/cols {maxd:.2e} dB"));
        r.check(
            ((sum - ws) / ws.abs().max(1.0)).abs() < sum_tol,
            format!("{name}: CLAP log-mel sum rust {sum:.3} py {ws:.3}"),
        );
    }
    r.finish();
}

fn num(m: &Value, k: &str) -> Option<f64> {
    m.get(k).and_then(|v| v.as_f64())
}

#[test]
fn measure_matches_features_measure() {
    let mut r = Report::new("measure");
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        println!("{name}");
        let (l, rr) = audio::load_stereo(&path).unwrap();
        let t = std::time::Instant::now();
        let m = features::measure(&l, &rr, true, false);
        println!("  measure (music, full) took {:.2} s", t.elapsed().as_secs_f64());
        let ours = serde_json::to_value(&m).unwrap();
        let py = &j["measure"];
        for (k, tol) in [
            ("duration", 0.0005),
            ("peak_dbfs", 0.1),
            ("true_peak_dbtp", 0.1),
            ("rms_dbfs", 0.1),
            ("lufs", 0.1),
            ("short_term_max_lufs", 0.1),
            ("lra_lu", 0.2),
            ("dc", 1e-5),
            ("noise_floor_dbfs", 0.1),
            ("dynamic_range_db", 0.1),
            ("digital_silence_frac", 1e-4),
            ("side_to_mid_db", 0.1),
            ("correlation", 0.001),
            ("attack_at", 0.001),
            ("peak_at", 0.001),
            ("attack_ms", 0.0),
            ("decay_ms", 0.0),
            ("centroid_hz", 1.0),
            ("rolloff95_hz", 0.0),
            ("bandwidth_hz", 0.0),
            ("flatness", 1e-4),
            ("brightness_hz", 1.0),
            ("tonal_peak_ratio_db", 0.1),
            ("harmonic_ratio", 0.005),
            ("f0_hz", 0.5),
            ("onset_count", 0.0),
            ("onsets_per_s", 0.01),
            ("tempo", 0.5),
            ("beat_ibi_std_ms", 2.0),
            ("clipped_samples", 0.0),
        ] {
            r.close(&format!("{name}: {k}"), num(&ours, k), num(py, k), tol);
        }
        for b in ["sub", "low", "lowmid", "mid", "presence", "air", "top"] {
            r.close(&format!("{name}: bands_db.{b}"), num(&ours["bands_db"], b), num(&py["bands_db"], b), 0.1);
        }
        let (on_r, on_p) = (f64s(&ours["onsets"]), f64s(&py["onsets"]));
        let matched = on_p.iter().filter(|p| on_r.iter().any(|q| (*q - **p).abs() <= 0.0121)).count();
        r.check(
            on_r.len() == on_p.len() && matched == on_p.len(),
            format!(
                "{name}: onsets {} vs {} (python onsets matched within one frame: {matched})",
                on_r.len(),
                on_p.len()
            ),
        );
        if py.get("beats").is_some() {
            let (br, bp) = (f64s(&ours["beats"]), f64s(&py["beats"]));
            let matched = bp.iter().filter(|p| br.iter().any(|q| (*q - **p).abs() <= 0.0121)).count();
            r.check(
                matched as f64 >= 0.9 * bp.len() as f64,
                format!("{name}: beats {} vs {} (matched {matched})", br.len(), bp.len()),
            );
            let kr = ours["key"][0]["key"].as_str().unwrap_or("-").to_string();
            let kp = py["key"][0]["key"].as_str().unwrap_or("-").to_string();
            r.check(
                kr == kp,
                format!("{name}: key {kr} (r {}) vs {kp} (r {})", ours["key"][0]["r"], py["key"][0]["r"]),
            );
        }
    }
    r.finish();
}

#[test]
fn music_grid_matches_timeline() {
    let mut r = Report::new("grid");
    for (j, path) in fixtures() {
        let Some(g) = j.get("grid") else { continue };
        let name = j["name"].as_str().unwrap();
        let (l, rr) = audio::load_stereo(&path).unwrap();
        let mono = audio::mean_mono(&l, &rr);
        let t = std::time::Instant::now();
        let ours = earshot::grid::music_grid(&mono);
        println!("{name}: music_grid {:.2} s", t.elapsed().as_secs_f64());
        r.close(&format!("{name}: tempo"), ours.tempo, opt(&g["tempo"]), 0.5);
        r.close(&format!("{name}: tempo_alt"), ours.tempo_alt, opt(&g["tempo_alt"]), 0.5);
        r.close(&format!("{name}: pulse_clarity"), ours.pulse_clarity, opt(&g["pulse_clarity"]), 0.01);
        r.close(&format!("{name}: beat_regularity"), ours.beat_regularity, opt(&g["beat_regularity"]), 0.01);
        r.check(
            ours.confidence == g["confidence"].as_str().unwrap(),
            format!("{name}: confidence {} vs {}", ours.confidence, g["confidence"]),
        );
        let bp = f64s(&g["beats"]);
        let matched = bp.iter().filter(|p| ours.beats.iter().any(|q| (*q - **p).abs() <= 0.0101)).count();
        r.check(
            matched as f64 >= 0.95 * bp.len() as f64,
            format!("{name}: beats {} vs {} (matched {matched})", ours.beats.len(), bp.len()),
        );
        let dp = f64s(&g["downbeats"]);
        let dm = dp.iter().filter(|p| ours.downbeats.iter().any(|q| (*q - **p).abs() <= 0.0101)).count();
        r.check(
            dm as f64 >= 0.9 * dp.len() as f64,
            format!("{name}: downbeats {} vs {} (matched {dm})", ours.downbeats.len(), dp.len()),
        );
    }
    r.finish();
}

/// measurements defined by soxr's own float32 round-off noise on noiseless synthetic signals (README, known
/// deviations): the spectral floor of a pure sine or sweep (Python's formula on our resampler's output, which is
/// within 2.4e-7 of soxr's, gives our value), and the nonzero-sample count around a 1 ms click (crest, texture)
fn known_deviation(name: &str, feature: &str) -> bool {
    matches!(
        (name, feature),
        ("sine.wav", "log_flatness" | "peakiness_db" | "spec_floor_db" | "quiet_flat")
            | ("sweep.flac", "quiet_flat")
            | ("click.wav", "crest_db" | "texture_kurt")
    )
}

#[test]
fn realness_signal_features_match() {
    let mut r = Report::new("signal_features");
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let m = audio::load_mono(&path).unwrap();
        let (x, gain, _, _) = earshot::realness::lift_quiet(&m);
        r.close(&format!("{name}: lift gain"), Some(gain), opt(&j["realness_gain"]), 1e-6);
        let clips = earshot::realness::clips(x.len());
        let pyc = j["realness_clips"].as_array().unwrap();
        r.check(clips.len() == pyc.len(), format!("{name}: {} clips vs {}", clips.len(), pyc.len()));
        for ((_, _, a, b), pc) in clips.iter().zip(pyc) {
            let c = &x[*a..*b];
            let f = if earshot::realness::audible(c) { earshot::realness::signal_features(c) } else { None };
            match (f, pc["features"].as_object()) {
                (Some(f), Some(pf)) => {
                    for (k, v) in pf {
                        let (ours, py) = (f.get(k), v.as_f64());
                        let tol = match k.as_str() {
                            "silence_frac" | "tonal_frames" | "line_share" => 0.01,
                            "bandwidth_khz" => 0.05,
                            "pitch_wobble" | "env_resid_db" | "texture_kurt" | "log_flatness" | "quiet_flat" => 0.05,
                            _ => 0.5,
                        };
                        let ours = if ours.is_finite() { Some(ours) } else { None };
                        if known_deviation(name, k) {
                            println!(
                                "  known deviation (soxr round-off defines it) {name}: {k} rust {ours:?} py {py:?}"
                            );
                            continue;
                        }
                        r.close(&format!("{name} @{:.1}s: {k}", pc["t0"].as_f64().unwrap()), ours, py, tol);
                    }
                }
                (None, None) => println!("  ok   {name}: silent clip"),
                (a, b) => r.check(false, format!("{name}: clip features {:?} vs {:?}", a.is_some(), b.is_some())),
            }
        }
    }
    r.finish();
}

#[test]
fn feats_and_differences_match() {
    let mut r = Report::new("compare");
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let (l, rr) = audio::load_stereo(&path).unwrap();
        let f = serde_json::to_value(earshot::compare::measure_feats(&l, &rr)).unwrap();
        for (k, v) in j["feats"].as_object().unwrap() {
            let tol = match k.as_str() {
                "brightness_hz" | "bandwidth_hz" => 1.0,
                "attack_ms" | "decay_ms" | "length_s" | "duration" => 0.001,
                _ => 0.1,
            };
            r.close(&format!("{name}: feats.{k}"), num(&f, k), v.as_f64(), tol);
        }
    }
    let c = cross();
    for (pair, d) in c["differences"].as_object().unwrap() {
        let fa: earshot::Feats = feats_from(&d["a"]);
        let fb: earshot::Feats = feats_from(&d["b"]);
        let (keys, txt) = earshot::compare::differences(&fa, &fb, 4);
        let want_keys: Vec<String> =
            d["result"][0].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        r.check(keys == want_keys, format!("{pair}: difference keys {keys:?} vs {want_keys:?}"));
        r.check(txt == d["result"][1].as_str().unwrap(), format!("{pair}: '{txt}' vs '{}'", d["result"][1]));
    }
    r.finish();
}

fn feats_from(v: &Value) -> earshot::Feats {
    let g = |k: &str| v.get(k).and_then(|x| x.as_f64());
    earshot::Feats {
        loudness_lufs: g("loudness_lufs"),
        peak_dbfs: g("peak_dbfs"),
        brightness_hz: g("brightness_hz"),
        bandwidth_hz: g("bandwidth_hz"),
        attack_ms: g("attack_ms"),
        decay_ms: g("decay_ms"),
        length_s: g("length_s"),
        duration: g("duration"),
        noise_floor_dbfs: g("noise_floor_dbfs"),
        width_db: g("width_db"),
        low_end_db: g("low_end_db"),
    }
}

// ------------------------------------------------------------------------------------------------ model checks
fn ears() -> &'static Ears {
    static E: OnceLock<Ears> = OnceLock::new();
    E.get_or_init(|| {
        let t = std::time::Instant::now();
        let e = Ears::load(EarsOptions::default()).expect("load models");
        println!("Ears::load {:.2} s", t.elapsed().as_secs_f64());
        e
    })
}

#[test]
#[ignore]
fn text_embeddings_match() {
    let e = ears();
    let mut r = Report::new("text");
    for (ph, v) in common()["phrases"].as_object().unwrap() {
        let toks = e.clap.tokenize(ph).unwrap();
        let want: Vec<i64> = v["tokens"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
        r.check(toks == want, format!("'{ph}': tokens {toks:?}"));
        let raw = e.embed_text_raw(&[ph]).unwrap().remove(0);
        let tpl = e.embed_text(&[ph]).unwrap().remove(0);
        let to64 = |v: &[f32]| v.iter().map(|x| *x as f64).collect::<Vec<_>>();
        let c1 = cosine(&to64(&raw), &f64s(&v["raw"]));
        let c2 = cosine(&to64(&tpl), &f64s(&v["templated"]));
        r.check(c1 >= 0.999, format!("'{ph}': raw text embedding cosine {c1:.7} (1 - cos {:.1e})", 1.0 - c1));
        r.check(c2 >= 0.999, format!("'{ph}': templated text embedding cosine {c2:.7} (1 - cos {:.1e})", 1.0 - c2));
        let (k, s) = e.query_kind(ph).unwrap();
        r.check(
            k == v["kind"][0].as_str().unwrap(),
            format!("'{ph}': query kind {k} {s} vs {} {}", v["kind"][0], v["kind"][1]),
        );
    }
    r.finish();
}

#[test]
#[ignore]
fn audio_embeddings_and_calibration_match() {
    let e = ears();
    let mut r = Report::new("clap");
    let mut worst_cos = 1.0f64;
    let mut worst_z = 0.0f64;
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let m = audio::load_mono(&path).unwrap();
        let t = std::time::Instant::now();
        let a = e.embed(&m).unwrap();
        let el = t.elapsed().as_secs_f64();
        let c = cosine(&a.iter().map(|x| *x as f64).collect::<Vec<_>>(), &f64s(&j["embedding"]));
        worst_cos = worst_cos.min(c);
        r.check(
            c >= 0.999,
            format!(
                "{name}: audio embedding cosine {c:.7} (1 - cos {:.1e}; {} windows, {el:.2} s)",
                1.0 - c,
                j["n_spans"]
            ),
        );
        let qs: Vec<&str> = j["scores"].as_array().unwrap().iter().map(|s| s["query"].as_str().unwrap()).collect();
        let sc = e.scores(&a, &qs).unwrap();
        for (s, p) in sc.iter().zip(j["scores"].as_array().unwrap()) {
            let dz = (s.z - p["z"].as_f64().unwrap()).abs();
            worst_z = worst_z.max(dz);
            r.check(
                dz <= 0.05,
                format!(
                    "{name}: '{}' z {} vs {} (|d| {dz:.3}), pct {} vs {}, rank {} vs {}",
                    s.query, s.z, p["z"], s.percentile, p["percentile"], s.rank_in_vocab, p["rank_in_vocab"]
                ),
            );
        }
        let mm = e.match_query(&a, "a kick drum", &["a snare drum hit", "a hi-hat"]).unwrap();
        let pm = &j["match"][0];
        r.check(
            mm.verdict == pm["verdict"].as_str().unwrap() || (mm.z - pm["z"].as_f64().unwrap()).abs() <= 0.05,
            format!("{name}: match verdict {} (z {}) vs {} (z {})", mm.verdict, mm.z, pm["verdict"], pm["z"]),
        );
        r.close(&format!("{name}: p_vs_alts"), mm.p_vs_alts, opt(&pm["p_vs_alts"]), 0.02);
        r.check(mm.validated == pm["validated"].as_bool().unwrap(), format!("{name}: validated {}", mm.validated));
        let multi = e.match_queries(&a, &["house music", "techno music"], &[]).unwrap();
        for pr in j["match_multi"].as_array().unwrap() {
            let q = pr["query"].as_str().unwrap();
            let ours = multi.iter().find(|x| x.query == q).unwrap();
            r.close(&format!("{name}: '{q}' forced choice"), ours.p_vs_alts, opt(&pr["p_vs_alts"]), 0.02);
            r.check(
                ours.validated == pr["validated"].as_bool().unwrap(),
                format!("{name}: '{q}' validated {} (unvalidated genre)", ours.validated),
            );
        }
        let tt = e.top_terms(&a, 8, &earshot::matching::SKIP_CATS);
        let want: Vec<&str> = j["top_terms"].as_array().unwrap().iter().map(|t| t[0].as_str().unwrap()).collect();
        let got: Vec<&str> = tt.iter().map(|t| t.term.as_str()).collect();
        let overlap = got.iter().take(5).filter(|t| want.iter().take(5).any(|w| w == *t)).count();
        r.check(overlap >= 4, format!("{name}: top terms {got:?} vs {want:?} (top-5 overlap {overlap})"));
    }
    println!(
        "worst audio embedding cosine {worst_cos:.7} (1 - cos {:.1e}), worst |z diff| {worst_z:.3}",
        1.0 - worst_cos
    );
    r.finish();
}

#[test]
#[ignore]
fn realness_matches() {
    let e = ears();
    let mut r = Report::new("realness");
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let m = audio::load_mono(&path).unwrap();
        let t = std::time::Instant::now();
        let ours = e.realness_named(&m, None, &path.file_name().unwrap().to_string_lossy()).unwrap();
        let py = &j["realness"];
        r.close(&format!("{name}: p_real ({:.2} s)", t.elapsed().as_secs_f64()), ours.p_real, opt(&py["p_real"]), 0.02);
        if m.len() as f64 <= 6.0 * 48000.0 && ours.gain_db.is_none() {
            // a precomputed embed() result is reused exactly where it equals what realness computes
            let emb = e.embed(&m).unwrap();
            let again = e.realness_named(&m, Some(&emb), &path.file_name().unwrap().to_string_lossy()).unwrap();
            r.close(&format!("{name}: p_real with a reused embedding"), again.p_real, ours.p_real, 1e-6);
        }
        r.check(
            ours.verdict == py["verdict"].as_str().unwrap(),
            format!("{name}: verdict '{}' vs {}", ours.verdict, py["verdict"]),
        );
        if let Some(segs) = py["segments"].as_array() {
            let os = ours.segments.clone().unwrap_or_default();
            r.check(os.len() == segs.len(), format!("{name}: {} segments vs {}", os.len(), segs.len()));
            for (a, b) in os.iter().zip(segs) {
                r.close(&format!("{name}: segment {} p_real", a.start), a.p_real, opt(&b["p_real"]), 0.02);
            }
        }
        if ours.summary != py["summary"].as_str().unwrap() {
            println!("  note summary differs:\n    rust: {}\n    py:   {}", ours.summary, py["summary"]);
        }
    }
    r.finish();
}

#[test]
#[ignore]
fn audioset_matches() {
    let e = ears();
    let mut r = Report::new("audioset");
    let mut worst = 0.0f64;
    for (j, path) in fixtures() {
        let name = j["name"].as_str().unwrap();
        let m = audio::load_mono(&path).unwrap();
        let t = std::time::Instant::now();
        let p = e.audioset(&m).unwrap();
        let el = t.elapsed().as_secs_f64();
        let want = f64s(&j["audioset"]);
        let d = p.iter().zip(&want).map(|(a, b)| (*a as f64 - b).abs()).fold(0.0, f64::max);
        worst = worst.max(d);
        let top = e.ast.as_ref().unwrap().top(&p, 3, 0.0);
        r.check(
            d <= 0.02,
            format!("{name}: AudioSet max |d| {d:.1e} ({el:.2} s); top {top:?} vs {}", j["audioset_top10"][0]),
        );
    }
    println!("worst AudioSet max abs diff {worst:.1e}");
    r.finish();
}

#[test]
#[ignore]
fn typicality_rank_find_describe_match() {
    let e = ears();
    let mut r = Report::new("search");
    let c = cross();
    let fx = fixtures();
    let names: Vec<String> = c["names"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    if fx.len() == names.len() {
        let embs: Vec<Vec<f32>> = fx.iter().map(|(_, p)| e.embed(&audio::load_mono(p).unwrap()).unwrap()).collect();
        // fixtures() is sorted by file name; cross['names'] is in generation order
        for (i, (j, _)) in fx.iter().enumerate() {
            let n = j["name"].as_str().unwrap();
            let refs: Vec<Vec<f32>> =
                embs.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, v)| v.clone()).collect();
            let t = earshot::compare::typicality_raw(&embs[i], &refs, 3);
            let p = &c["typicality"][n];
            r.close(&format!("{n}: typicality z"), Some(t.1), opt(&p["z"]), 0.15);
            r.close(&format!("{n}: mean_cos_knn"), Some(t.0), opt(&p["mean_cos_knn"]), 0.003);
        }
    }
    // rank of the synthetic media
    let media: Vec<PathBuf> = vec![crate_dir().join("parity/media")];
    let rk = earshot::describe::rank_files(e, &media, "a kick drum", None).unwrap();
    let want: Vec<&str> =
        c["rank"]["results"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    let got: Vec<&str> = rk.results.iter().map(|x| x.name.as_str()).collect();
    r.check(got.first() == want.first(), format!("rank: {got:?} vs {want:?}"));
    for (a, b) in rk.results.iter().zip(c["rank"]["results"].as_array().unwrap()) {
        if let Some(bb) = c["rank"]["results"].as_array().unwrap().iter().find(|x| x["name"].as_str() == Some(&a.name))
        {
            r.close(&format!("rank: {} z", a.name), Some(a.z), opt(&bb["z"]), 0.05);
        }
        let _ = b;
    }
    for (j, path) in &fx {
        let name = j["name"].as_str().unwrap();
        if let Some(f) = j.get("find") {
            let q = j["find_query"].as_str().unwrap();
            let t = std::time::Instant::now();
            let ours = e.find_file(path, q, 5, &Default::default()).unwrap();
            println!(
                "{name}: find '{q}' {:.2} s, {} candidates (py {})",
                t.elapsed().as_secs_f64(),
                ours.n_candidates,
                f["n_candidates"]
            );
            let ps = f["segments"].as_array().unwrap();
            r.check(
                ours.query_kind == f["query_kind"].as_str().unwrap(),
                format!("{name}: query kind {} vs {}", ours.query_kind, f["query_kind"]),
            );
            let best_r = ours.segments.first().map(|s| (s.start, s.end, s.z));
            let best_p = ps
                .first()
                .map(|s| (s["start"].as_f64().unwrap(), s["end"].as_f64().unwrap(), s["z"].as_f64().unwrap()));
            println!("  best rust {best_r:?} py {best_p:?}");
            let matched = ps
                .iter()
                .filter(|p| {
                    ours.segments.iter().any(|s| {
                        (s.start - p["start"].as_f64().unwrap()).abs() < 0.03
                            && (s.end - p["end"].as_f64().unwrap()).abs() < 0.03
                    })
                })
                .count();
            r.check(
                matched >= ps.len().saturating_sub(2),
                format!("{name}: find trims matched {matched} of {}", ps.len()),
            );
            for s in &ours.segments {
                println!("    rust {:.3}-{:.3} z {} {} {}", s.start, s.end, s.z, s.kind, s.verdict);
            }
            for p in ps {
                println!(
                    "    py   {:.3}-{:.3} z {} {} {}",
                    p["start"].as_f64().unwrap(),
                    p["end"].as_f64().unwrap(),
                    p["z"],
                    p["kind"],
                    p["verdict"]
                );
            }
        }
        if let Some(d) = j.get("describe") {
            let ours = e.describe(path).unwrap();
            println!("{name}: describe\n    rust: {}\n    py:   {}", ours.summary, d["summary"]);
            let want: Vec<&str> =
                d["top_tags"].as_array().unwrap().iter().take(3).map(|t| t["term"].as_str().unwrap()).collect();
            let got: Vec<&str> = ours.top_tags.iter().take(3).map(|t| t.term.as_str()).collect();
            r.check(got.first() == want.first(), format!("{name}: describe top tag {got:?} vs {want:?}"));
        }
    }
    r.finish();
}
