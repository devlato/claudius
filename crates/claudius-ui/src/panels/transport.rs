//! The top of the studio: transport and the LCD, the song's tempo / key / swing, the score, and the engine's
//! actions (new song, open, save, variations, improve, sounds, export, undo / redo) with what claudius is doing.

use claudius_api::{Command, GenreInfo, GenreSampling, State};
use eframe::egui::{self, Align, Align2, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::{Cx, View, WithRecording, small};
use crate::music::{self, Key};
use crate::theme::{self, ACCENT, LCD, LCD_TEXT, LINE, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets::{self, Icon, commit};

fn logo(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(122.0, 36.0), Sense::hover());
    let mark = Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - 12.0), crate::mark::size(24.0));
    crate::mark::paint(ui, mark);
    let name = pos2(mark.right() + 9.0, rect.center().y - 1.0);
    ui.painter().text(name, Align2::LEFT_CENTER, "Claudius", theme::prop(19.0), TEXT);
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
        // while a take records, the recorder plays the song: the player waits
        let has_audio = cx.player.has_audio() && !cx.view.record.recording();
        if widgets::icon_button(ui, Icon::ToStart, 30.0, false, has_audio, "to the start (Home)").clicked() {
            cx.player.seek(0.0);
        }
        let playing = cx.player.is_playing();
        let play_tip = if cx.view.record.recording() {
            "a take is recording: Space or the record button ends it"
        } else if has_audio {
            "play / pause (Space)"
        } else {
            "render the song first"
        };
        let play = widgets::icon_button(
            ui,
            if playing { Icon::Pause } else { Icon::Play },
            34.0,
            playing,
            has_audio,
            play_tip,
        );
        cx.view.anchors.play = Some(play.rect);
        if play.clicked() {
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
        let recording = cx.view.record.recording();
        let tip = if recording { "stop recording" } else { "record a take: a voice or an instrument" };
        let r = widgets::icon_button(ui, Icon::Record, 30.0, recording, cx.state.song.is_some(), tip);
        if r.clicked() {
            if recording {
                super::record::finish(cx);
            } else {
                cx.view.record.show();
            }
        }
        let tip =
            "add a recording of your own, whole as a track or with its vocals sampled (or drop one on the studio)";
        let open = cx.view.import.open;
        if widgets::icon_button(ui, Icon::Plus, 30.0, open, cx.state.song.is_some(), tip).clicked() {
            cx.view.import.open = !open;
        }
        // the demo in a browser plays along with a phone through the site's relay
        if cx.server.is_some() || cfg!(target_arch = "wasm32") {
            let served = cx.state.phone.is_some();
            let tip = if served {
                "a phone is playing along: show its code"
            } else {
                "play from a phone: pads, keys and a turntable with the song's sounds"
            };
            if widgets::icon_button(ui, Icon::Phone, 30.0, served, true, tip).clicked() {
                cx.view.phone.show(served);
            }
        }
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
            ui.label(small(if stale { "Previous score" } else { "Score" }));
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
        ui.label(RichText::new(&song.title).size(15.5).strong().color(TEXT));
        ui.horizontal(|ui| {
            widgets::pill(ui, &song.genre.replace('_', " "), LCD_TEXT);
            ui.label(small(format!("{} bars · {}", song.bars, crate::mock::fmt_time(song.seconds))))
                .on_hover_text(format!("seed {}: the same genre and seed make the same song", song.seed));
        });
    });
    ui.add_space(10.0);
    ui.separator();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(small("Tempo"));
        let r = ui
            .add(
                egui::DragValue::new(&mut song.bpm)
                    .range(40.0..=240.0)
                    .clamp_existing_to_range(false)
                    .speed(0.2)
                    .fixed_decimals(1)
                    .suffix(" BPM")
                    .update_while_editing(false),
            )
            .on_hover_text("beats a minute (drag or type)");
        if commit(&r) {
            cx.out.push(Command::SetTempo { bpm: song.bpm });
        }
    });
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(small("Key"));
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
        ui.label(small("Swing"));
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
/// the toolbar's swaps of the whole song's sounds: every part's and the drums' kit, the parts', or the kit
fn swap_menu(ui: &mut Ui, song: Option<&claudius_api::SongView>, out: &mut Vec<Command>) {
    let Some(song) = song else { return };
    let whole = |only: &str, to: Option<&str>| Command::SwapSound {
        track: None,
        to: to.map(str::to_string),
        only: (!only.is_empty()).then(|| only.to_string()),
    };
    ui.label(small("every note, pattern and mix setting stays; undo swaps back"));
    if ui
        .button("Swap every sound")
        .on_hover_text("each part onto its next sound, the drums onto the next kit")
        .clicked()
    {
        out.push(whole("", None));
        ui.close();
    }
    if !song.parts.is_empty()
        && ui.button("Swap the parts' sounds").on_hover_text("each part onto the next sound that fits it").clicked()
    {
        out.push(whole("parts", None));
        ui.close();
    }
    if song.kits.is_empty() {
        return;
    }
    ui.separator();
    if let Some(next) = &song.next_kit
        && ui.button(format!("Another drum kit: {next}")).clicked()
    {
        out.push(whole("drums", None));
        ui.close();
    }
    ui.menu_button("Put the drums on", |ui| {
        for k in &song.kits {
            let on = song.kit.as_ref() == Some(k);
            if ui.selectable_label(on, k).clicked() && !on {
                out.push(whole("drums", Some(k)));
                ui.close();
            }
        }
    });
}

