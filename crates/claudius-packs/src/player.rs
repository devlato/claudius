//! Rendering notes with a multi-sampled instrument: zone selection (key and velocity ranges, velocity crossfades,
//! round robin, random alternatives), band-limited pitch shifting with the source rate folded in, an
//! attack / hold / decay / sustain / release envelope, sustain loops, mono and choke cuts, unison ensembles.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::decode::{self, Remote, Sample};
use crate::model::{InstrumentDef, Kind, LoopMode, Zone};
use crate::resample::Interp;

/// Output sample rate of every render.
pub const SR: u32 = 48_000;
const SRF: f64 = SR as f64;

/// One note: `start` and `gate` in output samples (48 kHz), `pitch` in semitones (60 = C4, fractional allowed),
/// `velocity` 0..1. For drums, pitch 60 plays the sample at its recorded pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteOn {
    pub start: usize,
    pub gate: usize,
    pub pitch: f64,
    pub velocity: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstrumentInfo {
    /// canonical name: `pack:instrument` (plus `:variant` for variants after the first)
    pub name: String,
    pub pack: String,
    pub instrument: String,
    pub variant: usize,
    pub variants: usize,
    pub category: String,
    /// "pitched" or "drum"
    pub kind: String,
    pub description: String,
    /// lowest / highest recorded root (MIDI)
    pub lowest: u8,
    pub highest: u8,
    pub velocity_layers: usize,
    pub round_robin: usize,
    pub zones: usize,
    pub one_shot: bool,
    pub release: f32,
    pub licence: String,
    pub attribution: String,
}

pub struct Instrument {
    pub(crate) pack: String,
    pub(crate) def: InstrumentDef,
    pub(crate) variant: usize,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) remotes: Vec<Option<Remote>>,
    pub(crate) licence: String,
    pub(crate) attribution: String,
    norm: OnceLock<f32>,
    warned: Mutex<Vec<u32>>,
}

const ATTACK_PITCHED: f32 = 0.002;
const ATTACK_DRUM: f32 = 0.0005;
const CUT_MONO: f32 = 0.004;
const CUT_CHOKE: f32 = 0.008;
const CUT_RETRIGGER: f32 = 0.12;
/// reference peak level for the loudest zone near middle C at full velocity
const NORM_DB: f32 = -6.0;

fn db(g: f32) -> f32 {
    10f32.powf(g / 20.0)
}

impl Instrument {
    pub(crate) fn new(
        pack: &str,
        def: InstrumentDef,
        variant: usize,
        files: Vec<PathBuf>,
        remotes: Vec<Option<Remote>>,
        licence: &str,
        attribution: &str,
    ) -> Instrument {
        Instrument {
            pack: pack.to_string(),
            def,
            variant,
            files,
            remotes,
            licence: licence.to_string(),
            attribution: attribution.to_string(),
            norm: OnceLock::new(),
            warned: Mutex::new(Vec::new()),
        }
    }

    fn zones(&self) -> &[Zone] {
        self.def.variants.get(self.variant).map(|v| v.zones.as_slice()).unwrap_or(&[])
    }

    pub fn info(&self) -> InstrumentInfo {
        let zones = self.zones();
        let lowest = zones.iter().map(|z| z.root.round() as i32).min().unwrap_or(60).clamp(0, 127) as u8;
        let highest = zones.iter().map(|z| z.root.round() as i32).max().unwrap_or(60).clamp(0, 127) as u8;
        let mut layers: Vec<(u8, u8)> = zones.iter().map(|z| (z.lovel, z.hivel)).collect();
        layers.sort();
        layers.dedup();
        let rr = zones.iter().map(|z| z.seq_len.max(1) as usize).max().unwrap_or(1);
        let rand = zones.iter().filter(|z| z.hirand < 1.0 || z.lorand > 0.0).count();
        let mut name = format!("{}:{}", self.pack, self.def.name);
        if self.variant > 0 {
            name.push_str(&format!(":{}", self.variant));
        }
        InstrumentInfo {
            name,
            pack: self.pack.clone(),
            instrument: self.def.name.clone(),
            variant: self.variant,
            variants: self.def.variants.len(),
            category: self.def.category.clone(),
            kind: match self.def.kind {
                Kind::Pitched => "pitched".into(),
                Kind::Drum => "drum".into(),
            },
            description: self.def.description.clone(),
            lowest,
            highest,
            velocity_layers: layers.len().max(1),
            round_robin: if rand > 0 { rr.max(2) } else { rr },
            zones: zones.len(),
            one_shot: self.def.one_shot,
            release: self.def.release,
            licence: self.licence.clone(),
            attribution: self.attribution.clone(),
        }
    }

