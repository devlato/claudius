//! A song's sections as a producer hears them: where each starts, to the bar and at any bar (a verse of 17 bars stays
//! 17), which sections play the same music, and what each is (intro, verse, pre-chorus, chorus, bridge, instrumental,
//! outro; or, in a track no one sings in, intro, build, drop, break and outro).
//!
//! Each bar is heard as its harmony (the pitch classes of its four beats), its timbre (the shape of its spectrum) and
//! what plays in it (how loud, how bright, the kit, the bass, the voice). Bars are compared with every other bar, and a
//! section starts where what came before stops being like what comes after (Foote 2000, the novelty of a similarity
//! matrix), where the music that repeats changes (Serrà et al. 2014, structural features), and where the voice or the
//! kit comes in or goes. The boundaries are the best set of them under a preference for phrases of 8 and 16 bars that
//! never forbids another length (dynamic programming over the bars). Sections that play the same music, in the same key
//! or a step away, share a letter (A, B, C); a chorus is the loudest music a voice sings again and again, a verse the
//! other music it sings again and that leads to it, a pre-chorus what comes between, a bridge what it sings once
//! after a chorus (Paulus, Müller and Klapuri 2010 review the methods; Goto 2006 finds a chorus by its repeats).
//! A section builds over its last bars when they rise into a louder one, which is where the build resolves.

use serde::Serialize;

use crate::config::SRF;
use crate::model::SectionKind;

/// What a section is, as producers name the parts of a song.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Label {
    Intro,
    Verse,
    PreChorus,
    Chorus,
    PostChorus,
    Bridge,
    Instrumental,
    Build,
    Drop,
    Break,
    Groove,
    Outro,
}

impl Label {
    pub fn name(self) -> &'static str {
        match self {
            Label::Intro => "intro",
            Label::Verse => "verse",
            Label::PreChorus => "pre-chorus",
            Label::Chorus => "chorus",
            Label::PostChorus => "post-chorus",
            Label::Bridge => "bridge",
            Label::Instrumental => "instrumental",
            Label::Build => "build",
            Label::Drop => "drop",
            Label::Break => "break",
            Label::Groove => "groove",
            Label::Outro => "outro",
        }
    }

    /// the kind of section a song of claudius's plays it as: a chorus as its hook, a pre-chorus as a build, the music
    /// after a chorus that no one sings as a drop
    pub fn kind(self) -> SectionKind {
        match self {
            Label::Intro => SectionKind::Intro,
            Label::Verse => SectionKind::Verse,
            Label::PreChorus | Label::Build => SectionKind::Build,
            Label::Chorus => SectionKind::Hook,
            Label::PostChorus | Label::Drop => SectionKind::Drop,
            Label::Bridge | Label::Break => SectionKind::Break,
            Label::Instrumental | Label::Groove => SectionKind::Groove,
            Label::Outro => SectionKind::Outro,
        }
    }

    /// what a section a song of claudius's plays as `kind` is: its hook a chorus
    pub fn of(kind: SectionKind) -> Label {
        match kind {
            SectionKind::Intro => Label::Intro,
            SectionKind::Verse => Label::Verse,
            SectionKind::Build => Label::Build,
            SectionKind::Drop => Label::Drop,
            SectionKind::Break => Label::Break,
            SectionKind::Hook => Label::Chorus,
            SectionKind::Groove => Label::Groove,
            SectionKind::Outro => Label::Outro,
        }
    }

    /// a name for a section of a song of claudius's, as its scripts write names
    pub fn word(self) -> &'static str {
        match self {
            Label::PreChorus => "pre_chorus",
            Label::PostChorus => "post_chorus",
            other => other.name(),
        }
    }
}

/// What the analysis heard in each bar of a recording, for its sections.
pub(crate) struct Heard {
    /// the pitch classes of each of its four beats, each beat's as loud as another's (`harmony`)
    pub harmony: Vec<[f32; 48]>,
    /// the shape of its spectrum (`timbres`)
    pub timbre: Vec<[f32; 12]>,
    /// how loud it is, how much of that is in its highs, and its bass, in dB
    pub loud: Vec<f64>,
    pub bright: Vec<f64>,
    pub bass: Vec<f64>,
    /// how much the kit plays in it, in dB: its drums apart, or its hits in the mix
    pub kit: Vec<f64>,
    /// the share of it a voice sings in, where the voice is heard apart from the rest
    pub voice: Option<Vec<f64>>,
}

/// A section as the analysis hears it: its first bar, how many, what it is, the letter of the music it plays (0 for A),
/// and how many of its last bars build into the next.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Part {
    pub start: usize,
    pub bars: usize,
    pub label: Label,
    pub group: usize,
    pub build_bars: usize,
}

/// a square matrix of how alike each bar is to each other
struct Square {
    n: usize,
    v: Vec<f32>,
}

impl Square {
    fn new(n: usize, f: impl Fn(usize, usize) -> f32) -> Square {
        let mut v = vec![0.0; n * n];
        for i in 0..n {
            for j in i..n {
                let s = f(i, j);
                v[i * n + j] = s;
                v[j * n + i] = s;
            }
        }
        Square { n, v }
    }

    fn at(&self, i: usize, j: usize) -> f32 {
        self.v[i * self.n + j]
    }
}

