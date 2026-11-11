//! The wire contract between the claudius server and its clients (the egui UI, native and web, and the MCP tools),
//! and the reader of the settings files any claudius wrote (`settings`). Plain serde types only, so this crate builds
//! for wasm32 too.
//!
//! HTTP routes served by the app (default `http://127.0.0.1:7878`):
//!
//! | route | |
//! |---|---|
//! | `GET  /api/state?since=N` | `State`. With `since`, long-polls: answers as soon as `version > N` (or after ~20 s with the current state) |
//! | `POST /api/command` | body: a `Command` (JSON, tagged by `"cmd"`), answer: `Reply`. Long jobs (render, mutate, evolve) answer at once and report through `State.busy` / `State.log` |
//! | `GET  /api/script` | the current song as a script (text/plain) |
//! | `GET  /api/sounds` | `SoundList`: every sound name a script or the UI may use |
//! | `GET  /api/find?q=&limit=` | `claudius_intent::Answer`: the studio's actions the words find, by name and by meaning |
//! | `POST /api/hear` | body: what was said to the palette, a WAV file (`Content-Type: audio/wav`), at most 30 s; answer: `Heard` |
//! | `GET  /api/audio/{version}.wav` / `.mp3` | the render of that state version (`RenderView.url` points here) |
//! | `GET  /api/stems/{version}/{track}.wav` | one track of that render, when it was rendered with stems |
//! | `GET  /api/export/{version}/{name}.{ext}?format=SPEC` | that render in another format (`ExportFormat.spec`), as a download named `{name}.{ext}` |
//! | `POST /api/release/artwork` | body: a JPEG or PNG (`Content-Type: image/jpeg`, `image/png` or `application/octet-stream`), the release's artwork; answer: `Reply` |
//! | `GET  /api/release/artwork` | the release's artwork, as it was given |
//! | `GET  /api/release/preview.png` | the artwork at most 512 pixels on a side, as PNG |
//! | `POST /mcp` | the MCP endpoint (Streamable HTTP) |
//! | `GET  /` | the web UI |

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub mod examples;
pub mod settings;
pub mod words;

/// Drum step velocities by name, as step strings write them (`X` accent, `x` hit, `o` ghost) and the studio shows
/// them: an accent from `ACCENT`, a ghost below `GHOST`. The engine's step strings use the same values.
pub const ACCENT: f32 = 0.85;
pub const GHOST: f32 = 0.5;

/// the step-string character for a velocity (`.` for none)
pub fn step_char(v: f32) -> char {
    if v <= 0.0 {
        '.'
    } else if v >= ACCENT {
        'X'
    } else if v >= GHOST {
        'x'
    } else {
        'o'
    }
}

/// the velocity a step-string character plays: `X` 1.0, `x` 0.8, `o` 0.45, `1`-`9` tenths, rests 0; None for
/// anything else
pub fn step_velocity(c: char) -> Option<f32> {
    match c {
        'X' => Some(1.0),
        'x' => Some(0.8),
        'o' => Some(0.45),
        '1'..='9' => c.to_digit(10).map(|d| d as f32 / 10.0),
        '.' | '-' | '~' | '_' => Some(0.0),
        _ => None,
    }
}

pub const DEFAULT_PORT: u16 = 7878;

/// The `RenderView::url` of a preview: a render of the song's first bars only, which the browser's demo plays while
/// the whole song renders. It is no render of the song as it is.
pub const PREVIEW_URL: &str = "preview:";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// increases on every change of anything below
    pub version: u64,
    /// the version of the last change to the song itself: the render is current when `render.version >= song_version`
    #[serde(default)]
    pub song_version: u64,
    pub song: Option<SongView>,
    /// the latest finished render of the current song (absent until the first render)
    pub render: Option<RenderView>,
    pub verdict: Option<VerdictView>,
    /// what the engine is busy with ("rendering", "evolving: generation 2 of 5"), None when idle
    pub busy: Option<String>,
    /// 0..1 progress of the busy job when known
    pub progress: Option<f32>,
    /// whether what `busy` says is a job that `Command::Cancel` stops (a render, a mutation, evolution, a download),
    /// and not a step of listening to a recording, which runs to its end
    #[serde(default)]
    pub cancellable: bool,
    /// earshot's models are still loading: until then verdicts come from the meters and music theory only
    #[serde(default)]
    pub ears_loading: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// how many edits undo can take back, one at a time, and redo can bring back
    #[serde(default)]
    pub undo_steps: usize,
    #[serde(default)]
    pub redo_steps: usize,
    /// what undo / redo would take back or bring back ("tempo 124", "mutated: lead: rewrote the part")
    #[serde(default)]
    pub undo_label: Option<String>,
    #[serde(default)]
    pub redo_label: Option<String>,
    /// the changes made to the song, the latest first (at most 200): edits, undos, redos and goings back, as its
    /// history keeps them
    #[serde(default)]
    pub history: Vec<Done>,
    /// recent messages, oldest first (at most ~200)
    pub log: Vec<String>,
    pub genres: Vec<GenreInfo>,
    /// the formats a render can be exported to (`/api/export`, `Command::Export`)
    #[serde(default)]
    pub exports: Vec<ExportFormat>,
    /// the release the session is preparing for the open song (`Command::SetRelease`, `Command::WriteKit`)
    #[serde(default)]
    pub release: Option<ReleaseView>,
    /// the last take someone asked the person to record (`Command::RequestTake`), and how it went
    #[serde(default)]
    pub take_request: Option<TakeRequest>,
    /// the chat between the person and the assistants connected to claudius
    #[serde(default)]
    pub chat: ChatView,
    /// the last pitch check asked of the person (`request_pitch_check`), and how it went
    #[serde(default)]
    pub pitch_check: Option<PitchCheck>,
    /// the person's voice as their last pitch check heard it, in words; a tuner sings it as it heard it
    #[serde(default)]
    pub voice: Option<String>,
    /// the page a phone plays along on, while it is served (`POST /api/phone`)
    #[serde(default)]
    pub phone: Option<PhoneView>,
    /// the pieces the auto-sampler offers from a recording, until some are kept or another offer replaces them
    #[serde(default)]
    pub pieces: Option<PiecesView>,
}

/// The pieces the auto-sampler offers from a recording before any goes into the song (`sample_pieces`), each as it was
/// cut and as treated, to hear side by side: what the Sample window shows.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PiecesView {
    /// the recording, as it was named
    pub recording: String,
    /// the take: 0 for the best phrases, any other for another draw of them (`redraw_pieces`)
    pub seed: u64,
    pub pieces: Vec<PieceView>,
}

/// a piece the auto-sampler offers
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PieceView {
    /// what `keep_pieces` names it by: 1, 2 ...
    pub id: u32,
    /// where it is in the recording, in words ("bars 12-13, 0:31"), how many bars it lasts, and how much of it is sung
    pub place: String,
    pub bars: u32,
    pub sung: f64,
    /// where to hear it, as it was cut and as treated: paths on the claudius that offers it
    pub raw: String,
    pub treated: String,
    /// the treatment, in words
    pub treatment: String,
}

/// a piece to keep (`keep_pieces`): as it was cut, or `treated`
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct KeptPiece {
    pub id: u32,
    #[serde(default)]
    pub treated: bool,
}

/// The studio's transport, as it tells the server (`POST /api/transport`) so that a phone playing along knows where the
/// song is: whether it plays, and where it is, in seconds from its start.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Transport {
    pub playing: bool,
    pub at: f64,
}

/// A page with pads, keys and a turntable for a phone on the same network (`POST /api/phone`): its address, with the
/// key that opens it, which the studio shows as a QR code, and how many phones have it open.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PhoneView {
    pub url: String,
    #[serde(default)]
    pub phones: u32,
    /// the song played or stopped from a phone, which the studio does once (`PlayAsk::id`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play: Option<PlayAsk>,
}

/// A phone asked the studio to play the song from `at` seconds (from where it is, when none), or to stop it. Each ask has
/// an `id` of its own, so the studio does each once, and not one made before it opened.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PlayAsk {
    pub id: u64,
    pub playing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
}

/// A take someone (an assistant, say) asked the person to record. The studio's Record window opens for it, and the take
/// sent for it (`POST /api/takes?request=`, or `add_notes` with `request`) answers it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TakeRequest {
    pub id: u64,
    /// the section the take starts with
    pub from: String,
    /// what it is, for the mix (`vocal`, `lead` ...), as for `add_take`
    #[serde(default)]
    pub role: Option<String>,
    /// the track's name, when one was asked for
    #[serde(default)]
    pub name: Option<String>,
    /// what to record, in the asker's words ("the chorus, sung softly")
    #[serde(default)]
    pub note: String,
    pub status: TakeStatus,
    /// what claudius said of the take sent for it
    #[serde(default)]
    pub answer: Option<String>,
}

/// The chat between the person, in the studio, and the assistants connected to claudius over MCP: what each said, in
/// order (`CHAT_SHOWN` of it at most), and who listens for what the person writes now (`wait_for_message`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ChatView {
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    /// the assistant listening, as its client calls itself ("Claude Code"), until when, in ms since 1970
    #[serde(default)]
    pub listening: Option<String>,
    #[serde(default)]
    pub listening_until: Option<u64>,
    /// the agent claudius started itself (one of `CHAT_AGENTS`), while it runs
    #[serde(default)]
    pub agent: Option<String>,
    /// the message it is still writing (the last), which grows until it is whole
    #[serde(default)]
    pub writing: Option<u64>,
    /// whether this claudius starts agents (`POST /api/chat/agent`): one serving others' files does not
    #[serde(default)]
    pub starts_agents: bool,
}

/// the most messages of the chat a state holds, the latest
pub const CHAT_SHOWN: usize = 200;

/// the agents claudius starts itself over the Agent Client Protocol, for the chat: an id, and what the chat calls it
pub const CHAT_AGENTS: [(&str, &str); 3] = [("claude", "Claude Code"), ("gemini", "Gemini CLI"), ("codex", "Codex")];

/// `POST /api/chat/agent`: start the agent `agent` (an id of `CHAT_AGENTS`) for the chat
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StartAgent {
    pub agent: String,
}

/// a message of the chat: its id (each one more than the one before it), who said it (none for the person), what,
/// when (in ms since 1970), and the message it answers
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub id: u64,
    #[serde(default)]
    pub from: Option<String>,
    pub text: String,
    pub at: u64,
    #[serde(default)]
    pub reply_to: Option<u64>,
    /// a line about what an agent claudius started does (a tool it uses, that it stopped), not something it says
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub note: bool,
}

/// A pitch check (`request_pitch_check`): phrases the studio plays for the person to sing back, each followed by a gap
/// to answer it in, in whatever octave suits them. The recording sent for it (`POST /api/pitch-check?id=`) answers it,
/// and the voice claudius hears in it becomes the person's (`State.voice`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PitchCheck {
    pub id: u64,
    pub bpm: f64,
    /// the notes played: `t` and `d` in beats from the check's start, `p` a MIDI pitch
    pub notes: Vec<NoteView>,
    /// where each phrase's answer goes, (first beat, last beat)
    pub answers: Vec<(f64, f64)>,
    /// how long it lasts, beats
    pub beats: f64,
    /// what the asker said, in their words
    #[serde(default)]
    pub note: String,
    pub status: TakeStatus,
    /// what claudius heard in the answers
    #[serde(default)]
    pub answer: Option<String>,
}

/// how a take asked for went
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TakeStatus {
    /// the person has yet to send one
    #[default]
    Waiting,
    /// a take was sent for it, and claudius kept it
    Taken,
    /// the person will not record it
    Cancelled,
}

/// What stores and distributors ask about a release, as a song's release.json keeps it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ReleaseDetails {
    /// the song's title when empty
    pub title: String,
    /// a version of the song, such as "Extended Mix"
    pub version: String,
    pub artist: String,
    pub featuring: Vec<String>,
    /// the album or EP the song is on; a single when empty
    pub album: String,
    pub genre: String,
    pub subgenre: String,
    /// YYYY-MM-DD
    pub date: String,
    /// the record label: the artist when empty
    pub label: String,
    /// the songwriters' full legal names
    pub writers: Vec<String>,
    pub producers: Vec<String>,
    /// who owns the recording and the composition, for the ℗ and © lines: the artist when empty
    pub owner: String,
    pub isrc: String,
    pub upc: String,
    pub explicit: bool,
    /// how the song was made, which distributors ask: `written`, `assisted` or `generated`; None until it is said
    pub made: Option<String>,
}

