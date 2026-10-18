# Architecture

claudius is a Cargo workspace of eight crates. One of them, `claudius`, is the engine and does all the music; the
others decode audio, manage sample packs, listen, serve, draw and parse command lines around it.

## The crates

```text
                       claudius-cli  (the `claudius` binary)
                      /      |      \___________________________
                     /       |                                  \
          claudius-server    |                     claudius-ui (feature "studio")
           /     |     \     |                                  |
          /      |      \    |                                  |
 claudius-api  claudius (the engine)                      claudius-api
               /      |       \
              /       |        \
   claudius-packs   earshot   claudius-decode
          \           |
           \          |
          claudius-decode
```

| crate | what it is | depends on |
|---|---|---|
| `claudius` | the engine: theory, composition, patterns and scripts, synthesis, effects, rendering, the critic, evolution, analysis, the Apple Loops library | `claudius-packs`, `earshot`, `claudius-decode` |
| `claudius-decode` | audio files to samples: symphonia in pure Rust, ffmpeg as a fallback, limited to the one file and to 30 minutes; trims the encoder priming of AAC in CAF | symphonia |
| `claudius-packs` | the sample packs: registry, installer, SFZ and Strudel-map readers, name resolution, the multi-sample player, credits; also the `claudius-packs` binary | `claudius-decode` |
| `earshot` | a Rust port of the Python earshot: CLAP and the AudioSet tagger through ONNX Runtime, the calibrated vocabulary, realness, measurements | `claudius-decode`, `ort` |
| `claudius-api` | the wire types of the HTTP API: plain serde structs, so they build for WebAssembly | serde |
| `claudius-server` | the session (`App`), the HTTP routes, the MCP tools | `claudius`, `claudius-api`, `claudius-packs` |
| `claudius-ui` | the studio, in egui: a native window and a WebAssembly app, both clients of the HTTP API | `claudius-api` |
| `claudius-cli` | the `claudius` binary: one subcommand per task, `setup`, `doctor`, `connect` | all of the above |

Two dependency rules hold the shape together. The engine knows nothing about HTTP, sessions or windows: it takes a
blueprint and returns audio and a verdict. And the UI knows nothing about the engine: it depends only on
`claudius-api`, so it compiles to `wasm32-unknown-unknown` without pulling in ONNX Runtime, rayon or file access.
Everything the studio does, it does through the API, which keeps the API complete.

Outside `crates/` are `xtask/` (the repository's build and release chores, run as `cargo xtask`) and `fuzz/` (two
cargo-fuzz targets, in a workspace of their own because they need nightly).

## How a song flows

```text
  genre profile (data/genres/*.json) + seed + options
                     │ compose::generate
                     ▼
                 Blueprint ◀─────── script::run          scripts, studio edits, MCP tools
                     │    ◀─────── evolve::mutate         mutation, crossover
                     │    ◀─────── analysis::song         remakes of audio files
                     │ compose::arrange
                     ▼
                  Project          tracks with notes or audio clips, chords, sections, bus automation
                     │ render::render
                     ▼
                 Rendered          master, stems, per-track levels and gains, LUFS, true peak
                     │ judge::judge (+ ears::Ears::report)
                     ▼
                  Verdict          scores, measures, issues that name what to change
```

1. **Generate.** [`compose::generate`](song-model.md) makes every creative choice for a genre and a seed: tempo,
   key, two progressions, a form, drum patterns, the parts and their sounds, the mix. The result is a `Blueprint`:
   a song's genome, small (tens of kilobytes of JSON), serialisable and diffable.
2. **Edit.** Everything that changes a song changes its blueprint: a script, a step in the sequencer, a mutation, an
   undo. A blueprint is never patched in place by two parties; each edit produces a new one, and the session keeps the
   old one for undo.
3. **Arrange.** `compose::arrange` expands a blueprint into a `Project`, the way a producer lays a session out: one
   loop of every part per progression tiled across its sections, drum variants per section, fills, builds, gaps,
   transitions, and filter automation on the buses. Arranging is a pure function of the blueprint and cheap next to
   rendering, so projects are never stored by the session; every song folder gets one in `project.json`
   (`song::write_folder`), for `judge`.
4. **Render.** [`render::render`](rendering.md) plays every track through its sound, mixes them to the genre's
   targets, runs the buses and returns, and masters the sum to the target loudness under a true-peak ceiling.
5. **Judge.** [`judge::judge`](critic.md) measures the master, the arrangement and the score, and, when earshot's
   models are loaded, listens. Each issue carries a target (`master`, `form`, `part:lead`, `band:sub+` ...), which
   evolution uses to choose its next edits.