/// A bar's harmony: the pitch classes of each of its four beats, from `frames` of chroma `hop` samples apart
/// (`chroma::chroma`), each beat's scaled to as loud as another's; a beat no frame falls in takes the nearest.
pub(crate) fn harmony(frames: &[[f32; 12]], hop: usize, t0: f64, t1: f64) -> [f32; 48] {
    let mut out = [0.0f32; 48];
    if frames.is_empty() {
        return out;
    }
    let half = (super::chroma::NFFT / 2) as f64;
    let centre = |i: usize| (i as f64 * hop as f64 + half) / SRF;
    let frame_at = |t: f64| (((t * SRF - half) / hop as f64).round().max(0.0) as usize).min(frames.len() - 1);
    let beat = (t1 - t0) / 4.0;
    for k in 0..4 {
        let (a, b) = (t0 + k as f64 * beat, t0 + (k + 1) as f64 * beat);
        let mut c = [0.0f32; 12];
        let mut i = frame_at(a);
        while i > 0 && centre(i) >= a {
            i -= 1;
        }
        let mut any = false;
        while i < frames.len() && centre(i) < b {
            if centre(i) >= a {
                for (x, y) in c.iter_mut().zip(&frames[i]) {
                    *x += y;
                }
                any = true;
            }
            i += 1;
        }
        if !any {
            c = frames[frame_at(0.5 * (a + b))];
        }
        let norm = c.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 1e-9 {
            for (o, v) in out[k * 12..(k + 1) * 12].iter_mut().zip(c) {
                *o = v / norm;
            }
        }
    }
    out
}

/// the spectrum is read 4096 samples at a time, every 2048, for the timbre
const TIMBRE_FFT: usize = 4096;
const TIMBRE_HOP: usize = 2048;

/// Each bar's timbre: the shape of its spectrum, as the first twelve coefficients after the loudness of the cosine
/// transform of its log energies in bands a quarter of an octave wide from 40 Hz to 16 kHz (cepstral coefficients, as
/// speech and music recognition read timbre), averaged over the bar.
pub(crate) fn timbres(mono: &[f32], bar_starts: &[f64]) -> Vec<[f32; 12]> {
    let bars = bar_starts.len().saturating_sub(1);
    if mono.len() < TIMBRE_FFT || bars == 0 {
        return vec![[0.0; 12]; bars];
    }
    let bands = super::onsets::log_bands_from(TIMBRE_FFT, 4.0, 40.0, 16000.0);
    let nb = bands.len().max(1);
    let frames = 1 + (mono.len() - TIMBRE_FFT) / TIMBRE_HOP;
    let win: Vec<f32> = (0..TIMBRE_FFT)
        .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / TIMBRE_FFT as f64).cos()) as f32)
        .collect();
    let mut planner = realfft::RealFftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(TIMBRE_FFT);
    let (mut buf, mut spec) = (vec![0.0f32; TIMBRE_FFT], fwd.make_output_vec());
    let (mut sums, mut counts) = (vec![vec![0.0f64; nb]; bars], vec![0usize; bars]);
    let mut bar = 0;
    for f in 0..frames {
        let a = f * TIMBRE_HOP;
        let t = (a + TIMBRE_FFT / 2) as f64 / SRF;
        if t < bar_starts[0] {
            continue;
        }
        while bar < bars && t >= bar_starts[bar + 1] {
            bar += 1;
        }
        if bar >= bars {
            break;
        }
        for i in 0..TIMBRE_FFT {
            buf[i] = mono[a + i] * win[i];
        }
        if fwd.process(&mut buf, &mut spec).is_err() {
            continue;
        }
        for (s, (_, tri)) in sums[bar].iter_mut().zip(&bands) {
            let e: f64 = tri.iter().map(|&(k, w)| w as f64 * spec[k].norm_sqr() as f64).sum();
            *s += 10.0 * (e + 1e-10).log10();
        }
        counts[bar] += 1;
    }
    sums.iter()
        .zip(&counts)
        .map(|(s, &c)| {
            let mut out = [0.0f32; 12];
            if c == 0 {
                return out;
            }
            for (k, o) in out.iter_mut().enumerate() {
                let k = (k + 1) as f64;
                let sum: f64 = s
                    .iter()
                    .enumerate()
                    .map(|(i, v)| v / c as f64 * (std::f64::consts::PI * k * (i as f64 + 0.5) / nb as f64).cos())
                    .sum();
                *o = (sum * (2.0 / nb as f64).sqrt()) as f32;
            }
            out
        })
        .collect()
}

/// The share of each bar a voice sings in: the words of `sung` that fall in it.
pub(crate) fn sung_share(words: &[crate::cuts::Span], bar_starts: &[f64]) -> Vec<f64> {
    bar_starts
        .windows(2)
        .map(|w| {
            let (t0, t1) = (w[0], w[1]);
            let held: f64 = words.iter().map(|s| (s.end.min(t1) - s.start.max(t0)).max(0.0)).sum();
            (held / (t1 - t0).max(1e-6)).min(1.0)
        })
        .collect()
}

/// fewer bars than this are one section
const FEWEST: usize = 8;