pub fn toolbar(ui: &mut Ui, cx: &mut Cx) {
    let has_song = cx.state.song.is_some();
    let demo = cx.server.is_none();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let new_song = widgets::tool_button(ui, Icon::New, "New song", true, false)
            .on_hover_text("make a new song from a genre, or from a recording");
        cx.view.anchors.new_song = Some(new_song.rect);
        if new_song.clicked() {
            cx.view.new_song.open = true;
            if let Some(song) = &cx.state.song {
                cx.view.new_song.set_genre(&song.genre);
            }
        }
        let open = widgets::tool_button(ui, Icon::Open, "Open", true, false)
            .on_hover_text("open a song: a blueprint.json or a script file");
        egui::Popup::from_toggle_button_response(&open)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(320.0);
                ui.label(RichText::new("Open a song").strong());
                ui.label(small("a blueprint.json from a song's folder, or a claudius script"));
                ui.add_space(4.0);
                if widgets::tool_button(ui, Icon::Open, "Choose a file...", true, false).clicked() {
                    crate::pick::choose(ui.ctx(), crate::pick::Want::Song);
                    ui.close();
                }
            });
        let page = cfg!(target_arch = "wasm32");
        let (save, save_tip) = if demo {
            (
                "Download",
                "download the song as a FLAC file, lossless at about half a WAV's size (Export has MP3, Opus and WAV)",
            )
        } else if page {
            (
                "Save",
                "download the song, and save it in a folder of its own on the computer claudius runs on: blueprint, \
                 script, audio, stems and verdict",
            )
        } else {
            (
                "Save",
                "save the song in a folder of its own under ~/Music/claudius (blueprint, script, audio, stems and \
                 verdict), and show the folder",
            )
        };
        if widgets::tool_button(ui, Icon::Save, save, has_song, false).on_hover_text(save_tip).clicked() {
            ask_download(cx.state, cx.view, cx.out, demo, cx.now, None);
        }
        ui.separator();
        let tries = widgets::menu_button(ui, Icon::Mutate, "Variations", has_song)
            .on_hover_text("try versions of the song with a change or two each, and keep the one that sounds best");
        egui::Popup::from_toggle_button_response(&tries)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| variations_menu(ui, cx));
        let improve = widgets::menu_button(ui, Icon::Improve, "Improve", has_song)
            .on_hover_text("let claudius improve the song by itself, over rounds of variations");
        egui::Popup::from_toggle_button_response(&improve)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| improve_menu(ui, cx));
        let sounds = widgets::menu_button(ui, Icon::Swap, "Sounds", has_song)
            .on_hover_text("swap the song's sounds for others that fit: every part's, the drums' kit, or both");
        egui::Popup::menu(&sounds).show(|ui| swap_menu(ui, cx.state.song.as_ref(), cx.out));
        ui.separator();
        let export = widgets::menu_button(ui, Icon::Export, "Export", has_song).on_hover_text(
            "the song as a file (WAV, FLAC, MP3 and more), each track on its own, or ready for the stores",
        );
        egui::Popup::from_toggle_button_response(&export)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| export_menu(ui, cx));
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

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            status(ui, cx);
            if let Some(t) = &cx.view.toast
                && cx.now - t.at < 6.0
            {
                let color = if t.ok { TEXT_DIM } else { theme::BAD };
                // what came of it is its first line ("sampled 2 phrases of ..."); the rest is on hover and in the log
                let text: String = t.text.lines().next().unwrap_or_default().chars().take(120).collect();
                // what the label leaves for the dot and the gap to the buttons on the left
                let room = ui.available_width() - 10.0 - 2.0 * ui.spacing().item_spacing.x;
                ui.allocate_ui(vec2(room.max(0.0), ui.available_height()), |ui| {
                    ui.add(egui::Label::new(RichText::new(text).color(color)).truncate()).on_hover_text(&t.text);
                });
                widgets::dot(ui, if t.ok { theme::GOOD } else { theme::BAD });
            }
        });
    });
}

