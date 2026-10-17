# Code style and review

The short version: run `cargo xtask ci` before you push, make the change do one thing, and write code that reads like
the code around it. The rest of this page is what that means here, and what a reviewer looks at.

## Formatting and lints

- **rustfmt** with the repository's `rustfmt.toml`: `max_width = 120` and `use_small_heuristics = "Max"`, so short
  expressions stay on one line and lines may run to 120 columns. `cargo fmt --all` formats everything; CI checks it.
- **clippy** must be clean with `-D warnings` on every target: `cargo clippy --workspace --all-targets --locked -- -D
  warnings`. The studio is also checked for `wasm32-unknown-unknown`, since native builds never compile its web-only
  code, and the builds without the ears or without the studio window are checked too, since code behind
  `cfg(feature = ...)` compiles only there. `cargo xtask ci` runs all of these except the web one, which is
  `cargo clippy -p claudius-ui --target wasm32-unknown-unknown --locked -- -D warnings`.
- The engine allows four lints crate-wide, in `crates/claudius/src/lib.rs`: `needless_range_loop`,
  `too_many_arguments`, `type_complexity` and `should_implement_trait`. DSP code reads best as index loops over
  parallel buffers, and processors take their full parameter lists. Do not add allows elsewhere without a comment that
  says why the lint is wrong in that place.
- **typos** checks spelling in code, comments and docs. Domain words it does not know (`caf`, `thr`) go in
  `_typos.toml` with a comment.
- Edition 2024 and the minimum Rust version in `Cargo.toml` (`rust-version`). Let-chains (`if let ... && let ...`) are
  used throughout; use them where they read better than nesting.

## How the code reads

- **Like its neighbours.** Match the naming, the error style and the density of the module you are in. A new function
  in `theory/` looks like the functions already there.
- **Comments state what the code cannot.** A constraint, a reason, a unit, a source: `// a noise-built tail reads as
  hiss above ~9 kHz: real rooms and plates are darker up there`, or `// SAFETY: set once, before any thread of this test
  binary reads the environment`. Not what the next line does, not the history of the change, not why your version is
  right; that belongs in the pull request.
- **Doc comments** are short lowercase phrases that say what a thing is or returns: `/// the sound for a genre's preset
  entry: alternatives separated by |, the first that exists wins`. Module comments (`//!`) explain the module's job in a
  paragraph, so a reader knows whether they are in the right file.
- **Units** are in the name when they are not the defaults (beats, MIDI pitch, dB, 48 kHz): `attack_ms`, `size_mb`,
  `lufs`.
- **Errors are written for the person who will read them.** Say what was wrong, show the value, and say what would
  work: `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")`. Script functions return `String` errors
  that surface verbatim in the studio and in MCP replies; binaries use `anyhow` with context. Nothing panics on user
  input, a file's contents or a network answer.
- **Randomness is seeded.** Draw from `rng::Rng`, seeded from the song, forked per area with a label; patterns and the
  pack player use hashes of event positions. Never a thread-local generator or the clock.
- **Data over code.** Musical knowledge goes in genre files, effects are serde data, and new fields in serialised types
  get `#[serde(default)]`, so older blueprints, genre files and API clients keep working.

## What reviewers check

Every pull request is read for these. Pre-empting them in the description saves a round trip.

1. **One thing, explained.** The change does one thing, and the description says why, not only what.
2. **Tests.** New behaviour is tested; a bug fix comes with a test that failed before it. Music-facing changes show
   their effect: a snapshot diff, a verdict, or a render you listened to and describe.
3. **Docs.** User-visible behaviour changes the book in the same pull request, `js` examples included, and
   `script_reference.md` when the script language changes, since assistants read it.
4. **Determinism.** The same seed still gives the same song, or the snapshot diff shows exactly what changed and the
   description says it is intended.
5. **The round trip.** Anything new that a blueprint stores and a script can say prints back into the same blueprint
   (`script/print.rs`, and the tests in `tests/script.rs`).
6. **The sandbox and the file policy.** New script functions do no I/O beyond what `resolve_sound` already does; new
   commands that take paths check them with `config::path_allowed` or `config::write_allowed`; nothing new listens on
   the network or skips the guard ([Security model](../engineering/security.md)).
7. **Error messages.** Read the failure path: is the message one a person can act on?
8. **Performance.** Rendering and pattern queries are hot paths: no allocation per sample, no lock held across a render.
   For changes there, `cargo bench -p claudius` before and after.
9. **Compatibility.** The HTTP API's types and the blueprint format stay readable by older clients and files; a
   breaking change is marked `!` in the title and explained.
10. **Dependencies.** A new crate needs a reason, a licence on `deny.toml`'s list (permissive, plus MPL-2.0), and
    preferably no C. Sample data needs a licence that fits [0007](../design/0007-sample-licensing.md).
11. **The title.** A Conventional Commit that makes sense as a changelog line to someone who uses claudius.

## Reviewing

Review the change, not the person: say what you would do instead and why, and separate what must change from what you
would merely prefer. Approve when the checklist holds; small follow-ups can be issues. A maintainer merges with squash,
using the pull request's title, once CI's `ci ok` passes.

When you review a pull request written with an AI agent, review it like any other: the author is responsible for it,
and [Working with AI agents](agents.md) describes what is expected.
