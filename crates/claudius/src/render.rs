//! The renderer: a project becomes stems, buses and a mastered stereo file.
//!
//! Per track (in parallel): play the notes or clips through its sound (sampler, synth engine, transition effect,
//! warped loop) with swing and humanisation, run its insert chain, balance it to its role's target loudness, duck it
//! under the kick, pan it, send it to the reverb and delay returns. Buses get their own chains (with the composer's
//! filter automation), the master chain glues the sum, and the loudness stage brings it to the genre's LUFS target
//! under a true-peak ceiling.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
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

/// the one-shot buffer of a drum or sample sound, transposed by `semis` (cached)
fn one_shot(sound: &Sound, semis: i32, seed: u64) -> Result<Arc<Stereo>> {
    let key = format!("{}|{semis}|{seed}", serde_json::to_string(sound).unwrap_or_default());
    if let Some(x) = shots().lock().unwrap().get(&key) {
        return Ok(x.clone());
    }
    let base_key = format!("{}|0|{seed}", serde_json::to_string(sound).unwrap_or_default());
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
    let key = format!(
        "{}|{semis}|{seed}|{:.3}|{:.3}",
        serde_json::to_string(sound).unwrap_or_default(),
        lock.decay,
        lock.tone
    );
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
}

impl Timing {
    fn sample(&self, beat: f64, jitter: f64) -> i64 {
        ((swing(beat, self.swing) + jitter) * self.spb).round() as i64
    }

    /// the timing of track `t`: its own swing, when it has one
    fn of(self, t: &Track) -> Timing {
        Timing { swing: t.swing.unwrap_or(self.swing), ..self }
    }

    /// where note `n` starts: swung, then moved by its step's nudge
    fn onset(&self, n: &Note, jitter: f64) -> i64 {
        self.sample(n.t, jitter + n.lock.map_or(0.0, |l| l.nudge))
    }
}

