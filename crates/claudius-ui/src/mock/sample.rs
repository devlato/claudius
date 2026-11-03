//! The demo's sampler. A genre built on samples (trip-hop) plays recordings, as claudius plays them: a drum break, a
//! phrase the song is built on, and a texture under it. The demo asks the site for them: `samples/search` finds loops
//! on Freesound under CC0 or CC BY (through Openverse), and `samples/audio/…` serves their previews. The site's Worker
//! (`site/worker/index.js`) does the asking, so the browser talks to no one else.
//!
//! Each recording is cut where its stated tempo says the loop ends; the phrase's key and its chords bar by bar are
//! heard from its spectrum, and each sample gets its role's colour, as trip-hop's profile mixes it. Natively,
//! `CLAUDIUS_DEMO_SAMPLES` names the address (or a folder); without it, a genre plays what it plays in their place.

use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui;
use serde::Deserialize;

use super::Rng;
use super::kit::{Clip, TIMES};
use crate::music::{self, Key};

const RATE: u32 = super::synth::RATE;
const SR: f64 = RATE as f64;
/// the address of a sampler a test answers by hand
#[cfg(test)]
const BY_HAND: &str = "by hand";
/// searches tried for a role, and recordings fetched for each search, before the role goes without
const SEARCHES: usize = 2;
const TRIES: usize = 3;
/// the longest texture kept
const TEXTURE_SECS: f64 = 12.0;

/// what a sample is in a song
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// the recording the song is built on: its tempo, key and chords become the song's
    Phrase,
    /// a drum break, its tempo fitted to the song's
    Break,
    /// crackle or noise under everything
    Texture,
}

impl Role {
    /// in the order they are gathered: the break is fitted to the phrase's tempo
    pub const ALL: [Role; 3] = [Role::Phrase, Role::Break, Role::Texture];

    /// its track, as claudius names it
    pub fn track(self) -> &'static str {
        match self {
            Role::Phrase => "sample",
            Role::Break => "break",
            Role::Texture => "texture",
        }
    }

    /// the role its track is levelled as (claudius' `render::target_level`)
    pub fn level(self) -> &'static str {
        match self {
            Role::Phrase => "sample",
            Role::Break => "drums",
            Role::Texture => "texture",
        }
    }

    pub fn noun(self) -> &'static str {
        match self {
            Role::Phrase => "phrase",
            Role::Break => "break",
            Role::Texture => "texture",
        }
    }
}

/// what a sample's colour is made of, in the order it is applied
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    Highpass(f64),
    Lowpass(f64),
    /// drive in dB into a soft clip, then a lowpass at `lowpass` Hz
    Tape {
        drive_db: f64,
        lowpass: f64,
    },
    /// fewer bits, and each sample held for `down`
    Crush {
        bits: u32,
        down: usize,
    },
}

/// a role's searches (recordings made to loop: the demo cannot cut one out of a long recording), the words one of which
/// a recording's title or tags must have and those it must not (a search for "vinyl noise" finds welding too), and its
/// colour
pub struct Want {
    pub search: &'static [&'static str],
    pub words: &'static [&'static str],
    pub not: &'static [&'static str],
    pub fx: &'static [Fx],
}

/// the samples a genre is built on, in the demo
pub struct Spec {
    pub phrase: Want,
    pub drum_break: Want,
    pub texture: Want,
}

impl Spec {
    pub fn want(&self, role: Role) -> &Want {
        match role {
            Role::Phrase => &self.phrase,
            Role::Break => &self.drum_break,
            Role::Texture => &self.texture,
        }
    }
}

/// a recording a search found, as the site's Worker gives it
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Found {
    pub id: String,
    pub title: String,
    pub creator: String,
    /// `cc0` or `by`
    pub license: String,
    #[serde(default)]
    pub license_version: String,
    #[serde(default)]
    pub license_url: String,
    /// its page on Freesound
    pub page: String,
    /// its preview, relative to the samples' address
    pub audio: String,
    pub seconds: f64,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
struct Results {
    results: Vec<Found>,
}

impl Found {
    /// "CC BY 4.0", "CC0 1.0"
    pub fn licence(&self) -> String {
        match self.license.as_str() {
            "cc0" => "CC0 1.0".into(),
            other => format!("CC {} {}", other.to_uppercase(), self.license_version).trim().to_string(),
        }
    }

    /// as a credit names it: title, author, licence, where it is
    pub fn credit(&self) -> String {
        let licence = if self.license_url.is_empty() {
            self.licence()
        } else {
            format!("{} ({})", self.licence(), self.license_url)
        };
        format!("'{}' by {}, {licence}, {}", self.title, self.creator, self.page)
    }

    fn text(&self) -> Vec<String> {
        std::iter::once(self.title.as_str()).chain(self.tags.iter().map(|t| t.as_str())).map(str::to_string).collect()
    }
}

/// a title or tag that says the recording was taken from someone's record
const SUSPECT: &[&str] =
    &["sampled from", "sample from", "taken from", "from the song", "from the album", "vinyl rip", "copyrighted", "©"];

fn suspect(f: &Found) -> bool {
    let all = f.text().join(" ").to_lowercase();
    SUSPECT.iter().any(|s| all.contains(s))
}

/// words of letters and digits (and `#`, for keys)
fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !(c.is_alphanumeric() || c == '#')).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// the tempos a recording's title and tags state: "96bpm", "80 BPM", "[148bpm]", "bpm 120", "80bmp", a tag "70" beside
/// a tag "bpm"
pub fn said_tempos(f: &Found) -> Vec<f64> {
    let mut out = Vec::new();
    let mut push = |v: f64| {
        if (50.0..=200.0).contains(&v) && !out.contains(&v) {
            out.push(v);
        }
    };
    for text in f.text() {
        let ws: Vec<String> = words(&text.to_lowercase());
        for (i, w) in ws.iter().enumerate() {
            let digits: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest = &w[digits.len()..];
            if !digits.is_empty() && matches!(rest, "bpm" | "bmp") {
                push(digits.parse().unwrap_or(0.0));
            }
            if matches!(w.as_str(), "bpm" | "bmp") {
                for near in [i.checked_sub(1), Some(i + 1)].into_iter().flatten() {
                    if let Some(v) = ws.get(near).and_then(|n| n.parse::<f64>().ok()) {
                        push(v);
                    }
                }
            }
        }
    }
    if f.tags.iter().any(|t| t.eq_ignore_ascii_case("bpm")) {
        for t in &f.tags {
            if let Ok(v) = t.parse::<f64>() {
                push(v);
            }
        }
    }
    out
}

