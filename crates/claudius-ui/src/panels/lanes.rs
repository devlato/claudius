//! The signal flow drawn as the song's lanes (design 0025): each track a row the sound runs through from left to
//! right, from its source through each effect of its chain, drawn as a block that shows what it does, to its fader,
//! pan and meter, its sends and the bus it runs into; the buses, the reverb and delay returns and the master as lanes
//! of the same kind under the tracks, ending in the finished song.
//!
//! It is zoomed and panned as a map is (a pinch or the scroll wheel with Ctrl or Cmd zooms around the pointer, the
//! wheel or a drag of the background pans, Fit shows it all), and everything is edited where it is: a block opens its
//! settings with a click, moves along its chain or into another with a drag, switches off and on with its power button
//! and goes with ×; a + between two blocks adds one there. With the lanes focused, the arrows choose a block, Enter
//! opens it, Delete takes it out, B switches it off or on, F fits and + and − zoom.

use claudius_api::{Chain, Command, EFFECT_TEMPLATES, Graph, NodeKind, SongView, effect_summary, words};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, Key, Modifiers, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2,
    vec2,
};
use serde_json::{Value, json};

use super::graph::{chain_of, node_menu, set_chain, set_chains, set_loudness};
use super::{Cx, layer_color};
use crate::cmds::MixEdit;
use crate::theme::{self, ACCENT, LINE, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::commit;

/// the lanes' headers, at the left of the view whatever the pan, in points
const HEAD_W: f32 = 176.0;
/// a lane's height and the room between two, in the world's units (points at zoom 1)
const LANE_H: f32 = 92.0;
const LANE_GAP: f32 = 8.0;
/// the room between the tracks, the buses and returns, and the master
const GROUP_GAP: f32 = 30.0;
/// a track's source block, an effect's block, and the room between two blocks, where + adds one
const SOURCE_W: f32 = 132.0;
const BLOCK_W: f32 = 150.0;
const SLOT_W: f32 = 26.0;
/// the fader, pan and meter, a track's sends, and where its sound goes
const STRIP_W: f32 = 150.0;
const SENDS_W: f32 = 112.0;
const OUT_W: f32 = 132.0;
const COLUMN_GAP: f32 = 10.0;
/// the margin around everything
const PAD: f32 = 16.0;
pub(super) const MIN_ZOOM: f32 = 0.3;
pub(super) const MAX_ZOOM: f32 = 2.4;
/// under this zoom a block is its name and main value; over it, its picture and its knobs too
const DETAIL_ZOOM: f32 = 0.7;

/// What a lane is.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum LaneKind {
    /// a track, by its place in the song's tracks
    Track(usize),
    Bus(String),
    /// the reverb or the delay return
    Return(String),
    Master,
}

/// One lane: what it is, its node in the song's graph (what a click on its header chooses, for the inspector), what it
/// is called and a line about it, its chain, its colour, where its top is in the world, and where its sound goes.
pub(super) struct Lane {
    pub kind: LaneKind,
    pub node: String,
    pub title: String,
    pub about: String,
    pub chain: Chain,
    pub fx: Vec<Value>,
    pub color: Color32,
    pub top: f32,
    pub out: String,
}

impl Lane {
    /// where its chain starts: after its first block, a track's source or what feeds a bus, a return or the master, so
    /// that every lane's blocks line up
    fn chain_x(&self) -> f32 {
        PAD + SOURCE_W
    }

    /// effect `i`'s block
    fn block(&self, i: usize) -> Rect {
        let x = self.chain_x() + SLOT_W + i as f32 * (BLOCK_W + SLOT_W);
        Rect::from_min_size(pos2(x, self.top), vec2(BLOCK_W, LANE_H))
    }

    /// the slot before effect `j` (after the last at `j == len`), where + adds one and a block dragged lands
    fn slot(&self, j: usize) -> Rect {
        let x = self.chain_x() + j as f32 * (BLOCK_W + SLOT_W);
        Rect::from_min_size(pos2(x, self.top), vec2(SLOT_W, LANE_H))
    }

    /// where its chain ends, its last slot with it
    fn chain_end(&self) -> f32 {
        self.slot(self.fx.len()).right()
    }

    /// the slot a point at world `x` along it falls in: before the block it is nearest the start of
    fn slot_at(&self, x: f32) -> usize {
        let j = ((x - self.chain_x() + BLOCK_W / 2.0) / (BLOCK_W + SLOT_W)).floor();
        (j.max(0.0) as usize).min(self.fx.len())
    }

    /// whether its chain is one a block may be added to, moved within, or taken out of (a return's is its one setting)
    fn editable(&self) -> bool {
        !matches!(self.kind, LaneKind::Return(_))
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The song's lanes, top to bottom: its tracks in the arrangement's order, the buses its graph has, the reverb and
/// delay returns, and the master.
pub(super) fn lanes(song: &SongView, graph: &Graph) -> Vec<Lane> {
    let mut out = Vec::new();
    let mut y = PAD;
    for (i, t) in song.tracks.iter().enumerate() {
        let sound = words::name_in_words(&t.sound);
        out.push(Lane {
            kind: LaneKind::Track(i),
            node: format!("track:{}", t.name),
            title: t.name.clone(),
            about: if t.duck > 0.01 { format!("{sound} · ducks {:.0}%", t.duck * 100.0) } else { sound },
            chain: Chain::Track(t.name.clone()),
            fx: t.fx.clone(),
            color: layer_color(song, &t.name),
            top: y,
            out: format!("{} bus", t.bus),
        });
        y += LANE_H + LANE_GAP;
    }
    y += GROUP_GAP - LANE_GAP;
    let mut buses: Vec<&claudius_api::Node> =
        graph.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Bus(_))).collect();
    buses.sort_by(|a, b| a.row.total_cmp(&b.row));
    for n in buses {
        let NodeKind::Bus(name) = &n.kind else { continue };
        let fed = song.tracks.iter().filter(|t| t.bus == *name).count();
        out.push(Lane {
            kind: LaneKind::Bus(name.clone()),
            node: n.id.clone(),
            title: format!("{name} bus"),
            about: format!("from {fed} track{}", plural(fed)),
            chain: Chain::Bus(name.clone()),
            fx: chain_of(song, &Chain::Bus(name.clone())),
            color: ACCENT.into(),
            top: y,
            out: "master".into(),
        });
        y += LANE_H + LANE_GAP;
    }
    for (name, color) in [("reverb", Color32::from(theme::INFO)), ("delay", Color32::from(ACCENT))] {
        let senders = song.tracks.iter().filter(|t| if name == "reverb" { t.reverb } else { t.delay } > 0.005).count();
        out.push(Lane {
            kind: LaneKind::Return(name.into()),
            node: format!("return:{name}"),
            title: format!("{name} return"),
            about: format!("from {senders} track{}", plural(senders)),
            chain: Chain::Send(name.into()),
            fx: chain_of(song, &Chain::Send(name.into())),
            color,
            top: y,
            out: "master".into(),
        });
        y += LANE_H + LANE_GAP;
    }
    y += GROUP_GAP - LANE_GAP;
    out.push(Lane {
        kind: LaneKind::Master,
        node: "master".into(),
        title: "master".into(),
        about: format!("{:.1} LUFS, ceiling {:.1} dBTP", song.mix.lufs, song.mix.ceiling_db),
        chain: Chain::Master,
        fx: chain_of(song, &Chain::Master),
        color: ACCENT.into(),
        top: y,
        out: "the finished song".into(),
    });
    out
}

/// The columns after every lane's chain, the same for all of them as a mixer's are: the strip's left edge (fader,
/// pan, meter), the sends', where the sound goes, and the world's width and height.
pub(super) struct Columns {
    pub strip: f32,
    pub sends: f32,
    pub out: f32,
    pub world: Vec2,
}

pub(super) fn columns(lanes: &[Lane]) -> Columns {
    let strip = lanes.iter().map(Lane::chain_end).fold(PAD + SOURCE_W + SLOT_W, f32::max) + COLUMN_GAP;
    let sends = strip + STRIP_W + COLUMN_GAP;
    let out = sends + SENDS_W + COLUMN_GAP;
    let bottom = lanes.last().map_or(PAD, |l| l.top + LANE_H) + PAD;
    Columns { strip, sends, out, world: vec2(out + OUT_W + PAD, bottom) }
}

/// The view's zoom and pan after zooming by `factor` around the point `at` (on the screen), the world's point under it
/// staying under it: `origin` is where the world's (0, 0) is drawn at no pan.
pub(super) fn zoomed(zoom: f32, pan: Vec2, origin: Pos2, at: Pos2, factor: f32) -> (f32, Vec2) {
    let z = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    let under = (at - origin) / zoom + pan;
    (z, under - (at - origin) / z)
}

/// The zoom and pan the lanes open at: the whole width of a world `world` big in a view `view` big, never larger than
/// as they are drawn, and the lanes from the top, the rest a scroll away, as a mixer's tracks are.
pub(super) fn fitted_width(world: Vec2, view: Vec2) -> (f32, Vec2) {
    ((view.x / world.x.max(1.0)).clamp(MIN_ZOOM, 1.0), Vec2::ZERO)
}

/// The zoom and pan that show all of a world `world` big in a view `view` big: never larger than as it is.
pub(super) fn fitted(world: Vec2, view: Vec2) -> (f32, Vec2) {
    let z = (view.x / world.x.max(1.0)).min(view.y / world.y.max(1.0)).clamp(MIN_ZOOM, 1.0);
    (z, Vec2::ZERO)
}