/// onset sample positions of every track in each choke group
fn choke_onsets(p: &Project, tm: &Timing) -> BTreeMap<String, Vec<i64>> {
    let mut m: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for t in &p.tracks {
        if let Some(g) = &t.choke {
            let tm = tm.of(t);
            m.entry(g.clone()).or_default().extend(t.notes.iter().map(|n| tm.onset(n, 0.0)));
        }
    }
    for v in m.values_mut() {
        v.sort();
        v.dedup();
    }
    m
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
                let jitter = rng.normal() * human * 0.004 * p.bpm / 60.0;
                let pos = tm.of(t).onset(n, jitter);
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
            let evs: Vec<NoteEv> = t
                .notes
                .iter()
                .map(|n| {
                    let jitter = rng.normal() * human * 0.003 * p.bpm / 60.0;
                    let start = tm.sample(n.t, jitter).max(0) as usize;
                    let vel = (n.v as f64 * (1.0 + rng.normal() * human * 0.05)).clamp(0.05, 1.0) as f32;
                    NoteEv { start, gate: ((n.d * tm.spb) as usize).max(16), pitch: (n.p + t.transpose) as f64, vel }
                })
                .collect();
            out = voice::render(patch, &evs, len, SRF, p.bpm, seed);
        }
        Sound::Instrument { name, .. } => {
            let inst = claudius_packs::instrument(name, None)?;
            let notes: Vec<claudius_packs::NoteOn> = t
                .notes
                .iter()
                .map(|n| {
                    let jitter = rng.normal() * human * 0.004 * p.bpm / 60.0;
                    let vel = (n.v as f64 * (1.0 + rng.normal() * human * 0.05)).clamp(0.05, 1.0) as f32;
                    claudius_packs::NoteOn {
                        start: tm.sample(n.t, jitter).max(0) as usize,
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
        Sound::Loop { path, bpm, beats, .. } => {
            let once = t.clips.iter().any(|c| c.once);
            let w = crate::warp::warp(path, *bpm, *beats, p.bpm, t.transpose, !once)?;
            let wl = w.len().max(1);
            let slice = (t.slice_beats.max(0.0625) * tm.spb).round() as usize;
            let n_slices = (wl / slice.max(1)).max(1);
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
                        (idx * slice + rel % slice) % wl
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
        let pos = tm.onset(n, 0.0).max(0) as usize;
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
    sound: Arc<Stereo>,
    level: Option<f64>,
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
}

/// what a track's sound before the mix comes from: the track, but for what the mix does after its drive, the song's
/// tempo, swing, humanisation, seed, key and chords (which a tuned voice is tuned to), the render's length, the onsets
/// that choke it, and the files it plays as they are now
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
    if crate::config::FILES
        && let Sound::Sample { path, .. } | Sound::Loop { path, .. } = &t.sound
        && let Ok(md) = std::fs::metadata(path)
    {
        let mtime = md.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok());
        h.update(format!("{}|{:?}", md.len(), mtime).as_bytes());
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
    // the effects after the tracks settle within their tails; a region an edit of time (a tape stop) works on starts it
    let reach = (s.settle_secs() * SRF) as usize;
    let mut a = from.saturating_sub(reach);
    for (start, _) in s.regions() {
        if start < to && start < a {
            a = start;
        }
    }
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

type Made = (blake3::Hash, Arc<Stereo>, Option<f64>);

impl<'a> Song<'a> {
    fn of(p: &'a Project, opts: &Options) -> Result<Song<'a>> {
        if p.bars > crate::model::MAX_BARS || p.secs() > crate::model::MAX_SECONDS {
            anyhow::bail!("the song is too long to render ({} bars, {:.0} s)", p.bars, p.secs());
        }
        // a pattern with several sounds plays as a track for each, so the arranged song can have more than its blueprint
        crate::model::check_tracks(p.tracks.len(), p.secs()).map_err(anyhow::Error::msg)?;
        let tm = Timing { spb: SRF * 60.0 / p.bpm, swing: p.swing, humanize: p.humanize };
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
                    return Ok((key, k.sound.clone(), k.level));
                }
                let ts = crate::clock::Instant::now();
                let mut x = render_source(t, p, &self.tm, self.len, &self.chokes)?;
                fx::apply_chain(&t.mix.fx, &mut x, &self.ctx(&t.name, 0));
                if t.mix.drive_db > 0.0 {
                    let drive_db = t.mix.drive_db.min(crate::model::MAX_DRIVE_DB).into();
                    let drive = Effect::Saturate { kind: fx::SatKind::Tanh, drive_db, mix: 1.0.into() };
                    fx::apply_chain(std::slice::from_ref(&drive), &mut x, &self.ctx(&t.name, 0));
                }
                if debug() {
                    eprintln!("render: track {} in {:.2} s", t.name, ts.elapsed().as_secs_f64());
                }
                let level = loudness::active_level(&x);
                Ok((key, Arc::new(x), level))
            })
            .collect::<Result<Vec<_>>>()?;
        kept.tracks = p
            .tracks
            .iter()
            .zip(&sounds)
            .map(|(t, (key, sound, level))| {
                (t.name.clone(), KeptTrack { key: *key, sound: sound.clone(), level: *level })
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
        for (t, (_, sound, level)) in p.tracks.iter().zip(sounds) {
            let target = target_level(&t.role) + p.mix.levels.get(&t.role).copied().unwrap_or(0.0) + t.mix.level_db;
            let gain_db = level.map_or(0.0, |l| (target as f64 - l) as f32).clamp(-40.0, 30.0);
            let gain = db_to_gain(gain_db);
            let duck = self.duck.as_ref().filter(|_| t.mix.duck > 0.0 && t.role != "kick");
            let depth = t.mix.duck.clamp(0.0, 1.0);
            let pan = (t.mix.pan.abs() > 1e-3).then(|| audio::balance_gains(t.mix.pan));
            let bus = buses.entry(t.mix.bus.clone()).or_insert_with(|| Stereo::zeros(n));
            // a block at a time, each sample scaled, ducked and panned in that order
            let mut block = Stereo::zeros(BLOCK);
            let shape = |b: &mut Stereo, from: usize, n: usize| {
                for j in 0..n {
                    b.l[j] = sound.l[from + j] * gain;
                    b.r[j] = sound.r[from + j] * gain;
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
                let mut stem = Stereo::zeros(sound.len());
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

    /// where the edits of time on the buses and the master begin, in samples
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
                    Some(((start * self.tm.spb) as usize, ((start + len) * self.tm.spb) as usize))
                }
                _ => None,
            })
            .collect()
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
    fn a_render_renders_again_only_the_tracks_an_edit_changed_and_comes_out_the_same() {
        let g = crate::genres::get("house").unwrap();
        let o = crate::compose::Options { short: true, samples: 0.0, loops: false, ..Default::default() };
        let mut p = crate::compose::arrange(&crate::compose::generate(&g, 3, &o, None));
        let mut kept = Kept::default();
        render_kept(&p, &Options::default(), &mut kept).unwrap();
        let before: HashMap<String, Arc<Stereo>> =
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
    fn the_mix_ducks_under_a_nudged_kick_where_it_sounds() {
        let song = |trig: &str| {
            let code =
                format!(r#"blank("house"); drum("kick", "....x..........."); form([sec("drop", 1, "kick")]); {trig}"#);
            crate::compose::arrange(&crate::script::run(&code, None, None).unwrap().bp.unwrap())
        };
        let tm = Timing { spb: SRF * 60.0 / 120.0, swing: 0.5, humanize: 0.0 };
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
