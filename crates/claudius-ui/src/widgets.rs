//! Studio widgets drawn with the painter: icon buttons, rotary knobs, a channel fader, level meters, chips, badges.

use std::f32::consts::PI;
use std::ops::RangeInclusive;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::theme::{self, ACCENT, LINE, RAISED, RAISED_HI, TEXT, TEXT_DIM, TEXT_FAINT, WELL};

/// a slider-like edit by the user is done: released after a drag, or changed by a click or the keyboard. egui also
/// marks a widget changed when it clamps or rounds a value it was handed; that is not an edit and must not be sent
pub fn commit(r: &Response) -> bool {
    r.drag_stopped()
        || (r.changed() && !r.dragged() && (r.has_focus() || r.lost_focus() || r.clicked() || r.double_clicked()))
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    ToStart,
    Loop,
    Undo,
    Redo,
    Plus,
    Dice,
    Left,
    Right,
    Trash,
    Copy,
    Render,
    Mutate,
    Cancel,
    New,
    Save,
    Open,
    Record,
    Phone,
    Swap,
    /// the arrow on a button that opens a menu
    Menu,
    Improve,
    Export,
    Done,
    Drum,
    Keys,
    Bass,
    Voice,
    Pad,
    Melody,
    Sample,
    Effect,
    Muted,
    Solo,
}

fn arc(center: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    let n = ((to - from).abs() * r / 3.0).clamp(6.0, 64.0) as usize;
    (0..=n).map(|i| from + (to - from) * i as f32 / n as f32).map(|a| center + r * vec2(a.sin(), -a.cos())).collect()
}

impl Icon {
    #[cfg(test)]
    pub const ALL: [Icon; 36] = [
        Icon::Play,
        Icon::Pause,
        Icon::Stop,
        Icon::ToStart,
        Icon::Loop,
        Icon::Undo,
        Icon::Redo,
        Icon::Plus,
        Icon::Dice,
        Icon::Left,
        Icon::Right,
        Icon::Trash,
        Icon::Copy,
        Icon::Render,
        Icon::Mutate,
        Icon::Cancel,
        Icon::New,
        Icon::Save,
        Icon::Open,
        Icon::Record,
        Icon::Phone,
        Icon::Swap,
        Icon::Menu,
        Icon::Improve,
        Icon::Export,
        Icon::Done,
        Icon::Drum,
        Icon::Keys,
        Icon::Bass,
        Icon::Voice,
        Icon::Pad,
        Icon::Melody,
        Icon::Sample,
        Icon::Effect,
        Icon::Muted,
        Icon::Solo,
    ];

    /// what a track is, as an icon: a drum, keys, a bass, a voice, a pad, a line, a sample or an effect
    pub fn of_track(kind: &str, role: &str) -> Icon {
        let r = role.to_ascii_lowercase();
        let has = |k: &str| r.contains(k);
        match kind {
            "drum" | "pattern" => Icon::Drum,
            "take" if has("vocal") || has("vox") || r.is_empty() => Icon::Voice,
            "fx" => Icon::Effect,
            _ if has("vocal") || has("vox") || has("voice") => Icon::Voice,
            _ if has("bass") || has("sub") || has("808") => Icon::Bass,
            _ if has("chord") || has("keys") || has("piano") || has("organ") || has("stab") => Icon::Keys,
            _ if has("pad") || has("string") || has("atmo") || has("drone") || has("choir") => Icon::Pad,
            _ if has("lead") || has("arp") || has("melod") || has("pluck") || has("bell") || has("hook") => {
                Icon::Melody
            }
            _ if has("fx") || has("riser") || has("impact") || has("sweep") => Icon::Effect,
            _ if has("kick") || has("snare") || has("clap") || has("hat") || has("perc") || has("top") => Icon::Drum,
            "loop" | "take" => Icon::Sample,
            _ => Icon::Melody,
        }
    }