/// one check of a release, as a store or a distributor would make it
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReleaseCheck {
    /// `problem` (a store would turn the release down), `warning` or `note`
    pub level: String,
    /// `artwork`, `audio`, `sounds` or `details`
    pub about: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReleaseView {
    pub details: ReleaseDetails,
    /// the song's own title, which a title in the details replaces
    pub song_title: String,
    /// what the artwork is ("3000 × 3000 JPEG"), or why it cannot be read; None without artwork
    pub artwork: Option<String>,
    /// changes with every new artwork, for `/api/release/preview.png?v=`
    pub artwork_version: u64,
    /// what stores would say of the release with the render `checked` (problems first)
    pub checks: Vec<ReleaseCheck>,
    /// the render the checks are of; 0 until one is checked
    pub checked: u64,
    /// nothing a store would turn it down for
    pub ready: bool,
    /// the folder the last release kit went to
    pub kit: Option<String>,
    /// what `made` may be, with what each means
    pub made_options: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportFormat {
    /// what to ask for: `mp3`, `flac:16`, `opus:96`
    pub spec: String,
    /// what to call it: `MP3, 320 kbps`
    pub label: String,
    pub extension: String,
    pub lossless: bool,
    /// why this machine cannot write it (AAC and Apple Lossless need Core Audio or ffmpeg); None when it can
    #[serde(default)]
    pub unavailable: Option<String>,
}

/// A recording a free online library offers (CC0, public domain or CC BY), as a search lists it
/// (`GET /api/library/search`): what the Add a recording window shows, to hear and to use.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LibraryHit {
    /// `freesound:629052`, `commons:Swanee 1920.ogg`, `ccmixter:71182`: a command takes it as `library:<id>`
    pub id: String,
    pub title: String,
    pub creator: String,
    /// the licence as the library states it: "CC0 1.0", "CC BY 4.0"
    pub licence: String,
    #[serde(default)]
    pub seconds: Option<f64>,
    /// where a person reads about it
    pub page: String,
    /// where its audio is fetched: a path on the claudius that found it, or a URL of the demo's site
    pub audio: String,
    /// what a song that plays it credits
    pub credit: String,
    /// the library's whole record of it, which the demo's engine credits it by (a claudius recalls it by its id)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found: Option<serde_json::Value>,
}

/// A change in the song's history (`State::history`), as its log keeps it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Done {
    /// what `Command::GoBack` names it by: the replica that made it and its time there (`12@4f2a9c81d0e3b7a6`)
    pub id: String,
    /// `edit`, `undo`, `redo`, `back` (a going back to before a change), or `moved` (files the song plays moved, as a
    /// save copies its takes into its folder)
    pub kind: String,
    /// what undo calls it ("tempo 124"), or the change undone or redone
    pub label: String,
    /// what it changed, in words ("added the whistle track, playing the synth whistle")
    pub said: String,
    /// when, in milliseconds since 1970
    pub at: u64,
    /// whether `Command::GoBack` can take the song back to before it: an edit undo can still take back
    #[serde(default)]
    pub can_go_back: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GenreInfo {
    pub name: String,
    /// the family it belongs to ("hip hop", "house"), which the studio groups genres under
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
    /// what else it is called, and the eras, scenes and artists it is known by ("90s hip hop", "golden age")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub description: String,
    pub bpm: (f64, f64),
    /// what its songs built on samples dig for (none: a genre of one's own that says nothing of it)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<GenreSampling>,
    /// how layered its songs are when nobody says (`new_song`'s `complexity`); none: 0.5, as the genre is written
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<f64>,
}

/// What a genre's songs built on samples dig for in free libraries (`new_song`'s `sampled`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GenreSampling {
    /// whether its songs are built on samples unless asked not to be (trip-hop's are)
    pub by_default: bool,
    /// each role it samples, in the order it gathers them
    pub roles: Vec<SampleRoleInfo>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SampleRoleInfo {
    /// `phrase`, `break`, `texture` or `vocal`
    pub role: String,
    /// the kinds of record it is searched for ("disco", "gospel choir"), a song trying them in an order its seed picks
    pub search: Vec<String>,
    /// the shortest and longest recording worth fetching, in seconds
    pub seconds: (f64, f64),
}

/// The genres that answer `query`, best first: each of its words begins a word of a genre's name, one of its other
/// names (`aliases`), its family or its description, or a run of them typed together ("hiphop" finds hip hop; "rap"
/// finds cloud rap, not trap). One that the whole query names, or names otherwise, comes first; then those whose name
/// or other names hold its words, those whose family does, and those whose description does. An empty query answers
/// every genre.
pub fn search_genres<'a>(genres: &'a [GenreInfo], query: &str) -> Vec<&'a GenreInfo> {
    let asked = search_words(query);
    if asked.is_empty() {
        return genres.iter().collect();
    }
    let whole = asked.concat();
    let mut found: Vec<(u32, &GenreInfo)> = genres
        .iter()
        .filter_map(|g| {
            let name = search_words(&g.name);
            let aliases: Vec<Vec<String>> = g.aliases.iter().map(|a| search_words(a)).collect();
            let family = search_words(&g.family);
            let description = search_words(&g.description);
            let named = name.concat() == whole || aliases.iter().any(|a| a.concat() == whole);
            let mut score = if named {
                100
            } else if family.concat() == whole {
                20
            } else {
                0
            };
            for w in &asked {
                let weight = if begins(&name, w) {
                    8
                } else if aliases.iter().any(|a| begins(a, w)) {
                    6
                } else if begins(&family, w) {
                    4
                } else {
                    u32::from(begins(&description, w))
                };
                if weight == 0 && !named {
                    return None;
                }
                score += weight;
            }
            Some((score, g))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    found.into_iter().map(|(_, g)| g).collect()
}

/// the words of `s` as a genre search compares them: lowercase letters and digits, `&` as `n` (R&B is rnb), an
/// apostrophe dropped (d'angelo is dangelo)
/// The genres a person's words name outright, in the order they come, each with where in `search_words(text)` it is
/// named: a run of up to four words that is a genre's name or one of its other names ("lo-fi", "trip hop", "90s hip
/// hop"), the longest where runs overlap. "a chill lo-fi beat" names lofi in words 2 and 3; "trip hop with idm drums"
/// names trip hop, then idm.
pub fn genres_named<'a>(genres: &'a [GenreInfo], text: &str) -> Vec<(&'a GenreInfo, std::ops::Range<usize>)> {
    let words = search_words(text);
    let names: Vec<(String, &GenreInfo)> = genres
        .iter()
        .flat_map(|g| std::iter::once(&g.name).chain(&g.aliases).map(move |n| (search_words(n).concat(), g)))
        .filter(|(n, _)| !n.is_empty())
        .collect();
    let mut found: Vec<(&GenreInfo, std::ops::Range<usize>)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let hit = (1..=4.min(words.len() - i)).rev().find_map(|n| {
            let run = words[i..i + n].concat();
            names.iter().find(|(name, _)| *name == run).map(|(_, g)| (n, *g))
        });
        match hit {
            Some((n, g)) => {
                if !found.iter().any(|(f, _)| f.name == g.name) {
                    found.push((g, i..i + n));
                }
                i += n;
            }
            None => i += 1,
        }
    }
    found
}

/// the words a genre search reads in `s`: lower case, split at anything not a letter or a digit
pub fn search_words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .replace('&', "n")
        .replace(['\'', '’'], "")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// whether `w` begins one of `words`, or a run of them typed together ("hiphop" in "alt hip hop")
fn begins(words: &[String], w: &str) -> bool {
    (0..words.len()).any(|i| words[i..].concat().starts_with(w))
}

/// What the demo's page asks its engine to do with a recording the person chose, which comes with the file's bytes:
/// as the studio asks the app over HTTP (`/api/analyze`, `/api/remake`, `/api/sample`, `/api/groove`)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum Listen {
    Analyze {
        name: String,
    },
    Remake {
        name: String,
        genre: Option<String>,
        seed: Option<u64>,
        flip: bool,
        /// the song itself rebuilt, every part on claudius's sounds (`Command::Remake`'s `rebuild`)
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        rebuild: bool,
    },
    SampleSong {
        name: String,
        /// the library's record of it, when a free library offered it (`LibraryHit::found`): the phrases credit it
        #[serde(default, skip_serializing_if = "Option::is_none")]
        found: Option<serde_json::Value>,
        /// how it goes in (`SAMPLING_WAYS`; its phrases when none)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        how: Option<String>,
    },
    ExtractGroove {
        name: String,
    },
    /// a take recorded for the song (the Record window's), played from the start of section `from` as track `track`
    /// (take1, take2, ... when none), for a part of `role`, as `/api/takes` adds one
    Take {
        name: String,
        from: String,
        track: Option<String>,
        role: String,
        /// added in the Add a recording window rather than recorded: the answer is that window's (`add_recording`)
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        recording: bool,
        /// the library's record of it, when a free library offered it (`LibraryHit::found`): the take credits it
        #[serde(default, skip_serializing_if = "Option::is_none")]
        found: Option<serde_json::Value>,
    },
    /// the auto-sampler's pieces of a recording, offered before any goes into the song, as `/api/sample/pieces`
    /// offers them (`seed`, `count`: as for `sample_pieces`)
    SamplePieces {
        name: String,
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        count: Option<u32>,
        /// the library's record of it, when a free library offered it (`LibraryHit::found`): the pieces kept credit it
        #[serde(default, skip_serializing_if = "Option::is_none")]
        found: Option<serde_json::Value>,
    },
    /// the answers to pitch check `id`, sung over its phrases from their first beat, as `/api/pitch-check` hears them
    PitchCheck {
        id: u64,
    },
    /// a project a page saved, the zip itself (its blueprint, the recordings it plays and what each was cut from),
    /// opened as the song, as `/api/project` opens one in claudius
    OpenProject {
        name: String,
    },
}

impl Listen {
    /// the file's name, whose extension says its format (a pitch check has none: its bytes say)
    pub fn name(&self) -> &str {
        match self {
            Listen::Analyze { name } | Listen::SampleSong { name, .. } | Listen::ExtractGroove { name } => name,
            Listen::Remake { name, .. } | Listen::Take { name, .. } | Listen::SamplePieces { name, .. } => name,
            Listen::OpenProject { name } => name,
            Listen::PitchCheck { .. } => "the pitch check",
        }
    }

    /// the command it answers for, as a reply names it
    pub fn what(&self) -> &'static str {
        match self {
            Listen::Analyze { .. } => "analyze",
            Listen::Remake { .. } => "remake",
            Listen::SampleSong { .. } => "sample_song",
            Listen::ExtractGroove { .. } => "extract_groove",
            Listen::SamplePieces { .. } => "sample_pieces",
            Listen::Take { recording: true, .. } => "add_recording",
            Listen::Take { .. } => "take",
            Listen::PitchCheck { .. } => "pitch_check",
            Listen::OpenProject { .. } => "open",
        }
    }
}

/// the track a turntable's record of the whole mix is cut from (`claudius::scratch::MIX`)
pub const WHOLE_MIX: &str = "*";

