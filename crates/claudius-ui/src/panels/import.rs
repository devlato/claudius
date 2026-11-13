//! The Add a recording window: a song or a sound of the person's own, chosen or dropped on the studio, or one a free
//! online library offers (searched for and heard first). What claudius hears in it shows before anything is asked: its
//! length, tempo and key, whether a voice sings in it, its sections over its waveform and the phrases its voice sings
//! (each to hear). The ways it can go in are cards (added whole as a take, its sung phrases chosen first, a passage,
//! chops, a stab), and where it plays and how often are plain choices. The Sample window shows the phrases offered,
//! to hear and tick. Trimming a take is a window of its own (`Trim`), opened from its track's menu.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use claudius_api::{Command, LibraryHit, Listened, PieceView};
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use super::Cx;
use super::record::ROLES;
use crate::audio::{Player, Source};
use crate::pick::{self, Picked, Want};
use crate::theme::{self, ACCENT, LINE, RAISED, RAISED_HI, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::{self, Icon};

/// a file fetched, or why not
type Fetched = Result<Vec<u8>, String>;

/// how wide the window is
const WIDTH: f32 = 760.0;

/// The ways a recording goes into the song, as the window's cards offer them: by name ("whole", or one of
/// `claudius_api::SAMPLING_WAYS`), its icon, its name on the card, and what it does.
const WAYS: [(&str, Icon, &str, &str); 5] = [
    ("whole", Icon::Sample, "Whole", "all of it as a track, from the start of a section; trim it later"),
    ("phrases", Icon::Voice, "Its sung phrases", "the phrases its voice sings, each whole, to hear and pick first"),
    ("passage", Icon::Melody, "A passage", "a verse or a chorus of it, the song's chords following it there"),
    ("chops", Icon::Keys, "Chops", "its voice sliced and played in a new rhythm, like an instrument"),
    ("stabs", Icon::Drum, "A stab", "one short syllable struck on a few beats, like a drum"),
];

/// How often a phrase comes round in its sections (`Command::KeepPieces`'s `every`): none as the song suits, by its
/// genre and what plays there; each with its words on the window and what it does.
const EVERY: [(Option<&str>, &str, &str); 5] = [
    (None, "As the song suits", "by its genre and what plays there"),
    (Some("once"), "Once, as it starts", "a phrase sung once as each section starts"),
    (Some("4"), "Every 4 bars", "a phrase sung once every 4 bars, silence between"),
    (Some("8"), "Every 8 bars", "a phrase sung once every 8 bars, silence between"),
    (Some("through"), "All through it", "its phrases sung all through each section"),
];

/// the window, and the recording it holds
#[derive(Default)]
pub struct Import {
    pub open: bool,
    pub file: Option<Arc<Picked>>,
    /// the library's recording it holds: what the song credits, and, with a claudius, what it is added by
    hit: Option<LibraryHit>,
    /// what claudius heard in the recording held, and why it could not hear it
    heard: Option<Box<Listened>>,
    heard_failed: Option<String>,
    /// The recording held, to send to be heard for the window (`next_to_hear`); which recording held it is, counted, so
    /// that what was heard in one held before is known for that; and the one sent and not answered yet, by its name and
    /// its count, and whether what was heard in it has come. One goes at a time, so that the answer to it, which says
    /// it is the window's (`hear_recording`) but not which recording it was, is known for it.
    hear_it: Option<Arc<Picked>>,
    held: u64,
    asked: Option<(String, u64, bool)>,
    /// what was heard in the recording the Sample window offers pieces of, and which held it was: its timeline
    offer_heard: Option<Box<Listened>>,
    offer_held: u64,
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
    /// the words to search the libraries for, and whether the search shows while a recording is held
    query: String,
    browse: bool,
    /// a search for the app to send, and whether one is out
    pub search: Option<String>,
    searching: bool,
    /// what the last search found, or why nothing
    hits: Option<Result<Vec<LibraryHit>, String>>,
    /// a library's recording being fetched: to hold, in the demo; to hear what is in it, with a claudius, which holds
    /// it by its id already
    fetching: Option<(LibraryHit, Receiver<Fetched>, bool)>,
    /// what plays to be heard (a library's recording, a piece by its URL, or a phrase on the timeline), the words it
    /// goes by, and the player it plays in
    hearing: Option<String>,
    hearing_as: String,
    player: Option<Player>,
    /// the phrase heard on the timeline, from where to where in the recording, in seconds (it plays from the click,
    /// once the recording is here, and stops at its end); and what the player has of the recording
    span: Option<(f64, f64)>,
    span_source: Option<String>,
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
    /// the recording held is to be sampled now, as its Choose its phrases button samples it (a scripted run's
    /// `import:sample`)
    pub sample_now: bool,
    /// the way it goes in, by its name in `WAYS`; its sung phrases when none is chosen
    how: &'static str,
    /// the sections a sample of it plays in (those that suit it when none), and how often a phrase comes round there
    places: Vec<String>,
    every: Option<&'static str>,
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

/// A menu of the ways to sample a recording (`claudius_api::SAMPLING_WAYS`, or some of them), each in a few words and
/// what it does on hover; `how` holds the name of the one chosen, the first offered when it is none of them.
pub(super) fn way_menu(ui: &mut egui::Ui, id: &str, how: &mut &'static str, ways: &[(&'static str, &str, &str)]) {
    if !ways.iter().any(|w| w.0 == *how) {
        *how = ways[0].0;
    }
    let (words, about) = ways.iter().find(|w| w.0 == *how).map_or(("", ""), |w| (w.1, w.2));
    egui::ComboBox::from_id_salt(id)
        .selected_text(words)
        .width(210.0)
        .show_ui(ui, |ui| {
            for (name, words, about) in ways {
                ui.selectable_value(how, *name, *words).on_hover_text(*about);
            }
        })
        .response
        .on_hover_text(about);
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

/// a span of the recording marked on the timeline: a phrase its voice sings, or a piece offered (numbered), lit when
/// it is chosen
struct Mark {
    from: f64,
    to: f64,
    number: Option<u32>,
    on: bool,
}

impl Import {
    /// a recording dropped on the studio or chosen, read, to add: what claudius hears in it is asked for at once
    pub fn holds(&mut self, file: Picked) {
        let file = Arc::new(file);
        (self.file, self.hit, self.open, self.said, self.browse) = (Some(file.clone()), None, true, None, false);
        self.heard_anew(file);
    }

    /// another recording is held: what was heard in the last one is forgotten, and `file`, when there is one, is to be
    /// heard for the window
    fn held_anew(&mut self, file: Option<Arc<Picked>>) {
        self.held += 1;
        (self.heard, self.heard_failed, self.hear_it) = (None, None, file);
    }

    fn heard_anew(&mut self, file: Arc<Picked>) {
        self.held_anew(Some(file));
    }

    /// The recording to send to be heard for the window now: none while the last one sent is not answered yet, which
    /// it waits for.
    pub fn next_to_hear(&mut self) -> Option<Arc<Picked>> {
        if self.asked.is_some() {
            return None;
        }
        let file = self.hear_it.take()?;
        self.asked = Some((file.name.clone(), self.held, false));
        Some(file)
    }

    /// whether claudius is still to say what it hears in the recording held
    fn listening(&self) -> bool {
        self.hear_it.is_some() || self.asked.as_ref().is_some_and(|(_, h, heard)| *h == self.held && !heard)
    }

    /// whether what claudius heard in the recording named `name` is the window's, which asked for it
    pub fn wants(&self, name: &str) -> bool {
        self.asked.as_ref().is_some_and(|(n, _, heard)| n == name && !heard)
    }

    /// What claudius heard in the recording the window asked about, which comes before the answer: kept when it is the
    /// one held, and the Sample window's timeline when it is the one it offers pieces of.
    pub fn heard_it(&mut self, heard: Box<Listened>) {
        let Some((name, held, got)) = self.asked.as_mut() else { return };
        if *name != heard.name || *got {
            return;
        }
        let held = *held;
        *got = true;
        if held == self.offer_held && self.sampling && self.offer_heard.is_none() {
            self.offer_heard = Some(heard.clone());
        }
        if held == self.held {
            self.heard = Some(heard);
        }
    }

    /// claudius' answer to the recording the window sent to be heard (`hear_recording`), after what it heard in it:
    /// when nothing came, why, in the window, if it is the one still held
    pub fn hearing_answered(&mut self, ok: bool, message: &str) {
        let Some((_, held, heard)) = self.asked.take() else { return };
        if !heard && held == self.held {
            let why = if ok { "claudius said nothing of what it heard in it" } else { message };
            self.heard_failed = Some(why.to_string());
        }
    }

    /// the window open, searching the libraries for `words`, as its Search button searches them (the "+ Track"
    /// menu's search, a scripted run's `import:search:<words>`)
    pub fn search_for(&mut self, words: &str) {
        (self.open, self.browse) = (true, true);
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
        // its timeline is the recording it offers pieces of, whatever is held after
        (self.offer_heard, self.offer_held) = (self.heard.clone().filter(|_| open), self.held);
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
            Some(Sent::Hit(id)) => self.hit.as_ref().is_some_and(|h| &h.id == id),
            None => false,
        };
        if ok && same {
            (self.file, self.hit) = (None, None);
            self.held_anew(None);
            self.name.clear();
        }
        self.said = Some((ok, message));
    }

    /// what arrived meanwhile: a library's recording fetched to hold, a recording to hear decoded
    fn arrivals(&mut self) {
        if let Some((hit, rx, held)) = &self.fetching
            && let Ok(got) = rx.try_recv()
        {
            let hit = hit.clone();
            let (held, still) = (*held, self.hit.as_ref().is_some_and(|h| h.id == hit.id));
            self.fetching = None;
            match got {
                // one chosen instead meanwhile is the one held
                Ok(_) if held && !still => {}
                Ok(bytes) => {
                    let file = Arc::new(Picked { name: file_name(&hit), bytes, folder: None });
                    (self.file, self.hit, self.said, self.browse) = (Some(file.clone()), Some(hit), None, false);
                    // with a claudius it was held as it was chosen, by its id (`take_up`): the same one, now heard
                    match held {
                        true => self.hear_it = Some(file),
                        false => self.heard_anew(file),
                    }
                }
                Err(e) if held => {
                    if still {
                        self.heard_failed = Some(format!("it did not arrive to listen to: {e}"));
                    }
                }
                Err(e) => self.said = Some((false, format!("{} did not arrive: {e}", hit.title))),
            }
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
            if let Some((from, to)) = self.span {
                if player.error().is_some() {
                    self.span = None;
                } else if !player.is_loading()
                    && player.has_audio()
                    && (player.position() >= to || !player.is_playing() && player.position() > from + 0.25)
                {
                    player.pause();
                    (self.hearing, self.span) = (None, None);
                }
            }
            if self.hearing.is_some() && self.span.is_none() {
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
        (self.play_said, self.span, self.span_source) = (None, None, None);
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

    /// Plays the recording held from `from` to `to` seconds in (a phrase on the timeline), or stops it when that plays
    /// already: what claudius kept of it, from the claudius that heard it, or the file itself where it kept nothing
    /// (the demo's page, which plays any file a browser does). The recording is fetched once for all its phrases.
    fn hear_span(&mut self, ctx: &egui::Context, server: Option<&str>, from: f64, to: f64) {
        let id = format!("span:{from:.2}");
        let player = self.player.get_or_insert_with(|| Player::new(ctx));
        player.pause();
        self.play_said = None;
        if self.hearing.as_deref() == Some(id.as_str()) {
            (self.hearing, self.span) = (None, None);
            return;
        }
        let kept =
            self.heard.as_ref().and_then(|h| h.audio.as_ref()).filter(|a| a.tracks.iter().any(|t| t == "recording"));
        let (source, key) = match (kept, server, &self.file) {
            (Some(a), Some(base), _) => {
                let url = format!("{base}/api/listened/{}/recording", a.id);
                (Source::Url(url.clone()), url)
            }
            (_, _, Some(f)) => (Source::File(f.clone()), format!("file:{}", f.name)),
            _ => return,
        };
        if self.span_source.as_deref() != Some(key.as_str()) || player.error().is_some() {
            player.load(source, false);
            self.span_source = Some(key);
        }
        // played as it is clicked, since a browser lets a page play what the person has just asked for and may not
        // later on; one still on its way plays from the phrase once it is here
        player.seek(from);
        player.play();
        (self.hearing, self.hearing_as) = (Some(id), format!("the phrase at {}", clock(from)));
        self.span = Some((from, to));
    }

    /// Holds `hit`: with a claudius at once, by its id, which claudius downloads once and credits, its audio fetched
    /// to hear what is in it; in the demo, whose engine needs its audio, once that has arrived.
    fn take_up(&mut self, ctx: &egui::Context, hit: &LibraryHit, server: Option<&str>) {
        let held = server.is_some();
        if held {
            (self.file, self.hit, self.said, self.browse) = (None, Some(hit.clone()), None, false);
            self.held_anew(None);
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        ehttp::fetch(ehttp::Request::get(absolute(&hit.audio, server)), move |res| {
            let _ = tx.send(fetched(res));
            ctx.request_repaint();
        });
        self.fetching = Some((hit.clone(), rx, held));
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

/// seconds as a clock: 3:42
fn clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// a kind of section as a name starts: "verse" is "Verse"
fn capital(word: &str) -> String {
    let mut c = word.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// the sections of a recording heard, from where to where in seconds, each named as the window shows it: its kind,
/// counted when there are more of that kind ("Intro", "Verse 1", "Chorus 1", "Verse 2")
fn sections_of(l: &Listened) -> Vec<(f64, f64, String)> {
    let start = |b: usize| l.bars.get(b).map_or(l.end, |bar| bar.start);
    let mut seen: Vec<(&str, usize)> = Vec::new();
    l.sections
        .iter()
        .map(|s| {
            let of_kind = l.sections.iter().filter(|o| o.kind == s.kind).count();
            let n = match seen.iter_mut().find(|(k, _)| *k == s.kind) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    seen.push((&s.kind, 1));
                    1
                }
            };
            let name = if of_kind > 1 { format!("{} {n}", capital(&s.kind)) } else { capital(&s.kind) };
            (start(s.start_bar), start(s.start_bar + s.bars), name)
        })
        .collect()
}

/// what claudius heard in it, in a line: "3:42 · 92 BPM · A minor · a voice sings in it"
fn facts(l: &Listened) -> String {
    let voice = if l.phrases.is_empty() { "no voice heard in it" } else { "a voice sings in it" };
    format!("{} · {:.0} BPM · {} · {voice}", clock(l.secs), l.bpm, l.key)
}

/// `text` in `cell`, from its left, when it fits; whether it did
fn fit(p: &egui::Painter, cell: Rect, text: &str, size: f32, color: impl Into<Color32>) -> bool {
    let color = color.into();
    let galley = p.layout_no_wrap(text.to_string(), FontId::proportional(size), color);
    let fits = galley.size().x + 8.0 <= cell.width();
    if fits {
        p.galley(pos2(cell.left() + 4.0, cell.center().y - galley.size().y / 2.0), galley, color);
    }
    fits
}

/// The recording's sections over its waveform, with `marks` on it, and the playhead at `at` seconds when it plays:
/// the mark pressed, by its place in `marks`.
fn timeline(ui: &mut Ui, l: &Listened, marks: &[Mark], at: Option<f64>) -> Option<usize> {
    let total = l.secs.max(l.end).max(1e-3);
    let width = ui.available_width();
    let (strip, _) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
    let x = |t: f64| strip.left() + (t / total).clamp(0.0, 1.0) as f32 * width;
    let p = ui.painter_at(strip);
    for (t0, t1, name) in sections_of(l) {
        let cell = Rect::from_min_max(pos2(x(t0), strip.top()), pos2(x(t1), strip.bottom())).shrink2(vec2(1.0, 0.0));
        p.rect(cell, 3.0, RAISED_HI, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        fit(&p, cell, &name, 11.0, TEXT_DIM);
    }
    ui.add_space(2.0);
    let (wave, r) = ui.allocate_exact_size(vec2(width, 64.0), Sense::click());
    let p = ui.painter_at(wave);
    p.rect_filled(wave, 5.0, WELL);
    let mid = wave.center().y;
    let n = l.waveform.len().max(1);
    for (i, peak) in l.waveform.iter().enumerate() {
        let wx = wave.left() + (i as f32 + 0.5) / n as f32 * width;
        let h = (peak.clamp(0.0, 1.0) * (wave.height() / 2.0 - 4.0)).max(0.5);
        p.vline(wx, mid - h..=mid + h, Stroke::new(1.0, theme::alpha(TEXT_FAINT, 0.55)));
    }
    let pointer = r.hover_pos();
    let mut over = None;
    for (i, m) in marks.iter().enumerate() {
        let cell = Rect::from_min_max(
            pos2(x(m.from), wave.top() + 4.0),
            pos2(x(m.to).max(x(m.from) + 3.0), wave.bottom() - 4.0),
        );
        let hovered = pointer.is_some_and(|q| cell.expand2(vec2(2.0, 0.0)).contains(q));
        over = over.or(hovered.then_some(i));
        let fill = theme::alpha(
            ACCENT,
            if m.on {
                0.4
            } else if hovered {
                0.3
            } else {
                0.18
            },
        );
        p.rect_filled(cell, 3.0, fill);
        p.rect_stroke(cell, 3.0, Stroke::new(1.0, theme::alpha(ACCENT, 0.85)), egui::StrokeKind::Inside);
        if let Some(number) = m.number {
            let font = FontId::proportional(10.5);
            p.text(
                cell.left_top() + vec2(4.0, 2.0),
                Align2::LEFT_TOP,
                number.to_string(),
                font,
                theme::lit(ACCENT, 0.4),
            );
        }
    }
    if let Some(t) = at {
        p.vline(x(t), wave.y_range(), Stroke::new(2.0, ACCENT));
    }
    let r = match over.and_then(|i| marks.get(i)) {
        Some(m) => r.on_hover_text(format!("hear it: {} to {}", clock(m.from), clock(m.to))),
        None => r,
    };
    if over.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    over.filter(|_| r.clicked())
}

/// a card for each way the recording can go in, the one chosen lit
fn ways(ui: &mut Ui, how: &mut &'static str) {
    let gap = 8.0;
    let w = (ui.available_width() - gap * (WAYS.len() - 1) as f32) / WAYS.len() as f32;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (name, icon, title, about) in WAYS {
            let (rect, r) = ui.allocate_exact_size(vec2(w, 110.0), Sense::click());
            let on = *how == name;
            let base = if on { theme::mix(RAISED, ACCENT, 0.2) } else { RAISED.into() };
            let fill = if r.hovered() && !on { RAISED_HI.into() } else { base };
            let stroke = if on { Stroke::new(1.5, ACCENT) } else { Stroke::new(1.0, LINE) };
            let p = ui.painter();
            p.rect(rect, 8.0, fill, stroke, egui::StrokeKind::Inside);
            let inner = rect.shrink(12.0);
            widgets::paint_icon(p, Rect::from_min_size(inner.left_top(), vec2(16.0, 16.0)), icon, ACCENT);
            p.text(inner.left_top() + vec2(0.0, 24.0), Align2::LEFT_TOP, title, theme::medium(13.5), TEXT.into());
            let words = p.layout(about.to_string(), theme::prop(11.5), TEXT_DIM.into(), inner.width());
            p.galley(inner.left_top() + vec2(0.0, 44.0), words, TEXT_DIM.into());
            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
            r.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, title));
            if r.clicked() {
                *how = name;
            }
        }
    });
}

/// a row of the "Where and how often" choices: its name, then what is chosen in it
fn choice_row(ui: &mut Ui, name: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(140.0, 22.0), Sense::hover());
        ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, name, theme::prop(12.5), TEXT_DIM.into());
        add(ui);
    });
}

