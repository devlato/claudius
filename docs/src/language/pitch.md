# Pitch: notes, scales and chords

Three pattern kinds carry pitch: `note()` (absolute notes), `n()` (scale degrees) and `chord()` (chords). This page
gives the exact rules for each, then voicing and arpeggios. Pitches are MIDI numbers: 60 is middle C (C4), 69 is
the A at 440 Hz, and each step is a semitone.

| note | C1 | C2 | C3 | C4 | C5 | C6 |
|---|---|---|---|---|---|---|
| MIDI | 24 | 36 | 48 | 60 | 72 | 84 |

## Note names

`note()` reads each word as a note name:

- a letter `a` to `g`, upper or lower case;
- any number of accidentals: `#` or `s` for sharp, `b` or `f` for flat (`c#`, `cs`, `eb`, `ef`, `cbb`);
- an optional octave number. **Without one, the octave is 3**, or the track's `.octave(n)`.

So `c` is C3 (48), `c4` is 60, `a1` is 33, `f#2` is 42, `bb1` is 34. A number instead of a name is a MIDI note:
`note("60 64 67")` is a C major triad from middle C. Decimals are rounded.

```js
blank("house");
track("bass", note("a1 a1 [a2 a1] g1").s("sub_bass"));
track("keys", note("[a3, c4, e4] ~ [f3, a3, c4] ~").s("epiano"));   // chords written note by note
```

A word that is not a note name is an error when the track is made: `cannot read 'h3' as a note (names with an
octave like c3, f#4, bb2, or MIDI numbers)`.

## Scale degrees

`n()` reads numbers as degrees of a scale, counted from 0:

- degree 0 is the tonic in octave 3 (or `.octave(n)`): in A minor, A3 (57);
- degree 7 in a seven-note scale is the tonic an octave up; degree 5 in a pentatonic scale;
- negative degrees go down: −1 in A minor is G3;
- decimals are rounded; a word that is not a number is an error (`n() takes scale degrees (0 1 2 -1 ...), not
  'e'`).

The scale is the track's `.scale(name)`, or the song's key when the track has none. Writing melodies in degrees
keeps them in key, and `key()` moves them with the song.

```js
blank("house");
key("A minor");
track("lead", n("0 2 4 7 6 4 2 1").octave(4).s("pluck"));   // A C E A G E C B, from A4
```

### Scale names

A scale is written as a tonic and a name, separated by `:` or spaces, in any case: `"A:minor"`, `"a minor"`,
`"E:minor:pentatonic"`, `"F# dorian"`.

| name | intervals (semitones from the tonic) |
|---|---|
| `major`, `ionian`, `maj` (or no name) | 0 2 4 5 7 9 11 |
| `minor`, `aeolian`, `natural_minor`, `min`, `m` | 0 2 3 5 7 8 10 |
| `dorian` | 0 2 3 5 7 9 10 |
| `phrygian` | 0 1 3 5 7 8 10 |
| `lydian` | 0 2 4 6 7 9 11 |
| `mixolydian` | 0 2 4 5 7 9 10 |
| `harmonic_minor`, `harmonic` | 0 2 3 5 7 8 11 |
| `pentatonic`, `major_pentatonic` | 0 2 4 7 9 |
| `minor_pentatonic` | 0 3 5 7 10 |
| `blues`, `minor_blues` | 0 3 5 6 7 10 |
| `chromatic` | all twelve |

Words after the tonic are joined with `_`, so `"minor pentatonic"` and `"minor:pentatonic"` both name
`minor_pentatonic`. Locrian, melodic minor and other scales are not available; `.scale()` rejects them with
`unknown scale`.

## Keys