/// The tracks of `song` a phone's turntable can cut a record from, each with what it is (`sample`, `voice`, `part`,
/// `beat` or `mix`), in the order a DJ reaches for them: its samples and voices (a sampled phrase and its other
/// passage, a flip, a vocal chop, a take), its parts that play notes, its beats (a sampled break, a drum take), then
/// the whole mix. A drum lane alone (a kick, a clap) is one sound struck, no record to scratch, and nor is a texture's
/// noise or a riser.
pub fn records(song: &SongView) -> Vec<(String, String)> {
    const DRUMS: &[&str] = &["kick", "snare", "clap", "hat", "ohat", "shaker", "perc", "ride", "tom", "crash", "top"];
    let kind = |t: &TrackView| -> Option<(u8, &'static str)> {
        let drum = t.role == "drums" || DRUMS.contains(&t.role.as_str());
        match t.kind.as_str() {
            "drum" | "fx" => None,
            _ if t.role == "texture" => None,
            "loop" if t.role == "sample" => Some((0, "sample")),
            "loop" | "take" | "part" if t.role == "vocal" => Some((1, "voice")),
            "take" if !drum => Some((0, "sample")),
            "loop" | "take" | "pattern" if drum => Some((3, "beat")),
            _ => Some((2, "part")),
        }
    };
    let mut out: Vec<(u8, String, &'static str)> =
        song.tracks.iter().filter_map(|t| kind(t).map(|(rank, k)| (rank, t.name.clone(), k))).collect();
    out.sort_by_key(|(rank, _, _)| *rank);
    let mut out: Vec<(String, String)> = out.into_iter().map(|(_, name, k)| (name, k.to_string())).collect();
    if !song.tracks.is_empty() {
        out.push((WHOLE_MIX.into(), "mix".into()));
    }
    out
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SongView {
    pub title: String,
    pub genre: String,
    pub seed: u64,
    pub bpm: f64,
    /// e.g. "A minor"
    pub key: String,
    pub swing: f32,
    pub bars: u32,
    pub seconds: f64,
    /// progression name ("A", "B") -> Roman numerals ("i7", "VImaj7", ...)
    pub progressions: Vec<(String, Vec<String>)>,
    pub sections: Vec<SectionView>,
    /// every layer a section can switch on or off (drum lanes, parts, loops, pattern tracks)
    pub layers: Vec<String>,
    pub tracks: Vec<TrackView>,
    pub drums: Vec<DrumView>,
    /// the grooves the song keeps, heard in recordings, by name (lanes play them as they play `GROOVES`)
    #[serde(default)]
    pub grooves: Vec<String>,
    pub parts: Vec<PartView>,
    /// the drum kits the drums swap through (`Command::SwapSound` with `only` "drums"), in order: the genre's, the
    /// other installed kits, and `synthesized` (each lane's synthesized voice)
    #[serde(default)]
    pub kits: Vec<String>,
    /// the kit most of the drum lanes play now, and the one a swap of the drums puts them on next
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_kit: Option<String>,
    /// the mix around the tracks: the buses' chains, the master's, and what the sends feed
    #[serde(default)]
    pub mix: MixView,
    /// the song as a claudius script (runs back into this exact song)
    pub script: String,
    /// what was done to the song so far, oldest first
    pub lineage: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SectionView {
    pub name: String,
    /// intro, verse, build, drop, break, hook, groove, outro
    pub kind: String,
    pub start_bar: u32,
    pub bars: u32,
    pub energy: f32,
    pub layers: Vec<String>,
    /// the progression the section plays ("A", "B", ...)
    #[serde(default)]
    pub prog: String,
    /// drum pattern variant: main, alt, sparse, ...
    pub variant: String,
    pub filter_in: bool,
    pub build: bool,
    /// how its build-up builds (`BUILD_STYLES`), and how hard, 0 to 1
    #[serde(default = "classic")]
    pub build_style: String,
    #[serde(default = "half")]
    pub build_amount: f32,
    pub gap: bool,
    pub impact: bool,
    pub fills: bool,
}

fn classic() -> String {
    BUILD_STYLES[0].0.to_string()
}

fn half() -> f32 {
    0.5
}

/// The ways a section's build-up builds into the next (`SectionView::build_style`), the first the way when none is
/// said: each by name, in a few words, and what it plays.
pub const BUILD_STYLES: [(&str, &str, &str); 5] = [
    (
        "classic",
        "everything",
        "the snare rolls faster and louder, the kick leaves for the last bars, a riser climbs and the music's lows are \
         swept away",
    ),
    (
        "roll",
        "a drum roll",
        "the snare rolls into the next section and the kick leaves for the last bars, nothing else",
    ),
    (
        "sweep",
        "a filter sweep",
        "the music's lows swept away under a riser while the drums play on, as house and techno build",
    ),
    ("riser", "a riser", "a riser climbs over the section as it plays, and nothing else changes"),
    (
        "drop_out",
        "the drums drop out",
        "the drums leave for the last bars and the riser climbs over the music alone, holding its breath",
    ),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TrackView {
    pub name: String,
    /// "drum" | "part" | "loop" | "take" | "pattern" | "fx"
    pub kind: String,
    /// mixing role (kick, bass, chords, lead, top, ...)
    pub role: String,
    /// what makes the sound, in words ("synth patch 'pluck'", "kick cut from 'Big Room Kick' (Apple Loops)")
    pub sound: String,
    pub level_db: f32,
    pub pan: f32,
    pub reverb: f32,
    pub delay: f32,
    pub duck: f32,
    /// the mixer's drive in dB (0..36): saturation after its effects, at the loudness its level gives it
    #[serde(default)]
    pub drive_db: f32,
    pub muted: bool,
    /// soloed: when any track is, only soloed tracks play
    #[serde(default)]
    pub solo: bool,
    /// loudness of the track in the latest render (LUFS while playing), if rendered
    pub loudness: Option<f64>,
    /// its chain of effects, in order, each as the song keeps it: a map named by `fx` ({"fx": "reverb", "decay": 2.4,
    /// ...}), the same a script writes
    #[serde(default)]
    pub fx: Vec<serde_json::Value>,
    /// the bus it plays into (drums, bass, music, fx, or one of the song's own)
    #[serde(default)]
    pub bus: String,
    /// a take's length, in seconds of its recording (what `trim_take` cuts from)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    /// the sounds a part or a drum lane may swap to (`Command::SwapSound`), best first; none for other tracks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swaps: Vec<SoundChoice>,
    /// which of `swaps` it plays now, by name, and the one a swap with no sound named gives it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_swap: Option<String>,
    /// a track several sounds play at once (a rack): its layers, each as a script's `rack()` and
    /// `Command::SetLayers` take one: `{"sound": "sub_bass", "band": [0, 100], "level": -2, "pan": 0.3,
    /// "transpose": 12, "duck": 0.8, "fx": [...]}`, what is at its default left out
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<serde_json::Value>,
    /// a pattern track of several sounds: those a rack plays, each by its label (`set_layers` names one as
    /// `beat.kick`) with its layers as `layers` has them
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label_layers: Vec<(String, Vec<serde_json::Value>)>,
}

/// A sound a track may swap to: its name, as `part(..., #{ preset })` or `drum_sound()` take it; what it is (`synth`,
/// a kind of instrument such as `bass` or `electric piano`, a kit's name, or `synthesized`); and how near the genre
/// it is (`the genre's own`, `fits the genre`, `further afield`; for drums `the genre's kits`, `other kits`,
/// `synthesized`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SoundChoice {
    pub name: String,
    pub kind: String,
    pub group: String,
}

/// the mix around the tracks: each bus's chain of effects, the master's, and the effects the sends feed (effects as
/// `TrackView::fx` has them)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MixView {
    pub buses: Vec<(String, Vec<serde_json::Value>)>,
    pub master: Vec<serde_json::Value>,
    /// the master's loudness target (LUFS) and true-peak ceiling (dBTP)
    pub lufs: f32,
    pub ceiling_db: f32,
    pub reverb: serde_json::Value,
    pub delay: serde_json::Value,
}

/// What a chain of effects belongs to, as `Command::SetFx` names it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Chain {
    Track(String),
    Bus(String),
    Master,
    /// the effect a send feeds: "reverb" or "delay"
    Send(String),
}

impl Chain {
    /// as `Command::SetFx`'s `target` writes it
    pub fn target(&self) -> String {
        match self {
            Chain::Track(n) => format!("track:{n}"),
            Chain::Bus(n) => format!("bus:{n}"),
            Chain::Master => "master".into(),
            Chain::Send(n) => format!("send:{n}"),
        }
    }
}

/// what a node of the song's graph is
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// a track: a drum lane, a part, a loop, a take, a pattern track, the transitions
    Track(String),
    /// the effect at `index` of a chain
    Effect(Chain, usize),
    Bus(String),
    /// the reverb or delay the sends feed
    Return(String),
    /// the master's loudness stage, and the finished song
    Master,
    Out,
}

/// a node of the song's graph: what it is, its title and what it says of itself, and where it sits (a column of the
/// signal's way left to right, and a row)
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub title: String,
    pub detail: Vec<String>,
    pub column: usize,
    pub row: f32,
}