/// how often a phrase comes round, as chips, and under them what the one chosen does
fn every_row(ui: &mut Ui, every: &mut Option<&'static str>, after: &str) {
    choice_row(ui, "Each time", |ui| {
        for (value, words, about) in EVERY {
            if widgets::chip(ui, *every == value, words, ACCENT).on_hover_text(about).clicked() {
                *every = value;
            }
        }
    });
    let about = EVERY.iter().find(|e| e.0 == *every).map_or("", |e| e.2);
    ui.horizontal(|ui| {
        ui.add_space(140.0);
        ui.label(RichText::new(format!("{about}{after}")).size(11.5).color(TEXT_FAINT));
    });
}

/// a button in the accent's colour, for what the window is for
fn primary(ui: &mut Ui, label: &str, enabled: bool) -> egui::Response {
    let ink = theme::ink_on(ACCENT);
    let button = egui::Button::new(RichText::new(label).strong().color(ink)).fill(ACCENT).min_size(vec2(0.0, 30.0));
    ui.add_enabled(enabled, button)
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
        im.take_up(ctx, h, server.as_deref());
    }
    if im.player.as_ref().is_some_and(|p| p.is_playing() || p.is_loading()) {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
    let sections: Vec<String> =
        cx.state.song.as_ref().map(|s| s.sections.iter().map(|s| s.name.clone()).collect()).unwrap_or_default();
    let mut open = true;
    let mut cancel = false;
    let mut listen = None;
    egui::Window::new("Add a recording")
        .id(egui::Id::unique("add-a-recording"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 70.0))
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            let im = &mut cx.view.import;
            let held = im.hit.as_ref().map(|h| h.title.clone()).or_else(|| im.file.as_ref().map(|f| f.name.clone()));
            let Some(held) = held else {
                ui.add_space(4.0);
                ui.label(
                    "A song or a sound of your own, or one from a free library: claudius listens to it first, then \
                     it goes in whole as a track, or as its sung phrases, a passage, chops or a stab.",
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let tip = "WAV, MP3, FLAC, AAC, AIFF, Ogg or Opus";
                    if primary(ui, "Choose a file...", true).on_hover_text(tip).clicked() {
                        pick::choose(ui.ctx(), Want::Recording);
                    }
                    ui.weak("or drop one on the studio");
                    if im.fetching.is_some() {
                        ui.spinner();
                        ui.weak("fetching it ...");
                    }
                });
                ui.add_space(8.0);
                library(ui, im, server.as_deref());
                said(ui, &im.said);
                return;
            };
            recording(ui, im, &held);
            if im.browse {
                ui.add_space(6.0);
                library(ui, im, server.as_deref());
            }
            ui.add_space(8.0);
            if let Some(l) = im.heard.clone() {
                let marks: Vec<Mark> =
                    l.phrases.iter().map(|[from, to]| Mark { from: *from, to: *to, number: None, on: false }).collect();
                let at = im.span.and(im.player.as_ref()).filter(|p| !p.is_loading()).map(Player::position);
                if let Some(i) = timeline(ui, &l, &marks, at) {
                    im.hear_span(ui.ctx(), server.as_deref(), marks[i].from, marks[i].to);
                }
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().rect(
                        dot,
                        2.0,
                        theme::alpha(ACCENT, 0.3),
                        Stroke::new(1.0, ACCENT),
                        egui::StrokeKind::Inside,
                    );
                    let words = if l.phrases.is_empty() {
                        "no phrase sung whole heard in it".to_string()
                    } else {
                        "a phrase it sings, cut whole · click a phrase to hear it".to_string()
                    };
                    ui.label(RichText::new(words).size(11.5).color(TEXT_FAINT));
                    if let Some(why) = &im.play_said {
                        ui.label(RichText::new(format!("could not play it: {why}")).size(11.5).color(theme::BAD));
                    }
                });
            }
            if sections.is_empty() {
                ui.add_space(8.0);
                ui.label("Start a song first: the recording goes into it.");
                return;
            }
            ui.add_space(10.0);
            ui.label(RichText::new("How should it go in?").strong());
            ui.add_space(4.0);
            if !WAYS.iter().any(|w| w.0 == im.how) {
                im.how = "phrases";
            }
            ways(ui, &mut im.how);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Where and how often").strong());
                ui.label(RichText::new("you can change both in the arrangement later").size(11.5).color(TEXT_FAINT));
            });
            ui.add_space(4.0);
            placing(ui, im, &sections);
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                let words = if im.browse { "Hide the free libraries" } else { "Search the free libraries instead..." };
                if ui.link(RichText::new(words).color(ACCENT)).clicked() {
                    im.browse = !im.browse;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let free = im.sending.is_none();
                    let label = match im.how {
                        "whole" => "Add it whole",
                        "phrases" => "Choose its phrases \u{2192}",
                        "passage" => "Sample a passage",
                        "chops" => "Chop it",
                        _ => "Sample a stab",
                    };
                    let go = primary(ui, label, free).clicked();
                    cancel = ui.add(egui::Button::new("Cancel").min_size(vec2(0.0, 30.0))).clicked();
                    // with a claudius, a library's recording goes by its id, which it downloads once and credits
                    let by_id = im.hit.as_ref().filter(|_| !demo).map(|h| format!("library:{}", h.id));
                    let found = im.hit.as_ref().and_then(|h| h.found.clone());
                    let whole = std::mem::take(&mut im.whole) && free;
                    let phrases = std::mem::take(&mut im.sample_now) && free;
                    if whole || go && im.how == "whole" {
                        add_whole(cx.out, im, by_id.as_deref(), found.clone());
                    } else if phrases || go && im.how == "phrases" {
                        match (&by_id, &im.file) {
                            (Some(path), _) => cx.out.push(Command::SamplePieces {
                                path: path.clone(),
                                seed: None,
                                count: Some(4),
                                bars: None,
                                find: None,
                                kinds: vec!["phrase".into()],
                            }),
                            (None, Some(file)) => im.offer = Some((file.clone(), found.clone())),
                            (None, None) => {}
                        }
                        im.sample_afresh(true);
                    } else if go {
                        listen = sample(cx.out, im, by_id.as_deref(), found);
                    }
                });
            });
            let im = &cx.view.import;
            if im.sending.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.weak("claudius is at it ...");
                });
            }
            said(ui, &im.said);
        });
    if let Some(l) = listen {
        cx.view.listen = Some(l);
    }
    if !open || cancel {
        let im = &mut cx.view.import;
        im.open = false;
        if let Some(player) = im.player.as_mut() {
            player.stop();
        }
        (im.hearing, im.span) = (None, None);
    }
}

