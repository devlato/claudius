//! A song saved as a project (`song::pack_project`, what the demo downloads) opens in claudius
//! (`song::unpack_project`) as a song folder of its own, which plays the samples it holds; a zip that is no project, or
//! that would write outside its folder, is not opened.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Once;

use claudius::audio::Stereo;
use claudius::compose::{self, Options};
use claudius::model::{Blueprint, Sound};
use claudius::{export, genres, render, song};

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

fn scratch(what: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("claudius-project-{what}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// a tenth of a second of a tone, as FLAC
fn tone() -> Vec<u8> {
    let n = claudius::config::SR as usize / 10;
    let x: Vec<f32> = (0..n).map(|i| (i as f32 * 0.06).sin() * 0.5).collect();
    export::encode(&Stereo::from_mono(&x), &"flac:24".parse().unwrap(), &export::tags("tone")).unwrap()
}

/// a house song whose first drum plays a sample the project holds
fn sampled() -> Blueprint {
    without_installed_packs();
    let mut bp = compose::generate(&genres::get("house").unwrap(), 3, &Options::default(), None);
    bp.drums[0].sound =
        Sound::Sample { path: "samples/drums/kick.flac".into(), root: 60, start: 0.0, end: None, label: "kick".into() };
    bp
}

/// a zip of `entries`, deflated as Finder and Explorer zip a folder
fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_project_opens_as_a_song_folder_that_plays_the_samples_it_holds() {
    let bp = sampled();
    let about = br#"{"page": "https://freesound.org/s/1/", "licence": "CC0"}"#;
    let files = [
        ("samples/drums/kick.flac".to_string(), tone()),
        ("samples/drums/kick.flac.json".to_string(), about.to_vec()),
        ("credits.txt".to_string(), b"kick by someone (CC0)\n".to_vec()),
    ];
    let zip = song::pack_project(&bp, &files).unwrap();
    let songs = scratch("open");

    let dir = song::unpack_project(&zip, "My Song.zip", &songs).unwrap();
    assert_eq!(dir, songs.join("my-song"));
    for file in ["blueprint.json", "song.rhai", "credits.txt", "samples/drums/kick.flac.json"] {
        assert!(dir.join(file).is_file(), "{file} is in the folder");
    }
    let opened = song::read_blueprint(&dir.join("blueprint.json")).unwrap();
    let kick = dir.join("samples/drums/kick.flac").display().to_string();
    assert!(matches!(&opened.drums[0].sound, Sound::Sample { path, .. } if *path == kick), "the sample is found");
    let p = compose::arrange(&opened);
    let r = render::render(&p.slice(0, 2), &render::Options { stems: false, tail_secs: 0.0 }).unwrap();
    let lane = r.tracks.iter().find(|t| t.name == opened.drums[0].name).expect("the drum's track");
    assert!(lane.level_lufs.is_some(), "the drum plays the project's sample");

    let again = song::unpack_project(&zip, "My Song.zip", &songs).unwrap();
    assert_eq!(again, songs.join("my-song-2"), "a project opened twice is two folders");

    // unpacked and zipped again by hand, as Finder zips a folder: deflated, under a folder of its name, with what
    // Finder keeps of the files' attributes beside it
    let read = |f: &str| std::fs::read(dir.join(f)).unwrap();
    let (blueprint, sample) = (read("blueprint.json"), read("samples/drums/kick.flac"));
    let rezipped = zip_of(&[
        ("my-song/blueprint.json", &blueprint),
        ("my-song/samples/drums/kick.flac", &sample),
        ("__MACOSX/my-song/._blueprint.json", b"\x00\x05\x16\x07"),
    ]);
    let third = song::unpack_project(&rezipped, "my-song.zip", &songs).unwrap();
    assert!(!third.join("my-song").exists() && !third.join("__MACOSX").exists(), "the project, not its folder");
    let reopened = song::read_blueprint(&third.join("blueprint.json")).unwrap();
    let kick = third.join("samples/drums/kick.flac").display().to_string();
    assert!(matches!(&reopened.drums[0].sound, Sound::Sample { path, .. } if *path == kick), "a folder zipped opens");
}

#[test]
fn a_zip_that_is_no_project_or_writes_outside_its_folder_is_not_opened() {
    let songs = scratch("refuse");
    let bp = br#"{}"#;

    let outside = zip_of(&[("blueprint.json", bp), ("../outside.txt", b"x")]);
    let e = song::unpack_project(&outside, "evil.zip", &songs).unwrap_err().to_string();
    assert!(e.contains("outside itself"), "{e}");
    assert!(!songs.join("../outside.txt").exists() && !songs.join("evil").exists(), "nothing is written");

    let script = zip_of(&[("song.rhai", b"song()")]);
    let e = song::unpack_project(&script, "script.zip", &songs).unwrap_err().to_string();
    assert!(e.contains("no blueprint.json"), "{e}");

    let e = song::unpack_project(b"not a zip", "song.zip", &songs).unwrap_err().to_string();
    assert!(e.contains("is not a zip"), "{e}");

    let names: Vec<String> = (0..10_000).map(|i| format!("samples/{i}.flac")).collect();
    let many: Vec<(&str, &[u8])> =
        std::iter::once(("blueprint.json", &bp[..])).chain(names.iter().map(|n| (n.as_str(), &b"x"[..]))).collect();
    let e = song::unpack_project(&zip_of(&many), "many.zip", &songs).unwrap_err().to_string();
    assert!(e.contains("10001 files, more than a project does"), "{e}");
    assert!(!songs.join("many").exists(), "nothing is written");
}
