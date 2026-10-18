# MCP tools

claudius is an MCP server: an assistant connected to it makes, edits, renders and judges songs with the tools below,
on the same session the studio shows. [Working with an assistant](../getting-started/assistants.md) is the short
version of this page.

There are two ways to reach it:

- **stdio**: the client starts `claudius mcp` and talks to it over stdin and stdout. This is what desktop clients do.
- **Streamable HTTP**: the client connects to `http://127.0.0.1:7878/mcp` while `claudius studio` or `claudius serve`
  runs.

## Setting up a client

### Claude Desktop

```sh
claudius connect claude-desktop
```

This adds an entry to Claude Desktop's configuration, keeps the old file as `claude_desktop_config.json.bak`, and
asks you to quit and reopen Claude Desktop. `claudius connect claude-desktop --remove` takes it out again.

By hand, add this to the configuration file, with the absolute path of your `claudius` (`which claudius` prints
it; Claude Desktop does not see your shell's `PATH`):

```json
{ "mcpServers": { "claudius": { "command": "/Users/you/.local/bin/claudius", "args": ["mcp"] } } }
```

| system | configuration file |
|---|---|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

Claude Desktop starts its local servers itself, so `--http` is refused for it.

### Claude Code

```sh
claudius connect claude-code          # runs: claude mcp add --scope user claudius -- /path/to/claudius mcp
claudius connect claude-code --http   # runs: claude mcp add --scope user --transport http claudius http://127.0.0.1:7878/mcp
```

When the `claude` command is not on your `PATH`, `connect` prints the command for you to run where it is.

### Cursor

```sh
claudius connect cursor               # "claudius": { "command": "/path/to/claudius", "args": ["mcp"] }
claudius connect cursor --http        # "claudius": { "url": "http://127.0.0.1:7878/mcp" }
```

Both edit `mcpServers` in `~/.cursor/mcp.json` and keep a `.bak` copy. A project's `.cursor/mcp.json` takes the same
entry.

### Codex

```sh
claudius connect codex
```

This adds `[mcp_servers.claudius]` to `~/.codex/config.toml`, with the absolute path of `claudius` and `args =
["mcp"]`, and leaves the rest of the file as it was (comments included), with a `config.toml.bak` copy. Codex starts
local servers itself, so `--http` is refused for it.

### Skills

`claudius connect` also installs claudius' [skills](../getting-started/assistants.md#skills) for the clients that
read them from a folder (Claude Code, Cursor, Codex), and writes them as zips for the Claude apps. `--no-skills`
leaves them out.

### Any other client

`claudius connect print` prints the stdio configuration as JSON, the HTTP address, and the Claude Code command.
`--port` (or `CLAUDIUS_PORT`) changes the port in the HTTP address.

### ChatGPT and other assistants in the cloud

An assistant that runs on someone else's computer cannot start `claudius mcp` or reach 127.0.0.1. It needs a public
HTTPS address, so put claudius behind a tunnel and require a token:

```sh
TOKEN=$(openssl rand -hex 16); echo "$TOKEN"
claudius serve --public --token "$TOKEN"
cloudflared tunnel --url http://127.0.0.1:7878
```

`--public` makes claudius answer requests addressed to the tunnel's host name; it still listens on 127.0.0.1. In
ChatGPT's developer mode, add a custom connector with the URL `https://<tunnel>/mcp?token=<token>`. Clients that can
send headers can use `Authorization: Bearer <token>` instead of the query string. Open
`https://<tunnel>/?token=<token>` in a browser to use the studio through the same tunnel.

`--public` always runs with a token. Without `--token` it makes a new random one at every start and prints it, which
is enough for a browser but means updating the connector after each restart; a token of your own, as above, stays
the same. A shared claudius reads and writes files only in its own folders; the [security
model](../engineering/security.md) explains what the token, `--public` and those folders protect.

## The studio and the assistant on one song

`claudius mcp` first asks `http://127.0.0.1:<port>/api/hello` (or `--url`) whether a claudius is running:

```text
                     claudius studio (or serve) on :7878
                      session: song, undo, renders
                     /             |              \
         studio window      browser tab        claudius mcp  <-- stdio --  Claude Desktop
                                                (HTTP client)
```

- **A studio is running**: every tool call becomes a request to its HTTP API. The assistant edits the song you have
  open, you hear each render in the studio, and you can change things yourself between its calls. Undo in the studio
  takes back the assistant's changes too.
- **A claudius that asks for a token is running** (`serve --token`, or `--public`), and `claudius mcp` was not given
  that token: it stops with `the claudius at http://127.0.0.1:7878 asks for a token: pass --token or set
  CLAUDIUS_TOKEN (a wrong one is refused too)` rather than quietly working on a song of its own.
- **Nothing is running**: `claudius mcp` starts a session of its own and serves the web studio on `--port`. Open
  `http://127.0.0.1:7878` to see and hear what the assistant does. If the port is taken by another program, the
  session still works but has no web studio.

So open the studio before you start the assistant's chat. With `--url http://host:port` and `--token`, `claudius mcp`
drives a claudius elsewhere, and fails if none answers there.

A client connected over HTTP (`/mcp`) always works on that server's session; there is nothing to decide.

## The tools

A tool that edits the song waits until the change is rendered and judged, then answers with the song and the
verdict, so the assistant hears the result of every change. Each tool that waits gives up after a few minutes and
says the job is still running; `status` follows it from there.

| tool | parameters | what it does |
|---|---|---|
| `new_song` | `genre` (required), `seed`, `bpm`, `key`, `loops` (false) | composes a new song in a genre, renders and judges it |
| `get_song` | | the open song and its verdict, and the whole song as a script |
| `run_script` | `code` (required), `listen` (true) | edits the song with a [script](../language/overview.md) |
| `script_reference` | | the script language reference (markdown) |
| `listen` | `stems` (false) | renders the song if it changed and returns the verdict |
| `mutate` | `count` (4, 1 to 32), `keep_best` (true) | tries variations steered by the critic's issues and keeps the best if it beats the song |
| `evolve` | `generations` (3, 1 to 20), `children` (6, 2 to 32) | runs an evolutionary search from the song and keeps the best candidate if it beats the song |
| `status` | | idle or busy (with progress), the last 25 log lines, the latest verdict |
| `undo`, `redo` | | takes back or brings back the last change, then renders |
| `stop` | | stops a mutation after the candidate in progress, or an evolution after its generation; a render always finishes |
| `catalog` | | the genres with tempo ranges, drum names, synth presets, installed instruments and every pack with its licence |
| `save` | `path` (optional) | writes the song folder: `blueprint.json` and `song.rhai` at once, then, from a render of the song as it is then, the whole folder `claudius make` writes, stems included; a save asked for during a job waits for it |
| `open` | `path` (required) | opens a song folder, a `blueprint.json`, or a `.rhai` script (which runs on the open song) |
| `analyze` | `path` (required) | tempo, key, progressions, sections, drum grid and bass style of an audio file |
| `remake` | `path` (required), `genre`, `seed` | a new song on the skeleton of an audio file, in a genre's sounds |
| `command` | `command` (required, an object), `listen` (true) | sends one raw [API command](http-api.md#commands) |

Details that matter when you read an assistant's transcript:

- `listen: false` on `run_script` and `command` skips the wait, so an assistant can batch several edits and listen
  once. The studio still renders each change unless it runs with `--no-auto-render`.
- `listen` waits up to 240 s, `save` 240 s, `evolve` 280 s and `mutate` 300 s. `mutate` and `evolve` answer with the
  log lines the job wrote (one per candidate and its score) and then the song.
- A `seed` is optional everywhere. Without one, the session picks one from the clock; `new_song` names it in its
  message and `get_song` shows it in the script's first line, so the song can be made again.
- `save` without a path writes to `~/Music/claudius/<genre>-<title>-<seed>`. Paths are on the computer running
  claudius, and a shared claudius refuses paths outside its folders.
- `command` covers the edits the studio makes and scripts do not: `add_section`, `remove_section`, `move_section`,
  `toggle_layer`, `set_drum_step`, `set_section` (including `prog`), `set_part` (including `reseed`) and `set_mix`
  (including `solo`).
- A failed edit changes nothing and comes back as a tool error (`isError: true`) with claudius' message, the same text
  the studio shows:

```text
Runtime error: track 'x': unknown sound 'nope' (drums: bd kick sd snare cp clap hh ch hat oh ohat rd ride cr crash lt mt ht tom rim rs sh shaker cb cowbell snap perc; synths: sub_bass 808 reese acid saw_bass pluck_bass organ_bass wobble pluck supersaw pad warm_pad stab epiano keys bell organ strings square_lead chip; sampled: piano upright rhodes wurlitzer hammond string_ensemble violins cellos pizzicato brass trumpet flute harp marimba vibes kalimba upright_bass guitar choir sax ... (claudius-packs aliases)) (line 1, position 1)
```

### What the assistant reads back

This is `listen` after `tempo(122); mix("lead", #{ level: -2 });` on a house song made with seed 9, in a session on
port 7931:

```text
Crimson Tide — house at 122 BPM in G dorian, swing 0.53, 64 bars (2:06)
progression A: i7 ii7 i7 IV7
progression B: i7 VIImaj7 i7 IV7
form:
  0. intro (intro) bar 0 +8, energy 0.40, drums 'sparse' [filter_in]: kick hat perc top
  1. groove (groove) bar 8 +16, energy 0.70, drums 'main' [fills]: kick clap hat ohat perc bass chords top
  2. break (break) bar 24 +8, energy 0.35, drums 'main' [build, gap]: pad lead chords
  3. drop (drop) bar 32 +24, energy 0.95, drums 'main' [impact, fills]: kick clap hat ohat shaker perc bass chords lead arp top
  4. outro (outro) bar 56 +8, energy 0.40, drums 'sparse': kick hat perc bass
tracks:
  kick [drum kick] sampled bd (tr909:bd); level +0.0 dB, pan +0.00, reverb 0.00, delay 0.00, duck 0.00, -13.2 LUFS
  clap [drum clap] sampled cp (tr909:cp); level +0.0 dB, pan +0.00, reverb 0.18, delay 0.00, duck 0.00, -15.2 LUFS
  ...
  lead [part lead] synth patch 'pluck'; level -2.0 dB, pan +0.00, reverb 0.25, delay 0.20, duck 0.20, -20.7 LUFS
  arp [part arp] synth patch 'bell'; level +0.0 dB, pan +0.20, reverb 0.20, delay 0.25, duck 0.30, -21.2 LUFS
  fx [fx fx] risers, impacts, crashes; level +0.0 dB, pan +0.00, reverb 0.25, delay 0.00, duck 0.00
render: -10.1 LUFS, true peak -1.0 dBTP, 128 s
verdict 97.2/100: 97/100 (arc 0.90, mix 1.00, theory 1.00)
  scores: arc 0.90, mix 1.00, theory 1.00
audio: http://127.0.0.1:7931/api/audio/8.wav (the studio at http://127.0.0.1:7931 plays it)
```

`get_song` adds the song as a script in a `js` block. A verdict of a song with several sections also has a
`loudness by section (LUFS):` line, each section's median loudness in the order they play, which shows whether the
drop is really the loudest part. With earshot's models loaded, the verdict has a line that starts with `ears:`, and
up to eight issues follow, each with where its fix lives (`issue (0.24): dull 4-8 kHz: ... — fix: band:presence-`). [The critic and the ears](../engineering/critic.md)
explains them.

## Prompts that work

The assistant does better with what you want to hear than with what to type. Some that work well:

- "Make a deep house track at 122 BPM in F minor. Keep the intro sparse and make the drop hit hard. Listen to it and
  tell me the two biggest problems before you change anything."
- "Read the script reference, then replace the chords with a Rhodes playing i9 VImaj9 iv9 v7, two bars each."
- "The breakdown is too busy. Take the lead out of it and lower the pad by 3 dB. Undo if the verdict gets worse."
- "Try eight mutations and keep the best one. Then tell me what changed, in musical terms."
- "Remake ~/Music/reference.mp3 as lofi, keep its progression, and put the drums on the 808 kit."
- "Write the hats yourself as a pattern track: sixteenths, with an open hat on every offbeat and a roll before each
  drop."
- "Save it to ~/Music/claudius/night-drive."

Two habits help: ask the assistant to `get_song` before it edits a song you made in the studio, so it starts from
what is there, and ask it to say what it changed in musical words, since you hear the result and it reads numbers.

## MCP over HTTP

The endpoint at `/mcp` is MCP's Streamable HTTP transport. Every request is a POST with
`Content-Type: application/json` and `Accept: application/json, text/event-stream`; replies come as server-sent
events. The first request is `initialize`, whose reply carries an `mcp-session-id` header that later requests send
back. The endpoint sits behind the same guards as the rest of the server: loopback host names only unless `--public`,
and the token when one is set.

```sh
curl -si http://127.0.0.1:7878/mcp -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"curl","version":"1"}}}'
```

```text
HTTP/1.1 200 OK
content-type: text/event-stream
mcp-session-id: 67068efb-d5e6-4618-84eb-326443bc774c
...
```

A request without the session id gets `422 Unexpected message, expect initialize request`; one without both `Accept`
types gets `406 Not Acceptable`.
