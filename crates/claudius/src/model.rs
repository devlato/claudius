//! The two documents claudius works with.
//!
//! A `Blueprint` is a song's genome: tempo, key, progressions, the form (sections and which layers play in each),
//! drum lanes with their step patterns, melodic parts with their writing style, every sound, and the mix. Theory
//! writes it, mutation edits it, analysis of a reference song produces one.
//!
//! A `Project` is the arranged timeline the composer expands a blueprint into, like a DAW session: tracks with notes
//! or audio clips, effect chains with automation, buses and the master. Rendering a project is deterministic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fx::Effect;
use crate::synth::drums::{DrumParams, DrumVoice};
use crate::synth::voice::Patch;
use crate::theory::chord::Voicing;
use crate::theory::rhythm::Hit;
use crate::theory::scale::Key;
use crate::theory::{ChordSpan, Note};

/// where a sound comes from: always audio, either rendered in code or recorded
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sound {
    /// a procedural drum one-shot
    Drum { voice: DrumVoice, params: DrumParams },
    /// the synth engine with a patch (a copy of a factory preset, possibly mutated)
    Synth { preset: String, patch: Patch },
    /// a recorded one-shot played like a sampler (pitch follows the note: `root` plays it as recorded)
    Sample {
        path: String,
        #[serde(default = "middle_c")]
        root: i32,
        #[serde(default)]
        start: f64,
        #[serde(default)]
        end: Option<f64>,
        #[serde(default)]
        label: String,
    },
    /// a transition effect rendered in code at the note's length (riser, downlifter, impact, reverse_crash,
    /// sweep_up, sweep_down)
    Fx { fx: String },
    /// a multi-sampled instrument from an installed sample pack (`claudius-packs`): `name` is its canonical
    /// `pack:instrument`, `label` what it was asked for by ("piano", "bd" in bank "tr909")
    Instrument {
        name: String,
        #[serde(default)]
        label: String,
    },
    /// a recorded loop, warped to the project tempo and transposed to its key
    Loop {
        path: String,
        bpm: f64,
        beats: f64,
        #[serde(default)]
        key: Option<u8>,
        #[serde(default)]
        tonal: bool,
        #[serde(default)]
        label: String,
    },
}

fn middle_c() -> i32 {
    60
}

impl Sound {
    pub fn describe(&self) -> String {
        match self {
            Sound::Drum { voice, .. } => format!("synth {}", voice.name()),
            Sound::Synth { preset, .. } => format!("synth patch '{preset}'"),
            Sound::Sample { path, label, .. } => {
                if label.is_empty() {
                    short_path(path)
                } else {
                    label.clone()
                }
            }
            Sound::Fx { fx } => format!("synth {fx}"),
            Sound::Instrument { name, label } => {
                if label.is_empty() || label == name {
                    format!("sampled {name}")
                } else {
                    format!("sampled {label} ({name})")
                }
            }
            Sound::Loop { path, label, bpm, .. } => {
                format!("loop {} ({bpm:.0} BPM)", if label.is_empty() { short_path(path) } else { label.clone() })
            }
        }
    }
}

fn short_path(p: &str) -> String {
    std::path::Path::new(p).file_name().map_or_else(|| p.to_string(), |f| f.to_string_lossy().into_owned())
}

fn bus_music() -> String {
    "music".into()
}

/// per-track mix settings: level relative to the role's target loudness, pan, insert effects, sends, sidechain
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackMix {
    #[serde(default)]
    pub level_db: f32,
    #[serde(default)]
    pub pan: f32,
    #[serde(default)]
    pub fx: Vec<Effect>,
    #[serde(default)]
    pub reverb: f32,
    #[serde(default)]
    pub delay: f32,
    /// sidechain ducking depth from the kick (0..1)
    #[serde(default)]
    pub duck: f32,
    #[serde(default = "bus_music")]
    pub bus: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
    /// when any track is soloed, only soloed tracks play
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solo: bool,
}

