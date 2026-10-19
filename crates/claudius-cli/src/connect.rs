//! `claudius connect`: add claudius to an assistant: its MCP configuration, and the skills that teach it claudius.
//! The studio's "Connect an assistant" does the same through [`FromTheStudio`], which also keeps the studio's answer
//! about usage statistics.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Client {
    /// Claude Desktop (claude_desktop_config.json)
    ClaudeDesktop,
    /// Claude Code (through the `claude` command)
    ClaudeCode,
    /// Cursor (~/.cursor/mcp.json)
    Cursor,
    /// Codex (~/.codex/config.toml)
    Codex,
    /// print the configuration for any other client
    Print,
}

impl Client {
    /// the assistants the studio offers to connect
    pub const ASSISTANTS: [Client; 4] = [Client::ClaudeDesktop, Client::ClaudeCode, Client::Cursor, Client::Codex];

    /// as the command line names it (`claude-desktop`)
    pub fn id(self) -> String {
        clap::ValueEnum::to_possible_value(&self).map(|v| v.get_name().to_string()).unwrap_or_default()
    }

    pub fn name(self) -> &'static str {
        match self {
            Client::ClaudeDesktop => "Claude Desktop",
            Client::ClaudeCode => "Claude Code",
            Client::Cursor => "Cursor",
            Client::Codex => "Codex",
            Client::Print => "another client",
        }
    }
}

fn exe() -> Result<PathBuf> {
    let e = std::env::current_exe().context("cannot find the claudius executable")?;
    Ok(e.canonicalize().unwrap_or(e))
}

/// why an assistant should not start claudius from `exe`: a place macOS takes away again
fn temporary(exe: &Path) -> Option<&'static str> {
    let s = exe.to_string_lossy();
    if s.contains("/AppTranslocation/") {
        Some("the copy macOS runs of an app opened where it was downloaded")
    } else if s.starts_with("/Volumes/claudius ") {
        Some("its disk image")
    } else {
        None
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

pub fn claude_desktop_config() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support/Claude/claude_desktop_config.json")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home).join("Claude/claude_desktop_config.json")
    } else {
        home().join(".config/Claude/claude_desktop_config.json")
    }
}

pub fn cursor_config() -> PathBuf {
    home().join(".cursor/mcp.json")
}

pub fn codex_config() -> PathBuf {
    home().join(".codex/config.toml")
}

/// where a client reads skills, for those that read them from a folder
fn skills_dir(client: Client) -> Option<PathBuf> {
    match client {
        Client::ClaudeCode => Some(home().join(".claude/skills")),
        Client::Cursor => Some(home().join(".cursor/skills")),
        Client::Codex => Some(home().join(".codex/skills")),
        Client::ClaudeDesktop | Client::Print => None,
    }
}

/// how an assistant's configuration has claudius
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setup {
    Missing,
    /// the assistant starts this command
    Command(String),
    /// the assistant connects to a running claudius at this URL
    Url(String),
}

fn setup_of(command: Option<&str>, url: Option<&str>) -> Setup {
    match (command, url) {
        (Some(c), _) => Setup::Command(c.to_string()),
        (None, Some(u)) => Setup::Url(u.to_string()),
        (None, None) => Setup::Command(String::new()),
    }
}

/// claudius in Codex's TOML config
fn codex_setup(path: &Path) -> Setup {
    let doc = std::fs::read_to_string(path).ok().and_then(|t| t.parse::<toml_edit::DocumentMut>().ok());
    match doc.as_ref().and_then(|d| d.get("mcp_servers")).and_then(|s| s.get("claudius")) {
        Some(e) => setup_of(e.get("command").and_then(|c| c.as_str()), e.get("url").and_then(|u| u.as_str())),
        None => Setup::Missing,
    }
}

