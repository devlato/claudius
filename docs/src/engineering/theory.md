# Composition and theory

Composition happens in two places. `compose::generate` makes the creative choices and writes them into a blueprint:
which progressions, which form, which drum patterns, which style and seed for each part. `compose::arrange` turns
the stored choices into notes, calling the writers in `theory/` for every part. Both are deterministic, so a
blueprint is all it takes to get the same notes back.

| module | what it does |
|---|---|
| `genres.rs` | loads the genre profiles from `data/genres/*.json` and your genres folder |
| `compose.rs` | `generate`, `new_part`, `styles_for`, `part_sound`, kits, flipped samples (`add_sample`) |
| `compose/arrange.rs` | `arrange`, `cycle_spans`, `part_cycle` |
| `theory/scale.rs` | pitch classes, the seven modes, keys, scale degrees |
| `theory/chord.rs` | chord qualities and symbols, Roman numerals, voicing |
| `theory/progression.rs` | the progression library, the Markov generator, substitutions |
| `theory/rhythm.rs` | drum hits, lane templates, euclidean rhythms, rolls, fills, swing, syncopation |
| `theory/bass.rs` | bass lines in eleven styles |
| `theory/melody.rs` | leads, arpeggios and chord parts |

## Genres are data

Everything a genre knows is in one JSON file. The fields `generate` reads:

| field | what it decides |
|---|---|
| `bpm`, `swing`, `humanize` | ranges to draw from, and the looseness |
| `modes` | weighted modes for Markov progressions |
| `harmony`, `markov`, `ext` | weighted progression styles from the library, the chance of a Markov progression instead, and its chord extensions (0 triads, 1 sevenths, 2 ninths) |
| `bars_per_chord` | weighted chord lengths |
| `forms` | one or more forms, each a list of sections with layers, energy and flags |
| `drums` | lanes: a voice, a step template or a euclidean rhythm, named variants, parameter ranges, Apple Loops subtypes for one-shots, a choke group, whether to tune the kick to the key, a mix |
| `kits` | weighted sample-pack drum kits; the weight left over keeps the lanes' own sounds |
| `parts` | melodic parts: a role, weighted-by-repetition lists of presets and styles, a register, a density range, voices, voicing, arpeggio rates, a chance of leaving the part out, a mix |
| `loops` | Apple Loops parts: instrument, subtype, genre and mood filters, whether tonal |
| `transitions` | riser, impact, downlifter and reverse crash switches, the riser length |
| `mix` | the LUFS target, the reverb and delay, bus and master chains, sidechain release, per-role level offsets |
| `query`, `alts` | what the ears should hear (`"house music"`) and the genres it is most often confused with |

