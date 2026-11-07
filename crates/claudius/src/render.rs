//! The renderer: a project becomes stems, buses and a mastered stereo file.
//!
//! Per track (in parallel): play the notes or clips through its sound (sampler, synth engine, transition effect,
//! warped loop) with swing and humanisation, run its insert chain, balance it to its role's target loudness, duck it
//! under the kick, pan it, send it to the reverb and delay returns. Buses get their own chains (with the composer's
//! filter automation), the master chain glues the sum, and the loudness stage brings it to the genre's LUFS target
//! under a true-peak ceiling.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

use crate::audio::{self, Stereo, db_to_gain, midi_to_hz, repitch};
use crate::config::SRF;
use crate::fx::{self, Ctx, Effect};
use crate::loudness;
use crate::model::{Project, Sound, Track};
use crate::rng::{Rng, hash_str};
use crate::synth::voice::{self, NoteEv};
use crate::synth::{drums, fxsounds};
use crate::theory::rhythm::swing;
use crate::theory::{Lock, Note};

/// target loudness (LUFS while playing) of each role before the master stage
pub fn target_level(role: &str) -> f32 {
    match role {
        "kick" => -12.5,
        "snare" => -13.5,
        "clap" => -14.5,
        "hat" => -17.5,
        "ohat" => -18.5,
        "shaker" => -21.5,
        "perc" => -19.5,
        "ride" => -20.5,
        "tom" => -18.0,
        "crash" => -20.0,
        "bass" => -14.5,
        "chords" => -18.0,
        "keys" => -17.0,
        "pad" => -21.5,
        "lead" => -18.0,
        "arp" => -20.5,
        "top" => -20.0,
        "drums" => -14.5,
        "vocal" => -15.0,
        "sample" => -18.0,
        "synth" => -18.0,
        "texture" => -27.0,
        "riser" => -19.0,
        "impact" => -16.0,
        "downlifter" => -22.0,
        "reverse" => -21.0,
        _ => -19.0,
    }
}

fn is_drum_role(role: &str) -> bool {
    matches!(role, "kick" | "snare" | "clap" | "hat" | "ohat" | "shaker" | "perc" | "ride" | "tom" | "crash")
}

type ShotCache = Mutex<HashMap<String, Arc<Stereo>>>;

fn shots() -> &'static ShotCache {
    static C: OnceLock<ShotCache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// a sound as the shot cache knows it: its own words, and which audio a page handed over for its path
fn shot_name(sound: &Sound) -> String {
    let version = match sound {
        Sound::Sample { path, .. } => audio::remembered_version(path),
        _ => None,
    };
    format!("{}|{version:?}", serde_json::to_string(sound).unwrap_or_default())
}

/// the one-shot buffer of a drum or sample sound, transposed by `semis` (cached)
fn one_shot(sound: &Sound, semis: i32, seed: u64) -> Result<Arc<Stereo>> {
    let key = format!("{}|{semis}|{seed}", shot_name(sound));
    if let Some(x) = shots().lock().unwrap().get(&key) {
        return Ok(x.clone());
    }
    let base_key = format!("{}|0|{seed}", shot_name(sound));
    let base = if semis != 0 { shots().lock().unwrap().get(&base_key).cloned() } else { None };
    let base = match base {
        Some(b) => b,
        None => {
            let x = match sound {
                Sound::Drum { voice, params } => drums::render(*voice, params, SRF, seed),
                Sound::Sample { path, start, end, .. } => {
                    let x = audio::load(std::path::Path::new(path))?;
                    let a = audio::secs_to_samples(*start);
                    let b = end.map_or(x.len(), audio::secs_to_samples);
                    let mut s = x.slice(a, b);
                    let n = s.len();
                    s.fade(32.min(n), 256.min(n));
                    s
                }
                _ => Stereo::zeros(1),
            };
            let b = Arc::new(x);
            shots().lock().unwrap().insert(base_key, b.clone());
            b
        }
    };
    if semis == 0 {
        return Ok(base);
    }
    let x = Arc::new(repitch(&base, semis as f64));
    shots().lock().unwrap().insert(key, x.clone());
    Ok(x)
}

/// the one-shot a drum stroke plays: its lane's, with what its step locks worked in (a synth drum's own decay and
/// tone; a sample shortened and darkened or brightened), cached as the lane's is
fn locked_shot(sound: &Sound, semis: i32, seed: u64, lock: &Lock) -> Result<Arc<Stereo>> {
    if lock.decay == 1.0 && lock.tone == 0.0 {
        return one_shot(sound, semis, seed);
    }
    if let Sound::Drum { voice, params } = sound {
        let mut p = *params;
        p.decay *= lock.decay;
        p.tone = (p.tone + 0.5 * lock.tone).clamp(0.0, 1.0);
        return one_shot(&Sound::Drum { voice: *voice, params: p }, semis, seed);
    }
    let key = format!("{}|{semis}|{seed}|{:.3}|{:.3}", shot_name(sound), lock.decay, lock.tone);
    if let Some(x) = shots().lock().unwrap().get(&key) {
        return Ok(x.clone());
    }
    let mut x = (*one_shot(sound, semis, seed)?).clone();
    shape(&mut x, lock.decay, lock.tone);
    let x = Arc::new(x);
    shots().lock().unwrap().insert(key, x.clone());
    Ok(x)
}

