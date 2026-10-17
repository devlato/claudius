//! The studio's panels. Each draws from the current `State` and pushes `Command`s; the studio applies simple edits
//! optimistically and sends everything to the server once per frame.

pub mod arrange;
pub mod harmony;
pub mod inspector;
pub mod library;
pub mod log;
pub mod markdown;
pub mod mixer;
pub mod parts;
pub mod script;
pub mod sequencer;
pub mod transport;
pub mod verdict;

use claudius_api::{Command, SongView, SoundList, State};
use eframe::egui::{self, Color32, RichText, Ui};
use serde::{Deserialize, Serialize};

use crate::audio::Player;
use crate::theme::{self, TEXT_DIM, TEXT_FAINT};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Sequencer,
    Mixer,
    Parts,
    Harmony,
    Script,
    Sounds,
    Log,
}

impl Tab {
    pub const ALL: [Tab; 7] =
        [Tab::Sequencer, Tab::Mixer, Tab::Parts, Tab::Harmony, Tab::Script, Tab::Sounds, Tab::Log];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Sequencer => "Step sequencer",
            Tab::Mixer => "Mixer",
            Tab::Parts => "Parts",
            Tab::Harmony => "Harmony",
            Tab::Script => "Script",
            Tab::Sounds => "Sounds",
            Tab::Log => "Log",
        }
    }
}

/// app-level requests from the panels
pub enum Action {
    Retry,
    UseDemo,
    /// back to the server the studio was started with
    Connect,
}

/// what the studio knows about its connection
#[derive(Default)]
pub struct Link {
    pub label: String,
    pub mock: bool,
    /// the last state request succeeded
    pub online: bool,
    /// a state has arrived at least once
    pub ever: bool,
    pub error: Option<String>,
    pub retry_in: Option<f64>,
    /// "claudius 0.1.0, ears on" from `/api/hello`
    pub server: Option<String>,
}

pub struct NewSongForm {
    pub open: bool,
    pub genre: String,
    pub seed: String,
    pub loops: bool,
    pub bpm: Option<f64>,
    pub key: Option<String>,
}

#[derive(Default)]
pub struct ScriptEditor {
    pub text: String,
    /// the song's script the text started from
    pub base: String,
    pub dirty: bool,
    pub result: Option<(bool, String)>,
    pub cursor: Option<usize>,
    /// put the text cursor here next frame (after inserting a sound name)
    pub move_cursor: Option<usize>,
    pub awaiting: bool,
    pub filter: String,
}

impl ScriptEditor {
    pub fn sync(&mut self, script: &str) {
        if !self.dirty {
            self.text = script.to_string();
            self.base = script.to_string();
        }
    }
}

pub struct Toast {
    pub text: String,
    pub ok: bool,
    pub at: f64,
}

/// sequencer gesture in progress
pub enum Gesture {
    /// click-drag paints every touched cell on (or off)
    Paint { on: bool, touched: Vec<(usize, usize)> },
    /// shift-drag sets one cell's velocity
    Velocity { lane: usize, step: usize, start: f32, y0: f32 },
}

pub struct View {
    pub tab: Tab,
    pub section: usize,
    pub variant: String,
    pub gesture: Option<Gesture>,
    pub cell_menu: Option<(usize, usize)>,
    pub script: ScriptEditor,
    pub new_song: NewSongForm,
    pub stems: bool,
    pub mutate_count: usize,
    pub keep_best: bool,
    pub generations: usize,
    pub children: usize,
    pub looping: bool,
    /// loop the selected section rather than the whole render
    pub loop_section: bool,
    pub log_filter: String,
    pub add_kind: String,
    pub add_bars: u32,
    pub drag_section: Option<usize>,
    /// arrangement painting: (switch on?, row, sections touched)
    pub layer_paint: Option<(bool, usize, Vec<usize>)>,
    pub prog_text: Vec<(String, String)>,
    pub chip_edit: String,
    pub toast: Option<Toast>,
    /// the script reference (markdown from `/api/reference`), its window, and whether it is on its way
    pub reference: Option<String>,
    pub reference_open: bool,
    pub reference_loading: bool,
    /// the audio file for Analyze / Remake, and the last analysis (ok, text)
    pub audio_path: String,
    pub analysis: Option<(bool, String)>,
    pub analyzing: bool,
    pub library_filter: String,
    /// the path typed into "Open"
    pub open_path: String,
    /// the section name being typed (section index, text), so incoming states do not overwrite it
    pub name_edit: Option<(usize, String)>,
    pub volume: f32,
    pub sevenths: bool,
}