    fn sample(&self, file: u32) -> Option<Arc<Sample>> {
        let path = self.files.get(file as usize)?;
        let remote = self.remotes.get(file as usize).and_then(|r| r.as_ref());
        match decode::load(path, remote) {
            Ok(s) => Some(s),
            Err(e) => {
                let mut w = self.warned.lock().unwrap_or_else(|e| e.into_inner());
                if !w.contains(&file) {
                    w.push(file);
                    eprintln!("claudius-packs: {}:{}: {e:#}", self.pack, self.def.name);
                }
                None
            }
        }
    }

    /// Decode (and for lazy packs download) every sample of this instrument now instead of on first use.
    pub fn preload(&self) -> anyhow::Result<()> {
        let mut files: Vec<u32> = self.zones().iter().map(|z| z.file).collect();
        files.sort();
        files.dedup();
        files.par_iter().try_for_each(|&f| {
            let path = self.files.get(f as usize).ok_or_else(|| anyhow::anyhow!("bad zone file index"))?;
            let remote = self.remotes.get(f as usize).and_then(|r| r.as_ref());
            decode::load(path, remote).map(|_| ())
        })
    }

    fn vel_gain(&self, v: u8) -> f32 {
        let c = &self.def.vel_curve;
        if !c.is_empty() {
            let v = v as f32;
            let mut prev = (0u8, 0.0f32);
            for &(pv, pg) in c {
                if v <= pv as f32 {
                    let span = (pv as f32 - prev.0 as f32).max(1.0);
                    return prev.1 + (pg - prev.1) * ((v - prev.0 as f32) / span).clamp(0.0, 1.0);
                }
                prev = (pv, pg);
            }
            return prev.1;
        }
        (v as f32 / 127.0).powf(2.0 * self.def.veltrack.clamp(0.0, 1.0))
    }

    /// level so the loudest zone nearest middle C peaks at NORM_DB at full velocity
    fn norm(&self) -> f32 {
        *self.norm.get_or_init(|| {
            let zones = self.zones();
            let reference = zones
                .iter()
                .filter(|z| z.plays_vel(127))
                .min_by(|a, b| {
                    (a.root - 60.0).abs().partial_cmp(&(b.root - 60.0).abs()).unwrap_or(std::cmp::Ordering::Equal)
                })
                .or_else(|| zones.first());
            let Some(z) = reference else { return 1.0 };
            let Some(s) = self.sample(z.file) else { return 1.0 };
            let level = s.peak * db(z.gain_db) * z.xf_gain(127).max(1e-3);
            if level < 1e-6 {
                return 1.0;
            }
            (db(NORM_DB) / level).clamp(db(-30.0), db(48.0))
        })
    }

    /// Render notes into a new 48 kHz stereo buffer of `len` samples.
    pub fn render(&self, notes: &[NoteOn], len: usize) -> (Vec<f32>, Vec<f32>) {
        let mut l = vec![0.0; len];
        let mut r = vec![0.0; len];
        self.render_into(notes, &[], &mut l, &mut r);
        (l, r)
    }

    /// Like `render`, but every voice still sounding at a choke time is cut quickly (an open hi-hat choked by the
    /// closed hi-hat's hits).
    pub fn render_choked(&self, notes: &[NoteOn], chokes: &[usize], len: usize) -> (Vec<f32>, Vec<f32>) {
        let mut l = vec![0.0; len];
        let mut r = vec![0.0; len];
        self.render_into(notes, chokes, &mut l, &mut r);
        (l, r)
    }