/// the recording held: its name, what claudius heard in it (or that it is listening), and another to choose instead
fn recording(ui: &mut Ui, im: &mut Import, held: &str) {
    egui::Frame::new().fill(RAISED.into()).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (tile, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(tile, 6.0, WELL);
            widgets::paint_icon(ui.painter(), tile.shrink(10.0), Icon::Sample, ACCENT);
            ui.vertical(|ui| {
                ui.label(RichText::new(held).strong().size(14.0));
                let size = im.file.as_ref().map(|f| format!(" · {:.1} MB", f.bytes.len() as f64 / 1_048_576.0));
                let credit = im.hit.as_ref().map(|h| format!(" · {}", h.credit));
                let more = format!("{}{}", size.unwrap_or_default(), credit.unwrap_or_default());
                match (&im.heard, &im.heard_failed) {
                    (Some(l), _) => {
                        _ = ui.label(RichText::new(format!("{}{more}", facts(l))).size(12.0).color(TEXT_DIM))
                    }
                    (None, Some(why)) => {
                        let words = format!("claudius could not listen to it: {why}");
                        ui.label(RichText::new(words).size(12.0).color(theme::BAD));
                    }
                    (None, None) if im.listening() || im.fetching.is_some() => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            let words = format!("claudius listens to it: its tempo, key, sections and phrases{more}");
                            ui.label(RichText::new(words).size(12.0).color(TEXT_DIM));
                        });
                    }
                    (None, None) => {
                        _ = ui.label(RichText::new(more.trim_start_matches(" · ")).size(12.0).color(TEXT_DIM))
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Choose another...").on_hover_text("WAV, MP3, FLAC, AAC, AIFF, Ogg or Opus").clicked() {
                    pick::choose(ui.ctx(), Want::Recording);
                }
            });
        });
    });
}