A list like `"styles": ["offbeat", "syncopated", "offbeat"]` weights by repetition: `offbeat` is picked two times in
three. A preset entry can name alternatives separated by `|`: `"piano|epiano"` plays the sampled piano when its pack
is installed and the `epiano` synth otherwise. [`part_sound`](#sounds-for-parts-and-drums) explains why that does not
change the rest of the song.

## How `generate` writes a song

`generate_inner` draws, in order, from forks of one generator seeded with the song's seed (see
[Determinism](architecture.md#determinism)):

1. the tempo and the swing, from the genre's ranges; the tempo is drawn even when one is given, which then replaces
   the result, so that a tempo of your own changes nothing else;
2. **harmony**: with chance `markov` (or when the genre lists no styles), a Markov progression of 4 chords (2, one
   time in five) in one of the genre's modes; otherwise a progression from a library style, in that style's mode. The
   tonic is drawn the same way, always, and a key of your own replaces the tonic and the mode. Progression `B` is
   another progression of the same style half the time, else a substitution of `A` worked out in the style's mode.
   When a key of your own has another mode, both progressions then move to it the way `key()` moves them: diatonic
   chords keep their degree and take the new mode's quality, borrowed and applied chords stay as written. Chord length comes from `bars_per_chord`, and a one-chord
   progression lasts 4 bars;
3. **form**: one of the genre's forms, every section halved (to at least 4 bars) with `--short`;
4. **drums**: per lane, a one-shot from the library (with chance `samples`, 0.75 unless `--samples` or a script says
   otherwise, the same for `make`, scripts and the studio's new songs, when there is a library) or a
   synthesized voice with parameters in the genre's ranges (or a third of the way from the voice's default towards a
   random point), the kick tuned to the key; then a 4-bar pattern per variant from the lane's template or euclidean
   rhythm; then, when a kit was drawn, the kit's drum for the lane;
5. **parts**: per part, unless its `optional` chance leaves it out, a preset, a style, a density and a seed of its
   own. A chords, keys or pad part on a single-voice synth patch gets a light chorus so it does not sit dead centre;
6. **loops** from the library, when there is one;
7. **transitions**: a crash (recorded or synthesized) and the genre's switches;
8. the **mix** from the genre, and a **title** from two word lists.

With `--loops` (a loop family), the song is generated with the family's tempo and key, both progressions become the
chords read from the family's harmony loop, one chord per bar, and the family's chord, bass and beat loops replace the
parts and lanes of those names.

## Keys and modes

A `Key` is a tonic pitch class (C = 0) and one of seven modes: major, minor, dorian, phrygian, lydian, mixolydian and
harmonic minor. `Key::parse` reads `"Am"`, `"A minor"`, `"F# dorian"`, `"Eb"`, `"C#m"`. A key spells with flats when
its relative major sits on the flat side of the circle of fifths (`Bb major`, `D minor`), with sharps otherwise.

The functions the writers lean on: `degree_pitch(d, base)` (the pitch of scale degree `d` counted from the tonic at or
above `base`), `snap` (the nearest scale tone, ties going down), `step` (move by scale steps), `index_of` and
`from_index` (between pitches and diatonic indices), and `transpose_from` (the shortest way between two tonics,
−6 to +5 semitones), used for loops and key changes.

## Chords

`Chord` is a root, one of 20 qualities (triads, sus2 and sus4, sevenths, sixths, add9, ninths, minor 11th, power
chords) and an optional bass note. `Chord::parse` reads symbols (`Am7`, `F/A`, `Cmaj7`, `Bbm9`, `Dsus4`, `E5`, and
`Δ`, `ø`, `°` spellings), and `symbol` prints them back.

### Roman numerals

`chord::roman(numeral, key)` reads a numeral **in the key's own mode**, which is why progressions survive a change
of key:

- the degree is counted in the key's mode: `VI` in A minor is F, `IV` in D dorian is G;
- case gives the quality: `VI` major, `vi` minor; a suffix overrides it (`7` dominant on upper case and minor seventh
  on lower case, `maj7`, `9`, `6`, `add9`, `°`, `ø7`, `°7`, `+`, `sus4`, `11`);
- `b` and `#` before the numeral move the root: `bVII`, `bII`;
- `/` and a numeral makes an applied chord, read in the key of the target: `V7/V` in C is D7;
- `/` and a note name sets the bass: `I/E`.

`roman_of(chord, key)` goes back, choosing `b` or `#` for roots outside the scale. A script's `prog("A", "Am7 Fmaj7")`
stores chord names as numerals this way.

### Voicing

`chord::voice` picks the actual pitches of a chord inside a register, for `n` voices, given the previous voicing:

1. With more chord tones than voices, it drops the fifth, then the root (the bass has it). With fewer, it tries
   doubling the root, then the fifth, then the third, each try costing a little more.
2. It enumerates every way to place the tones inside the register, and scores each: distance from the span the style
   wants (`close` 3 to 14 semitones, `open` 10 to 24, `spread` 17 to 34); a penalty for intervals under a fourth below
   E3 (MIDI 52), where they turn to mud, and for semitone clashes below E4; and the voice-leading distance from the
   previous chord (the sum of each voice's movement), or, for the first chord, the distance from the middle of the
   register.
3. It takes the best. When the second best is within one point, it takes the second three times in ten, so repeated
   progressions do not voice identically every time.

The critic checks the result: `voice_leading_<track>` is the average movement per voice per chord change, and above
4.5 semitones it raises an issue.

## Progressions

**The library** in `progression::STYLES` has eleven styles, each a mode and a handful of progressions:
`minor_epic`, `minor_dark`, `minor_soul`, `dorian_vamp`, `phrygian`, `harmonic`, `minimal`, `major_pop`,
`major7_jazz`, `minor_80s` and `mixolydian`. For example `minor_soul` holds `i7 iv7 VII7 IIImaj7` and
`iiø7 V7 i7 i7`; genres weight the styles they use (house: `minor_soul` 0.35, `minor_epic` 0.25, `dorian_vamp` and
`major7_jazz` 0.2 each).

**The Markov generator** walks a 7 by 7 table of transition weights between scale degrees, one for major and one for
minor keys, written by hand around functional harmony: tonic to subdominant to dominant to tonic, plus the stepwise
and third moves pop and EDM use. A progression starts on the tonic (three times in four) or the sixth degree, never
repeats a chord immediately (its weight is cut to a tenth), and is accepted only when its last chord leads back to
its first with a weight of at least 1; otherwise it tries again, up to 64 times. The chords are the diatonic chords of
those degrees, extended to sevenths or ninths by the genre's `ext` (a major chord whose seventh is minor becomes a
dominant seventh).

**Substitutions** change one chord in a way a musician would, and say what they did, for the song's history (the
examples are on `i VI iv VII` in A minor):

| substitution | example |
|---|---|
| a chord a third away (up a sixth or up a third, in the scale) | `chord 2: VI -> iv (third relation)` |
| richer or plainer: a triad gains a seventh or ninth, an extended chord loses it | `chord 1: i -> i7 (extended)` |
| the chord before becomes the secondary dominant of this one | `chord 3: iv -> IV7 (secondary dominant of VII)` |
| modal interchange: the same root with the opposite third | `chord 4: VII -> vii (borrowed from the parallel mode)` |
| a suspension (sus2 or sus4), except on diminished chords | `chord 2: VI -> VIsus4 (suspension)` |
| swap two neighbouring chords | `swapped chords 2 and 3` |
| rotate the progression | `rotated the progression by 2` |

`generate` uses them to derive progression `B` from `A`, and evolution's `harmony_sub` operator applies one at a
time.

## Rhythm

**Lane templates.** A drum lane's habits are a `LaneTemplate`: a hit probability for every sixteenth (16 per bar,
32 for two bars), where 1 or more always hits; a velocity range; `ghost`, the chance of a soft extra hit on an empty
step; `rolls`, the chance per beat of a trap roll; and `vary`, how often the last bar of a phrase deviates. `generate`
samples one bar from it, repeats that bar over the 4-bar pattern, and sometimes swaps the last bar for a varied one
(probabilities raised by 1.6 and halved at random), so the pattern loops like a programmed beat instead of shuffling
every bar. Velocities follow the metrical weight of the step: the downbeat strongest, then beat 3, then beats 2 and
4, then the eighths, then the rest.

