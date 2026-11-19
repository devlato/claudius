//! The signal flow as a board to work on (`graph::show`): the song's sound from its sources to the finished song, left
//! to right, fitted to the width, and every part of it changed where it is shown.
//!
//! Each track is a row: its name and sound, its level and pan, mute and solo, its effects as chips (a click opens an
//! effect's settings beside it, a drag moves it along its chain, + adds one), a meter, and along its foot the sections
//! it plays in. Its wire runs into its bus, and its sends into the reverb and delay returns, as thick as they are. The
//! buses and the returns, with their chips, run into the master: its chain, its loudness, and then the finished song.
//! A right-click offers what can be done to anything, and a click on a row opens all of it in the inspector beside the
//! board (`graph::inspector`).

use claudius_api::{Chain, Graph, NodeKind, SongView, effect_summary, words};
use eframe::egui::{self, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::Value;

use super::graph::{add_menu, chain_of, effect, node_menu, set_chain};
use super::{Cx, layer_color};
use crate::cmds::MixEdit;
use crate::theme::{self, ACCENT, LINE, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::commit;

const MARGIN: f32 = 14.0;
const TRACK_W: f32 = 330.0;
const BUS_W: f32 = 230.0;
const MASTER_W: f32 = 230.0;
const OUT_W: f32 = 170.0;
/// the least room between two columns, for the wires
const GAP: f32 = 64.0;
const ROW_GAP: f32 = 8.0;
/// a row's title line, and a line of chips under it
const TITLE_H: f32 = 28.0;
const CHIP_H: f32 = 20.0;
const CHIP_GAP: f32 = 4.0;
/// a chip's widest, its summary cut short beyond it
const CHIP_MAX: f32 = 160.0;
/// the strip of sections along a track's foot
const FOOT_H: f32 = 3.0;

/// what a row of the board is
#[derive(Clone)]
enum Kind {
    Track(usize),
    Bus(String),
    Return(String),
}

/// a row: what it is, its node in the graph (chosen by a click), its title and a line about it, its chain and its card
struct Row {
    kind: Kind,
    node: String,
    title: String,
    about: String,
    chain: Option<Chain>,
    fx: Vec<Value>,
    card: Rect,
    color: Color32,
}

/// The board, in what the inspector leaves of the width.
pub(super) fn show(ui: &mut Ui, cx: &mut Cx, song: &SongView, graph: &Graph) {
    let area = ui.available_rect_before_wrap();
    let font = theme::prop(10.5);
    // what each chain's chips need, to know how tall each row stands
    let chip_w = |ui: &Ui, v: &Value| -> f32 {
        let w = ui.painter().layout_no_wrap(effect_summary(v), font.clone(), TEXT.into()).size().x;
        (w + 16.0).clamp(36.0, CHIP_MAX)
    };
    let lines_of = |ui: &Ui, fx: &[Value], width: f32| -> usize {
        let (mut x, mut lines) = (0.0, 1);
        for w in fx.iter().map(|v| chip_w(ui, v)).chain(std::iter::once(24.0)) {
            if x > 0.0 && x + w > width {
                lines += 1;
                x = 0.0;
            }
            x += w + CHIP_GAP;
        }
        lines
    };
    let height_of = |lines: usize, foot: bool| -> f32 {
        TITLE_H + lines as f32 * (CHIP_H + CHIP_GAP) + 6.0 + if foot { FOOT_H + 4.0 } else { 0.0 }
    };

    let ([x_track, x_bus, x_master, x_out], width) = columns(area.width());
    let (x_track, x_bus, x_master, x_out) =
        (area.left() + x_track, area.left() + x_bus, area.left() + x_master, area.left() + x_out);

    // the tracks, in the graph's order
    let mut rows: Vec<Row> = Vec::new();
    let mut y = area.top() + MARGIN;
    let mut track_nodes: Vec<&claudius_api::Node> =
        graph.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Track(_))).collect();
    track_nodes.sort_by(|a, b| a.row.total_cmp(&b.row));
    for n in track_nodes {
        let NodeKind::Track(name) = &n.kind else { continue };
        let Some(i) = song.tracks.iter().position(|t| &t.name == name) else { continue };
        let t = &song.tracks[i];
        let fx = chain_of(song, &Chain::Track(name.clone()));
        let h = height_of(lines_of(ui, &fx, TRACK_W - 24.0), true);
        let sound = words::name_in_words(&t.sound);
        let about = if t.duck > 0.01 { format!("{sound} · ducks {:.0}%", t.duck * 100.0) } else { sound };
        rows.push(Row {
            kind: Kind::Track(i),
            node: n.id.clone(),
            title: t.name.clone(),
            about,
            chain: Some(Chain::Track(name.clone())),
            fx,
            card: Rect::from_min_size(pos2(x_track, y), vec2(TRACK_W, h)),
            color: layer_color(song, name),
        });
        y += h + ROW_GAP;
    }
    let tracks_bottom = y;

    // the buses the graph has (every bus a track plays into, not only those with settings of their own), each level
    // with its tracks where there is room, then the returns under them
    let mut y = area.top() + MARGIN;
    let mut bus_nodes: Vec<&claudius_api::Node> =
        graph.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Bus(_))).collect();
    bus_nodes.sort_by(|a, b| a.row.total_cmp(&b.row));
    for n in bus_nodes {
        let NodeKind::Bus(name) = &n.kind else { continue };
        let members: Vec<&Row> =
            rows.iter().filter(|r| matches!(r.kind, Kind::Track(i) if song.tracks[i].bus == *name)).collect();
        let fx = chain_of(song, &Chain::Bus(name.clone()));
        let h = height_of(lines_of(ui, &fx, BUS_W - 24.0), false);
        let centre = if members.is_empty() {
            y + h / 2.0
        } else {
            members.iter().map(|r| r.card.center().y).sum::<f32>() / members.len() as f32
        };
        let top = (centre - h / 2.0).max(y);
        let count = members.len();
        rows.push(Row {
            kind: Kind::Bus(name.clone()),
            node: n.id.clone(),
            title: format!("{name} bus"),
            about: format!("{count} track{}", if count == 1 { "" } else { "s" }),
            chain: Some(Chain::Bus(name.clone())),
            fx,
            card: Rect::from_min_size(pos2(x_bus, top), vec2(BUS_W, h)),
            color: ACCENT.into(),
        });
        y = top + h + ROW_GAP * 2.0;
    }
    for (name, setting, color) in
        [("reverb", &song.mix.reverb, Color32::from(theme::INFO)), ("delay", &song.mix.delay, Color32::from(ACCENT))]
    {
        let h = TITLE_H + 14.0;
        rows.push(Row {
            kind: Kind::Return(name.to_string()),
            node: format!("return:{name}"),
            title: format!("{name} return"),
            about: effect_summary(setting),
            chain: None,
            fx: Vec::new(),
            card: Rect::from_min_size(pos2(x_bus, y + ROW_GAP), vec2(BUS_W, h)),
            color,
        });
        y += h + ROW_GAP * 2.0;
    }
    let bus_bottom = y;

    // the master, level with the buses and returns, and the finished song beside it
    let master_fx = chain_of(song, &Chain::Master);
    let master_h = TITLE_H + 18.0 + lines_of(ui, &master_fx, MASTER_W - 24.0) as f32 * (CHIP_H + CHIP_GAP) + 8.0;
    let mid = (area.top() + MARGIN + bus_bottom) / 2.0;
    let master =
        Rect::from_min_size(pos2(x_master, (mid - master_h / 2.0).max(area.top() + MARGIN)), vec2(MASTER_W, master_h));
    let out = Rect::from_min_size(pos2(x_out, master.center().y - 34.0), vec2(OUT_W, 68.0));
    let bottom = tracks_bottom.max(bus_bottom).max(master.bottom()) + MARGIN;

    // the board scrolls where the window is smaller than it
    let content = vec2(width, bottom - area.top());
    egui::ScrollArea::both().id_salt("flow-board").auto_shrink([false, false]).show(ui, |ui| {
        let (space, _) = ui.allocate_exact_size(content, Sense::hover());
        // the board was laid out from `area`; drawn where the scroll put it
        let shift = space.min - area.min;
        for r in &mut rows {
            r.card = r.card.translate(shift);
        }
        let (master, out) = (master.translate(shift), out.translate(shift));
        wires(ui, song, &rows, master, out);
        for r in &rows {
            row(ui, cx, song, r);
        }
        master_card(ui, cx, song, master, &master_fx);
        out_card(ui, cx, out);
    });
}