/// where it plays and how often, as the way it goes in asks: the section a take starts with, its role and its name;
/// the sections a sample plays in, and how often a phrase comes round in them
fn placing(ui: &mut Ui, im: &mut Import, sections: &[String]) {
    // a take starts with the song's first section, as a vocal, until chosen otherwise, whatever way is chosen: a
    // scripted run adds it whole without choosing the way
    if !sections.contains(&im.from) {
        im.from = sections[0].clone();
    }
    if im.role.is_empty() {
        im.role = "vocal".into();
    }
    if im.how == "whole" {
        choice_row(ui, "Starts with", |ui| {
            for s in sections {
                if widgets::chip(ui, im.from == *s, s, ACCENT).clicked() {
                    im.from = s.clone();
                }
            }
        });
        choice_row(ui, "As", |ui| {
            egui::ComboBox::from_id_salt("import-role").selected_text(format!("a {}", im.role)).show_ui(ui, |ui| {
                for r in ROLES {
                    ui.selectable_value(&mut im.role, r.to_string(), format!("a {r}"));
                }
            });
            ui.add(egui::TextEdit::singleline(&mut im.name).hint_text("track: take1").desired_width(140.0));
            ui.label(
                RichText::new("it plays once from there, on for as long as it lasts").size(11.5).color(TEXT_FAINT),
            );
        });
        return;
    }
    im.places.retain(|p| sections.contains(p));
    choice_row(ui, "Plays in", |ui| {
        for s in sections {
            let on = im.places.contains(s);
            if widgets::chip(ui, on, s, ACCENT).clicked() {
                match on {
                    true => im.places.retain(|p| p != s),
                    false => im.places.push(s.clone()),
                }
            }
        }
        if im.places.is_empty() {
            ui.label(RichText::new("none chosen: where it suits").size(11.5).color(TEXT_FAINT));
        }
    });
    if im.how == "phrases" {
        every_row(ui, &mut im.every, "");
    }
}

