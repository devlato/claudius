# Patterns and time

A pattern is a description of events in time that claudius can ask questions of: "what happens in bar 3?" This page
says exactly how those questions are asked and answered, because everything else (alternation, `every`, `prog`,
randomness) follows from it.

## One cycle is one bar

Pattern time is counted in **cycles**, and a cycle is always one bar of four beats at the song's tempo. At 120 BPM a
cycle lasts 2 seconds; at 174 BPM, 1.38 seconds. There is no separate cycles-per-second setting: change the tempo
with [`tempo()`](song-functions.md#tempo) and the patterns follow.

Every duration in the pattern language is a fraction of a bar:

| duration | cycles | in mini-notation |
|---|---|---|
| a bar | 1 | `a` |
| a half note | 0.5 | `a b` |
| a beat (quarter note) | 0.25 | `a*4` |
| an eighth note | 0.125 | `a*8` |
| a sixteenth note | 0.0625 | `a*16` |
| an eighth-note triplet | 1/12 | `a*12` |
| a sixteenth-note triplet | 1/24 | `a*24` |

So `.late(0.25)` moves a pattern one beat later, and `.off(0.125, f)` adds a copy an eighth note behind.

## Events

A pattern produces **events**. Each event has a *whole*, the span it occupies (start and end, in cycles), a value
(`"bd"`, `"c3"`, `4`, `"Am7"`), and controls set by methods such as `.gain()` or `.legato()`. In `s("bd sd")`,
bar 1 has two events: `bd` from 0 to 0.5 and `sd` from 0.5 to 1.

## Queries

When claudius needs a pattern's events, it asks for a span of cycles, from *b* up to (not including) *e*, and gets
**the events whose start falls inside the span**. An event that started earlier and is still sounding is not
returned, and events are never cut into fragments.

This differs from Strudel, which returns fragments of events that overlap the span. You notice it in one place: an
event longer than a bar exists only in the bar where it starts. `note("<c3 g3>/2")` has an event in bars 1 and 3,
none in bars 2 and 4; each note still lasts its full two bars, so you hear it. Methods that read another pattern at
each event look further back, so they still find it (see
[Reading one pattern from another](#reading-one-pattern-from-another)).

## Sections restart their patterns

A pattern track plays in the sections that list it (see [Form](form.md)). For a section of *N* bars, claudius asks
the pattern for cycles 0 to *N* and places the result at the start of the section. Every section therefore starts
the pattern from its first cycle:

- an alternation `<a b c>` starts again from `a`;
- `.every(4, f)` applies `f` on the first bar of each section, then every fourth bar;
- `chord("prog")` follows the section's progression from its first chord;
- random choices repeat: bar 1 of every section makes the same choices.

```js
blank("house");
tempo(120);
track("lead", n("<0 2 4 6>").scale("A:minor").octave(4).s("pluck"));
form([sec("groove", 6, "lead"), sec("break", 4, "lead")]);
```

The groove plays A, C, E, G, A, C; the break starts again on A. A section whose length is not a multiple of the
pattern's length cuts the pattern off, so a 4-bar idea in a 6-bar section plays once and a half.

Drum lanes written with [`drum()`](song-functions.md#drum) work the same way: their step pattern is tiled from the
start of each section.

## From events to notes

When a section is arranged, each event becomes a note:

- **start**: the event's start, in beats from the section's first beat (cycle × 4);
- **length**: the event's whole length × `legato` (1 when not set), never shorter than 0.02 beats;
- **velocity**: `velocity`, else `gain`, else 0.9 for sound patterns and 0.8 for the others; anything above 1 plays
  as 1;
- **pitch**: from the value, as described in [Pitch](pitch.md).

Events shifted by `.late()` or `.early()` follow the same rule: `.late(0.25)` pulls the last beat of cycle −1 into
the first beat of every section, and drops the last beat of the section's final bar.

Swing and humanize are not part of the pattern. They are applied when the song is rendered, to every track: see
[`swing()`](song-functions.md#swing) and [`humanize()`](song-functions.md#humanize).

## What a pattern's values mean

The constructor decides the pattern's *kind*, and the kind travels with it through every method:

| constructor | kind | values are |
|---|---|---|
| `s()`, `sound()` | sound | sound names |
| `note()` | note | note names or MIDI numbers |
| `n()` | degree | scale degrees |
| `chord()` | chord | chord symbols, Roman numerals, `prog` |
| `seq()` | plain | numbers and words, for controls and structure |

`stack()` and `cat()` take the kind (and the track settings such as `.s()` and `.scale()`) of their first pattern.
Stack patterns of one kind: `stack([note("c3"), s("bd")])` reads `bd` as a note name and fails with
`cannot read 'bd' as a note`.

## Reading one pattern from another

`.struct(p)`, `.mask(p)` and the per-event controls (`.gain(p)`, `.velocity(p)`, `.legato(p)`, `.add(p)`,
`.transpose(p)`, `.n(p)`) read a second pattern at the start of each event: they take the event of `p` that is
sounding at that moment, including one that started in an earlier bar (up to 64 bars back).

```js
blank("house");
// each bar's root, played on the struct's hits
track("bass", note("<a1 f1 c2 g1>").struct("x ~ x x ~ x x ~").s("sub_bass"));
// a different gain on each beat
track("hats", s("hh*16").gain("0.9 0.4 0.7 0.4"));
```

A value that lasts longer than a bar holds for all of it. Both of these play A for two bars, then F for two:

```js
blank("house");
track("bass", note("<a1 f1>/2").struct("x ~ x ~").s("sub_bass"));
track("bass2", note("<a1!2 f1!2>").struct("x ~ x ~").s("sub_bass"));
```

## Randomness

Random methods (`?` and `|` in mini-notation, `.degrade()`, `.degradeBy()`, `.sometimes()` and its relatives) are
deterministic. Each decision comes from a hash of a seed and the start time of the event it is about, and the seed
comes from code:

| random part | seeded by |
|---|---|
| `?`, `\|` in a string | the text of the string |
| `.degrade()`, `.degradeBy(p)` | the code of the pattern before the call |
| `.sometimes(f)`, `.often(f)` ... | the code of the pattern before the call, and the method's name |
| `.sometimesBy(p, f)` | the code of the pattern before the call |

What follows from that:

- The same script renders the same song every time, on every machine.
- Different bars get different choices, and every section repeats the choices of its own first bars.
- Two tracks with the same code make the same choices. `s("hh*8?")` on two tracks drops the same hits.
- Editing the code *before* a random method changes its choices; editing after it does not. In
  `s("hh*8").degrade().gain(0.6)`, changing the gain keeps the same hits, changing `"hh*8"` to `"hh*8 "` does not.

To hear a different variation of the same idea, change the code before the random method without changing the
music: a space added inside the pattern string (`"hh*8 "`) is enough. `.degrade()` and `.degradeBy(0.5)` make
identical choices, since they share a seed.
