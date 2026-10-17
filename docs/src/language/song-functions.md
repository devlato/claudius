# Song functions

Song functions edit the current song. Each one changes only what it names, in the order the script calls them.
Sections and arrangement are on [Form and sections](form.md); `mix()`, `master()` and the other mixer edits are on
[Mixing functions](mixing.md).

## Starting a song

### `song()`

```js no-run
song(genre)               // a new song, seeded from the clock
song(genre, seed)         // seed: integer
song(genre, options)      // options: map
```

Replaces the current song with one the generator writes: tempo, key, swing, two progressions, a form, drum lanes,
theory-written parts, sounds and a mix, all from the genre's profile. The same genre and seed give the same song on
any machine. The recordings installed (sample packs, Apple Loops) change only which recordings it plays.

| option | type | default | effect |
|---|---|---|---|
| `seed` | number | 1 | which song |
| `bpm` | number | from the genre's range | tempo |
| `key` | string | random tonic in one of the genre's modes | key, e.g. `"F minor"` |
| `short` | bool | `false` | halve every section (at least 4 bars), for sketches |
| `loops` | bool | `false` | build the song around a family of Apple Loops (GarageBand style), when they are installed |
| `samples` | number | 0.75 | chance that each drum lane uses a recorded one-shot from your library instead of synthesis |

