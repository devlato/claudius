//! The pieces of a recording the sampler offers besides its sung phrases (design 0021): loops of its hook and of
//! another passage, its drums as a break and its bass line, cut on its bars from its stems (from the mix, without the
//! ears), each scored for what makes a piece of its kind good. That is how alone what it is cut from plays in its bars
//! (its isolation, in decibels over the rest), how well it loops (its first beat against the beat after its end, as
//! LoopMaker measures a seam: Shi & Mysore, CHI 2018), and what its kind asks besides: a break how busy it is, a bass
//! line how much it moves, a hook how much of the recording it stands for, another passage how unlike the hook it is.
//! Each feature counts on a scale of its own (`scaled`), so that a piece is judged by how much of it it has.

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
    /// one hit of its instruments' chords and a beat of its decay, struck as a drum is
    Stab,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Phrase, Kind::Hook, Kind::Passage, Kind::Break, Kind::Bass, Kind::Stab];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Phrase => "phrase",
            Kind::Hook => "hook",
            Kind::Passage => "passage",
            Kind::Break => "break",
            Kind::Bass => "bass",
            Kind::Stab => "stab",
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
            Kind::Stab => "a stab of its chords".into(),
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
            Kind::Stab => ("a stab", "stabs"),
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

    /// How often producers sample a piece of this kind: of 42,447 samples on WhoSampled, 37% are drum loops, 34% bass
    /// lines, 17% hooks, 7% whole tracks (which another passage stands for here), 2% vocals and 2% other parts (which a
    /// stab stands for) (Bryan & Wang, ISMIR 2011).
    pub fn share(self) -> f64 {
        match self {
            Kind::Break => 0.3725,
            Kind::Bass => 0.3376,
            Kind::Hook => 0.1725,
            Kind::Passage => 0.072,
            Kind::Phrase => 0.0215,
            Kind::Stab => 0.0239,
        }
    }

    /// what a piece of it is cut from: the voice, the drums, the bass, or the other instruments (a hook, another
    /// passage and a stab)
    fn stem(self) -> &'static str {
        match self {
            Kind::Phrase => "vocals",
            Kind::Break => "drums",
            Kind::Bass => "bass",
            Kind::Hook | Kind::Passage | Kind::Stab => "other",
        }
    }

    /// the lengths a piece of it takes, in bars, each with how well that length suits it: a break and a bass line of 2
    /// or 4 bars, a loop of a hook of 4
    fn lengths(self) -> &'static [(u32, f64)] {
        match self {
            Kind::Phrase | Kind::Stab => &[],
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
    /// how good a piece of its kind it is, 0 to 1 (`quality`)
    pub score: f64,
    /// what it scored for, in words: "the drums alone, busy, it loops cleanly"
    pub about: String,
    /// the section of the recording it is in, by its place among them
    section: Option<usize>,
    /// its score as the take draws it: as it is on the first take, with chance and what was shown before on any other
    drawn: f64,
    /// what its first bar sounds like, beat by beat, to tell how alike two pieces are
    print: Vec<Beat>,
    /// a stab's hit, which no bars hold: where it is in the recording, in seconds, from its attack to a beat after
    pub span: Option<(f64, f64)>,
}