/// a note name as titles write it: "a", "C#", "Bb", and in Dutch or German "aes", "fis", "es"
fn note_of(w: &str) -> Option<i32> {
    let mut ch = w.chars();
    let base: i32 = match ch.next()?.to_ascii_lowercase() {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let shift = match ch.as_str().to_ascii_lowercase().as_str() {
        "" => 0,
        "#" | "is" => 1,
        "b" | "es" | "s" => -1,
        _ => return None,
    };
    Some((base + shift).rem_euclid(12))
}

/// the key a title or tag states: "in a-minor", "[C Minor]", "Amin", "C#min", "F#m", "Aes major"
pub fn said_key(f: &Found) -> Option<Key> {
    let mode = |w: &str| match w.to_ascii_lowercase().as_str() {
        "minor" | "min" => Some(1),
        "major" | "maj" => Some(0),
        _ => None,
    };
    for text in f.text() {
        let ws = words(&text);
        for (i, w) in ws.iter().enumerate() {
            if let (Some(tonic), Some(m)) = (note_of(w), ws.get(i + 1).and_then(|n| mode(n))) {
                return Some(Key { tonic, mode: m });
            }
            for (suffix, m) in [("minor", 1), ("major", 0), ("min", 1), ("maj", 0), ("m", 1)] {
                let lower = w.to_ascii_lowercase();
                let Some(head) = lower.strip_suffix(suffix) else { continue };
                // "Am", "F#m": a capital letter, so that "am" and "em" in a sentence are not keys
                if suffix == "m" && !w.starts_with(|c: char| c.is_ascii_uppercase()) {
                    continue;
                }
                if let Some(tonic) = note_of(head).filter(|_| !head.is_empty()) {
                    return Some(Key { tonic, mode: m });
                }
            }
        }
    }
    None
}

/// how a loop is played: its tempo, its length in beats at that tempo, and the seconds of the recording those take
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub bpm: f64,
    pub beats: u32,
    pub secs: f64,
    /// the recording states this tempo (its loop is cut on the beat)
    pub said: bool,
}

/// `bpm`, `bpm` doubled or halved: whichever is nearest `to`
fn fold(bpm: f64, to: f64) -> f64 {
    [0.25, 0.5, 1.0, 2.0, 4.0]
        .iter()
        .map(|k| bpm * k)
        .min_by(|a, b| (a / to).ln().abs().total_cmp(&(b / to).ln().abs()))
        .unwrap_or(bpm)
}

/// a loop's tempo: one its title or tags state, if a whole number of bars at it fills its length (a short tail left
/// out); else its length alone, as whole bars at a tempo in the genre's `range`. Counted at the tempo nearest the
/// genre's: 32 beats at 148 are 16 at 74, and a recording of more than 32 beats keeps its first half
pub fn fit_loop(secs: f64, said: &[f64], range: (f64, f64)) -> Option<Fit> {
    let mid = (range.0 * range.1).sqrt();
    for &s in said {
        for beats in [64u32, 32, 16, 8, 4] {
            let cut = beats as f64 * 60.0 / s;
            if cut > secs + 0.05 || cut < secs * 0.88 {
                continue;
            }
            let bpm = fold(s, mid);
            let mut beats = (beats as f64 * bpm / s).round() as u32;
            let mut cut = cut;
            while beats > 32 {
                beats /= 2;
                cut /= 2.0;
            }
            if beats >= 2 {
                return Some(Fit { bpm, beats, secs: cut.min(secs), said: true });
            }
        }
    }
    [4u32, 8, 16, 32]
        .iter()
        .map(|&b| (b, b as f64 * 60.0 / secs))
        .filter(|(_, bpm)| *bpm >= range.0 * 0.94 && *bpm <= range.1 * 1.06)
        .min_by(|a, b| (a.1 / mid).ln().abs().total_cmp(&(b.1 / mid).ln().abs()))
        .map(|(beats, bpm)| Fit { bpm, beats, secs, said: false })
}

/// how far a tempo lies outside a range, as a ratio's log (0 inside it)
fn outside(bpm: f64, range: (f64, f64)) -> f64 {
    if bpm < range.0 {
        (range.0 / bpm).ln()
    } else if bpm > range.1 {
        (bpm / range.1).ln()
    } else {
        0.0
    }
}

/// whether a recording's title or tags have one of `these` words
fn names(f: &Found, these: &[&str]) -> bool {
    f.text().iter().flat_map(|t| words(&t.to_lowercase())).any(|w| these.contains(&w.as_str()))
}