    /// its glyph in Lucide's icons (`theme::ICONS`), by the name Lucide gives it
    pub fn glyph(self) -> char {
        match self {
            Icon::Play => '\u{e13c}',    // play
            Icon::Pause => '\u{e12e}',   // pause
            Icon::Stop => '\u{e167}',    // square
            Icon::ToStart => '\u{e15f}', // skip-back
            Icon::Loop => '\u{e146}',    // repeat
            Icon::Undo => '\u{e2a1}',    // undo-2
            Icon::Redo => '\u{e2a0}',    // redo-2
            Icon::Plus => '\u{e13d}',    // plus
            Icon::Dice => '\u{e2c5}',    // dices
            Icon::Left => '\u{e06e}',    // chevron-left
            Icon::Right => '\u{e06f}',   // chevron-right
            Icon::Trash => '\u{e18e}',   // trash-2
            Icon::Copy => '\u{e09e}',    // copy
            Icon::Render => '\u{e55b}',  // audio-waveform
            Icon::Mutate => '\u{e15e}',  // shuffle
            Icon::Cancel => '\u{e1b2}',  // x
            Icon::New => '\u{e0c9}',     // file-plus
            Icon::Save => '\u{e0b2}',    // download
            Icon::Open => '\u{e247}',    // folder-open
            Icon::Record => '\u{e076}',  // circle
            Icon::Phone => '\u{e163}',   // smartphone
            Icon::Swap => '\u{e417}',    // arrow-right-left
            Icon::Menu => '\u{e06d}',    // chevron-down
            Icon::Improve => '\u{e412}', // sparkles
            Icon::Export => '\u{e55e}',  // file-music
            Icon::Done => '\u{e226}',    // circle-check
            Icon::Drum => '\u{e55d}',    // drum
            Icon::Keys => '\u{e561}',    // piano
            Icon::Bass => '\u{e55f}',    // guitar
            Icon::Voice => '\u{e349}',   // mic-vocal
            Icon::Pad => '\u{e283}',     // waves
            Icon::Melody => '\u{e122}',  // music
            Icon::Sample => '\u{e55a}',  // audio-lines
            Icon::Effect => '\u{e1b4}',  // zap
            Icon::Muted => '\u{e1ac}',   // volume-x
            Icon::Solo => '\u{e0f1}',    // headphones
        }
    }
}

/// `icon` drawn to fill `rect`: its glyph, or for Record a red dot, as a recorder's button is, unless it cannot be
/// pressed
pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    if icon == Icon::Record {
        painter.circle_filled(rect.center(), 0.36 * size, if color == TEXT_FAINT { color } else { theme::BAD });
        return;
    }
    let font = egui::FontId::new(size, egui::FontFamily::Name(theme::ICONS.into()));
    painter.text(rect.center(), Align2::CENTER_CENTER, icon.glyph(), font, color);
}

/// a square button with a painted icon; `active` lights it in the accent colour
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, active: bool, enabled: bool, tooltip: &str) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(size), if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && response.hovered();
    let fill = if active {
        theme::alpha(ACCENT, if hovered { 0.32 } else { 0.22 })
    } else if response.is_pointer_button_down_on() && enabled {
        theme::mix(RAISED_HI, Color32::WHITE, 0.08)
    } else if hovered {
        RAISED_HI
    } else {
        RAISED
    };
    let stroke = if active {
        Stroke::new(1.0, theme::alpha(ACCENT, 0.8))
    } else if hovered {
        Stroke::new(1.0, LINE)
    } else {
        Stroke::NONE
    };
    ui.painter().rect(rect, 7.0, fill, stroke, StrokeKind::Inside);
    let color = if !enabled {
        TEXT_FAINT
    } else if active {
        ACCENT
    } else if hovered {
        Color32::WHITE
    } else {
        TEXT
    };
    paint_icon(ui.painter(), rect.shrink(size * 0.24), icon, color);
    if tooltip.is_empty() { response } else { response.on_hover_text(tooltip) }
}

/// a toolbar button: icon + label
pub fn tool_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, accent: bool) -> Response {
    labelled(ui, icon, label, enabled, accent, false)
}

/// a toolbar button that opens a menu: icon, label and the menu's arrow
pub fn menu_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool) -> Response {
    labelled(ui, icon, label, enabled, false, true)
}

