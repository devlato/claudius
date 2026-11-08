//! The arrangement: the sections as blocks along the bars (coloured by kind, drag one to reorder), one row per layer
//! with a cell per section (click, or drag across cells, to switch the layer on or off there), and the waveform of
//! the latest render with the playhead underneath (click or drag to seek).

use claudius_api::{Command, SongView};
use eframe::egui::{
    self, Align2, Color32, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};

use super::{Cx, Tab, empty, layer_color, small};
use crate::cmds::MixEdit;
use crate::edits::SECTION_KINDS;
use crate::theme::{self, ACCENT, BG, LINE, LINE_SOFT, PLAYHEAD, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::{self, Icon};

const GUTTER: f32 = 138.0;

struct Timeline {
    x0: f32,
    x1: f32,
    bars: f64,
}

impl Timeline {
    fn x(&self, bar: f64) -> f32 {
        self.x0 + (bar / self.bars) as f32 * (self.x1 - self.x0)
    }

    fn bar(&self, x: f32) -> f64 {
        (((x - self.x0) / (self.x1 - self.x0)) as f64 * self.bars).clamp(0.0, self.bars)
    }
}

/// `base`, or `base` numbered (`lead2`, `lead3` ...) when the song has a track of that name
fn unique_track_name(song: &SongView, base: &str) -> String {
    let base: String = base.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    let base = if base.is_empty() { "track".to_string() } else { base };
    (1..)
        .map(|n| if n == 1 { base.clone() } else { format!("{base}{n}") })
        .find(|n| !song.layers.contains(n))
        .unwrap_or(base)
}

/// what the "+ Track" menu does once one of its buttons is pressed
enum Added {
    /// a command for claudius: a track generated or added
    Send(Command),
    /// the Add a recording window, searching the free libraries for these words
    Search(String),
    /// the Record window
    Record,
}

/// The "+ Track" menu: a track written the way a genre plays it, a drum lane or a part of a role of your own (its
/// name, and its sound from those installed), or a recording. What it does once a button is pressed.
fn add_track_menu(
    ui: &mut Ui,
    f: &mut super::AddTrackForm,
    song: &SongView,
    genres: &[claudius_api::GenreInfo],
    sounds: Option<&claudius_api::SoundList>,
) -> Option<Added> {
    use super::AddKind;
    ui.set_min_width(320.0);
    ui.horizontal(|ui| {
        for (kind, label, tip) in [
            (
                AddKind::Style,
                "In a style",
                "written the way a genre plays it: its drums, or a part in its idiom, on its sounds",
            ),
            (AddKind::Part, "Part", "notes the theory engine writes, in a role"),
            (AddKind::Drum, "Drum lane", "steps you write in the step sequencer"),
            (
                AddKind::Recording,
                "Recording",
                "a song or a sound: a file of yours, one a free library offers, or one you record",
            ),
        ] {
            ui.selectable_value(&mut f.kind, kind, label).on_hover_text(tip);
        }
    });
    match f.kind {
        AddKind::Style => return generate_menu(ui, f, song, genres).map(Added::Send),
        AddKind::Recording => return recording_menu(ui, f),
        AddKind::Part | AddKind::Drum => {}
    }
    let drum = f.kind == AddKind::Drum;
    if !drum {
        ui.horizontal_wrapped(|ui| {
            for r in ["bass", "chords", "pad", "keys", "lead", "arp"] {
                ui.selectable_value(&mut f.role, r.to_string(), r);
            }
        });
    }
    let short = f.sound.rsplit(':').next().unwrap_or("").to_string();
    let base = if drum { if short.is_empty() { "perc".to_string() } else { short } } else { f.role.clone() };
    let suggested = unique_track_name(song, &base);
    ui.horizontal(|ui| {
        ui.label(small("name"));
        ui.add(egui::TextEdit::singleline(&mut f.name).hint_text(suggested.as_str()).desired_width(150.0));
    });
    ui.add(egui::TextEdit::singleline(&mut f.filter).hint_text("search sounds").desired_width(f32::INFINITY));
    let hit = |n: &String| f.filter.is_empty() || n.to_lowercase().contains(&f.filter.to_lowercase());
    let (empty_list, empty_kit) = (Vec::new(), Vec::new());
    let (drums, kit, synths, instruments) = match sounds {
        Some(s) => (&s.drums, &s.kit, &s.synths, &s.instruments),
        None => (&empty_list, &empty_kit, &empty_list, &empty_list),
    };
    let lists: Vec<(String, Vec<String>)> = if drum {
        vec![
            ("synthesized".into(), drums.iter().filter(|n| hit(n)).cloned().collect()),
            ("recorded".into(), kit.iter().filter(|n| hit(n)).cloned().collect()),
        ]
    } else {
        // the role's synths first, then the rest; the installed instruments by their short names
        let made_for = |n: &String| {
            sounds.is_some_and(|s| s.synth_roles.iter().any(|(p, roles)| p == n && roles.contains(&f.role)))
        };
        let (mine, others): (Vec<String>, Vec<String>) = synths.iter().filter(|n| hit(n)).cloned().partition(made_for);
        let named: Vec<String> = instruments
            .iter()
            .filter(|n| !n.contains(':') && !kit.contains(n) && !drums.contains(n) && hit(n))
            .cloned()
            .collect();
        vec![(format!("synths for {}", f.role), mine), ("other synths".into(), others), ("instruments".into(), named)]
    };
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        for (heading, names) in &lists {
            if names.is_empty() {
                continue;
            }
            ui.label(small(heading.as_str()).weak());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for n in names {
                    ui.selectable_value(&mut f.sound, n.clone(), n.as_str());
                }
            });
        }
    });
    let name = if f.name.trim().is_empty() { suggested } else { f.name.trim().to_string() };
    let hint = if f.sound.is_empty() && !drum { "the genre draws its sound" } else { "" };
    // a lane needs its sound; a part may take the genre's
    let ready = !drum || !f.sound.is_empty();
    ui.horizontal(|ui| {
        let add = ui.add_enabled(ready, egui::Button::new(format!("Add {name}")));
        if !hint.is_empty() {
            ui.label(small(hint));
        }
        if add.clicked() {
            let cmd = Command::AddTrack {
                name,
                kind: if drum { "drum" } else { "part" }.into(),
                role: (!drum).then(|| f.role.clone()),
                sound: (!f.sound.is_empty()).then(|| f.sound.clone()),
            };
            f.name.clear();
            f.sound.clear();
            f.filter.clear();
            ui.close();
            Some(Added::Send(cmd))
        } else {
            None
        }
    })
    .inner
}

