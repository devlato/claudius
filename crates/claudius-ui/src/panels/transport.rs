//! The top of the studio: transport and the LCD, the song's tempo / key / swing, the score, and the engine's
//! actions (new song, render, mutate, evolve, undo / redo, save, cancel) with the busy indicator.

use claudius_api::Command;
use eframe::egui::{self, Align, Align2, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::{Cx, small};
use crate::music::{self, Key};
use crate::theme::{self, ACCENT, LCD, LCD_TEXT, LINE, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets::{self, Icon, commit};

fn logo(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(118.0, 36.0), Sense::hover());
    let p = ui.painter();
    let heights = [0.35, 0.7, 1.0, 0.55, 0.8];
    for (i, h) in heights.iter().enumerate() {
        let x = rect.left() + 4.0 + i as f32 * 5.0;
        let bar = Rect::from_center_size(pos2(x, rect.center().y), vec2(3.0, 22.0 * h));
        p.rect_filled(bar, 1.5, theme::mix(ACCENT, theme::BAD, i as f32 / 8.0));
    }
    p.text(pos2(rect.left() + 32.0, rect.center().y - 1.0), Align2::LEFT_CENTER, "claudius", theme::prop(19.0), TEXT);
}

fn lcd(ui: &mut Ui, cx: &mut Cx) {
    let (rect, response) = ui.allocate_exact_size(vec2(268.0, 42.0), Sense::click());
    let p = ui.painter();
    p.rect(rect, 6.0, LCD, Stroke::new(1.0, theme::alpha(LCD_TEXT, 0.18)), StrokeKind::Inside);
    let has_audio = cx.player.has_audio();
    let pos = cx.player.position();
    let dur = if has_audio { cx.player.duration() } else { cx.state.song.as_ref().map_or(0.0, |s| s.seconds) };
    let time = super::fmt_time(pos);
    p.text(pos2(rect.left() + 12.0, rect.top() + 15.0), Align2::LEFT_CENTER, &time, theme::mono(19.0), LCD_TEXT);
    p.text(
        pos2(rect.left() + 112.0, rect.top() + 17.0),
        Align2::LEFT_CENTER,
        format!("/ {}", super::fmt_time(dur)),
        theme::mono(11.0),
        theme::alpha(LCD_TEXT, 0.55),
    );
    let bar = cx.playhead_bar();
    let (b, beat) = (bar.floor() as u32 + 1, ((bar.fract() * 4.0).floor() as u32) + 1);
    let section = cx
        .state
        .song
        .as_ref()
        .and_then(|s| s.sections.iter().find(|x| bar >= x.start_bar as f64 && bar < (x.start_bar + x.bars) as f64))
        .map(|s| s.name.clone());
    let line2 = format!(
        "BAR {b:03}.{beat}   {}",
        section.as_deref().unwrap_or(if has_audio { "" } else { "no render yet" }).to_uppercase()
    );
    p.text(
        pos2(rect.left() + 12.0, rect.bottom() - 11.0),
        Align2::LEFT_CENTER,
        line2,
        theme::mono(10.5),
        theme::alpha(LCD_TEXT, 0.75),
    );
    let status = if cx.player.is_loading() {
        "LOADING"
    } else if cx.player.is_playing() {
        "PLAY"
    } else if has_audio {
        "STOP"
    } else {
        "EMPTY"
    };
    p.text(
        pos2(rect.right() - 10.0, rect.top() + 12.0),
        Align2::RIGHT_CENTER,
        status,
        theme::mono(9.5),
        theme::alpha(LCD_TEXT, 0.6),
    );
    if cx.view.looping {
        p.text(
            pos2(rect.right() - 10.0, rect.bottom() - 11.0),
            Align2::RIGHT_CENTER,
            if cx.view.loop_section { "LOOP SEC" } else { "LOOP" },
            theme::mono(9.5),
            ACCENT,
        );
    }
    response.on_hover_text("position / length of the latest render, bar.beat and section at the playhead");
}

/// row 1: logo, transport, LCD, song parameters, score
pub fn top_row(ui: &mut Ui, cx: &mut Cx) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        logo(ui);
        ui.add_space(6.0);
        let has_audio = cx.player.has_audio();
        if widgets::icon_button(ui, Icon::ToStart, 30.0, false, has_audio, "to the start (Home)").clicked() {
            cx.player.seek(0.0);
        }
        let playing = cx.player.is_playing();
        let play_tip = if has_audio { "play / pause (Space)" } else { "render the song first" };
        if widgets::icon_button(ui, if playing { Icon::Pause } else { Icon::Play }, 34.0, playing, has_audio, play_tip)
            .clicked()
        {
            if playing { cx.player.pause() } else { cx.player.play() }
        }
        if widgets::icon_button(ui, Icon::Stop, 30.0, false, has_audio, "stop and rewind").clicked() {
            cx.player.stop();
        }
        let loop_tip = if cx.view.loop_section {
            "loop the selected section (L)"
        } else {
            "loop the song (L); right-click: loop the selected section"
        };
        let r = widgets::icon_button(ui, Icon::Loop, 30.0, cx.view.looping, true, loop_tip);
        if r.clicked() {
            cx.view.looping = !cx.view.looping;
        }
        r.context_menu(|ui| {
            if ui.radio(!cx.view.loop_section, "loop the whole song").clicked() {
                cx.view.loop_section = false;
                cx.view.looping = true;
                ui.close();
            }
            if ui.radio(cx.view.loop_section, "loop the selected section").clicked() {
                cx.view.loop_section = true;
                cx.view.looping = true;
                ui.close();
            }
        });
        ui.add_space(4.0);
        lcd(ui, cx);
        ui.add_space(8.0);
        song_params(ui, cx);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let total = cx.state.verdict.as_ref().map(|v| v.total);
            let stale = cx.render_stale();
            let tip =
                cx.state.verdict.as_ref().map_or("no verdict yet: render the song".to_string(), |v| v.summary.clone());
            widgets::score_badge(ui, total, 36.0).on_hover_text(if stale {
                format!("{tip}\n\n(the song changed since this render)")
            } else {
                tip
            });
            ui.label(small(if stale { "score (old)" } else { "score" }));
        });
    });
}

