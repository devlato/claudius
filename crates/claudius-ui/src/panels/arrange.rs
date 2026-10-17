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
    let Some(song) = cx.state.song.as_ref() else {
        empty(ui, "No song yet: make one with New song.");
        return;
    };
    let render = cx.state.render.as_ref();
    let mut out: Vec<Command> = Vec::new();
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
            "{} sections · {} bars · {} layers",
            song.sections.len(),
            song.bars,
            song.layers.len()
        )));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for kind in SECTION_KINDS.iter().rev() {
                if song.sections.iter().any(|s| s.kind == *kind) {
                    ui.label(RichText::new(*kind).size(10.5).color(theme::section_color(kind)));
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, theme::section_color(kind));
                }
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
    painter.text(pos2(full.left() + 10.0, ruler.center().y), Align2::LEFT_CENTER, "BAR", theme::prop(9.5), TEXT_FAINT);
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
        "SECTIONS",
        theme::prop(9.5),
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
        clip.text(rect.left_top() + vec2(6.0, 10.0), Align2::LEFT_CENTER, &s.name, theme::prop(12.5), text_color);
        clip.text(
            rect.left_top() + vec2(6.0, 25.0),
            Align2::LEFT_CENTER,
            format!("{} bars · {:.0}%", s.bars, s.energy * 100.0),
            theme::prop(10.0),
            theme::alpha(text_color, 0.75),
        );

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
        let flags: Vec<&str> =
            [(s.filter_in, "filter in"), (s.build, "build"), (s.gap, "gap"), (s.impact, "impact"), (s.fills, "fills")]
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, n)| *n)
                .collect();
        let resp = resp.on_hover_text(format!(
            "{} ({}): bars {}-{}, energy {:.2}, drums '{}'{}\n{} layers: {}\nclick to inspect, drag to move, right-click for more",
            s.name,
            s.kind,
            s.start_bar + 1,
            s.start_bar + s.bars,
            s.energy,
            s.variant,
            if flags.is_empty() { String::new() } else { format!(", {}", flags.join(", ")) },
            s.layers.len(),
            s.layers.join(" ")
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
    let row_h = (rows_rect.height() / n as f32).clamp(15.0, 26.0);
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
                Rect::from_min_size(pos2(rect.left() + 8.0, y0 + 3.0), vec2(3.0, row_h - 6.0)),
                1.0,
                if muted { TEXT_FAINT } else { color },
            );
            p.text(
                pos2(rect.left() + 18.0, y0 + row_h / 2.0),
                Align2::LEFT_CENTER,
                layer,
                theme::prop(12.0),
                if muted { TEXT_FAINT } else { TEXT },
            );
            let kind = track.map(|t| t.kind.as_str()).unwrap_or(if song.drums.iter().any(|d| &d.lane == layer) {
                "drum"
            } else {
                "part"
            });
            let (tag, tag_color) = if soloed {
                ("solo", theme::SOLO)
            } else if silenced {
                ("silent", TEXT_FAINT)
            } else if muted {
                ("muted", theme::BAD)
            } else {
                (kind, TEXT_FAINT)
            };
            p.text(
                pos2(rect.left() + GUTTER - 10.0, y0 + row_h / 2.0),
                Align2::RIGHT_CENTER,
                tag,
                theme::prop(9.5),
                tag_color,
            );
            let is_drum = song.drums.iter().any(|d| &d.lane == layer);
            for s in &song.sections {
                let on = s.layers.iter().any(|l| l == layer);
                let cell = Rect::from_min_max(
                    pos2(tl.x(s.start_bar as f64) + 1.0, y0 + 2.0),
                    pos2(tl.x((s.start_bar + s.bars) as f64) - 1.0, y0 + row_h - 2.0),
                );
                if !on {
                    continue;
                }
                let base = if muted { TEXT_FAINT } else { color };
                p.rect_filled(cell, 3.0, theme::alpha(base, 0.2 + 0.32 * s.energy));
                p.line_segment(
                    [cell.left_top() + vec2(2.0, 0.5), cell.right_top() + vec2(-2.0, 0.5)],
                    Stroke::new(1.0, theme::alpha(base, 0.9)),
                );
                if is_drum {
                    for b in 0..s.bars {
                        let Some(d) = density(song, layer, &s.variant, b) else { continue };
                        let x = tl.x((s.start_bar + b) as f64);
                        let w = (px_bar - if px_bar > 3.0 { 1.0 } else { 0.0 }).max(0.8);
                        let h = (cell.height() - 4.0) * d;
                        p.rect_filled(
                            Rect::from_min_max(
                                pos2(x + 0.5, cell.bottom() - 1.0 - h),
                                pos2(x + 0.5 + w, cell.bottom() - 1.0),
                            ),
                            0.0,
                            theme::alpha(base, 0.55),
                        );
                    }
                } else if cell.width() > 46.0 {
                    p.with_clip_rect(cell).text(
                        cell.left_center() + vec2(5.0, 0.0),
                        Align2::LEFT_CENTER,
                        layer,
                        theme::prop(10.0),
                        theme::mix(base, Color32::WHITE, 0.6),
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
                    if resp.clicked() {
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
        resp.context_menu(|ui| {
            let Some(pos) = ui.ctx().input(|i| i.pointer.press_origin()) else { return };
            let row = ((pos.y - rect.top()) / row_h) as usize;
            let Some(layer) = song.layers.get(row) else { return };
            ui.label(RichText::new(layer).strong());
            if let Some(t) = song.tracks.iter().find(|t| &t.name == layer) {
                if ui.button(if t.muted { "Unmute" } else { "Mute" }).clicked() {
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
            }
            if ui.button("On in every section").clicked() {
                for (i, s) in song.sections.iter().enumerate() {
                    if !s.layers.contains(layer) {
                        out.push(Command::ToggleLayer { section: i, layer: layer.clone() });
                    }
                }
                ui.close();
            }
            if ui.button("Off in every section").clicked() {
                for (i, s) in song.sections.iter().enumerate() {
                    if s.layers.contains(layer) {
                        out.push(Command::ToggleLayer { section: i, layer: layer.clone() });
                    }
                }
                ui.close();
            }
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
                let note = if cx.state.busy.is_some() {
                    "older than the song: the new render is on its way"
                } else {
                    "older than the song: press Render"
                };
                let galley = painter.layout_no_wrap(note.to_string(), theme::prop(10.5), theme::WARN);
                let at = wave.right_top() + vec2(-8.0 - galley.size().x, 4.0);
                painter.rect_filled(
                    egui::Rect::from_min_size(at, galley.size()).expand(3.0),
                    3.0,
                    theme::alpha(WELL, 0.85),
                );
                painter.galley(at, galley, theme::WARN);
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
    cx.out.extend(out);
}
