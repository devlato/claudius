//! How far the music falls through Infinite Radio's blends from one song to the next: for pairs of songs the station
//! could play one after the other (written at the outgoing one's tempo, as it writes a song it will blend), the blend
//! as the station mixes it now, and as it mixed it before a change handed the beat over on a downbeat (every section
//! taken to play its beat, which leaves `radio::change` as it was), each change's dip: from the level of the songs
//! around it (the median beat of the outgoing song's last 8 bars before it and the next one's first 8 after it) to
//! the quietest beat of the change and the bar after it, in dB. A paired sign test over the pairs, the songs that sing
//! from their first bars and the club songs apart (whose blends over beat-only intros and outros should not move).
//! Beside each dip, the deepest hole the mix itself makes (`hole`): how far it falls below the louder of the two songs
//! as each would sound there alone, which a song's own quiet beat (a break, a pause before a drop) does not count in.
//!
//!   cargo run -p claudius-session --example radio_dips -- [SEEDS]

use std::sync::Arc;

use claudius::audio::Stereo;
use claudius::config::SRF;
use claudius_api::Command;
use claudius_session::host::Host;
use claudius_session::radio::{self, How, Song};

/// a song of `genre` at `bpm` in `key`, written and rendered whole, its sections' beats as the station reads them
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

/// the level of each beat of `x` from frame `from` for `beats` beats of `beat` frames, in dB (silence -120)
fn beats(x: &Stereo, from: usize, n: usize, beat: f64) -> Vec<f64> {
    (0..n)
        .filter_map(|k| {
            let a = from + (k as f64 * beat) as usize;
            let b = (from + ((k + 1) as f64 * beat) as usize).min(x.len());
            (b > a).then(|| {
                let e: f64 = (a..b).map(|i| f64::from(x.l[i]).powi(2) + f64::from(x.r[i]).powi(2)).sum();
                10.0 * (e / (2 * (b - a)) as f64).max(1e-12).log10()
            })
        })
        .collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v.get(v.len() / 2).copied().unwrap_or(-120.0)
}

/// how far the music falls through change `c` from `a` into `b`, in dB
fn dip(a: &Song, b: &Song, c: &radio::Change) -> f64 {
    let beat = 60.0 / a.bpm * SRF;
    let before = beats(&a.audio, c.at.saturating_sub((32.0 * beat) as usize), 32, beat);
    let after = beats(&b.audio, c.then, 32, 60.0 / b.bpm * SRF);
    let around = median(before.into_iter().chain(after).collect());
    let mut through = beats(&c.mixed, 0, (c.mixed.len() as f64 / beat).floor() as usize, beat);
    through.extend(beats(&b.audio, c.then, 4, 60.0 / b.bpm * SRF));
    let quietest = through.into_iter().fold(f64::INFINITY, f64::min);
    (around - quietest).max(0.0)
}

/// The deepest hole the mixing makes in change `c` from `a` into `b`: over its beats, how far the mix falls below the
/// louder of the two songs as each would sound there alone (`a` from where the change starts, `b` from where it comes
/// in), in dB. A song's own quiet beat (a break, a pause before a drop) is no hole, as the other song, or the song alone,
/// would be as quiet there; beats where both are near silence (under -60 dB) are left out.
fn hole(a: &Song, b: &Song, c: &radio::Change) -> f64 {
    let beat = 60.0 / a.bpm * SRF;
    let n = (c.mixed.len() as f64 / beat).floor() as usize;
    let mix = beats(&c.mixed, 0, n, beat);
    let alone_a = beats(&a.audio, c.at, n, beat);
    let alone_b = beats(&b.audio, c.then.saturating_sub(c.mixed.len()), n, beat);
    mix.iter()
        .zip(alone_a.iter().zip(&alone_b))
        .map(|(m, (x, y))| (x.max(*y), m))
        .filter(|(louder, _)| *louder > -60.0)
        .map(|(louder, m)| louder - m)
        .fold(0.0, f64::max)
}

/// a two-sided sign test of `n` pairs of which `k` went one way (ties left out)
fn sign_p(k: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let choose = |n: usize, k: usize| (0..k).fold(1.0f64, |c, i| c * (n - i) as f64 / (i + 1) as f64);
    let tail: f64 = (0..=k.min(n - k)).map(|i| choose(n, i)).sum::<f64>() / 2f64.powi(n as i32);
    (2.0 * tail).min(1.0)
}

fn main() -> Result<(), String> {
    let seeds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2);
    // the outgoing genre, the next and their tempo (the club pairs last)
    let pairs: [(&str, &str, f64); 10] = [
        ("hip_hop", "lofi", 88.0),
        ("lofi", "hip_hop", 86.0),
        ("rnb", "hip_hop", 92.0),
        ("boom_bap", "lofi", 90.0),
        ("trip_hop", "lofi", 84.0),
        ("neo_soul", "rnb", 92.0),
        ("trap", "phonk", 140.0),
        ("funk", "nu_disco", 112.0),
        ("house", "deep_house", 124.0),
        ("techno", "tech_house", 128.0),
    ];
    let mut rows: Vec<(bool, f64, f64)> = Vec::new();
    let mut holes: Vec<(bool, f64, f64)> = Vec::new();
    for (ga, gb, bpm) in pairs {
        for seed in 1..=seeds {
            let (a, b) = (song(ga, seed * 2, bpm, "A minor")?, song(gb, seed * 2 + 1, bpm, "C major")?);
            let now = radio::change(&a, &b, How::Blend, 0);
            // as before: every section taken to play its beat, so that no beat is handed over
            let all = |s: &Song| Song {
                audio: s.audio.clone(),
                sections: s.sections.iter().map(|x| (x.0, x.1, x.2.clone(), true)).collect(),
                ..*s
            };
            let was = radio::change(&all(&a), &all(&b), How::Blend, 0);
            let (d0, d1) = (dip(&a, &b, &was), dip(&a, &b, &now));
            let (h0, h1) = (hole(&a, &b, &was), hole(&a, &b, &now));
            let club = a.club && b.club;
            println!(
                "{ga} -> {gb} seed {seed}: before {d0:.1} dB ({}), now {d1:.1} dB ({}); the mix's deepest hole below the \
                 louder song alone: before {h0:.1} dB, now {h1:.1} dB",
                was.said, now.said
            );
            rows.push((club, d0, d1));
            holes.push((club, h0, h1));
        }
    }
    for (name, club, rows) in [
        ("songs that sing from their first bars", false, &rows),
        ("club songs", true, &rows),
        ("songs that sing from their first bars, the mix's holes", false, &holes),
        ("club songs, the mix's holes", true, &holes),
    ] {
        let of: Vec<&(bool, f64, f64)> = rows.iter().filter(|r| r.0 == club).collect();
        let n = of.len().max(1) as f64;
        let (b, a) = (of.iter().map(|r| r.1).sum::<f64>() / n, of.iter().map(|r| r.2).sum::<f64>() / n);
        let lower = of.iter().filter(|r| r.2 < r.1 - 0.05).count();
        let higher = of.iter().filter(|r| r.2 > r.1 + 0.05).count();
        let worse = of.iter().filter(|r| r.2 > r.1 + 1.0).count();
        println!(
            "{name}: {} changes, mean before {b:.2} dB, now {a:.2} dB; lower in {lower}, higher in {higher} (sign test p \
             {:.4}), more than 1 dB deeper in {worse}",
            of.len(),
            sign_p(lower, lower + higher)
        );
    }
    Ok(())
}
