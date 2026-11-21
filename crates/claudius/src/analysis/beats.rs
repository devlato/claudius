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

/// How well a beat of `p` frames lays a grid on the onsets `env`: in each stretch of 16 beats (all of a shorter sound),
/// at the phase that suits the stretch, the onset strength on its beats, half as much on its eighths and a quarter on
/// its sixteenths, against that of the envelope as a whole. A tempo four thirds or two thirds of the beat, which a syncopated kick or a dotted
/// rhythm repeats at as often as at the beat, puts most of its grid between the onsets, and scores near 1.
fn salience(env: &[f32], p: f64) -> f64 {
    let near = widened(env, (0.06 * p).round().clamp(1.0, 8.0) as usize);
    let mean = near.iter().map(|v| *v as f64).sum::<f64>() / near.len().max(1) as f64;
    let at = |t: f64| near.get(t.round() as usize).copied().unwrap_or(0.0) as f64;
    // a stretch of 16 beats, or of as many as a shorter sound holds (a loop of four bars is 16 beats at its own tempo,
    // and 32 at twice it: both must be heard on it)
    let beats = ((near.len() as f64 / p).floor() as usize).saturating_sub(1).min(16);
    if beats < 4 {
        return 0.0;
    }
    let span = beats as f64 * p;
    let (mut total, mut stretches) = (0.0, 0usize);
    let mut start = 0.0;
    while start + span + p <= near.len() as f64 {
        let mut best = 0.0f64;
        let mut phase = 0.0;
        while phase < p {
            let s: f64 = (0..beats)
                .map(|k| {
                    let b = start + phase + k as f64 * p;
                    at(b) + 0.5 * at(b + p / 2.0) + 0.25 * (at(b + p / 4.0) + at(b + 0.75 * p))
                })
                .sum();
            best = best.max(s);
            phase += 1.0;
        }
        total += best / (2.0 * beats as f64);
        stretches += 1;
        start += span;
    }
    if stretches == 0 || mean <= 1e-12 { 0.0 } else { total / stretches as f64 / mean }
}

/// How a whole mix's tempo, once chosen as it always was, is moved to the octave people tap: half or twice it competes
/// when its grid lands on the onsets nearly as well as the best grid does (`OCTAVE_NEAR` of it), and wins by the prior
/// over the tempos songs are played at, by its backbeat, which counts six times as much here (`OCTAVE_BACKBEAT`) as in
/// choosing the tempo, and by how well its grid lands, to the power `OCTAVE_FIT`: a slow song whose grid at its own
/// tempo lands clearly better keeps it, however near twice it lies to the tempos songs are played at; its prior is
/// wider below its centre than above (`OCTAVE_BELOW`). Tuned on the
/// Freesound Loop Dataset's loops with people's tempos (`examples/eval_beats.rs`, its tuning half), confirmed on the
/// other half.
const OCTAVE_NEAR: f64 = 0.85;
const OCTAVE_BACKBEAT: f64 = 3.0;
const OCTAVE_FIT: f64 = 4.0;
/// How wide (in octaves) the prior over the tempos songs are played at is below its centre, in the octave choice: slow
/// songs are common, and a prior as narrow below as above pulls them up to twice their tempo
const OCTAVE_BELOW: f64 = 0.8;

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

/// The shortest and the longest a beat may last, against the tempo's beat. A band's beat strays a few percent from its
/// tempo and comes back; one that lasts three quarters of a beat, a half or twice as long, is the tracker taking a
/// syncopated riff or a bass line's notes for beats, and over a few bars it slips a whole beat into the song's count:
/// its bars slide across the beat, and a piece cut from them plays a quarter fast against claudius's own parts (heard on
/// a live band's record, Men at Work's "Down Under"). Narrower than this, a band's own drift no longer fits. These are
/// for a whole mix, where a riff or a bass line is heard as often as a drum, and which the measurements were of.
const GAPS: (f64, f64) = (0.9, 1.1);

/// The gaps a kit heard apart from the rest (its stems) leaves its beats, as before `GAPS`: not measured with the
/// narrower ones yet, so not changed.
const KIT_GAPS: (f64, f64) = (0.5, 2.0);

