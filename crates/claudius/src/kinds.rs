//! The pieces of a recording the sampler offers besides its sung phrases (design 0021): loops of its hook and of
//! another passage, its drums as a break and its bass line, cut on its bars from its stems (from the mix, without the
//! ears), each scored for what makes a piece of its kind good. That is how alone what it is cut from plays in its bars
//! (its isolation, in decibels over the rest), how well it loops (its first beat against the beat after its end, as
//! LoopMaker measures a seam: Shi & Mysore, CHI 2018), and what its kind asks besides: a break how busy it is, a bass
//! line how much it moves, a hook how much of the recording it stands for, another passage how unlike the hook it is.
//! Each feature counts by its rank among the candidates of the same kind, so that no kind is judged on another's scale.

use crate::analysis::chroma;
use crate::analysis::song::{Analysis, SectionGuess};
use crate::audio::Stereo;
use crate::config::SRF;
use crate::dsp::fft;
use crate::model::SectionKind;
use crate::rng::Rng;
use crate::stems::Stems;

/// What a piece of a recording is, as producers sample records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// a phrase it sings whole (`sampler::pieces`)
    #[default]
    Phrase,
    /// bars of its hook, the section with chords that pulls most toward being one, without its drums, bass and voice
    Hook,
    /// bars of another passage with chords, without its drums, bass and voice
    Passage,
    /// its drums
    Break,
    /// its bass
    Bass,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Phrase, Kind::Hook, Kind::Passage, Kind::Break, Kind::Bass];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Phrase => "phrase",
            Kind::Hook => "hook",
            Kind::Passage => "passage",
            Kind::Break => "break",
            Kind::Bass => "bass",
        }
    }

    pub fn parse(s: &str) -> Result<Kind, String> {
        let s = s.trim().to_lowercase();
        Kind::ALL.into_iter().find(|k| k.name() == s).ok_or_else(|| {
            let names: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
            format!("no kind of piece '{s}' (kinds: {})", names.join(", "))
        })
    }

    /// a piece of this kind `bars` bars long, in words: "a 2-bar drum break"
    pub fn said(self, bars: u32) -> String {
        match self {
            Kind::Phrase => "a sung phrase".into(),
            Kind::Hook => format!("a {bars}-bar loop of its hook"),
            Kind::Passage => format!("a {bars}-bar loop of another passage"),
            Kind::Break => format!("a {bars}-bar drum break"),
            Kind::Bass => format!("a {bars}-bar bass line"),
        }
    }

    /// `n` pieces of this kind, in words: "a drum break", "2 bass lines"
    pub fn counted(self, n: usize) -> String {
        let (one, many) = match self {
            Kind::Phrase => ("a phrase", "phrases"),
            Kind::Hook => ("a loop of its hook", "loops of its hook"),
            Kind::Passage => ("a loop of another passage", "loops of other passages"),
            Kind::Break => ("a drum break", "drum breaks"),
            Kind::Bass => ("a bass line", "bass lines"),
        };
        if n == 1 { one.into() } else { format!("{n} {many}") }
    }

    /// pieces of `kinds`, counted by kind in the order each kind first comes: "2 phrases and a drum break"
    pub fn counted_all(kinds: &[Kind]) -> String {
        let mut seen: Vec<(Kind, usize)> = Vec::new();
        for k in kinds {
            match seen.iter_mut().find(|(s, _)| s == k) {
                Some((_, n)) => *n += 1,
                None => seen.push((*k, 1)),
            }
        }
        let words: Vec<String> = seen.iter().map(|(k, n)| k.counted(*n)).collect();
        match words.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            Some((last, _)) => last.clone(),
            None => String::new(),
        }
    }

    /// whether it plays notes, which go into the song's key (a break plays none)
    pub fn tonal(self) -> bool {
        self != Kind::Break
    }

    /// the lengths a piece of it takes, in bars, each with how well that length suits it: a break and a bass line of 2
    /// or 4 bars, a loop of a hook of 4
    fn lengths(self) -> &'static [(u32, f64)] {
        match self {
            Kind::Phrase => &[],
            Kind::Hook => &[(1, 0.5), (2, 0.8), (4, 1.0), (8, 0.9)],
            Kind::Passage => &[(2, 0.8), (4, 1.0), (8, 0.9)],
            Kind::Break => &[(1, 0.6), (2, 1.0), (4, 1.0)],
            Kind::Bass => &[(1, 0.8), (2, 1.0), (4, 0.9)],
        }
    }
}

