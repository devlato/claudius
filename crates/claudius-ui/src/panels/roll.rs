//! The piano roll: one cycle of a part over a progression, as notes on a grid of beats and pitches. Click an empty
//! place to add a note, drag a note to move it, drag its right edge to make it longer or shorter, right-click it to
//! take it out. A cycle the theory engine writes is shown as it writes it, and the first edit writes it out
//! (`set_notes`); Back to the theory engine lets it write the cycle again.

use claudius_api::{Command, NoteView, SongView};
use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::{Cx, empty, layer_color, small};
use crate::music::note_name;
use crate::theme::{self, LINE, LINE_SOFT, TEXT_DIM, TEXT_FAINT, WELL};

/// what the roll shows, and the note a drag holds
#[derive(Default)]
pub struct RollView {
    pub part: Option<String>,
    pub prog: Option<String>,
    /// the beats a note snaps to and starts as long as (a sixteenth when 0)
    pub grid: f64,
    /// the cycle while a note is dragged, sent when it is let go
    draft: Option<Vec<NoteView>>,
    drag: Option<Drag>,
}

#[derive(Clone, Copy)]
struct Drag {
    note: usize,
    resize: bool,
    /// where in the note it was taken: beats from its start, and semitones
    grab: f64,
}

/// the keyboard down the left
const KEYS: f32 = 34.0;
/// a note's right edge, in points, that resizes it
const EDGE: f32 = 5.0;
const GRIDS: [(&str, f64); 4] = [("1/16", 0.25), ("1/8", 0.5), ("1/4", 1.0), ("1/2", 2.0)];