fn song_params(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.as_mut() else {
        ui.label(RichText::new("no song yet").color(TEXT_FAINT));
        return;
    };
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.add_space(3.0);
        ui.label(RichText::new(&song.title).size(15.0).strong().color(TEXT));
        ui.horizontal(|ui| {
            widgets::pill(ui, &song.genre.replace('_', " "), theme::section_color("drop"));
            ui.label(small(format!(
                "seed {} · {} bars · {}",
                song.seed,
                song.bars,
                crate::mock::fmt_time(song.seconds)
            )));
        });
    });
    ui.add_space(10.0);
    ui.separator();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(small("BPM"));
        let r = ui
            .add(
                egui::DragValue::new(&mut song.bpm)
                    .range(40.0..=240.0)
                    .clamp_existing_to_range(false)
                    .speed(0.2)
                    .fixed_decimals(1)
                    .update_while_editing(false),
            )
            .on_hover_text("tempo (drag or type)");
        if commit(&r) {
            cx.out.push(Command::SetTempo { bpm: song.bpm });
        }
    });
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(small("KEY"));
        let key = Key::parse(&song.key).unwrap_or(Key { tonic: 9, mode: 1 });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut tonic = key.tonic;
            egui::ComboBox::from_id_salt("key_tonic").width(46.0).selected_text(key.pc_name(key.tonic)).show_ui(
                ui,
                |ui| {
                    for name in music::TONICS {
                        let pc = music::parse_pc(name).unwrap_or(0);
                        ui.selectable_value(&mut tonic, pc, name);
                    }
                },
            );
            let mut mode = key.mode;
            egui::ComboBox::from_id_salt("key_mode")
                .width(92.0)
                .selected_text(key.mode_name().replace('_', " "))
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in music::MODES.iter().enumerate() {
                        ui.selectable_value(&mut mode, i, name.replace('_', " "));
                    }
                });
            if tonic != key.tonic || mode != key.mode {
                let k = Key { tonic, mode };
                song.key = k.name();
                cx.out.push(Command::SetKey { key: k.name() });
            }
        });
    });
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(small("SWING"));
        let r = ui
            .add(
                egui::DragValue::new(&mut song.swing)
                    .range(0.5..=0.8)
                    .clamp_existing_to_range(false)
                    .speed(0.002)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|s| {
                        s.trim()
                            .trim_end_matches('%')
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .map(|v| if v > 1.0 { v / 100.0 } else { v })
                    })
                    .update_while_editing(false),
            )
            .on_hover_text("swing of the off 16ths: 50% straight, 58% shuffled");
        if commit(&r) {
            cx.out.push(Command::SetSwing { swing: song.swing });
        }
    });
}