/// set or remove `[mcp_servers.claudius]` in Codex's config, keeping everything else (comments included) and a backup
fn edit_codex(path: &Path, exe: Option<&str>) -> Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().with_context(|| {
        format!("{} is not valid TOML; fix it or move it aside, then run this again", path.display())
    })?;
    match exe {
        Some(exe) => {
            let servers = doc.entry("mcp_servers").or_insert_with(|| {
                let mut t = toml_edit::Table::new();
                t.set_implicit(true);
                toml_edit::Item::Table(t)
            });
            let Some(servers) = servers.as_table_mut() else {
                bail!("mcp_servers in {} is not a table", path.display())
            };
            let mut entry = toml_edit::Table::new();
            entry.insert("command", toml_edit::value(exe));
            entry.insert("args", toml_edit::value(toml_edit::Array::from_iter(["mcp"])));
            servers.insert("claudius", toml_edit::Item::Table(entry));
        }
        None => {
            if let Some(servers) = doc.get_mut("mcp_servers").and_then(|s| s.as_table_mut()) {
                servers.remove("claudius");
            }
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        std::fs::copy(path, path.with_extension("toml.bak"))?;
    }
    std::fs::write(path, doc.to_string()).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// install the skills where the client reads them, or take them out again
fn skills(client: Client, remove: bool, out: &mut Vec<String>) -> Result<()> {
    let names = crate::skills::names().join(", ");
    match (skills_dir(client), client) {
        (Some(dir), _) if remove => {
            if crate::skills::uninstall(&dir)? > 0 {
                out.push(format!("removed the claudius skills from {}", dir.display()));
            }
        }
        (Some(dir), _) => {
            crate::skills::install(&dir)?;
            out.push(format!("installed the claudius skills ({names}) in {}", dir.display()));
        }
        (None, Client::ClaudeDesktop) if !remove => {
            let dir = claudius::config::songs_dir().join("skills");
            crate::skills::zips(&dir)?;
            out.push(format!(
                "wrote the claudius skills ({names}) as zips in {}: upload each in Claude's settings, under \
                 Capabilities, to use them in Claude Desktop and on claude.ai",
                dir.display()
            ));
        }
        (None, Client::ClaudeDesktop) => {
            out.push(
                "skills uploaded to Claude stay there: remove them in Claude's settings, under Capabilities".into(),
            );
        }
        (None, _) => {}
    }
    Ok(())
}

/// claudius in a JSON config (Claude Desktop's, Cursor's, and Claude Code's `~/.claude.json`)
fn json_setup(path: &Path) -> Setup {
    let entry = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.pointer("/mcpServers/claudius").cloned());
    match entry {
        Some(e) => setup_of(e.get("command").and_then(Value::as_str), e.get("url").and_then(Value::as_str)),
        None => Setup::Missing,
    }
}

/// one assistant on this computer, as `connect` sees it
pub struct Found {
    pub client: Client,
    pub setup: Setup,
    /// installed here, as far as claudius can tell from the folders it keeps
    pub installed: bool,
}

/// the assistants the studio offers, and how each has claudius
pub fn assistants() -> Vec<Found> {
    let parent_exists = |p: &Path| p.parent().is_some_and(Path::exists);
    Client::ASSISTANTS
        .into_iter()
        .map(|client| {
            let (setup, installed) = match client {
                Client::ClaudeDesktop => {
                    let path = claude_desktop_config();
                    let app = cfg!(target_os = "macos") && Path::new("/Applications/Claude.app").exists();
                    (json_setup(&path), app || parent_exists(&path))
                }
                Client::ClaudeCode => {
                    let found = home().join(".claude").exists() || claude_command().is_some();
                    (json_setup(&home().join(".claude.json")), found)
                }
                Client::Cursor => (json_setup(&cursor_config()), parent_exists(&cursor_config())),
                Client::Codex | Client::Print => (codex_setup(&codex_config()), parent_exists(&codex_config())),
            };
            Found { client, setup, installed }
        })
        .collect()
}

