//! "Play from a phone": claudius serves a page with pads, keys and a turntable on this computer's network, with a key
//! of its own, and the window shows its address as a QR code to scan. While the page is served, the studio tells
//! claudius where the song is as it plays, which the page follows, and what is played on the phone comes back as edits
//! of the song.

use claudius_api::Transport;
use eframe::egui::{self, Color32, Rect, RichText, Sense, Ui, vec2};
use qrcodegen::{QrCode, QrCodeEcc};

use super::{Cx, small};
use crate::theme::{self, TEXT, TEXT_DIM};
use crate::widgets::{self, Icon};

#[derive(Default)]
pub struct Phone {
    pub open: bool,
    /// for the app to do: serve the page (true) or stop it (false)
    pub ask: Option<bool>,
    /// for the app to send: where the song is
    pub report: Option<Transport>,
    /// what was last told: whether it played, where, and when on the studio's clock
    told: Option<(bool, f64, f64)>,
    /// the last of a phone's asks to play or stop that the studio has seen (`claudius_api::PlayAsk::id`)
    played: Option<u64>,
    /// the QR code of the page's address
    code: Option<(String, QrCode)>,
    /// the page is asked for and not served yet
    asking: bool,
}

impl Phone {
    pub fn show(&mut self, served: bool) {
        self.open = true;
        if !served {
            self.serve();
        }
    }

    fn serve(&mut self) {
        self.ask = Some(true);
        self.asking = true;
    }

    /// the page was not served: the window says so, and offers to ask again
    pub fn refused(&mut self) {
        self.asking = false;
    }
}

/// Tells claudius where the song is while a phone's page is served, and shows the window while it is open.
pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    follow(cx);
    tell(cx);
    if !cx.view.phone.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Play from a phone")
        .id(egui::Id::unique("phone"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(WIDTH)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| body(ui, cx));
    if !open {
        cx.view.phone.open = false;
    }
}

/// What a phone asked of the song, done once: played from where it asked (or where it is), or stopped. What was asked
/// before the studio saw the page is not done.
fn follow(cx: &mut Cx) {
    let p = &mut cx.view.phone;
    let Some(view) = cx.state.phone.as_ref() else {
        p.played = None;
        return;
    };
    let ask = view.play.clone();
    let id = ask.as_ref().map_or(0, |a| a.id);
    match p.played {
        None => p.played = Some(id),
        Some(seen) if id > seen => {
            p.played = Some(id);
            let Some(ask) = ask else { return };
            if !ask.playing {
                cx.player.pause();
                return;
            }
            if let Some(at) = ask.at {
                cx.player.seek(at);
            }
            cx.player.play();
        }
        _ => {}
    }
}

/// a word to claudius when the song starts or stops, when it jumps (a seek, a loop), and every few seconds as it plays
fn tell(cx: &mut Cx) {
    let p = &mut cx.view.phone;
    if cx.state.phone.is_none() {
        p.told = None;
        return;
    }
    let (playing, at) = (cx.player.is_playing(), cx.player.position());
    let due = match p.told {
        None => true,
        Some((was, at0, when)) => {
            let expected = if was { at0 + (cx.now - when) } else { at0 };
            was != playing || (expected - at).abs() > 0.05 || (playing && cx.now - when > 5.0)
        }
    };
    if due {
        p.told = Some((playing, at, cx.now));
        p.report = Some(Transport { playing, at });
    }
}

/// the window's width: the QR code and the steps beside it
const WIDTH: f32 = 560.0;

/// the side of the QR code, its quiet zone in it
const CODE: f32 = 200.0;

/// what the phone's page is, on the window's first card
const WHAT: &str = "Your phone as an instrument";
const ABOUT: &str = "pads, keys and a turntable with this song's sounds, that record takes into it";

