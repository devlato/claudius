//! The studio's panels. Each draws from the current `State` and pushes `Command`s; the studio applies simple edits
//! optimistically and sends everything to the server once per frame.

pub mod arrange;
pub mod assistants;
pub mod graph;
pub mod harmony;
pub mod import;
pub mod inspector;
pub mod library;
pub mod log;
pub mod markdown;
pub mod mixer;
pub mod parts;
pub mod phone;
pub mod publish;
pub mod record;
pub mod roll;
pub mod script;
pub mod sequencer;
pub mod transport;
pub mod verdict;

use claudius_api::{Command, GenreInfo, SongView, SoundChoice, SoundList, State, TrackView};
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
    Notes,
    Harmony,
    Script,
    Sounds,
    Graph,
    Log,
}

impl Tab {
    /// in the dock's order: the tabs for making the song, then the ones at its right end (`at_the_end`)
    pub const ALL: [Tab; 9] = [
        Tab::Sequencer,
        Tab::Parts,
        Tab::Notes,
        Tab::Harmony,
        Tab::Mixer,
        Tab::Sounds,
        Tab::Script,
        Tab::Graph,
        Tab::Log,
    ];

    /// one of the tabs for looking under the song (its script, its signal flow, the log), at the dock's right end
    pub fn at_the_end(self) -> bool {
        matches!(self, Tab::Script | Tab::Graph | Tab::Log)
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Sequencer => "Drums",
            Tab::Mixer => "Mixer",
            Tab::Parts => "Instruments",
            Tab::Notes => "Notes",
            Tab::Harmony => "Chords",
            Tab::Script => "Script",
            Tab::Sounds => "Sounds",
            Tab::Graph => "Signal flow",
            Tab::Log => "Log",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            Tab::Sequencer => "each section's drum pattern, step by step",
            Tab::Mixer => "every track's level, pan, effects and sends",
            Tab::Parts => "each instrument's style, sound, range and how busy it plays",
            Tab::Notes => "an instrument's notes over a chord progression, as a piano roll",
            Tab::Harmony => "the song's chord progressions, and which section plays which",
            Tab::Script => "the whole song as a claudius script, to read, change and run",
            Tab::Sounds => "the sounds and drum kits claudius can use, and packs of more",
            Tab::Graph => "how every track runs through its effects to the master",
            Tab::Log => "what claudius did and said, newest last",
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

/// a recording to send to claudius: to listen to, or to make a new song of in a genre (with a seed)
pub struct Listen {
    pub file: std::sync::Arc<crate::pick::Picked>,
    pub remake: Option<(String, Option<u64>)>,
    /// the remake a flip of it, a new song on its hook and pieces of it, rather than a rework of all of it
    pub flip: bool,
    /// its vocals are to be sampled into the open song instead (the auto-sampler)
    pub sample: bool,
    /// its groove is to be taken for the open song's drum lanes instead
    pub groove: bool,
    /// the library's record of it, when a free library offered it (`LibraryHit::found`): what the demo's engine
    /// credits it by
    pub found: Option<serde_json::Value>,
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
    /// "claudius 0.1.0" from `/api/hello`
    pub server: Option<String>,
    /// whether that server has ears, from `/api/hello` (the state says when they are still loading)
    pub ears: Option<bool>,
}

pub struct NewSongForm {
    pub open: bool,
    pub genre: String,
    /// the genre fused with it, which plays the drums and lends its sounds (`genres::fuse`)
    pub with: Option<String>,
    pub seed: String,
    pub loops: bool,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    /// build it on samples from free libraries, as asked here; none: as the genre does
    pub sampled: Option<bool>,
    /// what its phrase is searched for; empty: what the genre's producers dig for
    pub sample_search: String,
    /// how layered it is, from 0 to 1; none: as the genre is
    pub complexity: Option<f64>,
    /// what the genres shown are searched by (`claudius_api::search_genres`); empty: all, by family
    pub genre_search: String,
    /// the dialog starts the song from a recording rather than from a genre alone
    pub recording: bool,
    /// and what it does with the recording
    pub with_recording: WithRecording,
}

/// what the New song dialog does with a recording
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WithRecording {
    #[default]
    Remake,
    Flip,
    Sample,
    Groove,
    Analyze,
}

impl NewSongForm {
    /// the genre as claudius takes it: `trip_hop`, or `trip_hop+idm` for a fusion
    pub fn genre_name(&self) -> String {
        match self.with.as_ref().filter(|w| **w != self.genre) {
            Some(w) => format!("{}+{w}", self.genre),
            None => self.genre.clone(),
        }
    }

    /// set from a song's genre, a fusion's first two genres included; it samples as that genre does until asked
    pub fn set_genre(&mut self, name: &str) {
        let mut parts = name.split('+');
        let genre = parts.next().unwrap_or(name).to_string();
        if genre != self.genre {
            self.sampled = None;
        }
        self.genre = genre;
        self.with = parts.next().map(String::from);
    }

    /// whether the song is built on samples, as the dialog shows it: as asked, else as the genre is by default
    pub fn sampling_on(&self, genres: &[GenreInfo]) -> bool {
        let sampling = genres.iter().find(|g| g.name == self.genre).and_then(|g| g.sampling.as_ref());
        sampling.is_some() && self.sampled.unwrap_or(sampling.is_some_and(|s| s.by_default))
    }

    /// the new song asked for: the words for its phrase go only with samples, as the dialog shows them
    pub fn command(&self, genres: &[GenreInfo]) -> Command {
        let words = self.sample_search.trim();
        Command::NewSong {
            genre: self.genre_name(),
            seed: self.seed.trim().parse().ok(),
            bpm: self.bpm,
            key: self.key.clone(),
            loops: self.loops,
            sampled: self.sampled,
            sample_search: Some(words.to_string()).filter(|w| !w.is_empty() && self.sampling_on(genres)),
            complexity: self.complexity,
        }
    }
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

/// a new track, as the arrangement's "+ Track" menu is filling it in
#[derive(Default)]
pub struct AddTrackForm {
    /// written the way a genre plays it (`Command::GenerateTrack`); otherwise a part or a drum lane of your own
    pub generate: bool,
    /// what a generated track is (drums, bass, strings ...), and the genre it is written in (empty: the song's)
    pub what: String,
    pub genre: String,
    /// a generated part in place of the song's part of its role
    pub replace: bool,
    /// a drum lane; otherwise a part
    pub drum: bool,
    /// a part's role: bass, chords, pad, keys, lead or arp
    pub role: String,
    /// left empty, a name made from the role or the sound
    pub name: String,
    /// left empty, a part plays what its genre draws
    pub sound: String,
    pub filter: String,
}

pub struct View {
    pub tab: Tab,
    pub section: usize,
    pub variant: String,
    pub gesture: Option<Gesture>,
    pub cell_menu: Option<(usize, usize)>,
    /// the trig the step sequencer's menu is editing: its lane, variant and step, and the trig as it stands
    pub trig_edit: Option<(String, String, usize, claudius_api::TrigView)>,
    pub script: ScriptEditor,
    pub new_song: NewSongForm,
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
    /// a download asked for, handed over once a render of the song as it was then is here
    pub download: Option<Download>,
    /// the script reference (markdown from `/api/reference`), its window, and whether it is on its way
    pub reference: Option<String>,
    pub reference_open: bool,
    pub reference_loading: bool,
    /// the recording chosen for Analyze / Remake, what is to be done with it (sent by the app), and the last analysis
    /// (ok, text)
    pub audio_file: Option<std::sync::Arc<crate::pick::Picked>>,
    pub listen: Option<Listen>,
    pub analysis: Option<(bool, String)>,
    /// how a take is sung, as the server heard it, until the window showing it is closed
    pub heard: Option<(bool, String)>,
    pub analyzing: bool,
    pub library_filter: String,
    /// the section name being typed (section index, text), so incoming states do not overwrite it
    pub name_edit: Option<(usize, String)>,
    pub volume: f32,
    pub sevenths: bool,
    pub assistants: assistants::Assistants,
    pub publish: publish::Publish,
    pub record: record::Record,
    /// the Add a recording window, and a take being trimmed
    pub import: import::Import,
    pub phone: phone::Phone,
    /// the graph tab: the node chosen, and where its canvas looks
    pub graph: graph::GraphView,
    /// what a take's melody becomes when it turns into notes (`claudius_api::NOTE_STYLES`)
    pub melody_style: String,
    /// the arrangement row whose menu is open (the row right-clicked: the pointer's press is gone once it opens)
    pub row_menu: Option<usize>,
    /// the arrangement's "+ Track" form
    pub add_track: AddTrackForm,
    /// the piano roll's part, progression, grid and drag
    pub roll: roll::RollView,
    /// the tour's step on screen, and whether it was taken or skipped (kept between runs)
    pub tour: Option<usize>,
    pub tour_done: bool,
    /// the tour ends with the question about usage statistics (the app, when it has not been answered)
    pub tour_asks: bool,
    pub anchors: crate::tour::Anchors,
    /// the update whose banner was put off with Later (until the next start)
    pub update_dismissed: Option<String>,
    /// "Check for updates" was asked for: say what it found
    pub update_asked: bool,
}

impl Default for View {
    fn default() -> Self {
        View {
            tab: Tab::Sequencer,
            section: 0,
            variant: "main".into(),
            gesture: None,
            cell_menu: None,
            trig_edit: None,
            script: ScriptEditor::default(),
            new_song: NewSongForm {
                open: false,
                genre: "house".into(),
                with: None,
                seed: String::new(),
                loops: false,
                bpm: None,
                key: None,
                sampled: None,
                sample_search: String::new(),
                complexity: None,
                genre_search: String::new(),
                recording: false,
                with_recording: WithRecording::default(),
            },
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
            download: None,
            reference: None,
            reference_open: false,
            reference_loading: false,
            audio_file: None,
            listen: None,
            analysis: None,
            heard: None,
            analyzing: false,
            library_filter: String::new(),
            name_edit: None,
            volume: 0.9,
            sevenths: true,
            assistants: assistants::Assistants::default(),
            publish: publish::Publish::default(),
            record: record::Record::default(),
            import: import::Import::default(),
            phone: phone::Phone::default(),
            graph: graph::GraphView::default(),
            melody_style: "line".into(),
            row_menu: None,
            add_track: AddTrackForm { generate: true, what: "bass".into(), role: "lead".into(), ..Default::default() },
            roll: roll::RollView::default(),
            tour: None,
            tour_done: false,
            tour_asks: false,
            anchors: crate::tour::Anchors::default(),
            update_dismissed: None,
            update_asked: false,
        }
    }
}

impl View {
    /// The answer to a recording sent to be listened to (`what`: "analyze", "remake", "sample_song" or
    /// "extract_groove"), shown under the buttons it was sent with. One that changed the song (or made one) closes the
    /// window too, so that the song shows with what changed in it: the toast every answer gets says what in its first
    /// line, and the render the change starts is on the busy line.
    pub fn listened(&mut self, what: &str, ok: bool, message: String) {
        self.analyzing = false;
        if ok && what != "analyze" {
            self.new_song.open = false;
        }
        self.analysis = Some((ok, message));
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

/// is the render older than the song, or only its first bars (the demo's preview while the rest renders)
pub fn render_stale(state: &State) -> bool {
    state.song.is_some()
        && state.render.as_ref().is_some_and(|r| r.version < state.song_version || r.url == claudius_api::PREVIEW_URL)
}

/// A download asked for: the song's version when it was, and when, and which song it was (its title, genre and seed).
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub version: u64,
    pub asked: f64,
    pub song: Option<(String, String, u64)>,
    /// the format asked for (an `ExportFormat`'s spec), or None for the backend's own
    pub format: Option<String>,
}

/// what a download waits for
pub enum Due {
    /// a render of the song as it was asked for
    Wait,
    /// this one
    Render(claudius_api::RenderView),
    /// none will come: why
    Off(&'static str),
}

impl Download {
    pub fn of(state: &State, now: f64, format: Option<String>) -> Download {
        Download { version: state.song_version, asked: now, song: state.song.as_ref().map(song_id), format }
    }

    /// Whether it is due: a render of the song asked for, once that song's version then is rendered; off when another
    /// song took its place, or no render came for `wait` seconds.
    pub fn due(&self, state: &State, now: f64, wait: f64) -> Due {
        if state.song.as_ref().map(song_id) != self.song {
            return Due::Off("another song took the place of the one asked for: download again");
        }
        match state.render.as_ref().filter(|r| r.version >= self.version && r.url != claudius_api::PREVIEW_URL) {
            Some(r) => Due::Render(r.clone()),
            None if now - self.asked > wait => {
                Due::Off("no render came for it: render the song, then download it again")
            }
            None => Due::Wait,
        }
    }
}

fn song_id(s: &SongView) -> (String, String, u64) {
    (s.title.clone(), s.genre.clone(), s.seed)
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

/// when default packs are missing: a line that says so, and a button that downloads them
pub fn missing_packs(ui: &mut Ui, cx: &mut Cx) {
    let Some(list) = cx.sounds else { return };
    let missing: Vec<&claudius_api::PackInfo> = list.packs.iter().filter(|p| p.default && !p.installed).collect();
    if missing.is_empty() {
        return;
    }
    let downloading = cx.state.busy.as_deref().is_some_and(|b| b.starts_with("downloading"));
    let gb = missing.iter().map(|p| p.size_mb).sum::<f64>() / 1000.0;
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(format!(
                "{} of the default sample packs are not installed, so their instruments are synthesized.",
                missing.len()
            ))
            .color(theme::TEXT_DIM),
        );
        let button = egui::Button::new(format!("Download the packs ({gb:.1} GB)"));
        if ui.add_enabled(!downloading, button).on_disabled_hover_text("downloading…").clicked() {
            cx.out.push(Command::InstallPacks { names: Vec::new() });
        }
    });
    ui.add_space(8.0);
}

/// A sound a track may swap to, as a person reads it: a kit by its name, a synthesized drum by its tuning, an
/// instrument by its name with its pack's, a synth by its preset.
pub fn choice_label(c: &SoundChoice) -> String {
    match c.group.as_str() {
        "the genre's kits" | "other kits" => c.kind.clone(),
        "synthesized" => match c.name.rsplit_once(':') {
            Some((_, "0")) => "synthesized, as it is".into(),
            Some((_, n)) => format!("synthesized, tuned {n}"),
            None => c.name.clone(),
        },
        _ => match c.name.split_once(':') {
            Some((pack, instrument)) => format!("{} ({pack})", instrument.replace('_', " ")),
            None => c.name.replace('_', " "),
        },
    }
}

/// the sound a swap of `t` with none named gives it, as `choice_label` reads it
pub fn next_label(t: &TrackView) -> Option<String> {
    let next = t.next_swap.as_ref()?;
    Some(t.swaps.iter().find(|c| c.name == *next).map_or_else(|| next.clone(), choice_label))
}

/// The swap for track `t` onto `to`, or onto its next sound.
pub fn swap(t: &str, to: Option<&str>) -> Command {
    Command::SwapSound { track: Some(t.to_string()), to: to.map(str::to_string), only: None }
}

/// The sounds `t` may swap to, under how near its genre each is (those further afield in a menu of their own), the
/// one it plays ticked; the swap clicked.
pub fn swap_list(ui: &mut Ui, t: &TrackView) -> Option<Command> {
    let mut picked = None;
    let mut item = |ui: &mut Ui, c: &SoundChoice| {
        let playing = t.playing.as_ref() == Some(&c.name);
        let r = ui.selectable_label(playing, choice_label(c)).on_hover_text(format!("{} · {}", c.name, c.kind));
        if r.clicked() && !playing {
            picked = Some(swap(&t.name, Some(&c.name)));
            ui.close();
        }
    };
    let mut groups: Vec<&str> = Vec::new();
    for c in &t.swaps {
        if !groups.contains(&c.group.as_str()) {
            groups.push(&c.group);
        }
    }
    for g in groups {
        let of = t.swaps.iter().filter(|c| c.group == g);
        if g == "further afield" {
            ui.menu_button(g, |ui| {
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| of.for_each(|c| item(ui, c)));
            });
        } else {
            ui.label(small(g));
            of.for_each(|c| item(ui, c));
        }
    }
    picked
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

/// the synth presets offered to play a melody or a part played in
const MELODIC: &[&str] = &[
    "pluck",
    "bell",
    "epiano",
    "keys",
    "square_lead",
    "supersaw",
    "strings",
    "organ",
    "chip",
    "stab",
    "pad",
    "saw_lead",
    "pwm_lead",
    "whistle",
    "soft_lead",
    "glide_lead",
    "brass_lead",
    "fm_lead",
    "vox_lead",
    "tine_piano",
    "clav",
    "house_piano",
    "rock_organ",
    "brass_stab",
    "solina",
    "glass_pad",
    "pwm_pad",
    "vowel_pad",
    "fm_marimba",
    "fm_kalimba",
    "harp_pluck",
    "mallet",
    "cowbell_lead",
    "acid",
];

/// what may play a melody: the installed instruments (not a drum machine's hits), and the synths made for a line
pub fn melodic_sounds(sounds: Option<&SoundList>) -> (Vec<String>, Vec<String>) {
    let Some(s) = sounds else { return (Vec::new(), Vec::new()) };
    let instruments = s.instruments.iter().filter(|i| !i.contains(':')).cloned().collect();
    let synths = s.synths.iter().filter(|p| MELODIC.contains(&p.as_str())).cloned().collect();
    (instruments, synths)
}

pub fn small(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.5).color(TEXT_DIM)
}

/// a percent typed or shown ("46", "46%", "1%") as the 0..1 it stands for, for a 0..1 a slider shows as a percent
pub fn percent(text: &str) -> Option<f64> {
    text.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u64, s % 60.0)
}

/// a frame for one tab page
pub fn page_frame() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use claudius_api::{GenreSampling, RenderView};