/// The columns' left edges from the board's left (the tracks, the buses and returns, the master, the finished song),
/// and the board's width, for `available` of it: what is left over the columns' least goes to the wires between them,
/// and a window narrower than that scrolls.
fn columns(available: f32) -> ([f32; 4], f32) {
    let need = MARGIN * 2.0 + TRACK_W + BUS_W + MASTER_W + OUT_W + GAP * 3.0;
    let width = available.max(need);
    let gap = GAP + (width - need) / 3.0;
    let bus = MARGIN + TRACK_W + gap;
    let master = bus + BUS_W + gap;
    ([MARGIN, bus, master, master + MASTER_W + gap], width)
}

/// the wires, behind the cards: each track into its bus, each send into its return as thick as it is, the buses and
/// the returns into the master, the master out
fn wires(ui: &Ui, song: &SongView, rows: &[Row], master: Rect, out: Rect) {
    let painter = ui.painter();
    let curve = |a: Pos2, b: Pos2, width: f32, color: Color32| {
        let bend = vec2(((b.x - a.x) * 0.5).max(30.0), 0.0);
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [a, a + bend, b - bend, b],
            false,
            Color32::TRANSPARENT,
            Stroke::new(width, color),
        ));
    };
    let card_of = |node: &str| rows.iter().find(|r| r.node == node).map(|r| r.card);
    for r in rows {
        match &r.kind {
            Kind::Track(i) => {
                let t = &song.tracks[*i];
                let from = r.card.right_center();
                if let Some(bus) = card_of(&format!("bus:{}", t.bus)) {
                    curve(from, bus.left_center(), 1.6, theme::alpha(r.color, 0.55));
                }
                for (send, amount) in [("reverb", t.reverb), ("delay", t.delay)] {
                    if amount > 0.01
                        && let Some(ret) = rows.iter().find(|x| x.node == format!("return:{send}"))
                    {
                        let a = from + vec2(0.0, 6.0);
                        curve(a, ret.card.left_center(), 0.6 + 3.4 * amount, theme::alpha(ret.color, 0.45));
                    }
                }
            }
            Kind::Bus(_) | Kind::Return(_) => {
                curve(r.card.right_center(), master.left_center(), 2.0, theme::alpha(TEXT_DIM, 0.6));
            }
        }
    }
    curve(master.right_center(), out.left_center(), 2.6, ACCENT.into());
}

