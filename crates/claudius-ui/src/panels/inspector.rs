//! The section inspector: name, length, intensity, chords, drum pattern, transitions and the tracks playing in the
//! selected section, plus adding, moving, duplicating and removing sections; and under it, for the track selected, the
//! track inspector.

use claudius_api::Command;
use eframe::egui::{self, RichText, Ui, vec2};

use super::{Cx, Tab, small};
use crate::cmds::{MixEdit, SectionEdit};
use crate::edits::SECTION_KINDS;
use crate::theme::{self, TEXT_DIM};
use crate::widgets::{self, Icon, commit};

/// a section's moves, in plain words, with what each does: its filter in, build, gap, impact and fills, in that order
pub const MOVES: [(&str, &str); 5] = [
    ("Filter sweep", "a lowpass filter opens across the section"),
    ("Build-up", "a build into the next section: a snare roll, a riser, a sweep, or the drums out, as you choose"),
    ("Pause at the end", "a beat of silence before the next section"),
    ("Impact", "a crash and an impact on its first beat"),
    ("Drum fills", "a drum fill at the end of every 8 bars"),
];

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
    let colors: Vec<egui::Color32> = layers_all.iter().map(|l| super::layer_color(song, l)).collect();
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

                ui.label(small("Length"));
                ui.horizontal(|ui| {
                    let r = ui.add(
                        egui::DragValue::new(&mut s.bars)
                            .range(1..=256)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .suffix(" bars")
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

                widgets::label_help(
                    ui,
                    "Intensity",
                    "How full and loud the section plays, from almost empty (0%) to everything at once (100%). An \
                     intro or a break is low, a drop high: it sets how many parts play and how busy the drums are.",
                    "language/form.html#energy",
                );
                let r = ui
                    .add(
                        egui::Slider::new(&mut s.energy, 0.0..=1.0)
                            .clamping(egui::SliderClamping::Edits)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                            .custom_parser(super::percent)
                            .trailing_fill(true),
                    )
                    .on_hover_text("how full and loud it plays: low for an intro or a break, high for a drop");
                if commit(&r) {
                    cx.out.push(SectionEdit { energy: Some(s.energy), ..Default::default() }.at(i));
                }
                ui.end_row();

                widgets::label_help(
                    ui,
                    "Chords",
                    "The chord progression this section plays: a list of chords that repeats. Sections can share one \
                     (A) or have their own (B for a break). Change the chords themselves in the Chords tab.",
                    "language/song-functions.html#prog",
                );
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

                widgets::label_help(
                    ui,
                    "Drum pattern",
                    "Which version of the drum patterns plays here: main, or a sparser or busier one. Each drum lane \
                     has its steps for each version; write them in the Drums tab.",
                    "language/song-functions.html#drum",
                );
                egui::ComboBox::from_id_salt("section_variant")
                    .selected_text(&s.variant)
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for v in &variants {
                            if ui.selectable_label(&s.variant == v, v).clicked() && &s.variant != v {
                                s.variant = v.clone();
                                cx.out.push(SectionEdit { variant: Some(v.clone()), ..Default::default() }.at(i));
                            }
                        }
                    })
                    .response
                    .on_hover_text("which of the drums' patterns this section plays (Drums shows each)");
                ui.end_row();

                widgets::label_help(
                    ui,
                    "Transitions",
                    "Moves that lead into or out of the section: a filter that opens over it, a build-up of riser \
                     and snare roll into the next, a beat of silence at its end, a crash on its first beat, drum fills.",
                    "language/form.html#what-the-flags-do",
                );
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    let on = [s.filter_in, s.build, s.gap, s.impact, s.fills];
                    for (k, ((label, tip), on)) in MOVES.into_iter().zip(on).enumerate() {
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

                if s.build {
                    widgets::label_help(
                        ui,
                        "Build-up",
                        "How the build-up leads into the next section, and how hard. Everything rolls the snare, \
                         takes the kick out, raises a riser and sweeps the low end away; the other ways take one or \
                         two of those. Harder makes the roll longer and louder, the sweep climb higher, the riser \
                         longer, and the drums leave for longer.",
                        "language/form.html#build",
                    );
                    ui.horizontal(|ui| {
                        let styles = claudius_api::BUILD_STYLES;
                        let shown = styles.iter().find(|b| b.0 == s.build_style).map_or(styles[0].1, |b| b.1);
                        egui::ComboBox::from_id_salt("build_style").width(130.0).selected_text(shown).show_ui(
                            ui,
                            |ui| {
                                for (name, words, about) in styles {
                                    let r = ui.selectable_label(s.build_style == name, words).on_hover_text(about);
                                    if r.clicked() && s.build_style != name {
                                        s.build_style = name.to_string();
                                        let e = SectionEdit { build_style: Some(name.into()), ..Default::default() };
                                        cx.out.push(e.at(i));
                                    }
                                }
                            },
                        );
                        ui.label(small("gentle"));
                        let r = ui
                            .add(egui::Slider::new(&mut s.build_amount, 0.0..=1.0).show_value(false))
                            .on_hover_text("how hard it builds: half is where every build-up started");
                        ui.label(small("hard"));
                        if commit(&r) {
                            let e = SectionEdit { build_amount: Some(s.build_amount), ..Default::default() };
                            cx.out.push(e.at(i));
                        }
                    });
                    ui.end_row();
                }

                widgets::label_help(
                    ui,
                    "Playing",
                    "The tracks that play in this section, lit when they do. Click one to switch it on or off here; \
                     all on and all off do every track. A double-click on a track's cell in the arrangement does the \
                     same.",
                    "getting-started/studio.html#the-panels",
                );
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for (layer, c) in layers_all.iter().zip(&colors) {
                        let on = s.layers.contains(layer);
                        let tip = if on { "plays here: click to leave it out" } else { "click to have it play here" };
                        if widgets::chip(ui, on, layer, *c).on_hover_text(tip).clicked() {
                            cx.out.push(Command::ToggleLayer { section: i, layer: layer.clone() });
                        }
                    }
                    let all = ui.add(egui::Button::new(small("all on")).frame(false));
                    if all.on_hover_text("every track plays here").clicked() {
                        cx.out.push(SectionEdit { layers: Some(layers_all.clone()), ..Default::default() }.at(i));
                    }
                    let none = ui.add(egui::Button::new(small("all off")).frame(false));
                    if none.on_hover_text("nothing plays here").clicked() {
                        cx.out.push(SectionEdit { layers: Some(Vec::new()), ..Default::default() }.at(i));
                    }
                });
                ui.end_row();
            });
        });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let looping = cx.view.looping && cx.view.loop_section;
        if widgets::chip(ui, looping, "Loop this section", theme::ACCENT)
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
        ui.label(small("Add a section after it"));
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

