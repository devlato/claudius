# HTTP API

`claudius studio`, `claudius serve` and a `claudius mcp` that runs its own session answer HTTP on
`http://127.0.0.1:7878` (`--port`, `CLAUDIUS_PORT`). The studio, in its window and in the browser, uses nothing but
this API, so anything it does you can do with `curl`. The types are plain serde structs in the `claudius-api` crate
(`crates/claudius-api/src/lib.rs`), which also compiles to WebAssembly for the web studio.

## Routes

| route | answer |
|---|---|
| `GET /api/hello` | `{"name":"claudius","version":"0.1.0","state":1,"ears":false}`: is this claudius, which version, the state version, whether earshot's models are loaded |
| `GET /api/state` | the [state](#the-state) at once |
| `GET /api/state?since=N` | the state as soon as its version is past `N` ([long polling](#versions-and-long-polling)) |
| `GET /api/state?settled=1` | the state once no job runs and the render is of the current song |
| `POST /api/command` | runs one [command](#commands) (JSON body) and answers with a [reply](#replies-and-errors) |
| `GET /api/script` | the song as a script, `text/plain` (`// no song yet: try song("house")` before the first song) |
| `GET /api/reference` | the script language reference, `text/markdown` |
| `GET /api/sounds` | every sound name: drum voices, synth presets, installed instruments, and every pack |
| `GET /api/audio/{version}.wav` | the master of that render, 48 kHz 24-bit |
| `GET /api/audio/{version}.mp3` | the same as MP3, encoded on the first request |
| `GET /api/stems/{version}/{track}.wav` | one track of a render made with stems |
| `/mcp` | the [MCP endpoint](mcp.md#mcp-over-http) |
| `GET /` and anything else | the web studio; when it is not built, a page that says how to build it |

`state`, `since`, `settled` and `timeout` combine as described below; `timeout` is in seconds, 20 by default and 600
at most.

## Versions and long polling

Two numbers tell a client what changed:

- `version` goes up on every change to anything in the state: a log line, the progress of a job, a new render, an
  edit.
- `song_version` is the `version` at which the song itself last changed. A render belongs to the song version it was
  made from, in `render.version`, so **the render is current when `render.version >= song_version`**.

A client keeps itself up to date with one request in flight:

```sh
curl -s http://127.0.0.1:7878/api/state                 # version 7
curl -s 'http://127.0.0.1:7878/api/state?since=7'       # returns when the version passes 7, or after 20 s
```

`since=N` answers as soon as the version is greater than `N`, and after `timeout` seconds otherwise, with whatever
the state is then. The client takes the `version` of each answer as its next `since`. A `since` ahead of the
server's version, as after a restart, waits out the timeout once and then answers with the current version.

`settled=1` waits for something else: that no job is running and that the render is of the current song. That is
the moment the verdict describes the song as it is now. When the session renders only on request
(`--no-auto-render`) and no render was asked for, it is settled at once. `settled` takes precedence over `since`.

```sh
curl -s -X POST http://127.0.0.1:7878/api/command -H 'Content-Type: application/json' \
  -d '{"cmd":"new_song","genre":"house","seed":9,"bpm":126}'
curl -s 'http://127.0.0.1:7878/api/state?settled=1&timeout=120'    # returned after 7.9 s, with the render and verdict
```

The MCP tools wait the same way after every edit.

## The state

`GET /api/state` returns a `State`. Here it is after the request above, with long lists cut short:

```json
{
  "version": 7, "song_version": 2, "busy": null, "progress": null, "ears_loading": false,
  "can_undo": true, "can_redo": false,
  "undo_label": "new song: Crimson Tide — house at 126 BPM in G dorian (seed 9)", "redo_label": null,
  "log": ["sample library: Apple Loops and your folders",
          "new song: Crimson Tide — house at 126 BPM in G dorian (seed 9)",
          "rendered in 7.7 s: -10.1 LUFS, verdict 98.0 — 98/100 (arc 0.93, mix 1.00, theory 1.00)"],
  "genres": [{ "name": "deep_house", "description": "Deep house: softer four-on-the-floor, ...", "bpm": [118.0, 124.0] }],
  "song": {
    "title": "Crimson Tide", "genre": "house", "seed": 9, "bpm": 126.0, "key": "G dorian", "swing": 0.5257842,
    "bars": 64, "seconds": 121.9047619047619,
    "progressions": [["A", ["i7", "ii7", "i7", "IV7"]], ["B", ["i7", "VIImaj7", "i7", "IV7"]]],
    "sections": [{ "name": "intro", "kind": "intro", "start_bar": 0, "bars": 8, "energy": 0.4,
                   "layers": ["kick", "hat", "perc", "top"], "prog": "A", "variant": "sparse",
                   "filter_in": true, "build": false, "gap": false, "impact": false, "fills": false }],
    "layers": ["kick", "clap", "hat", "ohat", "shaker", "perc", "bass", "chords", "pad", "lead", "arp"],
    "tracks": [{ "name": "kick", "kind": "drum", "role": "kick", "sound": "sampled bd (tr909:bd)",
                 "level_db": 0.0, "pan": 0.0, "reverb": 0.0, "delay": 0.0, "duck": 0.0,
                 "muted": false, "solo": false, "loudness": -13.2 }],
    "drums": [{ "lane": "kick", "pattern_bars": 4,
                "variants": [["main", [0.99353635, 0.0, 0.0, 0.0, 0.9903264, 0.0, 0.0, 0.0]]] }],
    "parts": [{ "name": "bass", "role": "bass", "style": "syncopated", "styles": ["offbeat", "syncopated", "offbeat"],
                "preset": "saw_bass", "presets": ["organ_bass", "pluck_bass", "saw_bass", "sub_bass", "808", "reese",
                "acid", "wobble", "upright_bass", "electric_bass", "synth_bass", "contrabass"],
                "register": [33, 52], "density": 0.42195085 }],
    "script": "// Crimson Tide — house (seed 9)\n// song(\"house\", 9);   // uncomment to start over from the generator\n...",
    "lineage": ["generated: house seed 9"]
  },
  "render": {
    "version": 2, "url": "/api/audio/2.mp3", "seconds": 123.58135416666667, "lufs": -10.091306995130768,
    "true_peak_db": -1.05, "bpm": 126.0,
    "sections": [["intro", 0.0, 15.238095238095237], ["groove", 15.238095238095237, 45.71428571428571]],
    "waveform": [0.8221747, 0.501973, 0.47014454, 0.8221747],
    "stems": []
  },
  "verdict": {
    "total": 97.96757459489606, "summary": "98/100 (arc 0.93, mix 1.00, theory 1.00)",
    "scores": [["arc", 0.9288651108213621], ["mix", 1.0], ["theory", 1.0]],
    "issues": [], "ears": null,
    "measures": [["arc_correlation", 0.86], ["band_air_db", -19.1], ["band_low_db", -3.2]]
  }
}
```

### `State`

| field | type | meaning |
|---|---|---|
| `version` | integer | goes up on every change |
| `song_version` | integer | the version of the last change to the song |
| `song` | `SongView` or null | the open song, null before the first one |
| `render` | `RenderView` or null | the latest finished render; it may be of an older song version while a new render runs |
| `verdict` | `VerdictView` or null | the verdict of that render |
| `busy` | string or null | the running job: `"rendering"`, `"listening"`, `"mutating: judging 4 variations"`, `"evolving: generation 2 of 3 done"`, `"saving"` |
| `progress` | number or null | 0 to 1, when the job knows it |
| `ears_loading` | boolean | earshot's models are still loading; until they are, verdicts come from the meters and music theory only |
| `can_undo`, `can_redo` | boolean | |
| `undo_label`, `redo_label` | string or null | what undo would take back, what redo would bring back |
| `log` | strings | the last 200 lines of the session log, oldest first |
| `genres` | `GenreInfo` list | every genre: `name`, `description`, `bpm` range |

### `SongView`

| field | meaning |
|---|---|
| `title`, `genre`, `seed`, `bpm`, `key`, `swing` | the song's settings; `key` reads like `"A minor"` |
| `bars`, `seconds` | its length |
| `progressions` | `[name, [numerals]]` pairs, `"A"` and `"B"` in generated songs |
| `sections` | `SectionView` per section: `name`, `kind`, `start_bar`, `bars`, `energy`, `layers`, `prog`, `variant` and the flags `filter_in`, `build`, `gap`, `impact`, `fills` |
| `layers` | every name a section can switch on: drum lanes, parts, loops, pattern tracks |
| `tracks` | `TrackView` per track: `name`, `kind` (`drum`, `part`, `loop`, `pattern`, `fx`), `role`, `sound` in words, the mix (`level_db`, `pan`, `reverb`, `delay`, `duck`, `muted`, `solo`) and `loudness` in the master (LUFS while playing) once rendered |
| `drums` | `DrumView` per lane: `lane`, `pattern_bars`, and `variants` as `[name, velocities]` with one velocity per sixteenth (0 for none) |
| `parts` | `PartView` per generated part: `name`, `role`, `style`, the `styles` its genre offers, `preset`, the `presets` it may switch to, `register` (MIDI low, high), `density` |
| `script` | the song as a script; running it on the song changes nothing |
| `lineage` | what was done to the song, oldest first |

The last entry of `tracks` is always `fx`, the transition effects (crashes, risers, impacts), whose mix scripts set
with `mix("fx", ...)`.

### `RenderView`

| field | meaning |
|---|---|
| `version` | the song version it was rendered from |
| `url` | `/api/audio/<version>.mp3`; the same path ending in `.wav` gives the 24-bit master |
| `seconds`, `lufs`, `true_peak_db`, `bpm` | the master's length and loudness |
| `sections` | `[name, start seconds, end seconds]`, for drawing the arrangement |
| `waveform` | about 1000 peak values from 0 to 1 |
| `stems` | track names with a stem at `/api/stems/<version>/<name>.wav` (empty unless rendered with stems) |

### `VerdictView`

| field | meaning |
|---|---|
| `total` | 0 to 100 |
| `summary` | one line: the total, the scores, the first three issues |
| `scores` | `[name, 0..1]` for `mix`, `arc`, `theory`, and with the ears `genre`, `production` (and `refs`) |
| `issues` | `[severity 0..1, what, where to fix it]`, worst first |
| `ears` | `EarsView` or null: `genre_query`, `genre_z`, `best_alt`, `best_alt_z`, `production_z`, `worst_flaw`, `worst_flaw_z`, `sounds_like`, `p_real` |
| `measures` | `[name, value]` meters: `lufs`, `true_peak_db`, `band_*_db`, `stereo_correlation`, `drop_minus_break_lu`, ... |

[The critic and the ears](../engineering/critic.md) explains every score, measure and issue target.

## Commands

`POST /api/command` takes one command as a JSON object tagged by `"cmd"`, with `Content-Type: application/json`. The
fields marked optional may be left out.

| `cmd` | fields | what it does |
|---|---|---|
| `new_song` | `genre`; optional `seed`, `bpm`, `key`, `loops` (false) | composes a new song |
| `load` | `path` | opens a song folder, a `blueprint.json`, or a script file (run on the open song) |
| `save` | optional `path` | writes `blueprint.json` and `song.rhai` now, then the whole folder from a render (see [Jobs](#jobs)); default folder `~/Music/claudius/<genre>-<title>-<seed>` |
| `set_tempo` | `bpm` | as `tempo()` |
| `set_key` | `key` | as `key()`: the progressions follow a change of mode |
| `set_swing` | `swing` | as `swing()` |
| `set_progression` | `name`, `chords` (strings) | as `prog()` |
| `set_section` | `index`; optional `name`, `bars`, `energy`, `layers`, `prog`, `variant`, `filter_in`, `build`, `gap`, `impact`, `fills` | changes any of a section's properties |
| `add_section` | `after`, `kind`, `bars` | inserts a section after index `after`, with that section's layers, energy and progression |
| `remove_section` | `index` | removes a section (a song keeps at least one) |
| `move_section` | `from`, `to` | moves a section |
| `toggle_layer` | `section`, `layer` | switches a track on or off in a section |
| `set_drum_step` | `lane`, `variant`, `step`, `velocity` | sets one sixteenth (0 removes the hit); a new variant starts as a copy of `main` |
| `set_drum_pattern` | `lane`, `variant`, `steps` | a whole variant in step notation, as `drum()` |
| `set_part` | `name`; optional `style`, `preset`, `register` (`[low, high]`), `density`, `reseed` (false) | as `part()`; `reseed` writes the part again with a new seed |
| `set_mix` | `track`; optional `level_db`, `pan`, `reverb`, `delay`, `duck`, `muted`, `solo` | as `mix()` |
| `run_script` | `code` | runs a script on the song |
| `analyze` | `path` | the analysis of an audio file, as text in the reply's `message` |
| `remake` | `path`; optional `genre`, `seed`, `bpm` | a new song on the file's skeleton; the genre defaults to the open song's, and the tempo to the recording's, halved or doubled into the genre's range when it is outside |
| `mutate` | optional `count` (4, 1 to 32), `keep_best` (true) | a job: tries variations and keeps the best if it beats the song |
| `evolve` | optional `generations` (3, 1 to 20), `children` (6, 2 to 32) | a job: an evolutionary search from the song |
| `install_packs` | optional `names` (the missing default packs when left out) | a job: downloads sample packs, as `claudius setup` does; not the opt-in packs, whose licence is unclear, and not on a shared claudius |
| `render` | optional `stems` (false) | a job: renders and judges the song |
| `undo`, `redo` | | |
| `cancel` | | stops a mutation or evolution after its current candidate or generation |

```json
{"cmd": "set_tempo", "bpm": 124}
{"cmd": "set_section", "index": 0, "energy": 0.5, "layers": ["kick", "hat"]}
{"cmd": "set_drum_step", "lane": "kick", "variant": "main", "step": 3, "velocity": 0.6}
{"cmd": "set_part", "name": "bass", "style": "rolling", "reseed": true}
{"cmd": "run_script", "code": "tempo(126); mix(\"lead\", #{ level: -2 });"}
```

`set_tempo`, `set_key`, `set_swing`, `set_progression`, `set_drum_pattern`, `set_part` and `set_mix` are turned into
the matching script call and run as a script, so they accept exactly what the script functions accept and fail with
the same messages. The other edits change the blueprint directly. Every edit that changes the song is one undo step
with a label, and starts a render after 200 ms unless the session runs with `--no-auto-render`.

### Jobs

`render`, `mutate`, `evolve` and `save` are jobs. A job runs on its own thread, one at a time, and the command
answers as soon as it has started. Follow it in `busy`, `progress` and `log`, or wait with `settled=1`. A `mutate`
or `evolve` while a job runs is refused, and the refusal leaves the version as it was; `render` and `save` wait for
the job in progress instead:

```json
{"ok": true, "message": "rendering after the job in progress", "version": 4}
{"ok": false, "message": "busy (rendering); try again when it is done, or cancel it", "version": 4}
```

A `save` writes `blueprint.json` and `song.rhai` at once. Asked for while the session is idle, it starts its own
render; asked for during a job, such as the render that follows an edit, it waits, and the job slot passes straight
to it when that job ends, so the session never looks idle in between and `settled=1` waits for the save too:

```json
{"ok": true, "message": "saving to /tmp/cl-save2 (blueprint.json and song.rhai now, the audio after the render)", "version": 14}
{"ok": true, "message": "saving to /tmp/cl-save2: blueprint.json and song.rhai now, the audio once the current job (rendering) is done", "version": 4}
```

Either way the save renders the song as it is when its turn comes, with stems, and rewrites the whole folder from
that render, so the score in it is always the song its audio plays: `song.wav`, `song.mp3`, `blueprint.json`,
`song.rhai`, `project.json`, `report.json`, `verdict.json`, `credits.txt` when sample packs play, and `stems/` as
32-bit float. The log then says `saved <folder>`, or `could not save <folder>: <reason>`.

`cancel` does not stop a render that has started.

## Replies and errors

Every command answers with a `Reply`:

| field | meaning |
|---|---|
| `ok` | whether the command did what it was asked |
| `message` | what happened, or why not |
| `version` | the state version after the command |

```json
{"ok": true, "message": "new song: Crimson Tide — house at 124 BPM in G dorian (seed 9)", "version": 2}
{"ok": false, "message": "no progression 'Z' (there are A, B)", "version": 11}
{"ok": false, "message": "Runtime error: expected ']', found the end (line 1, position 12)", "version": 11}
```

A command claudius understood but refused (a bad edit, a script error, a busy session) answers `200 OK` with
`"ok": false`, and changes nothing. The HTTP status says when the request itself is wrong:

| status | when | body |
|---|---|---|
| `400 Bad Request` | the JSON is not a command | a `Reply` whose message starts with `not a command:` and names the unknown `cmd` or the missing field |
| `401 Unauthorized` | a token is required and missing or wrong | `this claudius needs its token (Authorization: Bearer <token>, or ?token=<token>)` |
| `403 Forbidden` | the `Host` header is not a loopback name and the server is not `--public` | `claudius only answers on localhost (start it with --public and a --token to share it)` |
| `404 Not Found` | no such render or stem | `no render of version 999`, `no stem 'kick' in render 13 (render with stems)`, `audio is /api/audio/{version}.wav or .mp3` |
| `413 Payload Too Large` | a command body over 1 MiB | `Failed to buffer the request body: length limit exceeded` |
| `415 Unsupported Media Type` | a command without `Content-Type: application/json` | `commands are JSON (Content-Type: application/json)` |

## Audio

`/api/audio/{version}.wav` and `.mp3` serve the render of song version `version`, with `Range` requests, so players
can seek:

```text
$ curl -sI -H 'Range: bytes=0-99' http://127.0.0.1:7878/api/audio/13.wav
HTTP/1.1 206 Partial Content
content-type: audio/wav
accept-ranges: bytes
content-range: bytes 0-99/72955106
content-length: 100
```

The MP3 is encoded on the first request (a second or two for a song) and kept. A session keeps its last ten renders
on disk, so older versions
return 404. Stems exist only for a render made with `{"cmd": "render", "stems": true}` or by `save`, and only the
latest render's stems are served, as 24-bit WAV; the copies a `save` writes into the song folder are 32-bit float.

## Who may call it

The server is written for one person on one computer, and opens up only when asked. The
[security model](../engineering/security.md) has the reasoning; these are the rules as a client sees them.

**Host names.** Without `--public`, every request must name a loopback host in its `Host` header: `localhost`,
`127.0.0.1` or `[::1]`, with any port. Anything else gets 403, which stops a web page from reaching claudius through
DNS rebinding. It is not access control: any program on the machine can send `Host: localhost`.

**Tokens.** With `--token <secret>` (or `CLAUDIUS_TOKEN`), and always with `--public`, which makes a token when it is
given none, every request must carry the token, in one of three ways:

- `Authorization: Bearer <secret>`;
- `?token=<secret>` in the URL, which also answers with
  `Set-Cookie: claudius_token=<secret>; Path=/; HttpOnly; SameSite=Strict`, so a browser opened at
  `/?token=<secret>` keeps working without it. When the request came through https (`X-Forwarded-Proto: https`, which
  tunnels send), the cookie is also `Secure`;
- that cookie.

The comparison takes the same time wherever the strings differ. CORS preflight requests (`OPTIONS`) are answered
before the token check, so a browser can ask what is allowed.

**Commands are JSON.** `/api/command` refuses any other content type, and the MCP endpoint does the same, so a page
on another site cannot send a command with a plain form post or a `no-cors` fetch.

**CORS.** Cross-origin requests are allowed from pages served by `http://localhost`, `http://127.0.0.1` and
`http://[::1]` on any port, for `GET`, `POST` and `DELETE`, with the headers `Content-Type`, `Authorization`,
`mcp-session-id` and `mcp-protocol-version`. Pages from anywhere else, including `https://localhost`, get no
`Access-Control-Allow-Origin`, so the browser does not let them read the answer.

**Security headers.** Every response carries:

```text
x-content-type-options: nosniff
referrer-policy: no-referrer
x-frame-options: DENY
content-security-policy: frame-ancestors 'none'
```

so a browser does not guess content types, leak the address in a `Referer`, or let another page frame the studio.