/// The sections of a recording heard as `h`.
pub(crate) fn parts(h: &Heard) -> Vec<Part> {
    let n = h.loud.len();
    if n == 0 {
        return Vec::new();
    }
    if n < FEWEST {
        return vec![Part { start: 0, bars: n, label: Label::Groove, group: 0, build_bars: 0 }];
    }
    let (harm, scale) = centred(&h.harmony);
    let raw = Square::new(n, |i, j| (dot(&harm[i], &harm[j]) / scale).clamp(-1.0, 1.0));
    // the voice over the bars round each: a singer's breath between lines is no change of section
    let voice = h.voice.as_deref().map(phrased);
    let st = texture(h, voice.as_deref());
    let nov = novelty(&raw, &st, h, voice.as_deref());
    let bounds = segment(&nov, n);
    let segs: Vec<(usize, usize)> = bounds.windows(2).map(|w| (w[0], w[1])).collect();
    let group = grouped(&segs, h, &st);
    let label = labelled(&segs, &group, h);
    segs.iter()
        .enumerate()
        .map(|(i, &(a, b))| Part {
            start: a,
            bars: b - a,
            label: label[i],
            group: group[i],
            build_bars: segs.get(i + 1).map_or(0, |&next| builds(h, (a, b), next)),
        })
        .collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// the least a bar's harmony is taken to differ from the song's own, squared: four beats of chords that differ from the
/// song's mean differ by about 1 each, while a song on one chord all through differs only by noise, which must not
/// read as change
const HARMONY_FLOOR: f32 = 0.25;

/// Each bar's harmony less the song's own (what every bar shares, its key), and the square of their typical length
/// (`HARMONY_FLOOR` at least), which their products are divided by: what tells one bar's chords from another's, as
/// much as they differ and no more.
fn centred(h: &[[f32; 48]]) -> (Vec<[f32; 48]>, f32) {
    let n = h.len().max(1) as f32;
    let mut mean = [0.0f32; 48];
    for v in h {
        for (m, x) in mean.iter_mut().zip(v) {
            *m += x / n;
        }
    }
    let c: Vec<[f32; 48]> = h
        .iter()
        .map(|v| {
            let mut c = [0.0f32; 48];
            for ((o, x), m) in c.iter_mut().zip(v).zip(&mean) {
                *o = x - m;
            }
            c
        })
        .collect();
    let typical = c.iter().map(|v| v.iter().map(|x| x * x).sum::<f32>()).sum::<f32>() / n;
    (c, typical.max(HARMONY_FLOOR))
}

/// `v` over the bars round each, two either side, the nearer counting more: a voice's lines and the rests between
/// them, as the share of a phrase sung
fn phrased(v: &[f64]) -> Vec<f64> {
    let w = [0.5, 1.0, 1.0, 1.0, 0.5];
    (0..v.len())
        .map(|i| {
            let (mut s, mut ws) = (0.0, 0.0);
            for (d, wd) in w.iter().enumerate() {
                let j = i as i64 + d as i64 - 2;
                if j >= 0 && (j as usize) < v.len() {
                    s += wd * v[j as usize];
                    ws += wd;
                }
            }
            s / ws
        })
        .collect()
}

/// the mean and spread of `v`, the spread no less than `floor`: a feature that hardly changes is not made to
fn spread(v: &[f64], floor: f64) -> (f64, f64) {
    let n = v.len().max(1) as f64;
    let mean = v.iter().sum::<f64>() / n;
    let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    (mean, sd.max(floor))
}

/// How alike each pair of bars sounds, played as they are: their timbre, loudness, brightness, kit, bass and voice,
/// each in a unit a listener hears (3 dB of loudness counts as much as a voice coming in), as a Gaussian of the
/// distance between them, scaled to the median distance.
fn texture(h: &Heard, voice: Option<&[f64]>) -> Square {
    let n = h.loud.len();
    let mut feats: Vec<Vec<f64>> = vec![Vec::new(); n];
    for k in 0..12 {
        let col: Vec<f64> = h.timbre.iter().map(|t| t[k] as f64).collect();
        let (m, sd) = spread(&col, 1.5);
        for (f, x) in feats.iter_mut().zip(&col) {
            f.push((x - m) / sd / 12f64.sqrt());
        }
    }
    let mut add = |v: &[f64], unit: f64, weight: f64| {
        let (m, _) = spread(v, 1.0);
        for (f, x) in feats.iter_mut().zip(v) {
            f.push(weight * ((x - m) / unit).clamp(-4.0, 4.0));
        }
    };
    add(&h.loud, 3.0, 0.8);
    add(&h.bright, 3.0, 0.4);
    add(&h.kit, 6.0, 0.8);
    add(&h.bass, 6.0, 0.5);
    if let Some(v) = voice {
        add(v, 0.5, 0.8);
    }
    let dist = |i: usize, j: usize| feats[i].iter().zip(&feats[j]).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
    let mut all: Vec<f64> = (0..n).flat_map(|i| (i + 1..n).map(move |j| (i, j))).map(|(i, j)| dist(i, j)).collect();
    all.sort_by(f64::total_cmp);
    let scale = all.get(all.len() / 2).copied().unwrap_or(1.0).max(0.05);
    Square::new(n, |i, j| (-0.5 * (dist(i, j) / scale).powi(2)).exp() as f32)
}

/// How strongly a section starts at each bar: from 1 to `n - 1`, the novelty of the texture (a checkerboard kernel
/// along the diagonal, over 4 bars and over 2), a change in the chords played (`chords_change`), where runs of bars that
/// come back elsewhere start and end (`repeats`), and the voice, the kit or the loudness changing; each scaled so that
/// the song's clearest boundaries are about 1, but never by more than what a real change measures (`floor`), so that a
/// song that hardly changes keeps its novelty small rather than having its noise read as boundaries. The harmony is not
/// compared bar by bar as the texture is: chords change every bar within a section, so bars next to each other are
/// unlike each other whether a section starts or not.
fn novelty(raw: &Square, st: &Square, h: &Heard, voice: Option<&[f64]>) -> Vec<f64> {
    let n = raw.n;
    let scaled = |v: Vec<f64>, floor: f64| {
        let mut sorted: Vec<f64> = v.iter().copied().filter(|x| *x > 0.0).collect();
        sorted.sort_by(f64::total_cmp);
        let top = sorted.get(sorted.len() * 19 / 20).copied().unwrap_or(0.0).max(floor);
        v.into_iter().map(|x| (x / top).clamp(0.0, 1.5)).collect::<Vec<f64>>()
    };
    let (t4, t2) = (scaled(checker(st, 4), 0.3), scaled(checker(st, 2), 0.3));
    let (c4, c2) = (scaled(chords_change(&h.harmony, 4), 0.1), scaled(chords_change(&h.harmony, 2), 0.1));
    let rep = scaled(repeats(&along_diagonals(raw, 1)), 1.0);
    let jumps = jumps(h, voice);
    if std::env::var("CLAUDIUS_STRUCTURE_DEBUG").is_ok() {
        for p in 0..n {
            eprintln!(
                "parts {p:3} t4 {:.2} t2 {:.2} c4 {:.2} c2 {:.2} rep {:.2} j {:.2}",
                t4[p], t2[p], c4[p], c2[p], rep[p], jumps[p]
            );
        }
    }
    (0..n).map(|p| 0.35 * t4[p] + 0.15 * t2[p] + 0.2 * c4[p] + 0.15 * c2[p] + 0.35 * rep[p] + jumps[p]).collect()
}

/// At each bar, how unlike the pitch classes the `k` bars before it play are to those of the `k` from it: a change of
/// the chords a passage plays, whatever their order (a chorus on the verse's chords in another order changes none).
fn chords_change(harmony: &[[f32; 48]], k: usize) -> Vec<f64> {
    let n = harmony.len();
    let classes = |a: usize, e: usize| -> [f64; 12] {
        let mut c = [0.0f64; 12];
        for v in &harmony[a..e] {
            for (k, x) in v.iter().enumerate() {
                c[k % 12] += *x as f64;
            }
        }
        let norm = c.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-9);
        c.map(|x| x / norm)
    };
    (0..n)
        .map(|p| {
            if p == 0 {
                return 0.0;
            }
            let (x, y) = (classes(p.saturating_sub(k), p), classes(p, (p + k).min(n)));
            (1.0 - x.iter().zip(&y).map(|(a, b)| a * b).sum::<f64>()).max(0.0)
        })
        .collect()
}