/// what a job in progress says of itself, as a sentence starts ("Rendering", "Mutating: judging 4 variations")
fn sentence(job: &str) -> String {
    let mut c = job.chars();
    c.next().map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
}

/// the toolbar's right end: what claudius is doing, or whether what plays is the song as it is now
fn status(ui: &mut Ui, cx: &mut Cx) {
    if let Some(job) = cx.state.busy.clone() {
        // a step of listening to a recording runs to its end: only a job is offered to be cancelled
        if cx.state.cancellable
            && widgets::icon_button(ui, Icon::Cancel, 26.0, false, true, "cancel the running job").clicked()
        {
            cx.out.push(Command::Cancel);
        }
        if let Some(p) = cx.state.progress {
            ui.add(
                egui::ProgressBar::new(p.clamp(0.0, 1.0))
                    .desired_width(140.0)
                    .desired_height(12.0)
                    .fill(ACCENT)
                    .text(RichText::new(format!("{:.0}%", p * 100.0)).size(10.5)),
            );
        }
        ui.label(RichText::new(sentence(&job)).color(TEXT));
        ui.add(egui::Spinner::new().size(16.0).color(ACCENT));
    } else if cx.state.song.is_some() && (cx.state.render.is_none() || cx.render_stale()) {
        let r = widgets::tool_button(ui, Icon::Render, "Render", true, true)
            .on_hover_text("render, mix and master the song as it is now, then judge it");
        if r.clicked() {
            cx.out.push(Command::Render { stems: false });
        }
        let what = if cx.state.render.is_none() { "Not rendered yet" } else { "Changes not rendered yet" };
        ui.label(RichText::new(what).color(theme::WARN))
            .on_hover_text("what plays is from before the latest changes, and so is the score");
    } else if cx.state.song.is_some() {
        ui.label(RichText::new("Up to date").color(TEXT_DIM))
            .on_hover_text("what plays, and the score, are of the song as it is now");
        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        widgets::paint_icon(ui.painter(), rect, Icon::Done, theme::GOOD);
    }
    ui.add_space(12.0);
}

/// the Variations menu: how many versions of the song to try, and whether the best is kept
fn variations_menu(ui: &mut Ui, cx: &mut Cx) {
    ui.set_width(320.0);
    ui.label(RichText::new("Try variations").strong());
    ui.label(small(
        "claudius makes versions of the song with a change or two each (a part written again, another sound, a \
         section reworked), and listens to every one",
    ));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("How many");
        ui.add(egui::DragValue::new(&mut cx.view.mutate_count).range(1..=32));
    });
    ui.checkbox(&mut cx.view.keep_best, "Keep the best, if it beats the song")
        .on_hover_text("otherwise each one's score goes to the Log, and the song stays as it is");
    ui.add_space(6.0);
    let busy = cx.state.busy.is_some();
    let label = format!("Try {}", cx.view.mutate_count);
    let go = widgets::tool_button(ui, Icon::Mutate, &label, !busy, true);
    if busy {
        go.on_hover_text("wait for what claudius is doing to finish");
    } else if go.clicked() {
        cx.out.push(Command::Mutate { count: cx.view.mutate_count, keep_best: cx.view.keep_best });
        ui.close();
    }
}

/// the Improve menu: rounds of variations, each from the best so far
fn improve_menu(ui: &mut Ui, cx: &mut Cx) {
    ui.set_width(320.0);
    ui.label(RichText::new("Improve the song").strong());
    ui.label(small(
        "round after round, claudius tries variations of the best version so far, steered by what the critic \
         hears, and keeps whichever sounds best. Undo goes back to the song before.",
    ));
    ui.add_space(6.0);
    egui::Grid::new("improve").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.label("Rounds");
        ui.add(egui::DragValue::new(&mut cx.view.generations).range(1..=20));
        ui.end_row();
        ui.label("Variations a round");
        ui.add(egui::DragValue::new(&mut cx.view.children).range(2..=32));
        ui.end_row();
    });
    ui.add_space(6.0);
    let busy = cx.state.busy.is_some();
    let go = widgets::tool_button(ui, Icon::Improve, "Improve", !busy, true);
    if busy {
        go.on_hover_text("wait for what claudius is doing to finish");
    } else if go.clicked() {
        cx.out.push(Command::Evolve { generations: cx.view.generations, children: cx.view.children });
        ui.close();
    }
}