/// a card's frame, chosen or hovered or neither, with its colour down its left side
fn card(ui: &Ui, rect: Rect, color: Color32, chosen: bool, hovered: bool) {
    let stroke = if chosen {
        Stroke::new(2.0, ACCENT)
    } else if hovered {
        Stroke::new(1.2, TEXT_DIM)
    } else {
        Stroke::new(1.0, LINE)
    };
    let painter = ui.painter();
    painter.rect(rect, 7.0, RAISED, stroke, StrokeKind::Inside);
    painter.rect_filled(
        Rect::from_min_size(rect.min + vec2(0.0, 7.0), vec2(3.0, (rect.height() - 14.0).max(4.0))),
        1.5,
        color,
    );
}

/// one row: its card, its title and what it says, its controls (a track's), and its chain's chips
fn row(ui: &mut Ui, cx: &mut Cx, song: &SongView, r: &Row) {
    let back = ui.interact(r.card, ui.scope_id().with(("flow-row", &r.node)), Sense::click());
    let chosen = cx.view.graph.selected.as_deref() == Some(r.node.as_str());
    card(ui, r.card, r.color, chosen, back.hovered());
    if back.clicked() {
        cx.view.graph.selected = Some(r.node.clone());
        cx.view.graph.json = None;
    }
    let kind = match &r.kind {
        Kind::Track(i) => NodeKind::Track(song.tracks[*i].name.clone()),
        Kind::Bus(name) => NodeKind::Bus(name.clone()),
        Kind::Return(name) => NodeKind::Return(name.clone()),
    };
    back.context_menu(|ui| node_menu(ui, cx, song, &kind));

    // the title, and a line about it beside, cut short before the controls
    let controls_w = if matches!(r.kind, Kind::Track(_)) { 174.0 } else { 0.0 };
    let painter = ui.painter().with_clip_rect(Rect::from_min_max(
        r.card.min + vec2(10.0, 0.0),
        pos2(r.card.right() - controls_w - 8.0, r.card.bottom()),
    ));
    let title = painter.layout_no_wrap(r.title.clone(), theme::prop(12.5), TEXT.into());
    let title_w = title.size().x;
    painter.galley(r.card.min + vec2(12.0, 7.0), title, TEXT.into());
    painter.text(
        r.card.min + vec2(12.0 + title_w + 8.0, 9.0),
        Align2::LEFT_TOP,
        &r.about,
        theme::prop(10.5),
        TEXT_DIM.into(),
    );

    if let Kind::Track(i) = r.kind {
        track_controls(ui, cx, song, r, i);
        sections_foot(ui, song, r, i);
    }
    if let Some(c) = &r.chain {
        chips(
            ui,
            cx,
            song,
            c,
            &r.fx,
            Rect::from_min_max(r.card.min + vec2(12.0, TITLE_H), r.card.max - vec2(10.0, 4.0)),
        );
    }
}

