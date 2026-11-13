//! A remake that remixes the recording, rather than writing a new song only on its skeleton: the recording cut into
//! the pieces a producer would take from it, and a song of the genre built on them, as a song built on samples is
//! (`crate::sampling`).
//!
//! - Its hook, eight bars of the loudest section with chords (a hook or a drop when it has one; the loudest eight of it,
//!   `loudest`), without its drums (and, when its stems are apart and it does not sing, without its voice), is the
//!   phrase the song is built on: the song takes its key, its chords and a tempo it plays at, the recording's own or a
//!   multiple of it, faster or slower by whole semitones or chopped onto the song's grid (`sampling::meets`). A hook a
//!   voice sings in plays in its own key, unless the genre speeds its singers up (`SampleNeed::speeds_voices`).
//! - Another passage of it as long (of its verses, when it has any) is where the song's other sections start.
//! - The song goes on through the recording: each of its sections, but its intro, breaks and outro, plays as many bars
//!   of it on end as it has itself, of the kind it plays (its hooks' and drops', or the rest's), from the hook, from the
//!   other passage, or from bars no section before it played, each on its own bars' chords (`go_through`). When it
//!   sings (the ears hear a voice, or a pitch in a voice's range carries a third of a phrase), its voice sings on in
//!   them as it sang there.
//! - Its own drums, the busiest bars of them without the rest, are the break, in a genre that plays one.
//! - A flip of it plays in the intro, the breaks and the outro: as many bars as the hook of half beats, each from
//!   another bar of the recording that plays the hook's chord there, at the same place in its bar (now and then
//!   another), so that the song is heard in pieces from all over it, still on the hook's chords.
//!
//! Everything else is the genre's: its drums and bass, its parts, its form and its mix. That is the flip (`remix`).
//!
//! A rework (`rework`) keeps the recording whole instead, so that it stays the song it is: it plays from its first
//! downbeat to its end without its drums and with its low end cut, its voice, chords and melodies as they are, at its
//! own speed and pitch, its voice on a track of its own when its stems are apart. The song is its form bar for bar, on
//! its chords bar by bar, in its swing, and the genre plays under it: its drums, its bass on those chords' roots, its
//! break and textures, its mix.

use std::collections::BTreeMap;
use std::path::Path;

use crate::analysis::song::{Analysis, SectionGuess};
use crate::audio::Stereo;
use crate::compose::{self, FamilyPick, Library, Options};
use crate::config::SRF;
use crate::dsp::fft;
use crate::genres::{GenreProfile, LoopSpec, SampleNeed};
use crate::library::prepared::{Passage, Prepared, Role};
use crate::model::*;
use crate::rng::Rng;
use crate::sampling::{BREAK, PHRASE, SECOND};
use crate::stems::Stems;
use crate::theory::scale::Key;

/// the track a remix's flip plays on
pub const FLIP: &str = "flip";

/// the track a rework plays the whole recording on (all of it but its voice, when its stems are apart)
pub const ORIGINAL: &str = "original";

/// the track a rework plays the recording's voice on, when its stems are apart
pub const VOICE: &str = "vocals";

/// the share of a recording's power its bass and the rest hold at least for a hook cut from them to carry a song
/// without its voice (less: a voice over a beat, whose hook keeps its voice)
const BACKING: f64 = 0.15;

/// bars of a recording, as its analysis counts them: the first, and how many
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bars {
    pub first: usize,
    pub count: usize,
}

impl Bars {
    fn holds(&self, bar: usize) -> bool {
        (self.first..self.first + self.count).contains(&bar)
    }

    fn overlaps(&self, o: Bars) -> bool {
        self.first < o.first + o.count && o.first < self.first + self.count
    }

    fn meets(&self, s: &SectionGuess) -> bool {
        self.first < s.start_bar + s.bars && s.start_bar < self.first + self.count
    }

    /// in words, counting from 1
    fn said(&self) -> String {
        format!("bars {}-{}", self.first + 1, self.first + self.count)
    }
}

/// a section's pull toward being the hook: its energy, and more for a hook or a drop
fn hookness(s: &SectionGuess) -> f64 {
    s.energy as f64 + if matches!(s.kind, SectionKind::Hook | SectionKind::Drop) { 0.25 } else { 0.0 }
}

/// The hooks a remix may be built on, each with how much it pulls toward being one: eight bars from the start of each
/// section with chords (four, or two, of a shorter one).
fn hooks(a: &Analysis) -> Vec<(f64, Bars)> {
    a.sections
        .iter()
        .filter(|s| s.harmony && s.bars >= 2 && s.start_bar + 2 <= a.bars)
        .map(|s| {
            let count = [8, 4, 2].into_iter().find(|&n| s.bars >= n && s.start_bar + n <= a.bars).unwrap_or(2);
            (hookness(s), Bars { first: s.start_bar, count })
        })
        .collect()
}

/// The passages of the verses as long as the hook, from the sections with chords elsewhere but the intro and the
/// outro, each scored as one: a verse more than another kind, the quieter more.
fn verses(a: &Analysis, hook: Bars) -> Vec<(f64, Bars)> {
    a.sections
        .iter()
        .filter(|s| s.harmony && s.bars >= hook.count && s.start_bar + hook.count <= a.bars && !hook.meets(s))
        .filter(|s| !matches!(s.kind, SectionKind::Intro | SectionKind::Outro))
        .map(|s| {
            let verse = if s.kind == SectionKind::Verse { 1.0 } else { 0.0 };
            (verse - s.energy as f64, Bars { first: s.start_bar, count: hook.count })
        })
        .collect()
}

/// The recording's drums, as many bars as the hook from each section with a kick, the louder the busier.
fn drum_runs(a: &Analysis, hook: Bars) -> Vec<(f64, Bars)> {
    a.sections
        .iter()
        .filter(|s| s.kick && s.bars >= hook.count && s.start_bar + hook.count <= a.bars)
        .map(|s| (s.energy as f64, Bars { first: s.start_bar, count: hook.count }))
        .collect()
}

/// the one of `options` that scores best
fn best<T>(options: Vec<(f64, T)>) -> Option<T> {
    options.into_iter().max_by(|x, y| x.0.total_cmp(&y.0)).map(|o| o.1)
}

/// how far under the best an option may score and still be drawn, and how strongly the better are preferred among
/// them: the smaller, the more
const NEAR: f64 = 0.2;
const CHOOSY: f64 = 0.1;

/// One of `options` drawn with `rng` among those that score within `NEAR` of the best, the better the likelier (the
/// Gumbel-max trick): one remix of a recording is built on its best hook, another on its other chorus.
fn draw<T>(options: Vec<(f64, T)>, rng: &mut Rng) -> Option<T> {
    let top = options.iter().map(|o| o.0).fold(f64::MIN, f64::max);
    options
        .into_iter()
        .filter(|o| o.0 >= top - NEAR)
        .map(|(score, o)| (score / CHOOSY - (-rng.range(1e-12, 1.0).ln()).ln(), o))
        .max_by(|x, y| x.0.total_cmp(&y.0))
        .map(|o| o.1)
}

/// the hook: the section with chords that pulls most toward being one (`hooks`)
pub fn hook_bars(a: &Analysis) -> Option<Bars> {
    best(hooks(a))
}

/// a passage of the verses as long as the hook (`verses`): a verse when there is one, else the quietest that is not
/// an intro or an outro
pub fn verse_bars(a: &Analysis, hook: Bars) -> Option<Bars> {
    best(verses(a, hook))
}

/// the busiest of the recording's drums, as many bars as the hook: the loudest section with a kick
pub fn drum_bars(a: &Analysis, hook: Bars) -> Option<Bars> {
    best(drum_runs(a, hook))
}

/// Bars `b` moved, within the section they start, to the run of as many bars (a whole number of runs into it) where `x`
/// is loudest, by a decibel at least, so that a piece is not cut from a quiet opening: a section heard as one may hold a
/// long intro, or bars where only what `x` leaves out plays (a voice alone, with `x` the rest).
fn loudest(a: &Analysis, x: &Stereo, b: Bars) -> Bars {
    let Some(s) = a.sections.iter().find(|s| s.start_bar == b.first) else { return b };
    let power = |first: usize| power_of(a, x, first, b.count);
    let end = (s.start_bar + s.bars).min(a.bars);
    let mut best = (b.first, power(b.first));
    for first in (b.first..).step_by(b.count.max(1)).skip(1).take_while(|f| f + b.count <= end) {
        let p = power(first);
        if p > best.1 * 1.26 {
            best = (first, p);
        }
    }
    Bars { first: best.0, count: b.count }
}

/// the mean square of `x` over the recording's bars `first` to `first + count`, as `a` heard them
fn power_of(a: &Analysis, x: &Stereo, first: usize, count: usize) -> f64 {
    let at = |bar: usize| ((a.bar_start(bar) * SRF).max(0.0) as usize).min(x.len());
    let (i0, i1) = (at(first), at(first + count));
    x.l[i0..i1].iter().chain(&x.r[i0..i1]).map(|v| (*v as f64).powi(2)).sum::<f64>() / (2 * (i1 - i0)).max(1) as f64
}

fn db(power: f64) -> f64 {
    10.0 * power.max(1e-12).log10()
}

/// how far under a recording's loudest bar one is a rest, which no levelling raises
const REST_DB: f64 = 30.0;
/// the most a flip raises or lowers a piece to bring it to its hook's level
const FLIP_MOST_DB: f64 = 18.0;
/// how alike two bars' notes are at least for a flip to take one for the other (`alike`): A minor's for A minor
/// seventh's (0.87), not for C major's or F major's, which share two of its notes (0.67)
const NEAR_NOTES: f32 = 0.85;

/// how alike two bars' pitch classes are, 0 to 1: the cosine of the angle between them
fn alike(x: &[f32; 12], y: &[f32; 12]) -> f32 {
    let dot: f32 = x.iter().zip(y).map(|(a, b)| a * b).sum();
    let norm = |v: &[f32; 12]| v.iter().map(|a| a * a).sum::<f32>().sqrt();
    dot / (norm(x) * norm(y)).max(1e-9)
}
/// how much of the way a rework brings each of the recording's sections to their middle level, and the most it moves one
const REWORK_LEVEL: f64 = 0.5;
const REWORK_MOST_DB: f64 = 9.0;

/// The gain that brings each of the recording's bars in `x` to `to` dB (mean square), `share` of the way and at most
/// `most` dB either way; 1 for a bar that is a rest.
fn bar_gains(a: &Analysis, x: &Stereo, to: f64, share: f64, most: f64) -> Vec<f32> {
    let levels: Vec<f64> = (0..a.bars).map(|k| db(power_of(a, x, k, 1))).collect();
    let top = levels.iter().copied().fold(f64::MIN, f64::max);
    levels
        .iter()
        .map(|&d| if d < top - REST_DB { 1.0 } else { 10f64.powf((share * (to - d)).clamp(-most, most) / 20.0) as f32 })
        .collect()
}