/// the Export menu: the song in a format, its tracks one file each, and the files a distributor takes
fn export_menu(ui: &mut Ui, cx: &mut Cx) {
    ui.set_width(380.0);
    // the demo has no server to fetch a file from: its engine makes the one asked for
    let demo = cx.server.is_none();
    let name = cx.state.song.as_ref().map_or_else(|| "song".to_string(), |s| file_name(&s.title));
    let mut asked = None;
    ui.label(RichText::new("The song").strong());
    match cx.state.render.as_ref() {
        None => {
            ui.label(small("render the song first"));
        }
        Some(r) => {
            let version = r.version;
            if version < cx.state.song_version && !demo {
                ui.label(small(format!("render {version}, from before the latest change")));
            }
            for (lossless, heading) in [(true, "lossless"), (false, "compressed")] {
                if !cx.state.exports.iter().any(|f| f.lossless == lossless) {
                    continue;
                }
                ui.label(small(heading));
                ui.horizontal_wrapped(|ui| {
                    for f in cx.state.exports.iter().filter(|f| f.lossless == lossless) {
                        let path = format!("/api/export/{version}/{name}.{}?format={}", f.extension, f.spec);
                        let url = cx.server.map_or_else(|| path.clone(), |base| format!("{base}{path}"));
                        match &f.unavailable {
                            None if demo => {
                                if ui.button(&f.label).clicked() {
                                    asked = Some(f.spec.clone());
                                    ui.close();
                                }
                            }
                            None => {
                                ui.hyperlink_to(&f.label, url);
                            }
                            Some(why) => {
                                ui.add_enabled(false, egui::Label::new(&f.label)).on_disabled_hover_text(why);
                            }
                        }
                    }
                });
            }
        }
    }
    ui.add_space(4.0);
    ui.separator();
    ui.label(RichText::new("Each track on its own").strong());
    match cx.state.render.as_ref().filter(|r| !r.stems.is_empty()) {
        Some(r) => {
            let version = r.version;
            ui.label(small(format!("the tracks of render {version}, as 24-bit WAV")));
            ui.horizontal_wrapped(|ui| {
                for stem in &r.stems {
                    let url = cx.server.map_or_else(
                        || format!("/api/stems/{version}/{stem}.wav"),
                        |base| format!("{base}/api/stems/{version}/{stem}.wav"),
                    );
                    ui.hyperlink_to(stem, url);
                }
            });
        }
        None => {
            ui.label(small("one 24-bit WAV a track (stems), for mixing the song in another studio"));
            let busy = cx.state.busy.is_some();
            if ui.add_enabled(!busy, egui::Button::new("Render the tracks")).clicked() {
                cx.out.push(Command::Render { stems: true });
                ui.close();
            }
        }
    }
    if demo {
        ui.add_space(4.0);
        ui.separator();
        ui.label(RichText::new("The project").strong());
        let project = ui.button("claudius project (.zip)").on_hover_text(
            "the song itself, the samples it plays and where each came from: open it in claudius to carry on with it",
        );
        if project.clicked() {
            asked = Some(crate::mock::PROJECT.to_string());
            ui.close();
        }
    }
    ui.add_space(4.0);
    ui.separator();
    if ui
        .button("Publish…")
        .on_hover_text("check the song as stores do, and write the files and answers a distributor takes")
        .clicked()
    {
        cx.view.publish.show();
        ui.close();
    }
    if asked.is_some() {
        ask_download(cx.state, cx.view, cx.out, demo, cx.now, asked);
    }
}

