//! The command palette (Cmd+K, Ctrl+K elsewhere): the studio's actions (`claudius_intent::catalogue`), found from a few
//! words, each with where it lives in the studio and the key that does it too, as editors' palettes are. The words find
//! actions by name at once, here; the engine finds them by meaning too, as the person would say it ("make it louder",
//! "sing over it"), once its embedding model is there, and its answer replaces what the name found. A request of the
//! song in plain words ("more reverb on the snare", "undo the last 3 changes") is read too (`claudius_intent::ask`),
//! and what claudius would do for it comes first, a button for each reading, which says what it changes from what to
//! what: nothing is done until one is picked. The arrows pick one, Enter or a click does it, Escape closes the palette.

use claudius_api::{Command, Heard};
use claudius_intent::catalogue::ENTRIES;
use claudius_intent::{Answer, By, Finder, Found};
use eframe::egui::{self, Align2, Key, Modifiers, RichText, text::LayoutJob, vec2};

use super::{Cx, Main, Tab};
use crate::audio::record::Hearing;
use crate::theme::{self, Look, TEXT, TEXT_FAINT};
use crate::widgets::{self, Icon};

/// how many actions the palette shows at most
pub const SHOWN: usize = 12;

/// the palette while it is open: the words typed, and the action picked among those they find
#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    at: usize,
    /// the field is to take the keys, as it does when the palette opens
    focus: bool,
    /// where the status bar's link to it is, which opens and closes it itself: a press there is not one outside it
    pub link: Option<egui::Rect>,
    /// the words to ask the engine about, which the app sends it (`Backend::find`)
    pub ask: Option<String>,
    /// what the engine found for the words asked about last
    answer: Option<Answer>,
    /// when the words last changed, and when the engine was last asked about them (`Cx::now`)
    typed_at: f64,
    asked_at: f64,
    /// the action highlighted as the palette was last drawn, which Enter does, and whether the arrows picked it
    lit: Option<String>,
    moved: bool,
    /// the latest change made to the song when the palette last looked (the state's `history`)
    seen: Option<String>,
    /// whether what is said to the palette can be heard, which the app sets (`Backend::can_hear`)
    pub can_hear: bool,
    /// the microphone while it listens for what is said: whether it was told to stop, when it was asked to listen or,
    /// once it opened, when it began to hear, and when it last heard speech
    hearing: Option<Hearing>,
    stopping: bool,
    opened: bool,
    listened_at: f64,
    spoke_at: Option<f64>,
    /// what was said, a WAV file, which the app sends to be heard (`Backend::hear`); and that its words are not back
    pub say: Option<Vec<u8>>,
    waiting_words: bool,
    /// the words heard were typed in since the palette was last drawn
    retyped: bool,
    /// why what was said was not heard
    unheard: Option<String>,
    /// what there is to act on, as the palette was last drawn: it offers and finds only what works now
    now: Now,
}

/// how long a pause after speech ends listening, the longest the palette listens, and the longest it waits for the
/// microphone to open, which a browser asks the person about first (seconds); how loud speech is
const PAUSE: f64 = 1.2;
const LONGEST: f64 = 15.0;
pub(crate) const OPENING: f64 = 30.0;
const SPEECH: f32 = 0.02;

/// what the palette says when the microphone heard nothing at all
const NOTHING_HEARD: &str =
    "Nothing was heard. If claudius may not use the microphone, allow it in the system's privacy settings.";

/// whether listening is to stop: a pause after speech, or the longest it listens
fn said_enough(spoke_at: Option<f64>, listened_at: f64, now: f64) -> bool {
    spoke_at.is_some_and(|at| now - at >= PAUSE) || now - listened_at >= LONGEST
}

/// how often the palette asks again about words an engine found by name only, as its model may have come since, and
/// for how long after they were typed (seconds)
const AGAIN: (f64, f64) = (1.0, 60.0);

/// how often it asks again while the small model on the device reads the words, and for how long (seconds): the first
/// words it reads wait for it to download
const READING: (f64, f64) = (0.3, 60.0);

/// what a row's id starts with when it goes back to before a change made to the song (`Answer::done`), the change's
/// id after it
const BACK: &str = "back:";

/// what a row's id starts with when it does what a request asked (`Answer::proposed`), what it says it does after it
const DO: &str = "do:";

/// what a row's id starts with when it answers a question about the song (`Answer::told`), the answer after it
const TOLD: &str = "told:";

/// `text`, cut to `most` characters with an ellipsis
fn brief(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_string();
    }
    format!("{}…", text.chars().take(most - 1).collect::<String>().trim_end())
}

impl Palette {
    /// opened afresh, or closed
    pub fn toggle(&mut self) {
        *self = Palette { open: !self.open, focus: true, link: self.link, ..Palette::default() };
    }

    /// The words again, to the engine, while what it found for them it found by name only: a server answers so until
    /// its model has downloaded, and the demo once it has fetched its own.
    /// While the small model reads words the readings read nothing of, they are asked about again more often. The
    /// interval it waits for, while it does.
    fn ask_again(&mut self, now: f64) -> Option<f64> {
        let words = self.query.trim();
        let (every, waiting) = match self.answer.as_ref().filter(|a| a.query == words) {
            Some(a) if a.reading => (READING.0, now - self.typed_at < READING.1),
            Some(a) if !a.by_meaning => (AGAIN.0, now - self.typed_at < AGAIN.1),
            _ => return None,
        };
        if !waiting || words.is_empty() {
            return None;
        }
        if now - self.asked_at >= every {
            self.ask = Some(words.to_string());
            self.asked_at = now;
        }
        Some(every)
    }

    /// the small model is reading what is typed, and the palette still waits for what it reads
    fn reading(&self, now: f64) -> bool {
        self.answered().is_some_and(|a| a.reading) && now - self.typed_at < READING.1
    }

