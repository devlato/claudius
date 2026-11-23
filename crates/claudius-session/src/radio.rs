//! The station of the app's radio (design 0024), planned as the site's radio plans it (`site/assets/radio.js`): the
//! songs a claudius of its own writes as it plays them, a genre at a time, and how each comes in after the one before;
//! and the change from one song to the next rendered here as audio, sample for sample, which the app plays as one
//! stream (where the page schedules it on Web Audio's clock).

use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use claudius::audio::Stereo;
use claudius::config::SRF;
use claudius::dsp::filter::{Biquad, BiquadKind, FilterMode, Svf, cutoff_g};
use claudius::rng::Rng;
use claudius_api::GenreInfo;

/// what the station leaves out: no beat to mix on, or too harsh for a radio
pub const LEAVE_OUT: [&str; 6] = ["ambient", "breakcore", "gabber", "hard_techno", "hardstyle", "idm"];
/// the genres a station's first song is drawn from
pub const OPENERS: [&str; 10] =
    ["house", "deep_house", "nu_disco", "hip_hop", "lofi", "afrobeats", "dnb", "synthwave", "trap", "funk"];
/// how often the next song is one that mixes into the playing one beat for beat
const BLEND: f64 = 0.75;
/// how often a song that does not mix in beat for beat comes in after the last one stops like a record
const BRAKE: f64 = 0.2;
/// the bass of a song coming in is cut this far, and the outgoing one's once they swap
const BASS_CUT_DB: f64 = -26.0;
/// where a song's bass ends, for its cut
const BASS_HZ: f64 = 200.0;
/// the families whose songs are usually in a minor key
const MINOR: [&str; 8] =
    ["hip hop", "drum and bass", "bass music", "techno and trance", "hard dance", "electronic", "house", "edm"];
const NOTES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
/// how many genres the station keeps from coming round again too soon
const RECENT: usize = 4;
/// how many of the songs of a genre built on samples sample, the rest written on the genre's own sounds alone
const SAMPLED: f64 = 2.0 / 3.0;
/// how often a station that keeps to two genres or more plays a song fusing two of them
const FUSED: f64 = 1.0 / 6.0;

/// A key as the station hears one: its tonic and whether its third is minor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub tonic: u8,
    pub minor: bool,
}

impl Key {
    /// a key as the engine names one ("A minor", "F# dorian", "Eb harmonic_minor"); none when it is not one
    pub fn parse(text: &str) -> Option<Key> {
        let text = text.trim();
        let mut chars = text.chars();
        let base = match chars.next()? {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return None,
        };
        let rest = chars.as_str();
        let (shift, rest) = match rest.chars().next() {
            Some('#') => (1, &rest[1..]),
            Some('b') => (11, &rest[1..]),
            _ => (0, rest),
        };
        let mode = rest.trim().to_lowercase().replace([' ', '-'], "_");
        let minor = matches!(
            mode.as_str(),
            "m" | "min" | "minor" | "aeolian" | "dorian" | "phrygian" | "locrian" | "harmonic_minor" | "melodic_minor"
        );
        Some(Key { tonic: (base + shift) % 12, minor })
    }

    /// as `new_song` takes it: "A minor"
    pub fn name(&self) -> String {
        format!("{} {}", NOTES[self.tonic as usize % 12], if self.minor { "minor" } else { "major" })
    }

    /// a key the next song mixes into this one in: the same notes in the mode `family` is usually in, or now and then a
    /// fifth up or down (a step on the circle of fifths)
    fn next(self, family: &str, rng: &mut Rng) -> Key {
        let minor = MINOR.contains(&family);
        let mut k = self;
        if rng.chance(0.3) {
            k.tonic = (k.tonic + if rng.chance(0.5) { 7 } else { 5 }) % 12;
        }
        if k.minor != minor {
            k = Key { tonic: (k.tonic + if minor { 9 } else { 3 }) % 12, minor };
        }
        k
    }
}

/// How a song comes in after the one before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// beat for beat under the outgoing song's outro, the basses swapping halfway
    Blend,
    /// after the outgoing song's last beat goes into an echo and it is cut
    Echo,
    /// after the outgoing song stops like a record whose turntable is switched off
    Brake,
}

/// A song the station asks the engine for, as `new_song` takes it, and how it comes in.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    /// a genre, or two fused (`house+techno`)
    pub genre: String,
    pub bpm: f64,
    pub key: Option<Key>,
    pub seed: u64,
    pub how: How,
    /// whether it samples: none, as its genre does; `Some(false)` for the songs of a genre built on samples written
    /// on its own sounds alone, a third of them (`SAMPLED`)
    pub sampled: Option<bool>,
}

/// The station: the genres it plays, those it keeps to, and those it played last.
pub struct Station {
    all: Vec<GenreInfo>,
    keep: BTreeSet<String>,
    recent: VecDeque<String>,
    rng: Rng,
}

