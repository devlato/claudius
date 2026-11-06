//! A remake that remixes the recording, rather than writing a new song only on its skeleton: the recording cut into
//! the pieces a producer would take from it, and a song of the genre built on them, as a song built on samples is
//! (`crate::sampling`).
//!
//! - Its hook, four bars of the loudest section with chords (a hook or a drop when it has one), without its drums,
//!   is the phrase the song is built on: the song takes its key, its chords and a tempo it plays at, the recording's
//!   own or a multiple of it, faster or slower by whole semitones or chopped onto the song's grid
//!   (`sampling::meets`), and plays it in its hooks.
//! - Another passage of it as long (of its verses, when it has any), also without its drums, plays in the song's
//!   other sections.
//! - Its own drums, the busiest bars of them without the rest, are the break, in a genre that plays one.
//! - A flip of it plays in the intro, the breaks and the outro: as many bars as the hook of half beats, each from
//!   another bar of the recording that plays the hook's chord there, at the same place in its bar (now and then
//!   another), so that the song is heard in pieces from all over it, still on the hook's chords.
//! - Its sung phrases, when it sings (the ears hear a voice, or a pitch in a voice's range carries a third of a phrase),
//!   come in where they agree with the song's chords (`crate::sampler`).
//!
//! Everything else is the genre's: its drums and bass, its parts, its form and its mix. That is the flip (`remix`).
//!
//! A rework (`rework`) keeps the recording whole instead, so that it stays the song it is: it plays from its first
//! downbeat to its end without its drums and with its low end cut, its voice, chords and melodies as they are, at its
//! own speed and pitch. The song is its form bar for bar, on its chords bar by bar, in its swing, and the genre plays
//! under it: its drums, its bass on those chords' roots, its break and textures, its mix.

use std::collections::BTreeMap;
use std::path::Path;

use crate::analysis::song::{Analysis, SectionGuess};
use crate::audio::Stereo;
use crate::compose::{self, FamilyPick, Library, Options};
use crate::config::SRF;
use crate::dsp::fft;
use crate::genres::{GenreProfile, LoopSpec};
use crate::library::prepared::{Passage, Prepared, Role};
use crate::model::*;
use crate::rng::Rng;
use crate::sampling::{BREAK, PHRASE, SECOND};
use crate::theory::scale::Key;

/// the track a remix's flip plays on
pub const FLIP: &str = "flip";

/// the track a rework plays the whole recording on
pub const ORIGINAL: &str = "original";

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

/// the hook: four bars from the start of the section with chords that pulls most toward being one (two of a short
/// one)
pub fn hook_bars(a: &Analysis) -> Option<Bars> {
    let s = a
        .sections
        .iter()
        .filter(|s| s.harmony && s.bars >= 2 && s.start_bar + 2 <= a.bars)
        .max_by(|x, y| hookness(x).total_cmp(&hookness(y)))?;
    Some(Bars { first: s.start_bar, count: if s.bars >= 4 && s.start_bar + 4 <= a.bars { 4 } else { 2 } })
}

/// a passage of the verses as long as the hook, from a section with chords elsewhere (a verse when there is one, else
/// the quietest that is not an intro or an outro)
pub fn verse_bars(a: &Analysis, hook: Bars) -> Option<Bars> {
    let s = a
        .sections
        .iter()
        .filter(|s| s.harmony && s.bars >= hook.count && s.start_bar + hook.count <= a.bars && !hook.meets(s))
        .filter(|s| !matches!(s.kind, SectionKind::Intro | SectionKind::Outro))
        .min_by(|x, y| {
            let rank = |s: &SectionGuess| (s.kind != SectionKind::Verse, s.energy);
            let (rx, ry) = (rank(x), rank(y));
            rx.0.cmp(&ry.0).then(rx.1.total_cmp(&ry.1))
        })?;
    Some(Bars { first: s.start_bar, count: hook.count })
}

