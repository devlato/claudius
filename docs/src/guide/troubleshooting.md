# Troubleshooting

Start with `claudius doctor`: it checks the web studio, the packs, the models, Apple Loops, ffmpeg and the port, and
says what to do about each. The sections below are the problems it cannot see, by the message you get.

## The verdict has no ears

```text
earshot is unavailable (download failed: https://huggingface.co/Xenova/larger_clap_general/resolve/...); judging with meters and theory only
```

The verdict then has `mix`, `arc` and `theory` scores but no `genre` or `production`. The models download on first
use, 785 MB, from the Hugging Face Hub. The usual causes:

- **No network on the first run.** Run `claudius setup --no-packs` once while online.
- **`EARSHOT_OFFLINE=1` is set.** Then a model that is not cached fails with
  `onnx/audio_model.onnx is not in the cache (...) and offline mode is on (EARSHOT_OFFLINE=1 / EarsOptions::offline): run once online to download it`.
- **A download was altered or cut short.** Every model file is checked against a pinned SHA-256 hash, and a file
  that does not match is thrown away: `... does not match its pinned SHA-256 (got ..., expected ...); not using it`.
  Run it again; if it keeps failing, something between you and the Hub changes the files.
- **`EARSHOT_HOME` points somewhere unwritable**: `cannot create <folder>`.

The first load after the download spends about 30 s embedding earshot's vocabulary; later loads take 4 to 8 s. In the
studio, verdicts include the ears from the moment the log says `ears ready (...)`. `--no-ears` skips the models
altogether, which also saves about half a gigabyte of memory.

## The port is in use

```text
$ claudius serve --port 7933
Error: port 7933 is taken, by another claudius or another program (--port picks another)
```

`claudius studio` stops with the same message when the program on the port is not claudius; when it is a claudius,
the studio opens on it instead. `claudius doctor` tells the cases apart:

```text
  --  port 7933       used by another program: pass --port to studio, serve and mcp
  ok  port 7922       claudius is running here
```

Pick another port with `--port` or `CLAUDIUS_PORT`, for every command that should meet there: `claudius studio
--port 7900` and an assistant configured with `claudius mcp --port 7900`. `claudius mcp` does not stop: it runs a
session of its own without a web studio and says so on stderr:

```text
claudius mcp: a session of its own (no web UI: port 7933 is taken, by another claudius or another program (--port picks another))
```

## The assistant edits a different song from the studio

`claudius mcp` works on the studio's song only if it finds the studio. It looks at `http://127.0.0.1:7878` (or
`--port`, `--url`) when the assistant starts it, so:

- **The studio was started after the assistant.** Restart the assistant's MCP server (in Claude Desktop, quit and
  reopen it) once the studio is up. stderr says which way it went: `driving the claudius app at ...` or
  `a session of its own`.
- **The studio runs on another port.** Use the same `--port` in both.
- **The claudius runs with a token** (`claudius serve --token`, or `--public`). `claudius mcp` then stops instead of
  starting a session of its own, and the client shows a server that failed to start:
  `the claudius at http://127.0.0.1:7878 asks for a token: pass --token or set CLAUDIUS_TOKEN (a wrong one is refused
  too)`. Give it the token: `CLAUDIUS_TOKEN` in the client's environment, or `claudius mcp --token <token>`.

## An MCP client shows no claudius tools

- **The client was not restarted** after `claudius connect`. Claude Desktop reads its configuration at start.
- **The configured program is not there.** `connect` writes the absolute path of the `claudius` that ran it. If you
  moved or reinstalled it elsewhere (say from `~/.cargo/bin` to `~/.local/bin`), run `claudius connect` again. A
  hand-written entry with plain `"command": "claudius"` fails in apps that do not see your shell's `PATH`.
- **The configuration file is not valid JSON.** `connect` refuses to touch it:
  `<file> is not valid JSON; fix it or move it aside, then run this again`.
- **An HTTP entry with nothing listening.** `--http` entries (`"url": "http://127.0.0.1:7878/mcp"`) need
  `claudius studio` or `claudius serve` running.

To see what the client sees, run the server by hand and give it an `initialize` request:

```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}' | claudius mcp --no-ears
```

It should answer with a line starting `{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":...` and claudius'
instructions. The protocol is on stdout and everything else on stderr, which Claude Desktop writes to
`~/Library/Logs/Claude/mcp-server-claudius.log`.

## Sample packs are missing