    /// The latest change made to the song (the state's `history`): one made since the palette last looked (a command
    /// the engine was busy for, a job's result) has the words asked about again, as the changes they find have changed.
    fn follow_history(&mut self, latest: Option<&str>) {
        if self.seen.as_deref() != latest {
            self.seen = latest.map(str::to_string);
            let words = self.query.trim();
            if !words.is_empty() {
                self.ask = Some(words.to_string());
            }
        }
    }

    /// Which of `acts` is highlighted after the arrows: the one picked with them stays picked as the list is ranked
    /// anew (an answer by meaning replacing one by name), and without them the best is.
    fn follow(&mut self, acts: &[Found], down: bool, up: bool) {
        let held = self.lit.as_ref().and_then(|id| acts.iter().position(|a| &a.id == id));
        let last = acts.len().saturating_sub(1);
        self.at = match (down, up) {
            (true, _) => held.map_or(0, |i| (i + 1).min(last)),
            (_, true) => held.map_or(0, |i| i.saturating_sub(1)),
            _ if self.moved => held.unwrap_or(0),
            _ => 0,
        };
        self.moved |= down || up;
    }

    /// what the engine found for some words: kept while they are what is typed
    pub fn heard(&mut self, answer: Answer) {
        if answer.query == self.query.trim() {
            self.answer = Some(answer);
        }
    }

    /// what the palette shows, for a scripted run to read: that it listens, that it hears what was said, why nothing
    /// was heard, or the words in its field and the first row they find
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn told(&self) -> String {
        if let Some(h) = &self.hearing {
            return if h.open() { "listening" } else { "opening the microphone" }.into();
        }
        if self.waiting_words {
            return "hearing what was said".into();
        }
        if let Some(why) = &self.unheard {
            return format!("not heard: {why}");
        }
        match (self.query.trim(), self.shown().first()) {
            ("", _) => String::new(),
            (words, Some(row)) => format!("{words} -> {}", row.name),
            (words, None) => words.to_string(),
        }
    }

    /// Starts listening for what is said, or while it listens, stops: what it heard goes to be heard.
    pub(crate) fn listen_or_stop(&mut self, now: f64) {
        if let Some(h) = self.hearing.as_mut() {
            h.stop();
            self.stopping = true;
            return;
        }
        match Hearing::start() {
            Ok(h) => {
                (self.hearing, self.stopping, self.opened) = (Some(h), false, false);
                (self.listened_at, self.spoke_at, self.unheard) = (now, None, None);
            }
            Err(why) => self.unheard = Some(why),
        }
    }

    /// While it listens: what the microphone heard taken in, and listening stopped after a pause once something was
    /// said, or at the longest; once all of it is in, it goes to be heard. The interval to look again in, while it
    /// listens.
    fn listening(&mut self, now: f64) -> Option<f64> {
        let h = self.hearing.as_mut()?;
        if let Some(why) = h.error() {
            self.hearing = None;
            self.unheard = Some(why);
            return None;
        }
        if h.level() > SPEECH {
            self.spoke_at = Some(now);
        }
        if !self.opened && h.open() {
            (self.opened, self.listened_at) = (true, now);
        }
        if !self.opened && now - self.listened_at >= OPENING {
            self.hearing = None;
            self.unheard = Some("The microphone did not open. Allow claudius to use it, and try again.".into());
            return None;
        }
        if self.opened && !self.stopping && said_enough(self.spoke_at, self.listened_at, now) {
            h.stop();
            self.stopping = true;
        }
        if self.stopping
            && let Some(wav) = h.finish()
        {
            self.hearing = None;
            match self.spoke_at {
                Some(_) => (self.say, self.waiting_words) = (Some(wav), true),
                None => self.unheard = Some(NOTHING_HEARD.into()),
            }
            return None;
        }
        Some(0.05)
    }

    /// The words what was said holds (`Event::Heard`), typed in as if they were typed and asked about; or why there
    /// are none.
    pub fn said(&mut self, heard: Result<Heard, String>) {
        self.waiting_words = false;
        match heard {
            Ok(h) if !h.text.trim().is_empty() => {
                self.query = h.text;
                self.ask = Some(self.query.trim().to_string());
                (self.retyped, self.focus) = (true, true);
            }
            Ok(_) => self.unheard = Some("No words were heard. Say it again, nearer the microphone.".into()),
            Err(why) => self.unheard = Some(why),
        }
    }

    /// the engine's answer, while it is for what is typed
    fn answered(&self) -> Option<&Answer> {
        self.answer.as_ref().filter(|a| a.query == self.query.trim())
    }

    /// the rows shown for what is typed: the answers to a question it asks about the song, what the engine read it to
    /// ask of the song, then the actions it found, by name and by meaning, then the changes made to the song it found,
    /// to go back to before (but those a reading goes back to before already); or until it answers, what the words find
    /// by name here
    ///
    /// Only what works now is shown (`works`): before a song is open, nothing that needs one.
    fn shown(&self) -> Vec<Found> {
        let now = self.now;
        let offered = |f: &Found| run_of(&f.id).is_some_and(|r| works(&r, &now));
        let Some(a) = self.answered() else {
            let found = Finder::new(ENTRIES, None).find(&self.query, None, ENTRIES.len());
            return found.into_iter().filter(offered).take(SHOWN).collect();
        };
        let told = a.told.iter().map(|t| Found {
            id: format!("{TOLD}{}", t.said),
            name: t.said.clone(),
            place: brief(&t.detail, 72),
            keys: String::new(),
            score: 1.0,
            by: By::Meaning,
        });
        let asked = a.proposed.iter().filter(|p| p.commands.iter().all(|c| does(c, &now))).map(|p| Found {
            id: format!("{DO}{}", p.said),
            name: p.said.clone(),
            place: brief(&p.detail, 72),
            keys: String::new(),
            score: p.score,
            by: By::Meaning,
        });
        let proposed_back = |id: &str| {
            a.proposed.iter().any(|p| p.commands.iter().any(|c| matches!(c, Command::GoBack { to } if to == id)))
        };
        let back = a.done.iter().filter(|d| now.song && !proposed_back(&d.id)).map(|d| Found {
            id: format!("{BACK}{}", d.id),
            name: format!("Go back to before {}", d.name),
            place: brief(&d.place, 72),
            keys: String::new(),
            score: d.score,
            by: d.by,
        });
        told.chain(asked).chain(a.found.iter().filter(|f| offered(f)).cloned()).chain(back).collect()
    }