/// a track's level and pan to drag, its mute and solo, and its meter at its right edge
fn track_controls(ui: &mut Ui, cx: &mut Cx, song: &SongView, r: &Row, i: usize) {
    let t = &song.tracks[i];
    let name = t.name.clone();
    let right = r.card.right() - 10.0;
    let top = r.card.top() + 5.0;
    let mut sent: Vec<MixEdit> = Vec::new();

    let mut level = t.level_db;
    let lr = ui.put(
        Rect::from_min_size(pos2(right - 170.0, top), vec2(62.0, 18.0)),
        egui::DragValue::new(&mut level).speed(0.1).range(-36.0..=12.0).fixed_decimals(1).suffix(" dB"),
    );
    let lr = lr.on_hover_text("its level: drag, or double-click to type");
    let mut pan = t.pan;
    let pr = ui.put(
        Rect::from_min_size(pos2(right - 104.0, top), vec2(60.0, 18.0)),
        egui::DragValue::new(&mut pan)
            .speed(0.01)
            .range(-1.0..=1.0)
            .custom_formatter(|v, _| words::pan_in_words(v as f32))
            .custom_parser(super::parse_pan),
    );
    let pr = pr.on_hover_text("where it sits between the speakers: drag, or double-click to type");
    let on = |ui: &mut Ui, x: f32, label: &str, on: bool, color: Color32, tip: &str| -> bool {
        let rect = Rect::from_min_size(pos2(x, top), vec2(18.0, 18.0));
        let resp = ui.interact(
            rect,
            ui.scope_id().with(("flow-toggle", label, rect.min.x as i32, rect.min.y as i32)),
            Sense::click(),
        );
        let fill = if on {
            color
        } else if resp.hovered() {
            theme::alpha(TEXT, 0.08)
        } else {
            WELL.into()
        };
        ui.painter().rect_filled(rect, 4.0, fill);
        let ink = if on { Color32::from(theme::BG) } else { Color32::from(TEXT_DIM) };
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::prop(10.5), ink);
        resp.on_hover_text(tip).clicked()
    };
    let muted = on(ui, right - 40.0, "M", t.muted, theme::BAD.into(), "mute it");
    let solo = on(ui, right - 20.0, "S", t.solo, theme::SOLO.into(), "solo it: only the soloed tracks play");

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

    // the meter: its loudness from the latest render, riding the render's envelope while it plays
    let render = cx.state.render.as_ref();
    let playing = cx.player.is_playing();
    let live = render
        .filter(|_| playing)
        .and_then(|rv| super::mixer::envelope_db(rv, cx.player.position()))
        .and_then(|(at, avg)| t.loudness.map(|l| l as f32 + (at - avg)));
    let level = live.or(t.loudness.map(|l| l as f32));
    let m = Rect::from_min_max(
        pos2(r.card.right() - 6.0, r.card.top() + 8.0),
        pos2(r.card.right() - 3.0, r.card.bottom() - 10.0),
    );
    ui.painter().rect_filled(m, 1.5, WELL);
    if let Some(db) = level {
        let k = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
        let lit = Rect::from_min_max(pos2(m.left(), m.bottom() - k * m.height()), m.max);
        let hot = if db > -6.0 { theme::WARN } else { theme::GOOD };
        ui.painter().rect_filled(lit, 1.5, hot);
    }
}

