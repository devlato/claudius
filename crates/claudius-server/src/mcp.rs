//! The MCP tools: what an assistant (Claude, ChatGPT, any MCP client) uses to make and edit songs. They work
//! through a `Backend`: the session in this process, or a claudius app that is already running (through its HTTP
//! API), so an assistant connected with `claudius mcp` edits the very song the person has open in the studio.

use std::sync::Arc;
use std::time::Duration;

use claudius_api::{Command, Reply, SoundList, State, TakeStatus};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::app::App;
use claudius_session::views;

#[derive(Clone)]
pub enum Backend {
    /// the session in this process; `base` is where its HTTP server (audio, web UI) answers
    Local { app: Arc<App>, base: String },
    /// a claudius app running elsewhere
    Remote { base: String, token: Option<String> },
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(timeout)).http_status_as_error(false).build().into()
}

fn remote_get(base: &str, token: &Option<String>, path: &str, timeout: Duration) -> Result<String, String> {
    let mut req = agent(timeout).get(format!("{base}{path}"));
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let mut res = req.call().map_err(|e| format!("cannot reach claudius at {base}: {e}"))?;
    let ok = res.status().is_success();
    let body = res.body_mut().read_to_string().map_err(|e| e.to_string())?;
    if ok { Ok(body) } else { Err(format!("claudius at {base}: {body}")) }
}

fn remote_json<T: DeserializeOwned>(
    base: &str,
    token: &Option<String>,
    path: &str,
    timeout: Duration,
) -> Result<T, String> {
    serde_json::from_str(&remote_get(base, token, path, timeout)?).map_err(|e| e.to_string())
}

/// what answers at an address
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// a claudius app that takes these credentials
    Claudius,
    /// a claudius app that wants a token: none was given, or another one
    NeedsToken,
    /// nothing, or something that is not claudius
    Nothing,
}

/// what is answering at `base`?
pub fn probe(base: &str, token: &Option<String>) -> Probe {
    let mut req = agent(Duration::from_millis(800)).get(format!("{base}/api/hello"));
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let Ok(mut res) = req.call() else { return Probe::Nothing };
    let status = res.status();
    let body = res.body_mut().read_to_string().unwrap_or_default();
    if status.is_success() && body.contains("\"claudius\"") {
        Probe::Claudius
    } else if status == 401 && body.contains("claudius needs its token") {
        Probe::NeedsToken
    } else {
        Probe::Nothing
    }
}

impl Backend {
    pub fn base_url(&self) -> &str {
        match self {
            Backend::Local { base, .. } | Backend::Remote { base, .. } => base,
        }
    }

    pub async fn command(&self, c: Command) -> Result<Reply, String> {
        match self {
            Backend::Local { app, .. } => {
                let app = app.clone();
                tokio::task::spawn_blocking(move || app.command(c)).await.map_err(|e| e.to_string())
            }
            Backend::Remote { base, token } => {
                let (base, token) = (base.clone(), token.clone());
                tokio::task::spawn_blocking(move || -> Result<Reply, String> {
                    let mut req = agent(Duration::from_secs(600)).post(format!("{base}/api/command"));
                    if let Some(t) = &token {
                        req = req.header("Authorization", format!("Bearer {t}"));
                    }
                    let mut res = req
                        .header("Content-Type", "application/json")
                        .send_json(&c)
                        .map_err(|e| format!("cannot reach claudius at {base}: {e}"))?;
                    let body = res.body_mut().read_to_string().map_err(|e| e.to_string())?;
                    serde_json::from_str(&body).map_err(|_| format!("claudius at {base}: {body}"))
                })
                .await
                .map_err(|e| e.to_string())?
            }
        }
    }

    pub async fn state(&self) -> Result<State, String> {
        match self {
            Backend::Local { app, .. } => Ok(app.state()),
            Backend::Remote { base, token } => {
                let (base, token) = (base.clone(), token.clone());
                tokio::task::spawn_blocking(move || remote_json(&base, &token, "/api/state", Duration::from_secs(30)))
                    .await
                    .map_err(|e| e.to_string())?
            }
        }
    }

    /// the state once its version is past `since`, or after `timeout` with whatever it is then
    pub async fn state_after(&self, since: u64, timeout: Duration) -> Result<State, String> {
        match self {
            Backend::Local { app, .. } => Ok(app.state_after(Some(since), timeout).await),
            Backend::Remote { base, token } => {
                let (base, token) = (base.clone(), token.clone());
                let path = format!("/api/state?since={since}&timeout={}", timeout.as_secs().max(1));
                tokio::task::spawn_blocking(move || {
                    remote_json(&base, &token, &path, timeout + Duration::from_secs(30))
                })
                .await
                .map_err(|e| e.to_string())?
            }
        }
    }

    /// the state once no job runs and the render is of the current song
    pub async fn settled(&self, timeout: Duration) -> Result<State, String> {
        match self {
            Backend::Local { app, .. } => Ok(app.settled(timeout).await),
            Backend::Remote { base, token } => {
                let (base, token) = (base.clone(), token.clone());
                let path = format!("/api/state?settled=1&timeout={}", timeout.as_secs());
                tokio::task::spawn_blocking(move || {
                    remote_json(&base, &token, &path, timeout + Duration::from_secs(30))
                })
                .await
                .map_err(|e| e.to_string())?
            }
        }
    }

    pub async fn script(&self) -> Result<String, String> {
        match self {
            Backend::Local { app, .. } => Ok(app.script()),
            Backend::Remote { base, token } => {
                let (base, token) = (base.clone(), token.clone());
                tokio::task::spawn_blocking(move || remote_get(&base, &token, "/api/script", Duration::from_secs(30)))
                    .await
                    .map_err(|e| e.to_string())?
            }
        }
    }

    pub async fn sounds(&self) -> Result<SoundList, String> {
        match self {
            Backend::Local { app, .. } => Ok(app.sounds()),
            Backend::Remote { base, token } => {
                let (base, token) = (base.clone(), token.clone());
                tokio::task::spawn_blocking(move || remote_json(&base, &token, "/api/sounds", Duration::from_secs(30)))
                    .await
                    .map_err(|e| e.to_string())?
            }
        }
    }
}