/// The track inspector, for the track selected (a click on its row or one of its cells selects it): what plays it and
/// another sound, whether it plays in the section selected, whether it is heard, its level and pan, and where it is
/// edited.
pub fn track(ui: &mut Ui, cx: &mut Cx) {
    let Some(name) = cx.view.track.clone() else { return };
    let Some(song) = cx.state.song.as_mut() else { return };
    let sec = cx.view.section.min(song.sections.len().saturating_sub(1));
    let (section, plays) =
        song.sections.get(sec).map_or((String::new(), false), |s| (s.name.clone(), s.layers.contains(&name)));
    let editor = if song.drums.iter().any(|d| d.lane == name) {
        Some((Tab::Sequencer, "Its steps", "its pattern in the step sequencer, under Drums"))
    } else if song.parts.iter().any(|p| p.name == name) {
        Some((
            Tab::Parts,
            "Its sound and style",
            "how the theory engine writes it, and what plays it, under Instruments",
        ))
    } else {
        None
    };
    let color = super::layer_color(song, &name);
    // a track removed since it was selected lets go
    let Some(t) = song.tracks.iter_mut().find(|t| t.name == name) else {
        cx.view.track = None;
        return;
    };
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        widgets::heading(ui, "Track");
        ui.label(RichText::new(&name).strong().color(color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Cancel, 24.0, false, true, "let go of the track (Esc)").clicked() {
                cx.view.track = None;
            }
        });
    });
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(theme::alpha(color, 0.10))
        .stroke(egui::Stroke::new(1.0, theme::alpha(color, 0.45)))
        .corner_radius(6)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("track_grid").num_columns(2).spacing(vec2(10.0, 7.0)).show(ui, |ui| {
                widgets::label_help(
                    ui,
                    "Sound",
                    "The instrument, synth or recording this track plays. Another sound picks the next one that fits \
                     it and that no other track plays; its notes stay the same, and undo swaps back.",
                    "getting-started/studio.html#swapping-a-sound",
                );
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(&t.sound).color(TEXT_DIM)).wrap());
                    if let Some(next) = super::next_label(t)
                        && ui
                            .button(format!("Another sound: {next}"))
                            .on_hover_text("the next that fits it and no other part plays; undo swaps back")
                            .clicked()
                    {
                        cx.out.push(super::swap(&name, None));
                    }
                });
                ui.end_row();

                if !t.layers.is_empty() {
                    widgets::label_help(
                        ui,
                        "Layers",
                        "This track is played by several sounds at once, each in its own range of frequencies: a \
                         deep one for the low end, another for the body, another for the top, the way modern bass \
                         and chords are built. Each is levelled on its own, so none covers another.",
                        "language/sounds.html#layered-sounds",
                    );
                    ui.vertical(|ui| {
                        for layer in &t.layers {
                            ui.label(RichText::new(layer_words(layer)).size(11.5).color(TEXT_DIM));
                        }
                    });
                    ui.end_row();
                }

                ui.label(small(format!("In {section}")));
                let mut on = plays;
                let tip = "whether it plays in the section selected: the same as a double-click on its cell";
                if ui.checkbox(&mut on, "plays here").on_hover_text(tip).changed() {
                    cx.out.push(Command::ToggleLayer { section: sec, layer: name.clone() });
                }
                ui.end_row();

                ui.label(small("Heard"));
                ui.horizontal(|ui| {
                    if ui.selectable_label(t.muted, "Mute").on_hover_text("silence it everywhere").clicked() {
                        cx.out.push(MixEdit { muted: Some(!t.muted), ..Default::default() }.on(&name));
                    }
                    let tip = "only soloed tracks play while any track is soloed";
                    if ui.selectable_label(t.solo, "Solo").on_hover_text(tip).clicked() {
                        cx.out.push(MixEdit { solo: Some(!t.solo), ..Default::default() }.on(&name));
                    }
                });
                ui.end_row();

                widgets::label_help(
                    ui,
                    "Level",
                    "How loud this track is. 0 dB is the level claudius mixes a track of its kind at; below it is \
                     quieter, above it louder.",
                    "language/mixing.html#level",
                );
                let r = ui
                    // a track the mixer set quieter than the slider goes stays as it is until the slider is moved
                    .add(
                        egui::Slider::new(&mut t.level_db, -48.0..=12.0)
                            .clamping(egui::SliderClamping::Edits)
                            .suffix(" dB")
                            .fixed_decimals(1),
                    )
                    .on_hover_text("how loud it is against the level its role is mixed at (0)");
                if commit(&r) {
                    cx.out.push(MixEdit { level_db: Some(t.level_db), ..Default::default() }.on(&name));
                }
                ui.end_row();

                widgets::label_help(
                    ui,
                    "Pan",
                    "Where this track sits between your speakers: -1 is all the way left, 1 all the way right, 0 the \
                     middle. Spreading tracks apart makes room for each.",
                    "language/mixing.html#pan",
                );
                let r = ui
                    .add(egui::Slider::new(&mut t.pan, -1.0..=1.0).fixed_decimals(2))
                    .on_hover_text("where it sits from left (-1) to right (1)");
                if commit(&r) {
                    cx.out.push(MixEdit { pan: Some(t.pan), ..Default::default() }.on(&name));
                }
                ui.end_row();

                ui.label(small("Edit"));
                ui.horizontal_wrapped(|ui| {
                    if let Some((tab, label, tip)) = editor
                        && ui.button(label).on_hover_text(tip).clicked()
                    {
                        cx.view.tab = tab;
                    }
                    let tip = "its effects, sends and bus, in the signal flow";
                    if ui.button("Its effects").on_hover_text(tip).clicked() {
                        cx.view.tab = Tab::Graph;
                        cx.view.graph.selected = Some(format!("track:{name}"));
                    }
                });
                ui.end_row();
            });
        });
}

