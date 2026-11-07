//! The auto-sampler: the phrases of a recording that sound most like a voice, cut on the recording's own beat grid,
//! cleaned of its drums and of what plays to the sides, and placed in the song where they fit. Each becomes a loop of
//! its own (`vox1`, `vox2` ...) that the renderer warps to the song's tempo and moves to its key: the phrase, then
//! the bars of silence before it comes round again, in the sections that suit it, from the bar on which its notes
//! agree best with the song's chords, mixed as a vocal.

use std::path::Path;

use crate::analysis::song::Analysis;
use crate::analysis::{chroma, melody, song as analysis};
use crate::audio::Stereo;
use crate::config::SRF;
use crate::dsp::fft;
use crate::dsp::filter::{self, BiquadKind};
use crate::fx::{Effect, EqBand};
use crate::model::*;

/// how many phrases one call samples at most
pub const MOST: usize = 4;

/// what the ears listen for when nothing else is asked
pub const DEFAULT_FIND: &str = "a person singing";

/// what to sample, and where to
#[derive(Clone, Debug)]
pub struct Options {
    /// how many phrases, 1 to `MOST`
    pub count: usize,
    /// how long each is, in bars (1, 2 or 4); none: whichever of 1 and 2 sounds best
    pub bars: Option<u32>,
    /// what the ears listen for (`DEFAULT_FIND` when not said)
    pub find: Option<String>,
    /// the sections to place the phrases in, by name or kind; empty: the sections that suit each phrase
    pub sections: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options { count: 2, bars: None, find: None, sections: Vec::new() }
    }
}

/// a phrase of the recording worth sampling
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// where it starts in the recording, in seconds, and how many of its bars it lasts
    pub start: f64,
    pub bars: u32,
    /// the ears' z for what they listened for, when they listened
    pub z: Option<f64>,
    /// the share of it that holds a pitch in a voice's range, and of its sound that is in the middle
    pub sung: f64,
    pub centred: f64,
    pub score: f64,
}

/// what the sampler did: the tracks it added, and a line for each saying where it came from and went
#[derive(Debug, Default)]
pub struct Sampled {
    pub tracks: Vec<String>,
    pub lines: Vec<String>,
}

/// Samples the best vocal phrases of `src`, a recording called `name`, into the song: cut, cleaned, stored in `dir`
/// with the song's takes, placed and mixed. The ears, when given, listen for `opt.find`.
pub fn sample(
    bp: &mut Blueprint,
    src: &Stereo,
    name: &str,
    ears: Option<&crate::ears::Ears>,
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    if bp.form.is_empty() {
        return Err("the song has no sections to put the phrases in".into());
    }
    if let Some(b) = opt.bars.filter(|b| ![1, 2, 4].contains(b)) {
        return Err(format!("a phrase is 1, 2 or 4 bars, not {b}"));
    }
    if (src.len() as f64 / SRF) < 4.0 {
        return Err("the recording is too short to sample (4 s at least)".into());
    }
    // the sections asked for are checked before the recording is listened to
    groups(bp, &opt.sections)?;
    let a = analysis::analyze(src, None);
    let count = opt.count.clamp(1, MOST);
    let phrases = find(src, &a, ears, opt, count);
    if phrases.is_empty() {
        return Err(NO_VOICE.into());
    }
    let plain: Vec<(Phrase, Vec<Effect>)> = phrases.into_iter().map(|p| (p, Vec::new())).collect();
    place(bp, src, &a, name, &plain, opt, dir)
}

/// what the sampler says of a recording it finds no voice in
const NO_VOICE: &str = "nothing in the recording sounds like a voice: try other words for what to find";