/// a pan kept to the world: never further than its edge from any side, a little room to spare
pub(super) fn clamped(pan: Vec2, zoom: f32, world: Vec2, view: Vec2) -> Vec2 {
    let max = (world - view / zoom).max(Vec2::ZERO) + Vec2::splat(PAD);
    pan.clamp(-Vec2::splat(PAD), max)
}

/// whether an effect is switched off (`{"fx": "off", "effect": ...}`, design 0025)
pub(super) fn is_off(e: &Value) -> bool {
    e.get("fx").and_then(Value::as_str) == Some("off")
}

/// the effect itself, switched off or not
pub(super) fn inner(e: &Value) -> &Value {
    if is_off(e) { e.get("effect").unwrap_or(e) } else { e }
}

/// the effect switched off, or on again, with all its settings as they were
pub(super) fn switched(e: &Value) -> Value {
    if is_off(e) { inner(e).clone() } else { json!({"fx": "off", "effect": e}) }
}

/// Effect `from` of one chain (`fx`) moved to slot `to` of another (`into`, the same chain when `None`), as a block
/// dragged lands: the chains as they are then, the first and the second.
pub(super) fn moved(fx: &[Value], from: usize, into: Option<&[Value]>, to: usize) -> (Vec<Value>, Option<Vec<Value>>) {
    let mut a = fx.to_vec();
    if from >= a.len() {
        return (a, into.map(<[Value]>::to_vec));
    }
    let e = a.remove(from);
    match into {
        None => {
            // a slot after the block's old place is one nearer now it is out
            let to = if to > from { to - 1 } else { to };
            a.insert(to.min(a.len()), e);
            (a, None)
        }
        Some(other) => {
            let mut b = other.to_vec();
            b.insert(to.min(b.len()), e);
            (a, Some(b))
        }
    }
}

/// The first number in a setting: a number as it is, the first of a moving value's, a value by section's own.
fn first_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Array(a) => a.iter().find_map(first_number),
        Value::Object(o) => o.get("default").and_then(first_number).or_else(|| o.values().find_map(first_number)),
        _ => None,
    }
}

fn setting(e: &Value, k: &str, or: f64) -> f64 {
    e.get(k).and_then(first_number).unwrap_or(or)
}

/// The response in dB of one band of an equalizer or filter at `f` Hz, by the biquads of Robert Bristow-Johnson's
/// cookbook at 48 kHz: `kind` as the song names it, its frequency, gain and Q.
pub(super) fn band_db(kind: &str, f0: f64, gain_db: f64, q: f64, f: f64) -> f64 {
    let sr = 48_000.0;
    let w0 = std::f64::consts::TAU * f0.clamp(10.0, sr * 0.49) / sr;
    let (cw, sw) = (w0.cos(), w0.sin());
    let q = q.max(0.05);
    let alpha = sw / (2.0 * q);
    let a = 10f64.powf(gain_db / 40.0);
    let (b0, b1, b2, a0, a1, a2) = match kind {
        "lowpass" | "lp" | "ladder" => {
            ((1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha)
        }
        "highpass" | "hp" => ((1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
        "bandpass" | "bp" => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
        "notch" => (1.0, -2.0 * cw, 1.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
        "peak" => (1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a),
        "lowshelf" => {
            let s = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) - (a - 1.0) * cw + s),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                a * ((a + 1.0) - (a - 1.0) * cw - s),
                (a + 1.0) + (a - 1.0) * cw + s,
                -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                (a + 1.0) + (a - 1.0) * cw - s,
            )
        }
        "highshelf" => {
            let s = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) + (a - 1.0) * cw + s),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                a * ((a + 1.0) + (a - 1.0) * cw - s),
                (a + 1.0) - (a - 1.0) * cw + s,
                2.0 * ((a - 1.0) - (a + 1.0) * cw),
                (a + 1.0) - (a - 1.0) * cw - s,
            )
        }
        _ => return 0.0,
    };
    // |H(e^jw)| at f
    let w = std::f64::consts::TAU * f / sr;
    let (c1, s1, c2, s2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
    let num = ((b0 + b1 * c1 + b2 * c2).powi(2) + (b1 * s1 + b2 * s2).powi(2)).sqrt();
    let den = ((a0 + a1 * c1 + a2 * c2).powi(2) + (a1 * s1 + a2 * s2).powi(2)).sqrt();
    20.0 * (num / den.max(1e-12)).max(1e-9).log10()
}

/// An effect's frequency response in dB at `f` Hz, where it has one (an equalizer, a filter); `None` otherwise.
pub(super) fn response_db(e: &Value, f: f64) -> Option<f64> {
    match e.get("fx").and_then(Value::as_str)? {
        "eq" => Some(
            e.get("bands")?
                .as_array()?
                .iter()
                .map(|b| {
                    band_db(
                        b.get("kind").and_then(Value::as_str).unwrap_or("peak"),
                        setting(b, "freq", 1000.0),
                        setting(b, "gain_db", 0.0),
                        setting(b, "q", 0.707),
                        f,
                    )
                })
                .sum(),
        ),
        "filter" => {
            let mode = e.get("mode").and_then(Value::as_str).unwrap_or("lp");
            let q = 0.707 + 9.0 * setting(e, "res", 0.0).clamp(0.0, 1.0);
            let one = band_db(mode, setting(e, "cutoff", 1000.0), 0.0, q, f);
            // the ladder falls at 24 dB an octave: two poles' worth more
            Some(if mode == "ladder" { 2.0 * one } else { one })
        }
        _ => None,
    }
}

/// A compressor's or limiter's level out (dB) for a level in, with its knee; `None` for an effect of no such curve.
pub(super) fn transfer_db(e: &Value, input: f64) -> Option<f64> {
    let (threshold, ratio, knee, makeup) = match e.get("fx").and_then(Value::as_str)? {
        "compressor" => (
            setting(e, "threshold_db", -18.0),
            setting(e, "ratio", 4.0).max(1.0),
            setting(e, "knee_db", 0.0).max(0.0),
            setting(e, "makeup_db", 0.0),
        ),
        "limiter" => (setting(e, "ceiling_db", -1.0), 1000.0, 0.0, 0.0),
        _ => return None,
    };
    let over = input - threshold;
    let out = if knee > 0.0 && over.abs() <= knee / 2.0 {
        input + (1.0 / ratio - 1.0) * (over + knee / 2.0).powi(2) / (2.0 * knee)
    } else if over > 0.0 {
        threshold + over / ratio
    } else {
        input
    };
    Some(out + makeup)
}

/// A saturator's shape: what `x` (−1 to 1) comes out as, through `kind` at `drive_db`.
pub(super) fn shaped(kind: &str, drive_db: f64, x: f64) -> f64 {
    let d = 10f64.powf(drive_db / 20.0);
    let v = x * d;
    let y = match kind {
        "hard" => v.clamp(-1.0, 1.0),
        "fold" => (v * std::f64::consts::FRAC_PI_2).sin(),
        "tube" => (v + 0.2).tanh() - 0.2f64.tanh(),
        "tape" => v / (1.0 + v.abs()),
        _ => v.tanh(),
    };
    // drawn at the level it comes out, the curve's own tops at the frame's
    let top = match kind {
        "hard" | "fold" => 1.0,
        "tube" => ((d + 0.2).tanh() - 0.2f64.tanh()).abs().max(((-d + 0.2).tanh() - 0.2f64.tanh()).abs()),
        "tape" => d / (1.0 + d),
        _ => d.tanh(),
    };
    y / top.max(1e-9)
}

// ----- drawing and working with it -----

/// where the world's points are drawn: `origin` the world's (0, 0) at no pan, at `zoom`, moved by `pan` (world units)
#[derive(Clone, Copy)]
struct Map {
    origin: Pos2,
    zoom: f32,
    pan: Vec2,
}

impl Map {
    fn pos(&self, p: Pos2) -> Pos2 {
        self.origin + (p.to_vec2() - self.pan) * self.zoom
    }

    fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.pos(r.min), self.pos(r.max))
    }

    fn world(&self, p: Pos2) -> Pos2 {
        Pos2::ZERO + (p - self.origin) / self.zoom + self.pan
    }
}

