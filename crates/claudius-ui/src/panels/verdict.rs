//! The critic's verdict on the latest render: the total, one bar per component, the issues (each says how much it
//! costs and where to fix it), and folded under them what the ML ears heard (genre vs its closest confusion,
//! production vs its worst flaw, what it sounds like), the tonal balance and the other measures.

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

/// a score's name as a person says it, and what it judges
fn score_name(key: &str) -> (&str, &str) {
    match key {
        "arc" => ("Shape", "how its intensity rises and falls across the sections, as its genre's songs do"),
        "mix" => ("Mix", "loudness, peaks, the balance of lows and highs, the stereo width and the low end"),
        "theory" => ("Harmony", "the chords, the key, and how the melody and the bass sit on them"),
        "genre" => ("Genre", "how much it sounds like its genre to the listening model"),
        "production" => ("Production", "how well mixed and produced it sounds to the listening model"),
        "refs" => ("Records", "how close it sounds to records of its genre"),
        "time" => ("Timing", "whether every track plays in time with the song's tempo"),
        other => (other, "a part of the score"),
    }
}

/// how much an issue costs, in words
fn severity(s: f64) -> &'static str {
    if s >= 0.6 {
        "major"
    } else if s >= 0.3 {
        "worth fixing"
    } else {
        "minor"
    }
}

/// where an issue is fixed, in words, from the critic's target ("part:lead", "band:presence-", "width+", "form")
fn where_to_fix(target: &str) -> String {
    let (kind, what) = target.split_once(':').unwrap_or((target, ""));
    let band = |b: &str| match b {
        "lowmid" => "low mids".to_string(),
        "mid" => "mids".to_string(),
        "low" => "lows".to_string(),
        other => other.to_string(),
    };
    match kind {
        "part" => format!("the {what} part"),
        "time" => format!("the timing of {what}"),
        "level" if what.ends_with('+') => format!("{} quieter", what.trim_end_matches('+')),
        "level" if what.ends_with('-') => format!("{} louder", what.trim_end_matches('-')),
        "level" => format!("{what}'s level"),
        "sound" => format!("another sound for {what}"),
        "band" if what.ends_with('+') => format!("less {} in the mix", band(what.trim_end_matches('+'))),
        "band" if what.ends_with('-') => format!("more {} in the mix", band(what.trim_end_matches('-'))),
        "band" => format!("the {} in the mix", band(what)),
        "width+" => "a wider stereo image".into(),
        "width-" => "a narrower stereo image, and the low end in mono".into(),
        "master" => "the master: its loudness and peaks".into(),
        "form" => "the arrangement".into(),
        "genre" => "the genre's sounds and parts".into(),
        "production" => "the production: the sounds and the mix".into(),
        _ => target.into(),
    }
}

/// a meter's name in words, with its unit
fn measure_name(key: &str) -> String {
    match key {
        "lufs" => "loudness (LUFS)".into(),
        "true_peak_db" => "true peak (dBTP)".into(),
        "lra_lu" => "loudness range (LU)".into(),
        "stereo_correlation" => "stereo correlation (1 is mono)".into(),
        "low_end_correlation" => "low end correlation (1 is mono)".into(),
        "drop_minus_break_lu" => "drop over the break (LU)".into(),
        "arc_correlation" => "loudness following the energy".into(),
        other => other.replace('_', " "),
    }
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
        ui.label(RichText::new("of the song before its latest changes").size(11.5).color(theme::WARN));
    }
    ui.horizontal(|ui| {
        ring(ui, v.total);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (key, score) in &v.scores {
                let (name, tip) = score_name(key);
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(76.0, 14.0), egui::Label::new(RichText::new(name).size(11.5).color(TEXT_DIM)))
                        .on_hover_text(tip);
                    let w = (ui.available_width() - 34.0).max(60.0);
                    widgets::bar(ui, w, *score, theme::score_color(*score)).on_hover_text(tip);
                    ui.label(RichText::new(format!("{:.0}", score * 100.0)).size(11.5).color(TEXT));
                });
            }
        });
    });
    let summary = v.summary.split_once(". ").map_or(v.summary.as_str(), |(_, rest)| rest);
    if !summary.is_empty() && summary != v.summary {
        ui.label(RichText::new(summary).size(12.0).color(TEXT_DIM));
    }
    ui.add_space(6.0);

    let heading = match v.issues.len() {
        0 => "Nothing to fix".to_string(),
        1 => "One thing to fix".to_string(),
        n => format!("{n} things to fix"),
    };
    ui.label(RichText::new(heading).strong().size(13.0).color(TEXT));
    if v.issues.is_empty() {
        ui.label(RichText::new("the critic measures nothing wrong").size(11.5).color(theme::GOOD));
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
                        ui.add(egui::Label::new(RichText::new(what).size(12.0).color(TEXT)).wrap());
                        ui.label(
                            RichText::new(format!("{} · fix: {}", self::severity(*severity), where_to_fix(target)))
                                .size(11.0)
                                .color(TEXT_FAINT),
                        )
                        .on_hover_text(format!("severity {severity:.2}, at {target}"));
                    });
                });
            });
    }
    ui.add_space(6.0);

    egui::CollapsingHeader::new(RichText::new("What it measured").strong().size(13.0).color(TEXT))
        .id_salt("verdict_details")
        .default_open(false)
        .show(ui, |ui| details(ui, v));
}

/// what the ears heard and the meters measured, behind the verdict
fn details(ui: &mut Ui, v: &VerdictView) {
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

    ui.label(RichText::new("Meters").strong().size(12.5).color(TEXT));
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
            ui.label(RichText::new(measure_name(k)).size(11.0).color(TEXT_DIM));
            ui.label(RichText::new(format!("{x:.2}")).monospace().size(11.0).color(TEXT));
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_critics_targets_read_as_where_to_fix_it() {
        assert_eq!(where_to_fix("part:lead"), "the lead part");
        assert_eq!(where_to_fix("band:presence-"), "more presence in the mix");
        assert_eq!(where_to_fix("band:lowmid+"), "less low mids in the mix");
        assert_eq!(where_to_fix("width+"), "a wider stereo image");
        assert_eq!(where_to_fix("form"), "the arrangement");
        assert_eq!(where_to_fix("time:vox1"), "the timing of vox1");
        assert_eq!(where_to_fix("level:kick+"), "kick quieter");
        assert_eq!(where_to_fix("level:pad-"), "pad louder");
        assert_eq!(where_to_fix("sound:pad"), "another sound for pad");
        assert_eq!(where_to_fix("something new"), "something new");
        assert_eq!(severity(0.0), "minor");
        assert_eq!(severity(0.45), "worth fixing");
        assert_eq!(severity(0.9), "major");
    }
}
