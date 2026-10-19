# Working with an assistant

claudius speaks MCP, the protocol assistants use for tools. Once it is connected, you can ask Claude, ChatGPT or
Cursor for a song in plain words. The assistant writes a script or starts from a genre, listens through claudius'
critic, reads the issues, and revises, while you listen in the studio and steer.

## From the studio

The studio connects them with a click: **?** in the bottom right corner, then **Connect an assistant** (it is also
under New song while no song is open). It lists Claude Desktop, Claude Code, Cursor and Codex, says which are
connected, and **Connect** does what `claudius connect` below does, skills included. An assistant set up for another
claudius (an older install, say) offers **Use this app**. Then restart the assistant, as it says. This is the way for
the Mac app, the Windows installer and the Linux packages, whose `claudius` may not be on your `PATH`.

The web studio, a page in a browser, cannot change the settings of the apps on a computer, so it lists the commands to
run instead.

## The Claude Code plugin and the Claude Desktop extension

Claude Code installs claudius' MCP server and its [skills](#skills) in one go, as a plugin from this repository:

```sh
claude plugin marketplace add devlato/claudius
claude plugin install claudius@claudius
```

In a session, `/plugin marketplace add devlato/claudius` and then `/plugin install claudius@claudius` do the same.
The plugin starts the `claudius` on your `PATH`. For the Mac app dragged from its disk image, set its **Claudius
program** to `/Applications/Claudius.app/Contents/MacOS/Claudius` (`/plugin configure claudius@claudius`). Claude
Code offers the plugin's new version after each release.

On a Mac, Claude Desktop installs claudius as an extension: download `claudius-<version>.mcpb` from the
[latest release](https://github.com/devlato/claudius/releases/latest), open it, and click **Install**. It starts the
Claudius app, so install the app first; its **Claudius program** setting says where the app is, Applications unless
you change it. An extension carries no skills: upload the zips that **Connect an assistant** in the studio writes
(or `claudius skills --out <folder>`) in Claude's settings, under Capabilities.

## Claude Desktop

```sh
claudius connect claude-desktop
```

Quit and reopen Claude Desktop. claudius now appears in the tools menu of a new chat. The command adds one entry to
Claude's configuration file and keeps a backup of the old one next to it; `--remove` takes it out again.

It also writes claudius' [skills](#skills) as zips into `~/Music/claudius/skills/`. Upload them in Claude's
settings, under Capabilities, and Claude uses them in Claude Desktop and on claude.ai.

## Claude Code, Cursor and Codex

```sh
claudius connect claude-code
claudius connect cursor
claudius connect codex
```

Each adds the MCP server and installs the skills where that assistant reads them (`~/.claude/skills/`,
`~/.cursor/skills/`, `~/.codex/skills/`). `--no-skills` leaves the skills out, and `--remove` takes both out again.

For any other client, `claudius connect print` shows the configuration to paste. Clients that can reach a URL can
also connect over HTTP to a running studio: `http://127.0.0.1:7878/mcp`.

## With the assistant's own command

claudius is an ordinary MCP server: the assistant starts `claudius mcp`. So each assistant's own way of adding a
server works as well:

```sh
claude mcp add --scope user claudius -- claudius mcp    # Claude Code, in every project
codex mcp add claudius -- claudius mcp                  # Codex
```

Cursor adds it from a link: [Add claudius to Cursor](cursor://anysphere.cursor-deeplink/mcp/install?name=claudius&config=eyJjb21tYW5kIjoiY2xhdWRpdXMiLCJhcmdzIjpbIm1jcCJdfQ==)
opens Cursor, which asks before it adds the server. Or put it in `~/.cursor/mcp.json` yourself:

```json
{ "mcpServers": { "claudius": { "command": "claudius", "args": ["mcp"] } } }
```

Claude Desktop has no command for it: in its Settings, choose Developer, then Edit Config, and add the same entry with
the full path of `claudius` (`which claudius` prints it), since Claude Desktop does not see your shell's `PATH`.

These find `claudius` on your `PATH`, where the installers put it. The Mac app dragged from its disk image leaves none
there: use `/Applications/Claudius.app/Contents/MacOS/Claudius` in its place. `claudius connect print` prints every
one of these with the full path of the claudius you run it with.

They add the MCP server only. The [skills](#skills) come with `claudius connect`; `claudius skills --out <folder>`
writes them anywhere else, a folder each, to copy into `~/.claude/skills`, `~/.codex/skills` or `~/.cursor/skills`.

## Skills

The MCP tools give an assistant the controls; claudius' skills give it the craft. They are short guides, in the
[Agent Skills](https://agentskills.io) format, that an assistant reads when the conversation turns to music:

- **claudius-songs**: how to make and edit a song: listen, read the verdict, change one thing, and say what changed
  in musical terms; the script language with recipes; a starting point for each genre.
- **claudius-remix**: analysing a recording, remaking its skeleton in another genre, chopping and flipping it.
- **claudius-setup**: installing claudius, connecting assistants, and fixing what goes wrong.

`claudius connect` installs them for Claude Code, Cursor and Codex, and writes zips for the Claude apps.
`claudius skills --out <folder>` writes them anywhere else, for a project's own `.claude/skills/` folder, say.
They are built into claudius, so they always match the version you have. For an assistant without skills, such as a
ChatGPT project, `claudius-songs/SKILL.md` works as its instructions.

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
