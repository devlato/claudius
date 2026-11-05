//! The demo's sampler. A song built on samples plays recordings, as claudius plays them: a phrase the song is built
//! on, a drum break, and a texture under it, found among the kinds of record its genre digs for (the engine's
//! `GenreSampling`) or for words of one's own. The demo asks the site for them: `samples/search` finds recordings on
//! Freesound, Jamendo and Wikimedia Commons under CC0 or CC BY (through Openverse), and `samples/audio/…` serves them.
//! The site's Worker (`site/worker/index.js`) does the asking, so the browser talks to no one else.
//!
//! The demo only finds and fetches: the engine cuts each recording as the app cuts one (`claudius::sampling::cut`,
//! a phrase of two bars in its key with its chords, out of a whole song if need be), in its Web Worker, and answers
//! with what it kept or why it kept nothing. Natively, `CLAUDIUS_DEMO_SAMPLES` names the address (or a folder);
//! without it, a genre plays what it plays in their place.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use claudius_api::GenreSampling;
use eframe::egui;
use serde::Deserialize;

use super::Rng;

/// the address of a sampler a test answers by hand
#[cfg(test)]
const BY_HAND: &str = "by hand";
/// searches tried for a role, and recordings fetched for each search, before the role goes without
const SEARCHES: usize = 3;
const TRIES: usize = 3;

/// what a sample is in a song
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// the recording the song is built on: its tempo, key and chords become the song's
    Phrase,
    /// a drum break, played at the song's tempo
    Break,
    /// crackle, rain or a room under everything
    Texture,
    /// a note a voice sang, which a part plays on the song's chords where it lifts
    Vocal,
    /// a note a bass played, which the song's bass plays its notes on
    Bass,
}

impl Role {
    /// the engine's name for it (`claudius::library::prepared::Role`)
    pub fn noun(self) -> &'static str {
        match self {
            Role::Phrase => "phrase",
            Role::Break => "break",
            Role::Texture => "texture",
            Role::Vocal => "vocal",
            Role::Bass => "bass",
        }
    }

    fn named(name: &str) -> Option<Role> {
        [Role::Phrase, Role::Break, Role::Texture, Role::Vocal, Role::Bass].into_iter().find(|r| r.noun() == name)
    }
}

/// a recording a search found, as the site's Worker gives it
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Found {
    /// the library's own name for it: `freesound:629052`, `jamendo:582248`, `commons:Stringed_Disco.oga`
    pub id: String,
    /// where it lives: `freesound`, `jamendo`, `wikimedia_audio`
    #[serde(default)]
    pub provider: String,
    pub title: String,
    pub creator: String,
    /// `cc0` or `by`
    pub license: String,
    #[serde(default)]
    pub license_version: String,
    #[serde(default)]
    pub license_url: String,
    /// its page on its library
    pub page: String,
    /// the file, relative to the samples' address
    pub audio: String,
    /// what kind of file it is: `mp3`, `ogg`
    #[serde(default = "ogg")]
    pub ext: String,
    pub seconds: f64,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn ogg() -> String {
    "ogg".into()
}

#[derive(Deserialize)]
struct Results {
    results: Vec<Found>,
}

impl Found {
    /// "CC BY 4.0", "CC0 1.0"
    pub fn licence(&self) -> String {
        match self.license.as_str() {
            "cc0" => "CC0 1.0".into(),
            other => format!("CC {} {}", other.to_uppercase(), self.license_version).trim().to_string(),
        }
    }

    /// as a credit names it: title, author, licence, where it is
    pub fn credit(&self) -> String {
        let licence = if self.license_url.is_empty() {
            self.licence()
        } else {
            format!("{} ({})", self.licence(), self.license_url)
        };
        format!("'{}' by {}, {licence}, {}", self.title, self.creator, self.page)
    }

    /// as the engine knows a recording a library offers (`claudius_packs::online::Found`)
    fn as_engine_has_it(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "source": "openverse",
            "provider": self.provider,
            "title": self.title,
            "creator": self.creator,
            "licence": if self.license == "cc0" { "cc0" } else { "by" },
            "licence_name": self.licence(),
            "licence_url": self.license_url,
            "page": self.page,
            "audio": self.audio,
            "filetype": self.ext,
            "seconds": self.seconds,
            "tags": self.tags,
        })
    }

    fn text(&self) -> String {
        std::iter::once(self.title.as_str()).chain(self.tags.iter().map(|t| t.as_str())).collect::<Vec<_>>().join(" ")
    }
}

