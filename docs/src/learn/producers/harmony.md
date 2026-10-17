# Harmony

claudius knows the song's key and its chord progressions, and everything harmonic reads them: chord patterns, the
parts the theory engine writes, arpeggios, the bass check of the critic. This page covers how to write harmony so
that all of those agree, and exactly how claudius interprets it. The full rules for symbols and numerals are in
[Pitch](../../language/pitch.md).

## Key and mode

A song has one key, in one of seven modes: major, minor (aeolian), dorian, phrygian, lydian, mixolydian, harmonic
minor. Roman numerals are counted in the key's own mode, so the same numeral changes quality with the mode:

| numeral | A minor | A dorian | A phrygian | A harmonic minor |
|---|---|---|---|---|
| `II` | B | B | B♭ | B |
| `IV` | D | D | D | D |
| `iv` | Dm | Dm | Dm | Dm |
| `V` | E | E | E | E |
| `VI` | F | F♯ | F | F |
| `VII` | G | G | G | G♯ |

Upper and lower case only choose major or minor; they never move the root. `IV` in A minor is a D major chord
(borrowed from dorian) on the fourth degree, `iv` the diatonic D minor. Accidentals move the root: `bVII` in C major
is B♭, `bVI` is A♭.

## Writing a progression

```js
blank("deep_house");
key("F minor");
prog("A", "Fm9 Bbm9 Dbmaj7 Cm7");     // stored as i9 iv9 VImaj7 v7
prog("B", "Dbmaj7 Eb Fm9 Fm9");
bars_per_chord(1);
track("keys", chord("prog").struct("x ~ ~ x ~ ~ ~ ~").voicing(53, 72).s("epiano"));
form([sec("groove", 8, "keys"), sec("break", 8, "keys")]);
```

Chord names are converted to numerals in the key that is current when `prog()` runs, then stored as numerals. Set the
key first; a later `key()` transposes the progression (and changes chord qualities if the mode changes, see below).

