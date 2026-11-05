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
| `edits.rs` | edits applied to a `SongView`, for optimistic updates |
| `cmds.rs` | builders for commands that change a few fields |
| `panels/` | one module per panel |
| `audio/` | the player: rodio natively, an `HtmlAudioElement` on the web; the take [recorder](#recording): cpal and midir natively, Web Audio in the browser |
| `mock/` | the built-in demo: the claudius engine in a Web Worker (in the app, on a thread of its own) ([below](#the-demo)) |
| `music.rs` | the theory needed to label things: keys, numerals as chord names, note names |
| `theme.rs`, `widgets.rs` | the palette (one colour per role and section kind), knobs, faders, meters, chips |
| `mark.rs` | the logo's C, drawn at the size the header shows it, from the numbers `scripts/icons.py` draws the icons with |
| `pick.rs` | [choosing a file](#choosing-files) on the studio's computer and reading it into memory |
| `devplan.rs` | scripted runs of the studio, for checking the UI without a person: natively, and in a browser in a build with the `plan` feature |
| `native.rs` | `Native`, what the native window may do on its computer beyond the server (connect assistants) |
| `tour.rs` | the first-run tour: its cards, the parts of the studio they point at, and when it is done |

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
│       │ sounds │ graph │ log                           │ issues, ears, bands   │
├───────────────────────────────────────────────────────┴───────────────────────┤
│ status: connection, server, state version, audio errors, volume, shortcuts     │
└───────────────────────────────────────────────────────────────────────────────┘
```

| panel | what it edits | commands it sends |
|---|---|---|
| arrangement | section order; which layers play in which section (click or drag across cells); seeking; **+ Track**: a part of a role or a drum lane, named, its sound from `SoundList` (the role's synths first by `synth_roles`, the instruments, or a lane's drum voices and `kit`); a row's menu: mute, solo, on or off everywhere, removing the track, and a take's: how it is sung (shown in a window until closed), tuning it, its melody as notes | `move_section`, `toggle_layer`, `add_track`, `remove_track`, `assess_voice`, `tune`, `transcribe_take` |
| inspector | the selected section's name, bars, energy, variant and flags; adding, duplicating, removing | `set_section`, `add_section`, `remove_section`, `move_section` |
| step sequencer | one row per drum lane, one cell per sixteenth, one variant at a time; click toggles (velocity 0.8), drag paints, shift sets velocity, right-click offers presets; an accent dot and the "(accent)" and "(ghost)" labels follow `claudius_api::{ACCENT, GHOST}`, the thresholds of the engine's step strings | `set_drum_step`, `set_drum_pattern` |
| record | a take recorded over the song from a count-in, played back, kept; latency measured; a take an assistant asks for; a pitch check: its phrases synthesised from `State.pitch_check` and played as the song a take records over, the answers sent to `/api/pitch-check` when the phrases end | `add_notes`, `cancel_take_request`, `request_pitch_check`, `cancel_pitch_check` |
| mixer | a strip per track: pan, sends, duck, drive, fader, mute, solo, with its loudness from the latest render | `set_mix` |
| parts | style, sound, register (drag on a keyboard), density, reseed | `set_part` |
| piano roll | a part's cycle over a progression from `PartView::cycles`, on a grid of beats and pitches: click adds a note at the grid (1/16 to 1/2), a drag moves a note or, by its right edge, makes it longer, right-click takes it out; a drag edits a draft sent when let go, and a cycle the theory engine writes is written out by the first edit. The demo writes its cycles with the same code it plays them with (`mock::synth::write_cycles`) and plays a cycle written out | `set_notes` |
| harmony | which progression each section plays; progressions as chord chips, from the key's diatonic chords or as text | `set_section`, `set_progression` |
| script | the song as a script, highlighted, Run with Cmd or Ctrl+Enter, errors marking their line, sounds to insert, the reference | `run_script` |
| sounds | the packs and every sound name; click one to put it on a part | `set_part` |
| graph | the song's signal as nodes and edges (`SongView::graph`, laid out in columns from the tracks to the finished song) in an `egui::Scene` to pan and zoom; the inspector beside it edits the node chosen: a track's mix, bus, sections and sound, an effect's settings (each JSON field by its type, the rest as JSON text), a chain's effects added from `EFFECT_TEMPLATES`, moved or taken out, the master's loudness | `set_mix`, `toggle_layer`, `set_part`, `set_fx`, `run_script` (the master's loudness) |
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
`Cannot reach claudius at ...` with how to start one (`claudius serve`, or `cargo run --release -p claudius-cli --
serve` in a checkout), Retry and Use the demo, and forgets its version and sound list, since a
restarted server starts counting again. After the first state it asks once for `/api/sounds` and `/api/hello` (and for
`/api/hello` again when a state says the ears have finished loading), and for `/api/reference` when the reference
window opens.

`ehttp` runs requests on threads natively and with `fetch` in the browser, delivering results through a channel and
asking egui to repaint. Natively the server's address comes from the argument, `CLAUDIUS_SERVER` or
`http://127.0.0.1:7878`; on the web it is the page's own origin, or `?server=` in the URL, or the mock with `?mock`.

The native window has no way to send a token. `claudius studio` runs its own server without one; to use a
token-protected claudius, open it in a browser with `?token=`, which sets the cookie every later request carries.

What the window does on its own computer goes through `Native`, which `claudius studio` passes to `run_native`
(`connect::FromTheStudio` in `claudius-cli`): reading the assistants' configurations and connecting one, on a thread of
its own so that a slow `claude mcp add` does not stop the window, keeping the answer about usage statistics that the
tour's last card asks for ([privacy](../guide/privacy.md)), and the updater (`update.rs`), whose state the studio
reads each frame and repaints for while it downloads ([updates](../guide/updates.md)). None of it goes over HTTP, so neither a page in a
browser nor anything else that reaches the server can change those configurations; the web studio, which has no
`Native`, lists the `claudius connect` commands instead.

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

## Recording

The Record window (`panels/record.rs`) records a [take](../guide/samples.md#takes), holds it while the person plays it
back (`take::preview`, the song with the take in it, through a player of the window's own) or records it again, and
sends the one they keep to `POST /api/takes`, in the app and in a browser alike. The demo has none: a take goes to a
claudius, which keeps it with the song.

A take an assistant asks for (`request_take`) is the state's `take_request`. A new one that waits opens the window
set up for it (`Record::asked`), and the take kept for it, or the notes played, carry its id (`request` in the take's
query, in `add_notes`), so that claudius marks it taken with its answer, which `wait_for_take` returns to the
assistant. Not now sends `cancel_take_request`. A web studio opened at `?record=<id>`, the link the assistant gives,
opens on the Record window.

What a take is lives in `audio/take.rs`, for both:

- **What it is.** What to record and what plays meanwhile (`Plan`), and the take (`Take`).
- **What plays and what is kept.** What the output plays (`fill`), and the take cut where it starts (`cut`), as a
  float WAV (`wav`).
- **What went wrong.** How often the input fell silent in it (`hushed`).

`audio/record.rs` records in the app; `web_record.rs` takes its name in the browser, with the same API.

`audio/record.rs` opens two cpal streams of its own. The player is paused meanwhile.

- **The output** plays the latest render from the count-in on, resampled to the device's rate, with a click on every
  beat of the count-in, and of the take too when asked (`fill`, a plain function the tests read).
- **The input** sends the chosen channels to the window in chunks, and its loudest sample to the meter.

While the window is open and nothing records, a `Listener` keeps the input open for the meter alone, and closes it
when a take starts, since some systems give a device to one stream at a time.

Each stream's timestamps give its own latency: playback minus callback says when what the output plays is heard,
callback minus capture when what the input hears was captured. A host's streams need not share a clock's origin, so
the two are related through the system clock read in each callback. Of those estimates the earliest wins (a callback
runs late, never early), after the first four, while the streams settle. The take is cut from where the section's
downbeat was heard, plus the window's **Latency** for what the devices do not report (`cut`). It goes as a 32-bit
float WAV at the input's rate.

What the devices do not report can be large. On macOS cpal counts a device's buffer, latency and safety offset, but not
its streams' latency (`kAudioStreamPropertyLatency`), where the built-in speakers and microphone report their
processing: on a MacBook Pro, 14.5 ms and 50 ms. A take recorded through them landed 44 ms late; the reported figures
would have made it 20 ms early. Since only a measurement gets it right, **Measure** measures:

- **The clicks.** `Plan::measure` plays eight clicks (`audio/latency.rs`, a few cycles at 3 kHz) unevenly apart, with
  no count-in and no offset, and `Recorder::lateness` cuts what the input heard exactly as a take is cut.
- **Finding them.** `lateness` high-passes the recording at 1 kHz. The lag from -100 to +600 ms where at least five
  clicks line up with something loud, scored by the quietest of the five, gives where to look; the first arrival
  near each click (the sound straight from the speaker, not a louder echo) gives its lateness.
- **Being sure.** At least five clicks must stand four times above the room's noise and land within 1.5 ms of their
  middle, and the answer is the middle one's. Otherwise the window says why.
- **The result.** The window sets **Latency** to what it found, and says through which output and input. A MIDI take
  has a Latency of its own: a loop through the microphone also counts the input's latency, which a key pressed does
  not have.

The tests play the clicks through a simulated speaker and room: band limiting, echoes louder than the direct sound, a
reverberant tail, noise, hum, clipping, lost clicks and a bang. Through a MacBook Pro's speakers and microphone it
found 45 ms twice, and a take recorded then landed within half a millisecond of the song.

A stream's dropped buffer (`Xrun`), or a route that changed under it, does not end a take: the window says how often
the take dropped audio. Any other stream error ends it.

A MIDI input (midir) records in place of an audio input:

- **Timing.** Each note on and off is stamped with the system clock as it arrives, so it lines up with the song by the
  same estimate of when the output's first frame was heard.
- **The guide tone.** The messages also go to the output stream, where a small guide (`Guide`, a sine and a little of
  its octave, up to sixteen voices) plays them for a keyboard that has no sound of its own.
- **The notes.** At the end, `pair` matches each note on to its off. A note on a pitch already sounding ends the one
  before, and a note still held at the stop ends there.
- **Sending them.** The window puts the notes on the chosen grid in beats and sends them as `add_notes`, which makes
  a part of them.

A take that fell silent to the bit for 50 ms or more between sounds (`hushed`) says so with claudius' answer. A
microphone or a line in always hears some noise, so that is the input muted or gated, as the processing a system does
to a microphone for calls does. Frames a browser lost, which `Captured` fills with silence, are dropouts, not that. A
measurement that fails on such an input says the same.

### In the browser

`audio/web_record.rs` records with Web Audio, on one clock:

- **The song.** It is the render's WAV, which the window fetches for the take (`record::song`). `render_mix` fills it
  in a buffer at the audio context's rate, with the count-in and its click, as `fill` does natively. An
  `AudioBufferSourceNode` starts it at a set time on the context's clock, a quarter of a second after the input
  listens.
- **The input.** `getUserMedia` asks for the chosen input with the browser's echo cancelling, noise suppression and
  gain control off. It goes to an AudioWorklet (`record-worklet.js`, which trunk copies beside the page). The worklet
  hands the page about 43 ms at a time, each chunk stamped with the context frame its first sample reached the
  worklet in. `Captured` puts the chunks together, with silence for frames that went missing, each a dropout.
  Told to stop, the worklet hands over what it still holds and says it is done; the window waits for that, a second
  at most, before it cuts the take.
- **Where the take starts.** It is arithmetic (`take_starts`): when its downbeat was to play, plus what the browser
  reports of the output's latency (`baseLatency`, `outputLatency`) and the input's (the track's `latency`), plus the
  window's Latency.
- **A MIDI keyboard.** Web MIDI (`requestMIDIAccess`) gives the keyboards once the page may use them. Where the
  browser says it was allowed before (`permissions.query`), the window lists them without asking; otherwise it offers
  to ask. A message is stamped where it arrived (`timeStamp`, on the page's clock), however late the page handles
  it; Chromium stamps a real keyboard's 0, so then `key_time` takes when the page heard it. `heard_at_stamp` puts it
  where the song was heard then, from the output's reading (`getOutputTimestamp`: the context time the output was
  playing, and when). `pair` makes the notes, as natively. The guide tone is an oscillator a key (`Voices`: a key
  pressed again starts over, sixteen at most). A CoreMIDI keyboard playing every half second, recorded in Chromium
  on a MacBook Pro, came out a beat apart to within 12 ms.

Browsers report little of their latency. Chromium on a MacBook Pro reported 8.5 ms (5.8 ms of `baseLatency`, an
`outputLatency` of 0, and 2.7 ms on the input), and takes landed 60 ms late. **Measure** works as natively, through
`Recorder::lateness`.

On that Mac, Chromium's capture of the built-in microphone was processed even with all of its own processing off. A
tone from the speakers, whether the page played it or another app did, came in 30 dB down, with runs of silence up to
two thirds of a second. The app's capture of the same microphone had none. A take there has holes, and Measure cannot
hear its clicks. What did it was not pinned down; macOS's per-app Mic Mode on Voice Isolation does the same, so the
window says the input went silent and points there (`hushed`).

The unit tests run the browser's path natively, in 128-frame quanta: a song scheduled on the context's clock, heard,
recorded with a quantum lost, and cut on its downbeat to the frame; a measurement through it that finds an
unreported 0.18 s; and keys pressed in time with what is heard, which come out on the beat. `site/tests/record.spec.mjs` records in Chromium with a file for a microphone, against a claudius
of its own. It drives a studio built with the `plan` feature (`cargo xtask web --plan`), which runs the scripted plan
in its page's `?plan=` and prints what the Record window says to the console. The tests check that:

- a take kept arrives, plays in the song and holds the microphone's tone;
- a take an assistant asks for opens the window, plays back, and goes to the assistant; or is turned down;
- Measure with an input that cannot hear the clicks says so;
- a keyboard the test plays itself (a Web MIDI of its own, before the page's scripts) makes a part, a note a beat;
- a browser without Web MIDI, a page blocked from it, and one blocked from the microphone say what to do.

## Choosing files

Where the studio wants a file (a song to open, a recording to listen to or remake, a release's cover), the person
chooses one on the studio's own computer, and the studio reads it into memory (`pick.rs`). Natively that is the
system's dialog (rfd); in a browser, a file input of the page's own (hidden, `data-claudius-pick` naming what it is
for), which waits in the page until a file is chosen. What was chosen arrives through `pick::chosen`, which the app
reads each frame:

- **A song** goes as its text in `open_text`, which opens a blueprint.json (by the file's name) or runs a script; a
  file that is not text is no song, and a toast says so.
- **A recording** waits in the New song dialog for **Analyze** or **Remake**, which send it, the file itself, to
  `POST /api/analyze` or `POST /api/remake`, named so that its extension says its format. claudius decodes it in
  memory and keeps nothing of it.
- **A cover** goes to `POST /api/release/artwork`, as a dropped one does.

So nothing the studio asks for is a path on the computer claudius runs on, and a studio on another computer works
the same. Scripted runs open the dialogs (`pick:<song|audio|cover>`) and press the buttons (`listen:<analyze|remake>`);
`site/tests/files.spec.mjs` fills the page's file input as a person choosing a file would.

## The demo

`mock/` is the built-in demo: the claudius engine itself, which makes, edits and renders the studio's songs as the
server does, kept in memory. In a page it runs in a Web Worker, `claudius-engine` (`crates/claudius-web`), which trunk
builds beside the studio; in the app, on a thread of its own. Both run `claudius_session::host::Host`: a new song from
the composer, the API's edits (`claudius_session::edit`, the server's own), undo and redo, and a render after every
change, judged with the meters and the theory (the ears do not run in a browser). A render takes about an eighth of
the time the song plays on the one thread a page has (a 4:48 house song in 36 s), so a new song first plays its first
16 bars, a few seconds after it is made, while the rest renders: its render's `url` is `PREVIEW_URL` meanwhile, which
the studio treats as older than the song. An edit to a song heard whole is heard where the person listens instead:
the studio sends where its playhead is with each command (`Link::playhead`), and the engine hands over the last render
with the twelve seconds from there rendered again with the edit (`Host::window`, `render::render_window`), within a
second, at the loudness of the render before, while the whole song renders again; a song an edit made longer or shorter
gets its first bars. The player swaps either in where it was playing.

The page and the worker trade messages of JSON and audio (`mock/engine.rs`): a command at a time goes out, and its
reply and the state after it come back; a render comes with its master, handed over rather than copied. A message
sent before the worker has loaded its WebAssembly would be lost, so the page holds its messages until the worker
first speaks.

Where the browser has cores to spare, the engine makes a render's tracks side by side, as the app does on threads
(`crates/claudius-web/src/crew.rs`):
- It starts up to three copies of itself (`claudius-engine_loader.js?crew`), as many as there are cores beyond the
  page's and its own.
- It shares out the tracks the render would make anew (`Host::to_make`), the costliest first to the copy with the
  least to make, with the audio they play. Each copy speaks first, as the engine does, before it is handed its share.
  What a track costs is measured, not guessed (`render::track_cost`): half a second of a note of its patch at the
  octave it plays around, and half a second of noise through its effects, each once.
- The costliest track, when it is more than an even share of the work and a synth that plays its notes apart, is made
  in parts, one a copy (`render::part_bounds`, `track_part`), unless one part would still hold most of its work (as a
  chord held from one beat would, every part that it sounds in rendering it whole). Each copy renders the notes that sound in its stretch of
  the song and adds them up as a render does, so its part is the track there to the sample; the engine puts the parts
  together and runs the track's effects (`render::track_joined`).
- It keeps each track a copy posts back (`render::track_sound`, `Host::made`). The render that waited for them then
  only mixes, and comes out as it would have alone. A copy that has made its share stops, which gives back its memory.

A crew that fails leaves the engine to make the tracks itself, as it does on a phone, whose browser holds a page's
workers in the memory it gives the page, and when started as `claudius-engine_loader.js?alone`. With a crew of three,
a four-minute deep-house song renders whole in about 19 s where the engine alone takes 29, synthwave in 20 where it
takes 30, and trap in 12 where it takes 16, each the same to the sample.

What needs a computer of one's own the demo turns down, and says so: takes, folders, files written (the download
button hands over the render as a WAV file), packs, mutation and evolution. A recording to listen to gets an analysis
made up from its name, since the demo hears nothing, and a remake of one is a new song of the genre asked for. Scripts,
opened songs and every edit are the engine's, as in the studio.

```sh
cargo run -p claudius-ui -- --mock          # the native studio on the demo song
CLAUDIUS_MOCK=1 cargo run -p claudius-ui
```

On the web, `?mock` in the URL does the same.

### A phone with the demo

A phone plays along with the demo through the site's relay (`site/worker/room.js`): the demo's **Play from a phone**
joins a room, named with 128 random bits kept in the tab's `sessionStorage` (a reload keeps it), as its host, and shows
the phone's page on the site, `/phone/?room=<name>`, as a QR code. The page is the one `claudius serve` serves to a
phone ([Server, sessions and MCP](server.md#the-phones-page)); with `?room=` it sends each request as a message
`{id, method, path, body}` through the room. `mock/phone.rs` answers them as the server's `phone.rs` does: the song and
the clock from the demo's state and the transport the studio reports, each lane's hit, the keys' notes and a record
rendered by the engine (`claudius_session::host::Host::sound`, a message of their own to the worker) and kept as WAV
files for the song's version, and a take sent to the engine as a command of its own name, whose answer goes to the
phone and to the person as a toast. A sound goes back as a binary message: the phone's id, the request's, and the WAV.

### The demo's samples

A song built on samples is built on them in the demo too: one of a genre that is by default (trip-hop, boom bap,
lo-fi, jungle, big beat, filter house), or one asked to be (`sampled`, `sample_search`). Making it starts a job,
`mock/sample.rs`, that gathers what the genre samples, as the engine's genre info says (`GenreInfo::sampling`): a
phrase, and a drum break or a texture where the genre has them. For each it tries the genre's searches in an order the
song picks ("disco", "gospel choir", "funk drum break"; the phrase's searched for the words asked instead), takes the
recordings of a length the role takes under CC0 or CC BY, and fetches one, a loop or a whole song. The page only finds
and fetches: it hands the engine the file's bytes (`Link::recording`), and the engine, in its Web Worker, cuts it as
claudius cuts one (`Host::sample`, `sampling::cut`): it decodes the file, finds its beat grid, and cuts the phrase out
of the two bars of its harmonic sections that are loudest (the demo has no ears to choose by), with their key and
chords, or the break with its own hits, or sixteen seconds of texture, and keeps the cuts in memory. A recording it can
cut nothing from, or one that no speed brings into the genre's tempo in tune (the browser only varispeeds), it turns
down, and the page tries the next. The song is made from what was kept as the server makes one from the samples it
gathered (see [Songs built on samples](../guide/online-samples.md#songs-built-on-samples)), and the log gives each
sample's full credit with the links.

The demo asks the site, never the libraries: `samples/search?q=…` and `samples/audio/…`, relative to the
page. The site's Worker answers those (see [Privacy](../guide/privacy.md#the-website)). Natively,
`CLAUDIUS_DEMO_SAMPLES` names an address, such as the local site's:

```sh
cd site && node tests/serve.mjs &          # its /studio/samples/ asks Openverse and the libraries live
CLAUDIUS_DEMO_SAMPLES=http://127.0.0.1:4321/studio/samples cargo run -p claudius-ui -- --mock
```

## Scripted runs

`CLAUDIUS_UI_PLAN` drives the native window from a list of steps separated by `;`, so a change to the UI can be
checked, screenshotted and repeated without a person at the screen:

```sh
CLAUDIUS_UI_PLAN='wait:4;shot:/tmp/a.png;tab:mixer;play;wait:2;shot:/tmp/b.png;exit' cargo run -p claudius-ui -- --mock
```

The steps: `wait:<s>`, `tab:<name>`, `section:<i>`, `play`, `pause`, `seek:<s>`, `newsong`, `close`,
`tour[:<step>]`, `assistants` (the Connect an assistant dialog), `publish` (the Publish dialog), `cmd:<command JSON>`,
`size:<w>,<h>`, `script:<code>`
(into the editor, `\n` for a newline and `%3B` for `;`), `run`, `reference`, `audiopath:<file>`, `shot:<file.png>`
(the window as rendered), `exit`, and synthetic pointer input in points: `move:x,y`, `click:x,y`, `rclick:x,y`,
`drag:x1,y1,x2,y2`, `sdrag:x1,y1,x2,y2` (with shift). A scripted run starts the first-run tour only when a `tour` step
asks for it, so that the screenshots of other steps do not have its card in them.

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