/// the recording held added to the song whole, as a take
fn add_whole(out: &mut Vec<Command>, im: &mut Import, by_id: Option<&str>, found: Option<serde_json::Value>) {
    let name = Some(im.name.trim().to_string()).filter(|n| !n.is_empty());
    let role = if im.role.is_empty() { "vocal".to_string() } else { im.role.clone() };
    match (by_id, &im.file) {
        (Some(path), _) => {
            out.push(Command::AddTake { path: path.to_string(), from: im.from.clone(), name, role: Some(role) });
            im.sending = im.hit.as_ref().map(|h| Sent::Hit(h.id.clone()));
        }
        (None, Some(file)) => {
            im.upload = Some(super::record::Upload {
                wav: file.bytes.clone(),
                from: im.from.clone(),
                name,
                role,
                request: None,
                file: Some(file.name.clone()),
                found,
            });
            im.sending = Some(Sent::File(file.clone()));
        }
        (None, None) => {}
    }
    im.said = None;
}

/// the recording held sampled into the song as a passage, chops or a stab, in the sections chosen; a file chosen on
/// this computer is sent as it is, to listen to
fn sample(
    out: &mut Vec<Command>,
    im: &mut Import,
    by_id: Option<&str>,
    found: Option<serde_json::Value>,
) -> Option<super::Listen> {
    let how = Some(im.how.to_string());
    im.said = None;
    match (by_id, &im.file) {
        (Some(path), _) => {
            out.push(Command::SampleSong {
                path: path.to_string(),
                count: None,
                bars: None,
                find: None,
                sections: im.places.clone(),
                how,
                every: None,
            });
            im.sending = im.hit.as_ref().map(|h| Sent::Hit(h.id.clone()));
            None
        }
        (None, Some(file)) => {
            im.sending = Some(Sent::File(file.clone()));
            Some(super::Listen {
                file: file.clone(),
                remake: None,
                flip: false,
                rebuild: false,
                sample: true,
                how,
                sections: im.places.clone(),
                every: None,
                groove: false,
                found,
                recording: false,
            })
        }
        (None, None) => None,
    }
}