/// how one node feeds another
#[derive(Clone, Debug, PartialEq)]
pub enum EdgeKind {
    /// the signal itself
    Signal,
    /// a send, this much of it
    Send(f32),
    /// the kick ducking a track, this deep
    Sidechain(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// The song as a graph of its signal: each track, through its chain of effects, into its bus, through the bus's chain
/// into the master, through the master's chain and loudness stage out; the sends into the reverb and delay returns;
/// the kick ducking the tracks it ducks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// an effect in a few words: its name and its main settings ("saturate tape 6 dB, 50%", "filter lp 2400 Hz moving")
pub fn effect_summary(v: &serde_json::Value) -> String {
    let name = v.get("fx").and_then(|f| f.as_str()).unwrap_or("effect").replace('_', " ");
    let num = |k: &str| -> Option<String> {
        let x = v.get(k)?;
        Some(match x {
            serde_json::Value::Number(n) => {
                let f = n.as_f64().unwrap_or_default();
                if f.fract() == 0.0 { format!("{f:.0}") } else { format!("{f:.2}").trim_end_matches('0').to_string() }
            }
            serde_json::Value::Array(_) => "moving".into(),
            serde_json::Value::Object(_) => "by section".into(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => return None,
        })
    };
    let with = |unit: &str, k: &str| {
        num(k).map(|x| if x == "moving" || x == "by section" { format!("{k} {x}") } else { format!("{x}{unit}") })
    };
    let mix = num("mix").and_then(|m| match m.parse::<f64>() {
        Ok(f) if (f - 1.0).abs() < 1e-9 => None,
        Ok(f) => Some(format!("{:.0}%", f * 100.0)),
        Err(_) => Some(format!("mix {m}")),
    });
    let mut parts: Vec<String> = match name.as_str() {
        "gain" => vec![with(" dB", "db")],
        "eq" => vec![v.get("bands").and_then(|b| b.as_array()).map(|b| format!("{} bands", b.len()))],
        "filter" => vec![num("mode"), with(" Hz", "cutoff")],
        "compressor" => vec![with(" dB", "threshold_db"), num("ratio").map(|r| format!("{r}:1"))],
        "limiter" => vec![with(" dB", "ceiling_db")],
        "saturate" => vec![num("kind"), with(" dB", "drive_db")],
        "bitcrush" => vec![with(" bits", "bits")],
        "reverb" => vec![with(" s", "decay")],
        "delay" => vec![with(" beats", "beats"), with(" feedback", "feedback")],
        "width" => vec![with("x", "width")],
        "parallel" => vec![v.get("chains").and_then(|c| c.as_array()).map(|c| format!("{} chains", c.len()))],
        "tune" => {
            let scale = match v.get("scale") {
                Some(serde_json::Value::Object(o)) => o
                    .get("notes")
                    .and_then(|n| n.as_array())
                    .map(|n| n.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" ")),
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                _ => Some("chords".into()),
            };
            let strength = num("strength").and_then(|s| match s.parse::<f64>() {
                Ok(f) if (f - 1.0).abs() < 1e-9 => None,
                Ok(f) => Some(format!("{:.0}%", f * 100.0)),
                Err(_) => Some(format!("strength {s}")),
            });
            vec![scale, with(" ms", "speed_ms").or_else(|| Some("40 ms".into())), strength]
        }
        _ => Vec::new(),
    }
    .into_iter()
    .flatten()
    .collect();
    parts.extend(mix);
    if parts.is_empty() { name } else { format!("{name} {}", parts.join(", ")) }
}

impl SongView {
    /// the song as a graph of its signal (`Graph`), laid out in columns from the tracks to the finished song
    pub fn graph(&self) -> Graph {
        let mut g = Graph::default();
        let id_track = |n: &str| format!("track:{n}");
        let id_fx = |c: &Chain, i: usize| format!("{}#{i}", c.target());
        let deepest = self.tracks.iter().map(|t| t.fx.len()).max().unwrap_or(0);
        let bus_col = deepest + 1;
        // the buses the tracks play into, and the song's others, in the order a mix desk has them
        let mut buses: Vec<String> = Vec::new();
        for b in ["drums", "bass", "music", "fx"] {
            if self.tracks.iter().any(|t| t.bus == b) || self.mix.buses.iter().any(|(n, _)| n == b) {
                buses.push(b.to_string());
            }
        }
        for b in self.tracks.iter().map(|t| &t.bus).chain(self.mix.buses.iter().map(|(n, _)| n)) {
            if !b.is_empty() && !buses.contains(b) {
                buses.push(b.clone());
            }
        }
        let bus_chain = |b: &str| self.mix.buses.iter().find(|(n, _)| n == b).map(|(_, c)| c.as_slice()).unwrap_or(&[]);
        let bus_deepest = buses.iter().map(|b| bus_chain(b).len()).max().unwrap_or(0);
        let master_col = bus_col + bus_deepest + 1;
        let sections = |name: &str| -> Vec<String> {
            self.sections.iter().filter(|s| s.layers.iter().any(|l| l == name)).map(|s| s.name.clone()).collect()
        };
        for (row, t) in self.tracks.iter().enumerate() {
            let row = row as f32;
            let mut detail = vec![
                format!("{} {}: {}", t.kind, t.role, words::sound_in_words(&t.sound)),
                format!(
                    "{:+.1} dB, pan {}{}",
                    t.level_db,
                    words::pan_in_words(t.pan),
                    if t.muted { ", muted" } else { "" }
                ),
            ];
            let plays = sections(&t.name);
            if !plays.is_empty() {
                detail.push(format!("in {}", plays.join(" ")));
            }
            g.nodes.push(Node {
                id: id_track(&t.name),
                kind: NodeKind::Track(t.name.clone()),
                title: t.name.clone(),
                detail,
                column: 0,
                row,
            });
            let chain = Chain::Track(t.name.clone());
            let mut last = id_track(&t.name);
            for (i, e) in t.fx.iter().enumerate() {
                let id = id_fx(&chain, i);
                g.nodes.push(Node {
                    id: id.clone(),
                    kind: NodeKind::Effect(chain.clone(), i),
                    title: effect_summary(e),
                    detail: Vec::new(),
                    column: i + 1,
                    row,
                });
                g.edges.push(Edge { from: last, to: id.clone(), kind: EdgeKind::Signal });
                last = id;
            }
            let bus = if t.bus.is_empty() { "music" } else { t.bus.as_str() };
            g.edges.push(Edge { from: last.clone(), to: format!("bus:{bus}"), kind: EdgeKind::Signal });
            for (which, amount) in [("reverb", t.reverb), ("delay", t.delay)] {
                if amount > 0.0 {
                    g.edges.push(Edge {
                        from: last.clone(),
                        to: format!("return:{which}"),
                        kind: EdgeKind::Send(amount),
                    });
                }
            }
        }
        let kick = self.tracks.iter().find(|t| t.role == "kick").map(|t| id_track(&t.name));
        if let Some(k) = &kick {
            for t in self.tracks.iter().filter(|t| t.duck > 0.0 && Some(id_track(&t.name)) != kick) {
                g.edges.push(Edge { from: k.clone(), to: id_track(&t.name), kind: EdgeKind::Sidechain(t.duck) });
            }
        }
        let spread = self.tracks.len().max(1) as f32 / (buses.len() + 2) as f32;
        for (k, b) in buses.iter().enumerate() {
            let row = (k as f32 + 0.5) * spread;
            let fed = self.tracks.iter().filter(|t| t.bus == *b).count();
            g.nodes.push(Node {
                id: format!("bus:{b}"),
                kind: NodeKind::Bus(b.clone()),
                title: format!("{b} bus"),
                detail: vec![format!("{fed} track{}", if fed == 1 { "" } else { "s" })],
                column: bus_col,
                row,
            });
            let chain = Chain::Bus(b.clone());
            let mut last = format!("bus:{b}");
            for (i, e) in bus_chain(b).iter().enumerate() {
                let id = id_fx(&chain, i);
                g.nodes.push(Node {
                    id: id.clone(),
                    kind: NodeKind::Effect(chain.clone(), i),
                    title: effect_summary(e),
                    detail: Vec::new(),
                    column: bus_col + 1 + i,
                    row,
                });
                g.edges.push(Edge { from: last, to: id.clone(), kind: EdgeKind::Signal });
                last = id;
            }
            g.edges.push(Edge { from: last, to: "master".into(), kind: EdgeKind::Signal });
        }
        for (k, (which, e)) in [("reverb", &self.mix.reverb), ("delay", &self.mix.delay)].into_iter().enumerate() {
            let id = format!("return:{which}");
            if !g.edges.iter().any(|x| x.to == id) {
                continue;
            }
            let row = (buses.len() as f32 + 0.5 + k as f32 * 0.6) * spread;
            g.nodes.push(Node {
                id: id.clone(),
                kind: NodeKind::Return(which.into()),
                title: format!("{which} return"),
                detail: vec![effect_summary(e)],
                column: bus_col,
                row,
            });
            g.edges.push(Edge { from: id, to: "master".into(), kind: EdgeKind::Signal });
        }
        let mid = self.tracks.len().max(1) as f32 / 2.0 - 0.5;
        g.nodes.push(Node {
            id: "master".into(),
            kind: NodeKind::Master,
            title: "master".into(),
            detail: vec!["the buses and returns summed".into()],
            column: master_col,
            row: mid,
        });
        let mut last = "master".to_string();
        for (i, e) in self.mix.master.iter().enumerate() {
            let id = id_fx(&Chain::Master, i);
            g.nodes.push(Node {
                id: id.clone(),
                kind: NodeKind::Effect(Chain::Master, i),
                title: effect_summary(e),
                detail: Vec::new(),
                column: master_col + 1 + i,
                row: mid,
            });
            g.edges.push(Edge { from: last, to: id.clone(), kind: EdgeKind::Signal });
            last = id;
        }
        g.nodes.push(Node {
            id: "out".into(),
            kind: NodeKind::Out,
            title: "out".into(),
            detail: vec![format!("{:.1} LUFS, ceiling {:.1} dBTP", self.mix.lufs, self.mix.ceiling_db)],
            column: master_col + 1 + self.mix.master.len(),
            row: mid,
        });
        g.edges.push(Edge { from: last, to: "out".into(), kind: EdgeKind::Signal });
        g
    }

    /// the graph in words: every track's way through its effects into its bus, the buses', the returns' and the
    /// master's, and the sends and the ducking
    pub fn graph_text(&self) -> String {
        let g = self.graph();
        let title = |id: &str| g.nodes.iter().find(|n| n.id == id).map_or(id.to_string(), |n| n.title.clone());
        let ends =
            |id: &str| g.nodes.iter().any(|n| n.id == id && matches!(n.kind, NodeKind::Bus(_) | NodeKind::Master));
        let way = |start: &str| -> String {
            let mut at = start.to_string();
            let mut steps = vec![title(&at)];
            while let Some(e) = g.edges.iter().find(|e| e.from == at && e.kind == EdgeKind::Signal) {
                at = e.to.clone();
                steps.push(title(&at));
                if ends(&at) {
                    break;
                }
            }
            steps.join(" -> ")
        };
        let mut out = vec![format!(
            "{} ({}, {:.0} BPM, {}): each track through its effects into its bus, the buses and returns into the master, out",
            self.title, self.genre, self.bpm, self.key
        )];
        out.push("tracks:".into());
        for t in &self.tracks {
            let id = format!("track:{}", t.name);
            let mut line = format!("  {}", way(&id));
            let sends: Vec<String> = g
                .edges
                .iter()
                .filter_map(|e| match e.kind {
                    EdgeKind::Send(a)
                        if e.from == id || e.from.strip_prefix(&id).is_some_and(|r| r.starts_with('#')) =>
                    {
                        Some(format!("{} {a:.2}", e.to.trim_start_matches("return:")))
                    }
                    _ => None,
                })
                .collect();
            if !sends.is_empty() {
                line += &format!("; sends {}", sends.join(", "));
            }
            if t.duck > 0.0 && t.role != "kick" {
                line += &format!("; ducked {:.2} by the kick", t.duck);
            }
            let plays: Vec<&str> =
                self.sections.iter().filter(|s| s.layers.contains(&t.name)).map(|s| s.name.as_str()).collect();
            if !plays.is_empty() {
                line += &format!("; plays in {}", plays.join(" "));
            }
            out.push(line);
        }
        out.push("buses and returns:".into());
        for n in g.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Bus(_) | NodeKind::Return(_))) {
            let detail = if matches!(n.kind, NodeKind::Return(_)) {
                format!(" ({})", n.detail.join(", "))
            } else {
                String::new()
            };
            out.push(format!("  {}{detail} -> master", way(&n.id).trim_end_matches(" -> master")));
        }
        out.push(format!(
            "master: {} -> out at {:.1} LUFS, ceiling {:.1} dBTP",
            way("master").trim_end_matches(" -> out"),
            self.mix.lufs,
            self.mix.ceiling_db
        ));
        out.push("(the arranger adds its own at render: filter moves on the music and bass buses, width on the music bus, mono lows on the bass bus, a saturator on the drums bus where the chain has none)".into());
        out.join("\n")
    }
}

/// The ways the auto-sampler takes a recording into a song (`Command::SampleSong`'s `how`), the first when none is
/// said: each by name, in a few words, and what it does, as the studio and an assistant offer them.
pub const SAMPLING_WAYS: [(&str, &str, &str); 4] = [
    (
        "phrases",
        "its sung phrases",
        "its best sung phrases, a bar or two each, each a loop that comes round where its notes agree with the song's \
         chords",
    ),
    (
        "passage",
        "a passage, as a verse or chorus",
        "4 to 16 bars of its best sung part played through in the drops and hooks, as their verse or chorus, the \
         song's chords there following it",
    ),
    (
        "chops",
        "chopped into an instrument",
        "its best phrase cut in eighths, the slices with the voice played in a new rhythm as an instrument of their own",
    ),
    (
        "stabs",
        "a stab, struck as a drum",
        "one short hit of it, a syllable, struck on a few beats as a drum lane of its own, whose steps you can change",
    ),
];

