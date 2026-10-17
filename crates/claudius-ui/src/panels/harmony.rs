//! Harmony: the form coloured by the progression each section plays (click a section to switch it), and each
//! progression as chord chips (Roman numerals with the chord they make in the song's key). Click a chip to change
//! or remove it, add chords from the key's diatonic palette, or edit a progression as text.

use claudius_api::Command;
use eframe::egui::{self, Align2, Color32, PopupCloseBehavior, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

use super::{Cx, empty, page_frame, small};
use crate::cmds::SectionEdit;
use crate::music::{self, Key};
use crate::theme::{self, ACCENT, LINE, RAISED, RAISED_HI, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets;

fn chip(ui: &mut Ui, numeral: &str, chord: Option<&str>, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(86.0, 50.0), Sense::click());
    let hovered = resp.hovered();
    let valid = chord.is_some();
    let base = if valid { color } else { theme::BAD };
    ui.painter().rect(
        rect,
        8.0,
        if hovered { RAISED_HI } else { RAISED },
        Stroke::new(1.0, theme::alpha(base, if hovered { 0.9 } else { 0.5 })),
        StrokeKind::Inside,
    );
    ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(8.0, 0.0), vec2(rect.width() - 16.0, 3.0)), 1.5, base);
    ui.painter().text(rect.center() - vec2(0.0, 7.0), Align2::CENTER_CENTER, numeral, theme::prop(17.0), TEXT);
    ui.painter().text(
        rect.center() + vec2(0.0, 13.0),
        Align2::CENTER_CENTER,
        chord.unwrap_or("?"),
        theme::prop(11.0),
        if valid { TEXT_DIM } else { theme::BAD },
    );
    resp
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.as_ref() else {
        empty(ui, "No song yet.");
        return;
    };
    let key = Key::parse(&song.key).unwrap_or(Key { tonic: 9, mode: 1 });
    let palette = music::diatonic(&key, cx.view.sevenths);
    let mut out: Vec<Command> = Vec::new();
    page_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::heading(ui, "Harmony");
            ui.label(RichText::new(format!("in {}", song.key)).color(TEXT));
            ui.label(small(
                "degrees count in the key's own mode: VI in a minor key is the major chord on its 6th degree",
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut cx.view.sevenths, small("palette with 7ths"));
            });
        });
        ui.add_space(4.0);
        form_strip(ui, song, &mut out);
        ui.add_space(8.0);
        if song.progressions.is_empty() {
            ui.label(RichText::new("This song has no progressions.").color(TEXT_FAINT));
        }
        egui::ScrollArea::vertical().id_salt("harmony").auto_shrink([false, false]).show(ui, |ui| {
            for (name, chords) in &song.progressions {
                let color = theme::prog_color(name);
                egui::Frame::new()
                    .fill(theme::alpha(color, 0.06))
                    .stroke(Stroke::new(1.0, theme::alpha(color, 0.35)))
                    .corner_radius(8)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Progression {name}")).strong().size(14.0).color(TEXT));
                            let names: Vec<String> =
                                chords.iter().map(|c| music::chord(c, &key).map_or("?".into(), |x| x.name)).collect();
                            ui.label(small(names.join("  ·  ")));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let playing: Vec<&claudius_api::SectionView> =
                                    song.sections.iter().filter(|s| prog_of(s) == name.as_str()).collect();
                                if playing.is_empty() {
                                    ui.label(RichText::new("no section plays it").size(11.0).color(theme::WARN));
                                } else {
                                    let bars: u32 = playing.iter().map(|s| s.bars).sum();
                                    let names: Vec<&str> = playing.iter().map(|s| s.name.as_str()).collect();
                                    ui.label(
                                        RichText::new(format!("plays in {} ({bars} bars)", names.join(", ")))
                                            .size(11.0)
                                            .color(theme::mix(color, TEXT, 0.4)),
                                    );
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            for (ci, c) in chords.iter().enumerate() {
                                let resolved = music::chord(c, &key);
                                let resp = chip(ui, c, resolved.as_ref().map(|x| x.name.as_str()), color);
                                let tip = match &resolved {
                                    Some(ch) => format!(
                                        "{c} in {}: {} (notes {}) · click to change",
                                        song.key,
                                        ch.name,
                                        ch.tones.iter().map(|t| key.pc_name(ch.root + t)).collect::<Vec<_>>().join(" ")
                                    ),
                                    None => format!("'{c}' is not a numeral claudius reads"),
                                };
                                let resp = resp.on_hover_text(tip);
                                if resp.clicked() {
                                    cx.view.chip_edit = c.clone();
                                }
                                egui::Popup::from_toggle_button_response(&resp)
                                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                                    .show(|ui| {
                                        ui.set_min_width(260.0);
                                        ui.label(
                                            RichText::new(format!("chord {} of progression {name}", ci + 1)).strong(),
                                        );
                                        ui.horizontal(|ui| {
                                            let r = ui.add(
                                                egui::TextEdit::singleline(&mut cx.view.chip_edit)
                                                    .desired_width(90.0)
                                                    .hint_text("e.g. iv7"),
                                            );
                                            let valid = music::chord(&cx.view.chip_edit, &key);
                                            ui.label(
                                                RichText::new(
                                                    valid.as_ref().map_or("not a chord".into(), |v| v.name.clone()),
                                                )
                                                .color(if valid.is_some() { TEXT_DIM } else { theme::BAD }),
                                            );
                                            if (ui.button("Set").clicked()
                                                || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                                                && valid.is_some()
                                            {
                                                let mut next = chords.clone();
                                                next[ci] = cx.view.chip_edit.trim().to_string();
                                                out.push(Command::SetProgression { name: name.clone(), chords: next });
                                                ui.close();
                                            }
                                        });
                                        ui.label(small("diatonic"));
                                        ui.horizontal_wrapped(|ui| {
                                            for n in &palette {
                                                if ui.button(n).clicked() {
                                                    let mut next = chords.clone();
                                                    next[ci] = n.clone();
                                                    out.push(Command::SetProgression {
                                                        name: name.clone(),
                                                        chords: next,
                                                    });
                                                    ui.close();
                                                }
                                            }
                                        });
                                        ui.separator();
                                        ui.horizontal(|ui| {
                                            if ci > 0 && ui.button("move left").clicked() {
                                                let mut next = chords.clone();
                                                next.swap(ci, ci - 1);
                                                out.push(Command::SetProgression { name: name.clone(), chords: next });
                                                ui.close();
                                            }
                                            if ci + 1 < chords.len() && ui.button("move right").clicked() {
                                                let mut next = chords.clone();
                                                next.swap(ci, ci + 1);
                                                out.push(Command::SetProgression { name: name.clone(), chords: next });
                                                ui.close();
                                            }
                                            if ui.button("duplicate").clicked() {
                                                let mut next = chords.clone();
                                                next.insert(ci + 1, c.clone());
                                                out.push(Command::SetProgression { name: name.clone(), chords: next });
                                                ui.close();
                                            }
                                            if chords.len() > 1
                                                && ui.button(RichText::new("remove").color(theme::BAD)).clicked()
                                            {
                                                let mut next = chords.clone();
                                                next.remove(ci);
                                                out.push(Command::SetProgression { name: name.clone(), chords: next });
                                                ui.close();
                                            }
                                        });
                                    });
                            }
                            let (rect, add) = ui.allocate_exact_size(vec2(50.0, 50.0), Sense::click());
                            ui.painter().rect(
                                rect,
                                8.0,
                                if add.hovered() { RAISED_HI } else { theme::WELL },
                                Stroke::new(1.0, LINE),
                                StrokeKind::Inside,
                            );
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                "+",
                                theme::prop(22.0),
                                if add.hovered() { ACCENT } else { TEXT_FAINT },
                            );
                            let add = add.on_hover_text("add a chord");
                            egui::Popup::from_toggle_button_response(&add)
                                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                                .show(|ui| {
                                    ui.label(small("append a diatonic chord"));
                                    ui.horizontal_wrapped(|ui| {
                                        for n in &palette {
                                            let label = music::chord(n, &key)
                                                .map_or(n.clone(), |c| format!("{n} ({})", c.name));
                                            if ui.button(label).clicked() {
                                                let mut next = chords.clone();
                                                next.push(n.clone());
                                                out.push(Command::SetProgression { name: name.clone(), chords: next });
                                                ui.close();
                                            }
                                        }
                                    });
                                });
                        });
                        ui.add_space(6.0);
                        // the progression as text
                        let joined = chords.join(" ");
                        let slot = match cx.view.prog_text.iter().position(|(n, _)| n == name) {
                            Some(i) => i,
                            None => {
                                cx.view.prog_text.push((name.clone(), joined.clone()));
                                cx.view.prog_text.len() - 1
                            }
                        };
                        ui.horizontal(|ui| {
                            ui.label(small("as text"));
                            let id = ui.scope_id().with(("prog_text", name));
                            let editing = ui.memory(|m| m.has_focus(id));
                            if !editing {
                                cx.view.prog_text[slot].1 = joined.clone();
                            }
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut cx.view.prog_text[slot].1)
                                    .id(id)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(320.0),
                            );
                            let text = cx.view.prog_text[slot].1.clone();
                            let parsed: Vec<String> = text.split_whitespace().map(String::from).collect();
                            let bad: Vec<&String> = parsed.iter().filter(|c| music::chord(c, &key).is_none()).collect();
                            let apply = ui
                                .add_enabled(parsed != *chords && !parsed.is_empty(), egui::Button::new("Apply"))
                                .clicked()
                                || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                            if !bad.is_empty() {
                                ui.label(
                                    RichText::new(format!(
                                        "not chords: {}",
                                        bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                                    ))
                                    .color(theme::WARN)
                                    .size(11.0),
                                );
                            }
                            if apply && parsed != *chords && !parsed.is_empty() {
                                out.push(Command::SetProgression { name: name.clone(), chords: parsed });
                            }
                        });
                    });
                ui.add_space(8.0);
            }
            let used: Vec<String> = song.progressions.iter().map(|(n, _)| n.clone()).collect();
            if let Some(next) = ["A", "B", "C", "D"].iter().find(|n| !used.iter().any(|u| u == *n)) {
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("+ progression {next}"))
                        .on_hover_text("a new progression (sections choose theirs on the server)")
                        .clicked()
                    {
                        out.push(Command::SetProgression {
                            name: next.to_string(),
                            chords: palette.iter().take(4).cloned().collect(),
                        });
                    }
                });
            }
        });
    });
    cx.out.extend(out);
}