/// The Sample window, while it is open: the phrases the auto-sampler offers, numbered as they come in the recording and
/// marked on its timeline, each to hear as it was cut and treated and ticked to go into the song either way; how often
/// they come round, another take of them, and the ticked put in the song.
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
    let title = offered
        .as_ref()
        .map_or_else(|| "Its sung phrases".to_string(), |p| format!("{} · its sung phrases", p.recording));
    egui::Window::new(title)
        .id(egui::Id::unique("pieces"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 70.0))
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            let im = &mut cx.view.import;
            let Some(offered) = offered else {
                if im.sample_said.as_ref().is_none_or(|(ok, _)| *ok) {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("the auto-sampler is listening for the phrases it sings best ...");
                    });
                }
                said(ui, &im.sample_said);
                ui.add_space(6.0);
                cancel = ui.button("Cancel").clicked();
                return;
            };
            if im.choices.0 != offered.seed || im.choices.1.len() != offered.pieces.len() {
                // the best two, as they were cut, to start with: the pieces come numbered as they come in the
                // recording, and their scores say which sing best
                let mut best: Vec<usize> = (0..offered.pieces.len()).collect();
                best.sort_by(|a, b| offered.pieces[*b].sung.total_cmp(&offered.pieces[*a].sung));
                let choices = (0..offered.pieces.len())
                    .map(|i| Choice { used: best.iter().take(2).any(|b| *b == i), treated: false });
                im.choices = (offered.seed, choices.collect());
            }
            if ui.small_button("\u{2190} back").on_hover_text("the other ways to put it in").clicked() {
                cancel = true;
            }
            ui.add_space(4.0);
            if let Some(l) = im.offer_heard.clone() {
                let marks: Vec<Mark> = offered
                    .pieces
                    .iter()
                    .zip(&im.choices.1)
                    .filter(|(p, _)| p.to > p.from)
                    .map(|(p, c)| Mark { from: p.from, to: p.to, number: Some(p.id), on: c.used })
                    .collect();
                if let Some(i) = timeline(ui, &l, &marks, None) {
                    let id = marks[i].number;
                    if let Some(c) = offered.pieces.iter().position(|p| Some(p.id) == id) {
                        im.choices.1[c].used = !im.choices.1[c].used;
                    }
                }
                ui.add_space(8.0);
            }
            ui.horizontal(|ui| {
                let n = offered.pieces.len();
                let words =
                    if n == 1 { "1 phrase it sings best".to_string() } else { format!("{n} phrases it sings best") };
                ui.label(RichText::new(words).strong());
                ui.label(RichText::new("numbered as they come in the recording").size(11.5).color(TEXT_FAINT));
            });
            ui.add_space(4.0);
            let hear_now = im.hear_now.take();
            for (i, p) in offered.pieces.iter().enumerate() {
                piece(ui, im, p, i, hear_now, server.as_deref());
                ui.add_space(6.0);
            }
            if let Some(why) = &im.play_said {
                ui.label(RichText::new(format!("could not play it: {why}")).color(ui.visuals().error_fg_color));
            }
            let places = if im.places.is_empty() {
                "where each suits".to_string()
            } else {
                format!("in {}", im.places.join(", "))
            };
            every_row(ui, &mut im.every, &format!(" · {places}"));
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(theme::alpha(theme::GOOD, 0.1))
                .stroke(Stroke::new(1.0, theme::alpha(theme::GOOD, 0.45)))
                .corner_radius(6.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(
                            "Each piece is cut in the quiet before its first word and after its last note dies away: \
                             none starts or ends inside a word.",
                        )
                        .size(12.0)
                        .color(theme::lit(theme::GOOD, 0.35)),
                    );
                });
            ui.add_space(6.0);
            ui.separator();
            let keep: Vec<claudius_api::KeptPiece> = offered
                .pieces
                .iter()
                .zip(&im.choices.1)
                .filter(|(_, c)| c.used)
                .map(|(p, c)| claudius_api::KeptPiece { id: p.id, treated: c.treated })
                .collect();
            ui.horizontal(|ui| {
                let another = ui
                    .add(egui::Button::new("\u{21bb} Other phrases").min_size(vec2(0.0, 30.0)))
                    .on_hover_text("other phrases among the good ones, other treatments");
                if another.clicked() {
                    cx.out.push(Command::RedrawPieces { seed: None });
                    (im.sample_said, im.awaiting) = (None, true);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = match keep.len() {
                        0 => "Put them in the song".to_string(),
                        1 => "Put 1 phrase in the song".to_string(),
                        n => format!("Put {n} phrases in the song"),
                    };
                    let put = primary(ui, &label, !keep.is_empty()).on_disabled_hover_text("tick a phrase first");
                    if put.clicked() {
                        let every = im.every.map(str::to_string);
                        cx.out.push(Command::KeepPieces { keep: keep.clone(), sections: im.places.clone(), every });
                        im.sample_said = None;
                    }
                    if ui.add(egui::Button::new("Cancel").min_size(vec2(0.0, 30.0))).clicked() {
                        cancel = true;
                    }
                });
            });
            said(ui, &im.sample_said);
        });
    let im = &mut cx.view.import;
    // kept, the offer goes, and so does the window, and the one that offered it
    if !open || cancel || im.kept {
        if im.kept {
            im.open = false;
        }
        im.sample_afresh(false);
        if let Some(player) = im.player.as_mut() {
            player.stop();
        }
        (im.hearing, im.span) = (None, None);
    }
}