/// The "+ Track" menu's recording: a file of yours, one the free libraries offer, or one recorded now, each through
/// the window it goes in by (Add a recording, which also searches the libraries, and Record).
fn recording_menu(ui: &mut Ui, f: &mut super::AddTrackForm) -> Option<Added> {
    ui.label(small("a song or a sound: added whole as a take you trim and put effects on, or with its vocals sampled"));
    ui.add_space(2.0);
    if ui.button("Choose a file on this computer...").on_hover_text("WAV, MP3, FLAC, AAC, AIFF, Ogg or Opus").clicked()
    {
        crate::pick::choose(ui.ctx(), crate::pick::Want::Recording);
        ui.close();
    }
    ui.add_space(4.0);
    ui.label(small("or search the free libraries (CC0, public domain, CC BY), to hear and choose"));
    let search = ui
        .horizontal(|ui| {
            let words = ui.add(
                egui::TextEdit::singleline(&mut f.query)
                    .hint_text("a soul singer, a jazz drum break")
                    .desired_width(220.0),
            );
            let enter = words.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let go = ui.add_enabled(!f.query.trim().is_empty(), egui::Button::new("Search"));
            (go.clicked() || enter) && !f.query.trim().is_empty()
        })
        .inner;
    if search {
        ui.close();
        return Some(Added::Search(f.query.trim().to_string()));
    }
    ui.add_space(4.0);
    let record = ui
        .button("Record one...")
        .on_hover_text("sing or play it into the microphone, or play it on a MIDI keyboard, over the song");
    if record.clicked() {
        ui.close();
        return Some(Added::Record);
    }
    None
}

/// The "+ Track" menu's track in a style: what it is, the genre whose way of playing it to write it in, and for a part,
/// whether it takes the place of the song's part of its role. The command once Generate is pressed.
fn generate_menu(
    ui: &mut Ui,
    f: &mut super::AddTrackForm,
    song: &SongView,
    genres: &[claudius_api::GenreInfo],
) -> Option<Command> {
    ui.horizontal_wrapped(|ui| {
        for w in ["drums", "percussion", "bass", "chords", "keys", "pad", "strings", "lead", "arp"] {
            ui.selectable_value(&mut f.what, w.to_string(), w);
        }
    });
    let genre = if f.genre.is_empty() { song.genre.clone() } else { f.genre.clone() };
    // the styles in the menu itself: a list opened over it would close it
    ui.add(
        egui::TextEdit::singleline(&mut f.filter)
            .hint_text("search styles: a genre, a scene, an era")
            .desired_width(f32::INFINITY),
    );
    let words = f.filter.to_lowercase();
    let hit = |g: &&claudius_api::GenreInfo| {
        words.is_empty()
            || [&g.name, &g.family, &g.description].iter().any(|s| s.to_lowercase().contains(&words))
            || g.aliases.iter().any(|a| a.to_lowercase().contains(&words))
    };
    egui::ScrollArea::vertical().id_salt("generate-styles").max_height(150.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for g in genres.iter().filter(hit) {
                if ui.selectable_label(genre == g.name, &g.name).on_hover_text(&g.description).clicked() {
                    f.genre = g.name.clone();
                }
            }
        });
    });
    // the song's part it may be written in place of
    let target = match f.what.as_str() {
        "drums" | "percussion" => None,
        "strings" => song.parts.iter().find(|p| p.name.contains("string")),
        w => song.parts.iter().find(|p| p.role == w),
    };
    if let Some(t) = target {
        ui.checkbox(&mut f.replace, format!("in place of {}", t.name))
            .on_hover_text("written again in this style: its name, mix and sections stay");
    }
    let hint = match f.what.as_str() {
        "drums" => "its lanes in place of the song's of their names, and beside the others",
        "percussion" => "its percussion lanes beside the song's drums",
        _ if target.is_some() && f.replace => "",
        _ => "a new track, where the song plays its part of the role",
    };
    if !hint.is_empty() {
        ui.label(small(hint));
    }
    if ui.button(format!("Generate {genre} {}", f.what)).on_hover_text("another take each time").clicked() {
        ui.close();
        return Some(Command::GenerateTrack {
            what: f.what.clone(),
            genre: Some(genre),
            replace: target.filter(|_| f.replace).map(|t| t.name.clone()),
            name: None,
            seed: None,
        });
    }
    None
}

