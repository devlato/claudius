//! The step sequencer: one row per drum lane, one cell per 1/16 step of the lane's pattern (`pattern_bars * 16`),
//! for one variant at a time. Click toggles a step (velocity 0.8), dragging paints, shift-click / shift-drag sets the
//! velocity, right-click offers velocity presets, the step's trig (its chance, condition, ratchet, flam, nudge and
//! locks) and whole-lane patterns. A step with a trig shows what it does on its cell.

use claudius_api::Command;
use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::{Cx, Gesture, empty, layer_color, page_frame, small};
use crate::cmds::MixEdit;
use crate::edits::steps_to_string;
use crate::theme::{self, ACCENT, LINE, PLAYHEAD, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::{self, Icon};

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
        widgets::row(ui, "variants", |ui, side| {
            if side == widgets::Side::Right {
                ui.label(small("click toggles · drag paints · shift-drag sets velocity · right-click for more"));
                return;
            }
            widgets::heading(ui, "Variant");
            for v in &variants {
                let used: Vec<&str> =
                    song.sections.iter().filter(|s| &s.variant == v).map(|s| s.name.as_str()).collect();
                let (shown, about) = claudius_api::words::variant_in_words(v);
                let r = widgets::chip(ui, v == &variant, &shown, ACCENT).on_hover_text(format!(
                    "{}\n{}",
                    super::named(Some(about), v),
                    if used.is_empty() {
                        "no section uses this variant".to_string()
                    } else {
                        format!("used by {}", used.join(", "))
                    }
                ));
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
                // a lane of its own length plays that many steps: those after it are faint
                let len = lane_len(d);
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
                } else if d.length.is_some() || d.speed != 1.0 || d.swing.is_some() || d.groove.is_some() {
                    lane_time(d)
                } else {
                    format!("{} bar{}", d.pattern_bars, if d.pattern_bars == 1 { "" } else { "s" })
                };
                // on the label, a button that swaps its sound for the next, where its length is written
                let swap = Rect::from_center_size(label.right_center() - vec2(64.0, 0.0), vec2(20.0, 16.0));
                let next = track.and_then(super::next_label);
                let pointer = resp.hover_pos();
                if next.is_some() && pointer.is_some_and(|h| label.contains(h)) {
                    let over = pointer.is_some_and(|h| swap.contains(h));
                    if over {
                        p.rect_filled(swap, 3.0, theme::CONTROL_HI);
                    }
                    let icon = Rect::from_center_size(swap.center(), vec2(12.0, 12.0));
                    widgets::paint_icon(p, icon, Icon::Swap, if over { TEXT } else { TEXT_DIM });
                    if let Some(next) = next.as_ref().filter(|_| over) {
                        resp.clone().on_hover_text(format!("another sound for {}: {next}", d.lane));
                    }
                } else {
                    p.text(
                        label.right_center() - vec2(54.0, 0.0),
                        Align2::RIGHT_CENTER,
                        note,
                        theme::prop(9.5),
                        if has { TEXT_FAINT } else { theme::WARN },
                    );
                }
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
                    } else if next.is_some() && swap.contains(pos) {
                        out.push(super::swap(&d.lane, None));
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
                    let trig = trig_of(d, &variant, s % len.max(1)).filter(|_| v > 0.0 && !ghost);
                    // a nudged step sits off the grid by as much
                    let grid_cell = cell;
                    let cell = cell.translate(vec2(trig.map_or(0.0, |t| t.nudge) * cell_w, 0.0));
                    if cell != grid_cell {
                        p.rect_filled(grid_cell, 3.0, off);
                    }
                    p.rect_filled(cell, 3.0, fill);
                    if let Some(tr) = trig {
                        draw_trig(p, cell, tr);
                    }
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
                let trig = trig_of(&song.drums[li], &variant, s % len.max(1)).map(trig_words).unwrap_or_default();
                let trig = if trig.is_empty() || v <= 0.0 { String::new() } else { format!(" · {trig}") };
                resp.clone().on_hover_text(format!("{name} · bar {bar} beat {beat}.{tick} · {what}{trig}"));
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
                    let current = trig_of(&song.drums[li], &variant, step).cloned();
                    ui.menu_button("Trig", |ui| {
                        trig_editor(ui, &mut cx.view.trig_edit, &lane, &variant, step, current, &mut out);
                    });
                    ui.separator();
                }
                lane_menu(ui, &song.drums[li], song, &mut out);
                ui.separator();
                ui.label(RichText::new(format!("{lane} · {variant}")).strong());
                let bar = |pat: &str| -> String { pat.repeat((len / 16).max(1)) };
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

/// the trig of step `step` of `d`'s variant `variant` (main's when the lane has no such variant, as its steps are)
fn trig_of<'a>(d: &'a claudius_api::DrumView, variant: &str, step: usize) -> Option<&'a claudius_api::TrigView> {
    let shown = if d.variants.iter().any(|(v, _)| v == variant) { variant } else { "main" };
    d.trigs.iter().find(|(v, _)| v == shown)?.1.iter().find(|(s, _)| *s == step).map(|(_, t)| t)
}