/// a title or tag that says the recording was taken from someone's record, as the engine's libraries are read
/// (`claudius_packs::online::suspect`)
const SUSPECT: &[&str] = &[
    "sampled from",
    "sample from",
    "taken from",
    "from the song",
    "from the album",
    "from the record",
    "vinyl rip",
    "record rip",
    "70s sample",
    "80s sample",
    "60s sample",
    "copyrighted",
    "©",
];

fn suspect(f: &Found) -> bool {
    let all = f.text().to_lowercase();
    SUSPECT.iter().any(|s| all.contains(s))
}

/// A search of one's own as the engine keeps it (`claudius::sampling::words`): lower case, single spaces, at most 60
/// characters; none when nothing is left.
pub fn words(search: &str) -> Option<String> {
    let w: String = search.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let w: String = w.chars().take(60).collect();
    let w = w.trim().to_string();
    (!w.is_empty()).then_some(w)
}

/// whether a song is built on samples, as the engine decides (`claudius::sampling::sampled`): as `sampled` says, else
/// as its genre is by default, or when it is given words of its own
pub fn sampled(s: &GenreSampling, sampled: Option<bool>, search: Option<&str>) -> bool {
    sampled.unwrap_or(s.by_default || search.and_then(words).is_some())
}

enum Arrival {
    Search(String, Result<Vec<Found>, String>),
    Audio(String, Result<Arc<Vec<u8>>, String>),
}

/// a role being looked for: what it is searched for, in the order to try, and the lengths of recording it takes
struct Hunt {
    role: Role,
    terms: Vec<String>,
    seconds: (f64, f64),
    term: usize,
    state: Step,
}

enum Step {
    /// waiting for a search's results
    Search(String),
    /// trying what it found, in order
    Try(Vec<Found>, usize),
    /// the recording at the index is with the engine, sent as the id
    Cutting(Vec<Found>, usize, u64),
    Done,
}

/// a recording for the engine to cut: `claudius_session::host::Recording` as JSON, and the file's bytes
pub struct ToCut {
    pub id: u64,
    pub recording: serde_json::Value,
    pub bytes: Arc<Vec<u8>>,
}

/// the samples of a song being gathered
pub struct Gathering {
    genre: String,
    asked: Option<String>,
    hunts: Vec<Hunt>,
    /// what the engine cut, with the recording it came from
    pub kept: Vec<(Role, Found)>,
    to_cut: Vec<ToCut>,
    next: u64,
}

impl Gathering {
    pub fn done(&self) -> bool {
        self.hunts.iter().all(|h| matches!(h.state, Step::Done))
    }

    pub fn progress(&self) -> f32 {
        let done = self.hunts.iter().filter(|h| matches!(h.state, Step::Done)).count();
        done as f32 / self.hunts.len().max(1) as f32
    }

    /// what it is doing, for the studio's status line
    pub fn doing(&self) -> String {
        for h in &self.hunts {
            match &h.state {
                Step::Search(q) => return format!("sampling: looking for a {} ('{q}')", h.role.noun()),
                Step::Try(list, at) => {
                    if let Some(f) = list.get(*at) {
                        return format!("sampling: fetching a {}, '{}' by {}", h.role.noun(), f.title, f.creator);
                    }
                }
                Step::Cutting(list, at, _) => {
                    if let Some(f) = list.get(*at) {
                        return format!("sampling: cutting a {} from '{}'", h.role.noun(), f.title);
                    }
                }
                Step::Done => {}
            }
        }
        "sampling".into()
    }

    #[cfg(test)]
    pub fn kept(&self, role: Role) -> Option<&Found> {
        self.kept.iter().find(|(r, _)| *r == role).map(|(_, f)| f)
    }

    /// the recordings fetched since last asked, for the engine to cut
    pub fn take_cuts(&mut self) -> Vec<ToCut> {
        std::mem::take(&mut self.to_cut)
    }