The genre is one of `house deep_house techno trap lofi dnb future_bass synthwave`, or one of your own (see
[Genres](../learn/producers/genres-and-evolution.md)); case, spaces and hyphens do not matter (`"deep house"`). A
negative seed acts as 0. `bpm` and `key` change only the tempo and the key: the title, the form, progression `A` and
the drum patterns are the ones the seed gives without them. A key in another mode than the seed's moves the chords
into that mode, as [`key()`](#key) does. The options are checked like [`tempo()`](#tempo) and
[`key()`](#key) check their arguments, and an option the map should not have is an error rather than ignored (so is a
misspelt one).

Errors: `unknown genre 'polka' (try: deep_house, dnb, future_bass, house, lofi, synthwave, techno, trap)`;
`tempo between 40 and 240 BPM`; `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")`;
`song() has no option 'tempo' (options: seed bpm key loops short samples)`.

```js
song("techno", #{ seed: 3, bpm: 130, key: "F minor", short: true });
```

### `blank()`

```js no-run
blank(genre)
```

Replaces the current song with an empty one: a single 8-bar section called `loop` (kind `groove`, energy 0.8), no
tracks, and one progression, `A`, which is the key's tonic chord alone (`i` in a minor key, `I` in a major one) at
[`bars_per_chord(1)`](#bars_per_chord). The rest comes from the genre: the mix (bus effects, master chain, loudness
target, reverb and delay), the critic's genre, and a tempo, key, swing and humanize from the generator. Set the ones
you care about; in particular, set `prog("A", ...)` to the chords you play, since the critic checks the bass and
melody against it.

```js
blank("lofi");
tempo(84); key("D minor"); swing(0.58);
track("drums", s("bd ~ ~ bd, ~ sd, hh*8?0.15"));
```

A script that calls neither `song()` nor `blank()` edits the song that is open (the studio's, or the one passed with
`claudius script --on`), and with no song open it starts from `blank("house")`. So a script for another genre that
should run on its own starts with `blank()`.

Errors: as for `song()`.

## Global settings

### `title()`

```js no-run
title(name)    // string
```

The song's title, used in the song folder's name (`out/house-night-drive-1/`).

### `tempo()`

```js no-run
tempo(bpm)     // number, 40..240
bpm(bpm)       // the same function
```

Errors: `tempo between 40 and 240 BPM`; `tempo needs a number` (also from `bpm()`).

### `key()`

```js no-run
key(name)      // "A minor", "Am", "F#m", "Eb", "D dorian", "C# phrygian" ...
```

Sets the key. Roman-numeral progressions and `n()` patterns follow it; note names in `note()` patterns do not, since
they name absolute pitches. When the mode changes (minor to dorian, major to minor), every diatonic chord in every
progression moves to the same degree in the new mode: `Imaj7` in a major key becomes `i7` in minor, `IV7` in dorian
becomes `iv7` in minor. Borrowed chords (`bVII`) and applied chords (`V/V`) stay as written. Tonal Apple Loops in the
song are repitched the shortest way (at most six semitones).

Errors: `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")`.

```js
// G dorian to A minor: "i7 ii7 i7 IV7" becomes "i7 iiø7 i7 iv7"
key("A minor");
```

The accepted names and modes are in [Pitch](pitch.md#keys).

### `swing()`

```js no-run
swing(amount)  // number, clamped to 0..0.8
```

Sixteenth-note swing for the whole song. Each off-beat sixteenth (the second of each pair) is moved to *amount* of
the way through its eighth note: 0.5 is straight, 0.58 is a lazy hip-hop feel, 0.66 is close to triplets. Values up
to 0.501 play straight. Only notes exactly on the sixteenth grid move; eighth notes and triplets stay where they are.
Swing applies when the song renders, to drums, parts and pattern tracks alike.

### `humanize()`

```js no-run
humanize(amount)   // number, clamped to 0..1
```

Random timing and velocity, applied at render. The timing offset has a standard deviation of 4 ms × *amount* on drum
roles and 60 % of that on other tracks (synth tracks use 3 ms instead of 4); velocity varies by 5 % × *amount*. The
offsets are seeded by the song and the track name, so they are the same on every render.

## Harmony

### `prog()`

```js no-run
prog(name, chords)   // name: string; chords: string, separated by spaces
```

Sets a progression. Chords are Roman numerals (`"i7 VImaj7 iv7 v7"`) or chord names (`"Am7 Fmaj7 Dm7 Em7"`); names
are converted to numerals in the current key when `prog()` runs, so call `key()` first. Each chord lasts
[`bars_per_chord()`](#bars_per_chord) bars, and the progression loops.

Sections play `A` unless they say otherwise ([`sec().prog()`](form.md#section-methods)), and breaks play `B`, or `A`
when the song has no `B`. Any name works: `prog("verse", ...)` is used by sections that ask for `"verse"`. The
progressions drive `chord("prog")`, the generated parts, the chord track the critic checks melodies and basslines
against, and the harmony tab of the studio. [Pitch](pitch.md#roman-numerals) has the numeral rules.

A chord name outside the key gets an accidental (`Bb` in A minor is `bII`). A slash chord keeps its bass note, as a
name or as a numeral: `F/A` in A minor is stored as `VI/A`, and [`key()`](#key) moves the bass note with the chord.

Errors: `cannot read chord 'Q7' (Roman numerals like i7 VImaj7 bVII, or names like Am7)`.

```js
blank("house");
key("A minor");
prog("A", "Am7 Fmaj7 Dm7 Em7");     // stored as i7 VImaj7 iv7 v7
prog("B", "Fmaj7/A G Em7 Am7");     // stored as VImaj7/A VII v7 i7
bars_per_chord(1);
track("keys", chord("prog").s("epiano"));
```

### `bars_per_chord()`

```js no-run
bars_per_chord(bars)   // integer, clamped to 1..8
```

How many bars each chord lasts: `bars_per_chord(2)` holds each chord for two bars. It applies to every progression.
`blank()` starts at 1; a generated song has the generator's value, often 2.

## Drums

### `drum()`

```js no-run
drum(lane, steps)                     // the "main" variant, 16 steps per bar
drum(lane, variant, steps)
drum(lane, variant, steps, per_bar)   // per_bar: integer, clamped to 1..96
```

Writes a drum lane's pattern as a step string, the way a step sequencer shows it:

| character | hit |
|---|---|
| `X` | accent, velocity 1.0 |
| `x` | hit, 0.8 |
| `o` | ghost note, 0.45 |
| `1` to `9` | velocity 0.1 to 0.9 |
| `\|`, spaces | ignored, for readability |
| anything else (`.`, `-`, `0` ...) | rest |

There are 16 steps per bar unless `per_bar` says otherwise: 32 for thirty-second notes, 12 or 48 for triplet grids.
The length of the string decides the length of the pattern, rounded up to whole bars (at most 16): 16 steps are one
bar, 20 steps are two bars with the last 12 steps empty. All the variants of a lane have the same length; a shorter
one is repeated to fit.

On an existing lane, `drum()` replaces that variant. On a song without the lane, it creates one, if the name is a
[drum name](sounds.md#drum-names) (or `hats`): the lane uses the synthesized voice at its default settings, and joins
every section with energy 0.6 or more. [`drum_sound()`](#drum_sound) changes what it plays.

Sections choose a variant by name with [`.variant()`](form.md#section-methods); intros and outros use `sparse`, the
others `main`, and a lane without the requested variant plays `main`.

A lane's [mixing role](mixing.md#roles) comes from its name when the name is a drum name (`bd` and `kick` are kicks,
`sd` and `snare` snares), else from the sound it plays. The arranger finds lanes by role: snare rolls and fills go on
the snare lane, else the clap; the kick drops out at the end of a build, and the other tracks duck against it.

Errors: `no drum lane 'bongo', and drum() makes lanes only for drum names (bd kick sd ...); the song's lanes: kick clap
hat ohat shaker perc. For other sounds, track("bongo", s("..."))`.

```js
drum("kick", "X...x...X...x..x");                     // a pickup into the next bar
drum("hat", "sparse", "..x...x...x...x.");
drum("hat", "main", "x.x.x.x.x.x.x.x.x.x.x.x.x.x.xxxx", 32);
```

### `drum_sound()`

```js no-run
drum_sound(lane, sound)         // sound: string
drum_sound(lane, sound, bank)   // bank: string, a drum machine or kit
```

Sets what a drum lane plays, keeping its patterns:

- a [drum name](sounds.md#drum-names), for the synthesized voice: `drum_sound("snare", "cp")`;
- a kit sound from the sample packs: `drum_sound("kick", "tr909:bd")`, or the same as
  `drum_sound("kick", "bd", "tr909")`, with any [bank](sounds.md#drum-machines-and-kits); a variation number picks
  another recording (`"tr909:bd:12"`);
- another recording from the packs, such as `"conga"`;
- an audio file: `drum_sound("perc", "/Users/me/Samples/rim.wav")`.

Naming the sound the lane already plays keeps it as it is, including the tuning evolution gave a synthesized voice.
The lane keeps its name, so a lane named with a drum name keeps its role; a lane named otherwise takes the role of
its new sound.

Errors: `no drum lane 'bongo' (lanes: kick clap hat ohat shaker perc); drum() makes one`, or `(lanes: none yet)` on a
song without drum lanes; `'pad' is not a drum sound (a drum name, a kit sound, or a sample)` for a synth preset; the
[sound errors](sounds.md#how-a-name-is-resolved) for a name that does not resolve.

```js packs
song("house", 9);
drum_sound("kick", "tr909:bd");
drum_sound("clap", "cp", "tr808");
```

## Parts

### `part()`

```js no-run
part(name, options)   // options: map with any of the fields below
```

Edits a part the theory engine writes: the `bass`, `chords`, `pad`, `lead`, `arp` or `keys` of a generated song. On a
song without that part (a `blank()` song has none), `part()` first adds the genre's part, written by the theory
engine over the song's progressions and joining the sections the way a new [track](#track) does; the options then
edit it. `part("bass", #{})` adds the genre's bass as it comes.

| field | type | effect |
|---|---|---|
| `style` | string | how the part is written (below) |
| `preset` | string | a synth preset or a sampled instrument; naming the current preset keeps its patch |
| `register` | `[low, high]` | MIDI range the part is written in |
| `density` | number, clamped to 0..1 | how busy a lead is |
| `seed` | integer | a new seed rewrites this part, and only this part |
| `rate` | number, clamped to 1/32..4 | arpeggio step in beats (0.25 is sixteenths) |

| role | styles |
|---|---|
| bass | `sustain`, `pulse`, `root8`, `offbeat`, `octave`, `rolling`, `syncopated`, `kick_follow` (follows the kick, 808 style), `acid`, `walking`, `reese` |
| chords, pad, keys | `sustain`, `half`, `stabs`, `offbeat`, `pump`, `syncopated`, `charleston` |
| arp | `up`, `down`, `updown`, `random`, `pinky` |
| lead | `motif`, the only one: leads are motifs with A A′ A B phrasing; `density` and `seed` change them |

A style the part's role does not have is an error. A decimal `seed` is ignored.

Errors: `no part 'keys' (the song has: bass chords pad lead arp; house parts: bass chords pad lead arp); track()
writes your own`; `a bass part has no style 'wobbly' (styles: sustain pulse root8 ...)`; `unknown preset 'nope'
(synths: ...; or an installed instrument such as piano, rhodes, string_ensemble)`; `part() has no option 'styles'
(options: style preset register density seed rate)`.

```js
part("bass", #{ style: "rolling", preset: "reese" });
part("lead", #{ seed: 42, density: 0.4 });
part("chords", #{ style: "stabs", register: [55, 72] });
```

On a blank song, `part()` gives you the generator's parts over your own progression:

```js
blank("house");
key("A minor"); prog("A", "Am F C G");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
part("bass", #{ style: "offbeat" });
part("chords", #{ style: "stabs", preset: "stab" });
```

## Tracks

### `track()`

```js no-run
track(name, pattern)
```

Makes a pattern track, or replaces the track of that name: a drum lane, a generated part, a loop or another pattern
track (`track("bass", ...)` on a generated song replaces its bass part). The pattern's sounds and words are read
here, so an unknown sound, a word the pattern cannot read (`note("h3")`) or a pattern that is too dense fails at this
line.

Where it plays:

- A replaced track keeps the replaced track's place in every section.
- A new track joins every section with energy 0.6 or more, or every section when no section has any track yet (the
  first track of a `blank()` song).
- [`form()`](form.md#form) and [`layer()`](form.md#layer) decide it exactly.

Its mix comes from the pattern's own settings (`.level()`, `.pan()`, `.room()` ...); a `mix()` on the old track does
not carry over. Its role comes from `.role()`, else from its sound (see [Roles](mixing.md#roles)). A sound pattern
with several sounds becomes one track per sound when arranged: `drums.bd`, `drums.cp`, `drums.hh`.

Each `track()` writes a log line with the sounds it found:

```js
blank("house");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
```

```text
track drums: bd, cp, hh
```

Errors: `track 'x': unknown sound 'bdd' (...)`; `track 'x': cannot read 'h3' as a note (...)`; `track 'x': this
pattern is too dense to play: it makes more than 262144 events` (or `it needs more than 4000000 steps of work`);
`track 'x' plays 2048 notes per bar; the limit is 1024`. See [Errors and limits](limits.md).

### `mute()`, `unmute()`, `solo()`, `unsolo()`, `remove()`

```js no-run
mute(track)    unmute(track)
solo(track)    unsolo(track)
remove(track)
```

`mute()` silences a track; while any track is soloed, only soloed tracks play, and only they are judged. `remove()`
deletes the track and takes it out of every section. They work on drum lanes, parts, loops and pattern tracks.

Errors: `no track 'nope'`.

```js
mute("hat");
remove("arp");
```
