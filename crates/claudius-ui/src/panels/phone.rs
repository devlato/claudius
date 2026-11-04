//! "Play from a phone": claudius serves a page with pads, keys and a turntable on this computer's network, with a key
//! of its own, and the window shows its address as a QR code to scan. While the page is served, the studio tells
//! claudius where the song is as it plays, which the page follows, and what is played on the phone comes back as edits
//! of the song.

use claudius_api::Transport;
use eframe::egui::{self, Color32, Rect, RichText, Sense, Ui, vec2};
use qrcodegen::{QrCode, QrCodeEcc};

use super::{Cx, small};
use crate::theme::{TEXT, TEXT_DIM};

#[derive(Default)]
pub struct Phone {
    pub open: bool,
    /// for the app to do: serve the page (true) or stop it (false)
    pub ask: Option<bool>,
    /// for the app to send: where the song is
    pub report: Option<Transport>,
    /// what was last told: whether it played, where, and when on the studio's clock
    told: Option<(bool, f64, f64)>,
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
        .default_width(340.0)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| body(ui, cx));
    if !open {
        cx.view.phone.open = false;
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

fn body(ui: &mut Ui, cx: &mut Cx) {
    let Some(phone) = cx.state.phone.clone() else {
        if cx.view.phone.asking {
            ui.label(RichText::new("Asking claudius for the phone's page…").color(TEXT));
        } else {
            // it stopped: the demo opened again in another tab took the phone, or claudius stopped serving the page
            ui.label(RichText::new("The phone's page is not served here now.").color(TEXT));
            ui.add_space(6.0);
            if ui.button("Serve it here").clicked() {
                cx.view.phone.serve();
            }
        }
        return;
    };
    cx.view.phone.asking = false;
    let how = if cx.server.is_some() {
        "Scan this with your phone's camera. The phone has to be on the same Wi-Fi as this computer."
    } else {
        "Scan this with your phone's camera. It plays along with this page through claudius' site, on any network."
    };
    ui.label(RichText::new(how).color(TEXT));
    ui.add_space(10.0);
    ui.vertical_centered(|ui| qr(ui, &mut cx.view.phone.code, &phone.url));
    ui.add_space(8.0);
    ui.add(egui::Label::new(RichText::new(&phone.url).small().color(TEXT_DIM)).selectable(true));
    ui.add_space(4.0);
    // a MIDI controller plugged into this computer plays the page in a browser here: on this computer's own address,
    // which a browser lets use MIDI, as it does the site
    ui.horizontal(|ui| {
        ui.hyperlink_to(RichText::new("Open it on this computer").small(), here(&phone.url));
        ui.label(small("for a MIDI controller plugged in here: drum pads, keys, a jog wheel"));
    });
    ui.add_space(8.0);
    let who = match phone.phones {
        0 => "No phone has it open yet.".to_string(),
        1 => "A phone is playing along.".to_string(),
        n => format!("{n} phones are playing along."),
    };
    ui.label(RichText::new(who).color(TEXT));
    ui.label(small(
        "It has pads, keys and a turntable with this song's sounds. Play the song here, tap ● on the phone to record a \
         take, and tap it again to put the take in the song.",
    ));
    ui.add_space(8.0);
    if ui.button("Stop serving the page").clicked() {
        cx.view.phone.ask = Some(false);
        cx.view.phone.open = false;
    }
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
    let cell = (220.0 / (n + 2 * quiet) as f32).floor().max(2.0);
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
