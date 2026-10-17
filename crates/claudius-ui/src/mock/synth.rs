//! A rough preview render of a mock song, synthesized a slice per frame (no threads, so it runs on the web too):
//! drums from the step patterns, bass / chords / pad / lead / arp from the progressions, risers and impacts from
//! the section flags. It is not claudius' sound, just enough to try playback, seeking, looping and the waveform.

use std::collections::HashMap;
use std::f32::consts::TAU;

use claudius_api::SongView;

use crate::audio::Pcm;
use crate::music::{self, Key};

pub const RATE: u32 = 24_000;
const SR: f32 = RATE as f32;

#[derive(Clone)]
enum Voice {
    Kick,
    Snare,
    Clap,
    Hat,
    OpenHat,
    Shaker,
    Ride,
    Perc,
    Bass(f32),
    Stab(Vec<f32>),
    Pad(Vec<f32>),
    Pluck(f32),
    Lead(f32),
    Riser,
    Impact,
    Crash,
}

struct Ev {
    at: usize,
    len: usize,
    voice: Voice,
    gain: f32,
    track: usize,
}

pub struct Render {
    out: Vec<f32>,
    events: Vec<Ev>,
    next: usize,
    gaps: Vec<(usize, usize)>,
    tracks: Vec<String>,
    energy: Vec<f64>,
}

fn hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

fn db(x: f32) -> f32 {
    10f32.powf(x / 20.0)
}

/// the chord tones of a numeral as MIDI notes around `center`
fn voicing(numeral: &str, key: &Key, center: i32) -> Vec<i32> {
    let chord = music::chord(numeral, key).unwrap_or(music::Chord {
        root: key.tonic,
        tones: vec![0, 3, 7],
        name: String::new(),
    });
    let mut root = chord.root;
    while root + 12 <= center - 6 {
        root += 12;
    }
    while root > center + 6 {
        root -= 12;
    }
    chord.tones.iter().map(|t| root + t).collect()
}

fn bass_note(numeral: &str, key: &Key, lo: i32) -> i32 {
    let root = music::chord(numeral, key).map_or(key.tonic, |c| c.root);
    let mut n = root;
    while n < lo {
        n += 12;
    }
    while n >= lo + 12 {
        n -= 12;
    }
    n
}

