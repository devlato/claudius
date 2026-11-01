//! The sounds of the web studio's demo, made by claudius itself: every synth preset at a note every half octave
//! across its register, the TR-707 and TR-808 drum machines, and the transition effects, as Ogg Vorbis files with a
//! `kit.json` that says what each is. The demo plays them instead of its own rough synth.
//!
//! `cargo xtask site --sounds` runs it into the site. The drum machines come from their packs (public domain and
//! CC0, so the site may pass them on), installed into `$CLAUDIUS_PACKS`, which the xtask points at a folder of its
//! own.
//!
//! cargo run -p claudius --no-default-features --example demo_sounds -- <out dir>

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use claudius::audio::Stereo;
use claudius::export::{self, Spec};
use claudius::synth::drums::{self, DrumParams, DrumVoice};
use claudius::synth::{fxsounds, presets, voice};
use serde::Serialize;

const SR: f64 = 48_000.0;
/// Vorbis quality: about 110 kbps for stereo, transparent for these
const QUALITY: &str = "ogg:3";
/// every preset is levelled to this RMS while its note is held (about -20 dBFS), unless that would clip
const PRESET_RMS: f32 = 0.1;
const CEILING: f32 = 0.97;
/// drums and effects peak here
const DRUM_PEAK: f32 = 0.89;

/// where a lane's sound comes from: the drum machine's own sample, or claudius' drum synth
enum Hit {
    Machine(&'static str),
    Synth(DrumVoice),
}

/// a drum machine as the demo plays it: its name there, its pack, and where each lane's sound comes from
struct Machine {
    kit: &'static str,
    pack: &'static str,
    lanes: &'static [(&'static str, Hit)],
}

const KITS: &[Machine] = &[
    Machine {
        kit: "707",
        pack: "tr707",
        lanes: &[
            // the 707's own kick lasts a tenth of a second: a click under a house track, where claudius' has body
            ("kick", Hit::Synth(DrumVoice::Kick)),
            ("snare", Hit::Machine("sd")),
            ("clap", Hit::Machine("cp")),
            ("hat", Hit::Machine("hh")),
            ("ohat", Hit::Machine("oh")),
            ("ride", Hit::Machine("rd")),
            ("crash", Hit::Machine("cr")),
            ("rim", Hit::Machine("rim")),
            ("tom", Hit::Machine("mt")),
            ("cowbell", Hit::Machine("cb")),
        ],
    },
    Machine {
        kit: "808",
        pack: "tr808",
        lanes: &[
            ("kick", Hit::Machine("bd")),
            ("snare", Hit::Machine("sd")),
            ("clap", Hit::Machine("cp")),
            ("hat", Hit::Machine("hh")),
            ("ohat", Hit::Machine("oh")),
            ("shaker", Hit::Machine("sh")),
            ("crash", Hit::Machine("cr")),
            ("rim", Hit::Machine("rim")),
            ("tom", Hit::Machine("mt")),
            ("cowbell", Hit::Machine("cb")),
            // the demo's percussion lane: the low conga, a soft round knock under the hats
            ("perc", Hit::Machine("lc")),
        ],
    },
];

/// the times `File::energy` is taken at, in seconds
const TIMES: [f64; 9] = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

#[derive(Serialize)]
struct File {
    /// the note it plays
    note: i32,
    file: String,
    secs: f32,
    /// its K-weighted energy (mean square times seconds) from its start to each of `TIMES`: how loud a note of any
    /// length is, for the demo to set each track's level as claudius' mix does
    energy: Vec<f32>,
}

#[derive(Serialize)]
struct Sound {
    /// played at any note, resampled from the nearest of `samples`; otherwise played as it is
    pitched: bool,
    /// how long the sound takes to die away once its note ends, in seconds
    release: f32,
    samples: Vec<File>,
}

#[derive(Serialize)]
struct Kit {
    version: u32,
    rate: u32,
    credits: Vec<String>,
    sounds: BTreeMap<String, Sound>,
}

/// the notes a preset is rendered at: every six semitones across the register of its roles
fn notes(roles: &[&str]) -> Vec<i32> {
    match (roles.contains(&"bass"), roles.len()) {
        (true, 1) => (24..=54).step_by(6).collect(),
        (true, _) => (30..=72).step_by(6).collect(),
        _ => (42..=90).step_by(6).collect(),
    }
}

