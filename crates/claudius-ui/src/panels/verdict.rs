//! The critic's verdict on the latest render: the total, one bar per component, the issues (each says where to fix
//! it), what the ML ears heard (genre vs its closest confusion, production vs its worst flaw, what it sounds like),
//! the tonal balance and the other measures.

use claudius_api::VerdictView;
use eframe::egui::{self, Align2, Color32, Rect, RichText, Sense, Shape, Stroke, Ui, pos2, vec2};

use super::{Cx, small};
use crate::theme::{self, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets;

fn ring(ui: &mut Ui, total: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(84.0, 84.0), Sense::hover());
    let c = rect.center();
    let r = 36.0;
    let color = theme::score_color(total / 100.0);
    let pts = |from: f32, to: f32| -> Vec<egui::Pos2> {
        let n = 64;
        (0..=n).map(|i| from + (to - from) * i as f32 / n as f32).map(|a| c + r * vec2(a.sin(), -a.cos())).collect()
    };
    let tau = std::f32::consts::TAU;
    ui.painter().add(Shape::line(pts(0.0, tau), Stroke::new(7.0, WELL)));
    ui.painter().add(Shape::line(pts(0.0, tau * (total / 100.0).clamp(0.0, 1.0) as f32), Stroke::new(7.0, color)));
    ui.painter().text(c - vec2(0.0, 4.0), Align2::CENTER_CENTER, format!("{total:.0}"), theme::prop(26.0), color);
    ui.painter().text(c + vec2(0.0, 17.0), Align2::CENTER_CENTER, "/ 100", theme::prop(10.0), TEXT_FAINT);
}