/// An effect of each kind with settings that sound like something, to add to a chain (`Command::SetFx`): its name
/// (`fx`) and the effect as JSON.
pub const EFFECT_TEMPLATES: &[(&str, &str)] = &[
    ("gain", r#"{"fx": "gain", "db": -3}"#),
    (
        "eq",
        r#"{"fx": "eq", "bands": [{"kind": "highpass", "freq": 120}, {"kind": "peak", "freq": 3000, "gain_db": 2, "q": 1}]}"#,
    ),
    ("filter", r#"{"fx": "filter", "mode": "lp", "cutoff": 2400, "res": 0.2}"#),
    ("band", r#"{"fx": "band", "low_hz": 120, "high_hz": 2000}"#),
    (
        "compressor",
        r#"{"fx": "compressor", "threshold_db": -18, "ratio": 3, "attack_ms": 10, "release_ms": 100, "knee_db": 4, "makeup_db": 2}"#,
    ),
    ("limiter", r#"{"fx": "limiter", "ceiling_db": -1, "release_ms": 60}"#),
    ("saturate", r#"{"fx": "saturate", "kind": "tape", "drive_db": 6, "mix": 0.5}"#),
    ("bitcrush", r#"{"fx": "bitcrush", "bits": 10, "downsample": 2, "mix": 0.5}"#),
    ("reverb", r#"{"fx": "reverb", "decay": 2.4, "damping": 0.5, "predelay_ms": 20, "mix": 0.3}"#),
    ("delay", r#"{"fx": "delay", "beats": 0.75, "feedback": 0.35, "damping": 0.4, "pingpong": true, "mix": 0.25}"#),
    ("chorus", r#"{"fx": "chorus", "rate_hz": 0.5, "depth_ms": 3, "delay_ms": 12, "mix": 0.6}"#),
    ("flanger", r#"{"fx": "flanger", "rate_hz": 0.15, "depth_ms": 3, "delay_ms": 4, "feedback": 0.5, "mix": 0.5}"#),
    ("phaser", r#"{"fx": "phaser", "rate_hz": 0.3, "stages": 6, "depth": 0.7, "feedback": 0.4, "mix": 0.5}"#),
    ("width", r#"{"fx": "width", "width": 1.4, "mono_below_hz": 150}"#),
    ("tremolo", r#"{"fx": "tremolo", "beats": 0.5, "depth": 0.4}"#),
    ("auto_pan", r#"{"fx": "auto_pan", "beats": 2, "depth": 0.6}"#),
    ("gate", r#"{"fx": "gate", "steps": [1, 0, 1, 0, 1, 1, 0, 1], "step_beats": 0.25, "depth": 0.8, "smooth_ms": 3}"#),
    ("transient", r#"{"fx": "transient", "attack_db": 4, "sustain_db": -2}"#),
    ("tape", r#"{"fx": "tape", "wow": 0.4, "flutter": 0.25, "drive_db": 3, "lowpass_hz": 12000}"#),
    ("vinyl", r#"{"fx": "vinyl", "crackle": 0.25, "hiss_db": -46}"#),
    (
        "parallel",
        r#"{"fx": "parallel", "dry": 1, "chains": [{"amount": 0.4, "fx": [{"fx": "compressor", "threshold_db": -30, "ratio": 10, "attack_ms": 1, "release_ms": 60}]}]}"#,
    ),
    ("tune", r#"{"fx": "tune", "strength": 1, "speed_ms": 40, "humanize": 10, "vibrato": 1, "scale": "chords"}"#),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DrumView {
    pub lane: String,
    /// bars per pattern (the steps below cover pattern_bars * 16 sixteenth notes)
    pub pattern_bars: u32,
    /// variant name -> velocity per 1/16 step (0 = no hit)
    pub variants: Vec<(String, Vec<f32>)>,
    /// variant name -> the trigs of its steps, by 1/16 step
    #[serde(default)]
    pub trigs: Vec<(String, Vec<(usize, TrigView)>)>,
    /// its own cycle in sixteenths (none: its pattern's bars), how fast its steps go, its own swing (none: the song's),
    /// its groove (a name from `GROOVES` or the song's `grooves`) and how much of it
    #[serde(default)]
    pub length: Option<u32>,
    #[serde(default = "one_f32")]
    pub speed: f32,
    #[serde(default)]
    pub swing: Option<f32>,
    #[serde(default)]
    pub groove: Option<String>,
    #[serde(default = "one_f32")]
    pub groove_amount: f32,
    /// how far behind the beat it plays, in milliseconds (ahead, below zero)
    #[serde(default)]
    pub late_ms: f32,
}

fn one_f32() -> f32 {
    1.0
}

/// the grooves claudius has by name, as the engine's `rhythm::GROOVES`
pub const GROOVES: &[&str] =
    &["straight", "swing54", "swing58", "swing62", "swing66", "swing71", "shuffle", "dilla", "push", "lazy"];

/// What a step does beyond striking, as the engine's `Trig` keeps it (the same JSON): the chance it plays each time
/// round; when (`cond`: "always", "1:2" the first of every two times round a section, "3:4", "first", "last", "!first",
/// "!last"); strokes within its sixteenth (`ratchet`, 1 to 8) and how their velocity moves (`ramp`, -1 fading to 1
/// swelling); a flam this many ms before; a nudge off the grid in sixteenths (-0.5 to 0.5); and what it locks in the
/// sound: semitones, decay (times the lane's, 0.1 to 4), tone (-1 darker to 1 brighter), pan (-1 to 1).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TrigView {
    pub prob: f32,
    pub cond: String,
    pub ratchet: u8,
    pub ramp: f32,
    pub flam: f32,
    pub nudge: f32,
    pub pitch: i8,
    pub decay: f32,
    pub tone: f32,
    pub pan: f32,
}

impl Default for TrigView {
    fn default() -> TrigView {
        TrigView {
            prob: 1.0,
            cond: "always".into(),
            ratchet: 1,
            ramp: 0.0,
            flam: 0.0,
            nudge: 0.0,
            pitch: 0,
            decay: 1.0,
            tone: 0.0,
            pan: 0.0,
        }
    }
}

impl TrigView {
    /// the settings it changes, as `trig()` writes them in a script: `prob: 0.5, cond: "1:2"`
    pub fn script(&self) -> String {
        let d = TrigView::default();
        let n = |v: f32| {
            if v == v.trunc() { format!("{}", v as i64) } else { format!("{v:.4}").trim_end_matches('0').to_string() }
        };
        let mut parts = Vec::new();
        for (k, v, base) in [
            ("prob", self.prob, d.prob),
            ("ramp", self.ramp, d.ramp),
            ("flam", self.flam, d.flam),
            ("nudge", self.nudge, d.nudge),
            ("decay", self.decay, d.decay),
            ("tone", self.tone, d.tone),
            ("pan", self.pan, d.pan),
        ] {
            if v != base {
                parts.push(format!("{k}: {}", n(v)));
            }
        }
        if self.cond != d.cond && !self.cond.is_empty() {
            parts.push(format!("cond: \"{}\"", self.cond.replace(['"', '\\'], "")));
        }
        if self.ratchet != d.ratchet {
            parts.push(format!("ratchet: {}", self.ratchet));
        }
        if self.pitch != d.pitch {
            parts.push(format!("pitch: {}", self.pitch));
        }
        parts.join(", ")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PartView {
    pub name: String,
    pub role: String,
    pub style: String,
    /// styles this part may switch to (from its genre)
    pub styles: Vec<String>,
    pub preset: String,
    pub presets: Vec<String>,
    pub register: (i32, i32),
    pub density: f32,
    /// what it plays over each progression its sections play, one cycle each, then over those it has notes written out
    /// for and plays nowhere now
    #[serde(default)]
    pub cycles: Vec<CycleView>,
    /// how far behind the beat it plays, in milliseconds (ahead, below zero)
    #[serde(default)]
    pub late_ms: f32,
}

/// One cycle of a part over a progression: its notes (beats from the cycle's start), which come round every `beats`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CycleView {
    pub prog: String,
    pub beats: f64,
    /// written out (by hand, from a take, or in the piano roll); otherwise the theory engine writes them, and editing
    /// them writes them out
    pub written: bool,
    pub notes: Vec<NoteView>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RenderView {
    /// state version this render belongs to
    pub version: u64,
    /// relative URL of the mastered audio (mp3 for the browser; the same path with .wav gives the 24-bit master)
    pub url: String,
    pub seconds: f64,
    pub lufs: f64,
    pub true_peak_db: f32,
    pub bpm: f64,
    /// (section name, start s, end s)
    pub sections: Vec<(String, f64, f64)>,
    /// peak envelope for drawing, ~1000 points 0..1
    pub waveform: Vec<f32>,
    /// track names that have stems at /api/stems/{version}/{track}.wav
    pub stems: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct VerdictView {
    /// 0..100
    pub total: f64,
    pub summary: String,
    /// component -> 0..1 (mix, arc, theory, genre, production, refs, time)
    pub scores: Vec<(String, f64)>,
    /// (severity 0..1, what, where to fix it)
    pub issues: Vec<(f64, String, String)>,
    pub ears: Option<EarsView>,
    /// meters: lufs, true_peak_db, band_*_db, stereo_correlation, drop_minus_break_lu, ...
    pub measures: Vec<(String, f64)>,
    /// the issues one change fixes outright (`Command::QuickFix`), with no variations searched for it
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<QuickFixView>,
}

/// A quick fix of one of the verdict's issues: the issue, by where its fix lives (`VerdictView::issues`' third
/// field), and what the fix does, in words.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct QuickFixView {
    pub target: String,
    pub label: String,
}

/// a part of the verdict's score as a person says it, and what it judges
pub fn score_in_words(key: &str) -> (&str, &str) {
    match key {
        "arc" => ("Shape", "how its intensity rises and falls across the sections, as its genre's songs do"),
        "mix" => ("Mix", "loudness, peaks, the balance of lows and highs, the stereo width and the low end"),
        "theory" => ("Harmony", "the chords, the key, and how the melody and the bass sit on them"),
        "genre" => ("Genre", "how much it sounds like its genre to the listening model"),
        "production" => ("Production", "how well mixed and produced it sounds to the listening model"),
        "aesthetics" => (
            "Appeal",
            "how much listeners would enjoy it, and how well it is made, as a model of their ratings hears it",
        ),
        "refs" => ("Records", "how close it sounds to records of its genre"),
        "time" => ("Timing", "whether every track plays in time with the song's tempo"),
        other => (other, "a part of the score"),
    }
}

/// how much an issue of the verdict costs (its severity, 0..1), in words
pub fn severity_in_words(s: f64) -> &'static str {
    if s >= 0.6 {
        "major"
    } else if s >= 0.3 {
        "worth fixing"
    } else {
        "minor"
    }
}

/// Where an issue of the verdict is fixed, in words, from the critic's target: `part:lead` the lead part,
/// `band:presence-` more presence in the mix, `level:kick+` kick quieter, `width+` a wider stereo image, `form` the
/// arrangement, `form:intro` a shorter intro, and so on.
pub fn fix_in_words(target: &str) -> String {
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
        "form" => match what {
            "short" => "a longer song: a section again, or longer ones".into(),
            "long" => "a shorter song: a repeat left out, or shorter sections".into(),
            "intro" => "a shorter intro".into(),
            "same" => "sections told apart: a part, the drums or the chords changing between them".into(),
            "section" => "a long section shorter".into(),
            "peaks-" => "the hook or the drop again".into(),
            "peaks+" => "fewer hooks or drops".into(),
            _ => "the arrangement".into(),
        },
        "genre" => "the genre's sounds and parts".into(),
        "production" => "the production: the sounds and the mix".into(),
        _ => target.into(),
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EarsView {
    pub genre_query: String,
    pub genre_z: f64,
    pub best_alt: String,
    pub best_alt_z: f64,
    pub production_z: f64,
    pub worst_flaw: String,
    pub worst_flaw_z: f64,
    pub sounds_like: Vec<String>,
    pub p_real: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SoundList {
    /// procedural drum voices (kick, snare, clap, hh, oh, ride, crash, tom, rim, shaker, cowbell, snap, perc ...)
    pub drums: Vec<String>,
    /// synth presets (808, pluck, supersaw, pad, epiano, ...)
    pub synths: Vec<String>,
    /// multi-sampled instruments from installed packs ("piano", "strings", "tr909:bd" ...)
    pub instruments: Vec<String>,
    /// installed packs and whether they are installed
    pub packs: Vec<PackInfo>,
    /// the drums of the installed kits, as `pack:drum` ("tr909:bd", "tr808:cp" ...)
    #[serde(default)]
    pub kit: Vec<String>,
    /// the roles each synth preset is made for (bass, chords, pad, keys, lead, arp)
    #[serde(default)]
    pub synth_roles: Vec<(String, Vec<String>)>,
    /// what each synth preset and drum voice is, in a few words, by name
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub about: Vec<(String, String)>,
    /// for each role (bass, chords, pad, keys, lead, arp), the styles a part of it may be written in, each with what
    /// it plays in a few words
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<(String, Vec<(String, String)>)>,
}

impl SoundList {
    /// what the synth preset or drum voice `name` is, in a few words; of a drum of a kit (`tr909:bd`), its voice's
    pub fn sound_about(&self, name: &str) -> Option<&str> {
        let find = |n: &str| self.about.iter().find(|(k, _)| k == n).map(|(_, a)| a.as_str());
        let drum = name.split_once(':').map(|(_, n)| n).filter(|n| self.drums.iter().any(|d| d == n));
        find(name).or_else(|| drum.and_then(find))
    }

    /// what a part of `role` plays in `style`, in a few words
    pub fn style_about(&self, role: &str, style: &str) -> Option<&str> {
        let (_, styles) = self.styles.iter().find(|(r, _)| r == role)?;
        styles.iter().find(|(s, _)| s == style).map(|(_, a)| a.as_str())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PackInfo {
    pub name: String,
    pub description: String,
    pub license: String,
    pub installed: bool,
    pub size_mb: f64,
    /// one of the packs `claudius setup` installs
    #[serde(default)]
    pub default: bool,
    /// on an unclear licence: installed only when asked for by name, with `claudius-packs install --accept-licence`
    #[serde(default)]
    pub opt_in: bool,
}

/// what notes sung or played can become (`add_notes` and `transcribe_take` take one as `style`): the line as it is, with
/// a third above, doubled an octave down, as a triad under each note, down in a bass's register, as a rising arpeggio on
/// each note, or as a chord held through each bar the line sounds in
pub const NOTE_STYLES: &[&str] = &["line", "harmony", "octaves", "chords", "bass", "arp", "pad"];

/// a note: start and length in beats, MIDI pitch (60 is middle C), velocity 0..1 (0.8 when left out)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NoteView {
    pub t: f64,
    pub d: f64,
    pub p: i32,
    #[serde(default = "played_velocity")]
    pub v: f32,
}

fn played_velocity() -> f32 {
    0.8
}

/// a drum hit played on a pad: the lane it strikes, when (in beats from the start of the take's section) and how hard
/// (0..1, 0.8 when left out)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HitView {
    pub lane: String,
    pub t: f64,
    #[serde(default = "played_velocity")]
    pub v: f32,
}

/// where a scratched record was `t` seconds into the scratch: `at` seconds into the record, with the crossfader `fader`
/// open (0 shut, 1 open, the default)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ScratchMove {
    pub t: f64,
    pub at: f64,
    #[serde(default = "open_fader")]
    pub fader: f32,
}

fn open_fader() -> f32 {
    1.0
}

/// an edit or action; JSON `{"cmd": "set_tempo", "bpm": 124}`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// generate a new song from a genre (seed random when absent)
    NewSong {
        genre: String,
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        bpm: Option<f64>,
        #[serde(default)]
        key: Option<String>,
        /// build it around an Apple Loops family
        #[serde(default)]
        loops: bool,
        /// build it on samples from free libraries: a phrase to build it on, a break, a texture, as the genre's
        /// producers sample them (none: as the genre does; trip-hop's songs are, a house song is when asked)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sampled: Option<bool>,
        /// what to search the libraries for the phrase with ("70s soul", "bossa nova guitar"); none: the records the
        /// genre's producers dig for, one picked at random
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sample_search: Option<String>,
        /// how layered it is, from 0 (the genre's core, sparse) through 0.5 (the genre as written) to 1 (instruments
        /// doubled on instruments, a counter-melody, more percussion, samples on samples); none: the genre's own
        #[serde(default, skip_serializing_if = "Option::is_none")]
        complexity: Option<f64>,
    },
    /// ask the person for a take that starts with section `from`: the studio's Record window opens for it, saying
    /// `note`, and the take sent for it answers the request (`State.take_request`)
    RequestTake {
        from: String,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        note: String,
    },
    /// the take asked for as `id` will not be recorded
    CancelTakeRequest {
        id: u64,
    },
    /// the person writes to the assistants in the studio's chat (`State.chat`)
    ChatSay {
        text: String,
    },
    /// an assistant writes to the person in the studio's chat, as `from` (its name), answering message `reply_to`
    ChatAnswer {
        text: String,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        reply_to: Option<u64>,
    },
    /// an assistant listens for what the person writes, for `seconds` (50): the studio's chat says so while it does
    ChatListen {
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        seconds: Option<u64>,
    },
    /// the chat's history goes, in the studio and where claudius keeps it
    ClearChat,
    /// open a song folder, a blueprint.json, a script file, or a project the demo saved (a zip, unpacked into a song
    /// folder of its own)
    Load {
        path: String,
    },
    /// open a song from a file's text, as the studio reads a file chosen on its own computer: a blueprint.json's (when
    /// `name` ends in `.json`) or a script's. Nothing is kept of the file: saving makes a song folder of its own
    OpenText {
        /// the file's name, which says what it holds
        name: String,
        text: String,
    },
    /// write blueprint.json + song.wav/mp3 + script into a folder (default: a fresh folder under out/)
    Save {
        #[serde(default)]
        path: Option<String>,
    },
    /// write the latest render of the song as it is now in other formats (`ExportFormat.spec`), into `path` (default:
    /// the song's folder, as `Save` makes it)
    Export {
        formats: Vec<String>,
        #[serde(default)]
        path: Option<String>,
    },
    /// what stores ask about the release, all of it (`ReleaseDetails`), and optionally a JPEG or PNG file for the
    /// artwork; the release is checked again
    SetRelease {
        details: Box<ReleaseDetails>,
        #[serde(default)]
        artwork: Option<String>,
    },
    /// write the release kit from the latest render, when it is of the song as it is now and nothing stops the
    /// release: into `path` (default: release/ in the song's folder)
    WriteKit {
        #[serde(default)]
        path: Option<String>,
    },
    SetTempo {
        bpm: f64,
    },
    SetKey {
        key: String,
    },
    SetSwing {
        swing: f32,
    },
    SetProgression {
        name: String,
        chords: Vec<String>,
    },
    /// change any subset of a section's properties
    SetSection {
        index: usize,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        bars: Option<u32>,
        #[serde(default)]
        energy: Option<f32>,
        #[serde(default)]
        layers: Option<Vec<String>>,
        /// which progression it plays
        #[serde(default)]
        prog: Option<String>,
        #[serde(default)]
        variant: Option<String>,
        #[serde(default)]
        filter_in: Option<bool>,
        #[serde(default)]
        build: Option<bool>,
        /// how the build-up builds (`BUILD_STYLES`: classic, roll, sweep, riser, drop_out); naming one turns it on
        #[serde(default, skip_serializing_if = "Option::is_none")]
        build_style: Option<String>,
        /// how hard it builds, 0 (gently) to 1 (as hard as it goes); half by default
        #[serde(default, skip_serializing_if = "Option::is_none")]
        build_amount: Option<f32>,
        #[serde(default)]
        gap: Option<bool>,
        #[serde(default)]
        impact: Option<bool>,
        #[serde(default)]
        fills: Option<bool>,
    },
    AddSection {
        after: usize,
        kind: String,
        bars: u32,
    },
    RemoveSection {
        index: usize,
    },
    MoveSection {
        from: usize,
        to: usize,
    },
    ToggleLayer {
        section: usize,
        layer: String,
    },
    /// one drum step; velocity 0 removes the hit
    SetDrumStep {
        lane: String,
        variant: String,
        step: usize,
        velocity: f32,
    },
    /// a drum lane's own time and feel, as `lane()` sets it: `length` in sixteenths (0: its pattern's bars), `speed`
    /// (0.125 to 4), `swing` (0: the song's), `groove` (a name; "" none), `groove_amount` (0 to 1), and `late_ms`, how
    /// far behind the beat every hit plays (-80 to 80; below zero, ahead): a laid-back snare 20, a pushed kick -5
    SetLane {
        lane: String,
        #[serde(default)]
        length: Option<u32>,
        #[serde(default)]
        speed: Option<f32>,
        #[serde(default)]
        swing: Option<f32>,
        #[serde(default)]
        groove: Option<String>,
        #[serde(default)]
        groove_amount: Option<f32>,
        #[serde(default)]
        late_ms: Option<f32>,
    },
    /// the groove of the drums in an audio file, kept with the song as `name` (the file's name when not given), and
    /// the lanes `lanes` (every drum lane when none) put in it. Its tempo is `bpm` when given, else the Apple Loop's
    /// or one written in the file's name ("92bpm"), else heard.
    ExtractGroove {
        path: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        lanes: Vec<String>,
        #[serde(default)]
        bpm: Option<f64>,
    },
    /// what step `step` (a sixteenth, from 0) of a lane variant does beyond striking, as `trig()` sets it in a script;
    /// no `trig` clears it. A step without a hit gets one.
    SetTrig {
        lane: String,
        variant: String,
        step: usize,
        #[serde(default)]
        trig: Option<TrigView>,
    },
    /// a whole lane variant in step notation: "x...x...x...x..." (x hit, X accent, o ghost, . rest; 16 per bar)
    SetDrumPattern {
        lane: String,
        variant: String,
        steps: String,
    },
    SetPart {
        name: String,
        #[serde(default)]
        style: Option<String>,
        #[serde(default)]
        preset: Option<String>,
        #[serde(default)]
        register: Option<(i32, i32)>,
        #[serde(default)]
        density: Option<f32>,
        /// write the part again with a new seed
        #[serde(default)]
        reseed: bool,
        /// how far behind the beat it plays, in milliseconds (-80 to 80; below zero, ahead): a bass pulled behind
        #[serde(default)]
        late_ms: Option<f32>,
    },
    /// A new track. `kind` "drum" is a drum lane, silent until its steps are written, playing `sound`: a drum
    /// voice (`kick`, `snare`, `clap`, `hat`, `rim`, `tom` ...) or a kit's (`tr909:bd`). `kind` "part" is a part the
    /// theory engine writes, in `role`'s styles (bass, chords, pad, keys, lead or arp; the genre's part of that name
    /// when it has one), playing `sound`: a synth preset or an installed instrument (the genre's draw when left out).
    /// It plays where the form already names it, otherwise in every section with energy 0.6 or more.
    AddTrack {
        name: String,
        kind: String,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        sound: Option<String>,
    },
    /// Chords under part `part`'s melody, as a new part that harmonizes it (`name`, `<part>_chords` by default): chords
    /// that fit its notes and the song's own chords, worked out afresh as the melody changes, in `style` (`block`, the
    /// default, a chord under each of its notes with the melody on top; a chord part's rhythm: `sustain`, `half`,
    /// `stabs`, `offbeat`, `pump`, `syncopated`, `charleston`; or a comping idiom, `skank`, `gospel` ...), on `sound`
    /// (the melody's own for `block`, else the genre's draw for the role), in the sections the melody plays in.
    Harmonize {
        part: String,
        #[serde(default)]
        style: Option<String>,
        #[serde(default)]
        sound: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },
    /// Another sound for a track, as a producer swaps one: `track` (a part or a drum lane) plays `to` (one of its
    /// `TrackView::swaps`, or any synth preset, installed instrument or drum sound), or without it the next of its
    /// swaps that no other track plays; block chords under a part change with it. With no `track` every part and
    /// drum lane swaps: the drums onto the next kit (`SongView::next_kit`, or `to`, one of `SongView::kits`), each
    /// part to its next sound; `only` "drums" or "parts" swaps just those.
    SwapSound {
        #[serde(default)]
        track: Option<String>,
        #[serde(default)]
        to: Option<String>,
        #[serde(default)]
        only: Option<String>,
    },
    /// A track written the way genre `genre` (the song's, when left out) plays it. `what` is `drums` (the genre's lanes
    /// in place of the song's of their names, and beside the others), `percussion` (its percussion lanes beside the
    /// song's drums), or a part: `bass`, `chords`, `keys`, `pad`, `strings`, `lead` or `arp`, written as the genre's
    /// own part of the role is (its idiom, a sound from its palette, its register, density, voicing and mix), added
    /// as `name` (`<genre>_<what>` by default) or in place of the song's part `replace`. Each one is another take;
    /// `seed` gives one again.
    GenerateTrack {
        what: String,
        #[serde(default)]
        genre: Option<String>,
        #[serde(default)]
        replace: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        seed: Option<u64>,
    },
    /// Another take of part or drum lane `track` (`take`), or a variation of the take it plays: a part keeps its
    /// idiom's cells and draws their details again; a drum lane is varied a step or two, its strong hits kept.
    VaryTrack {
        track: String,
        #[serde(default)]
        take: bool,
    },
    /// A track out of the song, with every section's mention of it: a drum lane, part, loop, take or pattern track.
    RemoveTrack {
        name: String,
    },
    /// A part's notes over progression `prog`, written out: they replace what the theory engine writes there and come
    /// round with the progression (an empty list rests). No `notes`: the theory engine writes it again.
    SetNotes {
        part: String,
        prog: String,
        #[serde(default)]
        notes: Option<Vec<NoteView>>,
    },
    SetMix {
        track: String,
        #[serde(default)]
        level_db: Option<f32>,
        #[serde(default)]
        pan: Option<f32>,
        #[serde(default)]
        reverb: Option<f32>,
        #[serde(default)]
        delay: Option<f32>,
        #[serde(default)]
        duck: Option<f32>,
        /// the mixer's drive in dB (0..36): saturation after its effects, which thickens and roughens it
        #[serde(default)]
        drive_db: Option<f32>,
        #[serde(default)]
        muted: Option<bool>,
        #[serde(default)]
        solo: Option<bool>,
        /// the bus it plays into (drums, bass, music, fx, or one of the song's own)
        #[serde(default)]
        bus: Option<String>,
    },
    /// Replaces a chain of effects, as `fx()`, `bus()`, `master(#{ fx })` and `send_fx()` do in a script. `target` is
    /// `track:<name>`, `bus:<name>`, `master`, `send:reverb` or `send:delay` (a send takes exactly one effect); each
    /// effect as `TrackView::fx` has them.
    SetFx {
        target: String,
        fx: Vec<serde_json::Value>,
    },
    /// Has several sounds play a track's notes at once (a rack), as `rack()` does in a script: `track` a part, a drum
    /// lane or a pattern track's sound (`beat.kick`), each of `layers` as `TrackView::layers` has them: its `sound`
    /// (a synth preset, a drum, a sampled instrument or a file, with `bank` as `drum_sound` takes it), the `band`
    /// it keeps ([low, high] in Hz, 0 for no edge), its `level` in dB against the others, `pan`, `transpose` in
    /// semitones, `duck` under the kick (0 to 1) and `fx`. 1 to 8 layers.
    SetLayers {
        track: String,
        layers: Vec<serde_json::Value>,
    },
    /// run a claudius script against the current song (see the script reference)
    RunScript {
        code: String,
    },
    /// read an audio file: tempo, key, chords, sections, drum pattern (the answer's message)
    Analyze {
        path: String,
    },
    /// a new song built from the analysis of an audio file (its tempo, key, progression, form and groove)
    Remake {
        path: String,
        #[serde(default)]
        genre: Option<String>,
        #[serde(default)]
        seed: Option<u64>,
        /// the tempo (default: the recording's, halved or doubled into the genre's range when it is outside)
        #[serde(default)]
        bpm: Option<f64>,
        /// whether the song is built on the recording's sound (none or true); false: a new song on its skeleton only,
        /// none of its sound
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sampled: Option<bool>,
        /// built on its sound, how: a rework (none or false) plays all of the recording through, its voice and its
        /// chords as they are, without its drums and its bass, in its own sections and on its chords, with the
        /// genre's drums and bass under it; a flip (true) makes a new song of the genre on its hook, a passage of
        /// its verses, its drums, a flip of it and its sung phrases
        #[serde(default, skip_serializing_if = "Option::is_none")]
        flip: Option<bool>,
        /// rebuilt (true): the song itself, every part of it played on claudius's sounds so that each note can be
        /// changed, its drums struck as it strikes them and its bass and the rest as the notes the ears hear them
        /// play, in its own sections and on its chords, its voice kept on a track of its own and its melody written
        /// out; before `sampled` and `flip`
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rebuild: Option<bool>,
    },
    /// a new song like the recording claudius heard last (`analyze`), in `genre`: its tempo (halved or doubled into
    /// the genre's range when it is outside), key, progressions, form and swing, and none of its sound
    SongLike {
        #[serde(default)]
        genre: Option<String>,
        #[serde(default)]
        seed: Option<u64>,
    },
    /// samples a recording into the song (the auto-sampler), in one of `SAMPLING_WAYS`: by default its best sung
    /// phrases, cut on its beat grid and cleaned of its drums, fitted to the song's tempo and key, placed where they
    /// agree with its chords, mixed as vocals; or a passage of it as a verse or chorus, its voice chopped into an
    /// instrument, or a short hit of it struck as a drum. claudius keeps what it cut with the song.
    SampleSong {
        path: String,
        /// how many phrases, 1 to 4 (2)
        #[serde(default)]
        count: Option<u32>,
        /// how long each is, in bars: 1, 2 or 4 (whichever of 1 and 2 sounds best); a passage 4, 8 or 16
        #[serde(default)]
        bars: Option<u32>,
        /// what to listen for ("a person singing")
        #[serde(default)]
        find: Option<String>,
        /// the sections to put them in, by name or kind (those that suit each phrase)
        #[serde(default)]
        sections: Vec<String>,
        /// how it goes in: "phrases" (the default), "passage", "chops" or "stabs" (`SAMPLING_WAYS`)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        how: Option<String>,
    },
    /// a recording (an audio file) as a take: it plays once from the start of section `from`, on through the sections
    /// after it as long as it lasts, at the song's tempo; claudius keeps a copy with the song
    AddTake {
        path: String,
        from: String,
        /// the track's name (default take1, take2, ...)
        #[serde(default)]
        name: Option<String>,
        /// vocal, lead, chords, keys, bass, drums or texture: how loud the mix makes it (default lead)
        #[serde(default)]
        role: Option<String>,
    },
    /// take `track` cut down to its recording from `start` to `end` seconds in (to its end without one): the cut plays
    /// from the start of the same section, the recording stays for undo
    TrimTake {
        track: String,
        #[serde(default)]
        start: f64,
        #[serde(default)]
        end: Option<f64>,
    },
    /// notes played (a MIDI keyboard's take) or written note by note, in beats from the start of section `from`, chords
    /// and all, as new parts, one per sound, on in the sections the notes reach
    AddNotes {
        from: String,
        notes: Vec<NoteView>,
        /// synth presets or installed instruments ("epiano", "piano"): one part each
        sounds: Vec<String>,
        /// the first part's name (default played)
        #[serde(default)]
        name: Option<String>,
        /// lead (default), keys, chords, pad, arp or bass
        #[serde(default)]
        role: Option<String>,
        /// what the notes become: line (as they are, the default), harmony, octaves, chords, bass, arp or pad
        #[serde(default)]
        style: Option<String>,
        /// the take request (`request_take`) the notes answer, when they were played for one
        #[serde(default)]
        request: Option<u64>,
    },
    /// the melody of a take (sung, hummed, whistled or played one note at a time) as the notes of new parts, one per
    /// sound, in the song's key and on its grid, in the sections the take plays in; the take goes quiet unless
    /// `keep_take`
    TranscribeTake {
        take: String,
        /// synth presets or installed instruments ("pluck", "piano"): one part each
        sounds: Vec<String>,
        /// the first part's name (default melody)
        #[serde(default)]
        name: Option<String>,
        /// lead (default), keys, chords, pad, arp or bass
        #[serde(default)]
        role: Option<String>,
        /// keep notes outside the key where they were sung
        #[serde(default)]
        chromatic: bool,
        #[serde(default)]
        keep_take: bool,
        /// what the line becomes: line (as sung, the default), harmony, octaves, chords, bass, arp or pad
        #[serde(default)]
        style: Option<String>,
    },
    /// a beat, played or beatboxed, as the song's drums: the kicks, snares and hats of take `take`, or of the audio
    /// file at `path`, folded into a pattern of `bars` bars (1, 2 or 4; the recording's, up to 4) and written as
    /// the lanes' `variant` (main); a step hit only some of the times round plays with that chance. A file's tempo
    /// is `bpm`, else one in its name, else heard. The take goes quiet unless `keep_take`.
    TranscribeDrums {
        #[serde(default)]
        take: Option<String>,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        bars: Option<u32>,
        #[serde(default)]
        variant: Option<String>,
        #[serde(default)]
        bpm: Option<f64>,
        #[serde(default)]
        keep_take: bool,
    },
    /// drums played on pads (a phone's, a MIDI controller's) with the song from section `from`, as the lanes they
    /// strike: folded into a pattern of `bars` bars (1, 2 or 4; as much of 4 as was played) on a `grid` in beats (a
    /// sixteenth, 0.25, by default; 0 keeps the hits where they were played) and written as the lanes' `variant`
    /// (main); a step struck only some of the times round the take (from `start` to `beats`, in beats from the
    /// section's start: from 0 to its last hit by default) plays with that chance
    AddHits {
        from: String,
        hits: Vec<HitView>,
        #[serde(default)]
        bars: Option<u32>,
        #[serde(default)]
        variant: Option<String>,
        #[serde(default)]
        grid: Option<f64>,
        #[serde(default)]
        start: Option<f64>,
        #[serde(default)]
        beats: Option<f64>,
    },
    /// a scratch, kept as a take that plays from the start of section `from`: track `track` of the song alone (or the
    /// whole mix, for `WHOLE_MIX`) over `bars` bars (1, the default, to 8) from bar `bar` (0, the first, by default) is
    /// the record, read along `moves` through the crossfader; a `track` of `lib:...` is a sample from the library (the
    /// phone's `GET /api/library`), its first `bars` bars
    AddScratch {
        from: String,
        track: String,
        #[serde(default)]
        bar: u32,
        #[serde(default)]
        bars: Option<u32>,
        moves: Vec<ScratchMove>,
        /// the take's name (default scratch1, scratch2, ...)
        #[serde(default)]
        name: Option<String>,
        /// vocal, lead, chords, keys, bass, drums or texture: how loud the mix makes it (default lead)
        #[serde(default)]
        role: Option<String>,
        /// a record recorded off a microphone, in place of one cut from the song: a WAV file, in base64, up to
        /// `claudius::scratch::MAX_RECORDING` seconds long (the phone's turntable sends it)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recording: Option<String>,
    },
    /// Asks the person for a pitch check: the studio's Record window opens with phrases to sing back (`State.pitch_check`),
    /// the notes played between MIDI `low` and `high` (default 57 to 69, A3 to A4: sung in any octave)
    RequestPitchCheck {
        #[serde(default)]
        low: Option<i32>,
        #[serde(default)]
        high: Option<i32>,
        #[serde(default)]
        note: String,
    },
    /// the auto-sampler's pieces of a recording (an audio file, or `library:<id>`), offered before any goes into the
    /// song: `count` phrases (1 to 4, default 2) of `bars` bars (1, 2 or 4; whichever sounds best by default), each as
    /// it was cut and through a treatment, for take `seed` (0, the default: the best phrases; any other: another draw)
    SamplePieces {
        path: String,
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        bars: Option<u32>,
        /// what the ears listen for ("a person singing" by default)
        #[serde(default)]
        find: Option<String>,
    },
    /// the recording the auto-sampler offered pieces of, drawn again for take `seed` (the next one by default)
    RedrawPieces {
        #[serde(default)]
        seed: Option<u64>,
    },
    /// the pieces offered that `keep` names, each as it was cut or treated, sampled into the song in `sections` (by
    /// name or kind; those that suit each by default)
    KeepPieces {
        keep: Vec<KeptPiece>,
        #[serde(default)]
        sections: Vec<String>,
    },
    /// the person will not do pitch check `id` now
    CancelPitchCheck {
        id: u64,
    },
    /// How a take or a recording is sung: the notes it means, how near each came, and the singer (where they sing sharp
    /// or flat by register, how widely their notes scatter, how fast they settle, their vibrato). A take is heard in
    /// the song's key and over its chords from where it starts; a file in `key` when given. Changes nothing.
    AssessVoice {
        /// a take in the song
        #[serde(default)]
        take: Option<String>,
        /// or an audio file
        #[serde(default)]
        path: Option<String>,
        /// the key a file is sung in ("A minor"), which settles doubtful notes
        #[serde(default)]
        key: Option<String>,
    },
    /// Tunes a track (a take, a sampled vocal) to the notes it means: a `tune` effect first in its chain, or the one
    /// there changed, with what is given (the rest as before, or as a new tuner has them); `off` takes it out.
    Tune {
        track: String,
        /// how much of the way to the notes, 0 to 1 (default 1)
        #[serde(default)]
        strength: Option<f32>,
        /// how fast a note is brought onto its pitch after it starts, ms (default 40; 0 is the hard-tuned sound)
        #[serde(default)]
        speed_ms: Option<f32>,
        /// the cents of the singer's own wandering around a note that stay (default 10)
        #[serde(default)]
        humanize: Option<f32>,
        /// how much of the vibrato stays, 0 to 1 (default 1)
        #[serde(default)]
        vibrato: Option<f32>,
        /// what the notes are read against: chords (the default), key, chromatic, or the notes to go to ("C D E G A")
        #[serde(default)]
        scale: Option<String>,
        #[serde(default)]
        off: bool,
    },
    /// the verdict's quick fix of the issue at `target` (`VerdictView::fixes`), or all its quick fixes when none, as
    /// one edit: a recording stretched from the tempo it keeps, a track moved to the level its genre mixes it at, a
    /// band of the master into its range; none while the verdict is older than the song
    QuickFix {
        #[serde(default)]
        target: Option<String>,
    },
    /// try `count` random mutations, keep the best-judged one when `keep_best` (else only report them)
    Mutate {
        #[serde(default = "four")]
        count: usize,
        #[serde(default = "yes")]
        keep_best: bool,
    },
    Evolve {
        #[serde(default = "three")]
        generations: usize,
        #[serde(default = "six")]
        children: usize,
    },
    /// download and install sample packs: the default packs that are missing, or the named ones (not the opt-in
    /// packs, whose licence has to be accepted with `claudius-packs install --accept-licence`)
    InstallPacks {
        #[serde(default)]
        names: Vec<String>,
    },
    /// render the song (stems too when asked) and judge it
    Render {
        #[serde(default)]
        stems: bool,
    },
    Undo,
    Redo,
    /// go back to the song as it was before a change the state's `history` lists (`to`, its `id`): that change and
    /// every one since are undone, and redo brings them back one at a time
    GoBack {
        to: String,
    },
    /// Edits of the song made as one change, which the history records as one and undo takes back together, `said`
    /// what undo calls it: what a request in plain words asked, as the palette read it and the person confirmed it
    /// ("Mute kick, snare and hat"). Edits only (`Command::is_edit`), none of undo, redo, go_back or another
    /// `together`; when one of them fails, the song stays as it was.
    Together {
        said: String,
        commands: Vec<Command>,
    },
    /// stop a running render / mutate / evolve job
    Cancel,
}

impl Command {
    /// whether it is an edit of the song, made at once and recorded as one change: what `together` makes as one
    pub fn is_edit(&self) -> bool {
        matches!(
            self,
            Command::SetTempo { .. }
                | Command::SetKey { .. }
                | Command::SetSwing { .. }
                | Command::SetProgression { .. }
                | Command::SetDrumPattern { .. }
                | Command::SetPart { .. }
                | Command::AddTrack { .. }
                | Command::Harmonize { .. }
                | Command::SwapSound { .. }
                | Command::GenerateTrack { .. }
                | Command::VaryTrack { .. }
                | Command::RemoveTrack { .. }
                | Command::SetNotes { .. }
                | Command::SetMix { .. }
                | Command::SetLane { .. }
                | Command::SetTrig { .. }
                | Command::RunScript { .. }
                | Command::SetSection { .. }
                | Command::AddSection { .. }
                | Command::RemoveSection { .. }
                | Command::MoveSection { .. }
                | Command::ToggleLayer { .. }
                | Command::SetDrumStep { .. }
                | Command::SetFx { .. }
                | Command::SetLayers { .. }
                | Command::AddHits { .. }
        )
    }
}

fn four() -> usize {
    4
}
fn three() -> usize {
    3
}
fn six() -> usize {
    6
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Reply {
    pub ok: bool,
    pub message: String,
    /// the state version after the command (or when a long job was started)
    pub version: u64,
}

/// What was said to the palette (`POST /api/hear`): the words as the speech model heard them, and as the palette is to
/// read them, the words claudius knows put right; both empty when nothing was said.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Heard {
    pub heard: String,
    pub text: String,
}

/// The answer to `analyze` (`POST /api/analyze`): the reply, whose message says what was heard in words, and what was
/// heard as the studio shows it, when the recording was heard
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Analyzed {
    #[serde(flatten)]
    pub reply: Reply,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard: Option<Listened>,
}

/// What claudius heard in a recording, as the studio shows it while the recording plays: its tempo, key, form, the
/// chord of each bar and its beat, and the recording and its parts to play
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Listened {
    /// the file it came in
    pub name: String,
    /// how long it is, in seconds
    pub secs: f64,
    pub bpm: f64,
    /// whether it keeps one tempo; else its bars follow a band that speeds up and slows down
    pub steady: bool,
    /// how clearly a beat is heard in it, 0 to 1
    pub pulse: f64,
    /// its key, "D minor", and how sure claudius is of it, 0 to 1
    pub key: String,
    pub key_confidence: f64,
    /// where its offbeat sixteenths fall: 0.5 straight, 0.66 a triplet's swing
    pub swing: f32,
    /// how loud it is, integrated, in LUFS
    pub lufs: f64,
    /// its bars in order
    pub bars: Vec<BarHeard>,
    /// where the last bar ends, in seconds
    pub end: f64,
    pub sections: Vec<SectionHeard>,
    /// its two progressions, as Roman numerals in its key, and the bars each chord lasts: what `prog` and
    /// `bars_per_chord` take
    pub progression_a: Vec<String>,
    pub progression_b: Vec<String>,
    pub bars_per_chord: u32,
    /// how often each lane strikes each sixteenth of a bar in its busiest sections, 0 to 1
    pub kick: Vec<f32>,
    pub snare: Vec<f32>,
    pub hats: Vec<f32>,
    /// how its bass plays, in words
    pub bass: String,
    /// the recording and the parts claudius took it apart into, kept to play while it is shown; none when nothing of it
    /// is kept (the demo's page plays the file it has)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<ListenedAudio>,
}

/// the audio of a recording claudius heard, until it hears another: `GET /api/listened/{id}/{track}` is each of
/// `tracks` as a WAV file ("recording", then the parts it was taken apart into: "vocals", "drums", "bass", "other")
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ListenedAudio {
    pub id: u64,
    pub tracks: Vec<String>,
}

/// a bar of a recording: where it starts, in seconds, and the chord heard in it, by name and as a Roman numeral in the
/// recording's key
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BarHeard {
    pub start: f64,
    pub chord: String,
    pub numeral: String,
}

/// a section of a recording: its first bar (counting from 0), how many bars, what kind ("intro", "verse", "drop" ...),
/// how much energy it has, 0 to 1, and what plays in it
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SectionHeard {
    pub start_bar: usize,
    pub bars: usize,
    pub kind: String,
    pub energy: f32,
    pub kick: bool,
    pub bass: bool,
    pub hats: bool,
    pub harmony: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_genre_is_found_by_its_names_its_family_and_its_sound() {
        let g = |name: &str, family: &str, aliases: &[&str], description: &str| GenreInfo {
            name: name.into(),
            family: family.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            description: description.into(),
            ..Default::default()
        };
        let genres = [
            g("trap", "hip hop", &["atlanta"], "Trap: 808s and rolling hats"),
            g("boom_bap", "hip hop", &["90s hip hop", "golden age"], "Boom bap: dusty drums"),
            g("rnb", "r&b, soul and funk", &["slow jam"], "R&B: lush chords"),
            g("cloud_rap", "hip hop", &["based"], "Cloud rap: hazy pads"),
            g("house", "house", &[], "House: four on the floor"),
            g("deep_house", "house", &[], "Deep house: warm chords"),
        ];
        let names = |q: &str| search_genres(&genres, q).iter().map(|g| g.name.as_str()).collect::<Vec<_>>();
        assert_eq!(names("rap"), ["cloud_rap"], "a word begins with it: trap is no rap");
        assert_eq!(names("hiphop"), ["boom_bap", "cloud_rap", "trap"], "the other name first, then the family");
        assert_eq!(names("Golden age hip"), ["boom_bap"]);
        assert_eq!(names("90s"), ["boom_bap"]);
        assert_eq!(names("R&B"), ["rnb"]);
        assert_eq!(names("r n b"), ["rnb"]);
        assert_eq!(names("house"), ["house", "deep_house"], "the genre of the name, then the family's others");
        assert_eq!(names("dusty"), ["boom_bap"], "by its sound");
        assert_eq!(names("gang"), Vec::<&str>::new());
        assert_eq!(names(" ").len(), genres.len());
    }

    #[test]
    fn the_genres_a_sentence_names_come_in_its_order_and_a_longer_name_wins() {
        let g = |name: &str, aliases: &[&str]| GenreInfo {
            name: name.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            ..Default::default()
        };
        let genres = [
            g("lofi", &["lo-fi hip hop", "chillhop"]),
            g("trip_hop", &["bristol"]),
            g("idm", &["braindance"]),
            g("house", &[]),
            g("deep_house", &[]),
            g("boom_bap", &["90s hip hop"]),
        ];
        let named = |s: &str| genres_named(&genres, s).iter().map(|(g, _)| g.name.as_str()).collect::<Vec<_>>();
        assert_eq!(genres_named(&genres, "a chill lo-fi beat")[0].1, 2..4, "where: a, chill, lo, fi, beat");
        assert_eq!(named("a chill lo-fi beat for studying"), ["lofi"]);
        assert_eq!(named("Trip hop with IDM drums"), ["trip_hop", "idm"]);
        assert_eq!(named("deep house, not house"), ["deep_house", "house"]);
        assert_eq!(named("something 90s hip hop"), ["boom_bap"], "the longest run names it");
        assert_eq!(named("house house"), ["house"], "each genre once");
        assert_eq!(named("dark and moody"), Vec::<&str>::new());
    }

    #[test]
    fn the_critics_targets_read_as_where_to_fix_it() {
        assert_eq!(fix_in_words("part:lead"), "the lead part");
        assert_eq!(fix_in_words("band:presence-"), "more presence in the mix");
        assert_eq!(fix_in_words("band:lowmid+"), "less low mids in the mix");
        assert_eq!(fix_in_words("level:kick+"), "kick quieter");
        assert_eq!(fix_in_words("level:pad-"), "pad louder");
        assert_eq!(fix_in_words("sound:pad"), "another sound for pad");
        assert_eq!(fix_in_words("width+"), "a wider stereo image");
        assert_eq!(fix_in_words("form"), "the arrangement");
        assert_eq!(fix_in_words("form:intro"), "a shorter intro");
        assert_eq!(fix_in_words("form:peaks-"), "the hook or the drop again");
        assert_eq!(fix_in_words("time:vox1"), "the timing of vox1");
        assert_eq!(fix_in_words("something new"), "something new");
        assert_eq!(severity_in_words(0.0), "minor");
        assert_eq!(severity_in_words(0.45), "worth fixing");
        assert_eq!(severity_in_words(0.9), "major");
        assert_eq!(score_in_words("arc").0, "Shape");
    }

    #[test]
    fn a_turntable_takes_its_records_from_samples_voices_parts_and_beats_not_a_lone_drum() {
        let t = |name: &str, kind: &str, role: &str| TrackView {
            name: name.into(),
            kind: kind.into(),
            role: role.into(),
            ..Default::default()
        };
        let song = SongView {
            tracks: vec![
                t("kick", "drum", "kick"),
                t("clap", "drum", "clap"),
                t("bass", "part", "bass"),
                t("lead", "part", "lead"),
                t("break", "loop", "drums"),
                t("sample", "loop", "sample"),
                t("vox1", "part", "vocal"),
                t("texture", "loop", "texture"),
                t("riser", "fx", "riser"),
                t("take1", "take", "lead"),
                t("hats", "pattern", "hat"),
            ],
            ..Default::default()
        };
        let all = records(&song);
        let got: Vec<(&str, &str)> = all.iter().map(|(n, k)| (n.as_str(), k.as_str())).collect();
        assert_eq!(
            got,
            [
                ("sample", "sample"),
                ("take1", "sample"),
                ("vox1", "voice"),
                ("bass", "part"),
                ("lead", "part"),
                ("break", "beat"),
                ("hats", "beat"),
                (WHOLE_MIX, "mix"),
            ]
        );
        assert!(records(&SongView::default()).is_empty());
    }

    fn song() -> SongView {
        let track = |name: &str, role: &str, bus: &str, fx: Vec<serde_json::Value>, reverb: f32, duck: f32| TrackView {
            name: name.into(),
            kind: "part".into(),
            role: role.into(),
            bus: bus.into(),
            fx,
            reverb,
            duck,
            ..TrackView::default()
        };
        SongView {
            title: "Glass".into(),
            genre: "house".into(),
            tracks: vec![
                track("kick", "kick", "drums", vec![], 0.0, 0.0),
                track(
                    "lead",
                    "lead",
                    "music",
                    vec![
                        json!({"fx": "saturate", "kind": "tape", "drive_db": 6, "mix": 0.5}),
                        json!({"fx": "delay", "beats": 0.75, "feedback": {"rest": 0.3, "sections": {}}, "mix": 0.25}),
                    ],
                    0.3,
                    0.2,
                ),
            ],
            sections: vec![SectionView {
                name: "drop".into(),
                layers: vec!["kick".into(), "lead".into()],
                ..SectionView::default()
            }],
            mix: MixView {
                buses: vec![("drums".into(), vec![json!({"fx": "compressor", "threshold_db": -12, "ratio": 2.5})])],
                master: vec![json!({"fx": "limiter", "ceiling_db": -1})],
                lufs: -10.0,
                ceiling_db: -1.0,
                reverb: json!({"fx": "reverb", "decay": 2.2, "mix": 1}),
                delay: json!({"fx": "delay", "beats": 0.75, "feedback": 0.35, "mix": 1}),
            },
            ..SongView::default()
        }
    }

    #[test]
    fn a_song_is_a_graph_from_its_tracks_to_the_finished_song() {
        let g = song().graph();
        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no {id}"));
        let edge = |a: &str, b: &str| g.edges.iter().find(|e| e.from == a && e.to == b).map(|e| e.kind.clone());
        assert_eq!(edge("track:lead", "track:lead#0"), Some(EdgeKind::Signal));
        assert_eq!(edge("track:lead#1", "bus:music"), Some(EdgeKind::Signal));
        assert_eq!(edge("track:lead#1", "return:reverb"), Some(EdgeKind::Send(0.3)));
        assert_eq!(edge("track:kick", "track:lead"), Some(EdgeKind::Sidechain(0.2)));
        assert_eq!(edge("bus:drums#0", "master"), Some(EdgeKind::Signal));
        assert_eq!(edge("master#0", "out"), Some(EdgeKind::Signal));
        assert!(g.nodes.iter().all(|n| n.id != "return:delay"), "no track sends to the delay");
        // the columns follow the signal: tracks, their effects, the buses, theirs, the master, its, out
        assert_eq!((node("track:lead").column, node("track:lead#1").column, node("bus:drums").column), (0, 2, 3));
        assert_eq!((node("bus:drums#0").column, node("master").column, node("out").column), (4, 5, 7));
        assert_eq!(node("track:lead#0").kind, NodeKind::Effect(Chain::Track("lead".into()), 0));
        assert_eq!(node("track:lead#1").title, "delay 0.75 beats, feedback by section, 25%");
        assert_eq!(node("track:lead#0").title, "saturate tape, 6 dB, 50%");
        assert!(node("track:lead").detail.iter().any(|d| d == "in drop"));
    }

    #[test]
    fn the_graph_reads_as_words() {
        let text = song().graph_text();
        for line in [
            "  lead -> saturate tape, 6 dB, 50% -> delay 0.75 beats, feedback by section, 25% -> music bus; sends reverb 0.30; ducked 0.20 by the kick; plays in drop",
            "  drums bus -> compressor -12 dB, 2.5:1 -> master",
            "  reverb return (reverb 2.2 s) -> master",
            "master: master -> limiter -1 dB -> out at -10.0 LUFS, ceiling -1.0 dBTP",
        ] {
            assert!(text.contains(line), "{line}\n---\n{text}");
        }
        assert_eq!(Chain::Send("reverb".into()).target(), "send:reverb");
        // a bus reads to the end of its chain, and a track whose name begins another's keeps to its own sends
        let mut s = song();
        s.mix.buses[0].1.push(json!({"fx": "saturate", "kind": "tape", "drive_db": 3, "mix": 0.3}));
        let lead2 = TrackView {
            name: "lead2".into(),
            role: "lead".into(),
            bus: "music".into(),
            delay: 0.4,
            ..TrackView::default()
        };
        s.tracks.push(lead2);
        let text = s.graph_text();
        for line in [
            "  drums bus -> compressor -12 dB, 2.5:1 -> saturate tape, 3 dB, 30% -> master\n",
            "  lead -> saturate tape, 6 dB, 50% -> delay 0.75 beats, feedback by section, 25% -> music bus; sends reverb 0.30;",
            "  lead2 -> music bus; sends delay 0.40",
        ] {
            assert!(text.contains(line), "{line}\n---\n{text}");
        }
    }
}
