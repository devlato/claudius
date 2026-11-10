//! Tempo and beats: autocorrelation of the onset envelope (with a soft prior around 120 BPM, or a range the caller
//! knows), dynamic-programming beat tracking (Ellis 2007), and the downbeat as the beat phase where kicks and
//! harmony changes land. Music made in a DAW keeps one tempo, and its beats are a grid of it; a band speeds up and
//! slows down, and its beats are the ones it plays, followed through the song.

use super::chroma;
use super::drums::{self, Hit, Kind};
use super::onsets::{self, HOP};
use crate::audio::Stereo;
use crate::config::SRF;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Grid {
    pub bpm: f64,
    /// beat times (s)
    pub beats: Vec<f64>,
    /// index into `beats` of the first downbeat
    pub downbeat: usize,
    /// 0..1: how clearly the music pulses at this tempo
    pub clarity: f64,
    /// whether it keeps one tempo, its beats a grid of `bpm`; else they follow the beats played, and `bpm` is their
    /// middle tempo
    pub steady: bool,
}

/// Beats that keep each skipped one and lose each one slipped in between two: a gap of about `k` beats gets `k - 1`
/// beats spread through it, and a beat less than half a beat after the one before is left out.
fn repaired(tracked: &[f64], median: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(tracked.len());
    for &t in tracked {
        let Some(&last) = out.last() else {
            out.push(t);
            continue;
        };
        let k = ((t - last) / median).round();
        if k < 1.0 {
            continue;
        }
        for j in 1..k as usize {
            out.push(last + (t - last) * j as f64 / k);
        }
        out.push(t);
    }
    out
}

/// the seconds a beat of `beats` lasts: the slope of the line through them against their count, by least squares
fn slope(beats: &[f64]) -> f64 {
    let m = beats.len() as f64;
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
    for (k, &t) in beats.iter().enumerate() {
        let x = k as f64;
        (sx, sy, sxx, sxy) = (sx + x, sy + t, sxx + x * x, sxy + x * t);
    }
    let d = m * sxx - sx * sx;
    if d.abs() < 1e-12 { 0.5 } else { (m * sxy - sx * sy) / d }
}

/// Each beat moved onto the tempo the beats around it keep: the line through it and the `w` on either side, by least
/// squares, which takes out the tracker's jitter and keeps a band's speeding up and slowing down.
fn smoothed(beats: &[f64], w: usize) -> Vec<f64> {
    let n = beats.len();
    (0..n)
        .map(|i| {
            let (a, b) = (i.saturating_sub(w), (i + w + 1).min(n));
            let m = (b - a) as f64;
            let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
            for (k, &t) in beats.iter().enumerate().take(b).skip(a) {
                let x = k as f64;
                (sx, sy, sxx, sxy) = (sx + x, sy + t, sxx + x * x, sxy + x * t);
            }
            let d = m * sxx - sx * sx;
            if d.abs() < 1e-12 {
                return beats[i];
            }
            let slope = (m * sxy - sx * sy) / d;
            (sy - slope * sx) / m + slope * i as f64
        })
        .collect()
}

/// Whether `played` drifts off the grid of `period` seconds from `first`: somewhere 16 beats running sit off it, the
/// middle of them more than an eighth of a beat one way.
fn drifts(played: &[f64], first: f64, period: f64) -> bool {
    let off: Vec<f64> = played
        .iter()
        .map(|&t| {
            let k = ((t - first) / period).round();
            (t - first - k * period) / period
        })
        .collect();
    off.windows(16).any(|w| {
        let mut s = w.to_vec();
        s.sort_by(f64::total_cmp);
        s[s.len() / 2].abs() > 0.125
    })
}

fn autocorr(env: &[f32], max_lag: usize) -> Vec<f64> {
    let n = env.len();
    let m = env.iter().map(|v| *v as f64).sum::<f64>() / n.max(1) as f64;
    let x: Vec<f64> = env.iter().map(|v| *v as f64 - m).collect();
    let r0: f64 = x.iter().map(|v| v * v).sum::<f64>().max(1e-12);
    (0..max_lag.min(n)).map(|l| x[..n - l].iter().zip(&x[l..]).map(|(a, b)| a * b).sum::<f64>() / r0).collect()
}

/// The strongest of `env` within `w` frames of each frame: a swung sixteenth or a laid-back snare still lands on a
/// grid that has it a few milliseconds away.
fn widened(env: &[f32], w: usize) -> Vec<f32> {
    (0..env.len())
        .map(|i| env[i.saturating_sub(w)..(i + w + 1).min(env.len())].iter().fold(0f32, |m, v| m.max(*v)))
        .collect()
}