The house hat template reads like this, a pattern a person would program:

```json
{"steps": [0.7, 0.3, 0.25, 0.35, 0.7, 0.3, 0.25, 0.35, 0.7, 0.3, 0.25, 0.35, 0.7, 0.3, 0.25, 0.5],
 "vel": [0.35, 0.8], "vary": 0.2}
```

**Trap rolls** replace a whole beat with a burst of thirty-seconds, sixteenth triplets, a half beat of
thirty-seconds, or thirty-second triplets, with a velocity ramp up (six times in ten) or down. They fall mostly on the
last beat of a bar and rarely on the downbeat.

**Euclidean rhythms** spread `k` onsets over `n` steps as evenly as possible (`rhythm::euclid`, computed with the
`(i * k) % n < k` formula, which gives the same patterns as Bjorklund's algorithm up to rotation). A lane with
`"euclid": [5, 16]` gets a random rotation and a `sparse` variant with half the onsets.

**Builds and fills.** `snare_roll` plays quarters, then eighths, sixteenths and thirty-seconds, rising in velocity.
`fill` writes the last one or two beats of a phrase as a `roll` (sixteenths rising), `triplets`, or a `groove` of
random sixteenths.

**Swing** delays every off-beat sixteenth: within each eighth, the second sixteenth moves from halfway to
`swing` of the way through (0.5 is straight, 0.66 a triplet feel). It is applied when notes are rendered, so the
blueprint stays on the grid.

**Measures** for the critic and analysis: `syncopation` (Longuet-Higgins and Lee: an onset on a weak position
followed by silence on a stronger one scores the difference in metrical level) and `density` (onsets per beat).

## Bass lines

`bass::bassline` writes a one-bar motif once per part and transposes it onto every chord, which is how a producer
writes a bass line: one groove, followed through the changes. The motif's intervals are relative to the chord's bass
note, and some are resolved against the sounding chord (its third, its fifth, chord tones for walking lines, a
chromatic approach to the next root), so the line fits every chord. The root of each chord is the one nearest the
previous root, unless that strays more than 7 semitones from the middle of the part's register.

| style | the motif |
|---|---|
| `sustain`, `reese` | the root held for the whole chord |
| `pulse` | quarter notes |
| `root8` | eighths on the root, sometimes an octave jump at the end |
| `octave` | eighths alternating root and octave |
| `offbeat` | the off-beat eighths of house, sometimes a pickup on the last sixteenth |
| `rolling` | the three sixteenths after each beat, around the kick |
| `syncopated` | one of five sixteenth grooves, with octaves, fifths and thirds |
| `kick_follow` | a note on every kick of the kick lane's `main` pattern, held to the next, with occasional octave or fifth jumps and slides: the trap 808 |
| `acid` | sixteenths on a pentatonic, with rests, accents and slides, snapped to the key |
| `walking` | root, two chord tones and a chromatic approach to the next root |

A style the writer does not know plays `sustain`.

## Leads, arpeggios and chord parts

**Leads** (`melody::lead`) are built like a hook: a one-bar motif played in an A A' A B phrase every four bars.

1. The rhythm: 2 to 9 onsets in a bar, depending on density, drawn with weights that favour the strong sixteenths (or,
   for a syncopated lead, the off-beat ones), the first always on the downbeat.
2. The contour: steps in the scale, in one of four shapes (an arch, falling, rising, zigzag), with a leap of a third or
   fourth now and then, always followed by a step back the other way, and kept within a range of 12 scale steps.
3. The phrase: bar 2 (A') repeats the motif two steps higher (one step lower in the second half of an 8-bar
   phrase), bar 4 (B) is a sparser answer whose last note is held, landing on the motif's home note (the middle of
   the register, snapped to the scale) at the end of 8 bars and two steps above it otherwise.