A blueprint also prints as a script (`script::to_script`) that runs back into the same blueprint, which is how the
studio's script tab, `GET /api/script` and `get_song` show a song. [Patterns and the script
engine](patterns.md#printing-a-song) explains how that round trip is kept.

## One session, many clients

```text
  studio window ──┐
  browser tab ────┤ HTTP /api ──┐
  curl ───────────┘             │
  assistant (HTTP) ─── /mcp ────┼──▶  App: one Session behind a Mutex
  Claude Desktop ─ stdio ─ claudius mcp ─(HTTP)─┘       │
                                                        ├─ version on a watch channel → long polls
                                                        └─ one job thread at a time
```

`claudius-server` holds one `App` per process. Its `Session` is the song with its undo and redo stacks, the latest
render and verdict, a log, and the running job. Every client is a view of it: the native window and the browser
long-poll `GET /api/state?since=N`; the MCP tools call the same `App` methods in process, or the same HTTP routes when
`claudius mcp` drives a studio in another process. Because there is one copy of the song, a person in the studio and
an assistant over MCP see each other's edits within one long poll. [0005 One session, many
clients](../design/0005-one-session.md) records why, and [Server, sessions and MCP](server.md) how.

## Threads

| what | where it runs |
|---|---|
| the HTTP server and the MCP transport | a multi-threaded tokio runtime; in `claudius studio`, on its own thread, since the window needs the main thread |
| a command (`POST /api/command`) | `tokio::task::spawn_blocking`, because scripts and analysis take milliseconds to seconds |
| edits | one at a time, serialised by the session's `edit` mutex |
| long jobs (render, mutate, evolve, save) | a plain `std::thread`, one job at a time; panics are caught and logged |
| loading earshot's models | a background thread at startup; verdicts include the ears once it finishes |
| a render | rayon: every track in parallel, then the buses in parallel, the two returns in parallel, and each channel of the soft clipper and true-peak meter |
| evolution | rayon: the children of a generation are rendered and judged in parallel |
| synthesis | polyphonic notes of a synth track render in parallel |
| pack downloads | scoped threads, 8 by default |
| earshot | each ONNX session behind a mutex, with `EARSHOT_THREADS` intra-op threads |
| the studio's network | `ehttp`: a thread per request natively, `fetch` in the browser; replies come back on a channel |

The session's lock is held only to read or swap state, never across a render or a script, so `GET /api/state`
answers while a job runs. Long polls wait on a `tokio::sync::watch` channel that every state change bumps.

## Determinism

The same blueprint renders the same song, and the same genre, seed and options generate the same blueprint, given
the same installed sounds. The pieces that make that so:

- **One random generator, many streams.** `rng::Rng` is xoshiro256++ seeded through splitmix64. `Rng::fork(label)`
  derives an independent stream from the current state and the label without advancing it, so `generate` gives
  harmony, form, drums, the kit, parts, loops, transitions and the title a stream each: writing one more part does
  not change the drums.
- **Seeds stored where they are used.** Every part has its own `seed` in the blueprint, and its notes come from
  `seed ^ hash(progression name)`. Changing a part's seed rewrites that part alone; changing another part's style
  leaves it as it was.
- **Pattern randomness is a hash, not a stream.** `?`, `|`, `degrade` and `sometimes` decide each event from a hash
  of a seed and the event's start time. A pattern gives the same events whichever window you query and in whatever
  order, which is what lets a track be queried section by section (`tests/pattern_laws.rs` checks it).
- **Render seeds from names.** Each track's humanisation, each effect's noise and each reverb's impulse response are
  seeded with `project seed ^ hash(track or bus name)`.
- **Hashes.** `rng::hash_str` (64-bit FNV-1a) turns names into seeds and lets evolution skip blueprints it has seen;
  blake3 names cache files.

What is not deterministic: a seed taken from the clock when none is given (always printed), the sounds that
resolve differently on machines with different packs or Apple Loops, and floating-point results across CPUs, which
is why the generator snapshots summarise blueprints instead of comparing audio.

Streams have one trap: an option that skipped a random draw would shift every draw after it. So `generate` draws a
tempo and a tonic whether or not they are given, and a `bpm` or `key` of your own replaces only the result.
`song("house", #{ seed: 9, bpm: 131 })` is "Crimson Tide" at 131 BPM, with the form, progressions and drums of
`song("house", 9)`; `a_tempo_or_key_of_your_own_changes_only_that` in `tests/script.rs` holds the title, the form,
progression `A` and every drum pattern to that. A `key` brings its mode too, and progression `B` is derived in the
key, so it can differ when the mode does: seed 9 asked for `E minor` instead of its own G dorian turns `B` from
`i7 VIImaj7 i7 IV7` into `i7 VII7 i7 IV7`, while `A` keeps its numerals. A song printed as a script still starts from
`song("house", 9)`; the `tempo()` and `key()` lines after it set the rest.

## Errors

Engine functions return `anyhow::Result` or `Result<_, String>` with messages written for the person reading them
(`unknown sound 'bdd' (drums: ...; synths: ...)`). The server passes them through unchanged as `Reply.message`, MCP
tool errors and log lines, and the CLI prints them after `Error:`. A failed edit leaves the song as it was: scripts
run against a copy, and the session swaps the copy in only when the script succeeds.

## Where to go next

[A tour of the repository](../contributing/onboarding.md) walks the code in reading order. The other engineering
pages take one stage each: [the song model](song-model.md), [composition and theory](theory.md),
[patterns](patterns.md), [rendering](rendering.md), [the critic](critic.md),
[evolution and analysis](evolution-and-analysis.md), [sample packs](packs.md), [the server](server.md),
[the studio](ui.md), [testing](testing.md) and [security](security.md).
