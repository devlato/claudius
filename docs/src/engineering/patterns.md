# Patterns and the script engine

Scripts are [Rhai](https://rhai.rs) with two families of functions: patterns in Strudel-style mini-notation, and
song edits. This page is about how they work inside: what a pattern is and how it is queried, the budgets that keep
a pattern from running away, the sandbox, and how a song prints as a script that runs back into itself. The
[language reference](../language/overview.md) describes what they do for the person writing them.

| file | what it holds |
|---|---|
| `pattern/mod.rs` | the pattern tree (`Node`), queries, the work budget, `Pattern` with its code |
| `pattern/mini.rs` | the mini-notation tokenizer and parser, and its limits |
| `pattern/notes.rs` | from pattern events to notes: sounds, note names, scale degrees, chords and voicing |
| `script/mod.rs` | the engine: `run`, `eval_pattern`, the sandbox limits |
| `script/patterns.rs` | the pattern functions and methods registered with Rhai |
| `script/song.rs` | starting a song (`song`, `blank`), its settings, `prog`, `bars_per_chord`, `drum` and `track` |
| `script/mix.rs` | `part`, `drum_sound`, `mix`, `mute` and `solo`, `remove`, `master` |
| `script/form.rs` | `sec`, `form`, `section`, `layer`, and the checks on sections |
| `script/sounds.rs` | sound names: drum voices, presets, pack instruments, files, and the role each gets |
| `script/steps.rs` | drum step strings, both ways |
| `script/print.rs` | `to_script`: a song as a script |

## A pattern is a tree

`mini::parse` turns a string into a `Node`, and methods wrap nodes in more nodes:

| node | meaning |
|---|---|
| `Silence`, `Atom(value)` | nothing; one event per cycle with a word or a number |
| `Seq([(node, weight)])` | each cycle divided among the children by weight (`a b`, `a@3 b`) |
| `Stack([node])` | children at once (`a, b`) |
| `Alt([node])` | one child per cycle, in turn (`<a b c>`) |
| `Fast(node, factor)`, `Late(node, cycles)`, `Rev(node)` | time scaled, shifted, reversed within each cycle |
| `Every(n, offset, node, other)` | `other` on cycles where `cycle % n == offset` (`every`, `lastOf`) |
| `Degrade(node, p, seed)`, `Choose([node], seed)` | events dropped at random (`?`, `degradeBy`); one child picked at random per cycle (`a \| b`) |
| `Ply(node, k)` | every event split into `k` |
| `Struct(values, structure)`, `Mask(node, mask)` | timing from one pattern and values from another; events kept where the mask is true |
| `Ctrl(node, key, param)` | a per-event control (`gain`, `velocity`, `legato`, `add`, `transpose`, sample index) read from another pattern |
| `Sometimes(node, other, p, seed)`, `Keep(node, p, seed)` | the events a coin leaves alone, plus `other`, the transform of the events it picked; `Keep` is what hands the transform only the picked events |

A `Pattern` is a node behind an `Arc`, plus its `Kind` (what its values mean: sounds, notes, scale degrees, chords or
plain values), the track settings set on it (`.s()`, `.bank()`, `.room()` ...) and `repr`, the script code that built
it. Patterns are immutable: every method returns a new one.

## Queries

`query(node, b, e, out)` appends the events of `node` **whose onset lies in `[b, e)`**, in cycles, one cycle being
one bar. An event (`Ev`) is a whole span `[b, e)`, a value and its controls. The nodes compose by transforming the
query window on the way down and the events on the way back up:

- `Seq` visits each cycle the window touches, gives each child its slice of the cycle, queries the child in the
  child's own time (scaled so that the slice is one cycle), and maps the events back.
- `Alt` with `k` children sends cycle `c` to child `c % k`, as that child's cycle `c / k`. So `<a <b c>>` plays
  `a b a c`: the inner alternation advances only on its own turns.
- `Fast(f)` queries `[b·f, e·f)` and divides the events' times by `f`; `Late(t)` queries `[b − t, e − t)` and adds
  `t`.
- `Rev` queries each cycle whole and reflects every event inside it, so an event that ended at the end of the cycle
  starts at its beginning.
- `Struct` takes the true events of the structure and, for each, the value the values pattern has *sounding* at that
  onset (`sample_at`, the last event whose span contains it). `Mask` and `Ctrl` read the other pattern the same way.
- `sometimes(f)` builds `Sometimes(node, f(Keep(node)), p, seed)`. `Keep` passes the events whose hash is below `p`;
  `Sometimes` returns the original events whose hash is at least `p` and every event of the transformed copy. A
  transform that keeps time (`gain`, `add`, `ply`) therefore gets exactly the events the original side dropped. One
  that moves time (`fast`, `late`) asks `Keep` in its own time, so its picks follow that time instead.

Two properties follow, and `tests/pattern_laws.rs` checks them with generated patterns:

- **Windows do not matter.** Querying `[0, 1)`, `[1, 1.5)` and `[1.5, 3)` gives the same events as querying `[0, 3)`.
  Nothing is cut at a window edge, because only onsets count; an event that started before the window is not
  returned at all. This is where claudius differs from Strudel, which returns fragments of every event overlapping the
  window ([Patterns and time](../language/patterns.md) shows where you hear the difference).
- **Randomness is stable.** `Degrade`, `Choose`, `Sometimes` and `Keep` decide with `hash01(seed, onset)`, a hash of
  the event's onset time and a seed, instead of a random stream, so the same pattern gives the same events in any
  window, in any order, every time. Seeds come from the code: the parser seeds a small generator with a hash of the
  mini-notation string, and `sometimes` and `degrade` hash the pattern's `repr`. Two tracks with identical code make
  identical choices; change one character and they differ.

`Pattern::try_events(b, e)` runs a query and sorts the events by onset. Each track is placed section by section with
`notes::place`, which queries cycles `0` to the section's length and puts them at the section's start: every section
plays its patterns from their first cycle.

## The work budget

A query is recursive, and a few characters of mini-notation could ask for a lot. Three limits keep it in reach,
each the same on every machine and in every test run, since none of them is a time:

- **Speed.** `*`, `/`, `fast`, `slow` and `ply` take a factor above 0 and up to `MAX_SPEED` (256), checked when the
  pattern is built, so `s("bd*100000000")` fails while it parses:
  `'*' takes a number above 0 and up to 256, not 100000000`.
- **Work.** Every call to `query`, every cycle a node visits and every sub-event `Ply` makes spends one unit of a
  thread-local budget, `MAX_WORK` (4,000,000) per `try_events` call. A long sequence sped up a lot runs it out, such
  as a thousand rests and a `bd` played `*256`.
- **Events.** Every event a query makes (an atom's, a `Ply` copy, a `Struct` value) counts against a second budget,
  `MAX_EVENTS` (262,144), and running out also ends the work. This is the one that bounds memory, since the events
  gathered are kept until the query returns: speed-ups inside speed-ups, such as `s("[bd*256]*256")` at 65,536 hits
  a bar, stop here.

When a budget runs out, the query returns what it has, `try_events` notices, and the call fails, naming the event
budget first:

```text
track 'x': this pattern is too dense to play: it makes more than 262144 events
track 'x': this pattern is too dense to play: it needs more than 4000000 steps of work
```

Both are far beyond anything musical: a dense 64-bar drum pattern needs thousands of events. The worst case the limits
allow, `s("[bd*256]*256")`, stops at the event budget with the whole `claudius script` process at about 134 MB, and the
parse-time refusal of `s("bd*100000000")` at 33 MB. `Pattern::events` is the same query returning nothing when a budget
runs out, for callers that only look (such as listing a pattern's sound names).

`track()` adds a second limit on the result: it places 64 bars of the pattern, and more than `MAX_NOTES_PER_BAR`
(1,024) notes per bar on average fails with `track 'x' plays N notes per bar; the limit is 1024`.

## Mini-notation

`pattern/mini.rs` is a tokenizer and a recursive-descent parser. The grammar, as it is implemented, is written out in
[Mini-notation](../language/mini-notation.md); the parts that matter to someone changing it:

- **Tokens.** Words take letters, digits and `# : . _ ' - ^`; a number followed by letters is a word (`808`, `3am`).
  `~` is a rest, and so is a `-` not followed by a digit; `_` on its own lengthens the previous step.
- **Structure.** `stack` handles `,` (layers) and `|` (random choice) around `seq`, which handles steps, `_` and `!`.
  `step` reads one atom or group (`[ ]`, `< >`, and `{ }`, which is parsed exactly like `[ ]`), then its modifiers:
  `*`, `/`, `@`, `!` (with or without a count), `?` (with or without a probability) and `(k,n,r)`.
- **Limits.** The source is at most 4,096 characters (`MAX_SOURCE`, counted in characters, not bytes), brackets nest
  at most 64 deep (`MAX_DEPTH`), a step repeats at most 256 times and a euclidean rhythm has at most 256 steps
  (`MAX_STEPS`), `*` and `/` take factors up to 256 (`MAX_SPEED`, shared with `fast`, `slow` and `ply`), and a
  sequence has at most 1,024 steps. Each limit has its own error message, because a fuzzer finds these first.
- **Errors** say what was expected and where: `expected ']', found the end`, `unexpected ')'`, `'/' takes a number
  above 0 and up to 256, not 0`.

The `mini_notation` fuzz target feeds arbitrary text to `parse` and queries whatever parses; the `script` target
does the same for whole scripts. See [Testing](testing.md#fuzzing).

## From events to notes

`notes::place` turns a pattern's events into notes on the song's timeline, by the pattern's kind:

| kind | made by | an event becomes |
|---|---|---|
| sound | `s()`, `sound()` | a note at pitch 60 played by the sound named in the event; `bd:3` or `.n()` picks a variation |
| note | `note()` | a pitch from a note name (`c3`, `eb4`, octave 3 when left out) or a MIDI number |
| degree | `n()` | a scale degree in `.scale()` (default: the song's key) and `.octave()` (default 3) |
| chord | `chord()` | a chord from a symbol (`Am7`), a numeral (`VI`), or `prog` / `prog:B` (the song's progression at that bar of the section), voiced with voice leading in `.voicing(lo, hi)` (MIDI 52 to 74 by default), arpeggiated across the event with `.arp()`, or reduced to its bass note with `.root()` |
| plain | `seq()` | numbers, read as MIDI pitches when the pattern plays |

Start and length come from the event's span, four beats per cycle; `legato` scales the length (never below 0.02
beats); `velocity` or `gain` set the velocity (0.9 for sounds, 0.8 otherwise); `add` adds semitones to notes and scale
steps to degrees, and `transpose` always adds semitones. A word that is not a note, a degree or a chord is an error
naming the word, except a `prog` chord when the progression has no chord there.

`track(name, pattern)` places 64 bars to learn which sounds the pattern uses, resolves each label once
(`script::resolve_sound`: a `.bank()` kit, a drum voice, a synth preset, a pack instrument, an Apple Loops cut, or a
file), and stores the code with the resolved sounds as a `PatternTrack`. The track's role comes from `.role()`, or
from its one sound (`bd` is a kick, `808` a bass), or from its kind and median pitch (chords are chords, anything with
a median under MIDI 52 is a bass, the rest a lead). [The song model](song-model.md#loops-and-pattern-tracks) covers
how the arrangement plays it.

## The engine and its sandbox

`script::run(code, blueprint, library)` builds a fresh Rhai `Engine` for every run, registers the pattern functions
and the song functions (`song::register_song`, which also registers those of `mix.rs` and `form.rs`), and runs the
code. `run_with` is the same with the folders a shared session may read, which the server passes. The song functions
share an `Rc<RefCell<Ctx>>` holding the blueprint being edited, the Apple Loops library, the log and those folders;
each edit borrows it, changes the blueprint, and returns. A script that
edits without starting a song gets a blank house song first. When the script ends, `run` checks the song's size and
returns the new blueprint and the log. When it fails, nothing is returned but the error, and the caller still has the
old blueprint: the session swaps blueprints only on success.

Scripts come from people, from assistants and from files, so the engine assumes nothing about them:

| limit | value | set by |
|---|---|---|
| operations | 5,000,000 | `set_max_operations` |
| running time | 10 s | an `on_progress` callback that checks the clock |
| call depth | 64 | `set_max_call_levels` |
| expression depth | 128, 64 inside functions | `set_max_expr_depths` |
| string, array, map size | 200,000 characters, 10,000 items, 1,000 entries | `set_max_string_size` and friends |
| modules | none | a `DummyModuleResolver`, so `import` fails |
| `eval` | disabled | `disable_symbol("eval")` |
| files | none, except audio named in `.s()` | no file functions are registered; `resolve_sound` checks paths with `config::read_error` against the session's folders |

`print()` and `debug()` go to the log, not to stdout, which in `claudius mcp` carries the protocol. The same limits
apply to `eval_pattern`, which evaluates a pattern track's code with only the pattern functions registered every time
the song is arranged.

## How a pattern remembers its code

Every constructor and method appends to `repr`: `s("bd*4")`, then `.fast(2)`, then `.gain("1 0.7")`. Strings are
re-quoted with escapes, numbers printed by `fmt_num` (integers plainly, other numbers with at most four decimals).
Methods that take a closure (`every`, `lastOf`, `sometimes` and its relatives, `off`, `superimpose`) need the
closure's code too, which Rhai does not give back. So `apply_fn` calls the closure twice: once with the real pattern,
to get the result, and once with a placeholder pattern whose `repr` is `x`, to learn what the closure does to it.
The second call returns, say, `x.add(7)`, and the method prints `|x| x.add(7)`:

```js
~blank("house");
let up = 7;
track("lead", n("0 2 4").scale("A:minor").superimpose(|x| x.add(up)).sometimes(|x| x.fast(2)));
```

prints as

```text
track("lead", n("0 2 4").scale("A:minor").superimpose(|x| x.add(7)).sometimes(|x| x.fast(2)));
```

with the variable replaced by the value it had. A closure that inspects its argument instead of transforming it would
print what it did to the placeholder, but no pattern method gives a closure anything to inspect.

## Printing a song

`script::to_script(blueprint)` writes, in order: a header naming the title, genre and seed; `title`, `tempo`, `key`
and `swing`; `humanize` when it is not 0; every progression; `bars_per_chord` when it is not 1; every variant of every
drum lane as a step string; every generated part as a `part()` line with its style, preset, register, density and
seed; every pattern track's code as it was written; the form, one `sec()` per section with the methods that differ from
that kind's defaults; and a `mix()` line for every track whose mix differs from the mix it would have without one.
[Songs as scripts](../language/printing.md) shows the output.

The guarantee is that **running the printed script on the same song changes nothing**. Each part of the printer has a
partner on the reading side that keeps it true:

- **Numbers.** `f32s` and `f64s` print integers plainly and everything else with Rust's shortest representation that
  reads back as the same value, so `swing(0.5257842)` is exact.
- **Drum steps.** A step string can only say `X`, `x`, `o` or a digit, so it rounds velocities: `hits_to_steps`
  writes `X` from 0.85 (`STEP_ACCENT`), `o` below 0.5 (`STEP_GHOST`) and `x` between, and reading gives back 1.0, 0.8
  and 0.45 (a digit is tenths). The studio's accent and ghost marks use the same two numbers, as
  `claudius_api::{ACCENT, GHOST}`. `drum()` therefore compares the string it is given with the string the lane's
  current hits would print as, and keeps the exact hits when they match. The resolution is 16 steps per bar when
  every hit is on a sixteenth, else 32, else 48, printed after the string.
- **Parts.** `part()` with the preset the part already has keeps its patch, so the generator's variation of a preset
  and evolution's tweaks survive.
- **Sections.** `sec()` fills in the same defaults the printer leaves out (energy by kind, progression `B` for
  breaks, the `sparse` variant for intros and outros, `build` for builds, `fills` for drops, grooves and hooks), and
  `"*"` stands for every track exactly when a section has all of them.
- **Seeds.** A part's `u64` seed prints as a signed 64-bit integer, Rhai's integer type, and reads back as the same
  bits.
- **Pattern tracks.** The code is printed verbatim and resolves to the same sounds in the same environment.

The tests hold the printer to it: `songs_print_as_scripts_that_change_nothing` generates songs in five genres and
compares the blueprint before and after running its script, field by field, and
`printed_patterns_read_back_the_same` does the same for generated patterns. A change to the printer or to a song
function that breaks the guarantee fails them.

What a script cannot say is not printed: drum voices and kits, synth patches beyond the preset name, Apple Loops and
flipped samples, effect chains, the master, the transitions. Running the script on another song leaves those as that
song has them. [Songs as scripts](../language/printing.md#what-is-not-printed) lists them for users.