fn body(ui: &mut Ui, cx: &mut Cx) {
    ui.set_width(WIDTH);
    let Some(phone) = cx.state.phone.clone() else {
        let asking = cx.view.phone.asking;
        widgets::item_card(ui, Icon::Phone, WHAT, ABOUT, |ui| {
            if asking {
                ui.spinner();
            }
        });
        ui.add_space(10.0);
        if asking {
            ui.label(RichText::new("Getting its page ready for your phone…").color(TEXT_DIM));
            return;
        }
        // it stopped: the demo opened again in another tab took the phone, or claudius stopped sharing the page
        let why = if cx.server.is_some() {
            "Phones cannot open this song's page now: it was stopped. Share it again to play along."
        } else {
            "Phones cannot open this song's page now: the demo opened in another tab took it, or it was stopped."
        };
        ui.add(egui::Label::new(RichText::new(why).color(TEXT)).wrap());
        widgets::foot(ui, |ui| {
            if widgets::primary(ui, "Share it with my phone", true).clicked() {
                cx.view.phone.serve();
            }
            if ui.button("Close").clicked() {
                cx.view.phone.open = false;
            }
        });
        return;
    };
    cx.view.phone.asking = false;
    let who = match phone.phones {
        0 => "no phone yet".to_string(),
        1 => "a phone plays along".to_string(),
        n => format!("{n} phones play along"),
    };
    widgets::item_card(ui, Icon::Phone, WHAT, ABOUT, |ui| {
        let color = if phone.phones > 0 { theme::GOOD } else { theme::TEXT_FAINT };
        widgets::pill(ui, &who, color);
    });
    ui.add_space(12.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(CODE);
            qr(ui, &mut cx.view.phone.code, &phone.url);
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(&phone.url).small().color(TEXT_DIM)).selectable(true).wrap());
        });
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("To play along").font(theme::medium(14.0)).color(TEXT));
            ui.add_space(6.0);
            widgets::numbered(ui, 1, "Point your phone's camera at the code, and open the page it finds.");
            let network = if cx.server.is_some() {
                "Keep the phone on the same Wi-Fi as this computer."
            } else {
                "Any network will do: the phone plays along with this page through claudius' site."
            };
            widgets::numbered(ui, 2, network);
            widgets::numbered(
                ui,
                3,
                "Play the song here, and the phone follows it. Tap ● on the phone to record a take, and tap it again to \
                 put the take in the song.",
            );
            ui.add_space(8.0);
            // a MIDI controller plugged into this computer plays the page in a browser here: on this computer's own
            // address, which a browser lets use MIDI, as it does the site
            ui.label(small("A MIDI controller plugged in here (drum pads, keys, a jog wheel)?"));
            ui.hyperlink_to(RichText::new("Open the page on this computer").small(), here(&phone.url));
        });
    });
    // the window's foot: done with it, the page stays open to phones (the phone button stays lit); or no more phones
    widgets::foot(ui, |ui| {
        if widgets::primary(ui, "Done", true).clicked() {
            cx.view.phone.open = false;
        }
        let stop = ui
            .button("Stop sharing")
            .on_hover_text("The phones playing along stop, and this address no longer opens the page.");
        if stop.clicked() {
            cx.view.phone.ask = Some(false);
            cx.view.phone.open = false;
        }
        ui.label(small("Close it, and phones keep playing along."));
    });
}

/// the page's address on this computer: its own (127.0.0.1) for the page a claudius serves on the network
fn here(url: &str) -> String {
    match url.strip_prefix("http://").and_then(|rest| rest.split_once(':')) {
        Some((_, rest)) => format!("http://127.0.0.1:{rest}"),
        None => url.to_string(),
    }
}

/// the QR code of `url`, black on white with its quiet zone, in whole pixels so that no seams show between modules
fn qr(ui: &mut Ui, code: &mut Option<(String, QrCode)>, url: &str) {
    if code.as_ref().is_none_or(|(u, _)| u != url) {
        *code = QrCode::encode_text(url, QrCodeEcc::Medium).ok().map(|c| (url.to_string(), c));
    }
    let Some((_, q)) = code else { return };
    let (n, quiet) = (q.size(), 4);
    let cell = (CODE / (n + 2 * quiet) as f32).floor().max(2.0);
    let side = cell * (n + 2 * quiet) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, Color32::WHITE);
    let origin = rect.min.round() + vec2(quiet as f32 * cell, quiet as f32 * cell);
    for y in 0..n {
        for x in 0..n {
            if q.get_module(x, y) {
                let at = origin + vec2(x as f32 * cell, y as f32 * cell);
                p.rect_filled(Rect::from_min_size(at, vec2(cell, cell)), 0.0, Color32::BLACK);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_page_is_opened_here_on_this_computers_own_address() {
        let lan = "http://192.168.0.126:7879/?key=abc";
        assert_eq!(super::here(lan), "http://127.0.0.1:7879/?key=abc");
        let site = "https://claudius.devlato.com/phone/?room=0f";
        assert_eq!(super::here(site), site, "the demo's page is the site's, here as anywhere");
    }
}