/// row 2: the engine's actions and the busy indicator
pub fn toolbar(ui: &mut Ui, cx: &mut Cx) {
    let busy = cx.state.busy.is_some();
    let has_song = cx.state.song.is_some();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if widgets::tool_button(ui, Icon::New, "New song", true, false).on_hover_text("generate a new song from a genre").clicked() {
            cx.view.new_song.open = true;
            if let Some(song) = &cx.state.song {
                cx.view.new_song.genre = song.genre.clone();
            }
        }
        let open = widgets::icon_button(ui, Icon::Open, 28.0, false, true, "open a blueprint.json or a script file (a path on the server's machine)");
        egui::Popup::from_toggle_button_response(&open).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(380.0);
            ui.label(RichText::new("Open a song").strong());
            ui.label(small("a blueprint.json, a song folder's script, or any claudius script, as a path on the server's machine"));
            let r = ui.add(egui::TextEdit::singleline(&mut cx.view.open_path).hint_text("out/house-glass-garden-7/blueprint.json").desired_width(f32::INFINITY));
            let go = ui.add_enabled(!cx.view.open_path.trim().is_empty(), egui::Button::new("Open")).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if go && !cx.view.open_path.trim().is_empty() {
                cx.out.push(Command::Load { path: cx.view.open_path.trim().to_string() });
                ui.close();
            }
        });
        ui.separator();
        let busy_tip = format!("busy: {}", cx.state.busy.clone().unwrap_or_default());
        let r = widgets::tool_button(ui, Icon::Render, "Render", has_song, !busy)
            .on_hover_text(if busy { "render (again) when the current job is done; with stems: one file per track too" } else { "render, mix and master the song, then judge it (the server also renders after every change)" });
        if r.clicked() {
            cx.out.push(Command::Render { stems: cx.view.stems });
        }
        ui.checkbox(&mut cx.view.stems, small("stems")).on_hover_text("also write one file per track");
        if let Some(r) = cx.state.render.as_ref().filter(|r| !r.stems.is_empty()) {
            let version = r.version;
            ui.menu_button(small(format!("{} stems", r.stems.len())), |ui| {
                ui.label(small(format!("the tracks of render {version}, as 24-bit WAV")));
                for stem in &r.stems {
                    let url = cx.server.map_or_else(|| format!("/api/stems/{version}/{stem}.wav"), |base| format!("{base}/api/stems/{version}/{stem}.wav"));
                    ui.hyperlink_to(stem, url);
                }
            })
            .response
            .on_hover_text("download one file per track");
        }
        ui.separator();
        if widgets::tool_button(ui, Icon::Mutate, "Mutate", has_song && !busy, false)
            .on_hover_text("try random mutations, listen to each, keep the best")
            .on_disabled_hover_text(&busy_tip)
            .clicked()
        {
            cx.out.push(Command::Mutate { count: cx.view.mutate_count, keep_best: cx.view.keep_best });
        }
        ui.add(egui::DragValue::new(&mut cx.view.mutate_count).range(1..=32).prefix("×")).on_hover_text("candidates");
        ui.checkbox(&mut cx.view.keep_best, small("keep best"));
        ui.separator();
        if widgets::tool_button(ui, Icon::Evolve, "Evolve", has_song && !busy, false)
            .on_hover_text("several generations of mutation + listening")
            .on_disabled_hover_text(&busy_tip)
            .clicked()
        {
            cx.out.push(Command::Evolve { generations: cx.view.generations, children: cx.view.children });
        }
        ui.add(egui::DragValue::new(&mut cx.view.generations).range(1..=50).suffix(" gen")).on_hover_text("generations");
        ui.add(egui::DragValue::new(&mut cx.view.children).range(1..=32).suffix(" kids")).on_hover_text("children per generation");
        ui.separator();
        let cmd = widgets::cmd_key(ui.ctx());
        let undo_tip = match &cx.state.undo_label {
            Some(l) => format!("undo: {l} ({cmd}+Z)"),
            None => format!("nothing to undo ({cmd}+Z)"),
        };
        if widgets::icon_button(ui, Icon::Undo, 28.0, false, cx.state.can_undo, &undo_tip).clicked() {
            cx.out.push(Command::Undo);
        }
        let redo_tip = match &cx.state.redo_label {
            Some(l) => format!("redo: {l} (Shift+{cmd}+Z)"),
            None => format!("nothing to redo (Shift+{cmd}+Z)"),
        };
        if widgets::icon_button(ui, Icon::Redo, 28.0, false, cx.state.can_redo, &redo_tip).clicked() {
            cx.out.push(Command::Redo);
        }
        if widgets::icon_button(ui, Icon::Save, 28.0, false, has_song, "save blueprint, audio and script into a new folder under out/").clicked() {
            cx.out.push(Command::Save { path: None });
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(job) = cx.state.busy.clone() {
                if widgets::icon_button(ui, Icon::Cancel, 26.0, false, true, "cancel the running job").clicked() {
                    cx.out.push(Command::Cancel);
                }
                if let Some(p) = cx.state.progress {
                    ui.add(egui::ProgressBar::new(p.clamp(0.0, 1.0)).desired_width(150.0).desired_height(10.0).fill(ACCENT).text(RichText::new(format!("{:.0}%", p * 100.0)).size(10.0)));
                }
                ui.label(RichText::new(job).color(theme::mix(ACCENT, TEXT, 0.4)));
                ui.add(egui::Spinner::new().size(16.0).color(ACCENT));
            } else if let Some(t) = &cx.view.toast
                && cx.now - t.at < 6.0
            {
                let color = if t.ok { TEXT_DIM } else { theme::BAD };
                let text: String = t.text.chars().take(120).collect();
                ui.label(RichText::new(text).color(color)).on_hover_text(&t.text);
                widgets::dot(ui, if t.ok { theme::GOOD } else { theme::BAD });
            }
        });
    });
}

