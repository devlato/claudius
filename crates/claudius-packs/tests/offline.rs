//! The public API end to end against a pack written to a temporary directory (no network).

use std::path::Path;
use std::sync::Arc;

use claudius_packs::{NoteOn, SR};

fn write_sine(path: &Path, freq: f64, secs: f64, amp: f64, sr: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec =
        hound::WavSpec { channels: 2, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let n = (secs * sr as f64) as usize;
    for i in 0..n {
        let t = i as f64 / sr as f64;
        let env = (1.0 - t / secs).max(0.0);
        let v = ((2.0 * std::f64::consts::PI * freq * t).sin() * amp * env * 32767.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v / 2).unwrap();
    }
    w.finalize().unwrap();
}

fn freq_of(x: &[f32]) -> f64 {
    let mut c = Vec::new();
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            c.push(i);
        }
    }
    (c.len() - 1) as f64 * SR as f64 / (c[c.len() - 1] - c[0]) as f64
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

#[test]
fn offline_pack_api() {
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: this test binary has a single test, so nothing else reads the environment concurrently
    unsafe { std::env::set_var("CLAUDIUS_PACKS", dir.path()) };
    claudius_packs::refresh();
    let pack = dir.path().join("testpack");
    // C4 and C5 recordings at 44.1 kHz in two velocity layers (the loud layer is louder), two kick variants
    write_sine(&pack.join("files/c4_soft.wav"), 261.6256, 1.0, 0.2, 44100);
    write_sine(&pack.join("files/c4_loud.wav"), 261.6256, 1.0, 0.8, 44100);
    write_sine(&pack.join("files/c5_soft.wav"), 523.2511, 1.0, 0.2, 44100);
    write_sine(&pack.join("files/c5_loud.wav"), 523.2511, 1.0, 0.8, 44100);
    write_sine(&pack.join("files/kick1.wav"), 60.0, 0.3, 0.9, 48000);
    write_sine(&pack.join("files/kick2.wav"), 80.0, 0.3, 0.9, 48000);
    let index = serde_json::json!({
        "format": 1,
        "name": "testpack",
        "title": "Test pack",
        "description": "synthetic",
        "categories": ["test"],
        "licence": "CC-BY-4.0",
        "licence_url": "https://creativecommons.org/licenses/by/4.0/",
        "attribution": "Test sines by Nobody (CC BY 4.0)",
        "attribution_required": true,
        "source": "local",
        "version": "1",
        "user": true,
        "aliases": ["tp"],
        "files": [
            {"path": "files/c4_soft.wav"}, {"path": "files/c4_loud.wav"},
            {"path": "files/c5_soft.wav"}, {"path": "files/c5_loud.wav"},
            {"path": "files/kick1.wav"}, {"path": "files/kick2.wav"}
        ],
        "instruments": [
            {"name": "sine", "category": "synth", "kind": "pitched", "release": 0.05, "veltrack": 0.0, "variants": [{"zones": [
                {"file": 0, "root": 60.0, "hikey": 66, "hivel": 63},
                {"file": 1, "root": 60.0, "hikey": 66, "lovel": 64},
                {"file": 2, "root": 72.0, "lokey": 67, "hivel": 63},
                {"file": 3, "root": 72.0, "lokey": 67, "lovel": 64}
            ]}]},
            {"name": "bd", "category": "drums", "kind": "drum", "one_shot": true, "release": 0.05, "mono": true, "variants": [
                {"zones": [{"file": 4, "root": 60.0}]},
                {"zones": [{"file": 5, "root": 60.0}]}
            ]}
        ]
    });
    std::fs::write(pack.join("index.json"), serde_json::to_vec_pretty(&index).unwrap()).unwrap();

    let packs = claudius_packs::packs();
    let tp = packs.iter().find(|p| p.name == "testpack").expect("listed");
    assert!(tp.installed && tp.user && tp.attribution_required);
    assert_eq!(tp.instruments, 2);
    // curated packs are listed too, not installed here
    assert!(packs.iter().any(|p| p.name == "salamander" && !p.installed && p.default));
    assert!(packs.iter().any(|p| p.name == "tidal-drum-machines" && p.opt_in && p.warning.is_some()));

    let infos = claudius_packs::instruments();
    assert_eq!(infos.len(), 2);
    let sine_info = infos.iter().find(|i| i.instrument == "sine").unwrap();
    assert_eq!((sine_info.lowest, sine_info.highest, sine_info.velocity_layers), (60, 72, 2));
    let bd_info = infos.iter().find(|i| i.instrument == "bd").unwrap();
    assert_eq!((bd_info.kind.as_str(), bd_info.variants, bd_info.one_shot), ("drum", 2, true));

    // names
    let r = claudius_packs::resolve("sine", None).unwrap();
    assert_eq!((r.pack.as_str(), r.instrument.as_str(), r.variant), ("testpack", "sine", 0));
    assert_eq!(claudius_packs::resolve("bd", Some("tp")).unwrap().instrument, "bd");
    assert_eq!(claudius_packs::resolve("testpack:kick:1", None).unwrap().variant, 1);
    assert_eq!(claudius_packs::resolve("kick", None).unwrap().pack, "testpack");
    assert!(claudius_packs::resolve("testpack:bd:5", None).is_none());
    assert!(claudius_packs::instrument("cello", None).is_err());

    // playback: pitch from either zone, 44.1 kHz source folded into the 48 kHz output, velocity layers
    let sine = claudius_packs::instrument("sine", None).unwrap();
    assert!(Arc::ptr_eq(&sine, &claudius_packs::instrument("testpack:sine", None).unwrap()));
    let len = SR as usize;
    let e4 = NoteOn { start: 0, gate: len / 2, pitch: 64.0, velocity: 1.0 };
    let (l, r) = sine.render(&[e4], len);
    let f = freq_of(&l[2000..20000]);
    assert!((f - 329.63).abs() < 0.5, "E4 rendered at {f} Hz");
    assert!(peak(&r) < peak(&l) * 0.6, "stereo kept (right channel recorded at half level)");
    let g5 = NoteOn { pitch: 79.0, ..e4 };
    let (l, _) = sine.render(&[g5], len);
    assert!((freq_of(&l[2000..20000]) - 783.99).abs() < 1.0);
    // veltrack 0: loudness comes from the layers alone; the soft layer is 12 dB down
    let (loud, _) = sine.render(&[e4], len);
    let (soft, _) = sine.render(&[NoteOn { velocity: 0.3, ..e4 }], len);
    let ratio = peak(&soft) / peak(&loud);
    assert!((ratio - 0.25).abs() < 0.03, "layer ratio {ratio}");
    // gate + release: silent after 0.5 s + 50 ms
    assert!(loud[len / 2 + 2400 + 10..].iter().all(|v| v.abs() < 1e-6));

    // drum variants, mono retrigger, chokes, deterministic and identical across threads
    let k1 = claudius_packs::instrument("bd", Some("testpack")).unwrap();
    let k2 = claudius_packs::instrument("testpack:bd:1", None).unwrap();
    let hit = [NoteOn { start: 100, gate: 1, pitch: 60.0, velocity: 0.9 }];
    let (a, _) = k1.render(&hit, 20000);
    let (b, _) = k2.render(&hit, 20000);
    assert!((freq_of(&a[1000..12000]) - 60.0).abs() < 1.0);
    assert!((freq_of(&b[1000..12000]) - 80.0).abs() < 1.0);
    let pattern: Vec<NoteOn> =
        (0..16).map(|i| NoteOn { start: i * 6000, gate: 10, pitch: 60.0, velocity: 0.5 + 0.03 * i as f32 }).collect();
    let reference = k1.render(&pattern, 100_000);
    let outs: Vec<(Vec<f32>, Vec<f32>)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..4).map(|_| s.spawn(|| k1.render(&pattern, 100_000))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(outs.iter().all(|o| *o == reference));

    // credits list the CC BY pack
    let c = claudius_packs::credits();
    assert!(c.contains("Test sines by Nobody"), "{c}");
    assert!(claudius_packs::credits_for(&["bd@testpack"]).contains("Test sines"));

    claudius_packs::remove("testpack").unwrap();
    assert!(claudius_packs::resolve("sine", None).is_none());
    assert!(!dir.path().join("testpack").exists());
}