Progressions have names. Sections play `A` by default and breaks `B` (or `A`, when the song has no `B`); any other
name works with [`sec(...).prog("verse")`](../../language/form.md#section-methods), and a section that asks for a
progression the song does not have is an error. Every progression uses the same
[`bars_per_chord()`](../../language/song-functions.md#bars_per_chord), the number of bars each chord lasts. A
progression starts on its first chord at the start of every section and loops.

What numerals can express:

- diatonic triads and sevenths by degree and case: `i7 ii° III iv7 v7 VImaj7 VII`;
- extensions: `9`, `add9`, `6`, `maj7`, `maj9`, `11` (always minor 11), `sus2`, `sus4`;
- borrowed chords with accidentals: `bVI bVII` in major, `IV` (dorian) or `V7` (harmonic) in minor;
- applied chords: `V/V`, `V7/ii`, `viio7/V`, read in the key of the target chord;
- a bass note: `I/E`, `VI/A`, or as a name, `F/A` (stored as `VI/A` in A minor). `.root()`, the generated basslines
  and the critic use it, and `key()` moves it with the chord.

All of these work in pattern strings too: `chord("<I V7/V V7 I>")`, `chord("<Am F/A C G>")`.

## Chord patterns

`chord("prog")` plays the section's progression; `chord("prog:B")` always plays `B`; `chord("<Am7 Fmaj7>")` plays the
named chords regardless of the song's progressions. Prefer `prog`: the arranger's chord track, the generated parts
and the critic all use the song's progressions, so a pattern with its own chord names can disagree with all of them.

Every chord event is voiced with voice leading (see [Voicing](../../language/pitch.md#voicing)):

- three voices for triads, four for sevenths and anything larger; the fifth goes first when there are too many notes,
  then the root;
- close voicings (within about an octave and a half), placed in the range given by `.voicing(lo, hi)` (default
  52..74, E3 to D5);
- each chord chosen to move the voices as little as possible from the previous one, the first chord of a section
  near the middle of the range.

There is no drop-2 or spread voicing for patterns, and no control over inversions beyond the range. A narrow range
forces inversions: `.voicing(60, 72)` keeps every chord inside one octave above middle C.

Rhythm comes from the events: `.struct("x ~ ~ x ~ ~ x ~")` for a tresillo stab, `"x*16"` with a short `legato` for a
pumping sixteenth chord, `.arp("updown")` for an arpeggio. Layering two tracks on `chord("prog")` with different
ranges and rhythms (a pad low and long, stabs higher and short) keeps them in the same harmony automatically.

## Bass lines

Generated bass parts follow the progression and the kick (see the bass styles of
[`part()`](../../language/song-functions.md#part)). A pattern bass follows the progression with
[`.root()`](../../language/pitch.md#roots-bass-lines-from-the-chords), which plays each chord's root (or its slash
bass) in octave 2:

```js
blank("techno");
key("F phrygian");
prog("A", "i i II i");
bars_per_chord(2);
track("bass", chord("prog").root().octave(1).struct("[~ x x x]*4").s("sub_bass").legato(0.6));
```

`.octave(1)` puts the roots an octave lower: F1 for four bars, G♭1 for two, F1 for two. Change the progression and the
bass follows. When the line should move by step or leap to a different octave, write the notes out instead:
`note("<f1!4 gb1!2 f1!2>")` plays the same roots, one per bar.

On a blank song, [`part()`](../../language/song-functions.md#part) adds the genre's bass part over your progression:
`part("bass", #{ style: "rolling" })`.

The critic checks that the bass plays the chord's root (or its slash bass) on at least 70 % of the chord changes it
plays on; a bass that plays offbeats only is not checked.

## Changing key

`key()` moves everything written relatively: numerals, `n()` degrees, generated parts. Note names in `note()`
patterns are absolute and stay. When the mode changes, every diatonic chord keeps its degree and takes the new
mode's quality: `Imaj7 vi7 ii7 V7` in E major becomes `i7 VImaj7 iiø7 v7` in F minor. Borrowed (`bVII`) and
applied (`V/V`) chords are kept as written.

## The generator's harmony

`song(genre, seed)` picks a harmonic style from the genre's profile, or writes a progression with a
functional-harmony Markov chain (tonic, subdominant and dominant degrees, no chord repeated twice in a row, ending
on a chord that leads back to the start). The styles:

| style | mode | example progressions |
|---|---|---|
| `minor_epic` | minor | `i VI III VII`, `VI VII i i`, `i iv VI VII` |
| `minor_dark` | minor | `i i VI VII`, `i VI iv V`, `i v VI iv` |
| `minor_soul` | minor | `i7 iv7 VII7 IIImaj7`, `i7 VImaj7 iv7 v7`, `iiø7 V7 i7 i7` |
| `dorian_vamp` | dorian | `i7 IV7`, `i7 IV9 i7 VIImaj7`, `i7 ii7 i7 IV7` |
| `phrygian` | phrygian | `i II`, `i II i VII`, `i vii i II` |
| `harmonic` | harmonic minor | `i iv V i`, `i VI V V`, `i VI iv V` |
| `minimal` | minor | `i`, `i i i VI`, `i i iv iv`, `i9 i9 i9 VII` |
| `major_pop` | major | `I V vi IV`, `vi IV I V`, `I vi IV V` |
| `major7_jazz` | major | `Imaj7 vi7 ii7 V7`, `ii7 V7 Imaj7 Imaj7`, `ii9 V9 Imaj9 vi9` |
| `minor_80s` | minor | `i VI III VII`, `i VII VI VII`, `i III VII IV` |
| `mixolydian` | mixolydian | `I VII IV I`, `I VII`, `I IV VII IV` |

Progression `B` is another progression of the same style or a substitution of `A`. Evolution changes harmony with
these substitutions, each recorded in the song's lineage: a chord a third away, an extended or plain version, a
secondary dominant before a chord, a chord borrowed from the parallel mode, a suspension, two chords swapped, the
progression rotated.

## What the critic checks

The theory score reads the arranged song, not the audio:

| check | passes when |
|---|---|
| melody in key | 95 % of the lead's notes are in the key |
| chord tones on strong beats | 60 % of the lead's notes on beats 1 and 3 are tones of the chord sounding there |
| range | the lead spans 19 semitones or less |
| leaps | at most 20 % of the lead's moves (between notes less than two beats apart) are larger than a fifth |
| bass on the roots | 70 % of the bass notes that start on a chord change are the chord's root |
| voice leading | in chord, pad and keys tracks, voices move 4.5 semitones or less per change on average |

"The lead" is the first track whose role is `lead`, and "the bass" the first whose role is `bass`; the chords are the
song's progressions, as the arranger lays them out. That is why `chord("prog")` and `.role()` matter: a melody checked
against the wrong chords, or a pad checked as a melody, costs points for no musical reason.