impl Station {
    /// a station of `genres` (the engine's, less those it leaves out)
    pub fn new(genres: &[GenreInfo], seed: u64) -> Station {
        let all = genres.iter().filter(|g| !LEAVE_OUT.contains(&g.name.as_str())).cloned().collect();
        Station { all, keep: BTreeSet::new(), recent: VecDeque::new(), rng: Rng::new(seed) }
    }

    /// keeps the station to `genres` (none: every one), from the next song it plans
    pub fn keep_to(&mut self, genres: impl IntoIterator<Item = String>) {
        self.keep = genres.into_iter().collect();
    }

    /// the genres it keeps to (none: every one)
    pub fn kept(&self) -> &BTreeSet<String> {
        &self.keep
    }

    /// every genre it can play
    pub fn all(&self) -> &[GenreInfo] {
        &self.all
    }

    /// the genres it plays: those it keeps to, or every one
    pub fn genres(&self) -> Vec<&GenreInfo> {
        let chosen: Vec<&GenreInfo> = self.all.iter().filter(|g| self.keep.contains(&g.name)).collect();
        if chosen.is_empty() { self.all.iter().collect() } else { chosen }
    }

    /// its first song: one of the openers among its genres (else any of them), at the middle of its tempo range
    pub fn first(&mut self) -> Option<Spec> {
        let genres: Vec<GenreInfo> = self.genres().into_iter().cloned().collect();
        if genres.is_empty() {
            return None;
        }
        let openers: Vec<&GenreInfo> = genres.iter().filter(|g| OPENERS.contains(&g.name.as_str())).collect();
        let all: Vec<&GenreInfo> = genres.iter().collect();
        let g = (*self.rng.pick(if openers.is_empty() { &all } else { &openers })).clone();
        let sampled = self.sampled(&g);
        let spec = Spec {
            genre: g.name.clone(),
            bpm: ((g.bpm.0 + g.bpm.1) / 2.0).round(),
            key: None,
            seed: self.seed(),
            how: How::Echo,
            sampled,
        };
        self.remember(&g.name);
        Some(spec)
    }

    /// What to play after a song of `genre` at `bpm` in `key`: most of the time a genre whose tempo range takes it (or
    /// twice or half of it), written at exactly that tempo and in a key of the same notes, which blends in; else
    /// another genre at a tempo of its own and any key, which comes in after an echo or a stop. A station of a few
    /// genres plays them again sooner, and one of a single genre plays only it. Now and then (`FUSED`), a station that
    /// keeps to two genres or more fuses the genre chosen with another of them; and a third of the songs of a genre
    /// built on samples are written on its own sounds alone (`SAMPLED`).
    pub fn next(&mut self, genre: &str, bpm: f64, key: Key) -> Option<Spec> {
        let all = self.genres();
        let other: Vec<&GenreInfo> = all.iter().copied().filter(|g| g.name != genre).collect();
        let fresh: Vec<&GenreInfo> = other.iter().copied().filter(|g| !self.recent.contains(&g.name)).collect();
        let pool: Vec<GenreInfo> = match (fresh.is_empty(), other.is_empty()) {
            (false, _) => fresh.into_iter().cloned().collect(),
            (true, false) => other.into_iter().cloned().collect(),
            (true, true) => all.into_iter().cloned().collect(),
        };
        if pool.is_empty() {
            return None;
        }
        let seed = self.seed();
        let fits = |g: &GenreInfo, bpm: f64| bpm >= g.bpm.0 * 0.98 && bpm <= g.bpm.1 * 1.02;
        let mixable: Vec<(&GenreInfo, f64)> = pool
            .iter()
            .flat_map(|g| [1.0, 2.0, 0.5].into_iter().filter(move |f| fits(g, bpm * f)).map(move |f| (g, bpm * f)))
            .collect();
        let (g, mut spec) = if !mixable.is_empty() && self.rng.chance(BLEND) {
            let (g, bpm) = *self.rng.pick(&mixable);
            let spec = Spec {
                genre: g.name.clone(),
                bpm: (bpm * 10.0).round() / 10.0,
                key: Some(key.next(&g.family, &mut self.rng)),
                seed,
                how: How::Blend,
                sampled: None,
            };
            (g.clone(), spec)
        } else {
            let g = self.rng.pick(&pool).clone();
            let bpm = self.rng.range(g.bpm.0, g.bpm.1).round();
            let key = Key { tonic: self.rng.below(12) as u8, minor: MINOR.contains(&g.family.as_str()) };
            let how = if self.rng.chance(BRAKE) { How::Brake } else { How::Echo };
            let spec = Spec { genre: g.name.clone(), bpm, key: Some(key), seed, how, sampled: None };
            (g, spec)
        };
        spec.sampled = self.sampled(&g);
        self.remember(&g.name);
        // another of the genres it keeps to, fused into it now and then
        let kept: Vec<String> =
            self.genres().iter().filter(|o| self.keep.contains(&o.name)).map(|o| o.name.clone()).collect();
        let others: Vec<&String> = kept.iter().filter(|o| **o != g.name).collect();
        if kept.len() >= 2 && !others.is_empty() && self.rng.chance(FUSED) {
            let with = (*self.rng.pick(&others)).clone();
            spec.genre = format!("{}+{with}", g.name);
        }
        Some(spec)
    }

