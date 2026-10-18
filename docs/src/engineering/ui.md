# The studio UI

`crates/claudius-ui` is the studio: one egui application that runs as a native window (`claudius studio`, or the
standalone `claudius-ui` program) and as a WebAssembly app in the browser, served by `claudius serve` at `/`. It is a
client of the HTTP API and nothing else: it depends on `claudius-api` for the wire types, never on the engine.
[0008 egui for native and web](../design/0008-egui.md) records why. [The studio](../getting-started/studio.md) shows it
from the user's side.

The studio is still being finished; this page describes its structure, which is settled.

| file | what it does |
|---|---|
| `lib.rs`, `main.rs` | `run_native`, `start_web`, the `claudius-ui` program's arguments |
| `app.rs` | `Studio`, the `eframe::App`: state, layout, shortcuts, applying server state |
| `net.rs` | the connection: long polling, commands, the backend (a server or the mock) |
| `edits.rs` | edits applied to a `SongView`, for optimistic updates and for the mock |
| `cmds.rs` | builders for commands that change a few fields |
| `panels/` | one module per panel |
| `audio/` | the player: rodio natively, an `HtmlAudioElement` on the web |
| `mock/` | a built-in stand-in for the server, with a generated song and a rough preview synth |
| `music.rs` | the theory needed to label things: keys, numerals as chord names, note names |
| `theme.rs`, `widgets.rs` | the palette (one colour per role and section kind), knobs, faders, meters, chips |
| `devplan.rs` | scripted runs of the native window, for checking the UI without a person |

## Layout

```text
┌───────────────────────────────────────────────────────────────────────────────┐
│ transport: play, position, tempo / key / swing, score                          │
│ toolbar: new song, render, mutate, evolve, undo / redo, save, cancel, busy     │
├───────────────────────────────────────────────────────┬───────────────────────┤
│ arrangement: section blocks, one row per layer with    │ inspector: the        │
│ a cell per section, the waveform and playhead          │ selected section      │
├───────────────────────────────────────────────────────┤                       │
│ dock: step sequencer │ mixer │ parts │ harmony │ script │ verdict: scores,     │
│       │ sounds │ log                                   │ issues, ears, bands   │
├───────────────────────────────────────────────────────┴───────────────────────┤
│ status: connection, server, state version, audio errors, volume, shortcuts     │
└───────────────────────────────────────────────────────────────────────────────┘
```

| panel | what it edits | commands it sends |
|---|---|---|
| arrangement | section order; which layers play in which section (click or drag across cells); seeking | `move_section`, `toggle_layer` |
| inspector | the selected section's name, bars, energy, variant and flags; adding, duplicating, removing | `set_section`, `add_section`, `remove_section`, `move_section` |
| step sequencer | one row per drum lane, one cell per sixteenth, one variant at a time; click toggles (velocity 0.8), drag paints, shift sets velocity, right-click offers presets; an accent dot and the "(accent)" and "(ghost)" labels follow `claudius_api::{ACCENT, GHOST}`, the thresholds of the engine's step strings | `set_drum_step`, `set_drum_pattern` |
| mixer | a strip per track: pan, sends, duck, fader, mute, solo, with its loudness from the latest render | `set_mix` |
| parts | style, sound, register (drag on a keyboard), density, reseed | `set_part` |
| harmony | which progression each section plays; progressions as chord chips, from the key's diatonic chords or as text | `set_section`, `set_progression` |
| script | the song as a script, highlighted, Run with Cmd or Ctrl+Enter, errors marking their line, sounds to insert, the reference | `run_script` |
| sounds | the packs and every sound name; click one to put it on a part | `set_part` |
| verdict | read only: the total, a bar per score, issues with their targets, the ears, the tonal balance | |
| log | read only: `State.log`, filterable | |

The shortcuts are Space (play or pause), the arrow keys (pick a section), L (loop the section), Home (to the start),
Cmd or Ctrl+Z (undo) and Shift+Cmd or Ctrl+Z (redo), active when no text field has focus.

## State, commands and optimism

`Studio` keeps three copies of the server's state:

- `state`, what is on screen;
- `server`, the last state the server sent, without local edits, to fall back on when an edit is refused;
- `pending`, the newest state from the server, held back while something might be overwritten by it.

Every frame, the panels draw from `state` and push `Command`s into `out`. `flush` sends them, and for the edits it
can predict (tempo, key, swing, progressions, sections, layers, drum steps and patterns, parts, mix) it first applies
them to `state` itself with `edits::apply`, the same logic the mock uses, so a click in the sequencer shows at once
instead of after a round trip and a render. When the reply comes back refused, the song falls back to `server`.

