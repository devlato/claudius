//! The wire contract between the claudius server and its clients (the egui UI, native and web, and the MCP tools).
//! Plain serde types only, so this crate builds for wasm32 too.
//!
//! HTTP routes served by the app (default `http://127.0.0.1:7878`):
//!
//! | route | |
//! |---|---|
//! | `GET  /api/state?since=N` | `State`. With `since`, long-polls: answers as soon as `version > N` (or after ~20 s with the current state) |
//! | `POST /api/command` | body: a `Command` (JSON, tagged by `"cmd"`), answer: `Reply`. Long jobs (render, mutate, evolve) answer at once and report through `State.busy` / `State.log` |
//! | `GET  /api/script` | the current song as a script (text/plain) |
//! | `GET  /api/sounds` | `SoundList`: every sound name a script or the UI may use |
//! | `GET  /api/audio/{version}.wav` / `.mp3` | the render of that state version (`RenderView.url` points here) |
//! | `GET  /api/stems/{version}/{track}.wav` | one track of that render, when it was rendered with stems |
//! | `POST /mcp` | the MCP endpoint (Streamable HTTP) |
//! | `GET  /` | the web UI |

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Drum step velocities by name, as step strings write them (`X` accent, `x` hit, `o` ghost) and the studio shows
/// them: an accent from `ACCENT`, a ghost below `GHOST`. The engine's step strings use the same values.
pub const ACCENT: f32 = 0.85;
pub const GHOST: f32 = 0.5;

/// the step-string character for a velocity (`.` for none)
pub fn step_char(v: f32) -> char {
    if v <= 0.0 {
        '.'
    } else if v >= ACCENT {
        'X'
    } else if v >= GHOST {
        'x'
    } else {
        'o'
    }
}

/// the velocity a step-string character plays: `X` 1.0, `x` 0.8, `o` 0.45, `1`-`9` tenths, rests 0; None for
/// anything else
pub fn step_velocity(c: char) -> Option<f32> {
    match c {
        'X' => Some(1.0),
        'x' => Some(0.8),
        'o' => Some(0.45),
        '1'..='9' => c.to_digit(10).map(|d| d as f32 / 10.0),
        '.' | '-' | '~' | '_' => Some(0.0),
        _ => None,
    }
}