/// what a search found that a role can play, best first, the first few in an order the seed picks
fn rank(
    role: Role,
    found: &[Found],
    want: &Want,
    range: (f64, f64),
    tempo: f64,
    rng: &mut Rng,
) -> Vec<(Found, Option<Fit>)> {
    let mut scored: Vec<(f64, Found, Option<Fit>)> = found
        .iter()
        .filter(|f| matches!(f.license.as_str(), "cc0" | "by") && !suspect(f) && !f.audio.is_empty())
        .filter(|f| (want.words.is_empty() || names(f, want.words)) && !names(f, want.not))
        .filter_map(|f| {
            let said = said_tempos(f);
            match role {
                Role::Texture => {
                    (3.0..=30.0).contains(&f.seconds).then(|| (f.seconds.min(TEXTURE_SECS), f.clone(), None))
                }
                // a loop that states its tempo was cut on the beat; a sound that happens to be short was not
                Role::Phrase => {
                    let fit = fit_loop(f.seconds, &said, range).filter(|fit| fit.said)?;
                    let off = outside(fit.bpm, range);
                    let key = said_key(f);
                    let score = -6.0 * off
                        + if key.is_some() { 0.3 } else { 0.0 }
                        + if key.is_some_and(|k| k.is_minor()) { 0.2 } else { 0.0 }
                        + if fit.beats >= 8 { 0.2 } else { 0.0 };
                    (off < 0.06).then(|| (score, f.clone(), Some(fit)))
                }
                Role::Break => {
                    let fit = fit_loop(f.seconds, &said, (tempo / 1.2, tempo * 1.2))
                        .filter(|fit| fit.said && fit.beats >= 4)?;
                    let off = (tempo / fold(fit.bpm, tempo)).ln().abs();
                    (off < 0.16).then(|| (-4.0 * off, f.clone(), Some(fit)))
                }
            }
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top = scored.len().min(4);
    if top > 1 {
        scored[..top].rotate_left(rng.below(top));
    }
    scored.into_iter().map(|(_, f, fit)| (f, fit)).collect()
}

/// a recording as the demo plays it: at its rate, the right channel empty for a mono one
pub struct Pcm2 {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Pcm2 {
    fn len(&self) -> usize {
        self.l.len()
    }
}

/// a sample, cut and coloured, and what was heard in it
pub struct Cut {
    pub role: Role,
    pub clip: Arc<Clip>,
    /// a phrase's or a break's tempo, and its length in beats at it (none for a texture)
    pub bpm: f64,
    pub beats: u32,
    /// a phrase's key
    pub key: Option<Key>,
    /// a phrase's pitch classes, bar by bar
    pub bars: Vec<[f32; 12]>,
    pub found: Found,
}

impl Cut {
    /// as the studio shows its track's sound
    pub fn label(&self) -> String {
        let by = format!("'{}' by {} ({})", self.found.title, self.found.creator, self.found.licence());
        match self.role {
            Role::Texture => format!("loop {by}"),
            _ => format!("loop {by}, {:.0} BPM", self.bpm),
        }
    }

    /// what was cut, for the log
    pub fn about(&self, key: Option<&Key>) -> String {
        let bars = self.beats as f64 / 4.0;
        match (self.role, key.or(self.key.as_ref())) {
            (Role::Texture, _) => format!("{:.0} s", self.clip.secs),
            (Role::Phrase, Some(k)) => {
                format!("{bars} bars at {:.0} BPM, in {}: {}", self.bpm, k.name(), chords(&self.bars, k).join(" "))
            }
            _ => format!("{bars} bars at {:.0} BPM", self.bpm),
        }
    }
}

/// RBJ's biquad, run over a channel
fn biquad(x: &mut [f32], highpass: bool, hz: f64) {
    let w = 2.0 * PI * (hz / SR).min(0.49);
    let (cos, alpha) = (w.cos(), w.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2));
    let (b0, b1, b2) = if highpass {
        ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0)
    } else {
        ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0)
    };
    let (a0, a1, a2) = (1.0 + alpha, -2.0 * cos, 1.0 - alpha);
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for s in x.iter_mut() {
        let x0 = *s as f64;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        (x2, x1, y2, y1) = (x1, x0, y1, y0);
        *s = y0 as f32;
    }
}

/// a sample's colour, channel by channel
fn colour(x: &mut [f32], fx: &[Fx]) {
    for f in fx {
        match *f {
            Fx::Highpass(hz) => biquad(x, true, hz),
            Fx::Lowpass(hz) => biquad(x, false, hz),
            Fx::Tape { drive_db, lowpass } => {
                let g = 10f32.powf(drive_db as f32 / 20.0);
                let norm = g.tanh();
                for s in x.iter_mut() {
                    *s = (*s * g).tanh() / norm;
                }
                biquad(x, false, lowpass);
            }
            Fx::Crush { bits, down } => {
                let steps = (1u32 << bits.clamp(2, 24).saturating_sub(1)) as f32;
                let mut held = 0.0;
                for (i, s) in x.iter_mut().enumerate() {
                    if i % down.max(1) == 0 {
                        held = (*s * steps).round() / steps;
                    }
                    *s = held;
                }
            }
        }
    }
}

/// fades a loop's ends, so that it joins itself without a click
fn fade(x: &mut [f32], fade_in: usize, fade_out: usize) {
    for (i, s) in x.iter_mut().take(fade_in).enumerate() {
        *s *= i as f32 / fade_in as f32;
    }
    for (i, s) in x.iter_mut().rev().take(fade_out).enumerate() {
        *s *= i as f32 / fade_out as f32;
    }
}

/// BS.1770's K-weighting at 48 kHz, squared: the ear's loudness, as the kit's energies are measured
fn k_weighted(x: &[f32]) -> Vec<f64> {
    let (b1, a1) = (
        [1.535_124_859_586_97, -2.691_696_189_406_38, 1.198_392_810_852_85],
        [-1.690_659_293_182_41, 0.732_480_774_215_85],
    );
    let a2 = [-1.990_047_454_833_98, 0.990_072_250_366_21];
    let (mut z, mut w) = ([0f64; 2], [0f64; 2]);
    x.iter()
        .map(|&s| {
            let v = s as f64;
            let y = b1[0] * v + z[0];
            z[0] = b1[1] * v - a1[0] * y + z[1];
            z[1] = b1[2] * v - a1[1] * y;
            let u = y + w[0];
            w[0] = -2.0 * y - a2[0] * u + w[1];
            w[1] = y - a2[1] * u;
            u * u
        })
        .collect()
}

/// a cut as the kit's clips are: its K-weighted energy (both channels, mean square times seconds) up to each of
/// `TIMES`, and its power while it plays
fn clip(l: Vec<f32>, r: Vec<f32>, beats: u32) -> Clip {
    let (kl, kr) = (k_weighted(&l), k_weighted(if r.is_empty() { &l } else { &r }));
    let sums: Vec<f64> = kl
        .iter()
        .zip(&kr)
        .scan(0.0, |acc, (a, b)| {
            *acc += a + b;
            Some(*acc)
        })
        .collect();
    let n = l.len().max(1);
    let until = |frames: usize| sums.get(frames.min(n) - 1).copied().unwrap_or(0.0) / SR;
    let energy = TIMES.iter().map(|&t| until((t as f64 * SR) as usize) as f32).collect();
    let secs = l.len() as f32 / RATE as f32;
    let power = (until(n) / (n as f64 / SR)) as f32;
    Clip { l, r, secs, energy, power, beats: beats as f32 }
}

fn loudness(power: f32) -> f32 {
    10.0 * power.max(1e-12).log10() - 0.691
}

