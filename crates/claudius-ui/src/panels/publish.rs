//! "Publish": the release the session prepares for the song, as `claudius publish` does on the command line. The
//! artwork is dropped onto the studio (or, natively, read from a path), each detail is sent when its field is left,
//! and the checks come back with the state: what stores and distributors would say of the latest render.

use claudius_api::{Command, ReleaseDetails, ReleaseView};
use eframe::egui::{self, Color32, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

use super::{Cx, small};
use crate::theme::{self, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets::{self, Icon};

#[derive(Default)]
pub struct Publish {
    pub open: bool,
    /// the details as typed
    form: Form,
    /// the session's details the form last showed: when they change elsewhere (an assistant set them), so does it
    shown: Option<ReleaseDetails>,
    /// the details last sent, which come back as the session's
    sent: Option<ReleaseDetails>,
    /// a picture to send as the artwork, dropped or read from `path`
    pub upload: Option<Vec<u8>>,
    /// the preview on screen, and the artwork version it shows
    preview: Option<(u64, egui::TextureHandle)>,
    /// a preview that arrived, to make a texture of
    pub arrived: Option<(u64, Result<egui::ColorImage, String>)>,
    /// the artwork version whose preview was asked for
    asked: Option<u64>,
    /// a path to read the artwork from: a browser has no files to name
    #[cfg(not(target_arch = "wasm32"))]
    path: String,
    /// why a picture could not be read or shown
    error: Option<String>,
    /// dropped pictures a browser is reading
    reading: Option<ReadChannel>,
    /// the dialog has just opened: the session is told, and checks the release
    opened: bool,
}

type ReadChannel =
    (std::sync::mpsc::Sender<Result<Vec<u8>, String>>, std::sync::mpsc::Receiver<Result<Vec<u8>, String>>);

/// the details as the form edits them: lists are names between commas
#[derive(Clone, Default, PartialEq)]
struct Form {
    artist: String,
    title: String,
    version: String,
    featuring: String,
    album: String,
    genre: String,
    subgenre: String,
    date: String,
    label: String,
    writers: String,
    producers: String,
    owner: String,
    isrc: String,
    upc: String,
    explicit: bool,
    made: Option<String>,
}

fn split(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

impl Form {
    fn of(d: &ReleaseDetails) -> Form {
        Form {
            artist: d.artist.clone(),
            title: d.title.clone(),
            version: d.version.clone(),
            featuring: d.featuring.join(", "),
            album: d.album.clone(),
            genre: d.genre.clone(),
            subgenre: d.subgenre.clone(),
            date: d.date.clone(),
            label: d.label.clone(),
            writers: d.writers.join(", "),
            producers: d.producers.join(", "),
            owner: d.owner.clone(),
            isrc: d.isrc.clone(),
            upc: d.upc.clone(),
            explicit: d.explicit,
            made: d.made.clone(),
        }
    }

    fn details(&self) -> ReleaseDetails {
        let text = |s: &str| s.trim().to_string();
        ReleaseDetails {
            artist: text(&self.artist),
            title: text(&self.title),
            version: text(&self.version),
            featuring: split(&self.featuring),
            album: text(&self.album),
            genre: text(&self.genre),
            subgenre: text(&self.subgenre),
            date: text(&self.date),
            label: text(&self.label),
            writers: split(&self.writers),
            producers: split(&self.producers),
            owner: text(&self.owner),
            isrc: text(&self.isrc),
            upc: text(&self.upc),
            explicit: self.explicit,
            made: self.made.clone(),
        }
    }
}

impl Publish {
    pub fn show(&mut self) {
        self.open = true;
        self.opened = true;
        self.error = None;
    }

    /// the artwork version to fetch a preview of, once each
    pub fn wants_preview(&mut self, release: &ReleaseView) -> Option<u64> {
        let v = release.artwork_version;
        let shown = self.preview.as_ref().map(|(pv, _)| *pv);
        let wanted = self.open && release.artwork.is_some() && shown != Some(v) && self.asked != Some(v);
        wanted.then(|| {
            self.asked = Some(v);
            v
        })
    }
}

/// a picture dropped on the studio becomes the artwork (a browser reads it while the studio goes on)
fn take_drop(ctx: &egui::Context, p: &mut Publish) {
    if let Some((_, rx)) = &p.reading
        && let Ok(read) = rx.try_recv()
    {
        match read {
            Ok(bytes) => p.upload = Some(bytes),
            Err(e) => p.error = Some(e),
        }
    }
    let Some(file) = ctx.input(|i| i.raw.dropped_files.first().cloned()) else { return };
    p.error = None;
    #[cfg(not(target_arch = "wasm32"))]
    match file.bytes() {
        Ok(bytes) => p.upload = Some(bytes),
        Err(e) => p.error = Some(format!("cannot read {}: {e}", file.path().display())),
    }
    #[cfg(target_arch = "wasm32")]
    {
        let tx = p.reading.get_or_insert_with(std::sync::mpsc::channel).0.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let read = file.bytes_async().await.map_err(|e| format!("cannot read {}: {e}", file.path().display()));
            let _ = tx.send(read);
            ctx.request_repaint();
        });
    }
}

/// the dialog, while it is open
pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    if !cx.view.publish.open {
        return;
    }
    let release = cx.state.release.clone();
    let song_version = cx.state.song_version;
    let p = &mut cx.view.publish;
    take_drop(ctx, p);
    if let Some((v, image)) = p.arrived.take() {
        match image {
            Ok(img) => p.preview = Some((v, ctx.load_texture("release-artwork", img, egui::TextureOptions::LINEAR))),
            Err(e) => p.error = Some(format!("cannot show the artwork: {e}")),
        }
    }
    if let Some(r) = &release
        && p.shown.as_ref() != Some(&r.details)
    {
        if p.sent.as_ref() != Some(&r.details) {
            p.form = Form::of(&r.details);
        }
        p.shown = Some(r.details.clone());
    }
    let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
    let mut out = Vec::new();
    // the session checks a release once something is given for it: opening the dialog is asking for the checks
    if std::mem::take(&mut p.opened)
        && let Some(r) = &release
    {
        out.push(Command::SetRelease { details: Box::new(r.details.clone()), artwork: None });
    }
    let modal = egui::Modal::new(egui::Id::unique("publish")).show(ctx, |ui| {
        ui.set_width(780.0);
        ui.heading("Publish");
        ui.add_space(4.0);
        ui.label(small(
            "Spotify, Apple Music and the other stores take music from distributors. claudius checks the song the \
             way they do, and writes the files and the answers a distributor's form asks for.",
        ));
        ui.add_space(10.0);
        let Some(release) = &release else {
            ui.label(RichText::new("Make a song first.").color(TEXT));
            close_row(ui, p);
            return;
        };
        let mut send = false;
        let height = (ctx.content_rect().height() - 220.0).max(240.0);
        egui::ScrollArea::vertical().max_height(height).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                artwork(ui, p, release, hovering);
                ui.add_space(16.0);
                ui.vertical(|ui| send |= details(ui, &mut p.form, release));
            });
            ui.add_space(12.0);
            send |= made(ui, &mut p.form, release);
            ui.add_space(12.0);
            checks(ui, release, song_version);
        });
        if send && p.form.details() != release.details {
            let details = p.form.details();
            p.sent = Some(details.clone());
            out.push(Command::SetRelease { details: Box::new(details), artwork: None });
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let current = release.checked > 0 && release.checked >= song_version;
            let can = current && release.ready;
            let why = if !current {
                "the checks come once the song as it is now is rendered"
            } else {
                "fix the problems first: they are marked red"
            };
            let r = widgets::tool_button(ui, Icon::Save, "Write the release kit", can, can)
                .on_hover_text("the WAVs, the cover and release.txt, into release/ in the song's folder")
                .on_disabled_hover_text(why);
            if r.clicked() {
                out.push(Command::WriteKit { path: None });
            }
            if let Some(k) = &release.kit {
                ui.add(egui::Label::new(small(format!("written to {k}"))).truncate());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::tool_button(ui, Icon::Cancel, "Close", true, false).clicked() {
                    p.open = false;
                }
            });
        });
    });
    if modal.should_close() {
        cx.view.publish.open = false;
    }
    cx.out.extend(out);
}