    /// The engine's answer to the recording sent as `id`: what it cut (`prepared`), or why it cut nothing, in which
    /// case the role tries the next one. Returns what to log.
    pub fn answer(&mut self, id: u64, result: Result<serde_json::Value, String>) -> Vec<String> {
        let mut log = Vec::new();
        for h in self.hunts.iter_mut() {
            let Step::Cutting(list, at, sent) = &h.state else { continue };
            if *sent != id {
                continue;
            }
            let f = list[*at].clone();
            h.state = match result {
                Ok(_) => {
                    log.push(format!("  {}: {}", h.role.noun(), f.credit()));
                    self.kept.push((h.role, f));
                    Step::Done
                }
                Err(why) => {
                    log.push(format!("  passed over '{}' by {}: {why}", f.title, f.creator));
                    match at + 1 {
                        next if next < list.len().min(TRIES) => Step::Try(list.clone(), next),
                        _ => next_search(h, &mut log),
                    }
                }
            };
            break;
        }
        log
    }

    /// stops waiting for what has not come (a browser's fetch waits as long as the server takes); returns what to log
    pub fn give_up(&mut self) -> Vec<String> {
        let mut log = Vec::new();
        for h in self.hunts.iter_mut().filter(|h| !matches!(h.state, Step::Done)) {
            log.push(format!("  no {} in time: the song plays without one", h.role.noun()));
            h.state = Step::Done;
        }
        log
    }
}

/// the hunt's next search, or done with it when it has tried as many as it may
fn next_search(h: &mut Hunt, log: &mut Vec<String>) -> Step {
    h.term += 1;
    match h.terms.get(h.term).filter(|_| h.term < SEARCHES) {
        Some(t) => Step::Search(t.clone()),
        None => {
            log.push(format!("  no {} this time: the song plays without one", h.role.noun()));
            Step::Done
        }
    }
}

/// what a search found that a role can take: under CC0 or CC BY, not cut from someone's record, of a length it takes,
/// in an order `rng` picks
fn fitting(found: &[Found], seconds: (f64, f64), rng: &mut Rng) -> Vec<Found> {
    let mut out: Vec<Found> = found
        .iter()
        .filter(|f| matches!(f.license.as_str(), "cc0" | "by") && !suspect(f) && !f.audio.is_empty())
        .filter(|f| f.seconds >= seconds.0 && f.seconds <= seconds.1)
        .cloned()
        .collect();
    for i in (1..out.len()).rev() {
        out.swap(i, rng.below(i + 1));
    }
    out
}

/// what the demo asks the site for, and what has come back
pub struct Sampler {
    base: Option<String>,
    searched: HashMap<String, Result<Vec<Found>, String>>,
    fetched: HashMap<String, Result<Arc<Vec<u8>>, String>>,
    asked: HashSet<String>,
    tx: Sender<Arrival>,
    rx: Receiver<Arrival>,
    ctx: Option<egui::Context>,
}

fn default_base() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        Some("samples".into())
    } else {
        std::env::var("CLAUDIUS_DEMO_SAMPLES").ok().filter(|s| !s.trim().is_empty())
    }
}