/// the radix-2 FFT, in place
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// the twelve pitch classes a mono loop plays, bar by bar: its spectrum at a quarter of the rate, from 55 Hz to
/// 1.76 kHz, folded onto the notes nearest where its notes sit (a record a little sharp or flat still names them)
pub fn chroma(mono: &[f32], bars: usize) -> Vec<[f32; 12]> {
    const N: usize = 4096;
    let bars = bars.max(1);
    let mut x = mono.to_vec();
    biquad(&mut x, false, 4000.0);
    biquad(&mut x, false, 4000.0);
    let low: Vec<f64> = x.iter().step_by(4).map(|&s| s as f64).collect();
    let rate = SR / 4.0;
    let window: Vec<f64> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / N as f64).cos()).collect();
    // the spectrum's peaks in the notes' range: (frame centre in seconds, MIDI note, magnitude)
    let mut peaks: Vec<(f64, f64, f64)> = Vec::new();
    let mut start = 0;
    while start + N / 2 <= low.len() {
        let mut re: Vec<f64> = (0..N).map(|i| low.get(start + i).copied().unwrap_or(0.0) * window[i]).collect();
        let mut im = vec![0.0; N];
        fft(&mut re, &mut im);
        let mag: Vec<f64> = (0..N / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        let at = (start + N / 2) as f64 / rate;
        for k in 2..N / 2 - 1 {
            let f = k as f64 * rate / N as f64;
            if !(55.0..=1760.0).contains(&f) || mag[k] < mag[k - 1] || mag[k] < mag[k + 1] {
                continue;
            }
            // where between the bins the peak lies
            let (a, b, c) = (mag[k - 1], mag[k], mag[k + 1]);
            let off = 0.5 * (a - c) / (a - 2.0 * b + c).min(-1e-12);
            let f = (k as f64 + off) * rate / N as f64;
            peaks.push((at, 69.0 + 12.0 * (f / 440.0).log2(), b));
        }
        start += N / 2;
    }
    // the tuning: the mean of the peaks' distances from the nearest notes, round the circle
    let (mut s, mut c) = (0.0, 0.0);
    for &(_, m, w) in &peaks {
        let a = 2.0 * PI * (m - m.round());
        s += w * a.sin();
        c += w * a.cos();
    }
    let tuning = s.atan2(c) / (2.0 * PI);
    let secs = mono.len() as f64 / SR;
    let mut out = vec![[0f32; 12]; bars];
    for &(at, m, w) in &peaks {
        let bar = ((at / secs * bars as f64) as usize).min(bars - 1);
        let pc = ((m - tuning).round() as i64).rem_euclid(12) as usize;
        out[bar][pc] += w as f32;
    }
    out
}

/// Krumhansl and Kessler's key profiles
const MAJOR: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

