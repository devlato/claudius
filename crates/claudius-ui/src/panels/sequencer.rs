//! The step sequencer: one row per drum lane, one cell per 1/16 step of the lane's pattern (`pattern_bars * 16`),
//! for one variant at a time. Click toggles a step (velocity 0.8), dragging paints, shift-click / shift-drag sets the
//! velocity, right-click offers velocity presets and whole-lane patterns.

use claudius_api::Command;
use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::{Cx, Gesture, empty, layer_color, page_frame, small};
use crate::cmds::MixEdit;
use crate::edits::steps_to_string;
use crate::theme::{self, ACCENT, LINE, PLAYHEAD, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets;

const LABEL_W: f32 = 168.0;

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let head = cx.playhead_bar();
    let playing = cx.player.is_playing();
    let Some(song) = cx.state.song.as_mut() else {
        empty(ui, "No song yet.");
        return;
    };
    if song.drums.is_empty() {
        empty(ui, "This song has no drum lanes.");
        return;
    }
    let mut variants: Vec<String> = Vec::new();
    for d in &song.drums {
        for (v, _) in &d.variants {
            if !variants.contains(v) {
                variants.push(v.clone());
            }
        }
    }
    if !variants.contains(&cx.view.variant) {
        cx.view.variant = variants.first().cloned().unwrap_or_else(|| "main".into());
    }
    let variant = cx.view.variant.clone();
    let mut out: Vec<Command> = Vec::new();

    page_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::heading(ui, "Variant");
            for v in &variants {
                let used: Vec<&str> =
                    song.sections.iter().filter(|s| &s.variant == v).map(|s| s.name.as_str()).collect();
                let r = widgets::chip(ui, v == &variant, v, ACCENT).on_hover_text(if used.is_empty() {
                    "no section uses this variant".to_string()
                } else {
                    format!("used by {}", used.join(", "))
                });
                if r.clicked() {
                    cx.view.variant = v.clone();
                }
            }
            ui.add_space(10.0);
            let used: Vec<&str> =
                song.sections.iter().filter(|s| s.variant == variant).map(|s| s.name.as_str()).collect();
            ui.label(small(if used.is_empty() {
                "no section uses this variant".to_string()
            } else {
                format!("plays in {}", used.join(", "))
            }));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(small("click toggles · drag paints · shift-drag sets velocity · right-click for more"));
            });
        });
        ui.add_space(6.0);

        let max_steps = song.drums.iter().map(|d| (d.pattern_bars.max(1) * 16) as usize).max().unwrap_or(16);
        let avail = ui.available_width() - LABEL_W - 8.0;
        let groups = (max_steps / 4) as f32;
        let cell_w = ((avail - groups * 3.0) / max_steps as f32).clamp(9.0, 30.0);
        let row_h = ((ui.available_height() - 26.0) / song.drums.len() as f32).clamp(18.0, 30.0);
        let step_x = |s: usize| -> f32 { s as f32 * cell_w + (s / 4) as f32 * 3.0 + (s / 16) as f32 * 4.0 };
        let grid_w = step_x(max_steps) + 4.0;

        egui::ScrollArea::both().id_salt("seq_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let total_h = 20.0 + row_h * song.drums.len() as f32;
            let (rect, resp) = ui.allocate_exact_size(vec2(LABEL_W + grid_w, total_h), Sense::click_and_drag());
            let p = ui.painter();
            let gx = rect.left() + LABEL_W;
            let gy = rect.top() + 20.0;
            // bar / beat numbers
            for s in (0..max_steps).step_by(4) {
                let x = gx + step_x(s);
                let bar_start = s % 16 == 0;
                let label = if bar_start { format!("{}", s / 16 + 1) } else { format!("{}", (s % 16) / 4 + 1) };
                p.text(
                    pos2(x + 2.0, rect.top() + 9.0),
                    Align2::LEFT_CENTER,
                    label,
                    theme::prop(if bar_start { 11.0 } else { 9.5 }),
                    if bar_start { TEXT } else { TEXT_FAINT },
                );
            }
            // the playhead column (when the section under it plays this variant)
            let mut play_col: Option<usize> = None;
            if playing
                && let Some(s) =
                    song.sections.iter().find(|s| head >= s.start_bar as f64 && head < (s.start_bar + s.bars) as f64)
                && s.variant == variant
            {
                let in_sec = head - s.start_bar as f64;
                let bars = (max_steps / 16).max(1) as f64;
                play_col = Some(((in_sec % bars) * 16.0) as usize);
            }
            let n_lanes = song.drums.len();
            let cell_at = |pos: egui::Pos2| -> Option<(usize, usize)> {
                if pos.x < gx || pos.y < gy {
                    return None;
                }
                let lane = ((pos.y - gy) / row_h) as usize;
                let x = pos.x - gx;
                let step = (0..max_steps).find(|&s| x >= step_x(s) && x < step_x(s) + cell_w)?;
                (lane < n_lanes).then_some((lane, step))
            };
            let hover = resp.hover_pos().and_then(cell_at);
            let solo = super::any_solo(song);
            for (li, d) in song.drums.iter().enumerate() {
                let y = gy + li as f32 * row_h;
                let color = layer_color(song, &d.lane);
                let track = song.tracks.iter().find(|t| t.name == d.lane);
                let soloed = track.is_some_and(|t| t.solo);
                let muted = track.is_some_and(|t| t.muted) || (solo && !soloed);
                let has = d.variants.iter().any(|(v, _)| *v == variant);
                let steps = d
                    .variants
                    .iter()
                    .find(|(v, _)| *v == variant)
                    .or_else(|| d.variants.iter().find(|(v, _)| v == "main"))
                    .or(d.variants.first())
                    .map(|(_, s)| s.as_slice())
                    .unwrap_or(&[]);
                let len = (d.pattern_bars.max(1) * 16) as usize;
                let label = Rect::from_min_size(pos2(rect.left(), y + 1.0), vec2(LABEL_W - 10.0, row_h - 2.0));
                p.rect_filled(label, 4.0, RAISED);
                p.rect_filled(
                    Rect::from_min_size(label.min, vec2(4.0, label.height())),
                    2.0,
                    if muted { TEXT_FAINT } else { color },
                );
                p.text(
                    label.left_center() + vec2(12.0, 0.0),
                    Align2::LEFT_CENTER,
                    &d.lane,
                    theme::prop(12.5),
                    if muted { TEXT_FAINT } else { TEXT },
                );
                let note = if !has {
                    "uses main".to_string()
                } else {
                    format!("{} bar{}", d.pattern_bars, if d.pattern_bars == 1 { "" } else { "s" })
                };
                p.text(
                    label.right_center() - vec2(54.0, 0.0),
                    Align2::RIGHT_CENTER,
                    note,
                    theme::prop(9.5),
                    if has { TEXT_FAINT } else { theme::WARN },
                );
                // mute and solo buttons
                let is_muted = track.is_some_and(|t| t.muted);
                let m = Rect::from_center_size(label.right_center() - vec2(14.0, 0.0), vec2(20.0, 16.0));
                let sb = Rect::from_center_size(label.right_center() - vec2(38.0, 0.0), vec2(20.0, 16.0));
                p.rect(
                    m,
                    3.0,
                    if is_muted { theme::alpha(theme::BAD, 0.75) } else { WELL },
                    Stroke::new(1.0, LINE),
                    StrokeKind::Inside,
                );
                p.text(
                    m.center(),
                    Align2::CENTER_CENTER,
                    "M",
                    theme::prop(10.0),
                    if is_muted { Color32::WHITE } else { TEXT_DIM },
                );
                p.rect(sb, 3.0, if soloed { theme::SOLO } else { WELL }, Stroke::new(1.0, LINE), StrokeKind::Inside);
                p.text(
                    sb.center(),
                    Align2::CENTER_CENTER,
                    "S",
                    theme::prop(10.0),
                    if soloed { Color32::BLACK } else { TEXT_DIM },
                );
                if resp.clicked()
                    && let Some(pos) = resp.interact_pointer_pos()
                {
                    if m.contains(pos) {
                        out.push(MixEdit { muted: Some(!is_muted), ..Default::default() }.on(&d.lane));
                    } else if sb.contains(pos) {
                        out.push(MixEdit { solo: Some(!soloed), ..Default::default() }.on(&d.lane));
                    }
                }
                for s in 0..max_steps {
                    let x = gx + step_x(s);
                    let cell = Rect::from_min_size(pos2(x, y + 2.0), vec2(cell_w - 2.0, row_h - 4.0));
                    let ghost = s >= len;
                    let v = steps.get(s % len.max(1)).copied().unwrap_or(0.0);
                    let group_dark = (s / 4) % 2 == 0;
                    let off = if group_dark { theme::mix(WELL, RAISED, 0.55) } else { theme::mix(WELL, RAISED, 0.95) };
                    let mut fill = off;
                    if v > 0.0 {
                        fill = theme::mix(off, color, 0.35 + 0.65 * v);
                        if !has || muted {
                            fill = theme::mix(off, fill, 0.5);
                        }
                    }
                    if ghost {
                        fill = theme::mix(fill, WELL, 0.55);
                    }
                    p.rect_filled(cell, 3.0, fill);
                    if v > 0.0 && !ghost {
                        let h = (cell.height() - 4.0) * v;
                        let meter = Rect::from_min_max(
                            pos2(cell.left() + 2.0, cell.bottom() - 2.0 - h),
                            pos2(cell.left() + 4.0, cell.bottom() - 2.0),
                        );
                        p.rect_filled(meter, 1.0, theme::alpha(Color32::WHITE, 0.55));
                        if v >= claudius_api::ACCENT {
                            p.circle_filled(pos2(cell.right() - 4.5, cell.top() + 4.5), 2.0, Color32::WHITE);
                        }
                    }
                    if play_col == Some(s) {
                        p.rect_stroke(cell, 3.0, Stroke::new(1.5, PLAYHEAD), StrokeKind::Inside);
                    } else if hover == Some((li, s)) {
                        p.rect_stroke(
                            cell,
                            3.0,
                            Stroke::new(1.0, theme::alpha(Color32::WHITE, 0.6)),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
            if let Some(c) = play_col {
                let x = gx + step_x(c) + cell_w / 2.0 - 1.0;
                p.line_segment([pos2(x, rect.top() + 16.0), pos2(x, gy)], Stroke::new(2.0, ACCENT));
            }

            // gestures
            let (pressed, down, shift, pointer) = ui.input(|i| {
                (i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers.shift, i.pointer.interact_pos())
            });
            if pressed
                && resp.hovered()
                && let Some((li, s)) = hover
            {
                let (lane, len, steps) = lane_steps(song, li, &variant);
                let step = s % len.max(1);
                let v = steps.get(step).copied().unwrap_or(0.0);
                if shift {
                    let cell_top = gy + li as f32 * row_h + 2.0;
                    let y = pointer.map_or(cell_top, |p| p.y);
                    let nv = (1.0 - (y - cell_top) / (row_h - 4.0)).clamp(0.1, 1.0);
                    set_local(song, li, &variant, step, nv);
                    cx.view.gesture = Some(Gesture::Velocity { lane: li, step, start: nv, y0: y });
                } else {
                    let on = v <= 0.0;
                    out.push(Command::SetDrumStep {
                        lane,
                        variant: variant.clone(),
                        step,
                        velocity: if on { 0.8 } else { 0.0 },
                    });
                    cx.view.gesture = Some(Gesture::Paint { on, touched: vec![(li, step)] });
                }
            } else if down {
                match &mut cx.view.gesture {
                    Some(Gesture::Paint { on, touched }) => {
                        if let Some((li, s)) = pointer.and_then(cell_at) {
                            let (lane, len, steps) = lane_steps(song, li, &variant);
                            let step = s % len.max(1);
                            if !touched.contains(&(li, step)) {
                                touched.push((li, step));
                                let v = steps.get(step).copied().unwrap_or(0.0);
                                if (v > 0.0) != *on {
                                    out.push(Command::SetDrumStep {
                                        lane,
                                        variant: variant.clone(),
                                        step,
                                        velocity: if *on { 0.8 } else { 0.0 },
                                    });
                                }
                            }
                        }
                    }
                    Some(Gesture::Velocity { lane, step, start, y0 }) => {
                        if let Some(p) = pointer {
                            let nv = (*start - (p.y - *y0) / 90.0).clamp(0.05, 1.0);
                            set_local(song, *lane, &variant, *step, nv);
                        }
                    }
                    None => {}
                }
            } else if let Some(g) = cx.view.gesture.take()
                && let Gesture::Velocity { lane, step, .. } = g
            {
                let (name, _, steps) = lane_steps(song, lane, &variant);
                let v = steps.get(step).copied().unwrap_or(0.0);
                out.push(Command::SetDrumStep { lane: name, variant: variant.clone(), step, velocity: v });
            }
            if let Some(Gesture::Velocity { lane, step, .. }) = &cx.view.gesture {
                let (name, _, steps) = lane_steps(song, *lane, &variant);
                let v = steps.get(*step).copied().unwrap_or(0.0);
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    ui.scope_id().with("vel_tip"),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| {
                    ui.label(format!("{name} step {}: velocity {:.2}", step + 1, v));
                });
            } else if let Some((li, s)) = hover {
                let (name, len, steps) = lane_steps(song, li, &variant);
                let v = steps.get(s % len.max(1)).copied().unwrap_or(0.0);
                let bar = s / 16 + 1;
                let beat = (s % 16) / 4 + 1;
                let tick = s % 4 + 1;
                let what = if v <= 0.0 {
                    "rest".to_string()
                } else {
                    format!(
                        "velocity {v:.2}{}",
                        if v >= claudius_api::ACCENT {
                            " (accent)"
                        } else if v < claudius_api::GHOST {
                            " (ghost)"
                        } else {
                            ""
                        }
                    )
                };
                resp.clone().on_hover_text(format!("{name} · bar {bar} beat {beat}.{tick} · {what}"));
            }
            if resp.secondary_clicked() {
                cx.view.cell_menu = pointer.and_then(|p| {
                    if p.x < gx && p.y >= gy {
                        let li = ((p.y - gy) / row_h) as usize;
                        (li < song.drums.len()).then_some((li, usize::MAX))
                    } else {
                        cell_at(p)
                    }
                });
            }
            resp.context_menu(|ui| {
                let Some((li, s)) = cx.view.cell_menu else {
                    ui.label(small("right-click a step or a lane"));
                    return;
                };
                let (lane, len, steps) = lane_steps(song, li, &variant);
                if s != usize::MAX {
                    let step = s % len.max(1);
                    ui.label(RichText::new(format!("{lane} step {}", step + 1)).strong());
                    for (label, v) in [
                        ("Accent (1.0)", 1.0),
                        ("Normal (0.8)", 0.8),
                        ("Soft (0.6)", 0.6),
                        ("Ghost (0.35)", 0.35),
                        ("Off", 0.0),
                    ] {
                        if ui.button(label).clicked() {
                            out.push(Command::SetDrumStep {
                                lane: lane.clone(),
                                variant: variant.clone(),
                                step,
                                velocity: v,
                            });
                            ui.close();
                        }
                    }
                    ui.separator();
                }
                ui.label(RichText::new(format!("{lane} · {variant}")).strong());
                let bar = |pat: &str| -> String { pat.repeat(len / 16) };
                let presets = [
                    ("Clear", bar("................")),
                    ("Every beat (x...)", bar("x...x...x...x...")),
                    ("Offbeats (..x.)", bar("..x...x...x...x.")),
                    ("8ths (x.x.)", bar("x.x.x.x.x.x.x.x.")),
                    ("16ths", bar("xoxoxoxoxoxoxoxo")),
                    ("Backbeat (2 and 4)", bar("....x.......x...")),
                    ("Two-step", bar("x.........x.....")),
                ];
                for (label, pat) in presets {
                    if ui.button(label).clicked() {
                        out.push(Command::SetDrumPattern { lane: lane.clone(), variant: variant.clone(), steps: pat });
                        ui.close();
                    }
                }
                if variant != "main" && ui.button("Copy from main").clicked() {
                    let main =
                        song.drums[li].variants.iter().find(|(v, _)| v == "main").map(|(_, s)| steps_to_string(s));
                    if let Some(main) = main {
                        out.push(Command::SetDrumPattern { lane: lane.clone(), variant: variant.clone(), steps: main });
                    }
                    ui.close();
                }
                ui.separator();
                ui.label(small(format!("pattern: {}", steps_to_string(&steps[..steps.len().min(16)]))));
            });
        });
    });
    cx.out.extend(out);
}

