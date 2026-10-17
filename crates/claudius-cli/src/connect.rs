//! `claudius connect`: add claudius to an assistant's MCP configuration.

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

pub fn run(client: Client, http: bool, remove: bool, port: u16) -> Result<()> {
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
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