/// A piece of one of the kinds but a phrase: its bars, how good a piece of its kind it is, and why, in words.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub kind: Kind,
    /// its first bar in the recording (counting from 0) and how many it lasts
    pub first: usize,
    pub bars: u32,
    /// how good a piece of its kind it is among the candidates of its kind, 0 to 1
    pub score: f64,
    /// what it scored for, in words: "the drums alone, busy, it loops cleanly"
    pub about: String,
}

/// a stem plays alone in bars where it is this much louder than the rest together, in decibels
const ALONE_DB: f64 = 20.0;
/// what a piece is cut from has to play there: no quieter than this under its loudest bar, in decibels
const PRESENT_DB: f64 = 20.0;
/// how far under the best of its kind a piece may score and still be drawn for another take, and how strongly the
/// better are preferred among them: the smaller, the more (as a remix draws its hook)
const NEAR: f64 = 0.15;
const CHOOSY: f64 = 0.05;

/// What a piece of `kind` is cut from in `src`, and the rest: its stems when they are apart (a break the drums, a bass
/// line the bass, a loop the other instruments), else the mix's percussive part for a break and its harmonic part for a
/// loop; none for a bass line without the stems, which the mix does not give apart.
pub fn source(kind: Kind, src: &Stereo, stems: Option<&Stems>) -> Option<(Stereo, Stereo)> {
    let sum = |parts: &[&Stereo]| {
        let mut out = Stereo::zeros(parts.iter().map(|p| p.len()).max().unwrap_or(0));
        for p in parts {
            out.add_at(p, 0, 1.0, 1.0);
        }
        out
    };
    match (kind, stems) {
        (Kind::Phrase, _) => None,
        (Kind::Break, Some(s)) => Some((s.drums.clone(), sum(&[&s.vocals, &s.bass, &s.other]))),
        (Kind::Bass, Some(s)) => Some((s.bass.clone(), sum(&[&s.vocals, &s.drums, &s.other]))),
        (Kind::Hook | Kind::Passage, Some(s)) => Some((s.other.clone(), sum(&[&s.vocals, &s.drums, &s.bass]))),
        (Kind::Bass, None) => None,
        (Kind::Break | Kind::Hook | Kind::Passage, None) => {
            let mid: Vec<f32> = src.l.iter().zip(&src.r).map(|(l, r)| 0.5 * (l + r)).collect();
            let harmonic = fft::harmonic(&mid);
            let percussive: Vec<f32> = mid.iter().zip(&harmonic).map(|(m, h)| m - h).collect();
            let (h, p) = (Stereo { l: harmonic.clone(), r: harmonic }, Stereo { l: percussive.clone(), r: percussive });
            Some(if kind == Kind::Break { (p, h) } else { (h, p) })
        }
    }
}

/// A piece as a sample: as it was cut, its loudest moment at -1 dBFS, faded in and out over 2 ms, so that it loops
/// without a click and nothing of its bars is lost.
pub fn level(x: &Stereo) -> Stereo {
    let mut out = x.clone();
    let peak = out.peak();
    if peak > 0.0 {
        let g = 0.891 / peak;
        out.channels_mut().into_iter().for_each(|ch| ch.iter_mut().for_each(|s| *s *= g));
    }
    let fade = ((0.002 * SRF) as usize).min(out.len() / 4);
    out.fade(fade, fade);
    out
}