/// A sample's decay and tone moved: under a decay of 1, a falling curve over its length that takes it to e^-3 at
/// half the decay and on down; a tone below 0 blends towards it low-passed at 2 kHz, above 0 adds what is above that.
fn shape(x: &mut Stereo, decay: f32, tone: f32) {
    let n = x.len().max(1) as f32;
    let k = 3.0 * (1.0 / decay.clamp(0.1, 1.0) - 1.0);
    let a = 1.0 - (-std::f32::consts::TAU * 2_000.0 / SRF as f32).exp();
    for ch in x.channels_mut() {
        let mut low = 0f32;
        for (i, v) in ch.iter_mut().enumerate() {
            low += a * (*v - low);
            let toned = if tone < 0.0 { *v + -tone * (low - *v) } else { *v + tone * (*v - low) };
            *v = toned * (-k * i as f32 / n).exp();
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TrackReport {
    pub name: String,
    pub role: String,
    pub sound: String,
    pub bus: String,
    pub notes: usize,
    pub clips: usize,
    pub level_lufs: Option<f64>,
    pub gain_db: f32,
    /// how a recording on it keeps the song's time (a loop's or a take's; none for notes, or for under 8 beats of
    /// sound)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<KeptTime>,
}

/// How a track of recorded audio keeps the song's time, as its onsets say: how tightly they gather on the song's
/// sixteenths, against how tightly they gather on those of the tempo near it they keep best, and how far from their
/// place they wander. Its onsets, and not beats tracked in it, so that a tracker's slip by half a beat is not taken
/// for the recording's.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KeptTime {
    /// how tightly its onsets gather on the song's sixteenths, 0 (anywhere) to 1 (all on one place of them)
    pub on_grid: f64,
    /// the tempo, from 0.7 to 1.4 times the song's, on whose sixteenths they gather most tightly, and how tightly
    pub own_bpm: f64,
    pub own: f64,
    /// how far, on the song's sixteenths, they wander from where they gather over all of it: the most over any four
    /// bars, in milliseconds
    pub drift_ms: f64,
}

/// How `x`'s onsets keep the time of a song at `bpm` (`KeptTime`); none for under 32 of them, too few to tell.
pub fn kept_time(x: &Stereo, bpm: f64) -> Option<KeptTime> {
    let fps = SRF / crate::analysis::onsets::HOP as f64;
    let flux = crate::analysis::onsets::flux(&x.mono(), None);
    // its onsets: where the flux rises over its own mean around it (an eighth of a second each way), by how much
    let half = (0.125 * fps) as usize;
    let mut sums = vec![0.0f64; flux.len() + 1];
    for (i, v) in flux.iter().enumerate() {
        sums[i + 1] = sums[i] + *v as f64;
    }
    let onsets: Vec<(f64, f64)> = (0..flux.len())
        .filter_map(|i| {
            let (a, b) = (i.saturating_sub(half), (i + half + 1).min(flux.len()));
            let over = flux[i] as f64 - (sums[b] - sums[a]) / (b - a) as f64;
            (over > 0.0).then(|| (i as f64 / fps, over))
        })
        .collect();
    if onsets.len() < 32 {
        return None;
    }
    // how tightly onsets gather at one place on a grid of `period` seconds, and where on it (radians)
    let gather = |of: &[(f64, f64)], period: f64| {
        let (mut s, mut c, mut n) = (0.0, 0.0, 0.0);
        for (t, w) in of {
            let th = std::f64::consts::TAU * t / period;
            s += w * th.sin();
            c += w * th.cos();
            n += w;
        }
        ((s * s + c * c).sqrt() / f64::max(n, 1e-12), f64::atan2(s, c))
    };
    let sixteenth = |tempo: f64| 15.0 / tempo;
    let (on_grid, place) = gather(&onsets, sixteenth(bpm));
    // the tempo they keep best near the song's: a quarter of a percent at a time, then a fiftieth around the best
    let search = |best: (f64, f64), from: f64, to: f64, steps: usize| {
        (0..=steps)
            .map(|k| from * (to / from).powf(k as f64 / steps as f64))
            .map(|tempo| (tempo, gather(&onsets, sixteenth(tempo)).0))
            .fold(best, |b, c| if c.1 > b.1 { c } else { b })
    };
    let best = search((bpm, on_grid), 0.7 * bpm, 1.4 * bpm, 280);
    let best = search(best, best.0 * 0.9975, best.0 * 1.0025, 25);
    // each four bars' place against the whole's, where they hold onsets enough to have one
    let span = 16.0 * 60.0 / bpm;
    let windows = (onsets.last().map_or(0.0, |o| o.0) / span) as usize + 1;
    let weight: f64 = onsets.iter().map(|o| o.1).sum();
    let mut drift = 0.0f64;
    for w in 0..windows {
        let (from, to) = (w as f64 * span, (w + 1) as f64 * span);
        let these: Vec<(f64, f64)> = onsets.iter().copied().filter(|o| o.0 >= from && o.0 < to).collect();
        if these.iter().map(|o| o.1).sum::<f64>() < 0.25 * weight / windows as f64 {
            continue;
        }
        let d = gather(&these, sixteenth(bpm)).1 - place;
        let d = d - std::f64::consts::TAU * (d / std::f64::consts::TAU).round();
        drift = drift.max(d.abs() / std::f64::consts::TAU * sixteenth(bpm) * 1000.0);
    }
    Some(KeptTime { on_grid, own_bpm: best.0, own: best.1, drift_ms: drift })
}

pub struct Rendered {
    pub master: Stereo,
    pub stems: Vec<(String, Stereo)>,
    pub tracks: Vec<TrackReport>,
    pub lufs: f64,
    pub true_peak_db: f32,
    pub premaster_lufs: f64,
    /// what the master settled on for the whole song, which a window of it holds to (none: a file judged as it is)
    pub settled: Option<Settled>,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub stems: bool,
    pub tail_secs: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options { stems: false, tail_secs: 3.0 }
    }
}

#[derive(Clone, Copy)]
struct Timing {
    spb: f64,
    swing: f32,
    humanize: f32,
    /// the song's seed, which its drifts are drawn from
    seed: u64,
}

/// how far a hit drifts from the grid at `humanize` 1, in seconds, one standard deviation
const DRIFT_SECS: f64 = 0.015;

/// the most a hit drifts, in standard deviations
const DRIFT_MOST: f64 = 3.0;

/// whose drift the drum lanes share, as one player's limbs
const KIT: u64 = 0x6b69_7400_0000_0000;

/// how much of the drift a track takes: a kick holds the beat, hats and percussion wander the most, and a part's
/// player is looser than the kick and tighter than the hats
fn drift_share(role: &str) -> f64 {
    match role {
        "kick" => 0.5,
        "snare" | "clap" => 0.8,
        "hat" | "ohat" | "ride" | "crash" => 1.0,
        _ if is_drum_role(role) => 1.2,
        _ => 0.6,
    }
}

/// A value of unit variance at sixteenth `step` of a 1/f noise `seed` draws, the same each time it is asked for: Voss
/// and McCartney's, octaves of draws each held twice as long as the one before, summed, so that it wanders over every
/// span from a sixteenth to 32 bars.
fn pink(seed: u64, step: u64) -> f64 {
    const OCTAVES: u64 = 10;
    let draw = |octave: u64, at: u64| Rng::new(seed ^ (octave << 56) ^ at).normal();
    (0..OCTAVES).map(|k| draw(k, step >> k)).sum::<f64>() / (OCTAVES as f64).sqrt()
}

impl Timing {
    fn sample(&self, beat: f64, jitter: f64) -> i64 {
        ((swing(beat, self.swing) + jitter) * self.spb).round() as i64
    }

    /// the timing of track `t`: its own swing, when it has one
    fn of(self, t: &Track) -> Timing {
        Timing { swing: t.swing.unwrap_or(self.swing), ..self }
    }

    /// How far from the grid track `t` plays at `beat`, in beats: its own feel (`late_ms`), and with `humanize`, a
    /// drift as a player's. Hennig et al. (2011, "The nature and perception of fluctuations in human musical rhythms",
    /// PLOS ONE) found a player's deviations to be about 10 to 20 ms and correlated over tens of seconds, as a 1/f
    /// noise is, and listeners to prefer them to a humaniser's even, independent jitter: so a hit drifts with the ones
    /// around it, `DRIFT_SECS` at `humanize` 1 (as `drift_share` gives each role), the drum lanes together in part.
    fn feel(&self, t: &Track, beat: f64) -> f64 {
        let drift = if self.humanize > 0.0 {
            let step = (beat * 4.0).floor().max(0.0) as u64;
            let own = pink(self.seed ^ hash_str(&t.name), step);
            let z = if is_drum_role(&t.role) { 0.6 * pink(self.seed ^ KIT, step) + 0.8 * own } else { own };
            z.clamp(-DRIFT_MOST, DRIFT_MOST) * DRIFT_SECS * self.humanize as f64 * drift_share(&t.role)
        } else {
            0.0
        };
        (drift + t.late_ms as f64 / 1000.0) * SRF / self.spb
    }

    /// where track `t`'s note `n` starts: swung, moved by its feel, then by its step's nudge, and never before the song
    /// does (a hit ahead of the first beat plays on it, whole)
    fn onset(&self, t: &Track, n: &Note) -> i64 {
        self.sample(n.t, self.feel(t, n.t) + n.lock.map_or(0.0, |l| l.nudge)).max(0)
    }
}

/// onset sample positions of every track in each choke group
fn choke_onsets(p: &Project, tm: &Timing) -> BTreeMap<String, Vec<i64>> {
    let mut m: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for t in &p.tracks {
        if let Some(g) = &t.choke {
            let tm = tm.of(t);
            m.entry(g.clone()).or_default().extend(t.notes.iter().map(|n| tm.onset(t, n)));
        }
    }
    for v in m.values_mut() {
        v.sort();
        v.dedup();
    }
    m
}

/// a synth track's notes as its voices play them: where each starts (with its feel) and how long it holds, its
/// velocity humanised by the track's own draws, the first a track's source makes
fn synth_events(t: &Track, p: &Project, tm: &Timing) -> Vec<NoteEv> {
    let mut rng = Rng::new(p.seed ^ hash_str(&t.name));
    let human = tm.humanize as f64 * if is_drum_role(&t.role) { 1.0 } else { 0.6 };
    t.notes
        .iter()
        .map(|n| {
            let start = tm.sample(n.t, tm.feel(t, n.t)).max(0) as usize;
            let vel = (n.v as f64 * (1.0 + rng.normal() * human * 0.05)).clamp(0.05, 1.0) as f32;
            NoteEv { start, gate: ((n.d * tm.spb) as usize).max(16), pitch: (n.p + t.transpose) as f64, vel }
        })
        .collect()
}

fn render_source(
    t: &Track,
    p: &Project,
    tm: &Timing,
    len: usize,
    chokes: &BTreeMap<String, Vec<i64>>,
) -> Result<Stereo> {
    let mut rng = Rng::new(p.seed ^ hash_str(&t.name));
    let human = tm.humanize as f64 * if is_drum_role(&t.role) { 1.0 } else { 0.6 };
    let mut out = Stereo::zeros(len);
    let seed = p.seed ^ hash_str(&t.name);
    match &t.sound {
        Sound::Drum { .. } | Sound::Sample { .. } => {
            let root = if let Sound::Sample { root, .. } = &t.sound { *root } else { 60 };
            let group = t.choke.as_ref().and_then(|g| chokes.get(g));
            for n in &t.notes {
                let pos = tm.of(t).onset(t, n);
                let lock = n.lock.unwrap_or_default();
                let buf = locked_shot(&t.sound, n.p + t.transpose - root, seed, &lock)?;
                let vel = (n.v as f64 * (1.0 + rng.normal() * human * 0.05)).clamp(0.05, 1.0) as f32;
                let g = vel.powf(1.6);
                // a step's pan is a balance: the other side comes down
                let (gl, gr) = (g * (1.0 - lock.pan).min(1.0), g * (1.0 + lock.pan).min(1.0));
                let mut max_len = buf.len();
                if let Some(onsets) = group
                    && let Some(next) = onsets.iter().find(|&&o| o > pos + 32)
                {
                    max_len = max_len.min((next - pos) as usize);
                }
                if matches!(t.sound, Sound::Sample { .. }) && !is_drum_role(&t.role) {
                    max_len = max_len.min((n.d * tm.spb) as usize + (0.05 * SRF) as usize);
                }
                if max_len < buf.len() {
                    let mut s = buf.slice(0, max_len);
                    let f = (0.004 * SRF) as usize;
                    s.fade(0, f.min(max_len));
                    out.add_at(&s, pos, gl, gr);
                } else {
                    out.add_at(&buf, pos, gl, gr);
                }
            }
        }
        Sound::Synth { patch, .. } => {
            out = voice::render(patch, &synth_events(t, p, tm), len, SRF, p.bpm, seed);
        }
        Sound::Instrument { name, .. } => {
            let inst = claudius_packs::instrument(name, None)?;
            let notes: Vec<claudius_packs::NoteOn> = t
                .notes
                .iter()
                .map(|n| {
                    let vel = (n.v as f64 * (1.0 + rng.normal() * human * 0.05)).clamp(0.05, 1.0) as f32;
                    claudius_packs::NoteOn {
                        start: tm.sample(n.t, tm.feel(t, n.t)).max(0) as usize,
                        gate: ((n.d * tm.spb) as usize).max(16),
                        pitch: (n.p + t.transpose) as f64,
                        velocity: vel,
                    }
                })
                .collect();
            let cuts: Vec<usize> = t
                .choke
                .as_ref()
                .and_then(|g| chokes.get(g))
                .map(|o| o.iter().map(|&x| x.max(0) as usize).collect())
                .unwrap_or_default();
            let (l, r) = inst.render_choked(&notes, &cuts, len);
            out = Stereo { l, r };
        }
        Sound::Fx { fx } => {
            let root = midi_to_hz(48.0 + p.key.tonic as f64);
            for (k, n) in t.notes.iter().enumerate() {
                let secs = n.d * tm.spb / SRF;
                let s = match fx.as_str() {
                    "riser" => fxsounds::riser(secs, SRF, seed ^ k as u64, root),
                    "downlifter" => fxsounds::downlifter(secs, SRF, seed ^ k as u64),
                    "impact" => fxsounds::impact(SRF, seed ^ k as u64),
                    "reverse_crash" => fxsounds::reverse_crash(secs, SRF, seed ^ k as u64),
                    "sweep_down" => fxsounds::sweep(secs, SRF, seed ^ k as u64, false),
                    _ => fxsounds::sweep(secs, SRF, seed ^ k as u64, true),
                };
                let pos = (n.t * tm.spb).round() as i64;
                out.add_at(&s, pos, n.v, n.v);
            }
        }
        Sound::Loop { path, bpm, beats, .. } if t.retime => {
            // the loop at its own speed (its transpose resamples it, as a sampler's varispeed does), each slice of its
            // beats from the song's beat for it: cut short where the song is faster, a gap after it where slower
            let own = bpm * 2f64.powf(t.transpose as f64 / 12.0);
            let w = crate::warp::warp(path, *bpm, *beats, own, t.transpose, true, false)?;
            let wl = w.len().max(1);
            let beats_of = t.slice_beats.max(0.125);
            // the slices counted from the loop's beats, each from where it starts to the sample: added up as rounded,
            // they may come to more than the loop holds and lose its last
            let per = beats_of * 60.0 / own * SRF;
            let slice = per.round().max(1.0) as usize;
            let slot = (beats_of * tm.spb).round().max(1.0) as usize;
            let n_slices = ((*beats / beats_of).round() as usize).max(1);
            let (fade_in, fade_out) = ((0.001 * SRF) as usize, (0.003 * SRF) as usize);
            for c in &t.clips {
                let a = (c.t * tm.spb).round() as usize;
                let off = (c.offset * tm.spb).round() as usize;
                let b = (((c.t + c.d) * tm.spb).round() as usize).min(len);
                let g = db_to_gain(c.gain_db);
                for i in a..b {
                    let rel = i - a + off;
                    let (k, at) = (rel / slot, rel % slot);
                    let heard = slice.min(slot);
                    if at >= heard {
                        continue;
                    }
                    let idx = match t.chop.is_empty() {
                        true => k % n_slices,
                        false => t.chop[k % t.chop.len()].rem_euclid(n_slices as i32) as usize,
                    };
                    let edge = (at as f32 / fade_in as f32).min((heard - at) as f32 / fade_out as f32).min(1.0);
                    let src = ((idx as f64 * per).round() as usize + at).min(wl - 1);
                    out.l[i] += w.l[src] * g * edge;
                    out.r[i] += w.r[src] * g * edge;
                }
                let fi = ((c.fade_in * tm.spb) as usize).max(48).min(b - a);
                let fo = ((c.fade_out * tm.spb) as usize).max(96).min(b - a);
                for ch in out.channels_mut() {
                    audio::fade_slice(&mut ch[a..b], fi, fo);
                }
            }
        }
        Sound::Loop { path, bpm, beats, .. } => {
            let once = t.clips.iter().any(|c| c.once);
            let w = crate::warp::warp(path, *bpm, *beats, p.bpm, t.transpose, !once, t.role == "vocal")?;
            let wl = w.len().max(1);
            let beats_of = t.slice_beats.max(0.0625);
            let slice = (beats_of * tm.spb).round() as usize;
            let n_slices = ((*beats / beats_of).round() as usize).max(1);
            for c in &t.clips {
                let a = (c.t * tm.spb).round() as usize;
                let off = (c.offset * tm.spb).round() as usize;
                let mut b = (((c.t + c.d) * tm.spb).round() as usize).min(len);
                if c.once {
                    b = b.min(a + wl.saturating_sub(off));
                }
                let g = db_to_gain(c.gain_db);
                for i in a..b {
                    let rel = i - a + off;
                    let src = if t.chop.is_empty() {
                        rel % wl
                    } else {
                        let k = rel / slice;
                        let idx = t.chop[k % t.chop.len()].rem_euclid(n_slices as i32) as usize;
                        ((idx as f64 * beats_of * tm.spb).round() as usize + rel % slice) % wl
                    };
                    out.l[i] += w.l[src] * g;
                    out.r[i] += w.r[src] * g;
                }
                let fi = ((c.fade_in * tm.spb) as usize).max(48).min(b - a);
                let fo = ((c.fade_out * tm.spb) as usize).max(96).min(b - a);
                for ch in out.channels_mut() {
                    audio::fade_slice(&mut ch[a..b], fi, fo);
                }
            }
        }
    }
    Ok(out)
}

/// sidechain gain curve (1 = no ducking, 0 = fully ducked) from the kick's onsets
fn duck_curve(p: &Project, tm: &Timing, len: usize) -> Option<Vec<f32>> {
    let kick = p.tracks.iter().find(|t| t.role == "kick")?;
    let rel = (p.mix.duck_release.max(0.05) as f64 * tm.spb) as usize;
    let att = (0.003 * SRF) as usize;
    let mut env = vec![1.0f32; len];
    let tm = tm.of(kick);
    for n in &kick.notes {
        let pos = tm.onset(kick, n) as usize;
        let start = pos.saturating_sub(att);
        for i in start..(pos + rel).min(len) {
            let v = if i < pos {
                1.0 - (i - start) as f32 / att.max(1) as f32
            } else {
                let x = (i - pos) as f32 / rel as f32;
                1.0 - (1.0 - x).powi(2)
            };
            env[i] = env[i].min(v);
        }
    }
    Some(env)
}

/// What a render keeps of each track for the next one (`render_kept`): its sound through its effects and its drive,
/// before the mix levels, ducks and pans it, with the level the mix heard in it, under a key made of everything that
/// sound comes from. A track an edit left as it was (another track's notes, its own level or send, the master) is not
/// rendered again; one whose key has changed is, and what comes of it is the same either way.
#[derive(Default)]
pub struct Kept {
    tracks: HashMap<String, KeptTrack>,
}

struct KeptTrack {
    key: blake3::Hash,
    sound: Arc<Blocks>,
    level: Option<f64>,
    /// how it keeps the song's time (`timed`), once measured: a sound made elsewhere (`Kept::keep`) is measured here
    time: Option<Option<KeptTime>>,
}

/// how track `t` keeps a song at `bpm`'s time, from its sound (`kept_time`): a recording's only, a loop's or a take's
fn timed<S: std::borrow::Borrow<Stereo>>(t: &Track, sound: impl FnOnce() -> S, bpm: f64) -> Option<KeptTime> {
    matches!(t.sound, Sound::Loop { .. }).then(|| kept_time(sound().borrow(), bpm)).flatten()
}

/// A track's sound as a render keeps it: its samples in blocks of `BLOCK`, without the blocks that are silent all
/// through (a song's tracks rest about half of it), which read as silence. It holds what the sound holds, to the sample.
struct Blocks {
    len: usize,
    /// each block's left samples and then its right, the last block shorter
    blocks: Vec<Option<Box<[f32]>>>,
}

impl Blocks {
    fn of(x: &Stereo) -> Blocks {
        let len = x.len();
        let blocks = (0..len.div_ceil(BLOCK))
            .map(|b| {
                let r = b * BLOCK..((b + 1) * BLOCK).min(len);
                let (l, rr) = (&x.l[r.clone()], &x.r[r]);
                (l.iter().chain(rr).any(|v| *v != 0.0)).then(|| l.iter().chain(rr).copied().collect())
            })
            .collect();
        Blocks { len, blocks }
    }

    fn len(&self) -> usize {
        self.len
    }

    /// Its `k` samples from `from` on into `l` and `r`, silence where it keeps none; whether there were any but
    /// silence (when there were not, `l` and `r` are left as they were).
    fn read(&self, from: usize, k: usize, l: &mut [f32], r: &mut [f32]) -> bool {
        let first = from / BLOCK;
        let last = (from + k).div_ceil(BLOCK);
        if (first..last).all(|b| self.blocks.get(b).is_none_or(Option::is_none)) {
            return false;
        }
        let mut i = 0;
        while i < k {
            let (b, off) = ((from + i) / BLOCK, (from + i) % BLOCK);
            let take = (BLOCK - off).min(k - i);
            match self.blocks.get(b).and_then(|x| x.as_deref()) {
                Some(data) => {
                    let n = data.len() / 2;
                    l[i..i + take].copy_from_slice(&data[off..off + take]);
                    r[i..i + take].copy_from_slice(&data[n + off..n + off + take]);
                }
                None => {
                    l[i..i + take].fill(0.0);
                    r[i..i + take].fill(0.0);
                }
            }
            i += take;
        }
        true
    }

    /// the whole sound
    fn stereo(&self) -> Stereo {
        let mut x = Stereo::zeros(self.len);
        self.read(0, self.len, &mut x.l, &mut x.r);
        x
    }

    /// how many samples it holds, of both sides
    fn held(&self) -> usize {
        self.blocks.iter().flatten().map(|b| b.len()).sum()
    }
}

impl Kept {
    /// forgets every track (the sample packs changed under them)
    pub fn clear(&mut self) {
        self.tracks.clear();
    }

    /// the tracks it keeps
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// how many samples it holds, of every track and both sides (its tracks' silent blocks hold none)
    pub fn samples(&self) -> usize {
        self.tracks.values().map(|t| t.sound.held()).sum()
    }

    /// the tracks of `p` a render of it would make anew, each with the key it is kept under
    pub fn stale(&self, p: &Project, opts: &Options) -> Result<Vec<(String, TrackKey)>> {
        let s = Song::of(p, opts)?;
        Ok(p.tracks
            .iter()
            .map(|t| (t, track_key(p, t, s.len, &s.chokes)))
            .filter(|(t, key)| self.tracks.get(&t.name).is_none_or(|k| k.key != *key))
            .map(|(t, key)| (t.name.clone(), TrackKey(key)))
            .collect())
    }

    /// keeps `sound` and `level`, the track `name`'s as `track_sound` made them elsewhere, under `key`, for the renders
    /// after
    pub fn keep(&mut self, name: &str, key: TrackKey, sound: Stereo, level: Option<f64>) {
        let sound = Arc::new(Blocks::of(&sound));
        self.tracks.insert(name.to_string(), KeptTrack { key: key.0, sound, level, time: None });
    }
}

/// what a track's sound is kept under (`Kept::stale`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackKey(blake3::Hash);

/// The sound of the track `name` of `p` before the mix, as a render makes it, and the level the mix hears in it: for a
/// render elsewhere that keeps it (`Kept::keep`), so that tracks are made side by side where there are no threads (in
/// a browser's workers).
pub fn track_sound(p: &Project, opts: &Options, name: &str) -> Result<(Stereo, Option<f64>)> {
    let s = Song::of(p, opts)?;
    let t = p.tracks.iter().find(|t| t.name == name).with_context(|| format!("the song has no track {name}"))?;
    s.sound(t)
}

/// whether a track's sound can be made in parts side by side (`track_part`): a synth that plays its notes apart, not a
/// mono one, whose notes glide one into the next
pub fn splits(t: &Track) -> bool {
    matches!(&t.sound, Sound::Synth { patch, .. } if !patch.mono)
}

/// a track of `p` that splits (`splits`), by its name, and its patch
fn split_track<'a>(p: &'a Project, name: &str) -> Result<(&'a Track, &'a voice::Patch)> {
    let t = p.tracks.iter().find(|t| t.name == name).with_context(|| format!("the song has no track {name}"))?;
    match &t.sound {
        Sound::Synth { patch, .. } if !patch.mono => Ok((t, patch)),
        _ => anyhow::bail!("{name} is no synth that plays its notes apart, to make in parts"),
    }
}