fn labelled(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, accent: bool, menu: bool) -> Response {
    let font = theme::medium(13.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, TEXT);
    let size = vec2(galley.size().x + if menu { 54.0 } else { 40.0 }, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && response.hovered();
    let accent = accent && enabled;
    let base = if accent { theme::mix(RAISED, ACCENT, 0.22) } else { RAISED };
    let fill = if response.is_pointer_button_down_on() && enabled {
        theme::mix(base, Color32::WHITE, 0.12)
    } else if hovered {
        theme::mix(base, Color32::WHITE, 0.06)
    } else {
        base
    };
    let stroke = if accent {
        Stroke::new(1.0, theme::alpha(ACCENT, 0.7))
    } else if hovered {
        Stroke::new(1.0, LINE)
    } else {
        Stroke::NONE
    };
    ui.painter().rect(rect, 7.0, fill, stroke, StrokeKind::Inside);
    let color = if !enabled {
        TEXT_FAINT
    } else if accent {
        theme::mix(ACCENT, Color32::WHITE, 0.25)
    } else {
        TEXT
    };
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), Vec2::splat(15.0));
    paint_icon(ui.painter(), icon_rect, icon, color);
    ui.painter().galley(pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0), galley, color);
    if menu {
        let arrow = Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y + 0.5), Vec2::splat(12.0));
        paint_icon(ui.painter(), arrow, Icon::Menu, if enabled { TEXT_DIM } else { TEXT_FAINT });
    }
    response
}