/// hits per bar of a drum lane's pattern for a section (0..1 density)
fn density(song: &SongView, lane: &str, variant: &str, bar: u32) -> Option<f32> {
    let d = song.drums.iter().find(|d| d.lane == lane)?;
    let steps = d.variants.iter().find(|(v, _)| v == variant).or_else(|| d.variants.first()).map(|(_, s)| s)?;
    let pb = d.pattern_bars.max(1);
    let from = ((bar % pb) * 16) as usize;
    let sum: f32 = steps.iter().skip(from).take(16).sum();
    Some((sum / 8.0).clamp(0.0, 1.0))
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let bpm_audio = cx.audio_bpm();
    let head = cx.playhead_bar();
    let stale = cx.render_stale();
    let has_audio = cx.player.has_audio();
    let position = cx.player.position();
    let loop_region = cx.view.looping;
    let solo = cx.any_solo();
    let (instruments, synths) = super::melodic_sounds(cx.sounds);
    let mut style = cx.view.melody_style.clone();
    let mut row_menu = cx.view.row_menu;
    let mut add_form = std::mem::take(&mut cx.view.add_track);
    let sounds = cx.sounds;
    let genres = &cx.state.genres;
    let Some(song) = cx.state.song.as_ref() else {
        cx.view.add_track = add_form;
        empty(ui, "No song yet: make one with New song.");
        super::missing_packs(ui, cx);
        return;
    };
    let render = cx.state.render.as_ref();
    let mut out: Vec<Command> = Vec::new();
    // a take whose Trim window opens
    let mut trim = None;
    // the window the "+ Track" menu opens for a recording
    let mut opened = None;
    let full = ui.available_rect_before_wrap();
    let render_bars = render.map_or(0.0, |r| r.seconds * r.bpm.max(1.0) / 240.0);
    let tl = Timeline {
        x0: full.left() + GUTTER,
        x1: full.right() - 12.0,
        bars: (song.bars as f64).max(render_bars).max(1.0),
    };
    let px_bar = (tl.x1 - tl.x0) / tl.bars as f32;

    // header
    let header = Rect::from_min_size(full.min, vec2(full.width(), 26.0));
    let mut header_ui = ui.new_child(UiBuilder::new().max_rect(header.shrink2(vec2(10.0, 2.0))));
    header_ui.horizontal_centered(|ui| {
        widgets::heading(ui, "Arrangement");
        ui.label(small(format!(
            "{} sections · {} bars · {} tracks",
            song.sections.len(),
            song.bars,
            song.layers.len()
        )));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for kind in SECTION_KINDS.iter().rev().filter(|k| song.sections.iter().any(|s| s.kind == **k)) {
                // as many of the song's kinds as there is room for beside the heading, the last first
                let text = ui.painter().layout_no_wrap(kind.to_string(), theme::prop(10.5), TEXT_DIM).size().x;
                if ui.available_width() < text + 8.0 + 2.0 * ui.spacing().item_spacing.x {
                    break;
                }
                ui.label(RichText::new(*kind).size(10.5).color(theme::section_color(kind)));
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, theme::section_color(kind));
            }
        });
    });

    let painter = ui.painter_at(full);
    // ruler
    let ruler = Rect::from_min_max(pos2(tl.x0, header.bottom()), pos2(tl.x1, header.bottom() + 18.0));
    painter.rect_filled(
        Rect::from_min_max(pos2(full.left(), ruler.top()), pos2(full.right(), ruler.bottom())),
        0.0,
        WELL,
    );
    let step = [1u32, 2, 4, 8, 16, 32, 64].into_iter().find(|s| *s as f32 * px_bar >= 30.0).unwrap_or(64);
    for bar in 0..=tl.bars as u32 {
        let x = tl.x(bar as f64);
        if bar % step == 0 {
            painter
                .line_segment([pos2(x, ruler.bottom() - 7.0), pos2(x, ruler.bottom())], Stroke::new(1.0, TEXT_FAINT));
            painter.text(
                pos2(x + 3.0, ruler.center().y - 2.0),
                Align2::LEFT_CENTER,
                format!("{}", bar + 1),
                theme::prop(10.0),
                TEXT_DIM,
            );
        } else if px_bar >= 4.0 {
            painter.line_segment([pos2(x, ruler.bottom() - 3.0), pos2(x, ruler.bottom())], Stroke::new(1.0, LINE));
        }
    }
    painter.text(pos2(full.left() + 10.0, ruler.center().y), Align2::LEFT_CENTER, "Bar", theme::prop(11.0), TEXT_FAINT);
    let ruler_resp = ui.interact(ruler, ui.scope_id().with("ruler"), Sense::click_and_drag());
    if has_audio
        && (ruler_resp.clicked() || ruler_resp.dragged())
        && let Some(p) = ruler_resp.interact_pointer_pos()
    {
        cx.player.seek(tl.bar(p.x) * 240.0 / bpm_audio);
    }

    // sections
    let blocks = Rect::from_min_max(pos2(tl.x0, ruler.bottom() + 3.0), pos2(tl.x1, ruler.bottom() + 39.0));
    painter.text(
        pos2(full.left() + 10.0, blocks.center().y),
        Align2::LEFT_CENTER,
        "Sections",
        theme::prop(11.0),
        TEXT_FAINT,
    );
    let mut drop_slot: Option<usize> = None;
    for (i, s) in song.sections.iter().enumerate() {
        let rect = Rect::from_min_max(
            pos2(tl.x(s.start_bar as f64) + 1.0, blocks.top()),
            pos2(tl.x((s.start_bar + s.bars) as f64) - 1.0, blocks.bottom()),
        );
        let resp = ui.interact(rect, ui.scope_id().with(("section", i)), Sense::click_and_drag());
        let selected = cx.view.section == i;
        let color = theme::section_color(&s.kind);
        let fill = if selected { color } else { theme::mix(color, BG, if resp.hovered() { 0.18 } else { 0.32 }) };
        painter.rect_filled(rect, 4.0, fill);
        if s.build {
            painter.add(Shape::convex_polygon(
                vec![rect.left_bottom(), rect.right_bottom(), rect.right_top() + vec2(0.0, 6.0)],
                theme::alpha(Color32::WHITE, 0.12),
                Stroke::NONE,
            ));
        }
        if s.filter_in {
            let pts: Vec<_> = (0..=12)
                .map(|k| {
                    let t = k as f32 / 12.0;
                    pos2(rect.left() + t * rect.width(), rect.bottom() - 3.0 - (t * t) * (rect.height() - 10.0))
                })
                .collect();
            painter.add(Shape::line(pts, Stroke::new(1.5, theme::alpha(Color32::WHITE, 0.35))));
        }
        if s.impact {
            painter.line_segment(
                [rect.left_top() + vec2(1.5, 0.0), rect.left_bottom() + vec2(1.5, 0.0)],
                Stroke::new(3.0, theme::alpha(Color32::WHITE, 0.85)),
            );
        }
        if s.gap {
            let w = (px_bar * 0.25).max(3.0);
            painter.rect_filled(
                Rect::from_min_max(pos2(rect.right() - w, rect.top() + 4.0), pos2(rect.right(), rect.bottom() - 4.0)),
                1.0,
                theme::alpha(BG, 0.85),
            );
        }
        if s.fills {
            for b in (8..=s.bars).step_by(8) {
                let x = tl.x((s.start_bar + b) as f64) - px_bar * 0.5;
                painter.circle_filled(pos2(x, rect.bottom() - 4.0), 1.8, theme::alpha(Color32::WHITE, 0.7));
            }
        }
        if selected {
            painter.rect_stroke(rect, 4.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Inside);
        }
        let text_color = if selected { Color32::BLACK } else { theme::mix(color, Color32::WHITE, 0.75) };
        let clip = painter.with_clip_rect(rect.shrink(2.0));
        clip.text(rect.left_top() + vec2(6.0, 10.0), Align2::LEFT_CENTER, &s.name, theme::medium(12.5), text_color);
        let length = clip.text(
            rect.left_top() + vec2(6.0, 25.0),
            Align2::LEFT_CENTER,
            format!("{} bars", s.bars),
            theme::prop(10.5),
            theme::alpha(text_color, 0.8),
        );
        // how intense it is, as a meter of five rising steps after its length
        let lit = (s.energy * 5.0).round().clamp(if s.energy > 0.0 { 1.0 } else { 0.0 }, 5.0) as usize;
        for k in 0..5 {
            let h = 3.0 + k as f32 * 1.5;
            let x = length.right() + 6.0 + k as f32 * 4.0;
            clip.rect_filled(
                Rect::from_min_max(pos2(x, rect.top() + 29.5 - h), pos2(x + 2.5, rect.top() + 29.5)),
                0.5,
                theme::alpha(text_color, if k < lit { 0.85 } else { 0.25 }),
            );
        }

        if resp.clicked() {
            cx.view.section = i;
        }
        if resp.drag_started() {
            cx.view.section = i;
            cx.view.drag_section = Some(i);
        }
        if cx.view.drag_section == Some(i)
            && let Some(p) = ui.input(|inp| inp.pointer.interact_pos())
        {
            let slot = song.sections.iter().filter(|x| tl.x(x.start_bar as f64 + x.bars as f64 / 2.0) < p.x).count();
            drop_slot = Some(slot);
            if resp.drag_stopped() {
                let to = if slot > i { slot - 1 } else { slot };
                if to != i {
                    out.push(Command::MoveSection { from: i, to });
                    cx.view.section = to;
                }
                cx.view.drag_section = None;
            }
        }
        let flags: Vec<&str> = super::inspector::MOVES
            .iter()
            .zip([s.filter_in, s.build, s.gap, s.impact, s.fills])
            .filter(|(_, on)| *on)
            .map(|((name, _), _)| *name)
            .collect();
        let at = |bar: u32| super::fmt_time(bar as f64 * 240.0 / song.bpm.max(1.0));
        let resp = resp.on_hover_text(format!(
            "{} ({}): bars {}–{}, {}–{}\nintensity {:.0}%, drum pattern '{}'{}\nplaying: {}\nclick to edit it, drag to \
             move it, right-click for more",
            s.name,
            s.kind,
            s.start_bar + 1,
            s.start_bar + s.bars,
            at(s.start_bar),
            at(s.start_bar + s.bars),
            s.energy * 100.0,
            s.variant,
            if flags.is_empty() { String::new() } else { format!("\n{}", flags.join(", ")) },
            if s.layers.is_empty() { "nothing".to_string() } else { s.layers.join(", ") }
        ));
        resp.context_menu(|ui| {
            cx.view.section = i;
            ui.label(RichText::new(&s.name).strong());
            if ui.button("Duplicate").clicked() {
                out.push(Command::AddSection { after: i, kind: s.kind.clone(), bars: s.bars });
                out.push(Command::SetSection {
                    index: i + 1,
                    name: None,
                    bars: None,
                    energy: Some(s.energy),
                    layers: Some(s.layers.clone()),
                    prog: Some(s.prog.clone()),
                    variant: Some(s.variant.clone()),
                    filter_in: Some(s.filter_in),
                    build: Some(s.build),
                    gap: Some(s.gap),
                    impact: Some(s.impact),
                    fills: Some(s.fills),
                });
                ui.close();
            }
            ui.menu_button("Insert after", |ui| {
                for kind in SECTION_KINDS {
                    if ui.button(kind).clicked() {
                        out.push(Command::AddSection { after: i, kind: kind.into(), bars: 8 });
                        ui.close();
                    }
                }
            });
            ui.add_enabled_ui(i > 0, |ui| {
                if ui.button("Move left").clicked() {
                    out.push(Command::MoveSection { from: i, to: i - 1 });
                    cx.view.section = i - 1;
                    ui.close();
                }
            });
            ui.add_enabled_ui(i + 1 < song.sections.len(), |ui| {
                if ui.button("Move right").clicked() {
                    out.push(Command::MoveSection { from: i, to: i + 1 });
                    cx.view.section = i + 1;
                    ui.close();
                }
            });
            if ui.button("Loop this section").clicked() {
                cx.view.looping = true;
                cx.view.loop_section = true;
                ui.close();
            }
            ui.separator();
            ui.add_enabled_ui(song.sections.len() > 1, |ui| {
                if ui.button(RichText::new("Remove").color(theme::BAD)).clicked() {
                    out.push(Command::RemoveSection { index: i });
                    ui.close();
                }
            });
        });
    }
    if let Some(slot) = drop_slot {
        let x = song.sections.get(slot).map_or(tl.x(song.bars as f64), |s| tl.x(s.start_bar as f64));
        painter.line_segment([pos2(x, blocks.top() - 2.0), pos2(x, blocks.bottom() + 2.0)], Stroke::new(3.0, ACCENT));
    }
    if cx.view.drag_section.is_some() && !ui.input(|i| i.pointer.any_down()) {
        cx.view.drag_section = None;
    }

    // layer rows
    let wave_h = (full.height() * 0.2).clamp(48.0, 120.0);
    let rows_rect =
        Rect::from_min_max(pos2(full.left(), blocks.bottom() + 6.0), pos2(full.right(), full.bottom() - wave_h - 26.0));
    let n = song.layers.len().max(1);
    // rows tall enough to read, room for + Track under the last; more rows than fit scroll
    let row_h = ((rows_rect.height() - 28.0) / n as f32).clamp(22.0, 30.0);
    let mut rows_ui = ui.new_child(UiBuilder::new().max_rect(rows_rect));
    egui::ScrollArea::vertical().id_salt("arrange_rows").auto_shrink([false, false]).show(&mut rows_ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(vec2(rows_rect.width(), row_h * n as f32), Sense::click_and_drag());
        let p = ui.painter();
        let sel = song.sections.get(cx.view.section);
        if let Some(s) = sel {
            let r = Rect::from_min_max(
                pos2(tl.x(s.start_bar as f64), rect.top()),
                pos2(tl.x((s.start_bar + s.bars) as f64), rect.bottom()),
            );
            p.rect_filled(r, 0.0, theme::alpha(Color32::WHITE, 0.035));
        }
        let hover = resp.hover_pos();
        let cell_at = |pos: egui::Pos2| -> Option<(usize, usize)> {
            if pos.x < tl.x0 || pos.x > tl.x1 {
                return None;
            }
            let row = ((pos.y - rect.top()) / row_h) as usize;
            let bar = tl.bar(pos.x);
            let sec =
                song.sections.iter().position(|s| bar >= s.start_bar as f64 && bar < (s.start_bar + s.bars) as f64)?;
            (row < song.layers.len()).then_some((row, sec))
        };
        // the button on a row's label that swaps its sound for the next, where the kind is written
        let swap_at = |row: usize| {
            let y = rect.top() + (row as f32 + 0.5) * row_h;
            Rect::from_center_size(pos2(rect.left() + GUTTER - 20.0, y), vec2(22.0, (row_h - 4.0).clamp(12.0, 18.0)))
        };
        for (row, layer) in song.layers.iter().enumerate() {
            let y0 = rect.top() + row as f32 * row_h;
            let row_rect = Rect::from_min_size(pos2(rect.left(), y0), vec2(rect.width(), row_h));
            if row % 2 == 1 {
                p.rect_filled(
                    Rect::from_min_max(pos2(tl.x0, y0), pos2(tl.x1, y0 + row_h)),
                    0.0,
                    theme::alpha(Color32::WHITE, 0.012),
                );
            }
            p.line_segment(
                [pos2(rect.left(), row_rect.bottom()), pos2(rect.right(), row_rect.bottom())],
                Stroke::new(1.0, LINE_SOFT),
            );
            let color = layer_color(song, layer);
            let track = song.tracks.iter().find(|t| &t.name == layer);
            let soloed = track.is_some_and(|t| t.solo);
            let silenced = solo && !soloed;
            let muted = track.is_some_and(|t| t.muted) || silenced;
            p.rect_filled(
                Rect::from_min_size(pos2(rect.left() + 8.0, y0 + 4.0), vec2(3.0, row_h - 8.0)),
                1.0,
                if muted { TEXT_FAINT } else { color },
            );
            let is_drum = song.drums.iter().any(|d| &d.lane == layer);
            let kind = track.map(|t| t.kind.as_str()).unwrap_or(if is_drum { "drum" } else { "part" });
            // what the track is, or that it is soloed or muted
            let (icon, icon_color) = if soloed {
                (Icon::Solo, theme::SOLO)
            } else if muted && !silenced {
                (Icon::Muted, theme::BAD)
            } else {
                (Icon::of_track(kind, track.map_or("", |t| t.role.as_str())), if muted { TEXT_FAINT } else { TEXT_DIM })
            };
            let icon_rect = Rect::from_center_size(pos2(rect.left() + 23.0, y0 + row_h / 2.0), vec2(14.0, 14.0));
            widgets::paint_icon(p, icon_rect, icon, icon_color);
            let name_room = GUTTER - 36.0 - 26.0;
            let name = p.layout(layer.clone(), theme::prop(13.0), if muted { TEXT_FAINT } else { TEXT }, f32::INFINITY);
            let name_at = pos2(rect.left() + 36.0, y0 + (row_h - name.size().y) / 2.0);
            p.with_clip_rect(Rect::from_min_size(name_at, vec2(name_room, row_h))).galley(name_at, name, TEXT);
            let state = if soloed {
                "soloed: while any track is, only soloed tracks play"
            } else if silenced {
                "silent while another track is soloed"
            } else if muted {
                "muted"
            } else {
                ""
            };
            let on_label = hover.is_some_and(|h| h.x < tl.x0 && h.y >= y0 && h.y < y0 + row_h);
            let on_swap =
                track.is_some_and(|t| t.next_swap.is_some()) && hover.is_some_and(|h| swap_at(row).contains(h));
            if on_label && !on_swap && !state.is_empty() {
                resp.clone().on_hover_text(format!("{layer}: {state}"));
            }
            if on_label && track.is_some_and(|t| t.next_swap.is_some()) {
                let r = swap_at(row);
                let over = hover.is_some_and(|h| r.contains(h));
                if over {
                    p.rect_filled(r, 5.0, theme::CONTROL_HI);
                }
                let icon = Rect::from_center_size(r.center(), vec2(13.0, 13.0));
                widgets::paint_icon(p, icon, Icon::Swap, if over { TEXT } else { TEXT_DIM });
            }
            // a clip for each run of sections the track plays in: one name, its sections' energy as its shade, and
            // a drum lane's hits a bar as the height of what fills it
            let base = if muted { TEXT_FAINT } else { color };
            let plays = |s: &claudius_api::SectionView| s.layers.iter().any(|l| l == layer);
            let mut k = 0;
            while k < song.sections.len() {
                if !plays(&song.sections[k]) {
                    k += 1;
                    continue;
                }
                let first = k;
                while k < song.sections.len() && plays(&song.sections[k]) {
                    k += 1;
                }
                let run = &song.sections[first..k];
                let end = run.last().map_or(0, |s| s.start_bar + s.bars);
                let clip = Rect::from_min_max(
                    pos2(tl.x(run[0].start_bar as f64) + 1.0, y0 + 3.0),
                    pos2(tl.x(end as f64) - 1.0, y0 + row_h - 3.0),
                );
                let mut hits = Vec::new();
                for (j, s) in run.iter().enumerate() {
                    let part = Rect::from_min_max(
                        pos2(tl.x(s.start_bar as f64).max(clip.left()), clip.top()),
                        pos2(tl.x((s.start_bar + s.bars) as f64).min(clip.right()), clip.bottom()),
                    );
                    let (start, stop) = (if j == 0 { 4 } else { 0 }, if j + 1 == run.len() { 4 } else { 0 });
                    let corners = egui::CornerRadius { nw: start, sw: start, ne: stop, se: stop };
                    p.rect_filled(part, corners, theme::alpha(base, 0.2 + 0.3 * s.energy));
                    if j > 0 {
                        p.line_segment(
                            [pos2(part.left(), part.top() + 3.0), pos2(part.left(), part.bottom() - 3.0)],
                            Stroke::new(1.0, theme::alpha(BG, 0.6)),
                        );
                    }
                    if is_drum {
                        for b in 0..s.bars {
                            let Some(d) = density(song, layer, &s.variant, b) else { continue };
                            let x = tl.x((s.start_bar + b) as f64);
                            hits.push((x.max(part.left()), (x + px_bar).min(part.right()), d));
                        }
                    }
                }
                if !hits.is_empty() {
                    let well = clip.shrink2(vec2(0.0, 2.0));
                    let mut mesh = egui::Mesh::default();
                    for (x0, x1, d) in hits {
                        let top = well.bottom() - d * well.height();
                        mesh.add_colored_rect(
                            Rect::from_min_max(pos2(x0, top), pos2(x1, well.bottom())),
                            theme::alpha(base, 0.32),
                        );
                    }
                    p.add(Shape::mesh(mesh));
                }
                p.line_segment(
                    [clip.left_top() + vec2(3.0, 0.5), clip.right_top() + vec2(-3.0, 0.5)],
                    Stroke::new(1.0, theme::alpha(base, 0.9)),
                );
                if !is_drum && clip.width() > 46.0 {
                    p.with_clip_rect(clip.shrink(1.0)).text(
                        clip.left_center() + vec2(6.0, 0.5),
                        Align2::LEFT_CENTER,
                        layer,
                        theme::prop(11.0),
                        theme::mix(base, Color32::WHITE, 0.65),
                    );
                }
            }
        }
        for s in &song.sections {
            let x = tl.x(s.start_bar as f64);
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, LINE_SOFT));
        }
        // painting gesture: press toggles a cell, dragging along sets the others in the row to the same state
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        let down = ui.input(|i| i.pointer.primary_down());
        if pressed
            && resp.hovered()
            && let Some((row, sec)) = hover.and_then(cell_at)
        {
            let layer = &song.layers[row];
            let on = !song.sections[sec].layers.iter().any(|l| l == layer);
            out.push(Command::ToggleLayer { section: sec, layer: layer.clone() });
            cx.view.layer_paint = Some((on, row, vec![sec]));
            cx.view.section = sec;
        } else if down {
            if let Some((on, row, touched)) = &mut cx.view.layer_paint
                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
                && let Some((_, sec)) = cell_at(pos2(pos.x, rect.top() + *row as f32 * row_h + 1.0))
                && !touched.contains(&sec)
            {
                touched.push(sec);
                let layer = &song.layers[*row];
                let is_on = song.sections[sec].layers.iter().any(|l| l == layer);
                if is_on != *on {
                    out.push(Command::ToggleLayer { section: sec, layer: layer.clone() });
                }
            }
        } else {
            cx.view.layer_paint = None;
        }
        if let Some(pos) = hover {
            if pos.x < tl.x0 {
                let row = ((pos.y - rect.top()) / row_h) as usize;
                if let Some(layer) = song.layers.get(row) {
                    let r =
                        Rect::from_min_size(pos2(rect.left(), rect.top() + row as f32 * row_h), vec2(GUTTER, row_h));
                    p.rect_filled(r, 2.0, theme::alpha(Color32::WHITE, 0.04));
                    let next = song.tracks.iter().find(|t| &t.name == layer).and_then(super::next_label);
                    let on_swap = next.is_some() && swap_at(row).contains(pos);
                    if let Some(next) = next.filter(|_| on_swap) {
                        resp.clone().on_hover_text(format!("another sound for {layer}: {next}"));
                    }
                    if resp.clicked() && on_swap {
                        out.push(super::swap(layer, None));
                    } else if resp.clicked() {
                        cx.view.tab = if song.drums.iter().any(|d| &d.lane == layer) {
                            Tab::Sequencer
                        } else if song.parts.iter().any(|x| &x.name == layer) {
                            Tab::Parts
                        } else {
                            Tab::Mixer
                        };
                    }
                }
            } else if let Some((row, sec)) = cell_at(pos) {
                let s = &song.sections[sec];
                let cell = Rect::from_min_max(
                    pos2(tl.x(s.start_bar as f64) + 1.0, rect.top() + row as f32 * row_h + 2.0),
                    pos2(tl.x((s.start_bar + s.bars) as f64) - 1.0, rect.top() + (row + 1) as f32 * row_h - 2.0),
                );
                p.rect_stroke(cell, 3.0, Stroke::new(1.0, theme::alpha(Color32::WHITE, 0.5)), StrokeKind::Inside);
                let layer = &song.layers[row];
                let on = s.layers.iter().any(|l| l == layer);
                resp.clone().on_hover_text(format!(
                    "{layer} in {}: {} (click to switch {}, drag along the row to paint)",
                    s.name,
                    if on { "on" } else { "off" },
                    if on { "off" } else { "on" }
                ));
            }
        }
        // the mute / solo menu on a row label
        if resp.secondary_clicked() {
            row_menu = resp.interact_pointer_pos().map(|p| ((p.y - rect.top()) / row_h).max(0.0) as usize);
        }
        resp.context_menu(|ui| {
            let Some(layer) = row_menu.and_then(|row| song.layers.get(row)) else { return };
            ui.label(RichText::new(layer).strong());
            if let Some(t) = song.tracks.iter().find(|t| &t.name == layer) {
                let tip = if t.muted { "hear it again" } else { "silence it everywhere, keeping it in the song" };
                if ui.button(if t.muted { "Unmute" } else { "Mute" }).on_hover_text(tip).clicked() {
                    out.push(MixEdit { muted: Some(!t.muted), ..Default::default() }.on(layer));
                    ui.close();
                }
                if ui
                    .button(if t.solo { "Unsolo" } else { "Solo" })
                    .on_hover_text("while any track is soloed, only soloed tracks play")
                    .clicked()
                {
                    out.push(MixEdit { solo: Some(!t.solo), ..Default::default() }.on(layer));
                    ui.close();
                }
                if !t.swaps.is_empty() {
                    ui.separator();
                    if let Some(next) = super::next_label(t)
                        && ui
                            .button(format!("Another sound: {next}"))
                            .on_hover_text("the next that fits it and no other part plays; undo swaps back")
                            .clicked()
                    {
                        out.push(super::swap(layer, None));
                        ui.close();
                    }
                    ui.menu_button("Swap the sound to", |ui| out.extend(super::swap_list(ui, t)))
                        .response
                        .on_hover_text("choose the sound it plays from those that fit it; its notes stay");
                }
                if matches!(t.kind.as_str(), "part" | "drum") {
                    ui.separator();
                    let vary = |take: bool| Command::VaryTrack { track: layer.clone(), take };
                    if ui
                        .button("Another take")
                        .on_hover_text(
                            "written again: a part with a new seed, a lane as the genre it was written as plays it",
                        )
                        .clicked()
                    {
                        out.push(vary(true));
                        ui.close();
                    }
                    if ui
                        .button("Vary it")
                        .on_hover_text("the same take, other details: a part keeps its rhythm, a lane its strong hits")
                        .clicked()
                    {
                        out.push(vary(false));
                        ui.close();
                    }
                }
                if t.kind == "part" {
                    ui.separator();
                    ui.menu_button("Add chords under it", |ui| {
                        ui.label(small("chords that fit its notes and the song's, worked out again as it changes"));
                        for (label, style, hint) in [
                            ("A chord on each note", "block", "block chords under the melody, on its own sound"),
                            ("A pad under it", "sustain", "chords held under it, as a pad"),
                            ("Stabs", "stabs", "short chords on the beat and between"),
                            ("Offbeat", "offbeat", "a chord on each offbeat"),
                        ] {
                            if ui.button(label).on_hover_text(hint).clicked() {
                                out.push(Command::Harmonize {
                                    part: layer.clone(),
                                    style: Some(style.into()),
                                    sound: None,
                                    name: None,
                                });
                                ui.close();
                            }
                        }
                    });
                }
                if t.kind == "take" {
                    ui.separator();
                    if ui
                        .button("How is it sung?")
                        .on_hover_text("the notes it means, how near each came, and how its singer sings")
                        .clicked()
                    {
                        out.push(Command::AssessVoice { take: Some(layer.clone()), path: None, key: None });
                        ui.close();
                    }
                    if let Some(seconds) = t.seconds
                        && ui
                            .button("Trim...")
                            .on_hover_text("keep part of its recording: start it later in itself, or stop it sooner")
                            .clicked()
                    {
                        trim = Some(super::import::Trim::of(layer, seconds));
                        ui.close();
                    }
                    ui.menu_button("Tune it", |ui| {
                        ui.label(small("each note moved onto the note it means, as the song's chords read it"));
                        let tuned = t.fx.iter().any(|e| e.get("fx").and_then(|f| f.as_str()) == Some("tune"));
                        let tune = |speed: f32, humanize: f32, vibrato: f32| Command::Tune {
                            track: layer.clone(),
                            strength: Some(1.0),
                            speed_ms: Some(speed),
                            humanize: Some(humanize),
                            vibrato: Some(vibrato),
                            scale: None,
                            off: false,
                        };
                        for (label, hint, cmd) in [
                            ("Naturally", "the scoop into each note and the vibrato stay", tune(40.0, 10.0, 1.0)),
                            (
                                "Tightly",
                                "on the note within 15 ms, a little of the singer's own left",
                                tune(15.0, 3.0, 1.0),
                            ),
                            (
                                "Hard",
                                "snapped onto each note at once, held dead still: the robotic sound",
                                tune(0.0, 0.0, 0.0),
                            ),
                        ] {
                            if ui.button(label).on_hover_text(hint).clicked() {
                                out.push(cmd);
                                ui.close();
                            }
                        }
                        if tuned && ui.button("Not at all").clicked() {
                            let mut off = tune(0.0, 0.0, 0.0);
                            if let Command::Tune { off: o, .. } = &mut off {
                                *o = true;
                            }
                            out.push(off);
                            ui.close();
                        }
                    });
                    ui.menu_button("Turn it into drums", |ui| {
                        ui.label(small(
                            "its kicks, snares and hats, played or beatboxed, become the song's drum lanes, and the \
                             take goes quiet",
                        ));
                        for (label, bars) in [("1 bar", 1), ("2 bars", 2), ("4 bars", 4)] {
                            if ui
                                .button(label)
                                .on_hover_text("the pattern's length, folded from every time round")
                                .clicked()
                            {
                                out.push(Command::TranscribeDrums {
                                    take: Some(layer.clone()),
                                    path: None,
                                    bars: Some(bars),
                                    variant: None,
                                    bpm: None,
                                    keep_take: false,
                                });
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Turn the melody into notes", |ui| {
                        ui.label(small(
                            "a new part plays what was sung, in the key and on the grid, and the take goes quiet",
                        ));
                        ui.label(small("as").weak());
                        ui.horizontal_wrapped(|ui| {
                            for s in claudius_api::NOTE_STYLES {
                                ui.selectable_value(&mut style, s.to_string(), *s);
                            }
                        });
                        for (heading, names) in [("instruments", &instruments), ("synths", &synths)] {
                            if names.is_empty() {
                                continue;
                            }
                            ui.label(small(heading).weak());
                            for s in names {
                                if ui.button(s).clicked() {
                                    out.push(Command::TranscribeTake {
                                        take: layer.clone(),
                                        sounds: vec![s.clone()],
                                        name: None,
                                        role: None,
                                        chromatic: false,
                                        keep_take: false,
                                        style: Some(style.clone()),
                                    });
                                    ui.close();
                                }
                            }
                        }
                    });
                }
            }
            if ui.button("On in every section").on_hover_text("it plays all through the song").clicked() {
                for (i, s) in song.sections.iter().enumerate() {
                    if !s.layers.contains(layer) {
                        out.push(Command::ToggleLayer { section: i, layer: layer.clone() });
                    }
                }
                ui.close();
            }
            let tip = "silent all through the song, but kept, to switch on again where you like";
            if ui.button("Off in every section").on_hover_text(tip).clicked() {
                for (i, s) in song.sections.iter().enumerate() {
                    if s.layers.contains(layer) {
                        out.push(Command::ToggleLayer { section: i, layer: layer.clone() });
                    }
                }
                ui.close();
            }
            ui.separator();
            if ui
                .button(RichText::new("Remove the track").color(theme::BAD))
                .on_hover_text("out of the song and every section; Undo brings it back")
                .clicked()
            {
                out.push(Command::RemoveTrack { name: layer.clone() });
                ui.close();
            }
        });
        // a track more, under the last
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let add = ui.button(RichText::new("+ Track").size(11.5).color(TEXT_DIM)).on_hover_text(
                "add a track: one written in a style, a part or a drum lane of your own, or a recording",
            );
            // a form: choosing its role or sound keeps it open
            egui::Popup::from_toggle_button_response(&add)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| match add_track_menu(ui, &mut add_form, song, genres, sounds) {
                    Some(Added::Send(c)) => out.push(c),
                    Some(a) => opened = Some(a),
                    None => {}
                });
        });
    });

    // waveform
    let wave = Rect::from_min_max(pos2(tl.x0, full.bottom() - wave_h - 14.0), pos2(tl.x1, full.bottom() - 10.0));
    let gutter = Rect::from_min_max(pos2(full.left() + 8.0, wave.top()), pos2(tl.x0 - 8.0, wave.bottom()));
    painter.rect_filled(wave.expand2(vec2(0.0, 2.0)), 4.0, WELL);
    painter.text(gutter.left_top() + vec2(2.0, 10.0), Align2::LEFT_CENTER, "MASTER", theme::prop(10.0), TEXT_DIM);
    match render {
        Some(r) if !r.waveform.is_empty() => {
            painter.text(
                gutter.left_top() + vec2(2.0, 26.0),
                Align2::LEFT_CENTER,
                format!("{:.1} LUFS", r.lufs),
                theme::mono(10.5),
                TEXT,
            );
            painter.text(
                gutter.left_top() + vec2(2.0, 40.0),
                Align2::LEFT_CENTER,
                format!("{:.1} dBTP", r.true_peak_db),
                theme::mono(10.5),
                TEXT_DIM,
            );
            let mid = wave.center().y;
            let half = wave.height() / 2.0 - 3.0;
            let secs_x = |t: f64| tl.x(t * r.bpm.max(1.0) / 240.0);
            let color_at = |t: f64| -> Color32 {
                let name = r.sections.iter().find(|(_, a, b)| t >= *a && t < *b).map(|(n, _, _)| n.as_str());
                let kind = name.and_then(|n| song.sections.iter().find(|s| s.name == n)).map(|s| s.kind.as_str());
                theme::section_color(kind.unwrap_or("verse"))
            };
            let end_x = secs_x(r.seconds).min(wave.right());
            let len = r.waveform.len();
            let mut x = wave.left();
            while x < end_x {
                let t0 = tl.bar(x) * 240.0 / r.bpm.max(1.0);
                let t1 = tl.bar(x + 1.0) * 240.0 / r.bpm.max(1.0);
                let i0 = ((t0 / r.seconds.max(1e-6)) * len as f64) as usize;
                let i1 = (((t1 / r.seconds.max(1e-6)) * len as f64).ceil() as usize).max(i0 + 1).min(len);
                let peak = r.waveform.get(i0.min(len - 1)..i1).map_or(0.0, |w| w.iter().copied().fold(0.0, f32::max));
                let h = (peak.clamp(0.0, 1.0) * half).max(0.5);
                let c = color_at(t0);
                let played = has_audio && t0 < position;
                let c = if played { theme::mix(c, Color32::WHITE, 0.25) } else { theme::alpha(c, 0.62) };
                painter.line_segment([pos2(x + 0.5, mid - h), pos2(x + 0.5, mid + h)], Stroke::new(1.0, c));
                x += 1.0;
            }
            painter.line_segment(
                [pos2(wave.left(), mid), pos2(end_x, mid)],
                Stroke::new(1.0, theme::alpha(Color32::BLACK, 0.35)),
            );
            for (_, a, _) in &r.sections {
                let x = secs_x(*a);
                painter.line_segment(
                    [pos2(x, wave.top()), pos2(x, wave.bottom())],
                    Stroke::new(1.0, theme::alpha(Color32::WHITE, 0.08)),
                );
            }
            if loop_region {
                let (a, b) = if cx.view.loop_section {
                    song.sections.get(cx.view.section).map_or((0.0, r.seconds), |s| {
                        (s.start_bar as f64 * 240.0 / bpm_audio, (s.start_bar + s.bars) as f64 * 240.0 / bpm_audio)
                    })
                } else {
                    (0.0, r.seconds)
                };
                let lr =
                    Rect::from_min_max(pos2(secs_x(a), wave.top()), pos2(secs_x(b).min(wave.right()), wave.bottom()));
                painter.rect_filled(lr, 0.0, theme::alpha(ACCENT, 0.07));
                painter.line_segment([lr.left_top(), lr.right_top()], Stroke::new(2.0, theme::alpha(ACCENT, 0.8)));
            }
            if stale {
                let note = if r.url == claudius_api::PREVIEW_URL {
                    "a quick preview, while the whole song renders"
                } else {
                    "this is the song before its latest changes"
                };
                let galley = painter.layout_no_wrap(note.to_string(), theme::prop(11.5), TEXT_DIM);
                let at = wave.right_top() + vec2(-8.0 - galley.size().x, 4.0);
                painter.rect_filled(
                    egui::Rect::from_min_size(at, galley.size()).expand(3.0),
                    3.0,
                    theme::alpha(WELL, 0.85),
                );
                painter.galley(at, galley, TEXT_DIM);
            }
            let resp = ui.interact(wave, ui.scope_id().with("wave"), Sense::click_and_drag());
            if (resp.clicked() || resp.dragged())
                && let Some(p) = resp.interact_pointer_pos()
            {
                cx.player.seek(tl.bar(p.x) * 240.0 / r.bpm.max(1.0));
            }
            if let Some(p) = resp.hover_pos() {
                painter.line_segment(
                    [pos2(p.x, wave.top()), pos2(p.x, wave.bottom())],
                    Stroke::new(1.0, theme::alpha(Color32::WHITE, 0.35)),
                );
                let t = tl.bar(p.x) * 240.0 / r.bpm.max(1.0);
                resp.on_hover_text(format!("{}  (bar {:.1}), click to seek", super::fmt_time(t), tl.bar(p.x) + 1.0));
            }
        }
        _ => {
            let mut wave_ui = ui.new_child(UiBuilder::new().max_rect(wave));
            wave_ui.vertical_centered(|ui| {
                ui.add_space((wave.height() / 2.0 - 22.0).max(0.0));
                ui.label(
                    RichText::new(if cx.state.busy.is_some() { "rendering..." } else { "No render yet" })
                        .color(TEXT_DIM),
                );
                if cx.state.busy.is_none() && widgets::tool_button(ui, Icon::Render, "Render now", true, true).clicked()
                {
                    out.push(Command::Render { stems: false });
                }
            });
        }
    }

    // playhead across everything
    if has_audio {
        let x = tl.x(head);
        if x >= tl.x0 && x <= tl.x1 {
            painter.line_segment([pos2(x, ruler.top()), pos2(x, wave.bottom())], Stroke::new(1.5, PLAYHEAD));
            painter.add(Shape::convex_polygon(
                vec![pos2(x - 5.0, ruler.top()), pos2(x + 5.0, ruler.top()), pos2(x, ruler.top() + 7.0)],
                ACCENT,
                Stroke::NONE,
            ));
        }
    }
    painter.line_segment([pos2(tl.x0, ruler.bottom()), pos2(tl.x1, ruler.bottom())], Stroke::new(1.0, LINE));
    ui.advance_cursor_after_rect(full);
    cx.view.melody_style = style;
    cx.view.row_menu = row_menu;
    if trim.is_some() {
        cx.view.import.trim = trim;
    }
    match opened {
        Some(Added::Search(words)) => cx.view.import.search_for(&words),
        Some(Added::Record) => cx.view.record.show(),
        Some(Added::Send(_)) | None => {}
    }
    cx.view.add_track = add_form;
    cx.out.extend(out);
}
