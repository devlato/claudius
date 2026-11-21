//! Reading a whole song the way a producer would before remaking it: tempo, key and swing; its sections as the song's
//! own (`structure`: where each starts, to the bar, what it is, which play the same music, where a build resolves)
//! with their energy and which layers play (kick, bass, hats, harmony); the phrases a voice sings, to the beat; the
//! chords of every bar folded into looping progressions; drum habits per 16th step (kick / snare / hats); the bass
//! style. `to_blueprint` turns that skeleton into a blueprint for a genre, so a new song can be written on it and then
//! mutated.

use serde::{Serialize, Serializer};

use super::beats;
use super::chroma;
use super::melody;
use super::onsets::{self, HOP};
use super::structure::{self, Label};
use crate::audio::Stereo;
use crate::compose::{self, Library, Options};
use crate::config::SRF;
use crate::cuts;
use crate::dsp::filter;
use crate::genres::GenreProfile;
use crate::model::{Blueprint, SectionKind, SectionSpec};
use crate::stems::Stems;
use crate::theory::chord::{Chord, roman_of};
use crate::theory::rhythm::{self, LaneTemplate};
use crate::theory::scale::Key;

#[derive(Clone, Debug, Serialize)]
pub struct SectionGuess {
    pub start_bar: usize,
    pub bars: usize,
    pub energy: f32,
    /// the kind of section a song of claudius's plays it as (`Label::kind`)
    pub kind: SectionKind,
    /// what it is: intro, verse, pre-chorus, chorus, bridge ... (`structure`)
    pub label: Label,
    /// the music it plays, as a letter: sections with the same letter play the same music
    #[serde(serialize_with = "letter")]
    pub group: usize,
    /// how many of its last bars build into the next section, which resolves the build where it starts
    pub build_bars: usize,
    /// where a piece of it is best cut from the recording, in seconds: its first bar's line, or before it, in the quiet
    /// before its first phrase, where that is sung before the bar (a pickup)
    pub cut: f64,
    /// the share of its bars a voice sings in, where the voice is heard apart from the rest
    pub sung: f32,
    pub kick: bool,
    pub bass: bool,
    pub hats: bool,
    pub harmony: bool,
}

/// the letter of the music group `g` plays: A, B, C ... Z, then A1, B1 ...
pub fn group_letter(g: usize) -> String {
    let c = (b'A' + (g % 26) as u8) as char;
    if g < 26 { c.to_string() } else { format!("{c}{}", g / 26) }
}

fn letter<S: Serializer>(g: &usize, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&group_letter(*g))
}

/// A phrase a voice sings in the recording: where it starts and ends, in seconds, and in bars and beats (the bar
/// counted from 0 as `SectionGuess::start_bar` counts it, the beat from 1, in quarters of a beat).
#[derive(Clone, Debug, Serialize)]
pub struct SungPhrase {
    pub start: f64,
    pub end: f64,
    pub bar: i64,
    pub beat: f64,
    pub end_bar: i64,
    pub end_beat: f64,
}

impl SungPhrase {
    /// where it is sung, as a DAW writes a position, bars counted from 1: "27.2-31.4", the sixteenth after the beat
    /// where it is not on one ("26.4.3")
    pub fn said(&self) -> String {
        format!("{}-{}", position(self.bar, self.beat), position(self.end_bar, self.end_beat))
    }
}

/// bar `bar` (from 0) and beat `beat` (from 1, in quarters) as a DAW writes them, the bar from 1: "27.2", "26.4.3"
fn position(bar: i64, beat: f64) -> String {
    let whole = beat.floor();
    let sixteenth = ((beat - whole) * 4.0).round() as i64 + 1;
    if sixteenth == 1 { format!("{}.{whole:.0}", bar + 1) } else { format!("{}.{whole:.0}.{sixteenth}", bar + 1) }
}

/// a voice sings in a recording when its vocals hold this share of its power at least: a track no one sings in leaves
/// a little of its lead in them
const SINGS: f64 = 0.05;