fn prog_of(s: &claudius_api::SectionView) -> &str {
    if s.prog.is_empty() { "A" } else { s.prog.as_str() }
}

/// the form as blocks coloured by progression; click one to give it another progression
fn form_strip(ui: &mut Ui, song: &claudius_api::SongView, out: &mut Vec<Command>) {
    ui.label(small("which progression each section plays (click a section to switch it)"));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let total = song.bars.max(1) as f32;
    for (i, s) in song.sections.iter().enumerate() {
        let x0 = rect.left() + rect.width() * s.start_bar as f32 / total;
        let x1 = rect.left() + rect.width() * (s.start_bar + s.bars) as f32 / total;
        let r = Rect::from_min_max(egui::pos2(x0 + 1.0, rect.top()), egui::pos2(x1 - 1.0, rect.bottom()));
        let resp = ui.interact(r, ui.scope_id().with(("form_prog", i)), Sense::click());
        let prog = prog_of(s);
        let color = theme::prog_color(prog);
        let fill = if resp.hovered() { theme::alpha(color, 0.55) } else { theme::alpha(color, 0.34) };
        ui.painter().rect(r, 4.0, fill, Stroke::new(1.0, theme::alpha(color, 0.9)), StrokeKind::Inside);
        let clip = ui.painter().with_clip_rect(r.shrink(2.0));
        clip.text(r.left_top() + vec2(5.0, 9.0), Align2::LEFT_CENTER, &s.name, theme::prop(10.5), TEXT);
        clip.text(r.left_bottom() + vec2(5.0, -10.0), Align2::LEFT_CENTER, prog, theme::prop(13.0), Color32::WHITE);
        let resp =
            resp.on_hover_text(format!("{} ({} bars) plays progression {prog}: click to switch", s.name, s.bars));
        egui::Popup::menu(&resp).show(|ui| {
            ui.label(RichText::new(format!("{} plays", s.name)).strong());
            for (name, chords) in &song.progressions {
                let on = name == prog;
                let text = RichText::new(format!("{name}  {}", chords.join(" "))).color(if on {
                    theme::prog_color(name)
                } else {
                    TEXT
                });
                if ui.add_enabled(!on, egui::Button::new(text)).clicked() {
                    out.push(SectionEdit { prog: Some(name.clone()), ..Default::default() }.at(i));
                    ui.close();
                }
            }
        });
    }
}
