//! "Send feedback": what a person thinks of claudius, in their words, to the people who make it: what works, what does
//! not, what they wish it did, and where to answer them if they want an answer. claudius' version, the system and the
//! song's genre go with it unless they untick them; nothing else does. `feedback/` holds where it goes, and
//! `docs/src/guide/privacy.md` says what is kept.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui::{self, RichText};

use super::Cx;
use crate::theme::{self, TEXT_DIM, TEXT_FAINT};
use crate::widgets;

/// what the privacy page says of feedback
const PRIVACY: &str = "https://claudius.devlato.com/docs/guide/privacy.html#feedback";

/// the system it is sent from, as the endpoint names them
const OS: &str = if cfg!(target_arch = "wasm32") {
    "web"
} else if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "linux"
};

/// the window, while it is open, and a message on its way
#[derive(Default)]
pub struct Feedback {
    pub open: bool,
    text: String,
    contact: String,
    /// claudius' version, the system and the song's genre left out of the message
    leave_out: bool,
    sending: Option<Receiver<Result<(), String>>>,
    said: Option<Result<(), String>>,
}

impl Feedback {
    /// opens the window, what was last said of a message gone
    pub fn show(&mut self) {
        self.open = true;
        self.said = None;
    }

    /// the answer to a message on its way, once it has come: what was written goes once it is sent
    fn poll(&mut self) {
        let Some(rx) = &self.sending else { return };
        let said = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("it stopped halfway".into()),
        };
        if said.is_ok() {
            self.text.clear();
        }
        (self.said, self.sending) = (Some(said), None);
    }
}

/// The message as the endpoint takes it (`claudius_api::feedback_message`), from the studio or the web demo, with
/// claudius' version, the system and the song's genre unless left out (`details`: the genre, when a song is open).
pub fn message(text: &str, contact: &str, details: Option<Option<&str>>, demo: bool) -> serde_json::Value {
    let place = if demo { "demo" } else { "studio" };
    claudius_api::feedback_message(text, place, contact, details.map(|genre| (OS, genre)))
}

/// `m` sent to where feedback goes; its answer comes on the receiver
fn send(ctx: &egui::Context, m: &serde_json::Value) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let mut request = ehttp::Request::post(claudius_api::FEEDBACK_ENDPOINT, m.to_string().into_bytes());
    request.headers = ehttp::Headers::new(&[("Content-Type", "application/json")]);
    let ctx = ctx.clone();
    ehttp::fetch(request, move |res| {
        let said = match res {
            Ok(r) if r.ok => Ok(()),
            Ok(r) if r.status == 429 => Err("too many at once".into()),
            Ok(r) => Err(format!("it was not taken ({})", r.status)),
            Err(_) => Err("no connection".into()),
        };
        let _ = tx.send(said);
        ctx.request_repaint();
    });
    rx
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    let demo = cx.server.is_none();
    let genre = cx.state.song.as_ref().map(|s| s.genre.clone());
    let f = &mut cx.view.feedback;
    f.poll();
    if f.sending.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    if !f.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Send feedback")
        .id(egui::Id::unique("feedback"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "What works, what doesn't, what you wish it did. It goes to the people who make claudius.",
                )
                .color(TEXT_DIM),
            );
            ui.add_space(6.0);
            let field = egui::TextEdit::multiline(&mut f.text)
                .desired_rows(6)
                .desired_width(f32::INFINITY)
                .char_limit(claudius_api::FEEDBACK_MOST)
                .hint_text("Say it as you would to a friend");
            ui.add(field);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Your email, if you'd like an answer");
                ui.add(
                    egui::TextEdit::singleline(&mut f.contact)
                        .char_limit(200)
                        .hint_text("optional")
                        .desired_width(220.0),
                );
            });
            let what = match &genre {
                Some(g) => format!("Send claudius' version and the song's genre ({}) with it", super::genre_words(g)),
                None => "Send claudius' version with it".to_string(),
            };
            let mut with = !f.leave_out;
            if ui.checkbox(&mut with, what).changed() {
                f.leave_out = !with;
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Nothing else goes with it: not your song, its files or your name.")
                        .size(11.5)
                        .color(TEXT_FAINT),
                );
                ui.hyperlink_to(RichText::new("What is kept").size(11.5), PRIVACY);
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let ready = !f.text.trim().is_empty() && f.sending.is_none();
                if widgets::primary(ui, "Send", ready).clicked() {
                    let details = (!f.leave_out).then_some(genre.as_deref());
                    f.sending = Some(send(ui.ctx(), &message(&f.text, &f.contact, details, demo)));
                    f.said = None;
                }
                if ui.button("Close").clicked() {
                    f.open = false;
                }
                if f.sending.is_some() {
                    ui.spinner();
                    ui.label(RichText::new("sending…").color(TEXT_FAINT));
                }
                match &f.said {
                    Some(Ok(())) => {
                        ui.label(RichText::new("Sent. Thank you.").color(theme::GOOD));
                    }
                    Some(Err(why)) => {
                        ui.label(RichText::new(format!("Not sent: {why}. Try again in a moment.")).color(theme::BAD));
                    }
                    None => {}
                }
            });
        });
    if !open {
        f.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_holds_what_was_written_and_the_details_only_when_they_are_left_in() {
        let m = message("  the radio should mix when I skip  ", "", Some(Some("trip_hop")), false);
        assert_eq!(m["text"], "the radio should mix when I skip");
        assert_eq!(
            (m["where"].as_str(), m["song"].as_str(), m["os"].as_str()),
            (Some("studio"), Some("trip_hop"), Some(OS))
        );
        assert_eq!(m["version"], env!("CARGO_PKG_VERSION"));
        assert!(m.get("contact").is_none(), "no address given, none sent");
        let left_out = message("hi", " me@example.com ", None, true);
        assert_eq!((left_out["where"].as_str(), left_out["contact"].as_str()), (Some("demo"), Some("me@example.com")));
        assert!(left_out.get("version").is_none() && left_out.get("os").is_none() && left_out.get("song").is_none());
        // a fusion goes as it is named; a genre of one's own named otherwise stays out, so the message is still taken
        assert_eq!(message("x", "", Some(Some("trip_hop+idm")), false)["song"], "trip_hop+idm");
        assert_eq!(
            message("x", "", Some(Some("trip_hop+neo_soul+dub_techno+idm")), false)["song"],
            "trip_hop+neo_soul+dub_techno+idm"
        );
        assert!(message("x", "", Some(Some("My Genre!")), false).get("song").is_none());
        assert!(message("x", "", Some(None), false).get("song").is_none(), "no song open");
    }
}
