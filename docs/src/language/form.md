# Form and sections

The form is the song's list of sections: an intro, a groove, a break, a build, a drop. Each section has a kind, a
length in bars, an energy, a progression, a drum variant, the tracks (layers) that play in it, and flags that ask the
arranger for builds, gaps, impacts, fills and filter sweeps. This page says exactly what each one does.

## `form()`

```js no-run
form([sec(...), sec(...), ...])
```

Replaces the whole form. Sections play in the order given. Two sections with the same name are renamed in order:
the second `drop` becomes `drop2`, the third `drop3`.

Every layer must be a track of the song, or one of the genre's optional parts and loops (which may be absent from a
song, and are silent then). `"*"` and a missing layer list mean every track that exists when `form()` runs; tracks
added later join by the [`track()` rule](song-functions.md#track). Every section's progression must exist, except
`B`: a section that plays `B` on a song without one plays `A`.

Errors: `form() takes sec(...) items`; `section 'drop' uses 'snare', which is not a track (tracks: kick clap hat ohat
shaker perc bass chords pad lead arp)`; `section 'drop' plays progression 'C', which the song does not have
(progressions: A B; prog() adds one)`.

```js
form([
  sec("intro", 8, "kick hat perc pad").filter_in(),
  sec("groove", 16, "kick clap hat ohat perc bass chords"),
  sec("break", 8, "pad chords lead"),
  sec("build", 8, "clap hat pad lead").gap(),
  sec("drop", 16, "*").impact(),
  sec("outro", 8, "kick hat perc bass"),
]);
```

## `sec()`

```js no-run
sec(kind, bars)            // every track
sec(kind, bars, layers)    // layers: "kick hat", ["kick", "hat"] or "*"
```

`bars` is an integer, clamped to 1..256. The kind is one of the words below; `breakdown`, `chorus` and `loop` are
accepted for `break`, `hook` and `groove`, and the section keeps the word you wrote as its name.

Each kind comes with defaults:

| kind | energy | progression | drum variant | flags on |
|---|---|---|---|---|
| `intro` | 0.4 | `A` | `sparse` | |
| `verse` | 0.65 | `A` | `main` | |
| `groove` | 0.65 | `A` | `main` | fills |
| `build` | 0.6 | `A` | `main` | build |
| `drop` | 0.95 | `A` | `main` | fills |
| `hook` | 0.95 | `A` | `main` | fills |
| `break` | 0.35 | `B` (`A` when there is no `B`) | `main` | |
| `outro` | 0.4 | `A` | `sparse` | |

The kind also matters to the arranger (drops and hooks get crashes every 8 bars, a final outro closes a filter) and to
the critic, which compares the loudness of drops and hooks with breaks.

Errors: `unknown section kind 'party' (intro verse build drop break hook groove outro)`; `layers are a string ("kick
hats") or an array of names`.

### Section methods

Each method returns a new section, so they chain: `sec("drop", 16, "*").energy(1.0).impact()`.

| method | sets | values |
|---|---|---|
| `.name(s)` | the section's name, used by `section()`, `layer()` and the studio | string |
| `.energy(x)` | the section's energy | number, clamped to 0..1 |
| `.prog(name)` | which progression it plays | any progression name |
| `.variant(name)` | which drum-lane variant it plays | `main`, `sparse`, `alt`, `half` ... |
| `.build()` | a build into the next section | also `.build(true)`, `.build(false)` |
| `.gap()` | a beat of silence at the end | also takes `true` / `false` |
| `.impact()` | a crash and an impact on the first beat | also takes `true` / `false` |
| `.fills()` | drum fills at 8-bar phrase ends | also takes `true` / `false` |
| `.filter_in()` | a lowpass opening across the section | also takes `true` / `false` |

`.energy()` raises `energy needs a number` for anything else. A variant that does not exist plays `main`. A
progression that does not exist is an error when `form()` gets the section, unless it is `B` (see above).

## What the flags do

These are the exact rules of the arranger (`compose::arrange`). Several act on **drum lanes** only (the lanes written
by the generator or by [`drum()`](song-functions.md#drum)); pattern tracks are never rewritten, so a song made only of
patterns gets the risers, sweeps and crashes but no automatic rolls or fills.

The arranger finds lanes by their [role](mixing.md#roles). The *fill lane* is the first lane whose role is `snare`
(a lane named `snare` or `sd`, or one that plays a snare sound), or `clap` when there is none; the *kick lane* is the
first lane whose role is `kick`.

### `build`

- The fill lane stops its pattern for the last part of the section and plays a snare roll instead: quarter notes,
  then eighths, sixteenths and thirty-seconds, with velocity rising from 0.4 to 1.0. The roll lasts half of the
  section's first 8 bars, between 1 and 4 bars: 4 bars in an 8-bar build, 2 in a 4-bar one. It is added even when the
  fill lane is not a layer of the section.
- The kick lane stops for the last bar.
- A riser plays over the last bars of the section: the genre's riser length (8 bars in house, techno, dnb and future
  bass; 4 in deep house and synthwave), or the whole section if it is shorter. Trap and lofi have no riser.
- A highpass filter on the `music` and `bass` buses sweeps from 10 Hz to 900 Hz across the section and snaps back at
  the next one.

### `gap`

In the section's last beat, every track except the riser and the reverse crash is silent: notes that start there are
removed and notes that ring into it are cut. Audio loops keep playing. A gap before a drop makes the downbeat land
harder.

### `impact`

- A crash on the first beat, at full velocity.
- An impact hit and a downlifter, in genres that use them: house, techno, dnb and future bass have both; deep house
  has only the downlifter, trap only the impact, synthwave and lofi neither.
- A reverse crash over the last 2 beats of the section *before*, in genres that use it (all but techno and lofi).

### `fills`

At the end of every 8-bar phrase, counted from the section's start, the fill lane's last 1 or 2 beats are replaced by
a fill: a sixteenth-note roll, triplets, or a broken groove, at 85 % velocity. At about a third of those phrase ends,
the kick drops out for the last half beat. A section shorter than 8 bars gets no fills, and a 12-bar section gets one,
at bar 8.

### `filter_in`

A lowpass filter on the `music` and `bass` buses opens from 350 Hz to 20 kHz across the section: the classic intro
that comes into focus. The drums are not filtered.

### Without a flag

- **Drops and hooks** get a crash every 8 bars after their first bar, at velocity 0.7.
- **A final outro** (the last section, of kind `outro`) closes a lowpass on the `music` and `bass` buses from 20 kHz,
  starting 40 % of the way in, down to 500 Hz at the end.
- **Every section** starts its patterns from their first cycle and its progression from its first chord (see
  [Patterns and time](patterns.md#sections-restart-their-patterns)).

## Energy

Energy is a number from 0 to 1 that says how intense a section is meant to be. It does not change the sound by itself;
what plays decides that. It is used in three places:

- **the critic** checks that sections get louder where energy rises (the correlation between energy and measured
  loudness) and that the loudest drop or hook is at least 3 LU louder than the quietest break;
- **new tracks** made by `track()` or `drum()` join the sections with energy 0.6 or more;
- **the generator**, when it builds a song around Apple Loops, layers the loops into sections by energy.

So set energies that match the layers: a break with three quiet tracks at 0.35, a full drop at 0.95.

## `section()`

```js no-run
section(which, options)   // which: a section name or a 0-based index
```

Edits one section. A name picks the first section with that name.

| field | type | effect |
|---|---|---|
| `bars` | number | length, clamped to 1..256 |
| `energy` | number | clamped to 0..1 |
| `prog`, `variant`, `name` | string | as the methods above |
| `layers` | string, array or `"*"` | replaces the layers |
| `build`, `gap`, `impact`, `fills`, `filter_in` | bool | the flags |

The section is checked as `form()` checks it: its layers must be tracks (or the genre's optional parts and loops) and
its progression must exist. The map takes only the fields above.

Errors: `no section 'nope'`; `no section 12`; `section() takes a name or an index`; `section 'drop' uses 'leed',
which is not a track (tracks: ...)`; `section 'break' plays progression 'Z', which the song does not have
(progressions: A B; prog() adds one)`; `section() has no option 'bar' (options: bars energy prog variant name layers
build gap impact fills filter_in)`.

```js
section("drop", #{ energy: 1.0, layers: "kick clap hat bass chords lead" });
section(0, #{ bars: 16, filter_in: true });
```

## `layer()`

```js no-run
layer(section, track, on)   // section: name or 0-based index; on: bool
```

Switches one track on or off in one section, without touching the others. The track must exist, or be one of the
genre's optional parts or loops.

Errors: `no section 'nope'`; `layer() takes a section name or index`; `no track 'leed' (tracks: ...)`.

```js
layer("break", "lead", false);
layer("intro", "shaker", true);
```
