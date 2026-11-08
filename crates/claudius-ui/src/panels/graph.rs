//! The graph: the song as its signal flows, on a canvas to pan and zoom. Each track is a node, followed by a node for
//! each effect of its chain, into its bus, through the bus's effects into the master, through the master's into the
//! finished song; the sends run to the reverb and delay returns, and the kick's ducking to the tracks it ducks
//! (`SongView::graph`). A node chosen opens in the inspector beside the canvas, where everything about it changes: a
//! track's level, pan, sends, duck, drive, bus, mute, solo and the sections it plays in, its sound if it is a part; an
//! effect's settings, its place in its chain, or the effect itself, as the song keeps it; a chain's new effects; the
//! master's loudness. Every change is a command, so it undoes, and the graph follows the song.

use claudius_api::{Chain, Command, EFFECT_TEMPLATES, EdgeKind, Graph, NodeKind, SongView};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::Value;

use super::{Cx, empty, layer_color, small};
use crate::cmds::MixEdit;
use crate::theme::{self, ACCENT, LINE, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::commit;

const NODE_W: f32 = 196.0;
const COLUMN: f32 = 252.0;
const ROW: f32 = 100.0;

/// the graph's own state: the node chosen, where the canvas looks (nowhere yet: the whole graph), and the effect
/// being edited as JSON
pub struct GraphView {
    pub selected: Option<String>,
    pub scene: Rect,
    /// the node whose JSON is being edited, and the text so far, and why it was not taken
    json: Option<(String, String, Option<String>)>,
}

impl Default for GraphView {
    fn default() -> Self {
        GraphView { selected: None, scene: Rect::ZERO, json: None }
    }
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.clone() else {
        empty(ui, "No song yet.");
        return;
    };
    let graph = song.graph();
    egui::Panel::right("graph-inspector").resizable(true).default_size(320.0).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| inspector(ui, cx, &song, &graph));
    });
    let bounds = graph
        .nodes
        .iter()
        .map(|n| node_rect(n.column, n.row, n.detail.len()))
        .fold(Rect::NOTHING, |a, b| a.union(b))
        .expand(40.0);
    // the tracks and what follows them, big enough to read; Fit shows all of it
    let fit = ui.horizontal(|ui| {
        ui.label(small("drag to move, scroll or pinch to zoom, right-click a node for its menu"));
        ui.button(small("Fit")).on_hover_text("the whole graph in view").clicked()
    });
    if cx.view.graph.scene == Rect::ZERO {
        cx.view.graph.scene = Rect::from_min_size(bounds.min, ui.available_size() / 0.85);
    }
    if fit.inner {
        cx.view.graph.scene = bounds;
    }
    let mut scene = cx.view.graph.scene;
    let mut chosen = None;
    egui::Scene::new().zoom_range(0.15..=2.0).show(ui, &mut scene, |ui| {
        ui.allocate_rect(bounds, Sense::hover());
        chosen = canvas(ui, cx, &song, &graph);
    });
    cx.view.graph.scene = scene;
    if let Some(id) = chosen {
        if cx.view.graph.selected.as_ref() != Some(&id) {
            cx.view.graph.json = None;
        }
        cx.view.graph.selected = Some(id);
    }
}

/// where a node sits: its column and row on the canvas, as tall as its lines
fn node_rect(column: usize, row: f32, lines: usize) -> Rect {
    Rect::from_min_size(Pos2::new(column as f32 * COLUMN, row * ROW), vec2(NODE_W, 30.0 + 14.0 * lines as f32))
}