impl Default for TrackMix {
    fn default() -> Self {
        TrackMix {
            level_db: 0.0,
            pan: 0.0,
            fx: Vec::new(),
            reverb: 0.0,
            delay: 0.0,
            duck: 0.0,
            bus: bus_music(),
            mute: false,
            solo: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Intro,
    Verse,
    Build,
    Drop,
    Break,
    Hook,
    Groove,
    Outro,
}

impl SectionKind {
    pub fn name(self) -> &'static str {
        match self {
            SectionKind::Intro => "intro",
            SectionKind::Verse => "verse",
            SectionKind::Build => "build",
            SectionKind::Drop => "drop",
            SectionKind::Break => "break",
            SectionKind::Hook => "hook",
            SectionKind::Groove => "groove",
            SectionKind::Outro => "outro",
        }
    }
}

fn main_variant() -> String {
    "main".into()
}
fn prog_a() -> String {
    "A".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionSpec {
    pub name: String,
    pub kind: SectionKind,
    pub bars: u32,
    pub energy: f32,
    #[serde(default = "prog_a")]
    pub prog: String,
    /// names of the drum lanes, parts and loops that play in this section
    pub layers: Vec<String>,
    /// drum pattern variant for the section (main / alt / sparse / half)
    #[serde(default = "main_variant")]
    pub variant: String,
    /// open a lowpass across the section on the music bus
    #[serde(default)]
    pub filter_in: bool,
    /// riser, snare roll and highpass sweep into the next section
    #[serde(default)]
    pub build: bool,
    /// a beat of silence before the next section
    #[serde(default)]
    pub gap: bool,
    /// crash + impact on the downbeat
    #[serde(default)]
    pub impact: bool,
    /// drum fills at the end of every 8 bars
    #[serde(default)]
    pub fills: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumLane {
    pub name: String,
    pub sound: Sound,
    /// variant name -> hits over `pattern_bars` bars
    pub patterns: BTreeMap<String, Vec<Hit>>,
    pub pattern_bars: u32,
    #[serde(default)]
    pub pitch: i32,
    #[serde(default)]
    pub choke: Option<String>,
    #[serde(default)]
    pub mix: TrackMix,
}

impl DrumLane {
    /// what the lane plays (kick, snare, hat ...): its name when that is a drum word ("bd" and "kick" are both the
    /// kick), else its sound. The role picks the lane's level, its fills and the kick the bass locks to.
    pub fn role(&self) -> String {
        if let Some(r) = crate::script::drum_role(&self.name) {
            return r;
        }
        match &self.sound {
            Sound::Drum { voice, .. } => voice.role().into(),
            Sound::Instrument { label, .. } => crate::script::drum_role(label).unwrap_or_else(|| self.name.clone()),
            _ => self.name.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Bass,
    Chords,
    Pad,
    Lead,
    Arp,
    Keys,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Bass => "bass",
            Role::Chords => "chords",
            Role::Pad => "pad",
            Role::Lead => "lead",
            Role::Arp => "arp",
            Role::Keys => "keys",
        }
    }
}

fn close() -> Voicing {
    Voicing::Close
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub role: Role,
    pub sound: Sound,
    /// bass style, chord rhythm, arp pattern or lead flavour
    pub style: String,
    pub register: (i32, i32),
    #[serde(default)]
    pub density: f32,
    #[serde(default)]
    pub voices: usize,
    #[serde(default = "close")]
    pub voicing: Voicing,
    /// arp step in beats
    #[serde(default)]
    pub rate: f64,
    /// the part's own seed: changing it rewrites only this part
    pub seed: u64,
    /// notes written by hand or by analysis, per progression name (cycle-relative beats); overrides generation
    #[serde(default)]
    pub notes: BTreeMap<String, Vec<Note>>,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoopPart {
    pub name: String,
    pub role: String,
    pub sound: Sound,
    /// semitones applied to the loop (tonal loops follow the project key)
    #[serde(default)]
    pub transpose: i32,
    /// slice order for chopping (indices of `slice_beats`-long slices); empty = play as recorded
    #[serde(default)]
    pub chop: Vec<i32>,
    #[serde(default)]
    pub slice_beats: f64,
    #[serde(default)]
    pub mix: TrackMix,
}

/// a track written as a pattern expression (see `script`): the code is the source of truth, `sounds` the sound each
/// label in it resolved to when it was written
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternTrack {
    pub name: String,
    pub code: String,
    #[serde(default)]
    pub sounds: BTreeMap<String, Sound>,
    /// mixing role when the pattern plays one sound (empty: each sound gets the role of its name)
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transitions {
    pub crash: Sound,
    #[serde(default)]
    pub riser: Option<Sound>,
    #[serde(default)]
    pub impact: Option<Sound>,
    #[serde(default = "yes")]
    pub use_impact: bool,
    #[serde(default = "yes")]
    pub use_downlifter: bool,
    #[serde(default = "yes")]
    pub use_reverse_crash: bool,
    /// riser length in bars (capped by the build section)
    #[serde(default = "eight")]
    pub riser_bars: u32,
    #[serde(default)]
    pub mix: TrackMix,
}

fn yes() -> bool {
    true
}
fn eight() -> u32 {
    8
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Master {
    pub fx: Vec<Effect>,
    pub lufs: f32,
    #[serde(default = "ceiling")]
    pub ceiling_db: f32,
}

fn ceiling() -> f32 {
    -1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixPlan {
    pub reverb: Effect,
    pub delay: Effect,
    /// bus name -> insert chain (drums, bass, music, fx)
    pub buses: BTreeMap<String, Vec<Effect>>,
    /// sidechain recovery time in beats
    #[serde(default = "quarter")]
    pub duck_release: f32,
    /// role -> dB offset from the default target loudness of that role
    #[serde(default)]
    pub levels: BTreeMap<String, f32>,
    pub master: Master,
}

fn quarter() -> f32 {
    0.3
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Harmony {
    pub progressions: BTreeMap<String, Vec<String>>,
    pub bars_per_chord: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blueprint {
    pub version: u32,
    pub title: String,
    pub genre: String,
    pub seed: u64,
    pub bpm: f64,
    pub key: Key,
    pub swing: f32,
    #[serde(default)]
    pub humanize: f32,
    pub harmony: Harmony,
    pub form: Vec<SectionSpec>,
    pub drums: Vec<DrumLane>,
    pub parts: Vec<Part>,
    #[serde(default)]
    pub loops: Vec<LoopPart>,
    #[serde(default)]
    pub patterns: Vec<PatternTrack>,
    pub transitions: Transitions,
    pub mix: MixPlan,
    #[serde(default)]
    pub lineage: Vec<String>,
}

/// the longest song claudius makes, in bars and in time, and the most tracks it plays
pub const MAX_BARS: u32 = 512;
pub const MAX_SECONDS: f64 = 20.0 * 60.0;
pub const MAX_TRACKS: usize = 64;
/// tracks times seconds: a render holds every track in memory, 384 KB a second at 48 kHz, so this keeps a render
/// under about 4.6 GB (20 tracks of 10 minutes, or 40 of 5)
pub const MAX_TRACK_SECONDS: f64 = 12_000.0;

/// an error when a song with this many tracks, this long, would take more memory to render than claudius allows
pub fn check_tracks(tracks: usize, secs: f64) -> Result<(), String> {
    if tracks > MAX_TRACKS {
        return Err(format!("the song has {tracks} tracks; the limit is {MAX_TRACKS}"));
    }
    if tracks as f64 * secs > MAX_TRACK_SECONDS {
        return Err(format!(
            "{tracks} tracks of {:.1} minutes are more than claudius renders at once ({:.0} track-minutes, such as 20 \
             tracks of 10 minutes): use fewer tracks or a shorter form",
            secs / 60.0,
            MAX_TRACK_SECONDS / 60.0
        ));
    }
    Ok(())
}

impl Blueprint {
    /// an error when the song is longer or has more tracks than claudius renders (`MAX_BARS`, `MAX_SECONDS`,
    /// `check_tracks`)
    pub fn check_size(&self) -> Result<(), String> {
        let (bars, secs) = (self.bars(), self.secs());
        if bars > MAX_BARS {
            return Err(format!("the song is {bars} bars long; the limit is {MAX_BARS}"));
        }
        if secs > MAX_SECONDS {
            return Err(format!("the song is {:.0} minutes long; the limit is {:.0}", secs / 60.0, MAX_SECONDS / 60.0));
        }
        check_tracks(self.drums.len() + self.parts.len() + self.loops.len() + self.patterns.len(), secs)
    }

    pub fn bars(&self) -> u32 {
        self.form.iter().fold(0, |n, s| n.saturating_add(s.bars))
    }

    pub fn secs(&self) -> f64 {
        self.bars() as f64 * 4.0 * 60.0 / self.bpm
    }

    /// every audio file the song plays (recorded one-shots, samples and loops)
    pub fn files(&self) -> Vec<&str> {
        let sounds = self
            .drums
            .iter()
            .map(|d| &d.sound)
            .chain(self.parts.iter().map(|p| &p.sound))
            .chain(self.loops.iter().map(|l| &l.sound))
            .chain(self.patterns.iter().flat_map(|p| p.sounds.values()))
            .chain(
                [Some(&self.transitions.crash), self.transitions.riser.as_ref(), self.transitions.impact.as_ref()]
                    .into_iter()
                    .flatten(),
            );
        sounds
            .filter_map(|s| match s {
                Sound::Sample { path, .. } | Sound::Loop { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn layer_names(&self) -> Vec<String> {
        self.drums
            .iter()
            .map(|d| d.name.clone())
            .chain(self.parts.iter().map(|p| p.name.clone()))
            .chain(self.loops.iter().map(|l| l.name.clone()))
            .chain(self.patterns.iter().map(|p| p.name.clone()))
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// an audio clip on a loop track: `t` / `d` in song beats, `offset` = where in the (warped) loop it starts
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub t: f64,
    pub d: f64,
    #[serde(default)]
    pub offset: f64,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    /// mixing role (kick, clap, hat, bass, chords, lead, top, fx ...): decides the target level
    pub role: String,
    pub sound: Sound,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transpose: i32,
    #[serde(default)]
    pub chop: Vec<i32>,
    #[serde(default)]
    pub slice_beats: f64,
    #[serde(default)]
    pub choke: Option<String>,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionInfo {
    pub name: String,
    pub kind: SectionKind,
    pub start_bar: u32,
    pub bars: u32,
    pub energy: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub title: String,
    pub genre: String,
    pub bpm: f64,
    pub key: Key,
    pub swing: f32,
    pub humanize: f32,
    pub bars: u32,
    pub seed: u64,
    pub sections: Vec<SectionInfo>,
    pub chords: Vec<ChordSpan>,
    pub tracks: Vec<Track>,
    pub mix: MixPlan,
}

impl Project {
    pub fn beats(&self) -> f64 {
        self.bars as f64 * 4.0
    }

    pub fn secs(&self) -> f64 {
        self.beats() * 60.0 / self.bpm
    }

    pub fn section_at(&self, beat: f64) -> Option<&SectionInfo> {
        self.sections.iter().find(|s| beat >= s.start_bar as f64 * 4.0 && beat < (s.start_bar + s.bars) as f64 * 4.0)
    }

    /// the bars [from, to) as a project of their own (for quick previews)
    pub fn slice(&self, from_bar: u32, to_bar: u32) -> Project {
        let (a, b) = (from_bar as f64 * 4.0, to_bar.min(self.bars) as f64 * 4.0);
        let shift_param = |p: &crate::dsp::env::Param| match p {
            crate::dsp::env::Param::Curve(pts) => {
                crate::dsp::env::Param::Curve(pts.iter().map(|(t, v)| (t - a, *v)).collect())
            }
            c => c.clone(),
        };
        let shift_fx = |fx: &Effect| match fx {
            Effect::Filter { mode, cutoff, res, drive, mix } => {
                Effect::Filter { mode: *mode, cutoff: shift_param(cutoff), res: *res, drive: *drive, mix: *mix }
            }
            Effect::Gain { db } => Effect::Gain { db: shift_param(db) },
            Effect::TapeStop { start, len } => Effect::TapeStop { start: start - a, len: *len },
            Effect::Stutter { start, len, slice } => Effect::Stutter { start: start - a, len: *len, slice: *slice },
            Effect::Reverse { start, len } => Effect::Reverse { start: start - a, len: *len },
            other => other.clone(),
        };
        let tracks = self
            .tracks
            .iter()
            .map(|t| Track {
                notes: t.notes.iter().filter(|n| n.t >= a && n.t < b).map(|n| Note { t: n.t - a, ..*n }).collect(),
                clips: t
                    .clips
                    .iter()
                    .filter(|c| c.t + c.d > a && c.t < b)
                    .map(|c| {
                        let start = c.t.max(a);
                        Clip {
                            t: start - a,
                            d: (c.t + c.d).min(b) - start,
                            offset: c.offset + (start - c.t),
                            ..c.clone()
                        }
                    })
                    .collect(),
                mix: TrackMix { fx: t.mix.fx.iter().map(shift_fx).collect(), ..t.mix.clone() },
                ..t.clone()
            })
            .collect();
        let mut mix = self.mix.clone();
        for chain in mix.buses.values_mut() {
            *chain = chain.iter().map(shift_fx).collect();
        }
        Project {
            bars: to_bar.min(self.bars) - from_bar,
            sections: self
                .sections
                .iter()
                .filter(|s| s.start_bar + s.bars > from_bar && s.start_bar < to_bar)
                .map(|s| SectionInfo { start_bar: s.start_bar.saturating_sub(from_bar), ..s.clone() })
                .collect(),
            chords: self
                .chords
                .iter()
                .filter(|c| c.start + c.len > a && c.start < b)
                .map(|c| ChordSpan { start: c.start - a, ..*c })
                .collect(),
            tracks,
            mix,
            ..self.clone()
        }
    }
}
