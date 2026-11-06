//! The Add a recording window: a song or a sound of the person's own, chosen or dropped on the studio, added to the
//! song whole as a track (a take, which they trim and put effects on as they like) or with its vocals sampled into it.
//! Trimming a take is a window of its own (`Trim`), opened from its track's menu.

use std::sync::Arc;

use eframe::egui;

use super::record::ROLES;
use super::{Cx, Listen};
use crate::pick::{self, Picked, Want};

/// the window, and the recording it holds
#[derive(Default)]
pub struct Import {
    pub open: bool,
    pub file: Option<Arc<Picked>>,
    /// the section it starts with (the song's first when empty), the role it plays and its track's name
    from: String,
    role: String,
    name: String,
    /// the recording on its way to claudius, as a take
    pub upload: Option<super::record::Upload>,
    /// the answer is awaited
    sending: bool,
    /// what claudius said of the last one, and whether it was added
    said: Option<(bool, String)>,
    /// a take being trimmed
    pub trim: Option<Trim>,
    /// the recording is to be added whole now, as its button adds it (a scripted run's `import:whole`)
    pub whole: bool,
}

/// a take's trim, as the window sets it: from where to where in its recording, in seconds
pub struct Trim {
    pub track: String,
    pub seconds: f64,
    pub start: f64,
    pub end: f64,
}

impl Trim {
    pub fn of(track: &str, seconds: f64) -> Trim {
        Trim { track: track.to_string(), seconds, start: 0.0, end: seconds }
    }
}

impl Import {
    /// a recording dropped on the studio or chosen, read, to add
    pub fn holds(&mut self, file: Picked) {
        (self.file, self.open, self.said) = (Some(Arc::new(file)), true, None);
    }

    /// whether a take's answer is this window's
    pub fn waiting(&self) -> bool {
        self.sending
    }

    /// what claudius said of the last recording, for a scripted run to read
    pub fn told(&self) -> String {
        self.said.as_ref().map(|(_, text)| text.clone()).unwrap_or_default()
    }

