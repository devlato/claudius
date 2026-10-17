//! Render every built-in sound (drum voices, synth presets playing a phrase, transition effects) to WAV files, so
//! they can be auditioned and judged one by one.

use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;

use crate::audio::{Stereo, save_wav};
use crate::config::SRF;
use crate::synth::drums::{self, DrumParams, DrumVoice};
use crate::synth::voice::{self, NoteEv};
use crate::synth::{fxsounds, presets};

fn phrase(role: &str, bpm: f64) -> Vec<NoteEv> {
    let spb = SRF * 60.0 / bpm;
    let ev = |beat: f64, len: f64, pitch: f64, vel: f32| NoteEv {
        start: (beat * spb) as usize,
        gate: (len * spb) as usize,
        pitch,
        vel,
    };
    match role {
        "bass" => {
            let roots = [45.0, 41.0, 48.0, 43.0];
            let mut v = Vec::new();
            for (bar, r) in roots.iter().enumerate() {
                let b = bar as f64 * 4.0;
                for (k, off) in [0.0, 0.75, 1.5, 2.5, 3.0].iter().enumerate() {
                    let p = if k == 3 { r + 12.0 } else { *r };
                    v.push(ev(b + off, 0.5, p, if k == 0 { 1.0 } else { 0.75 }));
                }
            }
            v
        }
        "lead" | "arp" => {
            let mel = [69.0, 72.0, 76.0, 74.0, 72.0, 69.0, 67.0, 69.0, 72.0, 71.0, 69.0, 64.0, 65.0, 67.0, 69.0, 69.0];
            mel.iter().enumerate().map(|(i, p)| ev(i as f64, if i % 4 == 3 { 0.9 } else { 0.45 }, *p, 0.85)).collect()
        }
        _ => {
            let chords: [[f64; 4]; 4] = [
                [57.0, 60.0, 64.0, 69.0],
                [53.0, 57.0, 60.0, 65.0],
                [55.0, 60.0, 64.0, 67.0],
                [55.0, 59.0, 62.0, 67.0],
            ];
            chords
                .iter()
                .enumerate()
                .flat_map(|(bar, c)| c.iter().map(move |p| ev(bar as f64 * 4.0, 3.8, *p, 0.8)))
                .collect()
        }
    }
}

pub fn render_all(out: &Path) -> Result<Vec<std::path::PathBuf>> {
    let bpm = 120.0;
    let mut jobs: Vec<(String, Box<dyn Fn() -> Stereo + Send + Sync>)> = Vec::new();
    for v in DrumVoice::ALL {
        jobs.push((
            format!("drums/{}.wav", v.name()),
            Box::new(move || drums::render(v, &DrumParams::default_for(v), SRF, 7)),
        ));
    }
    for name in presets::names() {
        let role = presets::ROLES.iter().find(|(n, _)| *n == name).map(|(_, r)| r[0]).unwrap_or("chords");
        jobs.push((
            format!("synth/{name}.wav"),
            Box::new(move || {
                let p = presets::get(name).unwrap();
                let notes = phrase(role, bpm);
                let len = (16.0 * SRF * 60.0 / bpm) as usize + (2.0 * SRF) as usize;
                let mut x = voice::render(&p, &notes, len, SRF, bpm, 11);
                x.normalize(-1.0);
                x
            }),
        ));
    }
    jobs.push(("fx/riser.wav".into(), Box::new(|| fxsounds::riser(8.0, SRF, 3, 110.0))));
    jobs.push(("fx/downlifter.wav".into(), Box::new(|| fxsounds::downlifter(4.0, SRF, 3))));
    jobs.push(("fx/impact.wav".into(), Box::new(|| fxsounds::impact(SRF, 3))));
    jobs.push(("fx/reverse_crash.wav".into(), Box::new(|| fxsounds::reverse_crash(2.0, SRF, 3))));
    jobs.push(("fx/sweep_up.wav".into(), Box::new(|| fxsounds::sweep(4.0, SRF, 3, true))));
    jobs.par_iter()
        .map(|(rel, f)| {
            let p = out.join(rel);
            save_wav(&p, &f(), false)?;
            Ok(p)
        })
        .collect()
}