    #[test]
    fn a_percent_reads_as_the_percent_it_shows() {
        assert_eq!(percent("46"), Some(0.46));
        assert_eq!(percent("46%"), Some(0.46));
        assert_eq!(percent(" 1 % "), Some(0.01));
        assert_eq!(percent("100%"), Some(1.0));
        assert_eq!(percent("loud"), None);
    }

    #[test]
    fn the_tabs_at_the_end_of_the_dock_come_last() {
        let first = Tab::ALL.iter().position(|t| t.at_the_end()).unwrap_or(Tab::ALL.len());
        assert!(Tab::ALL[first..].iter().all(|t| t.at_the_end()), "{:?}", Tab::ALL);
        assert_eq!(Tab::ALL.iter().filter(|t| t.at_the_end()).count(), 3);
    }

    #[test]
    fn an_answer_that_changed_the_song_closes_the_window_it_came_from_and_an_analysis_or_a_refusal_keeps_it_open() {
        let mut v = View::default();
        let answers = [
            ("analyze", true, true),
            ("sample_song", false, true),
            ("sample_song", true, false),
            ("extract_groove", true, false),
            ("remake", true, false),
        ];
        for (what, ok, open) in answers {
            (v.new_song.open, v.analyzing) = (true, true);
            v.listened(what, ok, format!("{what}: what came of it\nwhere each piece went"));
            assert_eq!((v.new_song.open, v.analyzing), (open, false), "{what} {ok}");
            assert_eq!(v.analysis, Some((ok, format!("{what}: what came of it\nwhere each piece went"))));
        }
    }