/// each pair of bars as alike as the bars round them on the same diagonal are, `w` either side: a run of bars that
/// repeats another stands out from a bar that happens to sound like one, and a bar that differs within it does not
/// break it
fn along_diagonals(s: &Square, w: usize) -> Square {
    let n = s.n;
    Square::new(n, |i, j| {
        let (mut sum, mut count) = (0.0, 0);
        for d in -(w as i64)..=(w as i64) {
            let (a, b) = (i as i64 + d, j as i64 + d);
            if a >= 0 && b >= 0 && (a as usize) < n && (b as usize) < n {
                sum += s.at(a as usize, b as usize);
                count += 1;
            }
        }
        sum / count.max(1) as f32
    })
}

/// Foote's novelty: at each bar `p`, how alike the `k` bars before it are to each other and the `k` from it, less how
/// alike those before are to those after, with the bars nearest `p` counting most (a Gaussian taper)
fn checker(s: &Square, k: usize) -> Vec<f64> {
    let n = s.n;
    let w: Vec<f64> = (0..k).map(|i| (-0.5 * (i as f64 / (k as f64 / 2.0)).powi(2)).exp()).collect();
    let mut out = vec![0.0; n];
    for (p, o) in out.iter_mut().enumerate().skip(1) {
        let (mut sum, mut weight) = (0.0, 0.0);
        for i in 0..k.min(p) {
            for j in 0..k.min(p) {
                // before with before, and after with after, where both are in the song
                let (bi, bj) = (p - 1 - i, p - 1 - j);
                let (ai, aj) = (p + i, p + j);
                if ai >= n || aj >= n {
                    continue;
                }
                let ww = w[i] * w[j];
                sum += ww * (s.at(bi, bj) + s.at(ai, aj) - s.at(bi, aj) - s.at(ai, bj)) as f64;
                weight += 2.0 * ww;
            }
        }
        *o = if weight > 0.0 { (sum / weight).max(0.0) } else { 0.0 };
    }
    out
}

/// bars whose harmonies are this alike at least repeat each other
const REPEAT: f32 = 0.3;

