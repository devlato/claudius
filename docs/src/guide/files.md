# Files and settings

claudius keeps songs, sounds, models and caches in separate places, so that each can be backed up, moved or deleted
on its own. The paths below are for macOS and Linux; where an environment variable moves a folder, it is named.

## Where everything is

| what | macOS | Linux | moved by |
|---|---|---|---|
| songs saved from the studio and the MCP tools | `~/Music/claudius` | `$XDG_MUSIC_DIR/claudius`, else `./out` | the `path` of `save` |
| songs written by the command line | `./out` (where you run it) | `./out` | `-o` |
| caches | `~/Library/Caches/claudius` | `$XDG_CACHE_HOME/claudius` or `~/.cache/claudius` | `CLAUDIUS_HOME` |
| sample packs | `~/Library/Application Support/claudius/packs` | `~/.local/share/claudius/packs` | `CLAUDIUS_PACKS` |
| scratch space while a pack installs | `~/Library/Caches/claudius/packs` | `~/.cache/claudius/packs` | |
| listening models | `~/.cache/earshot-rs` | `~/.cache/earshot-rs` | `EARSHOT_HOME` |
| your genres | `~/Library/Application Support/claudius/genres` | `~/.config/claudius/genres` | |
| the answer about [usage statistics](privacy.md) | `~/Library/Application Support/claudius/telemetry.json` | `~/.config/claudius/telemetry.json` | |
| whether to [update](updates.md) automatically, and the day it last looked | `~/Library/Application Support/claudius/updates.json` | `~/.config/claudius/updates.json` | |
| studio window preferences | `~/Library/Application Support/claudius-studio/app.ron` | `~/.local/share/claudius-studio/app.ron` | |
| Apple Loops (read only) | `/Library/Audio/Apple Loops`, `~/Library/Audio/Apple Loops` | | |
| programs and web studio (install script) | `~/.local/bin`, `~/.local/share/claudius/web` | the same | `CLAUDIUS_INSTALL_DIR` |

On Linux, `$XDG_MUSIC_DIR` comes from `~/.config/user-dirs.dirs`; without it, the studio saves under `out` in the
folder it was started from. The web studio keeps its preferences in the browser's local storage.

## Song folders

A song folder holds a song in every form claudius has for it. One function, `song::write_folder`, writes every
folder, whether the CLI (`make`, `script`, `render`, `remake`, `evolve`, `mutate`) or a `save` in the studio or over
MCP made it, so they hold the same files:

| file | what it is |
|---|---|
| `song.wav` | the master, 48 kHz, 24-bit |
| `song.mp3` | a 256 kbps copy, when ffmpeg is installed |
| `blueprint.json` | the song's genome |
| `song.rhai` | the song as a script |
| `project.json` | the arranged session |
| `report.json` | tempo, key, form, loudness, and every track's level and gain |
| `verdict.json` | the critic's verdict |
| `credits.txt` | attributions, when sample packs play |
| `stems/<track>.wav` | every track, 32-bit float: always from a `save`, from `make` and `render` with `--stems` |

Stems are floating point because a track can peak above full scale before the master's limiter brings the mix down.
`claudius judge` runs its arrangement and theory checks when `project.json` sits next to the audio, so any song
folder gets the full verdict.

`blueprint.json` is the song: `claudius render`, `claudius script --on`, `claudius evolve --from`, `claudius
mutate`, and `open` in the studio and the MCP tools all start from it. [The song model](../engineering/song-model.md)
describes its fields. Every later claudius opens it; a newer claudius's song, in a format this one does not read
yet, says so and asks for an update. A blueprint that plays audio files (Apple Loops, chops, files from scripts) stores their paths,
so it plays on another computer only where those files exist.

## Caches

Everything in the cache folder can be deleted while claudius is not running; it is rebuilt on demand.

| folder | what | grows by |
|---|---|---|
| `decoded/` | every audio file claudius has read, decoded to raw 48 kHz stereo `.f32`, keyed by path, size and modification time | about 23 MB per minute of audio |
| `stretched/` | Apple Loops and samples warped to a tempo and key | one file per loop, tempo and key |
| `samples/` | drum one-shots cut from Apple Loops | a few hundred KB each |
| `user/library.json` | the user library: chops and found sounds from `claudius chop` | |
| `renders/<process id>/` | a running session's renders: `<version>.wav`, `.mp3`, and `<version>/` with stems | about 70 MB per four-minute render, and a WAV per track more for a render with stems, which every save makes: a new two-minute lofi song and one save left 433 MB |

A session keeps its last ten renders and deletes older ones as it goes. It holds a lock on `.in-use` in its folder
while it runs, which the system lets go of when the process ends, however it ends. When a session starts, it deletes
the folders of sessions that have ended: a free lock, and nothing written for a minute. Folders without a lock file,
from claudius before 0.3, go once nothing has been written to them for 12 hours. `decoded/` and `stretched/` are never
trimmed: on a machine that renders and analyses a lot, they reach gigabytes, and deleting them costs only the time to
decode again.

