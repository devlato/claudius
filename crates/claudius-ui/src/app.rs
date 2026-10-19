//! The studio: owns the backend (a claudius server or the mock), the state on screen, the player and the UI state,
//! and lays the panels out like a DAW: transport on top, the arrangement in the middle, the editors docked below,
//! the section inspector and the verdict on the right.

use std::sync::Arc;
use std::time::Duration;

use claudius_api::{Command, SoundList, State};
use eframe::egui::{self, Align, Color32, Key, Layout, Margin, Modifiers, RichText, Sense, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};

use crate::audio::Player;
use crate::edits;
use crate::native::{Native, Update};
use crate::net::{Backend, Connection, Event};
use crate::panels::{self, Action, Cx, Link, Tab, Toast, View};
use crate::theme::{self, ACCENT, BG, LINE, LINE_SOFT, PANEL, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets::{self, Icon};

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Prefs {
    tab: Tab,
    volume: f32,
    stems: bool,
    mutate_count: usize,
    keep_best: bool,
    generations: usize,
    children: usize,
    sevenths: bool,
    tour_done: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        let v = View::default();
        Prefs {
            tab: v.tab,
            volume: v.volume,
            stems: v.stems,
            mutate_count: v.mutate_count,
            keep_best: v.keep_best,
            generations: v.generations,
            children: v.children,
            sevenths: v.sevenths,
            tour_done: v.tour_done,
        }
    }
}

/// the claudius studio (an `eframe::App`)
pub struct Studio {
    backend: Backend,
    state: State,
    /// the newest state from the server, held back while the user drags something or edits are in flight
    pending: Option<State>,
    /// the last server state shown, without local edits (to fall back to when the server rejects an edit)
    server: State,
    has_state: bool,
    inflight: usize,
    inflight_since: f64,
    /// the highest `Reply.version` so far: a state older than this misses an edit the server already made
    confirmed: u64,
    /// where the studio was pointed at start (to come back from the demo)
    home: Connection,
    link: Link,
    sounds: Option<SoundList>,
    player: Player,
    view: View,
    out: Vec<Command>,
    actions: Vec<Action>,
    title: String,
    loop_region: Option<(f64, f64)>,
    /// the server's base URL, for links to its files
    server_url: Option<String>,
    /// play the demo if the server cannot be reached at all (a web page served without claudius behind it)
    pub fallback_to_demo: bool,
    /// what the native app can do on this computer (connect assistants); none on the web
    pub native: Option<Arc<dyn Native>>,
    #[cfg(not(target_arch = "wasm32"))]
    plan: Option<crate::devplan::Plan>,
}

/// the docs and the bug report form, for the help menu
const DOCS: &str = "https://github.com/devlato/claudius/blob/main/docs/src/SUMMARY.md";
const REPORT: &str = "https://github.com/devlato/claudius/issues/new?template=bug_report.yml";
const RELEASES: &str = "https://github.com/devlato/claudius/releases";
const UPDATES_TIP: &str = "look for a new release on GitHub once a day and download it in the background; it installs \
                           when you click Restart to update";
