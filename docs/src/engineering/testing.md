# Testing

claudius makes music, and most of what can go wrong with it is audible rather than a crash: a hat that moved, a song
that came out 3 dB too loud, a script that stopped printing back into itself. The tests are built around that. Some
check laws (a printed song changes nothing when run), some check measurements (a render lands within 1 LU of its
target), some freeze the generators so that a change to the music shows up as a readable diff in review.

## Running them

```sh
cargo xtask ci
```

runs what CI runs on every pull request: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked
-- -D warnings`, clippy again on the builds that leave features out (the studio without the ears, the engine and
server without the ears) and a check of the CLI without the studio window, so that code behind `cfg(feature = ...)`
keeps compiling, then the tests (with cargo-nextest when it is installed, `cargo test` otherwise), the doctests, and
the examples in the docs. The dev profile compiles the workspace at `opt-level = 2` and dependencies at 3, because many
tests render audio, so expect a long first build and fast test runs after it.

One kind at a time:

| command | what it runs |
|---|---|
| `cargo test -p claudius` | the engine's unit tests and every file in `crates/claudius/tests/` |
| `cargo test -p claudius --test script` | one of them (`pipeline`, `theory`, `script`, `docs_examples`, `pattern_laws`, `generators`) |
| `cargo test -p claudius-server` | the session, HTTP and MCP tests |
| `cargo test -p claudius-cli` | the binary end to end, and the `connect` unit test |
| `cargo test -p claudius-packs` | the packs' unit tests and the offline pack test |
| `cargo test -p claudius-packs -- --ignored` | the network test: real downloads |
| `cargo test -p claudius-decode` | the decoder |
| `cargo test -p claudius-ui` | the studio's tests: the native player, the mock, the script editor's highlighting and error lines, numeral labels |
| `cargo test -p earshot` | earshot's fast parity checks, no models |
| `cargo test -p earshot --release -- --ignored --nocapture` | earshot's model parity checks (downloads the models) |
| `cargo +nightly fuzz run script` | a fuzz target, from `fuzz/` |
| `cargo bench -p claudius` | the benchmarks |

## What each kind protects

### Unit tests

Every crate keeps small tests next to its code: the mini-notation parser in `pattern/mini.rs` (sequences, alternation,
weights, euclid, readable errors), velocity crossfades keeping constant power in `claudius-packs/src/model.rs`, name
resolution with banks and aliases in `resolve.rs`, SFZ inheritance, Strudel maps, WAV to FLAC being lossless,
`connect` editing a config file without losing other servers and refusing broken JSON, the decoder reading WAV at its
own rate and explaining what it cannot read. They pin down the behaviour of one function, so a failure points at it.

### `tests/pipeline.rs`

The engine end to end, on signals with known answers:

- every genre composes and arranges, every section's layers name real tracks (or the genre's optional parts), and a
  blueprint survives a round trip through JSON unchanged;
- a short house song renders with every sample finite, a true peak at most 0.05 dB over its ceiling, a loudness within
  1 LU of its target, and a verdict over 50;
- the beat tracker reads click tracks at 92, 124, 140 and 174 BPM within 1 % (half or double counts);
- chroma finds G major in I IV V I played as sine triads;
- loudness reads −3.0 LUFS (±0.2) for a −3 dBFS 1 kHz sine, the BS.1770 reference.

### `tests/theory.rs`

Keys parse and print; Roman numerals follow the key's mode (`VI` in A minor is F) and round-trip; chord symbols
round-trip; voicings stay in range, use only chord tones and move at most 24 semitones over three changes; Markov
progressions stay diatonic; euclidean rhythms spread their onsets; templates keep their certain hits; every bass style
lands on the chord's bass note at each change (except acid), and a lead stays in key and in range.

### `tests/script.rs`

The script language:

- patterns built from code have the right events, and keep their code, closures included;
- **the round trip**: songs in five genres print as scripts that, run on the same song, change nothing; and a
  pattern-built song does the same;
- pattern tracks play in their sections with exact note counts per sound, voiced in range, with the right roles;
- edits on a generated song do what they say, and a change of mode moves the progression's numerals;
- every example in the script reference (`script_reference.md`, which `claudius reference` prints and assistants
  read) runs;
- sampled instruments and drum machines resolve and render, when packs are installed (skipped otherwise);
- **the sandbox**: dense patterns, deep nesting, long strings, huge repeats and euclids, infinite loops, `import`,
  `eval`, songs over 512 bars or 20 minutes, and songs with more tracks than a render can hold all fail with the
  right message;
- errors say what to do (`unknown sound 'foo'` lists the synths, `cannot read key`, `no part 'nope'`).

### `tests/docs_examples.rs`

Every fenced `js` block in `docs/src` and in the README runs against a house song with seed 9, so the book cannot
drift from the language. The info string decides what is expected: `js` must succeed, `js error` must fail, `js packs` runs only when
packs are installed, `js no-run` is not run. Lines starting with `~` run but are hidden on the page. A block passes
when the script succeeds and, if it leaves a song, the song arranges into at least one track; it is not rendered.
A failure report starts with `<n> of <m> examples failed:` and gives, for each, `docs/src/<file>:<line>:`, the error
and the example's code, the line being that of the opening fence. [Writing docs](../contributing/docs.md#examples-are-tested)
explains how to write examples.

### `tests/pattern_laws.rs`

Properties that hold for any pattern, checked with proptest on 256 generated cases each, from mini-notation built out
of the pieces people write (`bd`, `~`, `cp:2`, numbers, `*2`, `/2`, `!`, `?`, `@2`, `(3,8)`, `(5,8,2)`, nested in
`[ ]`, `< >`, stacks and `[ ]*2`):

- the parser never panics, on any text up to 120 characters;
- events start inside the queried window, in order, with positive, finite lengths;
- querying `[0, 1)`, `[1, 1.5)` and `[1.5, 3)` gives the events of `[0, 3)`;
- `fast(k).slow(k)`, `late(t).early(t)` and `rev().rev()` give back the original;
- randomness (`sometimes`, `degradeBy`) gives the same events every time;
- `degradeBy(0)` keeps everything and `degradeBy(1)` nothing;
- a pattern's printed code evaluates to the same events.

When a case fails, proptest shrinks it to a small counterexample and saves it in `tests/pattern_laws.proptest-regressions`,
which is committed so the case keeps running.

### `tests/generators.rs`

A snapshot (with insta) of what each genre's generator writes for seed 42: tempo, key and swing, the progressions, every
section with its flags and layers, every drum variant as a step string, every part, the mix of every track. Numbers are
rounded, so floating-point differences between platforms cannot fail it, and the test points `CLAUDIUS_PACKS` at an
empty folder, so the snapshots describe the generators and not what is installed. A second test checks that the same
seed gives the same blueprint and the same project.

Any change to theory, genre profiles or the composer shows up here as a diff a reviewer can read: "the house hats
moved", "trap's progression changed". When the change is intended:

```sh
cargo test -p claudius --test generators     # fails and writes tests/snapshots/*.snap.new
cargo insta review                           # shows each diff; accept the intended ones
```

(`cargo install cargo-insta` provides the command.) Commit the updated `.snap` files with the change, and say in the
pull request how the music changed.

### `crates/claudius-server/tests/session.rs`

- a session makes a song, settles with a render over 60 s long and a verdict over 50, takes a script (tempo and solo),
  refuses a bad progression, and undoes and redoes back to the right tempo;
- a session limited to one folder refuses to load, save or play files outside it, including `inside/../escape`, and
  answers a missing file outside it exactly as an existing one, while a missing file inside is `does not exist`;
- the HTTP guards: security headers, 403 for a foreign `Host`, 415 for a command that is not JSON, 400 for an unknown
  command, 401 without the token, 200 with `?token=` (and a cookie) or a bearer header;
- MCP over HTTP: initialize, list the tools, make a song, get a tool error for an unknown sound, read the song back as a
  script;
- a save asked for while the new song's render runs waits for it, then writes the whole folder, stems included;
- the studio's accent and ghost thresholds (`claudius_api::{ACCENT, GHOST}`) equal the engine's step-string ones.

These tests run real renders with the library and the ears off, so they take tens of seconds.

### `crates/claudius-cli/tests/cli.rs`

The `claudius` binary as people and assistants run it, with `CLAUDIUS_PACKS` pointed at an empty folder so the result
is the same on every machine: the help names the commands, `genres` and `reference` print, a script from stdin prints
its song, a broken script fails with its reason on stderr, `connect print` prints valid JSON, `make` writes a song
folder with a verdict over 50, and MCP over stdio initialises, lists tools, runs a script and reads the song back.

### Packs, the decoder and earshot

`claudius-packs/tests/offline.rs` builds a pack of sine waves by hand (two velocity layers at two pitches, a drum with
two variants), installs nothing from the network, and checks resolution, layer selection, pitch, variants and
credits. A unit test in `install.rs`, `pack_names_stay_inside_the_packs_folder`, holds `remove` to plain pack names.
`tests/network.rs` is ignored by default: it installs `tr707` and a Strudel map into a temporary folder and plays
them, and checks that an opt-in pack refuses to install without consent.

earshot's `tests/parity.rs` compares the Rust port with the Python earshot on fixtures it wrote (`parity/fixtures/`,
made by `parity/make_fixtures.py`): decoding, resampling, the mel front end, CLAP embeddings (1 − cosine within 3e-10),
calibrated z-scores, AudioSet probabilities, realness, measurements, beat grids and search. The model checks are
ignored by default because they need the models; inputs that are Apple Loops are skipped where they are not
installed. [earshot's README](https://github.com/devlato/claudius/blob/main/crates/earshot/README.md#parity-with-the-python-earshot)
lists every tolerance.

## Fuzzing

`fuzz/` holds two [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets, in a workspace of their own because
libFuzzer needs nightly:

| target | feeds | must hold |
|---|---|---|
| `mini_notation` | any UTF-8 text to `mini::parse`, then queries what parses over `[0, 4)` and `[−1.5, 0.25)` | no panic, no hang, no runaway query |
| `script` | any UTF-8 text to `script::run` | no panic, no hang, no memory exhaustion |

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run mini_notation -- -max_total_time=300
```