/// the mean square of `x` over the recording's bar `bar`, as `a` heard it
fn bar_power(a: &Analysis, x: &Stereo, bar: usize) -> f64 {
    let at = |t: f64| ((t * SRF).max(0.0) as usize).min(x.len());
    let (i0, i1) = (at(a.bar_start(bar)), at(a.bar_start(bar + 1)));
    let sum: f64 = x.l[i0..i1].iter().chain(&x.r[i0..i1]).map(|v| (*v as f64).powi(2)).sum();
    sum / (2 * (i1 - i0)).max(1) as f64
}

fn db(power: f64) -> f64 {
    10.0 * power.max(1e-12).log10()
}

/// what a beat of a recording sounds like, as a seam compares two: its pitch classes, the shape of its spectrum in eight
/// octave bands (decibels about their mean) and its loudness
struct Beat {
    chroma: [f32; 12],
    bands: [f64; 8],
    db: f64,
}

fn beat_at(x: &Stereo, from: f64, secs: f64) -> Option<Beat> {
    let (i0, i1) = ((from * SRF).max(0.0) as usize, ((from + secs) * SRF).max(0.0) as usize);
    if i1 > x.len() || i1 <= i0 + 2048 {
        return None;
    }
    let mono: Vec<f32> = (i0..i1).map(|i| 0.5 * (x.l[i] + x.r[i])).collect();
    let mut sum = [0f32; 12];
    for frame in chroma::chroma(&mono, 2048) {
        sum.iter_mut().zip(frame).for_each(|(s, v)| *s += v);
    }
    let frames = fft::spectrogram(&mono, 2048, 1024);
    let mut bands = [0f64; 8];
    for frame in &frames {
        for (k, m) in frame.iter().enumerate() {
            let hz = k as f64 * SRF / 2048.0;
            // octaves from 62.5 Hz to 16 kHz
            if let Some(b) = (hz >= 62.5).then(|| ((hz / 62.5).log2().floor() as usize).min(7)) {
                bands[b] += (*m as f64).powi(2);
            }
        }
    }
    let n = frames.len().max(1) as f64;
    let logs: Vec<f64> = bands.iter().map(|e| db(e / n)).collect();
    let mean = logs.iter().sum::<f64>() / 8.0;
    let mut shape = [0f64; 8];
    shape.iter_mut().zip(&logs).for_each(|(s, l)| *s = l - mean);
    let power = mono.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / mono.len().max(1) as f64;
    Some(Beat { chroma: sum, bands: shape, db: db(power) })
}

fn cosine(a: &[f32; 12], b: &[f32; 12]) -> f64 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32; 12]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let d = norm(a) * norm(b);
    if d > 0.0 { (dot / d) as f64 } else { 0.0 }
}

/// How well bars `first`..`first + bars` of `x` loop, 0 to 1: how much their first beat resembles the beat after their
/// end, which looping puts it in place of (LoopMaker's seam, Shi & Mysore, CHI 2018): 1.0 × their chroma distance +
/// 0.6 × how far apart their spectra's shapes are, over 12 dB a band, + 0.2 × how far apart their loudness is, over 12
/// dB. A half when nothing follows the bars in the recording.
pub fn loopability(a: &Analysis, x: &Stereo, first: usize, bars: u32) -> f64 {
    let beat = a.bar_secs / 4.0;
    let end = first + bars as usize;
    if end >= a.bars {
        return 0.5;
    }
    let (Some(head), Some(after)) = (beat_at(x, a.bar_start(first), beat), beat_at(x, a.bar_start(end), beat)) else {
        return 0.5;
    };
    let shape =
        head.bands.iter().zip(&after.bands).map(|(p, q)| (p - q).powi(2)).sum::<f64>().sqrt() / (12.0 * 8f64.sqrt());
    let seam = (1.0 - cosine(&head.chroma, &after.chroma))
        + 0.6 * shape.min(1.0)
        + 0.2 * ((head.db - after.db).abs() / 12.0).min(1.0);
    (1.0 - seam / 1.8).clamp(0.0, 1.0)
}

