//! The command line: every command, its flags and their help text.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use claudius::library::Library;

use crate::connect;

#[derive(Parser)]
#[command(name = "claudius", version, about = "An automatic beatmaker that composes like a producer in a DAW")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(clap::Args, Clone, Default)]
pub(crate) struct EarsArgs {
    /// judge without earshot's models (meters and theory only)
    #[arg(long)]
    pub(crate) no_ears: bool,
    /// reference tracks (files or folders) the songs should resemble
    #[arg(long)]
    pub(crate) refs: Vec<PathBuf>,
    /// also run the AudioSet tagger for held-out tags in the report (slower)
    #[arg(long)]
    pub(crate) tags: bool,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Compose, arrange, mix and master a new song
    Make {
        #[arg(short, long, default_value = "house")]
        genre: String,
        #[arg(short, long)]
        seed: Option<u64>,
        #[arg(long)]
        bpm: Option<f64>,
        /// key, e.g. "Am", "F# minor", "Eb", "D dorian"
        #[arg(long)]
        key: Option<String>,
        /// halve every section (a quick sketch)
        #[arg(long)]
        short: bool,
        /// write every track as its own WAV too
        #[arg(long)]
        stems: bool,
        /// chance (0..1) that a drum lane uses a recorded one-shot from the library instead of synthesis
        #[arg(long, default_value_t = 0.75)]
        samples: f64,
        #[command(flatten)]
        sound: SoundArgs,
        /// flip a recording into the song (a chop from `claudius chop`, or any audio file); repeatable
        #[arg(long)]
        sample: Vec<PathBuf>,
        /// build the song around an Apple Loops family (its chords, bass and beat loops), GarageBand style
        #[arg(long)]
        loops: bool,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(short, long, default_value = "out")]
        out: PathBuf,
    },
    /// Evolve songs: mutate blueprints, audition the build + first drop of each, keep what the critic likes
    Evolve {
        #[arg(short, long, default_value = "house")]
        genre: String,
        /// start from this blueprint instead of freshly generated songs
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(short, long)]
        seed: Option<u64>,
        /// how many fresh songs to start from
        #[arg(long, default_value_t = 4)]
        start: usize,
        #[arg(long, default_value_t = 5)]
        generations: usize,
        /// parents kept per generation
        #[arg(long, default_value_t = 4)]
        population: usize,
        /// children per generation
        #[arg(long, default_value_t = 8)]
        children: usize,
        /// bars auditioned per candidate (build-up + first drop)
        #[arg(long, default_value_t = 32)]
        preview_bars: u32,
        /// best songs rendered in full at the end
        #[arg(long, default_value_t = 3)]
        keep: usize,
        #[arg(long, default_value_t = 0.75)]
        samples: f64,
        #[command(flatten)]
        sound: SoundArgs,
        /// start from songs built around Apple Loops families
        #[arg(long)]
        loops: bool,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(short, long, default_value = "out")]
        out: PathBuf,
    },
    /// Write mutated variants of a blueprint (each rendered, with what changed)
    Mutate {
        blueprint: PathBuf,
        #[arg(short, long, default_value_t = 4)]
        n: usize,
        #[arg(short, long)]
        seed: Option<u64>,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Analyse a song: tempo, key, swing, sections, chords, drum habits, bass style
    Analyze {
        file: PathBuf,
        /// a genre whose tempo range guides the beat tracker
        #[arg(short, long)]
        genre: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Write a new song on the skeleton of an existing one (its tempo, key, form, progressions, drum habits)
    Remake {
        file: PathBuf,
        #[arg(short, long, default_value = "house")]
        genre: String,
        #[arg(short, long)]
        seed: Option<u64>,
        /// the tempo (default: the recording's, halved or doubled into the genre's range when it is outside it)
        #[arg(long)]
        bpm: Option<f64>,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(short, long, default_value = "out")]
        out: PathBuf,
    },
    /// Chop a recording into bar loops and drum one-shots (added to the user library); --find pulls out moments
    /// matching a description ("a vocal chop", "a snare hit")
    Chop {
        file: PathBuf,
        #[arg(long, default_value_t = 2)]
        bars: usize,
        #[arg(short, long)]
        genre: Option<String>,
        /// a description of sounds to find and cut out (repeatable)
        #[arg(long)]
        find: Vec<String>,
        /// how many trims per --find query
        #[arg(long, default_value_t = 3)]
        top: usize,
        #[arg(short, long, default_value = "out/chops")]
        out: PathBuf,
    },
    /// Re-render a blueprint (e.g. one you edited by hand)
    Render {
        blueprint: PathBuf,
        #[arg(long)]
        stems: bool,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Judge an audio file (with the arrangement and theory checks when its project.json sits next to it)
    Judge {
        file: PathBuf,
        /// the genre it should be (default: the project's, or whatever it reads as)
        #[arg(short, long)]
        genre: Option<String>,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(long)]
        json: bool,
    },
    /// List the genre profiles
    Genres,
    /// Render every built-in drum voice, synth preset and transition effect to WAVs for auditioning
    Sounds {
        #[arg(short, long, default_value = "out/sounds")]
        out: PathBuf,
    },
    /// Run a claudius script (Rhai + Strudel-style patterns; `claudius reference` prints the language) and render
    /// the song it makes, or the song it edits with --on
    Script {
        /// the script file, or - for stdin
        file: PathBuf,
        /// edit this song (a blueprint.json or a folder written by make / save) instead of starting empty
        #[arg(long)]
        on: Option<PathBuf>,
        /// print the resulting song as a script instead of rendering it
        #[arg(long)]
        print: bool,
        #[command(flatten)]
        ears: EarsArgs,
        #[arg(short, long, default_value = "out")]
        out: PathBuf,
    },
    /// Print the script language reference
    Reference,
    /// Download the default sample packs (about 1.1 GB) and earshot's listening models (about 0.8 GB)
    Setup {
        /// do not ask before downloading
        #[arg(short, long)]
        yes: bool,
        /// skip the sample packs
        #[arg(long)]
        no_packs: bool,
        /// skip earshot's models
        #[arg(long)]
        no_models: bool,
        /// also get the AudioSet tagger (for `judge --tags`, about 0.35 GB more)
        #[arg(long)]
        tags: bool,
    },
    /// Check this installation: web studio, sample packs, models, Apple Loops, ffmpeg, the port, assistants
    Doctor {
        #[arg(long, default_value_t = claudius_server::DEFAULT_PORT, env = "CLAUDIUS_PORT")]
        port: u16,
    },
    /// Add claudius to an assistant (Claude Desktop, Claude Code, Cursor, Codex): its MCP configuration and the
    /// skills that teach it claudius; or print them for another
    Connect {
        #[arg(value_enum)]
        client: connect::Client,
        /// connect over HTTP to a running studio instead of letting the client start claudius
        #[arg(long)]
        http: bool,
        /// remove claudius (and its skills) from that client instead
        #[arg(long)]
        remove: bool,
        /// connect the MCP server only, without the skills
        #[arg(long)]
        no_skills: bool,
        #[arg(long, default_value_t = claudius_server::DEFAULT_PORT, env = "CLAUDIUS_PORT")]
        port: u16,
    },
    /// Write the skills that teach AI assistants claudius (a folder with a SKILL.md each, and zips of them for the
    /// Claude apps); `claudius connect` installs them for Claude Code, Cursor and Codex
    Skills {
        /// the folder to write them into
        #[arg(short, long, default_value = "claudius-skills")]
        out: PathBuf,
    },
    /// Open the studio: the native app with the engine inside (also serves the web UI and MCP on --port)
    Studio {
        #[command(flatten)]
        app: AppArgs,
    },
    /// Run the engine as a server: the web UI at http://127.0.0.1:PORT, the HTTP API, and MCP at /mcp
    Serve {
        #[command(flatten)]
        app: AppArgs,
        /// address to listen on (0.0.0.0 for the LAN, with --public). Any address but loopback requires a token
        /// (without --token, claudius makes one and prints it) and limits file access, as --public does
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// answer requests for any host name (behind a tunnel or on a LAN). Without --token, claudius makes a random
        /// token and prints it
        #[arg(long)]
        public: bool,
        /// require this token on every request (Authorization: Bearer, or ?token= once in the browser); a blank one
        /// counts as none
        #[arg(long, env = "CLAUDIUS_TOKEN")]
        token: Option<String>,
        /// a folder commands and scripts may read and write, besides ~/Music/claudius, claudius' caches and the sound
        /// libraries (repeatable). With --public or --host beyond loopback, or once one is given, file access is
        /// limited to these folders
        #[arg(long = "files", value_name = "DIR")]
        files: Vec<PathBuf>,
    },
    /// MCP over stdin/stdout for assistants (Claude Desktop, Claude Code, Cursor, ...): drives the claudius app
    /// running on --port, or runs a session of its own (with the web UI on --port) when none is running
    Mcp {
        #[command(flatten)]
        app: AppArgs,
        /// a claudius app elsewhere (default: the one on --port, if running)
        #[arg(long)]
        url: Option<String>,
        #[arg(long, env = "CLAUDIUS_TOKEN")]
        token: Option<String>,
    },
}