/// what a trig does, in a few words, for a step's tooltip
fn trig_words(t: &claudius_api::TrigView) -> String {
    let d = claudius_api::TrigView::default();
    let mut w = Vec::new();
    if t.prob < 1.0 {
        w.push(format!("{:.0}% of the time", t.prob * 100.0));
    }
    if t.cond != d.cond {
        w.push(format!("only {}", t.cond));
    }
    if t.ratchet > 1 {
        w.push(format!("{} strokes", t.ratchet));
    }
    if t.flam > 0.0 {
        w.push(format!("flam {:.0} ms", t.flam));
    }
    if t.nudge != 0.0 {
        w.push(format!("{} {:.2}", if t.nudge < 0.0 { "early" } else { "late" }, t.nudge.abs()));
    }
    if t.pitch != 0 {
        w.push(format!("{:+} st", t.pitch));
    }
    if t.decay != d.decay || t.tone != d.tone || t.pan != d.pan {
        w.push("sound locked".into());
    }
    w.join(", ")
}

/// a trig's marks on its cell (a nudge has moved the cell itself): a ratchet's strokes as the cell cut into as many,
/// its chance, its condition, a dot for a flam, a diamond for what it locks in the sound
fn draw_trig(p: &egui::Painter, cell: Rect, t: &claudius_api::TrigView) {
    let ink = theme::alpha(Color32::WHITE, 0.9);
    let font = theme::prop(8.0);
    if t.ratchet > 1 {
        let n = t.ratchet as usize;
        for k in 1..n {
            let x = cell.left() + k as f32 * cell.width() / n as f32;
            p.line_segment(
                [pos2(x, cell.top() + 1.0), pos2(x, cell.bottom() - 1.0)],
                Stroke::new(1.5, theme::alpha(Color32::BLACK, 0.55)),
            );
        }
    }
    if t.prob < 1.0 {
        p.text(
            cell.right_bottom() - vec2(2.0, 1.0),
            Align2::RIGHT_BOTTOM,
            format!("{:.0}", t.prob * 100.0),
            font.clone(),
            ink,
        );
    }
    let d = claudius_api::TrigView::default();
    if t.cond != d.cond {
        let short = match t.cond.as_str() {
            "first" => "F".to_string(),
            "last" => "L".to_string(),
            "!first" => "!F".to_string(),
            "!last" => "!L".to_string(),
            other => other.to_string(),
        };
        p.text(pos2(cell.center().x, cell.top() + 1.5), Align2::CENTER_TOP, short, font.clone(), ink);
    }
    if t.flam > 0.0 {
        p.circle_filled(pos2(cell.left() - 1.5, cell.center().y), 2.0, ink);
    }
    if t.pitch != 0 || t.decay != d.decay || t.tone != d.tone || t.pan != d.pan {
        let c = pos2(cell.right() - 4.5, cell.bottom() - 11.0);
        let r = 2.4;
        p.add(egui::Shape::convex_polygon(
            vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)],
            ink,
            Stroke::NONE,
        ));
    }
}

/// a slider or a drag let go, or a value typed or stepped: the moment to send it
fn let_go(r: &egui::Response) -> bool {
    r.drag_stopped() || (r.changed() && !r.dragged())
}