/// how busy a break of `x` over bars `first`..`first + bars` is, 0 to 1: the share of its sixteenths on which its
/// loudness rises sharply (to 1.6 times the sixteenth before, within 30 dB of its loudest)
fn busy(a: &Analysis, x: &Stereo, first: usize, bars: u32) -> f64 {
    let (from, step) = (a.bar_start(first), a.bar_secs / 16.0);
    let slots = 16 * bars as usize;
    let level: Vec<f64> = (0..slots)
        .map(|k| {
            let (i0, i1) = (((from + k as f64 * step) * SRF) as usize, ((from + (k + 1) as f64 * step) * SRF) as usize);
            let (i0, i1) = (i0.min(x.len()), i1.min(x.len()));
            x.l[i0..i1].iter().chain(&x.r[i0..i1]).map(|v| (*v as f64).powi(2)).sum::<f64>()
                / (2 * (i1 - i0)).max(1) as f64
        })
        .collect();
    let top = level.iter().copied().fold(0.0, f64::max);
    let hits = (1..slots).filter(|&k| level[k] > 1.6 * level[k - 1] && db(level[k]) > db(top) - 30.0).count();
    hits as f64 / slots as f64
}

/// how much a bass line of `x` over bars `first`..`first + bars` moves, 0 to 1: the share of its beats whose
/// strongest pitch class is not the one before
fn moves(a: &Analysis, x: &Stereo, first: usize, bars: u32) -> f64 {
    let beat = a.bar_secs / 4.0;
    let roots: Vec<Option<usize>> = (0..4 * bars as usize)
        .map(|k| {
            let b = beat_at(x, a.bar_start(first) + k as f64 * beat, beat)?;
            let (pc, top) = b.chroma.iter().enumerate().fold((0, 0f32), |m, (i, v)| if *v > m.1 { (i, *v) } else { m });
            (top > 0.0).then_some(pc)
        })
        .collect();
    let changes = roots.windows(2).filter(|w| w[0].is_some() && w[1].is_some() && w[0] != w[1]).count();
    changes as f64 / (roots.len().max(2) - 1) as f64
}

/// the bars' chroma, as the analysis heard the recording, from bar `first` for `count`
fn chroma_of(a: &Analysis, first: usize, count: u32) -> Option<&[[f32; 12]]> {
    a.bar_chroma.get(first..first + count as usize)
}

/// How much of the recording bars `first`..`first + bars` stand for, 0 to 1: how many other runs as long, from any of
/// its bars, play what they do (their chroma correlates 0.8 or more), three or more counting fully: a hook comes back.
fn fitness(a: &Analysis, first: usize, bars: u32) -> f64 {
    let Some(own) = chroma_of(a, first, bars) else { return 0.0 };
    let len = bars as usize;
    let again = (0..a.bars.saturating_sub(len))
        .filter(|&o| o + len <= first || o >= first + len)
        .filter(|&o| chroma_of(a, o, bars).and_then(|x| crate::sampling::correlation(own, x)).is_some_and(|c| c >= 0.8))
        .count();
    (again as f64 / 3.0).min(1.0)
}

/// a section's pull toward being the hook: its energy, and more for a hook or a drop (as a remix picks its hook)
fn hookness(s: &SectionGuess) -> f64 {
    s.energy as f64 + if matches!(s.kind, SectionKind::Hook | SectionKind::Drop) { 0.25 } else { 0.0 }
}

/// a candidate: its bars, the features its kind is scored by, and how well its length suits its kind
struct Candidate {
    first: usize,
    bars: u32,
    features: Vec<f64>,
    fits: f64,
}