/// the "New song" dialog
pub fn new_song_dialog(ctx: &egui::Context, cx: &mut Cx) {
    if !cx.view.new_song.open {
        return;
    }
    let modal = egui::Modal::new(egui::Id::unique("new_song")).show(ctx, |ui| {
        ui.set_width(640.0);
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
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let f = &mut cx.view.new_song;
            for (recording, label, tip) in [
                (
                    false,
                    "From a genre",
                    "claudius writes a whole song in a genre: its form, drums, parts, sounds and mix",
                ),
                (true, "From a recording", "claudius listens to a song or a sound of yours, and makes a song of it"),
            ] {
                if widgets::chip(ui, f.recording == recording, label, ACCENT).on_hover_text(tip).clicked() {
                    f.recording = recording;
                }
            }
        });
        ui.add_space(8.0);
        if cx.view.new_song.recording {
            from_recording(ui, cx);
        } else {
            from_genre(ui, cx);
        }
    });
    if modal.should_close() {
        cx.view.new_song.open = false;
    }
}

/// the New song dialog from a genre: the cards to choose it from, what else to ask of the song, and Create song
fn from_genre(ui: &mut Ui, cx: &mut Cx) {
    ui.label(small(
        "claudius writes a whole song from the genre's production knowledge: form, drums, parts, sounds and mix.",
    ));
    ui.add_space(6.0);
    let f = &mut cx.view.new_song;
    let genres = &cx.state.genres;
    if genres.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Genre");
            ui.text_edit_singleline(&mut f.genre);
        });
    } else {
        let desc = |g: &GenreInfo| g.description.split_once(": ").map_or(g.description.clone(), |(_, d)| d.to_string());
        // every card as tall as the longest description
        let text_height =
            genres.iter().map(|g| two_lines(ui.painter(), desc(g), 300.0 - 22.0).size().y).fold(0.0, f32::max);
        ui.add(
            egui::TextEdit::singleline(&mut f.genre_search)
                .hint_text("Search genres: rap, 90s, west coast, r&b, dusty …")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(4.0);
        let query = f.genre_search.trim().to_string();
        // all of them by family (a genre of one's own, with none, last), or the search's answers, best first
        let groups: Vec<(&str, Vec<&GenreInfo>)> = if query.is_empty() {
            let mut families: Vec<&str> = genres.iter().map(|g| g.family.as_str()).collect();
            families.sort_by_key(|f| (f.is_empty(), *f));
            families.dedup();
            families.into_iter().map(|f| (f, genres.iter().filter(|g| g.family == f).collect())).collect()
        } else {
            vec![("", claudius_api::search_genres(genres, &query))]
        };
        egui::ScrollArea::vertical().id_salt("genre_cards").max_height(330.0).show(ui, |ui| {
            if groups.iter().all(|(_, of)| of.is_empty()) {
                ui.label(small(format!("No genre answers “{query}”.")));
            }
            for (family, of) in &groups {
                if query.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(if family.is_empty() { "your own" } else { family }).strong());
                }
                egui::Grid::new(("genres", *family)).num_columns(2).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                    for (i, g) in of.iter().enumerate() {
                        let selected = f.genre == g.name;
                        let at = genres.iter().position(|o| o.name == g.name).unwrap_or(i);
                        let color = theme::section_color(
                            ["drop", "groove", "break", "build", "intro", "hook", "verse", "outro"][at % 8],
                        );
                        let (rect, r) = ui.allocate_exact_size(vec2(300.0, 32.0 + text_height), Sense::click());
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
                        let text = two_lines(ui.painter(), desc(g), rect.width() - 22.0);
                        ui.painter().galley(rect.min + vec2(12.0, 26.0), text, TEXT_DIM);
                        if r.on_hover_text(&g.description).clicked() && f.genre != g.name {
                            f.genre = g.name.clone();
                            // each genre samples as it does until asked otherwise
                            f.sampled = None;
                        }
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            }
        });
    }
    ui.add_space(6.0);
    let more = match options_set(f) {
        set if set.is_empty() => "More options".to_string(),
        set => format!("More options: {set}"),
    };
    egui::CollapsingHeader::new(RichText::new(more).strong()).id_salt("new_song_more").default_open(false).show(
        ui,
        |ui| {
            egui::Grid::new("new_song_opts").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                if !genres.is_empty() {
                    ui.label("Fused with");
                    let shown = f.with.as_ref().map_or("nothing".to_string(), |w| w.replace('_', " "));
                    egui::ComboBox::from_id_salt("ns_with")
                    .width(150.0)
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut f.with, None, "nothing");
                        for g in genres.iter().filter(|g| g.name != f.genre) {
                            ui.selectable_value(&mut f.with, Some(g.name.clone()), g.name.replace('_', " "));
                        }
                    })
                    .response
                    .on_hover_text(
                        "a fusion: the genre above leads (tempo, harmony, form, parts and mix), and this one plays the \
                         drums and lends its sounds and styles to the parts",
                    );
                    ui.end_row();
                }
                ui.label("Seed");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut f.seed).desired_width(110.0).hint_text("random"));
                    if widgets::icon_button(ui, Icon::Dice, 24.0, false, true, "a random seed").clicked() {
                        let t = (cx.now * 1000.0) as u64;
                        f.seed = ((t.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33)
                            % 10_000)
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
                ui.label("Layers");
                ui.horizontal(|ui| layer_options(ui, f, genres));
                ui.end_row();
                ui.label("Loops");
                ui.checkbox(&mut f.loops, "build it around an Apple Loops family (GarageBand style)");
                ui.end_row();
                let sampling = genres.iter().find(|g| g.name == f.genre).and_then(|g| g.sampling.clone());
                ui.label("Samples");
                ui.vertical(|ui| sample_options(ui, f, sampling.as_ref(), cx.server.is_none(), cx.now));
                ui.end_row();
            });
        },
    );
    ui.add_space(12.0);
    let mut create = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let create_btn =
                egui::Button::new(RichText::new("  Create song  ").strong().color(egui::Color32::BLACK)).fill(ACCENT);
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
        cx.out.push(f.command(&cx.state.genres));
        f.open = false;
    }
}