/// `phrases` of `src` (each with the effects it plays through) placed in the song, in the sections `opt` asks for, as
/// `sample` places them
fn place(
    bp: &mut Blueprint,
    src: &Stereo,
    a: &Analysis,
    name: &str,
    phrases: &[(Phrase, Vec<Effect>)],
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    let groups = groups(bp, &opt.sections)?;
    let bar = 240.0 / a.bpm.max(1.0);
    let transpose = crate::compose::transpose_into(&bp.key, (a.key.tonic, a.key.mode.is_minor()));
    let project = crate::compose::arrange(bp);
    // each group of sections takes a phrase from its first bar and another from the bar after it ends, answering it
    let mut taken: Vec<[bool; 2]> = vec![[false; 2]; groups.len()];
    let mut out = Sampled::default();
    let mut voiced: Vec<usize> = Vec::new();
    let voiced_before: Vec<usize> = (0..bp.form.len())
        .filter(|&si| bp.loops.iter().any(|l| l.sampled && bp.form[si].layers.contains(&l.name)))
        .collect();
    for (p, fx) in phrases {
        let (a0, a1) = ((p.start * SRF) as usize, (((p.start + p.bars as f64 * bar) * SRF) as usize).min(src.len()));
        let cut = clean(&src.slice(a0, a1));
        let profile = shift(&pitch_profile(&cut), transpose);
        let period = 2 * p.bars;
        let mut best: Option<(f64, usize, usize, f64)> = None;
        for (g, (_, secs)) in groups.iter().enumerate() {
            for slot in 0..2 {
                if taken[g][slot] {
                    continue;
                }
                let offset = slot as u32 * p.bars;
                // a slot no section of the group has room for would never sound
                let Some(fits) = fit(&project, secs, offset, p.bars, period, &profile) else { continue };
                // the earlier groups suit a vocal best: the drops and hooks, then the breaks, the verses, the builds
                let suits = fits + 0.12 * (groups.len() - g) as f64 / groups.len() as f64 - 0.05 * slot as f64;
                if best.is_none_or(|b| suits > b.0) {
                    best = Some((suits, g, slot, fits));
                }
            }
        }
        // a phrase with no room left may be longer than the next one
        let Some((_, g, slot, fits)) = best else { continue };
        taken[g][slot] = true;
        let offset = slot as u32 * p.bars;
        let file = crate::take::store(&layout(&cut, offset as f64 * bar, period as f64 * bar), dir)
            .map_err(|e| format!("cannot keep the phrase: {e:#}"))?;
        let names = bp.layer_names();
        let track = (1..).map(|k| format!("vox{k}")).find(|n| !names.contains(n)).unwrap_or_default();
        let from_bar = ((p.start - a.first_downbeat) / bar).round() as i64 + 1;
        bp.loops.push(LoopPart {
            name: track.clone(),
            role: "vocal".into(),
            sound: Sound::Loop {
                path: file.to_string_lossy().into_owned(),
                bpm: a.bpm,
                beats: period as f64 * 4.0,
                key: Some(a.key.tonic),
                tonal: true,
                label: format!("{name}, {}", bars_text(from_bar, p.bars)),
            },
            transpose,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix: TrackMix { fx: vocal_mix().fx.into_iter().chain(fx.iter().cloned()).collect(), ..vocal_mix() },
            from: None,
            sampled: true,
        });
        let (label, secs) = &groups[g];
        let secs: Vec<usize> = secs.iter().copied().filter(|&si| offset + p.bars <= bp.form[si].bars).collect();
        for &si in &secs {
            bp.form[si].layers.push(track.clone());
            voiced.push(si);
        }
        let heard = match p.z {
            Some(z) => format!("the ears {z:+.1} for '{}', ", opt.find.as_deref().unwrap_or(DEFAULT_FIND)),
            None => String::new(),
        };
        let moved = match transpose {
            0 => "in its own key".to_string(),
            t => {
                format!("{} {} into {}", t.abs(), if t > 0 { "semitones up" } else { "semitones down" }, bp.key.name())
            }
        };
        let at = if slot == 0 { "from the first bar" } else { "answering, from the bar after" };
        let names: Vec<&str> = secs.iter().map(|&si| bp.form[si].name.as_str()).collect();
        out.lines.push(format!(
            "{track}: {} of {name} ({heard}{:.0}% sung), {moved} at {:.0} BPM, every {period} bars {at} in the {label} \
             ({}), on chords it agrees with {:.0}%",
            bars_text(from_bar, p.bars),
            p.sung * 100.0,
            bp.bpm,
            names.join(", "),
            fits * 100.0
        ));
        out.tracks.push(track);
    }
    if out.tracks.is_empty() {
        let bars = phrases.iter().map(|(p, _)| p.bars).min().unwrap_or_default();
        return Err(format!("no section asked for has room for a phrase of {bars} bars: try shorter phrases"));
    }
    // a lead that plays with the voice steps back a little, once: beside a phrase sampled before, it has already
    let plays = |p: &Part, at: &[usize]| at.iter().any(|&si| bp.form[si].layers.contains(&p.name));
    let leads: Vec<String> = bp
        .parts
        .iter()
        .filter(|p| p.role == Role::Lead && plays(p, &voiced) && !plays(p, &voiced_before))
        .map(|p| p.name.clone())
        .collect();
    for p in bp.parts.iter_mut().filter(|p| leads.contains(&p.name)) {
        p.mix.level_db -= 1.5;
        out.lines.push(format!("{} 1.5 dB down, to leave room for the voice", p.name));
    }
    bp.lineage.push(format!("sampled {}: {} from {name}", out.tracks.join(" and "), plural(out.tracks.len())));
    Ok(out)
}