/// `x` with each of the recording's sections brought `REWORK_LEVEL` of the way to the middle of their levels (at most
/// `REWORK_MOST_DB`), the gain gliding to the next section's over the last beat before it, as a fader moves ahead of a
/// change: a song's quiet verse, against the genre's drums, which keep one level, would all but go under them, and a
/// loud hook over them. The song keeps its shape, at half its range.
fn sections_levelled(x: &Stereo, a: &Analysis) -> Stereo {
    let levels: Vec<f64> = a.sections.iter().map(|s| db(power_of(a, x, s.start_bar, s.bars))).collect();
    let top = levels.iter().copied().fold(f64::MIN, f64::max);
    let mut heard: Vec<f64> = levels.iter().copied().filter(|&d| d >= top - REST_DB).collect();
    if heard.len() < 2 {
        return x.clone();
    }
    heard.sort_by(f64::total_cmp);
    let middle = heard[heard.len() / 2];
    let gains: Vec<f32> = levels
        .iter()
        .map(|&d| {
            let g = if d < top - REST_DB {
                0.0
            } else {
                (REWORK_LEVEL * (middle - d)).clamp(-REWORK_MOST_DB, REWORK_MOST_DB)
            };
            10f64.powf(g / 20.0) as f32
        })
        .collect();
    let mut out = x.clone();
    let glide = ((a.bar_secs / 4.0) * SRF) as usize;
    for (i, s) in a.sections.iter().enumerate() {
        let at = |bar: usize| ((a.bar_start(bar) * SRF).max(0.0) as usize).min(x.len());
        let last = i + 1 == a.sections.len();
        let (from, to) =
            (if i == 0 { 0 } else { at(s.start_bar) }, if last { x.len() } else { at(s.start_bar + s.bars) });
        let (now, then) = (gains[i], if last { gains[i] } else { gains[i + 1] });
        let turn = to.saturating_sub(glide).max(from);
        for t in from..to {
            let g = if t >= turn { now + (then - now) * (t - turn) as f32 / (to - turn).max(1) as f32 } else { now };
            out.l[t] *= g;
            out.r[t] *= g;
        }
    }
    out
}

/// `x`'s sustained part and its strikes (`fft::harmonic`), which add up to it
fn split(x: &Stereo) -> (Stereo, Stereo) {
    let sustained = Stereo { l: fft::harmonic(&x.l), r: fft::harmonic(&x.r) };
    let struck = Stereo {
        l: x.l.iter().zip(&sustained.l).map(|(a, b)| a - b).collect(),
        r: x.r.iter().zip(&sustained.r).map(|(a, b)| a - b).collect(),
    };
    (sustained, struck)
}

/// bars `b` of `x` on the beats the recording plays, each as long as another (`Analysis::cut`), faded in and out over
/// a few milliseconds so that they loop without a click
pub(crate) fn cut(x: &Stereo, a: &Analysis, b: Bars) -> Stereo {
    let mut out = a.cut(x, b.first, b.count);
    let n = out.len();
    out.fade(((0.002 * SRF) as usize).min(n / 4), ((0.004 * SRF) as usize).min(n / 4));
    out
}

/// the chord of each of bars `b`, as Roman numerals in the recording's key; none unless the analysis heard them all
fn chords_of(a: &Analysis, b: Bars) -> Vec<String> {
    let chords: Vec<String> = (b.first..b.first + b.count).filter_map(|k| a.numerals.get(k).cloned()).collect();
    if chords.len() == b.count { chords } else { Vec::new() }
}

/// The flip: as many bars as the hook of half beats, each from another bar of the recording, of a section with chords,
/// that plays the hook's chord at that place. Where no other is heard on that chord, or no chord is heard, it is one of
/// the three whose notes are nearest the hook's there, `NEAR_NOTES` at least: the same chord heard as another (an A
/// minor and its seventh), never another chord; the hook's own bar when none is. Each comes from the same place in its
/// bar, now and then another place in it, and each as loud as the hook (its bar brought to the hook's level, at most
/// `FLIP_MOST_DB` either way: a piece of a quiet verse beside one of a loud chorus would jump). The audio, and the bar
/// each half beat came from.
pub fn flip(a: &Analysis, sustained: &Stereo, hook: Bars, rng: &mut Rng) -> (Stereo, Vec<usize>) {
    let gains = bar_gains(a, sustained, db(power_of(a, sustained, hook.first, hook.count)), 1.0, FLIP_MOST_DB);
    let half = a.bar_secs / 8.0;
    let mut out = Stereo::zeros((hook.count as f64 * a.bar_secs * SRF).round() as usize);
    let harmonic = |k: &usize| {
        !hook.holds(*k) && a.sections.iter().any(|s| s.harmony && (s.start_bar..s.start_bar + s.bars).contains(k))
    };
    let with_chords: Vec<usize> = (0..a.bars.min(a.numerals.len())).filter(harmonic).collect();
    let with_notes: Vec<usize> = (0..a.bars.min(a.bar_chroma.len())).filter(harmonic).collect();
    let nearest = |bar: usize| -> Vec<usize> {
        let Some(c) = a.bar_chroma.get(bar) else { return Vec::new() };
        let mut by: Vec<(f32, usize)> = with_notes
            .iter()
            .map(|&k| (alike(c, &a.bar_chroma[k]), k))
            .filter(|(near, _)| *near >= NEAR_NOTES)
            .collect();
        by.sort_by(|x, y| y.0.total_cmp(&x.0));
        by.into_iter().take(3).map(|(_, k)| k).collect()
    };
    let slot = (half * SRF).round() as usize;
    let (fade_in, fade_out) = ((0.002 * SRF) as usize, (0.004 * SRF) as usize);
    let mut from = Vec::new();
    for i in 0..hook.count * 8 {
        let (b, j) = (i / 8, i % 8);
        let chord = a.numerals.get(hook.first + b);
        let mut same: Vec<usize> = with_chords.iter().copied().filter(|&k| a.numerals.get(k) == chord).collect();
        if same.is_empty() {
            same = nearest(hook.first + b);
        }
        let k = if same.is_empty() { hook.first + b } else { same[rng.below(same.len())] };
        let j = if rng.chance(0.25) { rng.below(8) } else { j };
        from.push(k);
        // the half beat where it falls in that bar as it was played
        let (b0, b1) = (a.bar_start(k), a.bar_start(k + 1));
        let src = ((b0 + j as f64 * (b1 - b0) / 8.0) * SRF).round().max(0.0) as usize;
        let dst = (i as f64 * half * SRF).round() as usize;
        let gain = gains.get(k).copied().unwrap_or(1.0);
        for t in 0..slot {
            let (s, d) = (src + t, dst + t);
            if s >= sustained.len() || d >= out.len() {
                break;
            }
            let edge = gain * (t as f32 / fade_in as f32).min((slot - t) as f32 / fade_out as f32).min(1.0);
            out.l[d] += sustained.l[s] * edge;
            out.r[d] += sustained.r[s] * edge;
        }
    }
    (out, from)
}

/// The library a remix is made from: the recording's hook as the phrase and its drums as the break, and the
/// libraries' own one-shots, loops and textures; no other record's phrase, voice or bass.
struct Recording<'a> {
    phrase: &'a Prepared,
    drums: Option<&'a Prepared>,
    inner: Option<&'a dyn Library>,
}

impl Library for Recording<'_> {
    fn one_shot(&self, lane: &str, subtypes: &[String], genre: &GenreProfile, rng: &mut Rng) -> Option<Sound> {
        self.inner.and_then(|l| l.one_shot(lane, subtypes, genre, rng))
    }

    fn find_loop(
        &self,
        spec: &LoopSpec,
        genre: &GenreProfile,
        bpm: f64,
        key: &Key,
        rng: &mut Rng,
    ) -> Option<(Sound, i32)> {
        self.inner.and_then(|l| l.find_loop(spec, genre, bpm, key, rng))
    }

    fn family(&self, _: &GenreProfile, _: &mut Rng) -> Option<FamilyPick> {
        None
    }

    fn prepared(&self, role: Role, genre: &GenreProfile, asked: Option<&str>, rng: &mut Rng) -> Option<Prepared> {
        match role {
            Role::Phrase => Some(self.phrase.clone()),
            Role::Break => self.drums.cloned(),
            Role::Texture => self.inner.and_then(|l| l.prepared(role, genre, asked, rng)),
            Role::Vocal | Role::Bass => None,
        }
    }
}

/// The flip into `bp` as the phrase plays (at its tempo, pitch and grid), in the intro, the breaks and the outro, in
/// place of the phrase there, on the hook's chords; not where the phrase's other passage plays, which keeps its own.
/// The sections it plays in.
fn place_flip(bp: &mut Blueprint, path: &str, label: &str) -> Vec<String> {
    let Some(mut lp) = bp.loops.iter().find(|l| l.name == PHRASE).cloned() else { return Vec::new() };
    lp.name = FLIP.into();
    lp.chop = Vec::new();
    lp.mix.level_db -= 2.0;
    if let Sound::Loop { path: p, label: l, .. } = &mut lp.sound {
        *p = path.to_string();
        *l = format!("{label}, a flip of it");
    }
    let mut at = Vec::new();
    let quiet = |s: &SectionSpec| matches!(s.kind, SectionKind::Intro | SectionKind::Break | SectionKind::Outro);
    for s in bp.form.iter_mut().filter(|s| quiet(s) && !s.layers.iter().any(|l| l == SECOND)) {
        s.layers.retain(|l| l != PHRASE);
        s.layers.push(FLIP.into());
        s.prog = "A".into();
        at.push(s.name.clone());
    }
    if !at.is_empty() {
        bp.loops.push(lp);
    }
    at
}

/// The passages of the recording a flip goes on through, besides its hook and its other passage: each run of as many
/// bars as the hook in a section with chords that is not its intro or its outro, from the section's start, none of it
/// the hook's or the other passage's, and none so quiet it would drop out of the song (`FLIP_MOST_DB` under the hook);
/// those of its hooks and drops, and those of the rest, each in the recording's order.
fn passages(a: &Analysis, x: &Stereo, hook: Bars, verse: Option<Bars>) -> (Vec<Bars>, Vec<Bars>) {
    let loud = db(power_of(a, x, hook.first, hook.count));
    let (mut lifts, mut rest) = (Vec::new(), Vec::new());
    let inner = |s: &&SectionGuess| s.harmony && !matches!(s.kind, SectionKind::Intro | SectionKind::Outro);
    for s in a.sections.iter().filter(inner) {
        let end = (s.start_bar + s.bars).min(a.bars).min(a.numerals.len());
        for first in (s.start_bar..).step_by(hook.count).take_while(|f| f + hook.count <= end) {
            let b = Bars { first, count: hook.count };
            if b.overlaps(hook) || verse.is_some_and(|v| b.overlaps(v)) {
                continue;
            }
            if loud - db(power_of(a, x, b.first, b.count)) > FLIP_MOST_DB {
                continue;
            }
            match s.kind {
                SectionKind::Hook | SectionKind::Drop => lifts.push(b),
                _ => rest.push(b),
            }
        }
    }
    (lifts, rest)
}

/// The chords of bars `b` as the song counts its hook's (`A`, from `chords_of` the hook): moved as the hook was by
/// `semis` into the song's key, and a chord of the song to each of their bars, or to two of them where the song counts
/// the hook at half its tempo. None where they do not read.
fn counted_as_hook(bp: &Blueprint, a: &Analysis, hook: Bars, b: Bars, semis: i32) -> Vec<String> {
    let (theirs, numerals) = (chords_of(a, hook), chords_of(a, b));
    let Some(ours) = bp.harmony.progressions.get("A").filter(|_| !theirs.is_empty() && !numerals.is_empty()) else {
        return Vec::new();
    };
    let heard = Key::new((a.key.tonic as i32 + semis).rem_euclid(12) as u8, a.key.mode);
    let chords = crate::theory::progression::chords(&numerals, &heard);
    if chords.len() != numerals.len() {
        return Vec::new();
    }
    let chords: Vec<String> = chords.iter().map(|c| crate::theory::chord::roman_of(c, &bp.key)).collect();
    match ours.len() * 2 == theirs.len() {
        true => chords.into_iter().step_by(2).collect(),
        false => chords,
    }
}