/// Each candidate's quality among the others of its kind, 0 to 1: the weighted geometric mean of each feature's rank
/// among theirs (the best 1), times how well its length suits its kind.
fn qualities(cands: &[Candidate], weights: &[f64]) -> Vec<f64> {
    let n = cands.len() as f64;
    let rank = |f: usize, v: f64| {
        let below = cands.iter().filter(|c| c.features[f] < v).count() as f64;
        let same = cands.iter().filter(|c| c.features[f] == v).count() as f64;
        (below + 0.5 * same + 0.5) / n
    };
    let total: f64 = weights.iter().sum();
    cands
        .iter()
        .map(|c| {
            let log: f64 = weights.iter().enumerate().map(|(f, w)| w * rank(f, c.features[f]).ln()).sum();
            (log / total).exp() * c.fits
        })
        .collect()
}

/// The runs of bars a piece of `kind` may be cut from in `sections`: of each length its kind takes, from each
/// section's first bar and every run as long after it, within the section.
fn runs(kind: Kind, sections: &[&SectionGuess], total: usize) -> Vec<(usize, u32, f64)> {
    let mut out = Vec::new();
    for s in sections {
        let end = (s.start_bar + s.bars).min(total);
        for &(len, fits) in kind.lengths() {
            let mut first = s.start_bar;
            while first + len as usize <= end {
                out.push((first, len, fits));
                first += len as usize;
            }
        }
    }
    out
}

/// The best piece of `kind` the recording `a` heard holds, cut from `x` (what it plays in) apart from `rest`: the best
/// on take `seed` 0, and on any other one drawn among those near the best, the better the likelier (`rng`); none when
/// the recording has nothing of the kind.
pub fn best(kind: Kind, a: &Analysis, x: &Stereo, rest: &Stereo, seed: u64, rng: &mut Rng) -> Option<Cut> {
    let loudest = (0..a.bars).map(|b| db(bar_power(a, x, b))).fold(f64::MIN, f64::max);
    let hook = a.sections.iter().filter(|s| s.harmony).max_by(|p, q| hookness(p).total_cmp(&hookness(q)));
    let sections: Vec<&SectionGuess> = match kind {
        Kind::Phrase => return None,
        Kind::Hook => hook.into_iter().collect(),
        Kind::Passage => a
            .sections
            .iter()
            .filter(|s| s.harmony && !matches!(s.kind, SectionKind::Intro | SectionKind::Outro))
            .filter(|s| hook.is_none_or(|h| h.start_bar != s.start_bar))
            .collect(),
        Kind::Break | Kind::Bass => a.sections.iter().collect(),
    };
    let hook_chroma = hook.and_then(|h| chroma_of(a, h.start_bar, h.bars.min(4) as u32));
    let mut cands: Vec<Candidate> = Vec::new();
    for (first, bars, fits) in runs(kind, &sections, a.bars) {
        let bar_range = first..first + bars as usize;
        let power: f64 = bar_range.clone().map(|b| bar_power(a, x, b)).sum::<f64>() / bars as f64;
        // what it is cut from plays in every one of its bars
        if bar_range.clone().any(|b| db(bar_power(a, x, b)) < loudest - PRESENT_DB) {
            continue;
        }
        let others: f64 = bar_range.map(|b| bar_power(a, rest, b)).sum::<f64>() / bars as f64;
        let isolation = db(power) - db(others);
        let loops = loopability(a, x, first, bars);
        let features = match kind {
            Kind::Break => vec![isolation, busy(a, x, first, bars), loops],
            Kind::Bass => vec![isolation, moves(a, x, first, bars), loops],
            Kind::Hook => vec![fitness(a, first, bars), loops, isolation],
            Kind::Passage => {
                let alike = hook_chroma
                    .zip(chroma_of(a, first, bars.min(4)))
                    .and_then(|(h, own)| {
                        crate::sampling::correlation(&h[..own.len().min(h.len())], &own[..own.len().min(h.len())])
                    })
                    .unwrap_or(0.0);
                vec![1.0 - alike.max(0.0), loops, isolation]
            }
            Kind::Phrase => unreachable!("phrases are not cut here"),
        };
        cands.push(Candidate { first, bars, features, fits });
    }
    if cands.is_empty() {
        return None;
    }
    let weights: &[f64] = match kind {
        Kind::Break | Kind::Bass => &[1.0, 0.6, 1.0],
        Kind::Hook => &[1.0, 1.0, 0.5],
        _ => &[0.8, 1.0, 0.5],
    };
    let q = qualities(&cands, weights);
    let top = q.iter().copied().fold(0.0, f64::max);
    let pick = match seed {
        0 => (0..cands.len()).max_by(|&i, &j| q[i].total_cmp(&q[j]))?,
        _ => {
            (0..cands.len())
                .filter(|&i| q[i] >= top - NEAR)
                .map(|i| (q[i] / CHOOSY - (-rng.range(1e-12, 1.0).ln()).ln(), i))
                .max_by(|x, y| x.0.total_cmp(&y.0))?
                .1
        }
    };
    let c = &cands[pick];
    Some(Cut { kind, first: c.first, bars: c.bars, score: q[pick], about: about(kind, &c.features, a, c.first) })
}

