//! The Radio window (design 0024): in the app, a station of songs a claudius of its own writes as it plays them, a
//! genre at a time, each mixed into the next as a DJ mixes records (`crate::radio`), beside the person's song, which
//! stays as it is. In a browser the radio is the site's own page, which the window links to.

use eframe::egui::{self, RichText, Ui, vec2};

use super::Cx;
use crate::theme::TEXT;

/// the site's radio, where a browser plays it
#[cfg(target_arch = "wasm32")]
const SITE: &str = "https://claudius.devlato.com/radio/";

#[derive(Default)]
pub struct RadioView {
    pub open: bool,
    /// the station, from the first time the window opens, playing on while it is closed
    #[cfg(not(target_arch = "wasm32"))]
    pub radio: Option<crate::radio::Radio>,
    /// whether the studio's own player played when last looked: the song started there pauses the radio
    #[cfg(not(target_arch = "wasm32"))]
    studio_played: bool,
}

impl RadioView {
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

/// Keeps the station going while the window is closed, and shows the window while it is open.
pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    #[cfg(not(target_arch = "wasm32"))]
    follow(ctx, cx);
    if !cx.view.radio.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Infinite Radio")
        .id(egui::Id::unique("radio"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 90.0))
        .show(ctx, |ui| body(ui, cx));
    if !open {
        cx.view.radio.open = false;
    }
}

/// what the station's engine said and where its stream is; and the studio's song and the radio taking turns: one
/// started pauses the other
#[cfg(not(target_arch = "wasm32"))]
fn follow(ctx: &egui::Context, cx: &mut Cx) {
    let v = &mut cx.view.radio;
    let Some(r) = v.radio.as_mut() else { return };
    r.tick();
    let studio = cx.player.is_playing();
    if studio && !v.studio_played && r.playing() {
        r.toggle();
    }
    v.studio_played = studio;
    if r.tuned() && (r.playing() || r.downloading() || r.coming().is_some()) {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

#[cfg(target_arch = "wasm32")]
fn body(ui: &mut Ui, _cx: &mut Cx) {
    ui.label(
        RichText::new(
            "In a browser Infinite Radio is a page of its own: songs claudius writes as it plays them, one genre after \
             another, each mixed into the next as a DJ mixes records.",
        )
        .color(TEXT),
    );
    ui.add_space(8.0);
    ui.hyperlink_to("Open Infinite Radio", SITE);
}

#[cfg(not(target_arch = "wasm32"))]
fn body(ui: &mut Ui, cx: &mut Cx) {
    use crate::radio::{Radio, said};
    use crate::theme::TEXT_DIM;
    use crate::widgets;
    let ctx = ui.ctx().clone();
    let r = cx.view.radio.radio.get_or_insert_with(|| Radio::new(Some(ctx)));
    if let Some(e) = r.error() {
        ui.label(RichText::new(e).color(crate::theme::BAD));
    }
    widgets::heading(ui, "On air");
    match r.on_air() {
        Some((info, at, secs)) => {
            ui.label(RichText::new(&info.title).size(17.0).strong().color(TEXT));
            ui.label(
                RichText::new(format!("{} · {:.0} BPM · {}", said(&info.genre), info.bpm, info.key)).color(TEXT_DIM),
            );
            ui.add(egui::ProgressBar::new((at / secs.max(1.0)) as f32).desired_height(4.0));
            let time = format!("{} / {}", crate::mock::fmt_time(at), crate::mock::fmt_time(secs));
            ui.label(RichText::new(time).size(11.0).color(TEXT_DIM));
        }
        None => {
            let words = if r.tuned() { "The first song is being written…" } else { "Not tuned in" };
            ui.label(RichText::new(words).color(TEXT_DIM));
        }
    }
    if let Some(next) = r.coming() {
        ui.add_space(6.0);
        widgets::heading(ui, "Up next");
        ui.label(RichText::new(next).color(TEXT_DIM));
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let label = if !r.tuned() {
            "Tune in"
        } else if r.playing() {
            "Pause"
        } else {
            "Play"
        };
        if widgets::primary(ui, label, true).clicked() {
            // the radio playing gives the studio's song a rest
            if !r.playing() {
                cx.player.pause();
            }
            r.toggle();
        }
        let on = r.on_air().is_some();
        if ui.add_enabled(on, egui::Button::new("Skip")).on_hover_text("the next song, in on the beat").clicked() {
            r.skip();
        }
        let mut v = r.volume();
        if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false).text("Volume")).changed() {
            r.set_volume(v);
        }
    });
    ui.horizontal(|ui| {
        let on = r.on_air().is_some();
        let save = ui
            .add_enabled(on && !r.downloading(), egui::Button::new("Save this song"))
            .on_hover_text("an MP3 of the song on air, in the songs folder's radio");
        if save.clicked() {
            r.download();
        }
        let open = ui.add_enabled(on, egui::Button::new("Open it in the studio")).on_hover_text(
            "the studio writes the same song again, every part of it to change; undo brings back the song you had",
        );
        if open.clicked()
            && let Some(c) = r.studio()
        {
            cx.out.push(c);
        }
    });
    if !r.said.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new(&r.said).color(TEXT_DIM));
    }
    ui.add_space(6.0);
    genres(ui, r);
    if !r.played.is_empty() {
        ui.add_space(6.0);
        widgets::heading(ui, "Played");
        for i in &r.played {
            let line = format!("{}: {}, {:.0} BPM, {}", i.title, said(&i.genre), i.bpm, i.key);
            ui.label(RichText::new(line).size(11.5).color(TEXT_DIM));
        }
    }
}

/// the genres the station keeps to, by family, once its engine has said what it plays: a choice holds from the next
/// song it writes
#[cfg(not(target_arch = "wasm32"))]
fn genres(ui: &mut Ui, r: &mut crate::radio::Radio) {
    use crate::radio::said;
    use crate::theme::TEXT_DIM;
    let kept = r.kept();
    let title = match kept.len() {
        0 => "Genres: every one".to_string(),
        1..=3 => format!("Genres: {}", kept.iter().map(|g| said(g)).collect::<Vec<_>>().join(", ")),
        n => format!("Genres: {n} chosen"),
    };
    egui::CollapsingHeader::new(title).id_salt("radio_genres").show(ui, |ui| {
        let all = r.genres();
        if all.is_empty() {
            ui.label(RichText::new("The engine is starting…").color(TEXT_DIM));
            return;
        }
        let mut families: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for g in &all {
            let family = if g.family.is_empty() { "other".to_string() } else { g.family.clone() };
            families.entry(family).or_default().push(g.name.clone());
        }
        let mut chosen = kept.clone();
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for (family, names) in &families {
                ui.label(RichText::new(family.to_uppercase()).size(10.5).color(TEXT_DIM));
                ui.horizontal_wrapped(|ui| {
                    for n in names {
                        let mut on = chosen.contains(n);
                        if ui.checkbox(&mut on, said(n)).changed() {
                            if on {
                                chosen.insert(n.clone());
                            } else {
                                chosen.remove(n);
                            }
                        }
                    }
                });
            }
        });
        if chosen != kept {
            r.keep_to(chosen.into_iter().collect());
        }
    });
}