    /// whether a song of `g` samples: as the genre does, but for a third of the songs of a genre built on samples,
    /// written on its own sounds alone
    fn sampled(&mut self, g: &GenreInfo) -> Option<bool> {
        let built_on_samples = g.sampling.as_ref().is_some_and(|s| s.by_default);
        (built_on_samples && !self.rng.chance(SAMPLED)).then_some(false)
    }

    fn seed(&mut self) -> u64 {
        1 + self.rng.next_u64() % 999_999
    }

    fn remember(&mut self, genre: &str) {
        self.recent.push_back(genre.to_string());
        while self.recent.len() > RECENT {
            self.recent.pop_front();
        }
    }
}

/// A song as its change into the next needs it: its audio, its tempo, its length in bars and its sections (where each
/// starts and how many bars it lasts, and its kind).
pub struct Song {
    pub audio: Arc<Stereo>,
    pub bpm: f64,
    pub bars: u32,
    pub sections: Vec<(u32, u32, String)>,
}

impl Song {
    /// a bar, in frames
    fn bar(&self) -> f64 {
        240.0 / self.bpm * SRF
    }
}

/// The change from one song to the next: the frame of the outgoing song where it begins, the change itself (both
/// songs, mixed), and the frame of the next song it goes on from after it; how it came in, in words, and whether the
/// two were mixed beat for beat.
pub struct Change {
    pub at: usize,
    pub mixed: Stereo,
    pub then: usize,
    pub said: String,
    pub blended: bool,
}

/// where `a` starts to give way to the next: its outro when it has one of 8 bars or more, else its last 16 bars, never
/// before its middle; the bar it starts on
pub fn leaving(a: &Song) -> u32 {
    let from = match a.sections.last() {
        Some((start, bars, kind)) if kind == "outro" && *bars >= 8 => *start,
        _ => a.bars.saturating_sub(16),
    };
    from.max(a.bars.div_ceil(2))
}

/// The change from `a` to `b` as `how` says, from frame `soonest` of `a` on (where it plays and a little more, when the
/// next song came late), on one of `a`'s bar lines. A blend with fewer than 4 bars of `a` left echoes out instead.
pub fn change(a: &Song, b: &Song, how: How, soonest: usize) -> Change {
    let bar = a.bar();
    let from = (leaving(a) as f64 * bar) as usize;
    let at_bar = (soonest.max(from) as f64 / bar - 1e-6).ceil();
    let at = (at_bar * bar).round() as usize;
    let left = a.bars as i64 - at_bar as i64;
    match how {
        How::Blend if left >= 4 => blend(a, b, at, left as u32),
        How::Brake => brake(a, at),
        _ => echo_out(a, b, at, bar, false),
    }
}

/// The change for a skip, from `a`'s next beat after frame `now` (and a little after, for the change to be heard where
/// it is made): `b`, when it was to blend in (at `a`'s tempo, in a key of its notes), mixed in over two bars as a DJ
/// cuts a song short (`quick`); else `a`'s last beat into the echo and cut, and `b` in on that beat from its start.
pub fn cut(a: &Song, b: &Song, now: usize, how: How) -> Change {
    let beat = a.bar() / 4.0;
    let soonest = now as f64 + 0.15 * SRF;
    let at = ((soonest / beat).ceil() * beat).round() as usize;
    match how {
        How::Blend => quick(a, b, at),
        _ => echo_out(a, b, at, 0.0, true),
    }
}

/// `b` mixed in under `a` over two of `a`'s bars from `at`, as a DJ cuts a song short: `b` from its start, faded in
/// over a beat, its bass held back until the second bar, where the basses swap; `a` thinned by a rising high-pass and
/// faded out over the two bars
fn quick(a: &Song, b: &Song, at: usize) -> Change {
    let bar = a.bar();
    let span = (2.0 * bar).round() as usize;
    let half = bar.round() as usize;
    let beat = (bar / 4.0).round() as usize;
    let swap = half.saturating_sub(beat);
    let mut b_part = window(&b.audio, 0, span);
    bass(&mut b_part, |i| ramp_db(i, swap, half, BASS_CUT_DB, 0.0));
    gain(&mut b_part, |i| if i < beat { power(i as f64 / beat.max(1) as f64, true) } else { 1.0 });
    let mut a_part = window(&a.audio, at, span);
    bass(&mut a_part, |i| ramp_db(i, swap, half, 0.0, BASS_CUT_DB));
    thin(&mut a_part, |i| 10.0 * 80f64.powf(i as f64 / span.max(1) as f64));
    gain(&mut a_part, |i| power(i as f64 / span.max(1) as f64, false));
    let mut mixed = Stereo::zeros(span);
    mixed.mix(&a_part, 1.0);
    mixed.mix(&b_part, 1.0);
    Change { at, mixed, then: span, said: "skipped to, mixed in over two bars".into(), blended: true }
}