/// A piece the sampler offers before any goes into the song (`pieces`): a phrase of the recording, cut and cleaned of
/// its drums as `sample` cuts one, and the same through a treatment.
#[derive(Clone, Debug)]
pub struct Piece {
    pub phrase: Phrase,
    pub raw: Stereo,
    pub treated: Stereo,
    /// the treatment in words, and its effects, which a piece kept treated plays through
    pub treatment: String,
    pub fx: Vec<Effect>,
    /// the recording's tempo, which its bars are counted at
    pub bpm: f64,
}

impl Piece {
    /// where it is in the recording, in words: "bars 12-13, 0:31"
    pub fn place(&self) -> String {
        let at = self.phrase.start.max(0.0) as u64;
        let bar = 240.0 / self.bpm.max(1.0);
        let from = (self.phrase.start / bar).round() as i64 + 1;
        format!("{}, {}:{:02}", bars_text(from, self.phrase.bars), at / 60, at % 60)
    }
}

/// the treatments a piece is offered with, gentlest first: in words, and as effects, for a piece `beats` long
fn treatment(k: usize, beats: f64) -> (String, Vec<Effect>) {
    let chain = |json: serde_json::Value| -> Vec<Effect> { serde_json::from_value(json).unwrap_or_default() };
    let (words, fx) = match k % 7 {
        0 => (
            "dusty: through tape, filtered at 5 kHz with a little wow, over vinyl crackle",
            serde_json::json!([
                {"fx": "tape", "wow": 0.25, "flutter": 0.15, "drive_db": 3, "lowpass_hz": 5000},
                {"fx": "vinyl", "crackle": 0.25, "hiss_db": -42}
            ]),
        ),
        1 => (
            "washed: an echo on the dotted eighth into a long hall",
            serde_json::json!([
                {"fx": "delay", "beats": 0.75, "feedback": 0.35, "damping": 0.4, "pingpong": true, "mix": 0.3},
                {"fx": "reverb", "decay": 3.5, "damping": 0.4, "predelay_ms": 30, "mix": 0.35}
            ]),
        ),
        2 => (
            "lo-fi: crushed to 10 bits and filtered at 3.5 kHz",
            serde_json::json!([
                {"fx": "bitcrush", "bits": 10, "downsample": 2, "mix": 0.8},
                {"fx": "filter", "mode": "lp", "cutoff": 3500, "res": 0.15}
            ]),
        ),
        3 => (
            "phased: a slow phaser over a chorus",
            serde_json::json!([
                {"fx": "chorus", "rate_hz": 0.6, "depth_ms": 3, "delay_ms": 14, "mix": 0.35},
                {"fx": "phaser", "rate_hz": 0.25, "stages": 6, "depth": 0.8, "feedback": 0.4, "mix": 0.5}
            ]),
        ),
        4 => (
            "telephone: only its middle, from 400 Hz to 3 kHz, driven",
            serde_json::json!([
                {"fx": "filter", "mode": "hp", "cutoff": 400, "res": 0.3},
                {"fx": "filter", "mode": "lp", "cutoff": 3000, "res": 0.3},
                {"fx": "saturate", "kind": "hard", "drive_db": 9, "mix": 0.6}
            ]),
        ),
        5 => (
            "stuttered: its last beat repeated, faster and faster",
            serde_json::json!([{"fx": "stutter", "start": (beats - 1.0).max(0.0), "len": 1.0, "slice": 0.25}]),
        ),
        _ => (
            "reversed: its last two beats backwards, into an echo",
            serde_json::json!([
                {"fx": "reverse", "start": (beats - 2.0).max(0.0), "len": 2.0},
                {"fx": "delay", "beats": 0.5, "feedback": 0.3, "damping": 0.3, "mix": 0.25}
            ]),
        ),
    };
    (words.to_string(), chain(fx))
}

