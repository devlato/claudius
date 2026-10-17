# A tour of the repository

claudius is about 45,000 lines of Rust in eight crates, and most of what makes it interesting is in one of them. This
page gives you the map, follows one song through the code from the command line to the speakers, and suggests an order
to read things in, with what to look for in each. Set up your machine first with [Development setup](setup.md); the
commands below assume a built checkout.

## The map

```text
crates/
  claudius/                the engine: everything that makes or judges music
    src/model.rs             Blueprint (a song's decisions) and Project (the arranged timeline)
    src/compose.rs           generate: decisions from a genre and a seed (parts, sounds, kits, flipped samples)
    src/compose/arrange.rs   arrange: decisions into notes, sections and automation
    src/genres.rs            loads data/genres/*.json
    src/theory/              keys, chords and voicing, progressions, rhythm, bass lines, leads
    src/pattern/             mini-notation, pattern queries, events into notes
    src/script/              the Rhai engine; song functions (song.rs, mix.rs, form.rs); sound names; printing
    src/render.rs            the mixer and the master
    src/fx/ dsp/ synth/      effects; filters, oscillators, FFT; drum voices, the synth engine, risers
    src/loudness.rs          BS.1770 loudness
    src/judge.rs ears.rs     the critic, and the ears that feed it
    src/evolve.rs            mutation, crossover, the evolutionary search
    src/analysis/            reading tempo, key, chords and sections from audio
    src/library/             Apple Loops, chops, one-shot cutting
    src/config.rs            constants, folders, the shared-mode file policy
    src/song.rs              one song end to end, and write_folder, which writes every song folder
    data/genres/             the eight genres, as data
    tests/ benches/          the engine's tests and benchmarks
  claudius-packs/          sample packs: registry, installer, SFZ and Strudel maps, the player
  claudius-decode/         audio files to samples (symphonia, ffmpeg as fallback)
  earshot/                 the ears: earshot ported to Rust, CLAP and AudioSet on ONNX Runtime
  claudius-api/            the HTTP API's types, shared with the web studio
  claudius-server/         the session, HTTP routes, MCP tools
  claudius-ui/             the studio, native and WebAssembly
  claudius-cli/            the claudius program: main.rs (the commands), args.rs (flags), report.rs (printing)
docs/                      this book
fuzz/                      cargo-fuzz targets (their own workspace)
xtask/                     cargo xtask: ci, web, docs, site, dist, release
scripts/                   install scripts, the Homebrew formula template
site/                      the website
.github/                   CI, releases, fuzzing, dependency updates
```

Two rules keep the shape: the engine knows nothing about HTTP, sessions or windows, and the studio knows nothing about
the engine, only the API. [Architecture](../engineering/architecture.md) draws the dependencies.

## One song through the code

Follow `claudius make --genre house --seed 9` from start to finish. Keep the files open as you go.

**The command** (`crates/claudius-cli/src/main.rs`, `Cmd::Make`). clap parses the flags, which are defined in
`args.rs`. `genres::get("house")`
deserialises `data/genres/house.json`, plus any genre files of your own, into a `GenreProfile`. `Library::open`
reads the Apple Loops database if GarageBand content is installed. `listen()` loads earshot's models unless
`--no-ears`.

**The decisions** (`compose::generate`, then `generate_inner`). A seeded generator is forked into one stream per area:
harmony picks two progressions as Roman numerals, form picks a list of sections, drums sample a 4-bar pattern per lane
from the genre's step probabilities, parts get a preset, a style and a seed of their own. Sounds come from
`part_sound` and `kit_sound`, which prefer a sample-pack instrument where the genre asks for one and it is installed.
The result is a `Blueprint` (`model.rs`): about 30 KB of JSON that says everything about the song and contains almost
no notes.

**The layout** (`song::render_and_write`, then `compose::arrange` in `compose/arrange.rs`). The blueprint becomes a
`Project`. For each section
the drum patterns are tiled, with snare rolls in builds and fills every 8 bars; each part is written once per
progression by the writers in `theory/` (`bass::bassline`, `melody::chord_part`, `melody::lead`, `melody::arp`) and
tiled across its sections; transitions are placed; bus automation is added.

**The sound** (`render::render`). Every track is rendered in parallel: notes through a drum voice
(`synth/drums.rs`), the synth engine (`synth/voice.rs`), a sample-pack instrument (`claudius_packs::Instrument`) or a
loop (`warp.rs`); then its inserts, the auto-mixer's gain to its role's target loudness, the sidechain, the pan. The
buses and the two returns are summed, and `finalize` searches for the input gain that lands the master on the genre's
LUFS target under the true-peak ceiling.

**The judgement** (`judge::judge`, with `ears::Ears::report`). Meters, the energy arc, theory checks on the notes,
and earshot's calibrated CLAP scores become a `Verdict` with scores and issues.

**The files**. `render_and_write` hands the render to `song::write_folder`, which writes `song.wav`, `song.mp3`,
`blueprint.json`, `song.rhai`, `project.json`, `report.json`, `verdict.json` and `credits.txt` (and `stems/` with
`--stems`), and `report.rs` prints the summary. A save from the studio goes through the same `write_folder`.

Now the same song in the studio, after `claudius studio`:

1. `claudius_server::App::new` creates the session; `claudius_server::spawn` serves it on a thread; `claudius_ui::run_native`
   opens the window on the main thread.
2. The studio long-polls `GET /api/state?since=N` (`claudius-ui/src/net.rs`). You click a step in the sequencer
   (`panels/sequencer.rs`): it pushes `Command::SetDrumStep`, shows the step at once (`edits.rs`), and posts it.