/// a row of per-step strengths as `analyze` prints it: `X` strong, `x` clear, `.` faint, `-` nothing
pub fn grid(steps: &[f32]) -> String {
    steps
        .iter()
        .map(|&x| match x {
            x if x >= 0.85 => 'X',
            x if x >= 0.4 => 'x',
            x if x > 0.0 => '.',
            _ => '-',
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub bpm: f64,
    pub key: Key,
    pub key_confidence: f64,
    pub swing: f32,
    pub pulse_clarity: f64,
    pub first_downbeat: f64,
    pub bar_secs: f64,
    pub bars: usize,
    pub chords: Vec<String>,
    /// each bar's chord as a Roman numeral in `key`
    pub numerals: Vec<String>,
    pub progression_a: Vec<String>,
    pub progression_b: Vec<String>,
    pub bars_per_chord: u32,
    pub sections: Vec<SectionGuess>,
    /// each bar's pitch classes, summing to 1
    #[serde(skip)]
    pub bar_chroma: Vec<[f32; 12]>,
    /// each bar's kick, snare and hats: how hard each hits on each 16th step of the bar, 0..1 of how hard the lane hits
    /// through the song (its 98th percentile); none for a recording no kit plays in
    #[serde(skip)]
    pub bar_drums: Vec<[[f32; 16]; 3]>,
    /// per 16th step of a bar, 0..1: how often each lane hits there in the busiest sections
    pub kick_steps: Vec<f32>,
    pub snare_steps: Vec<f32>,
    pub hat_steps: Vec<f32>,
    pub bass_style: String,
    /// whether it keeps one tempo; else its bars follow the beats a band plays, which speed up and slow down
    pub steady: bool,
    /// where each of its bars starts, in seconds, and where the last ends (`bars + 1` of them): every fourth beat
    /// from the first downbeat, on a grid of `bar_secs` for a recording that keeps one tempo
    pub bar_starts: Vec<f64>,
    /// the phrases a voice sings, in the order sung, where the voice is heard apart from the rest
    pub phrases: Vec<SungPhrase>,
}

impl Analysis {
    /// How much its beat changes from bar to bar where its kit plays (a step hit at half the lane's usual strength or
    /// more counts as hit, `bar_drums`); none where a kit plays in fewer than eight pairs of bars.
    pub fn beat_change(&self) -> Option<BeatChange> {
        beat_change_of(&self.bar_drums)
    }

    /// Where bar `b` starts, in seconds (bar `bars` is where the last ends): the beats played, or the grid of
    /// `bar_secs` past the last of them and for an analysis that has no bars of its own.
    pub fn bar_start(&self, b: usize) -> f64 {
        match (self.bar_starts.get(b), self.bar_starts.last()) {
            (Some(&t), _) => t,
            (None, Some(&last)) => last + (b + 1 - self.bar_starts.len()) as f64 * self.bar_secs,
            (None, None) => self.first_downbeat + b as f64 * self.bar_secs,
        }
    }

    /// Bars `first` to `first + count` of `x`, the recording analyzed, each made `bar_secs` long: a band's speeding up
    /// and slowing down taken out (`warp::flatten`), so that the piece keeps one tempo, `240 / bar_secs`.
    pub fn cut(&self, x: &Stereo, first: usize, count: usize) -> Stereo {
        let (t0, t1) = (self.bar_start(first), self.bar_start(first + count));
        let at = |t: f64| ((t * SRF).round().max(0.0) as usize).min(x.len());
        let piece = x.slice(at(t0), at(t1));
        let starts: Vec<f64> = (first..=first + count).map(|b| self.bar_start(b) - t0).collect();
        crate::warp::flatten(&piece, &starts, self.bar_secs)
    }

    /// the tempo a piece `cut` from it keeps
    pub fn cut_bpm(&self) -> f64 {
        240.0 / self.bar_secs.max(1e-3)
    }

    /// What it heard, as `analyze` says it: the tempo and key, the progressions, each section (its bars counted from 1,
    /// what it is and the letter of its music, how much a voice sings of it, where it builds, its first phrase sung
    /// before its first bar, and what plays), the phrases sung, the drums' habits and the bass.
    pub fn text(&self) -> String {
        let mut out = format!(
            "{:.1} BPM (pulse clarity {:.2}), {} (confidence {:.2}), swing {:.2}, {} bars from {:.2} s\n",
            self.bpm,
            self.pulse_clarity,
            self.key.name(),
            self.key_confidence,
            self.swing,
            self.bars,
            self.first_downbeat
        );
        out.push_str(&format!(
            "progression A: {}  |  B: {}  ({} bar(s) per chord)\n",
            self.progression_a.join(" "),
            self.progression_b.join(" "),
            self.bars_per_chord
        ));
        for s in &self.sections {
            let mut said = vec![format!("energy {:.2}", s.energy)];
            if s.sung > 0.0 {
                said.push(format!("{:.0}% sung", s.sung * 100.0));
            }
            if s.build_bars > 0 {
                said.push(format!(
                    "building over its last {} bar{}",
                    s.build_bars,
                    if s.build_bars == 1 { "" } else { "s" }
                ));
            }
            let first = self.bar_start(s.start_bar);
            if s.cut < first - 0.05 {
                said.push(format!("sung from {:.2} s before its first bar", first - s.cut));
            }
            let playing: Vec<&str> = [(s.kick, "kick"), (s.bass, "bass"), (s.hats, "hats")]
                .into_iter()
                .filter(|p| p.0)
                .map(|p| p.1)
                .collect();
            if !playing.is_empty() {
                said.push(playing.join(" "));
            }
            out.push_str(&format!(
                "bars {}-{}  {} ({})  {}\n",
                s.start_bar + 1,
                s.start_bar + s.bars,
                s.label.name(),
                group_letter(s.group),
                said.join(", ")
            ));
        }
        if !self.phrases.is_empty() {
            let each: Vec<String> = self.phrases.iter().map(SungPhrase::said).collect();
            out.push_str(&format!("sung phrases: {}\n", each.join(", ")));
        }
        out.push_str(&format!(
            "kick  {}\nsnare {}\nhats  {}\nbass style: {}\n",
            grid(&self.kick_steps),
            grid(&self.snare_steps),
            grid(&self.hat_steps),
            self.bass_style
        ));
        out
    }

    /// the bar `t` seconds into the recording falls in, counting from 0 (bars before the first count back from it)
    pub fn bar_at(&self, t: f64) -> i64 {
        if self.bar_starts.len() < 2 || t < self.bar_start(0) || t >= self.bar_start(self.bars) {
            return ((t - self.first_downbeat) / self.bar_secs.max(1e-3)).floor() as i64;
        }
        self.bar_starts.partition_point(|&s| s <= t) as i64 - 1
    }
}

/// The start of each bar, every fourth beat from the downbeat, and the end of the last whole bar in a recording `secs`
/// long: one more than its bars, two at least. Bars before the first downbeat the beats were heard from go back, as
/// long as the first, to the start of the recording (within 60 ms of it).
fn bars_on(beats: &[f64], downbeat: usize, beat: f64, secs: f64) -> Vec<f64> {
    let mut out: Vec<f64> = beats.iter().skip(downbeat).step_by(4).copied().collect();
    if out.is_empty() {
        out.push(0.0);
    }
    let first = if out.len() > 1 { out[1] - out[0] } else { 4.0 * beat };
    let mut back = Vec::new();
    let mut t = out[0] - first;
    while t > -0.06 {
        back.push(t.max(0.0));
        t -= first;
    }
    back.reverse();
    back.extend(out);
    let mut out = back;
    // the bar after the last downbeat ends where the beats would bring the next, when the recording lasts that long
    let last = out.len() - 1;
    let bar = if last > 0 { out[last] - out[last - 1] } else { 4.0 * beat };
    if out[last] + bar <= secs + 1e-6 || out.len() < 2 {
        out.push(out[last] + bar);
    }
    out
}

/// where bar `b` starts in a recording whose bars start at `bar_starts`: before the first and past the last, on a grid
/// of `bar_len`
fn bar_time(bar_starts: &[f64], bar_len: f64, b: i64) -> f64 {
    let n = bar_starts.len() as i64;
    if b < 0 {
        bar_starts[0] + b as f64 * bar_len
    } else if b < n {
        bar_starts[b as usize]
    } else {
        bar_starts[(n - 1) as usize] + (b - n + 1) as f64 * bar_len
    }
}

/// the bar `t` seconds into a recording whose bars start at `bar_starts` falls in, counting from 0 (bars before the
/// first count back from it, on a grid of `bar_len`, as do bars past the last)
fn bar_index(bar_starts: &[f64], bar_len: f64, t: f64) -> i64 {
    let (first, last) = (bar_starts[0], bar_starts[bar_starts.len() - 1]);
    if t < first {
        ((t - first) / bar_len.max(1e-3)).floor() as i64
    } else if t >= last {
        bar_starts.len() as i64 - 1 + ((t - last) / bar_len.max(1e-3)).floor() as i64
    } else {
        bar_starts.partition_point(|&s| s <= t) as i64 - 1
    }
}

/// Where a piece of the section starting at bar `b` is best cut: the bar's line, or, where its first phrase is sung
/// before the bar and runs into it (a pickup, starting within three quarters of the bar before), before that phrase,
/// in the quiet after whatever is sung before it (`cuts::cut_before`). A line of the section before that ends before
/// the bar is no pickup, and of the phrases that run into the bar the one started last is. The voice `sung` is heard
/// in, when it is apart.
fn cut_at(b: usize, bar_starts: &[f64], sung: Option<(&cuts::Sung, &Stereo)>) -> f64 {
    let t = bar_starts[b];
    let Some((sung, voice)) = sung else { return t };
    let reach = if b > 0 { 0.75 * (t - bar_starts[b - 1]) } else { t };
    let Some(i) = sung.phrases.iter().rposition(|p| p.start >= t - reach && p.start < t + 0.1 && p.end > t) else {
        return t;
    };
    let p = sung.phrases[i];
    if p.start >= t - 0.05 {
        return t;
    }
    let after = if i > 0 { sung.phrases[i - 1].end } else { 0.0 };
    cuts::cut_before(voice, p.start, after, t)
}

fn band(mono: &[f32], lo: f64, hi: f64) -> Vec<f32> {
    let mut x = mono.to_vec();
    if lo > 0.0 {
        filter::butter(&mut x, true, lo, SRF, 4);
    }
    if hi > 0.0 {
        filter::butter(&mut x, false, hi, SRF, 4);
    }
    x
}

fn rms_db(x: &[f32], a: usize, b: usize) -> f64 {
    let b = b.min(x.len());
    if b <= a {
        return -120.0;
    }
    10.0 * (x[a..b].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (b - a) as f64).max(1e-14).log10()
}

fn steps_of(env: &[f32], bar_start: f64, bar_len: f64) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for (k, o) in out.iter_mut().enumerate() {
        let t = bar_start + k as f64 * bar_len / 16.0;
        let c = (t * SRF / HOP as f64) as i64;
        let w = ((0.015 * SRF) / HOP as f64).ceil() as i64;
        *o = ((c - w)..=(c + w))
            .filter(|i| *i >= 0 && (*i as usize) < env.len())
            .map(|i| env[i as usize])
            .fold(0.0, f32::max);
    }
    out
}

/// each bar's lanes, each scaled to how hard it hits through the song: its 98th percentile, so that a few loud hits do
/// not make every other one look soft
fn bar_drums(lanes: &[&Vec<[f32; 16]>; 3]) -> Vec<[[f32; 16]; 3]> {
    let bars = lanes[0].len();
    let scale: Vec<f32> = lanes
        .iter()
        .map(|lane| {
            let mut all: Vec<f32> = lane.iter().flatten().copied().collect();
            all.sort_by(f32::total_cmp);
            all.get(((all.len() as f64 * 0.98) as usize).min(all.len().saturating_sub(1))).copied().unwrap_or(0.0)
        })
        .collect();
    (0..bars)
        .map(|b| {
            let mut bar = [[0.0f32; 16]; 3];
            for (l, lane) in lanes.iter().enumerate() {
                for k in 0..16 {
                    bar[l][k] = if scale[l] > 1e-9 { (lane[b][k] / scale[l]).min(1.0) } else { 0.0 };
                }
            }
            bar
        })
        .collect()
}

/// `Analysis::beat_change` of bars `bar_drums` (`Analysis::bar_drums`)
fn beat_change_of(bar_drums: &[[[f32; 16]; 3]]) -> Option<BeatChange> {
    let hits: Vec<u64> = bar_drums
        .iter()
        .map(|bar| {
            let mut m = 0u64;
            for (l, lane) in bar.iter().enumerate() {
                for (k, v) in lane.iter().enumerate() {
                    if *v >= 0.5 {
                        m |= 1 << (l * 16 + k);
                    }
                }
            }
            m
        })
        .collect();
    // a bar the kit plays in: a kick or a snare in it
    let playing = |m: u64| m & 0xFFFF_FFFF != 0;
    let apart = |lag: usize| -> Vec<f64> {
        hits.windows(lag + 1)
            .filter(|w| playing(w[0]) && playing(w[lag]))
            .map(|w| f64::from((w[0] ^ w[lag]).count_ones()) / f64::from((w[0] | w[lag]).count_ones().max(1)))
            .collect()
    };
    let median = |mut v: Vec<f64>| -> Option<f64> {
        if v.len() < 8 {
            return None;
        }
        v.sort_by(f64::total_cmp);
        Some(v[v.len() / 2])
    };
    let next = apart(1);
    let turns = next.iter().filter(|d| **d >= 0.5).count() as f64 / next.len().max(1) as f64;
    Some(BeatChange { next: median(next)?, four: median(apart(4))?, turns })
}

/// How much a beat changes from bar to bar (`Analysis::beat_change`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct BeatChange {
    /// of the kick, snare and hat hits on the 16th steps of two bars in a row, the share in one and not both (Jaccard's
    /// distance), the median over the song's bars
    pub next: f64,
    /// the same of bars four apart: a beat whose phrase comes round every four bars changes here less than bar to bar
    pub four: f64,
    /// the share of bars in a row at least half of whose hits differ: fills, edits and switches
    pub turns: f64,
}