impl Render {
    pub fn new(song: &SongView) -> Render {
        let key = Key::parse(&song.key).unwrap_or(Key { tonic: 9, mode: 1 });
        let step = 60.0 / song.bpm.max(30.0) as f32 / 4.0;
        let swing = (song.swing - 0.5).max(0.0) * 2.0 * step;
        let total = ((song.bars as f32 * 16.0 * step + 2.0) * SR) as usize;
        let tracks: Vec<String> = song.tracks.iter().map(|t| t.name.clone()).collect();
        let track_index: HashMap<&str, usize> =
            song.tracks.iter().enumerate().map(|(i, t)| (t.name.as_str(), i)).collect();
        let mut events = Vec::new();
        let mut gaps = Vec::new();
        let at = |bar: u32, s: u32| -> usize {
            let t = (bar * 16 + s) as f32 * step + if s % 2 == 1 { swing } else { 0.0 };
            (t * SR) as usize
        };
        let secs = |x: f32| (x * SR) as usize;
        let solo = song.tracks.iter().any(|t| t.solo);
        let level = |name: &str| -> Option<(usize, f32)> {
            let i = *track_index.get(name)?;
            let t = &song.tracks[i];
            (!t.muted && (!solo || t.solo)).then(|| (i, db(t.level_db)))
        };

        for sec in &song.sections {
            let prog_name = if sec.prog.is_empty() { "A" } else { sec.prog.as_str() };
            let chords: Vec<String> = song
                .progressions
                .iter()
                .find(|(n, _)| n == prog_name)
                .or(song.progressions.first())
                .map(|(_, c)| c.clone())
                .unwrap_or_else(|| vec!["i".into()]);
            let sec_gain = 0.55 + 0.45 * sec.energy;
            let on = |layer: &str| sec.layers.iter().any(|l| l == layer);
            for d in &song.drums {
                let Some((track, gain)) = level(&d.lane).filter(|_| on(&d.lane)) else { continue };
                let pattern = d
                    .variants
                    .iter()
                    .find(|(v, _)| *v == sec.variant)
                    .or_else(|| d.variants.first())
                    .map(|(_, s)| s.as_slice())
                    .unwrap_or(&[]);
                let len = (d.pattern_bars.max(1) * 16) as usize;
                let (voice, base, dur) = match d.lane.as_str() {
                    "kick" => (Voice::Kick, 1.0, 0.38),
                    "snare" => (Voice::Snare, 0.75, 0.22),
                    "clap" => (Voice::Clap, 0.7, 0.24),
                    "hat" => (Voice::Hat, 0.55, 0.05),
                    "ohat" => (Voice::OpenHat, 0.45, 0.3),
                    "shaker" => (Voice::Shaker, 0.45, 0.07),
                    "ride" => (Voice::Ride, 0.5, 0.6),
                    _ => (Voice::Perc, 0.5, 0.12),
                };
                for bar in 0..sec.bars {
                    for s in 0..16u32 {
                        let v = pattern
                            .get(((bar as usize % (len / 16).max(1)) * 16 + s as usize) % len.max(1))
                            .copied()
                            .unwrap_or(0.0);
                        if v > 0.0 {
                            events.push(Ev {
                                at: at(sec.start_bar + bar, s),
                                len: secs(dur),
                                voice: voice.clone(),
                                gain: v * base * gain * sec_gain,
                                track,
                            });
                        }
                    }
                    let fill = sec.fills && bar % 8 == 7 && matches!(d.lane.as_str(), "snare" | "clap");
                    if fill {
                        for s in 12..16u32 {
                            events.push(Ev {
                                at: at(sec.start_bar + bar, s),
                                len: secs(dur),
                                voice: voice.clone(),
                                gain: (0.4 + 0.15 * (s - 12) as f32) * base * gain,
                                track,
                            });
                        }
                    }
                }
            }
            for p in &song.parts {
                let Some((track, gain)) = level(&p.name).filter(|_| on(&p.name)) else { continue };
                let gain = gain * sec_gain;
                let (lo, hi) = p.register;
                let center = (lo + hi) / 2;
                for bar in 0..sec.bars {
                    let numeral = &chords[(bar as usize) % chords.len()];
                    let b = sec.start_bar + bar;
                    let bar_len = secs(16.0 * step);
                    match p.role.as_str() {
                        "bass" => {
                            let steps: &[u32] = match p.style.as_str() {
                                "offbeat" => &[2, 6, 10, 14],
                                "syncopated" | "kick_follow" => &[0, 3, 6, 10, 13],
                                "rolling" => &[1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15],
                                "walking" => &[0, 4, 8, 12],
                                "sustain" | "reese" => &[0],
                                _ => &[0, 2, 4, 6, 8, 10, 12, 14],
                            };
                            let note = bass_note(numeral, &key, lo);
                            for (k, &s) in steps.iter().enumerate() {
                                let next = steps.get(k + 1).copied().unwrap_or(16);
                                let walk = if p.style == "walking" { [0, 3, 5, 7][k % 4] } else { 0 };
                                let dur = ((next - s) as f32 * step * 0.85).max(step * 0.8);
                                events.push(Ev {
                                    at: at(b, s),
                                    len: secs(dur),
                                    voice: Voice::Bass(hz((note + walk) as f32)),
                                    gain: 0.5 * gain,
                                    track,
                                });
                            }
                        }
                        "pad" => {
                            let f: Vec<f32> = voicing(numeral, &key, center).iter().map(|&n| hz(n as f32)).collect();
                            events.push(Ev {
                                at: at(b, 0),
                                len: bar_len,
                                voice: Voice::Pad(f),
                                gain: 0.5 * gain,
                                track,
                            });
                        }
                        "lead" => {
                            let tones = voicing(numeral, &key, center + 5);
                            let rhythm = [0u32, 3, 6, 8, 10, 12];
                            for (k, &s) in rhythm.iter().enumerate() {
                                if (k + bar as usize) % 5 == 4 {
                                    continue;
                                }
                                let n = tones[(k * 2 + bar as usize) % tones.len()] + if k == 3 { 12 } else { 0 };
                                let next = rhythm.get(k + 1).copied().unwrap_or(16);
                                events.push(Ev {
                                    at: at(b, s),
                                    len: secs((next - s) as f32 * step * 0.9),
                                    voice: Voice::Lead(hz(n as f32)),
                                    gain: 0.5 * gain,
                                    track,
                                });
                            }
                        }
                        "arp" => {
                            let mut tones = voicing(numeral, &key, center);
                            tones.extend(tones.clone().iter().map(|n| n + 12));
                            for s in 0..16u32 {
                                let i = if p.style == "updown" && (s / tones.len() as u32) % 2 == 1 {
                                    tones.len() - 1 - s as usize % tones.len()
                                } else {
                                    s as usize % tones.len()
                                };
                                events.push(Ev {
                                    at: at(b, s),
                                    len: secs(step * 1.6),
                                    voice: Voice::Pluck(hz(tones[i] as f32)),
                                    gain: 0.45 * gain,
                                    track,
                                });
                            }
                        }
                        _ => {
                            let f: Vec<f32> = voicing(numeral, &key, center).iter().map(|&n| hz(n as f32)).collect();
                            match p.style.as_str() {
                                "sustain" | "half" => {
                                    let half = if p.style == "half" { 2 } else { 1 };
                                    for h in 0..half {
                                        events.push(Ev {
                                            at: at(b, h * 8),
                                            len: bar_len / half as usize,
                                            voice: Voice::Pad(f.clone()),
                                            gain: 0.55 * gain,
                                            track,
                                        });
                                    }
                                }
                                style => {
                                    let steps: &[u32] = match style {
                                        "charleston" => &[0, 3],
                                        "syncopated" | "pump" => &[0, 3, 6, 10, 12],
                                        _ => &[2, 6, 10, 14],
                                    };
                                    for &s in steps {
                                        events.push(Ev {
                                            at: at(b, s),
                                            len: secs(0.28),
                                            voice: Voice::Stab(f.clone()),
                                            gain: 0.6 * gain,
                                            track,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for t in song.tracks.iter().filter(|t| t.kind == "loop") {
                let Some((track, gain)) = level(&t.name).filter(|_| on(&t.name)) else { continue };
                for bar in 0..sec.bars {
                    for s in 0..16u32 {
                        let g = if s % 4 == 2 {
                            0.55
                        } else if s % 2 == 1 {
                            0.35
                        } else {
                            0.22
                        };
                        events.push(Ev {
                            at: at(sec.start_bar + bar, s),
                            len: secs(0.06),
                            voice: Voice::Shaker,
                            gain: g * gain * sec_gain,
                            track,
                        });
                    }
                }
            }
            if let Some((track, gain)) = level("fx") {
                let start = at(sec.start_bar, 0);
                let end = at(sec.start_bar + sec.bars, 0);
                if sec.impact {
                    events.push(Ev { at: start, len: secs(1.6), voice: Voice::Impact, gain: 0.8 * gain, track });
                    events.push(Ev { at: start, len: secs(1.8), voice: Voice::Crash, gain: 0.6 * gain, track });
                }
                if sec.build {
                    let bars = sec.bars.min(8);
                    let from = at(sec.start_bar + sec.bars - bars, 0);
                    events.push(Ev {
                        at: from,
                        len: end.saturating_sub(from),
                        voice: Voice::Riser,
                        gain: 0.7 * gain,
                        track,
                    });
                }
                if sec.gap {
                    gaps.push((end.saturating_sub(secs(4.0 * step)), end));
                }
            }
        }
        events.sort_by_key(|e| e.at);
        let n_tracks = tracks.len();
        Render { out: vec![0.0; total], events, next: 0, gaps, tracks, energy: vec![0.0; n_tracks] }
    }

    pub fn done(&self) -> bool {
        self.next >= self.events.len()
    }

    /// renders about `budget` voice-samples; returns the progress 0..1
    pub fn step(&mut self, budget: usize) -> f32 {
        let mut spent = 0;
        while spent < budget && self.next < self.events.len() {
            let ev = &self.events[self.next];
            spent += ev.len * voices(&ev.voice);
            let energy = render(&mut self.out, ev);
            self.energy[ev.track] += energy;
            self.next += 1;
        }
        if self.events.is_empty() { 1.0 } else { self.next as f32 / self.events.len() as f32 }
    }

    /// the master (gaps cut, soft clipped, normalized), its waveform, and each track's loudness
    pub fn finish(mut self) -> (Pcm, Vec<f32>, Vec<(String, f64)>, f64) {
        for (a, b) in &self.gaps {
            let b = (*b).min(self.out.len());
            for s in &mut self.out[(*a).min(b)..b] {
                *s *= 0.02;
            }
        }
        let drive = 1.6f32;
        let norm = drive.tanh();
        let mut peak = 0f32;
        for s in &mut self.out {
            *s = (*s * drive).tanh() / norm;
            peak = peak.max(s.abs());
        }
        let gain = if peak > 0.0 { 0.9 / peak } else { 1.0 };
        let mut sum_sq = 0f64;
        for s in &mut self.out {
            *s *= gain;
            sum_sq += (*s as f64) * (*s as f64);
        }
        let n = self.out.len().max(1) as f64;
        let master_lufs = 10.0 * (sum_sq / n).max(1e-12).log10() - 0.691;
        let loudness = self
            .tracks
            .iter()
            .zip(&self.energy)
            .filter(|(_, e)| **e > 0.0)
            .map(|(t, e)| (t.clone(), 10.0 * (e * (gain as f64).powi(2) / n).max(1e-12).log10() - 0.691))
            .collect();
        let points = 1000;
        let bucket = (self.out.len() / points).max(1);
        let waveform =
            self.out.chunks(bucket).map(|c| c.iter().fold(0f32, |m, s| m.max(s.abs()))).take(points).collect();
        (Pcm { rate: RATE, channels: 1, samples: self.out }, waveform, loudness, master_lufs)
    }
}

fn voices(v: &Voice) -> usize {
    match v {
        Voice::Stab(f) => f.len(),
        Voice::Pad(f) => f.len() * 2,
        _ => 1,
    }
}

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// adds one event into `out`; returns its energy (sum of squares) for the track's loudness
fn render(out: &mut [f32], ev: &Ev) -> f64 {
    let mut noise = Noise(0x9e37_79b9 ^ (ev.at as u32).wrapping_mul(2_654_435_761) | 1);
    let mut prev = 0f32;
    let mut hp = |n: &mut Noise| {
        let x = n.next();
        let y = x - prev;
        prev = x;
        y * 0.5
    };
    let len = ev.len.min(out.len().saturating_sub(ev.at));
    let mut phase = 0f32;
    let mut lp = 0f32;
    let mut energy = 0f64;
    let release = |t: f32, total: f32, r: f32| ((total - t) / r).clamp(0.0, 1.0);
    let total = len as f32 / SR;
    for i in 0..len {
        let t = i as f32 / SR;
        let s = match &ev.voice {
            Voice::Kick => {
                let f = 48.0 + 110.0 * (-t * 30.0).exp();
                phase += TAU * f / SR;
                phase.sin() * (-t * 7.5).exp() + if t < 0.002 { noise.next() * 0.3 } else { 0.0 }
            }
            Voice::Snare => {
                hp(&mut noise) * (-t * 20.0).exp() * 1.1 + (TAU * 185.0 * t).sin() * (-t * 28.0).exp() * 0.45
            }
            Voice::Clap => {
                let burst = [0.0, 0.011, 0.022]
                    .iter()
                    .map(|o| if t >= *o { (-(t - o) * 140.0).exp() } else { 0.0 })
                    .sum::<f32>();
                let tail = if t > 0.03 { (-(t - 0.03) * 16.0).exp() * 0.6 } else { 0.0 };
                hp(&mut noise) * (burst + tail) * 1.2
            }
            Voice::Hat => hp(&mut noise) * (-t * 70.0).exp() * 0.7,
            Voice::OpenHat => hp(&mut noise) * (-t * 9.0).exp() * 0.55,
            Voice::Shaker => hp(&mut noise) * (t / 0.005).min(1.0) * (-t * 45.0).exp() * 0.45,
            Voice::Ride => {
                hp(&mut noise) * (-t * 5.0).exp() * 0.25 + (TAU * 3100.0 * t).sin() * (-t * 4.0).exp() * 0.05
            }
            Voice::Perc => (TAU * (900.0 + 300.0 * (-t * 40.0).exp()) * t).sin() * (-t * 28.0).exp() * 0.35,
            Voice::Bass(f) => {
                phase = (phase + f / SR).fract();
                let saw = 2.0 * phase - 1.0;
                lp += (1.0 - (-TAU * 520.0 / SR).exp()) * (saw - lp);
                let env = (t / 0.004).min(1.0) * release(t, total, 0.012);
                (lp * 0.6 + (TAU * f * t).sin() * 0.5) * env
            }
            Voice::Stab(fs) => {
                let tri = |f: f32| 2.0 * (2.0 * (f * t).fract() - 1.0).abs() - 1.0;
                fs.iter().map(|&f| tri(f)).sum::<f32>() * 0.16 * (-t * 7.0).exp() * (t / 0.003).min(1.0)
            }
            Voice::Pad(fs) => {
                let env = (t / 0.25).min(1.0) * release(t, total, 0.3);
                fs.iter().map(|&f| (TAU * f * t).sin() + (TAU * f * 1.004 * t).sin()).sum::<f32>() * 0.06 * env
            }
            Voice::Pluck(f) => {
                phase = (phase + f / SR).fract();
                let cutoff = 400.0 + 3000.0 * (-t * 18.0).exp();
                lp += (1.0 - (-TAU * cutoff / SR).exp()) * ((2.0 * phase - 1.0) - lp);
                lp * (-t * 12.0).exp() * 0.3
            }
            Voice::Lead(f) => {
                let vib = 1.0 + 0.004 * (TAU * 5.5 * t).sin();
                phase += TAU * f * vib / SR;
                let env = (t / 0.01).min(1.0) * release(t, total, 0.04);
                (phase.sin() + 0.3 * (3.0 * phase).sin() + 0.12 * (5.0 * phase).sin()) * env * 0.2
            }
            Voice::Riser => {
                let x = t / total.max(0.01);
                hp(&mut noise) * x * x * 0.5 + (TAU * (200.0 + 1400.0 * x) * t).sin() * x * 0.04
            }
            Voice::Impact => {
                (TAU * (40.0 + 60.0 * (-t * 6.0).exp()) * t).sin() * (-t * 2.2).exp()
                    + noise.next() * (-t * 9.0).exp() * 0.25
            }
            Voice::Crash => hp(&mut noise) * (-t * 2.5).exp() * 0.35,
        } * ev.gain;
        out[ev.at + i] += s;
        energy += (s * s) as f64;
    }
    energy
}