/// the edges, then the nodes; a node clicked is chosen, and its menu offers what can be done to it
fn canvas(ui: &mut Ui, cx: &mut Cx, song: &SongView, graph: &Graph) -> Option<String> {
    let rect_of =
        |id: &str| graph.nodes.iter().find(|n| n.id == id).map(|n| node_rect(n.column, n.row, n.detail.len()));
    let painter = ui.painter().clone();
    for e in &graph.edges {
        let (Some(a), Some(b)) = (rect_of(&e.from), rect_of(&e.to)) else { continue };
        let (color, width, label) = match e.kind {
            EdgeKind::Signal => (LINE, 1.6, None),
            EdgeKind::Send(x) => (theme::alpha(ACCENT, 0.55), 1.0, Some(format!("send {:.0}%", x * 100.0))),
            EdgeKind::Sidechain(x) => (theme::alpha(theme::BAD, 0.5), 1.0, Some(format!("duck {:.0}%", x * 100.0))),
        };
        // the kick's ducking runs down the tracks' left sides, the signal left to right
        let (p0, p3, bend) = match e.kind {
            EdgeKind::Sidechain(_) => (a.left_center(), b.left_center(), vec2(-60.0, 0.0)),
            _ => (a.right_center(), b.left_center(), vec2(((b.left() - a.right()) * 0.5).max(40.0), 0.0)),
        };
        let (p1, p2) = match e.kind {
            EdgeKind::Sidechain(_) => (p0 + bend, p3 + bend),
            _ => (p0 + bend, p3 - bend),
        };
        let curve = egui::epaint::CubicBezierShape::from_points_stroke(
            [p0, p1, p2, p3],
            false,
            Color32::TRANSPARENT,
            Stroke::new(width, color),
        );
        let mid = curve.sample(0.5);
        painter.add(curve);
        if let Some(text) = label {
            painter.text(mid, Align2::CENTER_CENTER, text, theme::prop(9.5), color);
        }
    }
    let mut chosen = None;
    for n in &graph.nodes {
        let rect = node_rect(n.column, n.row, n.detail.len());
        let r = ui.interact(rect, ui.scope_id().with(("graph-node", &n.id)), Sense::click());
        let selected = cx.view.graph.selected.as_ref() == Some(&n.id);
        let (fill, mark) = match &n.kind {
            NodeKind::Track(name) => (RAISED, layer_color(song, name)),
            NodeKind::Effect(..) => (WELL, TEXT_FAINT),
            NodeKind::Bus(_) | NodeKind::Return(_) => (theme::mix(RAISED, ACCENT, 0.08), ACCENT),
            NodeKind::Master | NodeKind::Out => (theme::mix(RAISED, ACCENT, 0.2), ACCENT),
        };
        let stroke = if selected {
            Stroke::new(2.0, ACCENT)
        } else if r.hovered() {
            Stroke::new(1.2, TEXT_DIM)
        } else {
            Stroke::new(1.0, LINE)
        };
        painter.rect(rect, 6.0, fill, stroke, StrokeKind::Inside);
        painter.rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(3.0, rect.height() - 12.0)), 1.0, mark);
        let title = egui::text::LayoutJob::simple_singleline(n.title.clone(), theme::prop(12.5), TEXT);
        let galley = ui.fonts_mut(|f| f.layout_job(title));
        painter.with_clip_rect(rect.shrink(4.0)).galley(rect.min + vec2(10.0, 7.0), galley, TEXT);
        for (k, line) in n.detail.iter().enumerate() {
            painter.with_clip_rect(rect.shrink(4.0)).text(
                rect.min + vec2(10.0, 26.0 + 14.0 * k as f32),
                Align2::LEFT_TOP,
                line,
                theme::prop(10.0),
                TEXT_DIM,
            );
        }
        if r.clicked() {
            chosen = Some(n.id.clone());
        }
        r.context_menu(|ui| node_menu(ui, cx, song, &n.kind));
    }
    chosen
}

/// what a node's menu offers: effects to add, an effect to move or take out, a track to mute or solo
fn node_menu(ui: &mut Ui, cx: &mut Cx, song: &SongView, kind: &NodeKind) {
    let chain = match kind {
        NodeKind::Track(name) => Some((Chain::Track(name.clone()), None)),
        NodeKind::Effect(c, i) if !matches!(c, Chain::Send(_)) => Some((c.clone(), Some(*i))),
        NodeKind::Bus(name) => Some((Chain::Bus(name.clone()), None)),
        NodeKind::Master => Some((Chain::Master, None)),
        _ => None,
    };
    if let Some((c, at)) = chain {
        let fx = chain_of(song, &c);
        ui.menu_button("Add an effect", |ui| {
            for (name, template) in EFFECT_TEMPLATES {
                if ui.button(name.replace('_', " ")).clicked() {
                    if let Ok(e) = serde_json::from_str::<Value>(template) {
                        let mut fx = fx.clone();
                        fx.insert(at.map_or(fx.len(), |i| i + 1), e);
                        set_chain(cx, &c, fx);
                    }
                    ui.close();
                }
            }
        });
        if let Some(i) = at {
            if ui.add_enabled(i > 0, egui::Button::new("Move earlier")).clicked() {
                let mut fx = fx.clone();
                fx.swap(i, i - 1);
                set_chain(cx, &c, fx);
                ui.close();
            }
            if ui.add_enabled(i + 1 < fx.len(), egui::Button::new("Move later")).clicked() {
                let mut fx = fx.clone();
                fx.swap(i, i + 1);
                set_chain(cx, &c, fx);
                ui.close();
            }
            if ui.button("Take it out").clicked() {
                let mut fx = fx.clone();
                fx.remove(i);
                set_chain(cx, &c, fx);
                ui.close();
            }
        }
    }
    if let NodeKind::Track(name) = kind
        && let Some(t) = song.tracks.iter().find(|t| t.name == *name)
    {
        ui.separator();
        if ui.button(if t.muted { "Unmute" } else { "Mute" }).clicked() {
            cx.out.push(MixEdit { muted: Some(!t.muted), ..Default::default() }.on(name));
            ui.close();
        }
        if ui.button(if t.solo { "Unsolo" } else { "Solo" }).clicked() {
            cx.out.push(MixEdit { solo: Some(!t.solo), ..Default::default() }.on(name));
            ui.close();
        }
    }
}