/// The step's trig as controls; what changes is sent as `set_trig` when a control is let go. The menu edits its own
/// copy, taken up afresh for another step.
fn trig_editor(
    ui: &mut Ui,
    edit: &mut Option<(String, String, usize, claudius_api::TrigView)>,
    lane: &str,
    variant: &str,
    step: usize,
    current: Option<claudius_api::TrigView>,
    out: &mut Vec<Command>,
) {
    let mut t = match edit.as_ref() {
        Some((l, v, s, t)) if l == lane && v == variant && *s == step => t.clone(),
        _ => current.unwrap_or_default(),
    };
    let mut send = false;
    egui::Grid::new(("trig-edit", lane, step)).num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label(small("chance"));
        let r = ui.add(egui::Slider::new(&mut t.prob, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
        send |= let_go(&r);
        ui.end_row();
        ui.label(small("plays"));
        let before = t.cond.clone();
        egui::ComboBox::from_id_salt(("trig-cond", lane, step)).selected_text(&t.cond).show_ui(ui, |ui| {
            for (c, what) in [
                ("always", "every time"),
                ("1:2", "the first of every two times round"),
                ("2:2", "the second of every two"),
                ("1:4", "the first of every four"),
                ("4:4", "the fourth of every four"),
                ("first", "the first time round in a section"),
                ("last", "the last time round in a section"),
                ("!first", "every time but the first"),
                ("!last", "every time but the last"),
            ] {
                ui.selectable_value(&mut t.cond, c.to_string(), c).on_hover_text(what);
            }
        });
        send |= t.cond != before;
        ui.end_row();
        ui.label(small("ratchet"));
        send |= let_go(&ui.add(egui::Slider::new(&mut t.ratchet, 1..=8).suffix(" strokes")));
        ui.end_row();
        ui.label(small("ramp"));
        send |= let_go(&ui.add(egui::Slider::new(&mut t.ramp, -1.0..=1.0)).on_hover_text("-1 fades, 1 swells"));
        ui.end_row();
        ui.label(small("flam"));
        send |= let_go(&ui.add(egui::Slider::new(&mut t.flam, 0.0..=60.0).suffix(" ms")));
        ui.end_row();
        ui.label(small("nudge"));
        let r = ui
            .add(egui::Slider::new(&mut t.nudge, -0.5..=0.5).suffix(" step"))
            .on_hover_text("below 0 early, above late");
        send |= let_go(&r);
        ui.end_row();
        ui.label(small("pitch"));
        send |= let_go(&ui.add(egui::Slider::new(&mut t.pitch, -12..=12).suffix(" st")));
        ui.end_row();
        ui.label(small("decay"));
        send |= let_go(&ui.add(egui::Slider::new(&mut t.decay, 0.1..=4.0).logarithmic(true).prefix("x")));
        ui.end_row();
        ui.label(small("tone"));
        send |=
            let_go(&ui.add(egui::Slider::new(&mut t.tone, -1.0..=1.0)).on_hover_text("darker below 0, brighter above"));
        ui.end_row();
        ui.label(small("pan"));
        send |= let_go(&ui.add(egui::Slider::new(&mut t.pan, -1.0..=1.0)));
        ui.end_row();
    });
    if ui.button("Clear the trig").clicked() {
        (t, send) = (claudius_api::TrigView::default(), true);
    }
    *edit = Some((lane.to_string(), variant.to_string(), step, t.clone()));
    if send {
        let trig = (t != claudius_api::TrigView::default()).then_some(t);
        out.push(Command::SetTrig { lane: lane.to_string(), variant: variant.to_string(), step, trig });
    }
}

/// a lane's own time and feel, in a few words for its label: "12 steps · x0.75 · dilla"
fn lane_time(d: &claudius_api::DrumView) -> String {
    let mut w = Vec::new();
    if let Some(l) = d.length {
        w.push(format!("{l} steps"));
    }
    if d.speed != 1.0 {
        w.push(format!("x{}", d.speed));
    }
    if let Some(sw) = d.swing {
        w.push(format!("{:.0}% swing", sw * 100.0));
    }
    if let Some(g) = &d.groove {
        w.push(g.clone());
    }
    if d.late_ms != 0.0 {
        w.push(late_text(d.late_ms));
    }
    w.join(" · ")
}

/// how far off the beat a lane plays, for its label and its slider: "20 ms late", "5 ms early", "on the beat"
fn late_text(ms: f32) -> String {
    match ms {
        ms if ms.abs() < 0.5 => "on the beat".into(),
        ms if ms > 0.0 => format!("{ms:.0} ms late"),
        ms => format!("{:.0} ms early", -ms),
    }
}