    #[test]
    fn every_synth_made_for_a_line_is_offered_to_play_a_melody() {
        use claudius::synth::presets;
        for p in presets::for_role("lead").into_iter().chain(presets::for_role("arp")) {
            assert!(MELODIC.contains(&p), "{p} is made for a line, but the melody pickers leave it out");
        }
    }

    #[test]
    fn the_new_song_asked_for_samples_as_the_dialog_shows_it() {
        let genre = |name: &str, by_default: bool| GenreInfo {
            name: name.into(),
            sampling: Some(GenreSampling { by_default, roles: Vec::new() }),
            ..Default::default()
        };
        let genres =
            [genre("house", false), genre("trip_hop", true), GenreInfo { name: "mine".into(), ..Default::default() }];
        let mut f = View::default().new_song;
        let asked = |f: &NewSongForm| match f.command(&genres) {
            Command::NewSong { sampled, sample_search, .. } => (sampled, sample_search),
            _ => unreachable!("a new song"),
        };
        // house as it is: no samples, and words typed under another genre do not ask for them
        f.sample_search = " 70s soul ".into();
        assert_eq!(asked(&f), (None, None));
        f.sampled = Some(true);
        assert_eq!(asked(&f), (Some(true), Some("70s soul".into())));
        // another genre samples as it does: trip-hop is, with the words
        f.set_genre("trip_hop");
        assert_eq!(asked(&f), (None, Some("70s soul".into())));
        f.sampled = Some(false);
        assert_eq!(asked(&f), (Some(false), None));
        // a genre that says nothing of what it samples sends no words
        f.set_genre("mine");
        assert!(!f.sampling_on(&genres) && asked(&f).1.is_none());
    }

    #[test]
    fn a_download_waits_for_its_songs_render_and_is_off_when_another_song_comes() {
        let song = |title: &str| SongView { title: title.into(), genre: "trap".into(), seed: 1, ..Default::default() };
        let render = |version| Some(RenderView { version, ..Default::default() });
        let mut state =
            State { song: Some(song("Liquid Signal")), song_version: 4, render: render(3), ..Default::default() };
        let d = Download::of(&state, 10.0, None);
        assert!(matches!(d.due(&state, 11.0, 300.0), Due::Wait), "render 3 is from before the click");
        state.render = render(4);
        assert!(matches!(d.due(&state, 12.0, 300.0), Due::Render(r) if r.version == 4));
        state.render = render(3);
        assert!(matches!(d.due(&state, 400.0, 300.0), Due::Off(why) if why.contains("no render came")));
        state.song = Some(song("Static Haze"));
        state.render = render(9);
        assert!(
            matches!(d.due(&state, 12.0, 300.0), Due::Off(why) if why.contains("another song")),
            "a newer render, of another song"
        );
    }
}