A script that asks for a drum machine or an instrument without packs fails with what to do:

```text
Error: Runtime error: track 'd': no sample packs are installed for bank 'tr909' (install them with: claudius-packs install --defaults) (line 1, position 17)
```

and an unknown sound ends its list with `sampled instruments need packs: claudius-packs install --defaults`. In a
session started with `--synth-only` the packs are installed but switched off, and the same mistakes say so:
`sampled instruments are off in this session (--synth-only)` and `bank '808' plays sample packs, which are off in
this session (--synth-only)`.

A song made where packs were installed does not render where they are not, because its blueprint names the
instruments it plays:

```text
Error: no installed instrument "tr808:sh" (no packs installed: claudius-packs install --defaults)
```

Install the packs (`claudius setup`), or only the pack named before the colon
(`claudius-packs install tr808`). Under `--synth-only` the hint reads `the sample packs are off in this session`
instead. A file deleted from inside a pack fails with
`sample missing: <path> (reinstall the pack)`; installing the pack again downloads only what is missing.

Installs that fail say so per pack and resume when run again:

```text
claudius-packs: vsco2: ...
Error: could not install: vsco2 (run again to resume)
```

Two causes need more than a retry. `set GITHUB_TOKEN if the API rate limit was hit` means GitHub's 60 requests an
hour without an account ran out: set `GITHUB_TOKEN` to any token. `cannot extract <archive>: ...` means the
system's `tar` could not unpack the `tr909` zip or the organ's `.tar.xz`; install bsdtar or GNU tar with xz support
(and `unzip`).

## ffmpeg is missing

claudius does not need ffmpeg to work. Without it:

- songs have `song.wav` and no `song.mp3`, without an error;
- the web studio is sent the WAV where it asks for MP3, which browsers play as well;
- files in formats claudius cannot decode itself fail with
  `cannot decode <file>: <reason> (installing ffmpeg adds more formats)`. It reads WAV, AIFF, FLAC, MP3, AAC and ALAC,
  CAF and Ogg Vorbis on its own.

`claudius doctor` shows `..  ffmpeg  not installed (optional: songs are saved as WAV only)`. claudius looks for it on
`PATH` and then in `/opt/homebrew/bin`, `/usr/local/bin` and `/usr/bin`, since apps started from the Dock do not
get your shell's `PATH`. `CLAUDIUS_FFMPEG=/path/to/ffmpeg` names one, `CLAUDIUS_FFMPEG=none` turns it off.

Two files do not decode even with ffmpeg:

- **A file longer than 30 minutes** (of stereo at 48 kHz): `<file> is too long: claudius decodes up to 30 minutes of
  stereo at 48 kHz`. Cut the part you want with any audio editor, or with ffmpeg:
  `ffmpeg -i mix.flac -t 600 first-ten-minutes.flac`.
- **A playlist or a list of files** (`.m3u8`, an ffconcat list): ffmpeg is allowed to open only the file it is given,
  so it says `Format not on whitelist`. Point claudius at the audio file itself.

## The studio window has no sound

The status bar at the bottom of the window shows audio problems after `audio:`:

- `no audio output: ...`: the window could not open the system's default output device when it started. It opens
  the device once, so after you plug in headphones or change the output, restart the window.
- `could not fetch http://127.0.0.1:7878/api/audio/<n>.wav: ...` or `... answered 404 Not Found`: the render is not
  there. A session keeps only its last ten renders.

If there is no error, there may be nothing to play yet. The first render of a new song takes a few seconds (the
toolbar shows the job in progress), and with `--no-auto-render` nothing renders until you press Render. Check the
volume slider at the bottom right too.

In the browser, the first playback has to start from a click (browsers block sound a page starts on its own), and
the page plays the MP3, or the WAV without ffmpeg.

If the window shows `Cannot reach the claudius server at http://127.0.0.1:7878`, the server it was opened for is
gone; start `claudius studio` again. `Use the demo` switches to a built-in song that needs no server.

## The studio window cannot open a claudius with a token

```text
$ claudius studio --port 7922
Error: the claudius on port 7922 asks for a token, which the studio window cannot send: open http://127.0.0.1:7922/?token=<token> in a browser, or start another studio with --port
```

`claudius studio` opens on a claudius it finds on the port, and starts its own session when there is none. The native
window cannot send a token, so a claudius started with `--token` or `--public` is for the browser: open the address
in the message with your token in place of `<token>`. `claudius studio --port 7900` starts a separate session instead,
with a song of its own.