/// a run of this many bars at least that repeats others, on end, is music that comes back
const RUN: usize = 4;

/// music comes back this many bars later at least: a progression that goes round every 4 bars within a section is no
/// section coming back
const AGAIN: usize = 8;

/// Where music that comes back starts and ends: for each lag of `AGAIN` bars or more, each run of `RUN` bars at least whose
/// harmonies repeat those the lag earlier (one bar that does not, between bars that do, kept in the run) marks the bar
/// it starts and the bar after it ends, both where it is and where it was first heard, the more the longer it is (a run
/// of 8 bars or more counting fully). A verse that comes back marks where both verses start and end, however alike or
/// unlike the bars within each are.
fn repeats(raw: &Square) -> Vec<f64> {
    let n = raw.n;
    let mut marks = vec![0.0; n + 1];
    let alike = |i: usize, d: usize| i + d < n && raw.at(i, i + d) > REPEAT;
    for d in AGAIN..n {
        let mut k = 0;
        while k + d < n {
            if !alike(k, d) {
                k += 1;
                continue;
            }
            let start = k;
            let mut end = k;
            while alike(end, d) || (alike(end + 1, d) && end > start) {
                end += 1;
            }
            let len = end - start;
            if len >= RUN {
                let w = (len as f64 / 8.0).min(1.0);
                for b in [start, start + d, end, end + d] {
                    if b < n {
                        marks[b] += w;
                    }
                }
            }
            k = end + 1;
        }
    }
    marks.truncate(n);
    marks[0] = 0.0;
    marks
}

/// at each bar, how much the voice (as heard over its phrases), the kit and the loudness change: the two bars before
/// it against the two from it
fn jumps(h: &Heard, voice: Option<&[f64]>) -> Vec<f64> {
    let n = h.loud.len();
    let mean = |v: &[f64], a: usize, b: usize| v[a..b].iter().sum::<f64>() / (b - a).max(1) as f64;
    let change = |v: &[f64], p: usize| (mean(v, p.saturating_sub(2), p) - mean(v, p, (p + 2).min(n))).abs();
    (0..n)
        .map(|p| {
            if p == 0 {
                return 0.0;
            }
            let voice = voice.map_or(0.0, |v| (change(v, p) / 0.5).min(1.5));
            let kit = (change(&h.kit, p) / 9.0).min(1.5);
            let loud = (change(&h.loud, p) / 4.0).min(1.5);
            0.25 * voice + 0.15 * kit + 0.15 * loud
        })
        .collect()
}

/// the preference for a section of `len` bars: none against 8 and 16, a little against 4 and 32, more against 12 and
/// 24, and as much against any other length as a boundary a little clearer than another makes up for; most against 2 or
/// 3, which a song seldom holds as a section of its own
fn prior(len: usize) -> f64 {
    match len {
        0 | 1 => f64::NEG_INFINITY,
        8 | 16 => 0.0,
        4 | 32 => -0.08,
        12 | 24 => -0.12,
        2 => -0.45,
        3 => -0.4,
        6 | 10 | 20 => -0.22,
        l if l > 32 => -0.3 - 0.03 * (l - 32) as f64,
        _ => -0.28,
    }
}

/// what a boundary costs: a section starts where the novelty there is more than this
const BOUNDARY: f64 = 0.42;

/// The bars sections start at, 0 first and `n` last: of every way to cut `n` bars into sections, the one whose
/// boundaries are the most novel, less what each costs and what its sections' lengths are preferred less.
fn segment(nov: &[f64], n: usize) -> Vec<usize> {
    let mut best = vec![f64::NEG_INFINITY; n + 1];
    let mut from = vec![0usize; n + 1];
    best[0] = 0.0;
    for e in 2..=n {
        for s in 0..=e - 2 {
            if best[s] == f64::NEG_INFINITY {
                continue;
            }
            let gain = if s == 0 { 0.0 } else { nov[s] - BOUNDARY };
            let v = best[s] + gain + prior(e - s);
            if v > best[e] {
                best[e] = v;
                from[e] = s;
            }
        }
    }
    let mut bounds = vec![n];
    let mut e = n;
    while e > 0 {
        e = from[e];
        bounds.push(e);
    }
    bounds.reverse();
    bounds.dedup();
    bounds
}

/// a bar's harmony moved `r` semitones, each beat's pitch classes rotated
fn rotated(v: &[f32; 48], r: i32) -> [f32; 48] {
    let mut out = [0.0f32; 48];
    for k in 0..4 {
        for pc in 0..12 {
            out[k * 12 + ((pc as i32 + r).rem_euclid(12)) as usize] = v[k * 12 + pc];
        }
    }
    out
}

/// each bar's harmony less its own mean, at unit length: the pattern of its pitch classes, which moves with a key change
fn patterned(h: &[f32; 48]) -> [f32; 48] {
    let m = h.iter().sum::<f32>() / 48.0;
    let mut out = [0.0f32; 48];
    for (o, x) in out.iter_mut().zip(h) {
        *o = x - m;
    }
    let norm = out.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-6 {
        out.iter_mut().for_each(|x| *x /= norm);
    }
    out
}

/// sections at least this alike play the same music
const SAME: f64 = 0.62;

