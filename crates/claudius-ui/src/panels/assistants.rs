//! "Connect an assistant": the assistants on this computer and how each has claudius, a button that connects one to
//! this app (what `claudius connect` does), and the command that does the same from a terminal. The web studio
//! cannot change files on the computer, so it shows the commands only.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use eframe::egui::{self, RichText, Ui, vec2};

use super::small;
use crate::native::{ASSISTANTS, Assistant, Connected, Native};
use crate::theme::{self, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets::{self, Icon};

/// what a connection did, in lines, or why it failed
type Answer = Result<Vec<String>, String>;

#[derive(Default)]
pub struct Assistants {
    pub open: bool,
    /// as the app last read them (read again after each connection)
    list: Option<Vec<Assistant>>,
    /// the connection on its way: the assistant's name and its answer to come
    pending: Option<(String, Receiver<Answer>)>,
    /// the last answer: the assistant's name, and what was done or why it failed
    answer: Option<(String, Answer)>,
}

impl Assistants {
    /// opens the dialog, reading the assistants afresh
    pub fn show(&mut self) {
        self.open = true;
        self.list = None;
        self.answer = None;
    }

    fn poll(&mut self) {
        let Some((name, rx)) = &self.pending else { return };
        let answer = match rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("the connection stopped halfway".into()),
        };
        self.answer = Some((name.clone(), answer));
        self.pending = None;
        self.list = None;
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start(&mut self, native: &Arc<dyn Native>, a: &Assistant, ctx: &egui::Context) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (native, id, ctx) = (native.clone(), a.id.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(native.connect(&id));
            ctx.request_repaint();
        });
        self.pending = Some((a.name.clone(), rx));
        self.answer = None;
    }

    #[cfg(target_arch = "wasm32")]
    fn start(&mut self, _native: &Arc<dyn Native>, _a: &Assistant, _ctx: &egui::Context) {}
}

/// the dialog, while it is open
pub fn dialog(ctx: &egui::Context, a: &mut Assistants, native: Option<&Arc<dyn Native>>) {
    if !a.open {
        return;
    }
    let modal = widgets::dialog(ctx, "assistants").show(ctx, |ui| {
        ui.set_width(600.0);
        if widgets::head(ui, "Connect an assistant") {
            a.open = false;
        }
        ui.label(small(
            "Claude, Cursor or Codex can then make and edit songs with you, while you hear every change here.",
        ));
        ui.add_space(8.0);
        list(ui, a, native);
        widgets::foot(ui, |ui| {
            if ui.button("Close").clicked() {
                a.open = false;
            }
        });
    });
    if modal.should_close() {
        a.open = false;
    }
}

/// the assistants and their buttons: the dialog's body, which the tour shows as well
pub fn list(ui: &mut Ui, a: &mut Assistants, native: Option<&Arc<dyn Native>>) {
    a.poll();
    let Some(native) = native else {
        commands(ui);
        return;
    };
    let list = a.list.get_or_insert_with(|| native.assistants()).clone();
    let busy = a.pending.as_ref().map(|(name, _)| name.clone());
    let mut start = None;
    for x in &list {
        let (text, color) = match &x.connected {
            Connected::ThisApp => ("connected to this app".to_string(), theme::GOOD),
            Connected::Other(cmd) => (format!("connected to another claudius ({cmd})"), theme::WARN),
            Connected::Http(url) => (format!("connected over HTTP ({url}), while the studio runs"), TEXT_DIM),
            Connected::No if x.found => ("not connected".to_string(), TEXT_DIM),
            Connected::No => ("not found on this computer".to_string(), TEXT_FAINT),
        };
        card(ui, &x.name, RichText::new(text).size(12.0).color(color), |ui| {
            copy_button(ui, &x.command);
            if busy.as_deref() == Some(x.name.as_str()) {
                ui.add(egui::Spinner::new().size(16.0).color(theme::ACCENT));
                return;
            }
            let label = match x.connected {
                Connected::ThisApp => "Connect again",
                Connected::Other(_) | Connected::Http(_) => "Use this app",
                Connected::No => "Connect",
            };
            // the one to do next, in the accent: an assistant found here and not connected yet
            let next = x.connected == Connected::No && x.found;
            let clicked = if next {
                widgets::primary(ui, label, busy.is_none()).clicked()
            } else {
                ui.add_enabled(busy.is_none(), egui::Button::new(label)).clicked()
            };
            if clicked {
                start = Some(x.clone());
            }
        });
    }
    if let Some(x) = start {
        a.start(native, &x, ui.ctx());
    }
    if let Some((name, answer)) = &a.answer {
        ui.add_space(8.0);
        match answer {
            Ok(lines) => {
                ui.label(RichText::new(format!("{name}:")).color(theme::GOOD).size(12.0));
                for l in lines {
                    ui.label(small(l.trim()));
                }
            }
            Err(e) => {
                ui.label(RichText::new(format!("{name} was not connected: {e}")).color(theme::BAD).size(12.0));
            }
        }
    }
}

/// the web studio's version: the commands, to run in a terminal on the computer claudius runs on
fn commands(ui: &mut Ui) {
    ui.label(small(
        "A page in a browser cannot change the settings of the apps on a computer: run one of these in a terminal \
         on the computer claudius runs on, then restart the assistant.",
    ));
    ui.add_space(6.0);
    for (id, name) in ASSISTANTS {
        let command = format!("claudius connect {id}");
        card(ui, name, RichText::new(&command).monospace().size(12.0).color(TEXT_DIM), |ui| copy_button(ui, &command));
    }
}

/// An assistant as a card of the window: an icon, its name, a line under it (whether and how it has claudius, or the
/// command that connects it) and what `right` puts at its right, as the window's other cards are
fn card(ui: &mut Ui, name: &str, line: RichText, right: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme::RAISED.into()).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (tile, _) = ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
            ui.painter().rect_filled(tile, 6.0, theme::WELL);
            widgets::paint_icon(ui.painter(), tile.shrink(9.0), Icon::Improve, theme::ACCENT);
            ui.vertical(|ui| {
                ui.label(RichText::new(name).color(TEXT).strong().size(13.5));
                ui.add(egui::Label::new(line).truncate());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
        });
    });
    ui.add_space(6.0);
}

fn copy_button(ui: &mut Ui, command: &str) {
    if widgets::icon_button(ui, Icon::Copy, 24.0, false, true, &format!("copy the command: {command}")).clicked() {
        ui.ctx().copy_text(command.to_string());
    }
}