const STATISTICS_TIP: &str = "a few anonymous counts, each at most once a day: the version, the system, which part you \
                              use, and where a setup or a crash failed; never your songs, files or name (`claudius \
                              telemetry` shows exactly what)";

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>, connection: Connection) -> Studio {
        theme::apply(&cc.egui_ctx);
        let prefs: Prefs = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        let mut view = View {
            tab: prefs.tab,
            volume: prefs.volume,
            stems: prefs.stems,
            mutate_count: prefs.mutate_count,
            keep_best: prefs.keep_best,
            generations: prefs.generations,
            children: prefs.children,
            sevenths: prefs.sevenths,
            tour_done: prefs.tour_done,
            ..View::default()
        };
        view.volume = view.volume.clamp(0.0, 1.0);
        let backend = Backend::new(&connection, &cc.egui_ctx);
        let mut player = Player::new(&cc.egui_ctx);
        player.set_volume(view.volume);
        let link = Link { label: backend.label(), mock: backend.is_mock(), ..Default::default() };
        Studio {
            backend,
            state: State::default(),
            pending: None,
            server: State::default(),
            has_state: false,
            inflight: 0,
            inflight_since: 0.0,
            confirmed: 0,
            server_url: match &connection {
                Connection::Server(url) => Some(url.clone()),
                Connection::Mock => None,
            },
            home: connection,
            link,
            sounds: None,
            player,
            view,
            out: Vec::new(),
            actions: Vec::new(),
            title: String::new(),
            loop_region: None,
            fallback_to_demo: false,
            native: None,
            #[cfg(not(target_arch = "wasm32"))]
            plan: crate::devplan::Plan::from_env(),
        }
    }

    fn switch(&mut self, connection: Connection, ctx: &egui::Context) {
        self.backend = Backend::new(&connection, ctx);
        self.state = State::default();
        self.pending = None;
        self.server = State::default();
        self.has_state = false;
        self.inflight = 0;
        self.confirmed = 0;
        self.sounds = None;
        self.link = Link { label: self.backend.label(), mock: self.backend.is_mock(), ..Default::default() };
        self.server_url = match &connection {
            Connection::Server(url) => Some(url.clone()),
            Connection::Mock => None,
        };
        self.player.clear();
        self.view.reference = None;
        self.view.reference_loading = false;
    }

    /// the tour from step `i`; it asks about usage statistics at the end when the app has no answer yet
    fn start_tour(&mut self, i: usize) {
        self.view.tour = Some(i);
        self.view.tour_asks = self.native.as_ref().is_some_and(|n| n.statistics().is_none());
    }

    fn toast(&mut self, text: String, ok: bool, now: f64) {
        self.view.toast = Some(Toast { text, ok, at: now });
    }

    fn handle(&mut self, ev: Event, now: f64) {
        match ev {
            Event::State(s) => {
                self.link.online = true;
                self.link.ever = true;
                self.link.error = None;
                self.pending = Some(*s);
            }
            Event::PollFailed(e) => {
                self.link.online = false;
                self.link.error = Some(e);
                self.confirmed = 0;
            }
            Event::Reply { what, result } => {
                self.inflight = self.inflight.saturating_sub(1);
                let script = what == "run_script";
                if matches!(what.as_str(), "analyze" | "remake") {
                    self.view.analyzing = false;
                    let (ok, message) = match &result {
                        Ok(r) => (r.ok, r.message.clone()),
                        Err(e) => (false, e.clone()),
                    };
                    if ok && what == "remake" {
                        self.view.new_song.open = false;
                    }
                    self.view.analysis = Some((ok, message));
                }
                let rejected = !matches!(&result, Ok(r) if r.ok);
                if rejected && is_edit(&what) && self.pending.is_none() {
                    self.state.song = self.server.song.clone();
                }
                match result {
                    Ok(reply) => {
                        self.confirmed = self.confirmed.max(reply.version);
                        if script {
                            self.view.script.awaiting = false;
                            self.view.script.result = Some((reply.ok, reply.message.clone()));
                            if reply.ok {
                                self.view.script.dirty = false;
                            }
                        }
                        let text = if reply.message.is_empty() { what.replace('_', " ") } else { reply.message };
                        self.toast(text, reply.ok, now);
                    }
                    Err(e) => {
                        if script {
                            self.view.script.awaiting = false;
                            self.view.script.result = Some((false, e.clone()));
                        }
                        self.toast(format!("could not send {}: {e}", what.replace('_', " ")), false, now);
                    }
                }
            }
            Event::Sounds(list) => self.sounds = Some(list),
            Event::Hello { name, ears } => {
                self.link.server = Some(name);
                self.link.ears = ears;
            }
            Event::Reference(result) => {
                self.view.reference_loading = false;
                self.view.reference =
                    Some(result.unwrap_or_else(|e| format!("# The reference is not available\n\n{e}")));
            }
        }
    }

    fn apply_pending(&mut self, ctx: &egui::Context, now: f64) {
        if self.pending.is_none() {
            return;
        }
        let dragging = ctx.dragged_id().is_some() || self.view.gesture.is_some() || self.view.layer_paint.is_some();
        let behind = self.inflight > 0 || self.pending.as_ref().is_some_and(|p| p.version < self.confirmed);
        if dragging || (behind && now - self.inflight_since < 3.0) {
            return;
        }
        if let Some(s) = self.pending.take() {
            self.server = s.clone();
            self.apply_state(s, ctx);
        }
    }

    fn apply_state(&mut self, new: State, ctx: &egui::Context) {
        let first = !self.has_state;
        let render_changed = match (&self.state.render, &new.render) {
            (Some(a), Some(b)) => a.version != b.version || a.url != b.url,
            (None, Some(_)) => true,
            _ => false,
        };
        let render_gone = self.state.render.is_some() && new.render.is_none();
        self.state = new;
        self.has_state = true;
        if let Some(song) = &self.state.song {
            self.view.section = self.view.section.min(song.sections.len().saturating_sub(1));
            self.view.script.sync(&song.script);
            if song.title != self.title {
                self.title = song.title.clone();
                set_title(ctx, &format!("{} · Claudius", song.title));
            }
        }
        if render_gone {
            self.player.clear();
        }
        if render_changed
            && let Some(r) = &self.state.render
            && let Some(src) = self.backend.audio(r)
        {
            self.player.load(src, !first);
        }
    }

    /// sends this frame's commands, showing simple edits at once
    fn flush(&mut self, now: f64) {
        for cmd in std::mem::take(&mut self.out) {
            if let Some(song) = self.state.song.as_mut() {
                let _ = edits::apply(song, &cmd);
            }
            if matches!(cmd, Command::NewSong { .. } | Command::Remake { .. } | Command::Load { .. }) {
                self.view.section = 0;
                self.view.script.dirty = false;
            }
            if matches!(cmd, Command::Analyze { .. } | Command::Remake { .. }) {
                self.view.analyzing = true;
            }
            self.inflight += 1;
            self.inflight_since = now;
            log::debug!("send {cmd:?}");
            self.backend.send(&cmd);
        }
    }

    fn update_loop(&mut self) {
        let region = if !self.view.looping {
            None
        } else if self.view.loop_section {
            let bpm = self
                .state
                .render
                .as_ref()
                .map(|r| r.bpm)
                .or(self.state.song.as_ref().map(|s| s.bpm))
                .unwrap_or(120.0)
                .max(1.0);
            self.state.song.as_ref().and_then(|s| s.sections.get(self.view.section)).map(|s| {
                let named = self
                    .state
                    .render
                    .as_ref()
                    .and_then(|r| r.sections.iter().find(|(n, _, _)| *n == s.name))
                    .map(|(_, a, b)| (*a, *b));
                named.unwrap_or((s.start_bar as f64 * 240.0 / bpm, (s.start_bar + s.bars) as f64 * 240.0 / bpm))
            })
        } else {
            Some((0.0, 0.0))
        };
        if region != self.loop_region {
            self.loop_region = region;
            self.player.set_loop(region);
            if let Some((a, b)) = region
                && b > a
            {
                let p = self.player.position();
                if p < a || p >= b {
                    self.player.seek(a);
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() || self.view.new_song.open {
            return;
        }
        let (space, redo, undo, home, l, left, right) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Space),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::NONE, Key::Home),
                i.consume_key(Modifiers::NONE, Key::L),
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::ArrowRight),
            )
        });
        if space && self.player.has_audio() {
            if self.player.is_playing() { self.player.pause() } else { self.player.play() }
        }
        if redo && self.state.can_redo {
            self.out.push(Command::Redo);
        } else if undo && self.state.can_undo {
            self.out.push(Command::Undo);
        }
        if home {
            self.player.seek(0.0);
        }
        if l {
            self.view.looping = !self.view.looping;
        }
        let n = self.state.song.as_ref().map_or(0, |s| s.sections.len());
        if left && self.view.section > 0 {
            self.view.section -= 1;
        }
        if right && self.view.section + 1 < n {
            self.view.section += 1;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_plan(&mut self, ctx: &egui::Context, now: f64) {
        use crate::devplan::Step;
        let Some(plan) = self.plan.as_mut() else { return };
        while let Some(step) = plan.next(ctx, now) {
            match step {
                Step::Tab(t) => {
                    if let Some(tab) = Tab::ALL.iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&t)) {
                        self.view.tab = *tab;
                    }
                }
                Step::Section(i) => self.view.section = i,
                Step::Play => self.player.play(),
                Step::Pause => self.player.pause(),
                Step::Seek(s) => self.player.seek(s),
                Step::NewSong => self.view.new_song.open = true,
                Step::Close => self.view.new_song.open = false,
                Step::Send(c) => self.out.push(c),
                Step::Size(w, h) => ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(w, h))),
                Step::Script(code) => {
                    self.view.script.text = code;
                    self.view.script.dirty = true;
                }
                Step::Run => {
                    self.view.script.awaiting = true;
                    self.view.script.result = None;
                    self.out.push(Command::RunScript { code: self.view.script.text.clone() });
                }
                Step::Reference => self.view.reference_open = true,
                Step::Tour(i) => {
                    self.view.tour = Some(i);
                    self.view.tour_asks = self.native.as_ref().is_some_and(|n| n.statistics().is_none());
                }
                Step::Assistants => self.view.assistants.show(),
                Step::AudioPath(p) => self.view.audio_path = p,
                Step::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    fn reference_window(&mut self, ctx: &egui::Context) {
        let mut open = self.view.reference_open;
        egui::Window::new("Script reference")
            .open(&mut open)
            .default_size([620.0, 680.0])
            .default_pos([ctx.content_rect().right() - 700.0, 90.0])
            .show(ctx, |ui| match &self.view.reference {
                Some(text) => {
                    egui::ScrollArea::vertical()
                        .id_salt("reference")
                        .auto_shrink([false, false])
                        .show(ui, |ui| panels::markdown::show(ui, text));
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().color(ACCENT));
                        ui.label(RichText::new("loading /api/reference...").color(TEXT_DIM));
                    });
                }
            });
        self.view.reference_open = open;
    }

    fn banner(&mut self, ui: &mut Ui) {
        let error = self.link.error.clone().unwrap_or_default();
        let retry = self.link.retry_in;
        let frame = egui::Frame::new()
            .fill(theme::mix(PANEL, theme::BAD, 0.22))
            .inner_margin(Margin::symmetric(14, 6))
            .stroke(Stroke::new(1.0, theme::alpha(theme::BAD, 0.6)));
        egui::Panel::top("offline").frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::dot(ui, theme::BAD);
                let what = if self.link.ever { "Lost the connection to" } else { "Cannot reach" };
                ui.label(
                    RichText::new(format!("{what} the claudius server at {}", self.link.label)).strong().color(TEXT),
                );
                ui.label(RichText::new(short_error(&error)).color(TEXT_DIM)).on_hover_text(&error);
                if let Some(t) = retry {
                    ui.label(RichText::new(format!("retrying in {:.0} s", t.ceil())).color(TEXT_FAINT));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Use the demo").on_hover_text("play with a built-in mock song instead").clicked() {
                        self.actions.push(Action::UseDemo);
                    }
                    if ui.button("Retry now").clicked() {
                        self.actions.push(Action::Retry);
                    }
                });
            });
        });
    }

    fn welcome(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.22).max(20.0));
            ui.label(RichText::new("Claudius").size(30.0).color(TEXT));
            ui.add_space(4.0);
            if self.link.mock {
                ui.label(RichText::new("starting the demo...").color(TEXT_DIM));
                return;
            }
            if self.link.error.is_none() {
                ui.label(RichText::new(format!("connecting to {}...", self.link.label)).color(TEXT_DIM));
                ui.add(egui::Spinner::new().size(22.0).color(ACCENT));
                return;
            }
            ui.label(RichText::new(format!("Cannot reach the claudius server at {}", self.link.label)).size(16.0).color(theme::mix(theme::BAD, TEXT, 0.4)));
            ui.add_space(6.0);
            ui.label(RichText::new("Start it with   claudius serve   (in a checkout: cargo run --release -p claudius-cli -- serve), or set CLAUDIUS_SERVER to its address.").color(TEXT_DIM));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let w = 290.0;
                ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
                if widgets::tool_button(ui, Icon::Loop, "Retry now", true, false).clicked() {
                    self.actions.push(Action::Retry);
                }
                if widgets::tool_button(ui, Icon::Play, "Open the demo instead", true, true).clicked() {
                    self.actions.push(Action::UseDemo);
                }
            });
        });
    }

    /// the banner for a newer release: ready to install, or out and updated another way
    fn update_bar(&mut self, ui: &mut Ui, ctx: &egui::Context, native: &Arc<dyn Native>, now: f64) {
        let update = native.update();
        if self.view.update_asked {
            match &update {
                Update::Current => {
                    self.view.update_asked = false;
                    self.toast(format!("Claudius {} is the latest", env!("CARGO_PKG_VERSION")), true, now);
                }
                Update::Failed(e) => {
                    self.view.update_asked = false;
                    self.toast(format!("could not look for updates: {e}"), false, now);
                }
                Update::Ready { .. } | Update::Elsewhere { .. } => {
                    self.view.update_asked = false;
                    self.view.update_dismissed = None;
                }
                Update::None | Update::Downloading { .. } => {}
            }
        }
        let (version, text) = match &update {
            Update::Ready { version } => (version.clone(), format!("Claudius {version} is ready to install.")),
            Update::Elsewhere { version, how } => (version.clone(), format!("Claudius {version} is out: {how}.")),
            _ => return,
        };
        if self.view.update_dismissed.as_deref() == Some(version.as_str()) {
            return;
        }
        let bar = egui::Frame::new().fill(theme::mix(PANEL, ACCENT, 0.14)).inner_margin(Margin::symmetric(12, 5));
        egui::Panel::top("update").frame(bar).show(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::dot(ui, ACCENT);
                ui.label(RichText::new(text).color(TEXT));
                if matches!(update, Update::Ready { .. })
                    && widgets::tool_button(ui, Icon::Loop, "Restart to update", true, true).clicked()
                {
                    match native.install_update() {
                        Ok(v) => {
                            self.toast(format!("updating to Claudius {v}"), true, now);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(e) => self.toast(format!("could not update: {e}"), false, now),
                    }
                }
                ui.hyperlink_to("What's new", format!("{RELEASES}/tag/v{version}"));
                if ui.small_button("Later").clicked() {
                    self.view.update_dismissed = Some(version.clone());
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let (dot, text) = if self.link.mock {
                (theme::WARN, "demo mode: a built-in mock plays the server".to_string())
            } else if self.link.online {
                (theme::GOOD, format!("connected to {}", self.link.label))
            } else {
                (theme::BAD, format!("offline: {}", self.link.label))
            };
            widgets::dot(ui, dot);
            ui.label(RichText::new(text).size(11.0).color(TEXT_DIM));
            if let Some(server) = self.link.server.as_ref().filter(|_| self.link.online) {
                let ears = match self.link.ears {
                    _ if self.server.ears_loading => ", ears loading",
                    Some(true) => ", ears on",
                    Some(false) => ", ears off",
                    None => "",
                };
                ui.label(RichText::new(format!("· {server}{ears}")).size(11.0).color(TEXT_FAINT));
            }
            if let Connection::Server(url) = &self.home
                && self.link.mock
                && ui.small_button(format!("connect to {url}")).clicked()
            {
                self.actions.push(Action::Connect);
            }
            if self.has_state {
                ui.label(RichText::new(format!("· state v{}", self.state.version)).size(11.0).color(TEXT_FAINT));
            }
            if let Some(Update::Downloading { version, progress }) = self.native.as_ref().map(|n| n.update()) {
                let text = format!("· downloading Claudius {version}, {:.0}%", progress * 100.0);
                ui.label(RichText::new(text).size(11.0).color(TEXT_FAINT));
            }
            if let Some(e) = self.player.error() {
                ui.label(RichText::new(format!("· audio: {e}")).size(11.0).color(theme::WARN));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button(RichText::new(" ? ").size(12.0).color(TEXT_DIM), |ui| {
                    if ui.button("Take the tour").clicked() {
                        self.start_tour(0);
                        ui.close();
                    }
                    if ui.button("Connect an assistant…").clicked() {
                        self.view.assistants.show();
                        ui.close();
                    }
                    if let Some(native) = &self.native {
                        if ui.button("Check for updates").clicked() {
                            native.check_for_updates();
                            self.view.update_asked = true;
                            ui.close();
                        }
                        let mut auto = native.automatic_updates();
                        let r = ui.checkbox(&mut auto, "Update automatically").on_hover_text(UPDATES_TIP);
                        if r.changed() {
                            let _ = native.set_automatic_updates(auto);
                        }
                    }
                    if let Some(native) = &self.native {
                        let mut on = native.statistics() == Some(true);
                        let r = ui.checkbox(&mut on, "Send anonymous usage statistics").on_hover_text(STATISTICS_TIP);
                        if r.changed() {
                            let _ = native.set_statistics(on);
                        }
                    }
                    ui.hyperlink_to("Documentation", DOCS);
                    ui.hyperlink_to("Report a problem", REPORT);
                })
                .response
                .on_hover_text("help: the tour, connecting an assistant, the documentation, reporting a problem");
                ui.add_space(6.0);
                let r =
                    ui.add(egui::Slider::new(&mut self.view.volume, 0.0..=1.0).show_value(false).trailing_fill(true));
                if r.changed() {
                    self.player.set_volume(self.view.volume);
                }
                ui.label(RichText::new("volume").size(11.0).color(TEXT_FAINT));
                let cmd = widgets::cmd_key(ui.ctx());
                ui.label(
                    RichText::new(format!(
                        "Space play · arrows pick a section · L loop · {cmd}+Z undo · Shift+{cmd}+Z redo"
                    ))
                    .size(11.0)
                    .color(TEXT_FAINT),
                );
            });
        });
    }

    fn dock(&mut self, ui: &mut Ui, now: f64) {
        let tabs = egui::Frame::new().fill(theme::mix(PANEL, BG, 0.4)).inner_margin(Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 0,
        });
        tabs.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for tab in Tab::ALL {
                    let active = self.view.tab == tab;
                    let galley = ui.painter().layout_no_wrap(tab.label().to_string(), theme::prop(12.5), TEXT);
                    let (rect, r) = ui.allocate_exact_size(vec2(galley.size().x + 28.0, 30.0), Sense::click());
                    let fill = if active {
                        PANEL
                    } else if r.hovered() {
                        theme::mix(PANEL, BG, 0.15)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, fill);
                    if active {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)),
                            1.0,
                            ACCENT,
                        );
                    }
                    let c = if active {
                        TEXT
                    } else if r.hovered() {
                        theme::mix(TEXT_DIM, TEXT, 0.5)
                    } else {
                        TEXT_DIM
                    };
                    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, c);
                    if r.clicked() {
                        self.view.tab = tab;
                    }
                }
            });
        });
        let mut cx = Cx {
            state: &mut self.state,
            view: &mut self.view,
            player: &mut self.player,
            out: &mut self.out,
            sounds: self.sounds.as_ref(),
            server: self.server_url.as_deref(),
            now,
        };
        match cx.view.tab {
            Tab::Sequencer => panels::sequencer::show(ui, &mut cx),
            Tab::Mixer => panels::mixer::show(ui, &mut cx),
            Tab::Parts => panels::parts::show(ui, &mut cx),
            Tab::Harmony => panels::harmony::show(ui, &mut cx),
            Tab::Script => panels::script::show(ui, &mut cx),
            Tab::Sounds => panels::library::show(ui, &mut cx),
            Tab::Log => panels::log::show(ui, &mut cx),
        }
    }
}