4. Each note is snapped to the key, and notes on strong beats (beats 1 and 3, or a note held a beat or more) move to
   the nearest chord tone of the chord sounding then. Notes outside the register are folded back in by octaves.

**Arpeggios** (`melody::arp`) voice each chord in close position at the bottom of the register, spread it over two
octaves, and walk it `up`, `down`, `updown`, `random` or `pinky` (the top note alternating with the others), one note
every `rate` beats.

**Chord parts** (`melody::chord_part`) voice every chord with voice leading and play it in a rhythm: `sustain`,
`half`, `stabs`, `offbeat`, `pump` (eighths), `syncopated` or `charleston`.

## Sounds for parts and drums

`compose::part_sound(entry, rng)` turns a preset entry into a sound. For `"piano|epiano"` it plays the first
alternative that exists (a synth preset, or an instrument an installed pack answers to), and falls back to the `pluck`
synth. It always draws the variation of the last synth alternative first, whether or not that is the sound it
returns, and varies any other synth from a fork of the generator; instruments draw nothing. So the generator's
streams advance the same way with or without sample packs, and installing packs changes the sounds of a song without
changing its notes. (A synth patch is varied by moving its filter cutoff by a factor of 0.8 to 1.25 and its detune by
0.85 to 1.2.)

Drum kits work the same way: the kit is drawn from a fork, and `kit_sound` maps lanes to the kit's drums (`kick` to
`bd`, `hat` to `hh`, `perc` to `rim`, ...), so a missing kit leaves the lane's synthesized voice in place. Apple Loops
one-shots follow the same rule: each lane draws the chance of using one, and picks it, from a fork of its own
(`dr.fork(lane)`), and draws that chance whether or not there is a library. The drum stream that writes the patterns
advances the same way either way, so a seed gives the same drum patterns on every machine, and Apple Loops change only
which sounds play them. The generator snapshot tests still run without a library, so that they describe the
generator and not what is installed.

A kick with `tune_to_key` has its fundamental moved to the tonic or the fifth of the key, whichever lies nearest its
drawn tuning within the lane's range, so the kick and the bass agree.
