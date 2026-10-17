//! Chopping any recording into material: bar loops cut on the beat grid (one per section, tagged with tempo, key and
//! the section's energy) and drum one-shots, saved as WAVs and registered in the user library.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use super::apple::LoopInfo;
use super::{oneshot, user_index_add};
use crate::analysis::song::{self, Analysis};
use crate::audio;
use crate::config::SRF;
use crate::rng::Rng;
use crate::song::slug;

#[derive(Serialize)]
pub struct Chopped {
    pub analysis: Analysis,
    pub loops: Vec<LoopInfo>,
    pub oneshots: Vec<(String, PathBuf)>,
}

/// the moments of a recording that best match a description (earshot's find), cut out as WAVs and added to the
#[cfg(feature = "ears")]
/// user library
pub fn find(
    ears: &crate::ears::Ears,
    path: &Path,
    query: &str,
    top: usize,
    out: &Path,
) -> Result<Vec<(PathBuf, earshot::Trim)>> {
    let x = audio::load(path)?;
    let trims = ears.inner.find(&crate::ears::Ears::mono(&x), query, top)?;
    let name = slug(&path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "song".into()));
    let dir = out.join(&name);
    std::fs::create_dir_all(&dir)?;
    let mut found = Vec::new();
    let mut entries = Vec::new();
    for (i, t) in trims.into_iter().enumerate() {
        let (a, b) = ((t.start * SRF) as usize, ((t.end * SRF) as usize).min(x.len()));
        if b <= a + 64 {
            continue;
        }
        let mut seg = x.slice(a, b);
        let len = seg.len();
        seg.fade(48.min(len / 4), ((0.01 * SRF) as usize).min(len / 4));
        let file = dir.join(format!("find-{}-{}.wav", slug(query), i + 1));
        audio::save_wav(&file, &seg, true)?;
        entries.push(LoopInfo {
            path: file.to_string_lossy().into_owned(),
            name: format!("{name} {query} {}", i + 1),
            tempo: -1.0,
            beats: 0.0,
            secs: t.end - t.start,
            key: None,
            minor: false,
            instrument: "Found".into(),
            subtype: query.to_string(),
            genre: "User".into(),
            descriptors: vec![format!("z {:.1}", t.z), t.verdict.clone()],
        });
        found.push((file, t));
    }
    user_index_add(&entries)?;
    Ok(found)
}

pub fn chop(path: &Path, bars_per_loop: usize, out: &Path, bpm_range: Option<(f64, f64)>) -> Result<Chopped> {
    let x = audio::load(path)?;
    let a = song::analyze(&x, bpm_range);
    let name = slug(&path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "song".into()));
    let dir = out.join(&name);
    std::fs::create_dir_all(&dir)?;
    let bar = a.bar_secs;
    let mut loops = Vec::new();
    for (i, s) in a.sections.iter().enumerate() {
        let n = bars_per_loop.min(s.bars).max(1);
        let t0 = a.first_downbeat + s.start_bar as f64 * bar;
        let (s0, s1) = ((t0 * SRF) as usize, ((t0 + n as f64 * bar) * SRF) as usize);
        if s1 > x.len() {
            continue;
        }
        let mut seg = x.slice(s0, s1);
        let len = seg.len();
        seg.fade(32.min(len), 64.min(len));
        let file = dir.join(format!("{:02}-{}-{}bars.wav", i + 1, s.kind.name(), n));
        audio::save_wav(&file, &seg, false)?;
        loops.push(LoopInfo {
            path: file.to_string_lossy().into_owned(),
            name: format!("{name} {} {}", s.kind.name(), i + 1),
            tempo: a.bpm,
            beats: (n * 4) as f64,
            secs: n as f64 * bar,
            key: if s.harmony { Some(a.key.tonic) } else { None },
            minor: a.key.mode.is_minor(),
            instrument: "Chop".into(),
            subtype: if s.kick { "Mixed".into() } else { "Music".into() },
            genre: "User".into(),
            descriptors: vec![s.kind.name().to_string(), format!("energy {:.2}", s.energy)],
        });
    }
    // drum one-shots, cut from the sparsest sections that still have the instrument
    let mut oneshots = Vec::new();
    let mut rng = Rng::new(0xc409);
    let mut order: Vec<&song::SectionGuess> = a.sections.iter().collect();
    order.sort_by(|p, q| p.energy.partial_cmp(&q.energy).unwrap());
    for lane in ["kick", "clap", "hat"] {
        for s in &order {
            if (lane == "kick" && !s.kick) || (lane == "hat" && !s.hats) {
                continue;
            }
            let t0 = a.first_downbeat + s.start_bar as f64 * bar;
            let (s0, s1) = ((t0 * SRF) as usize, (((t0 + s.bars.min(4) as f64 * bar) * SRF) as usize).min(x.len()));
            if let Some(hit) = oneshot::cut(&x.slice(s0, s1), lane, &mut rng) {
                let file = dir.join(format!("{lane}.wav"));
                audio::save_wav(&file, &hit, true)?;
                oneshots.push((lane.to_string(), file));
                break;
            }
        }
    }
    let subtype = |lane: &str| match lane {
        "kick" => "Kick",
        "hat" => "Hi-hat",
        _ => "Snare",
    };
    let mut entries = loops.clone();
    for (lane, p) in &oneshots {
        entries.push(LoopInfo {
            path: p.to_string_lossy().into_owned(),
            name: format!("{name} {lane} {}", if *lane == "clap" { "Clap" } else { "" }).trim().to_string(),
            tempo: -1.0,
            beats: 0.0,
            secs: 0.0,
            key: None,
            minor: false,
            instrument: "Drums".into(),
            subtype: subtype(lane).into(),
            genre: "User".into(),
            descriptors: vec!["one-shot".into()],
        });
    }
    user_index_add(&entries)?;
    Ok(Chopped { analysis: a, loops, oneshots })
}