/// what a piece of `kind` scored for, in words, from its features (as `best` measures them)
fn about(kind: Kind, f: &[f64], a: &Analysis, first: usize) -> String {
    let loops = |l: f64| match l {
        l if l >= 0.75 => "it loops cleanly",
        l if l >= 0.5 => "it loops",
        _ => "its seam shows as it loops",
    };
    let alone = |iso: f64, what: &str| match iso >= ALONE_DB {
        true => format!("the {what} alone"),
        false => format!("the {what}, taken from the rest"),
    };
    let section = a.sections.iter().find(|s| (s.start_bar..s.start_bar + s.bars).contains(&first));
    let of = section.map(|s| format!(" of the {}", s.kind.name())).unwrap_or_default();
    match kind {
        Kind::Break => {
            let busy = if f[1] >= 0.35 { "busy" } else { "sparse" };
            format!("{}, {busy}, {}", alone(f[0], "drums"), loops(f[2]))
        }
        Kind::Bass => {
            let moves = if f[1] >= 0.4 { "moving" } else { "steady" };
            format!("{}, {moves}, {}", alone(f[0], "bass"), loops(f[2]))
        }
        Kind::Hook => {
            let back = if f[0] >= 0.99 { "it comes back three times or more" } else { "it comes back" };
            format!("the instruments{of}, {back}, {}", loops(f[1]))
        }
        _ => {
            let unlike = if f[0] >= 0.5 { "unlike the hook" } else { "near the hook" };
            format!("the instruments{of}, {unlike}, {}", loops(f[1]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_reads_by_its_name_and_says_what_it_is() {
        for k in Kind::ALL {
            assert_eq!(Kind::parse(k.name()), Ok(k));
        }
        assert_eq!(Kind::parse(" Break "), Ok(Kind::Break));
        assert!(Kind::parse("cowbell").unwrap_err().contains("break"));
        assert_eq!(Kind::Break.said(2), "a 2-bar drum break");
        assert!(!Kind::Break.tonal() && Kind::Bass.tonal());
    }

    #[test]
    fn a_candidate_ranks_by_its_features_among_its_kind_and_by_how_its_length_suits_it() {
        let c = |features: Vec<f64>, fits: f64| Candidate { first: 0, bars: 2, features, fits };
        let q = qualities(
            &[c(vec![1.0, 0.0], 1.0), c(vec![0.0, 1.0], 1.0), c(vec![1.0, 1.0], 1.0), c(vec![1.0, 1.0], 0.5)],
            &[1.0, 1.0],
        );
        assert!(q[2] > q[0] && q[2] > q[1], "best at both: {q:?}");
        assert!((q[3] - q[2] / 2.0).abs() < 1e-9, "the same, at a length that suits its kind half as well: {q:?}");
    }
}
