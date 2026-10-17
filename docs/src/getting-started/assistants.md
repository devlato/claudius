# Working with an assistant

claudius speaks MCP, the protocol assistants use for tools. Once it is connected, you can ask Claude, ChatGPT or
Cursor for a song in plain words. The assistant writes a script or starts from a genre, listens through claudius'
critic, reads the issues, and revises, while you listen in the studio and steer.

## Claude Desktop

```sh
claudius connect claude-desktop
```

Quit and reopen Claude Desktop. claudius now appears in the tools menu of a new chat. The command adds one entry to
Claude's configuration file and keeps a backup of the old one next to it; `--remove` takes it out again.

## Claude Code and Cursor

```sh
claudius connect claude-code
claudius connect cursor
```

For any other client, `claudius connect print` shows the configuration to paste. Clients that can reach a URL can
also connect over HTTP to a running studio: `http://127.0.0.1:7878/mcp`.

## You and the assistant on the same song

Open the studio first (`claudius studio`). When the assistant starts claudius, it finds the studio already running
and works on the song you have open, so every change it makes shows up in front of you, and you can change things
yourself in between. Without a studio running, it starts a session of its own; open <http://127.0.0.1:7878> to
follow it.

## What to ask

Be specific about what you want to hear, the way you would brief a producer:

- "Make a deep house track at 122 BPM in F minor. Keep the intro sparse and make the drop hit hard."
- "The lead is too busy. Simplify it, and put a Rhodes on the chords instead of the synth."
- "Listen to it and fix the critic's two most serious issues. Tell me what you changed."
- "Try five variations of the drums and keep the best one."
- "Remake ~/Music/reference.mp3 as lofi."

The assistant can undo anything it does, and so can you (Cmd+Z in the studio).

## ChatGPT and other assistants in the cloud

These run on someone else's computer, so they need a public address. Run claudius with a token behind a tunnel:

```sh
TOKEN=$(openssl rand -hex 16); echo "$TOKEN"
claudius serve --public --token "$TOKEN"
cloudflared tunnel --url http://127.0.0.1:7878
```

Then add a custom connector (in ChatGPT's developer mode) with the URL `https://<your tunnel>/mcp?token=<token>`.
A shared claudius only reads and writes files in `~/Music/claudius`, its caches and the sound libraries; see the
[security model](../engineering/security.md).

## What the assistant can do

The [MCP tools](../guide/mcp.md) page lists every tool. In short: start, open and save songs; read the song and its
script; run scripts; render and judge; mutate and evolve; analyze and remake audio files; undo and redo.