fn template(avg: &[f32; 16]) -> Vec<f32> {
    let mx = avg.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    avg.iter()
        .map(|v| {
            let s = v / mx;
            if s >= 0.85 { 1.0 } else { ((s - 0.25) / 0.6).clamp(0.0, 0.95) }
        })
        .collect()
}

pub fn analyze(x: &Stereo, bpm_range: Option<(f64, f64)>) -> Analysis {
    analyze_with(x, None, bpm_range)
}

/// `analyze`, with the recording's `stems` heard apart (`crate::stems`): its beats and its drums in its drums, when a
/// kit plays (a recording without one has no drum habits, and no kick or hats in its sections); its chords in its
/// bass and the rest, without the voice's melody over them; its bass in its bass.
pub fn analyze_with(x: &Stereo, stems: Option<&Stems>, bpm_range: Option<(f64, f64)>) -> Analysis {
    let mono = x.mono();
    let drummed = stems.is_none_or(Stems::drummed);
    let drums = stems.map(|s| s.drums.mono());
    let harmonic = stems.map(|s| {
        let mut h = s.bass.clone();
        h.add_at(&s.other, 0, 1.0, 1.0);
        h.mono()
    });
    let bassline = stems.map(|s| s.bass.mono());
    let (drum_src, chord_src, bass_src) =
        (drums.as_deref().unwrap_or(&mono), harmonic.as_deref().unwrap_or(&mono), bassline.as_deref().unwrap_or(&mono));
    let kit = drummed && drums.is_some();
    let grid = beats::track_with(if drummed { drum_src } else { &mono }, chord_src, bpm_range, kit);
    let bar_starts = bars_on(&grid.beats, grid.downbeat, 60.0 / grid.bpm.max(1.0), mono.len() as f64 / SRF);
    let bars = bar_starts.len() - 1;
    let start = bar_starts[0];
    let bar_len = (bar_starts[bars] - start) / bars as f64;
    let at = |b: usize| (bar_starts[b.min(bars)] * SRF) as usize;
    let t_bar = |b: usize| bar_starts[b.min(bars)];
    // a bar as long as the band played it
    let len_of = |b: usize| bar_starts[(b + 1).min(bars)] - bar_starts[b.min(bars)];
    let (low, high) = (band(&mono, 30.0, 150.0), band(&mono, 5000.0, 0.0));
    let kick_env = onsets::flux(&band(drum_src, 30.0, 150.0), None);
    let snare_env = onsets::flux(drum_src, Some((1000.0, 5000.0)));
    let hat_env = onsets::flux(drum_src, Some((6000.0, 16000.0)));
    // the bass's level, bar by bar: the bass's own, apart, or the mix's low end
    let bass_low = stems.map(|_| band(bass_src, 30.0, 250.0));
    let chroma_hop = 4096;
    let ch = chroma::chroma(chord_src, chroma_hop);
    let bass_ch = chroma::chroma_band(bass_src, chroma_hop, 40.0, 220.0);
    let mut bar_bass: Vec<[f32; 12]> = Vec::with_capacity(bars);
    let mut rms = Vec::with_capacity(bars);
    let mut hi_db = Vec::with_capacity(bars);
    let mut bar_chroma: Vec<[f32; 12]> = Vec::with_capacity(bars);
    let mut kick = Vec::with_capacity(bars);
    let mut snare = Vec::with_capacity(bars);
    let mut hat = Vec::with_capacity(bars);
    let mut bass_db = Vec::with_capacity(bars);
    // where the kit comes in and goes, bar by bar, when the stems are apart
    let mut kit_db = Vec::with_capacity(bars);
    for b in 0..bars {
        let (a, e) = (at(b), at(b + 1));
        rms.push(rms_db(&mono, a, e));
        hi_db.push(rms_db(&high, a, e));
        bass_db.push(rms_db(bass_low.as_deref().unwrap_or(&low), a, e));
        if drums.is_some() {
            kit_db.push(rms_db(drum_src, a, e));
        }
        let (fa, fe) = (a / chroma_hop, (e / chroma_hop).min(ch.len()));
        let mut c = [0.0f32; 12];
        for f in ch.iter().take(fe).skip(fa) {
            for i in 0..12 {
                c[i] += f[i];
            }
        }
        let s: f32 = c.iter().sum::<f32>().max(1e-9);
        c.iter_mut().for_each(|v| *v /= s);
        bar_chroma.push(c);
        let mut cb = [0.0f32; 12];
        // the bass under the chord: second halves of the first three beats, where the kick has decayed (and a
        // sidechained bass swells back), before any walk-up to the next chord
        let beat = len_of(b) / 4.0;
        for (fi, f) in bass_ch.iter().enumerate().take(fe.min(bass_ch.len())).skip(fa) {
            let t = (fi * chroma_hop + chroma::NFFT / 2) as f64 / SRF - t_bar(b);
            let in_beat = (t / beat).fract();
            if (0.45..0.95).contains(&in_beat) && t < 3.0 * beat {
                for i in 0..12 {
                    cb[i] += f[i];
                }
            }
        }
        let sb: f32 = cb.iter().sum::<f32>().max(1e-9);
        cb.iter_mut().for_each(|v| *v /= sb);
        bar_bass.push(cb);
        let (t0, len) = (t_bar(b), len_of(b));
        kick.push(steps_of(&kick_env, t0, len));
        snare.push(steps_of(&snare_env, t0, len));
        hat.push(steps_of(&hat_env, t0, len));
    }
    // key from the loudness-weighted average chroma
    let mut avg = [0.0f32; 12];
    for (b, c) in bar_chroma.iter().enumerate() {
        let w = 10f64.powf(rms[b] / 20.0) as f32;
        for i in 0..12 {
            avg[i] += c[i] * w;
        }
    }
    // key: of the closest profile fits, the one whose tonic chord opens the bars most and that explains the most
    // chords diatonically
    let cands = chroma::key_candidates(&avg, 4);
    let mut best_key = (cands[0].0, f64::MIN, cands[0].1);
    for (k, c) in &cands {
        let chords_k: Vec<Chord> =
            (0..bars).map(|b| chroma::chord_with_bass(&bar_chroma[b], &bar_bass[b], Some(k)).0).collect();
        let diatonic = chords_k.iter().filter(|ch| ch.pcs().iter().all(|p| k.pcs().contains(p))).count() as f64
            / bars.max(1) as f64;
        let tonic =
            chords_k.iter().filter(|ch| ch.root == k.tonic && ch.quality.is_minorish() == k.mode.is_minor()).count()
                as f64
                / bars.max(1) as f64;
        let bass_tonic = (0..bars).map(|b| bar_bass[b][k.tonic as usize] as f64).sum::<f64>() / bars.max(1) as f64;
        let s = c + 0.5 * diatonic + 1.5 * tonic + 0.5 * bass_tonic;
        if s > best_key.1 {
            best_key = (*k, s, *c);
        }
    }
    let (key, key_conf) = (best_key.0, best_key.2);
    let chords: Vec<Chord> =
        (0..bars).map(|b| chroma::chord_with_bass(&bar_chroma[b], &bar_bass[b], Some(&key)).0).collect();

    // sections, as the song's own (`structure`): what each bar's beats play, how it sounds, and where a voice sings: in
    // the phrases of the vocals the ears' tagger hears as a voice, where it is there, and not a riff or a lead the
    // separation left in them
    let sings = stems.filter(|s| s.share(&s.vocals) >= SINGS);
    let sung = sings.map(|s| {
        let heard = cuts::sung(&s.vocals, &melody::pitches(&s.vocals));
        let asked = cuts::stretches(&heard);
        match crate::stems::voiced(&s.vocals, &asked) {
            Some(voiced) => cuts::voiced_only(&heard, &asked, &voiced),
            None => heard,
        }
    });
    // the kit's level in each bar: its drums apart, else how strongly its hits strike in the mix over the whole bar,
    // which does not hang on where the grid's steps fall (a bar drawn a little off the beats misses them)
    let kit_level: Vec<f64> = if kit_db.is_empty() {
        let mean = |env: &[f32], b: usize| {
            let (a, e) = ((t_bar(b) * SRF / HOP as f64) as usize, ((t_bar(b + 1) * SRF / HOP as f64) as usize));
            let e = e.min(env.len());
            if e <= a { 0.0 } else { env[a..e].iter().map(|v| *v as f64).sum::<f64>() / (e - a) as f64 }
        };
        (0..bars)
            .map(|b| 10.0 * (mean(&kick_env, b) + mean(&snare_env, b) + mean(&hat_env, b) + 1e-6).log10())
            .collect()
    } else {
        kit_db.clone()
    };
    // what the voice sings, beat by beat, in the bars it sings a tenth of at least
    let melody = |sings: Option<&Stems>, sung: Option<&cuts::Sung>| -> Option<Vec<[f32; 48]>> {
        let (s, sung) = (sings?, sung?);
        let frames = chroma::chroma(&s.vocals.mono(), chroma_hop);
        let share = structure::sung_share(&sung.words, &bar_starts);
        Some(
            (0..bars)
                .map(|b| match share[b] >= 0.1 {
                    true => structure::harmony(&frames, chroma_hop, t_bar(b), t_bar(b + 1)),
                    false => [0.0; 48],
                })
                .collect(),
        )
    };
    let heard = structure::Heard {
        harmony: (0..bars).map(|b| structure::harmony(&ch, chroma_hop, t_bar(b), t_bar(b + 1))).collect(),
        melody: melody(sings, sung.as_ref()),
        timbre: structure::timbres(&mono, &bar_starts),
        loud: rms.clone(),
        bright: hi_db.iter().zip(&rms).map(|(h, r)| h - r).collect(),
        bass: bass_db.clone(),
        kit: kit_level,
        // with the stems apart the voice is heard, and where it holds too little of the recording no one sings; without
        // them it is not heard at all
        voice: match (&sung, stems) {
            (Some(s), _) => Some(structure::sung_share(&s.words, &bar_starts)),
            (None, Some(_)) => Some(vec![0.0; bars]),
            (None, None) => None,
        },
        bar_secs: bar_len,
    };
    let parts = structure::parts(&heard);
    let sec_rms: Vec<f64> =
        parts.iter().map(|p| (p.start..p.start + p.bars).map(|b| rms[b]).sum::<f64>() / p.bars.max(1) as f64).collect();
    let (rmin, rmax) = sec_rms.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    // sections within 1.5 dB of each other all play at one level, a groove's, rather than each as loud as a drop or as
    // quiet as a break by a difference no one hears
    let flat = rmax - rmin < 1.5;
    let mean_steps = |v: &Vec<[f32; 16]>, a: usize, b: usize| -> f32 {
        (a..b).map(|i| v[i].iter().sum::<f32>()).sum::<f32>() / ((b - a).max(1) * 16) as f32
    };
    let kick_ref = mean_steps(&kick, 0, bars).max(1e-9);
    let hat_ref = mean_steps(&hat, 0, bars).max(1e-9);
    let bass_ref = bass_db.iter().cloned().fold(f64::MIN, f64::max);
    // the stems say whether a bass plays at all: a twentieth of the power at least
    let bassed = stems.is_none_or(|s| s.share(&s.bass) >= 0.02);
    let sung_with = sung.as_ref().zip(sings.map(|s| &s.vocals));
    let mut sections: Vec<SectionGuess> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let (a, b) = (p.start, p.start + p.bars);
        let e = if flat { 0.6 } else { ((sec_rms[i] - rmin) / (rmax - rmin)) as f32 };
        let bass_m = (a..b).map(|x| bass_db[x]).sum::<f64>() / (b - a).max(1) as f64;
        let kick_on = mean_steps(&kick, a, b) > 0.6 * kick_ref;
        let sung_share = heard.voice.as_ref().map_or(0.0, |v| v[a..b].iter().sum::<f64>() / (b - a).max(1) as f64);
        sections.push(SectionGuess {
            start_bar: a,
            bars: b - a,
            energy: 0.3 + 0.7 * e,
            kind: p.label.kind(),
            label: p.label,
            group: p.group,
            build_bars: p.build_bars,
            cut: cut_at(a, &bar_starts, sung_with),
            sung: sung_share as f32,
            kick: drummed && kick_on,
            bass: bassed && bass_m > bass_ref - 9.0,
            hats: drummed && mean_steps(&hat, a, b) > 0.5 * hat_ref,
            harmony: true,
        });
    }
    let beat_of = |t: f64| -> (i64, f64) {
        let bar = bar_index(&bar_starts, bar_len, t);
        let (t0, t1) = (bar_time(&bar_starts, bar_len, bar), bar_time(&bar_starts, bar_len, bar + 1));
        let beat = 1.0 + ((t - t0) / ((t1 - t0) / 4.0).max(1e-6) * 4.0).floor() / 4.0;
        (bar, beat.clamp(1.0, 4.75))
    };
    let phrases: Vec<SungPhrase> = sung
        .as_ref()
        .map(|s| {
            s.phrases
                .iter()
                .map(|p| {
                    let ((bar, beat), (end_bar, end_beat)) = (beat_of(p.start), beat_of(p.end));
                    SungPhrase { start: p.start, end: p.end, bar, beat, end_bar, end_beat }
                })
                .collect()
        })
        .unwrap_or_default();
    // progressions: the repeating cycle of chords in the loudest sections, and in the quiet ones
    let cycle = |a: usize, b: usize| -> (Vec<Chord>, u32) {
        let cs: Vec<Chord> = chords[a..b].to_vec();
        let mut best = (cs.len().clamp(1, 4), -1.0f64);
        for l in [2usize, 4, 8] {
            if cs.len() < 2 * l {
                continue;
            }
            let agree = (0..cs.len() - l).filter(|&i| cs[i] == cs[i + l]).count() as f64 / (cs.len() - l) as f64;
            if agree > best.1 + 0.05 {
                best = (l, agree);
            }
        }
        let mut c: Vec<Chord> = cs.into_iter().take(best.0).collect();
        let bpc = if c.len() >= 2 && c.len().is_multiple_of(2) && c.chunks(2).all(|p| p[0] == p[1]) {
            c = c.chunks(2).map(|p| p[0]).collect();
            2
        } else {
            1
        };
        (c, bpc)
    };
    let loud = sections
        .iter()
        .filter(|s| s.kind == SectionKind::Drop)
        .max_by_key(|s| s.bars)
        .or_else(|| sections.iter().max_by(|a, b| a.energy.partial_cmp(&b.energy).unwrap()));
    let (ca, bpc) = loud.map_or((Vec::new(), 1), |s| cycle(s.start_bar, s.start_bar + s.bars));
    let quiet =
        sections.iter().filter(|s| matches!(s.kind, SectionKind::Break | SectionKind::Intro)).max_by_key(|s| s.bars);
    let (cb, _) = quiet.map_or((ca.clone(), bpc), |s| cycle(s.start_bar, s.start_bar + s.bars));
    let to_rn = |cs: &[Chord]| cs.iter().map(|c| roman_of(c, &key)).collect::<Vec<_>>();

    // drums and swing from the busiest sections
    let busy: Vec<usize> =
        sections.iter().filter(|s| s.kick && s.energy >= 0.6).flat_map(|s| s.start_bar..s.start_bar + s.bars).collect();
    let busy = if busy.is_empty() { (0..bars).collect() } else { busy };
    let avg_steps = |v: &Vec<[f32; 16]>| -> [f32; 16] {
        let mut a = [0.0f32; 16];
        for b in &busy {
            for k in 0..16 {
                a[k] += v[*b][k];
            }
        }
        a
    };
    let (ks, ss, hs) = (avg_steps(&kick), avg_steps(&snare), avg_steps(&hat));
    let mut swing_votes = Vec::new();
    for b in busy.iter().take(64) {
        let (t0, beat) = (t_bar(*b), len_of(*b) / 4.0);
        for pair in 0..8 {
            let p0 = t0 + pair as f64 * beat / 2.0;
            let (fa, fe) =
                (((p0 + beat * 0.2) * SRF / HOP as f64) as usize, ((p0 + beat * 0.4) * SRF / HOP as f64) as usize);
            if fe >= hat_env.len() || fe <= fa {
                continue;
            }
            let (i, v) = (fa..fe).map(|i| (i, hat_env[i])).fold((fa, 0.0f32), |m, x| if x.1 > m.1 { x } else { m });
            if v > 0.0 {
                let off = (i * HOP) as f64 / SRF - p0;
                swing_votes.push((2.0 * off / beat).clamp(0.5, 0.7));
            }
        }
    }
    swing_votes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let swing = swing_votes.get(swing_votes.len() / 2).copied().unwrap_or(0.5) as f32;
    let swing = if swing < 0.53 { 0.5 } else { swing };
    // bass: offbeat eighths, sixteenth rolls, or long notes
    let on = (ks[0] + ks[4] + ks[8] + ks[12]) / 4.0;
    let off = (ks[2] + ks[6] + ks[10] + ks[14]) / 4.0;
    let busy16 = ks.iter().filter(|v| **v > 0.6 * ks.iter().cloned().fold(0.0, f32::max)).count();
    let bass_style = if busy16 >= 10 {
        "rolling"
    } else if off > 0.55 * on {
        "offbeat"
    } else if busy16 <= 2 {
        "sustain"
    } else {
        "syncopated"
    };

    Analysis {
        bpm: grid.bpm,
        key,
        key_confidence: (key_conf * 100.0).round() / 100.0,
        swing,
        pulse_clarity: (grid.clarity * 100.0).round() / 100.0,
        first_downbeat: (start * 1000.0).round() / 1000.0,
        bar_secs: bar_len,
        bars,
        chords: chords.iter().map(|c| c.symbol(key.flats())).collect(),
        numerals: chords.iter().map(|c| roman_of(c, &key)).collect(),
        progression_a: to_rn(&ca),
        progression_b: to_rn(&cb),
        bars_per_chord: bpc,
        sections,
        bar_chroma,
        // a recording no kit plays in has no drum habits
        bar_drums: if drummed { bar_drums(&[&kick, &snare, &hat]) } else { Vec::new() },
        kick_steps: if drummed { template(&ks) } else { vec![0.0; 16] },
        snare_steps: if drummed { template(&ss) } else { vec![0.0; 16] },
        hat_steps: if drummed { template(&hs) } else { vec![0.0; 16] },
        bass_style: bass_style.to_string(),
        steady: grid.steady,
        bar_starts,
        phrases,
    }
}

