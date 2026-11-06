//! Scratching: a record moved back and forth under the needle, its sound let through by a crossfader. The record is a
//! bar or a few of one of the song's own tracks, or of the whole mix (`record`), the moves are played on a turntable
//! (a phone's, a MIDI controller's jog wheel), and the scratch is the record read along them (`render`), kept as a take.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::model::Blueprint;
use crate::{compose, render};

/// the longest a scratch is, in seconds
pub const MAX_SECS: f64 = 120.0;

/// the most bars a record is
pub const MAX_BARS: u32 = 8;

/// the track a record of the whole mix is cut from
pub const MIX: &str = "*";

/// Where the record was under the needle `t` seconds into the scratch, `at` seconds into the record, and how far the
/// crossfader let it through (0 shut, 1 open).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub t: f64,
    pub at: f64,
    pub fader: f32,
}

/// A scratch on one of the song's tracks, or on the whole mix (`MIX`): the record, `bars` bars of track `track` from
/// bar `bar` (0 the first), and the moves made on it.
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

/// Track `track` of the song alone (or every track, for `MIX`) over `bars` bars from bar `bar` (0 the first), as the
/// record a scratch moves; a track muted, or left out while others are soloed, is heard all the same.
pub fn record(bp: &Blueprint, track: &str, bar: u32, bars: u32) -> Result<Stereo, String> {
    let slice = cut(bp, track, bar, bars)?;
    let r = render::render(&slice, &render::Options { stems: false, tail_secs: 0.0 }).map_err(|e| format!("{e:#}"))?;
    if r.master.l.iter().chain(&r.master.r).all(|s| s.abs() < 1e-4) {
        return Err(format!("{track} is silent in bars {} to {}: cut the record where it plays", bar + 1, bar + bars));
    }
    Ok(r.master)
}

/// What a record of `track` (or of the whole mix) over `bars` bars from `bar` was cut from, each file's as `credited`
/// says (`take::credited`): what a take of a scratch on it credits.
pub fn sources(
    bp: &Blueprint,
    track: &str,
    bar: u32,
    bars: u32,
    credited: impl Fn(&std::path::Path) -> crate::take::Sources,
) -> Result<crate::take::Sources, String> {
    Ok(crate::song::sources(&cut(bp, track, bar, bars)?, credited))
}

/// the song arranged with every track heard, then `track` alone (or every track, for `MIX`) over the bars of the record
fn cut(bp: &Blueprint, track: &str, bar: u32, bars: u32) -> Result<crate::model::Project, String> {
    let mut p = compose::arrange(&crate::pads::heard(bp));
    if track != MIX && !p.tracks.iter().any(|t| t.name == track) {
        let names: Vec<&str> = p.tracks.iter().map(|t| t.name.as_str()).collect();
        return Err(format!("no track '{track}' to scratch (tracks: {})", names.join(" ")));
    }
    if !(1..=MAX_BARS).contains(&bars) {
        return Err(format!("a record is 1 to {MAX_BARS} bars, not {bars}"));
    }
    if bar + bars > p.bars {
        return Err(format!("the song has {} bars (counted from 0), so {bars} from bar {bar} go past its end", p.bars));
    }
    if track != MIX {
        p.tracks.retain(|t| t.name == track);
    }
    Ok(p.slice(bar, bar + bars))
}

/// The record read along `moves` (in time order) through the crossfader: what the scratch sounds like. The record goes
/// round: past its end it plays from its start, and before its start from its end; held still, it is silent.
pub fn render(record: &Stereo, moves: &[Move]) -> Result<Stereo, String> {
    let mut x = read(record, moves)?;
    // a still record holds a sample, which is no sound: a high-pass at about 10 Hz takes it out
    let pole = 1.0 - (2.0 * std::f64::consts::PI * 10.0 / SRF) as f32;
    for side in [&mut x.l, &mut x.r] {
        let (mut was, mut out) = (0.0, 0.0);
        for s in side.iter_mut() {
            out = *s - was + pole * out;
            was = *s;
            *s = out;
        }
    }
    let n = x.len();
    x.fade(64.min(n), 256.min(n));
    Ok(x)
}

/// the crossfader's travel over which it opens, as a scratch DJ's does: a hair of it lets the whole record through
pub const CUT_IN: f32 = 0.08;