/// How much the downbeat of a whole mix heeds its kit's backbeat, beside the kick (0.5), the harmony change (1.5) and the
/// level jumps (0.75): a song whose chords change a push ahead of the bar (pop, rock and reggae often do) has its
/// harmony cue on the beat before it, and only the snares on two and four tell the bar's one from its four.
const BACKBEAT_DOWNBEAT: f64 = 1.5;

/// How clearly a whole mix's kit has to play a backbeat for it to count in the downbeat: the difference its phases score
/// at least this share of its hits' whole level. A backbeat that is there, a snare on two and four over any kick, scores
/// more than half of it; a kit striking every beat alike (four on the floor with no snare, or a kick and a hat on every
/// beat) plays none, and the few beats a song's length leaves over on one side must not be taken for one.
const BACKBEAT_CLEAR: f64 = 0.1;

/// The backbeat as the downbeat weighs it (`backbeat_by_phase`'s scores, `level` the kit's hits' whole level): each
/// phase from 0 to 1 when the kit plays a backbeat clearly (`BACKBEAT_CLEAR`), else nothing for any.
fn backbeat_cue(by_phase: [f64; 4], level: f64) -> [f64; 4] {
    let (lo, hi) = by_phase.iter().fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    if hi - lo < BACKBEAT_CLEAR * level || hi - lo < 1e-9 { [0.0; 4] } else { by_phase.map(|x| (x - lo) / (hi - lo)) }
}