    /// what the row `id` does: a reading's commands, while it is the engine's for what is typed, or an action's; an
    /// answer, nothing, which closes the palette as the person has read it
    fn run(&self, id: &str) -> Option<Run> {
        if let Some(said) = id.strip_prefix(TOLD) {
            return Some(Run::Do { said: said.to_string(), commands: Vec::new() });
        }
        let Some(said) = id.strip_prefix(DO) else { return run_of(id) };
        let p = self.answered()?.proposed.iter().find(|p| p.said == said)?;
        Some(Run::Do { said: p.said.clone(), commands: p.commands.clone() })
    }
}

/// what an action does
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Run {
    NewSong,
    Example,
    Open,
    Save,
    Play,
    ToStart,
    Stop,
    Loop,
    Undo,
    Redo,
    Render,
    Record,
    AddRecording,
    /// the Add a track window
    AddTrack,
    Phone,
    Paint,
    Tab(Tab),
    /// one of the song's views, or the arrangement again when it is the one shown
    View(Main),
    Variations,
    Improve,
    FixAll,
    Cancel,
    Stems,
    Packs,
    Publish,
    Assistants,
    Reference,
    Docs,
    Tour,
    /// the window of the keys the studio answers to
    Keys,
    /// the radio's window
    Radio,
    /// the chat with the assistants
    Chat,
    /// the Send feedback window
    Feedback,
    Look(Look),
    /// back to before a change made to the song: its id in the history
    GoBack(String),
    /// what a request asked, as the person confirmed it: what the button said, and the commands
    Do {
        said: String,
        commands: Vec<Command>,
    },
}

/// What there is to act on at a moment, as the studio's own buttons ask before they can be pressed: whether a song is
/// open, what the player holds, and the rest. The palette offers, finds and does only what works now (`works`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Now {
    /// a song is open
    pub song: bool,
    /// the player holds the song's audio, and no take is recording (which plays the song itself)
    pub audio: bool,
    /// a take is recording
    pub recording: bool,
    /// a change to undo, and one to redo
    pub undo: bool,
    pub redo: bool,
    /// claudius is at a job, which can be stopped
    pub busy: bool,
    /// the critic heard what quick fixes fix, in a verdict of the song as it is
    pub fixes: bool,
    /// a phone can play along: a claudius behind the studio, or the site's relay in a browser
    pub phone: bool,
    /// sample packs, or instruments of them, to download, and none downloading
    pub packs: bool,
    /// the chat has something in it: an assistant listening, messages, or assistants claudius can start there
    pub chat: bool,
}

impl Now {
    /// what there is to act on in the studio of `cx`, as its buttons see it
    pub(crate) fn of(cx: &Cx) -> Now {
        let st = &*cx.state;
        let recording = cx.view.record.recording();
        let downloading = st.busy.as_deref().is_some_and(|b| b.starts_with("downloading"));
        let chat = &st.chat;
        Now {
            song: st.song.is_some(),
            audio: cx.player.has_audio() && !recording,
            recording,
            undo: st.can_undo,
            redo: st.can_redo,
            busy: st.busy.is_some(),
            fixes: st.verdict.as_ref().is_some_and(|v| !v.fixes.is_empty()) && !cx.render_stale(),
            phone: cx.server.is_some() || cfg!(target_arch = "wasm32"),
            packs: cx.sounds.is_some_and(|l| l.packs.iter().any(|p| p.default && p.wanted())) && !downloading,
            chat: super::chat::listening(st, super::chat::now_ms()).is_some()
                || !chat.messages.is_empty()
                || chat.starts_agents,
        }
    }
}

/// Whether an action works `now`, as the button that does it in the studio would let it be pressed: those that need a
/// song once one is open, play once there is audio to play, undo once there is a change to undo, and so on; a new
/// song, an example, a song to open, the radio, the docs, the tour and the looks at any time.
pub(crate) fn works(run: &Run, now: &Now) -> bool {
    match run {
        Run::NewSong
        | Run::Example
        | Run::Open
        | Run::Assistants
        | Run::Reference
        | Run::Docs
        | Run::Tour
        | Run::Keys
        | Run::Radio
        | Run::Feedback
        | Run::Look(_) => true,
        // Space ends a take recording, as it plays the song otherwise
        Run::Play => now.audio || now.recording,
        Run::ToStart | Run::Stop => now.audio,
        Run::Undo => now.undo,
        Run::Redo => now.redo,
        Run::Cancel => now.busy,
        Run::FixAll => now.fixes,
        Run::Phone => now.phone,
        Run::Packs => now.packs,
        Run::Chat => now.chat,
        Run::Loop
        | Run::Save
        | Run::Render
        | Run::Record
        | Run::AddRecording
        | Run::AddTrack
        | Run::Paint
        | Run::Tab(_)
        | Run::View(_)
        | Run::Variations
        | Run::Improve
        | Run::Stems
        | Run::Publish
        | Run::GoBack(_) => now.song,
        Run::Do { commands, .. } => commands.iter().all(|c| does(c, now)),
    }
}

/// Whether a command a request was read as can be done `now`: one that makes or opens a song, listens to a recording,
/// downloads the packs or talks in the chat at any time; stopping a job while there is one; undo and redo while there
/// is a change to take back or bring back; the rest, which change, play or keep the song, once one is open.
fn does(c: &Command, now: &Now) -> bool {
    match c {
        Command::NewSong { .. }
        | Command::Load { .. }
        | Command::OpenText { .. }
        | Command::Analyze { .. }
        | Command::Remake { .. }
        | Command::SongLike { .. }
        | Command::InstallPacks { .. }
        | Command::ChatSay { .. }
        | Command::ChatListen { .. }
        | Command::ClearChat => true,
        Command::Cancel | Command::StopRemake { .. } => now.busy,
        Command::Undo => now.undo,
        Command::Redo => now.redo,
        Command::Together { commands, .. } => commands.iter().all(|c| does(c, now)),
        _ => now.song,
    }
}

