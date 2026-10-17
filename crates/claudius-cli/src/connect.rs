//! `claudius connect`: add claudius to an assistant: its MCP configuration, and the skills that teach it claudius.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
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

fn exe() -> Result<PathBuf> {
    let e = std::env::current_exe().context("cannot find the claudius executable")?;
    Ok(e.canonicalize().unwrap_or(e))
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

/// is claudius configured in Codex's TOML config?
pub fn codex_configured(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| t.parse::<toml_edit::DocumentMut>().ok())
        .is_some_and(|d| d.get("mcp_servers").and_then(|s| s.get("claudius")).is_some())
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
fn skills(client: Client, remove: bool) -> Result<()> {
    let names = crate::skills::names().join(", ");
    match (skills_dir(client), client) {
        (Some(dir), _) if remove => {
            if crate::skills::uninstall(&dir)? > 0 {
                println!("removed the claudius skills from {}", dir.display());
            }
        }
        (Some(dir), _) => {
            crate::skills::install(&dir)?;
            println!("installed the claudius skills ({names}) in {}", dir.display());
        }
        (None, Client::ClaudeDesktop) if !remove => {
            let dir = claudius::config::songs_dir().join("skills");
            crate::skills::zips(&dir)?;
            println!(
                "wrote the claudius skills ({names}) as zips in {}: upload each in Claude's settings, under \
                 Capabilities, to use them in Claude Desktop and on claude.ai",
                dir.display()
            );
        }
        (None, Client::ClaudeDesktop) => {
            println!("skills uploaded to Claude stay there: remove them in Claude's settings, under Capabilities");
        }
        (None, _) => {}
    }
    Ok(())
}

/// is claudius configured in this JSON config file?
pub fn configured(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|v| v.pointer("/mcpServers/claudius").is_some())
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
    let exe = exe()?;
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
                println!("removed claudius from {}", path.display());
            } else {
                println!(
                    "added claudius to {}\nQuit and reopen Claude Desktop; claudius shows up under the tools menu.",
                    path.display()
                );
            }
        }
        Client::Codex => {
            if http {
                bail!("Codex starts local servers itself; leave out --http");
            }
            let path = codex_config();
            edit_codex(&path, (!remove).then(|| exe.to_string_lossy().into_owned()).as_deref())?;
            println!(
                "{} claudius {} {}",
                if remove { "removed" } else { "added" },
                if remove { "from" } else { "to" },
                path.display()
            );
        }
        Client::Cursor => {
            let path = cursor_config();
            let entry = if http { json!({ "url": url }) } else { stdio };
            edit_config(&path, (!remove).then_some(entry))?;
            println!(
                "{} claudius {} {}",
                if remove { "removed" } else { "added" },
                if remove { "from" } else { "to" },
                path.display()
            );
        }
        Client::ClaudeCode => {
            let mut args: Vec<String> = if remove {
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
            match std::process::Command::new("claude").args(&args).status() {
                Ok(s) if s.success() => println!("done: claude {}", args.join(" ")),
                Ok(s) => bail!("`claude {}` failed ({s})", args.join(" ")),
                Err(_) => {
                    args.insert(0, "claude".into());
                    println!("The `claude` command is not on PATH. Run this where it is:\n\n  {}", args.join(" "));
                }
            }
        }
        Client::Print => {
            println!(
                "Local (the client starts claudius):\n{}",
                serde_json::to_string_pretty(&json!({ "mcpServers": { "claudius": stdio } }))?
            );
            println!("\nOver HTTP, while `claudius studio` or `claudius serve` runs:\n  {url}");
            println!("\nClaude Code:\n  claude mcp add --scope user claudius -- {} mcp", exe.display());
            println!(
                "\nAssistants in the cloud (ChatGPT) need a public HTTPS address: see `claudius serve --help` (--public, --token)."
            );
            println!("\nSkills for assistants that read SKILL.md folders: `claudius skills --out <folder>`.");
        }
    }
    if with_skills {
        skills(client, remove)?;
    }
    Ok(())
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
        assert!(codex_configured(&path) && path.with_extension("toml.bak").exists());
        edit_codex(&path, None).unwrap();
        assert!(!codex_configured(&path));
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
        assert!(configured(&path));
        assert!(path.with_extension("json.bak").exists());
        edit_config(&path, None).unwrap();
        assert!(!configured(&path));
        std::fs::write(&path, "{ not json").unwrap();
        assert!(edit_config(&path, None).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        std::fs::remove_dir_all(&dir).ok();
    }
}