/// The lanes, in what the inspector leaves of the width.
pub(super) fn show(ui: &mut Ui, cx: &mut Cx, song: &SongView, graph: &Graph) {
    let lanes = lanes(song, graph);
    let cols = columns(&lanes);
    let full = ui.available_rect_before_wrap();
    // a strip along the top for the zoom's buttons, then the lanes
    let bar = Rect::from_min_size(full.min, vec2(full.width(), 26.0));
    let canvas = Rect::from_min_max(pos2(full.left(), bar.bottom()), full.max);
    let view = vec2((canvas.width() - HEAD_W).max(40.0), canvas.height());
    let origin = pos2(canvas.left() + HEAD_W, canvas.top());
    let id = ui.scope_id().with("flow-lanes");

    // opened afresh, or Fit pressed: all of it in view
    let g = &mut cx.view.graph;
    if g.scene == Rect::ZERO || g.zoom <= 0.0 {
        (g.zoom, g.pan) = fitted_width(cols.world, view);
        g.scene = canvas;
    }
    toolbar(ui, cx, bar, cols.world, view, origin);

    let resp = ui.interact(canvas, id, Sense::click_and_drag());
    let focused = ui.memory(|m| m.has_focus(id));
    if resp.clicked() || resp.drag_started() {
        ui.memory_mut(|m| m.request_focus(id));
        cx.view.graph.selected = None;
    }
    if focused {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() },
            )
        });
    }
    // zoom around the pointer, pan with the wheel or a drag of the background
    let g = &mut cx.view.graph;
    if let Some(at) = ui.input(|i| i.pointer.hover_pos()).filter(|p| canvas.contains(*p)) {
        let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
        if (zoom - 1.0).abs() > 1e-4 {
            (g.zoom, g.pan) = zoomed(g.zoom, g.pan, origin, at, zoom);
        } else if scroll != Vec2::ZERO {
            g.pan -= scroll / g.zoom;
        }
    }
    if resp.dragged() {
        g.pan -= resp.drag_delta() / g.zoom;
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    g.pan = clamped(g.pan, g.zoom, cols.world, view);
    let map = Map { origin, zoom: g.zoom, pan: g.pan };

    let painter = ui.painter_at(canvas);
    painter.rect_filled(canvas, 0.0, theme::alpha(Color32::BLACK, 0.12));
    let body = Rect::from_min_max(origin, canvas.max);
    let mut drag: Option<Dragging> = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(body);
        for lane in &lanes {
            let r =
                map.rect(Rect::from_min_size(pos2(PAD - 6.0, lane.top), vec2(cols.world.x - 2.0 * PAD + 12.0, LANE_H)));
            if !r.intersects(body) {
                continue;
            }
            ui.painter().rect(
                r,
                8.0 * map.zoom.min(1.0),
                theme::alpha(RAISED, 0.55),
                Stroke::new(1.0, theme::alpha(LINE, 0.6)),
                StrokeKind::Inside,
            );
            signal(ui, &map, lane, &cols);
            match lane.kind {
                LaneKind::Track(i) => source(ui, song, &map, lane, i),
                _ => inputs(ui, song, &map, lane),
            }
            if let Some(d) = chain(ui, cx, song, &map, lane, &lanes) {
                drag = Some(d);
            }
            strip(ui, cx, song, &map, lane, &cols);
            if let LaneKind::Track(i) = lane.kind {
                sends(ui, cx, song, &map, lane, i, &cols);
            }
            out_pill(ui, &map, lane, &cols);
        }
        if let Some(d) = &drag {
            landing(ui, &map, &lanes, d);
        }
    });
    if let Some(d) = drag
        && d.stopped
    {
        land(cx, song, &lanes, &map, d);
    }
    headers(ui, cx, song, &map, &lanes, canvas);
    if focused {
        keys(ui, cx, song, &lanes, cols.world, view, origin);
    }
}

/// the zoom's buttons and what it is at, along the top
fn toolbar(ui: &mut Ui, cx: &mut Cx, bar: Rect, world: Vec2, view: Vec2, origin: Pos2) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(HEAD_W - 6.0);
            let g = &mut cx.view.graph;
            // about the middle of the view, as the keys zoom
            let centre = origin + view / 2.0;
            if ui.small_button("−").on_hover_text("zoom out (−)").clicked() {
                (g.zoom, g.pan) = zoomed(g.zoom, g.pan, origin, centre, 0.8);
            }
            ui.label(egui::RichText::new(format!("{:.0}%", g.zoom * 100.0)).size(11.0).color(TEXT_DIM));
            if ui.small_button("+").on_hover_text("zoom in (+)").clicked() {
                (g.zoom, g.pan) = zoomed(g.zoom, g.pan, origin, centre, 1.25);
            }
            if ui.small_button("Fit").on_hover_text("show all of it (F)").clicked() {
                (g.zoom, g.pan) = fitted(world, view);
            }
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("pinch or Ctrl/Cmd + scroll to zoom, drag to pan, click a block to change it")
                    .size(11.0)
                    .color(TEXT_FAINT),
            );
        });
    });
}

/// a track's source: what plays (a part's instrument, a kit, a recording), drawn as what it is
fn source(ui: &Ui, song: &SongView, map: &Map, lane: &Lane, i: usize) {
    let t = &song.tracks[i];
    let r = map.rect(Rect::from_min_size(pos2(PAD + 4.0, lane.top + 8.0), vec2(SOURCE_W - 12.0, LANE_H - 16.0)));
    let p = ui.painter();
    p.rect(
        r,
        6.0 * map.zoom.min(1.0),
        theme::mix(WELL, lane.color, 0.12),
        Stroke::new(1.0, theme::alpha(lane.color, 0.6)),
        StrokeKind::Inside,
    );
    let k = map.zoom.min(1.4);
    // its sign: pads for a kit, a wave for an instrument, a waveform for a recording
    let sign = Rect::from_center_size(pos2(r.left() + 20.0 * k, r.center().y), vec2(24.0 * k, 24.0 * k));
    let ink = theme::alpha(lane.color, 0.9);
    match t.kind.as_str() {
        "drum" => {
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let c = sign.min + vec2(dx * sign.width() / 2.0 + 1.0, dy * sign.height() / 2.0 + 1.0);
                p.rect_filled(Rect::from_min_size(c, sign.size() / 2.0 - vec2(2.0, 2.0)), 2.0, ink);
            }
        }
        "take" | "loop" => {
            let n = 9;
            for j in 0..n {
                let x = sign.left() + j as f32 * sign.width() / (n - 1) as f32;
                let h = sign.height() * (0.25 + 0.75 * ((j as f32 * 1.7).sin().abs()));
                p.line_segment(
                    [pos2(x, sign.center().y - h / 2.0), pos2(x, sign.center().y + h / 2.0)],
                    Stroke::new(1.5 * k, ink),
                );
            }
        }
        "pattern" => {
            for j in 0..4 {
                let x = sign.left() + j as f32 * sign.width() / 4.0;
                let h = if j % 2 == 0 { sign.height() } else { sign.height() * 0.45 };
                let bar = Rect::from_min_max(
                    pos2(x + 1.0, sign.bottom() - h),
                    pos2(x + sign.width() / 4.0 - 1.0, sign.bottom()),
                );
                p.rect_filled(bar, 1.5, ink);
            }
        }
        "fx" => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|j| {
                    let u = j as f32 / 16.0;
                    pos2(sign.left() + u * sign.width(), sign.bottom() - u * u * sign.height())
                })
                .collect();
            p.add(Shape::line(pts, Stroke::new(1.8 * k, ink)));
        }
        _ => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|j| {
                    let u = j as f32 / 24.0;
                    pos2(
                        sign.left() + u * sign.width(),
                        sign.center().y - (u * std::f32::consts::TAU).sin() * sign.height() * 0.4,
                    )
                })
                .collect();
            p.add(Shape::line(pts, Stroke::new(1.8 * k, ink)));
        }
    }
    if map.zoom >= 0.45 {
        let text = Rect::from_min_max(pos2(sign.right() + 8.0 * k, r.top()), r.max - vec2(6.0, 0.0));
        let clip = p.with_clip_rect(text.intersect(p.clip_rect()));
        clip.text(
            pos2(text.left(), r.center().y - 7.0 * k),
            Align2::LEFT_CENTER,
            &t.role,
            theme::prop(11.5 * k),
            TEXT.into(),
        );
        clip.text(
            pos2(text.left(), r.center().y + 8.0 * k),
            Align2::LEFT_CENTER,
            &lane.about,
            theme::prop(9.5 * k),
            TEXT_DIM.into(),
        );
    }
}

/// What feeds a bus, a return or the master, in its first block: a bus's tracks, a return's senders and how much each
/// sends, the master's buses and returns.
pub(super) fn feeds(song: &SongView, kind: &LaneKind) -> Vec<String> {
    match kind {
        LaneKind::Track(_) => Vec::new(),
        LaneKind::Bus(name) => song.tracks.iter().filter(|t| t.bus == *name).map(|t| t.name.clone()).collect(),
        LaneKind::Return(name) => song
            .tracks
            .iter()
            .map(|t| (t, if name == "reverb" { t.reverb } else { t.delay }))
            .filter(|(_, amount)| *amount > 0.005)
            .map(|(t, amount)| format!("{} {:.0}%", t.name, amount * 100.0))
            .collect(),
        LaneKind::Master => {
            let mut buses: Vec<String> = Vec::new();
            for t in &song.tracks {
                let b = format!("{} bus", t.bus);
                if !buses.contains(&b) {
                    buses.push(b);
                }
            }
            buses.extend(["reverb return".to_string(), "delay return".to_string()]);
            buses
        }
    }
}