/// The letter of the music each section plays, 0 for the first heard: sections that start and go on alike, in their
/// chords (the same, or moved up to two semitones, as a last chorus may be) and as they sound, joined while the two
/// groups nearest each other are alike on average (average linkage).
fn grouped(segs: &[(usize, usize)], h: &Heard, st: &Square) -> Vec<usize> {
    let m = segs.len();
    let pat: Vec<[f32; 48]> = h.harmony.iter().map(patterned).collect();
    let voice = |s: (usize, usize)| h.voice.as_ref().map(|v| v[s.0..s.1].iter().sum::<f64>() / (s.1 - s.0) as f64);
    let alike = |a: (usize, usize), b: (usize, usize)| {
        let k = (a.1 - a.0).min(b.1 - b.0);
        let chords = [0i32, 1, -1, 2, -2]
            .into_iter()
            .map(|r| {
                let s: f64 = (0..k).map(|i| dot(&pat[a.0 + i], &rotated(&pat[b.0 + i], r)) as f64).sum();
                s / k as f64 - 0.03 * r.abs() as f64
            })
            .fold(f64::MIN, f64::max);
        let sound = (0..k).map(|i| st.at(a.0 + i, b.0 + i) as f64).sum::<f64>() / k as f64;
        let sung = match (voice(a), voice(b)) {
            (Some(x), Some(y)) => 1.0 - (x - y).abs(),
            _ => 1.0,
        };
        0.55 * chords + 0.3 * sound + 0.15 * sung
    };
    let sim: Vec<Vec<f64>> =
        (0..m).map(|i| (0..m).map(|j| if i == j { 1.0 } else { alike(segs[i], segs[j]) }).collect()).collect();
    let mut clusters: Vec<Vec<usize>> = (0..m).map(|i| vec![i]).collect();
    loop {
        let mut best: Option<(usize, usize, f64)> = None;
        for p in 0..clusters.len() {
            for q in p + 1..clusters.len() {
                let pairs = clusters[p].len() * clusters[q].len();
                let s = clusters[p]
                    .iter()
                    .flat_map(|&i| clusters[q].iter().map(move |&j| (i, j)))
                    .map(|(i, j)| sim[i][j])
                    .sum::<f64>()
                    / pairs as f64;
                if best.is_none_or(|b| s > b.2) {
                    best = Some((p, q, s));
                }
            }
        }
        match best {
            Some((p, q, s)) if s >= SAME => {
                let moved = clusters.remove(q);
                clusters[p].extend(moved);
            }
            _ => break,
        }
    }
    // letters in the order the music is first heard
    clusters.sort_by_key(|c| c.iter().copied().min().unwrap_or(0));
    let mut group = vec![0; m];
    for (g, c) in clusters.iter().enumerate() {
        for &i in c {
            group[i] = g;
        }
    }
    group
}

/// a section a voice sings this much of at least is sung
const SUNG: f64 = 0.25;

/// What each section is. In a song a voice sings a quarter of at least: the chorus is the music it sings again and
/// again that is loudest and sung most, ends the singing and does not start it; the verse the other music it sings
/// again, that leads to the chorus most; a pre-chorus what comes between a verse and a chorus; a post-chorus loud
/// music no one sings right after a chorus; a bridge music sung once after a chorus; the intro and outro what plays
/// before the voice and after it; between, an instrumental, or a break where it is quiet or the kit stops. In a track
/// no one sings in, by its loudness: drops, the builds before them, breaks, and its intro and outro.
fn labelled(segs: &[(usize, usize)], group: &[usize], h: &Heard) -> Vec<Label> {
    let mean = |v: &[f64], s: (usize, usize)| v[s.0..s.1].iter().sum::<f64>() / (s.1 - s.0).max(1) as f64;
    let loud: Vec<f64> = segs.iter().map(|&s| mean(&h.loud, s)).collect();
    let kit: Vec<f64> = segs.iter().map(|&s| mean(&h.kit, s)).collect();
    // one music all through (a loop, a steady track): a groove, with an intro and an outro where they are quieter
    if group.iter().all(|&g| g == group[0]) {
        return played_labels(segs, group, &loud, &kit);
    }
    let bars_sung = h.voice.as_ref().map(|v| v.iter().filter(|x| **x >= 0.3).count()).unwrap_or(0);
    match &h.voice {
        Some(v) if bars_sung * 4 >= h.loud.len() => {
            let sung: Vec<f64> = segs.iter().map(|&s| mean(v, s)).collect();
            sung_labels(segs, group, &loud, &kit, &sung)
        }
        // the voice heard apart, and no one sings: a track
        Some(_) => played_labels(segs, group, &loud, &kit),
        // the voice not heard apart: as a song, sung wherever it plays fully (within 6 dB of its middle)
        None => {
            let mid = median(&loud);
            let sung: Vec<f64> = loud.iter().map(|&l| if l >= mid - 6.0 { 1.0 } else { 0.0 }).collect();
            sung_labels(segs, group, &loud, &kit, &sung)
        }
    }
}