#[derive(clap::Args, Clone)]
pub(crate) struct AppArgs {
    #[arg(long, default_value_t = claudius_server::DEFAULT_PORT, env = "CLAUDIUS_PORT")]
    pub(crate) port: u16,
    /// judge without earshot's models (meters and theory only)
    #[arg(long)]
    pub(crate) no_ears: bool,
    #[command(flatten)]
    pub(crate) sound: SoundArgs,
    /// render only when asked (by default every change is rendered and judged)
    #[arg(long)]
    pub(crate) no_auto_render: bool,
    /// the built web UI (default: crates/claudius-ui/dist of this checkout, or $CLAUDIUS_WEB)
    #[arg(long, env = "CLAUDIUS_WEB")]
    pub(crate) web: Option<PathBuf>,
}

impl AppArgs {
    pub(crate) fn app(&self) -> std::sync::Arc<claudius_server::App> {
        self.app_with(None)
    }

    pub(crate) fn app_with(&self, file_roots: Option<Vec<PathBuf>>) -> std::sync::Arc<claudius_server::App> {
        if self.sound.synth_only {
            claudius_packs::set_disabled(true);
        }
        claudius_server::App::new(claudius_server::Settings {
            library: self.sound.library_on(),
            ears: !self.no_ears,
            auto_render: !self.no_auto_render,
            file_roots,
        })
    }