/// The commands that do what a request asked: its edits as one change, which undo takes back at once and calls what
/// the button said; undo, redo and going back as they are, each a step.
fn commands_of(said: String, commands: Vec<Command>) -> Vec<Command> {
    if !commands.is_empty() && commands.iter().all(Command::is_edit) {
        vec![Command::Together { said, commands }]
    } else {
        commands
    }
}

/// the action of the catalogue's id, or of a row going back to before a change (`BACK`)
pub(crate) fn run_of(id: &str) -> Option<Run> {
    if let Some(change) = id.strip_prefix(BACK) {
        return Some(Run::GoBack(change.to_string()));
    }
    Some(match id {
        "play" => Run::Play,
        "to_start" => Run::ToStart,
        "stop" => Run::Stop,
        "loop" => Run::Loop,
        "undo" => Run::Undo,
        "redo" => Run::Redo,
        "new_song" => Run::NewSong,
        "example" => Run::Example,
        "open" => Run::Open,
        "save" => Run::Save,
        "render" => Run::Render,
        "record" => Run::Record,
        "add_recording" => Run::AddRecording,
        "add_track" => Run::AddTrack,
        "phone" => Run::Phone,
        "paint" => Run::Paint,
        "flow_large" | "tab:flow" => Run::View(Main::Flow),
        "tab:script" => Run::View(Main::Script),
        "variations" => Run::Variations,
        "improve" => Run::Improve,
        "fix_all" => Run::FixAll,
        "cancel" => Run::Cancel,
        "stems" => Run::Stems,
        "packs" => Run::Packs,
        "publish" => Run::Publish,
        "assistants" => Run::Assistants,
        "reference" => Run::Reference,
        "docs" => Run::Docs,
        "tour" => Run::Tour,
        "keys" => Run::Keys,
        "radio" => Run::Radio,
        "chat" => Run::Chat,
        "feedback" => Run::Feedback,
        "look:dark" => Run::Look(Look::Dark),
        "look:light" => Run::Look(Look::Light),
        "look:system" => Run::Look(Look::System),
        other => Run::Tab(match other.strip_prefix("tab:")? {
            "drums" => Tab::Sequencer,
            "instruments" => Tab::Parts,
            "notes" => Tab::Notes,
            "chords" => Tab::Harmony,
            "mixer" => Tab::Mixer,
            "sounds" => Tab::Sounds,
            "log" => Tab::Log,
            _ => return None,
        }),
    })
}

/// what the app does itself for an action the palette picked
pub enum Asked {
    Open,
    Tour,
}