/// a chain of effects as the song has it now
fn chain_of(song: &SongView, c: &Chain) -> Vec<Value> {
    match c {
        Chain::Track(n) => song.tracks.iter().find(|t| t.name == *n).map(|t| t.fx.clone()).unwrap_or_default(),
        Chain::Bus(n) => song.mix.buses.iter().find(|(b, _)| b == n).map(|(_, fx)| fx.clone()).unwrap_or_default(),
        Chain::Master => song.mix.master.clone(),
        Chain::Send(w) => vec![if w == "reverb" { song.mix.reverb.clone() } else { song.mix.delay.clone() }],
    }
}

/// the chain changed in the song shown at once, and sent to claudius
fn set_chain(cx: &mut Cx, c: &Chain, fx: Vec<Value>) {
    if let Some(song) = cx.state.song.as_mut() {
        put_chain(song, c, fx.clone());
    }
    cx.out.push(Command::SetFx { target: c.target(), fx });
}

fn put_chain(song: &mut SongView, c: &Chain, fx: Vec<Value>) {
    match c {
        Chain::Track(n) => {
            if let Some(t) = song.tracks.iter_mut().find(|t| t.name == *n) {
                t.fx = fx;
            }
        }
        Chain::Bus(n) => match song.mix.buses.iter_mut().find(|(b, _)| b == n) {
            Some((_, chain)) => *chain = fx,
            None => song.mix.buses.push((n.clone(), fx)),
        },
        Chain::Master => song.mix.master = fx,
        Chain::Send(w) => {
            if let Some(e) = fx.into_iter().next() {
                if w == "reverb" {
                    song.mix.reverb = e;
                } else {
                    song.mix.delay = e;
                }
            }
        }
    }
}

/// what the node chosen is, and everything about it to change
fn inspector(ui: &mut Ui, cx: &mut Cx, song: &SongView, graph: &Graph) {
    let Some(node) = cx.view.graph.selected.clone().and_then(|id| graph.nodes.iter().find(|n| n.id == id).cloned())
    else {
        ui.add_space(8.0);
        ui.label(RichText::new("The song as its signal flows").strong().color(TEXT));
        ui.label(small(
            "Each track runs through its effects into its bus, the buses and the reverb and delay returns into the \
             master, and the master out. Drag to move around, scroll or pinch to zoom, click a node to change it, \
             right-click it to add, move or take out effects.",
        ));
        return;
    };
    ui.add_space(6.0);
    ui.label(RichText::new(&node.title).strong().size(14.0).color(TEXT));
    for line in &node.detail {
        ui.label(small(line));
    }
    ui.add_space(6.0);
    match &node.kind {
        NodeKind::Track(name) => track(ui, cx, song, name),
        NodeKind::Effect(c, i) => effect(ui, cx, song, &node.id, c, *i),
        NodeKind::Bus(name) => chain_list(ui, cx, song, &Chain::Bus(name.clone())),
        NodeKind::Return(which) => effect(ui, cx, song, &node.id, &Chain::Send(which.clone()), 0),
        NodeKind::Master => master(ui, cx, song),
        NodeKind::Out => {
            ui.label(small("The finished song, after the master's chain and its loudness stage."));
        }
    }
}