/// Where each of at most `parts` parts of the track `name` begins and ends in the song's samples, the first from its
/// start and the last to the render's length, so that the notes that begin in each sound about as long as in any
/// other; and each part's share of the work of making the track: the notes that sound in it, each of which it renders
/// whole, against all of them (the shares come to more than 1 where a note sounds in two parts). Never a part twice,
/// none empty, and none no note sounds in, which joins the next (or, last, the one before): so fewer parts when the
/// notes begin in fewer places, and one for a track whose notes all begin together.
pub fn part_bounds(
    p: &Project,
    opts: &Options,
    name: &str,
    parts: usize,
) -> Result<Vec<(std::ops::Range<usize>, f64)>> {
    let s = Song::of(p, opts)?;
    let (t, patch) = split_track(p, name)?;
    let mut spans = voice::note_spans(patch, &synth_events(t, p, &s.tm), s.len, SRF);
    spans.sort_by_key(|(start, _)| *start);
    let total: f64 = spans.iter().map(|(_, l)| *l as f64).sum();
    let mut bounds = vec![0];
    let mut sum = 0.0;
    for (start, l) in &spans {
        if bounds.len() < parts.max(1) && sum >= total * bounds.len() as f64 / parts.max(1) as f64 {
            bounds.push((*start).max(*bounds.last().unwrap_or(&0)));
        }
        sum += *l as f64;
    }
    bounds.push(s.len);
    bounds.dedup();
    let share = |r: &std::ops::Range<usize>| {
        let sounding = spans.iter().filter(|(a, l)| *a < r.end && a + l > r.start).map(|(_, l)| *l as f64).sum::<f64>();
        if total > 0.0 { sounding / total } else { 0.0 }
    };
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    let mut from = 0;
    for w in bounds.windows(2) {
        if share(&(w[0]..w[1])) == 0.0 && w[1] < s.len {
            continue;
        }
        ranges.push(from..w[1]);
        from = w[1];
    }
    if ranges.len() > 1 && ranges.last().is_some_and(|r| share(r) == 0.0) {
        let last = ranges.pop().unwrap_or_default();
        if let Some(r) = ranges.last_mut() {
            r.end = last.end;
        }
    }
    if ranges.is_empty() {
        ranges.push(0..s.len);
    }
    Ok(ranges.into_iter().map(|r| (r.clone(), share(&r))).collect())
}

