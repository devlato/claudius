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
/// how often a song that does not mix in beat for beat comes in after the last one stops like a record: only after a song
/// of the families whose DJs do it (`BRAKES`), an echo out after any other
const BRAKE: f64 = 0.3;
const BRAKES: [&str; 2] = ["hip hop", "r&b, soul and funk"];
/// the families DJs blend over long beat-only intros and outros, and how long a blend runs at most, in the outgoing
/// song's bars: over the whole of the next song's intro between two of them, over a phrase of 8 for songs that sing
/// from their first bars
pub const CLUB: [&str; 6] = ["house", "techno and trance", "drum and bass", "edm", "bass music", "hard dance"];
const CLUB_BARS: u32 = 32;
const SONG_BARS: u32 = 8;
/// how many of the outgoing song's bars the next one's intro runs under, at most and at least, when the beat is handed
/// over on a downbeat (`hand_over`)
const HAND_OVER_MOST: u32 = 8;
const HAND_OVER_LEAST: u32 = 2;
/// a DJ mixer's middle band, round 1 kHz, and how far it is cut while a song's mids wait for the swap or after it
const MIDS_HZ: f64 = 1_000.0;
const MIDS_CUT_DB: f64 = -12.0;
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
            // a record stop after a song of a family whose DJs do it, an echo out after any other
            let first = genre.split('+').next().unwrap_or(genre);
            let brakes = self.all.iter().any(|g| g.name == first && BRAKES.contains(&g.family.as_str()));
            let how = if brakes && self.rng.chance(BRAKE) { How::Brake } else { How::Echo };
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

/// The layers a song's beat is: a section plays its beat when one of them plays there (a hat or a shaker alone keeps
/// time over an intro, and is not the beat a DJ hands over).
pub const BEAT: [&str; 4] = ["kick", "snare", "clap", "break"];