/// the labels of a song a voice sings (`labelled`)
fn sung_labels(segs: &[(usize, usize)], group: &[usize], loud: &[f64], kit: &[f64], sung: &[f64]) -> Vec<Label> {
    let m = segs.len();
    let vocal: Vec<bool> = sung.iter().map(|&s| s >= SUNG).collect();
    let (Some(first), Some(last)) = (vocal.iter().position(|&v| v), vocal.iter().rposition(|&v| v)) else {
        return played_labels(segs, group, loud, kit);
    };
    let groups = group.iter().copied().max().map_or(0, |g| g + 1);
    let members = |g: usize| (0..m).filter(move |&i| group[i] == g);
    let sung_in = |g: usize| members(g).filter(|&i| vocal[i]).count();
    let repeated: Vec<usize> = (0..groups).filter(|&g| sung_in(g) >= 2).collect();
    let top = loud.iter().copied().fold(f64::MIN, f64::max);
    let mean_of = |v: &[f64], g: usize| {
        let (s, c) = members(g).fold((0.0, 0), |(s, c), i| (s + v[i], c + 1));
        s / c.max(1) as f64
    };
    let chorus_score = |g: usize| {
        (mean_of(loud, g) - top) / 3.0
            + 0.4 * (sung_in(g) as f64 - 1.0)
            + 0.5 * mean_of(sung, g)
            + if group[last] == g { 0.5 } else { 0.0 }
            - if group[first] == g && repeated.len() >= 2 { 0.6 } else { 0.0 }
    };
    let chorus = repeated.iter().copied().max_by(|&a, &b| chorus_score(a).total_cmp(&chorus_score(b)));
    // the verse leads to the chorus: how many of its sections a chorus follows, within two sections
    let leads = |g: usize| {
        members(g).filter(|&i| (i + 1..(i + 3).min(m)).any(|j| Some(group[j]) == chorus)).count() as f64
            - 0.01 * members(g).next().unwrap_or(0) as f64
    };
    let verse = repeated.iter().copied().filter(|&g| Some(g) != chorus).max_by(|&a, &b| leads(a).total_cmp(&leads(b)));
    let mut out = vec![Label::Groove; m];
    for i in 0..m {
        if Some(group[i]) == chorus && vocal[i] {
            out[i] = Label::Chorus;
        } else if Some(group[i]) == verse && vocal[i] {
            out[i] = Label::Verse;
        }
    }
    // no music sung again: the loudest sung section is a chorus when it stands out by 2 dB, the rest are verses
    if chorus.is_none() {
        let sung_loud: Vec<usize> = (0..m).filter(|&i| vocal[i]).collect();
        let loudest = sung_loud.iter().copied().max_by(|&a, &b| loud[a].total_cmp(&loud[b]));
        for &i in &sung_loud {
            let others = sung_loud.iter().filter(|&&j| j != i).map(|&j| loud[j]).fold(f64::MIN, f64::max);
            out[i] = if Some(i) == loudest && loud[i] - others >= 2.0 { Label::Chorus } else { Label::Verse };
        }
    }
    let chorus_loud = (0..m).filter(|&i| out[i] == Label::Chorus).map(|i| loud[i]).fold(f64::MIN, f64::max);
    for i in 0..m {
        if out[i] != Label::Groove {
            continue;
        }
        let before_chorus = out.get(i + 1) == Some(&Label::Chorus);
        let after_chorus = i > 0 && out[i - 1] == Label::Chorus;
        out[i] = if before_chorus && segs[i].1 - segs[i].0 <= 12 && i > first.saturating_sub(1) {
            Label::PreChorus
        } else if after_chorus && !vocal[i] && loud[i] >= chorus_loud - 1.5 && i < last {
            Label::PostChorus
        } else if vocal[i] {
            let once = (0..m).filter(|&j| group[j] == group[i]).count() == 1;
            let chorus_before = out[..i].contains(&Label::Chorus);
            if once && chorus_before { Label::Bridge } else { Label::Verse }
        } else if i < first {
            Label::Intro
        } else if i > last {
            Label::Outro
        } else {
            let mid = median(loud);
            if loud[i] < mid - 3.0 || kit[i] < median(kit) - 9.0 { Label::Break } else { Label::Instrumental }
        };
    }
    out
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s.get(s.len() / 2).copied().unwrap_or(0.0)
}