fn is_black(n: i32) -> bool {
    matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

fn snap(x: f64, grid: f64) -> f64 {
    (x / grid).floor() * grid
}

/// the lowest and highest pitch the roll shows: the part's register and every note, with room around them
fn range(song: &SongView, part: usize, notes: &[NoteView]) -> (i32, i32) {
    let (lo, hi) = song.parts[part].register;
    let lo = notes.iter().map(|n| n.p).min().map_or(lo, |m| m.min(lo)) - 2;
    let hi = notes.iter().map(|n| n.p).max().map_or(hi, |m| m.max(hi)) + 2;
    let (lo, hi) = (lo.max(0), hi.min(127));
    // two octaves at least
    if hi - lo < 24 { (lo.min(127 - 24), (lo + 24).min(127)) } else { (lo, hi) }
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.as_ref() else {
        empty(ui, "No song yet: make one with New song.");
        return;
    };
    if song.parts.is_empty() {
        empty(ui, "The song has no parts: + Track in the arrangement adds one.");
        return;
    }
    let roll = &mut cx.view.roll;
    if roll.grid <= 0.0 {
        roll.grid = 0.25;
    }
    let pi = roll.part.as_ref().and_then(|n| song.parts.iter().position(|p| &p.name == n)).unwrap_or(0);
    let part = &song.parts[pi];
    ui.horizontal_wrapped(|ui| {
        ui.label(small("part"));
        for p in &song.parts {
            if ui
                .selectable_label(p.name == part.name, RichText::new(&p.name).color(layer_color(song, &p.name)))
                .clicked()
            {
                roll.part = Some(p.name.clone());
                roll.prog = None;
                roll.draft = None;
                roll.drag = None;
            }
        }
    });
    if part.cycles.is_empty() {
        empty(ui, &format!("{} plays in no section: switch it on in the arrangement to write its notes.", part.name));
        return;
    }
    let ci = roll.prog.as_ref().and_then(|g| part.cycles.iter().position(|c| &c.prog == g)).unwrap_or(0);
    let cycle = &part.cycles[ci];
    let mut out: Vec<Command> = Vec::new();
    ui.horizontal_wrapped(|ui| {
        ui.label(small("over"));
        for c in &part.cycles {
            let how = if c.written { "written out" } else { "the theory engine's" };
            let label = format!("{} · {how}", c.prog);
            if ui.selectable_label(c.prog == cycle.prog, label).on_hover_text(format!("{} beats", c.beats)).clicked() {
                roll.prog = Some(c.prog.clone());
                roll.draft = None;
                roll.drag = None;
            }
        }
        ui.separator();
        ui.label(small("grid"));
        for (label, g) in GRIDS {
            ui.selectable_value(&mut roll.grid, g, label);
        }
        if cycle.written {
            ui.separator();
            if ui
                .button("Back to the theory engine")
                .on_hover_text("forget these notes: the part writes this progression itself again")
                .clicked()
            {
                out.push(Command::SetNotes { part: part.name.clone(), prog: cycle.prog.clone(), notes: None });
            }
        }
    });
    ui.label(small(if cycle.written {
        "click to add a note, drag to move it, drag its right edge to make it longer, right-click to take it out"
    } else {
        "as the theory engine writes it: the first edit writes these notes out for you to change"
    }));

    let notes: Vec<NoteView> = roll.draft.clone().unwrap_or_else(|| cycle.notes.clone());
    let (lo, hi) = range(song, pi, &notes);
    let rows = (hi - lo + 1) as f32;
    let avail = ui.available_size();
    let row_h = (avail.y / rows).clamp(7.0, 14.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(avail.x, rows * row_h), Sense::click_and_drag());
    let grid_x = rect.left() + KEYS;
    let beats = cycle.beats.max(1.0);
    let x_of = |t: f64| grid_x + (t / beats) as f32 * (rect.right() - grid_x);
    let beat_of = |x: f32| ((x - grid_x) / (rect.right() - grid_x)) as f64 * beats;
    let y_of = |p: i32| rect.top() + (hi - p) as f32 * row_h;
    let pitch_of = |y: f32| hi - ((y - rect.top()) / row_h).floor() as i32;
    let color = layer_color(song, &part.name);
    let painter = ui.painter_at(rect);

    // rows, keys and lines
    painter.rect_filled(rect, 3.0, WELL);
    for p in lo..=hi {
        let y = y_of(p);
        let row = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + row_h));
        if is_black(p) {
            painter.rect_filled(Rect::from_min_max(pos2(grid_x, y), row.max), 0.0, Color32::from_black_alpha(40));
        }
        let key = Rect::from_min_max(row.min, pos2(grid_x - 2.0, y + row_h));
        painter.rect_filled(
            key.shrink2(vec2(0.0, 0.5)),
            1.0,
            if is_black(p) { Color32::from_gray(30) } else { Color32::from_gray(170) },
        );
        if p.rem_euclid(12) == 0 {
            painter.line_segment([pos2(grid_x, y + row_h), pos2(rect.right(), y + row_h)], Stroke::new(1.0, LINE));
            painter.text(
                pos2(key.right() - 3.0, key.center().y),
                Align2::RIGHT_CENTER,
                note_name(p),
                theme::prop(8.0),
                Color32::from_gray(40),
            );
        }
    }
    let grid = roll.grid;
    let mut t = 0.0;
    while t <= beats + 1e-9 {
        let x = x_of(t);
        let stroke = if (t / 4.0).fract().abs() < 1e-9 {
            Stroke::new(1.0, LINE)
        } else if t.fract().abs() < 1e-9 {
            Stroke::new(1.0, LINE_SOFT)
        } else {
            Stroke::new(0.5, theme::mix(WELL, LINE_SOFT, 0.5))
        };
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
        if (t / 4.0).fract().abs() < 1e-9 && t < beats {
            painter.text(
                pos2(x + 3.0, rect.top() + 6.0),
                Align2::LEFT_CENTER,
                format!("{}", (t / 4.0) as u32 + 1),
                theme::prop(8.5),
                TEXT_FAINT,
            );
        }
        t += grid;
    }

    // notes
    let note_rect = |n: &NoteView| {
        Rect::from_min_max(
            pos2(x_of(n.t), y_of(n.p) + 0.5),
            pos2(x_of(n.t + n.d).max(x_of(n.t) + 3.0), y_of(n.p) + row_h - 0.5),
        )
    };
    let alpha = if cycle.written { 1.0 } else { 0.55 };
    for n in &notes {
        let r = note_rect(n);
        let fill = theme::mix(WELL, color, (0.45 + 0.55 * n.v.clamp(0.0, 1.0)) * alpha);
        painter.rect_filled(r, 2.0, fill);
        painter.rect_stroke(r, 2.0, Stroke::new(1.0, theme::mix(color, Color32::BLACK, 0.35)), StrokeKind::Inside);
    }
    let at = |pos: egui::Pos2| notes.iter().rposition(|n| note_rect(n).contains(pos));
    if let Some(pos) = resp.hover_pos() {
        let (b, p) = (beat_of(pos.x), pitch_of(pos.y));
        let tip = match at(pos) {
            Some(i) => format!("{} at beat {:.2}, {:.2} beats long", note_name(notes[i].p), notes[i].t, notes[i].d),
            None if pos.x >= grid_x => format!("{} at beat {:.2}", note_name(p), snap(b, grid)),
            None => note_name(p),
        };
        resp.clone().on_hover_text_at_pointer(tip);
    }

    // editing: the notes as they are, written out on the first change
    let send = |out: &mut Vec<Command>, notes: Vec<NoteView>| {
        out.push(Command::SetNotes { part: part.name.clone(), prog: cycle.prog.clone(), notes: Some(notes) });
    };
    if resp.drag_started()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(i) = at(pos)
    {
        let r = note_rect(&notes[i]);
        roll.drag = Some(Drag { note: i, resize: pos.x >= r.right() - EDGE, grab: beat_of(pos.x) - notes[i].t });
        roll.draft = Some(notes.clone());
    }
    if let (Some(d), Some(draft), Some(pos)) = (roll.drag, roll.draft.as_mut(), resp.interact_pointer_pos())
        && let Some(n) = draft.get_mut(d.note)
    {
        if d.resize {
            let end = (snap(beat_of(pos.x), grid) + grid).clamp(n.t + grid, beats);
            n.d = end - n.t;
        } else {
            n.t = snap(beat_of(pos.x) - d.grab + grid / 2.0, grid).clamp(0.0, (beats - n.d).max(0.0));
            n.p = pitch_of(pos.y).clamp(0, 127);
        }
    }
    if resp.drag_stopped() {
        if let Some(draft) = roll.draft.take() {
            send(&mut out, draft);
        }
        roll.drag = None;
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && pos.x >= grid_x
        && at(pos).is_none()
    {
        let mut next = notes.clone();
        let t = snap(beat_of(pos.x), grid).clamp(0.0, (beats - grid).max(0.0));
        next.push(NoteView { t, d: grid.min(beats - t), p: pitch_of(pos.y).clamp(0, 127), v: 0.8 });
        next.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.p.cmp(&b.p)));
        send(&mut out, next);
    }
    if resp.secondary_clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(i) = at(pos)
    {
        let mut next = notes.clone();
        next.remove(i);
        send(&mut out, next);
    }
    if !cycle.written && roll.draft.is_none() && notes.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "the theory engine rests here: click to write a note",
            theme::prop(11.0),
            TEXT_DIM,
        );
    }
    cx.out.extend(out);
}
