//! Infinite Radio's changes from one song to the next, rendered to listen to: a blend between two house songs, one
//! between a hip hop song and a lo-fi one, and a skip, each from 16 bars before the change to 16 bars after it, as the
//! station mixes them (`radio::change`, `radio::cut`), on songs claudius writes for it.
//!
//!   cargo run -p claudius-session --example radio_mixes -- OUT_DIR

use std::sync::Arc;

use claudius::audio::Stereo;
use claudius::config::SRF;
use claudius_api::Command;
use claudius_session::host::Host;
use claudius_session::radio::{self, How, Song};

/// a song of `genre` at `bpm`, written and rendered whole, as the radio's engine writes one
fn song(genre: &str, seed: u64, bpm: f64, key: &str) -> Result<Song, String> {
    let mut host = Host::new(seed);
    let reply = host.command(Command::NewSong {
        genre: genre.into(),
        seed: Some(seed),
        bpm: Some(bpm),
        key: Some(key.into()),
        loops: false,
        sampled: Some(false),
        sample_search: None,
        complexity: None,
    });
    if !reply.ok {
        return Err(reply.message);
    }
    let audio = host.render().ok_or("no song to render")??;
    let s = host.state().song.ok_or("no song")?;
    let club = claudius::genres::get(genre).is_ok_and(|g| radio::CLUB.contains(&g.family.as_str()));
    let beat = |x: &claudius_api::SectionView| x.layers.iter().any(|l| radio::BEAT.contains(&l.as_str()));
    let sections = s.sections.iter().map(|x| (x.start_bar, x.bars, x.kind.clone(), beat(x))).collect();
    Ok(Song { audio: Arc::new(Stereo::from_interleaved(&audio.samples)), bpm: s.bpm, bars: s.bars, sections, club })
}

/// the change `c` from `a` into `b` as it is heard: `lead` frames of `a` before it, the change, and `after` frames of
/// `b` from where it goes on
fn heard(a: &Song, b: &Song, c: &radio::Change, lead: usize, after: usize) -> Stereo {
    let from = c.at.saturating_sub(lead);
    let mut out = Stereo::zeros(c.at - from + c.mixed.len() + after);
    for i in from..c.at {
        out.l[i - from] = a.audio.l.get(i).copied().unwrap_or(0.0);
        out.r[i - from] = a.audio.r.get(i).copied().unwrap_or(0.0);
    }
    out.add_at(&c.mixed, (c.at - from) as i64, 1.0, 1.0);
    let at = c.at - from + c.mixed.len();
    for i in 0..after {
        out.l[at + i] = b.audio.l.get(c.then + i).copied().unwrap_or(0.0);
        out.r[at + i] = b.audio.r.get(c.then + i).copied().unwrap_or(0.0);
    }
    out
}

fn main() -> Result<(), String> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("usage: radio_mixes OUT_DIR")?);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let write = |name: &str, x: &Stereo, said: &str| -> Result<(), String> {
        let path = out.join(name);
        std::fs::write(&path, claudius::audio::wav_bytes(x)).map_err(|e| e.to_string())?;
        println!("{}: {said} ({:.0} s)", path.display(), x.secs());
        Ok(())
    };
    let bars = |s: &Song, n: f64| (n * 240.0 / s.bpm * SRF) as usize;
    // two house songs at 124 BPM in keys of the same notes: a blend over the next one's intro
    let (a, b) = (song("house", 11, 124.0, "A minor")?, song("deep_house", 12, 124.0, "C major")?);
    let c = radio::change(&a, &b, How::Blend, 0);
    write("1-house-into-deep-house.wav", &heard(&a, &b, &c, bars(&a, 16.0), bars(&b, 16.0)), &c.said)?;
    // hip hop into lo-fi at 88 BPM: a blend over a phrase of 8
    let (a, b) = (song("hip_hop", 21, 88.0, "F minor")?, song("lofi", 22, 88.0, "Ab major")?);
    let c = radio::change(&a, &b, How::Blend, 0);
    write("2-hip-hop-into-lofi.wav", &heard(&a, &b, &c, bars(&a, 16.0), bars(&b, 16.0)), &c.said)?;
    // a skip halfway through a house song into another that was to blend
    let (a, b) = (song("house", 31, 124.0, "D minor")?, song("tech_house", 32, 124.0, "D minor")?);
    let now = bars(&a, (a.bars / 2) as f64);
    let c = radio::cut(&a, &b, now, How::Blend);
    write("3-a-skip.wav", &heard(&a, &b, &c, bars(&a, 8.0), bars(&b, 16.0)), &c.said)?;
    Ok(())
}