/// A layer of a layered sound (`TrackView::layers`) in a few words: its sound, the band it plays in, and what else it
/// does ("reese, 90 Hz to 1.2 kHz, -1 dB").
fn layer_words(layer: &serde_json::Value) -> String {
    let hz = |f: f64| if f >= 1000.0 { format!("{} kHz", (f / 100.0).round() / 10.0) } else { format!("{f:.0} Hz") };
    let mut words = vec![layer.get("sound").and_then(|s| s.as_str()).unwrap_or("a sound").replace('_', " ")];
    let band: Option<Vec<f64>> =
        layer.get("band").and_then(|b| b.as_array()).map(|b| b.iter().filter_map(|v| v.as_f64()).collect());
    if let Some([lo, hi]) = band.as_deref() {
        match (*lo > 0.0, *hi > 0.0) {
            (false, true) => words.push(format!("up to {}", hz(*hi))),
            (true, false) => words.push(format!("from {}", hz(*lo))),
            (true, true) => words.push(format!("{} to {}", hz(*lo), hz(*hi))),
            (false, false) => {}
        }
    }
    match layer.get("transpose").and_then(|v| v.as_i64()) {
        Some(12) => words.push("an octave up".into()),
        Some(-12) => words.push("an octave down".into()),
        Some(t) if t != 0 => words.push(format!("{t:+} semitones")),
        _ => {}
    }
    if let Some(db) = layer.get("level").and_then(|v| v.as_f64()).filter(|d| *d != 0.0) {
        words.push(format!("{db:+} dB"));
    }
    words.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_layer_says_its_sound_its_band_and_what_else_it_does() {
        let w = |j: serde_json::Value| layer_words(&j);
        assert_eq!(w(json!({"sound": "sub_bass", "band": [0, 90]})), "sub bass, up to 90 Hz");
        assert_eq!(w(json!({"sound": "reese", "band": [90, 1200], "level": -1})), "reese, 90 Hz to 1.2 kHz, -1 dB");
        assert_eq!(
            w(json!({"sound": "saw_lead", "band": [1200, 0], "transpose": 12, "level": -9})),
            "saw lead, from 1.2 kHz, an octave up, -9 dB"
        );
        assert_eq!(w(json!({"sound": "pad"})), "pad");
    }
}
