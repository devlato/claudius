# Coming from Strudel or TidalCycles

claudius borrows Strudel's mini-notation and many of its pattern methods, so most of your patterns carry over. The
model around them is different: a Strudel program is a live loop, and a claudius script describes a *song* (sections,
an arrangement, a mix) that is rendered and judged. This page lists every difference that changes what you write.

## The same groove in both

Strudel:

```text
setcpm(124/4)
stack(
  s("bd*4, [~ cp]*2, [~ hh]*4").bank("RolandTR909"),
  note("<a1 f1 d2 e2>").struct("~ x ~ x ~ x ~ [x x]").s("sawtooth").lpf(800),
  n("<0 2 4 6>").scale("A:minor").s("piano").room(0.5)
)
```

claudius:

```js packs
blank("house");
tempo(124); key("A minor");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4").bank("RolandTR909"));
track("bass",  note("<a1 f1 d2 e2>").struct("~ x ~ x ~ x ~ [x x]").s("saw_bass").lpf(800));
track("keys",  n("<0 2 4 6>").scale("A:minor").octave(4).s("piano").room(0.5));
```

Each pattern becomes a named track, the tempo is in BPM, and the sounds are claudius's: `saw_bass` is a synth preset,
`piano` a sampled grand, and the Strudel bank name `RolandTR909` is an alias of the `tr909` pack.

## Time