/// A song as its change into the next needs it: its audio, its tempo, its length in bars and its sections (where each
/// starts and how many bars it lasts, its kind, and whether its beat plays there: `BEAT`).
pub struct Song {
    pub audio: Arc<Stereo>,
    pub bpm: f64,
    pub bars: u32,
    pub sections: Vec<(u32, u32, String, bool)>,
    /// of a family DJs blend over long beat-only intros and outros (`CLUB`)
    pub club: bool,
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

/// where `a` starts to give way to the next: its outro when it has one of 8 bars or more, else its last 16 bars, or 8
/// bars before its beat stops for good where that is sooner (the most the next song's intro runs under it: `hand_over`),
/// never before its middle; the bar it starts on
pub fn leaving(a: &Song) -> u32 {
    let from = match a.sections.last() {
        Some((start, bars, kind, _)) if kind == "outro" && *bars >= 8 => *start,
        _ => a.bars.saturating_sub(16),
    };
    let end = beat_ends(a);
    let from = if end < a.bars { from.min(end.saturating_sub(HAND_OVER_MOST)) } else { from };
    from.max(a.bars.div_ceil(2))
}

/// the bar where `s`'s beat stops for good: the end of its last section that plays it (its end when none does, as a
/// song whose sections say nothing of it)
fn beat_ends(s: &Song) -> u32 {
    if !s.sections.iter().any(|x| x.3) {
        return s.bars;
    }
    s.sections.iter().rev().find(|x| x.3).map_or(s.bars, |x| x.0 + x.1)
}

/// how many bars `s` plays before its beat first comes in: its intro without drums (none when no section plays it)
fn beat_starts(s: &Song) -> u32 {
    s.sections.iter().find(|x| x.3).map_or(0, |x| x.0)
}

/// The change from `a` to `b` as `how` says, from frame `soonest` of `a` on (where it plays and a little more, when the
/// next song came late), on one of `a`'s bar lines. A blend with fewer than 4 bars of `a` left echoes out instead.
pub fn change(a: &Song, b: &Song, how: How, soonest: usize) -> Change {
    if how == How::Blend
        && let Some(c) = hand_over(a, b, soonest)
    {
        return c;
    }
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

/// When `a`'s beat stops before its end (an outro without drums) or `b`'s starts after its beginning (an intro without
/// them), a blend of the one over the other would leave the music without a beat while it lasts. Instead `b` comes in
/// under `a`'s last bars with a beat, so that `b`'s beat lands on the downbeat where `a`'s ends, as a DJ lets the next
/// record's intro run under the outgoing song's last bars and drops its beat on the one: `b`'s intro under as many of
/// `a`'s bars as it lasts (8 at most, its start left out to fit; 2 at least, the next song's first bars with its bass
/// held back), its fader up over the first half, its bass held back until the downbeat and its mids until just before
/// it, and `a` whole until its last beat, thinned and faded over it. What `a` would have played after its beat (its
/// outro) is left out. None when both beats run on through the change (the blend), or when `a`'s beat ends before
/// `soonest` (a song that came too late), or too near its start.
fn hand_over(a: &Song, b: &Song, soonest: usize) -> Option<Change> {
    let (end, start) = (beat_ends(a), beat_starts(b));
    if end >= a.bars && start == 0 {
        return None;
    }
    let bar = a.bar();
    // `b`'s bars counted in `a`'s, and its intro so
    let ratio = b.bar() / bar;
    let intro = start as f64 * ratio;
    let lead = intro.round().clamp(HAND_OVER_LEAST as f64, HAND_OVER_MOST as f64);
    let at_bar = end as f64 - lead;
    if at_bar < a.bars.div_ceil(2) as f64 || at_bar * bar + 1.0 < soonest as f64 {
        return None;
    }
    let at = (at_bar * bar).round() as usize;
    let span = (lead * bar).round() as usize;
    // the frames of `b`'s intro left out, so that its beat lands on `a`'s downbeat
    let skip = ((intro - lead).max(0.0) * bar).round() as usize;
    let beat = (bar / 4.0).round() as usize;
    let last = span.saturating_sub(beat);
    let mut b_part = window(&b.audio, skip, span);
    let rise = (span / 2).max(1);
    gain(&mut b_part, |i| if i < rise { power(i as f64 / rise as f64, true) } else { 1.0 });
    bass(&mut b_part, |i| ramp_db(i, last, span, BASS_CUT_DB, 0.0));
    mids(&mut b_part, |i| ramp_db(i, span.saturating_sub(2 * beat), last, MIDS_CUT_DB, 0.0));
    let mut a_part = window(&a.audio, at, span);
    let fall = beat.max(1) as f64;
    thin(&mut a_part, |i| if i < last { 10.0 } else { 10.0 * 30f64.powf((i - last) as f64 / fall) });
    gain(&mut a_part, |i| if i < last { 1.0 } else { power((i - last) as f64 / fall, false) });
    let mut mixed = Stereo::zeros(span);
    mixed.mix(&a_part, 1.0);
    mixed.mix(&b_part, 1.0);
    Some(Change {
        at,
        mixed,
        then: skip + span,
        said: format!("its intro under the last {lead} bars, its beat in on the downbeat where the other's ended"),
        blended: true,
    })
}

/// `b` mixed in under `a`'s last `left` bars from `at` as a DJ mixes on a three-band mixer: its fader up over the
/// first quarter with its bass and its mids held back, so that its highs come in first; the basses swapping halfway, on
/// the phrase, `b`'s mids coming in round it and `a`'s leaving after it; and `a` thinned out by a rising high-pass and
/// faded out over the second half. Over as many bars of `a` as `b`'s first section has of its own: at most 32 between
/// two songs of the families DJs blend over long beat-only intros (`CLUB`), 8 for others, at least 4, an even number.
fn blend(a: &Song, b: &Song, at: usize, left: u32) -> Change {
    let bar = a.bar();
    let intro = b.sections.first().map_or(16, |s| s.1);
    let ratio = b.bar() / bar;
    let most = if a.club && b.club { CLUB_BARS } else { SONG_BARS };
    let mut bars = left.min((intro as f64 * ratio).floor() as u32).min(most);
    bars -= bars % 2;
    let bars = bars.max(4);
    let span = (bars as f64 * bar).round() as usize;
    let half = (bars as f64 / 2.0 * bar).round() as usize;
    let beat = (bar / 4.0).round() as usize;
    let swap = half.saturating_sub(beat);
    let mut mixed = Stereo::zeros(span);
    // the next song: up over the first quarter, its bass cut until the swap and up by the half, its mids held back until
    // round the swap
    let quarter = (span / 4).max(1);
    let mut b_part = window(&b.audio, 0, span);
    bass(&mut b_part, |i| ramp_db(i, swap, half, BASS_CUT_DB, 0.0));
    mids(&mut b_part, |i| ramp_db(i, swap, half + beat, MIDS_CUT_DB, 0.0));
    gain(&mut b_part, |i| if i < quarter { power(i as f64 / quarter as f64, true) } else { 1.0 });
    // the outgoing one: its bass down by the half and its mids just after, then faded out and thinned over the second
    // half
    let mut a_part = window(&a.audio, at, span);
    bass(&mut a_part, |i| ramp_db(i, swap, half, 0.0, BASS_CUT_DB));
    mids(&mut a_part, |i| ramp_db(i, half, half + 2 * beat, 0.0, MIDS_CUT_DB));
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

/// `x` with its middle band (a broad bell round `MIDS_HZ`) lowered by `db(frame)`, 0 to `MIDS_CUT_DB`, as a DJ mixer's
/// mid knob turns: the bell cut through in full, mixed in as far as the frame asks
fn mids(x: &mut Stereo, db: impl Fn(usize) -> f64) {
    let cut = 10f64.powf(MIDS_CUT_DB / 20.0);
    for ch in [&mut x.l, &mut x.r] {
        let mut bell = Biquad::new(BiquadKind::Peak, MIDS_HZ, SRF, 0.7, MIDS_CUT_DB);
        for (i, v) in ch.iter_mut().enumerate() {
            let cut_through = bell.tick(*v);
            let how_far = ((1.0 - 10f64.powf(db(i) / 20.0)) / (1.0 - cut)).clamp(0.0, 1.0) as f32;
            *v += (cut_through - *v) * how_far;
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
        let sections = sections.into_iter().map(|(a, b, k)| (a, b, k.to_string(), true)).collect();
        Song { audio: Arc::new(Stereo::from_mono(&tone)), bpm, bars, sections, club: false }
    }

    /// `song`, its beat playing only in the sections named
    fn with_beat(s: Song, beat: &[&str]) -> Song {
        let sections = s
            .sections
            .into_iter()
            .map(|(a, b, k, _)| {
                let on = beat.contains(&k.as_str());
                (a, b, k, on)
            })
            .collect();
        Song { sections, ..s }
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

    #[test]
    fn a_beat_that_stops_before_the_end_is_handed_over_on_its_downbeat_the_next_intro_under_its_last_bars() {
        let bar = 2.0 * SRF;
        // a song whose outro has no beat, and one whose intro has none: the next intro's 4 bars under the last 4 of the
        // first's beat, the next beat in on the bar where the first's ended (its outro left out)
        let a = with_beat(song(55.0, 120.0, 64, vec![(0, 8, "intro"), (8, 48, "hook"), (56, 8, "outro")]), &["hook"]);
        let b = with_beat(song(82.0, 120.0, 64, vec![(0, 4, "intro"), (4, 60, "verse")]), &["verse"]);
        assert_eq!(leaving(&a), 48, "8 bars before the beat ends, the most an intro runs under it");
        let c = change(&a, &b, How::Blend, 0);
        assert_eq!(c.at, (52.0 * bar) as usize, "4 bars before the first's beat ends on bar 56");
        assert_eq!(
            (c.mixed.len(), c.then),
            ((4.0 * bar) as usize, (4.0 * bar) as usize),
            "on from the next one's beat"
        );
        assert!(c.blended && c.said.contains("downbeat"), "{}", c.said);
        let n = (bar / 2.0) as usize;
        // the first song whole under the intro, the next one's bass held back until the downbeat
        assert!(level(&c.mixed.l, 55.0, (bar * 2.0) as usize, n) > 0.3, "the outgoing bass under the intro");
        assert!(level(&c.mixed.l, 82.0, (bar * 2.0) as usize, n) < 0.03, "the next bass held back");
        assert!(level(&c.mixed.l, 656.0, (bar * 2.5) as usize, n) > 0.05, "the next song's top, coming in");
        // the outgoing song faded over its last beat, so that the next beat comes in alone on the downbeat
        let tail = (0.1 * SRF) as usize;
        assert!(level(&c.mixed.l, 440.0, c.mixed.len() - tail, tail) < 0.05, "the outgoing top faded out");
        // a long intro without a beat: 8 bars of it under the first, its start left out, so that its beat lands there
        let long = with_beat(song(82.0, 120.0, 64, vec![(0, 12, "intro"), (12, 52, "verse")]), &["verse"]);
        let c = change(&a, &long, How::Blend, 0);
        assert_eq!((c.at, c.then), ((48.0 * bar) as usize, (12.0 * bar) as usize), "from the intro's fifth bar");
        // a next song that starts on its beat, after an outro without one: its first 2 bars under the first's last 2
        let full = song(82.0, 120.0, 64, vec![(0, 64, "verse")]);
        let c = change(&a, &full, How::Blend, 0);
        assert_eq!((c.at, c.then), ((54.0 * bar) as usize, (2.0 * bar) as usize));
        // both beats running on through the change: the blend over the outro, as before
        let on = song(55.0, 120.0, 64, vec![(0, 16, "intro"), (16, 32, "drop"), (48, 16, "outro")]);
        let c = change(&on, &full, How::Blend, 0);
        assert!(c.said.contains("over 8 bars"), "{}", c.said);
        // a next song that came after the first's beat had ended: no hand-over, the change as it was
        let late = change(&a, &b, How::Blend, (58.3 * bar) as usize);
        assert!(!late.said.contains("downbeat"), "{}", late.said);
    }

    #[test]
    fn a_blend_runs_over_the_next_club_song_s_whole_intro_and_over_a_phrase_of_8_between_songs_that_sing_from_the_start()
     {
        let club = |hz: f32, sections| Song { club: true, ..song(hz, 126.0, 96, sections) };
        let a = club(55.0, vec![(0, 32, "intro"), (32, 32, "drop"), (64, 32, "outro")]);
        let b = club(82.0, vec![(0, 32, "intro"), (32, 64, "drop")]);
        let c = change(&a, &b, How::Blend, 0);
        let bar = 240.0 / 126.0 * SRF;
        assert!(c.blended && c.said.contains("32 bars"), "{}", c.said);
        assert_eq!(c.mixed.len(), (32.0 * bar).round() as usize);
        // songs that sing from their first bars: a phrase of 8, however long the next one's first section
        let (a, b) = (Song { club: false, ..a }, Song { club: false, ..b });
        let c = change(&a, &b, How::Blend, 0);
        assert!(c.blended && c.said.contains("8 bars"), "{}", c.said);
    }

    #[test]
    fn the_next_song_s_mids_come_in_round_the_swap_and_the_outgoing_one_s_leave_after_it() {
        // tops at 1200 Hz (the outgoing song) and 900 Hz (the next), both in a mixer's middle band
        let a = song(150.0, 120.0, 64, vec![(0, 16, "intro"), (16, 32, "drop"), (48, 16, "outro")]);
        let b = song(112.5, 120.0, 64, vec![(0, 8, "intro"), (8, 56, "drop")]);
        let c = change(&a, &b, How::Blend, 0);
        let (bar, n) = (2.0 * SRF, SRF as usize / 4);
        assert_eq!(c.mixed.len(), (8.0 * bar) as usize);
        // its third bar: the next song's fader up, its mids held back; the outgoing one's whole
        let early = (2.5 * bar) as usize;
        assert!(
            level(&c.mixed.l, 900.0, early, n) < 0.09,
            "the next mids held back: {}",
            level(&c.mixed.l, 900.0, early, n)
        );
        assert!(level(&c.mixed.l, 1200.0, early, n) > 0.15, "the outgoing mids");
        // its sixth: the next song's mids in, the outgoing one's out
        let late = (5.25 * bar) as usize;
        assert!(level(&c.mixed.l, 900.0, late, n) > 0.15, "the next mids: {}", level(&c.mixed.l, 900.0, late, n));
        assert!(
            level(&c.mixed.l, 1200.0, late, n) < 0.06,
            "the outgoing mids gone: {}",
            level(&c.mixed.l, 1200.0, late, n)
        );
    }

    #[test]
    fn a_record_stop_follows_only_a_song_of_a_family_whose_djs_do_it() {
        let key = Key { tonic: 9, minor: true };
        let hows = |from: &str| {
            let mut s = Station::new(&genres(), 11);
            (0..300).filter_map(|_| s.next(from, 90.0, key)).map(|n| n.how).collect::<Vec<_>>()
        };
        assert!(!hows("house").contains(&How::Brake), "never after house");
        assert!(!hows("synthwave").contains(&How::Brake), "nor after synthwave");
        assert!(hows("hip_hop").contains(&How::Brake), "now and then after hip hop");
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