/// a genre's description in two lines at most, the rest on hover
fn two_lines(painter: &egui::Painter, text: String, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text, theme::prop(11.0), TEXT_DIM, width);
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    painter.layout_job(job)
}

/// what of the New song dialog's options is asked for, in a line ("fused with idm, seed 42, 124 BPM"); empty: none
fn options_set(f: &super::NewSongForm) -> String {
    let mut set = Vec::new();
    if let Some(w) = &f.with {
        set.push(format!("fused with {}", w.replace('_', " ")));
    }
    if !f.seed.trim().is_empty() {
        set.push(format!("seed {}", f.seed.trim()));
    }
    if let Some(bpm) = f.bpm {
        set.push(format!("{bpm:.0} BPM"));
    }
    if let Some(key) = &f.key {
        set.push(key.replace('_', " "));
    }
    if f.complexity.is_some() {
        set.push("layers set".into());
    }
    if f.loops {
        set.push("Apple Loops".into());
    }
    match f.sampled {
        Some(true) => set.push("samples".into()),
        Some(false) => set.push("no samples".into()),
        None => {}
    }
    set.join(", ")
}

/// the New song dialog from a recording: the file, what to do with it, and what claudius heard in it
fn from_recording(ui: &mut Ui, cx: &mut Cx) {
    ui.label(small(
        "claudius reads a recording's tempo, key, chords, form and groove, and takes it apart into its voice, \
             drums, bass and the rest.",
    ));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let busy = cx.view.analyzing;
        if widgets::tool_button(ui, Icon::Open, "Choose a file...", !busy, false).clicked() {
            crate::pick::choose(ui.ctx(), crate::pick::Want::Audio);
        }
        match &cx.view.audio_file {
            Some(file) => ui.label(small(format!("{} ({:.1} MB)", file.name, file.bytes.len() as f64 / 1e6))),
            None => ui.label(small("no file chosen").color(TEXT_FAINT)),
        };
    });
    ui.add_space(8.0);
    ui.label(RichText::new("What to do with it").strong());
    let f = &mut cx.view.new_song;
    let shown = f.genre_name().replace('_', " ").replace('+', " + ");
    let song = cx.state.song.is_some();
    let drums = cx.state.song.as_ref().is_some_and(|s| !s.drums.is_empty());
    let choices = [
        (
            WithRecording::Remake,
            format!("Remake it as {shown}"),
            "a rework that stays the song it is: all of it plays through, its voice and its chords without its \
                 drums and its bass, in its own sections, with the genre's drums and bass under it",
            None,
        ),
        (
            WithRecording::Flip,
            format!("Flip it into a {shown} song"),
            "a new song of the genre built on its sound: its hook, another passage of it, its drums, a flip cut \
                 from all over it and its sung phrases",
            None,
        ),
        (
            WithRecording::Sample,
            "Sample its vocals into the song".to_string(),
            "its best sung phrases, fitted to the open song's tempo and key, where they agree with its chords",
            (!song).then_some("make or open a song first"),
        ),
        (
            WithRecording::Groove,
            "Put the song's drums in its groove".to_string(),
            "where each sixteenth of its drums lands and how hard, given to the open song's drum lanes",
            (!drums).then_some("make a song with drums first"),
        ),
        (
            WithRecording::Analyze,
            "Only say what claudius hears".to_string(),
            "its tempo, key, chords, form and groove, and nothing changed",
            None,
        ),
    ];
    let mut why_not = None;
    for (choice, label, what, unavailable) in &choices {
        ui.add_enabled_ui(unavailable.is_none(), |ui| {
            if ui.radio(f.with_recording == *choice, label.as_str()).clicked() {
                f.with_recording = *choice;
            }
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.add(egui::Label::new(small(unavailable.unwrap_or(what))).wrap());
            });
        });
        if f.with_recording == *choice {
            why_not = *unavailable;
        }
    }
    let genres = &cx.state.genres;
    if matches!(f.with_recording, WithRecording::Remake | WithRecording::Flip) && !genres.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("In the genre");
            egui::ComboBox::from_id_salt("ns_recording_genre")
                .width(180.0)
                .height(320.0)
                .selected_text(f.genre.replace('_', " "))
                .show_ui(ui, |ui| {
                    for g in genres {
                        if ui.selectable_label(f.genre == g.name, g.name.replace('_', " ")).clicked()
                            && f.genre != g.name
                        {
                            f.genre = g.name.clone();
                            f.sampled = None;
                        }
                    }
                });
            if !f.seed.trim().is_empty() {
                ui.label(small(format!("seed {}", f.seed.trim())));
            }
        });
    }
    if cx.view.analyzing {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().color(ACCENT));
            // what claudius says it is doing with the file: taking it apart, listening to it, sampling it ...
            let doing = cx.state.busy.clone().unwrap_or_else(|| "listening to the file".into());
            ui.label(small(format!("{doing}...")).color(theme::mix(ACCENT, TEXT, 0.4)));
            if let Some(p) = cx.state.progress.filter(|_| cx.state.busy.is_some()) {
                ui.add(
                    egui::ProgressBar::new(p.clamp(0.0, 1.0))
                        .desired_width(120.0)
                        .desired_height(8.0)
                        .fill(ACCENT)
                        .text(RichText::new(format!("{:.0}%", p * 100.0)).size(9.0)),
                );
            }
        });
    }
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
    let file = cx.view.audio_file.clone().filter(|_| !cx.view.analyzing);
    let f = &mut cx.view.new_song;
    let action = match f.with_recording {
        WithRecording::Remake => "Remake it",
        WithRecording::Flip => "Flip it",
        WithRecording::Sample => "Sample it",
        WithRecording::Groove => "Take its groove",
        WithRecording::Analyze => "Listen to it",
    };
    let mut go = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let ready = file.is_some() && why_not.is_none();
            let button = egui::Button::new(RichText::new(format!("  {action}  ")).strong().color(egui::Color32::BLACK))
                .fill(ACCENT);
            let r = ui.add_enabled(ready, button);
            let r = match (cx.view.analyzing, &cx.view.audio_file, why_not) {
                (true, _, _) => r.on_disabled_hover_text("claudius is still at the last one"),
                (_, None, _) => r.on_disabled_hover_text("choose a recording first"),
                (_, _, Some(why)) => r.on_disabled_hover_text(why),
                _ => r,
            };
            go = r.clicked();
            if ui.button("Cancel").clicked() {
                f.open = false;
            }
        });
    });
    if let Some(file) = file.filter(|_| go) {
        cx.view.analysis = None;
        let remake = matches!(f.with_recording, WithRecording::Remake | WithRecording::Flip)
            .then(|| (f.genre_name(), f.seed.trim().parse().ok()));
        cx.view.listen = Some(super::Listen {
            file,
            remake,
            flip: f.with_recording == WithRecording::Flip,
            sample: f.with_recording == WithRecording::Sample,
            groove: f.with_recording == WithRecording::Groove,
            found: None,
        });
    }
}