/// a stem plays alone in bars where it is this much louder than the rest together, in decibels
const ALONE_DB: f64 = 20.0;
/// what a piece is cut from has to play there: no quieter than this under its loudest bar, in decibels
const PRESENT_DB: f64 = 20.0;
/// how far under the best of its kind a piece may score and still be drawn for another take, and how strongly the
/// better are preferred among them: the smaller, the more (as a remix draws its hook). A piece shown on the take
/// before is drawn as though it scored `NEAR` less, so that another take offers others where there are any.
const NEAR: f64 = 0.15;
const CHOOSY: f64 = 0.05;
/// the most pieces of a kind a varied choice weighs (`cuts`), the best of those that do not overlap
const MANY: usize = 8;
/// the least a piece beyond the first of each kind adds to be offered (`varied`): its score, weighed by how often
/// producers sample its kind, times how unlike those already chosen it is
const FLOOR: f64 = 0.4;

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
        (Kind::Hook | Kind::Passage | Kind::Stab, Some(s)) => {
            Some((s.other.clone(), sum(&[&s.vocals, &s.drums, &s.bass])))
        }
        (Kind::Bass, None) => None,
        (Kind::Break | Kind::Hook | Kind::Passage | Kind::Stab, None) => {
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
#[derive(Clone, Debug, PartialEq)]
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
    // its spectrum from half a frame before it, so that an attack on the beat counts whole however the bars fall
    let early: Vec<f32> =
        (i0 as i64 - 1024..i1 as i64).map(|i| usize::try_from(i).map_or(0.0, |i| 0.5 * (x.l[i] + x.r[i]))).collect();
    let frames = fft::spectrogram(&early, 2048, 1024);
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
    // a band 30 dB under the beat's loudest is hardly heard against it, however much quieter still it is
    let logs: Vec<f64> = bands.iter().map(|e| db(e / n)).collect();
    let top = logs.iter().copied().fold(f64::MIN, f64::max);
    let logs: Vec<f64> = logs.iter().map(|l| l.max(top - 30.0)).collect();
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

/// How far apart two beats sound, 0 to 1, as LoopMaker weighs a seam (Shi & Mysore, CHI 2018): 1.0 × their chroma
/// distance + 0.6 × how far apart their spectra's shapes are, over 12 dB a band, + 0.2 × how far apart their loudness
/// is, over 12 dB; without the chroma when `pitched` is not, for the pitch classes of drums are their noise's.
fn apart(p: &Beat, q: &Beat, pitched: bool) -> f64 {
    let shape = p.bands.iter().zip(&q.bands).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt() / (12.0 * 8f64.sqrt());
    let chroma = if pitched { 1.0 } else { 0.0 };
    let seam = chroma * (1.0 - cosine(&p.chroma, &q.chroma))
        + 0.6 * shape.min(1.0)
        + 0.2 * ((p.db - q.db).abs() / 12.0).min(1.0);
    (seam / (chroma + 0.8)).clamp(0.0, 1.0)
}

/// How well bars `first`..`first + bars` of `x` loop, 0 to 1: how much their first beat resembles the beat after their
/// end, which looping puts it in place of (`apart`, a seam as LoopMaker hears one). A half when nothing follows the
/// bars in the recording.
pub fn loopability(a: &Analysis, x: &Stereo, first: usize, bars: u32) -> f64 {
    let beat = a.bar_secs / 4.0;
    let end = first + bars as usize;
    if end >= a.bars {
        return 0.5;
    }
    let (Some(head), Some(after)) = (beat_at(x, a.bar_start(first), beat), beat_at(x, a.bar_start(end), beat)) else {
        return 0.5;
    };
    1.0 - apart(&head, &after, true)
}

/// Whether `mono` plays notes as a loop of instruments does, and the ringing of a drum does not: a twentieth of its
/// energy at least from 220 Hz to 4 kHz, where chords and riffs sound, in pitch classes that are not flat (the
/// strongest twice their mean, or it is noise), two of them at least half as strong as the strongest and a minor third
/// apart or more (a low tone's leakage spreads a semitone or two round it). Weighed by energy, not frame by frame, so
/// that what little rings over a quiet stretch counts as little.
fn chordal(mono: &[f32]) -> bool {
    let (mut c, mut inside, mut all) = ([0f64; 12], 0.0, 0.0);
    for frame in fft::spectrogram(mono, 2048, 1024) {
        for (k, m) in frame.iter().enumerate() {
            let (hz, e) = (k as f64 * SRF / 2048.0, (*m as f64).powi(2));
            all += e;
            if (220.0..4000.0).contains(&hz) {
                inside += e;
                c[((12.0 * (hz / 440.0).log2()).round() as i64 + 9).rem_euclid(12) as usize] += e;
            }
        }
    }
    let top = c.iter().copied().fold(0.0, f64::max);
    let strong: Vec<usize> = (0..12).filter(|&i| top > 0.0 && c[i] >= 0.5 * top).collect();
    let apart = |p: usize, q: usize| (p + 12 - q) % 12 >= 3 && (q + 12 - p) % 12 >= 3;
    inside >= 0.05 * all
        && top >= 2.0 * c.iter().sum::<f64>() / 12.0
        && strong.iter().any(|&p| strong.iter().any(|&q| apart(p, q)))
}

/// bar `first` of `x`, as one channel
fn bar_of(a: &Analysis, x: &Stereo, first: usize) -> Vec<f32> {
    let at = |t: f64| ((t * SRF).max(0.0) as usize).min(x.len());
    (at(a.bar_start(first))..at(a.bar_start(first + 1))).map(|i| 0.5 * (x.l[i] + x.r[i])).collect()
}

/// what bar `first` of `x` sounds like, beat by beat (`Beat`), as far as the recording goes
fn print(a: &Analysis, x: &Stereo, first: usize) -> Vec<Beat> {
    let beat = a.bar_secs / 4.0;
    (0..4).map_while(|k| beat_at(x, a.bar_start(first) + k as f64 * beat, beat)).collect()
}

/// How alike two pieces are, 0 to 1, as a varied choice weighs them: as much as the shorter's bars overlap the
/// other's, a half when they are of the same section, or as much as their first bars sound alike beat by beat (`apart`:
/// beats a tenth apart or less sound as one, as a seam that close is not heard, and two fifths apart or more unlike);
/// not at all when they are cut from different stems, for a break and a bass line are never the same piece.
fn alike(p: &Cut, q: &Cut) -> f64 {
    if p.kind.stem() != q.kind.stem() {
        return 0.0;
    }
    let shared = (p.first + p.bars as usize).min(q.first + q.bars as usize).saturating_sub(p.first.max(q.first));
    let overlap = shared as f64 / p.bars.min(q.bars).max(1) as f64;
    let section = if p.section.is_some() && p.section == q.section { 0.5 } else { 0.0 };
    let n = p.print.len().min(q.print.len());
    let sound = match n {
        0 => 0.0,
        _ => {
            let apart: f64 = p.print.iter().zip(&q.print).map(|(x, y)| apart(x, y, p.kind.tonal())).sum();
            ((0.4 - apart / n as f64) / 0.3).clamp(0.0, 1.0)
        }
    };
    overlap.max(section).max(sound)
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
pub(crate) fn fitness(a: &Analysis, first: usize, bars: u32) -> f64 {
    let Some(own) = chroma_of(a, first, bars) else { return 0.0 };
    let len = bars as usize;
    let again = (0..=a.bars.saturating_sub(len))
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

/// A feature on its scale, 0 to 1, so that a candidate is judged by how much of it it has, not by how it ranks among
/// the others (a hair's more loopability is not worth what 20 dB of isolation is): isolation from -10 to 20 dB, how busy
/// a break is and how much a bass line moves up to a half, the rest as they are, 0 to 1. None goes under a fifth, so
/// that one weak feature lowers a piece without ruling it out.
fn scaled(kind: Kind, feature: usize, v: f64) -> f64 {
    let isolation = |db: f64| ((db + 10.0) / 30.0).clamp(0.0, 1.0);
    let share = |s: f64| (s / 0.5).clamp(0.0, 1.0);
    let s = match (kind, feature) {
        (Kind::Break | Kind::Bass, 0) | (Kind::Hook | Kind::Passage | Kind::Stab, 2) => isolation(v),
        (Kind::Break | Kind::Bass, 1) => share(v),
        _ => v.clamp(0.0, 1.0),
    };
    0.2 + 0.8 * s
}

/// A candidate's quality as a piece of its kind, 0 to 1: the weighted geometric mean of its features on their scales
/// (`scaled`), times how well its length suits its kind.
fn quality(kind: Kind, c: &Candidate, weights: &[f64]) -> f64 {
    let total: f64 = weights.iter().sum();
    let log: f64 = weights.iter().enumerate().map(|(f, w)| w * scaled(kind, f, c.features[f]).ln()).sum();
    (log / total).exp() * c.fits
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
    cuts(kind, a, x, rest, seed, &[], rng).into_iter().next()
}

/// The pieces of `kind` the recording `a` heard holds, cut from `x` (what it plays in) apart from `rest`, each scored as
/// a piece of its kind and drawn for take `seed`: as it scores on the first take, and on any other with chance (`rng`)
/// among those near the best of the kind, the better the likelier, one `shown` on the take before (each a kind and
/// where it is in the recording, in seconds) as though it scored `NEAR` less. The `MANY` drawn best, the best first, no
/// two sharing half the shorter's bars; none when the recording has nothing of the kind.
pub fn cuts(
    kind: Kind,
    a: &Analysis,
    x: &Stereo,
    rest: &Stereo,
    seed: u64,
    shown: &[(Kind, f64, f64)],
    rng: &mut Rng,
) -> Vec<Cut> {
    if kind == Kind::Stab {
        return stabs(a, x, rest, seed, shown, rng);
    }
    let loudest = (0..a.bars).map(|b| db(bar_power(a, x, b))).fold(f64::MIN, f64::max);
    let hook = a.sections.iter().filter(|s| s.harmony).max_by(|p, q| hookness(p).total_cmp(&hookness(q)));
    let sections: Vec<&SectionGuess> = match kind {
        Kind::Phrase | Kind::Stab => return Vec::new(),
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
        // and a loop of instruments plays notes, which a drum ringing in what is left of the drums does not
        if matches!(kind, Kind::Hook | Kind::Passage) && !chordal(&bar_of(a, x, first)) {
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
            Kind::Phrase | Kind::Stab => unreachable!("phrases and stabs are not cut on bars"),
        };
        cands.push(Candidate { first, bars, features, fits });
    }
    let weights: &[f64] = match kind {
        Kind::Break | Kind::Bass => &[1.0, 0.6, 1.0],
        Kind::Hook => &[1.0, 1.0, 0.5],
        _ => &[0.8, 1.0, 0.5],
    };
    let q: Vec<f64> = cands.iter().map(|c| quality(kind, c, weights)).collect();
    let top = q.iter().copied().fold(0.0, f64::max);
    let secs = |c: &Candidate| (a.bar_start(c.first), a.bar_start(c.first + c.bars as usize));
    let was_shown = |c: &Candidate| shown.iter().any(|&(k, f, t)| k == kind && same_span(secs(c), (f, t)));
    let mut drawn: Vec<(f64, usize)> = match seed {
        0 => (0..cands.len()).map(|i| (q[i], i)).collect(),
        _ => (0..cands.len())
            .filter(|&i| q[i] >= top - NEAR)
            .map(|i| {
                let less = if was_shown(&cands[i]) { NEAR } else { 0.0 };
                (q[i] - less - CHOOSY * (-rng.range(1e-12, 1.0).ln()).ln(), i)
            })
            .collect(),
    };
    drawn.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut out: Vec<Cut> = Vec::new();
    for (d, i) in drawn {
        let c = &cands[i];
        let shares = |o: &Cut| {
            let shared =
                (o.first + o.bars as usize).min(c.first + c.bars as usize).saturating_sub(o.first.max(c.first));
            2 * shared >= o.bars.min(c.bars) as usize
        };
        if out.iter().any(shares) {
            continue;
        }
        let section = a.sections.iter().position(|s| (s.start_bar..s.start_bar + s.bars).contains(&c.first));
        let (about, print) = (about(kind, &c.features, a, c.first), print(a, x, c.first));
        out.push(Cut { kind, first: c.first, bars: c.bars, score: q[i], about, section, drawn: d, print, span: None });
        if out.len() == MANY {
            break;
        }
    }
    out
}

/// The stabs of the instruments `x` plays apart from `rest`, in the recording `a` heard (`Kind::Stab`): each a hit
/// SuperFlux hears (`analysis::onsets::superflux_hits`) on which they play notes for the beat after it (`chordal`),
/// scored by how hard it comes in (its rise, the hardest's 1), how clean its tail is (how far into that beat the next
/// hit a quarter as hard comes, all of it when none does) and how alone the instruments play there, as a loop's
/// isolation; drawn for take
/// `seed` as `cuts` draws the other kinds, the `MANY` drawn best, no two in one beat.
fn stabs(a: &Analysis, x: &Stereo, rest: &Stereo, seed: u64, shown: &[(Kind, f64, f64)], rng: &mut Rng) -> Vec<Cut> {
    let beat = a.bar_secs / 4.0;
    let mono = x.mono();
    let hits = crate::analysis::onsets::superflux_hits(&mono);
    let hard = |h: &crate::analysis::onsets::Onset| h.rise.iter().sum::<f32>();
    let hardest = hits.iter().map(hard).fold(0f32, f32::max).max(1e-9);
    let power = |y: &Stereo, (from, to): (f64, f64)| {
        let (i0, i1) = (((from * SRF) as usize).min(y.len()), ((to * SRF) as usize).min(y.len()));
        y.l[i0..i1].iter().chain(&y.r[i0..i1]).map(|v| (*v as f64).powi(2)).sum::<f64>() / (2 * (i1 - i0)).max(1) as f64
    };
    let mut cands: Vec<((f64, f64), Candidate)> = Vec::new();
    for (i, h) in hits.iter().enumerate() {
        let (from, to) = (h.at as f64 / SRF, h.at as f64 / SRF + beat);
        if to * SRF > mono.len() as f64 || !chordal(&mono[h.at..(to * SRF) as usize]) {
            continue;
        }
        let attack = (hard(h) / hardest) as f64;
        // the next hit a quarter as hard at least: a faint tick in its tail does not spoil it
        let next =
            hits[i + 1..].iter().find(|n| hard(n) >= 0.25 * hard(h)).map_or(f64::INFINITY, |n| n.at as f64 / SRF);
        let tail = ((next - from) / beat).min(1.0);
        let isolation = db(power(x, (from, to))) - db(power(rest, (from, to)));
        let first = usize::try_from(a.bar_at(from)).unwrap_or(0);
        cands.push(((from, to), Candidate { first, bars: 1, features: vec![attack, tail, isolation], fits: 1.0 }));
    }
    let q: Vec<f64> = cands.iter().map(|(_, c)| quality(Kind::Stab, c, &[1.0, 0.8, 0.6])).collect();
    let top = q.iter().copied().fold(0.0, f64::max);
    let mut drawn: Vec<(f64, usize)> = match seed {
        0 => (0..cands.len()).map(|i| (q[i], i)).collect(),
        _ => (0..cands.len())
            .filter(|&i| q[i] >= top - NEAR)
            .map(|i| {
                let was_shown = shown.iter().any(|&(k, f, t)| k == Kind::Stab && same_span(cands[i].0, (f, t)));
                let less = if was_shown { NEAR } else { 0.0 };
                (q[i] - less - CHOOSY * (-rng.range(1e-12, 1.0).ln()).ln(), i)
            })
            .collect(),
    };
    drawn.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut out: Vec<Cut> = Vec::new();
    for (d, i) in drawn {
        let ((from, to), c) = &cands[i];
        if out.iter().any(|o| o.span.is_some_and(|s| s.1 > *from && s.0 < *to)) {
            continue;
        }
        let section = a.sections.iter().position(|s| (s.start_bar..s.start_bar + s.bars).contains(&c.first));
        let print = beat_at(x, *from, beat).into_iter().collect();
        let about = about(Kind::Stab, &c.features, a, c.first);
        let (first, span) = (c.first, Some((*from, *to)));
        out.push(Cut { kind: Kind::Stab, first, bars: 1, score: q[i], about, section, drawn: d, print, span });
        if out.len() == MANY {
            break;
        }
    }
    out
}

/// whether two spans of a recording, in seconds, are one piece's as a take draws away from those shown: they share
/// half the shorter at least
pub(crate) fn same_span(p: (f64, f64), q: (f64, f64)) -> bool {
    p.1.min(q.1) - p.0.max(q.0) >= 0.5 * (p.1 - p.0).min(q.1 - q.0)
}

/// A varied choice among `cuts` (each kind's as `cuts` drew them), `want` at most: the best of each kind first, then,
/// while there is room, the piece that adds the most that those chosen do not already have, as a greedy DPP chooses
/// (Chen, Zhang & Zhou, NeurIPS 2018) over L = diag(r) S diag(r), where S is how alike the pieces are (`alike`) and r
/// each piece's score as drawn, weighed by how often producers sample its kind (`Kind::share`) and twice that for a
/// kind in `lean` (those the song's genre samples). None that adds less than `FLOOR`. Their places in `cuts`, kind by
/// kind, each kind's as they come in the recording.
pub fn varied(cuts: &[Cut], want: usize, lean: &[Kind]) -> Vec<usize> {
    let most = Kind::ALL.iter().map(|k| k.share()).fold(0.0, f64::max);
    let weight = |k: Kind| (k.share() * if lean.contains(&k) { 2.0 } else { 1.0 } / most).sqrt();
    let r: Vec<f64> = cuts.iter().map(|c| c.drawn.max(0.0) * weight(c.kind)).collect();
    // the greedy DPP's Cholesky rows, kept up as each piece is chosen: each piece's part along those chosen, and what
    // of it is left, squared, which is what choosing it would add
    struct Rows {
        along: Vec<Vec<f64>>,
        left: Vec<f64>,
        chosen: Vec<usize>,
    }
    let choose = |rows: &mut Rows, j: usize| {
        let dj = rows.left[j].max(1e-12).sqrt();
        for i in (0..cuts.len()).filter(|i| *i != j && !rows.chosen.contains(i)) {
            let dot: f64 = rows.along[j].iter().zip(&rows.along[i]).map(|(p, q)| p * q).sum();
            let e = (r[j] * alike(&cuts[j], &cuts[i]) * r[i] - dot) / dj;
            rows.along[i].push(e);
            rows.left[i] -= e * e;
        }
        rows.chosen.push(j);
    };
    let n = cuts.len();
    let mut rows = Rows { along: vec![Vec::new(); n], left: r.iter().map(|x| x * x).collect(), chosen: Vec::new() };
    for kind in Kind::ALL {
        let of = (0..n).filter(|&i| cuts[i].kind == kind);
        if let Some(j) = of.max_by(|&p, &q| cuts[p].drawn.total_cmp(&cuts[q].drawn)) {
            choose(&mut rows, j);
        }
    }
    while rows.chosen.len() < want {
        let rest = (0..n).filter(|i| !rows.chosen.contains(i));
        let Some(j) = rest.max_by(|&p, &q| rows.left[p].total_cmp(&rows.left[q])) else { break };
        if rows.left[j].max(0.0).sqrt() < FLOOR {
            break;
        }
        choose(&mut rows, j);
    }
    let mut chosen = rows.chosen;
    chosen.sort_by_key(|&i| (cuts[i].kind, cuts[i].first));
    chosen
}

/// what a piece of `kind` scored for, in words, from its features (as `best` measures them)
pub(crate) fn about(kind: Kind, f: &[f64], a: &Analysis, first: usize) -> String {
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
            let back = match f[0] {
                f if f >= 0.99 => "it comes back three times or more",
                f if f > 0.0 => "it comes back",
                _ => "heard once",
            };
            format!("the instruments{of}, {back}, {}", loops(f[1]))
        }
        Kind::Stab => {
            let hard = if f[0] >= 0.6 { "struck hard" } else { "struck softly" };
            let tail = if f[1] >= 0.99 { "ringing a beat alone" } else { "another hit in its beat" };
            format!("a chord of the instruments{of}, {hard}, {tail}")
        }
        _ => {
            let unlike = if f[0] >= 0.5 { "unlike the hook" } else { "near the hook" };
            format!("the instruments{of}, {unlike}, {}", loops(f[1]))
        }
    }
}

/// the most pads a piece is chopped onto: half an MPC's bank, which few breaks need all of
pub const PADS: usize = 8;
/// hits that many apart or less (`unlike`) sound as one and share a pad
const SAME: f64 = 0.1;

/// What a hit adds as it comes in (`analysis::onsets::Onset::rise`), as hits are told apart: by octave, by pitch class,
/// and in all.
struct Rise {
    octaves: [f64; 10],
    chroma: [f64; 12],
    all: f64,
}

fn rise_of(rise: &[f32], centres: &[f64]) -> Rise {
    let (mut octaves, mut chroma) = ([0f64; 10], [0f64; 12]);
    for (r, hz) in rise.iter().zip(centres) {
        let r = *r as f64;
        octaves[((hz / 30.0).log2().floor().max(0.0) as usize).min(9)] += r;
        chroma[((12.0 * (hz / 440.0).log2()).round() as i64 + 9).rem_euclid(12) as usize] += r;
    }
    Rise { octaves, chroma, all: rise.iter().map(|r| *r as f64).sum() }
}

/// How unlike two hits are, 0 to 1: how differently what they add is spread over the octaves (1 less the cosine of
/// the two), and over the pitch classes when `pitched`, and a little how much louder one comes in, up to four times.
fn unlike(p: &Rise, q: &Rise, pitched: bool) -> f64 {
    let cos = |a: &[f64], b: &[f64]| {
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        let d = norm(a) * norm(b);
        if d > 0.0 { dot / d } else { 0.0 }
    };
    let chroma = if pitched { 1.0 } else { 0.0 };
    let louder = ((p.all.max(1e-9) / q.all.max(1e-9)).log2().abs() / 2.0).min(1.0);
    let d = (1.0 - cos(&p.octaves, &q.octaves)) + chroma * (1.0 - cos(&p.chroma, &q.chroma)) + 0.3 * louder;
    (d / (1.3 + chroma)).clamp(0.0, 1.0)
}

/// A piece chopped at its onsets (`chop`): its pads, each a hit of it as it was cut, and where each pad's hits came in
/// it, in beats from its start, each with how hard, as the loudest hit is 1.
pub struct Chopped {
    pub pads: Vec<Stereo>,
    pub hits: Vec<Vec<(f64, f32)>>,
    /// how many onsets it was cut at
    pub onsets: usize,
}

/// `x`, a piece `bars` bars long, chopped at its onsets as SuperFlux hears them (`analysis::onsets::superflux_hits`),
/// as producers chop a record onto a sampler's pads: each slice from an onset to the next, faded in over 1 ms and out
/// over 5, the hits that sound as one sharing a pad (`SAME`, by what each adds as it comes in, so that a hat is a hat
/// whatever rings on under it), and the closest of the rest put together until there are `PADS` at most; each pad the
/// hit most like the others on it, and its hits where they came. `pitched` tells them apart by their pitch classes
/// too. None when fewer than two onsets are heard.
pub fn chop(x: &Stereo, bars: u32, pitched: bool) -> Option<Chopped> {
    let onsets = crate::analysis::onsets::superflux_hits(&x.mono());
    if onsets.len() < 2 || bars == 0 {
        return None;
    }
    let centres = crate::analysis::onsets::superflux_centres();
    let at: Vec<usize> = onsets.iter().map(|o| o.at).collect();
    let rises: Vec<Rise> = onsets.iter().map(|o| rise_of(&o.rise, &centres)).collect();
    let beat = x.len() as f64 / SRF / (4 * bars) as f64;
    let ends: Vec<usize> = at.iter().skip(1).copied().chain([x.len()]).collect();
    let d = |i: usize, j: usize| unlike(&rises[i], &rises[j], pitched);
    let mut groups: Vec<Vec<usize>> = (0..at.len()).map(|i| vec![i]).collect();
    loop {
        let mean = |p: &[usize], q: &[usize]| {
            p.iter().flat_map(|&i| q.iter().map(move |&j| (i, j))).map(|(i, j)| d(i, j)).sum::<f64>()
                / (p.len() * q.len()) as f64
        };
        let pairs = (0..groups.len()).flat_map(|p| (p + 1..groups.len()).map(move |q| (p, q)));
        let Some((gap, p, q)) =
            pairs.map(|(p, q)| (mean(&groups[p], &groups[q]), p, q)).min_by(|x, y| x.0.total_cmp(&y.0))
        else {
            break;
        };
        if gap > SAME && groups.len() <= PADS {
            break;
        }
        let merged = groups.remove(q);
        groups[p].extend(merged);
        groups[p].sort_unstable();
    }
    groups.sort_by_key(|g| g[0]);
    let peak = |i: usize| x.slice(at[i], ends[i]).peak();
    let loudest = (0..at.len()).map(peak).fold(0f32, f32::max).max(1e-9);
    let mut pads = Vec::new();
    let mut hits = Vec::new();
    for g in &groups {
        let medoid = *g.iter().min_by(|&&i, &&j| {
            let far = |k: usize| g.iter().map(|&o| d(k, o)).sum::<f64>();
            far(i).total_cmp(&far(j))
        })?;
        let mut pad = x.slice(at[medoid], ends[medoid]);
        let n = pad.len();
        pad.fade(((0.001 * SRF) as usize).min(n / 4), ((0.005 * SRF) as usize).min(n / 2));
        pads.push(pad);
        hits.push(g.iter().map(|&i| (at[i] as f64 / SRF / beat, (peak(i) / loudest).clamp(0.25, 1.0))).collect());
    }
    Some(Chopped { pads, hits, onsets: at.len() })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// two bars of drums at 120 BPM: a kick on the first and third beats of each bar, a snare on the second and fourth,
    /// a hat on every eighth between them
    pub(crate) fn three_drums() -> Stereo {
        let beat = SRF / 2.0;
        let mut x = Stereo::zeros((8.0 * beat) as usize);
        let mut rng = crate::rng::Rng::new(4);
        let mut strike = |at: f64, secs: f64, sound: &mut dyn FnMut(f64) -> f32| {
            let from = (at * beat) as usize;
            for i in 0..(secs * SRF) as usize {
                let v = sound(i as f64 / SRF);
                (x.l[from + i], x.r[from + i]) = (x.l[from + i] + v, x.r[from + i] + v);
            }
        };
        let tau = std::f64::consts::TAU;
        for b in 0..8 {
            if b % 2 == 0 {
                strike(b as f64, 0.3, &mut |t| ((tau * 55.0 * t).sin() * 0.9 * (-t / 0.08).exp()) as f32);
            } else {
                strike(b as f64, 0.2, &mut |t| (rng.f32() - 0.5) * 0.8 * (-t / 0.06).exp() as f32);
            }
            let mut last = 0.0;
            strike(b as f64 + 0.5, 0.04, &mut |t| {
                let n = rng.f32() - 0.5;
                let v = (n - last) * 0.5 * (-t / 0.015).exp() as f32;
                last = n;
                v
            });
        }
        x
    }

    #[test]
    fn a_break_chopped_puts_the_hits_that_sound_as_one_on_a_pad_each_where_they_came() {
        let c = chop(&three_drums(), 2, false).expect("its hits");
        assert_eq!(c.onsets, 16, "{:?}", c.hits);
        let beats: Vec<Vec<f64>> =
            c.hits.iter().map(|h| h.iter().map(|(b, _)| (b * 4.0).round() / 4.0).collect()).collect();
        let eighths: Vec<f64> = (0..8).map(|b| b as f64 + 0.5).collect();
        assert_eq!(
            beats,
            [vec![0.0, 2.0, 4.0, 6.0], vec![0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5], vec![1.0, 3.0, 5.0, 7.0]]
        );
        assert_eq!(beats[1], eighths, "the hats, each on its eighth");
        assert!(c.hits.iter().flatten().all(|(b, _)| (b - (b * 2.0).round() / 2.0).abs() < 0.03), "{:?}", c.hits);
        assert_eq!(c.pads.len(), 3, "a kick, a hat and a snare");
        assert!(chop(&Stereo::zeros(SRF as usize), 1, false).is_none(), "silence has no hits to chop");
    }

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
    fn a_piece_scores_by_how_much_of_each_feature_it_has_and_how_its_length_suits_its_kind() {
        let c = |features: Vec<f64>, fits: f64| Candidate { first: 0, bars: 2, features, fits };
        let q = |c: Candidate| quality(Kind::Break, &c, &[1.0, 0.6, 1.0]);
        // drums alone that loop a little less well beat drums in the mix that loop a hair better
        assert!(q(c(vec![40.0, 0.3, 0.85], 1.0)) > q(c(vec![-8.0, 0.3, 0.97], 1.0)));
        let (best, half) = (q(c(vec![20.0, 0.5, 1.0], 1.0)), q(c(vec![20.0, 0.5, 1.0], 0.5)));
        assert!((best - 1.0).abs() < 1e-9 && (half - 0.5).abs() < 1e-9, "{best} {half}");
        assert!(q(c(vec![-30.0, 0.0, 0.0], 1.0)) >= 0.2 - 1e-9, "weak at all of them, still not none");
    }

    /// a piece of `kind` at bar `first`, `bars` long, in section `section`, scoring `score`, whose beats sound as
    /// `tone` has them: pieces of the same tone sound alike, of tones a whole apart unlike
    fn cut(kind: Kind, first: usize, bars: u32, section: usize, score: f64, tone: f64) -> Cut {
        let beat = |k: usize| Beat {
            chroma: std::array::from_fn(|i| if i == (k + 5 * tone as usize) % 12 { 1.0 } else { 0.0 }),
            bands: std::array::from_fn(|b| (b as f64 - 3.5) * 3.0 * tone),
            db: -12.0 - 6.0 * tone,
        };
        let (about, print) = (String::new(), (0..4).map(beat).collect());
        Cut { kind, first, bars, score, about, section: Some(section), drawn: score, print, span: None }
    }

    #[test]
    fn a_varied_choice_offers_the_best_of_each_kind_then_good_pieces_unlike_those_chosen() {
        let cuts = [
            cut(Kind::Break, 0, 2, 0, 0.9, 0.0),
            // as good, but it plays as the best does
            cut(Kind::Break, 8, 2, 1, 0.85, 0.0),
            // not as good, and unlike it
            cut(Kind::Break, 16, 2, 2, 0.7, 1.0),
            // unlike them all, but too weak to offer
            cut(Kind::Break, 24, 2, 3, 0.3, 2.0),
            cut(Kind::Bass, 0, 2, 0, 0.8, 0.0),
            // a passage weighs little as producers sample them, but it is the best of its kind
            cut(Kind::Passage, 8, 4, 1, 0.5, 0.0),
        ];
        assert_eq!(varied(&cuts, 6, &[]), [5, 0, 2, 4], "the passage, the best break, the one unlike it, the bass");
        assert_eq!(varied(&cuts, 3, &[]), [5, 0, 4], "the best of each kind, room or none");
        assert!((alike(&cuts[0], &cuts[1]) - 1.0).abs() < 1e-9 && alike(&cuts[0], &cuts[2]) < 0.5);
        assert_eq!(alike(&cuts[0], &cuts[4]), 0.0, "a break and a bass line are never the same piece");
    }

    #[test]
    fn a_loop_of_instruments_plays_notes_and_a_drum_ringing_or_noise_does_not() {
        let (n, tau) = (2 * SRF as usize, std::f64::consts::TAU);
        // tones, each its frequency and the seconds it sounds from and to
        let tones = |hz: &[(f64, f64, f64)]| -> Vec<f32> {
            let at = |t: f64| -> f64 {
                hz.iter().filter(|h| (h.1..h.2).contains(&t)).map(|h| (tau * h.0 * t).sin() * 0.3).sum()
            };
            (0..n).map(|i| at(i as f64 / SRF) as f32).collect()
        };
        assert!(chordal(&tones(&[(261.63, 0.0, 2.0), (329.63, 0.0, 2.0), (392.0, 0.0, 2.0)])), "a triad");
        assert!(chordal(&tones(&[(293.66, 0.0, 1.0), (440.0, 1.0, 2.0)])), "a riff, a note a second");
        let kick: Vec<f32> = (0..n)
            .map(|i| {
                let t = (i % (n / 4)) as f64 / SRF;
                ((tau * 60.0 * t).sin() * 0.8 * (-t / 0.05).exp()) as f32
            })
            .collect();
        assert!(!chordal(&kick), "a kick dying away on every beat");
        let mut rng = crate::rng::Rng::new(3);
        let noise: Vec<f32> = (0..n).map(|_| rng.f32() - 0.5).collect();
        assert!(!chordal(&noise), "noise");
    }

    #[test]
    fn a_varied_choice_offers_more_of_what_the_songs_genre_samples() {
        let cuts = [
            cut(Kind::Break, 0, 2, 0, 0.8, 0.0),
            cut(Kind::Break, 16, 2, 2, 0.62, 1.0),
            cut(Kind::Bass, 0, 2, 0, 0.8, 0.0),
            cut(Kind::Bass, 16, 2, 2, 0.62, 1.0),
        ];
        assert_eq!(varied(&cuts, 3, &[]), [0, 1, 2], "a break before a bass line, as producers sample more of them");
        assert_eq!(varied(&cuts, 3, &[Kind::Bass]), [0, 2, 3], "a bass line, for a genre built on them");
    }
}