/// the sections along a track's foot, each as long as it is, lit where the track plays
fn sections_foot(ui: &Ui, song: &SongView, r: &Row, i: usize) {
    let name = &song.tracks[i].name;
    let total: u32 = song.sections.iter().map(|s| s.bars).sum::<u32>().max(1);
    let (left, right) = (r.card.left() + 12.0, r.card.right() - 12.0);
    let y = r.card.bottom() - FOOT_H - 4.0;
    let mut x = left;
    for s in &song.sections {
        let w = (right - left) * s.bars as f32 / total as f32;
        let plays = s.layers.iter().any(|l| l == name);
        let color = if plays { theme::alpha(r.color, 0.85) } else { theme::alpha(TEXT_FAINT, 0.25) };
        ui.painter().rect_filled(Rect::from_min_size(pos2(x, y), vec2((w - 1.5).max(1.0), FOOT_H)), 1.0, color);
        x += w;
    }
}

/// a chain's effects as chips, in order, in `room`: a click opens an effect's settings beside it, a drag moves it along
/// the chain, a right-click offers to move or take it out; + adds one at the end
fn chips(ui: &mut Ui, cx: &mut Cx, song: &SongView, c: &Chain, fx: &[Value], room: Rect) {
    let font = theme::prop(10.5);
    // every chip's place first, so that a chip dropped finds the one it lands on
    let galleys: Vec<_> =
        fx.iter().map(|v| ui.painter().layout_no_wrap(effect_summary(v), font.clone(), TEXT.into())).collect();
    let (mut x, mut y) = (room.left(), room.top());
    let mut place = |w: f32| -> Rect {
        if x > room.left() && x + w > room.right() {
            x = room.left();
            y += CHIP_H + CHIP_GAP;
        }
        let rect = Rect::from_min_size(pos2(x, y), vec2(w, CHIP_H));
        x += w + CHIP_GAP;
        rect
    };
    let rects: Vec<Rect> = galleys.iter().map(|g| place((g.size().x + 16.0).clamp(36.0, CHIP_MAX))).collect();
    let plus = place(24.0);
    let mut moved: Option<(usize, usize)> = None;
    for (i, (v, galley)) in fx.iter().zip(galleys).enumerate() {
        let rect = rects[i];
        let resp = ui.interact(rect, ui.scope_id().with(("flow-chip", c.target(), i)), Sense::click_and_drag());
        let fill = if resp.hovered() || resp.dragged() { theme::mix(WELL, ACCENT, 0.18) } else { WELL.into() };
        ui.painter().rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, LINE), StrokeKind::Inside);
        ui.painter().with_clip_rect(rect.shrink2(vec2(7.0, 0.0))).galley(
            pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
            galley,
            TEXT.into(),
        );
        // where a chip being dragged would go
        if resp.dragged()
            && let Some(p) = ui.ctx().pointer_interact_pos()
            && let Some(to) = rects.iter().position(|r| r.contains(p))
            && to != i
        {
            let r = rects[to];
            let x = if to > i { r.right() + CHIP_GAP / 2.0 } else { r.left() - CHIP_GAP / 2.0 };
            ui.painter().line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(2.0, ACCENT));
        }
        if resp.drag_stopped()
            && let Some(p) = ui.ctx().pointer_interact_pos()
            && let Some(to) = rects.iter().position(|r| r.contains(p))
            && to != i
        {
            moved = Some((i, to));
        }
        let (shown, about) = words::effect_in_words(v.get("fx").and_then(Value::as_str).unwrap_or_default());
        let resp =
            resp.on_hover_text(format!("{shown}: {about}. Click to change it, drag to move it, right-click for more"));
        let node = format!("{}#{i}", c.target());
        egui::Popup::from_toggle_button_response(&resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(300.0);
                effect(ui, cx, song, &node, c, i);
            });
        resp.context_menu(|ui| node_menu(ui, cx, song, &NodeKind::Effect(c.clone(), i)));
    }
    // + at the end, for an effect to add
    let resp = ui.interact(plus, ui.scope_id().with(("flow-add", c.target())), Sense::click());
    let fill = if resp.hovered() { theme::mix(WELL, ACCENT, 0.18) } else { Color32::TRANSPARENT };
    let edge = Stroke::new(1.0, theme::alpha(TEXT_DIM, 0.5));
    ui.painter().rect(plus, CornerRadius::same(10), fill, edge, StrokeKind::Inside);
    ui.painter().text(plus.center(), Align2::CENTER_CENTER, "+", theme::prop(12.5), TEXT_DIM.into());
    let resp = resp.on_hover_text("add an effect at the end of this chain");
    egui::Popup::menu(&resp).show(|ui| add_menu(ui, cx, song, c, None));
    if let Some((from, to)) = moved {
        let mut fx = fx.to_vec();
        let e = fx.remove(from);
        fx.insert(to.min(fx.len()), e);
        set_chain(cx, c, fx);
    }
}