/// the most bars of the recording a section of a flip plays in one go
const LONGEST: usize = 32;

/// The song going on through the recording, a section at a time. Each of its sections that plays the hook (`PHRASE`)
/// or the other passage (`SECOND`), but its intro, breaks and outro, plays bars of the recording on end, cut by `store`
/// and played as the hook is, on a loop track of its own (`sample3`, `sample4` ...) and on those bars' chords (`A3`,
/// `A4` ...): from where it has the most of them, up to as many as the section holds (half as many as its bars in
/// double time, twice as many in half time; `LONGEST` at most, a whole number of the hook's), while they are of the
/// same kind (a hook's or a drop's, or the rest's) and no section has played them.
/// Where it may start: the first of the hook's sections from the hook, the first of the other passage's from it, and
/// each from any of `lifts`, or of `rest`, not played yet (the other's, once its own are all played); so that the song
/// goes on through the recording rather than round a passage. A section whose bars would be the hook or the other
/// passage plays it, one the recording has nothing left for plays what it did, and the hook's and the other passage's
/// tracks go when no section plays them. Each section that plays on, and its bars.
fn go_through(
    bp: &mut Blueprint,
    a: &Analysis,
    hook: Bars,
    verse: Option<Bars>,
    runs: [Vec<Bars>; 2],
    store: &mut dyn FnMut(Bars) -> Result<(String, String), String>,
) -> Result<Vec<(String, Bars)>, String> {
    let template = |name: &str| bp.loops.iter().find(|l| l.name == name).cloned();
    let Some(phrase) = template(PHRASE) else { return Ok(Vec::new()) };
    let second = template(SECOND).unwrap_or_else(|| phrase.clone());
    let semis = phrase.transpose;
    // the song's bars to each of the recording's, as the hook's loop spans them: two in double time, half of one in
    // half time
    let per = match &phrase.sound {
        Sound::Loop { beats, .. } if *beats > 0.0 => beats / (hook.count as f64 * 4.0),
        _ => 1.0,
    };
    // each lane: the track its sections play, as which a passage plays, and where its first section starts
    let base = [Some(hook.first), verse.map(|v| v.first)];
    let mut lanes = [(PHRASE, phrase, base[0]), (SECOND, second, base[1])];
    // the kind of bars each is: a hook's or a drop's (0), the rest's (1), or neither
    let kind = |bar: usize| {
        let s = a.sections.iter().find(|s| (s.start_bar..s.start_bar + s.bars).contains(&bar))?;
        let inner = s.harmony && !matches!(s.kind, SectionKind::Intro | SectionKind::Outro);
        inner.then(|| usize::from(!matches!(s.kind, SectionKind::Hook | SectionKind::Drop)))
    };
    let end = a.bars.min(a.numerals.len());
    let mut played: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    let mut went = Vec::new();
    for i in 0..bp.form.len() {
        let s = &bp.form[i];
        if matches!(s.kind, SectionKind::Intro | SectionKind::Break | SectionKind::Outro) {
            continue;
        }
        let Some(lane) = lanes.iter().position(|(name, ..)| s.layers.iter().any(|l| l == *name)) else { continue };
        let most = ((s.bars as f64 / per + 1e-6) as usize).min(LONGEST);
        let length = |first: usize| {
            let on = (first..end).take(most).take_while(|&b| !played.contains(&b) && kind(b) == kind(first)).count();
            on / hook.count * hook.count
        };
        let fresh = |runs: &[Bars]| runs.iter().map(|r| r.first).filter(|f| !played.contains(f)).collect::<Vec<_>>();
        let mut starts: Vec<usize> = lanes[lane].2.take().into_iter().collect();
        starts.extend(fresh(&runs[lane]));
        if starts.is_empty() {
            starts = fresh(&runs[1 - lane]);
        }
        let Some((first, count)) = starts
            .into_iter()
            .map(|f| (f, length(f)))
            .filter(|&(_, n)| n > 0)
            .min_by_key(|&(_, n)| std::cmp::Reverse(n))
        else {
            continue;
        };
        played.extend(first..first + count);
        if count == hook.count && Some(first) == base[lane] {
            continue;
        }
        let b = Bars { first, count };
        let (name, template, _) = &lanes[lane];
        let mut on = template.clone();
        on.name = format!("{PHRASE}{}", 3 + went.len());
        let (path, label) = store(b)?;
        let times = (count / hook.count) as f64;
        if let Sound::Loop { path: p, label: l, beats, .. } = &mut on.sound {
            (*p, *l, *beats) = (path, label, *beats * times);
        }
        // a chopped hook's slices, again for each run of it the passage holds
        let slices = template.chop.len() as i32;
        on.chop =
            (0..count / hook.count).flat_map(|r| template.chop.iter().map(move |c| c + r as i32 * slices)).collect();
        let chords = counted_as_hook(bp, a, hook, b, semis);
        let sec = &mut bp.form[i];
        for l in sec.layers.iter_mut().filter(|l| *l == *name) {
            *l = on.name.clone();
        }
        if !chords.is_empty() {
            sec.prog = format!("A{}", 3 + went.len());
            bp.harmony.progressions.insert(sec.prog.clone(), chords);
        }
        went.push((sec.name.clone(), b));
        bp.loops.push(on);
    }
    let heard = |name: &str| bp.form.iter().any(|s| s.layers.iter().any(|l| l == name));
    let gone: Vec<&str> = [PHRASE, SECOND].into_iter().filter(|n| !heard(n)).collect();
    bp.loops.retain(|l| !gone.contains(&l.name.as_str()));
    if !bp.form.iter().any(|s| s.prog == crate::sampling::SECOND_PROG) {
        bp.harmony.progressions.remove(crate::sampling::SECOND_PROG);
    }
    Ok(went)
}

/// the recording a remix is made of: its audio, what it is called, and what its analysis heard in it
pub struct Source<'a> {
    pub x: &'a Stereo,
    pub name: &'a str,
    pub a: &'a Analysis,
    /// its vocals, drums, bass and the rest apart, when the ears separated them (`crate::stems`)
    pub stems: Option<&'a Stems>,
}

/// what a remix cuts its pieces from
struct Apart {
    /// the recording without its drums, and without its voice when `voiced` is not
    music: Stereo,
    drums: Stereo,
    /// whether its voice, if it sings, is in `music`
    voiced: bool,
}

/// The recording apart. With its stems apart: all but its drums when it sings (`sings`), so that its voice sings on in
/// the passages it sings in rather than in a phrase or two of it coming round; else its bass and the rest, unless they
/// hold too little of it to carry a hook (`BACKING`). Without them, its sustained part and its strikes (`split`), voice
/// and all.
fn apart(src: &Source, sings: bool) -> Apart {
    match src.stems {
        Some(s) if sings => Apart { music: s.music(), drums: s.drums.clone(), voiced: true },
        Some(s) if s.share(&s.bass) + s.share(&s.other) >= BACKING => {
            Apart { music: s.backing(), drums: s.drums.clone(), voiced: false }
        }
        Some(s) => Apart { music: s.music(), drums: s.drums.clone(), voiced: true },
        None => {
            let (music, drums) = split(src.x);
            Apart { music, drums, voiced: true }
        }
    }
}

/// Whether a hook of `bpm`, with a pulse as clear as `pulse`, plays in its own key in a song of a genre that samples
/// as `n` does at a tempo in `range`: a voice sings in it (`sung`), which the genre does not mean to be sped up or
/// slowed down (`SampleNeed::speeds_voices`), and it meets the genre's tempo at its own pitch some way (`meets`:
/// stretched, where claudius stretches, or at its own tempo or a multiple of it). A voice sped up four semitones sings
/// like a cartoon's chipmunk, and a remake's sung phrases would be moved with it.
fn own_key(n: &SampleNeed, sung: bool, bpm: f64, pulse: f64, range: (f64, f64)) -> bool {
    sung && !n.speeds_voices
        && !crate::sampling::meets(bpm, pulse, 0.0, range, (0, 0), cfg!(feature = "native")).is_empty()
}