/// The pieces of `src` the sampler offers before any goes into the song: `opt.count` phrases (at most `MOST`), each
/// cut and cleaned as `sample` cuts one, and the same through a treatment. Take `seed` 0 offers the best phrases with
/// the gentler treatments; each other take draws again from the good ones (and their length, unless `opt.bars` says),
/// with other treatments. The ears, when given, listen for `opt.find`.
pub fn pieces(src: &Stereo, ears: Option<&crate::ears::Ears>, opt: &Options, seed: u64) -> Result<Vec<Piece>, String> {
    if let Some(b) = opt.bars.filter(|b| ![1, 2, 4].contains(b)) {
        return Err(format!("a phrase is 1, 2 or 4 bars, not {b}"));
    }
    if (src.len() as f64 / SRF) < 4.0 {
        return Err("the recording is too short to sample (4 s at least)".into());
    }
    let a = analysis::analyze(src, None);
    let bar = 240.0 / a.bpm.max(1.0);
    let count = opt.count.clamp(1, MOST);
    let mut rng = crate::rng::Rng::seed_from(seed, "pieces");
    let bars = opt.bars.or_else(|| (seed > 0).then(|| *rng.pick(&[1, 2, 2, 4])));
    let wanted = Options { bars, ..opt.clone() };
    let mut pool = find(src, &a, ears, &wanted, if seed == 0 { count } else { count * 3 });
    if pool.is_empty() {
        return Err(NO_VOICE.into());
    }
    // a later take draws from the good ones: the better a phrase, the likelier it comes
    if seed > 0 {
        let mut keyed: Vec<(f64, Phrase)> = pool.into_iter().map(|p| (p.score * rng.range(0.4, 1.6), p)).collect();
        keyed.sort_by(|x, y| y.0.total_cmp(&x.0));
        pool = keyed.into_iter().map(|(_, p)| p).collect();
    }
    pool.truncate(count);
    pool.sort_by(|x, y| x.start.total_cmp(&y.start));
    let first = if seed == 0 { 0 } else { rng.below(7) };
    Ok(pool
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let (a0, a1) =
                ((p.start * SRF) as usize, (((p.start + p.bars as f64 * bar) * SRF) as usize).min(src.len()));
            let raw = clean(&src.slice(a0, a1));
            let (treatment, fx) = treatment(first + i, p.bars as f64 * 4.0);
            let mut treated = raw.clone();
            let ctx = crate::fx::Ctx::whole(SRF, a.bpm, seed.wrapping_add(i as u64), treated.len());
            crate::fx::apply_chain(&fx, &mut treated, &ctx);
            Piece { phrase: p, raw, treated, treatment, fx, bpm: a.bpm }
        })
        .collect())
}

/// The pieces `pieces` offered that the person kept, each as it was cut or treated (`true`), sampled into the song
/// as `sample` places phrases, in the sections `opt` asks for (those that suit each, when none).
pub fn keep(
    bp: &mut Blueprint,
    src: &Stereo,
    name: &str,
    kept: &[(&Piece, bool)],
    opt: &Options,
    dir: &Path,
) -> Result<Sampled, String> {
    if bp.form.is_empty() {
        return Err("the song has no sections to put the pieces in".into());
    }
    if kept.is_empty() {
        return Err("keep at least one of the pieces".into());
    }
    groups(bp, &opt.sections)?;
    let a = analysis::analyze(src, None);
    let chosen: Vec<(Phrase, Vec<Effect>)> =
        kept.iter().map(|(p, treated)| (p.phrase.clone(), if *treated { p.fx.clone() } else { Vec::new() })).collect();
    place(bp, src, &a, name, &chosen, opt, dir)
}

/// where a phrase was in the recording, in its bars counted from 1
fn bars_text(from: i64, bars: u32) -> String {
    if bars == 1 { format!("bar {from}") } else { format!("bars {from}-{}", from + bars as i64 - 1) }
}

fn plural(n: usize) -> String {
    if n == 1 { "a vocal phrase".into() } else { format!("{n} vocal phrases") }
}

/// Where phrases may go: groups of sections by what suits a vocal, the drops and hooks first, then the breaks, the
/// verses and grooves, the builds, intros and outros; or the sections asked for, by name or kind, as one group.
fn groups(bp: &Blueprint, asked: &[String]) -> Result<Vec<(String, Vec<usize>)>, String> {
    use SectionKind::*;
    if !asked.is_empty() {
        let mut idx = Vec::new();
        for a in asked {
            let hits: Vec<usize> =
                (0..bp.form.len()).filter(|&i| bp.form[i].name == *a || bp.form[i].kind.name() == a).collect();
            if hits.is_empty() {
                let names: Vec<&str> = bp.form.iter().map(|s| s.name.as_str()).collect();
                return Err(format!("no section or kind '{a}' (sections: {})", names.join(" ")));
            }
            idx.extend(hits);
        }
        idx.sort_unstable();
        idx.dedup();
        return Ok(vec![("sections asked for".into(), idx)]);
    }
    let order: [(&str, &[SectionKind]); 4] = [
        ("drops and hooks", &[Drop, Hook]),
        ("breaks", &[Break]),
        ("verses and grooves", &[Verse, Groove]),
        ("builds, intro and outro", &[Build, Intro, Outro]),
    ];
    Ok(order
        .iter()
        .map(|(label, kinds)| {
            (label.to_string(), (0..bp.form.len()).filter(|&i| kinds.contains(&bp.form[i].kind)).collect::<Vec<_>>())
        })
        .filter(|(_, idx)| !idx.is_empty())
        .collect())
}