impl Default for View {
    fn default() -> Self {
        View {
            tab: Tab::Sequencer,
            section: 0,
            variant: "main".into(),
            gesture: None,
            cell_menu: None,
            script: ScriptEditor::default(),
            new_song: NewSongForm {
                open: false,
                genre: "house".into(),
                seed: String::new(),
                loops: false,
                bpm: None,
                key: None,
            },
            stems: false,
            mutate_count: 4,
            keep_best: true,
            generations: 3,
            children: 6,
            looping: false,
            loop_section: false,
            log_filter: String::new(),
            add_kind: "drop".into(),
            add_bars: 8,
            drag_section: None,
            layer_paint: None,
            prog_text: Vec::new(),
            chip_edit: String::new(),
            toast: None,
            reference: None,
            reference_open: false,
            reference_loading: false,
            audio_path: String::new(),
            analysis: None,
            analyzing: false,
            library_filter: String::new(),
            open_path: String::new(),
            name_edit: None,
            volume: 0.9,
            sevenths: true,
        }
    }
}

/// what every panel gets for a frame
pub struct Cx<'a> {
    pub state: &'a mut State,
    pub view: &'a mut View,
    pub player: &'a mut Player,
    pub out: &'a mut Vec<Command>,
    pub sounds: Option<&'a SoundList>,
    /// the server's base URL (None in the demo)
    pub server: Option<&'a str>,
    pub now: f64,
}

impl Cx<'_> {
    /// tempo of what is playing: the render's, else the song's
    pub fn audio_bpm(&self) -> f64 {
        self.state
            .render
            .as_ref()
            .map(|r| r.bpm)
            .filter(|b| *b > 0.0)
            .or(self.state.song.as_ref().map(|s| s.bpm))
            .unwrap_or(120.0)
    }

    /// the playhead in bars (fractional)
    pub fn playhead_bar(&self) -> f64 {
        self.player.position() * self.audio_bpm() / 240.0
    }

    /// is the render older than the song (the server's `song_version` moved past it)
    pub fn render_stale(&self) -> bool {
        render_stale(self.state)
    }

    /// is any track soloed (then only soloed tracks play)
    pub fn any_solo(&self) -> bool {
        self.state.song.as_ref().is_some_and(any_solo)
    }
}

pub fn render_stale(state: &State) -> bool {
    state.song.is_some() && state.render.as_ref().is_some_and(|r| r.version < state.song_version)
}

pub fn any_solo(song: &SongView) -> bool {
    song.tracks.iter().any(|t| t.solo)
}

/// a sampled instrument rather than a synth preset (synths are the ones the server lists as such)
pub fn is_sampled(name: &str, sounds: Option<&SoundList>) -> bool {
    match sounds {
        Some(s) if !s.synths.is_empty() => !s.synths.iter().any(|x| x == name),
        _ => name.contains(':'),
    }
}

/// a centred hint for empty panels
pub fn empty(ui: &mut Ui, text: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(text).color(TEXT_FAINT).size(13.0));
    });
}

/// the colour of a layer: its track's role when known, else its name
pub fn layer_color(song: &SongView, layer: &str) -> Color32 {
    let role = song
        .tracks
        .iter()
        .find(|t| t.name == layer)
        .map(|t| t.role.as_str())
        .or_else(|| song.parts.iter().find(|p| p.name == layer).map(|p| p.role.as_str()))
        .unwrap_or(layer);
    theme::role_color(role)
}

pub fn small(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.0).color(TEXT_DIM)
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u64, s % 60.0)
}

/// a frame for one tab page
pub fn page_frame() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8))
}