/// A phrase offered, as a row: ticked to go in, where it is and how sung, to hear as it was cut and treated, and which
/// of the two goes in.
fn piece(ui: &mut Ui, im: &mut Import, p: &PieceView, i: usize, hear_now: Option<(u32, bool)>, server: Option<&str>) {
    let used = im.choices.1[i].used;
    let stroke = if used { Stroke::new(1.0, theme::alpha(ACCENT, 0.9)) } else { Stroke::new(1.0, LINE) };
    let fill = if used { theme::mix(RAISED, ACCENT, 0.08) } else { RAISED.into() };
    egui::Frame::new().fill(fill).stroke(stroke).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.checkbox(&mut im.choices.1[i].used, "").on_hover_text("tick it to put it in the song");
            ui.vertical(|ui| {
                // "bars 12-13, 0:31": the bars go after its section, the time with how long and how sung it is
                let (bars, at) = p.place.split_once(", ").unwrap_or((p.place.as_str(), ""));
                let section = if p.section.is_empty() { String::new() } else { format!("{}, ", capital(&p.section)) };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} · {section}{bars}", p.id)).strong());
                    let bars_long = if p.bars == 1 { "1 bar".to_string() } else { format!("{} bars", p.bars) };
                    let at = if at.is_empty() { String::new() } else { format!("{at} · ") };
                    let words = format!("{at}{bars_long} · {:.0}% sung", p.sung * 100.0);
                    ui.label(RichText::new(words).size(11.5).color(TEXT_DIM));
                });
                if im.choices.1[i].used {
                    ui.horizontal(|ui| {
                        let choice = &mut im.choices.1[i];
                        if widgets::chip(ui, !choice.treated, "as cut", ACCENT)
                            .on_hover_text("as it was cut, cleaned of the recording's drums")
                            .clicked()
                        {
                            choice.treated = false;
                        }
                        // "washed: an echo on the dotted eighth into a long hall" is "washed", the rest on hover
                        let named = p.treatment.split_once(':').map_or(p.treatment.as_str(), |(name, _)| name);
                        if widgets::chip(ui, choice.treated, named, ACCENT)
                            .on_hover_text(format!("through its treatment: {}", p.treatment))
                            .clicked()
                        {
                            choice.treated = true;
                        }
                    });
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (treated, label, url, tip) in [
                    (true, "treated", &p.treated, p.treatment.as_str()),
                    (false, "as cut", &p.raw, "as it was cut, cleaned of the recording's drums"),
                ] {
                    let button = if im.loading(url) {
                        format!("\u{2026} {label}")
                    } else if im.hearing.as_deref() == Some(url.as_str()) {
                        "\u{25a0} stop".to_string()
                    } else {
                        format!("\u{25b6} {label}")
                    };
                    let asked = hear_now == Some((p.id, treated));
                    if ui.add(egui::Button::new(button).min_size(vec2(0.0, 26.0))).on_hover_text(tip).clicked() || asked
                    {
                        im.hear(ui.ctx(), url, &format!("{} {label}", p.id), url, server);
                    }
                }
            });
        });
    });
}