/// How well a beat of `p` frames lays a grid on the onsets `env`: in each stretch of 16 beats, at the phase that suits
/// the stretch, the onset strength on its beats, half as much on its eighths and a quarter on its sixteenths, against
/// that of the envelope as a whole. A tempo four thirds or two thirds of the beat, which a syncopated kick or a dotted
/// rhythm repeats at as often as at the beat, puts most of its grid between the onsets, and scores near 1.
fn salience(env: &[f32], p: f64) -> f64 {
    let near = widened(env, (0.06 * p).round().clamp(1.0, 8.0) as usize);
    let mean = near.iter().map(|v| *v as f64).sum::<f64>() / near.len().max(1) as f64;
    let at = |t: f64| near.get(t.round() as usize).copied().unwrap_or(0.0) as f64;
    let span = 16.0 * p;
    let (mut total, mut stretches) = (0.0, 0usize);
    let mut start = 0.0;
    while start + span + p <= near.len() as f64 {
        let mut best = 0.0f64;
        let mut phase = 0.0;
        while phase < p {
            let s: f64 = (0..16)
                .map(|k| {
                    let b = start + phase + k as f64 * p;
                    at(b) + 0.5 * at(b + p / 2.0) + 0.25 * (at(b + p / 4.0) + at(b + 0.75 * p))
                })
                .sum();
            best = best.max(s);
            phase += 1.0;
        }
        total += best / 32.0;
        stretches += 1;
        start += span;
    }
    if stretches == 0 || mean <= 1e-12 { 0.0 } else { total / stretches as f64 / mean }
}