/// a rotary knob: drag up / down (shift: fine), double-click resets to 0
pub fn knob(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    label: &str,
    color: Color32,
    bipolar: bool,
    fmt: impl Fn(f32) -> String,
) -> Response {
    let d = 30.0;
    let (rect, mut response) = ui.allocate_exact_size(vec2(d + 8.0, d + 14.0), Sense::click_and_drag());
    let (lo, hi) = (*range.start(), *range.end());
    if response.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) { 0.2 } else { 1.0 };
        let delta = -response.drag_delta().y * (hi - lo) / 140.0 * fine;
        if delta != 0.0 {
            *value = (*value + delta).clamp(lo, hi);
            response.mark_changed();
        }
    }
    if response.double_clicked() {
        *value = 0f32.clamp(lo, hi);
        response.mark_changed();
    }
    let center = pos2(rect.center().x, rect.top() + d / 2.0 + 1.0);
    let painter = ui.painter();
    let (a0, a1) = (-0.75 * PI, 0.75 * PI);
    let t = ((*value - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
    let a = a0 + (a1 - a0) * t;
    painter.add(Shape::line(arc(center, d / 2.0 - 1.5, a0, a1), Stroke::new(3.0, WELL)));
    let from = if bipolar { 0.0 } else { a0 };
    if (a - from).abs() > 0.01 {
        let (x, y) = if a < from { (a, from) } else { (from, a) };
        painter.add(Shape::line(arc(center, d / 2.0 - 1.5, x, y), Stroke::new(3.0, color)));
    }
    let hovered = response.hovered() || response.dragged();
    painter.circle_filled(center, d / 2.0 - 5.0, if hovered { RAISED_HI } else { theme::mix(RAISED, RAISED_HI, 0.5) });
    painter.circle_stroke(center, d / 2.0 - 5.0, Stroke::new(1.0, LINE));
    let tip = center + (d / 2.0 - 7.0) * vec2(a.sin(), -a.cos());
    painter.line_segment(
        [center + (d / 2.0 - 14.0) * vec2(a.sin(), -a.cos()), tip],
        Stroke::new(2.0, if hovered { Color32::WHITE } else { TEXT }),
    );
    let text = if hovered { fmt(*value) } else { label.to_string() };
    painter.text(
        pos2(rect.center().x, rect.bottom() - 6.0),
        Align2::CENTER_CENTER,
        text,
        theme::prop(10.5),
        if hovered { TEXT } else { TEXT_DIM },
    );
    response.on_hover_text(format!("{label}: {}  (drag, shift for fine, double-click resets)", fmt(*value)))
}

/// the fader's dB to 0..1 travel (finer around 0 dB, like a console)
fn fader_pos(db: f32, lo: f32, hi: f32) -> f32 {
    let t = ((db - lo) / (hi - lo)).clamp(0.0, 1.0);
    t.powf(0.75)
}

fn fader_db(pos: f32, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * pos.clamp(0.0, 1.0).powf(1.0 / 0.75)
}

/// a vertical channel fader in dB; double-click resets to 0 dB
pub fn fader(
    ui: &mut Ui,
    db: &mut f32,
    range: RangeInclusive<f32>,
    height: f32,
    color: Color32,
    enabled: bool,
) -> Response {
    let (lo, hi) = (*range.start(), *range.end());
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(44.0, height), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let track = Rect::from_min_max(
        pos2(rect.center().x + 6.0 - 2.0, rect.top() + 8.0),
        pos2(rect.center().x + 6.0 + 2.0, rect.bottom() - 8.0),
    );
    let y_of = |db: f32| track.bottom() - fader_pos(db, lo, hi) * track.height();
    if response.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) { 0.25 } else { 1.0 };
        let p = fader_pos(*db, lo, hi) - response.drag_delta().y / track.height() * fine;
        *db = ((fader_db(p, lo, hi) * 10.0).round() / 10.0).clamp(lo, hi);
        response.mark_changed();
    }
    if response.double_clicked() {
        *db = 0.0;
        response.mark_changed();
    }
    let painter = ui.painter();
    for mark in [12.0, 6.0, 0.0, -6.0, -12.0, -24.0, -48.0] {
        if mark < lo || mark > hi {
            continue;
        }
        let y = y_of(mark);
        let zero = mark == 0.0;
        painter.line_segment(
            [pos2(rect.left() + 14.0, y), pos2(track.left() - 3.0, y)],
            Stroke::new(1.0, if zero { TEXT_DIM } else { LINE }),
        );
        painter.text(
            pos2(rect.left() + 12.0, y),
            Align2::RIGHT_CENTER,
            format!("{}", mark.abs() as i32),
            theme::prop(8.5),
            if zero { TEXT_DIM } else { TEXT_FAINT },
        );
    }
    painter.rect_filled(track, 2.0, WELL);
    let y = y_of(*db);
    let fill = Rect::from_min_max(pos2(track.left(), y), track.max);
    painter.rect_filled(fill, 2.0, theme::alpha(color, if enabled { 0.55 } else { 0.25 }));
    let cap = Rect::from_center_size(pos2(track.center().x, y), vec2(24.0, 13.0));
    let hovered = response.hovered() || response.dragged();
    painter.rect(
        cap,
        3.0,
        if hovered { theme::mix(RAISED_HI, Color32::WHITE, 0.15) } else { RAISED_HI },
        Stroke::new(1.0, theme::mix(LINE, Color32::WHITE, 0.2)),
        StrokeKind::Inside,
    );
    painter.line_segment(
        [pos2(cap.left() + 4.0, cap.center().y), pos2(cap.right() - 4.0, cap.center().y)],
        Stroke::new(1.5, if enabled { color } else { TEXT_FAINT }),
    );
    response.on_hover_text(format!(
        "level {:+.1} dB relative to the role's target (drag, shift for fine, double-click: 0 dB)",
        *db
    ))
}

/// a vertical meter: `level` in dB on a `floor..0` scale, `peak` an optional marker
pub fn meter(ui: &mut Ui, size: Vec2, level: Option<f32>, floor: f32, peak: Option<f32>) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, WELL);
    let to_y = |db: f32| rect.bottom() - ((db - floor) / -floor).clamp(0.0, 1.0) * rect.height();
    if let Some(l) = level {
        let top = to_y(l);
        let segments: [(f32, f32, Color32); 4] = [
            (floor, -18.0, theme::GOOD),
            (-18.0, -9.0, theme::mix(theme::GOOD, theme::WARN, 0.6)),
            (-9.0, -3.0, theme::WARN),
            (-3.0, 0.0, theme::BAD),
        ];
        for (a, b, c) in segments {
            let (ya, yb) = (to_y(a), to_y(b.min(l)));
            if l > a && yb < ya {
                painter.rect_filled(
                    Rect::from_min_max(pos2(rect.left() + 1.0, yb.max(top)), pos2(rect.right() - 1.0, ya)),
                    1.0,
                    theme::alpha(c, 0.85),
                );
            }
        }
    }
    if let Some(p) = peak {
        let y = to_y(p);
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.5, TEXT));
    }
    response
}