/// commands the studio shows before the server confirms them
fn is_edit(what: &str) -> bool {
    matches!(
        what,
        "set_tempo"
            | "set_key"
            | "set_swing"
            | "set_progression"
            | "set_section"
            | "add_section"
            | "remove_section"
            | "move_section"
            | "toggle_layer"
            | "set_drum_step"
            | "set_drum_pattern"
            | "set_part"
            | "set_mix"
    )
}

fn short_error(e: &str) -> String {
    let lower = e.to_lowercase();
    if lower.contains("refused") {
        "(connection refused)".into()
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "(timed out)".into()
    } else if lower.contains("dns") || lower.contains("resolve") {
        "(unknown host)".into()
    } else if e.is_empty() {
        String::new()
    } else {
        let s: String = e.chars().take(90).collect();
        format!("({s})")
    }
}

fn set_title(ctx: &egui::Context, title: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_string()));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.set_title(title);
        }
    }
}

impl eframe::App for Studio {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        for ev in self.backend.update(now) {
            self.handle(ev, now);
        }
        self.link.retry_in = self.backend.retry_in(now);
        self.player.tick();
        self.apply_pending(ctx, now);
        self.update_loop();
        // keeps retries and mock jobs going while the window is hidden too
        if let Some(s) = self.backend.next_wake(now) {
            ctx.request_repaint_after(Duration::from_secs_f64(s));
        }
        // a scripted run must not stall behind other windows (macOS stops painting occluded ones)
        #[cfg(not(target_arch = "wasm32"))]
        if self.plan.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.view.anchors = Default::default();
        self.shortcuts(&ctx);
        #[cfg(not(target_arch = "wasm32"))]
        self.run_plan(&ctx, now);