/// the busiest of the recording's drums, as many bars as the hook: the loudest section with a kick
pub fn drum_bars(a: &Analysis, hook: Bars) -> Option<Bars> {
    let s = a
        .sections
        .iter()
        .filter(|s| s.kick && s.bars >= hook.count && s.start_bar + hook.count <= a.bars)
        .max_by(|x, y| x.energy.total_cmp(&y.energy))?;
    Some(Bars { first: s.start_bar, count: hook.count })
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

/// where bar `bar` of the recording starts in it, in samples
fn bar_at(a: &Analysis, bar: usize) -> usize {
    ((a.first_downbeat + bar as f64 * a.bar_secs) * SRF).round().max(0.0) as usize
}

/// bars `b` of `x`, faded in and out over a few milliseconds so that they loop without a click
fn cut(x: &Stereo, a: &Analysis, b: Bars) -> Stereo {
    let mut out = x.slice(bar_at(a, b.first), bar_at(a, b.first + b.count));
    let n = out.len();
    out.fade(((0.002 * SRF) as usize).min(n / 4), ((0.004 * SRF) as usize).min(n / 4));
    out
}

/// the chord of each of bars `b`, as Roman numerals in the recording's key; none unless the analysis heard them all
fn chords_of(a: &Analysis, b: Bars) -> Vec<String> {
    let chords: Vec<String> = (b.first..b.first + b.count).filter_map(|k| a.numerals.get(k).cloned()).collect();
    if chords.len() == b.count { chords } else { Vec::new() }
}

/// The flip: as many bars as the hook of half beats, each from another bar of the recording that plays the hook's
/// chord at that place (one of a section with chords; the hook's own bar when no other does), at the same place in its
/// bar, now and then another place in it. The audio, and the bar each half beat came from.
pub fn flip(a: &Analysis, sustained: &Stereo, hook: Bars, rng: &mut Rng) -> (Stereo, Vec<usize>) {
    let half = a.bar_secs / 8.0;
    let mut out = Stereo::zeros((hook.count as f64 * a.bar_secs * SRF).round() as usize);
    let with_chords: Vec<usize> = (0..a.bars.min(a.numerals.len()))
        .filter(|&k| {
            !hook.holds(k) && a.sections.iter().any(|s| s.harmony && (s.start_bar..s.start_bar + s.bars).contains(&k))
        })
        .collect();
    let slot = (half * SRF).round() as usize;
    let (fade_in, fade_out) = ((0.002 * SRF) as usize, (0.004 * SRF) as usize);
    let mut from = Vec::new();
    for i in 0..hook.count * 8 {
        let (b, j) = (i / 8, i % 8);
        let chord = a.numerals.get(hook.first + b);
        let same: Vec<usize> = with_chords.iter().copied().filter(|&k| a.numerals.get(k) == chord).collect();
        let k = if same.is_empty() { hook.first + b } else { same[rng.below(same.len())] };
        let j = if rng.chance(0.25) { rng.below(8) } else { j };
        from.push(k);
        let src = ((a.first_downbeat + k as f64 * a.bar_secs + j as f64 * half) * SRF).round() as usize;
        let dst = (i as f64 * half * SRF).round() as usize;
        for t in 0..slot {
            let (s, d) = (src + t, dst + t);
            if s >= sustained.len() || d >= out.len() {
                break;
            }
            let edge = (t as f32 / fade_in as f32).min((slot - t) as f32 / fade_out as f32).min(1.0);
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

/// the recording a remix is made of: its audio, what it is called, and what its analysis heard in it
pub struct Source<'a> {
    pub x: &'a Stereo,
    pub name: &'a str,
    pub a: &'a Analysis,
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
    let hook = hook_bars(a).ok_or("the recording has no section with chords long enough to build a remix on")?;
    let (sustained, struck) = split(x);
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
    let verse = verse_bars(a, hook);
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
    let drum_bars = crate::sampling::need(g, Role::Break).and_then(|_| drum_bars(a, hook));
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
    // the hook is what the remix is built on: it plays, whatever the genre's chance for a phrase
    let mut built = g.clone();
    if let Some(n) = built.sampling.as_mut().and_then(|s| s.phrase.as_mut()) {
        n.chance = 1.0;
    }
    let library = Recording { phrase: &phrase, drums: drums.as_ref(), inner: lib };
    let o = Options { sampled: Some(true), sample_search: None, family: false, ..opts.clone() };
    let mut bp = compose::generate(&built, seed, &o, Some(&library));
    if !bp.loops.iter().any(|l| l.name == PHRASE) {
        return Err(format!("{} could not play the recording's hook", g.name.replace('_', " ")));
    }
    let mut said = vec![format!("its hook ({}) without its drums", hook.said())];
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
    // its sung phrases, when it sings: a voice the ears hear, or a pitch followed through a third of a phrase
    let voiced = crate::sampler::Options::default();
    let sings = crate::sampler::find(x, a, ears, &voiced, 1)
        .first()
        .is_some_and(|p| p.sung >= 0.35 || p.z.is_some_and(|z| z >= 1.5));
    if sings && let Ok(sung) = crate::sampler::sample(&mut bp, x, name, ears, &voiced, dir) {
        said.push(format!("its sung phrases ({})", sung.tracks.join(", ")));
    }
    let line = format!("remixed from '{name}': {}", said.join("; "));
    bp.lineage.insert(1.min(bp.lineage.len()), line);
    Ok(bp)
}

/// The tempo a rework plays at, and the song's bars to each of the recording's: its own tempo (from the length of its
/// bars), or double it (two bars to each of its own) when that is nearer the genre's range, so that it plays at its
/// own speed and pitch on the song's grid. Never half: two of its bars to one of the song's would put two of its chords
/// in a bar of the song's, which plays one chord a bar at most.
pub fn rework_tempo(a: &Analysis, g: &GenreProfile) -> (f64, usize) {
    let own = 240.0 / a.bar_secs.max(1e-3);
    let (lo, hi) = g.bpm;
    let outside = |b: f64| (lo - b).max(b - hi).max(0.0);
    if outside(own * 2.0) < outside(own) { (own * 2.0, 2) } else { (own, 1) }
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
fn progression_name(i: usize) -> String {
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

/// The recording's sections as the song's, `per` bars of the song's to each of its own: each its own kind, length and
/// energy, on a progression of its own bars' chords (a chord to each of its bars; one name for each run of chords, `A`
/// the first), with the layers the genre plays in a section like it that `keep` takes, and the progressions
fn rework_form(
    a: &Analysis,
    per: usize,
    genre: &[SectionSpec],
    keep: impl Fn(&str) -> bool,
) -> (Vec<SectionSpec>, BTreeMap<String, Vec<String>>) {
    let mut form = Vec::new();
    let mut progs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut named: Vec<(Vec<String>, String)> = Vec::new();
    let mut last = a.numerals.iter().find(|c| !c.is_empty()).cloned().unwrap_or_else(|| "i".into());
    for (i, s) in a.sections.iter().enumerate() {
        let chords: Vec<String> = (s.start_bar..s.start_bar + s.bars)
            .map(|b| {
                if let Some(c) = a.numerals.get(b).filter(|c| !c.is_empty()) {
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
        layers.push(ORIGINAL.into());
        form.push(SectionSpec {
            name,
            kind: s.kind,
            bars: (s.bars * per) as u32,
            energy: s.energy,
            prog,
            layers,
            variant: t.map_or_else(|| "main".into(), |t| t.variant.clone()),
            filter_in: false,
            build: false,
            gap: false,
            impact: t.is_some_and(|t| t.impact),
            fills: t.is_some_and(|t| t.fills),
        });
    }
    (form, progs)
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
    let (x, name, a) = (src.x, src.name, src.a);
    if a.bars < 8 || a.sections.is_empty() {
        return Err("the recording is too short, or its sections too unclear, to rework".into());
    }
    // a tempo asked for stretches the recording to it, bar for bar
    let (bpm, per) = opts.bpm.map_or_else(|| rework_tempo(a, g), |b| (b, 1));
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
    let (form, progressions) = rework_form(a, per, &bp.form, keep);
    bp.form = form;
    bp.harmony.progressions = progressions;
    bp.harmony.bars_per_chord = per as u32;
    bp.swing = a.swing;
    let used: Vec<String> = bp.form.iter().flat_map(|s| s.layers.iter().cloned()).collect();
    bp.parts.retain(|p| used.contains(&p.name));
    bp.loops.retain(|l| used.contains(&l.name));
    // the recording, all of it, without its drums, and under the genre's bass and kick without its own low end: the
    // genre's treatment of a phrase it samples, high-passed at 150 Hz at least
    let (sustained, _) = split(x);
    let slug = crate::song::slug(Path::new(name).file_stem().map_or(name.into(), |s| s.to_string_lossy()).as_ref());
    let body = cut(&sustained, a, Bars { first: 0, count: a.bars });
    let path = crate::take::store(&body, &dir.join(&slug)).map_err(|e| format!("{e:#}"))?;
    let mut mix = crate::sampling::need(g, Role::Phrase).map(|n| n.mix.clone()).unwrap_or_default();
    mix.fx.retain(|e| !matches!(e, crate::fx::Effect::Filter { mode: crate::dsp::filter::FilterMode::Hp, .. }));
    let hp = crate::fx::Effect::Filter {
        mode: crate::dsp::filter::FilterMode::Hp,
        cutoff: crate::dsp::env::Param::Const(150.0),
        res: crate::dsp::env::Param::Const(0.05),
        drive: 1.0,
        mix: crate::dsp::env::Param::Const(1.0),
    };
    mix.fx.insert(0, hp);
    if mix.bus.is_empty() {
        mix.bus = "music".into();
    }
    // at its own tempo on the song's grid, `per` beats of the song's to each of its own: as long as the song's bars, or
    // stretched to a tempo asked for
    let own = 240.0 / a.bar_secs.max(1e-3) * per as f64;
    bp.loops.push(LoopPart {
        name: ORIGINAL.into(),
        role: "sample".into(),
        sound: Sound::Loop {
            path: path.to_string_lossy().into_owned(),
            bpm: own,
            beats: (a.bars * 4 * per) as f64,
            key: None,
            tonal: false,
            label: format!("{slug}, all of it"),
        },
        transpose: 0,
        chop: Vec::new(),
        slice_beats: 0.0,
        retime: false,
        mix,
        from: bp.form.first().map(|s| s.name.clone()),
        sampled: true,
    });
    let kept: Vec<&str> =
        bp.parts.iter().map(|p| p.name.as_str()).chain(bp.drums.iter().map(|d| d.name.as_str())).collect();
    let line = format!(
        "reworked from '{name}': all {} bars of it, its voice and its chords as they are, without its drums and its \
         low end, in its own {} sections and on its chords; {}'s {} under it",
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
        }
    }

    #[test]
    fn the_hook_is_the_loudest_section_with_chords_and_the_verse_another() {
        let a = heard();
        let hook = hook_bars(&a).unwrap();
        assert_eq!(hook, Bars { first: 8, count: 4 });
        assert_eq!(verse_bars(&a, hook), Some(Bars { first: 0, count: 4 }));
        assert_eq!(drum_bars(&a, hook), Some(Bars { first: 8, count: 4 }));
    }

    #[test]
    fn the_flip_plays_each_half_beat_from_another_bar_on_the_hooks_chord() {
        let a = heard();
        let hook = hook_bars(&a).unwrap();
        let x = Stereo::zeros((40.0 * SRF) as usize);
        let (out, from) = flip(&a, &x, hook, &mut Rng::new(5));
        assert_eq!(out.len(), (4.0 * 2.5 * SRF) as usize);
        assert_eq!(from.len(), 32);
        for (i, &k) in from.iter().enumerate() {
            assert_eq!(a.numerals[k], a.numerals[hook.first + i / 8], "half beat {i} from bar {k}");
            assert!(!hook.holds(k), "half beat {i} from the hook's own bar {k}");
        }
        let mut bars = from.clone();
        bars.sort_unstable();
        bars.dedup();
        assert!(bars.len() >= 4, "from {bars:?}: pieces from all over the recording");
    }

    /// a recording: `i VI III VII` in A minor at 96 BPM, a bar each, its chords as sine triads, a kick on one and three
    /// and a noise snare on two and four, quiet for eight bars and loud for eight more with a melody over them
    fn recording() -> Stereo {
        let bar = 2.5;
        let n = (16.0 * bar * SRF) as usize;
        let mut x = Stereo::zeros(n);
        let roots = [57.0, 53.0, 48.0, 55.0];
        let thirds = [3.0, 4.0, 4.0, 4.0];
        let mut noise = Rng::new(9);
        for i in 0..n {
            let t = i as f64 / SRF;
            let b = (t / bar) as usize;
            let (root, third) = (roots[b % 4], thirds[b % 4]);
            let loud = if b >= 8 { 1.0 } else { 0.45 };
            let tone = |m: f64| (std::f64::consts::TAU * 440.0 * 2f64.powf((m - 69.0) / 12.0) * t).sin();
            let mut v = 0.12 * loud * (tone(root) + tone(root + third) + tone(root + 7.0));
            if b >= 8 {
                v += 0.15 * tone(root + 12.0 + [0.0, 7.0, 3.0, 5.0][(t / (bar / 4.0)) as usize % 4]);
            }
            let beat = t / (bar / 4.0);
            let (k, into) = (beat as usize % 4, (beat.fract()) * bar / 4.0);
            if (k == 0 || k == 2) && into < 0.12 {
                v += 0.6 * (std::f64::consts::TAU * 55.0 * into).sin() * (1.0 - into / 0.12);
            }
            if (k == 1 || k == 3) && into < 0.08 {
                v += 0.3 * loud * noise.range(-1.0, 1.0) * (1.0 - into / 0.08);
            }
            x.l[i] = v as f32;
            x.r[i] = v as f32;
        }
        x
    }

    #[test]
    fn a_remake_remixes_the_recording_on_its_hook() {
        let x = recording();
        let a = crate::analysis::song::analyze(&x, None);
        let dir = std::env::temp_dir().join(format!("claudius-remix-{}", std::process::id()));
        let g = crate::genres::get("boom_bap").unwrap();
        let src = Source { x: &x, name: "my song.wav", a: &a };
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
        let src = Source { x: &x, name: "my song.wav", a: &a };
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
        let src = Source { x: &x, name: "my song.wav", a: &a };
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
        let src = Source { x: &x, name: "my song.wav", a: &a };
        let bp = rework(&src, &g, 3, &Options::default(), Some(&lib), &dir).unwrap();
        assert_eq!(bp.key, a.key);
        assert!(bp.loops.iter().all(|l| l.name != PHRASE && l.name != SECOND), "{:?}", bp.loops);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