fn correlation(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (x - ma, y - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    num / (da * db).sqrt().max(1e-9)
}

fn total(bars: &[[f32; 12]]) -> [f32; 12] {
    let mut all = [0f32; 12];
    for b in bars {
        for (a, x) in all.iter_mut().zip(b) {
            *a += x;
        }
    }
    all
}

/// how far a loop's pitch classes stand out from one another: the strongest's lead over the mean, as a ratio of it.
/// Notes stand out; noise, whose classes are all about as strong, does not
pub fn tonal(bars: &[[f32; 12]]) -> f32 {
    let all = total(bars);
    let mean = all.iter().sum::<f32>() / 12.0;
    let max = all.iter().copied().fold(0.0, f32::max);
    (max - mean) / mean.max(1e-9)
}

/// the key a loop's pitch classes fit best, and how well (a correlation); one its title states is preferred a little
pub fn key_of(bars: &[[f32; 12]], said: Option<Key>) -> (Key, f32) {
    let all = total(bars);
    let mut best = (Key { tonic: 9, mode: 1 }, f32::MIN, f32::MIN);
    for tonic in 0..12 {
        for (mode, profile) in [(0, &MAJOR), (1, &MINOR)] {
            let mut rotated = [0f32; 12];
            for (i, r) in rotated.iter_mut().enumerate() {
                *r = all[(i + tonic as usize) % 12];
            }
            let r = correlation(&rotated, profile);
            let key = Key { tonic, mode };
            let score = r + if said == Some(key) { 0.1 } else { 0.0 };
            if score > best.2 {
                best = (key, r, score);
            }
        }
    }
    (best.0, best.1)
}

/// the chord each bar plays, as a Roman numeral of `key`'s triads (a bar too quiet to tell keeps the one before)
pub fn chords(bars: &[[f32; 12]], key: &Key) -> Vec<String> {
    let triads: Vec<(String, music::Chord)> =
        music::diatonic(key, false).into_iter().filter_map(|n| music::chord(&n, key).map(|c| (n, c))).collect();
    let mut out: Vec<String> = Vec::new();
    for bar in bars {
        let total: f32 = bar.iter().sum();
        if total <= 1e-6 || triads.is_empty() {
            out.push(out.last().cloned().unwrap_or_else(|| if key.is_minor() { "i".into() } else { "I".into() }));
            continue;
        }
        let at = |pc: i32| bar[pc.rem_euclid(12) as usize] / total;
        let best = triads
            .iter()
            .map(|(n, c)| {
                let tones: f32 = c.tones.iter().map(|t| at(c.root + t)).sum();
                (n, tones - 0.25 * (1.0 - tones) + 0.15 * at(c.root))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(n, _)| n.clone())
            .unwrap_or_default();
        out.push(best);
    }
    out
}

/// a recording cut and coloured for its role; or why it cannot be played
pub fn cut(role: Role, pcm: &Pcm2, found: &Found, fit: Option<Fit>, want: &Want) -> Result<Cut, String> {
    let (frames, bpm, beats) = match (role, fit) {
        (Role::Texture, _) => (pcm.len().min((TEXTURE_SECS * SR) as usize), 0.0, 0),
        (_, Some(f)) => ((f.secs * SR).round() as usize, f.bpm, f.beats),
        (_, None) => return Err("no tempo to loop it at".into()),
    };
    if frames < (SR * 0.5) as usize || pcm.len() < frames * 9 / 10 {
        return Err(format!(
            "{:.1} s of audio arrived, not the {:.1} s it should loop",
            pcm.len() as f64 / SR,
            frames as f64 / SR
        ));
    }
    let take = |x: &[f32]| -> Vec<f32> { (0..frames).map(|i| x.get(i).copied().unwrap_or(0.0)).collect() };
    let (mut l, mut r) = (take(&pcm.l), if pcm.r.is_empty() { Vec::new() } else { take(&pcm.r) });
    let heard = clip(l.clone(), r.clone(), beats);
    if loudness(heard.power) < -45.0 {
        return Err("too quiet".into());
    }
    let (mut key, mut bars) = (None, Vec::new());
    if role == Role::Phrase {
        let mono: Vec<f32> =
            if r.is_empty() { l.clone() } else { l.iter().zip(&r).map(|(a, b)| (a + b) * 0.5).collect() };
        bars = chroma(&mono, (beats as usize / 4).max(1));
        let (k, fit) = key_of(&bars, said_key(found));
        if tonal(&bars) < 0.6 || fit < 0.4 {
            return Err(format!("no key to hear in it (it fits {} only {fit:.2})", k.name()));
        }
        key = Some(k);
    }
    for x in [&mut l, &mut r] {
        if x.is_empty() {
            continue;
        }
        colour(x, want.fx);
        let edge = if role == Role::Texture { (0.02 * SR) as usize } else { (0.001 * SR) as usize };
        fade(x, edge, edge.max((0.006 * SR) as usize));
    }
    Ok(Cut { role, clip: Arc::new(clip(l, r, beats)), bpm, beats, key, bars, found: found.clone() })
}

/// a cubic between samples: the previews come at 44.1 kHz, the demo plays at 48
fn resample(x: &[f32], from: u32) -> Vec<f32> {
    if from == RATE || x.is_empty() {
        return x.to_vec();
    }
    let step = from as f64 / SR;
    let n = ((x.len() as f64) / step) as usize;
    let at = |i: isize| x[i.clamp(0, x.len() as isize - 1) as usize];
    (0..n)
        .map(|k| {
            let pos = k as f64 * step;
            let i = pos as isize;
            let t = (pos - i as f64) as f32;
            let (y0, y1, y2, y3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
            let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c = -0.5 * y0 + 0.5 * y2;
            ((a * t + b) * t + c) * t + y1
        })
        .collect()
}

/// a preview (Ogg Vorbis) at the demo's rate
pub fn decode(bytes: Vec<u8>) -> Result<Pcm2, String> {
    let (l, r, rate) = super::kit::decode_at(bytes)?;
    Ok(Pcm2 { l: resample(&l, rate), r: resample(&r, rate) })
}

enum Arrival {
    Search(String, Result<Vec<Found>, String>),
    Audio(String, Result<Arc<Pcm2>, String>),
}

/// a role being looked for
struct Hunt {
    role: Role,
    terms: Vec<&'static str>,
    term: usize,
    state: Step,
}

enum Step {
    /// waiting for a search's results
    Search(String),
    /// trying what it found, in order
    Try(Vec<(Found, Option<Fit>)>, usize),
    Done,
}

/// a genre's samples being gathered for a song
pub struct Gathering {
    spec: &'static Spec,
    range: (f64, f64),
    hunts: Vec<Hunt>,
    pub cuts: Vec<Cut>,
}

impl Gathering {
    pub fn done(&self) -> bool {
        self.hunts.iter().all(|h| matches!(h.state, Step::Done))
    }

    pub fn progress(&self) -> f32 {
        let done = self.hunts.iter().filter(|h| matches!(h.state, Step::Done)).count();
        done as f32 / self.hunts.len().max(1) as f32
    }

    /// what it is doing, for the studio's status line
    pub fn doing(&self) -> String {
        for h in &self.hunts {
            match &h.state {
                Step::Search(q) => return format!("sampling: looking for a {} ('{q}')", h.role.noun()),
                Step::Try(list, at) => {
                    if let Some((f, _)) = list.get(*at) {
                        return format!("sampling: fetching a {}, '{}' by {}", h.role.noun(), f.title, f.creator);
                    }
                }
                Step::Done => {}
            }
        }
        "sampling".into()
    }

    pub fn cut(&self, role: Role) -> Option<&Cut> {
        self.cuts.iter().find(|c| c.role == role)
    }
}

/// what the demo asks the site for, and what has come back
pub struct Sampler {
    base: Option<String>,
    searched: HashMap<String, Result<Vec<Found>, String>>,
    fetched: HashMap<String, Result<Arc<Pcm2>, String>>,
    asked: HashSet<String>,
    tx: Sender<Arrival>,
    rx: Receiver<Arrival>,
    ctx: Option<egui::Context>,
}

fn default_base() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        Some("samples".into())
    } else {
        std::env::var("CLAUDIUS_DEMO_SAMPLES").ok().filter(|s| !s.trim().is_empty())
    }
}

/// a search's words in a query string
fn escape(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "%20".into(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

impl Sampler {
    pub fn new(ctx: Option<egui::Context>) -> Sampler {
        Sampler::at(default_base(), ctx)
    }

    /// a sampler asking the address or folder `base` (none: there are no samples)
    pub fn at(base: Option<String>, ctx: Option<egui::Context>) -> Sampler {
        let (tx, rx) = channel();
        Sampler { base, searched: HashMap::new(), fetched: HashMap::new(), asked: HashSet::new(), tx, rx, ctx }
    }

    /// whether there is anywhere to ask
    pub fn on(&self) -> bool {
        self.base.is_some()
    }

    fn get(&self, path: String, file: String, done: impl FnOnce(Result<Vec<u8>, String>) -> Arrival + Send + 'static) {
        let Some(base) = &self.base else { return };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let deliver = move |a: Arrival| {
            let _ = tx.send(a);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        };
        #[cfg(test)]
        if base == BY_HAND {
            return;
        }
        let base = base.trim_end_matches('/');
        #[cfg(not(target_arch = "wasm32"))]
        if !base.contains("://") {
            let path = format!("{base}/{file}");
            deliver(done(std::fs::read(&path).map_err(|e| format!("{path}: {e}"))));
            return;
        }
        let _ = file;
        let url = format!("{base}/{path}");
        ehttp::fetch(ehttp::Request::get(&url), move |res| {
            let bytes = match res {
                Ok(r) if r.ok => Ok(r.bytes),
                Ok(r) if r.status == 429 || r.status == 503 => Err("the library is busy: try again in a minute".into()),
                Ok(r) => Err(format!("{} {}", r.status, r.status_text)),
                Err(e) => Err(e),
            };
            deliver(done(bytes));
        });
    }

    fn search(&mut self, q: &str) {
        if !self.asked.insert(format!("search:{q}")) {
            return;
        }
        let query = q.to_string();
        // a folder of them names each search's file after its words
        let file = format!("search/{}.json", q.replace(' ', "_"));
        self.get(format!("search?q={}", escape(q)), file, move |bytes| {
            let found = bytes.and_then(|b| {
                serde_json::from_slice::<Results>(&b)
                    .map(|r| r.results)
                    .map_err(|e| format!("the search's answer: {e}"))
            });
            Arrival::Search(query, found)
        });
    }

    fn fetch(&mut self, audio: &str) {
        if !self.asked.insert(format!("audio:{audio}")) {
            return;
        }
        let name = audio.to_string();
        self.get(audio.to_string(), audio.to_string(), move |bytes| {
            Arrival::Audio(name, bytes.and_then(decode).map(Arc::new))
        });
    }

    /// takes what has arrived; true when something has
    pub fn poll(&mut self) -> bool {
        let mut new = false;
        while let Ok(a) = self.rx.try_recv() {
            new = true;
            match a {
                Arrival::Search(q, r) => {
                    self.searched.insert(q, r);
                }
                Arrival::Audio(name, r) => {
                    // the recordings fetched are kept while the demo runs, a few at a time
                    if self.fetched.len() > 8 {
                        self.fetched.retain(|_, v| v.is_err());
                    }
                    self.fetched.insert(name, r);
                }
            }
        }
        new
    }

    /// a sampler that asks no one: a test answers what it asks (`pending`)
    #[cfg(test)]
    pub fn by_hand() -> Sampler {
        Sampler::at(Some(BY_HAND.into()), None)
    }

    /// what it has asked for and not had: `search:<words>` and `audio:<path>`
    #[cfg(test)]
    pub fn pending(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .asked
            .iter()
            .filter(|a| match (a.strip_prefix("search:"), a.strip_prefix("audio:")) {
                (Some(q), _) => !self.searched.contains_key(q),
                (_, Some(p)) => !self.fetched.contains_key(p),
                _ => false,
            })
            .cloned()
            .collect();
        out.sort();
        out
    }

    /// a search's results arriving, as the site would send them
    #[cfg(test)]
    pub fn arrive_search(&self, q: &str, found: Result<Vec<Found>, String>) {
        let _ = self.tx.send(Arrival::Search(q.to_string(), found));
    }

    /// a recording arriving, decoded
    #[cfg(test)]
    pub fn arrive_audio(&self, audio: &str, pcm: Result<Pcm2, String>) {
        let _ = self.tx.send(Arrival::Audio(audio.to_string(), pcm.map(Arc::new)));
    }

    /// starts gathering what `spec` plays, for a song of the genre's `range` of tempos
    pub fn start(&mut self, spec: &'static Spec, range: (f64, f64), rng: &mut Rng) -> Gathering {
        let hunts = Role::ALL
            .iter()
            .map(|&role| {
                let mut terms: Vec<&'static str> = spec.want(role).search.to_vec();
                for i in (1..terms.len()).rev() {
                    terms.swap(i, rng.below(i + 1));
                }
                let state = terms.first().map_or(Step::Done, |t| Step::Search(t.to_string()));
                Hunt { role, terms, term: 0, state }
            })
            .collect();
        Gathering { spec, range, hunts, cuts: Vec::new() }
    }

    /// moves the gathering on as far as what has arrived allows; returns what to log
    pub fn advance(&mut self, g: &mut Gathering, rng: &mut Rng) -> Vec<String> {
        let mut log = Vec::new();
        let phrase_done =
            |g: &Gathering| g.hunts.iter().any(|h| h.role == Role::Phrase && matches!(h.state, Step::Done));
        for i in 0..g.hunts.len() {
            loop {
                let role = g.hunts[i].role;
                let next = match &g.hunts[i].state {
                    Step::Done => break,
                    Step::Search(q) => match self.searched.get(q) {
                        None => {
                            let q = q.clone();
                            self.search(&q);
                            break;
                        }
                        Some(Err(e)) => {
                            log.push(format!("  no {} from the search for '{q}': {e}", role.noun()));
                            None
                        }
                        // the break is fitted to the phrase's tempo: it waits for the phrase
                        Some(Ok(_)) if role == Role::Break && !phrase_done(g) => break,
                        Some(Ok(found)) => {
                            let tempo = g.cut(Role::Phrase).map_or((g.range.0 * g.range.1).sqrt(), |c| c.bpm);
                            let list = rank(role, found, g.spec.want(role), g.range, tempo, rng);
                            if list.is_empty() {
                                log.push(format!("  no {} the demo can loop in the search for '{q}'", role.noun()));
                                None
                            } else {
                                Some(Step::Try(list, 0))
                            }
                        }
                    },
                    Step::Try(list, at) => {
                        let (f, fit) = &list[*at];
                        match self.fetched.get(&f.audio) {
                            None => {
                                let audio = f.audio.clone();
                                self.fetch(&audio);
                                break;
                            }
                            Some(got) => {
                                let made = got
                                    .as_ref()
                                    .map_err(|e| e.clone())
                                    .and_then(|pcm| cut(role, pcm, f, *fit, g.spec.want(role)));
                                match made {
                                    Ok(c) => {
                                        g.cuts.push(c);
                                        Some(Step::Done)
                                    }
                                    Err(why) => {
                                        log.push(format!("  passed over '{}' by {}: {why}", f.title, f.creator));
                                        let at = at + 1;
                                        (at < list.len().min(TRIES)).then(|| Step::Try(list.clone(), at))
                                    }
                                }
                            }
                        }
                    }
                };
                let h = &mut g.hunts[i];
                h.state = match next {
                    Some(s) => s,
                    None => {
                        h.term += 1;
                        match h.terms.get(h.term).filter(|_| h.term < SEARCHES) {
                            Some(t) => Step::Search(t.to_string()),
                            None => {
                                log.push(format!("  no {} this time: the song plays without one", role.noun()));
                                Step::Done
                            }
                        }
                    }
                };
            }
        }
        log
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn found(id: &str, title: &str, secs: f64, tags: &[&str]) -> Found {
        Found {
            id: id.into(),
            title: title.into(),
            creator: "someone".into(),
            license: "by".into(),
            license_version: "4.0".into(),
            license_url: "https://creativecommons.org/licenses/by/4.0/".into(),
            page: format!("https://freesound.org/s/{id}/"),
            audio: format!("audio/1/{id}_1.ogg"),
            seconds: secs,
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    /// `beats` of a chord a bar (MIDI notes), sines at `bpm`, as a loop would be
    pub fn chord_loop(bpm: f64, bars: &[&[i32]], detune_cents: f64) -> Pcm2 {
        let bar = (4.0 * 60.0 / bpm * SR) as usize;
        let mut l = Vec::with_capacity(bar * bars.len());
        for notes in bars {
            for i in 0..bar {
                let t = i as f64 / SR;
                let env = (1.0 - i as f64 / bar as f64 * 0.5) as f32;
                let s: f64 = notes
                    .iter()
                    .map(|&m| {
                        let hz = 440.0 * 2f64.powf((m as f64 - 69.0 + detune_cents / 100.0) / 12.0);
                        (2.0 * PI * hz * t).sin()
                    })
                    .sum();
                l.push(env * 0.2 * s as f32);
            }
        }
        Pcm2 { l, r: Vec::new() }
    }

    #[test]
    fn titles_and_tags_say_tempos_and_keys() {
        let t = |title: &str, tags: &[&str]| said_tempos(&found("1", title, 5.0, tags));
        assert_eq!(t("duppyRnB01_96bpm.wav", &[]), [96.0]);
        assert_eq!(t("Hip-hop drum loop 80bmp.wav", &[]), [80.0]);
        assert_eq!(t("SOS Dark Piano Loop [148bpm] [C Minor]", &[]), [148.0]);
        assert_eq!(t("[Drumloop] 70 bpm garage beat", &[]), [70.0]);
        assert_eq!(t("drums", &["70", "beat", "bpm"]), [70.0]);
        assert_eq!(t("loop 002", &["hiphop"]), Vec::<f64>::new());
        let k = |title: &str| said_key(&found("1", title, 5.0, &[])).map(|k| k.name());
        assert_eq!(k("SOS Dark Piano Loop [148bpm] [C Minor]"), Some("C minor".into()));
        assert_eq!(k("Sad Hip Hop Piano Keys - 75bpm - Amin"), Some("A minor".into()));
        assert_eq!(k("Lofi Tape Piano Chords - 145bpm - C#min.wav"), Some("C# minor".into()));
        assert_eq!(k("dark piano-loop in f-minor 2"), Some("F minor".into()));
        assert_eq!(k("piano-loop in Aes major 1"), Some("Ab major".into()));
        assert_eq!(k("I am a loop"), None);
    }

    #[test]
    fn a_loop_is_cut_where_its_stated_tempo_says() {
        let range = (78.0, 94.0);
        assert_eq!(fit_loop(5.0, &[96.0], range), Some(Fit { bpm: 96.0, beats: 8, secs: 5.0, said: true }));
        // a 12.5 s file of 16 beats at 80, its tail left out
        assert_eq!(fit_loop(12.5, &[80.0], range), Some(Fit { bpm: 80.0, beats: 16, secs: 12.0, said: true }));
        // 64 beats at 148 are 32 at 74
        let f = fit_loop(25.95, &[148.0], range).unwrap();
        assert_eq!((f.bpm, f.beats), (74.0, 32));
        // a stated tempo the length does not hold is not believed; the length alone gives one in the range
        let f = fit_loop(11.0, &[100.0], range).unwrap();
        assert!(!f.said && f.beats == 16 && (f.bpm - 87.27).abs() < 0.01, "{f:?}");
        assert_eq!(fit_loop(1.0, &[], range), None);
    }

    #[test]
    fn the_phrase_is_ranked_by_how_it_fits_and_the_break_by_the_phrases_tempo() {
        let range = (78.0, 94.0);
        let list = [
            found("1", "dark piano-loop in a-minor", 8.0, &["100bpm", "120bpm"]),
            found("2", "Sad Hip Hop Piano Keys - 75bpm - Amin", 25.6, &[]),
            found("3", "Orchestral Strings Loop 90 BPM", 21.33, &[]),
            found("4", "piano sampled from an old record 90bpm", 21.33, &[]),
        ];
        let ranked = rank(Role::Phrase, &list, &SPEC.phrase, range, 86.0, &mut Rng::new(1));
        let ids: Vec<&str> = ranked.iter().map(|(f, _)| f.id.as_str()).collect();
        assert!(ids.contains(&"3") && ids.contains(&"2"), "{ids:?}");
        assert!(!ids.contains(&"1"), "120 BPM is far from trip-hop's tempos");
        assert!(!ids.contains(&"4"), "it says it was taken from a record");
        let breaks = [
            found("5", "drums 160bpm", 6.0, &[]),
            found("6", "duppy breakbeat 96bpm", 5.0, &[]),
            found("7", "beat 120 bpm", 8.0, &[]),
            // as long as a bar at 84, but a sound, not a loop: it states no tempo
            found("15", "foley handle cardboard box", 2.857, &["drum"]),
        ];
        let ranked = rank(Role::Break, &breaks, &SPEC.drum_break, range, 90.0, &mut Rng::new(1));
        let ids: Vec<&str> = ranked.iter().map(|(f, _)| f.id.as_str()).collect();
        assert_eq!(ids.len(), 2, "160 folds to 80 and 96 is near 90; 120 (60) is too far: {ids:?}");
        assert!(!ids.contains(&"7"));
    }

    #[test]
    fn a_phrase_is_heard_in_its_key_with_its_chords_even_a_little_flat() {
        // A minor, then F major: i and VI, 30 cents flat, as a slow record would play them
        let pcm = chord_loop(90.0, &[&[57, 60, 64, 69], &[53, 57, 60, 65]], -30.0);
        let f = found("8", "strings 90bpm", pcm.len() as f64 / SR, &[]);
        let fit = fit_loop(f.seconds, &said_tempos(&f), (78.0, 94.0));
        assert_eq!(fit.map(|f| (f.bpm, f.beats)), Some((90.0, 8)));
        let want = Want {
            search: &[],
            words: &[],
            not: &[],
            fx: &[Fx::Highpass(160.0), Fx::Tape { drive_db: 3.0, lowpass: 8500.0 }],
        };
        let c = cut(Role::Phrase, &pcm, &f, fit, &want).unwrap();
        assert_eq!(c.key.map(|k| k.name()), Some("A minor".into()));
        assert_eq!(chords(&c.bars, &c.key.unwrap()), ["i", "VI"]);
        // in the relative major, the same chords are named from C
        assert_eq!(chords(&c.bars, &Key::parse("C major").unwrap()), ["vi", "IV"]);
        assert_eq!(c.clip.beats, 8.0);
        assert!((c.clip.secs - 5.333).abs() < 0.01, "{}", c.clip.secs);
        assert!(loudness(c.clip.power) > -30.0);
        assert!(c.label().contains("'strings 90bpm' by someone (CC BY 4.0), 90 BPM"), "{}", c.label());
    }

    #[test]
    fn noise_has_no_key_and_silence_no_loop() {
        let mut rng = Rng::new(3);
        let noise = Pcm2 { l: (0..(SR as usize * 4)).map(|_| rng.range(-0.3, 0.3)).collect(), r: Vec::new() };
        let f = found("9", "noise 120bpm", 4.0, &[]);
        let fit = fit_loop(4.0, &[120.0], (78.0, 94.0));
        let want = Want { search: &[], words: &[], not: &[], fx: &[] };
        assert!(cut(Role::Phrase, &noise, &f, fit, &want).err().is_some_and(|e| e.contains("no key")));
        assert!(cut(Role::Texture, &noise, &f, None, &want).is_ok(), "noise is a texture");
        let quiet = Pcm2 { l: vec![0.0; SR as usize * 4], r: Vec::new() };
        assert_eq!(cut(Role::Break, &quiet, &f, fit, &want).err().as_deref(), Some("too quiet"));
    }

    #[test]
    fn a_preview_decodes_at_the_demos_rate() {
        let pcm = decode(include_bytes!("testdata/pluck-66.ogg").to_vec()).unwrap();
        assert!(pcm.len() > 20_000 && pcm.r.len() == pcm.len());
        let at_44 = resample(&vec![0.5; 44_100], 44_100);
        assert_eq!(at_44.len(), 48_000);
        assert!(at_44.iter().all(|s| (s - 0.5).abs() < 1e-6));
    }

    /// a sampler fed by hand, nothing fetched from anywhere: a search finds `list(words)`, a recording is `pcm(it)`
    pub fn gather(spec: &'static Spec, list: impl Fn(&str) -> Vec<Found>, pcm: impl Fn(&Found) -> Pcm2) -> Gathering {
        let mut s = Sampler::at(None, None);
        let mut rng = Rng::new(5);
        let mut g = s.start(spec, (78.0, 94.0), &mut rng);
        for _ in 0..20 {
            for h in &g.hunts {
                if let Step::Search(q) = &h.state {
                    s.arrive_search(q, Ok(list(q)));
                }
                if let Step::Try(l, at) = &h.state {
                    s.arrive_audio(&l[*at].0.audio, Ok(pcm(&l[*at].0)));
                }
            }
            s.poll();
            s.advance(&mut g, &mut rng);
            if g.done() {
                break;
            }
        }
        g
    }

    pub static SPEC: Spec = Spec {
        phrase: Want { search: &["strings loop"], words: &["strings", "piano"], not: &[], fx: &[] },
        drum_break: Want {
            search: &["drum loop"],
            words: &["drums", "drum", "beat", "breakbeat"],
            not: &[],
            fx: &[Fx::Lowpass(10_000.0)],
        },
        texture: Want { search: &["vinyl crackle"], words: &["vinyl"], not: &["foley"], fx: &[] },
    };

    #[test]
    fn a_texture_must_say_it_is_one() {
        let list = [
            found("13", "welding sparks", 8.0, &["crackle", "record", "foley"]),
            found("14", "Vinyl Dust 50.wav", 10.9, &["hiss"]),
            found("16", "vinyl stop foley", 3.5, &[]),
        ];
        let ranked = rank(Role::Texture, &list, &SPEC.texture, (78.0, 94.0), 86.0, &mut Rng::new(1));
        assert_eq!(ranked.iter().map(|(f, _)| f.id.as_str()).collect::<Vec<_>>(), ["14"]);
    }

    #[test]
    fn a_gathering_finds_a_phrase_then_a_break_fitted_to_it_and_a_texture() {
        let list = |q: &str| match q {
            "strings loop" => vec![found("10", "strings 90bpm", 5.333, &[])],
            "drum loop" => vec![found("11", "drums 92 bpm", 2.6087, &[])],
            _ => vec![found("12", "vinyl crackle", 6.0, &[])],
        };
        let g = gather(&SPEC, list, |f| match f.id.as_str() {
            "10" => chord_loop(90.0, &[&[57, 60, 64], &[53, 57, 60]], 0.0),
            _ => {
                let mut rng = Rng::new(7);
                Pcm2 { l: (0..(f.seconds * SR) as usize).map(|_| rng.range(-0.2, 0.2)).collect(), r: Vec::new() }
            }
        });
        assert!(g.done());
        let phrase = g.cut(Role::Phrase).expect("a phrase");
        assert_eq!((phrase.found.id.as_str(), phrase.bpm), ("10", 90.0));
        let brk = g.cut(Role::Break).expect("a break");
        assert_eq!((brk.found.id.as_str(), brk.bpm, brk.beats), ("11", 92.0, 4));
        assert_eq!(g.cut(Role::Texture).map(|c| c.found.id.as_str()), Some("12"));
    }

    #[test]
    fn a_search_that_fails_leaves_its_role_without_a_sample() {
        let mut s = Sampler::at(None, None);
        let mut rng = Rng::new(5);
        let mut g = s.start(&SPEC, (78.0, 94.0), &mut rng);
        for q in ["strings loop", "drum loop", "vinyl crackle"] {
            s.arrive_search(q, Err("the library is busy: try again in a minute".into()));
        }
        s.poll();
        let log = s.advance(&mut g, &mut rng);
        assert!(g.done() && g.cuts.is_empty(), "{log:?}");
        assert!(log.iter().any(|l| l.contains("busy")), "{log:?}");
        assert!(log.iter().any(|l| l.contains("no break this time")), "{log:?}");
    }
}
