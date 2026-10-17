# 0004 Rhai with Strudel-style patterns

Status: accepted

## Context

claudius needs a language in which a song can be written, read and edited, by people and by assistants:

- **Patterns.** Drum grooves, bass lines and chord rhythms are cyclic. Live coders have a compact, proven notation for
  them: TidalCycles' mini-notation, as Strudel uses it (`"bd*4, [~ cp]*2, hh*8?"`). Many musicians who code already
  know it, and assistants have seen a lot of it.
- **Song edits.** Tempo, key, progressions, parts, mix, form: the decisions in a blueprint, named as functions.
- **A sandbox.** Scripts come from assistants and from files people share. They must not read files, open sockets,
  run forever or exhaust memory.
- **Embedding.** The language runs inside the Rust engine, in the server's process, many times a minute.
- **Printing.** Every song must print as a script that runs back into itself ([0003](0003-blueprint.md)), so the
  language has to be able to state everything a blueprint holds.

The candidates were JavaScript (Strudel's own language, through QuickJS or V8), Lua, a language of claudius' own, and
JSON commands only.

## Decision

Scripts are [Rhai](https://rhai.rs), with Strudel-style mini-notation inside strings and pattern methods named after
Strudel's (`fast`, `every`, `sometimes`, `struct`, `off`, `superimpose` ...). One cycle is one bar. Song edits are plain
functions (`tempo()`, `prog()`, `drum()`, `part()`, `mix()`, `track()`, `form()`). Patterns are immutable values that
remember the code that built them, so a song's pattern tracks print exactly as written.

Rhai is written in Rust, embeds with a few lines, looks enough like JavaScript that assistants write it well, and has
the limits a sandbox needs built in (operations, call depth, string and array sizes, a progress callback for a time
limit, module resolution that can be switched off); it has no file or network functions unless the host registers
them. Patterns are claudius' own implementation (`pattern/`), not a binding to Strudel.

Rejected: JavaScript, because embedding a JS engine brings a large C or C++ dependency and makes the sandbox the
host's job, and because Strudel's semantics (per-event controls, fragments, its scheduler) do not map onto a
blueprint anyway. Lua, for similar embedding reasons and an unfamiliar syntax for most users. A language of our own,
because the general-purpose parts (variables, loops, closures, maps) would have to be built and documented. JSON
commands only, because nobody can read or write a song in them; they remain available as the HTTP API.

## Consequences

- Scripts are safe to run from anyone: the limits and the work budget for patterns are enforced by the engine, and
  fuzzed weekly ([Security](../engineering/security.md#scripts)).
- Strudel and Tidal users recognise most of it. What differs includes no polymeter (`{}` is a plain group), events
  returned by onset rather than as fragments, track settings (`.room()`, `.lpf()`) that apply to a whole track instead
  of per event, and one cycle per bar; [Coming from Strudel](../learn/producers/strudel.md) covers the rest.
- Closures print by calling them once on a placeholder pattern, so `|x| x.add(up)` prints as `|x| x.add(7)`: the value
  is kept, the variable name is lost.
- Rhai is stricter about types than JavaScript: `ply(1.5)` is `Function not found: ply (Pattern, f64)`, a common first
  error that [Errors and limits](../language/limits.md) explains.
- Rhai is slower than a JIT-compiled language, which does not matter for scripts that describe songs but rules out
  writing DSP in it.
- Maps are written `#{ k: v }`, which is Rhai's own and has to be learned.