    /// Mix the notes into existing buffers (equal lengths). Deterministic: the same notes give the same audio.
    pub fn render_into(&self, notes: &[NoteOn], chokes: &[usize], out_l: &mut [f32], out_r: &mut [f32]) {
        let len = out_l.len().min(out_r.len());
        let voices = self.plan(notes, chokes, len);
        if voices.is_empty() {
            return;
        }
        let zones = self.zones();
        let mut files: Vec<u32> = voices.iter().map(|v| zones[v.zone].file).collect();
        files.sort();
        files.dedup();
        let samples: HashMap<u32, Arc<Sample>> =
            files.par_iter().filter_map(|&f| self.sample(f).map(|s| (f, s))).collect();
        let norm = self.norm();
        // voices render in parallel, in batches so memory stays bounded, and are summed in a fixed order
        for batch in voices.chunks(64) {
            let rendered: Vec<(usize, Vec<f32>, Vec<f32>)> = batch
                .par_iter()
                .filter_map(|v| {
                    let z = &zones[v.zone];
                    let s = samples.get(&z.file)?;
                    self.render_voice(v, z, s, norm, len)
                })
                .collect();
            for (start, vl, vr) in rendered {
                let n = vl.len().min(len.saturating_sub(start));
                for i in 0..n {
                    out_l[start + i] += vl[i];
                    out_r[start + i] += vr[i];
                }
            }
        }
    }

    fn plan(&self, notes: &[NoteOn], chokes: &[usize], len: usize) -> Vec<Voice> {
        let zones = self.zones();
        if zones.is_empty() {
            return Vec::new();
        }
        let mut order: Vec<usize> = (0..notes.len()).collect();
        order.sort_by_key(|&i| notes[i].start);
        let mut rr: HashMap<u8, u32> = HashMap::new();
        let mut voices: Vec<Voice> = Vec::new();
        let unison = self.def.unison.clone();
        for (ni, &i) in order.iter().enumerate() {
            let n = notes[i];
            if n.start >= len || !n.pitch.is_finite() {
                continue;
            }
            let key = n.pitch.round().clamp(0.0, 127.0) as u8;
            let vel = ((n.velocity.clamp(0.0, 1.0) * 127.0).round() as u8).max(1);
            let count = rr.entry(key).or_insert(0);
            let rand = hash01(n.start as u64, ni as u64, key as u64);
            let picked = select(zones, key, n.pitch, vel, *count, rand);
            *count += 1;
            let vg = self.vel_gain(vel);
            let copies: Vec<(f64, usize, f32, f32)> = match &unison {
                Some(u) if u.voices > 1 => {
                    let k = u.voices as usize;
                    (0..k)
                        .map(|j| {
                            let x = 2.0 * j as f32 / (k - 1) as f32 - 1.0;
                            let delay = (((j as f32 * 0.618_034).fract()) * u.delay_ms * 0.001 * SR as f32) as usize;
                            let pan = if j % 2 == 0 { x } else { -x } * u.spread;
                            (
                                x as f64 * u.detune_cents as f64 / 100.0,
                                if j == 0 { 0 } else { delay },
                                pan,
                                1.0 / (k as f32).sqrt(),
                            )
                        })
                        .collect()
                }
                _ => vec![(0.0, 0, 0.0, 1.0)],
            };
            for &zi in &picked {
                let z = &zones[zi];
                let base = vg * z.xf_gain(vel) * db(z.gain_db + self.def.gain_db);
                for &(detune, delay, upan, ug) in &copies {
                    let pan = (z.pan / 100.0 + upan).clamp(-1.0, 1.0);
                    let (gl, gr) = (base * ug * (1.0 - pan).min(1.0), base * ug * (1.0 + pan).min(1.0));
                    let start = n.start + delay;
                    if start >= len {
                        continue;
                    }
                    voices.push(Voice {
                        zone: zi,
                        note: ni,
                        key,
                        start,
                        gate_end: if self.def.one_shot { None } else { Some(n.start + n.gate.max(1)) },
                        semis: (n.pitch + detune - z.root as f64) * z.keytrack as f64,
                        gl,
                        gr,
                        cut: None,
                    });
                }
            }
        }
        // cuts: mono instruments, repeated keys, chokes
        if self.def.mono || self.def.retrigger {
            for a in 0..voices.len() {
                let next = voices[a + 1..].iter().find(|b| {
                    b.note != voices[a].note && b.start > voices[a].start && (self.def.mono || b.key == voices[a].key)
                });
                if let Some(b) = next {
                    let fade = if self.def.mono { CUT_MONO } else { CUT_RETRIGGER };
                    voices[a].cut = Some((b.start, fade));
                }
            }
        }
        if !chokes.is_empty() {
            let mut ch = chokes.to_vec();
            ch.sort();
            for v in &mut voices {
                if let Some(&c) = ch.iter().find(|&&c| c > v.start)
                    && v.cut.is_none_or(|(t, _)| c < t)
                {
                    v.cut = Some((c, CUT_CHOKE));
                }
            }
        }
        voices
    }