/// `b` faded in under `a`'s last `left` bars from `at`, its bass cut, the basses swapping halfway, and `a` thinned out
/// by a rising high-pass and faded out over the second half; over as many bars of `a` as `b`'s first section has of its
/// own (at most 16, at least 4, an even number)
fn blend(a: &Song, b: &Song, at: usize, left: u32) -> Change {
    let bar = a.bar();
    let intro = b.sections.first().map_or(16, |s| s.1);
    let ratio = b.bar() / bar;
    let mut bars = left.min((intro as f64 * ratio).floor() as u32).min(16);
    bars -= bars % 2;
    let bars = bars.max(4);
    let span = (bars as f64 * bar).round() as usize;
    let half = (bars as f64 / 2.0 * bar).round() as usize;
    let beat = (bar / 4.0).round() as usize;
    let swap = half.saturating_sub(beat);
    let mut mixed = Stereo::zeros(span);
    // the next song: faded in over the first half, its bass cut until the swap and up by the half
    let mut b_part = window(&b.audio, 0, span);
    bass(&mut b_part, |i| ramp_db(i, swap, half, BASS_CUT_DB, 0.0));
    gain(&mut b_part, |i| if i < half { power(i as f64 / half as f64, true) } else { 1.0 });
    // the outgoing one: its bass down by the half, then faded out and thinned over the second half
    let mut a_part = window(&a.audio, at, span);
    bass(&mut a_part, |i| ramp_db(i, swap, half, 0.0, BASS_CUT_DB));
    let rise = (span - half).max(1) as f64;
    thin(&mut a_part, |i| if i < half { 10.0 } else { 10.0 * 50f64.powf((i - half) as f64 / rise) });
    gain(&mut a_part, |i| if i < half { 1.0 } else { power((i - half) as f64 / rise, false) });
    mixed.mix(&a_part, 1.0);
    mixed.mix(&b_part, 1.0);
    Change { at, mixed, then: span, said: format!("mixed in beat for beat over {bars} bars, in key"), blended: true }
}

/// `a` going out on an echo at `at`: its last beat fed into a feedback echo on a dotted eighth, the song cut on `at`,
/// and `b` coming in `gap` frames later while the echoes die away (a bar for an echo out, faded in over its first bar
/// and its bass held back for two; none for a skip, at once)
fn echo_out(a: &Song, b: &Song, at: usize, gap: f64, skip: bool) -> Change {
    let beat = (a.bar() / 4.0).round() as usize;
    let start = at.saturating_sub(beat);
    let gap = gap.round() as usize;
    let b_bar = b.bar().round() as usize;
    let len = (at - start) + gap + 2 * b_bar;
    let fade = (0.05 * SRF) as usize;
    // the outgoing song's last beat at full, cut over 50 ms at `at`; what goes to the echo rises over the beat
    let mut a_part = window(&a.audio, start, (at - start) + fade);
    let cut_at = at - start;
    gain(&mut a_part, |i| if i < cut_at { 1.0 } else { 1.0 - (i - cut_at) as f32 / fade as f32 });
    let mut send = a_part.clone();
    gain(&mut send, |i| (i as f32 / cut_at.max(1) as f32).min(1.0));
    let mut mixed = Stereo::zeros(len);
    mixed.add_at(&a_part, 0, 1.0, 1.0);
    mixed.mix(&echo(&send, len, 0.75 * beat as f64), 1.0);
    // the next song, from its start
    let mut b_part = window(&b.audio, 0, 2 * b_bar);
    if !skip {
        let held = 2 * b_bar - (0.05 * SRF) as usize;
        gain(&mut b_part, |i| (i as f32 / b_bar as f32).min(1.0));
        bass(&mut b_part, |i| ramp_db(i, held, 2 * b_bar, BASS_CUT_DB, 0.0));
    }
    mixed.add_at(&b_part, (cut_at + gap) as i64, 1.0, 1.0);
    let said = if skip { "skipped to, in on the beat" } else { "in as the last song echoed out" };
    Change { at: start, mixed, then: 2 * b_bar, said: said.into(), blended: false }
}

