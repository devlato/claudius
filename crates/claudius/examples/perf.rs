//! The workloads whose instructions CI counts (`cargo xtask perf`): the engine's main steps, each on fixed input and
//! run once. `perf <workload>` runs its setup and then it; `perf <workload> --setup` runs the setup alone, so that what
//! the workload costs is the difference between the two. `perf --list` names them. Each prints the wall time the
//! workload took, for a look on a machine without Valgrind.

use std::hint::black_box;
use std::time::Instant;

use claudius::audio::Stereo;
use claudius::compose::{self, Options};
use claudius::config::{SR, SRF};
use claudius::{audio, genres, judge, render, scratch, script};

const EDIT: &str = r#"tempo(126); key("F minor"); drum("kick", "x...x...x...x..x"); mix("pad", #{ level: -4 });"#;

type Workload = fn(bool);

const WORKLOADS: &[(&str, Workload)] = &[
    ("generate", generate),
    ("arrange", arrange),
    ("render", render_bars),
    ("judge", judge_bars),
    ("script", script_edit),
    ("encode", encode),
    ("decode", decode),
    ("scratch", scratch_record),
    ("harmonic", harmonic),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(name) = args.first().filter(|a| *a != "--list") else {
        for (name, _) in WORKLOADS {
            println!("{name}");
        }
        return;
    };
    let Some((_, run)) = WORKLOADS.iter().find(|(n, _)| n == name) else {
        eprintln!("no workload {name}: `perf --list` names them");
        std::process::exit(2);
    };
    run(args.iter().any(|a| a == "--setup"));
}

/// runs `work` unless only the setup was asked for, and says how long it took
fn timed<T>(setup_only: bool, work: impl FnOnce() -> T) {
    if setup_only {
        return;
    }
    let t = Instant::now();
    black_box(work());
    eprintln!("perf: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
}

/// a house song, short, as the tests make one
fn song() -> claudius::model::Blueprint {
    let g = genres::get("house").expect("house");
    compose::generate(&g, 1, &Options { short: true, ..Default::default() }, None)
}

/// a chord, held, `secs` long, with a click on each beat at 120 BPM: something with both a pitch and hits
fn tones(secs: f64) -> Stereo {
    let n = (secs * SRF) as usize;
    let l: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / SRF;
            let chord: f64 = [220.0, 277.18, 329.63].iter().map(|f| (std::f64::consts::TAU * f * t).sin()).sum();
            let click = if (t * 2.0).fract() < 0.004 { 0.5 } else { 0.0 };
            (0.2 * chord + click) as f32
        })
        .collect();
    Stereo { l: l.clone(), r: l }
}

fn generate(setup_only: bool) {
    let house = genres::get("house").expect("house");
    let layered = genres::get("alt_hip_hop").expect("alt_hip_hop");
    timed(setup_only, || {
        (
            compose::generate(&house, 1, &Options::default(), None),
            compose::generate(&layered, 2, &Options { complexity: Some(1.0), ..Default::default() }, None),
        )
    });
}

fn arrange(setup_only: bool) {
    let g = genres::get("house").expect("house");
    let bp = compose::generate(&g, 1, &Options::default(), None);
    timed(setup_only, || compose::arrange(&bp));
}

fn render_bars(setup_only: bool) {
    let four = compose::arrange(&song()).slice(0, 4);
    timed(setup_only, || render::render(&four, &render::Options::default()).expect("render"));
}

fn judge_bars(setup_only: bool) {
    let four = compose::arrange(&song()).slice(0, 4);
    let r = render::render(&four, &render::Options::default()).expect("render");
    timed(setup_only, || judge::judge(&four, &r, None));
}

fn script_edit(setup_only: bool) {
    let bp = song();
    timed(setup_only, || {
        let edited = script::run(EDIT, Some(bp.clone()), None).expect("edit").bp.expect("a song");
        script::to_script(&edited)
    });
}

/// eight seconds as an MP3 at 192 kbps
fn mp3(x: &Stereo) -> Vec<u8> {
    let samples = x.interleaved();
    let spec = claudius_encode::Spec { format: claudius_encode::Format::Mp3(192), rate: None };
    let pcm = claudius_encode::Pcm { samples: &samples, channels: 2, rate: SR };
    claudius_encode::encode(&spec, pcm, &claudius_encode::Tags::default()).expect("encode")
}

fn encode(setup_only: bool) {
    let x = tones(8.0);
    timed(setup_only, || mp3(&x));
}

fn decode(setup_only: bool) {
    let bytes = mp3(&tones(8.0));
    timed(setup_only, || audio::load_bytes(bytes, "mp3").expect("decode"));
}

fn scratch_record(setup_only: bool) {
    let record = tones(4.0);
    // a baby scratch: the record pushed forward and pulled back twice a beat for two seconds, the fader open
    let moves: Vec<scratch::Move> = (0..=200)
        .map(|i| {
            let t = i as f64 / 100.0;
            scratch::Move { t, at: 0.5 + 0.15 * (std::f64::consts::TAU * 2.0 * t).sin(), fader: 1.0 }
        })
        .collect();
    timed(setup_only, || scratch::render(&record, &moves).expect("scratch"));
}

fn harmonic(setup_only: bool) {
    let x = tones(6.0);
    timed(setup_only, || claudius::dsp::fft::harmonic(&x.l));
}