    fn render_voice(
        &self,
        v: &Voice,
        z: &Zone,
        s: &Sample,
        norm: f32,
        len: usize,
    ) -> Option<(usize, Vec<f32>, Vec<f32>)> {
        if s.is_empty() {
            return None;
        }
        let (l, r) = (&s.l[..], s.right());
        let step = 2f64.powf(v.semis / 12.0) * s.sr as f64 / SRF;
        if !step.is_finite() || step <= 0.0 {
            return None;
        }
        let src_end = if z.end > 0 { (z.end as usize).min(s.len()) } else { s.len() };
        let offset = (z.offset as usize).min(src_end);
        // loop region: the zone's, or the file's own loop for sustained instruments
        let (mode, ls, le) = match z.loop_mode {
            LoopMode::Continuous | LoopMode::Sustain if z.loop_end > z.loop_start => {
                (z.loop_mode, z.loop_start as usize, z.loop_end as usize)
            }
            LoopMode::NoLoop if !self.def.one_shot && self.def.kind == Kind::Pitched && z.loop_end == 0 => {
                match s.file_loop {
                    Some((a, b)) => (LoopMode::Sustain, a as usize, b as usize),
                    None => (LoopMode::NoLoop, 0, 0),
                }
            }
            _ => (LoopMode::NoLoop, 0, 0),
        };
        let looped =
            matches!(mode, LoopMode::Continuous | LoopMode::Sustain) && le > ls + 16 && le <= s.len() && ls >= offset;

        let attack = z.attack.unwrap_or(self.def.attack).max(if self.def.kind == Kind::Drum {
            ATTACK_DRUM
        } else {
            ATTACK_PITCHED
        });
        let att = ((attack * SR as f32) as usize).max(1);
        let hold = (z.hold * SR as f32) as usize;
        let decay = (z.decay * SR as f32) as usize;
        let sus = z.sustain.clamp(0.0, 1.0);
        let release = z.release.unwrap_or(self.def.release).max(0.005);

        // when the release starts (relative to the voice start) and how long it lasts
        let mut rel_at = usize::MAX;
        let mut rel_len = (release * SR as f32) as usize;
        if let Some(g) = v.gate_end {
            rel_at = g.saturating_sub(v.start);
        }
        if let Some((c, fade)) = v.cut {
            let c = c.saturating_sub(v.start);
            let fl = ((fade * SR as f32) as usize).max(8);
            if c < rel_at {
                rel_at = c;
                rel_len = fl;
            } else if c < rel_at.saturating_add(rel_len) {
                // already releasing: finish within the cut fade
                rel_len = rel_len.min(c - rel_at + fl);
            }
        }
        // natural length of the source from the offset, in output samples
        let natural = ((src_end - offset) as f64 / step).ceil() as usize + 1;
        let mut total = if looped && matches!(mode, LoopMode::Continuous) {
            rel_at.saturating_add(rel_len)
        } else if looped {
            // loop while held, then play on to the end of the file
            rel_at.saturating_add(natural).min(rel_at.saturating_add(rel_len))
        } else {
            natural.min(rel_at.saturating_add(rel_len))
        };
        if total == usize::MAX {
            total = natural;
        }
        let total = total.min(len.saturating_sub(v.start));
        if total == 0 {
            return None;
        }
        let gl = v.gl * norm;
        let gr = v.gr * norm;
        let mut out_l = vec![0.0f32; total];
        let mut out_r = vec![0.0f32; total];
        let mut it = Interp::new(step);
        let mut pos = offset as f64;
        let eps = 1e-3f32;
        let rel_mul = (eps.ln() / rel_len.max(1) as f32).exp();
        let dec_mul = if decay > 0 { (-5.0 / decay as f32).exp() } else { 0.0 };
        let mut dec_x = 1.0f32;
        let mut rel_x = 1.0f32;
        let mut level_at_rel = 1.0f32;
        let mut level = 0.0f32;
        for t in 0..total {
            let looping_now = looped && (mode == LoopMode::Continuous || t < rel_at);
            if looping_now && pos >= le as f64 {
                pos -= (le - ls) as f64;
            }
            if !looping_now && pos >= src_end as f64 {
                break;
            }
            // envelope
            if t < rel_at {
                level = if t < att {
                    0.5 - 0.5 * (std::f32::consts::PI * t as f32 / att as f32).cos()
                } else if t < att + hold {
                    1.0
                } else if decay > 0 && sus < 1.0 {
                    dec_x *= dec_mul;
                    sus + (1.0 - sus) * dec_x
                } else {
                    sus.max(if decay == 0 { 1.0 } else { sus })
                };
                level_at_rel = level;
            } else {
                rel_x *= rel_mul;
                let rel = ((rel_x - eps) / (1.0 - eps)).max(0.0);
                if rel <= 0.0 {
                    break;
                }
                level = level_at_rel * rel;
            }
            let (a, b) = if looping_now { it.at_looped(l, r, pos, ls, le) } else { it.at(l, r, pos) };
            out_l[t] = a * level * gl;
            out_r[t] = b * level * gr;
            pos += step;
        }
        let _ = level;
        Some((v.start, out_l, out_r))
    }
}