/// the search of the free libraries: words, what they offer, each to hear and to use
fn library(ui: &mut egui::Ui, im: &mut Import, server: Option<&str>) {
    ui.label("Search the free libraries (CC0, public domain, CC BY):");
    ui.horizontal(|ui| {
        let words = ui.add(
            egui::TextEdit::singleline(&mut im.query)
                .hint_text("a soul singer, a jazz drum break")
                .desired_width(300.0),
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
            ui.label(RichText::new(e).color(ui.visuals().error_fg_color));
            return;
        }
    };
    egui::ScrollArea::vertical().max_height(220.0).id_salt("library-hits").show(ui, |ui| {
        for h in &hits {
            ui.horizontal(|ui| {
                let (glyph, tip) = if im.loading(&h.id) {
                    ("\u{2026}", "on its way: it plays as it arrives")
                } else if im.hearing.as_deref() == Some(h.id.as_str()) {
                    ("\u{25a0}", "stop")
                } else {
                    ("\u{25b6}", "hear it")
                };
                if ui.small_button(glyph).on_hover_text(tip).clicked() {
                    im.hear(ui.ctx(), &h.id, &h.title, &h.audio, server);
                }
                let fetching = im.fetching.as_ref().is_some_and(|(f, _, held)| f.id == h.id && !held);
                let chosen = im.hit.as_ref().is_some_and(|c| c.id == h.id);
                let label = if fetching {
                    "fetching"
                } else if chosen {
                    "chosen"
                } else {
                    "Choose"
                };
                let choose = ui.add_enabled(!fetching && !chosen, egui::Button::new(label).small());
                if choose.on_hover_text("hold it here: claudius listens to it, then it goes in as you choose").clicked()
                {
                    im.take_up(ui.ctx(), h, server);
                }
                let secs = h.seconds.map(|s| format!(", {}", clock(s))).unwrap_or_default();
                ui.label(RichText::new(&h.title).strong());
                ui.weak(format!("{}, {}{secs}", h.creator, h.licence)).on_hover_text(&h.page);
            });
        }
    });
    if let Some(why) = im.play_said.as_ref().filter(|_| !im.sampling) {
        ui.label(RichText::new(format!("could not play it: {why}")).color(ui.visuals().error_fg_color));
    }
}

/// what claudius said of the last recording
fn said(ui: &mut egui::Ui, said: &Option<(bool, String)>) {
    if let Some((ok, text)) = said {
        let color = if *ok { ui.visuals().text_color() } else { ui.visuals().error_fg_color };
        ui.label(RichText::new(text).color(color));
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

#[cfg(test)]
mod tests {
    use super::*;
    use claudius_api::{BarHeard, SectionHeard};

    fn heard() -> Listened {
        let bar = |b: usize| BarHeard { start: 1.0 + b as f64 * 2.0, chord: "Am".into(), numeral: "i".into() };
        let section = |start_bar: usize, bars: usize, kind: &str| SectionHeard {
            start_bar,
            bars,
            kind: kind.into(),
            ..Default::default()
        };
        Listened {
            name: "Tranquilize.mp3".into(),
            secs: 222.0,
            bpm: 92.4,
            key: "A minor".into(),
            bars: (0..12).map(bar).collect(),
            end: 25.0,
            sections: vec![
                section(0, 2, "intro"),
                section(2, 4, "verse"),
                section(6, 4, "chorus"),
                section(10, 2, "verse"),
            ],
            phrases: vec![[3.2, 6.9]],
            ..Default::default()
        }
    }

    #[test]
    fn a_recording_heard_says_its_length_tempo_key_and_voice_and_names_its_sections_as_they_come() {
        let l = heard();
        assert_eq!(facts(&l), "3:42 · 92 BPM · A minor · a voice sings in it");
        let names: Vec<String> = sections_of(&l).into_iter().map(|s| s.2).collect();
        assert_eq!(names, ["Intro", "Verse 1", "Chorus", "Verse 2"]);
        let (from, to, _) = &sections_of(&l)[1];
        assert_eq!((*from, *to), (5.0, 13.0));
        assert_eq!(facts(&Listened { phrases: Vec::new(), ..l }), "3:42 · 92 BPM · A minor · no voice heard in it");
    }

    #[test]
    fn what_was_heard_goes_to_the_window_that_asked_and_so_does_the_answer_after_it() {
        let file = |name: &str| Picked { name: name.into(), bytes: vec![1, 2, 3], folder: None };
        let mut im = Import::default();
        im.holds(file("Tranquilize.mp3"));
        assert!(im.listening() && !im.wants("Tranquilize.mp3"), "held, not sent yet");
        assert_eq!(im.next_to_hear().map(|f| f.name.clone()).as_deref(), Some("Tranquilize.mp3"));
        assert!(im.wants("Tranquilize.mp3") && !im.wants("another.mp3") && im.next_to_hear().is_none());
        im.heard_it(Box::new(heard()));
        assert!(!im.wants("Tranquilize.mp3") && !im.listening() && im.heard.is_some(), "what was heard came");
        // the next goes once the answer after it has come, which is the first one's
        im.holds(file("broken.mp3"));
        assert!(im.next_to_hear().is_none(), "it waits for the answer");
        im.hearing_answered(true, "analyzed Tranquilize.mp3");
        assert!(im.heard_failed.is_none() && im.next_to_hear().is_some());
        // one that could not be heard says why, in the window
        im.hearing_answered(false, "cannot decode broken.mp3");
        assert_eq!(im.heard_failed.as_deref(), Some("cannot decode broken.mp3"));
        assert!(!im.listening());
    }

    #[test]
    fn what_was_heard_in_one_held_before_is_the_windows_but_not_shown_and_the_next_goes_after_its_answer() {
        let file = |name: &str| Picked { name: name.into(), bytes: vec![1, 2, 3], folder: None };
        let mut im = Import::default();
        im.holds(file("Tranquilize.mp3"));
        im.next_to_hear();
        // another chosen while the first is heard: what comes of the first stays in the window, unshown, and the
        // other goes once the first is answered
        im.holds(file("another.mp3"));
        assert!(im.listening() && im.next_to_hear().is_none());
        im.heard_it(Box::new(heard()));
        assert!(im.heard.is_none() && im.listening());
        im.hearing_answered(true, "analyzed Tranquilize.mp3");
        assert_eq!(im.next_to_hear().map(|f| f.name.clone()).as_deref(), Some("another.mp3"));
        im.heard_it(Box::new(Listened { name: "another.mp3".into(), ..heard() }));
        assert_eq!(im.heard.as_ref().map(|h| h.name.as_str()), Some("another.mp3"));
        im.hearing_answered(true, "analyzed another.mp3");
        // one held before that could not be heard says nothing of the one held now, which goes next
        im.holds(file("broken.mp3"));
        im.next_to_hear();
        im.holds(file("fine.mp3"));
        im.hearing_answered(false, "cannot decode broken.mp3");
        assert!(im.heard_failed.is_none() && im.listening(), "not the one held");
        assert_eq!(im.next_to_hear().map(|f| f.name.clone()).as_deref(), Some("fine.mp3"));
        // what is heard in a recording of another name, the studio's own, is not the window's
        assert!(!im.wants("Tranquilize.mp3"));
    }

    #[test]
    fn the_sample_windows_timeline_is_the_recording_it_offers_pieces_of() {
        let file = |name: &str| Picked { name: name.into(), bytes: vec![1, 2, 3], folder: None };
        let mut im = Import::default();
        im.holds(file("Tranquilize.mp3"));
        im.next_to_hear();
        // the pieces asked for before claudius has said what it hears: the timeline comes when it does
        im.sample_afresh(true);
        im.heard_it(Box::new(heard()));
        assert_eq!(im.offer_heard.as_ref().map(|h| h.name.as_str()), Some("Tranquilize.mp3"));
        im.hearing_answered(true, "analyzed Tranquilize.mp3");
        im.holds(file("another.mp3"));
        im.next_to_hear();
        im.heard_it(Box::new(Listened { name: "another.mp3".into(), ..heard() }));
        assert_eq!(im.offer_heard.as_ref().map(|h| h.name.as_str()), Some("Tranquilize.mp3"), "another held after");
        im.sample_afresh(false);
        assert!(im.offer_heard.is_none());
    }

    #[test]
    fn a_library_recording_held_by_its_id_is_the_same_one_when_its_audio_arrives_to_be_heard() {
        let hit = LibraryHit {
            id: "freesound:629052".into(),
            title: "Tranquilize".into(),
            audio: "/api/library/audio?id=freesound%3A629052&ext=.mp3".into(),
            ..LibraryHit::default()
        };
        let mut im = Import::default();
        // held at once by its id, as a claudius holds it (`take_up`), and the Sample window opened before its audio
        // has arrived to be heard
        (im.hit, im.open) = (Some(hit.clone()), true);
        im.held_anew(None);
        let (tx, rx) = std::sync::mpsc::channel();
        im.fetching = Some((hit, rx, true));
        im.sample_afresh(true);
        tx.send(Ok(vec![1, 2, 3])).unwrap();
        im.arrivals();
        let file = im.next_to_hear().expect("its audio, to hear");
        im.heard_it(Box::new(Listened { name: file.name.clone(), ..heard() }));
        assert!(im.heard.is_some(), "what was heard in the one held");
        assert_eq!(im.offer_heard.as_ref().map(|h| &h.name), Some(&file.name), "and the Sample window's timeline");
    }
}
