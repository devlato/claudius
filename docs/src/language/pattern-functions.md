# Pattern functions

Every function that makes or changes a pattern, with its parameters, what it accepts, the errors it raises and an
example. Methods are called on a pattern (`s("bd*4").fast(2)`) and return a new pattern; the original is unchanged.

Many parameters accept a *number, pattern string or pattern*: `gain(0.8)`, `gain("1 0.6")` and
`gain(seq("1 0.6"))` are the same kind of argument. Anything else raises
`expected a number, a pattern string or a pattern`. How a second pattern is read is explained in
[Reading one pattern from another](patterns.md#reading-one-pattern-from-another).

## Constructors

### `s()` and `sound()`

```js no-run
s(pattern)        // number, pattern string or pattern
sound(pattern)    // the same
```

A pattern of sound names: drums, synth presets, sampled instruments, kit sounds such as `tr808:sd` (see
[Sounds](sounds.md)). Each word is one hit; a number after the last colon picks a variation (`bd:3`,
`tr808:sd:2`). A track whose sound pattern uses several names is split into one track per name (`drums.bd`,
`drums.hh` ...) so each gets its own [mixing role](mixing.md#roles).

```js
blank("house");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
```

### `note()`

```js no-run
note(pattern)     // note names or MIDI numbers
```

A pattern of pitches: `c3`, `eb2`, `f#4`, or MIDI numbers (`60` is middle C). A name without an octave is in octave
3. The instrument comes from `.s()`; without it, the `pluck` synth plays. See [Pitch](pitch.md#note-names).

Errors: a word that is not a note name, when the track is made: `cannot read 'h3' as a note (names with an octave
like c3, f#4, bb2, or MIDI numbers)`.

```js
blank("house");
track("bass", note("<a1 f1 c2 g1>").struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
```

### `n()`

```js no-run
n(pattern)        // scale degrees, from 0
```

A pattern of scale degrees in the scale set by `.scale()`, or the song's key when there is none. Degree 0 is the
tonic in octave 3 (`.octave()` changes it); 7 in a seven-note scale is the tonic an octave up; negative degrees go
down. Without `.s()`, the `pluck` synth plays. See [Pitch](pitch.md#scale-degrees).

Errors: a word instead of a number, when the track is made: `n() takes scale degrees (0 1 2 -1 ...), not 'e'`.

```js
blank("house");
key("A minor");
track("lead", n("0 2 4 7 4 2").octave(4).s("pluck"));
```

### `chord()`

```js no-run
chord(pattern)    // chord symbols, Roman numerals, prog, prog:NAME
```

A pattern of chords: names (`Am7`), slash chords (`F/A`), Roman numerals (`ii7`, `V7/V`). Each event plays every
note of its chord, voiced with voice leading (see [Voicing](pitch.md#voicing)), or only its root with
[`.root()`](#track-settings). `prog` plays the song's progression for the section; `prog:B` always plays
progression `B` (or `A` when the song has no `B`). Without `.s()`, the `epiano` synth plays.

Errors: a word that is neither, when the track is made: `cannot read 'Xyz' as a chord (names like Am7, Fmaj7, Bb,
numerals like ii7, bVII, V7/V, or prog)`.

```js
blank("house");
key("A minor"); prog("A", "Am7 Fmaj7 Dm7 Em7"); bars_per_chord(1);
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano"));
```

### `seq()`

```js no-run
seq(pattern)      // plain values
```

A pattern of plain numbers and words, for arguments: `.gain(seq("1 0.6"))`, `.struct(seq("x ~ x x"))`. A string
argument does the same job, so `seq()` is mostly useful for building an argument once and reusing it.

### `stack()`

```js no-run
stack([pattern, pattern, ...])
```

Plays patterns together. The result has the kind and track settings of the first pattern, so stack patterns of one
kind.

Errors: `stack() takes patterns` when an item is not a pattern; `stack() needs at least one pattern` for `[]`.

```js
blank("house");
track("drums", stack([s("bd*4"), s("[~ cp]*2"), s("hh*8").gain(0.5)]));
```

### `cat()`

```js no-run
cat([pattern, pattern, ...])
```

One pattern per bar, in turn: bar 1 plays the first, bar 2 the second, and so on, then again. Like `stack()`, it
takes the kind and track settings of the first pattern. Errors as for `stack()`, with `cat()` in the message.

```js
blank("house");
track("drums", cat([s("bd*4, hh*8"), s("bd*4, hh*8"), s("bd*4, hh*8"), s("bd*2 [bd*4], hh*16")]));
```

### `silence()`

```js no-run
silence()
```

A pattern with no events.

## Time

### `fast()` and `slow()`

```js no-run
pattern.fast(factor)     // number above 0, up to 256
pattern.slow(factor)     // number above 0, up to 256
```

`fast(2)` plays the pattern twice per bar; `slow(2)` spreads one cycle over two bars. Factors can be decimals.

Errors: `fast needs a number`; `fast takes a number above 0 and up to 256, not 1000`; the same for `slow`.

```js
blank("house");
track("hats", s("hh hh hh oh").fast(2));          // eighth notes, an open hat on the "and" of 2 and 4
track("pad", chord("<Am F>").slow(2).s("pad"));     // each chord for two bars
```

### `late()` and `early()`

```js no-run
pattern.late(cycles)     // number; negative moves earlier
pattern.early(cycles)    // number; negative moves later
```

Shifts the pattern in time. `late(0.25)` is one beat; small values such as `late(0.02)` make a part sit lazily
behind the beat. At a section's start, a shifted pattern brings in the end of the bar before it.

Errors: `late needs a number`, `early needs a number`.

```js
blank("lofi");
track("keys", chord("<Dm9 G9>").s("epiano").late(0.02));
```

### `rev()`

```js no-run
pattern.rev()
```

Reverses each bar: the last event comes first.

```js
blank("house");
track("lead", n("0 1 2 4").scale("A:minor").octave(4).every(2, |x| x.rev()).s("pluck"));
```

### `ply()`

```js no-run
pattern.ply(count)       // integer, 1 to 256
```

Repeats each event `count` times inside its own slot: `s("bd sd").ply(2)` is `s("[bd bd] [sd sd]")`.

Errors: `ply takes a number above 0 and up to 256, not 300`; `ply takes a whole number from 1 to 256, not 1.5`.

```js
blank("trap");
track("hats", s("hh*8").lastOf(4, |x| x.ply(2)));
```

## Higher-order functions

These take a closure `|x| ...` that receives the pattern and returns a changed one. [Track settings](#track-settings)
written inside a closure (`|x| x.s("piano")`) have no effect: only the events it returns are used.

### `every()` and `lastOf()`

```js no-run
pattern.every(n, f)      // n: integer (below 1 acts as 1), f: closure
pattern.lastOf(n, f)
```

`every(n, f)` uses `f`'s version in the first bar of every `n`, counted from the start of the section (bars 1, 5,
9 ... for `n` = 4). `lastOf(n, f)` uses it in the last bar of every `n` (bars 4, 8, 12 ...), which is where a fill
belongs.

```js
blank("house");
track("drums", s("bd*4, [~ cp]*2").lastOf(4, |x| x.fast(2)));   // a doubled bar before each phrase
```

### `sometimes()` and its relatives

```js no-run
pattern.sometimes(f)       // probability 0.5
pattern.often(f)           // 0.75
pattern.rarely(f)          // 0.25
pattern.almostNever(f)     // 0.1
pattern.almostAlways(f)    // 0.9
pattern.sometimesBy(p, f)  // p: number; 0 never, 1 always
```

Decides once for each event of the pattern, with the given probability, whether `f` applies to it, the way Strudel
does. The chosen events go through `f` and the others play as they are, so each event is either itself or `f`'s
version of it, never both: `s("hh*16").sometimes(|x| x.late(0.01))` plays sixteen hats, some nudged late, and
`.sometimes(|x| x.ply(2))` doubles some events and leaves the others single. A transform that changes speed (`fast`,
`slow`) is the exception: it picks events in its own time, so its copies can land on top of originals. The choice is
random but repeatable (see [Randomness](patterns.md#randomness)).

Errors: `sometimesBy needs a number`.

```js
blank("house");
track("hats", s("hh*16").gain(0.6).sometimes(|x| x.gain(0.3)));         // accents vary hit by hit
track("ride", s("rd*8").sometimes(|x| x.ply(2)).gain(0.5));              // some eighths become two sixteenths
track("lead", n("0 2 4 2").scale("A:minor").octave(4).rarely(|x| x.add(7)).s("pluck"));
```

### `off()`

```js no-run
pattern.off(cycles, f)   // cycles: number; f: closure
```

Plays the pattern and, on top of it, `f`'s version shifted later by `cycles`. A classic is an echo an eighth note
behind, an octave up.

Errors: `off needs a number`.

```js
blank("house");
track("arp", n("0 2 4 6").scale("A:minor").octave(4).off(0.125, |x| x.add(7)).s("pluck"));
```

### `superimpose()`

```js no-run
pattern.superimpose(f)
```

Plays the pattern and `f`'s version together, unshifted. Use it for doublings: a third or an octave above.

```js
blank("house");
track("lead", n("0 1 2 4").scale("A:minor").octave(4).superimpose(|x| x.add(2)).s("pluck"));
```

### `degrade()` and `degradeBy()`

```js no-run
pattern.degrade()        // drops each event with probability 0.5
pattern.degradeBy(p)     // p: number; 0 keeps all, 1 drops all
```

Drops events at random, repeatably. `degradeBy(0.3)` keeps about 70 % of the events. The mini-notation `?` does the
same per step.

Errors: `degradeBy needs a number`.

```js
blank("house");
track("shaker", s("shaker*16").degradeBy(0.3).gain(0.5));
```

## Structure

### `struct()`

```js no-run
pattern.struct(rhythm)   // number, pattern string or pattern
```

Takes the timing from `rhythm` and the values from the pattern. Every "on" event of the rhythm (any non-zero number,
any word but `0`, `false`, `f`) becomes an event with the value the pattern has at that moment. Rests in the rhythm
are silent. Use it to give a melody or a chord progression a rhythm without rewriting it.

```js
blank("house");
track("stabs", chord("<Am7 Dm7>").struct("~ x ~ x ~ x ~ [x x]").s("stab"));
```

### `mask()`

```js no-run
pattern.mask(onoff)      // number, pattern string or pattern
```

Keeps the events of the pattern that start where `onoff` is on, and removes the others. Unlike `struct()`, it never
adds events or changes timing.

```js
blank("house");
track("hats", s("hh*16").mask("<1 [1 0] 1 [1 0 0 1]>"));   // the hats drop out in places, bar by bar
```

### `euclid()` and `euclidRot()`

```js no-run
pattern.euclid(k, n)         // integers: k hits over n slots
pattern.euclidRot(k, n, r)   // and rotated left by r slots
```

The pattern played on a [euclidean rhythm](mini-notation.md#euclidean-rhythms). `s("rim").euclid(5, 16)` is
`s("rim(5,16)")`. `n` is 1 to 256, as in mini-notation (otherwise `a euclidean rhythm has 1 to 256 steps, not
1000`); `k` is held between 0 and `n`; `r` wraps around, so a negative `r` rotates right.

```js
blank("techno");
track("rim", s("rim").euclidRot(5, 16, 2).gain(0.6));
```

## Per-event controls

Each control is read from its argument at the start of every event, so a pattern string gives different values in
different places.

### `gain()` and `velocity()`

```js no-run
pattern.gain(amount)       // number, pattern string or pattern; 0..1
pattern.velocity(amount)
```

How hard each note is played. Both set the same thing; when both are present, `velocity` wins. Without either, sound
patterns play at 0.9 and the others at 0.8. Values above 1 play as 1. On synthesized drums and audio files, the
level follows velocity to the power 1.6, so 0.5 sounds about 10 dB below 1; synth presets and sampled instruments
respond the way the instrument does, with tone as well as level (a sampled piano switches to softer recordings).

```js
blank("house");
track("hats", s("hh*16").gain("[0.4 0.25 0.8 0.25]*4"));   // accent on the off-beat eighth
```

### `legato()`

```js no-run
pattern.legato(fraction)   // number, pattern string or pattern; 1 = the full slot
```

How long each note lasts, as a fraction of its slot. 0.5 is staccato, 1 (the default) is the whole slot, and values
above 1 overlap the next note. On the mono synths with glide (`808`, `acid`, `reese`, `square_lead`) an overlap makes
the pitch slide into the next note; `sub_bass` and `wobble` tie the notes without starting the envelope again.
Arpeggiated notes default to 0.9.

```js
blank("house");
track("bass", note("a1*8").s("organ_bass").legato("0.4 0.9"));
```

### `add()` and `transpose()`

```js no-run
pattern.add(amount)        // number, pattern string or pattern; rounded to a whole number
pattern.transpose(amount)  // the same, in semitones always
```

Both move pitches. `transpose()` always counts semitones. `add()` adds to the pattern's values, so what one step
means depends on the pattern:

| pattern | `add(1)` moves | `transpose(1)` moves |
|---|---|---|
| `note()` | one semitone up | one semitone up |
| `n()` | one scale degree up: `add(7)` is an octave in a seven-note scale, `add(5)` in a pentatonic one | the played note one semitone up |
| `chord()` | the voicing range one semitone up (voice leading still picks the notes) | every voiced note one semitone up |
| `chord().root()` | the root one semitone up | the root one semitone up |
| `s()` | the sample or drum sound one semitone up | the same |

`n("0 2 4").transpose(12)` is the same line an octave up in any scale; `n("0 2 4").add(7)` is too, in a seven-note
scale. Several calls add up.

```js
blank("house");
key("A minor");
track("lead",  n("0 2 4 2").octave(4).add("<0 0 -2 -1>").s("pluck"));       // the motif moves by scale steps
track("lead2", n("0 2 4 2").octave(4).transpose(12).s("bell").level(-6));   // the same line an octave up
track("perc",  s("rim*8").transpose("0 7 12 7").gain(0.5));
```

### `n()` on a sound pattern

```js no-run
pattern.n(index)           // number, pattern string or pattern
```

On an `s()` pattern, picks the variation of each sound, like `bd:3`. It replaces an index written in the word.

```js
blank("house");
track("kick", s("bd*4").bank("tr909").n("0 0 3 0"));
```

## Track settings

These describe the track rather than single events, so they apply to the whole track wherever they appear in the
chain, and a later call replaces an earlier one. Only the pattern given to `track()` counts: settings inside a closure
are ignored.

| method | what it sets | values |
|---|---|---|
| `.s(name)`, `.sound(name)` | the instrument of a `note()`, `n()`, `chord()` or `seq()` pattern | a [sound name](sounds.md) or a file path |
| `.bank(name)` | the drum machine or kit for the names in an `s()` pattern | `tr909`, `tr808`, `tr707`, `acoustic`, `uzu`, `apple` ... |
| `.scale(name)` | the scale for `n()`, and the key for Roman numerals in `chord()` | `"A:minor"`, `"D dorian"`, `"E:minor:pentatonic"`, `"C:blues"` |
| `.octave(n)` | the octave of degree 0 in `n()`, of note names without an octave, and of `.root()` notes | integer; default 3, 2 for `.root()` |
| `.voicing()`, `.voicing(lo, hi)` | the range chord voicings stay in | MIDI integers, default 52, 74 |
| `.arp(mode)` | arpeggiate each chord event | `"up"`, `"down"`, `"updown"` |
| `.root()` | a `chord()` pattern plays only each chord's root (its slash bass, when it has one) | octave 2 unless `.octave(n)` says otherwise |
| `.role(name)` | the [mixing role](mixing.md#roles) | `kick`, `snare`, `bass`, `chords`, `pad`, `lead` ... |
| `.level(db)` | level in dB relative to the role's target | number |
| `.pan(x)` | stereo position | −1 left .. 1 right |
| `.room(x)`, `.reverb(x)` | reverb send | 0..1 |
| `.delay(x)` | delay send | 0..1 |
| `.duck(x)` | sidechain depth from the kick | 0..1; default 0.3, 0 on drums |
| `.lpf(hz)`, `.cutoff(hz)` | a lowpass filter on the track | Hz |
| `.hpf(hz)` | a highpass filter on the track | Hz |
| `.crush(bits)` | a bitcrusher | bits, 1..24 |
| `.distort(db)` | tanh saturation | drive in dB |

`.scale()` and `.arp()` check their argument: `unknown scale 'Q:minor' (try "A:minor", "C:major", "D:dorian",
"E:minor:pentatonic")`, `unknown arp 'sideways' ("up", "down" or "updown")`. The numeric settings raise
`<name> needs a number` for anything else, and are not clamped by the pattern; [Mixing functions](mixing.md) says
what the mixer does with them. `.s()` on an `s()` pattern does nothing, since the words already name the sounds.

```js
blank("deep_house");
track("keys", chord("<Fm9 Bbm9>").voicing(53, 72).s("epiano")
                .level(-2).pan(-0.2).room(0.3).delay(0.1).lpf(4000));
```

`.root()` turns a chord pattern into a bass line that follows it. On the song's progression, the bass changes when
the progression does:

```js
blank("house");
key("A minor"); prog("A", "Am F/A C G"); bars_per_chord(1);
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano"));
track("bass", chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));   // A2 A2 C2 G2
```