A new state from the long poll goes into `pending` and is applied only when it is safe: not while the user drags
something (a fader, a section, a painted row of steps), and not while an edit is in flight or the state is older than
a reply already received (`Reply.version`), for up to 3 s. Without this, a long poll answering between a click and
its reply would flash the old value back.

The script editor follows the same idea: while its text has unsaved edits, new states do not replace it; it follows
the song's script again once a run succeeds.

## The connection

`net::Remote` runs one long poll at a time: `GET /api/state` first, then `?since=<last version>`, with a 45 s client
timeout against the server's 20 s. Each answer schedules the next. On failure it backs off 1, 2, 3, then 5 s, shows
`Cannot reach the claudius server at ...` with how to start one (`claudius serve`, or `cargo run --release -p
claudius-cli -- serve` in a checkout), Retry and Use the demo, and forgets its version and sound list, since a
restarted server starts counting again. After the first state it asks once for `/api/sounds` and `/api/hello` (and for
`/api/hello` again when a state says the ears have finished loading), and for `/api/reference` when the reference
window opens.

`ehttp` runs requests on threads natively and with `fetch` in the browser, delivering results through a channel and
asking egui to repaint. Natively the server's address comes from the argument, `CLAUDIUS_SERVER` or
`http://127.0.0.1:7878`; on the web it is the page's own origin, or `?server=` in the URL, or the mock with `?mock`.

The native window has no way to send a token. `claudius studio` runs its own server without one; to use a
token-protected claudius, open it in a browser with `?token=`, which sets the cookie every later request carries.

## Playback

When a new render arrives (`render.version` changed), the player loads it:

- **Native** (`audio/native.rs`): fetches the 24-bit WAV (`/api/audio/<version>.wav`), decodes it with hound, and
  plays it through a rodio source that reads shared atomics for play, seek, loop region, position and volume, so the
  UI thread never blocks the audio thread. Play and pause ramp the gain over about 5 ms so they never click. The
  output device is opened once, when the window starts; if that fails the status bar shows `audio: no audio output:
  ...`.
- **Web** (`audio/web.rs`): an `HtmlAudioElement` plays the MP3, and `currentTime` is the playhead.

A new render of the same song keeps the playback position (and the loop region, which the studio sets
separately), so you can listen to a section while you edit it and hear each change land in place.

## The mock

`mock/` plays the server's part without one: a generated song per genre with a plausible form, patterns, parts and
mix; edits through `edits.rs`; undo and redo with labels; jobs simulated with progress and log lines; a critic that
reacts to edits the way the real verdict reads; and a rough preview render, synthesized a slice per frame (no threads,
so it runs in the browser). It is how the studio is developed and tested without an engine, and what Use the demo
switches to. It is not claudius' sound.

```sh
cargo run -p claudius-ui -- --mock          # the native studio on the demo song
CLAUDIUS_MOCK=1 cargo run -p claudius-ui
```

On the web, `?mock` in the URL does the same.

## Scripted runs

`CLAUDIUS_UI_PLAN` drives the native window from a list of steps separated by `;`, so a change to the UI can be
checked, screenshotted and repeated without a person at the screen:

```sh
CLAUDIUS_UI_PLAN='wait:4;shot:/tmp/a.png;tab:mixer;play;wait:2;shot:/tmp/b.png;exit' cargo run -p claudius-ui -- --mock
```

The steps: `wait:<s>`, `tab:<name>`, `section:<i>`, `play`, `pause`, `seek:<s>`, `newsong`, `close`,
`cmd:<command JSON>`, `size:<w>,<h>`, `script:<code>` (into the editor, `\n` for a newline and `%3B` for `;`), `run`,
`reference`, `audiopath:<file>`, `shot:<file.png>` (the window as rendered), `exit`, and synthetic pointer input in
points: `move:x,y`, `click:x,y`, `rclick:x,y`, `drag:x1,y1,x2,y2`, `sdrag:x1,y1,x2,y2` (with shift).

## Building

Natively, the studio is part of the `claudius` binary (the `studio` feature, on by default) and of `claudius-ui`. For
the web, trunk builds it into `crates/claudius-ui/dist`:

```sh
cargo xtask web                       # trunk build --release, with relative asset URLs
```

`Trunk.toml` builds `index.html` with relative URLs (`public_url = "./"`), so the same `dist` works at `/` of any
claudius and from a static file server, and hashes file names so a browser never runs a stale build. CI runs clippy
for `wasm32-unknown-unknown` separately, because the native clippy run never sees the code behind
`cfg(target_arch = "wasm32")`. Preferences (the open tab, volume, mutate and evolve settings) persist through eframe:
natively in `claudius-studio/app.ron` under the platform's data folder, on the web in local storage.
