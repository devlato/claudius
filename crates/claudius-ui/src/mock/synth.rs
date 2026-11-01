//! The demo's render of a mock song, a slice per frame (no threads, so it runs on the web too): drums from the step
//! patterns, bass, chords, pad, lead and arp from the progressions, risers and impacts from the section flags.
//!
//! With the demo's sounds (`kit`), each part plays its preset and each drum lane its genre's drum machine, as
//! claudius renders them, and every track is set to the loudness claudius' mix gives its role; without them, a
//! rough synth stands in. Tracks go through their level and pan into stereo, duck under the kick as far as their
//! `duck` says, and send into a ping-pong delay and a reverb; the master is soft-clipped and brought up to just
//! under full scale.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_4, SQRT_2, TAU};

use claudius_api::{PartView, SongView};

use super::kit::{Kit, Need, Sound};
use crate::audio::Pcm;
use crate::music::{self, Key};

pub const RATE: u32 = 48_000;
const SR: f32 = RATE as f32;
/// the sends' ring: longer than any note the demo plays, plus how far the notes run ahead of the effects
const RING: usize = 1 << 21;
/// effects are kept at most this far behind the notes, so the ring never laps them
const FX_LAG: usize = 12 * RATE as usize;
/// frames of effects or of the master a step processes
const CHUNK: usize = 1 << 16;
/// the longest riser
const RISER_MAX: f32 = 16.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Voice {
    Kick,
    Snare,
    Clap,
    Hat,
    OpenHat,
    Shaker,
    Ride,
    Perc,
    Bass,
    Stab,
    Pad,
    Pluck,
    Lead,
    Riser,
    Impact,
    Crash,
}

struct Ev {
    at: usize,
    len: usize,
    voice: Voice,
    /// MIDI notes; none for drums and effects
    notes: Vec<i32>,
    gain: f32,
    track: usize,
    /// what the kit plays it with, in `Render::sounds`
    sound: Option<usize>,
}

/// a track's level and pan in each channel, its sends, and how far it ducks under the kick
#[derive(Clone, Copy, Default)]
struct Mix {
    left: f32,
    right: f32,
    verb: f32,
    echo: f32,
    duck: f32,
}

#[derive(PartialEq)]
enum Stage {
    Notes,
    Master,
    Done,
}

/// the loudness claudius' mix gives each role while it plays, before the master (`render::target_level`)
fn target_level(role: &str) -> f32 {
    match role {
        "kick" => -12.5,
        "snare" => -13.5,
        "clap" => -14.5,
        "hat" => -17.5,
        "ohat" => -18.5,
        "shaker" => -21.5,
        "perc" => -19.5,
        "ride" => -20.5,
        "bass" => -14.5,
        "chords" => -18.0,
        "keys" => -17.0,
        "pad" => -21.5,
        "lead" => -18.0,
        "arp" => -20.5,
        "top" => -22.0,
        _ => -19.0,
    }
}

/// how loud claudius masters a genre (its `mix.lufs`)
pub(crate) fn master_level(genre: &str) -> f32 {
    match genre {
        "lofi" => -12.5,
        "deep_house" | "trip_hop" => -11.0,
        "synthwave" | "art_rock" | "dub_techno" => -10.5,
        "trap" | "techno" | "future_bass" | "filter_house" => -9.0,
        "dnb" | "big_beat" | "dance_pop" => -8.5,
        "electro_house" => -8.0,
        _ => -10.0,
    }
}

/// BS.1770's K-weighting at 48 kHz, as `claudius::loudness` does it: the ear's loudness, which a mix is levelled by
#[derive(Clone, Copy, Default)]
struct KWeight {
    z: [f64; 2],
    w: [f64; 2],
}

impl KWeight {
    fn power(&mut self, x: f32) -> f64 {
        let (b1, a1) = (
            [1.535_124_859_586_97, -2.691_696_189_406_38, 1.198_392_810_852_85],
            [-1.690_659_293_182_41, 0.732_480_774_215_85],
        );
        let a2 = [-1.990_047_454_833_98, 0.990_072_250_366_21];
        let v = x as f64;
        let y = b1[0] * v + self.z[0];
        self.z[0] = b1[1] * v - a1[0] * y + self.z[1];
        self.z[1] = b1[2] * v - a1[1] * y;
        let u = y + self.w[0];
        self.w[0] = -2.0 * y - a2[0] * u + self.w[1];
        self.w[1] = y - a2[1] * u;
        u * u
    }
}

/// the drum machine a genre plays: the 707's house and techno, the 808's trap, lo-fi and future bass
fn machine(genre: &str) -> &'static str {
    match genre {
        "trap" | "lofi" | "future_bass" | "trip_hop" => "808",
        _ => "707",
    }
}

/// the kit's sound for a drum lane: the genre's machine's, else the other machine's
fn drum_sound(kit: &Kit, genre: &str, lane: &str) -> Option<String> {
    let first = machine(genre);
    let other = if first == "707" { "808" } else { "707" };
    [first, other].iter().map(|m| format!("{m}:{lane}")).find(|s| kit.has(s))
}

/// the notes a part reaches: its register, an octave more for leads and arps
fn reach(p: &PartView) -> (i32, i32) {
    let up = if matches!(p.role.as_str(), "lead" | "arp") { 12 } else { 0 };
    (p.register.0, p.register.1 + up)
}

