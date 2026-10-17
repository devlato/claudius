//! The engine's log (`State.log`, oldest first), filterable, errors and warnings coloured.

use eframe::egui::{self, RichText, Ui};

use super::{Cx, page_frame, small};
use crate::theme::{self, TEXT, TEXT_FAINT};
use crate::widgets;

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    page_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::heading(ui, "Log");
            ui.add(egui::TextEdit::singleline(&mut cx.view.log_filter).hint_text("filter").desired_width(220.0));
            ui.label(small(format!("{} messages", cx.state.log.len())));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("copy all").clicked() {
                    ui.ctx().copy_text(cx.state.log.join("\n"));
                }
            });
        });
        ui.add_space(4.0);
        if let Some(song) = &cx.state.song {
            egui::Panel::right("history")
                .resizable(true)
                .default_size(300.0)
                .size_range(180.0..=520.0)
                .frame(egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 0, top: 0, bottom: 0 }))
                .show(ui, |ui| {
                    widgets::heading(ui, "Song history");
                    ui.label(small(format!("what was done to '{}', oldest first", song.title)));
                    egui::ScrollArea::vertical()
                        .id_salt("lineage")
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for (i, step) in song.lineage.iter().enumerate() {
                                ui.horizontal_top(|ui| {
                                    ui.label(
                                        RichText::new(format!("{:>2}", i + 1)).monospace().size(11.0).color(TEXT_FAINT),
                                    );
                                    ui.add(egui::Label::new(RichText::new(step).size(12.0).color(TEXT)).wrap());
                                });
                            }
                            if song.lineage.is_empty() {
                                ui.label(RichText::new("nothing yet").color(TEXT_FAINT));
                            }
                        });
                });
        }
        egui::Frame::new().fill(theme::WELL).corner_radius(6).inner_margin(8).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("log").auto_shrink([false, false]).stick_to_bottom(true).show(
                ui,
                |ui| {
                    let f = cx.view.log_filter.to_lowercase();
                    let mut any = false;
                    for line in cx.state.log.iter().filter(|l| f.is_empty() || l.to_lowercase().contains(&f)) {
                        any = true;
                        let l = line.to_lowercase();
                        let color = if l.contains("error") || l.contains("failed") || l.contains("cannot") {
                            theme::BAD
                        } else if l.contains("warn") || l.contains("cancel") {
                            theme::WARN
                        } else if l.starts_with("  ") {
                            theme::TEXT_DIM
                        } else {
                            TEXT
                        };
                        ui.label(RichText::new(line).monospace().size(12.0).color(color));
                    }
                    if !any {
                        ui.label(RichText::new("nothing yet").color(TEXT_FAINT));
                    }
                },
            );
        });
    });
}