/// a bus's, a return's or the master's first block: what feeds it, as many as it has room for
fn inputs(ui: &Ui, song: &SongView, map: &Map, lane: &Lane) {
    let r = map.rect(Rect::from_min_size(pos2(PAD + 4.0, lane.top + 8.0), vec2(SOURCE_W - 12.0, LANE_H - 16.0)));
    if !r.intersects(ui.clip_rect()) {
        return;
    }
    let p = ui.painter();
    p.rect(
        r,
        6.0 * map.zoom.min(1.0),
        theme::alpha(WELL, 0.6),
        Stroke::new(1.0, theme::alpha(lane.color, 0.45)),
        StrokeKind::Inside,
    );
    if map.zoom < 0.45 {
        return;
    }
    let k = map.zoom.min(1.4);
    let from = feeds(song, &lane.kind);
    let clip = p.with_clip_rect(r.shrink(2.0).intersect(p.clip_rect()));
    let line = 12.5 * k;
    clip.text(
        pos2(r.left() + 8.0, r.top() + 4.0 + line * 0.6),
        Align2::LEFT_CENTER,
        "from",
        theme::prop(9.0 * k),
        TEXT_FAINT.into(),
    );
    let rows = (((r.height() - 8.0 - line * 1.2) / line).floor() as usize).max(1);
    let shown = if from.len() > rows { rows.saturating_sub(1) } else { from.len() };
    for (j, name) in from.iter().take(shown).enumerate() {
        clip.text(
            pos2(r.left() + 8.0, r.top() + 4.0 + line * (1.7 + j as f32)),
            Align2::LEFT_CENTER,
            name,
            theme::prop(10.0 * k),
            TEXT_DIM.into(),
        );
    }
    if from.len() > shown {
        let more = format!("and {} more", from.len() - shown);
        clip.text(
            pos2(r.left() + 8.0, r.top() + 4.0 + line * (1.7 + shown as f32)),
            Align2::LEFT_CENTER,
            more,
            theme::prop(10.0 * k),
            TEXT_FAINT.into(),
        );
    }
    if from.is_empty() {
        clip.text(
            pos2(r.left() + 8.0, r.top() + 4.0 + line * 1.7),
            Align2::LEFT_CENTER,
            "nothing yet",
            theme::prop(10.0 * k),
            TEXT_FAINT.into(),
        );
    }
}

/// The signal's line through a lane, under its blocks: from its first block through each effect to its strip and on to
/// where its sound goes, an arrow in each gap, so the eye follows the sound across the room a short chain leaves.
fn signal(ui: &Ui, map: &Map, lane: &Lane, cols: &Columns) {
    let y = lane.top + LANE_H / 2.0;
    let ends = |l: f32, r: f32| [map.pos(pos2(l, y)), map.pos(pos2(r, y))];
    let stroke = Stroke::new((1.6 * map.zoom).clamp(1.0, 2.4), theme::alpha(lane.color, 0.55));
    let p = ui.painter();
    let start = PAD + SOURCE_W - 8.0;
    let faderless = matches!(lane.kind, LaneKind::Bus(_) | LaneKind::Return(_));
    p.line_segment(ends(start, if faderless { cols.out } else { cols.strip }), stroke);
    if !faderless {
        p.line_segment(ends(cols.strip + STRIP_W, cols.out), stroke);
    }
    // an arrowhead in each slot, pointing the way the sound goes
    if map.zoom >= 0.5 {
        let k = 4.0 * map.zoom.min(1.3);
        for j in 0..=lane.fx.len() {
            let c = map.pos(pos2(lane.slot(j).center().x, y));
            p.add(Shape::convex_polygon(
                vec![c + vec2(k, 0.0), c + vec2(-k, -k * 0.8), c + vec2(-k, k * 0.8)],
                theme::alpha(lane.color, 0.8),
                Stroke::NONE,
            ));
        }
    }
}

/// a block being dragged: from which lane and effect, where it is, and whether it was let go this frame
struct Dragging {
    lane: usize,
    from: usize,
    at: Pos2,
    stopped: bool,
}