/// how much of the record a crossfader set to `f` (0 shut, 1 at the other end) lets through: all of it once it is past
/// `CUT_IN`, on a smooth curve up to there
pub fn opening(f: f32) -> f32 {
    let x = (f / CUT_IN).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The record at the bandwidths a needle reads it at: the whole of it, and low-passed under a quarter, an eighth and a
/// sixteenth of the rate (at 9.6, 4.8 and 2.4 kHz), so that a record moved at two, four or eight times its speed does
/// not alias, its highs carried past half the rate and folded back as a scratch's whistle.
fn levels(record: &Stereo) -> Vec<Stereo> {
    let mut out = vec![record.clone()];
    for k in 2..=4 {
        let mut x = out[out.len() - 1].clone();
        for side in [&mut x.l, &mut x.r] {
            crate::dsp::filter::butter(side, false, 0.8 * SRF / 2f64.powi(k), SRF, 4);
        }
        out.push(x);
    }
    out
}

/// Where the record is at `t`, in seconds of it, and how fast it moves (seconds of it a second), between moves `k` and
/// `k + 1`: along a curve through the moves that keeps its speed as a hand changes it (Catmull-Rom's), rather than in
/// straight lines whose speed jumps at each move.
fn place(moves: &[Move], k: usize, t: f64) -> (f64, f64) {
    let (a, b) = (moves[k], moves[k + 1]);
    let dt = b.t - a.t;
    if dt <= 0.0 {
        return (b.at, 0.0);
    }
    let slope = |p: Move, q: Move| if q.t > p.t { (q.at - p.at) / (q.t - p.t) } else { (b.at - a.at) / dt };
    let m0 = if k > 0 { slope(moves[k - 1], b) } else { (b.at - a.at) / dt };
    let m1 = moves.get(k + 2).map_or((b.at - a.at) / dt, |&c| slope(a, c));
    let w = ((t - a.t) / dt).clamp(0.0, 1.0);
    let (w2, w3) = (w * w, w * w * w);
    let at = (2.0 * w3 - 3.0 * w2 + 1.0) * a.at
        + (w3 - 2.0 * w2 + w) * dt * m0
        + (3.0 * w2 - 2.0 * w3) * b.at
        + (w3 - w2) * dt * m1;
    let speed = ((6.0 * w2 - 6.0 * w) * a.at
        + (3.0 * w2 - 4.0 * w + 1.0) * dt * m0
        + (6.0 * w - 6.0 * w2) * b.at
        + (3.0 * w2 - 2.0 * w) * dt * m1)
        / dt;
    (at, speed)
}

/// the record under the needle at `pos` seconds of it, read at `speed` times its own speed: from the bandwidth that
/// leaves it unaliased (`levels`), between the two nearest
fn needle(levels: &[Stereo], pos: f64, speed: f64) -> (f32, f32) {
    let len = levels[0].len();
    let at = (pos * SRF).rem_euclid(len as f64);
    let (j, f) = ((at as usize).min(len - 1), (at - at.floor()) as f32);
    let k = (j + 1) % len;
    let level = speed.max(1.0).log2().min((levels.len() - 1) as f64);
    let (lo, mix) = (level.floor() as usize, (level - level.floor()) as f32);
    let hi = (lo + 1).min(levels.len() - 1);
    let read = |x: &Stereo| (x.l[j] + (x.l[k] - x.l[j]) * f, x.r[j] + (x.r[k] - x.r[j]) * f);
    let (a, b) = (read(&levels[lo]), read(&levels[hi]));
    (a.0 + (b.0 - a.0) * mix, a.1 + (b.1 - a.1) * mix)
}

/// the record read along `moves` through the crossfader, as it is under the needle
fn read(record: &Stereo, moves: &[Move]) -> Result<Stereo, String> {
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
    if record.is_empty() {
        return Err("the record is silent: it has no sound to scratch".into());
    }
    let levels = levels(record);
    let n = (end * SRF).round() as usize;
    let mut out = Stereo::zeros(n);
    // the fader moves in about 2 ms, as a crossfader's does, so cutting the sound does not click
    let slew = 1.0 - (-1.0 / (0.002 * SRF)).exp() as f32;
    let mut fader = opening(moves[0].fader);
    let mut k = 0;
    for i in 0..n {
        let t = i as f64 / SRF;
        while k + 2 < moves.len() && moves[k + 1].t <= t {
            k += 1;
        }
        let (a, b) = (moves[k], moves[k + 1]);
        let w = if b.t > a.t { ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 1.0 };
        fader += (opening(a.fader + (b.fader - a.fader) * w as f32) - fader) * slew;
        let (pos, speed) = place(moves, k, t);
        let (l, r) = needle(&levels, pos, speed.abs());
        out.l[i] = l * fader;
        out.r[i] = r * fader;
    }
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
        let out = read(&rec, &moves).unwrap();
        assert_eq!(out.len(), rec.len());
        for s in [0.25, 0.5, 1.0, 1.5] {
            assert!((at(&out, s) - at(&rec, s)).abs() < 1e-3, "{s} s: {} vs {}", at(&out, s), at(&rec, s));
        }
        assert!((out.r[22_050] + out.l[22_050]).abs() < 1e-6, "both sides are read");
    }

    #[test]
    fn a_record_pulled_back_slows_through_the_turn_and_plays_backwards() {
        let rec = ramp(2.0);
        // forward over a second, then back over half a second
        let moves = [(0.0, 0.0), (1.0, 1.0), (1.5, 0.0)].map(|(t, at)| Move { t, at, fader: 1.0 });
        let out = read(&rec, &moves).unwrap();
        assert!(at(&out, 1.1) > at(&out, 1.4), "it goes backwards");
        assert!((at(&out, 1.499) - at(&rec, 0.0)).abs() < 0.01, "to where the hand took it");
        // a hand turning a record back slows it to a stop first: its speed takes no step at the turn
        let slope = |s: f64| (at(&out, s + 0.001) - at(&out, s)) / 0.001;
        assert!(slope(0.998).abs() < 0.1 && slope(1.0).abs() < 0.1, "{} {}", slope(0.998), slope(1.0));
    }

    #[test]
    fn a_record_moved_fast_whistles_no_highs_folded_back() {
        // a 10 kHz tone moved at four times its speed would sound at 40 kHz, past half the rate, folded back to 8 kHz:
        // the needle reads it from the record low-passed at 4.8 kHz, where it is not
        let n = SRF as usize;
        let mut rec = Stereo::zeros(n);
        for i in 0..n {
            let v = (0.5 * (i as f64 * 10_000.0 * std::f64::consts::TAU / SRF).sin()) as f32;
            (rec.l[i], rec.r[i]) = (v, v);
        }
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let fast = [Move { t: 0.0, at: 0.0, fader: 1.0 }, Move { t: 0.2, at: 0.8, fader: 1.0 }];
        let out = read(&rec, &fast).unwrap();
        assert!(rms(&out.l[960..8640]) < 0.02, "{}", rms(&out.l[960..8640]));
        let plain = [Move { t: 0.0, at: 0.0, fader: 1.0 }, Move { t: 0.5, at: 0.5, fader: 1.0 }];
        assert!(rms(&read(&rec, &plain).unwrap().l[960..]) > 0.3, "at its own speed it plays");
    }

    #[test]
    fn a_crossfader_opens_within_a_hair_of_its_travel() {
        assert_eq!((opening(0.0), opening(CUT_IN), opening(0.5), opening(1.0)), (0.0, 1.0, 1.0, 1.0));
        assert!((opening(CUT_IN / 2.0) - 0.5).abs() < 1e-6);
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
        let out = read(&rec, &moves).unwrap();
        assert!((at(&out, 0.25) - 0.5).abs() < 1e-3);
        assert!(at(&out, 0.75).abs() < 1e-4, "shut: {}", at(&out, 0.75));
        let jump = out.l.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(jump < 0.05, "the cut is a short fade, not a step: {jump}");
    }

    #[test]
    fn the_record_goes_round_and_bad_moves_are_refused() {
        let rec = ramp(1.0);
        let moves = [Move { t: 0.0, at: 0.75, fader: 1.0 }, Move { t: 0.5, at: 1.25, fader: 1.0 }];
        let out = read(&rec, &moves).unwrap();
        assert!((at(&out, 0.1) - at(&rec, 0.85)).abs() < 1e-3, "before its end");
        assert!((at(&out, 0.4) - at(&rec, 0.15)).abs() < 1e-3, "past its end, from its start");
        let back = [Move { t: 0.0, at: 0.25, fader: 1.0 }, Move { t: 0.5, at: -0.25, fader: 1.0 }];
        assert!((at(&read(&rec, &back).unwrap(), 0.4) - at(&rec, 0.85)).abs() < 1e-3, "before its start, its end");
        assert!(render(&rec, &moves[..1]).is_err());
        assert!(render(&Stereo::zeros(0), &moves).is_err());
        let back = [Move { t: 1.0, at: 0.0, fader: 1.0 }, Move { t: 0.5, at: 0.5, fader: 1.0 }];
        assert!(render(&rec, &back).is_err());
        let loud = [Move { t: 0.0, at: 0.0, fader: 2.0 }, Move { t: 0.5, at: 0.5, fader: 1.0 }];
        assert!(render(&rec, &loud).is_err());
    }

    #[test]
    fn a_record_held_still_is_silent_and_one_moving_sounds() {
        let n = (1.0 * SRF) as usize;
        let mut rec = Stereo::zeros(n);
        for i in 0..n {
            rec.l[i] = (i as f32 * 440.0 * std::f32::consts::TAU / SRF as f32).sin() * 0.5 + 0.3;
            rec.r[i] = rec.l[i];
        }
        let held = [Move { t: 0.0, at: 0.5, fader: 1.0 }, Move { t: 1.0, at: 0.5, fader: 1.0 }];
        let still = render(&rec, &held).unwrap();
        assert!(still.l[24_000..].iter().all(|s| s.abs() < 0.01), "held still, it is silent");
        let moving = [Move { t: 0.0, at: 0.0, fader: 1.0 }, Move { t: 1.0, at: 1.0, fader: 1.0 }];
        let played = render(&rec, &moving).unwrap();
        let peak = played.l[24_000..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.05, "it plays the tone, without the offset: {peak}");
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
        let mut quiet = bp.clone();
        quiet.drums[0].patterns.values_mut().for_each(Vec::clear);
        let silent = record(&quiet, &lane, 0, 1).unwrap_err();
        assert!(silent.contains("is silent in bars 1 to 1"), "not a silent record: {silent}");
        assert!(record(&bp, &lane, 10_000, 1).is_err());
        let last = compose::arrange(&bp).bars - 1;
        assert!(record(&bp, &lane, last, 1).is_ok());
        assert!(record(&bp, &lane, last, 2).unwrap_err().contains("go past its end"), "not a shorter record");
        assert!(record(&bp, &lane, 0, 0).is_err());
    }

    #[test]
    fn a_record_of_the_whole_mix_holds_every_track() {
        let g = crate::genres::get("house").unwrap();
        let o = compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let bp = compose::generate(&g, 3, &o, None);
        let at = bp.form.iter().take_while(|s| s.kind != crate::model::SectionKind::Drop).map(|s| s.bars).sum();
        let mix = record(&bp, MIX, at, 1).unwrap();
        let lane = record(&bp, &bp.drums[0].name, at, 1).unwrap();
        assert_eq!(mix.len(), lane.len());
        // each record is mastered to the same loudness, so the mix is told from its kick by what else it holds
        let dot = |a: &Stereo, b: &Stereo| a.l.iter().zip(&b.l).map(|(x, y)| *x as f64 * *y as f64).sum::<f64>();
        let corr = dot(&mix, &lane) / (dot(&mix, &mix) * dot(&lane, &lane)).sqrt();
        assert!(corr < 0.9, "the mix is more than its kick: {corr}");
    }

    #[test]
    fn a_take_of_a_scratch_credits_what_its_record_was_cut_from_once_the_song_plays_it_no_more() {
        use crate::take;
        let dir = std::env::temp_dir().join(format!("claudius-scratch-credits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // a phrase from an online library, under CC BY, with its credit beside it
        let tone = Stereo::from_mono(&(0..48_000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect::<Vec<_>>());
        let phrase = dir.join("phrase.wav");
        std::fs::write(&phrase, crate::audio::wav_bytes(&tone)).unwrap();
        let found = serde_json::json!({"id": "freesound:2", "source": "openverse", "provider": "freesound",
            "title": "Keys", "creator": "someone", "licence": "by", "licence_name": "CC BY 4.0",
            "licence_url": "https://creativecommons.org/licenses/by/4.0/", "page": "https://freesound.org/s/2/",
            "audio": "", "filetype": "mp3", "seconds": 1.0});
        std::fs::write(dir.join("phrase.wav.json"), found.to_string()).unwrap();
        let g = crate::genres::get("house").unwrap();
        let o = compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = compose::generate(&g, 3, &o, None);
        let keys = bp.parts[0].name.clone();
        let path = phrase.display().to_string();
        bp.parts[0].sound = crate::model::Sound::Sample { path, root: 60, start: 0.0, end: None, label: "keys".into() };
        let cut = sources(&bp, MIX, 0, 1, take::credited).unwrap();
        assert_eq!(cut.recordings.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["freesound:2"]);
        let kick = sources(&bp, &bp.drums[0].name, 0, 1, take::credited).unwrap();
        assert!(kick.recordings.is_empty(), "a record of the kick was cut from no recording");
        // the take of a scratch on the mix, kept with what it was cut from, credits the phrase with the keys gone
        let file = take::store(&tone, &dir).unwrap();
        take::keep_sources(&file, &cut).unwrap();
        let from = bp.form[0].name.clone();
        take::add(&mut bp, &file, 1.0, &from, Some("scratch1"), None).unwrap();
        bp.parts.retain(|p| p.name != keys);
        for s in bp.form.iter_mut() {
            s.layers.retain(|l| *l != keys);
        }
        let credits = crate::song::credits(&compose::arrange(&bp));
        assert!(credits.contains("\"Keys\" by someone"), "{credits}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
