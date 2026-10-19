# Server, sessions and MCP

`crates/claudius-server` wraps the engine in a session that many clients share: the studio window, browser tabs,
assistants over MCP and anything that speaks HTTP. [0005 One session, many clients](../design/0005-one-session.md)
records why there is exactly one.

| file | what it holds |
|---|---|
| `lib.rs` | `serve` (on the caller's runtime) and `spawn` (on a thread with its own runtime, for the native window), and what a taken port says |
| `app/mod.rs` | `App` and its `Session`: state, versions, waiting for changes |
| `app/commands.rs` | every `Command`: edits, undo and redo, load and save, analysis |
| `app/jobs.rs` | the long jobs: render and judge, mutate, evolve, save, install packs |
| `http.rs` | the routes, the guard (host names, token), CORS, security headers |
| `mcp.rs` | the MCP tools, the backends they run on, the stdio and HTTP transports |
| `views.rs` | blueprints and verdicts as `claudius-api` types, and the text summaries the MCP tools return |

## The session

An `App` holds one `Session` behind a mutex:

| field | what it is |
|---|---|
| `version` | bumped by every change to anything below |
| `song_version` | the `version` at which the song last changed |
| `bp`, `view` | the blueprint, and its `SongView` (built once per change, not per request) |
| `undo`, `redo` | stacks of whole blueprints with labels, at most 200 |
| `render` | the latest finished render's `RenderView` and the paths of its stems |
| `verdict`, `loudness` | its verdict, and each track's loudness in the master |
| `busy`, `progress`, `job`, `rerender` | the running job, and whether a render was asked for meanwhile |
| `pending_saves` | the folders saves asked for during a job, all written from one render when the job ends |
| `log` | the last 200 lines |

Four primitives keep it consistent:

- `lock()` takes the session mutex for a moment, to read or swap state, never across a script or a render; a
  poisoned lock is recovered rather than propagated, since the state is valid after any panic in a job.
- `update(f)` runs `f` on the session, bumps `version`, and sends the new version on a `tokio::sync::watch` channel,
  which wakes every long poll.
- `try_update(f)` is `update` for a change that may be refused: when `f` returns an error it must have left the
  session as it was, and there is no new version, so nobody waiting on the state wakes for nothing. A failed undo or
  redo and a job refused as busy go through it.
- The `edit` mutex serialises edits. Scripts take milliseconds to seconds (cutting a one-shot from an Apple Loop the
  first time, for instance), and two edits must not both start from the same blueprint.

### Edits

Every edit works on a copy and commits only on success:

- `run_script(code)` runs the script on a copy of the blueprint. If the copy did not change, the reply is the
  script's log (or `ran; nothing changed`) and no undo step is made. If it changed, it is committed with a label for
  undo: the caller's (`tempo 124`, `mix bass: level: -2, duck: 0.6`), or one made from the script's first line.
- `set_tempo`, `set_key`, `set_swing`, `set_progression`, `set_drum_pattern`, `set_part` and `set_mix` build script
  code (every string through `pattern::quote`) and go through `run_script`, so the studio's controls and scripts
  validate the same way and fail with the same messages.
- The other edits (`set_section`, `add_section`, `remove_section`, `move_section`, `toggle_layer`, `set_drum_step`)
  change the blueprint directly in `edit_with`, which also checks the song's size and commits only when something
  changed.

`commit` pushes the old blueprint and the label onto `undo`, clears `redo`, rebuilds the view, logs the message, and
then `after_change` sets `song_version` to the new version and, unless the session runs with `--no-auto-render`,
starts a render. Undo and redo swap whole blueprints between the stacks; a blueprint is tens of kilobytes, so 200 of
them cost a few megabytes, and undo can never fail to restore exactly what was there.

### Versions and waiting

`state_after(since, timeout)` returns at once when `version > since`, and otherwise waits on the watch channel until
it is, or until the timeout. `settled(timeout)` waits until `is_settled()`: no job is running, and either there is no
song, or the render is of the current song (`render.version >= song_version`), or auto-render is off and no render is
pending. The HTTP API exposes both as `GET /api/state?since=N` and `?settled=1`; the MCP tools wait on `settled` after
every edit.

A render belongs to the song version it started from, so a client can always tell a stale render from a current one,
even while the next render runs. Audio URLs carry that version (`/api/audio/12.mp3`), so a URL always means the same
audio, until the session prunes it.

### Jobs

Renders, mutations, evolutions, saves and pack downloads are jobs. A pack download that ends passes its outcome to
`claudius_server::REPORT`, the hook through which `claudius-cli` counts it in its usage statistics, which send nothing
unless the person turned them on ([privacy](../guide/privacy.md)). `start_job` (and, for `install_packs`, which needs
no song, `take_slot` alone) takes the job slot through `try_update`, so a refusal
(`busy (rendering); try again when it is done, or cancel it`) changes nothing, not even the version; it marks the
session busy and `spawn_job` runs the job on a `std::thread`, catching panics (`the job failed (internal error)`).
When the job ends, `finish_job` decides who has the slot next, in this order:

1. waiting saves (`pending_saves`) take it straight away, without clearing the busy state, so nobody sees the
   session idle in between and `settled` waits for the save too; its render, of the song as it is then, also serves
   as the fresh render;
2. otherwise the busy state is cleared and, if the render is stale (a render was asked for during the job, or
   auto-render is on and the song changed), a render starts 100 ms later;
3. otherwise the session is idle.

That is also the debounce: an edit starts a render job that first sleeps 200 ms; edits arriving during the sleep or
the render only mark the render stale, and the job's end starts one more render for all of them. A burst of edits
made while a render runs costs one more render, not one per edit, and the last render is always of the final song.

`save(path)` checks the folder against the file policy, writes `blueprint.json` and `song.rhai` at once (a write
error is the reply), and then either starts a render job for the folder or, when a job holds the slot, adds the
folder to `pending_saves` and answers `saving to <folder>: blueprint.json and song.rhai now, the audio once the
current job (rendering) is done`. Every save made during one job gets its folder, all from the same render. If the job
ended between the two steps, the save tries again to start, so it is never left waiting for a job that is gone.

`render_job`:

1. takes the blueprint and `song_version` under the lock, and arranges and renders outside it, with stems when they
   were asked for or the render is for a save;
2. judges the render, with the ears when the background loader has finished (the session loads CLAP only, not the
   AudioSet tagger);
3. writes the master to `<cache>/renders/<process id>/<version>.wav`, and stems to `<version>/<track>.wav` as 24-bit
   WAVs;
4. builds the `RenderView`: URL, length, loudness, section times, a 1000-point waveform, stem names;
5. computes each track's loudness in the master (its measured level plus its gain plus the master's gain from
   pre-master to final);