/// The samples `range` of the track `name`'s source, as a render makes them there (`voice::render_range`): a part of a
/// track that splits, which `track_joined` puts together with the others.
pub fn track_part(p: &Project, opts: &Options, name: &str, range: std::ops::Range<usize>) -> Result<Stereo> {
    let s = Song::of(p, opts)?;
    let (t, patch) = split_track(p, name)?;
    let range = range.start.min(s.len)..range.end.min(s.len);
    let seed = p.seed ^ hash_str(&t.name);
    Ok(voice::render_range(patch, &synth_events(t, p, &s.tm), s.len, range, SRF, p.bpm, seed))
}

/// The track `name`'s sound from its parts (`track_part`, one after the other from the song's start to the render's
/// length), through its effects and drive as a render makes it, and the level the mix hears in it: what `track_sound`
/// makes.
pub fn track_joined(p: &Project, opts: &Options, name: &str, parts: &[Stereo]) -> Result<(Stereo, Option<f64>)> {
    let s = Song::of(p, opts)?;
    let (t, _) = split_track(p, name)?;
    let n: usize = parts.iter().map(Stereo::len).sum();
    anyhow::ensure!(n == s.len, "the parts of {name} come to {n} samples, not the song's {}", s.len);
    let mut x = Stereo::zeros(s.len);
    let mut at = 0;
    for part in parts {
        x.l[at..at + part.len()].copy_from_slice(&part.l);
        x.r[at..at + part.len()].copy_from_slice(&part.r);
        at += part.len();
    }
    Ok(s.finish(t, x))
}

/// About how long a track of a song `secs` long takes to make here, in seconds, to share a song's tracks out: its
/// source, and its effects and drive all through the song (`chain_cost`, measured). A synth's source is the seconds its
/// voices sound with their release (a mono synth's one voice from its first note to the end of its last) times how long
/// a second of its patch takes around their pitch (`voice::cost_of`, measured); anything else's, the seconds its notes
/// and clips last times a little (a drum's hits, a recording's notes and a loop's clips cost little).
pub fn track_cost(t: &Track, bpm: f64, secs: f64) -> f64 {
    let beat = 60.0 / bpm.max(1.0);
    let source = match &t.sound {
        Sound::Synth { patch, .. } => {
            let release = patch.aenv.release_samples(SRF) as f64 / SRF;
            let voiced = if patch.mono {
                let first = t.notes.iter().map(|n| n.t).fold(f64::MAX, f64::min);
                let last = t.notes.iter().map(|n| n.t + n.d).fold(f64::MIN, f64::max);
                if t.notes.is_empty() { 0.0 } else { (last - first) * beat + release }
            } else {
                t.notes.iter().map(|n| n.d * beat + release).sum::<f64>()
            };
            // at the pitch its notes play around
            let mut pitches: Vec<i32> = t.notes.iter().map(|n| n.p + t.transpose).collect();
            pitches.sort_unstable();
            voiced * voice::cost_of(patch, pitches.get(pitches.len() / 2).copied().unwrap_or(60) as f64)
        }
        Sound::Drum { .. } => t.notes.len() as f64 * 2e-4,
        _ => {
            let beats = t.notes.iter().map(|n| n.d + 1.0).sum::<f64>()
                + t.clips.iter().map(|c| c.d.max(0.0) + 1.0).sum::<f64>();
            beats * beat * 1e-3
        }
    };
    source + secs * chain_cost(&t.mix.fx, t.mix.drive_db, bpm)
}

/// how long a second of sound takes to go through `chain` and a drive of `drive_db` here, in seconds: measured once for
/// each chain, on half a second of noise
fn chain_cost(chain: &[Effect], drive_db: f32, bpm: f64) -> f64 {
    static COSTS: OnceLock<Mutex<HashMap<String, f64>>> = OnceLock::new();
    let key = serde_json::to_string(&(chain, drive_db, bpm.round())).unwrap_or_default();
    let costs = COSTS.get_or_init(Default::default);
    if let Some(c) = costs.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return *c;
    }
    let n = (0.5 * SRF) as usize;
    let mut rng = Rng::new(1);
    let mut x = Stereo::zeros(n);
    for i in 0..n {
        (x.l[i], x.r[i]) = (rng.f32() - 0.5, rng.f32() - 0.5);
    }
    let ctx = Ctx::whole(SRF, bpm, 1, n);
    let t0 = crate::clock::Instant::now();
    fx::apply_chain(chain, &mut x, &ctx);
    if drive_db > 0.0 {
        let drive = Effect::Saturate { kind: fx::SatKind::Tanh, drive_db: drive_db.into(), mix: 1.0.into() };
        fx::apply_chain(std::slice::from_ref(&drive), &mut x, &ctx);
    }
    let cost = t0.elapsed().as_secs_f64() / 0.5;
    costs.lock().unwrap_or_else(|e| e.into_inner()).insert(key, cost);
    cost
}

/// what a track's sound before the mix comes from: the track, but for what the mix does after its drive, the song's
/// tempo, swing, humanisation, seed, key and chords (which a tuned voice is tuned to), the render's length, the onsets
/// that choke it, and the audio it plays as it is now (a file's size and time, or which audio a page handed over for
/// its path)
fn track_key(p: &Project, t: &Track, len: usize, chokes: &BTreeMap<String, Vec<i64>>) -> blake3::Hash {
    let mut t = t.clone();
    let m = &mut t.mix;
    (m.level_db, m.pan, m.reverb, m.delay, m.duck, m.mute, m.solo) = (0.0, 0.0, 0.0, 0.0, 0.0, false, false);
    m.bus = String::new();
    let mut h = blake3::Hasher::new();
    h.update(&serde_json::to_vec(&t).unwrap_or_default());
    h.update(&serde_json::to_vec(&(p.bpm, p.swing, p.humanize, p.seed, &p.key, &p.chords, len)).unwrap_or_default());
    if let Some(onsets) = t.choke.as_ref().and_then(|g| chokes.get(g)) {
        h.update(&serde_json::to_vec(onsets).unwrap_or_default());
    }
    if let Sound::Sample { path, .. } | Sound::Loop { path, .. } = &t.sound {
        h.update(format!("{:?}", audio::remembered_version(path)).as_bytes());
        if crate::config::FILES
            && let Ok(md) = std::fs::metadata(path)
        {
            let mtime = md.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok());
            h.update(format!("{}|{:?}", md.len(), mtime).as_bytes());
        }
    }
    h.finalize()
}

/// the samples of a track the mix scales, ducks and pans at a time
const BLOCK: usize = 4096;

pub fn render(p: &Project, opts: &Options) -> Result<Rendered> {
    render_kept(p, opts, &mut Kept::default())
}

