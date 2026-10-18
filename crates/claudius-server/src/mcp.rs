//! The MCP tools: what an assistant (Claude, ChatGPT, any MCP client) uses to make and edit songs. They work
//! through a `Backend`: the session in this process, or a claudius app that is already running (through its HTTP
//! API), so an assistant connected with `claudius mcp` edits the very song the person has open in the studio.

use std::sync::Arc;
use std::time::Duration;

use claudius_api::{Command, Reply, SoundList, State};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::app::App;
use crate::views;

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
    /// the genre: house, deep_house, techno, trap, lofi, dnb, future_bass or synthwave (catalog lists them with their
    /// tempo ranges, and any genres of the person's own)
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
pub struct RemakeArgs {
    /// the audio file to learn from
    pub path: String,
    /// the genre for the sounds and production, one of those new_song takes (default: the open song's)
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
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
        self.edit(Command::Remake { path: a.path, genre: a.genre, seed: a.seed }, true).await
    }

    #[tool(
        description = "Send one raw claudius API command (JSON tagged by \"cmd\"), for edits scripts do not cover: add_section {after, kind, bars}, remove_section {index}, move_section {from, to}, toggle_layer {section, layer}, set_drum_step {lane, variant, step, velocity}, set_section {index, name?, bars?, energy?, layers?, prog?, variant?, build?, gap?, impact?, fills?, filter_in?}, set_part {name, style?, preset?, register?, density?, reseed?}, set_mix {track, level_db?, pan?, reverb?, delay?, duck?, muted?, solo?}."
    )]
    async fn command(&self, Parameters(a): Parameters<CommandArgs>) -> Result<String, String> {
        let c: Command = serde_json::from_value(a.command).map_err(|e| format!("not a claudius command: {e}"))?;
        self.edit(c, a.listen).await
    }
}

#[tool_handler(
    name = "claudius",
    instructions = "claudius is a beatmaker that composes like a producer in a DAW: synthesized and recorded sounds (never General MIDI), music theory, effects, mixing and mastering, and a critic with ears (earshot, a CLAP model). Typical session: get_song (see what is open) or new_song; read the verdict; edit with run_script (read script_reference once) or try mutate/evolve; listen again; undo a change that made it worse; save. The person you work with hears every change live in the claudius studio, so describe what you changed in musical terms. The scores guide; the person's taste decides."
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
        assert_eq!(read_only, ["analyze", "catalog", "get_song", "script_reference", "status"]);
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