/// The best `count` phrases of the recording: spans of whole bars on its beat grid, scored by how much of them holds
/// a pitch in a voice's range (A2 to C6) in the middle of the stereo picture, and, when the ears listen, by how much
/// they sound like what they listen for; no two overlapping, the best first.
pub fn find(src: &Stereo, a: &Analysis, ears: Option<&crate::ears::Ears>, opt: &Options, count: usize) -> Vec<Phrase> {
    let bar = 240.0 / a.bpm.max(1.0);
    let secs = src.len() as f64 / SRF;
    let lens: Vec<u32> = opt.bars.map_or_else(|| vec![1, 2], |b| vec![b]);
    let pitches = melody::pitches(src);
    let mut cands: Vec<Phrase> = Vec::new();
    for &bars in &lens {
        let len = bars as f64 * bar;
        let mut t = a.first_downbeat.max(0.0);
        while t + len <= secs {
            let (f0, f1) = ((t * 100.0) as usize, (((t + len) * 100.0) as usize).min(pitches.len()));
            let sung = if f1 > f0 {
                pitches[f0..f1].iter().filter(|p| p.is_some_and(|m| (45.0..=84.0).contains(&m))).count() as f64
                    / (f1 - f0) as f64
            } else {
                0.0
            };
            let (s0, s1) = ((t * SRF) as usize, ((t + len) * SRF) as usize);
            let (mut mid, mut side) = (0f64, 0f64);
            for i in s0..s1.min(src.len()) {
                mid += ((src.l[i] + src.r[i]) as f64 * 0.5).powi(2);
                side += ((src.l[i] - src.r[i]) as f64 * 0.5).powi(2);
            }
            let quiet = mid / (s1 - s0).max(1) as f64 <= 1e-6;
            let centred = if quiet { 0.0 } else { mid.sqrt() / (mid.sqrt() + side.sqrt()) };
            if !quiet {
                cands.push(Phrase { start: t, bars, z: None, sung, centred, score: 2.0 * sung + centred });
            }
            t += bar;
        }
    }
    // the ears listen to the spans that sound most sung, and their reading leads
    cands.sort_by(|x, y| y.score.total_cmp(&x.score));
    cands.truncate(64);
    let spans: Vec<(f64, f64)> = cands.iter().map(|p| (p.start, p.start + p.bars as f64 * bar)).collect();
    if let Some(z) = heard(ears, src, &spans, opt.find.as_deref().unwrap_or(DEFAULT_FIND)) {
        for (p, z) in cands.iter_mut().zip(z) {
            p.z = Some(z);
            p.score += z;
        }
        cands.sort_by(|x, y| y.score.total_cmp(&x.score));
    }
    let mut out: Vec<Phrase> = Vec::new();
    for p in cands {
        let (a0, a1) = (p.start, p.start + p.bars as f64 * bar);
        if out.iter().all(|q| a1 <= q.start + 1e-6 || a0 >= q.start + q.bars as f64 * bar - 1e-6) {
            out.push(p);
        }
        if out.len() == count {
            break;
        }
    }
    out
}

/// the ears' z for `query` over each span, when they listen
#[cfg(feature = "ears")]
fn heard(ears: Option<&crate::ears::Ears>, src: &Stereo, spans: &[(f64, f64)], query: &str) -> Option<Vec<f64>> {
    let e = ears?;
    if spans.is_empty() {
        return None;
    }
    let mono = crate::ears::Ears::mono(src);
    let qe = e.inner.embed_text(&[query]).ok()?.pop()?;
    let cands: Vec<earshot::search::Cand> = spans.iter().map(|&(a, b)| (a, b, "span", 0.0)).collect();
    e.inner.score_spans(&mono, &cands, &qe).ok().map(|(z, _)| z)
}

#[cfg(not(feature = "ears"))]
fn heard(_: Option<&crate::ears::Ears>, _: &Stereo, _: &[(f64, f64)], _: &str) -> Option<Vec<f64>> {
    None
}