/// a toggle chip; returns the response (clicked = toggle)
pub fn chip(ui: &mut Ui, on: bool, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), theme::prop(12.0), TEXT);
    let size = vec2(galley.size().x + 18.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let fill = if on {
        theme::alpha(color, if hovered { 0.42 } else { 0.3 })
    } else if hovered {
        RAISED_HI
    } else {
        RAISED
    };
    let stroke = Stroke::new(1.0, if on { theme::alpha(color, 0.9) } else { LINE });
    ui.painter().rect(rect, CornerRadius::same(11), fill, stroke, StrokeKind::Inside);
    let tc = if on { theme::mix(color, Color32::WHITE, 0.55) } else { TEXT_DIM };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, tc);
    response
}

/// a small rounded label
pub fn pill(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), theme::prop(11.0), color);
    let size = vec2(galley.size().x + 12.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(9),
        theme::alpha(color, 0.14),
        Stroke::new(1.0, theme::alpha(color, 0.45)),
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// a panel heading in small caps
pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).strong().color(TEXT_DIM).extra_letter_spacing(0.8));
}

/// A menu's first line: what it acts on, strong, then what that is in a few faint words ("bass", "a part, reese")
pub fn menu_title(ui: &mut Ui, name: &str, about: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(egui::RichText::new(name).size(13.5).strong().color(TEXT));
        if !about.is_empty() {
            ui.label(egui::RichText::new(about).size(11.5).color(TEXT_FAINT));
        }
    });
}

/// The name of a group of a menu's items, in small faint capitals with a little room above it, so that a long menu
/// reads as a few short ones
pub fn menu_group(ui: &mut Ui, name: &str) {
    ui.add_space(5.0);
    ui.label(egui::RichText::new(name.to_uppercase()).size(9.5).strong().color(TEXT_FAINT).extra_letter_spacing(1.0));
}

/// A menu item: its words, and on the right, faint, the key that does it too (none when `keys` is empty)
pub fn menu_item(ui: &mut Ui, label: impl Into<egui::WidgetText>, keys: &str) -> Response {
    ui.add(egui::Button::new(label).shortcut_text(egui::RichText::new(keys).size(11.0).color(TEXT_FAINT)))
}

/// claudius' docs on the web, which the help marks link into
pub const DOCS: &str = "https://claudius.devlato.com/docs/";

/// A small (?) beside a control that is not obvious: hovered, it says in plain words what the control does; clicked, it
/// opens the page of the docs that says more (`page` under `DOCS`, such as "language/form.html#energy").
pub fn help(ui: &mut Ui, text: &str, page: &str) -> Response {
    let size = 14.0;
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let color = if r.hovered() { TEXT } else { TEXT_FAINT };
    ui.painter().circle_stroke(rect.center(), size / 2.0 - 1.0, Stroke::new(1.0, color));
    ui.painter().text(rect.center() + vec2(0.0, 0.5), Align2::CENTER_CENTER, "?", theme::prop(9.5), color);
    let r = r
        .on_hover_ui(|ui| {
            ui.set_max_width(300.0);
            ui.label(text);
            ui.add_space(3.0);
            ui.label(egui::RichText::new("Click for more in the docs").size(11.0).color(ACCENT));
        })
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if r.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{DOCS}{page}")));
    }
    r
}

/// a form's label, small and dim, with its help mark: for the left column of a grid of settings
pub fn label_help(ui: &mut Ui, label: &str, text: &str, page: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new(label).size(11.5).color(TEXT_DIM));
        help(ui, text, page);
    });
}

