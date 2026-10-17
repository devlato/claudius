//! Real downloads (run with `cargo test -p claudius-packs -- --ignored`).

use claudius_packs::{InstallOptions, NoteOn, SR};

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

/// installs a small curated pack and a Strudel map into a temporary directory and renders with both
#[test]
#[ignore]
fn install_small_pack_and_strudel_map() {
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: this is the only test in this binary
    unsafe { std::env::set_var("CLAUDIUS_PACKS", dir.path()) };
    claudius_packs::refresh();

    claudius_packs::install("tr707", &|_, _| {}).unwrap();
    let info = claudius_packs::packs().into_iter().find(|p| p.name == "tr707").unwrap();
    assert!(info.installed && info.size_mb > 0.1);
    let bd = claudius_packs::instrument("bd", Some("707")).unwrap();
    let notes: Vec<NoteOn> =
        (0..4).map(|i| NoteOn { start: i * SR as usize / 2, gate: 100, pitch: 60.0, velocity: 0.9 }).collect();
    let (l, r) = bd.render(&notes, 2 * SR as usize);
    assert!(peak(&l) > 0.1 && peak(&r) > 0.1);

    // opt-in packs refuse to install without consent
    assert!(claudius_packs::install("tidal-drum-machines", &|_, _| {}).is_err());

    // a Strudel sample map from the github: shorthand, fetched lazily on first play
    let p = claudius_packs::add_sample_map("github:tidalcycles/uzu-drumkit").unwrap();
    assert_eq!(p.licence, "Unlicense");
    assert!(p.lazy && p.user);
    let r = claudius_packs::resolve("tidalcycles-uzu-drumkit:bd:1", None).unwrap();
    let kick = claudius_packs::instrument_ref(&r).unwrap();
    let (l, _) = kick.render(&notes[..1], SR as usize);
    assert!(peak(&l) > 0.1);

    // a lazy curated install renders too (piano-lite: Strudel's piano map)
    let opts = InstallOptions { lazy: true, ..Default::default() };
    claudius_packs::install_with("piano-lite", &opts, &|_, _| {}).unwrap();
    let piano = claudius_packs::instrument("piano-lite:piano", None).unwrap();
    let (l, _) = piano.render(&[NoteOn { start: 0, gate: SR as usize / 2, pitch: 64.0, velocity: 0.8 }], SR as usize);
    assert!(peak(&l) > 0.05);
    assert!(claudius_packs::credits().contains("Alexander Holm"));
}
