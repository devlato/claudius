//! The section inspector: name, bars, energy, drum variant and the transition flags of the selected section, plus
//! adding, moving, duplicating and removing sections.

use claudius_api::Command;
use eframe::egui::{self, RichText, Ui, vec2};

use super::{Cx, small};
use crate::cmds::SectionEdit;
use crate::edits::SECTION_KINDS;
use crate::theme::{self, TEXT, TEXT_DIM};
use crate::widgets::{self, Icon, commit};

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.as_mut() else { return };
    if song.sections.is_empty() {
        return;
    }
    let i = cx.view.section.min(song.sections.len() - 1);
    let n = song.sections.len();
    let variants: Vec<String> = {
        let mut v: Vec<String> = vec!["main".into()];
        for d in &song.drums {
            for (name, _) in &d.variants {
                if !v.contains(name) {
                    v.push(name.clone());
                }
            }
        }
        v
    };
    let layers_all = song.layers.clone();
    let bpm = song.bpm.max(1.0);
    let progressions = song.progressions.clone();
    let key = crate::music::Key::parse(&song.key);
    let s = &mut song.sections[i];
    let color = theme::section_color(&s.kind);
    ui.horizontal(|ui| {
        widgets::heading(ui, "Section");
        ui.label(small(format!("{} of {n}", i + 1)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(n > 1, |ui| {
                if widgets::icon_button(ui, Icon::Trash, 24.0, false, n > 1, "remove this section").clicked() {
                    cx.out.push(Command::RemoveSection { index: i });
                }
            });
            if widgets::icon_button(ui, Icon::Copy, 24.0, false, true, "duplicate this section after itself").clicked()
            {
                cx.out.push(Command::AddSection { after: i, kind: s.kind.clone(), bars: s.bars });
                cx.out.push(
                    SectionEdit {
                        energy: Some(s.energy),
                        layers: Some(s.layers.clone()),
                        variant: Some(s.variant.clone()),
                        filter_in: Some(s.filter_in),
                        build: Some(s.build),
                        gap: Some(s.gap),
                        impact: Some(s.impact),
                        fills: Some(s.fills),
                        ..Default::default()
                    }
                    .at(i + 1),
                );
                cx.view.section = i + 1;
            }
            if widgets::icon_button(ui, Icon::Right, 24.0, false, i + 1 < n, "move right").clicked() {
                cx.out.push(Command::MoveSection { from: i, to: i + 1 });
                cx.view.section = i + 1;
            }
            if widgets::icon_button(ui, Icon::Left, 24.0, false, i > 0, "move left").clicked() {
                cx.out.push(Command::MoveSection { from: i, to: i - 1 });
                cx.view.section = i - 1;
            }
        });
    });
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(theme::alpha(color, 0.12))
        .stroke(egui::Stroke::new(1.0, theme::alpha(color, 0.5)))
        .corner_radius(6)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("section_grid").num_columns(2).spacing(vec2(10.0, 7.0)).show(ui, |ui| {
                ui.label(small("Name"));
                ui.horizontal(|ui| {
                    let id = ui.make_persistent_id("section_name");
                    let editing =
                        ui.memory(|m| m.has_focus(id)) && cx.view.name_edit.as_ref().is_some_and(|(at, _)| *at == i);
                    if !editing {
                        cx.view.name_edit = Some((i, s.name.clone()));
                    }
                    if let Some((_, text)) = cx.view.name_edit.as_mut() {
                        let r = ui.add(egui::TextEdit::singleline(text).id(id).desired_width(120.0));
                        let name = text.trim().to_string();
                        if r.lost_focus() && !name.is_empty() && name != s.name {
                            s.name = name.clone();
                            cx.out.push(SectionEdit { name: Some(name), ..Default::default() }.at(i));
                        }
                    }
                    widgets::pill(ui, &s.kind, color);
                });
                ui.end_row();

                ui.label(small("Bars"));
                ui.horizontal(|ui| {
                    let r = ui.add(
                        egui::DragValue::new(&mut s.bars)
                            .range(1..=256)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .update_while_editing(false),
                    );
                    if commit(&r) {
                        cx.out.push(SectionEdit { bars: Some(s.bars), ..Default::default() }.at(i));
                    }
                    for b in [4, 8, 16, 32] {
                        if ui.selectable_label(s.bars == b, b.to_string()).clicked() {
                            s.bars = b;
                            cx.out.push(SectionEdit { bars: Some(b), ..Default::default() }.at(i));
                        }
                    }
                });
                ui.end_row();

                ui.label(small("Starts"));
                ui.label(
                    RichText::new(format!(
                        "bar {}  ·  {}",
                        s.start_bar + 1,
                        super::fmt_time(s.start_bar as f64 * 240.0 / bpm)
                    ))
                    .color(TEXT_DIM),
                );
                ui.end_row();

                ui.label(small("Energy"));
                let r = ui.add(
                    egui::Slider::new(&mut s.energy, 0.0..=1.0)
                        .clamping(egui::SliderClamping::Edits)
                        .fixed_decimals(2)
                        .trailing_fill(true),
                );
                if commit(&r) {
                    cx.out.push(SectionEdit { energy: Some(s.energy), ..Default::default() }.at(i));
                }
                ui.end_row();

                ui.label(small("Harmony"));
                ui.horizontal(|ui| {
                    let label = |name: &str| -> String {
                        let chords =
                            progressions.iter().find(|(n, _)| n == name).map(|(_, c)| c.as_slice()).unwrap_or(&[]);
                        let names: Vec<String> = chords
                            .iter()
                            .map(|c| key.as_ref().and_then(|k| crate::music::chord(c, k)).map_or(c.clone(), |x| x.name))
                            .collect();
                        format!("{name}  {}", names.join(" "))
                    };
                    let current = if s.prog.is_empty() { "A".to_string() } else { s.prog.clone() };
                    let (rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, theme::prog_color(&current));
                    egui::ComboBox::from_id_salt("section_prog")
                        .selected_text(label(&current))
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            for (name, _) in &progressions {
                                if ui
                                    .selectable_label(
                                        *name == current,
                                        RichText::new(label(name)).color(theme::prog_color(name)),
                                    )
                                    .clicked()
                                    && *name != current
                                {
                                    s.prog = name.clone();
                                    cx.out.push(SectionEdit { prog: Some(name.clone()), ..Default::default() }.at(i));
                                }
                            }
                        })
                        .response
                        .on_hover_text("the chord progression this section plays");
                });
                ui.end_row();

                ui.label(small("Drums"));
                egui::ComboBox::from_id_salt("section_variant").selected_text(&s.variant).width(110.0).show_ui(
                    ui,
                    |ui| {
                        for v in &variants {
                            if ui.selectable_label(&s.variant == v, v).clicked() && &s.variant != v {
                                s.variant = v.clone();
                                cx.out.push(SectionEdit { variant: Some(v.clone()), ..Default::default() }.at(i));
                            }
                        }
                    },
                );
                ui.end_row();

                ui.label(small("Moves"));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    let flags: [(&str, &str, bool); 5] = [
                        ("filter in", "open a lowpass across the section", s.filter_in),
                        ("build", "riser, snare roll and highpass sweep into the next section", s.build),
                        ("gap", "a beat of silence before the next section", s.gap),
                        ("impact", "crash + impact on the downbeat", s.impact),
                        ("fills", "drum fills at the end of every 8 bars", s.fills),
                    ];
                    for (k, (label, tip, on)) in flags.into_iter().enumerate() {
                        if widgets::chip(ui, on, label, color).on_hover_text(tip).clicked() {
                            let v = Some(!on);
                            let mut e = SectionEdit::default();
                            match k {
                                0 => (s.filter_in, e.filter_in) = (!on, v),
                                1 => (s.build, e.build) = (!on, v),
                                2 => (s.gap, e.gap) = (!on, v),
                                3 => (s.impact, e.impact) = (!on, v),
                                _ => (s.fills, e.fills) = (!on, v),
                            }
                            cx.out.push(e.at(i));
                        }
                    }
                });
                ui.end_row();

                ui.label(small("Layers"));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} of {}", s.layers.len(), layers_all.len())).color(TEXT));
                    if ui.small_button("all").on_hover_text("every layer plays here").clicked() {
                        cx.out.push(SectionEdit { layers: Some(layers_all.clone()), ..Default::default() }.at(i));
                    }
                    if ui.small_button("none").clicked() {
                        cx.out.push(SectionEdit { layers: Some(Vec::new()), ..Default::default() }.at(i));
                    }
                });
                ui.end_row();
            });
        });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let looping = cx.view.looping && cx.view.loop_section;
        if widgets::chip(ui, looping, "loop this section", theme::ACCENT)
            .on_hover_text("loop playback over the selected section (L toggles looping)")
            .clicked()
        {
            if looping {
                cx.view.looping = false;
            } else {
                cx.view.looping = true;
                cx.view.loop_section = true;
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(small("Add after"));
        egui::ComboBox::from_id_salt("add_kind").selected_text(&cx.view.add_kind).width(80.0).show_ui(ui, |ui| {
            for k in SECTION_KINDS {
                ui.selectable_value(&mut cx.view.add_kind, k.to_string(), k);
            }
        });
        ui.add(egui::DragValue::new(&mut cx.view.add_bars).range(1..=64).suffix(" bars"));
        if widgets::icon_button(ui, Icon::Plus, 24.0, false, true, "add a section after this one").clicked() {
            cx.out.push(Command::AddSection { after: i, kind: cx.view.add_kind.clone(), bars: cx.view.add_bars });
            cx.view.section = i + 1;
        }
    });
}