/// everything a song plays from the kit, to fetch before it is rendered
pub fn needs(song: &SongView, kit: &Kit) -> Vec<Need> {
    let mut out = Vec::new();
    let mut add = |sound: Option<String>, (low, high): (i32, i32)| {
        if let Some(sound) = sound.filter(|s| kit.has(s)) {
            let need = Need { sound, low, high };
            if !out.contains(&need) {
                out.push(need);
            }
        }
    };
    for d in &song.drums {
        add(drum_sound(kit, &song.genre, &d.lane), (60, 60));
    }
    if song.tracks.iter().any(|t| t.kind == "loop") {
        add(drum_sound(kit, &song.genre, "shaker"), (60, 60));
    }
    for p in &song.parts {
        add(Some(p.preset.clone()), reach(p));
    }
    for s in ["fx:impact".to_string(), "fx:riser".to_string()] {
        add(Some(s), (60, 60));
    }
    add(drum_sound(kit, &song.genre, "crash"), (60, 60));
    out
}

pub struct Render {
    /// interleaved stereo: the dry tracks, then the effects added, then the master
    out: Vec<f32>,
    /// the reverb's and the delay's sends: rings, emptied as the effects take them
    verb: Vec<f32>,
    echo: Vec<f32>,
    frames: usize,
    events: Vec<Ev>,
    next: usize,
    sounds: Vec<Sound>,
    mix: Vec<Mix>,
    /// the frames the kick hits at, for the tracks that duck under it
    kicks: Vec<usize>,
    /// how long a duck takes to recover
    duck_release: f32,
    gaps: Vec<(usize, usize)>,
    tracks: Vec<String>,
    energy: Vec<f64>,
    /// frames each track plays over, for its loudness
    active: Vec<f64>,
    /// the loudness each track was set to (LUFS while it plays), for the meters; None for the synth's tracks
    set_to: Vec<Option<f64>>,
    stage: Stage,
    fx: Effects,
    fx_at: usize,
    /// the master's K-weighted energy, to bring it to `target`
    weight: [KWeight; 2],
    weighted: f64,
    target: f32,
    /// its gain into the soft clip, once it is measured
    gain: f32,
    master_at: usize,
    /// the notes and hits the synth plays, for want of the kit's sounds
    pub from_synth: usize,
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

/// the gain at frame `t` of a track that ducks `depth` under kicks at `kicks`, `next` the index of the first kick
/// after the previous frame asked about (each note of a chord asks from its start again)
fn ducked(kicks: &[usize], next: &mut usize, t: usize, depth: f32, release: f32) -> f32 {
    while *next > 0 && kicks[*next - 1] > t {
        *next -= 1;
    }
    while *next < kicks.len() && kicks[*next] <= t {
        *next += 1;
    }
    let Some(&k) = kicks.get(next.wrapping_sub(1)).filter(|_| *next > 0) else { return 1.0 };
    let since = (t - k) as f32 / release;
    if since >= 1.0 { 1.0 } else { 1.0 - depth * (1.0 - since) * (1.0 - since) }
}

impl Render {
    pub fn new(song: &SongView, kit: &Kit) -> Render {
        let key = Key::parse(&song.key).unwrap_or(Key { tonic: 9, mode: 1 });
        let beat = 60.0 / song.bpm.max(30.0) as f32;
        let step = beat / 4.0;
        let swing = (song.swing - 0.5).max(0.0) * 2.0 * step;
        // and the reverb's tail
        let frames = ((song.bars as f32 * 16.0 * step + 2.5) * SR) as usize;
        let tracks: Vec<String> = song.tracks.iter().map(|t| t.name.clone()).collect();
        let track_index: HashMap<&str, usize> =
            song.tracks.iter().enumerate().map(|(i, t)| (t.name.as_str(), i)).collect();
        let solo = song.tracks.iter().any(|t| t.solo);
        let playing = |name: &str| -> Option<usize> {
            let i = *track_index.get(name)?;
            let t = &song.tracks[i];
            (!t.muted && (!solo || t.solo)).then_some(i)
        };

        // the kit's sounds, as far as they have arrived
        let mut sounds = Vec::new();
        let mut sound_index: HashMap<Need, usize> = HashMap::new();
        for need in needs(song, kit) {
            if let Some(s) = kit.sound(&need) {
                sound_index.insert(need, sounds.len());
                sounds.push(s);
            }
        }
        // by the notes too: two parts on one preset play it from the files for their own registers
        let sound_at = |name: Option<String>, (low, high): (i32, i32)| {
            name.and_then(|sound| sound_index.get(&Need { sound, low, high }).copied())
        };
        let sound_of = |name: Option<String>| sound_at(name, (60, 60));

        let mut events = Vec::new();
        let mut gaps = Vec::new();
        let mut kicks = Vec::new();
        let at = |bar: u32, s: u32| -> usize {
            let t = (bar * 16 + s) as f32 * step + if s % 2 == 1 { swing } else { 0.0 };
            (t * SR) as usize
        };
        let secs = |x: f32| (x * SR) as usize;
        let mut active = vec![0f64; tracks.len()];
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
            let sec_frames = (sec.bars as f32 * 16.0 * step * SR) as f64;
            let mut playing_here = vec![false; tracks.len()];
            for d in &song.drums {
                let Some(track) = playing(&d.lane).filter(|_| on(&d.lane)) else { continue };
                playing_here[track] = true;
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
                let sound = sound_of(drum_sound(kit, &song.genre, &d.lane));
                // with the kit, velocity and the section's energy shape the hits and the track gets its role's level
                let base = if sound.is_some() { 1.0 } else { base };
                for bar in 0..sec.bars {
                    for s in 0..16u32 {
                        let v = pattern
                            .get(((bar as usize % (len / 16).max(1)) * 16 + s as usize) % len.max(1))
                            .copied()
                            .unwrap_or(0.0);
                        if v > 0.0 {
                            let t = at(sec.start_bar + bar, s);
                            if voice == Voice::Kick {
                                kicks.push(t);
                            }
                            events.push(Ev {
                                at: t,
                                len: secs(dur),
                                voice,
                                notes: Vec::new(),
                                gain: v * base * sec_gain,
                                track,
                                sound,
                            });
                        }
                    }
                    let fill = sec.fills && bar % 8 == 7 && matches!(d.lane.as_str(), "snare" | "clap");
                    if fill {
                        for s in 12..16u32 {
                            events.push(Ev {
                                at: at(sec.start_bar + bar, s),
                                len: secs(dur),
                                voice,
                                notes: Vec::new(),
                                gain: (0.4 + 0.15 * (s - 12) as f32) * base,
                                track,
                                sound,
                            });
                        }
                    }
                }
            }
            for p in &song.parts {
                let Some(track) = playing(&p.name).filter(|_| on(&p.name)) else { continue };
                playing_here[track] = true;
                let sound = sound_at(Some(p.preset.clone()), reach(p));
                let unit = |synth: f32| if sound.is_some() { sec_gain } else { synth * sec_gain };
                let (lo, hi) = p.register;
                let center = (lo + hi) / 2;
                let mut note = |at: usize, len: usize, voice: Voice, notes: Vec<i32>, gain: f32| {
                    events.push(Ev { at, len, voice, notes, gain, track, sound });
                };
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
                            let root = bass_note(numeral, &key, lo);
                            for (k, &s) in steps.iter().enumerate() {
                                let next = steps.get(k + 1).copied().unwrap_or(16);
                                let walk = if p.style == "walking" { [0, 3, 5, 7][k % 4] } else { 0 };
                                let dur = ((next - s) as f32 * step * 0.85).max(step * 0.8);
                                note(at(b, s), secs(dur), Voice::Bass, vec![root + walk], unit(0.5));
                            }
                        }
                        "pad" => note(at(b, 0), bar_len, Voice::Pad, voicing(numeral, &key, center), unit(0.5)),
                        "lead" => {
                            let tones = voicing(numeral, &key, center + 5);
                            let rhythm = [0u32, 3, 6, 8, 10, 12];
                            for (k, &s) in rhythm.iter().enumerate() {
                                if (k + bar as usize) % 5 == 4 {
                                    continue;
                                }
                                let n = tones[(k * 2 + bar as usize) % tones.len()] + if k == 3 { 12 } else { 0 };
                                let next = rhythm.get(k + 1).copied().unwrap_or(16);
                                note(at(b, s), secs((next - s) as f32 * step * 0.9), Voice::Lead, vec![n], unit(0.5));
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
                                note(at(b, s), secs(step * 1.6), Voice::Pluck, vec![tones[i]], unit(0.45));
                            }
                        }
                        _ => {
                            let chord = voicing(numeral, &key, center);
                            match p.style.as_str() {
                                "sustain" | "half" => {
                                    let half = if p.style == "half" { 2 } else { 1 };
                                    for h in 0..half {
                                        let len = bar_len / half as usize;
                                        note(at(b, h * 8), len, Voice::Pad, chord.clone(), unit(0.55));
                                    }
                                }
                                style => {
                                    let steps: &[u32] = match style {
                                        "charleston" => &[0, 3],
                                        "syncopated" | "pump" => &[0, 3, 6, 10, 12],
                                        _ => &[2, 6, 10, 14],
                                    };
                                    for &s in steps {
                                        note(at(b, s), secs(0.28), Voice::Stab, chord.clone(), unit(0.6));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for t in song.tracks.iter().filter(|t| t.kind == "loop") {
                let Some(track) = playing(&t.name).filter(|_| on(&t.name)) else { continue };
                playing_here[track] = true;
                let sound = sound_of(drum_sound(kit, &song.genre, "shaker"));
                for bar in 0..sec.bars {
                    for s in 0..16u32 {
                        let g = match s % 4 {
                            2 => 0.55,
                            _ if s % 2 == 1 => 0.35,
                            _ => 0.22,
                        };
                        events.push(Ev {
                            at: at(sec.start_bar + bar, s),
                            len: secs(0.06),
                            voice: Voice::Shaker,
                            notes: Vec::new(),
                            gain: g * sec_gain,
                            track,
                            sound,
                        });
                    }
                }
            }
            if let Some(track) = playing("fx") {
                let start = at(sec.start_bar, 0);
                let end = at(sec.start_bar + sec.bars, 0);
                if sec.impact {
                    active[track] += (1.6 * SR) as f64;
                    let impact = sound_of(Some("fx:impact".into()));
                    let crash = sound_of(drum_sound(kit, &song.genre, "crash"));
                    for (voice, len, gain, sound) in
                        [(Voice::Impact, 1.6, 0.8, impact), (Voice::Crash, 1.8, 0.6, crash)]
                    {
                        events.push(Ev { at: start, len: secs(len), voice, notes: Vec::new(), gain, track, sound });
                    }
                }
                if sec.build {
                    let bars = sec.bars.min(8);
                    let from = at(sec.start_bar + sec.bars - bars, 0).max(end.saturating_sub(secs(RISER_MAX)));
                    active[track] += end.saturating_sub(from) as f64;
                    events.push(Ev {
                        at: from,
                        len: end.saturating_sub(from),
                        voice: Voice::Riser,
                        notes: Vec::new(),
                        gain: 0.7,
                        track,
                        sound: sound_of(Some("fx:riser".into())),
                    });
                }
                if sec.gap {
                    gaps.push((end.saturating_sub(secs(4.0 * step)), end));
                }
            }
            for (a, here) in active.iter_mut().zip(&playing_here) {
                if *here {
                    *a += sec_frames;
                }
            }
        }
        events.sort_by_key(|e| e.at);
        kicks.sort_unstable();

        // the tracks the kit plays get their role's loudness: from each sound's loudness and how much it plays
        let mut level = vec![1.0f32; tracks.len()];
        let mut power = vec![0f64; tracks.len()];
        for ev in &events {
            let Some(s) = ev.sound.map(|i| &sounds[i]) else { continue };
            let notes: &[i32] = if ev.notes.is_empty() { &[60] } else { &ev.notes };
            for &n in notes {
                let Some((clip, ratio)) = s.pick(n as f32) else { continue };
                let energy = if s.pitched {
                    // the clip read `ratio` times as fast: held, then fading over its release (a third of its energy)
                    let (held, gone) = (ev.len as f32 / SR * ratio, (ev.len as f32 / SR + s.release.max(0.03)) * ratio);
                    let until_held = clip.energy_until(held);
                    (until_held + (clip.energy_until(gone) - until_held) / 3.0) / ratio
                } else {
                    clip.energy_until(clip.secs)
                };
                power[ev.track] += (ev.gain as f64).powi(2) * energy as f64;
            }
        }
        let mut mix = vec![Mix::default(); tracks.len()];
        let mut set_to = vec![None; tracks.len()];
        for (i, t) in song.tracks.iter().enumerate() {
            let kit_track = events.iter().any(|e| e.track == i && e.sound.is_some());
            if kit_track && power[i] > 0.0 && active[i] > 0.0 {
                let lufs = 10.0 * (power[i] / (active[i] / SR as f64)).log10() - 0.691;
                let target = if t.kind == "fx" { -17.0 } else { target_level(&t.role) };
                level[i] = db(target - lufs as f32).clamp(0.01, 30.0);
                set_to[i] = Some(f64::from(target + t.level_db));
            }
            let theta = (t.pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
            let g = level[i] * db(t.level_db);
            mix[i] = Mix {
                left: g * theta.cos() * SQRT_2,
                right: g * theta.sin() * SQRT_2,
                verb: g * t.reverb.clamp(0.0, 1.0),
                echo: g * t.delay.clamp(0.0, 1.0),
                duck: if t.role == "kick" { 0.0 } else { t.duck.clamp(0.0, 1.0) * 0.8 },
            };
        }
        let from_synth = events.iter().filter(|e| e.sound.is_none()).count();
        let n_tracks = tracks.len();
        Render {
            out: vec![0.0; frames * 2],
            verb: vec![0.0; RING],
            echo: vec![0.0; RING],
            frames,
            from_synth,
            events,
            next: 0,
            sounds,
            mix,
            kicks,
            duck_release: beat * 0.45 * SR,
            gaps,
            tracks,
            energy: vec![0.0; n_tracks],
            active,
            set_to,
            stage: Stage::Notes,
            fx: Effects::new(beat),
            fx_at: 0,
            weight: [KWeight::default(); 2],
            weighted: 0.0,
            target: master_level(&song.genre),
            gain: 1.0,
            master_at: 0,
        }
    }

    pub fn done(&self) -> bool {
        self.stage == Stage::Done
    }

    /// renders about `budget` voice-samples, or a slice of the effects or the master; returns the progress 0..1
    pub fn step(&mut self, budget: usize) -> f32 {
        let mut spent = 0;
        match self.stage {
            Stage::Notes => {
                while spent < budget && self.next < self.events.len() {
                    let at = self.events[self.next].at;
                    // the effects keep up, so that the sends' ring never laps them
                    if at > self.fx_at + FX_LAG {
                        self.effects(at - FX_LAG / 2);
                    }
                    spent += self.render_event(self.next);
                    self.next += 1;
                }
                let frontier = self.events.get(self.next).map_or(self.frames, |e| e.at);
                self.effects((self.fx_at + CHUNK).min(frontier));
                if self.next >= self.events.len() && self.fx_at >= self.frames {
                    self.stage = Stage::Master;
                }
            }
            Stage::Master => {
                if self.master_at == 0 {
                    let lufs = 10.0 * (self.weighted / self.frames.max(1) as f64).max(1e-12).log10() - 0.691;
                    // the soft clip gives back about 3 dB: 1.6 below its knee, more where it bends the peaks
                    self.gain = db((self.target + 3.0 - lufs as f32).clamp(-30.0, 30.0));
                    self.weighted = 0.0;
                    self.weight = [KWeight::default(); 2];
                }
                let end = (self.master_at + CHUNK * 2).min(self.frames);
                let gain = self.gain;
                // the soft clip is the limiter: it never passes 0.89 (-1 dBFS)
                for frame in self.out[self.master_at * 2..end * 2].chunks_exact_mut(2) {
                    for (c, s) in frame.iter_mut().enumerate() {
                        *s = 0.89 * soft_clip(*s * gain / 1.07);
                        self.weighted += self.weight[c].power(*s);
                    }
                }
                self.master_at = end;
                if end >= self.frames {
                    self.stage = Stage::Done;
                }
            }
            Stage::Done => {}
        }
        let notes = if self.events.is_empty() { 1.0 } else { self.next as f32 / self.events.len() as f32 };
        let fx = self.fx_at as f32 / self.frames.max(1) as f32;
        let master = self.master_at as f32 / self.frames.max(1) as f32;
        0.6 * notes + 0.3 * fx + 0.1 * master
    }

    /// renders one event into the dry mix and the sends; returns the voice-samples it took
    fn render_event(&mut self, i: usize) -> usize {
        let ev = &self.events[i];
        let mix = self.mix[ev.track];
        let release = self.duck_release;
        let mut next_kick = self.kicks.partition_point(|&k| k <= ev.at);
        let (mut energy, mut spent) = (0f64, 0usize);
        let mut put = |out: &mut [f32], verb: &mut [f32], echo: &mut [f32], f: usize, l: f32, r: f32| {
            let g = if mix.duck > 0.0 { ducked(&self.kicks, &mut next_kick, f, mix.duck, release) } else { 1.0 };
            let (l, r) = (l * g, r * g);
            let (left, right) = (l * mix.left, r * mix.right);
            out[2 * f] += left;
            out[2 * f + 1] += right;
            let mono = (l + r) * 0.5;
            verb[f & (RING - 1)] += mono * mix.verb;
            echo[f & (RING - 1)] += mono * mix.echo;
            energy += ((left * left + right * right) * 0.5) as f64;
        };
        let room = self.frames.saturating_sub(ev.at);
        match ev.sound.map(|s| &self.sounds[s]) {
            Some(sound) => {
                let notes: &[i32] = if ev.notes.is_empty() { &[60] } else { &ev.notes };
                let fade = (sound.release.max(0.03) * SR) as usize;
                let attack = (0.002 * SR) as usize;
                for &n in notes {
                    let Some((clip, ratio)) = sound.pick(n as f32) else { continue };
                    let ratio = if ev.voice == Voice::Riser {
                        (clip.len() as f32 / ev.len.max(1) as f32).clamp(0.5, 2.0)
                    } else {
                        ratio
                    };
                    let whole = (clip.len() as f32 / ratio) as usize;
                    let frames = if sound.pitched { (ev.len + fade).min(whole) } else { whole }.min(room);
                    spent += frames;
                    for k in 0..frames {
                        let (l, r) = if ratio == 1.0 {
                            (clip.l[k], clip.r.get(k).copied().unwrap_or(clip.l[k]))
                        } else {
                            clip.linear(k as f32 * ratio)
                        };
                        let mut env = if k < attack { k as f32 / attack as f32 } else { 1.0 };
                        if sound.pitched && k > ev.len {
                            env *= 1.0 - (k - ev.len) as f32 / fade as f32;
                        }
                        let g = env * ev.gain;
                        put(&mut self.out, &mut self.verb, &mut self.echo, ev.at + k, l * g, r * g);
                    }
                }
            }
            None => {
                let len = ev.len.min(room);
                spent += len * ev.notes.len().max(1);
                let mut voice = SynthVoice::new(ev);
                for k in 0..len {
                    let s = voice.next(k, len) * ev.gain;
                    put(&mut self.out, &mut self.verb, &mut self.echo, ev.at + k, s, s);
                }
            }
        }
        self.energy[ev.track] += energy;
        spent
    }

    /// the delay, the reverb and the soft clip, for the frames before `to` (every note there has been rendered)
    fn effects(&mut self, to: usize) {
        let to = to.min(self.frames);
        let mut gap = self.gaps.partition_point(|g| g.1 <= self.fx_at);
        for f in self.fx_at..to {
            while gap < self.gaps.len() && self.gaps[gap].1 <= f {
                gap += 1;
            }
            let quiet = self.gaps.get(gap).is_some_and(|g| g.0 <= f) as u8 as f32 * 0.98;
            let k = f & (RING - 1);
            let (v, e) = (self.verb[k] * (1.0 - quiet), self.echo[k] * (1.0 - quiet));
            self.verb[k] = 0.0;
            self.echo[k] = 0.0;
            let (el, er) = self.fx.delay(e);
            let (rl, rr) = self.fx.reverb.tick(v + (el + er) * 0.2);
            for (c, wet) in [(0, el + rl), (1, er + rr)] {
                let x = self.out[2 * f + c] * (1.0 - quiet) + wet;
                self.weighted += self.weight[c].power(x);
                self.out[2 * f + c] = x;
            }
        }
        self.fx_at = self.fx_at.max(to);
    }

    /// the master (normalized), its waveform, each track's loudness while it plays, and the master's loudness
    pub fn finish(self) -> (Pcm, Vec<f32>, Vec<(String, f64)>, f64) {
        let gain = f64::from(self.gain * 0.89 / 1.07);
        let loudness = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| self.energy[*i] > 0.0 && self.active[*i] > 0.0)
            .map(|(i, t)| {
                let lufs = match self.set_to[i] {
                    Some(l) => l + 20.0 * gain.log10(),
                    None => 10.0 * (self.energy[i] / self.active[i] * gain * gain).max(1e-12).log10() - 0.691,
                };
                (t.clone(), lufs)
            })
            .collect();
        let master_lufs = 10.0 * (self.weighted / self.frames.max(1) as f64).max(1e-12).log10() - 0.691;
        let points = 1000;
        let bucket = (self.frames / points).max(1);
        let waveform =
            self.out.chunks(bucket * 2).map(|c| c.iter().fold(0f32, |m, s| m.max(s.abs()))).take(points).collect();
        (Pcm { rate: RATE, channels: 2, samples: self.out }, waveform, loudness, master_lufs)
    }
}

/// tanh, near enough: smooth up to full scale, and never past it
fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

/// the ping-pong delay (a dotted eighth, damped repeats) and the reverb the sends go into
struct Effects {
    left: Vec<f32>,
    right: Vec<f32>,
    pos: usize,
    lp_l: f32,
    lp_r: f32,
    reverb: Reverb,
}

impl Effects {
    fn new(beat: f32) -> Effects {
        let len = ((beat * 0.75 * SR) as usize).max(1);
        Effects { left: vec![0.0; len], right: vec![0.0; len], pos: 0, lp_l: 0.0, lp_r: 0.0, reverb: Reverb::new() }
    }

    fn delay(&mut self, input: f32) -> (f32, f32) {
        let (yl, yr) = (self.left[self.pos], self.right[self.pos]);
        self.lp_l += 0.4 * (yr - self.lp_l);
        self.lp_r += 0.4 * (yl - self.lp_r);
        self.left[self.pos] = input + self.lp_l * 0.35;
        self.right[self.pos] = self.lp_r * 0.35;
        self.pos = (self.pos + 1) % self.left.len();
        (yl, yr)
    }
}

struct Comb {
    buf: Vec<f32>,
    i: usize,
    store: f32,
}

struct Allpass {
    buf: Vec<f32>,
    i: usize,
}

/// Freeverb: eight damped combs into four allpasses a side, the right side's a little longer
struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    /// the input, with its lows taken out (300 Hz), so the reverb does not muddy the bass
    hp: f32,
    hp_in: f32,
}

impl Reverb {
    fn new() -> Reverb {
        let scale = SR / 44_100.0;
        let side = |spread: usize| {
            let combs = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617]
                .iter()
                .map(|&n| Comb { buf: vec![0.0; ((n + spread) as f32 * scale) as usize], i: 0, store: 0.0 })
                .collect();
            let allpasses = [556, 441, 341, 225]
                .iter()
                .map(|&n| Allpass { buf: vec![0.0; ((n + spread) as f32 * scale) as usize], i: 0 })
                .collect();
            (combs, allpasses)
        };
        let ((cl, al), (cr, ar)) = (side(0), side(23));
        Reverb { combs: [cl, cr], allpasses: [al, ar], hp: 0.0, hp_in: 0.0 }
    }

    fn tick(&mut self, x: f32) -> (f32, f32) {
        let a = (-TAU * 300.0 / SR).exp();
        self.hp = a * (self.hp + x - self.hp_in);
        self.hp_in = x;
        let input = self.hp * 0.015;
        let mut out = [0.0f32; 2];
        for (side, o) in out.iter_mut().enumerate() {
            let mut sum = 0.0;
            for c in &mut self.combs[side] {
                let y = c.buf[c.i];
                c.store = y * 0.75 + c.store * 0.25;
                c.buf[c.i] = input + c.store * 0.84;
                c.i = (c.i + 1) % c.buf.len();
                sum += y;
            }
            for ap in &mut self.allpasses[side] {
                let b = ap.buf[ap.i];
                ap.buf[ap.i] = sum + b * 0.5;
                ap.i = (ap.i + 1) % ap.buf.len();
                sum = b - sum;
            }
            *o = sum;
        }
        (out[0], out[1])
    }
}

/// the rough synth the demo plays without its sounds
struct SynthVoice {
    voice: Voice,
    freqs: Vec<f32>,
    noise: u32,
    prev: f32,
    band: (f32, f32),
    phase: f32,
    lp: f32,
}

impl SynthVoice {
    fn new(ev: &Ev) -> SynthVoice {
        SynthVoice {
            voice: ev.voice,
            freqs: ev.notes.iter().map(|&n| hz(n as f32)).collect(),
            noise: 0x9e37_79b9 ^ (ev.at as u32).wrapping_mul(2_654_435_761) | 1,
            prev: 0.0,
            band: (0.0, 0.0),
            phase: 0.0,
            lp: 0.0,
        }
    }

    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// noise with its lows taken out: the sizzle of hats and cymbals
    fn bright(&mut self) -> f32 {
        let x = self.white();
        let y = x - self.prev;
        self.prev = x;
        y * 0.5
    }