/// A remix of the recording `src` as a song of genre `g` (the module's doc says how), its pieces stored in `dir` with
/// the song's takes. `opts` as for a new song (its tempo, its layers); `lib` for the drums' one-shots and the genre's
/// loops; the ears, when given, listen for the recording's voice.
pub fn remix(
    src: &Source,
    g: &GenreProfile,
    seed: u64,
    opts: &Options,
    lib: Option<&dyn Library>,
    dir: &Path,
    ears: Option<&crate::ears::Ears>,
) -> Result<Blueprint, String> {
    let (x, name, a) = (src.x, src.name, src.a);
    // each remix of it draws its hook, its other passage and its drums anew among the good ones
    let mut takes = Rng::seed_from(seed, "remix");
    let hook =
        draw(hooks(a), &mut takes).ok_or("the recording has no section with chords long enough to build a remix on")?;
    // whether it sings: a voice the ears hear, or a pitch followed through a third of a phrase
    let voiced = crate::sampler::Options::default();
    let sings = crate::sampler::find(x, src.stems, a, ears, &voiced, 1)
        .first()
        .is_some_and(|p| p.sung >= 0.35 || p.z.is_some_and(|z| z >= 1.5));
    let Apart { music: sustained, drums: struck, voiced: voice_in_it } = apart(src, sings);
    let hook = loudest(a, &sustained, hook);
    let slug = crate::song::slug(Path::new(name).file_stem().map_or(name.into(), |s| s.to_string_lossy()).as_ref());
    let folder = dir.join(&slug);
    let store = |audio: &Stereo| -> Result<String, String> {
        crate::take::store(audio, &folder).map(|p| p.to_string_lossy().into_owned()).map_err(|e| format!("{e:#}"))
    };
    // a piece's tempo as its own length says it, which loops it to the sample
    let piece = |b: Bars, from: &Stereo| -> Result<(String, f64), String> {
        let c = cut(from, a, b);
        let secs = c.len() as f64 / SRF;
        Ok((store(&c)?, 60.0 * (b.count * 4) as f64 / secs.max(1e-3)))
    };
    let energy = |b: Bars| a.sections.iter().find(|s| b.meets(s)).map_or(0.0, |s| s.energy);
    let (path, bpm) = piece(hook, &sustained)?;
    let verse = draw(verses(a, hook), &mut takes).map(|v| loudest(a, &sustained, v));
    let second = match verse {
        Some(v) => {
            Some(Passage { path: piece(v, &sustained)?.0, chords: chords_of(a, v), louder: energy(v) > energy(hook) })
        }
        None => None,
    };
    let phrase = Prepared {
        path,
        role: Role::Phrase,
        genre: g.name.clone(),
        bpm,
        beats: (hook.count * 4) as f64,
        key: Some(a.key.tonic),
        minor: a.key.mode.is_minor(),
        kick: None,
        snare: None,
        hat: None,
        chords: chords_of(a, hook),
        z: None,
        source: format!("recording:{name}"),
        asked: None,
        second,
        root: None,
        pulse: Some(a.pulse_clarity),
    };
    let drum_bars = crate::sampling::need(g, Role::Break)
        .and_then(|_| draw(drum_runs(a, hook), &mut takes))
        .map(|b| loudest(a, &struck, b));
    let drums = match drum_bars {
        Some(b) => {
            let (path, bpm) = piece(b, &struck)?;
            Some(Prepared {
                path,
                role: Role::Break,
                bpm,
                key: None,
                minor: false,
                chords: Vec::new(),
                second: None,
                ..phrase.clone()
            })
        }
        None => None,
    };
    // the hook is what the remix is built on: it plays, whatever the genre's chance for a phrase; a voice in it, which
    // is not chopped up either, in its own key
    let mut built = g.clone();
    let range = opts.bpm.map_or(g.bpm, |b| (b * 0.997, b * 1.003));
    let mut in_key = false;
    if let Some(n) = built.sampling.as_mut().and_then(|s| s.phrase.as_mut()) {
        n.chance = 1.0;
        in_key = own_key(n, sings && voice_in_it, bpm, a.pulse_clarity, range);
        if in_key {
            n.pitch = (0, 0);
            n.chop = 0.0;
        }
    }
    let library = Recording { phrase: &phrase, drums: drums.as_ref(), inner: lib };
    let o = Options { sampled: Some(true), sample_search: None, family: false, ..opts.clone() };
    let mut bp = compose::generate(&built, seed, &o, Some(&library));
    if !bp.loops.iter().any(|l| l.name == PHRASE) {
        return Err(format!("{} could not play the recording's hook", g.name.replace('_', " ")));
    }
    let without = if voice_in_it { "its drums" } else { "its drums and its voice" };
    let key = if in_key { ", in its own key for the voice in it" } else { "" };
    let mut said = vec![format!("its hook ({}) without {without}{key}", hook.said())];
    if let Some(v) = verse {
        said.push(format!("another passage of it ({})", v.said()));
    }
    if let Some(b) = drum_bars.filter(|_| bp.loops.iter().any(|l| l.name == BREAK)) {
        said.push(format!("its own drums ({}) as the break", b.said()));
    }
    let (flipped, from) = flip(a, &sustained, hook, &mut Rng::new(seed ^ 0xf11f));
    let at = place_flip(&mut bp, &store(&flipped)?, &slug);
    if !at.is_empty() {
        let mut bars = from.clone();
        bars.sort_unstable();
        bars.dedup();
        said.push(format!("a flip of {} half beats from {} of its bars in {}", from.len(), bars.len(), at.join(", ")));
    }
    // and on through the rest of it, a passage after another, rather than round the same two
    let mut cut_it = |b: Bars| -> Result<(String, String), String> {
        Ok((piece(b, &sustained)?.0, format!("{slug}, {}", b.said())))
    };
    let (lifts, rest) = passages(a, &sustained, hook, verse);
    let went = go_through(&mut bp, a, hook, verse, [lifts, rest], &mut cut_it)?;
    if !went.is_empty() {
        let each: Vec<String> = went.iter().map(|(s, b)| format!("{s} {}", b.said())).collect();
        said.push(format!("on through it as the song goes ({})", each.join(", ")));
    }
    let line = format!("remixed from '{name}': {}", said.join("; "));
    bp.lineage.insert(1.min(bp.lineage.len()), line);
    Ok(bp)
}

/// how a rework's bars lie on the recording's
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Per {
    /// a bar of the song's to each of the recording's
    One,
    /// two of the song's to each of the recording's: double time
    Two,
    /// one of the song's to two of the recording's: half time
    Half,
}

impl Per {
    /// the song's bars to each of the recording's
    pub fn factor(self) -> f64 {
        match self {
            Per::One => 1.0,
            Per::Two => 2.0,
            Per::Half => 0.5,
        }
    }

    /// the song's bar that the recording's bar `b` starts in
    fn song_bar(self, b: usize) -> usize {
        match self {
            Per::One => b,
            Per::Two => 2 * b,
            Per::Half => b / 2,
        }
    }
}

/// The tempo a rework plays at, and how the song's bars lie on the recording's: its own tempo (from the length of its
/// bars), double it (two bars to each of its own) or half it (one to two of its own) when that is nearer the genre's
/// range, so that it plays at its own speed and pitch on the song's grid, and the genre's drums at a tempo the genre
/// plays (a hip hop beat under a song of 170 BPM plays at 85, not as fast as drum and bass). Half only where the
/// recording keeps its chord through two bars at a time, three pairs in four at least, since the song plays one chord a
/// bar at most: a recording heard at twice its tempo, whose bars are each half of one, always does.
pub fn rework_tempo(a: &Analysis, g: &GenreProfile) -> (f64, Per) {
    let own = 240.0 / a.bar_secs.max(1e-3);
    let (lo, hi) = g.bpm;
    let outside = |b: f64| (lo - b).max(b - hi).max(0.0);
    let pairs = a.numerals.chunks_exact(2).filter(|p| !p[0].is_empty() || !p[1].is_empty()).collect::<Vec<_>>();
    let held = pairs.iter().filter(|p| p[0] == p[1] || p[1].is_empty()).count();
    let halves = !pairs.is_empty() && 4 * held >= 3 * pairs.len();
    [Per::One, Per::Two, Per::Half]
        .into_iter()
        .filter(|p| *p != Per::Half || halves)
        .map(|p| (own * p.factor(), p))
        .min_by(|x, y| outside(x.0).total_cmp(&outside(y.0)))
        .unwrap_or((own, Per::One))
}

/// The library a rework draws on: the libraries' one-shots, loops, breaks and textures; no other record's phrase, voice
/// or bass, which would play against the recording, and a phrase would give the song its own key.
struct Under<'a> {
    inner: Option<&'a dyn Library>,
}

impl Library for Under<'_> {
    fn one_shot(&self, lane: &str, subtypes: &[String], genre: &GenreProfile, rng: &mut Rng) -> Option<Sound> {
        self.inner.and_then(|l| l.one_shot(lane, subtypes, genre, rng))
    }

    fn find_loop(
        &self,
        spec: &LoopSpec,
        genre: &GenreProfile,
        bpm: f64,
        key: &Key,
        rng: &mut Rng,
    ) -> Option<(Sound, i32)> {
        self.inner.and_then(|l| l.find_loop(spec, genre, bpm, key, rng))
    }

    fn family(&self, _: &GenreProfile, _: &mut Rng) -> Option<FamilyPick> {
        None
    }

    fn prepared(&self, role: Role, genre: &GenreProfile, asked: Option<&str>, rng: &mut Rng) -> Option<Prepared> {
        match role {
            Role::Break | Role::Texture => self.inner.and_then(|l| l.prepared(role, genre, asked, rng)),
            Role::Phrase | Role::Vocal | Role::Bass => None,
        }
    }
}

/// the `i`th progression's name, counting from 0: `A` to `Z`, then `AA`, `AB` ..., as a spreadsheet's columns go
pub(crate) fn progression_name(i: usize) -> String {
    let mut n = i + 1;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    out.iter().rev().map(|&b| b as char).collect()
}

/// the genre's section most like `s`: one of its kind, else the one nearest it in energy
fn like<'a>(form: &'a [SectionSpec], s: &SectionGuess) -> Option<&'a SectionSpec> {
    form.iter()
        .find(|t| t.kind == s.kind)
        .or_else(|| form.iter().min_by(|x, y| (x.energy - s.energy).abs().total_cmp(&(y.energy - s.energy).abs())))
}

/// The recording's sections as the song's, its bars lying on the song's as `per` says: each its own kind, length and
/// energy, on a progression of its own bars' chords (a chord to each of its bars, or in half time to each pair of them,
/// the first's; one name for each run of chords, `A` the first), with the layers the genre plays in a section like it
/// that `keep` takes and the recording's own `tracks`, and the progressions. In half time a section starts in the
/// song's bar its first bar falls in, and one of a single bar that shares the song's bar with the one before is left
/// to it.
fn rework_form(
    a: &Analysis,
    per: Per,
    genre: &[SectionSpec],
    keep: impl Fn(&str) -> bool,
    tracks: &[&str],
) -> (Vec<SectionSpec>, BTreeMap<String, Vec<String>>) {
    let mut form = Vec::new();
    let mut progs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut named: Vec<(Vec<String>, String)> = Vec::new();
    let mut last = a.numerals.iter().find(|c| !c.is_empty()).cloned().unwrap_or_else(|| "i".into());
    for (i, s) in a.sections.iter().enumerate() {
        let end = s.start_bar + s.bars;
        // in half time the last section takes the half of a bar an odd count leaves over, so none of it is cut
        let to = if per == Per::Half && i + 1 == a.sections.len() { end.div_ceil(2) } else { per.song_bar(end) };
        let from = per.song_bar(s.start_bar);
        if to <= from {
            continue;
        }
        // the recording's bars a chord each is taken from: each of them, or the first of each pair in half time
        let (first, count, step) = if per == Per::Half { (from, to - from, 2) } else { (s.start_bar, s.bars, 1) };
        let chords: Vec<String> = (first..first + count)
            .map(|b| {
                if let Some(c) = a.numerals.get(b * step).filter(|c| !c.is_empty()) {
                    last = c.clone();
                }
                last.clone()
            })
            .collect();
        let prog = match named.iter().find(|(c, _)| *c == chords) {
            Some((_, n)) => n.clone(),
            None => {
                let n = progression_name(named.len());
                named.push((chords.clone(), n.clone()));
                progs.insert(n.clone(), chords);
                n
            }
        };
        let of_kind = a.sections[..i].iter().filter(|o| o.kind == s.kind).count();
        let name = if of_kind == 0 { s.kind.name().to_string() } else { format!("{}{}", s.kind.name(), of_kind + 1) };
        let t = like(genre, s);
        let mut layers: Vec<String> = t.map_or(Vec::new(), |t| t.layers.iter().filter(|l| keep(l)).cloned().collect());
        layers.extend(tracks.iter().map(|t| t.to_string()));
        form.push(SectionSpec {
            name,
            kind: s.kind,
            bars: (to - from) as u32,
            energy: s.energy,
            prog,
            layers,
            variant: t.map_or_else(|| "main".into(), |t| t.variant.clone()),
            filter_in: false,
            build: false,
            build_style: Default::default(),
            build_amount: 0.5,
            gap: false,
            impact: t.is_some_and(|t| t.impact),
            fills: t.is_some_and(|t| t.fills),
        });
    }
    (form, progs)
}

/// the share of a recording's power the rest of it (not its voice, its drums or its bass) holds at least for a rework
/// to play it beside its voice
pub(crate) const REST: f64 = 0.02;