/// a lane's name, step count and the steps shown for `variant` (falling back to main)
fn lane_steps(song: &claudius_api::SongView, li: usize, variant: &str) -> (String, usize, Vec<f32>) {
    let d = &song.drums[li];
    let len = (d.pattern_bars.max(1) * 16) as usize;
    let steps = d
        .variants
        .iter()
        .find(|(v, _)| v == variant)
        .or_else(|| d.variants.iter().find(|(v, _)| v == "main"))
        .or(d.variants.first())
        .map(|(_, s)| s.clone())
        .unwrap_or_default();
    (d.lane.clone(), len, steps)
}

/// edit a step locally (during a velocity drag)
fn set_local(song: &mut claudius_api::SongView, lane: usize, variant: &str, step: usize, v: f32) {
    let d = &mut song.drums[lane];
    let len = (d.pattern_bars.max(1) * 16) as usize;
    if d.variants.iter().all(|(n, _)| n != variant) {
        let base = d
            .variants
            .iter()
            .find(|(n, _)| n == "main")
            .or(d.variants.first())
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| vec![0.0; len]);
        d.variants.push((variant.to_string(), base));
    }
    if let Some((_, steps)) = d.variants.iter_mut().find(|(n, _)| n == variant) {
        steps.resize(len, 0.0);
        steps[step % len] = v;
    }
}