/// The palette, while it is open: a field to type in and the actions it finds. What the app does itself comes back.
pub fn show(ctx: &egui::Context, cx: &mut Cx) -> Option<Asked> {
    if !cx.view.palette.open {
        return None;
    }
    let now = Now::of(cx);
    let p = &mut cx.view.palette;
    p.now = now;
    p.follow_history(cx.state.history.first().map(|d| d.id.as_str()));
    // the keys are the palette's, but for a field elsewhere being typed in
    let field_id = egui::Id::unique("command palette field");
    let ours = !ctx.text_edit_focused() || ctx.memory(|m| m.has_focus(field_id));
    let (down, up, enter, escape) = ctx.input_mut(|i| {
        (
            ours && i.consume_key(Modifiers::NONE, Key::ArrowDown),
            ours && i.consume_key(Modifiers::NONE, Key::ArrowUp),
            ours && i.consume_key(Modifiers::NONE, Key::Enter),
            ours && i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    if escape {
        p.open = false;
        return None;
    }
    let now = cx.now;
    if std::mem::take(&mut p.retyped) {
        (p.at, p.moved, p.typed_at, p.asked_at) = (0, false, now, now);
    }
    let acts = p.shown();
    // Enter does what was highlighted as the palette was drawn, whatever the list has been ranked as since
    let lit = p.lit.clone().or_else(|| acts.get(p.at).map(|a| a.id.clone()));
    let mut picked = enter.then(|| lit.as_deref().and_then(|id| p.run(id))).flatten();
    p.follow(&acts, down, up);
    let cmd = widgets::cmd_key(ctx);
    let shown = egui::Window::new("command palette")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 72.0))
        .show(ctx, |ui| {
            ui.set_width(520.0);
            let mut listen = false;
            let field = ui
                .horizontal(|ui| {
                    let mic = if p.can_hear { 30.0 } else { 0.0 };
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut p.query)
                            .id(field_id)
                            .hint_text("Type what to do: new song, more reverb on the snare, undo the last 3 changes ...")
                            .desired_width(ui.available_width() - mic)
                            .font(egui::TextStyle::Heading),
                    );
                    if p.can_hear {
                        let listening = p.hearing.is_some();
                        let tip = if listening { "Stop listening, and read what you said" } else { "Say what to do" };
                        listen = widgets::icon_button(ui, Icon::Voice, 18.0, listening, !p.waiting_words, tip).clicked();
                    }
                    field
                })
                .inner;
            if std::mem::take(&mut p.focus) {
                field.request_focus();
            }
            if field.changed() {
                (p.at, p.moved, p.typed_at, p.asked_at) = (0, false, now, now);
                let words = p.query.trim();
                p.ask = (!words.is_empty()).then(|| words.to_string());
                p.unheard = None;
            }
            if listen {
                p.listen_or_stop(now);
            }
            ui.add_space(4.0);
            if let Some(h) = &p.hearing {
                // what is said before the input opens is lost: the person is told when it hears
                let said = if h.open() { "Listening… say what to do, then pause" } else { "Opening the microphone…" };
                ui.label(RichText::new(said).size(11.0).color(TEXT_FAINT));
            } else if p.waiting_words {
                ui.label(RichText::new("Hearing your words…").size(11.0).color(TEXT_FAINT));
            } else if let Some(why) = &p.unheard {
                ui.label(RichText::new(why).size(11.0).color(theme::WARN));
            } else if p.reading(now) {
                ui.label(RichText::new("Reading your words…").size(11.0).color(TEXT_FAINT));
            } else if let Some(read) = p.answered().and_then(|a| a.read_as.as_deref()) {
                // a word typed with a slip, read as the word meant: what was found is for these words
                ui.label(RichText::new(format!("Read as “{read}”")).size(11.0).color(TEXT_FAINT));
            } else if acts.is_empty() {
                ui.label(RichText::new("Nothing by that name. Try other words, or read the docs.").color(TEXT_FAINT));
            }
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                for (i, a) in acts.iter().enumerate() {
                    // the answers to a question come first, then what the words asked, then the actions, then the
                    // changes made to the song, each of those with what it says, changes or changed under it
                    let (told, asked, back) = (a.id.starts_with(TOLD), a.id.starts_with(DO), a.id.starts_with(BACK));
                    if told && i == 0 {
                        ui.label(RichText::new("The song now").size(10.5).color(TEXT_FAINT));
                    }
                    if asked && !acts[..i].iter().any(|b| b.id.starts_with(DO)) {
                        ui.add_space(if i > 0 { 4.0 } else { 0.0 });
                        ui.label(RichText::new("What your words ask").size(10.5).color(TEXT_FAINT));
                    }
                    let after_asked = i > 0 && (acts[i - 1].id.starts_with(DO) || acts[i - 1].id.starts_with(TOLD));
                    if !told && !asked && !back && after_asked {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Actions").size(10.5).color(TEXT_FAINT));
                    }
                    if back && !acts[..i].iter().any(|b| b.id.starts_with(BACK)) {
                        ui.add_space(4.0);
                        ui.label(RichText::new("In the song's history").size(10.5).color(TEXT_FAINT));
                    }
                    let under = (told || asked || back) && !a.place.is_empty();
                    let mut job = LayoutJob::default();
                    job.append(&a.name, 0.0, egui::TextFormat::simple(theme::prop(13.5), TEXT.into()));
                    let place = egui::TextFormat::simple(theme::prop(11.0), TEXT_FAINT.into());
                    if under {
                        job.append(&format!("\n{}", a.place), 0.0, place);
                    } else if !a.place.is_empty() {
                        job.append(&a.place, 10.0, place);
                    }
                    let keys = a.keys.replace("Cmd", cmd);
                    let r = ui.add_sized(
                        vec2(ui.available_width(), if under { 40.0 } else { 26.0 }),
                        egui::Button::new(job)
                            .selected(i == p.at)
                            .shortcut_text(RichText::new(keys).size(11.0).color(TEXT_FAINT)),
                    );
                    let by_model = asked
                        && p.answered().is_some_and(|ans| {
                            ans.proposed.iter().any(|x| x.reading.by_model && a.id.strip_prefix(DO) == Some(x.said.as_str()))
                        });
                    let r = match (asked, a.by) {
                        _ if told => r.on_hover_text("what the song holds now: picking it changes nothing"),
                        (true, _) if by_model => r.on_hover_text(
                            "what the small model on this computer read your words to ask: nothing changes until you pick it",
                        ),
                        (true, _) => {
                            r.on_hover_text("what claudius read your words to ask: nothing changes until you pick it")
                        }
                        (_, By::Meaning) => r.on_hover_text("found by what your words mean"),
                        _ => r,
                    };
                    if i == p.at && (down || up) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        picked = p.run(&a.id);
                    }
                }
            });
            ui.add_space(2.0);
            let by = if p.answer.as_ref().is_some_and(|a| a.by_meaning) {
                "finds what you mean, on this computer"
            } else {
                "finds actions by name"
            };
            ui.label(
                RichText::new(format!(
                    "{cmd}+K opens this from anywhere · {by} · arrows pick · Enter does it · Esc closes"
                ))
                .size(10.5)
                .color(TEXT_FAINT),
            );
        });
    p.lit = acts.get(p.at).map(|a| a.id.clone());
    if let Some(every) = p.ask_again(now) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(every));
    }
    if let Some(every) = p.listening(now) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(every));
    }
    // a click outside it closes it, as a menu closes
    let link = cx.view.palette.link;
    let outside = |at: egui::Pos2| {
        shown.as_ref().is_some_and(|s| !s.response.rect.contains(at)) && !link.is_some_and(|l| l.contains(at))
    };
    if ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(outside)) {
        cx.view.palette.open = false;
    }
    let run = picked?;
    cx.view.palette.open = false;
    act(ctx, cx, run)
}