/// A phrase as a sample: the middle of the stereo picture, where a voice sits; its sustained part, without the drums
/// (`fft::harmonic`); above 160 Hz and below 11 kHz; its loudest moment at -1 dBFS, faded in and out.
pub fn clean(x: &Stereo) -> Stereo {
    let mid: Vec<f32> = x.l.iter().zip(&x.r).map(|(l, r)| 0.5 * (l + r)).collect();
    let mut v = fft::harmonic(&mid);
    filter::butter(&mut v, true, 160.0, SRF, 4);
    filter::butter(&mut v, false, 11_000.0, SRF, 4);
    let peak = v.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        let g = 0.891 / peak;
        v.iter_mut().for_each(|s| *s *= g);
    }
    let mut out = Stereo { l: v.clone(), r: v };
    let n = out.len();
    out.fade(((0.005 * SRF) as usize).min(n / 4), ((0.03 * SRF) as usize).min(n / 4));
    out
}

/// the phrase after `lead` seconds of silence, in `period` seconds: what a loop plays round
fn layout(cut: &Stereo, lead: f64, period: f64) -> Stereo {
    let mut out = Stereo::zeros((period * SRF).round() as usize);
    let at = (lead * SRF).round() as usize;
    for i in 0..cut.len().min(out.len().saturating_sub(at)) {
        out.l[at + i] = cut.l[i];
        out.r[at + i] = cut.r[i];
    }
    out
}

/// how much of each pitch class the phrase holds, its loudest 1
fn pitch_profile(x: &Stereo) -> [f32; 12] {
    let mono: Vec<f32> = x.l.iter().zip(&x.r).map(|(l, r)| 0.5 * (l + r)).collect();
    let mut sum = [0f32; 12];
    for frame in chroma::chroma(&mono, 2048) {
        for (s, v) in sum.iter_mut().zip(frame) {
            *s += v;
        }
    }
    let top = sum.iter().fold(0f32, |m, v| m.max(*v));
    if top > 0.0 {
        sum.iter_mut().for_each(|v| *v /= top);
    }
    sum
}

/// a pitch profile `semis` semitones up
fn shift(p: &[f32; 12], semis: i32) -> [f32; 12] {
    let mut out = [0f32; 12];
    for (pc, v) in p.iter().enumerate() {
        out[(pc as i32 + semis).rem_euclid(12) as usize] = *v;
    }
    out
}

/// How well a phrase of `bars` bars with pitch profile `profile` agrees with the chords under it, playing from
/// `offset` bars into each of the sections `secs` and every `period` bars after: the profile's cosine with the chord
/// tones under each playing, weighted by how long each chord lasts, averaged over every playing; none when no section
/// has room for it.
fn fit(p: &Project, secs: &[usize], offset: u32, bars: u32, period: u32, profile: &[f32; 12]) -> Option<f64> {
    let (mut sum, mut n) = (0f64, 0usize);
    for &si in secs {
        let Some(s) = p.sections.get(si) else { continue };
        let mut b = offset;
        while b + bars <= s.bars {
            let (t0, t1) = ((s.start_bar + b) as f64 * 4.0, (s.start_bar + b + bars) as f64 * 4.0);
            let mut tones = [0f32; 12];
            for c in p.chords.iter().filter(|c| c.start < t1 && c.start + c.len > t0) {
                let held = ((c.start + c.len).min(t1) - c.start.max(t0)) as f32;
                for pc in c.chord.pcs() {
                    tones[pc as usize] += held;
                }
            }
            sum += cosine(profile, &tones);
            n += 1;
            b += period.max(1);
        }
    }
    (n > 0).then(|| sum / n as f64)
}

fn cosine(a: &[f32; 12], b: &[f32; 12]) -> f64 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32; 12]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let d = norm(a) * norm(b);
    if d > 0.0 { (dot / d) as f64 } else { 0.0 }
}