fn close_row(ui: &mut Ui, p: &mut Publish) {
    ui.add_space(10.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if widgets::tool_button(ui, Icon::Cancel, "Close", true, false).clicked() {
            p.open = false;
        }
    });
}

/// the cover, or where to drop it, with what it is
fn artwork(ui: &mut Ui, p: &mut Publish, release: &ReleaseView, hovering: bool) {
    ui.vertical(|ui| {
        ui.set_width(220.0);
        let (rect, _) = ui.allocate_exact_size(vec2(220.0, 220.0), Sense::hover());
        let edge = if hovering { theme::ACCENT } else { theme::LINE };
        ui.painter().rect(
            rect,
            6.0,
            theme::WELL,
            Stroke::new(if hovering { 2.0 } else { 1.0 }, edge),
            StrokeKind::Inside,
        );
        match &p.preview {
            Some((v, tex)) if *v == release.artwork_version && release.artwork.is_some() => {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                let size = tex.size_vec2();
                let fit = size * (rect.width() / size.x).min(rect.height() / size.y);
                ui.painter().image(tex.id(), egui::Rect::from_center_size(rect.center(), fit), uv, Color32::WHITE);
            }
            _ => {
                let text = if hovering { "drop it here" } else { "drop the cover here" };
                let font = egui::FontId::proportional(14.0);
                ui.painter().text(rect.center() - vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, text, font, TEXT_DIM);
                let note = "a square JPEG or PNG, 3000 × 3000";
                let font = egui::FontId::proportional(11.0);
                ui.painter().text(rect.center() + vec2(0.0, 11.0), egui::Align2::CENTER_CENTER, note, font, TEXT_FAINT);
            }
        }
        ui.add_space(4.0);
        let what = release.artwork.clone().unwrap_or_else(|| "no artwork yet".into());
        ui.add(egui::Label::new(small(what)).wrap());
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal(|ui| {
            let typed = ui.add(egui::TextEdit::singleline(&mut p.path).hint_text("or its path").desired_width(180.0));
            let entered = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let named = !p.path.trim().is_empty();
            if widgets::icon_button(ui, Icon::Open, 24.0, false, named, "use this file").clicked() || (entered && named)
            {
                match std::fs::read(p.path.trim()) {
                    Ok(bytes) => {
                        p.upload = Some(bytes);
                        p.error = None;
                    }
                    Err(e) => p.error = Some(format!("cannot read {}: {e}", p.path.trim())),
                }
            }
        });
        if let Some(e) = &p.error {
            ui.add(egui::Label::new(RichText::new(e).size(11.0).color(theme::BAD)).wrap());
        }
    });
}