The pack installer's scratch space holds downloaded archives and their unpacked files only while a pack installs,
and is removed afterwards.

## Sample packs and models

A pack is a folder under the packs folder with an `index.json` and a `files/` folder;
[Sample packs](packs.md) covers installing and removing them. `claudius-packs list` prints the folder in use.

The models live under `models/<repository>/<revision>/` in the models folder, one revision folder per pinned model,
with CLAP's text embeddings of earshot's vocabulary cached in `text/clap-d78c3994912c.bin`. Deleting the folder makes
the next verdict download 785 MB again (1.13 GB with the AudioSet tagger).

## Your genres

A genre is a JSON file. claudius loads its eight built-in genres, then every `.json` file in your genres folder; a
file whose `name` matches a built-in genre replaces it. The fields are those of
`crates/claudius/data/genres/house.json`; copy it, rename the `name`, change what you like, and
`claudius genres` lists it. A file that does not parse makes every genre lookup fail, the built-in genres included,
so `make` and `song()` fail with `Error: genre file <path>` and the JSON error under `Caused by:`, and the studio's
list of genres comes up empty. Fix or move the file and try again.

## The web studio

`claudius studio`, `serve` and `mcp` serve the web studio at `/` from the first of these folders that has an
`index.html`:

1. `--web <dir>`, or `CLAUDIUS_WEB`;
2. `share/claudius/web` next to the program's folder (`~/.local/share/claudius/web` for `~/.local/bin/claudius`);
3. `web` inside the program's folder;
4. `crates/claudius-ui/dist` of the checkout the program was built from (`cargo xtask web` builds it there).

Without one, `/` shows a page that says how to build it, and the API and MCP work as usual.

## Assistant configuration

`claudius connect` edits these files, keeping a copy of the previous version as `<file>.bak`:

| client | file |
|---|---|
| Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS), `%APPDATA%\Claude\claude_desktop_config.json` (Windows), `~/.config/Claude/claude_desktop_config.json` (Linux) |
| Cursor | `~/.cursor/mcp.json` |
| Claude Code | its own configuration, through `claude mcp add --scope user` |

## Environment variables

| variable | default | what it changes |
|---|---|---|
| `CLAUDIUS_HOME` | the cache folder above | where caches and session renders go |
| `CLAUDIUS_PACKS` | the packs folder above | where packs are installed and read |
| `CLAUDIUS_PACKS_CACHE_MB` | `3072` | memory for decoded sample-pack audio |
| `CLAUDIUS_PORT` | `7878` | `--port` of `studio`, `serve`, `mcp`, `doctor` and `connect` |
| `CLAUDIUS_TOKEN` | none | `--token` of `serve` and `mcp` |
| `CLAUDIUS_WEB` | found as above | the web studio folder |
| `CLAUDIUS_FFMPEG` | `ffmpeg` on `PATH`, then `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin` | the ffmpeg claudius uses for formats it cannot decode and for MP3; `none` turns it off |
| `CLAUDIUS_DEBUG` | unset | when set, renders print their timings and the beat tracker its cues to stderr |
| `DO_NOT_TRACK`, `CLAUDIUS_TELEMETRY` | unset | `DO_NOT_TRACK=1` or `CLAUDIUS_TELEMETRY=0` keeps [usage statistics](privacy.md) off whatever the answer |
| `CLAUDIUS_TELEMETRY_URL` | the Worker in `telemetry/` | where usage statistics go, to test an endpoint (the only place a debug build sends them) |
| `CLAUDIUS_UPDATES` | on | `off` stops the studio's daily look for [updates](updates.md) |
| `CLAUDIUS_UPDATE_SINCE` | unset | a version for the updater to act as, to try an update against the latest release |
| `EARSHOT_HOME` | `~/.cache/earshot-rs` | where the listening models and text embeddings go |
| `EARSHOT_OFFLINE` | unset | `1` never downloads models; a verdict without them uses meters and theory |
| `EARSHOT_THREADS` | half the cores, at most 4 | threads per model |
| `EARSHOT_FFMPEG`, `EARSHOT_FFPROBE` | found like `CLAUDIUS_FFMPEG` | the ffmpeg and ffprobe earshot decodes reference tracks with |
| `GITHUB_TOKEN`, `GH_TOKEN` | unset | sent to GitHub's API while installing packs, for its higher rate limit |
| `CLAUDIUS_SERVER` | `http://127.0.0.1:7878` | the server the standalone `claudius-ui` program connects to |
| `CLAUDIUS_MOCK` | unset | `1` makes `claudius-ui` run against its built-in demo song |
| `CLAUDIUS_UI_PLAN` | unset | a scripted run of the studio window, for testing ([The studio UI](../engineering/ui.md#scripted-runs)) |

The install script reads `CLAUDIUS_VERSION`, `CLAUDIUS_INSTALL_DIR` and `CLAUDIUS_DOWNLOAD_URL`; see
[Install](../getting-started/install.md).