/// A lane's own time and feel as controls: its length in steps (the pattern's, or its own), its speed, its swing
/// (the song's, or its own), its groove (one of the song's or a template) and how much of it, and how far behind or
/// ahead of the beat it plays. Each sends `set_lane` when let go.
fn lane_menu(ui: &mut Ui, d: &claudius_api::DrumView, song: &claudius_api::SongView, out: &mut Vec<Command>) {
    let set = |length: Option<u32>,
               speed: Option<f32>,
               swing: Option<f32>,
               groove: Option<String>,
               amount: Option<f32>| {
        Command::SetLane { lane: d.lane.clone(), length, speed, swing, groove, groove_amount: amount, late_ms: None }
    };
    ui.label(RichText::new(format!("{}'s time", d.lane)).strong());
    egui::Grid::new(("lane-time", &d.lane)).num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label(small("length"));
        let mut length = d.length.unwrap_or(0);
        let r = ui
            .add(
                egui::Slider::new(&mut length, 0..=d.pattern_bars.max(1) * 16)
                    .custom_formatter(|v, _| if v < 1.0 { "its pattern".into() } else { format!("{v} steps") }),
            )
            .on_hover_text("a length of its own: 12 against the bar's 16 is a polymeter");
        if let_go(&r) {
            out.push(set(Some(length), None, None, None, None));
        }
        ui.end_row();
        ui.label(small("speed"));
        ui.horizontal(|ui| {
            for (label, v) in [("¼", 0.25), ("½", 0.5), ("¾", 0.75), ("1", 1.0), ("1½", 1.5), ("2", 2.0)] {
                if ui.selectable_label(d.speed == v, label).on_hover_text(format!("x{v}")).clicked() {
                    out.push(set(None, Some(v), None, None, None));
                }
            }
        });
        ui.end_row();
        ui.label(small("swing"));
        ui.horizontal(|ui| {
            if ui.selectable_label(d.swing.is_none(), "the song's").clicked() && d.swing.is_some() {
                out.push(set(None, None, Some(0.0), None, None));
            }
            // its own starts where the song's is, so choosing it changes nothing until the slider moves
            if ui.selectable_label(d.swing.is_some(), "its own").clicked() && d.swing.is_none() {
                out.push(set(None, None, Some(song.swing.clamp(0.5, 0.8)), None, None));
            }
            if let Some(mut s) = d.swing {
                let r = ui.add(egui::Slider::new(&mut s, 0.5..=0.8));
                if let_go(&r) {
                    out.push(set(None, None, Some(s), None, None));
                }
            }
        });
        ui.end_row();
        ui.label(small("groove"));
        let current = d.groove.clone().unwrap_or_else(|| "none".into());
        egui::ComboBox::from_id_salt(("lane-groove", &d.lane)).selected_text(&current).show_ui(ui, |ui| {
            let names = std::iter::once("none".to_string())
                .chain(claudius_api::GROOVES.iter().map(|g| g.to_string()))
                .chain(song.grooves.iter().cloned());
            for g in names {
                if ui.selectable_label(g == current, &g).clicked() && g != current {
                    out.push(set(None, None, None, Some(if g == "none" { String::new() } else { g }), None));
                }
            }
        });
        ui.end_row();
        if d.groove.is_some() {
            ui.label(small("how much"));
            let mut a = d.groove_amount;
            let r = ui.add(egui::Slider::new(&mut a, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
            if let_go(&r) {
                out.push(set(None, None, None, None, Some(a)));
            }
            ui.end_row();
        }
        ui.label(small("feel"));
        let most = crate::edits::MOST_LATE;
        let mut late = d.late_ms;
        let r = ui
            .add(egui::Slider::new(&mut late, -most..=most).step_by(1.0).custom_formatter(|v, _| late_text(v as f32)))
            .on_hover_text(
                "every hit behind the beat (laid back, as a snare dragged 20 ms) or ahead of it (pushed), alike, as a \
                 drum machine's shift timing; the song's humanize drifts it besides",
            );
        if let_go(&r) {
            out.push(Command::SetLane {
                lane: d.lane.clone(),
                length: None,
                speed: None,
                swing: None,
                groove: None,
                groove_amount: None,
                late_ms: Some(late),
            });
        }
        ui.end_row();
    });
}

/// the steps a time round of lane `d` plays: its own length, at most its pattern's
fn lane_len(d: &claudius_api::DrumView) -> usize {
    let pattern = (d.pattern_bars.max(1) * 16) as usize;
    d.length.map_or(pattern, |l| (l as usize).min(pattern))
}

/// a lane's name, step count and the steps shown for `variant` (falling back to main)
fn lane_steps(song: &claudius_api::SongView, li: usize, variant: &str) -> (String, usize, Vec<f32>) {
    let d = &song.drums[li];
    let len = lane_len(d);
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