/// a lane's chain: its blocks and the + slots between them; a block dragged is said back
fn chain(ui: &mut Ui, cx: &mut Cx, song: &SongView, map: &Map, lane: &Lane, lanes: &[Lane]) -> Option<Dragging> {
    let index = lanes.iter().position(|l| l.node == lane.node).unwrap_or(0);
    let mut dragging = None;
    let mut changed: Option<Vec<Value>> = None;
    for (i, e) in lane.fx.iter().enumerate() {
        let r = map.rect(lane.block(i).shrink2(vec2(0.0, 6.0)));
        if !r.intersects(ui.clip_rect()) {
            continue;
        }
        let node = format!("{}#{i}", lane.chain.target());
        let sense = if lane.editable() { Sense::click_and_drag() } else { Sense::click() };
        let resp = ui.interact(r, ui.scope_id().with(("flow-block", &node)), sense);
        let chosen = cx.view.graph.selected.as_deref() == Some(node.as_str());
        let off = is_off(e);
        block(ui, map, r, e, lane.color, Look { chosen, hovered: resp.hovered() || resp.dragged(), off });
        let resp = if resp.dragged() {
            resp.on_hover_and_drag_cursor(egui::CursorIcon::Grabbing)
        } else {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        };
        if resp.clicked() {
            cx.view.graph.selected = Some(node.clone());
            cx.view.graph.json = None;
        }
        // its power and its ×, along its top, once it is big enough to press them
        if lane.editable() && map.zoom >= DETAIL_ZOOM {
            let s = 16.0 * map.zoom.min(1.3);
            let power = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(s, s));
            let pr = ui
                .interact(power, ui.scope_id().with(("flow-power", &node)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let c = if off { TEXT_FAINT } else { theme::GOOD };
            ui.painter().circle_stroke(power.center(), s * 0.36, Stroke::new(1.6, c));
            ui.painter().line_segment(
                [power.center() - vec2(0.0, s * 0.45), power.center() - vec2(0.0, s * 0.12)],
                Stroke::new(1.6, c),
            );
            if pr
                .on_hover_text(if off { "switch it on (B)" } else { "switch it off, keeping its settings (B)" })
                .clicked()
            {
                let mut fx = lane.fx.clone();
                fx[i] = switched(e);
                changed = Some(fx);
            }
            let close = Rect::from_min_size(pos2(r.right() - s - 6.0, r.top() + 6.0), vec2(s, s));
            let cr = ui
                .interact(close, ui.scope_id().with(("flow-remove", &node)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let ink = if cr.hovered() { theme::BAD } else { TEXT_DIM };
            ui.painter().text(
                close.center(),
                Align2::CENTER_CENTER,
                "×",
                theme::prop(13.0 * map.zoom.min(1.3)),
                ink.into(),
            );
            if cr.on_hover_text("take it out (Delete)").clicked() {
                let mut fx = lane.fx.clone();
                fx.remove(i);
                changed = Some(fx);
            }
        }
        if lane.editable() {
            resp.context_menu(|ui| node_menu(ui, cx, song, &NodeKind::Effect(lane.chain.clone(), i)));
        }
        if (resp.dragged() || resp.drag_stopped())
            && let Some(at) = ui.ctx().pointer_interact_pos()
        {
            dragging = Some(Dragging { lane: index, from: i, at, stopped: resp.drag_stopped() });
        }
    }
    // + before each block and after the last, where a block is added
    if lane.editable() && map.zoom >= 0.5 {
        for j in 0..=lane.fx.len() {
            let s = map.rect(lane.slot(j));
            let plus = Rect::from_center_size(s.center(), vec2(18.0, 18.0) * map.zoom.min(1.2));
            if !plus.intersects(ui.clip_rect()) {
                continue;
            }
            let pr = ui
                .interact(plus, ui.scope_id().with(("flow-add", lane.chain.target(), j)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let fill = if pr.hovered() { theme::mix(WELL, ACCENT, 0.25) } else { theme::alpha(WELL, 0.8) };
            ui.painter().circle(plus.center(), plus.width() / 2.0, fill, Stroke::new(1.0, theme::alpha(TEXT_DIM, 0.5)));
            ui.painter().text(
                plus.center(),
                Align2::CENTER_CENTER,
                "+",
                theme::prop(12.0 * map.zoom.min(1.2)),
                TEXT_DIM.into(),
            );
            let pr = pr.on_hover_text("add an effect here");
            egui::Popup::menu(&pr).show(|ui| {
                for (name, template) in EFFECT_TEMPLATES {
                    let (shown, about) = words::effect_in_words(name);
                    if ui.button(shown).on_hover_text(about).clicked() {
                        if let Ok(e) = serde_json::from_str::<Value>(template) {
                            let mut fx = lane.fx.clone();
                            let at = j.min(fx.len());
                            fx.insert(at, e);
                            set_chain(cx, &lane.chain, fx);
                            // chosen, so that its settings are open in the inspector to set
                            cx.view.graph.selected = Some(format!("{}#{at}", lane.chain.target()));
                            cx.view.graph.json = None;
                        }
                        ui.close();
                    }
                }
            });
        }
    }
    if let Some(fx) = changed {
        set_chain(cx, &lane.chain, fx);
    }
    dragging
}

/// where a block dragged would land: a line in the slot under the pointer
fn landing(ui: &Ui, map: &Map, lanes: &[Lane], d: &Dragging) {
    let w = map.world(d.at);
    let Some(target) = lanes.iter().find(|l| l.editable() && w.y >= l.top && w.y <= l.top + LANE_H) else { return };
    let s = map.rect(target.slot(target.slot_at(w.x)));
    ui.painter().line_segment(
        [pos2(s.center().x, s.top() + 4.0), pos2(s.center().x, s.bottom() - 4.0)],
        Stroke::new(3.0, ACCENT),
    );
}

/// a block let go: moved along its chain, or into the chain of the lane it was let go over
fn land(cx: &mut Cx, _song: &SongView, lanes: &[Lane], map: &Map, d: Dragging) {
    let w = map.world(d.at);
    let from = &lanes[d.lane];
    let Some((ti, target)) =
        lanes.iter().enumerate().find(|(_, l)| l.editable() && w.y >= l.top && w.y <= l.top + LANE_H)
    else {
        return;
    };
    let to = target.slot_at(w.x);
    if ti == d.lane {
        if to == d.from || to == d.from + 1 {
            return;
        }
        let (fx, _) = moved(&from.fx, d.from, None, to);
        set_chain(cx, &from.chain, fx);
    } else {
        // out of one lane and into the other as one change, so that one undo puts it back where it was
        let (a, b) = moved(&from.fx, d.from, Some(target.fx.as_slice()), to);
        let name = from.fx.get(d.from).and_then(|e| inner(e).get("fx")).and_then(Value::as_str).unwrap_or_default();
        let said = format!("Move {} from {} to {}", words::effect_in_words(name).0, from.title, target.title);
        let mut chains = vec![(from.chain.clone(), a)];
        chains.extend(b.map(|b| (target.chain.clone(), b)));
        set_chains(cx, said, chains);
    }
}

/// how a block is drawn: chosen, under the pointer, switched off
#[derive(Clone, Copy)]
struct Look {
    chosen: bool,
    hovered: bool,
    off: bool,
}

/// One effect's block: its frame, its name and main value, and, zoomed in, the picture of what it does.
fn block(ui: &Ui, map: &Map, r: Rect, e: &Value, color: Color32, look: Look) {
    let Look { chosen, hovered, off } = look;
    let p = ui.painter();
    let stroke = if chosen {
        Stroke::new(2.0, ACCENT)
    } else if hovered {
        Stroke::new(1.2, TEXT_DIM)
    } else {
        Stroke::new(1.0, LINE)
    };
    let fill = if off { theme::alpha(WELL, 0.5) } else { Color32::from(WELL) };
    p.rect(r, CornerRadius::same((7.0 * map.zoom.min(1.0)) as u8), fill, stroke, StrokeKind::Inside);
    p.rect_filled(
        Rect::from_min_size(r.min, vec2(r.width(), 3.0 * map.zoom.min(1.0))),
        2.0,
        theme::alpha(color, if off { 0.25 } else { 0.85 }),
    );
    let e = inner(e);
    let kind = e.get("fx").and_then(Value::as_str).unwrap_or("effect");
    let (shown, _) = words::effect_in_words(kind);
    let k = map.zoom.min(1.4);
    let ink = if off { Color32::from(TEXT_FAINT) } else { Color32::from(TEXT) };
    if map.zoom < DETAIL_ZOOM {
        let clip = p.with_clip_rect(r.shrink(3.0).intersect(p.clip_rect()));
        clip.text(r.center() - vec2(0.0, 6.0 * k), Align2::CENTER_CENTER, &shown, theme::prop(11.0 * k), ink);
        clip.text(
            r.center() + vec2(0.0, 8.0 * k),
            Align2::CENTER_CENTER,
            main_value(e),
            theme::prop(9.5 * k),
            TEXT_DIM.into(),
        );
        return;
    }
    let head = 26.0 * k;
    let clip = p.with_clip_rect(
        Rect::from_min_max(r.min + vec2(26.0 * k, 0.0), pos2(r.right() - 24.0 * k, r.top() + head))
            .intersect(p.clip_rect()),
    );
    clip.text(
        pos2(r.left() + 28.0 * k, r.top() + head / 2.0 + 1.0),
        Align2::LEFT_CENTER,
        &shown,
        theme::prop(11.5 * k),
        ink,
    );
    let plot = Rect::from_min_max(pos2(r.left() + 8.0, r.top() + head), pos2(r.right() - 8.0, r.bottom() - 18.0 * k));
    picture(ui, plot, e, if off { TEXT_FAINT.into() } else { color });
    let clip = p.with_clip_rect(r.shrink(4.0).intersect(p.clip_rect()));
    clip.text(
        pos2(r.center().x, r.bottom() - 9.0 * k),
        Align2::CENTER_CENTER,
        main_value(e),
        theme::prop(9.5 * k),
        TEXT_DIM.into(),
    );
}

/// what a block says it is set to, in a few words: the summary without its name
fn main_value(e: &Value) -> String {
    let summary = effect_summary(e);
    let name = e.get("fx").and_then(Value::as_str).unwrap_or_default().replace('_', " ");
    summary.strip_prefix(&name).map_or(summary.clone(), |s| s.trim().to_string())
}

/// the picture of what an effect does, in `r`
fn picture(ui: &Ui, r: Rect, e: &Value, color: Color32) {
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let faint = Stroke::new(1.0, theme::alpha(TEXT_FAINT, 0.35));
    let line = Stroke::new(1.8, color);
    let kind = e.get("fx").and_then(Value::as_str).unwrap_or_default();
    let curve = |f: &dyn Fn(f32) -> f32| -> Vec<Pos2> {
        (0..=48)
            .map(|j| j as f32 / 48.0)
            .map(|u| pos2(r.left() + u * r.width(), r.bottom() - f(u).clamp(0.0, 1.0) * r.height()))
            .collect()
    };
    match kind {
        "eq" | "filter" => {
            // 20 Hz to 20 kHz across, ±18 dB up and down
            p.line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], faint);
            let pts = curve(&|u| {
                let f = 20.0 * 1000f64.powf(u as f64);
                let db = response_db(e, f).unwrap_or(0.0).clamp(-18.0, 18.0);
                0.5 + db as f32 / 36.0
            });
            p.add(Shape::line(pts, line));
        }
        "compressor" | "limiter" => {
            // −60 to 0 dB in, out, the diagonal under it
            p.line_segment([r.left_bottom(), r.right_top()], faint);
            let pts = curve(&|u| {
                let input = -60.0 + 60.0 * u as f64;
                ((transfer_db(e, input).unwrap_or(input) + 60.0) / 60.0) as f32
            });
            p.add(Shape::line(pts, line));
            let t = if kind == "limiter" { setting(e, "ceiling_db", -1.0) } else { setting(e, "threshold_db", -18.0) };
            let x = r.left() + ((t + 60.0) / 60.0).clamp(0.0, 1.0) as f32 * r.width();
            p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, theme::alpha(color, 0.35)));
        }
        "saturate" => {
            p.line_segment([r.left_bottom(), r.right_top()], faint);
            let kind = e.get("kind").and_then(Value::as_str).unwrap_or("tanh").to_string();
            let drive = setting(e, "drive_db", 6.0);
            let pts = curve(&|u| (0.5 + 0.5 * shaped(&kind, drive, (2.0 * u - 1.0) as f64)) as f32);
            p.add(Shape::line(pts, line));
        }
        "gain" => {
            let db = setting(e, "db", 0.0) as f32;
            let y = r.center().y - (db / 24.0).clamp(-0.5, 0.5) * r.height();
            p.line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], faint);
            p.rect_filled(
                Rect::from_min_max(
                    pos2(r.center().x - 10.0, y.min(r.center().y)),
                    pos2(r.center().x + 10.0, y.max(r.center().y)),
                ),
                2.0,
                color,
            );
        }
        "reverb" => {
            let decay = setting(e, "decay", 2.0).max(0.1) as f32;
            let pre = (setting(e, "predelay_ms", 0.0) as f32 / 1000.0).min(0.5);
            // four seconds across: a wash that dies away
            let pts = curve(&|u| {
                let t = u * 4.0;
                if t < pre { 0.0 } else { (-(t - pre) * 6.9 / decay).exp() * 0.9 }
            });
            p.add(Shape::line(pts, line));
        }
        "delay" => {
            let fb = setting(e, "feedback", 0.3).clamp(0.0, 0.95) as f32;
            let beats = setting(e, "beats", 0.5).max(0.05) as f32;
            let step = (beats / 4.0).min(1.0) * r.width();
            let mut x = r.left() + 4.0;
            let mut h = 0.9;
            while x < r.right() && h > 0.03 {
                p.line_segment([pos2(x, r.bottom()), pos2(x, r.bottom() - h * r.height())], Stroke::new(2.0, color));
                x += step.max(6.0);
                h *= fb.max(0.05);
            }
        }
        "bitcrush" => {
            let bits = setting(e, "bits", 8.0).clamp(1.0, 16.0) as f32;
            let levels = 2f32.powf(bits.min(6.0));
            let pts = curve(&|u| {
                let v = (u * std::f32::consts::TAU).sin();
                0.5 + 0.45 * (v * levels / 2.0).round() / (levels / 2.0)
            });
            p.add(Shape::line(pts, line));
        }
        "chorus" | "flanger" | "phaser" | "tremolo" | "auto_pan" | "tape" => {
            let rate = setting(e, "rate_hz", 1.0).clamp(0.1, 8.0) as f32;
            let pts = curve(&|u| 0.5 + 0.35 * (u * std::f32::consts::TAU * (1.0 + rate)).sin());
            p.add(Shape::line(pts, line));
        }
        "width" => {
            let w = setting(e, "width", 1.0).clamp(0.0, 2.0) as f32;
            let c = pos2(r.center().x, r.bottom());
            let spread = 0.15 + 0.6 * w / 2.0;
            for s in [-1.0, 1.0] {
                let a = std::f32::consts::FRAC_PI_2 + s * spread * std::f32::consts::FRAC_PI_2;
                p.line_segment([c, c + vec2(a.cos(), -a.sin()) * r.height() * 0.95], line);
            }
        }
        "gate" => {
            let steps: Vec<f32> = e
                .get("steps")
                .and_then(Value::as_array)
                .map(|s| s.iter().filter_map(Value::as_f64).map(|v| v as f32).collect())
                .unwrap_or_default();
            let n = steps.len().max(1);
            for (j, v) in steps.iter().enumerate() {
                let w = r.width() / n as f32;
                let x = r.left() + j as f32 * w;
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(x + 1.0, r.bottom() - v.clamp(0.0, 1.0) * r.height()),
                        pos2(x + w - 1.0, r.bottom()),
                    ),
                    1.0,
                    color,
                );
            }
        }
        _ => {
            p.text(r.center(), Align2::CENTER_CENTER, "·", theme::prop(14.0), theme::alpha(color, 0.6));
        }
    }
}