/// How much the kit (`hits`) plays a backbeat at a beat of `beat` seconds: in a cycle of two beats, laid where it
/// suits them best, the snares on its second beat and the kicks on its first, less the snares on the first and the
/// kicks on the second, against all of both, each counted within a 32nd note of the beat, as hard as it is struck. A
/// kick on one and a snare on two and four give most; at twice the tempo the snare falls on the kick's beat, at half
/// of it between the beats, and both give less. Each stretch of eight bars is laid on its own, as a beat a little off
/// the music's drifts off it over a song.
fn backbeat(hits: &[Hit], beat: f64) -> f64 {
    let span = 32.0 * beat;
    let mut stretches: Vec<Vec<(f64, f64, bool)>> = Vec::new();
    for h in hits.iter().filter(|h| h.kind != Kind::Hat) {
        let i = (h.t / span) as usize;
        if stretches.len() <= i {
            stretches.resize(i + 1, Vec::new());
        }
        stretches[i].push(((h.t / beat).rem_euclid(2.0), h.level as f64, h.kind == Kind::Snare));
    }
    let near = |at: f64, to: f64| {
        let d = (at - to).rem_euclid(2.0);
        d.min(2.0 - d) <= 0.125
    };
    let (mut got, mut all) = (0.0, 0.0);
    for s in &stretches {
        let best = (0..64)
            .map(|k| {
                let one = k as f64 / 32.0;
                s.iter()
                    .map(|&(at, w, snare)| match (near(at, one), near(at, one + 1.0), snare) {
                        (_, true, true) | (true, _, false) => w,
                        (true, _, true) | (_, true, false) => -w,
                        _ => 0.0,
                    })
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        got += best;
        all += s.iter().map(|h| h.1).sum::<f64>();
    }
    if all <= 1e-12 { 0.0 } else { got / all }
}

/// the lag of a peak of `ac` at `l`, between frames: the vertex of the parabola through it and its neighbours
fn peak_lag(ac: &[f64], l: usize) -> f64 {
    let (a, b, c) = (ac[l - 1], ac[l], ac[l + 1]);
    let d = a - 2.0 * b + c;
    if d.abs() < 1e-12 { l as f64 } else { l as f64 + (0.5 * (a - c) / d).clamp(-0.5, 0.5) }
}

fn standardise(v: &[f32]) -> Vec<f32> {
    let mean = v.iter().map(|x| *x as f64).sum::<f64>() / v.len().max(1) as f64;
    let sd = (v.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt().max(1e-9);
    v.iter().map(|x| (*x as f64 / sd) as f32).collect()
}

pub fn track(mono: &[f32], range: Option<(f64, f64)>) -> Grid {
    track_with(mono, mono, range, false)
}

/// The beats heard in `mono`'s onsets (a recording's drums, apart from it, are the clearest; `kit` when `mono` is
/// them alone) and its downbeat in `harmony` as well (where its chords change and its level jumps), the two as long
/// as each other.
pub fn track_with(mono: &[f32], harmony: &[f32], range: Option<(f64, f64)>, kit: bool) -> Grid {
    // in beat-driven music the kick marks the beat: the low band counts twice as much as the full-band flux
    let full = standardise(&onsets::flux(mono, None));
    let kick = standardise(&onsets::flux(mono, Some((30.0, 160.0))));
    let env: Vec<f32> = full.iter().zip(&kick).map(|(a, b)| a + 2.0 * b).collect();
    let hits = drums::hear(&Stereo::from_mono(mono));
    let fps = SRF / HOP as f64;
    let (lo, hi) = range.unwrap_or((55.0, 200.0));
    let max_lag = (fps * 60.0 / lo * 2.2) as usize + 2;
    let ac = autocorr(&env, max_lag);
    let centre = range.map_or(110.0, |(a, b)| (a * b).sqrt());
    let prior = |p: f64| (-0.5 * ((60.0 * fps / p / centre).log2() / 0.6).powi(2)).exp();
    let lag_lo = (fps * 60.0 / hi).floor().max(2.0) as usize;
    let lag_hi = ((fps * 60.0 / lo).ceil() as usize).min(ac.len().saturating_sub(1) / 2);
    // the beats the onsets repeat at most (the autocorrelation's peaks), each at that, at twice and at half its tempo;
    // of those whose grid lands on the onsets nearly as well as the best one's (a tempo at twice or half the music's
    // does, one a third or a quarter off it does not), the one nearest the tempos songs are played at (about 110 BPM)
    // whose kit plays a backbeat. The kit apart from the rest is heard well enough to let its backbeat outweigh that
    // tempo; in a whole mix, where a chord or a bass note is heard as a hit as often as a drum is, hardly at all.
    let (near_best, backbeat_weight) = if kit { (0.75, 3.0) } else { (0.95, 0.5) };
    let mut peaks: Vec<(f64, usize)> = (lag_lo.max(1)..=lag_hi.max(lag_lo))
        .filter(|&l| l + 1 < ac.len() && ac[l] >= ac[l - 1] && ac[l] >= ac[l + 1])
        .map(|l| (ac[l] + 0.5 * ac.get(2 * l).copied().unwrap_or(0.0), l))
        .collect();
    peaks.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (lag_min, lag_max) = (lag_lo as f64, lag_hi.max(lag_lo) as f64 + 0.5);
    let mut candidates: Vec<f64> = Vec::new();
    for p in peaks.iter().take(6).map(|&(_, l)| peak_lag(&ac, l)) {
        for q in [p, p / 2.0, p * 2.0] {
            if (lag_min..=lag_max).contains(&q) && !candidates.iter().any(|c| (c / q - 1.0).abs() < 0.02) {
                candidates.push(q);
            }
        }
    }
    let heard: Vec<(f64, f64, f64)> =
        candidates.iter().map(|&p| (p, salience(&env, p), backbeat(&hits, p / fps))).collect();
    let most = heard.iter().map(|h| h.1).fold(0.0, f64::max);
    let period = heard
        .iter()
        .filter(|h| h.1 >= near_best * most)
        .map(|&(p, _, b)| (prior(p) * (backbeat_weight * b).exp(), p))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(fps * 0.5, |(_, p)| p);
    if std::env::var_os("CLAUDIUS_DEBUG").is_some() {
        let mut seen: Vec<String> = heard
            .iter()
            .map(|&(p, s, b)| format!("{:.1}: salience {s:.2} prior {:.2} backbeat {b:+.2}", 60.0 * fps / p, prior(p)))
            .collect();
        seen.sort();
        eprintln!("beats: candidates {}", seen.join("; "));
    }
    let clarity = ac.get(period.round() as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    // dynamic programming: each beat rewards onset strength and penalises deviating from the period
    let n = env.len();
    let mut score: Vec<f64> = env.iter().map(|v| *v as f64).collect();
    let mut back = vec![usize::MAX; n];
    let p = period;
    for t in 0..n {
        let a = (t as f64 - 2.0 * p).max(0.0) as usize;
        let b = (t as f64 - p / 2.0).max(0.0) as usize;
        let mut bs = f64::MIN;
        let mut bi = usize::MAX;
        for tau in a..b.min(t) {
            let d = ((t - tau) as f64 / p).ln();
            let s = score[tau] - 100.0 * d * d;
            if s > bs {
                bs = s;
                bi = tau;
            }
        }
        if bi != usize::MAX && bs > 0.0 {
            score[t] += bs;
            back[t] = bi;
        }
    }
    let tail = (n as f64 - p).max(0.0) as usize;
    let mut t = (tail..n).max_by(|a, b| score[*a].partial_cmp(&score[*b]).unwrap()).unwrap_or(0);
    let mut frames = Vec::new();
    while t != usize::MAX {
        frames.push(t);
        t = back[t];
    }
    frames.reverse();
    let tracked: Vec<f64> = frames.iter().map(|f| (*f * HOP + HOP / 2) as f64 / SRF).collect();
    // the beats as the tracker heard them, a skipped one put back and one slipped in left out
    let heard = (tracked.len() > 8).then(|| {
        let mut ibi: Vec<f64> = tracked.windows(2).map(|w| w[1] - w[0]).collect();
        ibi.sort_by(f64::total_cmp);
        repaired(&tracked, ibi[ibi.len() / 2])
    });
    // their tempo: the slope of a line through them, finer than the frames each falls on (a median of the gaps between
    // them is a whole number of frames, which at 96 BPM is 117 or 118, nearly 1 % apart)
    let bpm0 = if let Some(h) = heard.as_ref().filter(|h| h.len() > 8) { 60.0 / slope(h) } else { 60.0 * fps / p };
    // music made in a DAW keeps one tempo: fit the constant grid (tempo within 0.5 %, every phase) that lands on
    // the most onset energy; the dynamic-programming beats drift and insert beats through breakdowns
    let sample = |pos: f64| -> f64 {
        let i = pos.floor() as usize;
        if i + 1 >= env.len() {
            return 0.0;
        }
        let f = pos - i as f64;
        env[i] as f64 * (1.0 - f) + env[i + 1] as f64 * f
    };
    // around each tempo the beats were heard at: their slope (which beats slipping onto the offbeats bend), the median
    // gap between them, and the onsets' own period
    let median_gap = heard.as_ref().filter(|h| h.len() > 8).map(|h| {
        let mut ibi: Vec<f64> = h.windows(2).map(|w| w[1] - w[0]).collect();
        ibi.sort_by(f64::total_cmp);
        60.0 / ibi[ibi.len() / 2]
    });
    let mut best_grid = (f64::MIN, bpm0, 0.0f64);
    for centre in [Some(bpm0), median_gap, Some(60.0 * fps / p)].into_iter().flatten() {
        let mut b = centre * 0.995;
        while b <= centre * 1.005 {
            let per = fps * 60.0 / b;
            let mut off = 0.0;
            while off < per {
                let mut s = 0.0;
                let mut pos = off;
                while pos < env.len() as f64 - 1.0 {
                    s += sample(pos);
                    pos += per;
                }
                if s > best_grid.0 {
                    best_grid = (s, b, off);
                }
                off += 1.0;
            }
            b += 0.005;
        }
    }
    let (_, grid_bpm, off) = best_grid;
    let per_s = 60.0 / grid_bpm;
    let first = (off * HOP as f64 + HOP as f64 / 2.0) / SRF;
    let secs = mono.len() as f64 / SRF;
    let grid: Vec<f64> = (0..).map(|k| first + k as f64 * per_s).take_while(|t| *t < secs).collect();
    // a band's beats drift off any one tempo: where those it plays sit off the grid and land on more of the onsets,
    // they are the beats
    let strength = |beats: &[f64]| -> f64 {
        let at = |t: f64| {
            let c = (t * fps - 0.5).round() as i64;
            (c - 2..=c + 2)
                .filter_map(|i| usize::try_from(i).ok().and_then(|i| env.get(i)))
                .fold(0.0f32, |m, v| m.max(*v))
        };
        beats.iter().map(|&t| at(t) as f64).sum::<f64>() / beats.len().max(1) as f64
    };
    let played = heard.as_ref().filter(|h| h.len() >= 16).map(|h| smoothed(h, 8));
    if std::env::var_os("CLAUDIUS_DEBUG").is_some()
        && let Some(p) = &played
    {
        let off: Vec<String> = p
            .iter()
            .step_by(8)
            .map(|&t| format!("{:.2}", (t - first - ((t - first) / per_s).round() * per_s) / per_s))
            .collect();
        eprintln!(
            "beats: bpm0 {bpm0:.3} autocorr {:.3} grid {grid_bpm:.3} first {first:.3}; played {} vs grid {}: drifts {} strength {:.3} vs {:.3}; offsets {}",
            60.0 * fps / period,
            p.len(),
            grid.len(),
            drifts(p, first, per_s),
            strength(p),
            strength(&grid),
            off.join(" ")
        );
    }
    let (beats, bpm, steady) = match played {
        Some(p) if drifts(&p, first, per_s) && strength(&p) > 1.05 * strength(&grid) => {
            let mut ibi: Vec<f64> = p.windows(2).map(|w| w[1] - w[0]).collect();
            ibi.sort_by(f64::total_cmp);
            let bpm = 60.0 / ibi[ibi.len() / 2];
            (p.into_iter().filter(|t| (0.0..secs).contains(t)).collect::<Vec<_>>(), bpm, false)
        }
        _ => (grid, grid_bpm, true),
    };
    // downbeat: the phase where the harmony changes (mean chroma over the half-beat before vs after the beat,
    // bass included) and the low end hits hardest
    let low = onsets::flux(mono, Some((30.0, 150.0)));
    let low_sd = (low.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / low.len().max(1) as f64).sqrt().max(1e-9);
    let chop = HOP * 4;
    let ch = chroma::chroma_band(harmony, chop, 110.0, 2000.0);
    let beat_s = 60.0 / bpm.max(1.0);
    let mean_chroma = |a: f64, b: f64| -> [f64; 12] {
        let (fa, fb) = (((a * SRF) / chop as f64).max(0.0) as usize, ((b * SRF) / chop as f64) as usize);
        let mut m = [0.0f64; 12];
        for f in ch.iter().take(fb.min(ch.len())).skip(fa) {
            for i in 0..12 {
                m[i] += f[i] as f64;
            }
        }
        m
    };
    let cos_dist = |a: &[f64; 12], c: &[f64; 12]| {
        let d = a.iter().zip(c).map(|(x, y)| x * y).sum::<f64>();
        let n = a.iter().map(|x| x * x).sum::<f64>().sqrt() * c.iter().map(|x| x * x).sum::<f64>().sqrt();
        if n > 1e-12 { 1.0 - d / n } else { 0.0 }
    };
    let rms_at = |a: f64, b: f64| -> f64 {
        let (sa, sb) = (((a * SRF) as usize).min(harmony.len()), ((b * SRF) as usize).min(harmony.len()));
        if sb <= sa {
            return -120.0;
        }
        10.0 * (harmony[sa..sb].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (sb - sa) as f64).max(1e-14).log10()
    };
    // three cues per beat phase, each scaled to its best phase: kick strength, harmony change (a bar of chroma
    // before vs after), and votes from the biggest level jumps (sections start on downbeats)
    let mut kick_p = [0.0f64; 4];
    let mut change_p = [0.0f64; 4];
    let mut jumps: Vec<(f64, usize)> = Vec::new();
    for (i, &b) in beats.iter().enumerate() {
        kick_p[i % 4] += low.get((b * SRF / HOP as f64) as usize).copied().unwrap_or(0.0) as f64 / low_sd;
        change_p[i % 4] += cos_dist(
            &mean_chroma(b - beat_s * 4.0, b - beat_s * 0.05),
            &mean_chroma(b + beat_s * 0.05, b + beat_s * 4.0),
        );
        jumps.push(((rms_at(b, b + 4.0 * beat_s) - rms_at(b - 4.0 * beat_s, b)).abs(), i % 4));
    }
    jumps.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut jump_p = [0.0f64; 4];
    for (j, ph) in jumps.iter().take(12) {
        jump_p[*ph] += j;
    }
    let norm = |v: [f64; 4]| -> [f64; 4] {
        let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        if hi - lo < 1e-9 { [0.0; 4] } else { v.map(|x| (x - lo) / (hi - lo)) }
    };
    let (kn, cn, jn) = (norm(kick_p), norm(change_p), norm(jump_p));
    // harmony change leads: level jumps also come from pre-drop gaps and risers, the kick from rolling basses
    let phase_score: [f64; 4] = std::array::from_fn(|i| 0.5 * kn[i] + 1.5 * cn[i] + 0.75 * jn[i]);
    if std::env::var_os("CLAUDIUS_DEBUG").is_some() {
        eprintln!(
            "beats: first {:.3} s, {} beats; per phase: kick {kn:.2?} harmony change {cn:.2?} jumps {jn:.2?} total {phase_score:.2?}",
            beats.first().copied().unwrap_or(0.0),
            beats.len()
        );
    }
    let downbeat = (0..4).max_by(|a, b| phase_score[*a].partial_cmp(&phase_score[*b]).unwrap()).unwrap_or(0);
    Grid { bpm: (bpm * 10.0).round() / 10.0, beats, downbeat, clarity, steady }
}
