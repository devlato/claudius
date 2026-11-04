//! Scratching: a record moved back and forth under the needle, its sound let through by a crossfader. The record is a
//! bar or a few of one of the song's own tracks (`record`), the moves are played on a turntable (a phone's, a MIDI
//! controller's jog wheel), and the scratch is the record read along them (`render`), kept as a take.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::model::Blueprint;
use crate::{compose, render};

/// the longest a scratch is, in seconds
pub const MAX_SECS: f64 = 120.0;

/// the most bars a record is
pub const MAX_BARS: u32 = 8;

/// Where the record was under the needle `t` seconds into the scratch, `at` seconds into the record, and how far the
/// crossfader let it through (0 shut, 1 open).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub t: f64,
    pub at: f64,
    pub fader: f32,
}

/// A scratch on one of the song's tracks: the record, `bars` bars of track `track` from bar `bar` (0 the first), and
/// the moves made on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Take {
    pub track: String,
    pub bar: u32,
    pub bars: u32,
    pub moves: Vec<Move>,
}

/// what scratch `t` sounds like
pub fn take(bp: &Blueprint, t: &Take) -> Result<Stereo, String> {
    let record = record(bp, &t.track, t.bar, t.bars)?;
    render(&record, &t.moves)
}

/// the name of the song's next scratch: scratch1, scratch2, ...
pub fn name(bp: &Blueprint) -> String {
    let taken = bp.layer_names();
    (1..).map(|k| format!("scratch{k}")).find(|n| !taken.contains(n)).unwrap_or_default()
}

/// Track `track` of the song alone over `bars` bars from bar `bar` (0 the first), as the record a scratch moves.
pub fn record(bp: &Blueprint, track: &str, bar: u32, bars: u32) -> Result<Stereo, String> {
    let mut p = compose::arrange(bp);
    if !p.tracks.iter().any(|t| t.name == track) {
        let names: Vec<&str> = p.tracks.iter().map(|t| t.name.as_str()).collect();
        return Err(format!("no track '{track}' to scratch (tracks: {})", names.join(" ")));
    }
    if bar >= p.bars {
        return Err(format!("the song has {} bars, so no bar {bar} (they count from 0)", p.bars));
    }
    if !(1..=MAX_BARS).contains(&bars) {
        return Err(format!("a record is 1 to {MAX_BARS} bars, not {bars}"));
    }
    p.tracks.retain(|t| t.name == track);
    let slice = p.slice(bar, bar + bars);
    let r = render::render(&slice, &render::Options { stems: false, tail_secs: 0.0 }).map_err(|e| format!("{e:#}"))?;
    Ok(r.master)
}