/// the `claude` command: on PATH, or where its installers put it, since an app opened from the Finder or the Start
/// menu gets a short PATH
fn claude_command() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    let path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    let usual = [
        home().join(".local/bin"),
        home().join(".claude/local"),
        home().join(".npm-global/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    find_in(path.iter().chain(&usual), names)
}

fn find_in<'a>(dirs: impl IntoIterator<Item = &'a PathBuf>, names: &[&str]) -> Option<PathBuf> {
    dirs.into_iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// runs `claude` with its own folder first on PATH: an npm install of it is a node script, and node is usually
/// installed in the same folder
fn claude(claude: &Path, args: &[impl AsRef<OsStr>]) -> Result<Output> {
    let mut path: Vec<PathBuf> = claude.parent().map(Path::to_path_buf).into_iter().collect();
    path.extend(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default());
    Command::new(claude)
        .args(args)
        .env("PATH", std::env::join_paths(path)?)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot run {}", claude.display()))
}

/// change `mcpServers.claudius` in a JSON config, keeping everything else and a backup of the original
fn edit_config(path: &Path, entry: Option<Value>) -> Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".into());
    let mut root: Value =
        serde_json::from_str(if text.trim().is_empty() { "{}" } else { &text }).with_context(|| {
            format!("{} is not valid JSON; fix it or move it aside, then run this again", path.display())
        })?;
    let Some(obj) = root.as_object_mut() else { bail!("{} does not hold a JSON object", path.display()) };
    let servers = obj.entry("mcpServers").or_insert_with(|| Value::Object(Map::new()));
    let Some(servers) = servers.as_object_mut() else { bail!("mcpServers in {} is not an object", path.display()) };
    match entry {
        Some(e) => {
            servers.insert("claudius".into(), e);
        }
        None => {
            servers.remove("claudius");
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        std::fs::copy(path, path.with_extension("json.bak"))?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&root)? + "\n")
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

pub fn run(client: Client, http: bool, remove: bool, port: u16, with_skills: bool) -> Result<()> {
    for line in connect(client, http, remove, port, with_skills)? {
        println!("{line}");
    }
    Ok(())
}

/// connects claudius to an assistant, or takes it out with `remove`: what was done and what to do next, in lines
pub fn connect(client: Client, http: bool, remove: bool, port: u16, with_skills: bool) -> Result<Vec<String>> {
    let exe = exe()?;
    if !remove && let Some(why) = temporary(&exe) {
        bail!(
            "claudius is running from {}, {why}, which goes away: move claudius.app into Applications, open it \
             from there, and connect again",
            exe.display()
        );
    }
    let mut out = Vec::new();
    let url = format!("http://127.0.0.1:{port}/mcp");
    let stdio = json!({ "command": exe.to_string_lossy(), "args": ["mcp"] });
    match client {
        Client::ClaudeDesktop => {
            if http {
                bail!("Claude Desktop starts local servers itself; leave out --http");
            }
            let path = claude_desktop_config();
            edit_config(&path, (!remove).then_some(stdio))?;
            if remove {
                out.push(format!("removed claudius from {}", path.display()));
            } else {
                out.push(format!("added claudius to {}", path.display()));
                out.push("Quit and reopen Claude Desktop; claudius shows up under the tools menu.".into());
            }
        }
        Client::Codex => {
            if http {
                bail!("Codex starts local servers itself; leave out --http");
            }
            let path = codex_config();
            edit_codex(&path, (!remove).then(|| exe.to_string_lossy().into_owned()).as_deref())?;
            out.push(format!(
                "{} claudius {} {}",
                if remove { "removed" } else { "added" },
                if remove { "from" } else { "to" },
                path.display()
            ));
            if !remove {
                out.push("Start a new Codex session to load it.".into());
            }
        }
        Client::Cursor => {
            let path = cursor_config();
            let entry = if http { json!({ "url": url }) } else { stdio };
            edit_config(&path, (!remove).then_some(entry))?;
            out.push(format!(
                "{} claudius {} {}",
                if remove { "removed" } else { "added" },
                if remove { "from" } else { "to" },
                path.display()
            ));
            if !remove {
                out.push("Restart Cursor, or reload its window, to load it.".into());
            }
        }
        Client::ClaudeCode => {
            let args: Vec<String> = if remove {
                vec!["mcp".into(), "remove".into(), "--scope".into(), "user".into(), "claudius".into()]
            } else if http {
                vec![
                    "mcp".into(),
                    "add".into(),
                    "--scope".into(),
                    "user".into(),
                    "--transport".into(),
                    "http".into(),
                    "claudius".into(),
                    url.clone(),
                ]
            } else {
                vec![
                    "mcp".into(),
                    "add".into(),
                    "--scope".into(),
                    "user".into(),
                    "claudius".into(),
                    "--".into(),
                    exe.to_string_lossy().into_owned(),
                    "mcp".into(),
                ]
            };
            match claude_command() {
                Some(cmd) => {
                    if !remove {
                        // `mcp add` refuses a name it already has: a claudius set up before makes way
                        claude(&cmd, &["mcp", "remove", "--scope", "user", "claudius"])?;
                    }
                    let o = claude(&cmd, &args)?;
                    if !o.status.success() {
                        let why = String::from_utf8_lossy(if o.stderr.is_empty() { &o.stdout } else { &o.stderr });
                        bail!("`claude {}` failed ({}): {}", args.join(" "), o.status, why.trim());
                    }
                    out.push(format!("done: claude {}", args.join(" ")));
                    if !remove {
                        out.push(
                            "Claude Code sessions started from now on have it; restart one that is running.".into(),
                        );
                    }
                }
                None => out.push(format!(
                    "The `claude` command is not on PATH. Run this where it is:\n\n  claude {}",
                    args.join(" ")
                )),
            }
        }
        Client::Print => {
            out.push(format!(
                "Local (the client starts claudius):\n{}",
                serde_json::to_string_pretty(&json!({ "mcpServers": { "claudius": stdio } }))?
            ));
            out.push(format!("\nOver HTTP, while `claudius studio` or `claudius serve` runs:\n  {url}"));
            out.push(format!("\nClaude Code:\n  claude mcp add --scope user claudius -- {} mcp", exe.display()));
            out.push(
                "\nAssistants in the cloud (ChatGPT) need a public HTTPS address: see `claudius serve --help` (--public, \
                 --token)."
                    .into(),
            );
            out.push("\nSkills for assistants that read SKILL.md folders: `claudius skills --out <folder>`.".into());
        }
    }
    if with_skills {
        skills(client, remove, &mut out)?;
    }
    Ok(out)
}

/// connects assistants for the studio window, which runs in this program (`claudius studio`)
#[cfg(feature = "studio")]
pub struct FromTheStudio;

#[cfg(feature = "studio")]
impl claudius_ui::Native for FromTheStudio {
    fn assistants(&self) -> Vec<claudius_ui::Assistant> {
        use claudius_ui::Connected;
        let me = exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let shown = if me.contains(' ') { format!("\"{me}\"") } else { me.clone() };
        assistants()
            .into_iter()
            .map(|f| claudius_ui::Assistant {
                id: f.client.id(),
                name: f.client.name().into(),
                connected: match f.setup {
                    Setup::Missing => Connected::No,
                    Setup::Command(c) if c == me => Connected::ThisApp,
                    Setup::Command(c) => Connected::Other(c),
                    Setup::Url(u) => Connected::Http(u),
                },
                found: f.installed,
                command: format!("{shown} connect {}", f.client.id()),
            })
            .collect()
    }

    fn connect(&self, id: &str) -> Result<Vec<String>, String> {
        let client = <Client as clap::ValueEnum>::from_str(id, true)?;
        connect(client, false, false, claudius_server::DEFAULT_PORT, true).map_err(|e| format!("{e:#}"))
    }

    fn statistics(&self) -> Option<bool> {
        match crate::telemetry::status() {
            (_, Some(_)) => Some(false),
            (answer, None) => answer,
        }
    }

    fn set_statistics(&self, on: bool) -> Result<(), String> {
        crate::telemetry::set(on).map_err(|e| format!("{e:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_edits_keep_the_rest_of_the_file() {
        let dir = std::env::temp_dir().join(format!("claudius-codex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let before = "# my settings\nmodel = \"gpt\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        std::fs::write(&path, before).unwrap();
        edit_codex(&path, Some("/bin/claudius")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# my settings\nmodel = \"gpt\""), "{text}");
        assert!(text.contains("[mcp_servers.other]") && text.contains("[mcp_servers.claudius]"), "{text}");
        assert!(text.contains("command = \"/bin/claudius\"") && text.contains("args = [\"mcp\"]"), "{text}");
        assert!(
            codex_setup(&path) == Setup::Command("/bin/claudius".into()) && path.with_extension("toml.bak").exists()
        );
        edit_codex(&path, None).unwrap();
        assert_eq!(codex_setup(&path), Setup::Missing);
        assert!(std::fs::read_to_string(&path).unwrap().contains("[mcp_servers.other]"));
        // a new file gets just the one table
        std::fs::remove_file(&path).unwrap();
        edit_codex(&path, Some("/bin/claudius")).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("[mcp_servers.claudius]"));
        std::fs::write(&path, "not = = toml").unwrap();
        assert!(edit_codex(&path, None).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not = = toml");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn edits_keep_other_servers_and_back_up() {
        let dir = std::env::temp_dir().join(format!("claudius-connect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        edit_config(&path, Some(json!({"command": "/bin/claudius", "args": ["mcp"]}))).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(v["mcpServers"]["claudius"]["args"][0], "mcp");
        assert_eq!(json_setup(&path), Setup::Command("/bin/claudius".into()));
        assert!(path.with_extension("json.bak").exists());
        edit_config(&path, None).unwrap();
        assert_eq!(json_setup(&path), Setup::Missing);
        std::fs::write(&path, "{ not json").unwrap();
        assert!(edit_config(&path, None).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_url_is_a_setup_too() {
        let dir = std::env::temp_dir().join(format!("claudius-setup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let json = dir.join("mcp.json");
        std::fs::write(&json, r#"{"mcpServers":{"claudius":{"url":"http://127.0.0.1:7878/mcp"}}}"#).unwrap();
        assert_eq!(json_setup(&json), Setup::Url("http://127.0.0.1:7878/mcp".into()));
        let toml = dir.join("config.toml");
        std::fs::write(&toml, "[mcp_servers.claudius]\ncommand = \"/opt/claudius\"\n").unwrap();
        assert_eq!(codex_setup(&toml), Setup::Command("/opt/claudius".into()));
        assert_eq!(json_setup(&dir.join("missing.json")), Setup::Missing);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_app_that_macos_moves_or_unmounts_is_not_connected() {
        let translocated =
            Path::new("/private/var/folders/x/AppTranslocation/1234/d/claudius.app/Contents/MacOS/claudius");
        assert!(temporary(translocated).is_some());
        assert!(temporary(Path::new("/Volumes/claudius 0.2.0/claudius.app/Contents/MacOS/claudius")).is_some());
        assert!(temporary(Path::new("/Applications/claudius.app/Contents/MacOS/claudius")).is_none());
        assert!(temporary(Path::new("/Volumes/Music drive/claudius.app/Contents/MacOS/claudius")).is_none());
    }

    #[test]
    fn claude_is_found_outside_the_path_too() {
        let dir = std::env::temp_dir().join(format!("claudius-claude-{}", std::process::id()));
        let (empty, bin) = (dir.join("empty"), dir.join("bin"));
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("claude"), "").unwrap();
        assert_eq!(find_in([&empty, &bin], &["claude"]), Some(bin.join("claude")));
        assert_eq!(find_in([&empty], &["claude"]), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_studio_names_assistants_as_the_command_line_does() {
        let ids: Vec<String> = Client::ASSISTANTS.into_iter().map(Client::id).collect();
        assert_eq!(ids, ["claude-desktop", "claude-code", "cursor", "codex"]);
        for c in Client::ASSISTANTS {
            assert_eq!(<Client as clap::ValueEnum>::from_str(&c.id(), true), Ok(c));
        }
    }
}
