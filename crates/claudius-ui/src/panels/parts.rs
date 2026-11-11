//! The melodic parts: per part its writing style, its sound (a synth preset or a sampled instrument), its register
//! (drag on the keyboard or edit the notes), its density, and Reseed to write it again.

use claudius_api::{Command, PartView, words};
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
    let nexts: Vec<Option<String>> =
        song.parts.iter().map(|p| song.tracks.iter().find(|t| t.name == p.name).and_then(super::next_label)).collect();
    let mut out = Vec::new();
    let library = cx.sounds;
    page_frame().show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("parts").auto_shrink([false, false]).show(ui, |ui| {
            for (i, part) in song.parts.iter_mut().enumerate() {
                let color = colors[i];
                let frame = egui::Frame::new()
                    .fill(RAISED.into())
                    .corner_radius(6)
                    .inner_margin(egui::Margin { left: 14, right: 10, top: 8, bottom: 8 })
                    .stroke(Stroke::new(1.0, theme::LINE_SOFT));
                let r = frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    // the keyboard takes what the name, the style, the sound, the range's notes, the density and the
                    // buttons leave of the row
                    let keys = (ui.available_width() - 800.0).clamp(120.0, 300.0);
                    // the range, the density and the buttons go under the rest when the row is narrow
                    widgets::row(ui, ("part", i), |ui, side| {
                        if side == widgets::Side::Right {
                            range_density_buttons(ui, part, color, keys, &mut out);
                            return;
                        }
                        ui.vertical(|ui| {
                            ui.set_width(130.0);
                            ui.label(RichText::new(&part.name).size(14.0).strong().color(TEXT));
                            let plays = words::sound_in_words(&sounds[i]);
                            ui.add(egui::Label::new(small(format!("{} · {plays}", part.role))).truncate())
                                .on_hover_text(format!("a {} part, played by {plays}", part.role));
                        });
                        ui.vertical(|ui| {
                            widgets::label_help(
                                ui,
                                "Style",
                                "How the theory engine writes the part: a bass that holds each root, rolls in \
                                 eighths or plays a genre's idiom (one drop, slap, G-funk ...); chords held, stabbed \
                                 or comped as a genre's keyboard player would.",
                                "language/song-functions.html#part",
                            );
                            let about =
                                |s: &str| library.and_then(|l| l.style_about(&part.role, s)).map(str::to_string);
                            let tip = super::named(about(&part.style).as_deref(), &part.style);
                            egui::ComboBox::from_id_salt(("style", i))
                                .selected_text(words::name_in_words(&part.style))
                                .width(110.0)
                                .truncate()
                                .show_ui(ui, |ui| {
                                    for s in part.styles.clone() {
                                        let r = ui
                                            .selectable_label(part.style == s, words::name_in_words(&s))
                                            .on_hover_text(super::named(about(&s).as_deref(), &s));
                                        if r.clicked() && part.style != s {
                                            part.style = s.clone();
                                            out.push(PartEdit { style: Some(s), ..Default::default() }.on(&part.name));
                                        }
                                    }
                                })
                                .response
                                .on_hover_text(tip);
                        });
                        ui.vertical(|ui| {
                            let sampled = super::is_sampled(&part.preset, library);
                            ui.label(small(if sampled { "Sound (sampled)" } else { "Sound (synth)" }));
                            ui.horizontal(|ui| {
                                let tip = |s: &str| super::named(super::sound_about(s, library).as_deref(), s);
                                egui::ComboBox::from_id_salt(("preset", i))
                                    .selected_text(words::name_in_words(&part.preset))
                                    .width(140.0)
                                    .truncate()
                                    .show_ui(ui, |ui| {
                                        let (samples, synths): (Vec<String>, Vec<String>) =
                                            part.presets.iter().cloned().partition(|s| super::is_sampled(s, library));
                                        for (title, names) in
                                            [("synth presets", synths), ("sampled instruments", samples)]
                                        {
                                            if names.is_empty() {
                                                continue;
                                            }
                                            ui.label(small(title));
                                            for s in names {
                                                let r = ui
                                                    .selectable_label(part.preset == s, words::name_in_words(&s))
                                                    .on_hover_text(tip(&s));
                                                if r.clicked() && part.preset != s {
                                                    part.preset = s.clone();
                                                    out.push(
                                                        PartEdit { preset: Some(s), ..Default::default() }
                                                            .on(&part.name),
                                                    );
                                                }
                                            }
                                        }
                                        ui.label(small("more in the Sounds tab"));
                                    })
                                    .response
                                    .on_hover_text(tip(&part.preset));
                                if let Some(next) = &nexts[i]
                                    && widgets::icon_button(
                                        ui,
                                        Icon::Swap,
                                        22.0,
                                        false,
                                        true,
                                        &format!("another sound: {next}"),
                                    )
                                    .clicked()
                                {
                                    out.push(super::swap(&part.name, None));
                                }
                            });
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

/// a part's range on its keyboard, its density and its Vary and Reseed buttons: the right of its row
fn range_density_buttons(ui: &mut Ui, part: &mut PartView, color: Color32, keys: f32, out: &mut Vec<Command>) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            widgets::label_help(
                ui,
                "Register",
                "The lowest and highest notes the part may play. Drag on the keyboard or the two notes: a bass sits \
                 low, a lead high, and parts that keep apart sound clearer.",
                "language/song-functions.html#part",
            );
            let range = format!("{} – {}", note_name(part.register.0), note_name(part.register.1));
            ui.label(RichText::new(range).size(11.0).color(TEXT));
            ui.label(small(format!("({} semitones)", part.register.1 - part.register.0)));
        });
        ui.horizontal(|ui| {
            let kb = keyboard(ui, &mut part.register, color, keys);
            if kb.drag_stopped() {
                out.push(PartEdit { register: Some(part.register), ..Default::default() }.on(&part.name));
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
                out.push(PartEdit { register: Some(part.register), ..Default::default() }.on(&part.name));
            }
        });
    });
    ui.vertical(|ui| {
        // its value in its label, so that the rail is all it needs beside the keyboard
        widgets::label_help(
            ui,
            &format!("Density {:.0}%", part.density * 100.0),
            "How busy the part is: few notes at 0%, as many as its style plays at 100%.",
            "language/song-functions.html#part",
        );
        ui.spacing_mut().slider_width = 70.0;
        let r = ui
            .add(
                egui::Slider::new(&mut part.density, 0.0..=1.0).clamping(egui::SliderClamping::Edits).show_value(false),
            )
            .on_hover_text("how busy it plays: few notes at 0%, many at 100%");
        if commit(&r) {
            out.push(PartEdit { density: Some(part.density), ..Default::default() }.on(&part.name));
        }
    });
    if widgets::tool_button(ui, Icon::Mutate, "Vary", true, false)
        .on_hover_text("the same take, other details: its rhythm stays, its ghosts, octaves and approaches change")
        .clicked()
    {
        out.push(Command::VaryTrack { track: part.name.clone(), take: false });
    }
    if widgets::tool_button(ui, Icon::Dice, "Reseed", true, false)
        .on_hover_text("write this part again with a new seed: another take")
        .clicked()
    {
        out.push(PartEdit { reseed: true, ..Default::default() }.on(&part.name));
    }
}
