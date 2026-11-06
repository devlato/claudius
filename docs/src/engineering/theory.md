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
| `forms` | one or more forms, each a list of sections with layers, energy and flags: templates, which each song [varies](#a-form-and-a-mix-of-its-own) |
| `drums` | lanes: a voice, a step template or a euclidean rhythm, named variants, parameter ranges, Apple Loops subtypes for one-shots, a choke group, whether to tune the kick to the key, a mix, and a `late_ms` range its feel is drawn from (ms behind the beat: a laid-back snare `[8, 24]`, a pushed kick `[-6, 0]`) |
| `kits` | weighted sample-pack drum kits; the weight left over keeps the lanes' own sounds |
| `kit_mix` | the chance that a drum lane plays a kit of its own instead (another installed kit, or its synthesized voice); 0.3 when not given |
| `palette` | by role, how a part's sound is drawn beyond its own presets: `core`, the chance of one of its own, and a weight for each kind (`synth`, or an instrument category: `strings`, `brass`, `mallets`, `electric piano` ...) |
| `parts` | melodic parts: a role, weighted-by-repetition lists of presets and styles, a register, a density range, voices, voicing, arpeggio rates, a chance of leaving the part out, its own `core` (an acid line stays acid), a mix |
| `loops` | Apple Loops parts: instrument, subtype, genre and mood filters, whether tonal |
| `transitions` | riser, impact, downlifter and reverse crash switches, the riser length |
| `mix` | the LUFS target, the reverb and delay, bus and master chains, sidechain release, per-role level offsets; each song [varies](#a-form-and-a-mix-of-its-own) the reverb, the delay, the sends and the colours |
| `query`, `alts` | what the ears should hear (`"house music"`) and the genres it is most often confused with |
| `accepted_flaws` | production flaws of the ears' that are the genre's sound, left out of its production reading (lofi's hiss) |

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
   Chord length comes from `bars_per_chord`; a one-chord progression lasts 4 bars, and a song with eight chords in
   `A` or `B` plays 2 bars a chord at most.
   Then, each from a fork of its own, a four-chord `A` is [answered](#progressions) with chance 0.4 and `B` with
   0.25, when the eight chords last 16 bars or less. When a key of your own has another mode, the progressions then
   move to it the way `key()` moves them: diatonic chords keep their degree and take the new mode's quality,
   borrowed chords keep their sound spelled in the new mode, and applied chords stay as written;
3. **form**: one of the genre's forms, made the song's own by `form::vary` ([below](#a-form-and-a-mix-of-its-own)),
   then every section halved (to at least 4 bars) with `--short`; a form asked for by number (`Options::form`) plays
   as written. In a form with verses and hooks, the hooks play a chorus of their own with chance 0.35: progression
   `C`, another progression of the style or a substitution of `A`, never what `A` or `B` play (as drawn or answered,
   or the first half of their eight chords) and no longer a loop than `A`'s or 16 bars, worked out in the style's
   mode and moved to the key like the others;
4. **drums**: per lane, a one-shot from the library (with chance `samples`, 0.75 unless `--samples` or a script says
   otherwise, the same for `make`, scripts and the studio's new songs, when there is a library) or a
   synthesized voice with parameters in the genre's ranges (or a third of the way from the voice's default towards a
   random point), the kick tuned to the key; then a 4-bar pattern per variant from the lane's template or euclidean
   rhythm; then, when a kit was drawn, the kit's drum for the lane;
5. **parts**: per part, unless its `optional` chance leaves it out, a preset, a style, a density and a seed of its
   own. A chords, keys or pad part on a single-voice synth patch, or on an instrument recorded in mono (FreePats' FM
   piano, say), gets a light chorus so it does not sit dead centre;
6. **loops** from the library, when there is one;
7. **transitions**: a crash (recorded or synthesized) and the genre's switches;
8. the **mix** from the genre, made the song's own by `colour::vary` ([below](#a-form-and-a-mix-of-its-own)), and a
   **title** from two word lists.

With `--loops` (a loop family), the song is generated with the family's tempo and key, both progressions become the
chords read from the family's harmony loop, one chord per bar, and the family's chord, bass and beat loops replace the
parts and lanes of those names.

### A form and a mix of its own

A genre's forms are templates. `form::vary` changes one for its song, each change drawn with a chance of its own, so
that songs of a genre rarely share a form (24 songs of house, lofi, trap, dance-pop or techno play at least 18):

| change | chance |
|---|---|
| a section another length its kind takes: an intro or outro 4, 8 or 16 bars, a verse, hook or break 8 or 16, a build 4 or 8, a drop 16, 24 or 32, a groove 8, 16 or 32 | 0.45 a section |
| a third verse and hook, after the last hook before the first break (or after the last hook) | 0.3 |
| a bridge, 8 bars of the melodic parts without drums or bass on progression `B`: before the last hook or drop (and the build into it), in a song of two or more with no break between the first and the last, or after a song's only hook or drop when more than the outro follows | 0.35 |
| the last hook or drop played twice, the second at full energy | 0.3 |
| a lift, a 4-bar build without the kick at the energy of the section before it, into a first hook that has no build, in a genre with risers | 0.35 |
| no intro (a cold start); an intro of only its drums (when they have the kick) or only its melodic parts | 0.15; 0.25 |
| no outro (the song ends on its last section); an outro of half its tracks | 0.15; 0.25 |
| the first verse without one melodic part; a later verse with one more track from the next hook; the first hook without one of its melodic parts | 0.3; 0.35; 0.25 |
| fills switched in a verse, groove or drop; a gap switched before a hook or drop; the intro's filter sweep switched | 0.25; 0.3; 0.4 |
| each section's energy moved | up to ±0.05 |

The song then keeps to 2 to 5½ minutes at its tempo, its longest sections shortened or its shortest lengthened, and
each repeated section gets a name of its own (`verse3`, `chorus4`). A section only leaves out what it can: a
stripped intro, a thinned outro or a bridge keeps a drum lane or a part that every song of the genre has, never only
an optional part or a loop. A section's energy moves with what it plays, so that the [critic](critic.md) still hears
its loudness follow its energy: 0.15 for a kick gained or lost, 0.1 for a bass, 0.05 for anything else, at most 0.2
for a section left without drums, and 0.1 less for an intro whose filter sweep is switched on (0.1 more when it is
switched off).

`colour::vary` does the same for the mix:

- the reverb: its decay up to 1.5 times longer or shorter (0.6 to 8 s), its damping ±0.15, its pre-delay −10 to
  +25 ms, its width and its low cut;
- the delay: half the time another length (a sixteenth, a dotted sixteenth, an eighth, a dotted eighth, a quarter or
  a dotted quarter), its feedback, and ping-pong switched three times in ten;
- each part's reverb and delay sends;
- four parts in ten get a colour by role, unless they already have an effect of its kind: on a pad a chorus, phaser,
  flanger, slow autopan or more width; on chords and keys a chorus, phaser, tremolo, autopan or tape; on a lead tape,
  a tube, a chorus, phaser or flanger; on an arp an autopan, bitcrusher, phaser or chorus; on a bass a tube, tape or a
  little wavefolding. A phaser's or bitcrusher's mix rises in the hooks and drops;
- three songs in ten high-pass the music bus in the breaks, at 250 to 500 Hz;
- half the time, the master's saturation becomes tape, a tube or tanh.

Both draw from forks of their own, one for each change, so a song's tempo, harmony, drums and parts stay what its
seed makes them whatever its form and mix draw, and a new or changed rule leaves the other changes' draws as they were.

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

`power` is the exception: a power chord, as a rock guitar plays it, is the root, the fifth above it and, with three
voices and room, the octave. It has no third, so a distorted guitar stays clear. Its root moves to the root nearest
the last one's.

The critic checks the result: `voice_leading_<track>` is the average movement per voice per chord change, and above
4.5 semitones it raises an issue.

## Progressions

**The library** in `progression::STYLES` has eleven styles, each a mode and five to thirteen progressions:
`minor_epic`, `minor_dark`, `minor_soul`, `dorian_vamp`, `phrygian`, `harmonic`, `minimal`, `major_pop`,
`major7_jazz`, `minor_80s` and `mixolydian`. For example `minor_soul` holds `i7 iv7 VII7 IIImaj7` and
`iiø7 V7 i7 i7`; genres weight the styles they use (house: `minor_soul` 0.35, `minor_epic` 0.25, `dorian_vamp` and
`major7_jazz` 0.2 each). Some progressions bend the mode as songs do: borrowed chords (`I iii IV iv`,
`I bVI bVII I`), applied ones (`I V/vi vi IV`, `Imaj7 VI7 ii7 V7`), a tritone substitute (`ii9 bII7 Imaj9 Imaj9`),
the Neapolitan (`i VI bII V`), and two of eight chords (`I V vi iii IV I IV V`). `every_library_progression_reads_in_its_mode`
in `tests/theory.rs` reads every numeral of every style in all twelve keys.

**An answer** (`progression::answer`) is what a player does with a four-chord loop the second time round: the loop,
then the loop again with another way back to its first chord. The ways, weighted:

| way | when | weight | example, the second time round |
|---|---|---|---|
| the dominant of the first chord | the first chord is not diminished | 2 | `I V vi IV` → `I V vi V7` |
| in a major key: the flat seventh, the minor fourth, or the flat sixth and seventh, borrowed from the parallel minor | the loop starts on the tonic | 1 each | `I V vi IV` → `I V vi bVII`, `I V vi iv`, `I V bVI bVII` |
| in a minor key: the major fourth (as dorian has it), the sixth and seventh, or the flat second (as phrygian has it) | the loop starts on the tonic | 1, 1, 0.5 | `i iv i v` → `i iv i IV`, `i iv VI VII`, `i iv i bII` |
| a suspension of the last chord | its fourth and fifth are in the key | 1 | `I vi IV V` → `I vi IV Vsus4` |

A way the loop already ends on is no answer. In a loop of sevenths, a borrowed chord comes as the mode it is
borrowed from has it (`bVII7` from the parallel minor, `IV7` from dorian, `bIImaj7` from phrygian). Half the time one
chord between the first and the new ending is coloured as well: a seventh or ninth with the key's own notes, or a
suspension whose notes are in the key. `answer` says what it did (`answered, ending on the minor fourth, borrowed
(iv), chord 7 coloured (vi7)`).

**The Markov generator** walks a 7 by 7 table of transition weights between scale degrees, one for major and one for
minor keys, written by hand around functional harmony: tonic to subdominant to dominant to tonic, plus the stepwise
and third moves pop and EDM use. A progression starts on the tonic (three times in four) or the sixth degree, never
repeats a chord immediately (its weight is cut to a tenth), and is accepted only when its last chord leads back to
its first with a weight of at least 1; otherwise it tries again, up to 64 times. The chords are the diatonic chords of
those degrees, extended to sevenths or ninths by the genre's `ext` (a major chord whose seventh is minor becomes a
dominant seventh, and a chord whose ninth is not in the key, such as the fifth of a minor key, stays a seventh).

**Substitutions** change one chord in a way a musician would, and say what they did, for the song's history (the
examples are on `i VI iv VII` in A minor):

| substitution | example |
|---|---|
| a chord a third away (up a sixth or up a third, in the scale) | `chord 2: VI -> iv (third relation)` |
| richer or plainer: a triad gains the key's own seventh or ninth (on the fifth of a major key a dominant seventh, never a major one), an extended chord loses it | `chord 1: i -> i7 (extended)` |
| the chord before becomes the secondary dominant of this one | `chord 3: iv -> IV7 (secondary dominant of VII)` |
| modal interchange: the same root with the opposite third | `chord 4: VII -> vii (borrowed from the parallel mode)` |
| a suspension (sus2 or sus4) whose notes are in the key, never on diminished chords | `chord 2: VI -> VIsus2 (suspension)` |
| swap two neighbouring chords | `swapped chords 2 and 3` |
| rotate the progression | `rotated the progression by 2` |

`generate` uses them to derive progression `B` from `A` (and `C`, a chorus of its own, half the time), and
evolution's `harmony_sub` operator applies one at a time.

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

**Trigs.** A hit may carry a `Trig`, what its step does beyond striking, as a drum machine's does. `arrange` plays a
lane's pattern round by round over each section (`lane_notes`):

- **When.** A step's condition counts the times round from the section's start (`Cond::plays`), and its chance is
  drawn for each time round from a generator seeded by the song, the lane, the section, the time round and the step.
  So a song sounds the same in every render, and different from bar to bar.
- **How.** A step that plays becomes its strokes (`Trig::strokes`): a flam's grace stroke the flam's milliseconds
  before at half the velocity, then the ratchet's strokes spread evenly over the sixteenth, their velocities ramping
  up to the hit's or down from it by three quarters at most.
- **The sound.** Its pitch goes on the strokes' notes. Its nudge, decay, tone and pan go with them as a `Lock`, and
  render adds the nudge after swing (swing moves only notes exactly on an off-beat sixteenth). A synthesized drum's
  decay and tone are its own settings, rendered and cached as another sound; a sample is cut shorter by a falling
  curve and blended towards itself low-passed at 2 kHz (darker) or away from it (brighter). The pan is a balance.

**A lane's own time.** A time round of a lane lasts its `period`: its own `length` in sixteenths (at most its
pattern's; else its pattern's bars), divided by its `speed`. `lane_notes` plays the pattern's hits up to that length, their times divided by the
speed, round after round over the section, so a lane twelve steps long against the others' sixteen is a polymeter,
and conditions count its own times round. A lane's own `swing` goes on its track (`Track.swing`), and render uses it
for that track's notes, its chokes and the duck curves it drives, in place of the song's.

**Grooves.** A `Groove` is a timing in beats and a velocity factor for each of the sixteen sixteenths of a bar
(`rhythm::groove` names claudius' own; the song keeps others in `Blueprint.grooves`). `Groove::apply` moves a stroke
exactly on a sixteenth of the song by `groove_amount` of its position's timing, and scales its velocity by
`groove_amount` of the way to its accent; strokes off the grid (32nds, triplets, a ratchet's inner strokes, a flam's
grace stroke) keep their place. A lane in a groove plays straight unless it has a swing of its own, so the song's
swing doesn't add to the groove's on the sixteenths it leaves where they are.

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

`compose::draw_sound` chooses a part's sound. It first draws one of the part's own presets with
`compose::part_sound(entry, rng)`: for `"piano|epiano"` that plays the first alternative that exists (a synth preset,
or an instrument an installed pack answers to), and falls back to the `pluck` synth. It always draws the variation of
the last synth alternative first, whether or not that is the sound it returns, and varies any other synth from a fork
of the generator; instruments draw nothing. So the generator's main stream advances the same way with or without
sample packs, and installing packs changes the sounds of a song without changing its notes.

Then, from a fork of its own, it draws whether the part keeps that sound: the genre's `palette` for the role gives the
chance it does (`core`), else a kind by its weight (`palette.rs`). What it draws is what its genre has a place for,
first of all the synths that fit the genre's parts of the role (`compose::fitting_synths`): the presets those parts
name (for `"vibraphone|mallet"`, the mallet) and the others of their families (`presets::FAMILIES`: the two electric
pianos, the soft leads, the analog basses ...), any of the synth pads for a pad, or the palette's own `synths` where
it lists them. A bell, a whistle, a chip tune's square, an 808 and an acid line are families of one, and no kind of
instrument stands in with one the genre does not name, so a genre that names none of them plays none.

- `synth`: one of those synths, varied widely (`compose::widen`: the filter, envelopes, detune, vibrato, FM, drive
  and glide each moved up or down by half again at most, now and then a voice more or less in the unison, or a soft
  octave above it). A part's own synth is varied too, by half as much.
- an instrument category (`strings`, `brass`, `mallets`, `electric piano` ...): any installed instrument of it whose
  recorded range holds most of the part's register (within 5 semitones of its outermost samples), whose articulation
  suits the role (a pad sustains: no pizzicato, no one-shot, no piano; a lead is one player, not a section; chords on
  strings, brass or woodwinds are the section's; a bass starts low enough), and whose pack's licence lets music made
  with it be published as one likes: packs without a licence, and share-alike or non-commercial ones, are only chosen
  by hand. With no such instrument installed, a synth stands in: one of those synths that stands in for the category
  (an FM electric piano for `electric piano`, a mallet or a marimba for `mallets`, a brass patch for `brass`, the
  whistle for `woodwinds`), else, for a category that does not pierce a mix, one made for the role (the clavinet for
  a keys part's `guitar`, the vocal lead for a lead's `voice`); else none, and the category is not drawn.

  The categories whose sound pierces a mix, `mallets`, `woodwinds` and `brass` (`palette::PIERCING`), play only where
  the genre plays one of their kind itself: where one of those synths stands in for them, or the instrument is one
  the genre's parts name (`"sax|brass_lead"`: an installed saxophone; `compose::named_instruments`). No genre plays the
  whistle, so an installed flute, oboe or harmonica plays only where a part names it. An instrument whose category
  holds more than one character plays only where the synth it sounds like does (`palette::character`): a
  glockenspiel, tubular bells or a xylophone where the genre plays the bell, a vibraphone where it plays the mallet.

No two parts of a song play the same sound (`palette::identity`): a draw leaves out what the others play, and a part
whose own sound is another's draws from the palette. The genre's own parts weigh it all: a lead keeps its own sound
three times in ten in every genre, an acid line always (its `core` is 1), trap's 808 nineteen times in twenty.

Drum kits work the same way: the kit is drawn from a fork, and `kit_sound` maps lanes to the kit's drums (`kick` to
`bd`, `hat` to `hh`, `perc` to `rim`, ...), so a missing kit leaves the lane's synthesized voice in place. Then each
lane, from a fork of its own, plays a kit of its own with the genre's `kit_mix` chance: any installed kit whose licence
lets music be published freely and that has the lane's sound, or its synthesized voice, so a song's kick, snare and
hats can come from different machines. Apple Loops
one-shots follow the same rule: each lane draws the chance of using one, and picks it, from a fork of its own
(`dr.fork(lane)`), and draws that chance whether or not there is a library. The drum stream that writes the patterns
advances the same way either way, so a seed gives the same drum patterns on every machine, and Apple Loops change only
which sounds play them. The generator snapshot tests still run without a library, so that they describe the
generator and not what is installed.

A kick with `tune_to_key` has its fundamental moved to the tonic or the fifth of the key, whichever lies nearest its
drawn tuning within the lane's range, so the kick and the bass agree.
