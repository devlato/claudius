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

struct Timing {
    spb: f64,
    swing: f32,
    humanize: f32,
}

impl Timing {
    fn sample(&self, beat: f64, jitter: f64) -> i64 {
        ((swing(beat, self.swing) + jitter) * self.spb).round() as i64
    }
}

/// onset sample positions of every track in each choke group
fn choke_onsets(p: &Project, tm: &Timing) -> BTreeMap<String, Vec<i64>> {
    let mut m: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for t in &p.tracks {
        if let Some(g) = &t.choke {
            m.entry(g.clone()).or_default().extend(t.notes.iter().map(|n| tm.sample(n.t, 0.0)));
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
                let pos = tm.sample(n.t, jitter);
                let buf = one_shot(&t.sound, n.p + t.transpose - root, seed)?;
                let vel = (n.v as f64 * (1.0 + rng.normal() * human * 0.05)).clamp(0.05, 1.0) as f32;
                let g = vel.powf(1.6);
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
                    out.add_at(&s, pos, g, g);
                } else {
                    out.add_at(&buf, pos, g, g);
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
            let w = crate::warp::warp(path, *bpm, *beats, p.bpm, t.transpose)?;
            let wl = w.len().max(1);
            let slice = (t.slice_beats.max(0.0625) * tm.spb).round() as usize;
            let n_slices = (wl / slice.max(1)).max(1);
            for c in &t.clips {
                let a = (c.t * tm.spb).round() as usize;
                let b = (((c.t + c.d) * tm.spb).round() as usize).min(len);
                let off = (c.offset * tm.spb).round() as usize;
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
    for n in &kick.notes {
        let pos = tm.sample(n.t, 0.0).max(0) as usize;
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

pub fn render(p: &Project, opts: &Options) -> Result<Rendered> {
    if p.bars > crate::model::MAX_BARS || p.secs() > crate::model::MAX_SECONDS {
        anyhow::bail!("the song is too long to render ({} bars, {:.0} s)", p.bars, p.secs());
    }
    // a pattern with several sounds plays as a track for each, so the arranged song can have more than its blueprint
    crate::model::check_tracks(p.tracks.len(), p.secs()).map_err(anyhow::Error::msg)?;
    let t0 = std::time::Instant::now();
    let tm = Timing { spb: SRF * 60.0 / p.bpm, swing: p.swing, humanize: p.humanize };
    let tail = opts.tail_secs.max(0.0)
        + p.tracks.iter().map(|t| fx::chain_tail_secs(&t.mix.fx, p.bpm)).fold(0.0, f64::max).min(6.0);
    let len = (p.beats() * tm.spb) as usize + (tail * SRF) as usize;
    let chokes = choke_onsets(p, &tm);
    let duck = duck_curve(p, &tm, len);
    let ctx = |name: &str| Ctx { sr: SRF, bpm: p.bpm, seed: p.seed ^ hash_str(name) };

    struct Done {
        bus: String,
        stem: Stereo,
        reverb: f32,
        delay: f32,
        report: TrackReport,
    }
    let done: Vec<Done> = p
        .tracks
        .par_iter()
        .map(|t| -> Result<Done> {
            let ts = std::time::Instant::now();
            let mut x = render_source(t, p, &tm, len, &chokes)?;
            fx::apply_chain(&t.mix.fx, &mut x, &ctx(&t.name));
            if debug() {
                eprintln!("render: track {} in {:.2} s", t.name, ts.elapsed().as_secs_f64());
            }
            let level = loudness::active_level(&x);
            let target = target_level(&t.role) + p.mix.levels.get(&t.role).copied().unwrap_or(0.0) + t.mix.level_db;
            let gain_db = level.map_or(0.0, |l| (target as f64 - l) as f32).clamp(-40.0, 30.0);
            x.scale(db_to_gain(gain_db));
            if t.mix.duck > 0.0
                && t.role != "kick"
                && let Some(d) = &duck
            {
                let depth = t.mix.duck.clamp(0.0, 1.0);
                for i in 0..x.len().min(d.len()) {
                    let g = 1.0 - depth * (1.0 - d[i]);
                    x.l[i] *= g;
                    x.r[i] *= g;
                }
            }
            if t.mix.pan.abs() > 1e-3 {
                let (gl, gr) = audio::balance_gains(t.mix.pan);
                x.scale_lr(gl, gr);
            }
            let report = TrackReport {
                name: t.name.clone(),
                role: t.role.clone(),
                sound: t.sound.describe(),
                bus: t.mix.bus.clone(),
                notes: t.notes.len(),
                clips: t.clips.len(),
                level_lufs: level.map(|l| (l * 10.0).round() / 10.0),
                gain_db: (gain_db * 10.0).round() / 10.0,
            };
            Ok(Done { bus: t.mix.bus.clone(), stem: x, reverb: t.mix.reverb, delay: t.mix.delay, report })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut buses: BTreeMap<String, Stereo> = BTreeMap::new();
    let mut rev_send = Stereo::zeros(len);
    let mut del_send = Stereo::zeros(len);
    let mut stems = Vec::new();
    let mut tracks = Vec::new();
    for d in done {
        buses.entry(d.bus.clone()).or_insert_with(|| Stereo::zeros(len)).mix(&d.stem, 1.0);
        if d.reverb > 0.0 {
            rev_send.mix(&d.stem, d.reverb);
        }
        if d.delay > 0.0 {
            del_send.mix(&d.stem, d.delay);
        }
        if opts.stems {
            stems.push((d.report.name.clone(), d.stem));
        }
        tracks.push(d.report);
    }
    let mut sum = Stereo::zeros(len);
    let bus_out: Vec<(String, Stereo)> = buses
        .into_par_iter()
        .map(|(name, mut x)| {
            if let Some(chain) = p.mix.buses.get(&name) {
                fx::apply_chain(chain, &mut x, &ctx(&name));
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
            fx::apply(&e, &mut s, &ctx(e.name()));
            if let Some(d) = dry {
                s.mix(&d, -1.0);
            }
            s
        })
        .collect();
    for r in &returns {
        if let Some(d) = &duck {
            let mut r = r.clone();
            for i in 0..r.len().min(d.len()) {
                let g = 1.0 - 0.3 * (1.0 - d[i]);
                r.l[i] *= g;
                r.r[i] *= g;
            }
            sum.mix(&r, 1.0);
        } else {
            sum.mix(r, 1.0);
        }
    }
    let premaster_lufs = loudness::integrated(&sum);
    let mut master = sum;
    if debug() {
        eprintln!("render: tracks + buses + returns in {:.2} s", t0.elapsed().as_secs_f64());
    }
    fx::apply_chain(&p.mix.master.fx, &mut master, &ctx("master"));
    if debug() {
        eprintln!(
            "render: master chain done at {:.2} s ({:.2} LUFS)",
            t0.elapsed().as_secs_f64(),
            loudness::integrated(&master)
        );
    }
    let (lufs, tp) = finalize(&mut master, p.mix.master.lufs, p.mix.master.ceiling_db);
    if debug() {
        eprintln!("render: finalized at {:.2} s", t0.elapsed().as_secs_f64());
    }
    let end = master.active_len(-80.0).max((p.beats() * tm.spb) as usize).min(master.len());
    master.truncate(end);
    let fo = ((0.5 * SRF) as usize).min(end);
    master.fade(0, fo);
    Ok(Rendered { master, stems, tracks, lufs, true_peak_db: tp, premaster_lufs })
}

fn debug() -> bool {
    std::env::var_os("CLAUDIUS_DEBUG").is_some()
}

/// gain, soft clip, limit, and a final trim if the true peak still pokes over `ceiling`
fn master_stage(src: &Stereo, gain_db: f64, ceiling: f32) -> (Stereo, f32) {
    let mut out = src.clone();
    out.scale(db_to_gain(gain_db as f32));
    fx::soft_clip(&mut out, ceiling - 0.1, 0.7, SRF);
    fx::limit(&mut out, ceiling - 0.5, 50.0, SRF);
    let mut tp = fx::true_peak_db(&out);
    if tp > ceiling - 0.02 {
        out.scale(db_to_gain(ceiling - 0.05 - tp));
        tp = ceiling - 0.05;
    }
    (out, tp)
}

/// loudness to `target` LUFS through a soft clipper and a true-peak-safe limiter (secant search on the input gain)
pub fn finalize(x: &mut Stereo, target: f32, ceiling: f32) -> (f64, f32) {
    let src = x.clone();
    let t = target as f64;
    let mut g0 = t - loudness::integrated(&src);
    let (mut out, mut tp) = master_stage(&src, g0, ceiling);
    let mut l0 = loudness::integrated(&out);
    if debug() {
        eprintln!("finalize: input {:.2} LUFS, gain {g0:+.2} -> {l0:.2} LUFS (tp {tp:.2})", loudness::integrated(&src));
    }
    let mut g1 = g0 + (t - l0);
    for _ in 0..6 {
        if (t - l0).abs() < 0.15 {
            break;
        }
        let (o, p) = master_stage(&src, g1, ceiling);
        let l1 = loudness::integrated(&o);
        if debug() {
            eprintln!("finalize: gain {g1:+.2} -> {l1:.2} LUFS (tp {p:.2})");
        }
        out = o;
        tp = p;
        let slope = if (g1 - g0).abs() > 1e-3 { ((l1 - l0) / (g1 - g0)).clamp(0.15, 1.5) } else { 1.0 };
        let next = g1 + (t - l1) / slope;
        g0 = g1;
        l0 = l1;
        g1 = next.clamp(g0 - 6.0, g0 + 6.0);
    }
    *x = out;
    (loudness::integrated(x), tp)
}