/// two z-scores on one axis: what it is vs what it might be mistaken for
fn versus(ui: &mut Ui, a_label: &str, a: f64, b_label: &str, b: f64, good_is_a: bool) {
    let width = ui.available_width().min(300.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 34.0), Sense::hover());
    let p = ui.painter();
    let (lo, hi) = (-2.0f64, 4.0f64);
    let x = |z: f64| rect.left() + ((z - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 * rect.width();
    let axis_y = rect.top() + 12.0;
    p.line_segment([pos2(rect.left(), axis_y), pos2(rect.right(), axis_y)], Stroke::new(2.0, WELL));
    p.line_segment([pos2(x(0.0), axis_y - 4.0), pos2(x(0.0), axis_y + 4.0)], Stroke::new(1.0, TEXT_FAINT));
    let ca = if good_is_a { theme::GOOD } else { theme::BAD };
    let cb = if good_is_a { theme::WARN } else { theme::GOOD };
    for (z, c) in [(a, ca), (b, cb)] {
        p.circle_filled(pos2(x(z), axis_y), 5.0, c);
    }
    p.text(pos2(x(a), axis_y + 13.0), Align2::CENTER_CENTER, format!("{a_label} {a:+.1}"), theme::prop(10.0), ca);
    p.text(pos2(x(b), axis_y - 11.0), Align2::CENTER_CENTER, format!("{b_label} {b:+.1}"), theme::prop(10.0), cb);
}

fn measure(v: &VerdictView, key: &str) -> Option<f64> {
    v.measures.iter().find(|(k, _)| k == key).map(|(_, x)| *x)
}

fn tonal_balance(ui: &mut Ui, v: &VerdictView) {
    let bands = [
        ("sub", "band_sub_db"),
        ("low", "band_low_db"),
        ("low-mid", "band_lowmid_db"),
        ("mid", "band_mid_db"),
        ("presence", "band_presence_db"),
        ("air", "band_air_db"),
    ];
    let values: Vec<(&str, f64)> = bands.iter().filter_map(|(n, k)| measure(v, k).map(|x| (*n, x))).collect();
    if values.is_empty() {
        return;
    }
    ui.label(small("tonal balance (dB)"));
    let width = ui.available_width().min(300.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 70.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, WELL);
    let (lo, hi) = (-30.0, 0.0);
    let bw = rect.width() / values.len() as f32;
    for (i, (name, db)) in values.iter().enumerate() {
        let t = ((db - lo) / (hi - lo)).clamp(0.02, 1.0) as f32;
        let x0 = rect.left() + i as f32 * bw + 6.0;
        let bar = Rect::from_min_max(
            pos2(x0, rect.bottom() - 16.0 - t * (rect.height() - 26.0)),
            pos2(x0 + bw - 12.0, rect.bottom() - 16.0),
        );
        let c = theme::mix(
            Color32::from_rgb(90, 150, 255),
            Color32::from_rgb(255, 170, 90),
            i as f32 / (values.len() - 1).max(1) as f32,
        );
        p.rect_filled(bar, 2.0, theme::alpha(c, 0.8));
        p.text(
            pos2(bar.center().x, bar.top() - 6.0),
            Align2::CENTER_CENTER,
            format!("{db:.0}"),
            theme::prop(9.0),
            TEXT_DIM,
        );
        p.text(pos2(bar.center().x, rect.bottom() - 7.0), Align2::CENTER_CENTER, *name, theme::prop(9.0), TEXT_FAINT);
    }
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let stale = cx.render_stale();
    widgets::heading(ui, "Verdict");
    let Some(v) = cx.state.verdict.as_ref() else {
        ui.label(
            RichText::new(if cx.state.busy.is_some() {
                "listening..."
            } else {
                "Render the song to hear what the critic thinks."
            })
            .color(TEXT_FAINT),
        );
        return;
    };
    if stale {
        ui.label(RichText::new("for an older version of the song").size(11.0).color(theme::WARN));
    }
    ui.horizontal(|ui| {
        ring(ui, v.total);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (name, score) in &v.scores {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(64.0, 14.0), egui::Label::new(RichText::new(name).size(11.0).color(TEXT_DIM)));
                    let w = (ui.available_width() - 40.0).max(60.0);
                    widgets::bar(ui, w, *score, theme::score_color(*score));
                    ui.label(RichText::new(format!("{score:.2}")).monospace().size(10.5).color(TEXT));
                });
            }
        });
    });
    let summary = v.summary.split_once(". ").map_or(v.summary.as_str(), |(_, rest)| rest);
    if !summary.is_empty() && summary != v.summary {
        ui.label(RichText::new(summary).size(11.5).color(TEXT_DIM));
    }
    ui.add_space(6.0);

    ui.label(RichText::new(format!("Issues ({})", v.issues.len())).strong().size(12.5).color(TEXT));
    if v.issues.is_empty() {
        ui.label(RichText::new("nothing measurable to fix").size(11.0).color(theme::GOOD));
    }
    for (severity, what, target) in &v.issues {
        egui::Frame::new()
            .fill(theme::alpha(theme::score_color(1.0 - severity), 0.07))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(6, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let c = theme::score_color(1.0 - severity);
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, c);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.add(egui::Label::new(RichText::new(what).size(11.5).color(TEXT)).wrap());
                        ui.label(
                            RichText::new(format!("severity {severity:.2} · fix at {target}"))
                                .size(10.0)
                                .color(TEXT_FAINT),
                        );
                    });
                });
            });
    }
    ui.add_space(6.0);

    if let Some(e) = &v.ears {
        ui.label(RichText::new("Ears").strong().size(12.5).color(TEXT));
        ui.label(small(format!("genre: \"{}\" vs its closest confusion \"{}\"", e.genre_query, e.best_alt)));
        versus(ui, "genre", e.genre_z, "alt", e.best_alt_z, true);
        ui.label(small(format!("production: \"professionally mixed\" vs worst flaw \"{}\"", e.worst_flaw)));
        versus(ui, "pro", e.production_z, "flaw", e.worst_flaw_z, true);
        if !e.sounds_like.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(small("sounds like"));
                for s in &e.sounds_like {
                    widgets::pill(ui, s, theme::mix(theme::LCD_TEXT, TEXT, 0.3));
                }
            });
        }
        if let Some(p) = e.p_real {
            ui.label(small(format!("realness p = {p:.2} (held out, never optimised)")));
        }
        ui.add_space(6.0);
    }

    ui.label(RichText::new("Measures").strong().size(12.5).color(TEXT));
    tonal_balance(ui, v);
    let key_measures = [
        "lufs",
        "true_peak_db",
        "lra_lu",
        "stereo_correlation",
        "low_end_correlation",
        "drop_minus_break_lu",
        "arc_correlation",
    ];
    egui::Grid::new("measures").num_columns(2).spacing(vec2(10.0, 2.0)).striped(false).show(ui, |ui| {
        let mut shown = Vec::new();
        for k in key_measures.iter().copied().chain(v.measures.iter().map(|(k, _)| k.as_str())) {
            if shown.contains(&k) || k.starts_with("band_") {
                continue;
            }
            let Some(x) = measure(v, k) else { continue };
            shown.push(k);
            ui.label(RichText::new(k.replace('_', " ")).size(10.5).color(TEXT_DIM));
            ui.label(RichText::new(format!("{x:.2}")).monospace().size(10.5).color(TEXT));
            ui.end_row();
        }
    });
}
