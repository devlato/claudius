//! The Add a recording window: a song or a sound of the person's own, chosen or dropped on the studio, or one a free
//! online library offers (searched for and heard first), added to the song whole as a track (a take, which they trim
//! and put effects on as they like) or with its vocals sampled into it. Trimming a take is a window of its own
//! (`Trim`), opened from its track's menu.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use claudius_api::{Command, LibraryHit};
use eframe::egui;

use super::record::ROLES;
use super::{Cx, Listen};
use crate::audio::{Player, Source};
use crate::pick::{self, Picked, Want};

/// a file fetched, or why not
type Fetched = Result<Vec<u8>, String>;

/// the window, and the recording it holds
#[derive(Default)]
pub struct Import {
    pub open: bool,
    pub file: Option<Arc<Picked>>,
    /// the library's recording it holds: what the song credits, and, with a claudius, what it is added by
    hit: Option<LibraryHit>,
    /// the section it starts with (the song's first when empty), the role it plays and its track's name
    from: String,
    role: String,
    name: String,
    /// the recording on its way to claudius, as a take
    pub upload: Option<super::record::Upload>,
    /// the recording on its way, whose answer is awaited: one chosen meanwhile stays when it is added
    sending: Option<Sent>,
    /// what claudius said of the last one, and whether it was added
    said: Option<(bool, String)>,
    /// a take being trimmed
    pub trim: Option<Trim>,
    /// the recording is to be added whole now, as its button adds it (a scripted run's `import:whole`)
    pub whole: bool,
    /// the words to search the libraries for
    query: String,
    /// a search for the app to send, and whether one is out
    pub search: Option<String>,
    searching: bool,
    /// what the last search found, or why nothing
    hits: Option<Result<Vec<LibraryHit>, String>>,
    /// a library's recording being fetched to hold (in the demo, whose engine needs its audio)
    fetching: Option<(LibraryHit, Receiver<Fetched>)>,
    /// the library's recording playing to be heard, and the player it plays in
    hearing: Option<String>,
    player: Option<Player>,
    /// a recording to hear, fetched and decoded in the app, arriving: which one, and the decoding
    decoding: Option<(String, Receiver<Result<crate::audio::Pcm, String>>)>,
    /// the search's recording to use now, by its place in the list, as its Use it button uses it (a scripted run's
    /// `import:use:<n>`)
    pub use_nth: Option<usize>,
}

/// what the window sent: a recording it holds, or a library's, by its id
enum Sent {
    File(Arc<Picked>),
    Hit(String),
}

/// a take's trim, as the window sets it: from where to where in its recording, in seconds
pub struct Trim {
    pub track: String,
    pub seconds: f64,
    pub start: f64,
    pub end: f64,
}

impl Trim {
    pub fn of(track: &str, seconds: f64) -> Trim {
        Trim { track: track.to_string(), seconds, start: 0.0, end: seconds }
    }
}

impl Import {
    /// a recording dropped on the studio or chosen, read, to add
    pub fn holds(&mut self, file: Picked) {
        (self.file, self.hit, self.open, self.said) = (Some(Arc::new(file)), None, true, None);
    }

    /// the libraries searched for `words`, as the window's Search button searches them (a scripted run's
    /// `import:search:<words>`)
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn ask(&mut self, words: &str) {
        self.open = true;
        self.query = words.to_string();
        (self.search, self.searching, self.hits) = (Some(self.query.clone()), true, None);
    }

    /// what the last search offered, for a scripted run to read: the titles, or why nothing
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn listed(&self) -> String {
        match &self.hits {
            Some(Ok(hits)) => hits.iter().map(|h| h.title.as_str()).collect::<Vec<_>>().join(" | "),
            Some(Err(e)) => e.clone(),
            None => String::new(),
        }
    }

    /// what the libraries offered for the search sent, or why nothing
    pub fn found(&mut self, hits: Result<Vec<LibraryHit>, String>) {
        (self.searching, self.hits) = (false, Some(hits));
    }