    /// noise around `hz`: the body of a clap or a snare, without the fizz above it
    fn banded(&mut self, hz: f32) -> f32 {
        let x = self.white();
        let f = 2.0 * (std::f32::consts::PI * hz / SR).sin();
        let (low, band) = self.band;
        let low = low + f * band;
        let band = band + f * (x - low - 0.7 * band);
        self.band = (low, band);
        band
    }

    fn next(&mut self, i: usize, len: usize) -> f32 {
        let t = i as f32 / SR;
        let total = len as f32 / SR;
        let release = |r: f32| ((total - t) / r).clamp(0.0, 1.0);
        let f0 = self.freqs.first().copied().unwrap_or(110.0);
        match self.voice {
            Voice::Kick => {
                let f = 48.0 + 110.0 * (-t * 30.0).exp();
                self.phase += TAU * f / SR;
                self.phase.sin() * (-t * 7.5).exp()
            }
            Voice::Snare => {
                self.banded(1800.0) * (-t * 22.0).exp() * 0.5 + (TAU * 185.0 * t).sin() * (-t * 28.0).exp() * 0.35
            }
            Voice::Clap => {
                let burst = [0.0, 0.011, 0.022]
                    .iter()
                    .map(|o| if t >= *o { (-(t - o) * 140.0).exp() } else { 0.0 })
                    .sum::<f32>();
                let tail = if t > 0.03 { (-(t - 0.03) * 16.0).exp() * 0.6 } else { 0.0 };
                self.banded(1200.0) * (burst + tail) * 0.45
            }
            Voice::Hat => self.bright() * (-t * 70.0).exp() * 0.35,
            Voice::OpenHat => self.bright() * (-t * 9.0).exp() * 0.28,
            Voice::Shaker => self.bright() * (t / 0.005).min(1.0) * (-t * 45.0).exp() * 0.25,
            Voice::Ride => self.bright() * (-t * 5.0).exp() * 0.12,
            // a soft conga: a low knock that falls in pitch, under the hats rather than over them
            Voice::Perc => {
                let f = 190.0 + 70.0 * (-t * 30.0).exp();
                self.phase += TAU * f / SR;
                self.phase.sin() * (-t * 14.0).exp() * 0.3
            }
            Voice::Bass => {
                self.phase = (self.phase + f0 / SR).fract();
                let saw = 2.0 * self.phase - 1.0;
                self.lp += (1.0 - (-TAU * 520.0 / SR).exp()) * (saw - self.lp);
                (self.lp * 0.6 + (TAU * f0 * t).sin() * 0.5) * (t / 0.004).min(1.0) * release(0.012)
            }
            Voice::Stab => {
                let tri = |f: f32| 2.0 * (2.0 * (f * t).fract() - 1.0).abs() - 1.0;
                self.freqs.iter().map(|&f| tri(f)).sum::<f32>() * 0.16 * (-t * 7.0).exp() * (t / 0.003).min(1.0)
            }
            Voice::Pad => {
                let env = (t / 0.25).min(1.0) * release(0.3);
                self.freqs.iter().map(|&f| (TAU * f * t).sin() + (TAU * f * 1.004 * t).sin()).sum::<f32>() * 0.06 * env
            }
            Voice::Pluck => {
                self.phase = (self.phase + f0 / SR).fract();
                let cutoff = 400.0 + 3000.0 * (-t * 18.0).exp();
                self.lp += (1.0 - (-TAU * cutoff / SR).exp()) * ((2.0 * self.phase - 1.0) - self.lp);
                self.lp * (-t * 12.0).exp() * 0.3
            }
            Voice::Lead => {
                let vib = 1.0 + 0.004 * (TAU * 5.5 * t).sin();
                self.phase += TAU * f0 * vib / SR;
                let env = (t / 0.01).min(1.0) * release(0.04);
                (self.phase.sin() + 0.3 * (3.0 * self.phase).sin()) * env * 0.2
            }
            Voice::Riser => {
                let x = t / total.max(0.01);
                self.bright() * x * x * 0.5 + (TAU * (200.0 + 1400.0 * x) * t).sin() * x * 0.04
            }
            Voice::Impact => {
                (TAU * (40.0 + 60.0 * (-t * 6.0).exp()) * t).sin() * (-t * 2.2).exp()
                    + self.white() * (-t * 9.0).exp() * 0.25
            }
            Voice::Crash => self.bright() * (-t * 2.5).exp() * 0.35,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::kit::{Clip, TestSound};
    use crate::mock::song;

    /// a decaying tone, measured as the demo's files are (unweighted, which a test of levels can afford)
    fn tone(freq: f32, secs: f32, decay: f32) -> Clip {
        let n = (secs * SR) as usize;
        let l: Vec<f32> =
            (0..n).map(|i| i as f32 / SR).map(|t| (TAU * freq * t).sin() * (-t * decay).exp() * 0.5).collect();
        let energy = crate::mock::kit::TIMES
            .iter()
            .map(|t| l.iter().take((t * SR) as usize).map(|x| x * x).sum::<f32>() / SR)
            .collect();
        Clip { secs, l, r: Vec::new(), energy }
    }

    /// a kit with a sound for everything a demo song plays
    fn full_kit() -> Kit {
        let mut sounds: Vec<TestSound> = Vec::new();
        let presets = [
            "sub_bass",
            "808",
            "reese",
            "acid",
            "saw_bass",
            "pluck_bass",
            "organ_bass",
            "wobble",
            "pluck",
            "supersaw",
            "pad",
            "warm_pad",
            "stab",
            "epiano",
            "keys",
            "bell",
            "organ",
            "strings",
            "square_lead",
            "chip",
        ];
        for p in presets {
            let clips = (24..=90).step_by(6).map(|n| (n, tone(hz(n as f32), 3.0, 0.5))).collect();
            sounds.push((p, true, 0.2, clips));
        }
        let drums = [
            "707:kick",
            "707:snare",
            "707:clap",
            "707:hat",
            "707:ohat",
            "707:ride",
            "707:crash",
            "808:kick",
            "808:snare",
            "808:clap",
            "808:hat",
            "808:ohat",
            "808:shaker",
            "808:perc",
            "808:crash",
        ];
        // short hits, as drums are: sixteenth-note hats do not run into each other
        for d in drums {
            sounds.push((d, false, 0.0, vec![(60, tone(200.0, 0.09, 40.0))]));
        }
        sounds.push(("fx:impact", false, 0.0, vec![(60, tone(50.0, 2.0, 2.0))]));
        sounds.push(("fx:riser", false, 0.0, vec![(60, tone(400.0, 8.0, 0.0))]));
        Kit::with(sounds)
    }

    fn finished(mut r: Render) -> (Pcm, Vec<f32>, Vec<(String, f64)>, f64) {
        while !r.done() {
            r.step(5_000_000);
        }
        r.finish()
    }

    #[test]
    fn without_its_sounds_the_demo_plays_its_synth() {
        let song = song::generate("house", 7, None, None, false);
        let r = Render::new(&song, &Kit::at(None, None));
        assert!(r.from_synth > 0);
        let (pcm, waveform, loudness, _) = finished(r);
        assert_eq!((pcm.rate, pcm.channels), (48_000, 2));
        assert!(pcm.samples.iter().all(|s| s.is_finite()));
        let peak = pcm.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((0.85..=0.9).contains(&peak), "the master just under full scale: {peak}");
        assert_eq!(waveform.len(), 1000);
        assert!(loudness.iter().any(|(t, _)| t == "kick"));
    }

    #[test]
    fn with_its_sounds_each_track_plays_from_the_kit_at_its_roles_level() {
        let song = song::generate("house", 7, None, None, false);
        let r = Render::new(&song, &full_kit());
        assert_eq!(r.from_synth, 0, "every note and hit from the kit");
        let (pcm, _, loudness, _) = finished(r);
        assert!(pcm.samples.iter().all(|s| s.is_finite()));
        let level = |t: &str| loudness.iter().find(|(n, _)| n == t).map(|(_, l)| *l).expect(t);
        // claudius' mix: kick -12.5, clap -14.5, hat -17.5, bass -14.5, pad -21.5 LUFS while they play
        let kick = level("kick");
        for (t, under) in [("clap", 2.0), ("hat", 5.0), ("bass", 2.0), ("pad", 9.0)] {
            let d = kick - level(t);
            assert!((d - under).abs() < 3.0, "{t} is {d:.1} dB under the kick, and claudius' mix puts it {under}");
        }
    }

    #[test]
    fn parts_on_one_preset_play_it_from_the_files_for_their_own_notes() {
        let mut song = song::generate("house", 7, None, None, false);
        let bass = song.parts.iter().position(|p| p.role == "bass").unwrap();
        let above = (0..song.parts.len()).max_by_key(|&i| song.parts[i].register.0).unwrap();
        assert!(song.parts[above].register.0 > song.parts[bass].register.1);
        song.parts[above].preset = song.parts[bass].preset.clone();
        let r = Render::new(&song, &full_kit());
        for p in [&song.parts[bass], &song.parts[above]] {
            let (low, high) = reach(p);
            let track = song.tracks.iter().position(|t| t.name == p.name).unwrap();
            let sound = r.events.iter().find(|e| e.track == track).and_then(|e| e.sound).expect(&p.name);
            let notes: Vec<i32> = r.sounds[sound].clips.iter().map(|(n, _)| *n).collect();
            assert!(
                notes.iter().all(|n| (low - 6..=high + 6).contains(n)),
                "{} reaches {low}..{high}: {notes:?}",
                p.name
            );
        }
    }

    #[test]
    fn each_genre_is_mastered_as_loud_as_claudius_masters_it() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../claudius/data/genres");
        for g in song::GENRES {
            let file = std::fs::read(dir.join(format!("{}.json", g.name))).unwrap();
            let genre: serde_json::Value = serde_json::from_slice(&file).unwrap();
            assert_eq!(Some(master_level(g.name) as f64), genre["mix"]["lufs"].as_f64(), "{}", g.name);
        }
    }
}