fn yes() -> bool {
    true
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NewSongArgs {
    /// the genre: house, deep_house, techno, trap, lofi, dnb, future_bass, synthwave, trip_hop, big_beat,
    /// electro_house, nu_disco, filter_house, dance_pop, neo_psychedelia, art_rock, idm, dub_techno, boom_bap,
    /// uk_drill, dubstep, uk_garage, jungle, afrobeats, amapiano, reggaeton, trance or neo_soul (catalog lists them
    /// with their tempo ranges, and any genres of the person's own); or two or three fused, such as trip_hop+idm:
    /// the first leads (tempo, harmony, form, parts, mix), the last plays the drums and lends its sounds
    pub genre: String,
    /// random seed: the same seed and genre give the same song
    #[serde(default)]
    pub seed: Option<u64>,
    /// tempo in BPM (default: picked from the genre's range)
    #[serde(default)]
    pub bpm: Option<f64>,
    /// e.g. "A minor", "F#m", "Eb major", "D dorian" (default: picked from the genre)
    #[serde(default)]
    pub key: Option<String>,
    /// build the song around a family of Apple Loops recorded together (GarageBand style), when installed
    #[serde(default)]
    pub loops: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScriptArgs {
    /// a claudius script (see script_reference), e.g. `tempo(124); track("hats", s("[~ hh]*4").gain(0.7));`
    pub code: String,
    /// render and judge after the change (default true; set false to batch several edits quickly)
    #[serde(default = "yes")]
    pub listen: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListenArgs {
    /// also render every track on its own (for stems in the studio or the saved folder)
    #[serde(default)]
    pub stems: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MutateArgs {
    /// how many variations to try (1 to 32)
    #[serde(default = "four")]
    #[schemars(range(min = 1, max = 32))]
    pub count: usize,
    /// keep the best variation when it beats the current song (false: only report the scores)
    #[serde(default = "yes")]
    pub keep_best: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EvolveArgs {
    /// generations (1 to 20)
    #[serde(default = "three")]
    #[schemars(range(min = 1, max = 20))]
    pub generations: usize,
    /// children per generation (2 to 32)
    #[serde(default = "six")]
    #[schemars(range(min = 2, max = 32))]
    pub children: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PathArgs {
    /// a path on the computer running claudius
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveArgs {
    /// folder to write into (default: Music/claudius/<genre>-<title>-<seed>)
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExportArgs {
    /// the formats to write, each format[:quality][@rate]: "mp3", "mp3:v0", "wav:16", "flac", "aac", "opus:96"
    pub formats: Vec<String>,
    /// folder to write into (default: the song's folder, Music/claudius/<genre>-<title>-<seed>)
    #[serde(default)]
    pub path: Option<String>,
}

/// What stores ask about a release. Give only what the person told you; what is left out stays as it was.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct PublishArgs {
    /// the name the song comes out under
    #[serde(default)]
    pub artist: Option<String>,
    /// the title, when it is not the song's
    #[serde(default)]
    pub title: Option<String>,
    /// a version of the song, such as "Extended Mix" or "Radio Edit"
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub featuring: Option<Vec<String>>,
    /// the album or EP the song is on; without one it is a single
    #[serde(default)]
    pub album: Option<String>,
    /// the store genre ("Dance", "Electronic", "Hip-Hop/Rap") and subgenre
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub subgenre: Option<String>,
    /// the release date, YYYY-MM-DD
    #[serde(default)]
    pub date: Option<String>,
    /// the record label (the artist when there is none)
    #[serde(default)]
    pub label: Option<String>,
    /// the songwriters' full legal names
    #[serde(default)]
    pub writers: Option<Vec<String>>,
    #[serde(default)]
    pub producers: Option<Vec<String>>,
    /// who owns the recording and the composition, for the ℗ and © lines (the artist when empty)
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub upc: Option<String>,
    #[serde(default)]
    pub explicit: Option<bool>,
    /// how the song was made, in the person's own answer: written (they wrote it), assisted (you or claudius'
    /// generator wrote some of it and they shaped it) or generated (you or the generator wrote it). Never choose it
    #[serde(default)]
    pub made: Option<String>,
    /// the artwork file the person gave: a square JPEG or PNG, 3000 × 3000
    #[serde(default)]
    pub artwork: Option<String>,
    /// write the release kit (without it, only check)
    #[serde(default)]
    pub write: bool,
    /// the kit's folder (default: release/ in the song's folder)
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemakeArgs {
    /// the audio file to learn from
    pub path: String,
    /// the genre for the sounds and production, one of those new_song takes (default: the open song's)
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    /// the tempo (default: the recording's, halved or doubled into the genre's range when it is outside it)
    #[serde(default)]
    pub bpm: Option<f64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TakeArgs {
    /// the recording: an audio file (WAV, FLAC, MP3, ...) made at the song's tempo, starting on the section's downbeat
    pub path: String,
    /// the section it starts with, by name (get_song lists them)
    pub from: String,
    /// the track's name: letters, digits, - and _ (default take1, take2, ...)
    #[serde(default)]
    pub name: Option<String>,
    /// vocal, lead, chords, keys, bass, drums or texture: how loud the mix makes it (default lead)
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SampleArgs {
    /// the recording to sample: an audio file (WAV, FLAC, MP3, AAC, Ogg, AIFF)
    pub path: String,
    /// how many phrases, 1 to 4 (default 2)
    #[serde(default)]
    pub count: Option<u32>,
    /// how long each phrase is, in bars: 1, 2 or 4 (default: whichever of 1 and 2 sounds best)
    #[serde(default)]
    pub bars: Option<u32>,
    /// what to listen for (default "a person singing"; "a rap verse", "a choir" ...)
    #[serde(default)]
    pub find: Option<String>,
    /// the sections to put the phrases in, by name or kind (default: those that suit each phrase)
    #[serde(default)]
    pub sections: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssessVoiceArgs {
    /// a take of the song, by name (get_song lists the tracks; takes are kind "take")
    #[serde(default)]
    pub take: Option<String>,
    /// or an audio file of someone singing (WAV, FLAC, MP3, AAC, Ogg, AIFF)
    #[serde(default)]
    pub path: Option<String>,
    /// the key a file is sung in ("A minor"), which settles doubtful notes (a take is heard in the song's)
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TuneArgs {
    /// the track to tune: a take, a sampled vocal, any track of one voice
    pub track: String,
    /// how much of the way to the notes, 0 to 1 (default 1)
    #[serde(default)]
    pub strength: Option<f32>,
    /// how fast a note is brought onto its pitch after it starts, ms: 40 is natural (the default), 10 tight, 0 the
    /// hard-tuned sound
    #[serde(default)]
    pub speed_ms: Option<f32>,
    /// the cents of the singer's own wandering around a note that stay, as expression (default 10; 0 for hard tuning)
    #[serde(default)]
    pub humanize: Option<f32>,
    /// how much of the vibrato stays, 0 to 1 (default 1; 0 for hard tuning)
    #[serde(default)]
    pub vibrato: Option<f32>,
    /// what the notes are read against: chords (the song's key and the chord under each note, the default), key,
    /// chromatic, or the notes to go to ("C D E G A")
    #[serde(default)]
    pub scale: Option<String>,
    /// take the tuner out
    #[serde(default)]
    pub off: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GrooveArgs {
    /// a recording with drums in it (a loop, a song, a break): WAV, FLAC, MP3, AAC, Ogg, AIFF
    pub path: String,
    /// what to call the groove in the song (default: the file's name)
    #[serde(default)]
    pub name: Option<String>,
    /// the drum lanes to put in it (default: all of them)
    #[serde(default)]
    pub lanes: Vec<String>,
    /// its tempo, when known (default: an Apple Loop's, or one written in the file's name such as "92bpm", or else
    /// heard; a loop's tempo heard can come out twice or half what it is)
    #[serde(default)]
    pub bpm: Option<f64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PitchCheckArgs {
    /// the lowest and highest notes played, MIDI (default 57 and 69, A3 to A4); the person sings them in whatever
    /// octave suits them, so the default suits most voices
    #[serde(default)]
    pub low: Option<i32>,
    #[serde(default)]
    pub high: Option<i32>,
    /// what to tell the person, shown in the window ("so that I know how to tune your takes")
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaitCheckArgs {
    /// the pitch check's id, as request_pitch_check gave it
    pub id: u64,
    /// how long to wait at most before saying the person is still at it, seconds (default 50)
    #[serde(default)]
    pub seconds: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RequestTakeArgs {
    /// the section the take starts with, by name (get_song lists them)
    pub from: String,
    /// vocal, lead, chords, keys, bass, drums or texture for a recording (how loud the mix makes it); keys, lead,
    /// chords, pad, arp or bass for a part played on a MIDI keyboard
    #[serde(default)]
    pub role: Option<String>,
    /// the track's name: letters, digits, - and _ (default take1, take2 ... or played)
    #[serde(default)]
    pub name: Option<String>,
    /// what to record, in a few words the Record window shows the person ("the chorus melody, sung softly")
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaitTakeArgs {
    /// the request's id, as request_take gave it
    pub id: u64,
    /// how long to wait at most, in seconds (default 50, at most 300)
    #[serde(default)]
    pub seconds: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NoteArg {
    /// where it starts, in beats from the section's start
    pub t: f64,
    /// how long it lasts, in beats
    pub d: f64,
    /// its MIDI pitch: 60 is middle C, 69 the A above it
    pub p: i32,
    /// how hard it is played, 0..1 (default 0.8)
    #[serde(default)]
    pub v: Option<f32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NotesArgs {
    /// the section the notes start with, by name (get_song lists them)
    pub from: String,
    /// the notes, several at once for a chord
    pub notes: Vec<NoteArg>,
    /// what plays them, one new part each: synth presets or installed instruments (catalog lists them)
    pub sounds: Vec<String>,
    /// the first part's name: letters, digits, - and _ (default played)
    #[serde(default)]
    pub name: Option<String>,
    /// lead (default), keys, chords, pad, arp or bass
    #[serde(default)]
    pub role: Option<String>,
    /// what the notes become: line (as they are, the default), harmony (a third above), octaves (doubled below),
    /// chords (a triad under each note), bass (down in a bass's register), arp (each note broken into a rising
    /// arpeggio) or pad (a chord held through each bar the line sounds in)
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TranscribeArgs {
    /// the take's name (get_song lists its tracks; a take's kind is "take")
    pub take: String,
    /// what plays the melody, one new part each: synth presets or installed instruments (catalog lists them)
    pub sounds: Vec<String>,
    /// the first part's name: letters, digits, - and _ (default melody; the next ones melody2, melody3 ...)
    #[serde(default)]
    pub name: Option<String>,
    /// lead (default), keys, chords, pad, arp or bass
    #[serde(default)]
    pub role: Option<String>,
    /// keep notes outside the song's key where they were sung (default: each goes to the key's nearest tone)
    #[serde(default)]
    pub chromatic: bool,
    /// let the take go on playing beside the new parts (default: it is muted)
    #[serde(default)]
    pub keep_take: bool,
    /// what the melody becomes: line (as sung, the default), harmony, octaves, chords, bass, arp or pad (as add_notes)
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DrumsArgs {
    /// a take of the song with a beat in it, played or beatboxed (get_song lists its tracks; a take's kind is "take")
    #[serde(default)]
    pub take: Option<String>,
    /// or a recording with a beat in it (a drum loop, a break): WAV, FLAC, MP3, AAC, Ogg, AIFF
    #[serde(default)]
    pub path: Option<String>,
    /// the pattern's length in bars: 1, 2 or 4 (default: the recording's, up to 4)
    #[serde(default)]
    pub bars: Option<u32>,
    /// the lanes' variant to write (default main)
    #[serde(default)]
    pub variant: Option<String>,
    /// a recording's tempo, when known (default: one in its file name, such as "92bpm", or heard); a take's is the
    /// song's
    #[serde(default)]
    pub bpm: Option<f64>,
    /// let the take go on playing beside the drums (default: it is muted)
    #[serde(default)]
    pub keep_take: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CommandArgs {
    /// one command object, e.g. {"cmd": "add_section", "after": 2, "kind": "break", "bars": 8}
    pub command: serde_json::Value,
    /// render and judge after it (default true)
    #[serde(default = "yes")]
    pub listen: bool,
}

#[derive(Clone)]
pub struct Studio {
    b: Backend,
}

fn new_lines(before: &[String], after: &[String]) -> Vec<String> {
    let start = match before.last() {
        Some(last) => after.iter().rposition(|l| l == last).map_or(0, |i| i + 1),
        None => 0,
    };
    after[start.min(after.len())..].to_vec()
}

impl Studio {
    pub fn new(b: Backend) -> Self {
        Studio { b }
    }

    fn describe(&self, st: &State) -> String {
        let Some(song) = &st.song else { return "No song is open yet: start one with new_song.".into() };
        let mut out = views::summary(song, st.verdict.as_ref(), st.render.as_ref());
        if let Some(r) = &st.render {
            out.push_str(&format!(
                "audio: {}{} (the studio at {} plays it)\n",
                self.b.base_url(),
                r.url.replace(".mp3", ".wav"),
                self.b.base_url()
            ));
        }
        if let Some(b) = &st.busy {
            out.push_str(&format!(
                "still busy: {b}{}\n",
                st.progress.map(|p| format!(" ({:.0} %)", p * 100.0)).unwrap_or_default()
            ));
        }
        out
    }

    async fn heard(&self, stems: bool) -> Result<State, String> {
        let r = self.b.command(Command::Render { stems }).await?;
        if !r.ok {
            return Err(r.message);
        }
        self.b.settled(Duration::from_secs(240)).await
    }

    async fn edit(&self, c: Command, listen: bool) -> Result<String, String> {
        let r = self.b.command(c).await?;
        if !r.ok {
            return Err(r.message);
        }
        if !listen {
            return Ok(r.message);
        }
        let st = self.heard(false).await?;
        Ok(format!("{}\n\n{}", r.message, self.describe(&st)))
    }

    async fn job(&self, c: Command, timeout: u64) -> Result<String, String> {
        let before = self.b.state().await?.log;
        let r = self.b.command(c).await?;
        if !r.ok {
            return Err(r.message);
        }
        let st = self.b.settled(Duration::from_secs(timeout)).await?;
        let lines = new_lines(&before, &st.log);
        let mut out = lines.join("\n");
        if st.busy.is_some() {
            out.push_str("\n\nThe job is still running: call status to follow it, or stop to end it.");
        } else {
            out.push_str("\n\n");
            out.push_str(&self.describe(&st));
        }
        Ok(out)
    }
}

#[tool_router]
impl Studio {
    #[tool(
        description = "Start a new song: claudius composes it in a genre the way a producer would (drums, bass, chords, pad, lead, arrangement with builds and drops, mix and master), renders it with real synthesized or recorded sounds, and judges it. Returns the song and the critic's verdict."
    )]
    async fn new_song(&self, Parameters(a): Parameters<NewSongArgs>) -> Result<String, String> {
        self.edit(Command::NewSong { genre: a.genre, seed: a.seed, bpm: a.bpm, key: a.key, loops: a.loops }, true).await
    }

    #[tool(
        description = "The song that is open: tempo, key, progressions, the sections with the tracks playing in each, every track's sound and mix, the latest verdict, and the whole song as a claudius script (running that script on the song changes nothing; edit a copy of it to change the song).",
        annotations(read_only_hint = true)
    )]
    async fn get_song(&self) -> Result<String, String> {
        let st = self.b.state().await?;
        let script = self.b.script().await?;
        Ok(format!("{}\nscript:\n```js\n{script}```", self.describe(&st)))
    }

    #[tool(
        description = "Edit the song with a claudius script: Rhai with Strudel-style patterns. Song edits: tempo(), key(), swing(), prog(), bars_per_chord(), drum(), drum_sound(), part(), mix(), mute(), track(name, pattern), form([sec(...)]), section(), layer(). Patterns: s(\"bd*4, [~ cp]*2\"), note(\"<a1 f1>\").s(\"808\"), chord(\"prog\").voicing().s(\"epiano\"), chord(\"prog\").root().s(\"808\"), n(\"0 2 4\").scale(\"A:minor\") with .fast() .every() .sometimes() .struct() .gain() .room() .delay() and more. Read script_reference before the first script. Renders and judges the result unless listen is false."
    )]
    async fn run_script(&self, Parameters(a): Parameters<ScriptArgs>) -> Result<String, String> {
        self.edit(Command::RunScript { code: a.code }, a.listen).await
    }

    #[tool(
        description = "The claudius script language reference: song edits, the pattern functions and mini-notation, sound names, examples.",
        annotations(read_only_hint = true)
    )]
    async fn script_reference(&self) -> String {
        claudius::script::REFERENCE.to_string()
    }

    #[tool(
        description = "Render the song and judge it: loudness and true peak, scores (mix, arc, theory, genre, production), issues with where to fix them, and what earshot hears (genre fit against its usual confusions, production quality against typical flaws, what it sounds like). The person hears the render in the claudius studio."
    )]
    async fn listen(&self, Parameters(a): Parameters<ListenArgs>) -> Result<String, String> {
        let st = self.heard(a.stems).await?;
        Ok(self.describe(&st))
    }

    #[tool(
        description = "Try a few variations of the song, the edits a producer would try (drum patterns, sounds, progressions, arrangement, mix), biased toward the critic's issues; judge each on a preview and keep the best one when it beats the current song."
    )]
    async fn mutate(&self, Parameters(a): Parameters<MutateArgs>) -> Result<String, String> {
        self.job(Command::Mutate { count: a.count, keep_best: a.keep_best }, 300).await
    }

    #[tool(
        description = "Evolve the song: generations of mutations and crossovers judged by the critic, keeping the best. About 10-40 s per generation; if it outlasts the call, follow it with status."
    )]
    async fn evolve(&self, Parameters(a): Parameters<EvolveArgs>) -> Result<String, String> {
        self.job(Command::Evolve { generations: a.generations, children: a.children }, 280).await
    }

    #[tool(
        description = "What claudius is doing (the running job and its progress), the recent log and the latest verdict.",
        annotations(read_only_hint = true)
    )]
    async fn status(&self) -> Result<String, String> {
        let st = self.b.state().await?;
        let log: Vec<String> = st.log.iter().rev().take(25).rev().cloned().collect();
        let mut out = match &st.busy {
            Some(b) => {
                format!("busy: {b}{}\n", st.progress.map(|p| format!(" ({:.0} %)", p * 100.0)).unwrap_or_default())
            }
            None => "idle\n".into(),
        };
        out.push_str(&format!("log:\n{}\n", log.join("\n")));
        if let Some(v) = &st.verdict {
            out.push_str(&views::verdict_text(v));
        }
        Ok(out)
    }

    #[tool(description = "Undo the last change to the song.")]
    async fn undo(&self) -> Result<String, String> {
        self.edit(Command::Undo, true).await
    }

    #[tool(description = "Redo the last undone change.")]
    async fn redo(&self) -> Result<String, String> {
        self.edit(Command::Redo, true).await
    }

    #[tool(
        description = "Stop the running job after its current step: a mutation after the candidate in progress, an evolution after its generation, a pack download after the pack in progress. A render always finishes."
    )]
    async fn stop(&self) -> Result<String, String> {
        Ok(self.b.command(Command::Cancel).await?.message)
    }

    #[tool(
        description = "What there is to work with: the genres (with tempo ranges and descriptions), drum names, synth presets and installed instruments.",
        annotations(read_only_hint = true)
    )]
    async fn catalog(&self) -> Result<String, String> {
        let st = self.b.state().await?;
        let s = self.b.sounds().await?;
        let mut out = String::from("genres:\n");
        for g in &st.genres {
            out.push_str(&format!("  {} ({:.0}-{:.0} BPM): {}\n", g.name, g.bpm.0, g.bpm.1, g.description));
        }
        out.push_str(&format!("drums: {}\nsynths: {}\n", s.drums.join(" "), s.synths.join(" ")));
        if !s.instruments.is_empty() {
            out.push_str(&format!("instruments: {}\n", s.instruments.join(" ")));
        }
        for p in &s.packs {
            out.push_str(&format!(
                "  pack {} ({}, {}{}): {}\n",
                p.name,
                p.license,
                if p.installed { "installed" } else { "not installed" },
                if p.size_mb > 0.0 { format!(", {:.0} MB", p.size_mb) } else { String::new() },
                p.description
            ));
        }
        Ok(out)
    }

    #[tool(
        description = "Save the song into a folder: blueprint.json, song.rhai (the song as a script), song.wav, song.mp3, stems/ and the verdict."
    )]
    async fn save(&self, Parameters(a): Parameters<SaveArgs>) -> Result<String, String> {
        self.job(Command::Save { path: a.path }, 240).await
    }

    #[tool(
        description = "Export the song as audio files: wav (24-bit, 48 kHz), wav:16 (CD: 16-bit, 44.1 kHz), wav:32 (float), aiff, flac, flac:16, alac (Apple Lossless), mp3 (320 kbps), mp3:v0, mp3:192, mp3:128, aac (256 kbps .m4a), opus (160 kbps, the smallest for the web), opus:96, ogg (Vorbis). Each is format[:quality][@rate]. Writes them into the song's folder (or path), named after the song, rendering first when the song changed since its last render."
    )]
    async fn export(&self, Parameters(a): Parameters<ExportArgs>) -> Result<String, String> {
        let st = self.b.state().await?;
        if st.render.as_ref().is_none_or(|r| r.version < st.song_version) {
            self.heard(false).await?;
        }
        let r = self.b.command(Command::Export { formats: a.formats, path: a.path }).await?;
        if r.ok { Ok(r.message) } else { Err(r.message) }
    }

    #[tool(
        description = "Check the song for release as stores and distributors do (its artwork, its audio, the licences of its sounds, its details), and with write, write a release kit: the WAVs and the cover a distributor takes, and release.txt with the answers its form asks for. Give only what the person told you: the artist, the songwriters' full legal names, the artwork file, and how the song was made, which you must never choose for them. Renders first when the song changed. Problems (--) stop the kit."
    )]
    async fn publish(&self, Parameters(a): Parameters<PublishArgs>) -> Result<String, String> {
        let st = self.b.state().await?;
        let mut d = st.release.map(|r| r.details).ok_or("no song to release: make one first")?;
        let set = |field: &mut String, v: Option<String>| {
            if let Some(v) = v {
                *field = v;
            }
        };
        set(&mut d.artist, a.artist);
        set(&mut d.title, a.title);
        set(&mut d.version, a.version);
        set(&mut d.album, a.album);
        set(&mut d.genre, a.genre);
        set(&mut d.subgenre, a.subgenre);
        set(&mut d.date, a.date);
        set(&mut d.label, a.label);
        set(&mut d.owner, a.owner);
        set(&mut d.isrc, a.isrc);
        set(&mut d.upc, a.upc);
        for (list, given) in
            [(&mut d.featuring, a.featuring), (&mut d.writers, a.writers), (&mut d.producers, a.producers)]
        {
            if let Some(g) = given {
                *list = g;
            }
        }
        if let Some(e) = a.explicit {
            d.explicit = e;
        }
        if a.made.is_some() {
            d.made = a.made;
        }
        let r = self.b.command(Command::SetRelease { details: Box::new(d), artwork: a.artwork }).await?;
        if !r.ok {
            return Err(r.message);
        }
        let st = match st.render.as_ref().is_some_and(|r| r.version >= st.song_version) {
            true => self.b.state().await?,
            false => self.heard(false).await?,
        };
        let release = st.release.ok_or("no song to release")?;
        let problems = release.checks.iter().filter(|c| c.level == "problem").count();
        let mut text = match (release.checked, problems) {
            (0, _) => "not checked yet: the song has no render".to_string(),
            (_, 0) => "ready for a distributor".to_string(),
            (_, n) => format!("{n} problem(s), marked --, to fix before a distributor takes it"),
        };
        if !release.checks.is_empty() {
            text = format!("{text}\n{}", crate::app::report_lines(&release.checks));
        }
        if a.write {
            let r = self.b.command(Command::WriteKit { path: a.path }).await?;
            text = format!("{text}\n\n{}", r.message);
            if !r.ok {
                return Err(text);
            }
        }
        Ok(text)
    }

    #[tool(description = "Open a saved song: a folder or blueprint.json written by save, or a .rhai script.")]
    async fn open(&self, Parameters(a): Parameters<PathArgs>) -> Result<String, String> {
        self.edit(Command::Load { path: a.path }, true).await
    }

    #[tool(
        description = "Analyze an audio file (any song or loop, any format ffmpeg reads): tempo, key, chord progression, sections with energy, the drum pattern, the bass style.",
        annotations(read_only_hint = true)
    )]
    async fn analyze(&self, Parameters(a): Parameters<PathArgs>) -> Result<String, String> {
        let r = self.b.command(Command::Analyze { path: a.path }).await?;
        if r.ok { Ok(r.message) } else { Err(r.message) }
    }

    #[tool(
        description = "Make a new song from an audio file: claudius analyzes it (tempo, key, progression, form, groove) and composes its own song with that skeleton in a genre's sounds and production."
    )]
    async fn remake(&self, Parameters(a): Parameters<RemakeArgs>) -> Result<String, String> {
        self.edit(Command::Remake { path: a.path, genre: a.genre, seed: a.seed, bpm: a.bpm }, true).await
    }

    #[tool(
        description = "Add a recording to the song as a take: an audio file someone recorded for it (a vocal, a guitar, a sung melody), which plays once from the start of section `from` and on through the sections after it for as long as it lasts, at the song's tempo. claudius keeps a copy with the song, mixes the take by its role and judges it with the rest. It turns off like any track (toggle_layer)."
    )]
    async fn add_take(&self, Parameters(a): Parameters<TakeArgs>) -> Result<String, String> {
        self.edit(Command::AddTake { path: a.path, from: a.from, name: a.name, role: a.role }, true).await
    }

    #[tool(
        description = "The song as a graph of its signal, in words: each track through its chain of effects into its bus (with its sends, its ducking under the kick, and the sections it plays in), each bus through its effects into the master, the reverb and delay returns, and the master's chain out at its loudness target. Each effect in a few words; get_song's script has every setting. To change the graph: run_script with fx(), bus(), master(#{ fx }) and send_fx(), or command with set_fx (a chain as JSON: target track:<name>, bus:<name>, master, send:reverb or send:delay) and set_mix (bus, sends, duck).",
        annotations(read_only_hint = true)
    )]
    async fn graph(&self) -> Result<String, String> {
        let st = self.b.state().await?;
        Ok(st.song.ok_or("no song yet: start one with new_song")?.graph_text())
    }

    #[tool(
        description = "Sample a recording's vocals into the song: claudius finds the phrases of the recording that most sound like a voice singing (or `find`), cuts them on the recording's beat grid, cleans them of its drums and of what plays to the sides, fits them to the song's tempo and key, places each in the sections that suit it (the drops and hooks, then the breaks ...) from the bar where its notes agree best with the song's chords, every so many bars, and mixes them as vocals (vox1, vox2 ...; a lead playing with them steps back 1.5 dB). Says where each phrase came from and went, then listens. The phrases are kept with the song; undo takes them out."
    )]
    async fn sample_song(&self, Parameters(a): Parameters<SampleArgs>) -> Result<String, String> {
        let c = Command::SampleSong { path: a.path, count: a.count, bars: a.bars, find: a.find, sections: a.sections };
        self.edit(c, true).await
    }

    #[tool(
        description = "How a take of the song or a recording is sung: the notes it means (read with the song's key and chords for a take, or `key` for a file, and with how this singer sings), how many land within 25 and 50 cents of the note meant, the furthest off, and the singer: their range, where they sing sharp or flat at the bottom, the middle and the top of it, how widely their held notes scatter, how fast they settle on a note, their vibrato. Changes nothing.",
        annotations(read_only_hint = true)
    )]
    async fn assess_voice(&self, Parameters(a): Parameters<AssessVoiceArgs>) -> Result<String, String> {
        let r = self.b.command(Command::AssessVoice { take: a.take, path: a.path, key: a.key }).await?;
        if r.ok { Ok(r.message) } else { Err(r.message) }
    }

    #[tool(
        description = "Tune a track (a take, a sampled vocal) to the notes it means: a tune effect first in its chain, or the one there changed. The tuner reads the notes the voice means against the song's key and the chord under each (or `scale`), learns from the take how its singer sings, and moves each note onto its pitch as far as the singer needs: their usual error goes, and of their scatter only `humanize` cents stay. `speed_ms` 40 keeps the scoop into each note, 0 snaps it (with humanize 0 and vibrato 0: the hard-tuned, robotic sound). The voice keeps its own character. `off` takes the tuner out. It sounds once the song renders; assess_voice says how the take was sung before it."
    )]
    async fn tune(&self, Parameters(a): Parameters<TuneArgs>) -> Result<String, String> {
        let c = Command::Tune {
            track: a.track,
            strength: a.strength,
            speed_ms: a.speed_ms,
            humanize: a.humanize,
            vibrato: a.vibrato,
            scale: a.scale,
            off: a.off,
        };
        self.edit(c, true).await
    }

    #[tool(
        description = "Ask the person to record a take for the song (a vocal, a guitar, a part on a MIDI keyboard ...) that starts with section `from`. The studio's Record window opens for it, showing `note`; the person records it over the song, listens back, records it again as often as they like, and sends it, or says not now. Returns at once with the request's id and the link that opens the studio, for when none is open; then call wait_for_take with the id until the take arrives. The take is added as add_take adds one (or, from a keyboard, as add_notes adds notes)."
    )]
    async fn request_take(&self, Parameters(a): Parameters<RequestTakeArgs>) -> Result<String, String> {
        let c = Command::RequestTake { from: a.from, role: a.role, name: a.name, note: a.note.unwrap_or_default() };
        let r = self.b.command(c).await?;
        if !r.ok {
            return Err(r.message);
        }
        let id = self.b.state().await?.take_request.map(|t| t.id).unwrap_or_default();
        Ok(format!(
            "{}. A studio that is open shows its Record window now; otherwise the person opens {}/?record={id} in a \
             browser, or runs claudius studio. Call wait_for_take with id {id}: it returns when the take arrives, or \
             after a while to say the person is still at it.",
            r.message,
            self.b.base_url()
        ))
    }

    #[tool(
        description = "Take the groove of a recording's drums: claudius finds its tempo and beat grid, measures where each sixteenth of the bar lands against the grid and how hard (its swing, a backbeat played behind the beat, its accents), keeps that as a groove of the song, and puts the drum lanes in it (`lanes`, or all of them). Says what it heard. Give `bpm` when the tempo is known: heard, a loop's can come out twice or half what it is. A lane's groove is a lane() setting afterwards: lane(\"hat\", #{ groove: \"name\", groove_amount: 0.5 }), or one of claudius' own (straight, swing54 to swing71, shuffle, dilla, push, lazy)."
    )]
    async fn extract_groove(&self, Parameters(a): Parameters<GrooveArgs>) -> Result<String, String> {
        self.edit(Command::ExtractGroove { path: a.path, name: a.name, lanes: a.lanes, bpm: a.bpm }, true).await
    }

    #[tool(
        description = "Ask the person for a pitch check: the studio's Record window opens with six short phrases of four notes, each played and then sung back by the person in the gap after it, in any octave (about a minute). claudius hears how near each note came and learns their voice: their range, where they go sharp or flat, how widely they scatter, steps against leaps, their vibrato; tune then sings their takes as it heard them. Returns at once with the check's id and the link that opens the studio; then call wait_for_pitch_check with the id."
    )]
    async fn request_pitch_check(&self, Parameters(a): Parameters<PitchCheckArgs>) -> Result<String, String> {
        let r = self
            .b
            .command(Command::RequestPitchCheck { low: a.low, high: a.high, note: a.note.unwrap_or_default() })
            .await?;
        if !r.ok {
            return Err(r.message);
        }
        let id = self.b.state().await?.pitch_check.map(|c| c.id).unwrap_or_default();
        Ok(format!(
            "{}. A studio that is open shows its Record window now; otherwise the person opens {}/?record=1 in a \
             browser, or runs claudius studio. Call wait_for_pitch_check with id {id}.",
            r.message,
            self.b.base_url()
        ))
    }

    #[tool(
        description = "Wait for the pitch check asked for with request_pitch_check: returns what claudius heard once the person has sung it (how many notes, how near, in which octave, steps against leaps, and their voice), says so if they will not do it now, or, after `seconds` (default 50), that they are still at it: call it again then.",
        annotations(read_only_hint = true)
    )]
    async fn wait_for_pitch_check(&self, Parameters(a): Parameters<WaitCheckArgs>) -> Result<String, String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(a.seconds.unwrap_or(50).clamp(1, 300));
        let mut st = self.b.state().await?;
        loop {
            let Some(c) = st.pitch_check.clone().filter(|c| c.id == a.id) else {
                return Err(match st.pitch_check {
                    Some(newer) if newer.id > a.id => format!("pitch check {} gave way to check {}", a.id, newer.id),
                    _ => format!("no pitch check {}: ask with request_pitch_check", a.id),
                });
            };
            match c.status {
                TakeStatus::Taken => return Ok(format!("the pitch check is in: {}", c.answer.unwrap_or_default())),
                TakeStatus::Cancelled => {
                    return Ok(format!("the person will not do the pitch check now (check {})", a.id));
                }
                TakeStatus::Waiting => {}
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Ok(format!(
                    "the person has not sung the pitch check yet (check {}): call wait_for_pitch_check again",
                    a.id
                ));
            }
            st = self.b.state_after(st.version, left.min(Duration::from_secs(20))).await?;
        }
    }

    #[tool(
        description = "Wait for the take asked for with request_take: returns what claudius made of it once the person sends it (then listen and set_mix as after add_take), says so if they will not record it, or, after `seconds` (default 50), that they are still at it: call it again then.",
        annotations(read_only_hint = true)
    )]
    async fn wait_for_take(&self, Parameters(a): Parameters<WaitTakeArgs>) -> Result<String, String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(a.seconds.unwrap_or(50).clamp(1, 300));
        let mut st = self.b.state().await?;
        loop {
            let asked = st.take_request.clone();
            let Some(t) = asked.filter(|t| t.id == a.id) else {
                return Err(match st.take_request {
                    Some(newer) if newer.id > a.id => format!("request {} gave way to request {}", a.id, newer.id),
                    _ => format!("no take request {}: ask with request_take", a.id),
                });
            };
            match t.status {
                TakeStatus::Taken => return Ok(format!("the take arrived: {}", t.answer.unwrap_or_default())),
                TakeStatus::Cancelled => return Ok(format!("the person will not record it (request {})", a.id)),
                TakeStatus::Waiting => {}
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Ok(format!(
                    "the person has not sent the take yet (request {}): call wait_for_take again",
                    a.id
                ));
            }
            st = self.b.state_after(st.version, left.min(Duration::from_secs(20))).await?;
        }
    }

    #[tool(
        description = "Write notes into the song: a melody, a bass line or chords, note by note, in beats from the start of section `from` (t start, d length, p MIDI pitch with 60 middle C, v velocity 0..1; notes at the same t make a chord). They become the notes of new parts, one playing each of `sounds` (synth presets or installed instruments), on in the sections the notes reach; a section keeps the notes over it by its progression, and other sections with that progression play the same. The parts are ordinary parts afterwards: set_part changes their sound."
    )]
    async fn add_notes(&self, Parameters(a): Parameters<NotesArgs>) -> Result<String, String> {
        let notes =
            a.notes.iter().map(|n| claudius_api::NoteView { t: n.t, d: n.d, p: n.p, v: n.v.unwrap_or(0.8) }).collect();
        let c = Command::AddNotes {
            from: a.from,
            notes,
            sounds: a.sounds,
            name: a.name,
            role: a.role,
            style: a.style,
            request: None,
        };
        self.edit(c, true).await
    }

    #[tool(
        description = "Turn a take's melody into notes: a take sung, hummed, whistled or played one note at a time becomes the notes of new parts, one playing each of `sounds` (synth presets or installed instruments, so several make a set of instruments on one melody), in the song's key and on its sixteenth-note grid, in the sections the take plays in. The take is muted unless keep_take. The parts are ordinary parts afterwards: set_part changes their sound."
    )]
    async fn transcribe_take(&self, Parameters(a): Parameters<TranscribeArgs>) -> Result<String, String> {
        let c = Command::TranscribeTake {
            take: a.take,
            sounds: a.sounds,
            name: a.name,
            role: a.role,
            chromatic: a.chromatic,
            keep_take: a.keep_take,
            style: a.style,
        };
        self.edit(c, true).await
    }

    #[tool(
        description = "Turn a beat into the song's drums: the kicks, snares and hats of a take (a beat played, or beatboxed into the microphone) or of a recording (`path`: a drum loop, a break), each struck on the song's sixteenth grid, folded into a pattern of `bars` bars and written into the song's kick, snare (or clap) and hat lanes as their `variant` patterns. A step hit only some of the times round plays with that chance (a trig). The take is muted unless keep_take. The lanes are ordinary lanes afterwards: trig(), lane() and drum() change them."
    )]
    async fn transcribe_drums(&self, Parameters(a): Parameters<DrumsArgs>) -> Result<String, String> {
        let c = Command::TranscribeDrums {
            take: a.take,
            path: a.path,
            bars: a.bars,
            variant: a.variant,
            bpm: a.bpm,
            keep_take: a.keep_take,
        };
        self.edit(c, true).await
    }

    #[tool(
        description = "Send one raw claudius API command (JSON tagged by \"cmd\"), for edits scripts do not cover: add_section {after, kind, bars}, remove_section {index}, move_section {from, to}, toggle_layer {section, layer}, set_drum_step {lane, variant, step, velocity}, set_section {index, name?, bars?, energy?, layers?, prog?, variant?, build?, gap?, impact?, fills?, filter_in?}, set_part {name, style?, preset?, register?, density?, reseed?}, add_track {name, kind: \"drum\" | \"part\", role?, sound?}, remove_track {name}, set_notes {part, prog, notes?: [{t, d, p, v}]} (a part's notes over a progression written out; no notes: the theory engine's again), set_mix {track, level_db?, pan?, reverb?, delay?, duck?, drive_db?, muted?, solo?, bus?}, add_take {path, from, name?, role?}, transcribe_take {take, sounds, name?, role?, chromatic?, keep_take?, style?}, add_notes {from, notes: [{t, d, p, v}], sounds, name?, role?, style?}, add_hits {from, hits: [{lane, t, v}], bars?, variant?, grid?, beats?} (drums played on pads, t in beats from the section's start), add_scratch {from, track, moves: [{t, at, fader}], bar?, bars?, name?, role?} (a scratch on bars of a track, kept as a take: t seconds into the scratch, at seconds into the record, fader 0 shut to 1 open)."
    )]
    async fn command(&self, Parameters(a): Parameters<CommandArgs>) -> Result<String, String> {
        let c: Command = serde_json::from_value(a.command).map_err(|e| format!("not a claudius command: {e}"))?;
        self.edit(c, a.listen).await
    }
}

