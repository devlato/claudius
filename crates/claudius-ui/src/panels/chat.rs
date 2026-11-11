//! The chat with the assistants connected to claudius over MCP, in a window of its own: what each side said, in order,
//! and a field to write in while an assistant listens (`wait_for_message`). It opens by itself when an assistant writes,
//! and from the status bar, which says who listens; with no assistant listening, the field says how to have one listen.

use claudius_api::{ChatMessage, Command, State};
use eframe::egui::{self, Key, Modifiers, RichText};

use super::{Cx, small};
use crate::theme::{self, ACCENT, TEXT, TEXT_DIM, TEXT_FAINT};

/// the window, what is being written in it, and the last message seen (none yet in this run)
#[derive(Default)]
pub struct Chat {
    pub open: bool,
    draft: String,
    seen: Option<u64>,
}

impl Chat {
    /// Opens the window when an assistant has written since the last message seen; whether one has.
    pub fn follow(&mut self, state: &State) -> bool {
        // none until the first state has come from claudius: the studio starts with an empty one
        if state.version == 0 {
            return false;
        }
        let last = state.chat.messages.last().map_or(0, |m| m.id);
        let after = unread(self.seen, last);
        let theirs = state.chat.messages.iter().any(|m| m.id > after && m.from.is_some());
        self.open |= theirs;
        self.seen = Some(last);
        theirs
    }
}

/// The id after which the chat's messages are new to the person, from `seen`, the last they saw (none yet in this
/// run), and `last`, the latest now: the chat kept from before this run is not new, and one cleared since starts its
/// ids again.
pub fn unread(seen: Option<u64>, last: u64) -> u64 {
    match seen {
        None => last,
        Some(seen) if seen > last => 0,
        Some(seen) => seen,
    }
}

/// the time now, in ms since 1970, as the state's times are
pub fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    let now = js_sys::Date::now() as u64;
    #[cfg(not(target_arch = "wasm32"))]
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    now
}

/// who listens now, at `now_ms` (ms since 1970)
pub fn listening(state: &State, now_ms: u64) -> Option<&str> {
    let until = state.chat.listening_until?;
    state.chat.listening.as_deref().filter(|_| until > now_ms)
}

/// the status bar's word on the chat: who listens, or that there is one to read; nothing when there is neither
pub fn status(ui: &mut egui::Ui, state: &State, view: &mut super::View, now_ms: u64) {
    let who = listening(state, now_ms);
    let text = match (who, state.chat.messages.is_empty()) {
        (Some(who), _) => format!("· ● {who} is listening"),
        (None, false) => "· chat".to_string(),
        (None, true) => return,
    };
    let color = if who.is_some() { theme::GOOD } else { TEXT_DIM };
    let r = ui.add(egui::Label::new(RichText::new(text).size(11.0).color(color)).sense(egui::Sense::click()));
    if r.on_hover_text("the chat with the assistants connected to claudius").clicked() {
        view.chat.open = !view.chat.open;
    }
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx, now_ms: u64) {
    if !cx.view.chat.open {
        return;
    }
    let mut open = true;
    let who = listening(cx.state, now_ms).map(str::to_string);
    egui::Window::new("Chat")
        .id(egui::Id::unique("chat"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(460.0)
        .default_height(420.0)
        .show(ctx, |ui| {
            match &who {
                Some(w) => ui.label(small(format!("{w} is listening")).color(theme::GOOD)),
                None => ui.label(small(NOBODY).color(TEXT_FAINT)),
            };
            ui.add_space(4.0);
            let messages = cx.state.chat.messages.clone();
            egui::ScrollArea::vertical().id_salt("chat_messages").max_height(300.0).stick_to_bottom(true).show(
                ui,
                |ui| {
                    ui.set_width(ui.available_width());
                    if messages.is_empty() {
                        ui.label(small("nothing said yet").color(TEXT_FAINT));
                    }
                    for m in &messages {
                        message(ui, m);
                    }
                },
            );
            ui.separator();
            let field = egui::TextEdit::multiline(&mut cx.view.chat.draft)
                .id(egui::Id::unique("chat_field"))
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(if who.is_some() { "write to it, Enter sends" } else { "no assistant listens" });
            let r = ui.add_enabled(who.is_some(), field);
            let enter = r.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
            ui.horizontal(|ui| {
                let send =
                    ui.add_enabled(who.is_some() && !cx.view.chat.draft.trim().is_empty(), egui::Button::new("Send"));
                if (send.clicked() || enter) && !cx.view.chat.draft.trim().is_empty() {
                    let text = std::mem::take(&mut cx.view.chat.draft);
                    cx.out.push(Command::ChatSay { text: text.trim().to_string() });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let clear = ui
                        .add_enabled(!messages.is_empty(), egui::Button::new(small("Clear the chat")).frame(false))
                        .on_hover_text("its history goes, here and where claudius keeps it");
                    if clear.clicked() {
                        cx.out.push(Command::ClearChat);
                    }
                });
            });
        });
    if !open {
        cx.view.chat.open = false;
    }
}

/// how to have an assistant listen, when none does
const NOBODY: &str = "No assistant is listening. Ask yours, connected to claudius over MCP, to wait for your messages \
                      (its wait_for_message tool): then write here.";

/// a message: who said it and when, what it answers, and what it says
fn message(ui: &mut egui::Ui, m: &ChatMessage) {
    let (who, color) = match &m.from {
        Some(w) => (w.as_str(), ACCENT),
        None => ("you", theme::INFO),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(who).strong().size(12.0).color(color));
        ui.label(small(clock(m.at)).color(TEXT_FAINT));
        if let Some(r) = m.reply_to {
            ui.label(small(format!("to {r}")).color(TEXT_FAINT));
        }
    });
    ui.add(egui::Label::new(RichText::new(&m.text).color(TEXT)).wrap());
    ui.add_space(6.0);
}