/// The record read along `moves` (in time order) through the crossfader: what the scratch sounds like.
pub fn render(record: &Stereo, moves: &[Move]) -> Result<Stereo, String> {
    if moves.len() < 2 {
        return Err("a scratch needs the record to move: two moves at least".into());
    }
    if let Some(m) = moves.iter().find(|m| !(m.t.is_finite() && m.at.is_finite() && (0.0..=1.0).contains(&m.fader))) {
        return Err(format!("a move has a time, a place on the record and a fader from 0 to 1, not {m:?}"));
    }
    if moves.windows(2).any(|w| w[1].t < w[0].t) || moves[0].t < 0.0 {
        return Err("the moves go in time order, from 0 s on".into());
    }
    let end = moves[moves.len() - 1].t;
    if end > MAX_SECS {
        return Err(format!("the scratch is {end:.0} s long, and one is {MAX_SECS:.0} s at most"));
    }
    let n = (end * SRF).round() as usize;
    let mut out = Stereo::zeros(n);
    let last = record.len() as f64 - 1.0;
    // the fader moves in about 2 ms, as a crossfader's does, so cutting the sound does not click
    let slew = 1.0 - (-1.0 / (0.002 * SRF)).exp() as f32;
    let mut fader = moves[0].fader;
    let mut k = 0;
    for i in 0..n {
        let t = i as f64 / SRF;
        while k + 2 < moves.len() && moves[k + 1].t <= t {
            k += 1;
        }
        let (a, b) = (moves[k], moves[k + 1]);
        let w = if b.t > a.t { ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 1.0 };
        fader += (a.fader + (b.fader - a.fader) * w as f32 - fader) * slew;
        let at = (a.at + (b.at - a.at) * w) * SRF;
        if !(0.0..last).contains(&at) {
            continue;
        }
        let (j, f) = (at as usize, (at - at.floor()) as f32);
        out.l[i] = (record.l[j] + (record.l[j + 1] - record.l[j]) * f) * fader;
        out.r[i] = (record.r[j] + (record.r[j + 1] - record.r[j]) * f) * fader;
    }
    out.fade(64.min(n), 256.min(n));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(secs: f64) -> Stereo {
        let n = (secs * SRF) as usize;
        let mut x = Stereo::zeros(n);
        for i in 0..n {
            x.l[i] = i as f32 / n as f32;
            x.r[i] = -x.l[i];
        }
        x
    }

    fn at(x: &Stereo, secs: f64) -> f32 {
        x.l[(secs * SRF) as usize]
    }

    #[test]
    fn a_record_moved_at_its_speed_plays_as_it_is() {
        let rec = ramp(2.0);
        let moves = [Move { t: 0.0, at: 0.0, fader: 1.0 }, Move { t: 2.0, at: 2.0, fader: 1.0 }];
        let out = render(&rec, &moves).unwrap();
        assert_eq!(out.len(), rec.len());
        for s in [0.25, 0.5, 1.0, 1.5] {
            assert!((at(&out, s) - at(&rec, s)).abs() < 1e-3, "{s} s: {} vs {}", at(&out, s), at(&rec, s));
        }
        assert!((out.r[22_050] + out.l[22_050]).abs() < 1e-6, "both sides are read");
    }

    #[test]
    fn a_record_pulled_back_plays_backwards_at_the_speed_it_moves() {
        let rec = ramp(2.0);
        // forward over a second, then back over half a second: twice the speed, the other way
        let moves = [
            Move { t: 0.0, at: 0.0, fader: 1.0 },
            Move { t: 1.0, at: 1.0, fader: 1.0 },
            Move { t: 1.5, at: 0.0, fader: 1.0 },
        ];
        let out = render(&rec, &moves).unwrap();
        assert!((at(&out, 1.25) - at(&rec, 0.5)).abs() < 1e-3);
        assert!(at(&out, 1.1) > at(&out, 1.4), "it goes backwards");
    }

    #[test]
    fn a_shut_fader_cuts_the_sound_without_a_click() {
        let mut rec = Stereo::zeros((2.0 * SRF) as usize);
        rec.l.iter_mut().chain(rec.r.iter_mut()).for_each(|s| *s = 0.5);
        let moves = [
            Move { t: 0.0, at: 0.0, fader: 1.0 },
            Move { t: 0.5, at: 0.5, fader: 1.0 },
            Move { t: 0.5, at: 0.5, fader: 0.0 },
            Move { t: 1.0, at: 1.0, fader: 0.0 },
        ];
        let out = render(&rec, &moves).unwrap();
        assert!((at(&out, 0.25) - 0.5).abs() < 1e-3);
        assert!(at(&out, 0.75).abs() < 1e-4, "shut: {}", at(&out, 0.75));
        let jump = out.l.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(jump < 0.05, "the cut is a short fade, not a step: {jump}");
    }

    #[test]
    fn off_the_record_is_silence_and_bad_moves_are_refused() {
        let rec = ramp(1.0);
        let moves = [Move { t: 0.0, at: -1.0, fader: 1.0 }, Move { t: 0.5, at: -0.5, fader: 1.0 }];
        assert!(render(&rec, &moves).unwrap().l.iter().all(|s| *s == 0.0));
        assert!(render(&rec, &moves[..1]).is_err());
        let back = [Move { t: 1.0, at: 0.0, fader: 1.0 }, Move { t: 0.5, at: 0.5, fader: 1.0 }];
        assert!(render(&rec, &back).is_err());
        let loud = [Move { t: 0.0, at: 0.0, fader: 2.0 }, Move { t: 0.5, at: 0.5, fader: 1.0 }];
        assert!(render(&rec, &loud).is_err());
    }

    #[test]
    fn a_record_is_one_track_of_the_song() {
        let g = crate::genres::get("house").unwrap();
        let o = compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let bp = compose::generate(&g, 3, &o, None);
        let lane = bp.drums[0].name.clone();
        let rec = record(&bp, &lane, 0, 1).unwrap();
        let bar = 4.0 * 60.0 / bp.bpm;
        assert!((rec.secs() - bar).abs() < 0.01, "{} s, a bar is {bar} s", rec.secs());
        assert!(rec.l.iter().any(|s| s.abs() > 0.01), "the lane sounds");
        assert!(record(&bp, "nothing", 0, 1).unwrap_err().contains("tracks:"));
        assert!(record(&bp, &lane, 10_000, 1).is_err());
        assert!(record(&bp, &lane, 0, 0).is_err());
    }
}