/// the labels of a track no one sings in (`labelled`): by how loud each section is between the quietest and the
/// loudest (all at one level when they are within 1.5 dB, as a groove's are), drops are the loudest with the kit
/// playing and the music they play; a build comes before a drop, the intro before the first and the outro after the
/// last; a break is quiet or has no kit, a groove the rest
fn played_labels(segs: &[(usize, usize)], group: &[usize], loud: &[f64], kit: &[f64]) -> Vec<Label> {
    let m = segs.len();
    let (lo, hi) = loud.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    let flat = hi - lo < 1.5;
    let e: Vec<f64> = loud.iter().map(|&v| if flat { 0.6 } else { (v - lo) / (hi - lo) }).collect();
    let kit_top = kit.iter().copied().fold(f64::MIN, f64::max);
    let kitted = |i: usize| kit[i] > kit_top - 9.0;
    let mut out = vec![Label::Groove; m];
    for i in 0..m {
        if e[i] >= 0.85 && kitted(i) {
            out[i] = Label::Drop;
        }
    }
    // the same music as a drop, nearly as loud, is a drop too
    for i in 0..m {
        if out[i] != Label::Drop && e[i] >= 0.6 && (0..m).any(|j| out[j] == Label::Drop && group[j] == group[i]) {
            out[i] = Label::Drop;
        }
    }
    let first_drop = out.iter().position(|l| *l == Label::Drop);
    let last_drop = out.iter().rposition(|l| *l == Label::Drop);
    for i in 0..m {
        if out[i] == Label::Drop {
            continue;
        }
        let next_drop = out.get(i + 1) == Some(&Label::Drop);
        out[i] = if i == 0 && e[i] < 0.85 {
            Label::Intro
        } else if i + 1 == m && m > 2 && e[i] < 0.85 {
            Label::Outro
        } else if next_drop && e[i] >= 0.35 {
            Label::Build
        } else if first_drop.is_some_and(|f| i < f) && e[i] < 0.5 {
            Label::Intro
        } else if last_drop.is_some_and(|l| i > l) && e[i] < 0.5 && (i + 1..m).all(|j| out[j] != Label::Drop) {
            Label::Outro
        } else if !kitted(i) || e[i] < 0.5 {
            Label::Break
        } else {
            Label::Groove
        };
    }
    // an intro is the first sections only, an outro the last
    for i in 1..m {
        if out[i] == Label::Intro && out[i - 1] != Label::Intro {
            out[i] = Label::Break;
        }
    }
    for i in (0..m.saturating_sub(1)).rev() {
        if out[i] == Label::Outro && out[i + 1] != Label::Outro {
            out[i] = Label::Break;
        }
    }
    out
}

/// a bar of a build rises over the one before it by this much at least (dB of loudness and half its brightness)
const STEP_UP: f64 = 0.1;

/// How many of the last bars of section `s` build into section `next`: where `next` is louder (by 1.5 dB, or with the
/// kit or the bass coming in), the bars from the last back to where they stop rising (their loudness and half their
/// brightness, each bar `STEP_UP` over the one before; a bar held level in the middle of the rise counts, a level
/// stretch before it does not), two at least, rising 1.5 dB at least in all. None where it does not lead into
/// something louder.
fn builds(h: &Heard, s: (usize, usize), next: (usize, usize)) -> usize {
    let mean = |v: &[f64], a: usize, b: usize| v[a..b].iter().sum::<f64>() / (b - a).max(1) as f64;
    let rises = mean(&h.loud, next.0, next.1) - mean(&h.loud, s.0, s.1) >= 1.5
        || mean(&h.kit, next.0, next.1) - mean(&h.kit, s.0, s.1) >= 6.0
        || mean(&h.bass, next.0, next.1) - mean(&h.bass, s.0, s.1) >= 6.0;
    if !rises {
        return 0;
    }
    let lift: Vec<f64> = (s.0..s.1).map(|b| h.loud[b] + 0.5 * h.bright[b]).collect();
    let up = |j: usize| lift[j] - lift[j - 1] >= STEP_UP;
    let level = |j: usize| (lift[j] - lift[j - 1]).abs() < STEP_UP;
    // back from the last bar while each rises over the one before, past one level bar where the rise goes on before it
    let mut first = lift.len() - 1;
    while first > 0 {
        if up(first) {
            first -= 1;
        } else if level(first) && first > 1 && up(first - 1) {
            first -= 2;
        } else {
            break;
        }
    }
    let k = lift.len() - first;
    if k >= 2 && lift[lift.len() - 1] - lift[first] >= 1.5 { k } else { 0 }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn the_preferred_lengths_of_a_section_never_forbid_another() {
        assert_eq!(prior(8), 0.0);
        assert_eq!(prior(16), 0.0);
        assert!(prior(17).is_finite() && prior(21).is_finite() && prior(6).is_finite());
        assert!(prior(17) > -BOUNDARY, "a clear boundary at 17 bars outweighs the preference for 16");
        assert!(prior(64) < prior(32));
    }

    #[test]
    fn the_segmentation_takes_the_clear_boundaries_at_any_bar_and_no_others() {
        // a boundary at 17 and at 23 as clear as can be, and nothing else
        let mut nov = vec![0.05; 40];
        nov[17] = 1.2;
        nov[23] = 1.2;
        assert_eq!(segment(&nov, 40), vec![0, 17, 23, 40]);
        // nothing changes in 96 bars: sections of 32
        let flat = vec![0.0; 96];
        let b = segment(&flat, 96);
        assert!(b.windows(2).all(|w| w[1] - w[0] <= 32), "{b:?}");
    }

    #[test]
    fn a_rising_tail_builds_into_a_louder_section() {
        let n = 16;
        let mut loud = vec![-20.0; n];
        // bars 4 to 7 rise 1 dB a bar into the louder section from bar 8
        for (k, b) in (4..8).enumerate() {
            loud[b] = -20.0 + k as f64;
        }
        for v in loud.iter_mut().skip(8) {
            *v = -12.0;
        }
        let h = Heard {
            harmony: vec![[0.0; 48]; n],
            timbre: vec![[0.0; 12]; n],
            loud,
            bright: vec![0.0; n],
            bass: vec![-30.0; n],
            kit: vec![-30.0; n],
            voice: None,
        };
        assert_eq!(builds(&h, (0, 8), (8, 16)), 4);
        // and nothing builds into something quieter
        assert_eq!(builds(&h, (8, 16), (0, 8)), 0);
    }
}