CI runs each target for five minutes every Saturday, keeping the corpus in the Actions cache and uploading any crash.
A crash leaves its input in `fuzz/artifacts/<target>/`: turn it into a regular test before fixing the bug, so it stays
fixed. The sandbox limits in [Patterns and the script engine](patterns.md#the-engine-and-its-sandbox) exist because
fuzzing finds the gaps first.

## Benchmarks

`crates/claudius/benches/engine.rs` (criterion) times generating and arranging a house song, rendering 8 bars,
querying a dense pattern for 64 bars, running a script edit and printing a song. Results go to `target/criterion`,
with comparisons against the previous run. CI only checks that the benchmarks build.

## The rest of CI

The tests run on Linux and macOS, and on Windows without the ears as an experimental job that reports without
blocking. Besides them, every pull request checks: formatting; clippy on the workspace, on the studio for
`wasm32-unknown-unknown`, and on the builds that leave features out; that the Intel Mac release target compiles;
that the web studio builds with trunk; that the workspace builds on the minimum supported Rust version
(`rust-version` in `Cargo.toml`); `cargo deny` (licences, advisories, sources); spelling with typos; that the book
builds and its examples run; that the benchmarks build; the workflows and install scripts pass actionlint, shellcheck
and PSScriptAnalyzer; and the website builds, its internal links resolve (lychee, with the book at `/docs/`), and it
passes its smoke test. The only required check is `ci ok`, which fails when any of them failed or was cancelled.
[Development setup](../contributing/setup.md) covers installing the tools.

## Tests and the machine they run on

Installed packs, Apple Loops and models change what claudius does, so the tests decide what they see:

- generator snapshots and the CLI tests point `CLAUDIUS_PACKS` at an empty folder;
- tests that need packs skip without them (`sampled_instruments_and_drum_machines`, `js packs` examples);
- the engine's tests pass no Apple Loops library to `generate`, and the session tests run with `library: false`;
- no test loads earshot's models except earshot's ignored parity checks.

Tests that render with packs installed are slower but check the same things. CI has no packs, no Apple Loops and no
models, so whatever passes there passes on a clean machine.

## Adding a test

- **A bug** gets a test that fails before the fix, in the file of the layer where it lives: a parser bug in
  `pattern/mini.rs`, a script function in `tests/script.rs`, a session behaviour in `tests/session.rs`.
- **A new script function** needs a case in `tests/script.rs`, an example in the language reference (a `js` block,
  which `docs_examples` runs), and, when it changes what a song stores, a check that the song still prints back into
  itself. [Making a change](../contributing/first-change.md) walks through one.
- **A change to the music** (theory, a genre, the composer) updates the generator snapshots; review the diff as music
  and describe it in the pull request.
- **A new pattern method** with a law (an inverse, an identity) belongs in `tests/pattern_laws.rs`, and a new
  mini-notation feature in the strategy that generates sources there.
- **A new HTTP route or command** gets a case in `tests/session.rs`, through `http::router` with `tower::ServiceExt`
  and no network.
- **Pack handling** is tested with a hand-written pack in a temporary folder, as `tests/offline.rs` does, never with
  downloads in the default test run.