#[tool_handler(
    name = "claudius",
    instructions = "claudius is a beatmaker that composes like a producer in a DAW: synthesized and recorded sounds (never General MIDI), music theory, effects, mixing and mastering, and a critic with ears (earshot, a CLAP model). Typical session: get_song (see what is open) or new_song; read the verdict; edit with run_script (read script_reference once) or try mutate/evolve; listen again; undo a change that made it worse; save. The person you work with hears every change live in the claudius studio, so describe what you changed in musical terms. When the song wants their own voice or playing, ask them to record it with request_take, then wait_for_take. The scores guide; the person's taste decides."
)]
impl ServerHandler for Studio {}

/// the MCP endpoint of a running app (Streamable HTTP)
pub fn http_service(app: Arc<App>, public: bool) -> StreamableHttpService<Studio, LocalSessionManager> {
    let mut config = StreamableHttpServerConfig::default();
    if public {
        config = config.disable_allowed_hosts();
    }
    let base = app.base_url();
    StreamableHttpService::new(
        move || Ok(Studio::new(Backend::Local { app: app.clone(), base: base.clone() })),
        Arc::new(LocalSessionManager::default()),
        config,
    )
}

/// MCP over stdin/stdout (what desktop assistants launch)
pub async fn serve_stdio(b: Backend) -> anyhow::Result<()> {
    use rmcp::ServiceExt;
    let service = Studio::new(b).serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_that_only_look_say_so() {
        let mut read_only: Vec<String> = Studio::tool_router()
            .list_all()
            .into_iter()
            .filter(|t| t.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(true))
            .map(|t| t.name.to_string())
            .collect();
        read_only.sort();
        assert_eq!(
            read_only,
            [
                "analyze",
                "assess_voice",
                "catalog",
                "get_song",
                "graph",
                "script_reference",
                "status",
                "wait_for_pitch_check",
                "wait_for_take"
            ]
        );
    }

    #[test]
    fn new_song_names_every_genre() {
        let schema = serde_json::to_value(schemars::schema_for!(NewSongArgs)).unwrap();
        let text = schema["properties"]["genre"]["description"].as_str().unwrap();
        for genre in claudius::genres::builtin_names() {
            assert!(text.contains(genre), "{genre} is missing from: {text}");
        }
    }

    #[test]
    fn the_counts_say_their_range() {
        let schema = serde_json::to_value(schemars::schema_for!(EvolveArgs)).unwrap();
        let children = &schema["properties"]["children"];
        assert_eq!((children["minimum"].as_u64(), children["maximum"].as_u64()), (Some(2), Some(32)));
    }
}