/// a lane's strip: a track's level and pan with its meter, the master's loudness and ceiling
fn strip(ui: &mut Ui, cx: &mut Cx, song: &SongView, map: &Map, lane: &Lane, cols: &Columns) {
    let r = map.rect(Rect::from_min_size(pos2(cols.strip, lane.top + 8.0), vec2(STRIP_W, LANE_H - 16.0)));
    if !r.intersects(ui.clip_rect()) || matches!(lane.kind, LaneKind::Bus(_) | LaneKind::Return(_)) {
        return;
    }
    ui.painter().rect(
        r,
        6.0 * map.zoom.min(1.0),
        theme::alpha(WELL, 0.7),
        Stroke::new(1.0, theme::alpha(LINE, 0.8)),
        StrokeKind::Inside,
    );
    let k = map.zoom.min(1.3);
    // the fields by the lane's own id: lanes out of view are not drawn, so ids by order would move as the view does
    ui.push_id(("flow-strip", &lane.node), |ui| match lane.kind {
        LaneKind::Track(i) => {
            let t = &song.tracks[i];
            let name = t.name.clone();
            meter(ui, cx, song, r, i);
            if map.zoom < DETAIL_ZOOM {
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    format!("{:+.1} dB", t.level_db),
                    theme::prop(10.5 * k),
                    TEXT.into(),
                );
                return;
            }
            let (mut level, mut pan) = (t.level_db, t.pan);
            let lr = ui.put(
                Rect::from_min_size(r.min + vec2(8.0, 10.0 * k), vec2(r.width() - 26.0, 20.0 * k)),
                egui::DragValue::new(&mut level).speed(0.1).range(-36.0..=12.0).fixed_decimals(1).suffix(" dB"),
            );
            let lr = lr.on_hover_text("its level: drag, or double-click to type");
            let pr = ui.put(
                Rect::from_min_size(r.min + vec2(8.0, 38.0 * k), vec2(r.width() - 26.0, 20.0 * k)),
                egui::DragValue::new(&mut pan)
                    .speed(0.01)
                    .range(-1.0..=1.0)
                    .custom_formatter(|v, _| words::pan_in_words(v as f32))
                    .custom_parser(super::parse_pan),
            );
            let pr = pr.on_hover_text("where it sits between the speakers: drag, or double-click to type");
            let mut sent: Vec<MixEdit> = Vec::new();
            if let Some(st) = cx.state.song.as_mut().and_then(|s| s.tracks.get_mut(i)) {
                if lr.changed() {
                    st.level_db = level;
                }
                if commit(&lr) {
                    sent.push(MixEdit { level_db: Some(level), ..Default::default() });
                }
                if pr.changed() {
                    st.pan = pan;
                }
                if commit(&pr) {
                    sent.push(MixEdit { pan: Some(pan), ..Default::default() });
                }
            }
            cx.out.extend(sent.into_iter().map(|e| e.on(&name)));
        }
        LaneKind::Master => {
            let (mut lufs, mut ceiling) = (song.mix.lufs, song.mix.ceiling_db);
            if map.zoom < DETAIL_ZOOM {
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    format!("{lufs:.1} LUFS"),
                    theme::prop(10.5 * k),
                    TEXT.into(),
                );
                return;
            }
            let lr = ui.put(
                Rect::from_min_size(r.min + vec2(8.0, 10.0 * k), vec2(r.width() - 16.0, 20.0 * k)),
                egui::DragValue::new(&mut lufs).speed(0.1).range(-24.0..=-6.0).fixed_decimals(1).suffix(" LUFS"),
            );
            let lr = lr.on_hover_text("how loud the finished song is made: drag, or double-click to type");
            let cr = ui.put(
                Rect::from_min_size(r.min + vec2(8.0, 38.0 * k), vec2(r.width() - 16.0, 20.0 * k)),
                egui::DragValue::new(&mut ceiling).speed(0.05).range(-6.0..=0.0).fixed_decimals(1).suffix(" dBTP"),
            );
            let cr = cr.on_hover_text("the highest its peaks may reach: drag, or double-click to type");
            if let Some(s) = cx.state.song.as_mut() {
                if lr.changed() {
                    s.mix.lufs = lufs;
                }
                if cr.changed() {
                    s.mix.ceiling_db = ceiling;
                }
            }
            if commit(&lr) {
                set_loudness(cx, lufs);
            } else if commit(&cr) {
                cx.out.push(Command::RunScript { code: format!("master(#{{ lufs: {lufs}, ceiling: {ceiling} }});") });
            }
        }
        LaneKind::Bus(_) | LaneKind::Return(_) => {}
    });
}

/// a track's meter down the strip's right side: how loud it is now as it plays, else as the last render heard it
fn meter(ui: &Ui, cx: &Cx, song: &SongView, r: Rect, i: usize) {
    let t = &song.tracks[i];
    let sounds = super::mixer::sounds(
        t,
        song.layers.contains(&t.name),
        super::mixer::layers_at(song, cx.playhead_bar()).as_deref(),
        super::any_solo(song),
    );
    let live = cx
        .state
        .render
        .as_ref()
        .filter(|_| cx.player.is_playing())
        .and_then(|rv| super::mixer::envelope_db(rv, cx.player.position()))
        .map(|(at, avg)| t.loudness.filter(|_| sounds).map(|l| l as f32 + (at - avg)));
    let level = live.unwrap_or(t.loudness.map(|l| l as f32));
    let m = Rect::from_min_max(pos2(r.right() - 10.0, r.top() + 6.0), pos2(r.right() - 5.0, r.bottom() - 6.0));
    ui.painter().rect_filled(m, 2.0, WELL);
    if let Some(db) = level {
        let k = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
        let lit = Rect::from_min_max(pos2(m.left(), m.bottom() - k * m.height()), m.max);
        ui.painter().rect_filled(lit, 2.0, if db > -6.0 { theme::WARN } else { theme::GOOD });
    }
}

/// a track's sends to the reverb and the delay returns
fn sends(ui: &mut Ui, cx: &mut Cx, song: &SongView, map: &Map, lane: &Lane, i: usize, cols: &Columns) {
    let r = map.rect(Rect::from_min_size(pos2(cols.sends, lane.top + 8.0), vec2(SENDS_W, LANE_H - 16.0)));
    if !r.intersects(ui.clip_rect()) || map.zoom < DETAIL_ZOOM {
        return;
    }
    let t = &song.tracks[i];
    let name = t.name.clone();
    let k = map.zoom.min(1.3);
    let send = |ui: &mut Ui, y: f32, value: &mut f32, word: &'static str, tip: &'static str| {
        ui.put(
            Rect::from_min_size(pos2(r.left() + 4.0, r.top() + y * k), vec2(r.width() - 8.0, 20.0 * k)),
            egui::DragValue::new(value)
                .speed(0.005)
                .range(0.0..=1.0)
                .custom_formatter(move |v, _| format!("{word} {:.0}%", v * 100.0))
                .custom_parser(move |s| super::percent(s.trim().trim_start_matches(word))),
        )
        .on_hover_text(tip)
    };
    let (mut reverb, mut delay) = (t.reverb, t.delay);
    // by the lane's own id, as the strip's fields are
    let (rv, dv) = ui
        .push_id(("flow-sends", &lane.node), |ui| {
            let rv = send(
                ui,
                10.0,
                &mut reverb,
                "reverb",
                "how much of it goes into the reverb return: drag, or double-click to type",
            );
            let dv = send(
                ui,
                38.0,
                &mut delay,
                "delay",
                "how much of it goes into the delay return: drag, or double-click to type",
            );
            (rv, dv)
        })
        .inner;
    let mut sent: Vec<MixEdit> = Vec::new();
    if let Some(st) = cx.state.song.as_mut().and_then(|s| s.tracks.get_mut(i)) {
        if rv.changed() {
            st.reverb = reverb;
        }
        if commit(&rv) {
            sent.push(MixEdit { reverb: Some(reverb), ..Default::default() });
        }
        if dv.changed() {
            st.delay = delay;
        }
        if commit(&dv) {
            sent.push(MixEdit { delay: Some(delay), ..Default::default() });
        }
    }
    cx.out.extend(sent.into_iter().map(|e| e.on(&name)));
}

/// where a lane's sound goes, at its end
fn out_pill(ui: &Ui, map: &Map, lane: &Lane, cols: &Columns) {
    let r = map.rect(Rect::from_min_size(pos2(cols.out, lane.top + LANE_H / 2.0 - 13.0), vec2(OUT_W, 26.0)));
    if !r.intersects(ui.clip_rect()) {
        return;
    }
    let k = map.zoom.min(1.3);
    let p = ui.painter();
    p.rect(
        r,
        CornerRadius::same((13.0 * map.zoom.min(1.0)) as u8),
        theme::alpha(lane.color, 0.12),
        Stroke::new(1.0, theme::alpha(lane.color, 0.6)),
        StrokeKind::Inside,
    );
    let clip = p.with_clip_rect(r.shrink(2.0).intersect(p.clip_rect()));
    clip.text(r.center(), Align2::CENTER_CENTER, format!("→ {}", lane.out), theme::prop(10.5 * k), TEXT.into());
}