/// the form; true when a field was left (or the checkbox changed), to send what it says
fn details(ui: &mut Ui, form: &mut Form, release: &ReleaseView) -> bool {
    let mut left = false;
    let artist = if form.artist.trim().is_empty() { "the artist's name".to_string() } else { form.artist.clone() };
    egui::Grid::new("release_details").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        let mut row = |ui: &mut Ui, label: &str, value: &mut String, hint: &str| {
            ui.label(RichText::new(label).size(12.0).color(TEXT_DIM));
            left |= ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(330.0)).lost_focus();
            ui.end_row();
        };
        row(ui, "Artist", &mut form.artist, "the name the song comes out under");
        row(ui, "Title", &mut form.title, &release.song_title);
        row(ui, "Version", &mut form.version, "Extended Mix, Radio Edit (if it is one)");
        row(ui, "Featuring", &mut form.featuring, "featured artists, between commas");
        row(ui, "Album or EP", &mut form.album, "without one, the song is a single");
        row(ui, "Genre", &mut form.genre, "Dance, Electronic, Hip-Hop/Rap");
        row(ui, "Subgenre", &mut form.subgenre, "House, Techno, Lo-Fi Hip-Hop");
        row(ui, "Release date", &mut form.date, "YYYY-MM-DD, four weeks ahead or more");
        row(ui, "Label", &mut form.label, &artist);
        row(ui, "Songwriters", &mut form.writers, "full legal names, between commas");
        row(ui, "Producers", &mut form.producers, "between commas");
        row(ui, "Owner", &mut form.owner, &format!("{artist}, for the copyright lines"));
        row(ui, "ISRC", &mut form.isrc, "the distributor assigns one");
        row(ui, "UPC", &mut form.upc, "the distributor assigns one");
        ui.label(RichText::new("Lyrics").size(12.0).color(TEXT_DIM));
        left |= ui.checkbox(&mut form.explicit, small("explicit")).changed();
        ui.end_row();
    });
    left
}

/// how the song was made, which only the person can say; true when it changed
fn made(ui: &mut Ui, form: &mut Form, release: &ReleaseView) -> bool {
    widgets::heading(ui, "How was it made?");
    ui.label(small("Distributors ask whether music is made with AI, and stores label it. Only you can answer."));
    ui.add_space(4.0);
    let mut changed = false;
    for (name, what) in &release.made_options {
        let on = form.made.as_deref() == Some(name.as_str());
        let text = RichText::new(what).size(12.5).color(if on { TEXT } else { TEXT_DIM });
        if ui.radio(on, text).clicked() && !on {
            form.made = Some(name.clone());
            changed = true;
        }
    }
    changed
}

/// what stores and distributors would say, problems first
fn checks(ui: &mut Ui, release: &ReleaseView, song_version: u64) {
    widgets::heading(ui, "What stores would say");
    ui.add_space(2.0);
    if release.checked == 0 {
        ui.label(small("checked once the song is rendered"));
        return;
    }
    if release.checked < song_version {
        ui.label(RichText::new("of an earlier render: checked again after the next one").size(11.0).color(theme::WARN));
    }
    if release.checks.is_empty() {
        ui.label(RichText::new("nothing to fix: ready for a distributor").size(12.5).color(theme::GOOD));
    }
    for c in &release.checks {
        let color = match c.level.as_str() {
            "problem" => theme::BAD,
            "warning" => theme::WARN,
            _ => TEXT_FAINT,
        };
        ui.horizontal_top(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, color);
            let text = format!("{}: {}", c.about, c.message);
            let tint = if c.level == "note" { TEXT_DIM } else { TEXT };
            ui.add(egui::Label::new(RichText::new(text).size(12.0).color(tint)).wrap());
        });
    }
}