/// The kit's backbeat for each of the four beat phases a bar could start on: the kicks on the bar's first and third
/// beats and the snares on its second and fourth, less the snares on one and three and the kicks on two and four, each
/// hit counted on the beat it falls within an eighth of a beat of, as hard as it is struck. A phase and the one two
/// beats on score alike: it tells a bar's one and three from its two and four, and the other cues tell one from three.
fn backbeat_by_phase(beats: &[f64], hits: &[Hit], beat_s: f64) -> [f64; 4] {
    let (mut kick_at, mut snare_at) = (vec![0.0f64; beats.len()], vec![0.0f64; beats.len()]);
    for h in hits.iter().filter(|h| h.kind != Kind::Hat) {
        let i = beats.partition_point(|&b| b < h.t);
        let near = [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter(|&j| j < beats.len())
            .min_by(|&a, &b| (beats[a] - h.t).abs().total_cmp(&(beats[b] - h.t).abs()))
            .filter(|&j| (beats[j] - h.t).abs() <= beat_s / 8.0);
        if let Some(j) = near {
            if h.kind == Kind::Snare {
                snare_at[j] += h.level as f64;
            } else {
                kick_at[j] += h.level as f64;
            }
        }
    }
    std::array::from_fn(|ph| {
        (0..beats.len())
            .map(|i| if (i + ph) % 2 == 0 { kick_at[i] - snare_at[i] } else { snare_at[i] - kick_at[i] })
            .sum()
    })
}

/// The beats of `env` (onset strength per frame) at a beat of about `p` frames, by dynamic programming (Ellis 2007):
/// each beat earns the onset strength under it and pays for straying from `p`, by the square of the log of how far,
/// and a beat lasts `gaps` of `p` at the least and the most. The frames of the beats, first to last.
fn beat_frames(env: &[f32], p: f64, gaps: (f64, f64)) -> Vec<usize> {
    let n = env.len();
    let mut score: Vec<f64> = env.iter().map(|v| *v as f64).collect();
    let mut back = vec![usize::MAX; n];
    for t in 0..n {
        let a = (t as f64 - gaps.1 * p).max(0.0) as usize;
        let b = (t as f64 - gaps.0 * p).max(0.0) as usize;
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
    let mut t = (tail..n).max_by(|a, b| score[*a].total_cmp(&score[*b])).unwrap_or(0);
    let mut frames = Vec::new();
    while t != usize::MAX {
        frames.push(t);
        t = back[t];
    }
    frames.reverse();
    frames
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
    let chosen = heard
        .iter()
        .filter(|h| h.1 >= near_best * most)
        .map(|&(p, _, b)| (prior(p) * (backbeat_weight * b).exp(), p))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(fps * 0.5, |(_, p)| p);
    // a whole mix's tempo then moves to the octave people tap: to half or twice it where that one's grid lands on the
    // onsets nearly as well, it lies nearer the tempos songs are played at, and the mix's drums play a backbeat at it.
    // Only its octave moves: it is the same tempo at half or twice it as the one chosen.
    let period = if kit || heard.is_empty() {
        chosen
    } else {
        // the prior again, wider below its centre (`OCTAVE_BELOW`)
        let leaning = |p: f64| {
            let o = (60.0 * fps / p / centre).log2();
            (-0.5 * (o / if o < 0.0 { OCTAVE_BELOW } else { 0.6 }).powi(2)).exp()
        };
        [chosen, chosen / 2.0, chosen * 2.0]
            .into_iter()
            .filter(|q| (lag_min..=lag_max).contains(q))
            .map(|q| (q, salience(&env, q) / most.max(1e-9)))
            .filter(|&(q, fit)| q == chosen || fit >= OCTAVE_NEAR)
            .map(|(q, fit)| (leaning(q) * (OCTAVE_BACKBEAT * backbeat(&hits, q / fps)).exp() * fit.powf(OCTAVE_FIT), q))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(chosen, |(_, q)| q)
    };
    if std::env::var_os("CLAUDIUS_DEBUG").is_some() {
        let mut seen: Vec<String> = heard
            .iter()
            .map(|&(p, s, b)| format!("{:.1}: salience {s:.2} prior {:.2} backbeat {b:+.2}", 60.0 * fps / p, prior(p)))
            .collect();
        seen.sort();
        eprintln!("beats: candidates {}", seen.join("; "));
    }
    let clarity = ac.get(period.round() as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let p = period;
    let gaps = if kit { KIT_GAPS } else { GAPS };
    let tracked: Vec<f64> = beat_frames(&env, p, gaps).iter().map(|f| (*f * HOP + HOP / 2) as f64 / SRF).collect();
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
    // a whole mix's kit, heard among the chords that move ahead of the bar, says where its backbeat is; a drum stem's
    // kick and harmony already say where the bar starts
    let backbeat_w = if kit { 0.0 } else { BACKBEAT_DOWNBEAT };
    let back_p = backbeat_by_phase(&beats, &hits, beat_s);
    let kit_level: f64 = hits.iter().filter(|h| h.kind != Kind::Hat).map(|h| h.level as f64).sum();
    let (kn, cn, jn, bn) = (norm(kick_p), norm(change_p), norm(jump_p), backbeat_cue(back_p, kit_level));
    // harmony change leads: level jumps also come from pre-drop gaps and risers, the kick from rolling basses
    let phase_score: [f64; 4] = std::array::from_fn(|i| 0.5 * kn[i] + 1.5 * cn[i] + 0.75 * jn[i] + backbeat_w * bn[i]);
    if std::env::var_os("CLAUDIUS_DEBUG").is_some() {
        eprintln!(
            "beats: first {:.3} s, {} beats; per phase: kick {kn:.2?} harmony change {cn:.2?} jumps {jn:.2?} backbeat {bn:.2?} total {phase_score:.2?}",
            beats.first().copied().unwrap_or(0.0),
            beats.len()
        );
    }
    let downbeat = (0..4).max_by(|a, b| phase_score[*a].partial_cmp(&phase_score[*b]).unwrap()).unwrap_or(0);
    Grid { bpm: (bpm * 10.0).round() / 10.0, beats, downbeat, clarity, steady }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_is_heard_on_a_loop_of_four_bars_at_its_own_tempo_as_at_twice_it() {
        // 16 beats of 120 frames, struck on each beat and more softly on each eighth, and nothing after
        let p = 120.0;
        let mut env = vec![0.0f32; 16 * 120 + 30];
        for k in 0..32 {
            env[k * 60] = if k % 2 == 0 { 1.0 } else { 0.5 };
        }
        let (own, twice) = (salience(&env, p), salience(&env, p / 2.0));
        assert!(own > 3.0 && own > 0.8 * twice, "at its tempo {own:.2}, at twice it {twice:.2}");
        // a grid a third off the beat lies between the strikes
        assert!(salience(&env, 0.75 * p) < 0.6 * own);
    }

    #[test]
    fn the_backbeat_tells_the_bars_one_and_three_from_its_two_and_four() {
        // a kit of 16 beats of half a second: kicks on the bar's one and three, snares on two and four, played a little
        // behind and ahead of the beat
        let beats: Vec<f64> = (0..16).map(|k| k as f64 * 0.5).collect();
        let hits: Vec<Hit> = beats
            .iter()
            .enumerate()
            .map(|(k, &t)| Hit {
                t: t + if k % 2 == 0 { 0.02 } else { -0.015 },
                kind: if k % 2 == 0 { Kind::Kick } else { Kind::Snare },
                level: 1.0,
            })
            .collect();
        let back = backbeat_by_phase(&beats, &hits, 0.5);
        assert!(back[0] > 0.0 && back[2] > 0.0, "the bar starting on a kick: {back:?}");
        assert!(back[1] < 0.0 && back[3] < 0.0, "the bar starting on a snare: {back:?}");
        assert_eq!(back[0], back[2]);
        // hats say nothing of it, and a hit more than an eighth of a beat from every beat is no beat's
        let more: Vec<Hit> = hits
            .iter()
            .cloned()
            .chain([Hit { t: 0.25, kind: Kind::Hat, level: 5.0 }, Hit { t: 1.25, kind: Kind::Snare, level: 5.0 }])
            .collect();
        assert_eq!(backbeat_by_phase(&beats, &more, 0.5), back);
    }

    /// A kit that plays a backbeat clearly says which beats are one and three; one that strikes every beat alike says
    /// nothing, however many beats one side has more of, so it cannot outweigh where a song's level jumps.
    #[test]
    fn only_a_backbeat_played_clearly_counts_in_the_downbeat() {
        // the snare on two and four over a kick on one and three: the phases one beat apart as far apart as can be
        assert_eq!(backbeat_cue([8.0, -8.0, 8.0, -8.0], 16.0), [1.0, 0.0, 1.0, 0.0]);
        // a kick alike on all 33 beats: the side with one beat more is no backbeat
        assert_eq!(backbeat_cue([1.0, -1.0, 1.0, -1.0], 33.0), [0.0; 4]);
        assert_eq!(backbeat_cue([0.0; 4], 0.0), [0.0; 4]);
    }

    #[test]
    fn a_riff_repeating_at_three_quarters_of_the_beat_slips_no_beat_into_the_count() {
        // a beat of 100 frames: strikes on 20 beats, then nine beats where only a riff strikes, every three quarters of
        // a beat (twelve strikes, as strong as the beat's), then 20 beats again. A tracker that lets a beat last three
        // quarters of one follows the riff and counts three beats too many (53, where 50 belong).
        let p = 100.0;
        let mut env = vec![0.0f32; 50 * 100];
        let strike = |env: &mut Vec<f32>, at: f64| env[at.round() as usize] = 20.0;
        for k in 0..20 {
            strike(&mut env, k as f64 * p + 50.0);
        }
        for j in 0..12 {
            strike(&mut env, 20.0 * p + 50.0 + j as f64 * 0.75 * p);
        }
        for k in 29..49 {
            strike(&mut env, k as f64 * p + 50.0);
        }
        let beats = beat_frames(&env, p, GAPS);
        let on = |at: f64| beats.iter().position(|&b| (b as f64 - at).abs() <= 3.0);
        let first = on(50.0).expect("a beat on the first strike");
        for k in 30..48 {
            let i = on(k as f64 * p + 50.0).unwrap_or_else(|| panic!("no beat on strike {k}: {beats:?}"));
            assert_eq!(i - first, k, "beat {k} after the riff counted as {}", i - first);
        }
    }
}