/// the "New song" dialog
pub fn new_song_dialog(ctx: &egui::Context, cx: &mut Cx) {
    if !cx.view.new_song.open {
        return;
    }
    let modal = egui::Modal::new(egui::Id::unique("new_song")).show(ctx, |ui| {
        ui.set_width(620.0);
        ui.horizontal(|ui| {
            widgets::paint_icon(
                ui.painter(),
                Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(18.0, 18.0)),
                Icon::New,
                ACCENT,
            );
            ui.add_space(24.0);
            ui.heading("New song");
        });
        ui.label(small(
            "claudius writes a whole song from the genre's production knowledge: form, drums, parts, sounds and mix.",
        ));
        ui.add_space(8.0);
        let f = &mut cx.view.new_song;
        let genres = &cx.state.genres;
        if genres.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Genre");
                ui.text_edit_singleline(&mut f.genre);
            });
        } else {
            egui::Grid::new("genres").num_columns(2).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                for (i, g) in genres.iter().enumerate() {
                    let selected = f.genre == g.name;
                    let color = theme::section_color(
                        ["drop", "groove", "break", "build", "intro", "hook", "verse", "outro"][i % 8],
                    );
                    let (rect, r) = ui.allocate_exact_size(vec2(300.0, 64.0), Sense::click());
                    let fill = if selected {
                        theme::alpha(color, 0.22)
                    } else if r.hovered() {
                        theme::RAISED_HI
                    } else {
                        theme::RAISED
                    };
                    let stroke = if selected { Stroke::new(1.5, color) } else { Stroke::new(1.0, LINE) };
                    ui.painter().rect(rect, 6.0, fill, stroke, StrokeKind::Inside);
                    ui.painter().rect_filled(
                        Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(3.0, rect.height() - 16.0)),
                        1.0,
                        color,
                    );
                    ui.painter().text(
                        rect.min + vec2(12.0, 14.0),
                        Align2::LEFT_CENTER,
                        g.name.replace('_', " "),
                        theme::prop(14.0),
                        TEXT,
                    );
                    ui.painter().text(
                        rect.right_top() + vec2(-10.0, 14.0),
                        Align2::RIGHT_CENTER,
                        format!("{:.0}-{:.0} BPM", g.bpm.0, g.bpm.1),
                        theme::prop(11.0),
                        TEXT_DIM,
                    );
                    let desc = g.description.split_once(": ").map_or(g.description.as_str(), |(_, d)| d);
                    let galley =
                        ui.painter().layout(desc.to_string(), theme::prop(10.5), TEXT_DIM, rect.width() - 22.0);
                    let clip = Rect::from_min_max(rect.min + vec2(12.0, 26.0), rect.max - vec2(8.0, 4.0));
                    ui.painter().with_clip_rect(clip).galley(clip.min, galley, TEXT_DIM);
                    if r.on_hover_text(&g.description).clicked() {
                        f.genre = g.name.clone();
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        }
        ui.add_space(10.0);
        egui::Grid::new("new_song_opts").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
            ui.label("Seed");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut f.seed).desired_width(110.0).hint_text("random"));
                if widgets::icon_button(ui, Icon::Dice, 24.0, false, true, "a random seed").clicked() {
                    let t = (cx.now * 1000.0) as u64;
                    f.seed = ((t.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33) % 10_000)
                        .to_string();
                }
                ui.label(small("the same genre + seed makes the same song"));
            });
            ui.end_row();
            ui.label("Tempo");
            ui.horizontal(|ui| {
                let mut auto = f.bpm.is_none();
                if ui.checkbox(&mut auto, "from the genre").changed() {
                    f.bpm = if auto {
                        None
                    } else {
                        Some(
                            genres
                                .iter()
                                .find(|g| g.name == f.genre)
                                .map_or(124.0, |g| ((g.bpm.0 + g.bpm.1) / 2.0).round()),
                        )
                    };
                }
                if let Some(bpm) = &mut f.bpm {
                    ui.add(egui::DragValue::new(bpm).range(40.0..=240.0).speed(0.5).suffix(" BPM"));
                }
            });
            ui.end_row();
            ui.label("Key");
            ui.horizontal(|ui| {
                let mut auto = f.key.is_none();
                if ui.checkbox(&mut auto, "from the genre").changed() {
                    f.key = if auto { None } else { Some("A minor".into()) };
                }
                if let Some(key) = &mut f.key {
                    let k = Key::parse(key).unwrap_or(Key { tonic: 9, mode: 1 });
                    let (mut tonic, mut mode) = (k.tonic, k.mode);
                    egui::ComboBox::from_id_salt("ns_tonic").width(46.0).selected_text(k.pc_name(k.tonic)).show_ui(
                        ui,
                        |ui| {
                            for name in music::TONICS {
                                ui.selectable_value(&mut tonic, music::parse_pc(name).unwrap_or(0), name);
                            }
                        },
                    );
                    egui::ComboBox::from_id_salt("ns_mode")
                        .width(110.0)
                        .selected_text(k.mode_name().replace('_', " "))
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in music::MODES.iter().enumerate() {
                                ui.selectable_value(&mut mode, i, name.replace('_', " "));
                            }
                        });
                    *key = Key { tonic, mode }.name();
                }
            });
            ui.end_row();
            ui.label("Loops");
            ui.checkbox(&mut f.loops, "build it around an Apple Loops family (GarageBand style)");
            ui.end_row();
        });
        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new("Or start from an audio file").strong());
        ui.label(small(format!(
            "claudius reads its tempo, key, chords, form and groove; Remake writes a new {} song on that skeleton",
            f.genre.replace('_', " ")
        )));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut cx.view.audio_path)
                    .hint_text("a path on the server's machine: ~/Music/some song.mp3")
                    .desired_width(360.0),
            );
            let path = cx.view.audio_path.trim().to_string();
            let ready = !path.is_empty() && !cx.view.analyzing;
            if ui
                .add_enabled(ready, egui::Button::new("Analyze"))
                .on_hover_text("read the file and show what claudius hears")
                .clicked()
            {
                cx.view.analysis = None;
                cx.out.push(Command::Analyze { path: path.clone() });
            }
            if ui
                .add_enabled(ready, egui::Button::new(format!("Remake as {}", f.genre.replace('_', " "))))
                .on_hover_text("a new song on the file's skeleton (the seed above, if any)")
                .clicked()
            {
                cx.view.analysis = None;
                cx.out.push(Command::Remake {
                    path,
                    genre: Some(f.genre.clone()),
                    seed: f.seed.trim().parse().ok(),
                    bpm: None,
                });
            }
            if cx.view.analyzing {
                ui.add(egui::Spinner::new().color(ACCENT));
                ui.label(small("listening to the file..."));
            }
        });
        if let Some((ok, text)) = &cx.view.analysis {
            let c = if *ok { theme::GOOD } else { theme::BAD };
            egui::Frame::new()
                .fill(theme::WELL)
                .stroke(Stroke::new(1.0, theme::alpha(c, 0.5)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().id_salt("analysis").max_height(190.0).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(text).monospace().size(11.5).color(if *ok {
                                TEXT
                            } else {
                                theme::mix(c, TEXT, 0.4)
                            }))
                            .wrap(),
                        );
                    });
                });
        }
        ui.add_space(12.0);
        let mut create = false;
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let create_btn =
                    egui::Button::new(RichText::new("  Create song  ").strong().color(egui::Color32::BLACK))
                        .fill(ACCENT);
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ui.ctx().text_edit_focused();
                if ui.add(create_btn).clicked() || enter {
                    create = true;
                }
                if ui.button("Cancel").clicked() {
                    f.open = false;
                }
            });
        });
        if create {
            cx.out.push(Command::NewSong {
                genre: f.genre.clone(),
                seed: f.seed.trim().parse().ok(),
                bpm: f.bpm,
                key: f.key.clone(),
                loops: f.loops,
            });
            f.open = false;
        }
    });
    if modal.should_close() {
        cx.view.new_song.open = false;
    }
}
