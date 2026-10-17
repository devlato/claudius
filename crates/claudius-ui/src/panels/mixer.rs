//! The mixer: a channel strip per track (pan, reverb and delay sends, sidechain duck, level fader, mute, solo) with
//! its loudness from the latest render, and the master strip. While anything is soloed only soloed tracks play, so
//! the others are dimmed. While playing, the meters follow the render's envelope for the tracks that play in the
//! section under the playhead.

use claudius_api::{Command, RenderView, TrackView};
use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

use super::{Cx, empty, layer_color, page_frame};
use crate::cmds::MixEdit;
use crate::theme::{self, LINE, LINE_SOFT, RAISED, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::{self, commit};

const STRIP_W: f32 = 86.0;

/// the render's peak envelope at `t` in dB, and its average
fn envelope_db(render: &RenderView, t: f64) -> Option<(f32, f32)> {
    let n = render.waveform.len();
    if n == 0 || render.seconds <= 0.0 {
        return None;
    }
    let i = ((t / render.seconds) * n as f64) as usize;
    let at = render.waveform.get(i.min(n - 1)).copied().unwrap_or(0.0).max(1e-4);
    let avg = render.waveform.iter().map(|v| 20.0 * v.max(1e-4).log10()).sum::<f32>() / n as f32;
    Some((20.0 * at.log10(), avg))
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let playing = cx.player.is_playing();
    let position = cx.player.position();
    let head = cx.playhead_bar();
    let Some(song) = cx.state.song.as_mut() else {
        empty(ui, "No song yet.");
        return;
    };
    if song.tracks.is_empty() {
        empty(ui, "No tracks.");
        return;
    }
    let render = cx.state.render.as_ref();
    let env = render.filter(|_| playing).and_then(|r| envelope_db(r, position));
    let section_layers: Option<Vec<String>> = song
        .sections
        .iter()
        .find(|s| head >= s.start_bar as f64 && head < (s.start_bar + s.bars) as f64)
        .map(|s| s.layers.clone());
    let colors: Vec<Color32> = song.tracks.iter().map(|t| layer_color(song, &t.name)).collect();
    let layers = song.layers.clone();
    let solo = super::any_solo(song);
    let mut out = Vec::new();
    page_frame().show(ui, |ui| {
        let h = ui.available_height();
        egui::ScrollArea::horizontal().id_salt("mixer").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (i, t) in song.tracks.iter_mut().enumerate() {
                    let plays = match (&section_layers, layers.contains(&t.name)) {
                        (Some(l), true) => l.contains(&t.name),
                        _ => true,
                    };
                    let silenced = t.muted || (solo && !t.solo);
                    // while playing: the track's loudness riding the render's envelope, silent where it does not play
                    let live =
                        env.map(|(at, avg)| t.loudness.filter(|_| plays && !silenced).map(|l| l as f32 + (at - avg)));
                    strip(ui, t, colors[i], h, live, solo, &mut out);
                }
                ui.add_space(8.0);
                master(ui, render, h, env.map(|(at, _)| at));
            });
        });
    });
    cx.out.extend(out);
}

/// a small toggle button on the strip (mute, solo)
fn toggle(ui: &mut Ui, on: bool, label: &str, color: Color32, width: f32, tip: &str) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(width, 20.0), Sense::click());
    let fill = if on {
        color
    } else if r.hovered() {
        theme::RAISED_HI
    } else {
        WELL
    };
    ui.painter().rect(rect, 4.0, fill, Stroke::new(1.0, LINE), StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::prop(10.5),
        if on { Color32::BLACK } else { TEXT_DIM },
    );
    r.on_hover_text(tip)
}