    pub(crate) fn web(&self) -> Option<PathBuf> {
        web_dir(self.web.clone())
    }
}

/// the built web UI: --web, an install's share/claudius/web, the Mac app's Resources/web, or this checkout's
/// crates/claudius-ui/dist
pub(crate) fn web_dir(explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit.or_else(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.canonicalize().ok())
            .and_then(|e| e.parent().map(PathBuf::from));
        let installed = exe_dir.as_ref().map(|d| d.join("../share/claudius/web"));
        let app = exe_dir.as_ref().map(|d| d.join("../Resources/web"));
        let beside = exe_dir.as_ref().map(|d| d.join("web"));
        let checkout = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../claudius-ui/dist"));
        [installed, app, beside, checkout]
            .into_iter()
            .flatten()
            .find(|d| d.join("index.html").exists())
            .map(|d| d.canonicalize().unwrap_or(d))
    })
}

/// where sounds may come from
#[derive(clap::Args, Clone, Default)]
pub(crate) struct SoundArgs {
    /// synthesize every sound: no sample packs, Apple Loops or user library
    #[arg(long)]
    pub(crate) synth_only: bool,
    /// leave out Apple Loops and your own library; the sample packs still play
    #[arg(long)]
    pub(crate) no_library: bool,
}

impl SoundArgs {
    pub(crate) fn library_on(&self) -> bool {
        !self.synth_only && !self.no_library
    }

    /// the Apple Loops and user library unless it is left out (and the packs too, with --synth-only)
    pub(crate) fn library(&self) -> Result<Option<Library>> {
        if self.synth_only {
            claudius_packs::set_disabled(true);
        }
        if !self.library_on() {
            return Ok(None);
        }
        open_library()
    }
}

/// the Apple Loops and user library, when there is one
pub(crate) fn open_library() -> Result<Option<Library>> {
    Ok(Some(Library::open()?).filter(|l| !l.is_empty()))
}