/// a track's mix, its sections, its sound, and its chain
fn track(ui: &mut Ui, cx: &mut Cx, song: &SongView, name: &str) {
    let Some(i) = cx.state.song.as_ref().and_then(|s| s.tracks.iter().position(|t| t.name == name)) else { return };
    let buses: Vec<String> = song
        .graph()
        .nodes
        .iter()
        .filter_map(|n| match &n.kind {
            NodeKind::Bus(b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    let mut sent = Vec::new();
    if let Some(t) = cx.state.song.as_mut().map(|s| &mut s.tracks[i]) {
        egui::Grid::new("graph-track").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            let mut field = |ui: &mut Ui,
                             label: &str,
                             v: &mut f32,
                             range: std::ops::RangeInclusive<f32>,
                             edit: fn(f32) -> MixEdit| {
                ui.label(small(label));
                let r = ui.add(egui::Slider::new(v, range));
                if commit(&r) {
                    sent.push(edit(*v));
                }
                ui.end_row();
            };
            field(ui, "level dB", &mut t.level_db, -36.0..=12.0, |v| MixEdit {
                level_db: Some(v),
                ..Default::default()
            });
            field(ui, "pan", &mut t.pan, -1.0..=1.0, |v| MixEdit { pan: Some(v), ..Default::default() });
            field(ui, "reverb send", &mut t.reverb, 0.0..=1.0, |v| MixEdit { reverb: Some(v), ..Default::default() });
            field(ui, "delay send", &mut t.delay, 0.0..=1.0, |v| MixEdit { delay: Some(v), ..Default::default() });
            field(ui, "duck", &mut t.duck, 0.0..=1.0, |v| MixEdit { duck: Some(v), ..Default::default() });
            field(ui, "drive dB", &mut t.drive_db, 0.0..=36.0, |v| MixEdit { drive_db: Some(v), ..Default::default() });
            crate::widgets::label_help(
                ui,
                "bus",
                "The group the track plays into before the master: drums, bass, music or fx, or one of your own. \
                 A bus's effects (a compressor gluing the drums together) work on everything in it.",
                "language/mixing.html#buses",
            );
            let mut bus = t.bus.clone();
            egui::ComboBox::from_id_salt("graph-bus").selected_text(&bus).show_ui(ui, |ui| {
                for b in &buses {
                    ui.selectable_value(&mut bus, b.clone(), b);
                }
            });
            if bus != t.bus {
                t.bus = bus.clone();
                sent.push(MixEdit { bus: Some(bus), ..Default::default() });
            }
            ui.end_row();
            ui.label(small("mute, solo"));
            ui.horizontal(|ui| {
                if ui.checkbox(&mut t.muted, "").changed() {
                    sent.push(MixEdit { muted: Some(t.muted), ..Default::default() });
                }
                if ui.checkbox(&mut t.solo, "").changed() {
                    sent.push(MixEdit { solo: Some(t.solo), ..Default::default() });
                }
            });
            ui.end_row();
        });
    }
    cx.out.extend(sent.into_iter().map(|e| e.on(name)));
    ui.add_space(6.0);
    ui.label(small("plays in"));
    ui.horizontal_wrapped(|ui| {
        for (k, s) in song.sections.iter().enumerate() {
            let mut on = s.layers.iter().any(|l| l == name);
            if ui.checkbox(&mut on, small(&s.name)).changed() {
                cx.out.push(Command::ToggleLayer { section: k, layer: name.to_string() });
            }
        }
    });
    if let Some(p) = song.parts.iter().find(|p| p.name == name) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(small("sound"));
            let mut preset = p.preset.clone();
            egui::ComboBox::from_id_salt("graph-preset").selected_text(&preset).show_ui(ui, |ui| {
                for x in &p.presets {
                    ui.selectable_value(&mut preset, x.clone(), x);
                }
            });
            if preset != p.preset {
                cx.out.push(Command::SetPart {
                    name: name.to_string(),
                    style: None,
                    preset: Some(preset),
                    register: None,
                    density: None,
                    reseed: false,
                    late_ms: None,
                });
            }
        });
    }
    ui.add_space(8.0);
    chain_list(ui, cx, song, &Chain::Track(name.to_string()));
}