/// The recording's tracks in a rework, each the whole of it, the cut under what is not its voice, and what they leave
/// out, in words. Apart in its stems: the rest of it above its bass (`ORIGINAL`, its voice in it when it does not
/// sing, none when its voice is all there is above its bass), cut at 80 Hz, under the kick, and its voice (`VOICE`)
/// when it sings. Else its sustained part (`ORIGINAL`), cut at 150 Hz.
fn wholes(src: &Source) -> (Vec<(&'static str, Stereo)>, f32, &'static str) {
    match src.stems {
        Some(s) if s.sung() => {
            let rest = (s.share(&s.other) >= REST).then(|| (ORIGINAL, s.other.clone()));
            (rest.into_iter().chain([(VOICE, s.vocals.clone())]).collect(), 80.0, "its drums and its bass")
        }
        Some(s) => (vec![(ORIGINAL, s.upper())], 80.0, "its drums and its bass"),
        None => (vec![(ORIGINAL, split(src.x).0)], 150.0, "its drums and its low end"),
    }
}

/// the recording's voice as a rework mixes it: as a sampled phrase is (`sampler::vocal_mix`), but its body kept down
/// to 100 Hz, where a voice sung low still has it, less of the song's reverb, delay and ducking, for a singer's whole
/// line that its own room already holds, and 2 dB louder: the song's own singer, on the genre's drums and bass, not
/// under them
pub(crate) fn voice_mix() -> TrackMix {
    let mut mix = crate::sampler::vocal_mix();
    for fx in &mut mix.fx {
        if let crate::fx::Effect::Eq { bands } = fx {
            for b in bands.iter_mut().filter(|b| matches!(b.kind, crate::dsp::filter::BiquadKind::Highpass)) {
                b.freq = 100.0;
            }
        }
    }
    TrackMix { level_db: mix.level_db + 2.0, reverb: 0.12, delay: 0.08, duck: 0.2, ..mix }
}

/// A rework of the recording `src` as a song of genre `g` (the module's doc says how), the recording kept in `dir`
/// with the song's takes. `opts` as for a new song (its tempo, its layers); `lib` for the drums' one-shots and the
/// genre's own break, textures and loops.
pub fn rework(
    src: &Source,
    g: &GenreProfile,
    seed: u64,
    opts: &Options,
    lib: Option<&dyn Library>,
    dir: &Path,
) -> Result<Blueprint, String> {
    let (name, a) = (src.name, src.a);
    if a.bars < 8 || a.sections.is_empty() {
        return Err("the recording is too short, or its sections too unclear, to rework".into());
    }
    // a tempo asked for stretches the recording to it, bar for bar
    let (bpm, per) = opts.bpm.map_or_else(|| rework_tempo(a, g), |b| (b, Per::One));
    let o = Options { bpm: Some(bpm), key: Some(a.key), sample_search: None, family: false, ..opts.clone() };
    let mut bp = compose::generate(g, seed, &o, Some(&Under { inner: lib }));
    bp.key = a.key;
    // the genre's drums, bass, break and textures play under it; its chords, keys, pads and leads would play over the
    // recording's own (where the chords heard in it are wrong, against them), and another record's phrase or voice
    // against it. The bass plays only the chords' roots, which the analysis hears best.
    let lanes: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    let under = |p: &Part| p.role == crate::model::Role::Bass;
    let parts: Vec<String> = bp.parts.iter().filter(|p| under(p)).map(|p| p.name.clone()).collect();
    let loops = [BREAK, crate::sampling::TEXTURE];
    let keep = |l: &str| lanes.iter().chain(&parts).any(|n| n == l) || loops.contains(&l);
    // the recording, all of it, without its drums, and under the genre's bass and kick without its own
    let (wholes, low_cut, without) = wholes(src);
    let tracks: Vec<&str> = wholes.iter().map(|w| w.0).collect();
    let (form, progressions) = rework_form(a, per, &bp.form, keep, &tracks);
    bp.form = form;
    bp.harmony.progressions = progressions;
    bp.harmony.bars_per_chord = if per == Per::Two { 2 } else { 1 };
    bp.swing = a.swing;
    let used: Vec<String> = bp.form.iter().flat_map(|s| s.layers.iter().cloned()).collect();
    bp.parts.retain(|p| used.contains(&p.name));
    bp.loops.retain(|l| used.contains(&l.name));
    // what is not its voice goes through the genre's treatment of a phrase it samples, high-passed at `low_cut` at
    // least
    let slug = crate::song::slug(Path::new(name).file_stem().map_or(name.into(), |s| s.to_string_lossy()).as_ref());
    let mut mix = crate::sampling::need(g, Role::Phrase).map(|n| n.mix.clone()).unwrap_or_default();
    mix.fx.retain(|e| !matches!(e, crate::fx::Effect::Filter { mode: crate::dsp::filter::FilterMode::Hp, .. }));
    let hp = crate::fx::Effect::Filter {
        mode: crate::dsp::filter::FilterMode::Hp,
        cutoff: crate::dsp::env::Param::Const(low_cut),
        res: crate::dsp::env::Param::Const(0.05),
        drive: 1.0,
        mix: crate::dsp::env::Param::Const(1.0),
    };
    mix.fx.insert(0, hp);
    if mix.bus.is_empty() {
        mix.bus = "music".into();
    }
    // at its own tempo on the song's grid, its beats as long as the song's bars lay them (two of the song's to each of
    // its own in double time, one to two in half time), or stretched to a tempo asked for
    let own = 240.0 / a.bar_secs.max(1e-3) * per.factor();
    let voice_apart = tracks.contains(&VOICE);
    for (track, audio) in &wholes {
        let body = cut(&sections_levelled(audio, a), a, Bars { first: 0, count: a.bars });
        let path = crate::take::store(&body, &dir.join(&slug)).map_err(|e| format!("{e:#}"))?;
        let (role, mix, label) = match *track {
            VOICE => ("vocal", voice_mix(), format!("{slug}, its voice")),
            _ if voice_apart => ("sample", mix.clone(), format!("{slug}, all of it but its voice")),
            _ => ("sample", mix.clone(), format!("{slug}, all of it")),
        };
        bp.loops.push(LoopPart {
            name: track.to_string(),
            role: role.into(),
            sound: Sound::Loop {
                path: path.to_string_lossy().into_owned(),
                bpm: own,
                beats: a.bars as f64 * 4.0 * per.factor(),
                key: None,
                tonal: false,
                label,
            },
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix,
            from: bp.form.first().map(|s| s.name.clone()),
            sampled: true,
        });
    }
    let kept: Vec<&str> =
        bp.parts.iter().map(|p| p.name.as_str()).chain(bp.drums.iter().map(|d| d.name.as_str())).collect();
    let apart = if voice_apart { format!(", its voice on a track of its own ({VOICE})") } else { String::new() };
    let time = match per {
        Per::One => String::new(),
        Per::Two => format!(" in double time, at {bpm:.0} BPM to its {:.0}", 240.0 / a.bar_secs.max(1e-3)),
        Per::Half => format!(" in half time, at {bpm:.0} BPM to its {:.0}", 240.0 / a.bar_secs.max(1e-3)),
    };
    let line = format!(
        "reworked from '{name}': all {} bars of it, its voice and its chords as they are{apart}, without {without}, in \
         its own {} sections and on its chords; {}'s {} under it{time}",
        a.bars,
        bp.form.len(),
        g.name.replace('_', " "),
        kept.join(", ")
    );
    bp.lineage.insert(1.min(bp.lineage.len()), line);
    Ok(bp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::Mode;

    fn section(start_bar: usize, bars: usize, energy: f32, kind: SectionKind) -> SectionGuess {
        SectionGuess { start_bar, bars, energy, kind, kick: true, bass: true, hats: true, harmony: true }
    }

    /// sixteen bars at 96 BPM in A minor, `i VI III VII` a bar each, a verse and then a hook
    fn heard() -> Analysis {
        let numerals: Vec<String> = ["i", "VI", "III", "VII"].iter().cycle().take(16).map(|s| s.to_string()).collect();
        Analysis {
            bpm: 96.0,
            key: Key::new(9, Mode::Minor),
            key_confidence: 0.9,
            swing: 0.5,
            pulse_clarity: 0.8,
            first_downbeat: 0.0,
            bar_secs: 2.5,
            bars: 16,
            chords: Vec::new(),
            numerals: numerals.clone(),
            progression_a: numerals[..4].to_vec(),
            progression_b: Vec::new(),
            bars_per_chord: 1,
            sections: vec![section(0, 8, 0.5, SectionKind::Verse), section(8, 8, 0.9, SectionKind::Hook)],
            bar_chroma: Vec::new(),
            kick_steps: Vec::new(),
            snare_steps: Vec::new(),
            hat_steps: Vec::new(),
            bass_style: "sustain".into(),
            steady: true,
            bar_starts: Vec::new(),
        }
    }

    #[test]
    fn the_hook_is_the_loudest_section_with_chords_and_the_verse_another() {
        let a = heard();
        let hook = hook_bars(&a).unwrap();
        assert_eq!(hook, Bars { first: 8, count: 8 });
        assert_eq!(verse_bars(&a, hook), Some(Bars { first: 0, count: 8 }));
        assert_eq!(drum_bars(&a, hook), Some(Bars { first: 8, count: 8 }));
    }

    #[test]
    fn a_piece_is_cut_from_the_loudest_bars_of_its_section_and_one_that_opens_as_loud_keeps_its_opening() {
        let a = heard();
        // the hook's section, bars 8 to 15: all but silent for four bars, then loud
        let mut x = Stereo::zeros((40.0 * SRF) as usize);
        let set = |x: &mut Stereo, from: f64, to: f64, v: f32| {
            for i in (from * 2.5 * SRF) as usize..(to * 2.5 * SRF) as usize {
                x.l[i] = v;
                x.r[i] = v;
            }
        };
        set(&mut x, 8.0, 12.0, 0.001);
        set(&mut x, 12.0, 16.0, 0.5);
        let hook = Bars { first: 8, count: 4 };
        assert_eq!(loudest(&a, &x, hook), Bars { first: 12, count: 4 });
        // louder later by less than a decibel: the section's own opening
        set(&mut x, 8.0, 12.0, 0.47);
        assert_eq!(loudest(&a, &x, hook), hook);
        // bars that start no section stay where they are
        assert_eq!(loudest(&a, &x, Bars { first: 10, count: 4 }), Bars { first: 10, count: 4 });
    }

    #[test]
    fn the_flip_plays_each_half_beat_from_another_bar_on_the_hooks_chord() {
        let a = heard();
        let hook = hook_bars(&a).unwrap();
        let x = Stereo::zeros((40.0 * SRF) as usize);
        let (out, from) = flip(&a, &x, hook, &mut Rng::new(5));
        assert_eq!(out.len(), (8.0 * 2.5 * SRF) as usize);
        assert_eq!(from.len(), 64);
        for (i, &k) in from.iter().enumerate() {
            assert_eq!(a.numerals[k], a.numerals[hook.first + i / 8], "half beat {i} from bar {k}");
            assert!(!hook.holds(k), "half beat {i} from the hook's own bar {k}");
        }
        let mut bars = from.clone();
        bars.sort_unstable();
        bars.dedup();
        assert!(bars.len() >= 4, "from {bars:?}: pieces from all over the recording");
    }

    /// a tone, as loud as `amp` gives each bar of `heard()` (2.5 s each)
    fn tone_in_bars(amp: impl Fn(usize) -> f32) -> Stereo {
        let mut x = Stereo::zeros((40.0 * SRF) as usize);
        for i in 0..x.len() {
            let t = i as f64 / SRF;
            let v = amp((t / 2.5) as usize) * (std::f64::consts::TAU * 220.0 * t).sin() as f32;
            (x.l[i], x.r[i]) = (v, v);
        }
        x
    }

    #[test]
    fn a_flips_pieces_from_a_quiet_verse_play_as_loud_as_its_hook() {
        let a = heard();
        let hook = hook_bars(&a).unwrap();
        // the verse 15 dB under the hook
        let x = tone_in_bars(|b| if b < 8 { 0.089 } else { 0.5 });
        let (out, from) = flip(&a, &x, hook, &mut Rng::new(5));
        assert!(from.iter().any(|&k| k < 8), "a piece of the verse among {from:?}");
        let slot = (2.5 / 8.0 * SRF) as usize;
        for (i, &k) in from.iter().enumerate() {
            // the middle of each half beat, past its fades
            let p = &out.l[i * slot + slot / 4..i * slot + 3 * slot / 4];
            let rms = (p.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / p.len() as f64).sqrt();
            let at = 20.0 * (rms / (0.5 / 2f64.sqrt())).log10();
            assert!(at.abs() < 1.0, "half beat {i} from bar {k}: {at:+.1} dB against the hook");
        }
    }

    #[test]
    fn a_reworks_quiet_section_comes_halfway_up_to_its_loud_one() {
        let a = heard();
        // the verse 12 dB under the hook
        let x = tone_in_bars(|b| if b < 8 { 0.1 } else { 0.4 });
        let out = sections_levelled(&x, &a);
        let (verse, hook) = (db(power_of(&a, &out, 0, 8)), db(power_of(&a, &out, 8, 8)));
        assert!((hook - verse - 6.0).abs() < 0.5, "the verse {:.1} dB under the hook", hook - verse);
        assert!((hook - db(power_of(&a, &x, 8, 8))).abs() < 0.01, "the loudest stays where it was");
    }

    /// a recording: `i VI III VII` in A minor at 96 BPM, a bar each, its chords as sine triads, a kick on one and three
    /// and a noise snare on two and four, quiet for eight bars and loud for eight more with a melody over them
    fn recording() -> Stereo {
        let s = recording_apart();
        let mut x = s.music();
        x.add_at(&s.drums, 0, 1.0, 1.0);
        x
    }

    /// `recording`'s stems: its melody the vocals, its chords the rest, and no bass
    fn recording_apart() -> Stems {
        let bar = 2.5;
        let n = (16.0 * bar * SRF) as usize;
        let (mut chords, mut melody, mut drums) = (Stereo::zeros(n), Stereo::zeros(n), Stereo::zeros(n));
        let roots = [57.0, 53.0, 48.0, 55.0];
        let thirds = [3.0, 4.0, 4.0, 4.0];
        let mut noise = Rng::new(9);
        for i in 0..n {
            let t = i as f64 / SRF;
            let b = (t / bar) as usize;
            let (root, third) = (roots[b % 4], thirds[b % 4]);
            let loud = if b >= 8 { 1.0 } else { 0.45 };
            let tone = |m: f64| (std::f64::consts::TAU * 440.0 * 2f64.powf((m - 69.0) / 12.0) * t).sin();
            let c = 0.12 * loud * (tone(root) + tone(root + third) + tone(root + 7.0));
            let m = if b >= 8 {
                0.15 * tone(root + 12.0 + [0.0, 7.0, 3.0, 5.0][(t / (bar / 4.0)) as usize % 4])
            } else {
                0.0
            };
            let beat = t / (bar / 4.0);
            let (k, into) = (beat as usize % 4, (beat.fract()) * bar / 4.0);
            let mut d = 0.0;
            if (k == 0 || k == 2) && into < 0.12 {
                d += 0.6 * (std::f64::consts::TAU * 55.0 * into).sin() * (1.0 - into / 0.12);
            }
            if (k == 1 || k == 3) && into < 0.08 {
                d += 0.3 * loud * noise.range(-1.0, 1.0) * (1.0 - into / 0.08);
            }
            for (x, v) in [(&mut chords, c), (&mut melody, m), (&mut drums, d)] {
                x.l[i] = v as f32;
                x.r[i] = v as f32;
            }
        }
        Stems { vocals: melody, drums, bass: Stereo::zeros(n), other: chords }
    }

    /// forty bars at 96 BPM in A minor: a verse, a hook, another verse and another hook, and an outro, eight bars each,
    /// each on a progression of its own
    fn heard_through() -> Analysis {
        let progs =
            [["i", "VI", "III", "VII"], ["iv", "v", "i", "i"], ["VI", "VII", "i", "v"], ["iv", "VI", "VII", "i"]];
        let numerals: Vec<String> =
            progs.iter().chain([&["i"; 4]]).flat_map(|p| p.iter().cycle().take(8)).map(|s| s.to_string()).collect();
        let kinds = [(0.5, SectionKind::Verse), (0.9, SectionKind::Hook), (0.55, SectionKind::Verse)];
        let kinds = kinds.into_iter().chain([(0.85, SectionKind::Hook), (0.3, SectionKind::Outro)]);
        Analysis {
            bars: 40,
            numerals,
            sections: kinds.enumerate().map(|(i, (e, k))| section(i * 8, 8, e, k)).collect(),
            ..heard()
        }
    }

    /// a hundred seconds of `heard_through`, its hooks louder than the rest
    fn played_through() -> Stereo {
        let mut x = Stereo::zeros((100.0 * SRF) as usize);
        for i in 0..x.len() {
            let t = i as f64 / SRF;
            let loud = if matches!((t / 2.5) as usize / 8, 1 | 3) { 0.5 } else { 0.3 };
            let v = loud * (std::f64::consts::TAU * 220.0 * t).sin() as f32;
            (x.l[i], x.r[i]) = (v, v);
        }
        x
    }

    #[test]
    fn a_flip_in_double_time_plays_as_many_of_the_recordings_bars_as_its_sections_hold() {
        // verses of sixteen bars on end, half of which a section of sixteen of the song's bars holds in double time
        let kinds = [(0.5, SectionKind::Verse, 16), (0.9, SectionKind::Hook, 8), (0.55, SectionKind::Verse, 16)];
        let kinds = kinds.into_iter().chain([(0.85, SectionKind::Hook, 8), (0.3, SectionKind::Outro, 8)]);
        let mut start = 0;
        let sections = kinds
            .map(|(e, k, n)| {
                start += n;
                section(start - n, n, e, k)
            })
            .collect();
        let progs = [["i", "VI", "III", "VII"], ["iv", "v", "i", "i"]];
        let numerals = (0..56).map(|b| progs[b / 8 % 2][b % 4].to_string()).collect();
        let a = Analysis { bars: 56, numerals, sections, ..heard() };
        let mut x = Stereo::zeros((56.0 * a.bar_secs * SRF) as usize);
        for i in 0..x.len() {
            let t = i as f64 / SRF;
            let bar = (t / a.bar_secs) as usize;
            let loud = if (16..24).contains(&bar) || (40..48).contains(&bar) { 0.5 } else { 0.3 };
            let v = loud * (std::f64::consts::TAU * 220.0 * t).sin() as f32;
            (x.l[i], x.r[i]) = (v, v);
        }
        let dir = std::env::temp_dir().join(format!("claudius-flip-double-{}", std::process::id()));
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        // drum and bass counts the recording's 96 in double time: two of the song's bars to each of its own
        let bp = remix(&src, &crate::genres::get("dnb").unwrap(), 3, &Options::default(), None, &dir, None).unwrap();
        let hook = hook_bars(&a).unwrap();
        let counted = bp.loops.iter().find(|l| l.name == PHRASE).map(|l| match &l.sound {
            Sound::Loop { beats, .. } => *beats / (hook.count as f64 * 4.0),
            _ => 0.0,
        });
        assert_eq!(counted, Some(2.0), "the hook in double time");
        // the tracks the sections that go on through it play: sample3, sample4 ...
        let on = |name: &str| name.strip_prefix(PHRASE).and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| n >= 3);
        let mut went = 0;
        for s in &bp.form {
            for l in bp.loops.iter().filter(|l| s.layers.contains(&l.name) && on(&l.name)) {
                let Sound::Loop { beats, .. } = &l.sound else { continue };
                assert!(
                    *beats <= s.bars as f64 * 4.0 + 1e-6,
                    "{} plays {beats} beats in {} bars of {}",
                    l.name,
                    s.bars,
                    s.name
                );
                went += 1;
            }
        }
        assert!(went > 0, "{:?}", bp.loops.iter().map(|l| &l.name).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_flip_goes_on_through_the_recording_rather_than_round_its_hook_and_one_passage() {
        let (a, x) = (heard_through(), played_through());
        let dir = std::env::temp_dir().join(format!("claudius-flip-through-{}", std::process::id()));
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let bp =
            remix(&src, &crate::genres::get("boom_bap").unwrap(), 3, &Options::default(), None, &dir, None).unwrap();
        let (hook, verse) = (built_on(&bp, "its hook ("), built_on(&bp, "another passage of it ("));
        // the hook and another passage, eight bars each; then the song's second verse and its second hook each play
        // eight bars neither of them is, of their own kind, each on its own chords; then round again
        let first = |name: &str| match bp.loops.iter().find(|l| l.name == name).map(|l| &l.sound) {
            Some(Sound::Loop { label, .. }) => bars_said(label),
            _ => panic!("no {name} in {:?}", bp.loops.iter().map(|l| &l.name).collect::<Vec<_>>()),
        };
        let (on, then) = (first("sample3"), first("sample4"));
        for b in [on, then] {
            assert!(b != hook && b != verse, "bars {} again: the hook {hook}, the other {verse}", b + 1);
        }
        assert!(!matches!(on / 8, 1 | 3) && matches!(then / 8, 1 | 3), "a verse's bars, then a hook's: {on}, {then}");
        let plays = |name: &str| bp.form.iter().find(|s| s.name == name).map(|s| (s.layers.clone(), s.prog.clone()));
        let (verse2, hook2) = (plays("verse2").unwrap(), plays("hook2").unwrap());
        assert!(verse2.0.contains(&"sample3".to_string()) && verse2.1 == "A3", "{verse2:?}");
        assert!(hook2.0.contains(&"sample4".to_string()) && hook2.1 == "A4", "{hook2:?}");
        let counted = |first: usize| -> Vec<String> {
            let own = &a.numerals[first..first + 8];
            match bp.harmony.progressions["A"].len() {
                4 => own.iter().step_by(2).cloned().collect(),
                _ => own.to_vec(),
            }
        };
        assert_eq!((&bp.harmony.progressions["A3"], &bp.harmony.progressions["A4"]), (&counted(on), &counted(then)));
        let again = plays("hook3").unwrap();
        assert!(again.0.contains(&PHRASE.to_string()) && again.1 == "A", "{again:?}");
        let said = format!(
            "on through it as the song goes (verse2 bars {}-{}, hook2 bars {}-{})",
            on + 1,
            on + 8,
            then + 1,
            then + 8
        );
        assert!(bp.lineage.iter().any(|l| l.contains(&said)), "{:?}", bp.lineage);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// the first bar of "bars 17-24" in `said`, counting from 0
    fn bars_said(said: &str) -> usize {
        let at = said.find("bars ").expect("bars said") + 5;
        said[at..].split('-').next().and_then(|n| n.parse::<usize>().ok()).expect("a bar") - 1
    }

    /// the first bar of what the remix `bp` says it is built on after `what`
    fn built_on(bp: &Blueprint, what: &str) -> usize {
        let line = bp.lineage.iter().find(|l| l.starts_with("remixed from")).expect("a remix");
        bars_said(&line[line.find(what).unwrap_or_else(|| panic!("{what} in {line}"))..])
    }

    #[test]
    fn a_flip_made_again_is_built_on_other_pieces_of_the_recording_the_best_most_often() {
        let a = heard_through();
        let hook = hook_bars(&a).unwrap();
        // the hooks are bars 9-16 (the louder) and 25-32, the verses bars 1-8 (the quieter) and 17-24: the other hook
        // is no verse, and the outro neither
        let (mut hooks_drawn, mut verses_drawn) = (BTreeMap::new(), BTreeMap::new());
        for seed in 0..200 {
            let mut rng = Rng::new(seed);
            *hooks_drawn.entry(draw(hooks(&a), &mut rng).unwrap().first).or_insert(0) += 1;
            *verses_drawn.entry(draw(verses(&a, hook), &mut rng).unwrap().first).or_insert(0) += 1;
        }
        assert_eq!(hooks_drawn.keys().copied().collect::<Vec<_>>(), [8, 24], "{hooks_drawn:?}");
        assert_eq!(verses_drawn.keys().copied().collect::<Vec<_>>(), [0, 16], "{verses_drawn:?}");
        assert!(hooks_drawn[&8] > hooks_drawn[&24], "the louder hook most often: {hooks_drawn:?}");
        assert!(verses_drawn[&0] > verses_drawn[&16], "the quieter verse most often: {verses_drawn:?}");
        // and a remix made again with another seed is built on them
        let x = played_through();
        let dir = std::env::temp_dir().join(format!("claudius-flip-again-{}", std::process::id()));
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let g = crate::genres::get("boom_bap").unwrap();
        let drawn = |seed: u64| draw(hooks(&a), &mut Rng::seed_from(seed, "remix")).unwrap().first;
        let (one, other) = (0..).find(|&s| drawn(s) == 8).zip((0..).find(|&s| drawn(s) == 24)).unwrap();
        for (seed, hook) in [(one, 8), (other, 24)] {
            let bp = remix(&src, &g, seed, &Options::default(), None, &dir, None).unwrap();
            assert_eq!(built_on(&bp, "its hook ("), hook, "seed {seed}: {:?}", bp.lineage);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_chord_no_other_bar_plays_gives_way_to_its_seventh_but_never_to_another_chord() {
        // the hook, bars 9 to 16, all A minor; before it C major, but for bar 6, A minor's seventh
        let (am, am7, c) = (
            [0.33, 0., 0., 0., 0.33, 0., 0., 0., 0., 0.34, 0., 0.],
            [0.25, 0., 0., 0., 0.25, 0., 0., 0.25, 0., 0.25, 0., 0.],
            [0.33, 0., 0., 0., 0.33, 0., 0., 0.34, 0., 0., 0., 0.],
        );
        let notes = |seventh: bool| -> Vec<[f32; 12]> {
            (0..16)
                .map(|k| {
                    if k >= 8 {
                        am
                    } else if k == 5 && seventh {
                        am7
                    } else {
                        c
                    }
                })
                .collect()
        };
        let numerals = |seventh: bool| -> Vec<String> {
            (0..16)
                .map(|k| {
                    if k >= 8 {
                        "i"
                    } else if k == 5 && seventh {
                        "i7"
                    } else {
                        "III"
                    }
                    .to_string()
                })
                .collect()
        };
        let x = Stereo::zeros((40.0 * SRF) as usize);
        let a = Analysis { numerals: numerals(true), bar_chroma: notes(true), ..heard() };
        let hook = hook_bars(&a).unwrap();
        let (_, from) = flip(&a, &x, hook, &mut Rng::new(5));
        assert!(from.iter().all(|&k| k == 5), "the seventh, and no bar of C major: {from:?}");
        // with no seventh to give way to, the hook's own bars
        let a = Analysis { numerals: numerals(false), bar_chroma: notes(false), ..heard() };
        let (_, from) = flip(&a, &x, hook, &mut Rng::new(5));
        assert!(from.iter().all(|&k| hook.holds(k)), "{from:?}");
    }

    #[test]
    fn where_no_chord_is_heard_the_flip_plays_bars_whose_notes_are_nearest_the_hooks() {
        // no chord heard; the notes of A minor in every bar, but those of bars 2 and 3, which are C major's
        let am = |k: usize| {
            if matches!(k, 2 | 3) {
                [0.4, 0., 0., 0., 0.3, 0., 0., 0.3, 0., 0., 0., 0.]
            } else {
                [0.3, 0., 0., 0., 0.3, 0., 0., 0., 0., 0.4, 0., 0.]
            }
        };
        let a = Analysis { numerals: Vec::new(), bar_chroma: (0..16).map(am).collect(), ..heard() };
        let hook = hook_bars(&a).unwrap();
        let (_, from) = flip(&a, &Stereo::zeros((40.0 * SRF) as usize), hook, &mut Rng::new(5));
        assert!(from.iter().all(|&k| !hook.holds(k) && !matches!(k, 2 | 3)), "{from:?}");
        let mut bars = from.clone();
        bars.sort_unstable();
        bars.dedup();
        assert!(bars.len() >= 3, "from {bars:?}");
    }

    #[test]
    fn a_remake_remixes_the_recording_on_its_hook() {
        let x = recording();
        let a = crate::analysis::song::analyze(&x, None);
        let dir = std::env::temp_dir().join(format!("claudius-remix-{}", std::process::id()));
        let g = crate::genres::get("boom_bap").unwrap();
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let bp = remix(&src, &g, 3, &Options::default(), None, &dir, None).unwrap();
        let phrase = bp.loops.iter().find(|l| l.name == PHRASE).expect("the hook plays");
        let Sound::Loop { path, label, .. } = &phrase.sound else { panic!("a loop") };
        assert!(Path::new(path).starts_with(dir.join("my-song")), "{path}");
        assert_eq!(label, "my-song");
        assert!(bp.lineage.iter().any(|l| l.starts_with("remixed from 'my song.wav': its hook")), "{:?}", bp.lineage);
        // a tempo boom bap plays, the recording's own or a multiple of it, sped or chopped as a sampler plays it
        assert!((g.bpm.0..=g.bpm.1).contains(&bp.bpm), "{}", bp.bpm);
        let flip = bp.loops.iter().find(|l| l.name == FLIP).expect("a flip");
        assert_eq!((flip.transpose, flip.retime), (phrase.transpose, phrase.retime), "the flip plays as the hook does");
        assert!(bp.form.iter().filter(|s| s.layers.iter().any(|l| l == FLIP)).all(|s| s.prog == "A"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rework_plays_the_whole_recording_in_its_own_sections_on_its_chords() {
        let x = recording();
        let a = crate::analysis::song::analyze(&x, None);
        let dir = std::env::temp_dir().join(format!("claudius-rework-{}", std::process::id()));
        let g = crate::genres::get("boom_bap").unwrap();
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let bp = rework(&src, &g, 3, &Options::default(), None, &dir).unwrap();
        // at its own tempo, which boom bap plays: the recording at its own speed and pitch
        assert!((bp.bpm - 240.0 / a.bar_secs).abs() < 1e-9 && (bp.bpm - 96.0).abs() < 0.5, "{}", bp.bpm);
        // its sections, bar for bar, each on its own bars' chords, a bar each
        assert_eq!(bp.form.len(), a.sections.len());
        assert_eq!(bp.harmony.bars_per_chord, 1);
        for (s, heard) in bp.form.iter().zip(&a.sections) {
            assert_eq!((s.kind, s.bars as usize), (heard.kind, heard.bars));
            let chords = &bp.harmony.progressions[&s.prog];
            assert_eq!(chords.len(), heard.bars, "{}", s.name);
            assert_eq!(chords[0], a.numerals[heard.start_bar], "{}", s.name);
        }
        // the genre's drums and bass under it, none of its chords, keys or leads over it
        assert!(bp.form.iter().any(|s| s.layers.iter().any(|l| l == "kick")), "{:?}", bp.form);
        assert!(bp.parts.iter().any(|p| p.role == crate::model::Role::Bass));
        assert!(bp.parts.iter().all(|p| p.role == crate::model::Role::Bass), "{:?}", bp.parts);
        assert!(bp.loops.iter().all(|l| [ORIGINAL, BREAK, crate::sampling::TEXTURE].contains(&l.name.as_str())));
        assert!(
            bp.lineage.iter().any(|l| l.starts_with("reworked from 'my song.wav': all 16 bars")),
            "{:?}",
            bp.lineage
        );
        // the recording plays once, all through: one take from the first bar to the last
        let bars: f64 = bp.form.iter().map(|s| s.bars as f64).sum();
        let p = compose::arrange(&bp);
        let t = p.tracks.iter().find(|t| t.name == ORIGINAL).expect("the recording plays");
        assert_eq!(t.clips.len(), 1, "{:?}", t.clips);
        assert_eq!((t.clips[0].t, t.clips[0].offset, t.clips[0].d), (0.0, 0.0, bars * 4.0));
        let Sound::Loop { path, bpm, beats, .. } = &t.sound else { panic!("a loop") };
        assert!(Path::new(path).starts_with(dir.join("my-song")), "{path}");
        assert!((bpm - bp.bpm).abs() < 1e-9 && (beats - bars * 4.0).abs() < 1e-6, "{bpm} {beats}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// the mean power of `x`'s left channel from `from` to `to` seconds
    fn power(x: &Stereo, from: f64, to: f64) -> f64 {
        let (a, b) = ((from * SRF) as usize, ((to * SRF) as usize).min(x.len()));
        x.l[a..b].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (b - a).max(1) as f64
    }

    #[test]
    fn a_rework_with_its_stems_apart_plays_its_voice_and_the_rest_on_tracks_of_their_own() {
        let (x, s) = (recording(), recording_apart());
        let a = crate::analysis::song::analyze_with(&x, Some(&s), None);
        let dir = std::env::temp_dir().join(format!("claudius-rework-stems-{}", std::process::id()));
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: Some(&s) };
        let bp = rework(&src, &crate::genres::get("boom_bap").unwrap(), 3, &Options::default(), None, &dir).unwrap();
        // each all through from its first bar, in every section
        let p = compose::arrange(&bp);
        let heard = |name: &str| {
            let lp = bp.loops.iter().find(|l| l.name == name).unwrap_or_else(|| panic!("{name} plays"));
            let t = p.tracks.iter().find(|t| t.name == name).unwrap();
            assert_eq!(t.clips.len(), 1, "{name}: {:?}", t.clips);
            assert_eq!((t.clips[0].t, t.clips[0].offset), (0.0, 0.0), "{name}");
            let Sound::Loop { path, .. } = &lp.sound else { panic!("a loop") };
            (lp.role.clone(), crate::audio::load(Path::new(path)).unwrap())
        };
        let ((rest_role, rest), (voice_role, voice)) = (heard(ORIGINAL), heard(VOICE));
        assert_eq!((rest_role.as_str(), voice_role.as_str()), ("sample", "vocal"));
        for s in &bp.form {
            assert!([ORIGINAL, VOICE].iter().all(|t| s.layers.iter().any(|l| l == t)), "{}: {:?}", s.name, s.layers);
        }
        // the melody comes in at the ninth bar: the voice is silent before it, and the rest, its chords, is not
        let bar = a.bar_secs;
        assert!(power(&voice, bar, 7.0 * bar) < 1e-4 * power(&voice, 9.0 * bar, 15.0 * bar));
        assert!(power(&rest, bar, 7.0 * bar) > 0.05 * power(&rest, 9.0 * bar, 15.0 * bar));
        let said = "its voice on a track of its own (vocals), without its drums and its bass";
        assert!(bp.lineage.iter().any(|l| l.contains(said)), "{:?}", bp.lineage);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_flip_with_its_stems_apart_is_built_on_all_but_its_drums_and_its_voice_sings_on_in_it() {
        let (x, s) = (recording(), recording_apart());
        let a = crate::analysis::song::analyze_with(&x, Some(&s), None);
        let dir = std::env::temp_dir().join(format!("claudius-flip-stems-{}", std::process::id()));
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: Some(&s) };
        let bp =
            remix(&src, &crate::genres::get("boom_bap").unwrap(), 3, &Options::default(), None, &dir, None).unwrap();
        // the hook as its stems without the drums, its melody, which sings, with its chords
        let hook = loudest(&a, &s.music(), hook_bars(&a).unwrap());
        let phrase = bp.loops.iter().find(|l| l.name == PHRASE).expect("the hook plays");
        let Sound::Loop { path, .. } = &phrase.sound else { panic!("a loop") };
        let got = crate::audio::load(Path::new(path)).unwrap();
        let off = |x: &Stereo| got.l.iter().zip(&x.l).map(|(a, b)| ((a - b) as f64).powi(2)).sum::<f64>();
        let (with_voice, backing) = (off(&cut(&s.music(), &a, hook)), off(&cut(&s.backing(), &a, hook)));
        assert!(with_voice < 1e-4 * backing, "{with_voice} against {backing}");
        let said = format!("its hook ({}) without its drums, in its own key for the voice in it", hook.said());
        assert!(bp.lineage.iter().any(|l| l.contains(&said)), "{:?}", bp.lineage);
        // its voice sings on where it sang, rather than in a phrase or two of it on tracks of their own
        assert!(!bp.loops.iter().any(|l| l.role == "vocal"), "{:?}", bp.loops);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hook_a_voice_sings_in_plays_in_its_own_key_unless_the_genre_speeds_its_singers_up() {
        let need = |g: &str| crate::sampling::need(&crate::genres::get(g).unwrap(), Role::Phrase).unwrap().clone();
        let (house, chipmunk) = (need("house"), need("chipmunk_soul"));
        // house speeds a record of 96 up four semitones into its 120 to 126, but not one a voice sings in
        assert!(house.pitch.1 >= 4 && !house.speeds_voices);
        assert!(own_key(&house, true, 96.0, 0.8, (120.0, 126.0)));
        assert!(!own_key(&house, false, 96.0, 0.8, (120.0, 126.0)), "a hook no voice sings in");
        assert!(!own_key(&chipmunk, true, 96.0, 0.8, (80.0, 96.0)), "chipmunk soul speeds its singers up");
        // remixed into house without its stems apart, a recording whose melody, a voice's, sings out over its chords
        // keeps its hook in its own key; one whose melody no one hears over them is sped up, as house plays a record
        let s = recording_apart();
        let mut sung = s.other.clone();
        sung.add_at(&s.vocals, 0, 4.0, 4.0);
        sung.add_at(&s.drums, 0, 1.0, 1.0);
        let dir = std::env::temp_dir().join(format!("claudius-flip-key-{}", std::process::id()));
        let g = crate::genres::get("house").unwrap();
        let transposed = |x: &Stereo| -> Vec<i32> {
            let a = crate::analysis::song::analyze(x, None);
            let src = Source { x, name: "my song.wav", a: &a, stems: None };
            (1..3)
                .map(|seed| {
                    let bp = remix(&src, &g, seed, &Options::default(), None, &dir, None).unwrap();
                    let kept = bp.lineage.iter().any(|l| l.contains("in its own key for the voice in it"));
                    let semis = bp.loops.iter().find(|l| l.name == PHRASE).expect("the hook plays").transpose;
                    assert_eq!(kept, semis == 0 && bp.key == a.key, "seed {seed}: {:?}", bp.lineage);
                    semis
                })
                .collect()
        };
        assert_eq!(transposed(&sung), [0, 0]);
        assert!(transposed(&recording()).iter().any(|&s| s != 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progressions_past_z_take_two_letters_and_none_is_named_twice() {
        let names: Vec<String> = (0..800).map(progression_name).collect();
        assert_eq!(
            (names[0].as_str(), names[25].as_str(), names[26].as_str(), names[27].as_str()),
            ("A", "Z", "AA", "AB")
        );
        assert_eq!((names[51].as_str(), names[701].as_str(), names[702].as_str()), ("AZ", "ZZ", "AAA"));
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
    }

    /// the recording's loop in a rework: the tempo it declares and the song's beats it lasts
    fn original(bp: &Blueprint) -> (f64, f64) {
        let lp = bp.loops.iter().find(|l| l.name == ORIGINAL).expect("the recording plays");
        let Sound::Loop { bpm, beats, .. } = &lp.sound else { panic!("a loop") };
        (*bpm, *beats)
    }

    #[test]
    fn a_rework_in_double_time_plays_two_bars_to_each_of_the_recordings_and_one_asked_for_stretches_it() {
        let x = recording();
        let a = crate::analysis::song::analyze(&x, None);
        let dir = std::env::temp_dir().join(format!("claudius-rework-tempo-{}", std::process::id()));
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let own = 240.0 / a.bar_secs;
        // drum and bass plays twice the recording's 96: two of its bars to each of the recording's, a chord each
        let dnb = crate::genres::get("dnb").unwrap();
        let bp = rework(&src, &dnb, 3, &Options::default(), None, &dir).unwrap();
        assert!((bp.bpm - 2.0 * own).abs() < 1e-9, "{}", bp.bpm);
        assert_eq!(bp.harmony.bars_per_chord, 2);
        for (s, heard) in bp.form.iter().zip(&a.sections) {
            assert_eq!(s.bars as usize, 2 * heard.bars, "{}", s.name);
            assert_eq!(bp.harmony.progressions[&s.prog].len(), heard.bars, "{}", s.name);
        }
        let bars: f64 = bp.form.iter().map(|s| s.bars as f64).sum();
        let (bpm, beats) = original(&bp);
        assert!((bpm - bp.bpm).abs() < 1e-9 && (beats - bars * 4.0).abs() < 1e-6, "unstretched: {bpm} {beats}");
        // a tempo asked for keeps its bars and stretches the recording to it
        let asked = Options { bpm: Some(100.0), ..Options::default() };
        let bp = rework(&src, &crate::genres::get("boom_bap").unwrap(), 3, &asked, None, &dir).unwrap();
        assert_eq!((bp.bpm, bp.harmony.bars_per_chord), (100.0, 1));
        let bars: f64 = bp.form.iter().map(|s| s.bars as f64).sum();
        let (bpm, beats) = original(&bp);
        assert!((bpm - own).abs() < 1e-9 && (beats - bars * 4.0).abs() < 1e-6, "at its own {own}: {bpm} {beats}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rework_of_a_fast_recording_that_holds_its_chords_two_bars_plays_in_half_time() {
        // sixteen bars at 170 BPM, each chord held two bars
        let mut a = heard();
        (a.bpm, a.bar_secs) = (170.0, 240.0 / 170.0);
        a.numerals =
            ["i", "i", "VI", "VI", "III", "III", "VII", "VII"].iter().cycle().take(16).map(|s| s.to_string()).collect();
        let hip_hop = crate::genres::get("hip_hop").unwrap();
        let (bpm, per) = rework_tempo(&a, &hip_hop);
        assert!((bpm - 85.0).abs() < 1e-9 && per == Per::Half, "{bpm} {per:?}");
        // a chord to each bar: the song keeps the recording's tempo, since it plays one chord a bar at most
        let every = Analysis { numerals: heard().numerals, ..a.clone() };
        assert_eq!(rework_tempo(&every, &hip_hop).1, Per::One);
        // one bar of the song's to two of the recording's, a chord each, the first of the two, and the recording as it is
        let x = Stereo::zeros((16.0 * a.bar_secs * SRF) as usize);
        let dir = std::env::temp_dir().join(format!("claudius-rework-half-{}", std::process::id()));
        let src = Source { x: &x, name: "fast.wav", a: &a, stems: None };
        let bp = rework(&src, &hip_hop, 3, &Options::default(), None, &dir).unwrap();
        assert!(
            (bp.bpm - 85.0).abs() < 1e-9 && bp.harmony.bars_per_chord == 1,
            "{} {}",
            bp.bpm,
            bp.harmony.bars_per_chord
        );
        for (s, heard) in bp.form.iter().zip(&a.sections) {
            assert_eq!(s.bars as usize, heard.bars / 2, "{}", s.name);
            assert_eq!(bp.harmony.progressions[&s.prog], ["i", "VI", "III", "VII"], "{}", s.name);
        }
        let bars: f64 = bp.form.iter().map(|s| s.bars as f64).sum();
        let (bpm, beats) = original(&bp);
        assert!((bpm - 85.0).abs() < 1e-9 && (beats - bars * 4.0).abs() < 1e-6, "unstretched: {bpm} {beats}");
        assert!(bp.lineage.iter().any(|l| l.contains("in half time, at 85 BPM to its 170")), "{:?}", bp.lineage);
        // an odd count of bars: the last section takes the half bar left over, and the song is as long as all of it
        let mut odd = a.clone();
        odd.bars = 17;
        odd.numerals.push("i".into());
        odd.sections[1].bars = 9;
        let x = Stereo::zeros((17.0 * a.bar_secs * SRF) as usize);
        let src = Source { x: &x, name: "odd.wav", a: &odd, stems: None };
        let bp = rework(&src, &hip_hop, 3, &Options::default(), None, &dir).unwrap();
        let bars: f64 = bp.form.iter().map(|s| s.bars as f64).sum();
        assert_eq!((bp.form[0].bars, bp.form[1].bars), (4, 5));
        let (_, beats) = original(&bp);
        assert!(bars * 4.0 >= beats && beats == 34.0, "{bars} bars for {beats} beats of the recording");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rework_plays_no_other_records_phrase_and_keeps_the_recordings_key() {
        let x = recording();
        let a = crate::analysis::song::analyze(&x, None);
        let dir = std::env::temp_dir().join(format!("claudius-rework-key-{}", std::process::id()));
        // a library with a phrase in C# minor, which a song of boom bap would be built on, in its key
        let phrase = Prepared {
            path: "demo:samples/phrase/boom_bap/other/phrase.wav".into(),
            role: Role::Phrase,
            genre: "boom_bap".into(),
            bpm: 92.0,
            beats: 16.0,
            key: Some(1),
            minor: true,
            kick: None,
            snare: None,
            hat: None,
            chords: vec!["i".into(), "VI".into(), "III".into(), "VII".into()],
            z: None,
            source: "another record".into(),
            asked: None,
            second: None,
            root: None,
            pulse: Some(0.8),
        };
        let lib = crate::library::Library::with_prepared(vec![phrase]);
        let g = crate::genres::get("boom_bap").unwrap();
        let built = compose::generate(&g, 3, &Options { sampled: Some(true), ..Options::default() }, Some(&lib));
        assert_eq!(built.key.tonic, 1, "a song of the genre would take the phrase's key");
        let src = Source { x: &x, name: "my song.wav", a: &a, stems: None };
        let bp = rework(&src, &g, 3, &Options::default(), Some(&lib), &dir).unwrap();
        assert_eq!(bp.key, a.key);
        assert!(bp.loops.iter().all(|l| l.name != PHRASE && l.name != SECOND), "{:?}", bp.loops);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