    /// claudius' answer to the recording sent
    pub fn answered(&mut self, ok: bool, message: String) {
        self.sending = false;
        if ok {
            self.file = None;
            self.name.clear();
        }
        self.said = Some((ok, message));
    }
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    if let Some(got) = pick::chosen(Want::Recording) {
        match got {
            Ok(file) => cx.view.import.holds(file),
            Err(e) => (cx.view.import.open, cx.view.import.said) = (true, Some((false, e))),
        }
    }
    trim(ctx, cx);
    if !cx.view.publish.open && ctx.input(|i| !i.raw.hovered_files.is_empty()) {
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop-recording"));
        let painter = ctx.layer_painter(layer);
        let rect = ctx.content_rect();
        painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(170));
        let words = "Drop it to add the recording to the song";
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            words,
            egui::FontId::proportional(22.0),
            egui::Color32::WHITE,
        );
    }
    if !cx.view.import.open {
        return;
    }
    let sections: Vec<String> =
        cx.state.song.as_ref().map(|s| s.sections.iter().map(|s| s.name.clone()).collect()).unwrap_or_default();
    let mut open = true;
    egui::Window::new("Add a recording").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        let im = &mut cx.view.import;
        ui.horizontal(|ui| {
            let label = if im.file.is_some() { "Choose another..." } else { "Choose a file..." };
            if ui.button(label).on_hover_text("WAV, MP3, FLAC, AAC, AIFF, Ogg or Opus").clicked() {
                pick::choose(ui.ctx(), Want::Recording);
            }
            ui.weak("or drop one on the studio");
        });
        let Some(file) = im.file.clone() else {
            ui.label(
                "A song or a sound of your own: added whole as a track, which you trim and put effects on, or \
                      with its vocals sampled into the song.",
            );
            said(ui, &im.said);
            return;
        };
        ui.label(
            egui::RichText::new(format!("{} ({:.1} MB)", file.name, file.bytes.len() as f64 / 1_048_576.0)).strong(),
        );
        if sections.is_empty() {
            ui.label("Start a song first: the recording goes into it.");
            return;
        }
        if !sections.contains(&im.from) {
            im.from = sections[0].clone();
        }
        if im.role.is_empty() {
            im.role = "vocal".into();
        }
        egui::Grid::new("import-take").num_columns(2).show(ui, |ui| {
            ui.label("starts with");
            egui::ComboBox::from_id_salt("import-from").selected_text(im.from.as_str()).show_ui(ui, |ui| {
                for s in &sections {
                    ui.selectable_value(&mut im.from, s.clone(), s);
                }
            });
            ui.end_row();
            ui.label("plays as");
            egui::ComboBox::from_id_salt("import-role").selected_text(im.role.as_str()).show_ui(ui, |ui| {
                for r in ROLES {
                    ui.selectable_value(&mut im.role, r.to_string(), *r);
                }
            });
            ui.end_row();
            ui.label("track");
            ui.add(egui::TextEdit::singleline(&mut im.name).hint_text("take1").desired_width(140.0));
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let whole = ui
                .add_enabled(!im.sending, egui::Button::new("Add it whole as a track"))
                .on_hover_text("it plays once from the start of the section, on for as long as it lasts");
            if whole.clicked() || (std::mem::take(&mut im.whole) && !im.sending) {
                let name = Some(im.name.trim().to_string()).filter(|n| !n.is_empty());
                im.upload = Some(super::record::Upload {
                    wav: file.bytes.clone(),
                    from: im.from.clone(),
                    name,
                    role: im.role.clone(),
                    request: None,
                });
                (im.sending, im.said) = (true, None);
            }
            let sample =
                ui.add_enabled(!im.sending, egui::Button::new("Sample its vocals into the song")).on_hover_text(
                    "the auto-sampler: its best sung phrases, cleaned of its drums, in the sections that suit them",
                );
            if sample.clicked() {
                cx.view.listen = Some(Listen { file, remake: None, flip: false, sample: true, groove: false });
                im.open = false;
            }
        });
        let im = &cx.view.import;
        if im.sending {
            ui.weak("adding it ...");
        }
        said(ui, &im.said);
        ui.weak("Trim it from its track's menu in the arrangement; effects go on it in the mixer, as on any track.");
    });
    if !open {
        cx.view.import.open = false;
    }
}

/// what claudius said of the last recording
fn said(ui: &mut egui::Ui, said: &Option<(bool, String)>) {
    if let Some((ok, text)) = said {
        let color = if *ok { ui.visuals().text_color() } else { ui.visuals().error_fg_color };
        ui.label(egui::RichText::new(text).color(color));
    }
}

/// the Trim window, while a take is being trimmed
fn trim(ctx: &egui::Context, cx: &mut Cx) {
    let Some(t) = cx.view.import.trim.as_mut() else { return };
    let mut open = true;
    let mut done = false;
    egui::Window::new(format!("Trim {}", t.track)).open(&mut open).resizable(false).collapsible(false).show(
        ctx,
        |ui| {
            ui.label(format!("{} is {:.2} s of its recording. Keep:", t.track, t.seconds));
            egui::Grid::new("trim-take").num_columns(2).show(ui, |ui| {
                ui.label("from");
                ui.add(egui::Slider::new(&mut t.start, 0.0..=t.seconds).suffix(" s").max_decimals(2));
                ui.end_row();
                ui.label("to");
                ui.add(egui::Slider::new(&mut t.end, 0.0..=t.seconds).suffix(" s").max_decimals(2));
                ui.end_row();
            });
            t.end = t.end.max(t.start);
            ui.weak("It still plays from the start of its section; undo brings the whole of it back.");
            let keeps = t.end - t.start >= 0.1 && (t.start > 0.0 || t.end < t.seconds);
            if ui.add_enabled(keeps, egui::Button::new("Trim")).clicked() {
                let end = (t.end < t.seconds).then_some(t.end);
                cx.out.push(claudius_api::Command::TrimTake { track: t.track.clone(), start: t.start, end });
                done = true;
            }
        },
    );
    if done || !open {
        cx.view.import.trim = None;
    }
}
