//! The melodic parts: per part its writing style, its sound (a synth preset or a sampled instrument), its register
//! (drag on the keyboard or edit the notes), its density, and Reseed to write it again.

use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use super::{Cx, empty, layer_color, page_frame, small};
use crate::cmds::PartEdit;
use crate::music::note_name;
use crate::theme::{self, RAISED, TEXT, WELL};
use crate::widgets::{self, Icon, commit};

const LOW: i32 = 21;
const HIGH: i32 = 108;

fn is_black(n: i32) -> bool {
    matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

/// a piano strip from A0 to C8 with the register lit; drag across it to set a new range
fn keyboard(ui: &mut Ui, register: &mut (i32, i32), color: Color32, width: f32) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click_and_drag());
    let whites: Vec<i32> = (LOW..=HIGH).filter(|n| !is_black(*n)).collect();
    let ww = rect.width() / whites.len() as f32;
    let x_of_white = |n: i32| -> f32 { rect.left() + whites.iter().position(|w| *w == n).unwrap_or(0) as f32 * ww };
    let note_at = |x: f32| -> i32 {
        let i = (((x - rect.left()) / ww) as usize).min(whites.len() - 1);
        whites[i]
    };
    let p = ui.painter();
    p.rect_filled(rect, 3.0, WELL);
    let (lo, hi) = *register;
    for &n in &whites {
        let r = Rect::from_min_size(pos2(x_of_white(n) + 0.5, rect.top()), vec2(ww - 1.0, rect.height()));
        let lit = n >= lo && n <= hi;
        p.rect_filled(
            r,
            1.5,
            if lit { theme::mix(Color32::from_gray(200), color, 0.55) } else { Color32::from_gray(150) },
        );
        if n % 12 == 0 && ww > 3.0 {
            p.text(
                pos2(r.center().x, r.bottom() - 6.0),
                Align2::CENTER_CENTER,
                format!("{}", n / 12 - 1),
                theme::prop(7.5),
                Color32::from_gray(60),
            );
        }
    }
    for n in LOW..=HIGH {
        if !is_black(n) {
            continue;
        }
        let x = x_of_white(n - 1) + ww * 0.68;
        let r = Rect::from_min_size(pos2(x, rect.top()), vec2(ww * 0.64, rect.height() * 0.6));
        let lit = n >= lo && n <= hi;
        p.rect_filled(
            r,
            1.0,
            if lit { theme::mix(Color32::from_gray(25), color, 0.75) } else { Color32::from_gray(25) },
        );
    }
    if let (Some(a), Some(b)) =
        (resp.interact_pointer_pos().filter(|_| resp.dragged()), ui.input(|i| i.pointer.press_origin()))
    {
        let (x, y) = (note_at(a.x), note_at(b.x));
        let new = (x.min(y), x.max(y).max(x.min(y) + 1));
        if new != *register {
            *register = new;
            resp.mark_changed();
        }
    }
    resp.on_hover_text(format!(
        "register {} .. {} (drag across the keys to set a new range)",
        note_name(lo),
        note_name(hi)
    ))
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.as_mut() else {
        empty(ui, "No song yet.");
        return;
    };
    if song.parts.is_empty() {
        empty(ui, "This song has no melodic parts.");
        return;
    }
    let colors: Vec<Color32> = song.parts.iter().map(|p| layer_color(song, &p.name)).collect();
    let sounds: Vec<String> = song
        .parts
        .iter()
        .map(|p| song.tracks.iter().find(|t| t.name == p.name).map(|t| t.sound.clone()).unwrap_or_default())
        .collect();
    let mut out = Vec::new();
    let library = cx.sounds;
    page_frame().show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("parts").auto_shrink([false, false]).show(ui, |ui| {
            for (i, part) in song.parts.iter_mut().enumerate() {
                let color = colors[i];
                let frame = egui::Frame::new()
                    .fill(RAISED)
                    .corner_radius(6)
                    .inner_margin(egui::Margin { left: 14, right: 10, top: 8, bottom: 8 })
                    .stroke(Stroke::new(1.0, theme::LINE_SOFT));
                let r = frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(150.0);
                            ui.label(RichText::new(&part.name).size(14.0).strong().color(TEXT));
                            ui.label(small(format!("{} · {}", part.role, sounds[i].replace("synth patch ", ""))));
                        });
                        ui.vertical(|ui| {
                            ui.label(small("Style"));
                            egui::ComboBox::from_id_salt(("style", i)).selected_text(&part.style).width(110.0).show_ui(
                                ui,
                                |ui| {
                                    for s in part.styles.clone() {
                                        if ui.selectable_label(part.style == s, &s).clicked() && part.style != s {
                                            part.style = s.clone();
                                            out.push(PartEdit { style: Some(s), ..Default::default() }.on(&part.name));
                                        }
                                    }
                                },
                            );
                        });
                        ui.vertical(|ui| {
                            let sampled = super::is_sampled(&part.preset, library);
                            ui.label(small(if sampled { "Sound (sampled)" } else { "Sound (synth)" }));
                            egui::ComboBox::from_id_salt(("preset", i))
                                .selected_text(&part.preset)
                                .width(140.0)
                                .show_ui(ui, |ui| {
                                    let (samples, synths): (Vec<String>, Vec<String>) =
                                        part.presets.iter().cloned().partition(|s| super::is_sampled(s, library));
                                    for (title, names) in [("synth presets", synths), ("sampled instruments", samples)]
                                    {
                                        if names.is_empty() {
                                            continue;
                                        }
                                        ui.label(small(title));
                                        for s in names {
                                            if ui.selectable_label(part.preset == s, &s).clicked() && part.preset != s {
                                                part.preset = s.clone();
                                                out.push(
                                                    PartEdit { preset: Some(s), ..Default::default() }.on(&part.name),
                                                );
                                            }
                                        }
                                    }
                                    ui.label(small("more in the Sounds tab"));
                                });
                        });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(small("Register"));
                                ui.label(
                                    RichText::new(format!(
                                        "{} – {}",
                                        note_name(part.register.0),
                                        note_name(part.register.1)
                                    ))
                                    .size(11.0)
                                    .color(TEXT),
                                );
                                ui.label(small(format!("({} semitones)", part.register.1 - part.register.0)));
                            });
                            ui.horizontal(|ui| {
                                let kb = keyboard(ui, &mut part.register, color, 300.0);
                                if kb.drag_stopped() {
                                    out.push(
                                        PartEdit { register: Some(part.register), ..Default::default() }.on(&part.name),
                                    );
                                }
                                let (mut lo, mut hi) = part.register;
                                let fmt = |v: f64, _: std::ops::RangeInclusive<usize>| note_name(v as i32);
                                let parse = |s: &str| crate::music::parse_note(s).map(|n| n as f64);
                                let r1 = ui.add(
                                    egui::DragValue::new(&mut lo)
                                        .range(LOW..=HIGH - 1)
                                        .clamp_existing_to_range(false)
                                        .speed(0.15)
                                        .custom_formatter(fmt)
                                        .custom_parser(parse)
                                        .update_while_editing(false),
                                );
                                let r2 = ui.add(
                                    egui::DragValue::new(&mut hi)
                                        .range(LOW + 1..=HIGH)
                                        .clamp_existing_to_range(false)
                                        .speed(0.15)
                                        .custom_formatter(fmt)
                                        .custom_parser(parse)
                                        .update_while_editing(false),
                                );
                                if (lo, hi) != part.register {
                                    part.register = (lo.min(hi - 1), hi.max(lo + 1));
                                }
                                if commit(&r1) || commit(&r2) {
                                    out.push(
                                        PartEdit { register: Some(part.register), ..Default::default() }.on(&part.name),
                                    );
                                }
                            });
                        });
                        ui.vertical(|ui| {
                            ui.label(small("Density"));
                            let r = ui.add(
                                egui::Slider::new(&mut part.density, 0.0..=1.0)
                                    .clamping(egui::SliderClamping::Edits)
                                    .fixed_decimals(2),
                            );
                            if commit(&r) {
                                out.push(PartEdit { density: Some(part.density), ..Default::default() }.on(&part.name));
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::tool_button(ui, Icon::Dice, "Reseed", true, false)
                                .on_hover_text("write this part again with a new seed")
                                .clicked()
                            {
                                out.push(PartEdit { reseed: true, ..Default::default() }.on(&part.name));
                            }
                        });
                    });
                });
                let rect = r.response.rect;
                ui.painter().rect_filled(
                    Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(4.0, rect.height() - 12.0)),
                    2.0,
                    color,
                );
                ui.add_space(4.0);
            }
            ui.label(small("register: the lowest and highest note the part may use · density: how busy it writes"));
        });
    });
    cx.out.extend(out);
}