/// The New song dialog's layers: how layered the song is (`complexity`), from its genre's core to instruments doubled
/// on instruments and samples on samples; as the genre is until asked otherwise.
fn layer_options(ui: &mut Ui, f: &mut super::NewSongForm, genres: &[GenreInfo]) {
    let own = genres.iter().find(|g| g.name == f.genre).and_then(|g| g.complexity).unwrap_or(0.5);
    let mut auto = f.complexity.is_none();
    if ui.checkbox(&mut auto, "from the genre").changed() {
        f.complexity = if auto { None } else { Some(own) };
    }
    if let Some(c) = &mut f.complexity {
        ui.spacing_mut().slider_width = 160.0;
        ui.add(egui::Slider::new(c, 0.0..=1.0).show_value(false)).on_hover_text(
            "0: the genre's core, sparse. 0.5: the genre as written. 1: its harmony and lead doubled on second \
             instruments, a counter-melody, an arpeggio over the hooks, a pad, a clap on the snare, a shaker and \
             percussion, and more of the samples a sampled genre colours its songs with",
        );
    }
    ui.label(small(layers_said(f.complexity.unwrap_or(own))));
}

/// what a song of complexity `c` plays, in a few words
pub fn layers_said(c: f64) -> &'static str {
    match c {
        c if c < 0.15 => "sparse: the genre's core",
        c if c < 0.4 => "lean: fewer parts",
        c if c < 0.6 => "as the genre writes it",
        c if c < 0.85 => "layered: doubles, more percussion",
        _ => "dense: instruments on instruments",
    }
}