    /// whether an answer from claudius is awaited
    pub fn waiting(&self) -> bool {
        self.sending.is_some()
    }

    /// what claudius said of the last recording, for a scripted run to read
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn told(&self) -> String {
        self.said.as_ref().map(|(_, text)| text.clone()).unwrap_or_default()
    }

    /// claudius' answer to the recording sent
    pub fn answered(&mut self, ok: bool, message: String) {
        let sent = self.sending.take();
        let same = match &sent {
            Some(Sent::File(s)) => self.file.as_ref().is_some_and(|f| Arc::ptr_eq(f, s)),
            Some(Sent::Hit(id)) => self.file.is_none() && self.hit.as_ref().is_some_and(|h| &h.id == id),
            None => false,
        };
        if ok && same {
            (self.file, self.hit) = (None, None);
            self.name.clear();
        }
        self.said = Some((ok, message));
    }

    /// what arrived meanwhile: a library's recording fetched to hold, a recording to hear decoded
    fn arrivals(&mut self) {
        if let Some((hit, rx)) = &self.fetching
            && let Ok(got) = rx.try_recv()
        {
            match got {
                Ok(bytes) => {
                    let file = Picked { name: file_name(hit), bytes, folder: None };
                    (self.file, self.hit, self.said) = (Some(Arc::new(file)), Some(hit.clone()), None);
                }
                Err(e) => self.said = Some((false, format!("{} did not arrive: {e}", hit.title))),
            }
            self.fetching = None;
        }
        if let Some((id, rx)) = &self.decoding
            && let Ok(got) = rx.try_recv()
        {
            // one stopped, or another asked for since, is not played
            let wanted = self.hearing.as_deref() == Some(id.as_str());
            self.decoding = None;
            match (got, self.player.as_mut()) {
                (Ok(pcm), Some(player)) if wanted => {
                    player.load(Source::Pcm(Arc::new(pcm)), false);
                    player.play();
                }
                (Err(e), _) if wanted => (self.hearing, self.said) = (None, Some((false, e))),
                _ => {}
            }
        }
        if let Some(player) = self.player.as_mut() {
            player.tick();
            if self.hearing.is_some() && self.decoding.is_none() && !player.is_playing() && player.has_audio() {
                self.hearing = None;
            }
        }
    }

    /// plays `hit` to be heard, or stops it when it plays already
    fn hear(&mut self, ctx: &egui::Context, hit: &LibraryHit, server: Option<&str>) {
        let player = self.player.get_or_insert_with(|| Player::new(ctx));
        player.stop();
        if self.hearing.as_deref() == Some(hit.id.as_str()) {
            self.hearing = None;
            return;
        }
        self.hearing = Some(hit.id.clone());
        let url = absolute(&hit.audio, server);
        // a browser plays any kind of file; the app decodes it as claudius does
        #[cfg(target_arch = "wasm32")]
        {
            player.load(Source::Url(url), false);
            player.play();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = channel();
            let ctx = ctx.clone();
            ehttp::fetch(ehttp::Request::get(&url), move |res| {
                let pcm = fetched(res).and_then(|bytes| {
                    // a name without a kind: its bytes say what it is (a claudius serves it by its id)
                    let x = claudius_session::listen::decode(bytes, "the recording")?;
                    let samples = x.l.iter().zip(&x.r).flat_map(|(l, r)| [*l, *r]).collect();
                    Ok(crate::audio::Pcm { rate: claudius::config::SR, channels: 2, samples })
                });
                let _ = tx.send(pcm);
                ctx.request_repaint();
            });
            self.decoding = Some((hit.id.clone(), rx));
        }
    }

