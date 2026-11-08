//! The Add a recording window: a song or a sound of the person's own, chosen or dropped on the studio, or one a free
//! online library offers (searched for and heard first), added to the song whole as a track (a take, which they trim
//! and put effects on as they like) or with its vocals sampled into it. Trimming a take is a window of its own
//! (`Trim`), opened from its track's menu.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use claudius_api::{Command, LibraryHit};
use eframe::egui;

use super::Cx;
use super::record::ROLES;
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
    /// what plays to be heard (a library's recording or a piece, by its id), the words it goes by, and the player it
    /// plays in
    hearing: Option<String>,
    hearing_as: String,
    player: Option<Player>,
    /// why the last one to be heard did not play
    play_said: Option<String>,
    /// a recording to hear, fetched and decoded in the app, arriving: which one, and the decoding
    decoding: Option<(String, Receiver<Result<crate::audio::Pcm, String>>)>,
    /// the search's recording to use now, by its place in the list, as its Choose button chooses it (a scripted run's
    /// `import:use:<n>`)
    pub use_nth: Option<usize>,
    /// a piece to hear now, by its id, treated or as it was cut, as its button plays it (a scripted run's
    /// `import:hear:<n>[:treated]`)
    pub hear_now: Option<(u32, bool)>,
    /// a recording held, to send for the auto-sampler's pieces (`/api/sample/pieces`), with the library's record of
    /// it when a free library offered it (which the demo's engine credits it by)
    pub offer: Option<(Arc<Picked>, Option<serde_json::Value>)>,
    /// the recording held is to be sampled now, as its Sample it button samples it (a scripted run's
    /// `import:sample`)
    pub sample_now: bool,
    /// the Sample window, which shows the pieces offered: whether it is open, what claudius said, and for each piece
    /// of the take it shows whether it goes into the song and whether treated
    pub sampling: bool,
    pub sample_said: Option<(bool, String)>,
    choices: (u64, Vec<Choice>),
    /// the offer asked for is on its way: the one claudius holds meanwhile is the last one's, which is not shown; and
    /// the state's version it comes in, which the answer says (the state before it still holds the last one's)
    awaiting: bool,
    offer_version: u64,
    /// the pieces chosen are in the song: the window closes
    kept: bool,
}

/// what to do with a piece the auto-sampler offers
#[derive(Clone, Copy)]
struct Choice {
    used: bool,
    treated: bool,
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

    /// claudius' answer to the pieces asked for, drawn again or kept, and the state's version after it
    /// (`kept`: the answer to the pieces kept, which closes the window when they went in)
    pub fn pieces_answered(&mut self, ok: bool, message: String, kept: bool, version: u64) {
        self.kept |= ok && kept;
        // an offer that did not come leaves the last one's unshown; one that came shows from the state it is in, and
        // says itself, in the window
        if ok {
            (self.awaiting, self.offer_version) = (false, version);
        }
        self.sample_said = (!ok).then_some((ok, message));
    }

    /// the Sample window opened anew, as an offer is asked for, or closed: nothing chosen, nothing said
    fn sample_afresh(&mut self, open: bool) {
        (self.sampling, self.sample_said, self.choices, self.kept) = (open, None, (u64::MAX, Vec::new()), false);
        self.awaiting = open;
    }