## A song opens but does not render

Songs that play audio files (Apple Loops, chops, files from scripts) store their paths. On another computer, or after
the files moved:

```text
render failed: no such file: /Users/me/Samples/vox.wav
```

On a shared claudius, opening such a song fails earlier, with `cannot open <folder>/blueprint.json: the song plays
<path>, which is outside the folders this claudius may read`, whether the file is missing or outside the shared
folders.

## A shared claudius refuses to save

```text
/Users/<you>/Samples/mix is outside the folders this claudius may write
```

A claudius started with `--public`, `--files`, or `--host` with an address other than loopback saves only inside its
folders: `~/Music/claudius` (the default), its
caches and the sound libraries, plus each `--files` folder. Save there, or start it with `--files` for the folder you
want.

## A script or a song hits a limit

Scripts run under limits so that a mistake cannot hang claudius; [Errors and limits](../language/limits.md) lists
them all. The ones people meet:

| message | what to change |
|---|---|
| `the script ran for more than 10 seconds` | a loop that does too much; scripts are meant to describe a song, not compute one |
| `Too many operations` | the same, counted in operations (5 million) |
| `'*' takes a number above 0 and up to 256, not 100000` | a smaller factor; the same limit holds for `/`, `fast`, `slow` and `ply` |
| `track 'x': this pattern is too dense to play: it makes more than 262144 events` | fewer speed-ups inside speed-ups: `s("[bd*256]*256")` is 65,536 hits a bar |
| `track 'x': this pattern is too dense to play: it needs more than 4000000 steps of work` | a long sequence sped up a lot, such as a thousand rests and a `bd` played `*256` |
| `track 'x' plays 2048 notes per bar; the limit is 1024` | fewer events per bar |
| `the song is 640 bars long; the limit is 512` | shorter sections, or fewer of them |
| `the song is 25 minutes long; the limit is 20` | the same at a slow tempo |
| `the song has 70 tracks; the limit is 64` | fewer tracks; a pattern with several sounds plays as a track for each |
| `24 tracks of 10.0 minutes are more than claudius renders at once (200 track-minutes, ...)` | fewer tracks, or a shorter form |
| `mini-notation is limited to 4096 characters (this one has 6000)` | split the pattern into tracks |

The HTTP API refuses commands over 1 MiB with `413 Payload Too Large`. `mutate` tries at most 32 variations and
`evolve` runs at most 20 generations of at most 32 children; larger numbers are lowered to those.

## The session is busy

```text
busy (rendering); try again when it is done, or cancel it
```

One job runs at a time: a render, a mutation, an evolution or a save. Edits still work while a job runs, and a render
asked for during a job runs after it. A save asked for during a job waits for it too, and says so:
`saving to <folder>: blueprint.json and song.rhai now, the audio once the current job (rendering) is done`.
`cancel` (`stop` for assistants) ends a mutation or an evolution after the candidate or generation in progress; a
render always finishes.

## The analysis of a song is wrong

`claudius analyze` and `remake` read tempo, key and chords from the mixed audio, and get some songs wrong:

- **Half or double tempo.** Pass `--genre`, whose tempo range guides the beat tracker.
- **The relative key.** A minor song can read as its relative major (D minor as F major or B♭ major when the chords
  favour them); the chords are usually right, and named in the other key.
- **The progression starts on the wrong chord.** The downbeat was placed a beat or a bar off, so the progression is
  rotated.

Analysis is best on steady electronic music with a clear kick. [Evolution and
analysis](../engineering/evolution-and-analysis.md#reading-a-song) explains how it reads a song.

## The disk is filling up

The cache folder (`~/Library/Caches/claudius`, `~/.cache/claudius` on Linux) grows: decoded audio (about 23 MB per
minute of every file claudius reads), warped loops, and each running session's last ten renders (about 70 MB each for
a four-minute song). Delete it while claudius is not running; [Files and settings](files.md#caches) says what each
folder holds. Sample packs and models are elsewhere and are not affected.

## Something else

Run the command again with `CLAUDIUS_DEBUG=1` for render timings, and report it at
[github.com/devlato/claudius/issues](https://github.com/devlato/claudius/issues) with the command, its output and
`claudius doctor`. Security problems go to the private channel in [SECURITY.md](https://github.com/devlato/claudius/blob/main/SECURITY.md).