/// Does what an action of the catalogue does, picked in the palette or by its key (`keys::answer`); what the app does
/// itself for it, if anything. An action that does not work now (`works`) does nothing, as its button cannot be pressed.
pub(crate) fn act(ctx: &egui::Context, cx: &mut Cx, run: Run) -> Option<Asked> {
    if !works(&run, &Now::of(cx)) {
        return None;
    }
    let example = run == Run::Example;
    match run {
        Run::NewSong | Run::Example => {
            cx.view.new_song.open = true;
            if let Some(song) = &cx.state.song {
                cx.view.new_song.set_genre(&song.genre);
            }
            if example {
                cx.view.new_song.start = super::Start::Example;
            }
        }
        Run::Open => return Some(Asked::Open),
        Run::Save => super::transport::ask_download(cx.state, cx.view, cx.out, cx.server.is_none(), cx.now, None),
        // as Space does: a take recording ends, and the song plays only once it has its audio
        Run::Play if cx.view.record.recording() => cx.view.record.stop = true,
        Run::Play if cx.player.is_playing() => cx.player.pause(),
        Run::Play => cx.player.play(),
        Run::ToStart => cx.player.seek(0.0),
        Run::Stop => cx.player.stop(),
        Run::Loop => cx.view.looping = !cx.view.looping,
        Run::Undo => cx.out.push(Command::Undo),
        Run::Redo => cx.out.push(Command::Redo),
        Run::Render => cx.out.push(Command::Render { stems: false }),
        Run::Record => cx.view.record.show(),
        Run::AddRecording => cx.view.import.open = true,
        Run::AddTrack => cx.view.add_track.open = true,
        Run::Phone => cx.view.phone.show(cx.state.phone.is_some()),
        Run::Paint => cx.view.paint = !cx.view.paint,
        // the dock is drawn with the arrangement only: a tab picked shows it again
        Run::Tab(t) => {
            cx.view.tab = t;
            cx.view.main = Main::Arrangement;
        }
        Run::View(m) => {
            let open = cx.view.main != m && cx.state.song.is_some();
            match m {
                Main::Flow => super::graph::open(cx, open),
                _ => cx.view.main = if open { m } else { Main::Arrangement },
            }
        }
        // as the status bar's buttons do
        Run::Variations => cx.out.push(Command::Mutate { count: cx.view.mutate_count, keep_best: cx.view.keep_best }),
        Run::Improve => cx.out.push(Command::Evolve { generations: cx.view.generations, children: cx.view.children }),
        Run::FixAll => cx.out.push(Command::QuickFix { target: None }),
        Run::Cancel => cx.out.push(Command::Cancel),
        Run::Stems => cx.out.push(Command::Render { stems: true }),
        Run::Packs => cx.out.push(Command::InstallPacks { names: Vec::new() }),
        Run::Publish => cx.view.publish.show(),
        Run::Assistants => cx.view.assistants.show(),
        Run::Reference => cx.view.reference_open = true,
        Run::Docs => ctx.open_url(egui::OpenUrl::new_tab(widgets::DOCS)),
        Run::Tour => return Some(Asked::Tour),
        Run::Keys => cx.view.keys.open = !cx.view.keys.open,
        Run::Radio => cx.view.radio.toggle(),
        Run::Chat => cx.view.chat.open = true,
        Run::Feedback => cx.view.feedback.show(),
        Run::Look(look) => {
            (cx.view.look, cx.view.theme) = (look, None);
            theme::wear(ctx, None, look);
        }
        Run::GoBack(to) => cx.out.push(Command::GoBack { to }),
        Run::Do { said, commands } => cx.out.extend(commands_of(said, commands)),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a song open and playing, with a change to undo: what the tests of finding and reading look for works
    const SONG: Now = Now {
        song: true,
        audio: true,
        recording: false,
        undo: true,
        redo: false,
        busy: false,
        fixes: false,
        phone: true,
        packs: false,
        chat: false,
    };

    #[test]
    fn every_action_of_the_catalogue_is_one_the_palette_does_and_every_tab_is_there() {
        for e in ENTRIES {
            assert!(run_of(e.id).is_some(), "{}: the palette does nothing for it", e.id);
        }
        for t in Tab::ALL {
            assert!(ENTRIES.iter().any(|e| run_of(e.id) == Some(Run::Tab(t)) && e.name == t.label()), "{}", t.label());
        }
        // and the song's two views beside the arrangement, which the switcher shows too
        for m in [Main::Flow, Main::Script] {
            assert!(ENTRIES.iter().any(|e| run_of(e.id) == Some(Run::View(m)) && e.name == m.label()), "{}", m.label());
        }
    }

    #[test]
    fn what_is_typed_is_found_by_name_until_the_engine_answers_for_it() {
        let mut p = Palette { query: "mix".into(), now: SONG, ..Palette::default() };
        assert_eq!(p.shown()[0].name, "Mixer");
        let found = |name: &str| Found {
            id: "tab:sounds".into(),
            name: name.into(),
            place: "tab".into(),
            keys: String::new(),
            score: 0.8,
            by: By::Meaning,
        };
        p.heard(Answer {
            query: "mi".into(),
            found: vec![found("an old answer")],
            by_meaning: true,
            ..Answer::default()
        });
        assert_eq!(p.shown()[0].name, "Mixer", "an answer for other words is not shown");
        p.heard(Answer { query: "mix".into(), found: vec![found("Sounds")], by_meaning: true, ..Answer::default() });
        assert_eq!(p.shown()[0].name, "Sounds", "the engine's answer, by meaning too");
        p.query = "mixer".into();
        assert_eq!(p.shown()[0].name, "Mixer", "words typed since are found by name until it answers for them");
    }

    fn act(id: &str) -> Found {
        Found { id: id.into(), name: id.into(), place: String::new(), keys: String::new(), score: 0.5, by: By::Name }
    }

    #[test]
    fn an_action_picked_with_the_arrows_stays_picked_as_the_list_is_ranked_anew_and_else_the_best_is() {
        let mut p = Palette::default();
        let by_name = [act("new_song"), act("tab:mixer"), act("improve")];
        let by_meaning = [act("tab:mixer"), act("improve"), act("new_song")];
        p.follow(&by_name, false, false);
        p.lit = Some(by_name[p.at].id.clone());
        p.follow(&by_meaning, false, false);
        assert_eq!(by_meaning[p.at].id, "tab:mixer", "nothing picked: the best of the new ranking");
        p.lit = Some(by_meaning[p.at].id.clone());
        p.follow(&by_meaning, true, false);
        assert_eq!(by_meaning[p.at].id, "improve");
        p.lit = Some(by_meaning[p.at].id.clone());
        p.follow(&by_name, false, false);
        assert_eq!(by_name[p.at].id, "improve", "picked with the arrows: it stays picked where the list moved it");
    }

    #[test]
    fn a_change_the_words_found_is_a_row_after_the_actions_that_goes_back_to_before_it() {
        let mut p = Palette { query: "before the whistles".into(), now: SONG, ..Palette::default() };
        let change = Found {
            id: "7@4f2a".into(),
            name: "track whistle".into(),
            place: "added the whistle track (lead), playing the synth whistle, and a long while more of it".into(),
            keys: String::new(),
            score: 0.7,
            by: By::Meaning,
        };
        p.heard(Answer {
            query: p.query.clone(),
            found: vec![act("undo")],
            done: vec![change],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["undo", "Go back to before track whistle"]
        );
        assert!(rows[1].place.ends_with('…') && rows[1].place.chars().count() == 72, "{}", rows[1].place);
        assert_eq!(run_of(&rows[1].id), Some(Run::GoBack("7@4f2a".into())));
    }

    #[test]
    fn a_change_made_while_the_palette_is_open_has_its_words_asked_about_again() {
        let mut p = Palette::default();
        p.follow_history(Some("1@a"));
        assert!(p.ask.is_none(), "nothing typed, nothing to ask");
        p.query = "faster".into();
        p.follow_history(Some("1@a"));
        assert!(p.ask.is_none(), "no change since it last looked");
        p.follow_history(Some("2@a"));
        assert_eq!(p.ask.take().as_deref(), Some("faster"), "a change the engine was busy for, made since");
        p.follow_history(Some("2@a"));
        assert!(p.ask.is_none());
    }

    #[test]
    fn words_found_by_name_only_are_asked_about_again_a_second_apart_for_a_minute() {
        let mut p = Palette { query: "make it louder".into(), ..Palette::default() };
        let answer = |by_meaning| Answer {
            query: "make it louder".into(),
            found: vec![act("tab:mixer")],
            by_meaning,
            ..Answer::default()
        };
        p.heard(answer(false));
        assert!(p.ask_again(0.5).is_some() && p.ask.is_none(), "not within a second of asking");
        assert!(p.ask_again(1.0).is_some() && p.ask.take().as_deref() == Some("make it louder"));
        assert!(p.ask_again(1.5).is_some() && p.ask.is_none());
        assert!(p.ask_again(61.0).is_none() && p.ask.is_none(), "a minute after the words were typed, no more");
        p.heard(answer(true));
        assert!(p.ask_again(2.0).is_none() && p.ask.is_none(), "found by meaning: nothing more to ask");
    }

    #[test]
    fn words_the_small_model_is_reading_are_asked_about_again_until_it_has_read_them() {
        let mut p = Palette { query: "can the kick be a bit softer".into(), ..Palette::default() };
        let query = p.query.clone();
        let answer = |reading| Answer { query: query.clone(), by_meaning: true, reading, ..Answer::default() };
        p.heard(answer(true));
        assert!(p.reading(0.1));
        assert_eq!(p.ask_again(0.1), Some(READING.0), "not within its interval of asking");
        assert!(p.ask.is_none());
        assert_eq!(p.ask_again(0.3), Some(READING.0));
        assert_eq!(p.ask.take().as_deref(), Some("can the kick be a bit softer"));
        assert!(p.ask_again(30.0).is_some(), "the first words wait for the model to download");
        assert!(p.ask_again(61.0).is_none(), "long after the words were typed, it stops asking");
        assert!(!p.reading(61.0), "and says no more that it is reading");
        p.heard(answer(false));
        assert!(!p.reading(0.6) && p.ask_again(0.6).is_none(), "read: nothing more to ask");
    }

    #[test]
    fn listening_stops_after_a_pause_once_something_was_said_or_at_the_longest() {
        assert!(!said_enough(None, 0.0, 5.0), "nothing said yet: it listens on");
        assert!(!said_enough(Some(4.5), 0.0, 5.0), "a breath is no pause");
        assert!(said_enough(Some(3.0), 0.0, 3.0 + PAUSE));
        assert!(said_enough(None, 0.0, LONGEST), "so long at most");
    }

    #[test]
    fn the_words_heard_are_typed_in_and_asked_about() {
        let mut p = Palette { waiting_words: true, ..Palette::default() };
        p.said(Ok(Heard { heard: "Undu the last two changes.".into(), text: "undo the last two changes".into() }));
        assert_eq!(
            (p.query.as_str(), p.ask.as_deref()),
            ("undo the last two changes", Some("undo the last two changes"))
        );
        assert!(p.retyped && p.focus && !p.waiting_words);
        p.said(Ok(Heard::default()));
        assert!(p.unheard.as_deref().is_some_and(|w| w.starts_with("No words were heard")), "{:?}", p.unheard);
        p.said(Err("this claudius cannot hear what is said: update it".into()));
        assert_eq!(p.unheard.as_deref(), Some("this claudius cannot hear what is said: update it"));
    }

    fn proposal(said: &str, commands: Vec<Command>) -> claudius_intent::Proposal {
        claudius_intent::Proposal {
            said: said.into(),
            detail: String::new(),
            commands,
            reading: claudius_intent::ask::Reading::default(),
            score: 0.9,
        }
    }

    fn muted(track: &str) -> Command {
        Command::SetMix {
            track: track.into(),
            level_db: None,
            pan: None,
            reverb: None,
            delay: None,
            duck: None,
            drive_db: None,
            muted: Some(true),
            solo: None,
            bus: None,
        }
    }

    #[test]
    fn the_answer_to_a_question_comes_first_and_picking_it_changes_nothing() {
        let mut p = Palette { query: "is the bass too loud".into(), now: SONG, ..Palette::default() };
        let told = claudius_intent::Told { said: "Bass is at -3 dB".into(), detail: String::new() };
        let down = proposal("Turn bass down from -3 dB to -6 dB", vec![muted("bass")]);
        p.heard(Answer {
            query: p.query.clone(),
            found: vec![act("tab:mixer")],
            proposed: vec![down],
            told: vec![told],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Bass is at -3 dB", "Turn bass down from -3 dB to -6 dB", "tab:mixer"]);
        let Some(Run::Do { said, commands }) = p.run(&rows[0].id) else { panic!("{:?}", p.run(&rows[0].id)) };
        assert!(commands_of(said, commands).is_empty(), "an answer sends nothing");
    }

    #[test]
    fn what_a_request_asks_comes_first_each_reading_a_row_that_does_its_edits_as_one_change() {
        let mut p = Palette { query: "mute the drums".into(), now: SONG, ..Palette::default() };
        let mute = proposal("Mute kick and snare", vec![muted("kick"), muted("snare")]);
        p.heard(Answer {
            query: p.query.clone(),
            found: vec![act("tab:mixer")],
            proposed: vec![mute],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Mute kick and snare", "tab:mixer"]);
        let Some(Run::Do { said, commands }) = p.run(&rows[0].id) else { panic!("{:?}", p.run(&rows[0].id)) };
        let sent = commands_of(said, commands);
        assert_eq!(
            sent,
            [Command::Together { said: "Mute kick and snare".into(), commands: vec![muted("kick"), muted("snare")] }],
            "one change, which undo calls what the button said"
        );
        p.query = "mute the drum".into();
        assert!(p.run(&rows[0].id).is_none(), "words typed since: what it read for others is done no more");
        assert_eq!(
            commands_of("Undo the last 2 changes".into(), vec![Command::Undo, Command::Undo]),
            [Command::Undo, Command::Undo],
            "undo is steps of its own"
        );
    }

    #[test]
    fn a_change_a_reading_goes_back_to_before_is_not_offered_again_in_the_history() {
        let mut p = Palette { query: "go back to before the whistles".into(), now: SONG, ..Palette::default() };
        let change = Found {
            id: "7@4f2a".into(),
            name: "track whistle".into(),
            place: "added the whistle track".into(),
            keys: String::new(),
            score: 0.7,
            by: By::Meaning,
        };
        let back = proposal("Go back to before track whistle", vec![Command::GoBack { to: "7@4f2a".into() }]);
        p.heard(Answer {
            query: p.query.clone(),
            done: vec![change],
            proposed: vec![back],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Go back to before track whistle"]);
        assert_eq!(
            p.run(&rows[0].id),
            Some(Run::Do {
                said: "Go back to before track whistle".into(),
                commands: vec![Command::GoBack { to: "7@4f2a".into() }]
            })
        );
    }

    #[test]
    fn before_a_song_is_open_the_palette_offers_and_finds_only_what_works_without_one() {
        let rows = Palette::default().shown();
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        for id in ["new_song", "example", "open", "radio", "assistants", "docs", "tour", "keys", "look:dark"] {
            assert!(ids.contains(&id), "{id} works with no song open: {ids:?}");
        }
        for id in ["play", "undo", "save", "render", "record", "add_recording", "tab:mixer", "tab:flow", "improve"] {
            assert!(!ids.contains(&id), "{id} needs a song: {ids:?}");
        }
        for words in ["mixer", "save", "play", "record a take", "drums", "signal flow", "fix"] {
            let rows = Palette { query: words.into(), ..Palette::default() }.shown();
            assert!(
                rows.iter().all(|r| run_of(&r.id).is_some_and(|run| works(&run, &Now::default()))),
                "{words}: {rows:?}"
            );
        }
        let rows = Palette { query: "new".into(), ..Palette::default() }.shown();
        assert_eq!(rows.first().map(|r| r.id.as_str()), Some("new_song"), "what works is still found");
    }

    #[test]
    fn what_the_engine_finds_and_reads_is_offered_only_where_it_works_now() {
        let mut p = Palette { query: "a house song with the drums muted".into(), ..Palette::default() };
        let new_song = Command::NewSong {
            genre: "house".into(),
            seed: None,
            bpm: None,
            key: None,
            loops: false,
            sampled: None,
            sample_search: None,
            complexity: None,
        };
        let change = Found {
            id: "3@1c".into(),
            name: "track drums".into(),
            place: String::new(),
            keys: String::new(),
            score: 0.6,
            by: By::Meaning,
        };
        p.heard(Answer {
            query: p.query.clone(),
            found: vec![act("tab:mixer"), act("new_song")],
            done: vec![change],
            proposed: vec![
                proposal("Mute the drums", vec![muted("drums")]),
                proposal("New house song", vec![new_song]),
            ],
            by_meaning: true,
            ..Answer::default()
        });
        let names = |p: &Palette| p.shown().iter().map(|r| r.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&p), ["New house song", "new_song"], "with no song open, what needs one is not offered");
        p.now = SONG;
        assert_eq!(
            names(&p),
            ["Mute the drums", "New house song", "tab:mixer", "new_song", "Go back to before track drums"],
            "with a song open, all of it"
        );
    }

    #[test]
    fn an_action_is_offered_once_it_can_be_done_as_its_button_can_be_pressed() {
        let song = Now { song: true, ..Now::default() };
        let offered = |now: Now, id: &str| works(&run_of(id).expect(id), &now);
        assert!(offered(song, "tab:mixer") && offered(song, "save") && offered(song, "add_recording"));
        assert!(!offered(song, "play") && offered(Now { audio: true, ..song }, "play"), "play once there is audio");
        assert!(offered(Now { recording: true, ..song }, "play"), "Space ends a take recording");
        assert!(!offered(Now { recording: true, ..song }, "stop"), "the take plays the song, not the player");
        assert!(!offered(song, "undo") && offered(Now { undo: true, ..song }, "undo"));
        assert!(!offered(song, "redo") && offered(Now { redo: true, ..song }, "redo"));
        assert!(!offered(song, "cancel") && offered(Now { busy: true, ..song }, "cancel"), "a job to stop");
        assert!(!offered(song, "fix_all") && offered(Now { fixes: true, ..song }, "fix_all"), "fixes the critic found");
        assert!(!offered(song, "phone") && offered(Now { phone: true, ..Now::default() }, "phone"));
        assert!(!offered(song, "packs") && offered(Now { packs: true, ..Now::default() }, "packs"));
        assert!(!offered(song, "chat") && offered(Now { chat: true, ..Now::default() }, "chat"));
        let back = |commands: Vec<Command>, now: Now| works(&Run::Do { said: String::new(), commands }, &now);
        assert!(
            !back(vec![Command::Undo, Command::Undo], song) && back(vec![Command::Undo], Now { undo: true, ..song })
        );
        assert!(back(Vec::new(), Now::default()), "an answer, which changes nothing, is always shown");
    }
}