/// `a` stopping from `at` like a record whose turntable is switched off (slowing to a twentieth of its speed over
/// 0.9 s, faded out from 0.5 s), and the next song in at once after it, a second on, from its start
fn brake(a: &Song, at: usize) -> Change {
    let len = SRF as usize;
    let mut mixed = Stereo::zeros(len);
    let (slow, quiet) = (0.9 * SRF, 0.5 * SRF);
    let mut p = 0.0f64;
    for i in 0..len {
        let t = i as f64;
        let rate = if t < slow { 1.0 - 0.95 * t / slow } else { 0.05 };
        let g = if t < quiet {
            1.0
        } else if t < slow {
            1.0 - (t - quiet) / (slow - quiet)
        } else {
            0.0
        };
        let (k, f) = (at + p as usize, p.fract() as f32);
        for (out, src) in [(&mut mixed.l, &a.audio.l), (&mut mixed.r, &a.audio.r)] {
            let x0 = src.get(k).copied().unwrap_or(0.0);
            let x1 = src.get(k + 1).copied().unwrap_or(0.0);
            out[i] = (x0 + (x1 - x0) * f) * g as f32;
        }
        p += rate;
    }
    Change { at, mixed, then: 0, said: "dropped in as the last record stopped".into(), blended: false }
}

/// frames `from..from + n` of `x`, silence past its end
fn window(x: &Stereo, from: usize, n: usize) -> Stereo {
    let mut w = Stereo::zeros(n);
    let to = (from + n).min(x.len());
    if from < to {
        w.l[..to - from].copy_from_slice(&x.l[from..to]);
        w.r[..to - from].copy_from_slice(&x.r[from..to]);
    }
    w
}

/// each frame of `x` times `g` of it
fn gain(x: &mut Stereo, g: impl Fn(usize) -> f32) {
    for i in 0..x.len() {
        let k = g(i);
        x.l[i] *= k;
        x.r[i] *= k;
    }
}

/// an equal-power fade at `x` of the way (0 to 1): up, or down
fn power(x: f64, up: bool) -> f32 {
    let x = x.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2;
    (if up { x.sin() } else { x.cos() }) as f32
}

/// `from` dB until frame `a`, `to` dB from frame `b`, a straight line in decibels between
fn ramp_db(i: usize, a: usize, b: usize, from: f64, to: f64) -> f64 {
    if i <= a {
        from
    } else if i >= b {
        to
    } else {
        from + (to - from) * (i - a) as f64 / (b - a) as f64
    }
}

/// `x`'s bass at `db(frame)` decibels (0 down to `BASS_CUT_DB`), as the page turns a low shelf at `BASS_HZ` down: the
/// shelf cut all the way, and as much of it in place of the sound as the decibels ask
fn bass(x: &mut Stereo, db: impl Fn(usize) -> f64) {
    let cut = 10f64.powf(BASS_CUT_DB / 20.0);
    for ch in [&mut x.l, &mut x.r] {
        let mut shelf = Biquad::new(BiquadKind::Lowshelf, BASS_HZ, SRF, std::f64::consts::FRAC_1_SQRT_2, BASS_CUT_DB);
        for (i, v) in ch.iter_mut().enumerate() {
            let low = shelf.tick(*v);
            let how_far = ((1.0 - 10f64.powf(db(i) / 20.0)) / (1.0 - cut)).clamp(0.0, 1.0) as f32;
            *v += (low - *v) * how_far;
        }
    }
}

/// `x` through a high-pass whose cutoff is `hz(frame)`
fn thin(x: &mut Stereo, hz: impl Fn(usize) -> f64) {
    let k = Svf::k(0.0);
    for ch in [&mut x.l, &mut x.r] {
        let mut f = Svf::default();
        for (i, v) in ch.iter_mut().enumerate() {
            *v = f.tick(*v, cutoff_g(hz(i), SRF), k, FilterMode::Hp);
        }
    }
}