/// `render`, drawing on what `kept` keeps of the tracks from the render before, and keeping this one's
pub fn render_kept(p: &Project, opts: &Options, kept: &mut Kept) -> Result<Rendered> {
    let t0 = crate::clock::Instant::now();
    let s = Song::of(p, opts)?;
    let sounds = s.sounds(kept)?;
    let mut out = s.mix(&sounds, 0..s.len, opts.stems);
    let premaster_lufs = loudness::integrated(&out.sum);
    if debug() {
        eprintln!("render: tracks + buses + returns in {:.2} s", t0.elapsed().as_secs_f64());
    }
    let mut master = s.master(out.sum, 0);
    if debug() {
        eprintln!(
            "render: master chain done at {:.2} s ({:.2} LUFS)",
            t0.elapsed().as_secs_f64(),
            loudness::integrated(&master)
        );
    }
    let m = finalize(&mut master, p.mix.master.lufs, p.mix.master.ceiling_db);
    if debug() {
        eprintln!("render: finalized at {:.2} s", t0.elapsed().as_secs_f64());
    }
    let end = master.active_len(-80.0).max((p.beats() * s.tm.spb) as usize).min(master.len());
    master.truncate(end);
    let fo = ((0.5 * SRF) as usize).min(end);
    master.fade(0, fo);
    let settled = Some(Settled { gain_db: m.gain_db, trim_db: m.trim_db, end, len: s.len });
    crate::metrics::observe(crate::metrics::Metric::Render, master.secs(), t0.elapsed().as_secs_f64());
    Ok(Rendered {
        master,
        stems: std::mem::take(&mut out.stems),
        tracks: out.tracks,
        lufs: m.lufs,
        true_peak_db: m.true_peak_db,
        premaster_lufs,
        settled,
    })
}

/// What the last whole render settled for the whole song, which a window of it holds to (`render_window`): the gain
/// the master's loudness search found and the trim that kept its true peak under the ceiling (both measured over the
/// whole song), where it ends, and how long it rendered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settled {
    pub gain_db: f64,
    pub trim_db: f32,
    pub end: usize,
    pub len: usize,
}

/// A window of a song: its samples from `start` on, as the whole song's render has them.
pub struct Window {
    pub start: usize,
    pub master: Stereo,
}

/// The song's `n` samples from `from` on as its whole render would make them, mixed from what `kept` keeps (a track an
/// edit changed rendered whole first, as `render_kept` would) and through the buses, returns and master from early
/// enough that every effect has settled as the whole song's would have (their automation, LFOs and noise where they
/// would be), at the master's gain and trim the last whole render settled (`settled`). So what comes out differs from
/// the whole song's render only by that gain: an edit that makes the song louder or quieter is levelled again by the
/// next whole render. None when the song is no longer as long as the one `settled` was rendered from.
pub fn render_window(
    p: &Project,
    opts: &Options,
    kept: &mut Kept,
    settled: &Settled,
    from: usize,
    n: usize,
) -> Result<Option<Window>> {
    let s = Song::of(p, opts)?;
    if s.len != settled.len {
        return Ok(None);
    }
    let sounds = s.sounds(kept)?;
    let from = from.min(settled.end);
    let to = (from + n).min(settled.end);
    // the effects after the tracks settle within their tails
    let a = s.mix_start(from.saturating_sub((s.settle_secs() * SRF) as usize));
    // the limiter looks ahead, and tape and the true-peak filters lag: a little after the window too
    let b = (to + (0.25 * SRF) as usize).min(s.len);
    let out = s.mix(&sounds, a..b, false);
    let mut master = s.master(out.sum, a);
    master.scale(db_to_gain(settled.gain_db as f32));
    let ceiling = p.mix.master.ceiling_db;
    fx::soft_clip(&mut master, ceiling - 0.1, 0.7, SRF);
    fx::limit(&mut master, ceiling - 0.5, 50.0, SRF);
    if settled.trim_db != 0.0 {
        master.scale(db_to_gain(settled.trim_db));
    }
    let mut w = master.slice(from - a, to - a);
    // the whole song's fade at its end
    let fo = ((0.5 * SRF) as usize).min(settled.end);
    for k in from.max(settled.end - fo)..to {
        let i = settled.end - 1 - k;
        let g = 0.5 - 0.5 * (std::f32::consts::PI * i as f32 / fo as f32).cos();
        if fo > 1 {
            w.l[k - from] *= g;
            w.r[k - from] *= g;
        }
    }
    Ok(Some(Window { start: from, master: w }))
}

/// A song as the renderer lays it out: its tracks' timing, its length with the tails, its chokes and its duck.
struct Song<'a> {
    p: &'a Project,
    tm: Timing,
    len: usize,
    chokes: BTreeMap<String, Vec<i64>>,
    duck: Option<Vec<f32>>,
}

/// what the tracks make on their way to the master: the sum of the buses and returns, each track's stem when asked,
/// and what the mix did to each track
struct Mixed {
    sum: Stereo,
    stems: Vec<(String, Stereo)>,
    tracks: Vec<TrackReport>,
}

type Made = (blake3::Hash, Arc<Blocks>, Option<f64>, Option<KeptTime>);