struct Voice {
    zone: usize,
    note: usize,
    key: u8,
    start: usize,
    gate_end: Option<usize>,
    semis: f64,
    gl: f32,
    gr: f32,
    cut: Option<(usize, f32)>,
}

/// deterministic pseudo-random number in [0, 1)
fn hash01(a: u64, b: u64, c: u64) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ c.wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// Zones that sound for a note: inside the key and velocity ranges (or the nearest root inside the velocity layer
/// when no range covers the key), then filtered by round robin position and random range.
pub(crate) fn select(zones: &[Zone], key: u8, pitch: f64, vel: u8, rr_count: u32, rand: f32) -> Vec<usize> {
    let mut cands: Vec<usize> =
        (0..zones.len()).filter(|&i| zones[i].plays_key(key) && zones[i].plays_vel(vel)).collect();
    if cands.is_empty() {
        let in_layer: Vec<usize> = (0..zones.len()).filter(|&i| zones[i].plays_vel(vel)).collect();
        let pool = if in_layer.is_empty() {
            // velocity gap: the layer closest in velocity
            let dist = |z: &Zone| if vel < z.lovel { z.lovel - vel } else { vel.saturating_sub(z.hivel) };
            let best = zones.iter().map(dist).min().unwrap_or(0);
            (0..zones.len()).filter(|&i| dist(&zones[i]) == best).collect()
        } else {
            in_layer
        };
        let nearest = pool.iter().copied().min_by(|&a, &b| {
            let da = (zones[a].root as f64 - pitch).abs();
            let db = (zones[b].root as f64 - pitch).abs();
            // on a tie prefer the zone above: pitching down sounds more natural than up
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(zones[b].root.partial_cmp(&zones[a].root).unwrap_or(std::cmp::Ordering::Equal))
        });
        let Some(nz) = nearest else { return Vec::new() };
        let (lk, hk, root) = (zones[nz].lokey, zones[nz].hikey, zones[nz].root);
        cands = pool
            .into_iter()
            .filter(|&i| zones[i].lokey == lk && zones[i].hikey == hk && zones[i].root == root)
            .collect();
    }
    let rr: Vec<usize> = cands
        .iter()
        .copied()
        .filter(|&i| {
            let z = &zones[i];
            z.seq_len <= 1 || z.seq_pos as u32 == (rr_count % z.seq_len as u32) + 1
        })
        .collect();
    let rr = if rr.is_empty() { cands.clone() } else { rr };
    let picked: Vec<usize> = rr
        .iter()
        .copied()
        .filter(|&i| {
            let z = &zones[i];
            rand >= z.lorand && (rand < z.hirand || z.hirand >= 1.0)
        })
        .collect();
    if picked.is_empty() { rr.into_iter().take(1).collect() } else { picked }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Variant;

    fn zone(file: u32, root: f32, lokey: u8, hikey: u8, lovel: u8, hivel: u8) -> Zone {
        Zone { file, root, lokey, hikey, lovel, hivel, ..Default::default() }
    }

    #[test]
    fn selection_by_key_velocity_and_round_robin() {
        let zones = vec![
            zone(0, 60.0, 0, 62, 0, 63),
            zone(1, 64.0, 63, 127, 0, 63),
            zone(2, 60.0, 0, 62, 64, 127),
            zone(3, 64.0, 63, 127, 64, 127),
        ];
        assert_eq!(select(&zones, 61, 61.0, 30, 0, 0.5), vec![0]);
        assert_eq!(select(&zones, 61, 61.0, 100, 0, 0.5), vec![2]);
        assert_eq!(select(&zones, 70, 70.0, 100, 0, 0.5), vec![3]);
        // round robin pair on one key
        let mut rr = vec![zone(0, 60.0, 0, 127, 0, 127), zone(1, 60.0, 0, 127, 0, 127)];
        rr[0].seq_len = 2;
        rr[0].seq_pos = 1;
        rr[1].seq_len = 2;
        rr[1].seq_pos = 2;
        assert_eq!(select(&rr, 60, 60.0, 90, 0, 0.0), vec![0]);
        assert_eq!(select(&rr, 60, 60.0, 90, 1, 0.0), vec![1]);
        assert_eq!(select(&rr, 60, 60.0, 90, 2, 0.0), vec![0]);
        // random alternatives
        let mut rnd = vec![zone(0, 60.0, 0, 127, 0, 127), zone(1, 60.0, 0, 127, 0, 127)];
        rnd[0].hirand = 0.5;
        rnd[1].lorand = 0.5;
        assert_eq!(select(&rnd, 60, 60.0, 90, 0, 0.2), vec![0]);
        assert_eq!(select(&rnd, 60, 60.0, 90, 0, 0.7), vec![1]);
    }

    #[test]
    fn nearest_root_when_no_range_covers_the_key() {
        // narrow zones (lokey = hikey = root) as Strudel maps produce before ranges are assigned
        let zones = vec![zone(0, 48.0, 48, 48, 0, 127), zone(1, 60.0, 60, 60, 0, 127), zone(2, 72.0, 72, 72, 0, 127)];
        assert_eq!(select(&zones, 57, 57.0, 64, 0, 0.0), vec![1]);
        assert_eq!(select(&zones, 30, 30.0, 64, 0, 0.0), vec![0]);
        // tie between 48 and 60 at 54: the zone above wins
        assert_eq!(select(&zones, 54, 54.0, 64, 0, 0.0), vec![1]);
        // velocity gap: nearest layer
        let gap = vec![zone(0, 60.0, 0, 127, 1, 40), zone(1, 60.0, 0, 127, 90, 127)];
        assert_eq!(select(&gap, 60, 60.0, 60, 0, 0.0), vec![0]);
        assert_eq!(select(&gap, 60, 60.0, 80, 0, 0.0), vec![1]);
    }

    fn test_instrument(dir: &std::path::Path, one_shot: bool, release: f32, mono: bool) -> Instrument {
        // a 0.5 s 440 Hz sine at 44.1 kHz, root A4
        let p = dir.join("a4.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&p, spec).unwrap();
        for i in 0..22050 {
            w.write_sample(((2.0 * std::f64::consts::PI * 440.0 * i as f64 / 44100.0).sin() * 16000.0) as i16).unwrap();
        }
        w.finalize().unwrap();
        let def = InstrumentDef {
            name: "sine".into(),
            category: "test".into(),
            kind: if one_shot { Kind::Drum } else { Kind::Pitched },
            description: String::new(),
            one_shot,
            release,
            attack: 0.0,
            gain_db: 0.0,
            veltrack: 1.0,
            vel_curve: Vec::new(),
            mono,
            retrigger: false,
            unison: None,
            aliases: Vec::new(),
            variants: vec![Variant { name: String::new(), zones: vec![zone(0, 69.0, 0, 127, 0, 127)] }],
        };
        Instrument::new("test", def, 0, vec![p], vec![None], "CC0-1.0", "")
    }

    fn zero_crossing_freq(x: &[f32]) -> f64 {
        let mut c = Vec::new();
        for i in 1..x.len() {
            if x[i - 1] < 0.0 && x[i] >= 0.0 {
                c.push(i);
            }
        }
        (c.len() - 1) as f64 * SRF / (c[c.len() - 1] - c[0]) as f64
    }

    #[test]
    fn pitch_gate_release_and_no_clicks() {
        let dir = tempfile::tempdir().unwrap();
        let inst = test_instrument(dir.path(), false, 0.05, false);
        // C5 (72) from an A4 root: 3 semitones up, 0.2 s gate
        let note = NoteOn { start: 4800, gate: 9600, pitch: 72.0, velocity: 1.0 };
        let (l, r) = inst.render(&[note], 48000);
        assert_eq!(l.len(), 48000);
        assert_eq!(l, r);
        assert!(l[..4800].iter().all(|v| *v == 0.0));
        let f = zero_crossing_freq(&l[6000..13000]);
        let want = 440.0 * 2f64.powf(3.0 / 12.0);
        assert!((f - want).abs() < 1.0, "{f} vs {want}");
        // peak normalised to -6 dBFS at full velocity
        let peak = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((peak - db(-6.0)).abs() < 0.03, "peak {peak}");
        // silent 50 ms after the gate + release
        assert!(l[4800 + 9600 + 2400 + 10..].iter().all(|v| v.abs() < 1e-6));
        // no clicks: the first sample is tiny and sample-to-sample steps stay small
        assert!(l[4800].abs() < 1e-3);
        let max_step = l.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        assert!(max_step < 0.12, "max step {max_step}");
        // softer velocity is quieter
        let (soft, _) = inst.render(&[NoteOn { velocity: 0.5, ..note }], 48000);
        let sp = soft.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(sp < peak * 0.4 && sp > peak * 0.15, "{sp} {peak}");
    }

    #[test]
    fn one_shot_mono_and_determinism() {
        let dir = tempfile::tempdir().unwrap();
        let inst = test_instrument(dir.path(), true, 0.05, true);
        let notes = [
            NoteOn { start: 0, gate: 10, pitch: 60.0, velocity: 1.0 },
            NoteOn { start: 9600, gate: 10, pitch: 60.0, velocity: 1.0 },
        ];
        let (l, _) = inst.render(&notes, 48000);
        // one-shot ignores the 10-sample gate: still sounding at 0.1 s
        assert!(l[4800].abs() > 0.0 || l[4801].abs() > 0.0);
        // mono: the first hit is cut at the second, so the sum around 0.3 s equals a single hit
        let (single, _) = inst.render(&notes[1..], 48000);
        for i in 9600 + 400..20000 {
            assert!((l[i] - single[i]).abs() < 1e-5);
        }
        let (again, _) = inst.render(&notes, 48000);
        assert_eq!(l, again);
        // choke: an open-hat style cut 0.05 s after the start
        let (ch, _) = inst.render_choked(&notes[..1], &[2400], 48000);
        assert!(ch[2400 + 800..].iter().all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn instrument_is_send_and_sync() {
        fn ok<T: Send + Sync>() {}
        ok::<Instrument>();
    }
}