        let bar = egui::Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 0));
        {
            let mut cx = Cx {
                state: &mut self.state,
                view: &mut self.view,
                player: &mut self.player,
                out: &mut self.out,
                sounds: self.sounds.as_ref(),
                server: self.server_url.as_deref(),
                now,
            };
            egui::Panel::top("transport")
                .frame(bar)
                .exact_size(58.0)
                .show_separator_line(false)
                .show(ui, |ui| panels::transport::top_row(ui, &mut cx));
            let tools = egui::Frame::new()
                .fill(theme::mix(PANEL, BG, 0.35))
                .inner_margin(Margin::symmetric(12, 0))
                .stroke(Stroke::new(1.0, LINE_SOFT));
            egui::Panel::top("toolbar")
                .frame(tools)
                .exact_size(40.0)
                .show(ui, |ui| panels::transport::toolbar(ui, &mut cx));
        }
        if let Some(native) = self.native.clone() {
            self.update_bar(ui, &ctx, &native, now);
        }
        if self.fallback_to_demo && !self.link.ever && !self.link.mock && self.link.error.is_some() {
            self.fallback_to_demo = false;
            self.actions.push(Action::UseDemo);
        } else if !self.link.mock && !self.link.online && self.link.error.is_some() {
            self.banner(ui);
        }
        let status = egui::Frame::new()
            .fill(theme::mix(PANEL, BG, 0.5))
            .inner_margin(Margin::symmetric(12, 0))
            .stroke(Stroke::new(1.0, LINE_SOFT));
        egui::Panel::bottom("status").frame(status).exact_size(24.0).show(ui, |ui| self.status_bar(ui));

        if self.state.song.is_some() {
            let side =
                egui::Frame::new().fill(PANEL).inner_margin(Margin::same(10)).stroke(Stroke::new(1.0, LINE_SOFT));
            let shown = egui::Panel::right("side")
                .frame(side)
                .resizable(true)
                .default_size(350.0)
                .size_range(290.0..=600.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("side_scroll").auto_shrink([false, false]).show(ui, |ui| {
                        let mut cx = Cx {
                            state: &mut self.state,
                            view: &mut self.view,
                            player: &mut self.player,
                            out: &mut self.out,
                            sounds: self.sounds.as_ref(),
                            server: self.server_url.as_deref(),
                            now,
                        };
                        panels::inspector::show(ui, &mut cx);
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);
                        panels::verdict::show(ui, &mut cx);
                    });
                });
            self.view.anchors.side = Some(shown.response.rect);
            let dock = egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE));
            let dock_size = (ui.available_height() * 0.38).clamp(200.0, 360.0);
            let shown = egui::Panel::bottom("dock")
                .frame(dock)
                .resizable(true)
                .default_size(dock_size)
                .size_range(180.0..=800.0)
                .show(ui, |ui| self.dock(ui, now));
            self.view.anchors.editors = Some(shown.response.rect);
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ui, |ui| {
            if self.state.song.is_some() {
                let mut cx = Cx {
                    state: &mut self.state,
                    view: &mut self.view,
                    player: &mut self.player,
                    out: &mut self.out,
                    sounds: self.sounds.as_ref(),
                    server: self.server_url.as_deref(),
                    now,
                };
                panels::arrange::show(ui, &mut cx);
            } else if self.has_state {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.3);
                    ui.label(RichText::new("No song yet").size(22.0).color(TEXT));
                    ui.label(
                        RichText::new("Make one: pick a genre and claudius writes, arranges and mixes it.")
                            .color(TEXT_DIM),
                    );
                    ui.add_space(10.0);
                    if widgets::tool_button(ui, Icon::New, "New song", true, true).clicked() {
                        self.view.new_song.open = true;
                    }
                    ui.add_space(6.0);
                    if widgets::tool_button(ui, Icon::Plus, "Connect an assistant", true, false).clicked() {
                        self.view.assistants.show();
                    }
                });
            } else {
                self.welcome(ui);
            }
        });
        {
            let mut cx = Cx {
                state: &mut self.state,
                view: &mut self.view,
                player: &mut self.player,
                out: &mut self.out,
                sounds: self.sounds.as_ref(),
                server: self.server_url.as_deref(),
                now,
            };
            panels::transport::new_song_dialog(&ctx, &mut cx);
        }
        panels::assistants::dialog(&ctx, &mut self.view.assistants, self.native.as_ref());
        // the first time, once the studio is up; a scripted run starts it only when asked
        #[cfg(not(target_arch = "wasm32"))]
        let scripted = self.plan.is_some();
        #[cfg(target_arch = "wasm32")]
        let scripted = false;
        if self.has_state && !self.view.tour_done && self.view.tour.is_none() && !scripted {
            self.start_tour(0);
        }
        let done = self.view.tour_done;
        crate::tour::show(&ctx, &mut self.view, self.native.as_ref());
        // remembered at once: eframe saves only every 30 s on the web, and a skipped tour should stay skipped
        if self.view.tour_done
            && !done
            && let Some(storage) = frame.storage_mut()
        {
            self.save(storage);
            storage.flush();
        }

        if self.view.reference_open {
            self.reference_window(&ctx);
        }
        if self.view.reference_open && self.view.reference.is_none() && !self.view.reference_loading {
            self.view.reference_loading = true;
            self.backend.fetch_reference();
        }

        for action in std::mem::take(&mut self.actions) {
            match action {
                Action::Retry => self.backend.retry(),
                Action::UseDemo => self.switch(Connection::Mock, &ctx),
                Action::Connect => self.switch(self.home.clone(), &ctx),
            }
        }
        self.flush(now);

        let mut wake: Option<f64> = None;
        let mut soon = |s: f64| wake = Some(wake.map_or(s, |w: f64| w.min(s)));
        if self.player.is_playing() {
            soon(1.0 / 40.0);
        }
        if self.player.is_loading() || self.pending.is_some() {
            soon(0.08);
        }
        if let Some(s) = self.backend.next_wake(now) {
            soon(s);
        }
        if let Some(t) = &self.view.toast
            && now - t.at < 6.0
        {
            soon((6.0 - (now - t.at)).max(0.05));
        }
        // the updater works on its own thread: look again while it is busy, or just after the start
        match self.native.as_ref().map(|n| n.update()) {
            Some(Update::Downloading { .. }) => soon(0.5),
            Some(Update::None) if self.view.update_asked || now < 120.0 => soon(2.0),
            _ => {}
        }
        if let Some(w) = wake {
            ctx.request_repaint_after(Duration::from_secs_f64(w));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let v = &self.view;
        let prefs = Prefs {
            tab: v.tab,
            volume: v.volume,
            stems: v.stems,
            mutate_count: v.mutate_count,
            keep_best: v.keep_best,
            generations: v.generations,
            children: v.children,
            sevenths: v.sevenths,
            tour_done: v.tour_done,
        };
        eframe::set_value(storage, eframe::APP_KEY, &prefs);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(plan) = self.plan.as_mut() {
            plan.feed(raw_input);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }

    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(&mut *self)
    }
}