/// a chain's effects in order, each to move or take out, and an effect to add at its end
fn chain_list(ui: &mut Ui, cx: &mut Cx, song: &SongView, c: &Chain) {
    let fx = chain_of(song, c);
    ui.label(RichText::new("effects").strong().color(TEXT));
    if fx.is_empty() {
        ui.label(small("none"));
    }
    let mut change: Option<Vec<Value>> = None;
    for (i, e) in fx.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.selectable_label(false, claudius_api::effect_summary(e)).clicked() {
                cx.view.graph.selected = Some(format!("{}#{i}", c.target()));
                cx.view.graph.json = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("x").on_hover_text("take it out").clicked() {
                    let mut f = fx.clone();
                    f.remove(i);
                    change = Some(f);
                }
                if ui.add_enabled(i + 1 < fx.len(), egui::Button::new("v").small()).on_hover_text("later").clicked() {
                    let mut f = fx.clone();
                    f.swap(i, i + 1);
                    change = Some(f);
                }
                if ui.add_enabled(i > 0, egui::Button::new("^").small()).on_hover_text("earlier").clicked() {
                    let mut f = fx.clone();
                    f.swap(i, i - 1);
                    change = Some(f);
                }
            });
        });
    }
    egui::ComboBox::from_id_salt(("graph-add", c.target())).selected_text("add an effect").show_ui(ui, |ui| {
        for (name, template) in EFFECT_TEMPLATES {
            if ui.selectable_label(false, name.replace('_', " ")).clicked()
                && let Ok(e) = serde_json::from_str::<Value>(template)
            {
                let mut f = fx.clone();
                f.push(e);
                change = Some(f);
            }
        }
    });
    if let Some(f) = change {
        set_chain(cx, c, f);
    }
}

/// the master's loudness target and ceiling, and its chain
fn master(ui: &mut Ui, cx: &mut Cx, song: &SongView) {
    let (mut lufs, mut ceiling) = (song.mix.lufs, song.mix.ceiling_db);
    let mut send = false;
    egui::Grid::new("graph-master").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        crate::widgets::label_help(
            ui,
            "loudness LUFS",
            "How loud the finished song is, as streaming services measure it: about -14 for Spotify and Apple \
             Music, -9 to -6 for loud club music. Louder squeezes the dynamics.",
            "language/mixing.html#master",
        );
        let r = ui.add(egui::Slider::new(&mut lufs, -24.0..=-5.0));
        send |= commit(&r);
        ui.end_row();
        crate::widgets::label_help(
            ui,
            "ceiling dBTP",
            "The highest the song's peaks may reach, in decibels below full scale (true peak): -1 keeps it from \
             distorting once it is turned into an MP3 or a stream.",
            "language/mixing.html#master",
        );
        let r = ui.add(egui::Slider::new(&mut ceiling, -3.0..=-0.1));
        send |= commit(&r);
        ui.end_row();
    });
    if let Some(s) = cx.state.song.as_mut() {
        (s.mix.lufs, s.mix.ceiling_db) = (lufs, ceiling);
    }
    if send {
        cx.out.push(Command::RunScript { code: format!("master(#{{ lufs: {lufs}, ceiling: {ceiling} }});") });
    }
    ui.add_space(8.0);
    chain_list(ui, cx, song, &Chain::Master);
}