    /// what plays to be heard, for a scripted run to read: that it plays, once it has played a moment, or why not
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn hearing_told(&self) -> String {
        let playing = self.player.as_ref().is_some_and(|p| p.is_playing() && p.position() > 0.25);
        match (&self.hearing, &self.play_said) {
            (Some(_), _) if playing => format!("{} plays", self.hearing_as),
            (None, Some(why)) => format!("{} did not play: {why}", self.hearing_as),
            _ => String::new(),
        }
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
                (Err(e), _) if wanted => (self.hearing, self.play_said) = (None, Some(e)),
                _ => {}
            }
        }
        if let Some(player) = self.player.as_mut() {
            player.tick();
            if self.hearing.is_some() {
                if let Some(why) = player.error() {
                    (self.hearing, self.play_said) = (None, Some(why.to_string()));
                } else if self.decoding.is_none() && !player.is_loading() && !player.is_playing() && player.has_audio()
                {
                    self.hearing = None;
                }
            }
        }
    }

    /// whether what goes by `id` is on its way to be heard, fetched or decoded
    fn loading(&self, id: &str) -> bool {
        self.hearing.as_deref() == Some(id)
            && (self.decoding.as_ref().is_some_and(|(d, _)| d == id)
                || self.player.as_ref().is_some_and(Player::is_loading))
    }

    /// plays the audio at `url` to be heard (a library's recording, or a piece), known by `id` and told as `told`, or
    /// stops it when it plays already
    fn hear(&mut self, ctx: &egui::Context, id: &str, told: &str, url: &str, server: Option<&str>) {
        let player = self.player.get_or_insert_with(|| Player::new(ctx));
        player.stop();
        self.play_said = None;
        if self.hearing.as_deref() == Some(id) {
            self.hearing = None;
            return;
        }
        (self.hearing, self.hearing_as) = (Some(id.to_string()), told.to_string());
        // a browser plays any kind of file, and one handed over in its URL
        #[cfg(target_arch = "wasm32")]
        {
            player.load(Source::Url(absolute(url, server)), false);
            player.play();
        }
        // the app decodes it as claudius does, before it is played: what played before does not play meanwhile
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = channel();
            if let Some(pcm) = decoded(url) {
                let _ = tx.send(pcm);
            } else {
                let ctx = ctx.clone();
                ehttp::fetch(ehttp::Request::get(absolute(url, server)), move |res| {
                    let pcm = fetched(res).and_then(|bytes| {
                        // a name without a kind: its bytes say what it is (a claudius serves it by its id)
                        let x = claudius_session::listen::decode(bytes, "the recording")?;
                        let samples = x.l.iter().zip(&x.r).flat_map(|(l, r)| [*l, *r]).collect();
                        Ok(crate::audio::Pcm { rate: claudius::config::SR, channels: 2, samples })
                    });
                    let _ = tx.send(pcm);
                    ctx.request_repaint();
                });
            }
            self.decoding = Some((id.to_string(), rx));
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

/// a piece the demo's engine hands over as a file in its URL, decoded as claudius decodes one, in the app (a browser
/// plays the URL as it is); none for one to fetch
#[cfg(not(target_arch = "wasm32"))]
fn decoded(url: &str) -> Option<Result<crate::audio::Pcm, String>> {
    Some(
        claudius_session::listen::mp3_of(url)?
            .and_then(|bytes| claudius_session::listen::decode(bytes, "piece.mp3"))
            .map(|x| crate::audio::Pcm {
                rate: claudius::config::SR,
                channels: 2,
                samples: x.l.iter().zip(&x.r).flat_map(|(l, r)| [*l, *r]).collect(),
            }),
    )
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
        ui.label("Then add it to the song whole, or sample its vocals into it:");
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
            // the Sample window offers the pieces first, to hear and choose
            let sample = ui.add_enabled(free, egui::Button::new("Sample its vocals...")).on_hover_text(
                "the auto-sampler cuts its best sung phrases into pieces: you hear each, as it was cut and treated, and \
                 choose which go into the song",
            );
            if sample.clicked() || (std::mem::take(&mut im.sample_now) && free) {
                match (&by_id, &im.file) {
                    (Some(path), _) => cx.out.push(Command::SamplePieces {
                        path: path.clone(),
                        seed: None,
                        count: Some(4),
                        bars: None,
                        find: None,
                    }),
                    (None, Some(file)) => im.offer = Some((file.clone(), found.clone())),
                    (None, None) => {}
                }
                im.sample_afresh(true);
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

/// The Sample window, while it is open: the pieces the auto-sampler offers, each to hear as it was cut and treated,
/// ticked to go into the song either way, another take of them, and the ticked put in the song.
pub fn pieces(ctx: &egui::Context, cx: &mut Cx) {
    if !cx.view.import.sampling {
        return;
    }
    let im = &cx.view.import;
    let current = !im.awaiting && cx.state.version >= im.offer_version;
    let offered = cx.state.pieces.clone().filter(|_| current);
    let server = cx.server.map(str::to_string);
    let mut open = true;
    let mut cancel = false;
    let title = offered.as_ref().map_or_else(|| "Sample".to_string(), |p| format!("Sample {}", p.recording));
    egui::Window::new(title).id(egui::Id::unique("pieces")).open(&mut open).resizable(false).collapsible(false).show(
        ctx,
        |ui| {
            let im = &mut cx.view.import;
            let Some(offered) = offered else {
                if im.sample_said.as_ref().is_none_or(|(ok, _)| *ok) {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("the auto-sampler is listening for the best sung phrases ...");
                    });
                }
                said(ui, &im.sample_said);
                ui.add_space(6.0);
                cancel = ui.button("Cancel").clicked();
                return;
            };
            if im.choices.0 != offered.seed || im.choices.1.len() != offered.pieces.len() {
                // the best two, as they were cut, to start with
                let choices = (0..offered.pieces.len()).map(|i| Choice { used: i < 2, treated: false }).collect();
                im.choices = (offered.seed, choices);
            }
            ui.label("Hear each piece as it was cut and treated, tick the ones you want, then put them in the song.");
            ui.add_space(4.0);
            let hear_now = im.hear_now.take();
            egui::Grid::new("pieces-grid").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
                for (i, p) in offered.pieces.iter().enumerate() {
                    let tip = format!("{} bars, {:.0}% sung: tick it to put it in the song", p.bars, p.sung * 100.0);
                    let words = egui::RichText::new(format!("{}. {}", p.id, p.place)).strong();
                    ui.checkbox(&mut im.choices.1[i].used, words).on_hover_text(tip);
                    for (treated, label, url, tip) in [
                        (false, "as cut", &p.raw, "as it was cut, cleaned of the recording's drums"),
                        (true, "treated", &p.treated, p.treatment.as_str()),
                    ] {
                        let button = if im.loading(url) {
                            format!("… {label}")
                        } else if im.hearing.as_deref() == Some(url.as_str()) {
                            "■ stop".to_string()
                        } else {
                            format!("▶ {label}")
                        };
                        let asked = hear_now == Some((p.id, treated));
                        if ui.small_button(button).on_hover_text(tip).clicked() || asked {
                            im.hear(ui.ctx(), url, &format!("{} {label}", p.id), url, server.as_deref());
                        }
                    }
                    let choice = &mut im.choices.1[i];
                    ui.add_enabled_ui(choice.used, |ui| {
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut choice.treated, false, "as cut");
                            let tip = format!("through its treatment: {}", p.treatment);
                            ui.selectable_value(&mut choice.treated, true, "treated").on_hover_text(tip);
                        });
                    });
                    ui.end_row();
                }
            });
            if let Some(why) = &im.play_said {
                ui.label(egui::RichText::new(format!("could not play it: {why}")).color(ui.visuals().error_fg_color));
            }
            ui.add_space(6.0);
            let keep: Vec<claudius_api::KeptPiece> = offered
                .pieces
                .iter()
                .zip(&im.choices.1)
                .filter(|(_, c)| c.used)
                .map(|(p, c)| claudius_api::KeptPiece { id: p.id, treated: c.treated })
                .collect();
            ui.horizontal(|ui| {
                let another = ui.button("Another take").on_hover_text("other phrases, other lengths, other treatments");
                if another.clicked() {
                    cx.out.push(Command::RedrawPieces { seed: None });
                    (im.sample_said, im.awaiting) = (None, true);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = match keep.len() {
                        0 => "Put them in the song".to_string(),
                        1 => "Put 1 piece in the song".to_string(),
                        n => format!("Put {n} pieces in the song"),
                    };
                    let put = egui::Button::new(egui::RichText::new(label).strong().color(egui::Color32::BLACK))
                        .fill(crate::theme::ACCENT);
                    let put = ui.add_enabled(!keep.is_empty(), put).on_disabled_hover_text("tick a piece first");
                    if put.clicked() {
                        cx.out.push(Command::KeepPieces { keep: keep.clone(), sections: Vec::new() });
                        im.sample_said = None;
                    }
                    cancel = ui.button("Cancel").clicked();
                });
            });
            said(ui, &im.sample_said);
        },
    );
    let im = &mut cx.view.import;
    // kept, the offer goes, and so does the window
    if !open || cancel || im.kept {
        im.sample_afresh(false);
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
                let (glyph, tip) = if im.loading(&h.id) {
                    ("…", "on its way: it plays as it arrives")
                } else if im.hearing.as_deref() == Some(h.id.as_str()) {
                    ("■", "stop")
                } else {
                    ("▶", "hear it")
                };
                if ui.small_button(glyph).on_hover_text(tip).clicked() {
                    im.hear(ui.ctx(), &h.id, &h.title, &h.audio, server);
                }
                let fetching = im.fetching.as_ref().is_some_and(|(f, _)| f.id == h.id);
                let chosen = im.hit.as_ref().is_some_and(|c| c.id == h.id);
                let label = if fetching {
                    "fetching"
                } else if chosen {
                    "chosen"
                } else {
                    "Choose"
                };
                let choose = ui.add_enabled(!fetching && !chosen, egui::Button::new(label).small());
                if choose.on_hover_text("hold it here: then add it whole, or sample its vocals").clicked() {
                    im.take_up(ui.ctx(), h, demo);
                }
                let secs = h.seconds.map(|s| format!(", {}:{:02}", s as u64 / 60, s as u64 % 60)).unwrap_or_default();
                ui.label(egui::RichText::new(&h.title).strong());
                ui.weak(format!("{}, {}{secs}", h.creator, h.licence)).on_hover_text(&h.page);
            });
        }
    });
    if let Some(why) = im.play_said.as_ref().filter(|_| !im.sampling) {
        ui.label(egui::RichText::new(format!("could not play it: {why}")).color(ui.visuals().error_fg_color));
    }
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
