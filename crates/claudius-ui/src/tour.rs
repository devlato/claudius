//! The tour: a few cards the first time the studio opens, each pointing at a part of it, ending with connecting an
//! assistant and, in the app, the one question about anonymous usage statistics (off unless the answer is yes). It
//! never blocks the studio. Skip (or Esc) ends it for good; the help menu (?) starts it again.

use std::sync::Arc;

use eframe::egui::{self, Align, Align2, Id, LayerId, Layout, Margin, Order, Rect, RichText, Stroke, StrokeKind, vec2};

use crate::native::Native;
use crate::panels::{View, assistants, small};
use crate::theme::{ACCENT, TEXT, TEXT_DIM};
use crate::widgets::{self, Icon};

/// where the parts of the studio the tour points at were drawn this frame
#[derive(Default)]
pub struct Anchors {
    pub new_song: Option<Rect>,
    pub play: Option<Rect>,
    /// the side panel, with the section and the verdict
    pub side: Option<Rect>,
    /// the docked editors
    pub editors: Option<Rect>,
}

struct Step {
    title: &'static str,
    body: &'static str,
}

const STEPS: [Step; 5] = [
    Step {
        title: "Make a song",
        body: "Pick a genre, and claudius writes a whole song: the drums, bass, chords and lead, the arrangement, the \
               mix and the master. The same genre and seed always give the same song.",
    },
    Step {
        title: "Listen",
        body: "Space plays and stops. Every change is rendered again by itself, and the waveform under the arrangement \
               says when a newer render is on its way.",
    },
    Step {
        title: "Read the verdict",
        body: "The critic scores the song out of 100 with its meters, music theory and listening models. Fix first \
               names what costs the most and where it usually comes from. It is a guide: your ears decide.",
    },
    Step {
        title: "Change anything",
        body: "Click a section to edit it, toggle steps in the sequencer, mix in the Mixer, or change the whole song as \
               a script. {cmd}+Z undoes anything.",
    },
    Step {
        title: "Bring an assistant",
        body: "Connect Claude, Cursor or Codex, then ask it for a song in plain words: it makes and edits the song here \
               while you listen.",
    },
];

const STATISTICS: Step = Step {
    title: "Help improve claudius?",
    body: "claudius can send a few anonymous counts, each at most once a day: its version, your system (macOS, Linux or \
           Windows, and the chip), which part you use (the studio, an assistant, a command), and where a setup or a \
           crash failed. Never your songs, files, paths or name, and nothing that tells you apart from anyone else. It \
           stays off unless you say yes, and ? in the bottom right corner changes it later.",
};

/// the card for the tour's step, while there is one and no dialog is in front
pub fn show(ctx: &egui::Context, view: &mut View, native: Option<&Arc<dyn Native>>) {
    let Some(i) = view.tour else { return };
    if view.new_song.open || view.assistants.open {
        return;
    }
    if ctx.input(|inp| inp.key_pressed(egui::Key::Escape)) {
        finish(view);
        return;
    }
    let count = STEPS.len() + usize::from(view.tour_asks && native.is_some());
    let i = i.min(count - 1);
    let last = i + 1 == count;
    let step = STEPS.get(i).unwrap_or(&STATISTICS);
    let anchor = match i {
        0 => view.anchors.new_song,
        1 => view.anchors.play,
        2 => view.anchors.side,
        3 => view.anchors.editors,
        _ => None,
    };
    let screen = ctx.content_rect();
    let (pos, pivot) = match anchor {
        // a panel down the side: the card goes beside it
        Some(a) if a.height() > 240.0 && a.width() < 700.0 => (a.left_top() + vec2(-12.0, 12.0), Align2::RIGHT_TOP),
        Some(a) if a.bottom() + 260.0 < screen.bottom() => (a.left_bottom() + vec2(0.0, 10.0), Align2::LEFT_TOP),
        Some(a) => (a.left_top() - vec2(0.0, 10.0), Align2::LEFT_BOTTOM),
        None => (screen.center(), Align2::CENTER_CENTER),
    };
    if let Some(a) = anchor {
        ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("tour_spot"))).rect_stroke(
            a.expand(4.0),
            6.0,
            Stroke::new(2.0, ACCENT),
            StrokeKind::Outside,
        );
    }
    let cmd = widgets::cmd_key(ctx);
    let body = step.body.replace("{cmd}", cmd);
    let mut go = None;
    egui::Area::new(Id::unique("tour")).order(Order::Foreground).pivot(pivot).fixed_pos(pos).constrain(true).show(
        ctx,
        |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(14)).show(ui, |ui| {
                ui.set_width(if i + 1 >= STEPS.len() { 640.0 } else { 360.0 });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(step.title).size(16.0).color(TEXT).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(small(format!("{} of {count}", i + 1)));
                    });
                });
                ui.add_space(4.0);
                ui.label(RichText::new(body).color(TEXT_DIM));
                if i + 1 == STEPS.len() {
                    ui.add_space(8.0);
                    assistants::list(ui, &mut view.assistants, native);
                    ui.add_space(6.0);
                    ui.label(small("The tour is under ? in the bottom right corner, whenever you want it again."));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if !last && ui.link(RichText::new("Skip the tour").size(12.0).color(TEXT_DIM)).clicked() {
                        go = Some(None);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if i >= STEPS.len() {
                            // two answers, and neither is picked for the person
                            for (label, on) in [("No, thanks", false), ("Send anonymous statistics", true)] {
                                if widgets::tool_button(ui, Icon::Right, label, true, false).clicked() {
                                    if let Some(n) = native {
                                        let _ = n.set_statistics(on);
                                    }
                                    go = Some(None);
                                }
                            }
                        } else if widgets::tool_button(ui, Icon::Right, if last { "Done" } else { "Next" }, true, true)
                            .clicked()
                        {
                            go = Some((!last).then_some(i + 1));
                        }
                        if i > 0 && widgets::tool_button(ui, Icon::Left, "Back", true, false).clicked() {
                            go = Some(Some(i - 1));
                        }
                    });
                });
            });
        },
    );
    match go {
        Some(Some(j)) => view.tour = Some(j),
        Some(None) => finish(view),
        None => {}
    }
}

fn finish(view: &mut View) {
    view.tour = None;
    view.tour_done = true;
}