/// a search's words in a query string
fn escape(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "%20".into(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

impl Sampler {
    pub fn new(ctx: Option<egui::Context>) -> Sampler {
        Sampler::at(default_base(), ctx)
    }

    /// a sampler asking the address or folder `base` (none: there are no samples)
    pub fn at(base: Option<String>, ctx: Option<egui::Context>) -> Sampler {
        let (tx, rx) = channel();
        Sampler { base, searched: HashMap::new(), fetched: HashMap::new(), asked: HashSet::new(), tx, rx, ctx }
    }

    /// whether there is anywhere to ask
    pub fn on(&self) -> bool {
        self.base.is_some()
    }

    fn get(&self, path: String, file: String, done: impl FnOnce(Result<Vec<u8>, String>) -> Arrival + Send + 'static) {
        let Some(base) = &self.base else { return };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let deliver = move |a: Arrival| {
            let _ = tx.send(a);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        };
        #[cfg(test)]
        if base == BY_HAND {
            return;
        }
        let base = base.trim_end_matches('/');
        #[cfg(not(target_arch = "wasm32"))]
        if !base.contains("://") {
            let path = format!("{base}/{file}");
            deliver(done(std::fs::read(&path).map_err(|e| format!("{path}: {e}"))));
            return;
        }
        let _ = file;
        let url = format!("{base}/{path}");
        ehttp::fetch(ehttp::Request::get(&url), move |res| {
            let bytes = match res {
                Ok(r) if r.ok => Ok(r.bytes),
                Ok(r) if r.status == 429 || r.status == 503 => Err("the library is busy: try again in a minute".into()),
                Ok(r) => Err(format!("{} {}", r.status, r.status_text)),
                Err(e) => Err(e),
            };
            deliver(done(bytes));
        });
    }

    fn search(&mut self, q: &str) {
        if !self.asked.insert(format!("search:{q}")) {
            return;
        }
        let query = q.to_string();
        // a folder of them names each search's file after its words
        let file = format!("search/{}.json", q.replace(' ', "_"));
        self.get(format!("search?q={}", escape(q)), file, move |bytes| {
            let found = bytes.and_then(|b| {
                serde_json::from_slice::<Results>(&b)
                    .map(|r| r.results)
                    .map_err(|e| format!("the search's answer: {e}"))
            });
            Arrival::Search(query, found)
        });
    }

    fn fetch(&mut self, audio: &str) {
        if !self.asked.insert(format!("audio:{audio}")) {
            return;
        }
        let name = audio.to_string();
        self.get(audio.to_string(), audio.to_string(), move |bytes| Arrival::Audio(name, bytes.map(Arc::new)));
    }

    /// takes what has arrived; true when something has
    pub fn poll(&mut self) -> bool {
        let mut new = false;
        while let Ok(a) = self.rx.try_recv() {
            new = true;
            match a {
                Arrival::Search(q, r) => {
                    self.searched.insert(q, r);
                }
                Arrival::Audio(name, r) => {
                    // the recordings fetched are kept while the demo runs, a few at a time; one let go is asked for
                    // again when a song wants it
                    if self.fetched.len() > 8 {
                        let gone: Vec<String> =
                            self.fetched.iter().filter(|(_, v)| v.is_ok()).map(|(k, _)| k.clone()).collect();
                        for k in gone {
                            self.fetched.remove(&k);
                            self.asked.remove(&format!("audio:{k}"));
                        }
                    }
                    self.fetched.insert(name, r);
                }
            }
        }
        new
    }

    /// a sampler that asks no one: a test answers what it asks (`pending`)
    #[cfg(test)]
    pub fn by_hand() -> Sampler {
        Sampler::at(Some(BY_HAND.into()), None)
    }

    /// what it has asked for and not had: `search:<words>` and `audio:<path>`
    #[cfg(test)]
    pub fn pending(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .asked
            .iter()
            .filter(|a| match (a.strip_prefix("search:"), a.strip_prefix("audio:")) {
                (Some(q), _) => !self.searched.contains_key(q),
                (_, Some(p)) => !self.fetched.contains_key(p),
                _ => false,
            })
            .cloned()
            .collect();
        out.sort();
        out
    }

    /// a search's results arriving, as the site would send them
    #[cfg(test)]
    pub fn arrive_search(&self, q: &str, found: Result<Vec<Found>, String>) {
        let _ = self.tx.send(Arrival::Search(q.to_string(), found));
    }

    /// a recording's file arriving
    #[cfg(test)]
    pub fn arrive_audio(&self, audio: &str, bytes: Result<Vec<u8>, String>) {
        let _ = self.tx.send(Arrival::Audio(audio.to_string(), bytes.map(Arc::new)));
    }

    /// Starts gathering what a song of `genre` is built on, as `sampling` says it samples: each role's searches in an
    /// order `rng` picks, the phrase's for the words `asked` when there are any. What failed before (a busy library, a
    /// recording that did not come) is asked for again.
    pub fn start(&mut self, genre: &str, sampling: &GenreSampling, asked: Option<&str>, rng: &mut Rng) -> Gathering {
        let failed: Vec<String> = self
            .searched
            .iter()
            .filter(|(_, r)| r.is_err())
            .map(|(q, _)| format!("search:{q}"))
            .chain(self.fetched.iter().filter(|(_, r)| r.is_err()).map(|(a, _)| format!("audio:{a}")))
            .collect();
        self.searched.retain(|_, r| r.is_ok());
        self.fetched.retain(|_, r| r.is_ok());
        for k in failed {
            self.asked.remove(&k);
        }
        let asked = asked.and_then(words);
        let hunts = sampling
            .roles
            .iter()
            .filter_map(|r| {
                let role = Role::named(&r.role)?;
                let mut terms = match (&asked, role) {
                    (Some(w), Role::Phrase) => vec![w.clone()],
                    _ => r.search.clone(),
                };
                for i in (1..terms.len()).rev() {
                    terms.swap(i, rng.below(i + 1));
                }
                let state = terms.first().map_or(Step::Done, |t| Step::Search(t.clone()));
                Some(Hunt { role, terms, seconds: r.seconds, term: 0, state })
            })
            .collect();
        Gathering { genre: genre.to_string(), asked, hunts, kept: Vec::new(), to_cut: Vec::new(), next: 1 }
    }

    /// moves the gathering on as far as what has arrived allows; returns what to log
    pub fn advance(&mut self, g: &mut Gathering, rng: &mut Rng) -> Vec<String> {
        let mut log = Vec::new();
        for i in 0..g.hunts.len() {
            loop {
                let role = g.hunts[i].role;
                let next = match &g.hunts[i].state {
                    Step::Done | Step::Cutting(..) => break,
                    Step::Search(q) => match self.searched.get(q) {
                        None => {
                            let q = q.clone();
                            self.search(&q);
                            break;
                        }
                        Some(Err(e)) => {
                            log.push(format!("  no {} from the search for '{q}': {e}", role.noun()));
                            None
                        }
                        Some(Ok(found)) => {
                            let list = fitting(found, g.hunts[i].seconds, rng);
                            if list.is_empty() {
                                log.push(format!("  no {} the demo can use in the search for '{q}'", role.noun()));
                                None
                            } else {
                                Some(Step::Try(list, 0))
                            }
                        }
                    },
                    Step::Try(list, at) => {
                        let f = &list[*at];
                        match self.fetched.get(&f.audio) {
                            None => {
                                let audio = f.audio.clone();
                                self.fetch(&audio);
                                break;
                            }
                            Some(Ok(bytes)) => {
                                let id = g.next;
                                g.next += 1;
                                let recording = serde_json::json!({
                                    "role": role.noun(),
                                    "genre": g.genre,
                                    "asked": g.asked.as_ref().filter(|_| role == Role::Phrase),
                                    "found": f.as_engine_has_it(),
                                    "ext": f.ext,
                                });
                                g.to_cut.push(ToCut { id, recording, bytes: bytes.clone() });
                                Some(Step::Cutting(list.clone(), *at, id))
                            }
                            Some(Err(why)) => {
                                log.push(format!("  passed over '{}' by {}: {why}", f.title, f.creator));
                                let at = at + 1;
                                (at < list.len().min(TRIES)).then(|| Step::Try(list.clone(), at))
                            }
                        }
                    }
                };
                let h = &mut g.hunts[i];
                h.state = match next {
                    Some(s) => s,
                    None => next_search(h, &mut log),
                };
            }
        }
        log
    }
}

#[cfg(test)]
pub mod tests {
    use claudius_api::SampleRoleInfo;

    use super::*;

    pub fn found(id: &str, title: &str, secs: f64, tags: &[&str]) -> Found {
        Found {
            id: format!("freesound:{id}"),
            provider: "freesound".into(),
            title: title.into(),
            creator: "someone".into(),
            license: "by".into(),
            license_version: "4.0".into(),
            license_url: "https://creativecommons.org/licenses/by/4.0/".into(),
            page: format!("https://freesound.org/s/{id}/"),
            audio: format!("audio/1/{id}_1.ogg"),
            ext: "wav".into(),
            seconds: secs,
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    /// what a genre samples, as the engine says: a phrase, a break and a texture, each searched for one way
    pub fn sampling() -> GenreSampling {
        let role = |role: &str, search: &str, seconds: (f64, f64)| SampleRoleInfo {
            role: role.into(),
            search: vec![search.into()],
            seconds,
        };
        GenreSampling {
            by_default: true,
            roles: vec![
                role("phrase", "film score", (6.0, 900.0)),
                role("break", "funk drum break", (2.0, 60.0)),
                role("texture", "vinyl crackle", (5.0, 600.0)),
            ],
        }
    }

    /// a gathering fed by hand: a search finds `list(words)`, each recording is some bytes, and the engine cuts what
    /// `cuts(it)` says
    fn gather(
        asked: Option<&str>,
        list: impl Fn(&str) -> Vec<Found>,
        cuts: impl Fn(&serde_json::Value) -> Result<(), String>,
    ) -> (Gathering, Vec<serde_json::Value>) {
        let mut s = Sampler::by_hand();
        let mut rng = Rng::new(5);
        let mut g = s.start("trip_hop", &sampling(), asked, &mut rng);
        let mut sent = Vec::new();
        for _ in 0..20 {
            s.advance(&mut g, &mut rng);
            for asked in s.pending() {
                if let Some(q) = asked.strip_prefix("search:") {
                    s.arrive_search(q, Ok(list(q)));
                } else if let Some(a) = asked.strip_prefix("audio:") {
                    s.arrive_audio(a, Ok(a.as_bytes().to_vec()));
                }
            }
            s.poll();
            for c in g.take_cuts() {
                let answer = cuts(&c.recording).map(|()| serde_json::json!({ "path": "demo:x" }));
                sent.push(c.recording);
                g.answer(c.id, answer);
            }
            if g.done() {
                break;
            }
        }
        (g, sent)
    }

    #[test]
    fn a_gathering_hands_the_engine_what_it_fetched_and_keeps_what_the_engine_cut() {
        let list = |q: &str| match q {
            "film score" => vec![found("10", "Main Titles", 184.0, &[]), found("11", "Hush", 2.0, &[])],
            "funk drum break" => vec![found("12", "drums 92 bpm", 2.6, &[])],
            _ => vec![found("13", "Vinyl Crackle Analogue", 6.0, &[])],
        };
        let (g, sent) = gather(None, list, |_| Ok(()));
        assert!(g.done());
        assert_eq!(g.kept(Role::Phrase).map(|f| f.title.as_str()), Some("Main Titles"), "a whole recording");
        assert_eq!(g.kept(Role::Break).map(|f| f.id.as_str()), Some("freesound:12"));
        assert_eq!(g.kept(Role::Texture).map(|f| f.id.as_str()), Some("freesound:13"));
        // each as the engine's Recording: its role, the song's genre, the recording as the engine knows one
        let phrase = sent.iter().find(|r| r["role"] == "phrase").unwrap();
        assert_eq!((phrase["genre"].as_str(), phrase["ext"].as_str()), (Some("trip_hop"), Some("wav")));
        let f = &phrase["found"];
        assert_eq!((f["id"].as_str(), f["licence"].as_str()), (Some("freesound:10"), Some("by")));
        assert_eq!(f["licence_name"].as_str(), Some("CC BY 4.0"));
        assert!(sent.iter().all(|r| r["asked"].is_null()));
    }

    #[test]
    fn words_of_ones_own_are_what_the_phrase_is_searched_for() {
        let (g, sent) = gather(Some("  70s   Soul "), |q| vec![found(&q.len().to_string(), q, 30.0, &[])], |_| Ok(()));
        assert_eq!(g.kept(Role::Phrase).map(|f| f.title.as_str()), Some("70s soul"));
        assert_eq!(g.kept(Role::Break).map(|f| f.title.as_str()), Some("funk drum break"), "the genre's own");
        let phrase = sent.iter().find(|r| r["role"] == "phrase").unwrap();
        assert_eq!(phrase["asked"].as_str(), Some("70s soul"));
        assert!(sent.iter().filter(|r| r["role"] != "phrase").all(|r| r["asked"].is_null()));
    }

    #[test]
    fn a_recording_the_engine_cuts_nothing_from_makes_way_for_the_next() {
        let list = |q: &str| match q {
            "film score" => vec![found("20", "Rain Ambience", 60.0, &[]), found("21", "Strings", 40.0, &[])],
            _ => Vec::new(),
        };
        // the engine hears no bars in the first it is sent
        let first = std::cell::Cell::new(true);
        let (g, sent) =
            gather(None, list, |_| if first.replace(false) { Err("no bars to cut it on".into()) } else { Ok(()) });
        assert!(g.done());
        let phrases: Vec<&serde_json::Value> = sent.iter().filter(|r| r["role"] == "phrase").collect();
        assert_eq!(phrases.len(), 2);
        let second = phrases[1]["found"]["title"].as_str();
        assert_eq!(g.kept(Role::Phrase).map(|f| f.title.as_str()), second, "the next one it found");
        assert!(g.kept(Role::Break).is_none() && g.kept(Role::Texture).is_none(), "nothing found for them");
    }

    #[test]
    fn what_a_role_cannot_take_is_not_fetched() {
        let mut rng = Rng::new(1);
        let list = [
            found("30", "too short", 1.0, &[]),
            Found { license: "by-nc".into(), ..found("31", "not for publishing", 30.0, &[]) },
            found("32", "piano sampled from an old record", 30.0, &[]),
            found("33", "a whole song", 200.0, &[]),
        ];
        let ids: Vec<String> = fitting(&list, (6.0, 900.0), &mut rng).into_iter().map(|f| f.id).collect();
        assert_eq!(ids, ["freesound:33"]);
    }

    #[test]
    fn a_song_is_sampled_as_the_engine_decides() {
        let s = GenreSampling { by_default: false, ..sampling() };
        assert!(!sampled(&s, None, None) && sampled(&s, None, Some("disco")) && sampled(&s, Some(true), None));
        assert!(!sampled(&sampling(), Some(false), Some("disco")) && sampled(&sampling(), None, None));
        assert!(!sampled(&s, None, Some("   ")));
    }

    #[test]
    fn a_recording_let_go_of_is_asked_for_again() {
        let mut s = Sampler::by_hand();
        for i in 0..12 {
            let audio = format!("audio/1/{i}_1.ogg");
            s.fetch(&audio);
            s.arrive_audio(&audio, Ok(vec![0; 10]));
            s.poll();
        }
        assert!(s.fetched.len() <= 9, "a few are kept: {}", s.fetched.len());
        let gone = (0..12).map(|i| format!("audio/1/{i}_1.ogg")).find(|a| !s.fetched.contains_key(a)).unwrap();
        s.fetch(&gone);
        assert_eq!(s.pending(), [format!("audio:{gone}")], "asked for again, not waited for forever");
    }

    #[test]
    fn a_search_that_failed_is_asked_again_for_the_next_song() {
        let mut s = Sampler::by_hand();
        let mut rng = Rng::new(5);
        let mut g = s.start("trip_hop", &sampling(), None, &mut rng);
        s.advance(&mut g, &mut rng);
        for q in ["film score", "funk drum break", "vinyl crackle"] {
            s.arrive_search(q, Err("the library is busy: try again in a minute".into()));
        }
        s.poll();
        s.advance(&mut g, &mut rng);
        assert!(g.done() && s.pending().is_empty());
        let mut again = s.start("trip_hop", &sampling(), None, &mut rng);
        s.advance(&mut again, &mut rng);
        assert_eq!(s.pending().len(), 3, "each search asked again: {:?}", s.pending());
    }

    #[test]
    fn a_gathering_that_waits_too_long_gives_up_on_what_has_not_come() {
        let mut s = Sampler::by_hand();
        let mut rng = Rng::new(5);
        let mut g = s.start("trip_hop", &sampling(), None, &mut rng);
        s.advance(&mut g, &mut rng);
        assert!(!g.done());
        let log = g.give_up();
        assert!(g.done() && g.kept.is_empty() && log.len() == 3, "{log:?}");
    }

    #[test]
    fn a_search_that_fails_leaves_its_role_without_a_sample() {
        let mut s = Sampler::at(None, None);
        let mut rng = Rng::new(5);
        let mut g = s.start("trip_hop", &sampling(), None, &mut rng);
        for q in ["film score", "funk drum break", "vinyl crackle"] {
            s.arrive_search(q, Err("the library is busy: try again in a minute".into()));
        }
        s.poll();
        let log = s.advance(&mut g, &mut rng);
        assert!(g.done() && g.kept.is_empty(), "{log:?}");
        assert!(log.iter().any(|l| l.contains("busy")), "{log:?}");
        assert!(log.iter().any(|l| l.contains("no break this time")), "{log:?}");
    }
}