    /// holds `hit`: by its id, which a claudius downloads it by, or, in the demo, by its audio, fetched now
    fn take_up(&mut self, ctx: &egui::Context, hit: &LibraryHit, demo: bool) {
        if !demo {
            (self.file, self.hit, self.said) = (None, Some(hit.clone()), None);
            return;
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        ehttp::fetch(ehttp::Request::get(&hit.audio), move |res| {
            let _ = tx.send(fetched(res));
            ctx.request_repaint();
        });
        self.fetching = Some((hit.clone(), rx));
    }
}

/// what a fetch brought: the file, or why not
fn fetched(res: Result<ehttp::Response, String>) -> Fetched {
    match res {
        Ok(r) if r.ok => Ok(r.bytes),
        Ok(r) => Err(format!("{} {}", r.status, r.status_text)),
        Err(e) => Err(e),
    }
}

/// a library recording's file name, after its title, with the kind of file its audio is
fn file_name(hit: &LibraryHit) -> String {
    match hit.audio.rsplit('.').next().filter(|e| e.len() <= 4 && !e.contains(['/', '?', '='])) {
        Some(ext) => format!("{}.{ext}", hit.title),
        // its bytes say what it is
        None => hit.title.clone(),
    }
}

/// `audio` as a URL to fetch: as it is, or on the claudius the studio talks to
fn absolute(audio: &str, server: Option<&str>) -> String {
    match server {
        Some(base) if audio.starts_with('/') => format!("{base}{audio}"),
        _ => audio.to_string(),
    }
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    if let Some(got) = pick::chosen(Want::Recording) {
        match got {
            Ok(file) => cx.view.import.holds(file),
            Err(e) => (cx.view.import.open, cx.view.import.said) = (true, Some((false, e))),
        }
    }
    cx.view.import.arrivals();
    trim(ctx, cx);
    if !cx.view.publish.open && ctx.input(|i| !i.raw.hovered_files.is_empty()) {
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop-recording"));
        let painter = ctx.layer_painter(layer);
        let rect = ctx.content_rect();
        painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(170));
        let words = "Drop it to add the recording to the song";
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            words,
            egui::FontId::proportional(22.0),
            egui::Color32::WHITE,
        );
    }
    if !cx.view.import.open {
        return;
    }
    let demo = cx.server.is_none();
    let server = cx.server.map(str::to_string);
    let im = &mut cx.view.import;
    if let Some(n) = im.use_nth.take_if(|_| im.hits.is_some())
        && let Some(Ok(hits)) = im.hits.clone()
        && let Some(h) = hits.get(n)
    {
        im.take_up(ctx, h, demo);
    }
    let sections: Vec<String> =
        cx.state.song.as_ref().map(|s| s.sections.iter().map(|s| s.name.clone()).collect()).unwrap_or_default();
    let mut open = true;
    egui::Window::new("Add a recording").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        let im = &mut cx.view.import;
        ui.horizontal(|ui| {
            let label = if im.file.is_some() || im.hit.is_some() { "Choose another..." } else { "Choose a file..." };
            if ui.button(label).on_hover_text("WAV, MP3, FLAC, AAC, AIFF, Ogg or Opus").clicked() {
                pick::choose(ui.ctx(), Want::Recording);
            }
            ui.weak("or drop one on the studio");
        });
        library(ui, im, demo, server.as_deref());
        ui.separator();
        let held = im.hit.as_ref().map(|h| h.title.clone()).or_else(|| im.file.as_ref().map(|f| f.name.clone()));
        let Some(held) = held else {
            ui.label(
                "A song or a sound of your own, or one from a free library: added whole as a track, which you \
                      trim and put effects on, or with its vocals sampled into the song.",
            );
            said(ui, &im.said);
            return;
        };
        ui.label(egui::RichText::new(held).strong());
        if let Some(h) = &im.hit {
            ui.weak(format!("credit: {}", h.credit));
        } else if let Some(f) = &im.file {
            ui.weak(format!("{:.1} MB", f.bytes.len() as f64 / 1_048_576.0));
        }
        if sections.is_empty() {
            ui.label("Start a song first: the recording goes into it.");
            return;
        }
        if !sections.contains(&im.from) {
            im.from = sections[0].clone();
        }
        if im.role.is_empty() {
            im.role = "vocal".into();
        }
        egui::Grid::new("import-take").num_columns(2).show(ui, |ui| {
            ui.label("starts with");
            egui::ComboBox::from_id_salt("import-from").selected_text(im.from.as_str()).show_ui(ui, |ui| {
                for s in &sections {
                    ui.selectable_value(&mut im.from, s.clone(), s);
                }
            });
            ui.end_row();
            ui.label("plays as");
            egui::ComboBox::from_id_salt("import-role").selected_text(im.role.as_str()).show_ui(ui, |ui| {
                for r in ROLES {
                    ui.selectable_value(&mut im.role, r.to_string(), *r);
                }
            });
            ui.end_row();
            ui.label("track");
            ui.add(egui::TextEdit::singleline(&mut im.name).hint_text("take1").desired_width(140.0));
            ui.end_row();
        });
        ui.add_space(4.0);
        // with a claudius, a library's recording goes by its id, which it downloads once and credits
        let by_id = im.hit.as_ref().filter(|_| !demo).map(|h| format!("library:{}", h.id));
        let found = im.hit.as_ref().and_then(|h| h.found.clone());
        ui.horizontal(|ui| {
            let free = im.sending.is_none();
            let whole = ui
                .add_enabled(free, egui::Button::new("Add it whole as a track"))
                .on_hover_text("it plays once from the start of the section, on for as long as it lasts");
            if whole.clicked() || (std::mem::take(&mut im.whole) && free) {
                let name = Some(im.name.trim().to_string()).filter(|n| !n.is_empty());
                match (&by_id, &im.file) {
                    (Some(path), _) => {
                        cx.out.push(Command::AddTake {
                            path: path.clone(),
                            from: im.from.clone(),
                            name,
                            role: Some(im.role.clone()),
                        });
                        im.sending = im.hit.as_ref().map(|h| Sent::Hit(h.id.clone()));
                    }
                    (None, Some(file)) => {
                        im.upload = Some(super::record::Upload {
                            wav: file.bytes.clone(),
                            from: im.from.clone(),
                            name,
                            role: im.role.clone(),
                            request: None,
                            file: Some(file.name.clone()),
                            found: found.clone(),
                        });
                        im.sending = Some(Sent::File(file.clone()));
                    }
                    (None, None) => {}
                }
                im.said = None;
            }
            let sample = ui.add_enabled(free, egui::Button::new("Sample its vocals into the song")).on_hover_text(
                "the auto-sampler: its best sung phrases, cleaned of its drums, in the sections that suit them",
            );
            if sample.clicked() {
                match (&by_id, &im.file) {
                    (Some(path), _) => cx.out.push(Command::SampleSong {
                        path: path.clone(),
                        count: None,
                        bars: None,
                        find: None,
                        sections: Vec::new(),
                    }),
                    (None, Some(file)) => {
                        cx.view.listen = Some(Listen {
                            file: file.clone(),
                            remake: None,
                            flip: false,
                            sample: true,
                            groove: false,
                            found: found.clone(),
                        })
                    }
                    (None, None) => {}
                }
                im.open = false;
            }
        });
        let im = &cx.view.import;
        if im.sending.is_some() {
            ui.weak("adding it ...");
        }
        said(ui, &im.said);
        ui.weak("Trim it from its track's menu in the arrangement; effects go on it in the mixer, as on any track.");
    });
    if !open {
        let im = &mut cx.view.import;
        im.open = false;
        if let Some(player) = im.player.as_mut() {
            player.stop();
        }
        im.hearing = None;
    }
}