pub const DEFAULT_PORT: u16 = 7878;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// increases on every change of anything below
    pub version: u64,
    /// the version of the last change to the song itself: the render is current when `render.version >= song_version`
    #[serde(default)]
    pub song_version: u64,
    pub song: Option<SongView>,
    /// the latest finished render of the current song (absent until the first render)
    pub render: Option<RenderView>,
    pub verdict: Option<VerdictView>,
    /// what the engine is busy with ("rendering", "evolving: generation 2 of 5"), None when idle
    pub busy: Option<String>,
    /// 0..1 progress of the busy job when known
    pub progress: Option<f32>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// what undo / redo would take back or bring back ("tempo 124", "mutated: lead: rewrote the part")
    #[serde(default)]
    pub undo_label: Option<String>,
    #[serde(default)]
    pub redo_label: Option<String>,
    /// recent messages, oldest first (at most ~200)
    pub log: Vec<String>,
    pub genres: Vec<GenreInfo>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GenreInfo {
    pub name: String,
    pub description: String,
    pub bpm: (f64, f64),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SongView {
    pub title: String,
    pub genre: String,
    pub seed: u64,
    pub bpm: f64,
    /// e.g. "A minor"
    pub key: String,
    pub swing: f32,
    pub bars: u32,
    pub seconds: f64,
    /// progression name ("A", "B") -> Roman numerals ("i7", "VImaj7", ...)
    pub progressions: Vec<(String, Vec<String>)>,
    pub sections: Vec<SectionView>,
    /// every layer a section can switch on or off (drum lanes, parts, loops, pattern tracks)
    pub layers: Vec<String>,
    pub tracks: Vec<TrackView>,
    pub drums: Vec<DrumView>,
    pub parts: Vec<PartView>,
    /// the song as a claudius script (runs back into this exact song)
    pub script: String,
    /// what was done to the song so far, oldest first
    pub lineage: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SectionView {
    pub name: String,
    /// intro, verse, build, drop, break, hook, groove, outro
    pub kind: String,
    pub start_bar: u32,
    pub bars: u32,
    pub energy: f32,
    pub layers: Vec<String>,
    /// the progression the section plays ("A", "B", ...)
    #[serde(default)]
    pub prog: String,
    /// drum pattern variant: main, alt, sparse, ...
    pub variant: String,
    pub filter_in: bool,
    pub build: bool,
    pub gap: bool,
    pub impact: bool,
    pub fills: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TrackView {
    pub name: String,
    /// "drum" | "part" | "loop" | "pattern" | "fx"
    pub kind: String,
    /// mixing role (kick, bass, chords, lead, top, ...)
    pub role: String,
    /// what makes the sound, in words ("synth patch 'pluck'", "kick cut from 'Big Room Kick' (Apple Loops)")
    pub sound: String,
    pub level_db: f32,
    pub pan: f32,
    pub reverb: f32,
    pub delay: f32,
    pub duck: f32,
    pub muted: bool,
    /// soloed: when any track is, only soloed tracks play
    #[serde(default)]
    pub solo: bool,
    /// loudness of the track in the latest render (LUFS while playing), if rendered
    pub loudness: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DrumView {
    pub lane: String,
    /// bars per pattern (the steps below cover pattern_bars * 16 sixteenth notes)
    pub pattern_bars: u32,
    /// variant name -> velocity per 1/16 step (0 = no hit)
    pub variants: Vec<(String, Vec<f32>)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PartView {
    pub name: String,
    pub role: String,
    pub style: String,
    /// styles this part may switch to (from its genre)
    pub styles: Vec<String>,
    pub preset: String,
    pub presets: Vec<String>,
    pub register: (i32, i32),
    pub density: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RenderView {
    /// state version this render belongs to
    pub version: u64,
    /// relative URL of the mastered audio (mp3 for the browser; the same path with .wav gives the 24-bit master)
    pub url: String,
    pub seconds: f64,
    pub lufs: f64,
    pub true_peak_db: f32,
    pub bpm: f64,
    /// (section name, start s, end s)
    pub sections: Vec<(String, f64, f64)>,
    /// peak envelope for drawing, ~1000 points 0..1
    pub waveform: Vec<f32>,
    /// track names that have stems at /api/stems/{version}/{track}.wav
    pub stems: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct VerdictView {
    /// 0..100
    pub total: f64,
    pub summary: String,
    /// component -> 0..1 (mix, arc, theory, genre, production, refs)
    pub scores: Vec<(String, f64)>,
    /// (severity 0..1, what, where to fix it)
    pub issues: Vec<(f64, String, String)>,
    pub ears: Option<EarsView>,
    /// meters: lufs, true_peak_db, band_*_db, stereo_correlation, drop_minus_break_lu, ...
    pub measures: Vec<(String, f64)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EarsView {
    pub genre_query: String,
    pub genre_z: f64,
    pub best_alt: String,
    pub best_alt_z: f64,
    pub production_z: f64,
    pub worst_flaw: String,
    pub worst_flaw_z: f64,
    pub sounds_like: Vec<String>,
    pub p_real: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SoundList {
    /// procedural drum voices (kick, snare, clap, hh, oh, ride, crash, tom, rim, shaker, cowbell, snap, perc ...)
    pub drums: Vec<String>,
    /// synth presets (808, pluck, supersaw, pad, epiano, ...)
    pub synths: Vec<String>,
    /// multi-sampled instruments from installed packs ("piano", "strings", "tr909:bd" ...)
    pub instruments: Vec<String>,
    /// installed packs and whether they are installed
    pub packs: Vec<PackInfo>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PackInfo {
    pub name: String,
    pub description: String,
    pub license: String,
    pub installed: bool,
    pub size_mb: f64,
}

/// an edit or action; JSON `{"cmd": "set_tempo", "bpm": 124}`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// generate a new song from a genre (seed random when absent)
    NewSong {
        genre: String,
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        bpm: Option<f64>,
        #[serde(default)]
        key: Option<String>,
        /// build it around an Apple Loops family
        #[serde(default)]
        loops: bool,
    },
    /// open a blueprint.json or a script file
    Load {
        path: String,
    },
    /// write blueprint.json + song.wav/mp3 + script into a folder (default: a fresh folder under out/)
    Save {
        #[serde(default)]
        path: Option<String>,
    },
    SetTempo {
        bpm: f64,
    },
    SetKey {
        key: String,
    },
    SetSwing {
        swing: f32,
    },
    SetProgression {
        name: String,
        chords: Vec<String>,
    },
    /// change any subset of a section's properties
    SetSection {
        index: usize,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        bars: Option<u32>,
        #[serde(default)]
        energy: Option<f32>,
        #[serde(default)]
        layers: Option<Vec<String>>,
        /// which progression it plays
        #[serde(default)]
        prog: Option<String>,
        #[serde(default)]
        variant: Option<String>,
        #[serde(default)]
        filter_in: Option<bool>,
        #[serde(default)]
        build: Option<bool>,
        #[serde(default)]
        gap: Option<bool>,
        #[serde(default)]
        impact: Option<bool>,
        #[serde(default)]
        fills: Option<bool>,
    },
    AddSection {
        after: usize,
        kind: String,
        bars: u32,
    },
    RemoveSection {
        index: usize,
    },
    MoveSection {
        from: usize,
        to: usize,
    },
    ToggleLayer {
        section: usize,
        layer: String,
    },
    /// one drum step; velocity 0 removes the hit
    SetDrumStep {
        lane: String,
        variant: String,
        step: usize,
        velocity: f32,
    },
    /// a whole lane variant in step notation: "x...x...x...x..." (x hit, X accent, o ghost, . rest; 16 per bar)
    SetDrumPattern {
        lane: String,
        variant: String,
        steps: String,
    },
    SetPart {
        name: String,
        #[serde(default)]
        style: Option<String>,
        #[serde(default)]
        preset: Option<String>,
        #[serde(default)]
        register: Option<(i32, i32)>,
        #[serde(default)]
        density: Option<f32>,
        /// write the part again with a new seed
        #[serde(default)]
        reseed: bool,
    },
    SetMix {
        track: String,
        #[serde(default)]
        level_db: Option<f32>,
        #[serde(default)]
        pan: Option<f32>,
        #[serde(default)]
        reverb: Option<f32>,
        #[serde(default)]
        delay: Option<f32>,
        #[serde(default)]
        duck: Option<f32>,
        #[serde(default)]
        muted: Option<bool>,
        #[serde(default)]
        solo: Option<bool>,
    },
    /// run a claudius script against the current song (see the script reference)
    RunScript {
        code: String,
    },
    /// read an audio file: tempo, key, chords, sections, drum pattern (the answer's message)
    Analyze {
        path: String,
    },
    /// a new song built from the analysis of an audio file (its tempo, key, progression, form and groove)
    Remake {
        path: String,
        #[serde(default)]
        genre: Option<String>,
        #[serde(default)]
        seed: Option<u64>,
    },
    /// try `count` random mutations, keep the best-judged one when `keep_best` (else only report them)
    Mutate {
        #[serde(default = "four")]
        count: usize,
        #[serde(default = "yes")]
        keep_best: bool,
    },
    Evolve {
        #[serde(default = "three")]
        generations: usize,
        #[serde(default = "six")]
        children: usize,
    },
    /// render the song (stems too when asked) and judge it
    Render {
        #[serde(default)]
        stems: bool,
    },
    Undo,
    Redo,
    /// stop a running render / mutate / evolve job
    Cancel,
}

fn four() -> usize {
    4
}
fn three() -> usize {
    3
}
fn six() -> usize {
    6
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Reply {
    pub ok: bool,
    pub message: String,
    /// the state version after the command (or when a long job was started)
    pub version: u64,
}