impl<'a> Song<'a> {
    fn of(p: &'a Project, opts: &Options) -> Result<Song<'a>> {
        if p.bars > crate::model::MAX_BARS || p.secs() > crate::model::MAX_SECONDS {
            anyhow::bail!("the song is too long to render ({} bars, {:.0} s)", p.bars, p.secs());
        }
        // a pattern with several sounds plays as a track for each, so the arranged song can have more than its blueprint
        crate::model::check_tracks(p.tracks.len(), p.secs()).map_err(anyhow::Error::msg)?;
        let tm = Timing { spb: SRF * 60.0 / p.bpm, swing: p.swing, humanize: p.humanize, seed: p.seed };
        let tail = opts.tail_secs.max(0.0)
            + p.tracks.iter().map(|t| fx::chain_tail_secs(&t.mix.fx, p.bpm)).fold(0.0, f64::max).min(6.0);
        let len = (p.beats() * tm.spb) as usize + (tail * SRF) as usize;
        let chokes = choke_onsets(p, &tm);
        let duck = duck_curve(p, &tm, len);
        Ok(Song { p, tm, len, chokes, duck })
    }

    /// what an effect on `name` works in, the buffer beginning `start` samples into the song
    fn ctx(&self, name: &str, start: usize) -> Ctx {
        Ctx { start, ..Ctx::whole(SRF, self.p.bpm, self.p.seed ^ hash_str(name), self.len) }
    }

    /// a track's sound before the mix, through its effects and its drive, and the level the mix hears in it
    fn sound(&self, t: &Track) -> Result<(Stereo, Option<f64>)> {
        let ts = crate::clock::Instant::now();
        let made = self.finish(t, render_source(t, self.p, &self.tm, self.len, &self.chokes)?);
        if debug() {
            eprintln!("render: track {} in {:.2} s", t.name, ts.elapsed().as_secs_f64());
        }
        Ok(made)
    }

    /// a track's source (`render_source`) through its effects and its drive, and the level the mix hears in it
    fn finish(&self, t: &Track, mut x: Stereo) -> (Stereo, Option<f64>) {
        fx::apply_chain(&t.mix.fx, &mut x, &self.ctx(&t.name, 0));
        if t.mix.drive_db > 0.0 {
            let drive_db = t.mix.drive_db.min(crate::model::MAX_DRIVE_DB).into();
            let drive = Effect::Saturate { kind: fx::SatKind::Tanh, drive_db, mix: 1.0.into() };
            fx::apply_chain(std::slice::from_ref(&drive), &mut x, &self.ctx(&t.name, 0));
        }
        let level = loudness::active_level(&x);
        (x, level)
    }

    /// each track's sound before the mix: kept from the render before when nothing it comes from has changed, and kept
    /// for the next
    fn sounds(&self, kept: &mut Kept) -> Result<Vec<Made>> {
        let p = self.p;
        let sounds: Vec<Made> = p
            .tracks
            .par_iter()
            .map(|t| -> Result<Made> {
                let key = track_key(p, t, self.len, &self.chokes);
                if let Some(k) = kept.tracks.get(&t.name).filter(|k| k.key == key) {
                    let time = k.time.clone().unwrap_or_else(|| timed(t, || k.sound.stereo(), p.bpm));
                    return Ok((key, k.sound.clone(), k.level, time));
                }
                let (x, level) = self.sound(t)?;
                let time = timed(t, || &x, p.bpm);
                Ok((key, Arc::new(Blocks::of(&x)), level, time))
            })
            .collect::<Result<Vec<_>>>()?;
        kept.tracks = p
            .tracks
            .iter()
            .zip(&sounds)
            .map(|(t, (key, sound, level, time))| {
                let k = KeptTrack { key: *key, sound: sound.clone(), level: *level, time: Some(time.clone()) };
                (t.name.clone(), k)
            })
            .collect();
        Ok(sounds)
    }

    /// The tracks' samples in `range` into their buses and the sends, levelled to their roles' targets, ducked under
    /// the kick and panned, as the sum of each one's own copy scaled in that order would be; the buses through their
    /// chains and the returns through theirs, added up.
    fn mix(&self, sounds: &[Made], range: std::ops::Range<usize>, want_stems: bool) -> Mixed {
        let p = self.p;
        let (a, n) = (range.start, range.len());
        let mut buses: BTreeMap<String, Stereo> = BTreeMap::new();
        let mut rev_send = Stereo::zeros(n);
        let mut del_send = Stereo::zeros(n);
        let mut stems = Vec::new();
        let mut tracks = Vec::new();
        for (t, (_, sound, level, time)) in p.tracks.iter().zip(sounds) {
            let target = target_level(&t.role) + p.mix.levels.get(&t.role).copied().unwrap_or(0.0) + t.mix.level_db;
            let gain_db = level.map_or(0.0, |l| (target as f64 - l) as f32).clamp(-40.0, 30.0);
            let gain = db_to_gain(gain_db);
            let duck = self.duck.as_ref().filter(|_| t.mix.duck > 0.0 && t.role != "kick");
            let depth = t.mix.duck.clamp(0.0, 1.0);
            let pan = (t.mix.pan.abs() > 1e-3).then(|| audio::balance_gains(t.mix.pan));
            let bus = buses.entry(t.mix.bus.clone()).or_insert_with(|| Stereo::zeros(n));
            // a block at a time, each sample scaled, ducked and panned in that order; a block it holds only silence in
            // adds nothing
            let mut block = Stereo::zeros(BLOCK);
            let shape = |b: &mut Stereo, from: usize, n: usize| {
                for j in 0..n {
                    b.l[j] *= gain;
                    b.r[j] *= gain;
                }
                if let Some(d) = duck {
                    for j in 0..n.min(d.len().saturating_sub(from)) {
                        let g = 1.0 - depth * (1.0 - d[from + j]);
                        b.l[j] *= g;
                        b.r[j] *= g;
                    }
                }
                if let Some((gl, gr)) = pan {
                    for j in 0..n {
                        b.l[j] *= gl;
                        b.r[j] *= gr;
                    }
                }
            };
            let end = range.end.min(sound.len());
            for from in (a..end).step_by(BLOCK) {
                let k = BLOCK.min(end - from);
                if !sound.read(from, k, &mut block.l[..k], &mut block.r[..k]) {
                    continue;
                }
                shape(&mut block, from, k);
                let (bl, br) = (&block.l[..k], &block.r[..k]);
                let at = from - a;
                for (o, v) in bus.l[at..at + k].iter_mut().zip(bl) {
                    *o += v;
                }
                for (o, v) in bus.r[at..at + k].iter_mut().zip(br) {
                    *o += v;
                }
                for (send, amount) in [(&mut rev_send, t.mix.reverb), (&mut del_send, t.mix.delay)] {
                    if amount > 0.0 {
                        for (o, v) in send.l[at..at + k].iter_mut().zip(bl) {
                            *o += v * amount;
                        }
                        for (o, v) in send.r[at..at + k].iter_mut().zip(br) {
                            *o += v * amount;
                        }
                    }
                }
            }
            if want_stems {
                let mut stem = sound.stereo();
                shape(&mut stem, 0, sound.len());
                stems.push((t.name.clone(), stem));
            }
            tracks.push(TrackReport {
                name: t.name.clone(),
                role: t.role.clone(),
                sound: t.sound.describe(),
                bus: t.mix.bus.clone(),
                notes: t.notes.len(),
                clips: t.clips.len(),
                level_lufs: level.map(|l| (l * 10.0).round() / 10.0),
                gain_db: (gain_db * 10.0).round() / 10.0,
                time: time.clone(),
            });
        }
        let mut sum = Stereo::zeros(n);
        let bus_out: Vec<(String, Stereo)> = buses
            .into_par_iter()
            .map(|(name, mut x)| {
                if let Some(chain) = p.mix.buses.get(&name) {
                    fx::apply_chain(chain, &mut x, &self.ctx(&name, a));
                }
                (name, x)
            })
            .collect();
        for (_, x) in &bus_out {
            sum.mix(x, 1.0);
        }
        let returns: Vec<Stereo> = [(rev_send, p.mix.reverb.clone()), (del_send, p.mix.delay.clone())]
            .into_par_iter()
            .map(|(mut s, e)| {
                let dry = matches!(e, Effect::Delay { .. }).then(|| s.clone());
                fx::apply(&e, &mut s, &self.ctx(e.name(), a));
                if let Some(d) = dry {
                    s.mix(&d, -1.0);
                }
                s
            })
            .collect();
        for r in &returns {
            if let Some(d) = &self.duck {
                let mut r = r.clone();
                for i in 0..r.len().min(d.len().saturating_sub(a)) {
                    let g = 1.0 - 0.3 * (1.0 - d[a + i]);
                    r.l[i] *= g;
                    r.r[i] *= g;
                }
                sum.mix(&r, 1.0);
            } else {
                sum.mix(r, 1.0);
            }
        }
        Mixed { sum, stems, tracks }
    }

    /// the master chain on `sum`, which begins `start` samples into the song
    fn master(&self, mut sum: Stereo, start: usize) -> Stereo {
        fx::apply_chain(&self.p.mix.master.fx, &mut sum, &self.ctx("master", start));
        sum
    }

    /// how long the effects after the tracks take to settle: the longest of their tails, and a second
    fn settle_secs(&self) -> f64 {
        let m = &self.p.mix;
        let chains = m.buses.values().map(|c| c.as_slice()).chain([m.master.fx.as_slice()]);
        let returns = [std::slice::from_ref(&m.reverb), std::slice::from_ref(&m.delay)];
        chains.chain(returns).map(|c| fx::chain_tail_secs(c, self.p.bpm)).fold(0.0, f64::max) + 1.0
    }

    /// where the edits of time on the buses and the master begin and end, in samples, reaching at least as far as
    /// they work
    fn regions(&self) -> Vec<(usize, usize)> {
        let m = &self.p.mix;
        m.buses
            .values()
            .flatten()
            .chain(&m.master.fx)
            .filter_map(|e| match e {
                Effect::TapeStop { start, len }
                | Effect::Stutter { start, len, .. }
                | Effect::Reverse { start, len } => {
                    Some(((start * self.tm.spb) as usize, ((start + len) * self.tm.spb).ceil() as usize))
                }
                _ => None,
            })
            .collect()
    }

    /// Where a mix must begin to make every sample from `a` on as the whole song's does: at `a`, or at the start of
    /// a region an edit of time (a tape stop) works on that `a` falls in, which it works on whole. One that ended
    /// before leaves what follows it as it was.
    fn mix_start(&self, mut a: usize) -> usize {
        let regions = self.regions();
        while let Some(start) = regions.iter().filter(|(s, e)| *s < a && *e > a).map(|r| r.0).min() {
            a = start;
        }
        a
    }
}

fn debug() -> bool {
    std::env::var_os("CLAUDIUS_DEBUG").is_some()
}

/// gain, soft clip, limit, and a final trim if the true peak still pokes over `ceiling`: the master, its true peak,
/// and the trim (0 when none)
fn master_stage(src: &Stereo, gain_db: f64, ceiling: f32) -> (Stereo, f32, f32) {
    let mut out = src.clone();
    out.scale(db_to_gain(gain_db as f32));
    fx::soft_clip(&mut out, ceiling - 0.1, 0.7, SRF);
    fx::limit(&mut out, ceiling - 0.5, 50.0, SRF);
    let mut tp = fx::true_peak_db(&out);
    let mut trim = 0.0;
    if tp > ceiling - 0.02 {
        trim = ceiling - 0.05 - tp;
        out.scale(db_to_gain(trim));
        tp = ceiling - 0.05;
    }
    (out, tp, trim)
}

/// loudness to `target` LUFS through a soft clipper and a true-peak-safe limiter (secant search on the input gain)
/// what `finalize` settled on: the loudness and true peak it came to, by the gain it found and the trim after it
pub struct Mastered {
    pub lufs: f64,
    pub true_peak_db: f32,
    pub gain_db: f64,
    pub trim_db: f32,
}

pub fn finalize(x: &mut Stereo, target: f32, ceiling: f32) -> Mastered {
    let src = x.clone();
    let t = target as f64;
    let mut g0 = t - loudness::integrated(&src);
    let (mut out, mut tp, mut trim) = master_stage(&src, g0, ceiling);
    let mut gain = g0;
    let mut l0 = loudness::integrated(&out);
    // the loudness of `out`, measured once
    let mut lufs = l0;
    if debug() {
        eprintln!("finalize: input {:.2} LUFS, gain {g0:+.2} -> {l0:.2} LUFS (tp {tp:.2})", loudness::integrated(&src));
    }
    let mut g1 = g0 + (t - l0);
    for _ in 0..6 {
        if (t - l0).abs() < 0.15 {
            break;
        }
        let (o, p, tr) = master_stage(&src, g1, ceiling);
        let l1 = loudness::integrated(&o);
        if debug() {
            eprintln!("finalize: gain {g1:+.2} -> {l1:.2} LUFS (tp {p:.2})");
        }
        out = o;
        tp = p;
        trim = tr;
        gain = g1;
        lufs = l1;
        let slope = if (g1 - g0).abs() > 1e-3 { ((l1 - l0) / (g1 - g0)).clamp(0.15, 1.5) } else { 1.0 };
        let next = g1 + (t - l1) / slope;
        g0 = g1;
        l0 = l1;
        g1 = next.clamp(g0 - 6.0, g0 + 6.0);
    }
    *x = out;
    Mastered { lufs, true_peak_db: tp, gain_db: gain, trim_db: trim }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a track of `role` striking every sixteenth over `bars` bars, `late_ms` behind the beat
    fn ticks(role: &str, bars: u32, late_ms: f32) -> Track {
        let notes =
            (0..bars * 16).map(|i| Note { t: f64::from(i) / 4.0, d: 0.25, p: 60, v: 0.8, lock: None }).collect();
        let sound = Sound::Sample { path: String::new(), root: 60, start: 0.0, end: None, label: String::new() };
        Track {
            name: role.into(),
            role: role.into(),
            sound,
            notes,
            clips: Vec::new(),
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            choke: None,
            mix: crate::model::TrackMix::default(),
            swing: None,
            late_ms,
        }
    }

    fn sd(x: &[f64]) -> f64 {
        let m = x.iter().sum::<f64>() / x.len() as f64;
        (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    /// a minute of clicks on the eighths of `bpm`, each `late(k)` seconds after its place, `k` the four bars it is in
    fn clicks(bpm: f64, late: impl Fn(usize) -> f64) -> Stereo {
        let mut x = Stereo::zeros((60.0 * SRF) as usize);
        let eighth = 30.0 / bpm;
        let mut noise = crate::rng::Rng::new(3);
        for i in 0..(59.5 / eighth) as usize {
            let at = ((i as f64 * eighth + late(i / 32)) * SRF) as usize;
            for j in 0..(0.01 * SRF) as usize {
                let v = (noise.f32() - 0.5) * (1.0 - j as f32 / (0.01 * SRF) as f32);
                x.l[at + j] += v;
                x.r[at + j] += v;
            }
        }
        x
    }

    #[test]
    fn a_recording_on_the_songs_grid_keeps_its_time_and_one_off_it_keeps_a_tempo_of_its_own() {
        let on = kept_time(&clicks(120.0, |_| 0.0), 120.0).unwrap();
        assert!(on.on_grid > 0.8 && on.on_grid > 0.98 * on.own && on.drift_ms < 3.0, "{on:?}");
        assert!((on.own_bpm - 120.0).abs() < 0.1, "{on:?}");
        // played 3 % slow, as a recording whose tempo was misread plays: its own tempo, and off the song's sixteenths
        let off = kept_time(&clicks(116.4, |_| 0.0), 120.0).unwrap();
        assert!((off.own_bpm - 116.4).abs() < 0.3 && off.on_grid < 0.2 * off.own, "{off:?}");
        // at the song's tempo, but settling 40 ms late after its first 32 s: about 20 ms off its place either way
        let wanders = kept_time(&clicks(120.0, |k| if k < 4 { 0.0 } else { 0.04 }), 120.0).unwrap();
        assert!((wanders.own_bpm - 120.0).abs() < 0.3 && (wanders.drift_ms - 20.0).abs() < 5.0, "{wanders:?}");
        // silence has no onsets to tell by
        assert!(kept_time(&Stereo::zeros((10.0 * SRF) as usize), 120.0).is_none());
    }

    fn corr(a: &[f64], b: &[f64]) -> f64 {
        let (ma, mb) = (a.iter().sum::<f64>() / a.len() as f64, b.iter().sum::<f64>() / b.len() as f64);
        let cov = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum::<f64>() / a.len() as f64;
        cov / (sd(a) * sd(b))
    }

    #[test]
    fn a_hit_drifts_as_a_player_does_with_the_hits_around_it_and_hats_more_than_the_kick() {
        let tm = Timing { spb: SRF * 60.0 / 90.0, swing: 0.5, humanize: 1.0, seed: 7 };
        let secs = |t: &Track| -> Vec<f64> { t.notes.iter().map(|n| tm.feel(t, n.t) * tm.spb / SRF).collect() };
        let (hat, kick, bass) = (ticks("hat", 64, 0.0), ticks("kick", 64, 0.0), ticks("bass", 64, 0.0));
        let (h, k, b) = (secs(&hat), secs(&kick), secs(&bass));
        assert!((sd(&h) / DRIFT_SECS - 1.0).abs() < 0.35, "hats drift {:.1} ms", sd(&h) * 1e3);
        assert!((sd(&k) / sd(&h) - 0.5).abs() < 0.2, "the kick holds the beat: {:.1} ms", sd(&k) * 1e3);
        assert!(h.iter().all(|x| x.abs() <= DRIFT_MOST * DRIFT_SECS + 1e-9), "never past the cap");
        // a hit moves with the one before it, as a humaniser's independent jitter does not; the kit moves together and
        // a part on its own, from hit to hit (two drifts' own slow wanders make their levels look alike by chance)
        assert!(corr(&h[1..], &h[..h.len() - 1]) > 0.6, "{}", corr(&h[1..], &h[..h.len() - 1]));
        let step = |x: &[f64]| x.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
        let shared = corr(&step(&h), &step(&k));
        assert!(shared > 0.2, "the lanes share a player: {shared}");
        let apart = corr(&step(&h), &step(&b));
        assert!(apart.abs() < 0.15, "a part has a player of its own: {apart}");
        // over the bars: the drift of a bar's mean wanders far more than an even jitter's would (1/16 of its variance)
        let bars: Vec<f64> = h.chunks(16).map(|c| c.iter().sum::<f64>() / 16.0).collect();
        assert!(sd(&bars).powi(2) > 0.3 * sd(&h).powi(2), "{} against {}", sd(&bars), sd(&h));
        let still = Timing { humanize: 0.0, ..tm };
        assert!(hat.notes.iter().all(|n| still.feel(&hat, n.t) == 0.0), "humanize 0: on the grid");
    }

    #[test]
    fn a_lane_twenty_ms_late_plays_every_hit_twenty_ms_behind_the_beat() {
        let tm = Timing { spb: SRF * 60.0 / 90.0, swing: 0.5, humanize: 0.0, seed: 7 };
        let snare = ticks("snare", 2, 20.0);
        for n in &snare.notes {
            assert_eq!(tm.onset(&snare, n) - tm.sample(n.t, 0.0), (0.020 * SRF).round() as i64);
        }
        let pushed = ticks("kick", 1, -5.0);
        assert_eq!(tm.onset(&pushed, &pushed.notes[4]) - tm.sample(1.0, 0.0), -(0.005 * SRF).round() as i64);
        assert_eq!(tm.onset(&pushed, &pushed.notes[0]), 0, "ahead of the song's first beat, it plays on it");
        // and in a render: the song's snare, alone and late, sounds that much later
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut p = crate::compose::arrange(&crate::compose::generate(&g, 3, &o, None));
        p.humanize = 0.0;
        p.tracks.retain(|t| t.role == "clap" || t.role == "snare");
        p.tracks.truncate(1);
        p.tracks[0].notes = vec![Note { t: 4.0, d: 0.25, p: 60, v: 0.9, lock: None }];
        let first = |late: f32| {
            let mut q = p.slice(0, 2);
            q.tracks[0].late_ms = late;
            let x = render(&q, &Options { stems: false, tail_secs: 0.0 }).unwrap().master;
            let peak = x.l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            x.l.iter().position(|v| v.abs() > 0.05 * peak).unwrap() as i64
        };
        let moved = first(20.0) - first(0.0);
        assert!((moved - 960).abs() <= 2, "20 ms is 960 samples, not {moved}");
        // a hit on the song's first beat, pushed ahead of it, keeps its attack: it sounds as it would on the beat
        p.tracks[0].notes[0].t = 0.0;
        let opening = |late: f32| {
            let mut q = p.slice(0, 2);
            q.tracks[0].late_ms = late;
            render(&q, &Options { stems: false, tail_secs: 0.0 }).unwrap().master.l[..4_800].to_vec()
        };
        assert_eq!(opening(-20.0), opening(0.0));
    }

    #[test]
    fn a_sample_locked_shorter_dies_away_sooner_and_darker_loses_its_top() {
        let n = 24_000;
        let noise: Vec<f32> = (0..n).map(|i| ((i * 7_919 % 1_000) as f32 / 500.0 - 1.0) * 0.5).collect();
        let x = Stereo { l: noise.clone(), r: noise };
        let tail = |s: &Stereo| s.l[n / 2..].iter().map(|v| v * v).sum::<f32>();
        let mut short = x.clone();
        shape(&mut short, 0.5, 0.0);
        assert!(tail(&short) < 0.05 * tail(&x), "{} {}", tail(&short), tail(&x));
        let mut same = x.clone();
        shape(&mut same, 1.0, 0.0);
        assert!(same.l == x.l && same.r == x.r, "a decay of 1 and no tone leave it as it is");
        // darker: what changes from sample to sample (the top) is smaller
        let edge = |s: &Stereo| s.l.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>();
        let mut dark = x.clone();
        shape(&mut dark, 1.0, -1.0);
        assert!(edge(&dark) < 0.5 * edge(&x));
    }

    #[test]
    fn a_sample_a_page_hands_over_anew_at_its_path_is_rendered_again() {
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut p = crate::compose::arrange(&crate::compose::generate(&g, 3, &o, None));
        let path = "demo:test/kick-anew.wav";
        let thump = |hz: f32| {
            let l: Vec<f32> =
                (0..4_800).map(|i| (i as f32 * hz / 48_000.0 * std::f32::consts::TAU).sin() * 0.8).collect();
            Stereo { r: l.clone(), l }
        };
        audio::remember(path, thump(55.0));
        let kick = p.tracks.iter().position(|t| t.role == "kick").unwrap();
        p.tracks[kick].sound =
            Sound::Sample { path: path.into(), root: 60, start: 0.0, end: None, label: "kick".into() };
        let mut kept = Kept::default();
        render_kept(&p, &Options::default(), &mut kept).unwrap();
        let before = kept.tracks[&p.tracks[kick].name].sound.clone();
        audio::remember(path, thump(80.0));
        let r = render_kept(&p, &Options::default(), &mut kept).unwrap();
        assert!(!Arc::ptr_eq(&kept.tracks[&p.tracks[kick].name].sound, &before), "the kick is rendered again");
        let fresh = render(&p, &Options::default()).unwrap();
        assert!(r.master.l == fresh.master.l, "and the song is the one a fresh render makes");
    }

    #[test]
    fn a_render_renders_again_only_the_tracks_an_edit_changed_and_comes_out_the_same() {
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut p = crate::compose::arrange(&crate::compose::generate(&g, 3, &o, None));
        let mut kept = Kept::default();
        render_kept(&p, &Options::default(), &mut kept).unwrap();
        let before: HashMap<String, Arc<Blocks>> =
            kept.tracks.iter().map(|(n, k)| (n.clone(), k.sound.clone())).collect();
        let same = |kept: &Kept, name: &str| Arc::ptr_eq(&kept.tracks[name].sound, &before[name]);
        // a level, a pan and a send: no track is rendered again, and the song is the one a fresh render makes
        let bass = p.tracks.iter().position(|t| t.name == "bass").unwrap();
        p.tracks[bass].mix.level_db += 3.0;
        p.tracks[bass].mix.pan = 0.4;
        p.tracks[bass].mix.reverb = 0.3;
        let r = render_kept(&p, &Options::default(), &mut kept).unwrap();
        assert!(p.tracks.iter().all(|t| same(&kept, &t.name)), "nothing rendered again");
        let fresh = render(&p, &Options::default()).unwrap();
        assert!(r.master.l == fresh.master.l && r.master.r == fresh.master.r, "the same song, sample for sample");
        // the bass's notes: the bass alone is rendered again
        p.tracks[bass].notes.pop();
        let r = render_kept(&p, &Options::default(), &mut kept).unwrap();
        assert!(!same(&kept, "bass"));
        assert!(p.tracks.iter().filter(|t| t.name != "bass").all(|t| same(&kept, &t.name)));
        let fresh = render(&p, &Options::default()).unwrap();
        assert!(r.master.l == fresh.master.l && r.master.r == fresh.master.r);
        // the tempo changes every track
        p.bpm += 2.0;
        render_kept(&p, &Options::default(), &mut kept).unwrap();
        assert!(p.tracks.iter().all(|t| !same(&kept, &t.name)));
    }

    #[test]
    fn a_track_is_kept_without_its_silence_and_reads_back_to_the_sample() {
        // noise in the second block and a moment of the fourth, silence elsewhere, and a short last block
        let n = 4 * BLOCK + 1000;
        let mut x = Stereo::zeros(n);
        let mut rng = Rng::new(4);
        for i in (BLOCK..2 * BLOCK).chain(3 * BLOCK + 10..3 * BLOCK + 20) {
            x.l[i] = rng.f32() - 0.5;
            x.r[i] = -x.l[i];
        }
        x.r[n - 1] = 0.25;
        let b = Blocks::of(&x);
        assert_eq!(b.held(), 2 * (BLOCK + BLOCK + 1000), "the second, fourth and last blocks");
        let all = b.stereo();
        assert!(all.l == x.l && all.r == x.r);
        // from anywhere, across blocks
        let (mut l, mut r) = (vec![9.0; 3000], vec![9.0; 3000]);
        assert!(b.read(BLOCK - 1500, 3000, &mut l, &mut r));
        assert!(l == x.l[BLOCK - 1500..BLOCK + 1500] && r == x.r[BLOCK - 1500..BLOCK + 1500]);
        assert!(!b.read(10, 3000, &mut l, &mut r), "silence");
        // a song's tracks rest much of it
        let g = crate::genres::get("trap").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let p = crate::compose::arrange(&crate::compose::generate(&g, 2, &o, None));
        let mut kept = Kept::default();
        let r = render_kept(&p, &Options::default(), &mut kept).unwrap();
        let whole = 2 * p.tracks.len() * Song::of(&p, &Options::default()).unwrap().len;
        let share = kept.samples() as f64 / whole as f64;
        eprintln!("trap: its tracks are kept in {:.0}% of their length", share * 100.0);
        assert!(share < 0.7, "{share}");
        assert!(r.master.l == render(&p, &Options::default()).unwrap().master.l);
    }

    #[test]
    fn a_window_of_a_song_is_the_whole_songs_render_there() {
        for genre in ["house", "lofi", "synthwave", "trip_hop"] {
            let g = crate::genres::get(genre).unwrap();
            let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
            let p = crate::compose::arrange(&crate::compose::generate(&g, 5, &o, None));
            let mut kept = Kept::default();
            let whole = render_kept(&p, &Options::default(), &mut kept).unwrap();
            let settled = whole.settled.unwrap();
            let n = (4.0 * SRF) as usize;
            let mut worst = 0.0f32;
            for from in [(3.0 * SRF) as usize, settled.end / 2, settled.end - n / 2] {
                let w = render_window(&p, &Options::default(), &mut kept, &settled, from, n).unwrap().unwrap();
                assert_eq!(w.start, from);
                for i in 0..w.master.len() {
                    worst = worst.max((w.master.l[i] - whole.master.l[from + i]).abs());
                    worst = worst.max((w.master.r[i] - whole.master.r[from + i]).abs());
                }
            }
            eprintln!("{genre}: a window is within {worst:e} of the whole song's render");
            assert!(worst < 1e-6, "{genre}: {worst}");
        }
    }

    #[test]
    fn tracks_made_elsewhere_and_kept_render_the_song_as_it_would_have_been() {
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let p = crate::compose::arrange(&crate::compose::generate(&g, 5, &o, None));
        let opts = Options::default();
        let mut kept = Kept::default();
        let stale = kept.stale(&p, &opts).unwrap();
        assert_eq!(stale.len(), p.tracks.len(), "a first render makes every track");
        for (name, key) in stale {
            let (sound, level) = track_sound(&p, &opts, &name).unwrap();
            kept.keep(&name, key, sound, level);
        }
        assert!(kept.stale(&p, &opts).unwrap().is_empty(), "and then none");
        let shared = render_kept(&p, &opts, &mut kept).unwrap();
        let alone = render(&p, &opts).unwrap();
        assert!(shared.master.l == alone.master.l && shared.master.r == alone.master.r);
        // an edit to a synth's notes leaves the other tracks kept
        let mut q = p.clone();
        let synth = q.tracks.iter().position(|t| matches!(t.sound, Sound::Synth { .. })).unwrap();
        q.tracks[synth].notes.pop();
        let stale: Vec<String> = kept.stale(&q, &opts).unwrap().into_iter().map(|(n, _)| n).collect();
        assert_eq!(stale, [q.tracks[synth].name.clone()]);
        let drum = q.tracks.iter().find(|t| matches!(t.sound, Sound::Drum { .. })).unwrap();
        let costs = |t: &Track| track_cost(t, q.bpm, q.secs());
        assert!(costs(&q.tracks[synth]) > costs(drum), "a synth's notes cost more than a drum's hits");
    }

    #[test]
    fn a_synths_track_made_in_parts_and_joined_is_the_track_made_whole() {
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let p = crate::compose::arrange(&crate::compose::generate(&g, 5, &o, None));
        let opts = Options::default();
        let len = Song::of(&p, &opts).unwrap().len;
        // parts that cover the song in order, none empty or silent, which joined are the track made whole
        let whole_in_parts = |q: &Project, name: &str| -> Vec<(std::ops::Range<usize>, f64)> {
            let bounds = part_bounds(q, &opts, name, 3).unwrap();
            assert!(!bounds.is_empty() && bounds.len() <= 3, "{bounds:?}");
            assert!(bounds[0].0.start == 0 && bounds.last().unwrap().0.end == len);
            assert!(bounds.windows(2).all(|w| w[0].0.end == w[1].0.start), "{bounds:?}");
            assert!(bounds.iter().all(|(r, share)| r.start < r.end && *share > 0.0 && *share <= 1.0), "{bounds:?}");
            assert!(bounds.iter().map(|(_, share)| share).sum::<f64>() >= 1.0 - 1e-9);
            let parts: Vec<Stereo> =
                bounds.iter().map(|(r, _)| track_part(q, &opts, name, r.clone()).unwrap()).collect();
            let (joined, level) = track_joined(q, &opts, name, &parts).unwrap();
            let (whole, whole_level) = track_sound(q, &opts, name).unwrap();
            assert!(joined.l == whole.l && joined.r == whole.r && level == whole_level, "{name}");
            bounds
        };
        let split: Vec<&Track> = p.tracks.iter().filter(|t| splits(t)).collect();
        assert!(!split.is_empty(), "house has synths that play their notes apart");
        assert!(
            split.iter().any(|t| whole_in_parts(&p, &t.name).len() == 3),
            "a track of notes all through splits in three"
        );
        // notes that all begin together, at the start or later (a chord held from bar 4): some part holds all of the
        // work, which is why a crew would not split it
        let mut q = p.clone();
        let at = q.tracks.iter().position(splits).unwrap();
        let name = q.tracks[at].name.clone();
        for t in [0.0, 16.0] {
            for n in q.tracks[at].notes.iter_mut() {
                n.t = t;
            }
            let all = whole_in_parts(&q, &name);
            assert!(all.iter().map(|(_, share)| *share).fold(0.0, f64::max) > 0.99, "{t}: {all:?}");
        }
        q.tracks[at].notes.truncate(1);
        let one = whole_in_parts(&q, &name);
        assert!(one.len() == 1 && one[0].0 == (0..len), "one note is one part: {one:?}");
        // a mono synth glides one note into the next: it is made whole
        let mono = p.tracks.iter().find(|t| matches!(&t.sound, Sound::Synth { patch, .. } if patch.mono));
        if let Some(t) = mono {
            assert!(!splits(t) && track_part(&p, &opts, &t.name, 0..len).is_err());
        }
    }

    #[test]
    fn a_window_mixes_from_where_a_tape_stop_it_falls_in_starts_and_not_from_one_that_ended() {
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut p = crate::compose::arrange(&crate::compose::generate(&g, 5, &o, None));
        p.mix.master.fx.push(Effect::TapeStop { start: 16.0, len: 4.0 });
        let s = Song::of(&p, &Options::default()).unwrap();
        let beat = |b: f64| (b * s.tm.spb) as usize;
        let reach = (s.settle_secs() * SRF) as usize;
        // a mix that would begin in the stop begins where it does; one that would begin after it, where it would
        let (inside, after) = (beat(18.0) + reach, beat(21.0) + reach);
        assert_eq!(s.mix_start(inside - reach), beat(16.0));
        assert_eq!(s.mix_start(after - reach), after - reach);
        let mut kept = Kept::default();
        let whole = render_kept(&p, &Options::default(), &mut kept).unwrap();
        let settled = whole.settled.unwrap();
        let n = (2.0 * SRF) as usize;
        for from in [inside, after] {
            let w = render_window(&p, &Options::default(), &mut kept, &settled, from, n).unwrap().unwrap();
            let worst = (0..w.master.len())
                .map(|i| {
                    (w.master.l[i] - whole.master.l[from + i])
                        .abs()
                        .max((w.master.r[i] - whole.master.r[from + i]).abs())
                })
                .fold(0.0f32, f32::max);
            assert!(worst < 1e-6, "from {from}: {worst}");
        }
    }

    #[test]
    fn the_mix_ducks_under_a_nudged_kick_where_it_sounds() {
        let song = |trig: &str| {
            let code =
                format!(r#"blank("house"); drum("kick", "....x..........."); form([sec("drop", 1, "kick")]); {trig}"#);
            crate::compose::arrange(&crate::script::run(&code, None, None).unwrap().bp.unwrap())
        };
        let tm = Timing { spb: SRF * 60.0 / 120.0, swing: 0.5, humanize: 0.0, seed: 0 };
        let dip = |p: &Project| duck_curve(p, &tm, 96_000).unwrap().iter().position(|&g| g < 0.99).unwrap() as i64;
        let (plain, late) = (song(""), song(r#"trig("kick", 4, #{ nudge: 0.5 });"#));
        assert!((dip(&late) - dip(&plain) - 3_000).abs() <= 1, "{} {}", dip(&plain), dip(&late));
    }

    #[test]
    fn a_synth_drum_locked_shorter_is_its_own_sound_with_less_decay() {
        let kick = Sound::Drum {
            voice: drums::DrumVoice::Kick,
            params: drums::DrumParams::default_for(drums::DrumVoice::Kick),
        };
        let whole = locked_shot(&kick, 0, 1, &Lock::default()).unwrap();
        let short = locked_shot(&kick, 0, 1, &Lock { decay: 0.3, ..Lock::default() }).unwrap();
        let energy = |s: &Stereo, from: usize| s.l[from.min(s.len())..].iter().map(|v| v * v).sum::<f32>();
        assert!(energy(&short, 9_600) < 0.2 * energy(&whole, 9_600), "less after 200 ms");
        assert!(Arc::ptr_eq(&short, &locked_shot(&kick, 0, 1, &Lock { decay: 0.3, ..Lock::default() }).unwrap()));
    }
}