/// the search of the free libraries: words, what they offer, each to hear and to use
fn library(ui: &mut egui::Ui, im: &mut Import, demo: bool, server: Option<&str>) {
    ui.add_space(4.0);
    ui.label("or search the free libraries (CC0, public domain, CC BY):");
    ui.horizontal(|ui| {
        let words = ui.add(
            egui::TextEdit::singleline(&mut im.query)
                .hint_text("a soul singer, a jazz drum break")
                .desired_width(240.0),
        );
        let enter = words.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let go = ui.add_enabled(!im.searching && !im.query.trim().is_empty(), egui::Button::new("Search"));
        if (go.clicked() || enter) && !im.query.trim().is_empty() && !im.searching {
            (im.search, im.searching, im.hits) = (Some(im.query.trim().to_string()), true, None);
        }
    });
    if im.searching {
        ui.weak("searching ...");
    }
    let Some(hits) = im.hits.clone() else { return };
    let hits = match hits {
        Ok(hits) if hits.is_empty() => {
            ui.weak("nothing under a licence that lets you publish music made with it: try other words");
            return;
        }
        Ok(hits) => hits,
        Err(e) => {
            ui.label(egui::RichText::new(e).color(ui.visuals().error_fg_color));
            return;
        }
    };
    egui::ScrollArea::vertical().max_height(220.0).id_salt("library-hits").show(ui, |ui| {
        for h in &hits {
            ui.horizontal(|ui| {
                let playing = im.hearing.as_deref() == Some(h.id.as_str());
                let tip = if playing { "stop" } else { "hear it" };
                if ui.small_button(if playing { "■" } else { "▶" }).on_hover_text(tip).clicked() {
                    im.hear(ui.ctx(), h, server);
                }
                let fetching = im.fetching.as_ref().is_some_and(|(f, _)| f.id == h.id);
                let chosen = im.hit.as_ref().is_some_and(|c| c.id == h.id);
                let label = if fetching {
                    "fetching"
                } else if chosen {
                    "chosen"
                } else {
                    "Use it"
                };
                if ui.add_enabled(!fetching && !chosen, egui::Button::new(label).small()).clicked() {
                    im.take_up(ui.ctx(), h, demo);
                }
                let secs = h.seconds.map(|s| format!(", {}:{:02}", s as u64 / 60, s as u64 % 60)).unwrap_or_default();
                ui.label(egui::RichText::new(&h.title).strong());
                ui.weak(format!("{}, {}{secs}", h.creator, h.licence)).on_hover_text(&h.page);
            });
        }
    });
}

