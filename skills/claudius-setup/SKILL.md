---
name: claudius-setup
description: >-
  Installs, connects and troubleshoots claudius, the beatmaker with MCP tools for assistants: the installers, the
  sample packs and listening models (claudius setup), checking an installation (claudius doctor), adding claudius to
  Claude Desktop, Claude Code, Cursor, Codex or a cloud assistant such as ChatGPT (claudius connect, a tunnel and a
  token), opening the studio, and fixing what goes wrong. Use it when claudius is not installed or not connected, its
  tools are missing or failing, the verdict has no ears, sounds or packs are missing, the port is taken, or someone
  asks how to set claudius up for an assistant.
---

# Setting up claudius

## Install

| system | command |
|---|---|
| macOS (Apple silicon, Intel), Linux (x86_64, ARM64, glibc 2.38+) | `curl -fsSL https://github.com/devlato/claudius/releases/latest/download/install.sh \| sh` |
| Windows (preview) | `irm https://github.com/devlato/claudius/releases/latest/download/install.ps1 \| iex` in PowerShell |
| from source, with a Rust toolchain | `cargo install --locked --path crates/claudius-cli` and `--path crates/claudius-packs` in a clone |

The installer puts `claudius` and `claudius-packs` in `~/.local/bin` (on Windows, a folder it adds to `PATH`). On
Linux without ALSA it installs the headless build, which serves the studio to a browser instead of opening a window.

Then:

```sh
claudius setup      # the default sample packs (about 1.1 GB) and the listening models (about 0.8 GB); asks first
claudius doctor     # what is installed and what is missing, with the fix for each
```

`setup --yes` does not ask, `--no-packs` and `--no-models` leave either out, and `--tags` adds the AudioSet tagger
(about 0.35 GB more). Without the packs, claudius plays synthesized sounds only; without the models, the verdict
judges with meters and music theory but has no genre or production reading. Intel Macs have no models at all: ONNX
Runtime has no build for them.

## Connect an assistant

`claudius connect <client>` adds claudius to an assistant and installs these skills where that assistant reads
them. `--remove` takes both out again.

| client | command | MCP | skills |
|---|---|---|---|
| Claude Code | `claudius connect claude-code` | through `claude mcp add`, for every project | `~/.claude/skills/` |
| Cursor | `claudius connect cursor` | `~/.cursor/mcp.json` | `~/.cursor/skills/` |
| Codex | `claudius connect codex` | `~/.codex/config.toml` | `~/.codex/skills/` |
| Claude Desktop | `claudius connect claude-desktop` | its `claude_desktop_config.json` | zips to upload in Claude's settings, written to a folder it prints |
| anything else | `claudius connect print` | prints the configuration | `claudius skills --out DIR` writes them |

- Desktop clients start `claudius mcp` themselves. Quit and reopen Claude Desktop after connecting.
- With `--http`, Claude Code and Cursor connect to a studio that is already running (`http://127.0.0.1:7878/mcp`)
  instead of starting their own.
- When a studio is running, `claudius mcp` drives it, so the assistant edits the song the person has open and they
  hear each change. With no studio, it starts a session of its own and prints the address of its web studio.

### Assistants in the cloud (ChatGPT, claude.ai)

They cannot start a program or reach 127.0.0.1, so claudius needs a public HTTPS address and a token:

```sh
TOKEN=$(openssl rand -hex 16); echo "$TOKEN"
claudius serve --public --token "$TOKEN"
cloudflared tunnel --url http://127.0.0.1:7878
```

Add a custom connector with `https://<tunnel>/mcp?token=<token>` (ChatGPT's developer mode), or send
`Authorization: Bearer <token>` where the client can. `https://<tunnel>/?token=<token>` opens the studio through the
same tunnel. A shared claudius reads and writes files only in its own folders. Treat the token like a password: who
has it controls claudius.

## Open the studio

- `claudius studio`: the native window with the engine inside; it also serves the web studio, the API and MCP on
  port 7878.
- `claudius serve`: the engine and the web studio at `http://127.0.0.1:7878`, for a browser or a machine without a
  display.

## When something goes wrong

Run `claudius doctor` first; it names most problems. The common ones:

| symptom | cause and fix |
|---|---|
| the verdict has no genre or production scores, or says `earshot is unavailable` | the models are missing: `claudius setup --no-packs`. On Intel Macs this is expected |
| `unknown sound 'piano'`, or `sampled instruments need packs` | the packs are not installed: `claudius setup --no-models`, or `claudius-packs install <pack>` |
| `sampled instruments are off in this session (--synth-only)` | the session was started with `--synth-only`; start it without |
| `port 7878 is taken` | another claudius (then just use it) or another program: `--port` picks another |
| the assistant shows no claudius tools | the client was not restarted, or its configuration points at an old path: run `claudius connect <client>` again |
| `asks for a token` | the running claudius was started with a token: pass `--token` (or set `CLAUDIUS_TOKEN`) to `claudius mcp` |
| the assistant and the studio show different songs | `claudius mcp` started before the studio, so it has a session of its own: restart the assistant once the studio runs |
| `busy (rendering)` | a job is running: wait, call `status`, or `stop` it |
| no `song.mp3` | ffmpeg is not installed: `song.wav` is the same song; install ffmpeg for MP3s |

The full guide, with every message, is in the claudius docs under Troubleshooting.