/// the master: its chain as chips, one a line, + under them, and its loudness and ceiling to drag
fn master_card(ui: &mut Ui, cx: &mut Cx, song: &SongView, rect: Rect, fx: &[Value]) {
    let back = ui.interact(rect, ui.scope_id().with("flow-master"), Sense::click());
    let chosen = cx.view.graph.selected.as_deref() == Some("master");
    card(ui, rect, ACCENT.into(), chosen, back.hovered());
    if back.clicked() {
        cx.view.graph.selected = Some("master".into());
        cx.view.graph.json = None;
    }
    back.context_menu(|ui| node_menu(ui, cx, song, &NodeKind::Master));
    ui.painter().text(rect.min + vec2(12.0, 7.0), Align2::LEFT_TOP, "master", theme::prop(12.5), TEXT.into());
    let mut lufs = song.mix.lufs;
    let lr = ui.put(
        Rect::from_min_size(pos2(rect.right() - 94.0, rect.top() + 5.0), vec2(84.0, 18.0)),
        egui::DragValue::new(&mut lufs).speed(0.1).range(-24.0..=-5.0).fixed_decimals(1).suffix(" LUFS"),
    );
    let lr = lr.on_hover_text("how loud the finished song is made: drag, or double-click to type");
    if commit(&lr) {
        super::graph::set_loudness(cx, lufs);
    }
    ui.painter().text(
        rect.min + vec2(12.0, TITLE_H),
        Align2::LEFT_TOP,
        format!("ceiling {:.1} dBTP", song.mix.ceiling_db),
        theme::prop(10.0),
        TEXT_FAINT.into(),
    );
    // its chain's chips, under its loudness
    let room = Rect::from_min_max(pos2(rect.left() + 12.0, rect.top() + TITLE_H + 18.0), rect.max - vec2(10.0, 4.0));
    chips(ui, cx, song, &Chain::Master, fx, room);
}

/// the finished song: its loudness and true peak from the latest render, and a meter riding it while it plays
fn out_card(ui: &mut Ui, cx: &mut Cx, rect: Rect) {
    card(ui, rect, ACCENT.into(), false, false);
    ui.painter().text(
        rect.min + vec2(12.0, 7.0),
        Align2::LEFT_TOP,
        "the finished song",
        theme::prop(12.5),
        TEXT.into(),
    );
    let render = cx.state.render.as_ref();
    let said = render
        .map_or_else(|| "not rendered yet".to_string(), |r| format!("{:.1} LUFS · {:.1} dBTP", r.lufs, r.true_peak_db));
    ui.painter().text(rect.min + vec2(12.0, 28.0), Align2::LEFT_TOP, said, theme::prop(10.5), TEXT_DIM.into());
    let live = render
        .filter(|_| cx.player.is_playing())
        .and_then(|rv| super::mixer::envelope_db(rv, cx.player.position()))
        .map(|(at, _)| at);
    let m = Rect::from_min_size(pos2(rect.left() + 12.0, rect.bottom() - 14.0), vec2(rect.width() - 24.0, 4.0));
    ui.painter().rect_filled(m, 2.0, WELL);
    if let Some(db) = live.or(render.map(|r| r.lufs as f32)) {
        let k = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
        ui.painter().rect_filled(Rect::from_min_size(m.min, vec2(k * m.width(), m.height())), 2.0, ACCENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// However wide the window, the columns keep the order of the sound and room for the wires between them, and the
    /// last ends at the board's edge: a wide window gives the wires its room, a narrow one scrolls.
    #[test]
    fn the_columns_fit_the_width_with_room_for_the_wires() {
        for available in [480.0, 1000.0, 1180.0, 1600.0, 2560.0] {
            let ([track, bus, master, out], width) = columns(available);
            assert!(width >= available, "never narrower than the window: {available}");
            assert_eq!(track, MARGIN);
            assert!(track + TRACK_W + GAP <= bus + 0.01, "{available}");
            assert!(bus + BUS_W + GAP <= master + 0.01, "{available}");
            assert!(master + MASTER_W + GAP <= out + 0.01, "{available}");
            assert!((out + OUT_W + MARGIN - width).abs() < 0.01, "the last column ends at the edge: {available}");
        }
    }
}