- **One cycle is one bar**, always. There is no `setcps` or `setcpm`; [`tempo(bpm)`](../../language/song-functions.md#tempo)
  sets the song's tempo, and a cycle lasts four beats of it.
- **Patterns play inside sections.** A track plays in the sections that list it, and every section starts the
  pattern from cycle 0: alternations, `every()` and random choices start over. There is no global clock running
  through the song.
- **Queries return whole events** whose onset falls in the span, never fragments. An event longer than a bar exists
  only in the bar where it starts, though it still sounds for its full length, and `.struct()`, `.mask()` and the
  controls still find it in the bars after (see [Patterns and time](../../language/patterns.md#queries)).

## Syntax

| Strudel (JavaScript) | claudius (Rhai) |
|---|---|
| `x => x.fast(2)` | `\|x\| x.fast(2)` |
| `stack(a, b, c)` | `stack([a, b, c])` |
| `cat(a, b)` | `cat([a, b])` |
| `silence` | `silence()` |
| `$: s("bd*4")` | `track("kick", s("bd*4"))` |
| `'single quotes'` | `"double quotes"` |
| `samples('github:user/repo')` | `claudius-packs add-map github:user/repo` on the command line |

## Mini-notation

Most of it is identical. The differences ([full list](../../language/mini-notation.md#differences-from-strudel)):

- `{ }` is a plain group: no polymeter, no `%`. A fractional `.fast()` runs a figure across bar lines instead
  (see [Polymeter](rhythm.md#idioms)).
- `.` is not a separator; it is an error.
- Weights inside `< >` are ignored: write `<a!3 b>` to hold `a` for three cycles.
- `*` and `/` take numbers, not patterns, above 0 and up to 256 (so do `fast()`, `slow()` and `ply()`).
- `a ! 3` is three `a`s.
- `|` and `,` at the same level group differently; bracket the choice: `[a | b], c`.

Euclidean rhythms are Strudel's: `bd(3,8)`, `bd(5,8,2)` and `.euclid(7, 16)` give the same steps in both. So do slash
chords and applied chords, which stay one word: `chord("<Am F/A C G>")`, `chord("<I V7/V>")`.

## Values and controls

- **`n()` is always scale degrees.** In Strudel, `n()` without `.scale()` is a sample index (or a note number). In
  claudius, `n("0 2 4")` without a scale uses the song's key, and the sample index is `bd:3` or the method
  `.n("0 3")` on an `s()` pattern.
- **`.transpose()` is always semitones; `.add()` adds to the value.** On `note()` both move semitones; on `n()`,
  `.add(2)` moves two scale degrees and `.transpose(12)` moves an octave. On `chord()`, `.add()` moves the voicing range
  and `.transpose()` the voiced notes (see
  [`add()` and `transpose()`](../../language/pattern-functions.md#add-and-transpose)).
- **`.gain()` is velocity.** It is how hard a note is played (0 to 1), not an amplitude multiplier: claudius levels
  every track to its mixing role, so the track's loudness is set with `.level(dB)` or [`mix()`](../../language/mixing.md#mix).
  `.velocity()` is the same control and wins when both are set.
- **Note names default to octave 3**, as in Strudel: `note("c")` is C3, MIDI 48.

## Effects are per track

Strudel effects are controls on each event; claudius effects belong to the track:

| Strudel | claudius |
|---|---|
| `.room(0.5)`, `.size(0.8)` | `.room(0.5)` is a send to the song's reverb, whose size the genre sets |
| `.delay(0.5)`, `.delaytime()`, `.delayfeedback()` | `.delay(0.5)` is a send to a tempo-synced delay set by the genre |
| `.pan(0.7)`, 0 to 1 | `.pan(0.4)`, −1 to 1 with 0 in the centre |
| `.lpf("<400 2000>")` | `.lpf(800)`, one number per track |
| `.crush(4)`, `.distort(0.5)` | `.crush(bits)`, `.distort(dB)` |
| `.orbit()`, `.duckorbit()` | `.duck(depth)`: sidechain from the kick |

A patterned effect argument is an error: `.lpf("<400 2000>")` raises `lpf needs a number`. Filter movement comes from
the arrangement instead (an intro's `filter_in`, a build's highpass sweep).

## Sounds

- `.s()` on a note pattern sets the track's instrument; it takes one name, not a pattern, so a note pattern cannot
  switch instruments from event to event. Use two tracks.
- `sawtooth`, `square`, `triangle` and `sine` are not sounds; use a [synth preset](../../language/sounds.md#synth-presets)
  (`saw_bass`, `pluck`, `square_lead`, `sub_bass` ...).
- Drum names are the usual `bd sd cp hh oh rd cr lt mt ht rim cb sh`. Without `.bank()` they play claudius's
  synthesized drum voices; with `.bank("tr909")` (or `RolandTR909`, `808`, `acoustic` ...) the recorded machines.
  A bank can also go in the word, `s("tr909:bd*4, tr808:cp*2")`, to mix machines in one pattern; a number after the
  last colon is the sample index (`tr808:sd:3`).
- A sound pattern with several names is split into one track per name when arranged (`drums.bd`, `drums.hh`), so each
  is mixed by its own role.

## Methods

Available, with the same meaning: `fast slow late early rev ply every lastOf sometimes often rarely almostNever
almostAlways sometimesBy off superimpose degrade degradeBy struct mask euclid euclidRot gain velocity legato add
transpose n s sound bank scale voicing arp`. Differences in detail:

- `ply()` and `every()` take integers; `.every(4, f)` is the first bar of every four, like Strudel's `firstOf`.
- `.arp()` takes a mode (`"up"`, `"down"`, `"updown"`), not an index pattern.
- `.voicing()` takes a MIDI range; voice leading is always on.
- Randomness is seeded by the code, not by the cycle alone: two tracks with the same pattern make the same choices.

Not available: continuous signals (`sine`, `saw`, `rand`, `perlin`, `range`, `segment`), `jux`, sample slicing
(`chop`, `striate`, `slice`, `splice`, `begin`, `end`, `speed`, `loopAt`, `fit`), `chunk`, `iter`, `palindrome`,
`echo`/`stut`, `when`, ADSR and filter-envelope controls, `vowel`, and the live-coding commands (`hush`, `setcps`).

## What claudius adds

- A song: [sections](../../language/form.md) with energies, builds, gaps, impacts and fills.
- Harmony the song knows about: [`prog()`](../../language/song-functions.md#prog), Roman numerals, `chord("prog")`,
  and `chord("prog").root()` for a bass line that follows it.
- Parts written by music theory, drum grammars and genre profiles ([`song()`](../../language/song-functions.md#song)).
- A mix: roles, target levels, sidechain, buses, a mastered result at a LUFS target.
- A critic that tells you what to fix, and evolution that tries fixes for you.