/// how long a preset's notes are held: long enough for the longest note a demo song gives that role
fn held(name: &str, roles: &[&str]) -> f64 {
    match name {
        "pad" | "warm_pad" | "strings" | "organ" => 4.5,
        _ if roles.contains(&"bass") => 3.5,
        _ => 2.5,
    }
}

/// the sound without the silence after it ends (under -60 dB), faded over its last 5 ms
fn trimmed(mut x: Stereo) -> Stereo {
    let heard = |i: &usize| x.l[*i].abs().max(x.r[*i].abs()) > 0.001;
    let end = (0..x.len()).rev().find(heard).map_or(0, |i| (i + 1 + (0.01 * SR) as usize).min(x.len()));
    x.l.truncate(end);
    x.r.truncate(end);
    let fade = ((0.005 * SR) as usize).min(end);
    for k in 0..fade {
        let g = k as f32 / fade as f32;
        x.l[end - 1 - k] *= g;
        x.r[end - 1 - k] *= g;
    }
    x
}

fn scaled(mut x: Stereo, gain: f32) -> Stereo {
    for s in x.l.iter_mut().chain(x.r.iter_mut()) {
        *s *= gain;
    }
    x
}

fn peak(x: &Stereo) -> f32 {
    x.l.iter().chain(&x.r).fold(0.0f32, |m, s| m.max(s.abs()))
}

fn rms(x: &Stereo, len: usize) -> f32 {
    let n = len.min(x.len()).max(1);
    let sum: f32 = (0..n).map(|i| (x.l[i] * x.l[i] + x.r[i] * x.r[i]) * 0.5).sum();
    (sum / n as f32).sqrt()
}

struct Writer<'a> {
    dir: &'a Path,
    spec: Spec,
    bytes: u64,
}

impl Writer<'_> {
    /// the file, mono when `mono` (drums and basses: half the size, and half the memory in the browser)
    fn write(&mut self, name: &str, x: &Stereo, mono: bool) -> Result<String> {
        let file = format!("{name}.ogg");
        let samples: Vec<f32> =
            if mono { x.l.iter().zip(&x.r).map(|(l, r)| (l + r) * 0.5).collect() } else { x.interleaved() };
        let pcm = claudius_encode::Pcm { samples: &samples, channels: if mono { 1 } else { 2 }, rate: SR as u32 };
        let encoded = claudius_encode::encode(&self.spec, pcm, &export::Tags::default())?;
        self.bytes += encoded.len() as u64;
        std::fs::write(self.dir.join(&file), encoded).with_context(|| format!("cannot write {file}"))?;
        Ok(file)
    }
}

fn secs(x: &Stereo) -> f32 {
    x.len() as f32 / SR as f32
}

/// the file as the kit lists it, with its energy up to each of `TIMES`
fn file(note: i32, file: String, x: &Stereo) -> File {
    let energy = TIMES
        .iter()
        .map(|&t| {
            let n = ((t * SR) as usize).min(x.len()).max(1);
            let part = Stereo { l: x.l[..n].to_vec(), r: x.r[..n].to_vec() };
            let whole = n as f64 / SR;
            let lufs = claudius::loudness::blocks(&part, whole + 1.0, whole + 1.0).first().map_or(-70.0, |b| b.1);
            (10f64.powf((lufs + 0.691) / 10.0) * whole) as f32
        })
        .collect();
    File { note, file, secs: secs(x), energy }
}

fn presets(w: &mut Writer, sounds: &mut BTreeMap<String, Sound>) -> Result<()> {
    for (name, roles) in presets::ROLES {
        let patch = presets::get(name).with_context(|| format!("no preset {name}"))?;
        let gate = held(name, roles);
        let tail = (patch.aenv.r * 2.0 + 0.3).min(3.0);
        let len = ((gate + tail) * SR) as usize;
        let notes = notes(roles);
        let rendered: Vec<(i32, Stereo)> = notes
            .iter()
            .map(|&n| {
                let ev = voice::NoteEv { start: 0, gate: (gate * SR) as usize, pitch: f64::from(n), vel: 0.8 };
                (n, voice::render(&patch, &[ev], len, SR, 120.0, 0xC1A0_0D10 ^ n as u64))
            })
            .collect();
        // levelled on the note in the middle of the register, and kept under the ceiling on every note
        let middle = &rendered[rendered.len() / 2].1;
        let loudest = rendered.iter().map(|(_, x)| peak(x)).fold(0.0f32, f32::max);
        let gain = (PRESET_RMS / rms(middle, (gate * SR) as usize).max(1e-6)).min(CEILING / loudest.max(1e-6));
        let mono = *roles == ["bass"];
        let mut samples = Vec::new();
        for (n, x) in rendered {
            let x = trimmed(scaled(x, gain));
            let name = w.write(&format!("{name}-{n}"), &x, mono)?;
            samples.push(file(n, name, &x));
        }
        sounds.insert(name.to_string(), Sound { pitched: true, release: patch.aenv.r as f32, samples });
    }
    Ok(())
}