6. for a save, writes the whole song folder from this render with `song::write_folder`, the function the CLI uses,
   so the score in it is the song its audio plays;
7. publishes the render unless a newer one already landed, logs `rendered in 7.7 s: -10.1 LUFS, verdict 98.0 — ...`
   and, for a save, `saved <folder>` or `could not save <folder>: <reason>`, and deletes renders older than the last
   ten.

`mutate_job` and `evolve_job` run the searches described in [Evolution and analysis](evolution-and-analysis.md),
reporting progress through `busy`, `progress` and one log line per candidate, and commit the best result as one
undoable change. `cancel` sets a flag those two check between candidates and generations; renders do not check it.

### Files

`Settings::file_roots` is the [file policy](../guide/samples.md#what-a-shared-claudius-may-read-and-write): `None`
for a claudius only you can reach, `config::shared_roots()` plus the `--files` folders for one started with
`--public`, `--host` beyond loopback, or `--files`. `readable(path)` guards `load`, `analyze` and `remake` with `config::read_error`, which
scripts use for `.s(path)` too, so both refuse a path outside the roots with the same words whether or not it
exists. `config::write_allowed` guards `save`, and opening a blueprint checks every file it plays. [The security
model](security.md#files) discusses its limits.

### Starting up

`App::new` creates its render folder, deletes the render folders of sessions that last wrote more than 12 hours ago,
opens the Apple Loops library and the user library (unless `--synth-only` or `--no-library`), and, unless
`--no-ears`, loads earshot's models on a background thread, logging `ears ready (<seconds> s): verdicts now include
earshot's listening` or why not. `--synth-only` also turns the sample packs off for the whole process
(`claudius_packs::set_disabled`), before the session starts, so nothing resolves to a sampled instrument.
The session answers from the first moment; verdicts made before the ears are ready have none.

## HTTP

`http::router(app, options)` builds the axum router:

```text
request ─▶ CORS ─▶ security headers ─▶ guard ─▶ routes: /api/..., /mcp, the web studio or the no-UI page
```

- **CORS** is the outermost layer, so preflight requests are answered before the guard asks for a token. Allowed
  origins are `http://localhost`, `http://127.0.0.1` and `http://[::1]` on any port.
- **Security headers** go on every response, including the guard's refusals: `X-Content-Type-Options: nosniff`,
  `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY`, `Content-Security-Policy: frame-ancestors 'none'`.
- **The guard** refuses a `Host` that is not a loopback name unless the server is `--public`, then, with a token,
  accepts a matching `Authorization: Bearer`, `claudius_token` cookie or `?token=` (which also sets the cookie, with
  `Secure` when `X-Forwarded-Proto` says https), comparing in constant time.
- **Commands** must be `application/json` and at most 1 MiB, and run on `spawn_blocking`, since an edit can take
  seconds.
- **Audio and stems** are served with `tower-http`'s `ServeFile`, which handles `Range`; an MP3 is encoded with ffmpeg
  on first request, and the WAV is served in its place without ffmpeg.
- **The web studio** is served from its `dist` folder with `index.html` as the fallback for any other path; without
  one, a short page says how to build it.

`serve` binds the listener, records the address it got as the session's base URL, which the MCP tools put in front
of audio links, and only then calls back with the address, so `claudius serve` never prints an address it did not
get. `spawn` does the same on a thread with its own multi-threaded runtime, because `claudius studio` must give the
main thread to the window. Both turn a taken port into `port <port> is taken, by another claudius or another program
(--port picks another)`. The server takes its token from `Options`; for `claudius serve --public` without
`--token`, the CLI makes one first (`new_token`: 16 bytes from `getrandom`, in hex) and prints it with the address.

[HTTP API](../guide/http-api.md) documents the routes and types for clients.

## MCP

`mcp::Studio` is an `rmcp` server handler whose tools are methods marked `#[tool]`; their parameter structs derive
`JsonSchema`, so the schemas clients see come from the same types, including each field's doc comment. The handler's
`instructions` tell the assistant the typical session (look, edit with scripts, listen, undo what got worse, save) and
that a person is listening.

The tools run on a `Backend`:

- **`Local`**: the `App` in this process. Commands run on `spawn_blocking`; state and waiting call the `App` directly.
- **`Remote`**: a claudius in another process, reached through its HTTP API with `ureq`: `POST /api/command` (600 s
  timeout, for jobs that start slowly), `GET /api/state`, `GET /api/state?settled=1&timeout=N`, `/api/script` and
  `/api/sounds`, with the token as a bearer header.

`mcp::probe(base, token)` asks `/api/hello` with an 800 ms timeout and returns a `Probe`: `Claudius` for a success
whose body names `"claudius"`, `NeedsToken` for a `401` whose body is claudius' token refusal, `Nothing` for anything
else. `claudius mcp` picks `Remote` on `Claudius`, fails on `NeedsToken` (`the claudius at <url> asks for a token:
pass --token or set CLAUDIUS_TOKEN (a wrong one is refused too)`), and on `Nothing` fails when `--url` was given or
runs `Local` with a web server of its own. `claudius studio` uses the same probe to open the window on a running
claudius or to explain that it cannot send a token, and `claudius doctor` reports anything but `Nothing` as a
claudius running on the port. The HTTP endpoint at `/mcp` always uses `Local` on its server's `App`.

Two helpers shape every tool:

- `edit(command, listen)` sends the command; on success, with `listen`, it asks for a render (a no-op when the render
  is current) and waits up to 240 s for the session to settle, then returns the reply and `describe(state)`: the song
  summary, the verdict and the audio link.
- `job(command, timeout)` remembers the log, sends the command, waits for the session to settle (240 to 300 s), and
  returns the log lines written since, the candidate scores of a mutation or evolution, followed by the description,
  or a note that the job is still running.

The transports: `serve_stdio(backend)` for `claudius mcp`, where stdout carries the protocol and everything else goes
to stderr; and `http_service(app, public)`, rmcp's Streamable HTTP service with a local session manager, nested at
`/mcp`. rmcp's own allowed-hosts check is turned off for `--public` servers, since the outer guard has already decided.

## Views

`views.rs` turns the engine's documents into the wire types. Most of it is direct, with two places that encode
decisions:

- A part's `presets` (what the studio offers to switch to) are the genre's entries, every alternative of an `a|b`
  entry that exists here, then the synth presets suited to the role, then installed instruments suited to it
  (`instruments_for`: upright and electric bass for a bass, pianos, Rhodes, Wurlitzer and organs for chords, strings,
  choir and brass for pads, ...).
- `summary` and `verdict_text` write the song and the verdict as the plain text the MCP tools return. They are the
  assistant's eyes, so they list every section with its flags and layers, every track with its sound, mix and
  measured loudness, and up to eight issues with their targets.

## The wire contract

`claudius-api` holds every type that crosses HTTP: `State`, `SongView`, `RenderView`, `VerdictView`, `Command`,
`Reply` and the rest. It depends only on serde, so the web studio compiles it to WebAssembly. New fields get
`#[serde(default)]`, so an older studio still reads a newer server's state and the other way round, and `Command` is a
tagged enum (`{"cmd": "set_tempo", ...}`), so a client sending an unknown command gets a clear `400` instead of a
silent no-op.

`tests/session.rs` runs a session through new songs, renders, scripts, solo, undo and redo, the file policy (with
probes for files outside it), the HTTP guards, MCP over HTTP, and a save made during a render; it also pins
`claudius_api::{ACCENT, GHOST}`, the studio's accent and ghost marks, to the engine's step-string thresholds
`claudius::script::{STEP_ACCENT, STEP_GHOST}`. See [Testing](testing.md).