/// The New song dialog's samples: whether the song is built on samples from free libraries (as the genre does, until
/// asked otherwise), and the words its phrase is searched for; with none, the kinds of record the genre digs for, which
/// the field shows.
fn sample_options(ui: &mut Ui, f: &mut super::NewSongForm, sampling: Option<&GenreSampling>, demo: bool, now: f64) {
    let libraries = if demo {
        "Freesound, Jamendo and Wikimedia Commons"
    } else {
        "Freesound, Jamendo, Wikimedia Commons and ccMixter"
    };
    let default = sampling.is_some_and(|s| s.by_default);
    let mut on = sampling.is_some() && f.sampled.unwrap_or(default);
    let r = ui
        .add_enabled(sampling.is_some(), egui::Checkbox::new(&mut on, "auto-sample from free libraries"))
        .on_hover_text(format!(
            "builds the song on recordings from {libraries}, under CC0 or CC BY and credited: a phrase it takes its key, \
             tempo and chords from, played up or down as a sampler plays it, a drum break, a texture. {}",
            if default {
                "This genre's songs are built on samples unless you untick it."
            } else {
                "This genre's songs are when you tick it."
            }
        ))
        .on_disabled_hover_text("this genre says nothing of what it samples");
    if r.changed() {
        f.sampled = Some(on);
    }
    let Some(s) = sampling.filter(|_| on) else { return };
    let theirs: Vec<&str> = s
        .roles
        .iter()
        .find(|r| r.role == "phrase")
        .map(|r| r.search.iter().map(String::as_str).collect())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let hint = format!("{}, ...", theirs.iter().take(3).copied().collect::<Vec<_>>().join(", "));
        ui.add(egui::TextEdit::singleline(&mut f.sample_search).desired_width(300.0).char_limit(60).hint_text(hint));
        if !theirs.is_empty()
            && widgets::icon_button(ui, Icon::Dice, 24.0, false, true, "one of the records this genre digs for")
                .clicked()
        {
            let t = (now * 1000.0) as u64;
            let mut i =
                ((t.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33) as usize) % theirs.len();
            if theirs[i] == f.sample_search.trim() {
                i = (i + 1) % theirs.len();
            }
            f.sample_search = theirs[i].to_string();
        }
    });
    ui.label(small(if f.sample_search.trim().is_empty() {
        "empty: the records the genre's producers dig for, tried in an order the seed picks".to_string()
    } else {
        "its phrase is searched for these words, a kind of record rather than a genre's name; the break and the \
         texture come from the genre's own"
            .to_string()
    }));
}

/// What the download button asks for: in a page, the song as a download once a render of it as it is now is here (the
/// demo renders it when its render is older); a server saves the song's folder too, and the app only saves it.
pub(crate) fn ask_download(
    state: &State,
    view: &mut View,
    out: &mut Vec<Command>,
    demo: bool,
    now: f64,
    format: Option<String>,
) {
    if demo || cfg!(target_arch = "wasm32") {
        view.download = Some(super::Download::of(state, now, format));
    }
    if !demo {
        out.push(Command::Save { path: None });
    } else if state.busy.is_none() && (state.render.is_none() || super::render_stale(state)) {
        out.push(Command::Render { stems: false });
    }
}

/// a song title as a file name: `Glass Garden` -> `glass-garden`
pub(crate) fn file_name(title: &str) -> String {
    let name: String = title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    match name.trim_matches('-') {
        "" => "song".to_string(),
        n => n.to_string(),
    }
}