/// what claudius said of the last recording
fn said(ui: &mut egui::Ui, said: &Option<(bool, String)>) {
    if let Some((ok, text)) = said {
        let color = if *ok { ui.visuals().text_color() } else { ui.visuals().error_fg_color };
        ui.label(egui::RichText::new(text).color(color));
    }
}

/// the Trim window, while a take is being trimmed
fn trim(ctx: &egui::Context, cx: &mut Cx) {
    let Some(t) = cx.view.import.trim.as_mut() else { return };
    let mut open = true;
    let mut done = false;
    egui::Window::new(format!("Trim {}", t.track)).open(&mut open).resizable(false).collapsible(false).show(
        ctx,
        |ui| {
            ui.label(format!("{} is {:.2} s of its recording. Keep:", t.track, t.seconds));
            egui::Grid::new("trim-take").num_columns(2).show(ui, |ui| {
                ui.label("from");
                ui.add(egui::Slider::new(&mut t.start, 0.0..=t.seconds).suffix(" s").max_decimals(2));
                ui.end_row();
                ui.label("to");
                ui.add(egui::Slider::new(&mut t.end, 0.0..=t.seconds).suffix(" s").max_decimals(2));
                ui.end_row();
            });
            t.end = t.end.max(t.start);
            ui.weak("It still plays from the start of its section; undo brings the whole of it back.");
            let keeps = t.end - t.start >= 0.1 && (t.start > 0.0 || t.end < t.seconds);
            if ui.add_enabled(keeps, egui::Button::new("Trim")).clicked() {
                let end = (t.end < t.seconds).then_some(t.end);
                cx.out.push(claudius_api::Command::TrimTake { track: t.track.clone(), start: t.start, end });
                done = true;
            }
        },
    );
    if done || !open {
        cx.view.import.trim = None;
    }
}