/// the lanes' headers, kept at the left: a lane's name, colour, what it is, and a track's mute and solo
fn headers(ui: &mut Ui, cx: &mut Cx, song: &SongView, map: &Map, lanes: &[Lane], canvas: Rect) {
    let column = Rect::from_min_max(canvas.min, pos2(canvas.left() + HEAD_W, canvas.bottom()));
    ui.painter().rect_filled(column, 0.0, theme::PANEL);
    for lane in lanes {
        let top = map.pos(pos2(0.0, lane.top)).y;
        let r = Rect::from_min_max(pos2(column.left() + 6.0, top), pos2(column.right() - 6.0, top + LANE_H * map.zoom));
        if !r.intersects(column) {
            continue;
        }
        let resp = ui
            .interact(r.intersect(column), ui.scope_id().with(("flow-head", &lane.node)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let chosen = cx.view.graph.selected.as_deref() == Some(lane.node.as_str());
        let p = ui.painter().with_clip_rect(column);
        p.rect(
            r,
            6.0,
            if chosen { theme::mix(RAISED, ACCENT, 0.12) } else { Color32::from(RAISED) },
            Stroke::new(if chosen { 1.6 } else { 1.0 }, if chosen { ACCENT } else { LINE }),
            StrokeKind::Inside,
        );
        p.rect_filled(
            Rect::from_min_size(r.min + vec2(0.0, 6.0), vec2(3.0, (r.height() - 12.0).max(2.0))),
            1.5,
            lane.color,
        );
        let k = map.zoom.clamp(0.6, 1.0);
        p.text(
            pos2(r.left() + 12.0, r.top() + 12.0 * k),
            Align2::LEFT_CENTER,
            &lane.title,
            theme::prop(12.5 * k),
            TEXT.into(),
        );
        if r.height() > 40.0 {
            p.text(
                pos2(r.left() + 12.0, r.top() + 30.0 * k),
                Align2::LEFT_CENTER,
                &lane.about,
                theme::prop(10.0 * k),
                TEXT_DIM.into(),
            );
        }
        if resp.clicked() {
            cx.view.graph.selected = Some(lane.node.clone());
            cx.view.graph.json = None;
        }
        let kind = match &lane.kind {
            LaneKind::Track(i) => Some(NodeKind::Track(song.tracks[*i].name.clone())),
            LaneKind::Bus(n) => Some(NodeKind::Bus(n.clone())),
            LaneKind::Return(n) => Some(NodeKind::Return(n.clone())),
            LaneKind::Master => None,
        };
        if let Some(kind) = kind {
            resp.context_menu(|ui| node_menu(ui, cx, song, &kind));
        }
        if let LaneKind::Track(i) = lane.kind
            && r.height() > 56.0
        {
            let t = &song.tracks[i];
            let name = t.name.clone();
            let y = r.bottom() - 26.0;
            let toggle = |ui: &mut Ui, x: f32, label: &str, on: bool, color: Color32, tip: &str| -> bool {
                let b = Rect::from_min_size(pos2(x, y), vec2(20.0, 18.0));
                let resp = ui
                    .interact(b, ui.scope_id().with(("flow-toggle", label, &name)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let fill = if on {
                    color
                } else if resp.hovered() {
                    theme::alpha(TEXT, 0.08)
                } else {
                    WELL.into()
                };
                ui.painter().rect_filled(b, 4.0, fill);
                ui.painter().text(
                    b.center(),
                    Align2::CENTER_CENTER,
                    label,
                    theme::prop(10.5),
                    if on { theme::BG.into() } else { TEXT_DIM.into() },
                );
                resp.on_hover_text(tip).clicked()
            };
            let muted = toggle(ui, r.left() + 12.0, "M", t.muted, theme::BAD.into(), "mute it");
            let solo =
                toggle(ui, r.left() + 36.0, "S", t.solo, theme::SOLO.into(), "solo it: only the soloed tracks play");
            let mut sent: Vec<MixEdit> = Vec::new();
            if let Some(st) = cx.state.song.as_mut().and_then(|s| s.tracks.get_mut(i)) {
                if muted {
                    st.muted = !st.muted;
                    sent.push(MixEdit { muted: Some(st.muted), ..Default::default() });
                }
                if solo {
                    st.solo = !st.solo;
                    sent.push(MixEdit { solo: Some(st.solo), ..Default::default() });
                }
            }
            cx.out.extend(sent.into_iter().map(|e| e.on(&name)));
        }
    }
}

/// the keyboard, with the lanes focused: the arrows choose a block (its settings in the inspector), Delete takes it
/// out, B switches it off or on, F fits, + and − zoom
fn keys(ui: &mut Ui, cx: &mut Cx, _song: &SongView, lanes: &[Lane], world: Vec2, view: Vec2, origin: Pos2) {
    let pressed = |ui: &mut Ui, k: Key| ui.input_mut(|i| i.consume_key(Modifiers::NONE, k));
    let g = &mut cx.view.graph;
    if pressed(ui, Key::F) {
        (g.zoom, g.pan) = fitted(world, view);
    }
    let centre = origin + view / 2.0;
    if pressed(ui, Key::Plus) || pressed(ui, Key::Equals) {
        (g.zoom, g.pan) = zoomed(g.zoom, g.pan, origin, centre, 1.25);
    }
    if pressed(ui, Key::Minus) {
        (g.zoom, g.pan) = zoomed(g.zoom, g.pan, origin, centre, 0.8);
    }
    // where the choice is: a lane and a block in it (none: its header)
    let at = g.selected.as_deref().and_then(|s| {
        lanes.iter().enumerate().find_map(|(li, l)| {
            if s == l.node {
                return Some((li, None));
            }
            let rest = s.strip_prefix(&format!("{}#", l.chain.target()))?;
            rest.parse::<usize>().ok().filter(|i| *i < l.fx.len()).map(|i| (li, Some(i)))
        })
    });
    let id_of = |li: usize, i: Option<usize>| -> String {
        match i {
            Some(i) => format!("{}#{i}", lanes[li].chain.target()),
            None => lanes[li].node.clone(),
        }
    };
    let (li, bi) = at.unwrap_or((0, None));
    let mut next = None;
    if pressed(ui, Key::ArrowRight) {
        next = Some(match bi {
            None if !lanes[li].fx.is_empty() => (li, Some(0)),
            Some(i) if i + 1 < lanes[li].fx.len() => (li, Some(i + 1)),
            other => (li, other),
        });
    }
    if pressed(ui, Key::ArrowLeft) {
        next = Some(match bi {
            Some(0) | None => (li, None),
            Some(i) => (li, Some(i - 1)),
        });
    }
    if pressed(ui, Key::ArrowDown) && li + 1 < lanes.len() {
        let n = &lanes[li + 1];
        next = Some((li + 1, bi.map(|i| i.min(n.fx.len().saturating_sub(1))).filter(|_| !n.fx.is_empty())));
    }
    if pressed(ui, Key::ArrowUp) && li > 0 {
        let n = &lanes[li - 1];
        next = Some((li - 1, bi.map(|i| i.min(n.fx.len().saturating_sub(1))).filter(|_| !n.fx.is_empty())));
    }
    if let Some((l, b)) = next {
        cx.view.graph.selected = Some(id_of(l, b));
        cx.view.graph.json = None;
        // the choice kept in view
        let r = match b {
            Some(i) => lanes[l].block(i),
            None => Rect::from_min_size(pos2(0.0, lanes[l].top), vec2(PAD, LANE_H)),
        };
        let g = &mut cx.view.graph;
        let (lo, hi) = (g.pan, g.pan + view / g.zoom);
        if r.left() < lo.x || r.right() > hi.x {
            g.pan.x = (r.center().x - view.x / g.zoom / 2.0).max(-PAD);
        }
        if r.top() < lo.y || r.bottom() > hi.y {
            g.pan.y = (r.center().y - view.y / g.zoom / 2.0).max(-PAD);
        }
    }
    if let Some((l, Some(i))) = at {
        let lane = &lanes[l];
        if lane.editable() && (pressed(ui, Key::Delete) || pressed(ui, Key::Backspace)) {
            let mut fx = lane.fx.clone();
            fx.remove(i);
            set_chain(cx, &lane.chain, fx);
            cx.view.graph.selected = Some(lane.node.clone());
        } else if lane.editable() && pressed(ui, Key::B) {
            let mut fx = lane.fx.clone();
            fx[i] = switched(&fx[i]);
            set_chain(cx, &lane.chain, fx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claudius_api::TrackView;

    fn song() -> SongView {
        let track = |name: &str, bus: &str, fx: Vec<Value>| TrackView {
            name: name.into(),
            bus: bus.into(),
            fx,
            reverb: if name == "lead" { 0.3 } else { 0.0 },
            ..Default::default()
        };
        SongView {
            tracks: vec![
                track("kick", "drums", vec![]),
                track("bass", "bass", vec![json!({"fx": "saturate", "kind": "tape", "drive_db": 6})]),
                track("lead", "music", vec![json!({"fx": "eq", "bands": []}), json!({"fx": "delay", "beats": 0.75})]),
            ],
            ..Default::default()
        }
    }

    /// The tracks come first in their order, then the buses the graph has, both returns and the master last; and a
    /// lane's blocks follow one another, every block with a slot before it and one after the last.
    #[test]
    fn the_lanes_run_from_the_tracks_to_the_master_and_their_blocks_left_to_right() {
        let s = song();
        let lanes = lanes(&s, &s.graph());
        let kinds: Vec<&LaneKind> = lanes.iter().map(|l| &l.kind).collect();
        assert_eq!(kinds[..3], [&LaneKind::Track(0), &LaneKind::Track(1), &LaneKind::Track(2)]);
        assert!(matches!(lanes.last().unwrap().kind, LaneKind::Master));
        let returns = lanes.iter().filter(|l| matches!(l.kind, LaneKind::Return(_))).count();
        assert_eq!(returns, 2, "both returns, sent to or not");
        assert!(lanes.windows(2).all(|w| w[1].top > w[0].top), "top to bottom");
        let lead = &lanes[2];
        assert!(lead.slot(0).right() <= lead.block(0).left() + 0.01);
        assert!(lead.block(0).right() <= lead.slot(1).left() + 0.01);
        assert!(lead.block(1).right() <= lead.slot(2).left() + 0.01);
        assert_eq!(lead.slot_at(lead.block(0).center().x - 50.0), 0);
        assert_eq!(lead.slot_at(lead.block(1).center().x + 50.0), 2);
        let cols = columns(&lanes);
        assert!(lanes.iter().all(|l| l.chain_end() < cols.strip), "the strips line up after every chain");
        assert!(cols.strip < cols.sends && cols.sends < cols.out && cols.out < cols.world.x);
    }

    /// A bus says the tracks that feed it, a return its senders and how much each sends, and the master its buses and
    /// both returns; and every lane's blocks start in the same column.
    #[test]
    fn each_lane_says_what_feeds_it_and_the_blocks_line_up() {
        let s = song();
        assert_eq!(feeds(&s, &LaneKind::Bus("music".into())), ["lead"]);
        assert_eq!(feeds(&s, &LaneKind::Return("reverb".into())), ["lead 30%"]);
        assert!(feeds(&s, &LaneKind::Return("delay".into())).is_empty());
        assert_eq!(
            feeds(&s, &LaneKind::Master),
            ["drums bus", "bass bus", "music bus", "reverb return", "delay return"]
        );
        let lanes = lanes(&s, &s.graph());
        let firsts: Vec<f32> = lanes.iter().map(|l| l.block(0).left()).collect();
        assert!(firsts.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01), "{firsts:?}");
    }

    /// Zooming keeps the world's point under the pointer where it was, at every zoom, and stops at the zoom's ends.
    #[test]
    fn a_zoom_keeps_the_point_under_the_pointer_and_stays_in_its_range() {
        let origin = pos2(100.0, 50.0);
        let (mut zoom, mut pan) = (1.0, vec2(30.0, 10.0));
        let at = pos2(400.0, 300.0);
        let under = Map { origin, zoom, pan }.world(at);
        for factor in [1.25, 1.25, 0.5, 2.0, 0.8] {
            (zoom, pan) = zoomed(zoom, pan, origin, at, factor);
            let now = Map { origin, zoom, pan }.world(at);
            assert!((now - under).length() < 1e-3, "{factor}: {now:?} against {under:?}");
        }
        assert_eq!(zoomed(MAX_ZOOM, pan, origin, at, 2.0).0, MAX_ZOOM);
        assert_eq!(zoomed(MIN_ZOOM, pan, origin, at, 0.5).0, MIN_ZOOM);
        let (z, p) = fitted(vec2(2000.0, 1000.0), vec2(1000.0, 1000.0));
        assert!((z - 0.5).abs() < 1e-6 && p == Vec2::ZERO);
        assert_eq!(fitted(vec2(100.0, 100.0), vec2(1000.0, 1000.0)).0, 1.0, "never larger than as it is");
        // opened, the lanes fill the width and run down the view, however many there are
        let (z, p) = fitted_width(vec2(1100.0, 3000.0), vec2(1100.0, 700.0));
        assert!((z - 1.0).abs() < 1e-6 && p == Vec2::ZERO);
        assert!((fitted_width(vec2(2200.0, 3000.0), vec2(1100.0, 700.0)).0 - 0.5).abs() < 1e-6);
    }

    /// An effect dragged from one lane into another is one change, which one undo takes back whole: out of the first
    /// lane and into the second, said as a move.
    #[test]
    fn an_effect_dragged_into_another_lane_is_one_change() {
        use claudius_api::{Command, State};

        use super::super::View;
        use crate::audio::Player;
        let s = song();
        let all = lanes(&s, &s.graph());
        let ctx = egui::Context::default();
        let (mut state, mut view, mut player) = (State::default(), View::default(), Player::new(&ctx));
        state.song = Some(s.clone());
        let mut out = Vec::new();
        let mut cx = Cx {
            state: &mut state,
            view: &mut view,
            player: &mut player,
            out: &mut out,
            sounds: None,
            server: None,
            now: 0.0,
        };
        let map = Map { origin: Pos2::ZERO, zoom: 1.0, pan: Vec2::ZERO };
        // the bass's saturation (its lane's first block) let go after the lead's delay
        let lead = &all[2];
        let at = map.pos(pos2(lead.slot(2).center().x, lead.top + LANE_H / 2.0));
        land(&mut cx, &s, &all, &map, Dragging { lane: 1, from: 0, at, stopped: true });
        let [Command::Together { said, commands }] = cx.out.as_slice() else { panic!("not one change: {:?}", cx.out) };
        assert!(said.starts_with("Move ") && said.ends_with("from bass to lead"), "{said}");
        let chains: Vec<(&str, usize)> = commands
            .iter()
            .filter_map(|c| match c {
                Command::SetFx { target, fx } => Some((target.as_str(), fx.len())),
                _ => None,
            })
            .collect();
        assert_eq!(chains, [("track:bass", 0), ("track:lead", 3)], "out of the bass, into the lead after its two");
        let lead_fx = &cx.state.song.as_ref().unwrap().tracks[2].fx;
        assert_eq!(lead_fx[2]["fx"], "saturate", "the studio shows it moved at once");
    }

    /// Switched off, an effect keeps everything it had; switched on again it is what it was.
    #[test]
    fn an_effect_switched_off_and_on_again_is_what_it_was() {
        let e = json!({"fx": "compressor", "threshold_db": -20, "ratio": 4, "attack_ms": 5, "release_ms": 80});
        let off = switched(&e);
        assert!(is_off(&off) && !is_off(&e));
        assert_eq!(off["effect"], e);
        assert_eq!(inner(&off), &e);
        assert_eq!(switched(&off), e);
    }

    /// A block moves along its chain to the slot it is let go in, and into another chain at the slot there.
    #[test]
    fn a_block_moves_to_the_slot_it_is_let_go_in() {
        let fx: Vec<Value> = ["a", "b", "c"].iter().map(|n| json!({"fx": n})).collect();
        let names = |v: &[Value]| v.iter().map(|e| e["fx"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(names(&moved(&fx, 0, None, 3).0), ["b", "c", "a"]);
        assert_eq!(names(&moved(&fx, 2, None, 0).0), ["c", "a", "b"]);
        assert_eq!(names(&moved(&fx, 1, None, 1).0), ["a", "b", "c"], "let go where it was");
        let other = vec![json!({"fx": "x"})];
        let (a, b) = moved(&fx, 1, Some(other.as_slice()), 1);
        assert_eq!(
            (names(&a), names(&b.unwrap())),
            (vec!["a".to_string(), "c".into()], vec!["x".to_string(), "b".into()])
        );
    }

    /// The pictures' curves say what the effects do: a low-pass passes the lows and takes the highs away, a peak lifts
    /// its band, a compressor turns down what is over its threshold by its ratio, a limiter holds its ceiling, and
    /// saturation stays within its frame however hard it is driven.
    #[test]
    fn the_curves_say_what_each_effect_does() {
        let lp = json!({"fx": "filter", "mode": "lp", "cutoff": 1000});
        assert!(response_db(&lp, 100.0).unwrap().abs() < 0.5);
        assert!(response_db(&lp, 8000.0).unwrap() < -20.0);
        let eq = json!({"fx": "eq", "bands": [{"kind": "peak", "freq": 1000, "gain_db": 6, "q": 1}]});
        assert!((response_db(&eq, 1000.0).unwrap() - 6.0).abs() < 0.1);
        assert!(response_db(&eq, 50.0).unwrap().abs() < 0.3);
        let comp = json!({"fx": "compressor", "threshold_db": -20, "ratio": 4});
        assert!((transfer_db(&comp, -30.0).unwrap() + 30.0).abs() < 1e-9);
        assert!((transfer_db(&comp, 0.0).unwrap() + 15.0).abs() < 1e-9);
        let lim = json!({"fx": "limiter", "ceiling_db": -1});
        assert!((transfer_db(&lim, 0.0).unwrap() + 1.0).abs() < 0.01);
        assert!(transfer_db(&json!({"fx": "reverb"}), 0.0).is_none());
        for kind in ["tanh", "tube", "tape", "hard", "fold"] {
            for drive in [0.0, 12.0, 30.0] {
                for x in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                    assert!(shaped(kind, drive, x).abs() <= 1.0 + 1e-9, "{kind} at {drive} dB, {x}");
                }
            }
        }
        assert_eq!(first_number(&json!({"sections": {"drop": 9000}, "default": 1200})), Some(1200.0));
        assert_eq!(first_number(&json!([1200, 9000])), Some(1200.0));
    }
}