/// a sampled phrase mixed as a vocal: above 200 Hz, a little presence, compressed, with some reverb and delay, ducked
/// a little under the kick
pub(crate) fn vocal_mix() -> TrackMix {
    TrackMix {
        level_db: -1.0,
        fx: vec![
            Effect::Eq {
                bands: vec![
                    EqBand { kind: BiquadKind::Highpass, freq: 200.0, gain_db: 0.0, q: 0.7 },
                    EqBand { kind: BiquadKind::Peak, freq: 3000.0, gain_db: 1.5, q: 1.0 },
                ],
            },
            Effect::Compressor {
                threshold_db: -20.0,
                ratio: 3.0,
                attack_ms: 5.0,
                release_ms: 80.0,
                knee_db: 4.0,
                makeup_db: 2.0,
                mix: 1.0.into(),
            },
        ],
        reverb: 0.25,
        delay: 0.15,
        duck: 0.35,
        ..TrackMix::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::{Chord, Quality};

    #[test]
    fn a_phrase_fits_where_its_notes_are_the_chords_tones() {
        // two sections of 8 bars: A minor for four bars, then F major
        let am = Chord::new(9, Quality::Min);
        let f = Chord::new(5, Quality::Maj);
        let mut p = crate::compose::arrange(&crate::script::blank("house").unwrap());
        p.sections = vec![
            SectionInfo { name: "drop".into(), kind: SectionKind::Drop, start_bar: 0, bars: 8, energy: 1.0 },
            SectionInfo { name: "drop2".into(), kind: SectionKind::Drop, start_bar: 8, bars: 8, energy: 1.0 },
        ];
        p.chords = (0..4)
            .map(|k| crate::theory::ChordSpan {
                start: k as f64 * 16.0,
                len: 16.0,
                chord: if k % 2 == 0 { am } else { f },
            })
            .collect();
        // a phrase on A, C and E fits the A minor bars, and is half as good every 2 bars across both chords
        let mut amin = [0f32; 12];
        for pc in [9, 0, 4] {
            amin[pc] = 1.0;
        }
        assert!(fit(&p, &[0], 0, 4, 8, &amin).unwrap() > 0.99, "on A minor every time");
        let across = fit(&p, &[0, 1], 0, 2, 2, &amin).unwrap();
        assert!(across > 0.5 && across < 0.9, "{across}");
        assert!(fit(&p, &[0], 4, 4, 8, &amin) < fit(&p, &[0], 0, 4, 8, &amin), "on F major it fits less");
        assert_eq!(fit(&p, &[0], 8, 4, 16, &amin), None, "no room 8 bars into 8 bars");
        assert_eq!(shift(&amin, 3)[0], 1.0, "A minor three up is C minor: C is there");
    }

    #[test]
    fn a_phrase_waits_its_turn_in_its_loop() {
        let cut = Stereo { l: vec![0.5; 4_800], r: vec![0.5; 4_800] };
        let lay = layout(&cut, 0.25, 0.5);
        assert_eq!(lay.len(), 24_000);
        assert!(lay.l[..12_000].iter().all(|&v| v == 0.0), "silence before it");
        assert!(lay.l[12_000..16_800].iter().all(|&v| v == 0.5) && lay.l[16_800..].iter().all(|&v| v == 0.0));
    }

    /// 120 BPM, eight bars: a noisy hat to the left all through, and in the middle a hum from second `from` to `to`
    fn hummed(from: usize, to: usize) -> Stereo {
        let sr = SRF as usize;
        let n = 16 * sr;
        let mut x = Stereo::zeros(n);
        let mut rng = crate::rng::Rng::new(3);
        let mut phase = 0f64;
        for i in 0..n {
            if i % (sr / 4) < sr / 50 {
                x.l[i] += (rng.f32() - 0.5) * 0.6;
            }
            if (from * sr..to * sr).contains(&i) {
                // a sung A3 with a little vibrato, its second harmonic too
                let t = i as f64 / SRF;
                phase += std::f64::consts::TAU * 220.0 * (1.0 + 0.01 * (t * 5.0 * std::f64::consts::TAU).sin()) / SRF;
                let v = (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15) as f32;
                x.l[i] += v;
                x.r[i] += v;
            }
        }
        x
    }

    #[test]
    fn a_sung_line_in_the_middle_is_found_and_the_drums_to_the_sides_are_not() {
        // a hum in bars 3-4
        let (sr, x) = (SRF as usize, hummed(4, 8));
        let a = Analysis { bpm: 120.0, first_downbeat: 0.0, ..analysis::analyze(&x, Some((110.0, 130.0))) };
        let found = find(&x, &a, None, &Options { bars: Some(2), ..Options::default() }, 1);
        assert_eq!(found.len(), 1);
        assert!((found[0].start - 4.0).abs() < 1e-9, "{:?}", found);
        assert!(found[0].sung > 0.8 && found[0].centred > 0.6, "{:?}", found);
        let clean = clean(&x.slice(4 * sr, 8 * sr));
        assert!((clean.peak() - 0.891).abs() < 0.01 && clean.l == clean.r);
    }

    #[test]
    fn a_phrase_plays_only_where_it_has_room_and_a_lead_steps_back_once() {
        let g = crate::genres::get("trip_hop").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = crate::compose::generate(&g, 1, &o, None);
        let lead = bp.parts.iter().find(|p| p.role == Role::Lead).expect("trip-hop has a lead").name.clone();
        // a hook of 4 bars and one of 2: a phrase of 2 bars answering from bar 3 has room only in the first
        let mut hook = bp.form[0].clone();
        (hook.name, hook.kind, hook.bars, hook.layers) = ("hook".into(), SectionKind::Drop, 4, bp.layer_names());
        let short = SectionSpec { name: "hook2".into(), bars: 2, ..hook.clone() };
        bp.form = vec![hook, short];
        let level = |bp: &Blueprint| bp.parts.iter().find(|p| p.name == lead).unwrap().mix.level_db;
        let on = |bp: &Blueprint, track: &str| -> Vec<String> {
            bp.form.iter().filter(|s| s.layers.iter().any(|l| l == track)).map(|s| s.name.clone()).collect()
        };
        let (was, src) = (level(&bp), hummed(4, 12));
        let dir = std::env::temp_dir().join(format!("claudius-sampler-{}", std::process::id()));
        let ask =
            |sections: &str| Options { count: 2, bars: Some(2), sections: vec![sections.into()], ..Options::default() };
        let out = sample(&mut bp, &src, "song", None, &ask("drop"), &dir).unwrap();
        assert_eq!(out.tracks, ["vox1", "vox2"], "{:?}", out.lines);
        assert!(out.lines.iter().any(|l| l.contains("answering")), "{:?}", out.lines);
        for (track, line) in out.tracks.iter().zip(&out.lines) {
            let room: &[&str] = if line.contains("answering") { &["hook"] } else { &["hook", "hook2"] };
            assert_eq!(on(&bp, track), room, "{line}");
        }
        assert_eq!(level(&bp), was - 1.5);
        // the short hook alone has room for one phrase, from its first bar; the lead stepped back for the first ones
        let out = sample(&mut bp, &src, "song", None, &ask("hook2"), &dir).unwrap();
        assert_eq!(out.tracks, ["vox3"], "{:?}", out.lines);
        assert_eq!(level(&bp), was - 1.5, "the lead steps back once");
        assert!(bp.loops.iter().all(|l| l.sampled && l.kept() && l.from.is_none()));
        // phrases of 4 bars have no room in it at all
        let four = Options { bars: Some(4), ..ask("hook2") };
        let err = sample(&mut bp, &src, "song", None, &four, &dir).unwrap_err();
        assert!(err.contains("no section asked for has room for a phrase of 4 bars"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pieces_are_offered_raw_and_treated_drawn_anew_each_take_and_kept_as_chosen() {
        let src = hummed(4, 12);
        let opt = Options { count: 1, bars: Some(2), ..Options::default() };
        let first = pieces(&src, None, &opt, 0).unwrap();
        assert_eq!(first.len(), 1);
        let p = &first[0];
        assert!((4.0..=10.0).contains(&p.phrase.start), "the hum, not the hats: {:?}", p.phrase);
        assert_eq!((p.raw.len(), p.treated.len()), (p.raw.len(), p.raw.len()));
        assert!(p.raw.l != p.treated.l && !p.fx.is_empty(), "treated is another sound");
        assert!(p.treatment.starts_with("dusty"), "take 0 is the gentlest: {}", p.treatment);
        assert_eq!(pieces(&src, None, &opt, 0).unwrap()[0].raw.l, p.raw.l, "the same take, the same pieces");
        let later: Vec<String> =
            (1..6).map(|seed| pieces(&src, None, &opt, seed).unwrap()[0].treatment.clone()).collect();
        assert!(later.iter().any(|t| *t != p.treatment), "other takes, other treatments: {later:?}");
        // kept treated, a piece plays through its treatment; kept as it was, through the vocal's chain alone
        let g = crate::genres::get("trip_hop").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut bp = crate::compose::generate(&g, 1, &o, None);
        let dir = std::env::temp_dir().join(format!("claudius-pieces-{}", std::process::id()));
        let out = keep(&mut bp, &src, "song", &[(p, true)], &Options::default(), &dir).unwrap();
        let vox = bp.loops.iter().find(|l| l.name == out.tracks[0]).unwrap();
        assert_eq!(vox.mix.fx.len(), vocal_mix().fx.len() + p.fx.len());
        let out = keep(&mut bp, &src, "song", &[(p, false)], &Options::default(), &dir).unwrap();
        let vox = bp.loops.iter().find(|l| l.name == out.tracks[0]).unwrap();
        assert_eq!(vox.mix.fx.len(), vocal_mix().fx.len());
        assert!(keep(&mut bp, &src, "song", &[], &Options::default(), &dir).unwrap_err().starts_with("keep at least"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