[`key()`](song-functions.md#key) takes a tonic and a mode: `"A minor"`, `"Am"`, `"F#m"`, `"Eb"` (major),
`"D dorian"`, `"C# phrygian"`, `"Bb mixolydian"`, `"E harmonic minor"`. The tonic accepts `#`, `b`, `♯` and `♭`. The
seven modes in the first seven rows of the table above are the keys a song can be in; the pentatonic, blues and
chromatic scales exist only for `n()`.

## Chord symbols

`chord()` reads a word as a chord symbol when it starts with a note letter: a root, then a quality suffix.

| suffix | chord | notes on C |
|---|---|---|
| (none), `M`, `maj` | major | C E G |
| `m`, `min`, `-` | minor | C E♭ G |
| `dim`, `o`, `°` | diminished | C E♭ G♭ |
| `aug`, `+` | augmented | C E G♯ |
| `sus2` | suspended 2nd | C D G |
| `sus4`, `sus` | suspended 4th | C F G |
| `5` | power chord | C G |
| `6` | major 6th | C E G A |
| `m6` | minor 6th | C E♭ G A |
| `7` | dominant 7th | C E G B♭ |
| `maj7`, `M7`, `Δ`, `Δ7` | major 7th | C E G B |
| `m7`, `-7`, `min7` | minor 7th | C E♭ G B♭ |
| `m7b5`, `ø`, `ø7` | half-diminished | C E♭ G♭ B♭ |
| `dim7`, `o7`, `°7` | diminished 7th | C E♭ G♭ A |
| `add9` | added 9th | C E G D |
| `madd9` | minor added 9th | C E♭ G D |
| `9` | dominant 9th | C E G B♭ D |
| `maj9`, `M9`, `Δ9` | major 9th | C E G B D |
| `m9` | minor 9th | C E♭ G B♭ D |
| `m11` | minor 11th | C E♭ G B♭ D F |

Roots are case-insensitive (`am7` is A minor 7). A **slash chord** names its bass note after a slash: `F/A` is F
major over A, `C/E` is C major over E. Inside a pattern string, `°`, `+` and `♭` are not allowed by
[the tokenizer](mini-notation.md#tokens): write `dim` or `o`, `aug`, and `b`; a slash followed by a letter stays in
the word, so `chord("<Am F/A C G>")` works. An unknown symbol (`Cmaj13`, `Xyz`) is an error when the track is made:
`cannot read 'Xyz' as a chord (names like Am7, Fmaj7, Bb, numerals like ii7, bVII, V7/V, or prog)`.

The slash bass does not change the voicing (the chord's own notes are voiced as usual). It is what
[`.root()`](#roots-bass-lines-from-the-chords) plays, what generated basslines play on that chord, and what the
critic expects the bass to play.

## Roman numerals

A Roman numeral names a chord by its place in the key, so a progression in numerals follows every key change.
`chord()` reads any word that is not a chord symbol as a numeral, and [`prog()`](song-functions.md#prog) stores
progressions as numerals.

- **Degree**: `I II III IV V VI VII`, counted in the key's own mode. `VI` in A minor is F; `IV` in A dorian is D;
  `II` in E phrygian is F.
- **Case** picks major or minor when there is no suffix: `IV` is major, `iv` is minor. Case never moves the root.
- **Accidentals** before the numeral move the root a semitone: `bVII`, `bVI`, `bIII`, `#iv`. These are the borrowed
  chords.
- **Suffixes**: `7` is a dominant 7th on an upper-case numeral and a minor 7th on a lower-case one; `9` and `6` work
  the same way; `add9` gives add9 or madd9 by case. `°`, `o`, `dim` make a diminished triad; `ø`, `ø7`, `m7b5` a
  half-diminished 7th; `°7`, `o7`, `dim7` a diminished 7th; `+`, `aug` augmented; `11` always a minor 11th. Any
  other chord-symbol suffix (`maj7`, `maj9`, `sus4`, `m7`) sets the quality outright, whatever the case.
- **Applied chords**: `V/V`, `V7/ii`, `viio7/V` read the part before the slash in the key of the chord after it
  (major or minor, by that chord's quality). In C major, `V/V` is D major. They work in `prog()` and in pattern
  strings.
- **A note after the slash** (`I/E`, `VI/A`) is a slash chord: the chord, with that note in the bass.

| numeral in A minor | chord | numeral in C major | chord |
|---|---|---|---|
| `i` | Am | `I` | C |
| `iiø7` | Bm7♭5 | `ii7` | Dm7 |
| `III` | C | `iii` | Em |
| `iv7` | Dm7 | `IVmaj7` | Fmaj7 |
| `v7` | Em7 | `V7` | G7 |
| `V7` | E7 | `V/V` | D |
| `VImaj7` | Fmaj7 | `vi9` | Am9 |
| `VII` | G | `bVII` | B♭ |

```js
blank("house");
key("A minor");
track("keys", chord("<i VImaj7 iv7 V7>").s("epiano"));   // Am, Fmaj7, Dm7, E7: a harmonic-minor turn
```

A chord pattern with `.scale()` reads its numerals in that scale instead of the song's key:
`chord("<i VI III VII>").scale("C:minor")` plays Cm, A♭, E♭, B♭ in any song.

## The song's progression: `prog` and `prog:NAME`

In a chord pattern, the word `prog` means "the chord the song's progression has here". The progression is the one
the section plays (`A` unless the section says otherwise; breaks play `B`, or `A` when there is none), with
[`bars_per_chord()`](song-functions.md#bars_per_chord) bars per chord, starting from its first chord at the start of
each section and looping. `prog:B` always reads progression `B`, whatever the section.

```js
blank("house");
key("A minor");
prog("A", "Am7 Fmaj7 Dm7 Em7");
prog("B", "Fmaj7 G Em7 Am7");
bars_per_chord(1);
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano"));
track("pad",  chord("prog").s("pad").role("pad"));
form([sec("groove", 8, "*"), sec("break", 8, "pad")]);
```

The keys and the pad play A in the groove and switch to B in the break, without being told. A name with no
progression behind it (`prog:C` when there is no `C`) plays progression `A`, as the generated parts do. A section
cannot ask for a progression the song does not have: [`form()`](form.md#form) and [`section()`](form.md#section)
refuse it, except that a break with no `B` plays `A`.

Writing harmony this way has a second benefit: the arranger's chord track, the generated parts and the critic's
harmony checks all read the song's progressions. A chord pattern with explicit names plays what it says, but the
critic still checks the melody and bass against `prog("A")` (on a `blank()` song, the tonic chord alone), so set it
to the same chords.

## Roots: bass lines from the chords

`.root()` makes a chord pattern play only each chord's root, or its slash bass when it has one, as a single note in
octave 2 (`.octave(n)` moves it). On `chord("prog")`, that is a bass line that follows the song's progression, and
changes when you change the progression:

```js
blank("house");
key("A minor");
prog("A", "Am F/A C G");
bars_per_chord(1);
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano"));
track("bass", chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("organ_bass"));
```

The bass plays A2, A2 (the slash bass of `F/A`), C2 and G2, a bar each. Because every chord's root lands in the same
octave, the line jumps where a bassist would step (G2 up to A2 is fine, A2 down to C2 is a sixth); write the notes
out with `note()` when the shape of the line matters.

## Voicing

Every chord event is voiced, whether or not you call `.voicing()`:

1. **How many voices**: the number of different notes in the chord, but at least 3 and at most 4. A triad gets
   three voices, a 7th chord four, a 9th or 11th chord four.
2. **Which notes go** when there are more notes than voices: the fifth first, then the root (the bass covers it). A
   9th chord keeps the third, seventh and ninth plus the root.
3. **Doubling** when there are fewer notes than voices (a power chord): the root first, then the fifth.
4. **Where**: every arrangement of those notes inside the range is scored. Close voicings (3 to 14 semitones from
   bottom to top) are preferred; intervals under a fourth below E3 and semitone clusters below E4 are penalised; and
   the distance each voice moves from the previous chord counts most. The first chord of a section is placed near
   the middle of the range.

The range is `.voicing(lo, hi)` in MIDI numbers, by default 52 to 74 (E3 to D5). `.voicing()` without arguments
uses the default. `.add(n)` shifts the range by n semitones.

```js
blank("house");
key("A minor"); prog("A", "Am7 Fmaj7 Dm7 Em7"); bars_per_chord(1);
track("keys", chord("prog").voicing(55, 74).s("epiano"));    // Am7 as C4 E4 G4 A4
track("pad",  chord("prog").voicing(45, 64).s("pad").role("pad"));
```

The keys hold common tones and move the other voices by step: from Am7 (C4 E4 G4 A4) to Fmaj7 only the G moves,
down to F; to Dm7 the E moves down to D (C4 D4 F4 A4); to Em7 the C, F and A each step down (B3 D4 E4 G4). A range
narrower than an octave can leave no valid voicing, in which case the chord tones are stacked upwards from `lo`.

## Arpeggios

```js no-run
chord(...).arp(mode)    // "up", "down", "updown"
```

`.arp()` plays each chord event's voiced notes one after another, evenly across the event:

| mode | order, for a four-note chord |
|---|---|
| `up` | 1 2 3 4 |
| `down` | 4 3 2 1 |
| `updown` | 1 2 3 4 3 2 |

Each note lasts its share × `legato` (0.9 by default). Any other mode plays `up`. The speed comes from the events:
one chord per bar arpeggiated over a triad gives three notes per bar; give the pattern more events with `.struct()`
or `*` for faster arpeggios.

```js
blank("synthwave");
key("A minor"); prog("A", "Am F C G"); bars_per_chord(1);
track("arp", chord("prog").struct("x*4").arp("updown").voicing(57, 76).s("pluck"));   // triads: 16th notes
```

A triad in `updown` has four steps (1 2 3 2), so four events per bar give straight sixteenths; `up` would give
triplets.

## Velocity and length

Pitch patterns use the same controls as every pattern: [`gain()`/`velocity()`](pattern-functions.md#gain-and-velocity)
for how hard, [`legato()`](pattern-functions.md#legato) for how long. A note lasts its whole slot unless `legato`
says otherwise, so `note("a1 ~ ~ ~")` is a quarter note and `note("a1 _ _ _")` a whole bar.