/// the tempo for a remake of a song at `bpm` in genre `g`: the same tempo when the genre plays it, else halved or
/// doubled when that brings it into the genre's range (a 124 BPM house song is a 62 BPM half-time beat), else the
/// nearest end of the range
pub fn remake_tempo(bpm: f64, g: &GenreProfile) -> f64 {
    let (lo, hi) = g.bpm;
    let outside = |b: f64| (lo - b).max(b - hi).max(0.0);
    let nearest = [bpm, bpm / 2.0, bpm * 2.0].into_iter().min_by(|a, b| outside(*a).total_cmp(&outside(*b)));
    nearest.unwrap_or(bpm).clamp(lo, hi).round()
}

/// a blueprint for genre `g` on the analysed song's skeleton (its tempo, key, swing, form, progressions, drum
/// habits and bass style); sounds and mix come from the genre. The tempo is `opts.bpm` when given, else
/// [`remake_tempo`].
pub fn to_blueprint(
    a: &Analysis,
    g: &GenreProfile,
    seed: u64,
    opts: &Options,
    lib: Option<&dyn Library>,
    source: &str,
) -> Blueprint {
    let o = Options { bpm: Some(opts.bpm.unwrap_or_else(|| remake_tempo(a.bpm, g))), key: Some(a.key), ..opts.clone() };
    let mut bp = compose::generate(g, seed, &o, lib);
    if !a.progression_a.is_empty() {
        bp.harmony.progressions.insert("A".into(), a.progression_a.clone());
        bp.harmony.progressions.insert(
            "B".into(),
            if a.progression_b.is_empty() { a.progression_a.clone() } else { a.progression_b.clone() },
        );
        bp.harmony.bars_per_chord = a.bars_per_chord;
    }
    // the form below plays A and B only: a chorus of the generator's own would be left unplayed
    bp.harmony.progressions.remove("C");
    bp.swing = a.swing;
    let drums: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    let parts: Vec<String> = bp.parts.iter().map(|p| p.name.clone()).collect();
    let loops: Vec<String> = bp.loops.iter().map(|l| l.name.clone()).collect();
    let has = |v: &Vec<String>, n: &str| v.iter().any(|x| x == n);
    let mut form = Vec::new();
    for (i, s) in a.sections.iter().enumerate() {
        let mut layers: Vec<String> = Vec::new();
        if s.kick && has(&drums, "kick") {
            layers.push("kick".into());
        }
        for d in &drums {
            let hat_like = matches!(d.as_str(), "hat" | "ohat" | "shaker" | "ride");
            let back = matches!(d.as_str(), "clap" | "snare");
            if (hat_like && s.hats) || (back && s.kick && s.energy >= 0.5) || (d == "perc" && s.energy >= 0.6) {
                layers.push(d.clone());
            }
        }
        if s.bass && has(&parts, "bass") && s.energy >= 0.45 {
            layers.push("bass".into());
        }
        for p in &parts {
            let take = match p.as_str() {
                "chords" | "keys" => s.energy >= 0.4,
                "pad" => s.energy < 0.75 || s.kind == SectionKind::Break,
                "lead" => s.energy >= 0.8 || s.kind == SectionKind::Break,
                "arp" => s.energy >= 0.9,
                "acid" => s.energy >= 0.9,
                _ => false,
            };
            if take && !layers.contains(p) {
                layers.push(p.clone());
            }
        }
        for l in &loops {
            if s.energy >= 0.6 {
                layers.push(l.clone());
            }
        }
        if layers.is_empty() {
            layers.push(parts.first().cloned().unwrap_or_else(|| "kick".into()));
        }
        let bars = ((s.bars as f64 / 4.0).round().max(1.0) * 4.0) as u32;
        let next_drop = a.sections.get(i + 1).is_some_and(|n| n.kind == SectionKind::Drop);
        form.push(SectionSpec {
            name: format!("{}{}", s.label.word(), i + 1),
            kind: s.kind,
            bars,
            energy: s.energy,
            prog: if matches!(s.kind, SectionKind::Break | SectionKind::Intro | SectionKind::Outro) {
                "B".into()
            } else {
                "A".into()
            },
            layers,
            variant: if s.energy < 0.5 { "sparse".into() } else { "main".into() },
            filter_in: s.kind == SectionKind::Intro,
            build: s.kind == SectionKind::Build || (next_drop && s.kind != SectionKind::Drop),
            build_style: Default::default(),
            build_amount: 0.5,
            build_bars: None,
            gap: next_drop && s.kind != SectionKind::Drop,
            gap_beats: 1.0,
            impact: s.kind == SectionKind::Drop && i > 0 && a.sections[i - 1].kind != SectionKind::Drop,
            fills: matches!(s.kind, SectionKind::Drop | SectionKind::Groove),
            takes: Default::default(),
        });
    }
    if !form.is_empty() {
        bp.form = form;
    }
    let mut r = crate::rng::Rng::new(seed ^ 0xa11a);
    for (lane, steps) in
        [("kick", &a.kick_steps), ("clap", &a.snare_steps), ("snare", &a.snare_steps), ("hat", &a.hat_steps)]
    {
        if let Some(d) = bp.drums.iter_mut().find(|d| d.name == lane)
            && steps.iter().any(|v| *v > 0.0)
        {
            let t = LaneTemplate { steps: steps.clone(), vel: (0.7, 1.0), ghost: 0.0, rolls: 0.0, vary: 0.2 };
            d.patterns.insert("main".into(), rhythm::generate(&t, d.pattern_bars, &mut r));
        }
    }
    if let Some(bass) = bp.parts.iter_mut().find(|p| p.name == "bass")
        && g.parts.iter().any(|p| p.name == "bass" && p.styles.contains(&a.bass_style))
    {
        bass.style = a.bass_style.clone();
    }
    bp.lineage = vec![format!(
        "remade from {source}: {:.0} BPM, {}, {} sections, progression {}",
        a.bpm,
        a.key.name(),
        a.sections.len(),
        a.progression_a.join(" ")
    )];
    bp
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Label, SectionKind, analyze, analyze_with, beat_change_of, remake_tempo};
    use crate::audio::Stereo;
    use crate::config::SRF;
    use crate::genres;
    use crate::stems::Stems;

    #[test]
    fn a_beat_that_loops_one_bar_never_changes_and_one_of_two_bars_changes_bar_to_bar_but_not_four_on() {
        // kick on 1 and 3, snare on 2 and 4, hats on the eighths; the second bar a kick before the third beat instead
        let bar = |kicks: &[usize]| {
            let mut b = [[0.0f32; 16]; 3];
            kicks.iter().for_each(|&k| b[0][k] = 1.0);
            [4, 12].into_iter().for_each(|k| b[1][k] = 0.9);
            (0..16).step_by(2).for_each(|k| b[2][k] = 0.7);
            b
        };
        let (a, b) = (bar(&[0, 8]), bar(&[0, 6]));
        let looped = vec![a; 16];
        let same = beat_change_of(&looped).unwrap();
        assert_eq!((same.next, same.four, same.turns), (0.0, 0.0, 0.0));
        // two bars of 12 hits each that move one kick: 2 of the 13 hits in either differ
        let two: Vec<_> = (0..16).map(|i| if i % 2 == 0 { a } else { b }).collect();
        let changed = beat_change_of(&two).unwrap();
        assert!((changed.next - 2.0 / 13.0).abs() < 1e-9, "{changed:?}");
        assert_eq!((changed.four, changed.turns), (0.0, 0.0), "four bars on, the same bar");
        // a hit at under half the lane's strength is no hit, and too few bars with a kit tell nothing
        let soft: Vec<_> = (0..16).map(|i| if i % 2 == 0 { a } else { [[0.4; 16]; 3] }).collect();
        assert!(beat_change_of(&soft).is_none(), "only every other bar has a kit: too few pairs");
        assert!(beat_change_of(&looped[..8]).is_none());
    }

    #[test]
    fn a_section_is_cut_before_its_own_pickup_not_before_a_line_of_the_section_before() {
        // bars of 2 s; a line of the section before sung from 4.6 s to 5.6 s, then the next section's pickup from
        // 5.7 s, running over its bar line at 6 s
        let bar_starts: Vec<f64> = (0..8).map(|b| b as f64 * 2.0).collect();
        let span = |start: f64, end: f64| crate::cuts::Span { start, end };
        let lines = [span(4.6, 5.6), span(5.7, 7.0)];
        let sung = crate::cuts::Sung { words: lines.to_vec(), phrases: lines.to_vec() };
        let mut voice = Stereo::zeros((16.0 * SRF) as usize);
        for l in lines {
            for i in (l.start * SRF) as usize..(l.end * SRF) as usize {
                let v = ((std::f64::consts::TAU * 220.0 * i as f64 / SRF).sin() * 0.3) as f32;
                (voice.l[i], voice.r[i]) = (v, v);
            }
        }
        let cut = super::cut_at(3, &bar_starts, Some((&sung, &voice)));
        assert!(cut > 5.6 && cut < 5.7, "before the pickup, after the line before: {cut}");
    }

    /// the form of `pop_song`: each section, what it is and its bars
    pub(crate) const POP_FORM: [(Label, usize); 10] = [
        (Label::Intro, 4),
        (Label::Verse, 8),
        (Label::PreChorus, 6),
        (Label::Chorus, 8),
        (Label::Verse, 8),
        (Label::PreChorus, 6),
        (Label::Chorus, 8),
        (Label::Bridge, 8),
        (Label::Chorus, 8),
        (Label::Outro, 4),
    ];

    /// A song of known form at 120 BPM in A minor, its stems apart (`POP_FORM`): an intro of 4 bars, a verse of 8, a
    /// pre-chorus of 6 that builds (a snare roll that thickens and noise that rises), a chorus of 8 whose phrases are
    /// sung from the beat before their bars, again, a bridge of 8, a last chorus and an outro of 4 (its verses with a
    /// backbeat, as a pop song's). Each kind of
    /// section plays chords of its own (sine triads, a bar each), drums and a bass of its own, and a voice sings a
    /// melody of its own (a tone with vibrato, a word a beat) in all but the intro and outro.
    pub(crate) fn pop_song() -> (Stereo, Stems) {
        let (bar, beat) = (2.0f64, 0.5f64);
        let bars: usize = POP_FORM.iter().map(|s| s.1).sum();
        let n = (bars as f64 * bar * SRF) as usize;
        let (mut other, mut vocals, mut drums, mut bass) =
            (Stereo::zeros(n), Stereo::zeros(n), Stereo::zeros(n), Stereo::zeros(n));
        let hz = |m: f64| 440.0 * 2f64.powf((m - 69.0) / 12.0);
        let mut noise = crate::rng::Rng::new(11);
        let put = |x: &mut Stereo, i: usize, v: f64| {
            if i < x.len() {
                x.l[i] += v as f32;
                x.r[i] += v as f32;
            }
        };
        let mut start = 0usize;
        for &(label, len) in &POP_FORM {
            // a bar's chord: root and whether minor
            let chords: &[(f64, bool)] = match label {
                Label::Intro | Label::Outro => &[(57.0, true), (53.0, false)],
                Label::Verse => &[(57.0, true), (53.0, false), (48.0, false), (55.0, false)],
                Label::PreChorus => {
                    &[(50.0, true), (50.0, true), (53.0, false), (53.0, false), (52.0, false), (52.0, false)]
                }
                Label::Chorus => &[(53.0, false), (55.0, false), (48.0, false), (57.0, true)],
                _ => &[(50.0, true), (57.0, true), (50.0, true), (52.0, false)],
            };
            let level = match label {
                Label::Chorus => 0.09,
                Label::Intro | Label::Outro | Label::Bridge => 0.05,
                _ => 0.07,
            };
            for b in 0..len {
                let (root, minor) = chords[b % chords.len()];
                let t0 = (start + b) as f64 * bar;
                let (i0, i1) = ((t0 * SRF) as usize, ((t0 + bar) * SRF) as usize);
                let rise = if label == Label::PreChorus { (b as f64 + 1.0) / len as f64 } else { 0.0 };
                for i in i0..i1 {
                    let t = i as f64 / SRF;
                    let mut v = 0.0;
                    for m in [root, root + if minor { 3.0 } else { 4.0 }, root + 7.0] {
                        v += level * (std::f64::consts::TAU * hz(m) * t).sin();
                    }
                    // the pre-chorus's noise rises into the chorus
                    v += 0.12 * rise * rise * noise.range(-1.0, 1.0);
                    put(&mut other, i, v);
                    // the bass on the root, an octave and more down, where the kit plays
                    if matches!(label, Label::Verse | Label::PreChorus | Label::Chorus) {
                        let lvl = if label == Label::Chorus { 0.25 } else { 0.15 };
                        put(&mut bass, i, lvl * (std::f64::consts::TAU * hz(root - 12.0) * t).sin());
                    }
                }
                // the drums, beat by beat: a kick, a snare and hats, each a short burst
                let hit = |x: &mut Stereo, at: f64, len: f64, f: Option<f64>, lvl: f64, rng: &mut crate::rng::Rng| {
                    let (a, e) = ((at * SRF) as usize, ((at + len) * SRF) as usize);
                    for i in a..e {
                        let u = (i - a) as f64 / SRF;
                        let env = 1.0 - u / len;
                        let v = match f {
                            Some(f) => (std::f64::consts::TAU * f * u).sin(),
                            None => rng.range(-1.0, 1.0),
                        };
                        put(x, i, lvl * env * v);
                    }
                };
                for k in 0..16 {
                    let at = t0 + k as f64 * beat / 4.0;
                    let (on_beat, eighth) = (k % 4 == 0, k % 2 == 0);
                    match label {
                        Label::Verse => {
                            if k == 0 || k == 8 {
                                hit(&mut drums, at, 0.12, Some(55.0), 0.5, &mut noise);
                            }
                            if k == 4 || k == 12 {
                                hit(&mut drums, at, 0.08, None, 0.2, &mut noise);
                            }
                            if eighth {
                                hit(&mut drums, at, 0.02, None, 0.06, &mut noise);
                            }
                        }
                        Label::PreChorus => {
                            if k == 0 {
                                hit(&mut drums, at, 0.12, Some(55.0), 0.5, &mut noise);
                            }
                            // a roll of eighths, then sixteenths, louder bar by bar
                            if eighth || b * 2 >= len {
                                hit(&mut drums, at, 0.06, None, 0.08 + 0.25 * rise, &mut noise);
                            }
                        }
                        Label::Chorus => {
                            if on_beat {
                                hit(&mut drums, at, 0.12, Some(55.0), 0.6, &mut noise);
                            }
                            if k == 4 || k == 12 {
                                hit(&mut drums, at, 0.08, None, 0.3, &mut noise);
                            }
                            hit(&mut drums, at, 0.02, None, 0.08, &mut noise);
                        }
                        Label::Bridge if on_beat => hit(&mut drums, at, 0.02, None, 0.05, &mut noise),
                        _ => {}
                    }
                }
            }
            // the voice: phrases of a word a beat, each section's melody its own; a chorus's phrases start on the beat
            // before their bars
            let (every, words, from, notes): (usize, usize, f64, &[f64]) = match label {
                Label::Verse => (2, 6, 0.0, &[64.0, 65.0, 67.0, 65.0, 64.0, 62.0]),
                Label::PreChorus => (2, 6, 0.0, &[65.0, 67.0, 69.0, 69.0, 71.0, 72.0]),
                Label::Chorus => (2, 7, -1.0, &[72.0, 76.0, 74.0, 72.0, 71.0, 72.0, 69.0]),
                Label::Bridge => (4, 6, 0.0, &[62.0, 62.0, 60.0, 62.0, 64.0, 62.0]),
                _ => (0, 0, 0.0, &[]),
            };
            if every > 0 {
                let loud = if label == Label::Chorus { 0.35 } else { 0.25 };
                for p in (0..len).step_by(every) {
                    let t_p = (start + p) as f64 * bar + from * beat;
                    for w in 0..words {
                        let (w0, w1) = (t_p + w as f64 * beat, t_p + w as f64 * beat + 0.44);
                        let f0 = hz(notes[w % notes.len()]);
                        let mut phase = 0.0f64;
                        for i in (w0 * SRF) as usize..(w1 * SRF) as usize {
                            let u = i as f64 / SRF - w0;
                            let vib = 1.0 + 0.017 * (std::f64::consts::TAU * 5.5 * u).sin();
                            phase += std::f64::consts::TAU * f0 * vib / SRF;
                            let env = (u / 0.015).min(1.0) * ((0.44 - u) / 0.03).clamp(0.0, 1.0);
                            let v = env * loud * (phase.sin() + 0.5 * (2.0 * phase).sin() + 0.25 * (3.0 * phase).sin());
                            put(&mut vocals, i, v);
                        }
                    }
                }
            }
            start += len;
        }
        let stems = Stems { vocals, drums, bass, other };
        let mut mix = stems.music();
        mix.add_at(&stems.drums, 0, 1.0, 1.0);
        (mix, stems)
    }

    #[test]
    fn a_songs_sections_are_its_own_found_to_the_bar_named_and_grouped_by_what_they_play() {
        let (mix, stems) = pop_song();
        let a = analyze_with(&mix, Some(&stems), None);
        let heard: Vec<(usize, usize, &str, String)> =
            a.sections.iter().map(|s| (s.start_bar, s.bars, s.label.name(), super::group_letter(s.group))).collect();
        assert!(
            (a.bpm - 120.0).abs() < 1.0 && a.first_downbeat.abs() < 0.05,
            "{} BPM from {}",
            a.bpm,
            a.first_downbeat
        );
        // every boundary to the bar, the pre-chorus's 6 bars kept
        let mut want = vec![0];
        for (_, len) in POP_FORM {
            want.push(want.last().unwrap() + len);
        }
        let starts: Vec<usize> = a.sections.iter().map(|s| s.start_bar).chain([a.bars]).collect();
        assert_eq!(starts, want, "{heard:?}");
        let labels: Vec<Label> = a.sections.iter().map(|s| s.label).collect();
        assert_eq!(labels, POP_FORM.iter().map(|s| s.0).collect::<Vec<_>>(), "{heard:?}");
        // the verses play the same music, as do the pre-choruses and the choruses, and the bridge its own
        let g: Vec<usize> = a.sections.iter().map(|s| s.group).collect();
        assert!(g[1] == g[4] && g[2] == g[5] && g[3] == g[6] && g[6] == g[8], "{heard:?}");
        assert!([g[1], g[2], g[3]].iter().all(|&x| x != g[7]) && g[1] != g[3] && g[2] != g[3], "{heard:?}");
        // a chorus plays as the song's hook, and each pre-chorus builds into one over its bars
        assert!(a.sections.iter().filter(|s| s.label == Label::Chorus).all(|s| s.kind == SectionKind::Hook));
        for i in [2, 5] {
            assert!(a.sections[i].build_bars >= 3, "{:?}", a.sections[i]);
        }
        assert_eq!(a.sections[3].build_bars, 0, "a chorus leads into a quieter verse");
        // a chorus is cut before the word sung on the beat before it, a verse on its bar line
        for i in [3, 6, 8] {
            let (cut, line) = (a.sections[i].cut, a.bar_start(a.sections[i].start_bar));
            assert!(cut < line - 0.3 && cut > line - 0.6, "chorus {i} cut at {cut:.3} s, its bar at {line:.3} s");
        }
        assert!((a.sections[1].cut - a.bar_start(4)).abs() < 1e-9);
        // its phrases to the beat: the first verse's first sung from bar 5's first beat, the first chorus's from bar
        // 18's last
        assert_eq!(a.phrases.first().map(super::SungPhrase::said).as_deref().map(|s| &s[..3]), Some("5.1"));
        assert!(a.phrases.iter().any(|p| p.said().starts_with("18.4")), "{:?}", a.phrases);
        assert!(a.text().contains("bars 13-18  pre-chorus"), "{}", a.text());
    }

    #[test]
    fn without_its_voice_apart_a_song_still_has_its_own_sections_and_its_chorus() {
        let (mix, _) = pop_song();
        let a = analyze(&mix, None);
        // in seconds: without the kit apart, the tracker may count this song's bars at half its tempo
        let heard: Vec<(f64, &str)> = a.sections.iter().map(|s| (a.bar_start(s.start_bar), s.label.name())).collect();
        let starts: Vec<f64> = POP_FORM
            .iter()
            .scan(0.0, |t, s| {
                let at = *t;
                *t += s.1 as f64 * 2.0;
                Some(at)
            })
            .collect();
        for t in &starts {
            assert!(heard.iter().any(|(s, _)| (s - t).abs() < 0.3), "nothing starts at {t} s: {heard:?}");
        }
        let choruses: Vec<f64> = heard.iter().filter(|h| h.1 == "chorus").map(|h| h.0).collect();
        let want: Vec<f64> =
            POP_FORM.iter().zip(&starts).filter(|(s, _)| s.0 == Label::Chorus).map(|(_, t)| *t).collect();
        assert_eq!(choruses.len(), want.len(), "{heard:?}");
        assert!(choruses.iter().zip(&want).all(|(c, w)| (c - w).abs() < 0.3), "{heard:?}");
    }

    #[test]
    fn a_steady_recording_is_cut_into_sections_and_none_of_them_is_a_drop() {
        // 96 bars at 120 BPM of the same: a kick on every beat, a hat on every offbeat, a chord held all through
        let sr = crate::config::SR as usize;
        let beat = sr / 2;
        let mut x = Stereo::zeros(96 * 4 * beat);
        let (kick, hat) = (sr / 20, sr / 100);
        let mut phase = [0f64; 3];
        for i in 0..x.len() {
            let t = i % beat;
            let mut v = 0.0;
            if t < kick {
                v += (1.0 - t as f32 / kick as f32) * (std::f32::consts::TAU * 55.0 * t as f32 / sr as f32).sin() * 0.6;
            }
            if (t + beat / 2) % beat < hat {
                v += if i % 3 == 0 { 0.15 } else { -0.15 };
            }
            for (p, f) in phase.iter_mut().zip([220.0, 277.18, 329.63]) {
                *p += std::f64::consts::TAU * f / sr as f64;
                v += p.sin() as f32 * 0.08;
            }
            (x.l[i], x.r[i]) = (v, v);
        }
        let a = analyze(&x, None);
        let heard: Vec<(SectionKind, usize)> = a.sections.iter().map(|s| (s.kind, s.bars)).collect();
        assert!(a.sections.len() >= 2 && a.sections.iter().all(|s| s.bars <= 32), "{} bars: {heard:?}", a.bars);
        assert!(a.sections.iter().all(|s| s.kind != SectionKind::Drop), "{heard:?}");
        // as even as the phrases allow: no scrap of a section beside a long one
        let (short, long) = a.sections.iter().fold((usize::MAX, 0), |(lo, hi), s| (lo.min(s.bars), hi.max(s.bars)));
        assert!(long <= 2 * short, "{heard:?}");
    }

    #[test]
    fn a_remake_plays_at_a_tempo_its_genre_plays() {
        let (lofi, house, trap) =
            (genres::get("lofi").unwrap(), genres::get("house").unwrap(), genres::get("trap").unwrap());
        assert_eq!(remake_tempo(80.0, &lofi), 80.0, "a tempo the genre plays stays");
        assert_eq!(remake_tempo(160.0, &lofi), 80.0, "half time");
        assert_eq!(remake_tempo(70.0, &trap), 140.0, "double time");
        // 124 BPM house as lofi: its half, 62, is the nearest, and the slowest lofi is 70
        assert_eq!(remake_tempo(124.0, &lofi), 70.0);
        assert_eq!(remake_tempo(140.0, &house), 126.0);
    }
}