/// the verdict score as a big coloured badge
pub fn score_badge(ui: &mut Ui, total: Option<f64>, height: f32) -> Response {
    let text = total.map_or("--".to_string(), |t| format!("{t:.0}"));
    let color = total.map_or(TEXT_FAINT, |t| theme::score_color(t / 100.0));
    let galley = ui.painter().layout_no_wrap(text, theme::prop(height * 0.55), color);
    let size = vec2((galley.size().x + 22.0).max(height * 1.3), height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    ui.painter().rect(
        rect,
        CornerRadius::same((height / 2.0) as u8),
        theme::alpha(color, 0.16),
        Stroke::new(1.5, theme::alpha(color, 0.75)),
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// a small status dot
pub fn dot(ui: &mut Ui, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
    response
}

/// "Cmd" on macOS, "Ctrl" elsewhere (for shortcut hints)
pub fn cmd_key(ctx: &egui::Context) -> &'static str {
    if ctx.os() == egui::os::OperatingSystem::Mac { "Cmd" } else { "Ctrl" }
}

/// a horizontal bar for a 0..1 value
pub fn bar(ui: &mut Ui, width: f32, value: f64, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, 4.0, WELL);
    let fill = Rect::from_min_size(rect.min, vec2(rect.width() * value.clamp(0.0, 1.0) as f32, rect.height()));
    ui.painter().rect_filled(fill, 4.0, color);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_a_glyph_of_the_icon_font_and_the_text_font_draws_the_studios_punctuation() {
        let ctx = egui::Context::default();
        ctx.set_fonts(theme::fonts());
        // the fonts are loaded with the first frame, whose textures nothing draws
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        let icons = egui::FontId::new(16.0, egui::FontFamily::Name(theme::ICONS.into()));
        for icon in Icon::ALL {
            assert!(ctx.fonts_mut(|f| f.has_glyph(&icons, icon.glyph())), "no glyph {:?}", icon.glyph());
        }
        for c in "Aé·–—…→×÷±°ЖΩ".chars() {
            assert!(ctx.fonts_mut(|f| f.has_glyph(&theme::prop(13.0), c)), "no glyph {c}");
        }
    }

    #[test]
    fn every_page_a_help_mark_opens_is_in_the_docs_with_its_heading() {
        use std::path::Path;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let docs = root.join("../../docs/src");
        // an anchor as mdBook makes one of a heading
        let anchor = |heading: &str| -> String {
            heading
                .chars()
                .filter_map(|c| match c {
                    c if c.is_alphanumeric() || c == '_' || c == '-' => Some(c.to_ascii_lowercase()),
                    c if c.is_whitespace() => Some('-'),
                    _ => None,
                })
                .collect()
        };
        let mut sources = Vec::new();
        let mut dirs = vec![root.join("src")];
        while let Some(dir) = dirs.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    sources.push(std::fs::read_to_string(&p).unwrap());
                }
            }
        }
        let mut seen = 0;
        for src in &sources {
            for (i, _) in src.match_indices(".html#") {
                let start = src[..i].rfind('"').unwrap() + 1;
                let end = i + src[i..].find('"').unwrap();
                let (page, id) = src[start..end].split_once(".html#").unwrap();
                // a page of the docs is a folder and a name ("language/form"), not this test's own words
                let path_like =
                    |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || "-/".contains(c));
                if !page.contains('/') || !path_like(page) {
                    continue;
                }
                let md = std::fs::read_to_string(docs.join(format!("{page}.md")))
                    .unwrap_or_else(|_| panic!("no docs/src/{page}.md for a help mark"));
                let headings: Vec<String> = md
                    .lines()
                    .filter_map(|l| l.strip_prefix('#'))
                    .map(|h| anchor(h.trim_start_matches('#').trim()))
                    .collect();
                assert!(headings.iter().any(|h| h == id), "docs/src/{page}.md has no heading for #{id}");
                seen += 1;
            }
        }
        assert!(seen >= 10, "the studio's help marks link into the docs: {seen} links");
    }
}