fn drums(w: &mut Writer, sounds: &mut BTreeMap<String, Sound>, credits: &mut Vec<String>) -> Result<()> {
    for Machine { kit, pack, lanes } in KITS {
        if !claudius_packs::installed().iter().any(|p| p.name == *pack) {
            eprintln!("installing {pack} into {}", claudius_packs::packs_dir().display());
            let opts = claudius_packs::InstallOptions { lazy: false, threads: 4, accept_opt_in: false };
            claudius_packs::install_with(pack, &opts, &|_, _| {})?;
            claudius_packs::refresh();
        }
        if let Some(p) = claudius_packs::packs().into_iter().find(|p| p.name == *pack) {
            // a licence's identifier (CC0-1.0); one in words is in the attribution already
            let code = !p.licence.contains(' ') && !p.attribution.contains(&p.licence);
            credits.push(if code { format!("{} ({})", p.attribution, p.licence) } else { p.attribution.clone() });
        }
        for (lane, hit) in lanes.iter() {
            let x = match hit {
                Hit::Machine(sample) => {
                    let inst = claudius_packs::instrument(sample, Some(pack))?;
                    let len = (3.0 * SR) as usize;
                    let pitch = f64::from(inst.info().lowest);
                    let (l, r) =
                        inst.render(&[claudius_packs::NoteOn { start: 0, gate: len, pitch, velocity: 0.9 }], len);
                    Stereo { l, r }
                }
                Hit::Synth(v) => drums::render(*v, &DrumParams::default_for(*v), SR, 7),
            };
            let gain = DRUM_PEAK / peak(&x).max(1e-6);
            let x = trimmed(scaled(x, gain));
            let sample = file(60, w.write(&format!("{kit}-{lane}"), &x, true)?, &x);
            sounds.insert(format!("{kit}:{lane}"), Sound { pitched: false, release: 0.0, samples: vec![sample] });
        }
    }
    Ok(())
}

fn effects(w: &mut Writer, sounds: &mut BTreeMap<String, Sound>) -> Result<()> {
    let made = [("impact", fxsounds::impact(SR, 7)), ("riser", fxsounds::riser(8.0, SR, 7, 220.0))];
    for (name, x) in made {
        let gain = DRUM_PEAK / peak(&x).max(1e-6);
        let x = trimmed(scaled(x, gain));
        let sample = file(60, w.write(&format!("fx-{name}"), &x, false)?, &x);
        sounds.insert(format!("fx:{name}"), Sound { pitched: false, release: 0.0, samples: vec![sample] });
    }
    Ok(())
}

fn main() -> Result<()> {
    let out = std::env::args().nth(1).context("usage: demo_sounds <out dir>")?;
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {out}"))?;
    let mut w = Writer { dir, spec: QUALITY.parse()?, bytes: 0 };
    let (mut sounds, mut credits) = (BTreeMap::new(), Vec::new());
    presets(&mut w, &mut sounds)?;
    drums(&mut w, &mut sounds, &mut credits)?;
    effects(&mut w, &mut sounds)?;
    let files: usize = sounds.values().map(|s| s.samples.len()).sum();
    let kit = Kit { version: 1, rate: SR as u32, credits, sounds };
    std::fs::write(dir.join("kit.json"), serde_json::to_string_pretty(&kit)?)?;
    eprintln!("demo sounds: {} sounds in {files} files, {:.1} MB, in {out}", kit.sounds.len(), w.bytes as f64 / 1e6);
    Ok(())
}