/// a time of day as the chat shows it, in UTC, `14:03`
fn clock(ms: u64) -> String {
    let secs = ms / 1000 % 86_400;
    format!("{:02}:{:02}", secs / 3600, secs / 60 % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claudius_api::ChatView;

    fn state(messages: Vec<ChatMessage>, listening: Option<(&str, u64)>) -> State {
        let chat = ChatView {
            messages,
            listening: listening.map(|(w, _)| w.to_string()),
            listening_until: listening.map(|(_, u)| u),
        };
        State { chat, version: 1, ..Default::default() }
    }

    fn said(id: u64, from: Option<&str>) -> ChatMessage {
        ChatMessage { id, from: from.map(str::to_string), text: "hi".into(), at: 0, reply_to: None }
    }

    #[test]
    fn the_window_opens_when_an_assistant_writes_and_not_for_the_person() {
        let mut chat = Chat::default();
        assert!(!chat.follow(&state(vec![said(1, None)], None)) && !chat.open, "the person's own message");
        assert!(chat.follow(&state(vec![said(1, None), said(2, Some("Claude Code"))], None)) && chat.open);
        chat.open = false;
        assert!(!chat.follow(&state(vec![said(1, None), said(2, Some("Claude Code"))], None)) && !chat.open, "seen");
    }

    #[test]
    fn the_chat_kept_from_before_does_not_open_the_window_and_one_cleared_counts_again() {
        let mut chat = Chat::default();
        assert!(!chat.follow(&State::default()), "the studio's own empty state, before claudius's");
        let kept = vec![said(1, None), said(2, Some("Claude Code"))];
        assert!(!chat.follow(&state(kept, None)) && !chat.open, "kept from before this run");
        assert!(!chat.follow(&state(Vec::new(), None)), "cleared");
        assert!(chat.follow(&state(vec![said(1, Some("Claude Code"))], None)) && chat.open, "its ids start again");
        assert_eq!((unread(None, 7), unread(Some(9), 2), unread(Some(3), 7)), (7, 0, 3));
    }

    #[test]
    fn an_assistant_listens_until_its_time_is_up() {
        let s = state(Vec::new(), Some(("Claude Code", 1_000)));
        assert_eq!((listening(&s, 999), listening(&s, 1_000)), (Some("Claude Code"), None));
        assert_eq!(clock(13 * 3_600_000 + 5 * 60_000), "13:05");
    }
}