fn strip(
    ui: &mut Ui,
    t: &mut TrackView,
    color: Color32,
    height: f32,
    live: Option<Option<f32>>,
    solo: bool,
    out: &mut Vec<Command>,
) {
    let stroke = if t.solo { Stroke::new(1.5, theme::SOLO) } else { Stroke::new(1.0, LINE_SOFT) };
    let frame =
        egui::Frame::new().fill(RAISED).corner_radius(6).inner_margin(egui::Margin::symmetric(4, 6)).stroke(stroke);
    let silenced = solo && !t.solo;
    let resp = frame.show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(STRIP_W - 8.0);
            ui.set_height(height - 24.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            if silenced || t.muted {
                ui.multiply_opacity(0.45);
            }
            let dim = t.muted || silenced;
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(&t.name).strong().size(12.5).color(if dim { TEXT_FAINT } else { TEXT }))
                    .on_hover_text(format!("{} ({}, role {})", t.sound, t.kind, t.role));
                let what = if silenced {
                    "silent: another track is soloed".to_string()
                } else {
                    format!("{} · {}", t.kind, t.role)
                };
                ui.label(RichText::new(what).size(9.5).color(if t.solo { theme::SOLO } else { TEXT_FAINT }));
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let r = widgets::knob(ui, &mut t.pan, -1.0..=1.0, "pan", color, true, |v| {
                    if v.abs() < 0.01 {
                        "C".into()
                    } else if v < 0.0 {
                        format!("L{:.0}", -v * 100.0)
                    } else {
                        format!("R{:.0}", v * 100.0)
                    }
                });
                if commit(&r) {
                    out.push(MixEdit { pan: Some(t.pan), ..Default::default() }.on(&t.name));
                }
                let r = widgets::knob(ui, &mut t.reverb, 0.0..=1.0, "reverb", color, false, |v| {
                    format!("{:.0}%", v * 100.0)
                });
                if commit(&r) {
                    out.push(MixEdit { reverb: Some(t.reverb), ..Default::default() }.on(&t.name));
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let r =
                    widgets::knob(ui, &mut t.delay, 0.0..=1.0, "delay", color, false, |v| format!("{:.0}%", v * 100.0));
                if commit(&r) {
                    out.push(MixEdit { delay: Some(t.delay), ..Default::default() }.on(&t.name));
                }
                let r =
                    widgets::knob(ui, &mut t.duck, 0.0..=1.0, "duck", color, false, |v| format!("{:.0}%", v * 100.0));
                if commit(&r) {
                    out.push(MixEdit { duck: Some(t.duck), ..Default::default() }.on(&t.name));
                }
            });
            let fader_h = (ui.available_height() - 64.0).clamp(60.0, 220.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.add_space(2.0);
                let r = widgets::fader(ui, &mut t.level_db, -36.0..=12.0, fader_h, color, !t.muted);
                if commit(&r) {
                    out.push(MixEdit { level_db: Some(t.level_db), ..Default::default() }.on(&t.name));
                }
                let shown = live.unwrap_or(t.loudness.map(|l| l as f32)).map(|l| l.min(0.0));
                let tip = match t.loudness {
                    Some(l) => format!("{} plays at {l:.1} LUFS in the latest render", t.name),
                    None => "render to measure".into(),
                };
                widgets::meter(ui, vec2(9.0, fader_h), shown, -48.0, t.loudness.map(|l| l as f32)).on_hover_text(tip);
            });
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(format!("{:+.1} dB", t.level_db)).monospace().size(11.0).color(if dim {
                    TEXT_FAINT
                } else {
                    TEXT
                }));
                ui.label(
                    RichText::new(t.loudness.map_or("— LUFS".into(), |l| format!("{l:.1} LUFS")))
                        .size(9.5)
                        .color(TEXT_DIM),
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let w = (STRIP_W - 24.0) / 2.0;
                    if toggle(ui, t.muted, "M", theme::alpha(theme::BAD, 0.85), w, "mute").clicked() {
                        t.muted = !t.muted;
                        out.push(MixEdit { muted: Some(t.muted), ..Default::default() }.on(&t.name));
                    }
                    if toggle(
                        ui,
                        t.solo,
                        "S",
                        theme::SOLO,
                        w,
                        "solo: while any track is soloed, only soloed tracks are arranged, rendered and judged",
                    )
                    .clicked()
                    {
                        t.solo = !t.solo;
                        out.push(MixEdit { solo: Some(t.solo), ..Default::default() }.on(&t.name));
                    }
                });
            });
        })
    });
    let rect = resp.response.rect;
    let top = if t.solo {
        theme::SOLO
    } else if t.muted || silenced {
        TEXT_FAINT
    } else {
        color
    };
    ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(6.0, 1.0), vec2(rect.width() - 12.0, 3.0)), 1.5, top);
}

fn master(ui: &mut Ui, render: Option<&RenderView>, height: f32, live: Option<f32>) {
    let frame = egui::Frame::new()
        .fill(theme::mix(RAISED, theme::ACCENT, 0.06))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .stroke(Stroke::new(1.0, theme::alpha(theme::ACCENT, 0.4)));
    frame.show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(104.0);
            ui.set_height(height - 24.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("MASTER").strong().size(12.5).color(theme::ACCENT));
                ui.label(RichText::new("the mastered render").size(9.5).color(TEXT_FAINT));
            });
            ui.add_space(6.0);
            let meter_h = (ui.available_height() - 70.0).clamp(60.0, 260.0);
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                let level = live.or(render.map(|r| r.lufs as f32));
                widgets::meter(ui, vec2(14.0, meter_h), level, -48.0, render.map(|r| r.true_peak_db))
                    .on_hover_text("peak envelope while playing; the line is the true peak");
                widgets::meter(ui, vec2(14.0, meter_h), level, -48.0, render.map(|r| r.true_peak_db));
            });
            ui.add_space(4.0);
            ui.vertical_centered(|ui| match render {
                Some(r) => {
                    ui.label(RichText::new(format!("{:.1} LUFS", r.lufs)).monospace().size(12.0).color(TEXT));
                    ui.label(
                        RichText::new(format!("{:.1} dBTP", r.true_peak_db))
                            .monospace()
                            .size(10.5)
                            .color(if r.true_peak_db > -1.0 { theme::WARN } else { TEXT_DIM }),
                    );
                    ui.label(
                        RichText::new(format!("{:.0} BPM · {}", r.bpm, super::fmt_time(r.seconds)))
                            .size(9.5)
                            .color(TEXT_FAINT),
                    );
                }
                None => {
                    ui.label(RichText::new("not rendered").size(10.5).color(TEXT_FAINT));
                }
            });
        })
    });
}