/// `x` fed into an echo `len` frames long: a delay of `delay` frames feeding back on itself at 0.55, darker each time
/// round (a low-pass at 3.5 kHz), only the echoes heard
fn echo(x: &Stereo, len: usize, delay: f64) -> Stereo {
    let d = (delay.round() as usize).max(1);
    let (g, k) = (cutoff_g(3500.0, SRF), Svf::k(0.0));
    let mut out = Stereo::zeros(len);
    for (src, dst) in [(&x.l, &mut out.l), (&x.r, &mut out.r)] {
        let mut line = vec![0.0f32; len + d];
        let mut f = Svf::default();
        for i in 0..len {
            let y = f.tick(if i >= d { line[i - d] } else { 0.0 }, g, k, FilterMode::Lp);
            dst[i] = y;
            line[i] = src.get(i).copied().unwrap_or(0.0) + 0.55 * y;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genre(name: &str, family: &str, bpm: (f64, f64)) -> GenreInfo {
        GenreInfo { name: name.into(), family: family.into(), bpm, ..Default::default() }
    }

    fn genres() -> Vec<GenreInfo> {
        vec![
            genre("house", "house", (118.0, 128.0)),
            genre("deep_house", "house", (115.0, 125.0)),
            genre("techno", "techno and trance", (125.0, 140.0)),
            genre("dnb", "drum and bass", (165.0, 178.0)),
            genre("hip_hop", "hip hop", (80.0, 100.0)),
            genre("lofi", "hip hop", (70.0, 90.0)),
            genre("synthwave", "electronic", (80.0, 118.0)),
            genre("ambient", "electronic", (60.0, 100.0)),
        ]
    }

    #[test]
    fn keys_are_read_as_the_engine_names_them_and_the_next_has_the_same_notes() {
        assert_eq!(Key::parse("A minor"), Some(Key { tonic: 9, minor: true }));
        assert_eq!(Key::parse("F# dorian"), Some(Key { tonic: 6, minor: true }));
        assert_eq!(Key::parse("Eb major"), Some(Key { tonic: 3, minor: false }));
        assert_eq!(Key::parse("C harmonic_minor"), Some(Key { tonic: 0, minor: true }));
        assert_eq!(Key::parse("Bb mixolydian"), Some(Key { tonic: 10, minor: false }));
        assert_eq!(Key::parse("H minor"), None);
        assert_eq!(Key { tonic: 9, minor: true }.name(), "A minor");
        // the same notes in the mode a family is usually in, or a step round the circle of fifths
        let mut rng = Rng::new(1);
        for _ in 0..50 {
            let next = Key { tonic: 9, minor: true }.next("soul", &mut rng);
            assert!(!next.minor && [0, 7, 5].contains(&next.tonic), "{next:?}");
            let next = Key { tonic: 9, minor: true }.next("house", &mut rng);
            assert!(next.minor && [9, 4, 2].contains(&next.tonic), "{next:?}");
        }
    }

    #[test]
    fn a_station_plays_its_genres_less_those_it_leaves_out_and_keeps_to_those_chosen() {
        let mut s = Station::new(&genres(), 7);
        assert!(s.all().iter().all(|g| g.name != "ambient"), "no beat to mix on");
        let first = s.first().unwrap();
        assert!(OPENERS.contains(&first.genre.as_str()));
        let g = s.all().iter().find(|g| g.name == first.genre).unwrap();
        assert_eq!(first.bpm, ((g.bpm.0 + g.bpm.1) / 2.0).round(), "at the middle of its range");
        // every one: no genre comes round again within four songs, and most mix in beat for beat, at a tempo of theirs
        let (mut now, mut blends) = (first, 0);
        let mut played = vec![now.genre.clone()];
        let mut key = Key { tonic: 9, minor: true };
        for _ in 0..40 {
            let next = s.next(&now.genre, now.bpm, key).unwrap();
            let g = s.all().iter().find(|g| g.name == next.genre).unwrap().clone();
            assert!(next.bpm >= g.bpm.0 * 0.98 && next.bpm <= g.bpm.1 * 1.02, "{next:?} in {g:?}");
            if next.how == How::Blend {
                blends += 1;
                let ratio = next.bpm / now.bpm;
                assert!([1.0, 2.0, 0.5].iter().any(|f| (ratio - f).abs() < 0.01), "{ratio}");
            }
            assert!(!played.iter().rev().take(4).any(|p| *p == next.genre), "{next:?} after {played:?}");
            key = next.key.unwrap();
            played.push(next.genre.clone());
            now = next;
        }
        assert!(blends >= 5, "songs that mix in beat for beat blend: {blends} of 40");
        // kept to one genre, it plays only it; to two, only them
        s.keep_to(["house".to_string()]);
        let mut now = s.first().unwrap();
        assert_eq!(now.genre, "house");
        for _ in 0..10 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            assert_eq!(now.genre, "house");
        }
        s.keep_to(["house".to_string(), "techno".to_string()]);
        for _ in 0..10 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            // now and then the two fused
            assert!(now.genre.split('+').all(|g| g == "house" || g == "techno"), "{now:?}");
        }
    }

    /// a song of `secs` of a sine at `hz` (and its octave, quieter), at `bpm`, of `bars` bars and the sections given
    fn song(hz: f32, bpm: f64, bars: u32, sections: Vec<(u32, u32, &str)>) -> Song {
        let n = (bars as f64 * 240.0 / bpm * SRF) as usize + SRF as usize;
        let tone: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / SRF as f32;
                0.4 * (std::f32::consts::TAU * hz * t).sin() + 0.2 * (std::f32::consts::TAU * hz * 8.0 * t).sin()
            })
            .collect();
        let sections = sections.into_iter().map(|(a, b, k)| (a, b, k.to_string())).collect();
        Song { audio: Arc::new(Stereo::from_mono(&tone)), bpm, bars, sections }
    }

    /// the level of the band near `hz` in `x` from frame `a` for `n` frames (a correlation with the tone)
    fn level(x: &[f32], hz: f32, a: usize, n: usize) -> f32 {
        let (mut s, mut c) = (0.0f32, 0.0f32);
        for (i, v) in x[a..a + n].iter().enumerate() {
            let p = std::f32::consts::TAU * hz * (a + i) as f32 / SRF as f32;
            s += v * p.sin();
            c += v * p.cos();
        }
        2.0 * (s * s + c * c).sqrt() / n as f32
    }

    #[test]
    fn a_blend_comes_in_on_the_outro_under_its_bass_and_swaps_the_basses_halfway() {
        let a = song(55.0, 120.0, 64, vec![(0, 16, "intro"), (16, 32, "drop"), (48, 16, "outro")]);
        let b = song(82.0, 120.0, 64, vec![(0, 8, "intro"), (8, 56, "drop")]);
        let c = change(&a, &b, How::Blend, 0);
        let bar = 2.0 * SRF;
        assert_eq!(c.at, (48.0 * bar) as usize, "on the outro's first bar");
        assert_eq!(c.mixed.len(), (8.0 * bar) as usize, "over as many bars as the next song's intro");
        assert!(c.blended && c.said.contains("8 bars"));
        let n = (bar / 2.0) as usize;
        // at its start: the outgoing song whole, the next one's bass cut and faded nearly away
        assert!(level(&c.mixed.l, 55.0, 0, n) > 0.3, "the outgoing bass");
        assert!(level(&c.mixed.l, 82.0, (bar * 2.0) as usize, n) < 0.03, "the next bass held back");
        assert!(level(&c.mixed.l, 656.0, (bar * 2.5) as usize, n) > 0.05, "the next song's top, coming in");
        // after the swap: the next bass whole, the outgoing one's cut
        let late = (bar * 6.5) as usize;
        assert!(level(&c.mixed.l, 82.0, late, n) > 0.35, "the next bass");
        assert!(level(&c.mixed.l, 55.0, late, n) < 0.03, "the outgoing bass cut");
        // at its very end, the next song alone, as it goes on from there
        let tail = (0.1 * SRF) as usize;
        assert!(level(&c.mixed.l, 440.0, c.mixed.len() - tail, tail) < 0.01, "the outgoing top faded out");
        assert_eq!(c.then, c.mixed.len());
        // a song that came too late for the outro comes in on the next bar line after where it plays
        let late = change(&a, &b, How::Blend, (60.3 * bar) as usize);
        assert_eq!(late.at + (0.5 * SRF) as usize, (61.0 * bar) as usize, "cut on bar 61, from the beat before");
        assert!(!late.blended, "with 3 bars left it echoes out");
    }

    #[test]
    fn an_echo_out_cuts_on_the_bar_and_the_next_song_comes_in_a_bar_later_while_the_echoes_die_away() {
        let a = song(110.0, 120.0, 32, vec![(0, 32, "drop")]);
        let b = song(300.0, 90.0, 32, vec![(0, 32, "drop")]);
        let c = change(&a, &b, How::Echo, 0);
        let (bar, beat) = (2.0 * SRF, 0.5 * SRF);
        let at = (16.0 * bar) as usize;
        assert_eq!(c.at, at - beat as usize, "from the last beat before the cut");
        let n = (0.2 * SRF) as usize;
        let cut = beat as usize;
        assert!(level(&c.mixed.l, 110.0, cut + (0.3 * SRF) as usize, n) > 0.01, "the echoes after the cut");
        assert!(level(&c.mixed.l, 300.0, cut + (0.3 * SRF) as usize, n) < 0.005, "and no next song yet");
        let b_in = cut + bar as usize;
        let b_bar = (240.0 / 90.0 * SRF) as usize;
        assert!(level(&c.mixed.l, 300.0, b_in + b_bar, n) > 0.1, "the next song in a bar later");
        assert_eq!((c.then, c.mixed.len()), (2 * b_bar, b_in + 2 * b_bar));
        // a skip: the next song at once on the next beat after the playhead
        let s = cut_now(&a, &b, (5.1 * bar) as usize);
        let beat_at = ((5.1 * bar + 0.15 * SRF) / beat).ceil() * beat;
        assert_eq!(s.at, beat_at as usize - beat as usize);
        assert!(level(&s.mixed.l, 300.0, beat as usize + n, n) > 0.1, "the next song on the beat");
    }

    fn cut_now(a: &Song, b: &Song, now: usize) -> Change {
        cut(a, b, now, How::Echo)
    }

    #[test]
    fn a_skip_into_a_song_that_was_to_blend_mixes_it_in_over_two_bars_its_basses_swapping_on_the_second() {
        let a = song(55.0, 120.0, 32, vec![(0, 32, "drop")]);
        let b = song(82.0, 120.0, 32, vec![(0, 8, "intro"), (8, 24, "drop")]);
        let (bar, beat) = (2.0 * SRF, 0.5 * SRF);
        let c = cut(&a, &b, (5.1 * bar) as usize, How::Blend);
        let beat_at = ((5.1 * bar + 0.15 * SRF) / beat).ceil() * beat;
        assert_eq!(c.at, beat_at as usize, "from the next beat after the playhead");
        assert_eq!((c.mixed.len(), c.then), ((2.0 * bar).round() as usize, (2.0 * bar).round() as usize));
        assert!(c.blended && c.said == "skipped to, mixed in over two bars", "{}", c.said);
        let n = (beat / 2.0) as usize;
        // its first bar: the outgoing bass, the next song's top coming in, its bass held back
        assert!(level(&c.mixed.l, 55.0, (0.25 * bar) as usize, n) > 0.2, "the outgoing bass");
        assert!(level(&c.mixed.l, 82.0, (0.25 * bar) as usize, n) < 0.03, "the next bass held back");
        assert!(level(&c.mixed.l, 656.0, (0.5 * bar) as usize, n) > 0.1, "the next song's top");
        // the second: the basses swapped, and at the end the next song alone
        assert!(level(&c.mixed.l, 82.0, (1.25 * bar) as usize, n) > 0.35, "the next bass");
        assert!(level(&c.mixed.l, 55.0, (1.25 * bar) as usize, n) < 0.03, "the outgoing bass cut");
        let tail = (0.1 * SRF) as usize;
        assert!(level(&c.mixed.l, 440.0, c.mixed.len() - tail, tail) < 0.01, "the outgoing top faded out");
        // a song that was not to blend comes in on the beat after an echo, as before
        assert!(!cut(&a, &b, (5.1 * bar) as usize, How::Echo).blended);
    }

    /// `genres()` with lofi built on samples, as the engine says trip hop and lo-fi are
    fn sampling_genres() -> Vec<GenreInfo> {
        let mut all = genres();
        for g in all.iter_mut().filter(|g| g.name == "lofi") {
            g.sampling = Some(claudius_api::GenreSampling { by_default: true, roles: Vec::new() });
        }
        all
    }

    #[test]
    fn a_third_of_the_songs_of_a_genre_built_on_samples_play_without_and_the_rest_as_their_genres_do() {
        let mut s = Station::new(&sampling_genres(), 3);
        s.keep_to(["lofi".to_string()]);
        let (mut now, mut without) = (s.first().unwrap(), 0);
        let key = Key { tonic: 9, minor: true };
        for _ in 0..300 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            assert_eq!(now.genre, "lofi");
            assert!(matches!(now.sampled, None | Some(false)), "{now:?}");
            without += (now.sampled == Some(false)) as usize;
        }
        assert!((70..=130).contains(&without), "a third without samples: {without} of 300");
        s.keep_to(["house".to_string()]);
        for _ in 0..50 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            assert_eq!(now.sampled, None, "house as house does");
        }
    }

    #[test]
    fn a_station_of_two_genres_or_more_fuses_two_of_them_now_and_then_and_one_of_a_single_genre_never() {
        let mut s = Station::new(&genres(), 5);
        s.keep_to(["house".to_string(), "techno".to_string(), "deep_house".to_string()]);
        let (mut now, mut fused) = (s.first().unwrap(), 0);
        let key = Key { tonic: 9, minor: true };
        for _ in 0..300 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            let names: Vec<&str> = now.genre.split('+').collect();
            assert!(names.len() <= 2 && names.iter().all(|n| ["house", "techno", "deep_house"].contains(n)), "{now:?}");
            if let [a, b] = names[..] {
                assert_ne!(a, b, "two genres, not one twice");
                fused += 1;
            }
            // what follows it follows the genre it was written at the tempo of
            now.genre = names[0].to_string();
        }
        assert!((25..=80).contains(&fused), "now and then: {fused} of 300");
        s.keep_to(["house".to_string()]);
        for _ in 0..60 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            assert_eq!(now.genre, "house", "a single genre is never fused");
        }
        // every genre: none kept to, none fused
        s.keep_to(Vec::<String>::new());
        for _ in 0..60 {
            now = s.next(&now.genre, now.bpm, key).unwrap();
            assert!(!now.genre.contains('+'), "{now:?}");
        }
    }

    #[test]
    fn a_record_stop_slows_the_song_to_nothing_in_a_second() {
        let a = song(220.0, 120.0, 32, vec![(0, 32, "drop")]);
        let b = song(330.0, 140.0, 32, vec![(0, 32, "drop")]);
        let c = change(&a, &b, How::Brake, 0);
        assert_eq!((c.mixed.len(), c.then), (SRF as usize, 0));
        // the tone falls in pitch as it slows: fewer zero crossings in its second tenth than its first
        let crossings =
            |a: usize, b: usize| c.mixed.l[a..b].windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        let tenth = (0.1 * SRF) as usize;
        assert!(crossings(4 * tenth, 5 * tenth) < crossings(0, tenth) * 3 / 4);
        assert!(c.mixed.l[(0.95 * SRF) as usize..].iter().all(|v| v.abs() < 1e-6), "silent once it has stopped");
    }
}