3. `http.rs` checks the host name, parses the JSON, and calls `App::command` on a blocking thread
   (`claudius-server/src/app/commands.rs`). `edit_with` changes a copy of the blueprint, `commit` pushes the old one
   for undo, and `after_change` starts a render job (`app/jobs.rs`).
4. The job arranges, renders and judges as above, writes `<cache>/renders/<pid>/<version>.wav`, and bumps the version.
   The long poll returns, the studio sees a new `render.version`, fetches the WAV and plays it from where you were.

And an assistant, through `claudius mcp`: `mcp.rs` finds the studio on the port, picks `Backend::Remote`, and turns a
`run_script` tool call into `POST /api/command`, which lands in `App::run_script` and `script::run_with`, the same path
as the studio's script tab. That is the whole architecture: one session, every client a view of it.

## A reading order

A first pass takes a few evenings. Each step says what to look for.

1. **`crates/claudius/src/model.rs`.** The two documents. Notice what a `Part` stores (a style and a seed, not notes),
   that every `Sound` variant is audio, and `TrackMix.level_db` being relative to a role, not a fader.
2. **`crates/claudius/data/genres/house.json`.** What a genre knows: weighted choices, step probabilities, forms with
   flags, presets with `a|b` alternatives, the mix. Most musical changes are edits to files like this one.
3. **`compose.rs`, `generate_inner`, then `compose/arrange.rs`, `arrange`.** The creative step and the layout step.
   Notice the `rng.fork(...)` calls, the tempo and tonic drawn even when they are given, and why both matter
   ([Determinism](../engineering/architecture.md#determinism)).
4. **`theory/bass.rs`.** A short writer that shows the idea behind all of them: a one-bar motif, written once,
   transposed onto every chord. Then skim `melody.rs` for the lead's A A' A B phrasing.
5. **`render.rs`, `render()`.** The whole mix in about 130 lines: sources, the auto-mixer, the sidechain, buses,
   returns, the master. Then `finalize` for the loudness search.
6. **`judge.rs`.** What "good" means here, and the issue targets evolution reads.
7. **`pattern/mod.rs`, `query`; `pattern/mini.rs`; `script/mod.rs`; `script/song.rs`, `track()`.** How a mini-notation
   string becomes a tree, how a tree is queried by onset, the work and event budgets, the sandbox limits, and how a
   pattern becomes a track. `script/mix.rs` and `script/form.rs` hold the rest of the song functions.
8. **`script/print.rs` and `songs_print_as_scripts_that_change_nothing` in `tests/script.rs`.** The round trip, and the
   small concessions on the reading side that keep it exact ([Printing a song](../engineering/patterns.md#printing-a-song)).
9. **`evolve.rs`.** The 25 operators, how issues weight them, the (μ + λ) loop with previews.
10. **`crates/claudius-server/src/app/`, then `http.rs` and `mcp.rs`.** The session, its versions and jobs; the guard;
    the tools and their two backends.
11. **`crates/claudius-ui/src/app.rs` and `net.rs`.** How the studio holds back server state while you drag, and applies
    edits optimistically.
12. **`crates/claudius-packs/src/resolve.rs` and `player.rs`, and earshot's README.** Names to instruments, zones to
    sound; how the ears were ported and checked.

Read with claudius running. These make the code concrete:

```sh
echo 'song("house", 9);' | claudius script - --print          # a song's decisions, as a script
claudius make --genre house --seed 9 --short --no-ears          # the whole pipeline in ~10 s
CLAUDIUS_DEBUG=1 claudius make --genre house --short --no-ears  # with render timings
claudius serve --no-ears                                        # then: curl -s localhost:7878/api/state
cargo test -p claudius --test script                            # the language, end to end
```

## Conventions you will meet

- **Units.** Time in beats from the start of the song, four beats to a bar; pitch in MIDI semitones (60 is middle C);
  velocity 0 to 1; levels in dB; 48 kHz planar stereo `f32` everywhere. Names carry units when they are not these
  (`release_ms`, `ceiling_db`, `size_mb`).
- **Seeded randomness only.** Every random choice draws from `rng::Rng`, seeded from the song and forked per area, or
  from a hash (patterns, the pack player). Nothing reads the clock except to pick a seed when none is given.
- **Data over code.** Genres are JSON; effects are serde enums; a blueprint is plain data. New fields get serde
  defaults so older files keep loading.
- **Errors are for people.** Messages say what is wrong and what would work: `unknown sound 'bdd' (drums: bd kick sd
  ...)`, `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")`.
- **Comments state constraints**, not what the next line does: `// a noise-built tail reads as hiss above ~9 kHz: real
  rooms and plates are darker up there`. Doc comments are short lowercase phrases; module comments explain the module's
  job in prose.

[Code style and review](review.md) has the rest.

## Where to start contributing

Good first changes, roughly from least to most code:

- **An example or a fix in this book.** Every `js` block runs in CI, so examples stay true.
- **A genre tweak.** Change a weight, a template or a form in `data/genres/*.json`, look at the generator snapshot diff,
  and listen ([Making a change](first-change.md#a-genre-tweak)).
- **A progression style** in `theory/progression.rs`, or a bass or chord rhythm in `theory/`.
- **A script function**, such as a pattern method Strudel has and claudius lacks
  ([Making a change](first-change.md) walks through one).
- **A mutation operator** in `evolve.rs`.
- **A sample pack**, if you find a well-mapped, clearly licensed library ([Sample packs](../engineering/packs.md#adding-a-pack)).

Before anything larger, open an issue and describe what you want to change; the design records in
[Design decisions](../design/index.md) explain the choices you might want to revisit, and why they were made.
