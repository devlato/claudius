# HTTP API

`claudius studio`, `claudius serve` and a `claudius mcp` that runs its own session answer HTTP on
`http://127.0.0.1:7878` (`--port`, `CLAUDIUS_PORT`). The studio, in its window and in the browser, uses nothing but
this API, so anything it does you can do with `curl`. The types are plain serde structs in the `claudius-api` crate
(`crates/claudius-api/src/lib.rs`), which also compiles to WebAssembly for the web studio.

## Routes

| route | answer |
|---|---|
| `GET /api/hello` | `{"name":"claudius","version":"0.1.0","state":1,"ears":false}`: is this claudius, which version, the state version, whether earshot's models are loaded |
| `GET /api/metrics` | how fast this claudius has run since it started, in Prometheus' text format: for each of rendering, listening with the ears and decoding a recording, how many times (`claudius_render_total`), the seconds of audio (`claudius_render_audio_seconds_total`), the seconds they took (`claudius_render_seconds_total`) and the last one's speed in seconds of audio a second (`claudius_render_speed`); its version (`claudius_build_info`), uptime and the song's changes. A Prometheus server can scrape it; nothing leaves the computer unless something asks |
| `GET /api/state` | the [state](#the-state) at once |
| `GET /api/state?since=N` | the state as soon as its version is past `N` ([long polling](#versions-and-long-polling)) |
| `GET /api/state?settled=1` | the state once no job runs and the render is of the current song |
| `POST /api/command` | runs one [command](#commands) (JSON body) and answers with a [reply](#replies-and-errors) |
| `GET /api/script` | the song as a script, `text/plain` (`// no song yet: try song("house")` before the first song) |
| `GET /api/reference` | the script language reference, `text/markdown` |
| `GET /api/sounds` | every sound name: drum voices, synth presets, installed instruments, and every pack; also the installed kits' drums (`kit`), the roles each synth preset is made for (`synth_roles`), what each synth preset and drum voice is in a few words (`about`), and for each role the styles a part may be written in, each with what it plays (`styles`) |
| `GET /api/genres?q=WORDS` | the genres as the state lists them, or those that answer the search `q`, best first: a name or another name, an era, a scene, an artist or a sound (`?q=west%20coast`) |
| `POST /api/hear` | what was said to the [command palette](../getting-started/studio.md#saying-it), heard: the body is a WAV file of it (`Content-Type: audio/wav`), at most 30 s, any rate and channels; the answer is `{"heard", "text"}`, the words as the speech model heard them and as the palette reads them, words claudius knows put right ("undu" as undo, "base" as bass when the song has a bass), both empty when nothing was said. The speech model downloads the first time (17 MB). 415 for another content type, 422 for a file that is no WAV or is too long |
| `GET /api/find?q=WORDS` | the studio's actions the [command palette](../getting-started/studio.md#finding-an-action) finds for `WORDS` (optional `limit`, 12 by default and 40 at most), best first: `{"query", "found": [{"id", "name", "place", "keys", "score", "by"}], "done": [...], "proposed": [...], "reading", "by_meaning"}`, where `by` says whether an action was found by its `name` or by what the words mean (`meaning`), `done` holds the changes in the state's `history` the words find that `go_back` can reach (at most 5: `id`, what undo calls it as `name`, what it changed as `place`), `proposed` holds what the words ask of the song, read as what claudius would do for it, for the person to confirm (at most 4, the likeliest first: `said`, what it does in words; `detail`, what it does to each track when it does more than one thing; `commands`, the commands that do it; `reading`, what was read: its `kind`, `tracks`, numbers, `way`, the change it goes back to before as `about`, and `by_model` when the request model read it; and `score`), `reading` says the request model is still reading words the readings proposed nothing for (ask again in a moment; the first such words download it), and `by_meaning` says whether the embedding model is there yet (the first request downloads it); all the actions, in order, for no words |
| `GET /api/audio/{version}.wav` | the master of that render, 48 kHz 24-bit |
| `GET /api/audio/{version}.mp3` | the same as MP3, encoded on the first request |
| `GET /api/stems/{version}/{track}.wav` | one track of a render made with stems |
| `GET /api/export/{version}/{name}.{ext}?format=SPEC` | that render in another [format](export.md#formats), as a download named `{name}.{ext}` |
| `POST /api/release/artwork` | the release's [artwork](publish.md), as the file itself with `Content-Type: image/jpeg`, `image/png` or `application/octet-stream` (40 MB at most); answers with a [reply](#replies-and-errors) |
| `GET /api/release/artwork` | the artwork as it was given |
| `GET /api/release/preview.png` | the artwork at most 512 pixels on a side, as PNG |
| `POST /api/sample` | a recording whose vocals to [sample into the song](samples.md#sampling-a-songs-vocals), as the file itself, as for `/api/takes`; optional `name` (whose extension says the format), `count`, `bars`, `find`, `sections` (comma-separated) and `how` ([`phrases`, `passage`, `chops` or `stabs`](samples.md#other-ways-to-sample)) in the query, as for `sample_song`; answers with a [reply](#replies-and-errors) |
| `POST /api/groove` | a recording whose drums' [groove](../language/song-functions.md#groove) to take, as the file itself, as for `/api/takes`; optional `name` (whose extension says the format, and whose `92bpm` says the tempo), `groove` (what to call it), `lanes` (comma-separated) and `bpm` in the query, as for `extract_groove`; answers with a [reply](#replies-and-errors) |
| `POST /api/pitch-check?id=ID` | the answers to [pitch check](voice.md#checking-your-pitch) `id`, sung from its first beat, as the file itself, as for `/api/takes`; answers with a [reply](#replies-and-errors) saying what claudius heard, and the voice it heard becomes the person's (`voice`) |
| `GET /api/library/search?q=WORDS` | the recordings the [free online libraries](online-samples.md#choosing-one-in-the-studio) offer for `WORDS` (optional `limit`, 20 by default), as a JSON array of `LibraryHit`: `id`, `title`, `creator`, `licence`, `seconds`, `page`, `audio` (where to fetch it from this claudius) and `credit`; a command takes one's `id` as `library:<id>` wherever it takes a recording's path |
| `GET /api/library/audio?id=ID` | a recording a search found, downloaded once into the samples folder with its credit, to hear |
| `POST /api/sample/pieces` | a recording, as the file itself, whose [pieces](samples.md#choosing-the-pieces-first) the auto-sampler offers; optional `name`, `seed`, `count`, `bars` and `find` in the query, as for `sample_pieces`; answers with a [reply](#replies-and-errors), and the pieces go in the state's `pieces` |
| `GET /api/pieces/FILE` | a piece the auto-sampler offers, to hear (as `pieces` names it) |
| `POST /api/takes?from=SECTION` | a recording as a [take](samples.md#takes), as the file itself (WAV, FLAC, Ogg, MP3 or AIFF) with `Content-Type: audio/...` or `application/octet-stream` (300 MB at most); optional `name` and `role` in the query, as for `add_take`, and `request`, the id of the [take request](samples.md#a-take-an-assistant-asks-for) it answers; answers with a [reply](#replies-and-errors) |
| `POST /api/analyze?name=FILE` | a recording to listen to, as the file itself with `Content-Type: audio/...` or `application/octet-stream` (300 MB at most), named in the query so that its extension says its format (WAV, FLAC, MP3, AAC, ALAC, Ogg, AIFF ...); answers with a [reply](#replies-and-errors) whose message is what `analyze` says, and `heard`: what was heard as the studio shows it (its `bpm`, `steady`, `key`, `key_confidence`, `swing`, `lufs`, `secs`; `bars`, each with its `start` in seconds and its `chord` and `numeral`; `sections`, each with its `start_bar`, `bars`, `kind` and `energy` and whether its `kick`, `bass`, `hats` and `harmony` play; `progression_a`, `progression_b`, `bars_per_chord`; `kick`, `snare` and `hats`, a strength for each sixteenth of a bar; `bass`, in words), and `audio`, its `id` and the `tracks` claudius keeps of it until it analyzes another |
| `GET /api/listened/{id}/{track}` | a track of the recording analyzed last as a WAV file, `id` its answer's `heard.audio.id`: `recording`, and with the ears the parts it took it apart into, `vocals`, `drums`, `bass` and `other`; 404 once another is analyzed |
| `POST /api/remake?name=FILE` | a recording to rework (or flip, or remake), sent as for `/api/analyze`; optional `genre`, `seed`, `bpm`, `sampled` and `flip` in the query, as for `remake`; answers with a [reply](#replies-and-errors) |
| `POST /api/project?name=FILE` | a [project the demo saved](../getting-started/studio.md#carrying-on-in-claudius), as the zip itself with `Content-Type: application/zip` or `application/octet-stream` (300 MB at most), named in the query; claudius unpacks it into a song folder of its own under `~/Music/claudius`, named after the file, and opens it there; answers with a [reply](#replies-and-errors) |
| `/mcp` | the [MCP endpoint](mcp.md#mcp-over-http) |
| `POST /api/phone` | serves the [page a phone plays along on](phone.md), with a new key (or answers with the one being served): `{"url": "http://192.168.1.23:7879/?key=…", "phones": 0}`, also in the state as `phone`; 503 when the computer is on no network |
| `DELETE /api/phone` | stops serving it |
| `POST /api/transport` | where the studio's song is, `{"playing": true, "at": 12.5}` (seconds from its start), for the phone's page to follow; the studio sends it as it plays while the page is served |
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
curl -s 'http://127.0.0.1:7878/api/state?settled=1&timeout=120'    # returned after 6.5 s, with the render and verdict
```

The MCP tools wait the same way after every edit.

## The state

`GET /api/state` returns a `State`. Here it is after the request above, with long lists cut short:

```json
{
  "version": 7, "song_version": 2, "busy": null, "progress": null, "cancellable": false, "ears_loading": false,
  "can_undo": true, "can_redo": false,
  "undo_label": "new song: Crimson Tide — house at 126 BPM in G dorian (seed 9)", "redo_label": null,
  "log": ["sample library: Apple Loops and your folders",
          "new song: Crimson Tide — house at 126 BPM in G dorian (seed 9)",
          "rendered in 6.3 s: -9.9 LUFS, verdict 92.6 — 93/100 (arc 0.86, mix 0.92, theory 1.00). Fix first: nearly mono (correlation 0.96): widen pads, chords and effects; the drop is only 2.8 LU louder than the breakdown (aim for 3-7): thin the break or push the drop"],
  "genres": [{ "name": "deep_house", "family": "house", "aliases": ["deep", "soulful house", "lounge house"],
               "description": "Deep house: softer four-on-the-floor, ...", "bpm": [118.0, 124.0] }],
  "song": {
    "title": "Crimson Tide", "genre": "house", "seed": 9, "bpm": 126.0, "key": "G dorian", "swing": 0.5257842,
    "bars": 72, "seconds": 137.14285714285714,
    "progressions": [["A", ["i7", "IV9", "i7", "VIImaj7", "i7", "IV9", "i7", "V7"]], ["B", ["i7", "ii7", "i7", "VIImaj7"]]],
    "sections": [{ "name": "intro", "kind": "intro", "start_bar": 0, "bars": 8, "energy": 0.35950592,
                   "layers": ["kick", "hat", "perc"], "prog": "A", "variant": "sparse",
                   "filter_in": true, "build": false, "gap": false, "impact": false, "fills": false }],
    "layers": ["kick", "clap", "hat", "ohat", "shaker", "perc", "bass", "chords", "pad", "lead", "arp"],
    "tracks": [{ "name": "kick", "kind": "drum", "role": "kick", "sound": "sampled bd (tr909:bd)",
                 "level_db": 0.0, "pan": 0.0, "reverb": 0.0, "delay": 0.0, "duck": 0.0,
                 "muted": false, "solo": false, "loudness": -13.0, "fx": [], "bus": "drums" }],
    "drums": [{ "lane": "kick", "pattern_bars": 4,
                "variants": [["main", [0.99353635, 0.0, 0.0, 0.0, 0.9903264, 0.0, 0.0, 0.0]]] }],
    "parts": [{ "name": "bass", "role": "bass", "style": "syncopated", "styles": ["offbeat", "syncopated", "offbeat"],
                "preset": "saw_bass", "presets": ["organ_bass", "pluck_bass", "saw_bass", "sub_bass", "808", "reese",
                "acid", "wobble", "log_drum", "fm_bass", "rubber_bass", "moog_bass", "dist_bass", "deep_bass",
                "salamander:piano"],
                "register": [33, 52], "density": 0.42195085 }],
    "script": "// Crimson Tide — house (seed 9)\n// song(\"house\", 9);   // uncomment to start over from the generator\n...",
    "lineage": ["generated: house seed 9"]
  },
  "render": {
    "version": 2, "url": "/api/audio/2.mp3", "seconds": 137.75695833333333, "lufs": -9.918853251636394,
    "true_peak_db": -1.05, "bpm": 126.0,
    "sections": [["intro", 0.0, 15.238095238095237], ["groove", 15.238095238095237, 76.19047619047619]],
    "waveform": [0.7832023, 0.6933847, 0.24473634, 0.7832023],
    "stems": []
  },
  "verdict": {
    "total": 92.55483794905726,
    "summary": "93/100 (arc 0.86, mix 0.92, theory 1.00). Fix first: nearly mono (correlation 0.96): widen pads, chords and effects; the drop is only 2.8 LU louder than the breakdown (aim for 3-7): thin the break or push the drop",
    "scores": [["arc", 0.8594193282170034], ["mix", 0.92], ["theory", 1.0]],
    "issues": [[0.35, "nearly mono (correlation 0.96): widen pads, chords and effects", "width+"],
               [0.0695062248036405, "the drop is only 2.8 LU louder than the breakdown (aim for 3-7): thin the break or push the drop", "form"]],
    "ears": null,
    "measures": [["arc_correlation", 0.76], ["band_air_db", -18.5], ["band_low_db", -3.4]]
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
| `busy` | string or null | the running job: `"rendering"`, `"listening"`, `"mutating: judging 4 variations"`, `"evolving: generation 2 of 3 done"`, `"saving"`, or a step of listening to a recording sent with `/api/sample`, `/api/groove`, `/api/remake` or `/api/analyze`: `"taking song.mp3 apart into its voice, drums, bass and the rest"`, `"sampling the voice of song.mp3"` |
| `progress` | number or null | 0 to 1, when the job knows it |
| `cancellable` | bool | whether `busy` is a job that `cancel` stops; a step of listening to a recording runs to its end |
| `ears_loading` | boolean | earshot's models are still loading; until they are, verdicts come from the meters and music theory only |
| `can_undo`, `can_redo` | boolean | |
| `undo_steps`, `redo_steps` | integer | how many edits undo can take back, one at a time, and redo can bring back |
| `undo_label`, `redo_label` | string or null | what undo would take back, what redo would bring back |
| `history` | `Done` list | the last 200 changes made to the song, the latest first, as its [history](../engineering/server.md#the-history) keeps them: each change's `id` (what `go_back` takes), its `kind` (`edit`, `undo`, `redo`, `back` for a going back, or `moved` when a save copied the song's takes into its folder), its `label` (what undo calls it), what it changed in words (`said`: "added the whistle track (lead), playing the synth whistle; tempo 124 to 128"), when (`at`, milliseconds since 1970), and whether `go_back` can still take the song back to before it (`can_go_back`) |
| `log` | strings | the last 200 lines of the session log, oldest first |
| `genres` | `GenreInfo` list | every genre: `name`, `description`, `bpm` range |
| `release` | `ReleaseView` or null | the release in the making for the song: its `details` (as `set_release` takes them), `song_title`, what the `artwork` is and its `artwork_version`, the `checks` (each a `level`, `problem`, `warning` or `note`, what it is `about` and its `message`) of the render `checked`, whether it is `ready`, where the last `kit` went, and the `made_options` |
| `pitch_check` | `PitchCheck` or null | the last [pitch check](voice.md#checking-your-pitch) asked of the person: its `id`, `bpm`, the `notes` to play (`t` and `d` in beats, `p` MIDI), where each phrase's answer goes (`answers`, beats), how long it lasts (`beats`), the asker's `note`, its `status` (`waiting`, `taken` or `cancelled`), and, once sung, claudius' `answer` |
| `voice` | string or null | the person's voice as their last pitch check heard it, in words |
| `pieces` | `PiecesView` or null | the auto-sampler's last [offer](samples.md#choosing-the-pieces-first): the `recording`, the take's `seed`, and its `pieces`, each with its `id`, `place` in the recording, `bars`, how `sung` it is, where to hear it as it was cut (`raw`) and `treated`, and its `treatment` in words |
| `take_request` | `TakeRequest` or null | the last [take someone asked the person to record](samples.md#a-take-an-assistant-asks-for): its `id`, `from`, `role`, `name` and `note` as `request_take` gave them, its `status` (`waiting`, `taken` or `cancelled`), and, once taken, claudius' `answer` to the take |

### `SongView`

| field | meaning |
|---|---|
| `title`, `genre`, `seed`, `bpm`, `key`, `swing` | the song's settings; `key` reads like `"A minor"` |
| `bars`, `seconds` | its length |
| `progressions` | `[name, [numerals]]` pairs, `"A"` and `"B"` in generated songs |
| `sections` | `SectionView` per section: `name`, `kind`, `start_bar`, `bars`, `energy`, `layers`, `prog`, `variant` and the flags `filter_in`, `build`, `gap`, `impact`, `fills` |
| `layers` | every name a section can switch on: drum lanes, parts, loops, pattern tracks |
| `tracks` | `TrackView` per track: `name`, `kind` (`drum`, `part`, `loop`, `pattern`, `fx`), `role`, `sound` in words, the mix (`level_db`, `pan`, `reverb`, `delay`, `duck`, `muted`, `solo`), its `bus`, its chain of effects `fx` (each as the song keeps it, a map named by `fx`, as [`fx()`](../language/mixing.md#effect-chains) writes it), `loudness` in the master (LUFS while playing) once rendered, and for a take, `seconds`: how long its recording is |
| `mix` | `MixView`: the `buses` as `[name, effects]`, the `master` chain, its `lufs` and `ceiling_db`, and the `reverb` and `delay` the sends feed |
| `drums` | `DrumView` per lane: `lane`, `pattern_bars`, `variants` as `[name, velocities]` with one velocity per sixteenth (0 for none), and `trigs` as `[name, [[step, trig]]]` for the steps with a [trig](../language/song-functions.md#trig) (`prob`, `cond`, `ratchet`, `ramp`, `flam`, `nudge`, `pitch`, `decay`, `tone`, `pan`, each only where it differs from its default), and the lane's own time and feel as [`lane()`](../language/song-functions.md#lane) sets it: `length` (sixteenths before it starts again, when it has a length of its own), `speed` (1), `swing` (when it has its own), `groove` and `groove_amount` (1) |
| `grooves` | the names of the grooves the song keeps, heard in recordings or written as tables; lanes play them as they play claudius' own |
| `parts` | `PartView` per generated part: `name`, `role`, `style`, the `styles` its genre offers, `preset`, the `presets` it may switch to, `register` (MIDI low, high), `density`, and `cycles`: for each progression its sections play, then each it has notes written out for and plays nowhere now, `prog`, the `beats` its cycle comes round in, whether it is `written` out, and its `notes` (`{t, d, p, v}`, beats from the cycle's start), as the theory engine writes them or as written |
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
| `fixes` | `{target, label}` for each issue one change fixes (the `quick_fix` command makes it), absent when none |

[The critic and the ears](../engineering/critic.md) explains every score, measure and issue target.

## Commands

`POST /api/command` takes one command as a JSON object tagged by `"cmd"`, with `Content-Type: application/json`. The
fields marked optional may be left out.

| `cmd` | fields | what it does |
|---|---|---|
| `new_song` | `genre`; optional `seed`, `bpm`, `key`, `loops` (false), `sampled` (as the genre is), `sample_search`, `complexity` (0 to 1; the genre's own) | composes a new song; `sampled` builds it on samples from free libraries, `sample_search` searches for its phrase with your words ([Songs built on samples](online-samples.md#songs-built-on-samples)), `complexity` says how [layered](../learn/producers/genres-and-evolution.md#how-layered-a-song-is) it is |
| `load` | `path` | opens a song folder, a `blueprint.json`, a script file (run on the open song), or a project the demo saved (a `.zip`, unpacked into a song folder of its own under `~/Music/claudius`) |
| `open_text` | `name`, `text` | opens a song from a file's text, as the studio sends a file chosen on its computer: a `blueprint.json`'s when `name` ends in `.json`, otherwise a script's; saving makes a song folder of its own |
| `save` | optional `path` | writes `blueprint.json` and `song.rhai` now, then the whole folder from a render (see [Jobs](#jobs)); default folder `~/Music/claudius/<genre>-<title>-<seed>` |
| `export` | `formats` (such as `["mp3", "flac:16"]`), optional `path` | writes the latest render in those [formats](export.md#formats), when it is of the song as it is now (otherwise: `the song has changed since its last render: render it, then export`); default folder as for `save` |
| `set_release` | `details` (every field of `ReleaseDetails`: `artist`, `title`, `writers`, `made` ...), optional `artwork` (a JPEG or PNG file) | replaces what stores are told about the [release](publish.md), and checks it again |
| `write_kit` | optional `path` | writes the release kit from the latest render, when it is of the song as it is now and nothing stops the release; default `release/` in the song's folder |
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
| `set_trig` | `lane`, `variant`, `step` (a sixteenth, from 0); optional `trig` | what the step does beyond striking, as [`trig()`](../language/song-functions.md#trig) sets it; no `trig` clears it, and a step without a hit gets one |
| `set_lane` | `lane`; optional `length` (0: its pattern's), `speed`, `swing` (0: the song's), `groove` (`""`: none), `groove_amount`, `late_ms` (-80 to 80: every hit that far behind the beat; ahead, below zero) | the lane's own time and feel, as [`lane()`](../language/song-functions.md#lane) sets it |
| `set_part` | `name`; optional `style`, `preset`, `register` (`[low, high]`), `density`, `reseed` (false), `late_ms` (-80 to 80) | as `part()`; `reseed` writes the part again with a new seed |
| `add_track` | `name` (one word of letters, digits, `_` and `-`, not one the song has), `kind` (`drum` or `part`); optional `role` (a part's: `bass`, `chords`, `pad`, `keys`, `lead`, `arp`), `sound` | a drum lane, silent until its steps are written, playing `sound` (a drum voice or a kit's `tr909:bd`), as `drum()` and `drum_sound()`; or a part of your own, as [`part()` with a `role`](../language/song-functions.md#part), playing `sound` (a synth preset or an instrument; the genre's draw when left out). It plays where the form names it, else in every section with energy 0.6 or more |
| `remove_track` | `name` | a drum lane, part, loop, take or pattern track out of the song and every section, as `remove()` |
| `set_notes` | `part`, `prog`; optional `notes` (`[{t, d, p, v}]`, beats from the cycle's start; velocity 0.8 when left out) | the part's notes over that progression written out, as [`notes()`](../language/song-functions.md#notes): they replace what the theory engine writes there (an empty list rests); without `notes`, the theory engine writes it again, as `clear_notes()` |
| `set_mix` | `track`; optional `level_db`, `pan`, `reverb`, `delay`, `duck`, `drive_db`, `muted`, `solo`, `bus` | as `mix()` |
| `set_fx` | `target` (`track:<name>`, `bus:<name>`, `master`, `send:reverb`, `send:delay`), `fx` (effects as `TrackView.fx` has them; a send takes exactly one) | replaces that chain, as [`fx()`, `bus()`, `master(#{ fx })` and `send_fx()`](../language/mixing.md#effect-chains) do; an effect it cannot read is refused with its place and why |
| `run_script` | `code` | runs a script on the song |
| `analyze` | `path` | the analysis of an audio file, as text in the reply's `message` |
| `song_like` | optional `genre`, `seed` | a new song like the recording analyzed last, in the genre (the open song's by default): its tempo, halved or doubled into the genre's range when it is outside, its key, progressions, form and swing, none of its sound |
| `tune` | `track`; optional `strength`, `speed_ms`, `humanize`, `vibrato`, `scale` (`chords`, `key`, `chromatic` or notes such as `"C D E G A"`), `off` | a `tune` effect first in the track's chain, the one there changed with what is given, or with `off` taken out ([Tuning a voice](voice.md#tuning-a-voice)) |
| `assess_voice` | `take`, or `path` and optional `key` | how a take of the song or an audio file is sung ([Your voice](voice.md)), as text in the reply's `message`; changes nothing |
| `remake` | `path`; optional `genre`, `seed`, `bpm`, `sampled` (true), `flip` (false) | a rework of the file in the genre, all of it through in its own sections and on its chords with the genre's drums and bass under it ([Remaking a song from audio](../cookbook/remake.md)); `flip: true`, a new song of the genre on its hook and pieces of it; `sampled: false`, a new song on its skeleton only; the genre defaults to the open song's, and the tempo to the recording's, or double or half it when that is nearer the genre's range (half only where the recording holds its chords two bars) |
| `harmonize` | `part`; optional `style` (`block`, the default, a chord style: `sustain`, `half`, `stabs`, `offbeat`, `pump`, `syncopated`, `charleston`, or a [comping idiom](../engineering/theory.md#comping-idioms): `skank`, `gospel`, `house_piano` ...), `sound`, `name` (`<part>_chords`) | chords under the part's melody, as a new part that [harmonizes it](../language/song-functions.md#part), on in the sections the melody plays in; for `block`, on the melody's own sound |
| `generate_track` | `what` (`drums`, `percussion`, `bass`, `chords`, `keys`, `pad`, `strings`, `lead`, `arp`); optional `genre` (the song's), `replace` (a part of the song), `name` (`<genre>_<what>`), `seed` | a track [written the way the genre plays it](../getting-started/studio.md#writing-a-track-in-a-style): a part as the genre's own part of the role is (its idiom, a sound from its palette, its register, density, voicing and mix), added where the song plays its part of the role or in place of `replace` (its name, mix and sections kept); the genre's drums in place of the song's lanes of their names and beside the others; its percussion beside the song's drums. Each is another take |
| `vary_track` | `track` (a part or a drum lane); optional `take` (false) | a variation of the take it plays (a part's idiom keeps its cells and draws their details again; a lane is varied a step or two, its strong hits kept), or with `take`, another take (a part's seed anew; a lane from the template of the genre it was written as, the song's or the one its drums were generated from) |
| `swap_sound` | optional `track` (a part or a drum lane), `to`, `only` (`drums` or `parts`, with no `track`) | another sound for the track: `to` (one of its `swaps`, or any synth preset, installed instrument or drum sound), or its `next_swap`, the next of its `swaps` no other track plays; block chords under a part change with it. With no `track` the whole song swaps: the drums onto the `next_kit` (or `to`, one of `kits`) and each part onto its next sound. It runs as `part()` with a `preset` and `drum_sound()`, so every note, pattern and mix setting stays; a part played by a rack runs as `rack()`, the layers that play the sound it is heard by on the new one and the rest (the sub under a stacked bass) as they were |
| `add_take` | `path`, `from` (a section's name); optional `name` (take1, take2, ...), `role` (`vocal`, `lead`, `chords`, `keys`, `bass`, `drums`, `texture`; lead) | the recording as a [take](samples.md#takes): it plays once from the start of section `from` and on through the sections after it as long as it lasts; the song keeps a copy |
| `trim_take` | `track`; optional `start` (0) and `end` (its end), in seconds into its recording | the take cut down to that much of its recording ([Takes](samples.md#takes)): it plays from the start of the same section, the sections past its new end drop it, and the recording stays for undo |
| `sample_pieces` | `path` (a file, or `library:<id>`); optional `seed` (0: the best phrases; any other: another draw), `count` (2, 1 to 4), `bars` (1, 2 or 4), `find` | the auto-sampler's [pieces](samples.md#choosing-the-pieces-first) of the recording, each as it was cut and through a treatment, in `pieces`, before any goes into the song |
| `redraw_pieces` | optional `seed` (the next take) | the same recording's pieces drawn again |
| `keep_pieces` | `keep` (`[{"id": 1, "treated": true}]`); optional `sections` | the pieces chosen sampled into the song, each as it was cut or through its treatment; the offer goes |
| `sample_song` | `path`; optional `count` (2, 1 to 4), `bars` (1, 2 or 4), `find` ("a person singing"), `sections` | [samples the recording's vocals into the song](samples.md#sampling-a-songs-vocals): its best sung phrases as loops `vox1`, `vox2` ..., placed and mixed; the reply says where each came from and went |
| `extract_groove` | `path`; optional `name` (the file's), `lanes` (every drum lane), `bpm` | the [groove](../language/song-functions.md#groove) of the recording's drums, kept with the song, and the lanes put in it; its tempo is `bpm`, else an Apple Loop's from GarageBand's catalogue, else one in the file's name (`92bpm`), else heard. The reply says the tempo and what it heard |
| `transcribe_drums` | `take`, or `path`; optional `bars` (1, 2 or 4; the recording's, up to 4), `variant` (main), `bpm`, `keep_take` (false) | [a beat, played or beatboxed, as the song's drums](samples.md#turning-a-take-into-drums): its kicks, snares and hats folded into a pattern and written into the kick, snare (else clap) and hat lanes; steps hit only some times round play with that chance; a take goes quiet unless `keep_take` |
| `add_hits` | `from` (a section's name), `hits` (`[{lane, t, v}]`: the lane struck, beats from the section's start, velocity, 0.8 when left out); optional `bars` (1, 2 or 4; as much of 4 as was played), `variant` (main), `grid` (0.25, a sixteenth; 0 keeps the hits where they were played), `start` and `beats` (when the take began and how long it went on, in beats from the section's start; 0 and its last hit) | drums played on pads (a phone's, a MIDI controller's) as the lanes they strike, folded into a pattern; a step struck only some of the times round plays with that chance, as hard as it was struck on average |
| `add_scratch` | `from` (a section's name), `track` (one of the song's tracks, `*` for the whole mix, or a sample from the library as the phone's `GET /api/library` names it, `lib:...`), optional `recording` (a record recorded off a microphone in place of the track's, a WAV file in base64, up to 10 s; mono at 24 kHz, as the phone sends it, keeps a command under its 1 MiB), `moves` (`[{t, at, fader}]`: seconds into the scratch, seconds into the record, the crossfader from 0, shut, to 1, open, open when left out); optional `bar` (0, the first), `bars` (1, up to 8), `name` (scratch1, scratch2, ...), `role` (lead) | a scratch kept as a [take](samples.md#takes) that plays from the start of section `from`: the track alone over the bars is the record, read back and forth along the moves through the crossfader |
| `transcribe_take` | `take`, `sounds` (presets or instruments); optional `name` (melody), `role` (lead), `chromatic` (false), `keep_take` (false), `style` (line) | [the take's melody as notes](samples.md#turning-a-take-into-notes), played by a new part for each sound; the take goes quiet unless `keep_take` |
| `add_notes` | `from` (a section's name), `notes` (`[{t, d, p, v}]`: beats from the section's start, length in beats, MIDI pitch, velocity, 0.8 when left out), `sounds`; optional `name` (played), `role` (lead), `style` (line), `request` (the take request the notes answer) | [notes played or written](samples.md#notes-played-on-a-keyboard), chords and all, as a new part for each sound |
| `request_take` | `from` (a section's name); optional `role` (as `add_take`'s, or a part's: `pad`, `arp` ...), `name`, `note` (what to record, for the person) | [asks the person to record a take](samples.md#a-take-an-assistant-asks-for): the studio's Record window opens for it; the request goes in `take_request`, waiting, and replaces one asked before |
| `cancel_take_request` | `id` | the person will not record the take asked for, as the Record window's Not now says |
| `request_pitch_check` | optional `low` and `high` (MIDI, default 57 and 69), `note` | asks the person for a [pitch check](voice.md#checking-your-pitch): the Record window opens with its phrases; it goes in `pitch_check`, waiting, and replaces one asked before |
| `cancel_pitch_check` | `id` | the person will not do the pitch check now |
| `quick_fix` | optional `target` (one of the verdict's `fixes`; none: all of them) | makes the verdict's [quick fixes](../getting-started/studio.md#quick-fixes) as one edit; refused while the verdict is of the song before its last change |
| `mutate` | optional `count` (4, 1 to 32), `keep_best` (true) | a job: tries variations and keeps the best if it beats the song |
| `evolve` | optional `generations` (3, 1 to 20), `children` (6, 2 to 32) | a job: an evolutionary search from the song |
| `install_packs` | optional `names` (the missing default packs when left out) | a job: downloads sample packs, as `claudius setup` does; not the opt-in packs, whose licence is unclear, and not on a shared claudius |
| `render` | optional `stems` (false) | a job: renders and judges the song |
| `undo`, `redo` | | |
| `go_back` | `to` (a change's `id` in the state's `history`) | the song as it was before that change: it and every change since are undone, as that many undos would, and redo brings them back one at a time; refused for a change undo can no longer take back (`can_go_back` false) |
| `together` | `said` (what undo calls it), `commands` (edits of the song: `set_mix`, `set_tempo`, `set_key`, `swap_sound`, `toggle_layer`, `remove_track` and the other edits, but no undo, redo, `go_back` or `together`) | the edits made as one change, which the history records as one and undo takes back at once; when one fails, none is made and the song stays as it was. The palette sends it for a request in plain words a person confirmed |
| `cancel` | | stops a mutation or evolution after its current candidate or generation |

```json
{"cmd": "set_tempo", "bpm": 124}
{"cmd": "set_section", "index": 0, "energy": 0.5, "layers": ["kick", "hat"]}
{"cmd": "set_drum_step", "lane": "kick", "variant": "main", "step": 3, "velocity": 0.6}
{"cmd": "set_part", "name": "bass", "style": "rolling", "reseed": true}
{"cmd": "add_track", "name": "lead2", "kind": "part", "role": "lead", "sound": "saw_lead"}
{"cmd": "add_track", "name": "conga", "kind": "drum", "sound": "tr909:lt"}
{"cmd": "run_script", "code": "tempo(126); mix(\"lead\", #{ level: -2 });"}
```

`set_tempo`, `set_key`, `set_swing`, `set_progression`, `set_drum_pattern`, `set_part`, `add_track`, `remove_track`,
`set_notes` and `set_mix` are turned into the matching script call and run as a script, so they accept exactly what the script
functions accept and fail with the same messages (`add_track` checks its name first). The other edits change the blueprint directly. Every edit that changes the song is one undo step
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