/// the settings an effect has, by kind: numbers to drag, switches, modes to choose; what else it has (bands, chains,
/// settings that move) as JSON, the way the song keeps it
fn effect(ui: &mut Ui, cx: &mut Cx, song: &SongView, id: &str, c: &Chain, i: usize) {
    let fx = chain_of(song, c);
    let Some(mut e) = fx.get(i).cloned() else { return };
    let name = e.get("fx").and_then(|f| f.as_str()).unwrap_or_default().to_string();
    let mut committed = false;
    let mut changed = false;
    if let Some(obj) = e.as_object_mut() {
        let keys: Vec<String> = obj.keys().filter(|k| *k != "fx").cloned().collect();
        egui::Grid::new(("graph-fx", id)).num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            for k in keys {
                ui.label(small(k.replace('_', " ")));
                let v = obj.get_mut(&k).expect("a key of its own");
                match v {
                    Value::Number(n) => {
                        let mut x = n.as_f64().unwrap_or_default();
                        let speed = (x.abs() * 0.01).max(0.001);
                        let r = ui.add(egui::DragValue::new(&mut x).speed(speed).max_decimals(3));
                        if r.changed() {
                            *v = serde_json::json!(x);
                            changed = true;
                        }
                        committed |= commit(&r);
                    }
                    Value::Bool(b) => {
                        committed |= ui.checkbox(b, "").changed();
                    }
                    Value::String(s) => {
                        let options: &[&str] = match (name.as_str(), k.as_str()) {
                            ("filter", "mode") => &["lp", "hp", "bp", "notch", "ladder"],
                            ("saturate", "kind") => &["tanh", "tube", "tape", "hard", "fold"],
                            ("tune", "scale") => &["chords", "key", "chromatic"],
                            _ => &[],
                        };
                        let before = s.clone();
                        egui::ComboBox::from_id_salt(("graph-opt", id, &k)).selected_text(s.as_str()).show_ui(
                            ui,
                            |ui| {
                                for o in options {
                                    ui.selectable_value(s, o.to_string(), *o);
                                }
                            },
                        );
                        committed |= *s != before;
                    }
                    other => {
                        let what = if other.get("sections").is_some() {
                            "by section: in the JSON below"
                        } else if other.is_array() && name != "eq" && name != "gate" && name != "parallel" {
                            "moving: in the JSON below"
                        } else {
                            "in the JSON below"
                        };
                        ui.label(small(what).color(TEXT_FAINT));
                    }
                }
                ui.end_row();
            }
        });
    }
    if changed || committed {
        let mut f = fx.clone();
        f[i] = e.clone();
        if let Some(s) = cx.state.song.as_mut() {
            put_chain(s, c, f.clone());
        }
        if committed {
            cx.out.push(Command::SetFx { target: c.target(), fx: f });
        }
    }
    ui.add_space(8.0);
    egui::CollapsingHeader::new(small("as the song keeps it (JSON)")).id_salt(("graph-json", id)).show(ui, |ui| {
        let g = &mut cx.view.graph;
        if g.json.as_ref().is_none_or(|(at, _, _)| at != id) {
            g.json = Some((id.to_string(), serde_json::to_string_pretty(&e).unwrap_or_default(), None));
        }
        let mut apply = None;
        if let Some((_, text, why)) = g.json.as_mut() {
            ui.add(egui::TextEdit::multiline(text).code_editor().desired_rows(8).desired_width(f32::INFINITY));
            if ui.button("Apply").clicked() {
                match serde_json::from_str::<Value>(text) {
                    Ok(v) if v.get("fx").is_some_and(|f| f.is_string()) => {
                        *why = None;
                        apply = Some(v);
                    }
                    Ok(_) => *why = Some("an effect is a map with its name in \"fx\"".into()),
                    Err(err) => *why = Some(err.to_string()),
                }
            }
            if let Some(why) = why {
                ui.label(small(why.clone()).color(theme::BAD));
            }
        }
        if let Some(v) = apply {
            let mut f = fx.clone();
            f[i] = v;
            set_chain(cx, c, f);
            cx.view.graph.json = None;
        }
    });
    if !matches!(c, Chain::Send(_)) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let mut f = fx.clone();
            let mut moved = None;
            if ui.add_enabled(i > 0, egui::Button::new("Move earlier")).clicked() {
                f.swap(i, i - 1);
                moved = Some(i - 1);
            }
            if ui.add_enabled(i + 1 < fx.len(), egui::Button::new("Move later")).clicked() {
                f.swap(i, i + 1);
                moved = Some(i + 1);
            }
            if ui.button("Take it out").clicked() {
                f.remove(i);
                cx.view.graph.selected = None;
                set_chain(cx, c, f);
            } else if let Some(to) = moved {
                cx.view.graph.selected = Some(format!("{}#{to}", c.target()));
                set_chain(cx, c, f);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_chain_changed_on_the_graph_is_the_songs_at_once() {
        let mut song = SongView {
            tracks: vec![claudius_api::TrackView { name: "lead".into(), bus: "music".into(), ..Default::default() }],
            ..Default::default()
        };
        let reverb = json!({"fx": "reverb", "decay": 3});
        put_chain(&mut song, &Chain::Track("lead".into()), vec![reverb.clone()]);
        put_chain(&mut song, &Chain::Bus("glue".into()), vec![json!({"fx": "gain", "db": -2})]);
        put_chain(&mut song, &Chain::Send("delay".into()), vec![json!({"fx": "delay", "beats": 1.5})]);
        assert_eq!(chain_of(&song, &Chain::Track("lead".into())), vec![reverb]);
        assert_eq!(chain_of(&song, &Chain::Bus("glue".into())).len(), 1);
        assert_eq!(song.mix.delay["beats"], 1.5);
        assert!(EFFECT_TEMPLATES.iter().all(|(n, t)| serde_json::from_str::<Value>(t).unwrap()["fx"] == *n));
    }

    #[test]
    fn nodes_sit_in_their_columns_and_rows() {
        let a = node_rect(0, 0.0, 2);
        let b = node_rect(1, 1.0, 0);
        assert!(a.right() < b.left() && a.bottom() < b.top() + ROW);
        assert_eq!(a.height(), 58.0);
    }
}
