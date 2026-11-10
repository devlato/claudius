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
| `theory/rhythm.rs` | drum hits, lane templates, euclidean rhythms, rolls, swing, syncopation |
| `theory/drummer.rs` | how each genre's drummers end their phrases: turns and fills |
| `theory/idiom.rs` | idioms: cells, turns and phrases, written in notes over the chords, varied by take |
| `theory/bass.rs` | bass lines in the genres' idioms and twelve styles |
| `theory/comp.rs` | chord parts in the genres' comping idioms |
| `theory/lead.rs` | the genres' lead idioms |
| `theory/melody.rs` | leads' motifs, arpeggios and chord parts' rhythms |

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
| `palette` | by role, how a part's sound is drawn beyond its own presets: `core`, the chance of one of its own, a weight for each kind (`synth`, or an instrument category: `strings`, `brass`, `mallets`, `electric piano` ...), and `retro`, whether it plays the sounds of an era gone (synthwave's FM piano and string machine) |
| `parts` | melodic parts: a role, weighted-by-repetition lists of presets and styles, a register, a density range, voices, voicing, arpeggio rates, a chance of leaving the part out, its own `core` (an acid line stays acid), a mix, and `layers`, the sounds it stacks into a [rack](../language/sounds.md#layered-sounds): each `"$"` for the sound drawn or a synth preset by name, with a `band`, `level`, `duck`, `transpose` and `fx` (trap's 808 and the 808 saturated for grit, future bass's bass over a sine sub an octave down). A part that doubles it plays one sound |
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
| the first verse without one melodic part (never the bass); a later verse with one more track from the next hook; the first hook without one of its melodic parts (never the bass) | 0.3; 0.35; 0.25 |
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

**Builds.** `snare_roll` plays quarters, then eighths, sixteenths and thirty-seconds, rising in velocity, on the
snare (or the clap) over the last half of a build.

**Swing** delays every off-beat sixteenth: within each eighth, the second sixteenth moves from halfway to
`swing` of the way through (0.5 is straight, 0.66 a triplet feel). It is applied when notes are rendered, so the
blueprint stays on the grid.

**Measures** for the critic and analysis: `syncopation` (Longuet-Higgins and Lee: an onset on a weak position
followed by silence on a stronger one scores the difference in metrical level) and `density` (onsets per beat).

### Drummers

A pattern loops; a drummer ends phrases. Each genre has a drummer (`theory::drummer`), and in a section with `fills`
it ends the section's phrases as that genre's drummers do. At the end of four bars it plays one of its turns, small
and on a lane or two, and at the end of eight bars and of the section one of its fills (up to the beat of silence
before the next section, when there is one; a build ends on its roll instead). Each is a chance, as often as the
drummer plays them, and it never plays the same one twice running when it knows another. A turn or fill is drawn once
per phrase from `seed ^ hash("drummer") ^ section index` and played the same on every lane, so the kick and the snare
of a break trade where they should and the hats stop together.

Turns and fills are written lane by lane, in cells that end where the phrase does:

| step | what sounds |
|---|---|
| `X`, `x`, `o` | a hit: accented, played, a ghost note |
| `h`, `m`, `l`, `f` | a hit pitched high, mid, low, on the floor, as a drummer goes down the toms |
| `.` | a rest: what the lane played there is left out |

A cell is in sixteenths, unless its steps follow `3:` (eighth-note triplets), `6:` (sixteenth-note triplets) or `8:`
(thirty-seconds), and a `<` at its start plays them louder and louder: `<8:xxxxxxxx` is a beat of thirty-seconds
rising, a trap hat roll. A fill names its lanes by what they play, and plays on the song's lane that plays it or the
nearest it has: a snare fill on the clap, or the rim in reggae's kit; toms on the congas or the percussion, or
pitched on the snare in synthwave's, as its drum machine's toms are. A fill whose lane the song has nothing like
isn't played. While it plays, the lanes it leaves alone play on, or the drummer's hands leave the hats and the ride
(the toms down the kit), or every lane stops (a beat of silence for the tag).

| drummer | genres | turns | fills |
|---|---|---|---|
| `boom_bap` | boom bap, alt hip hop, chipmunk soul, gangsta rap, hip hop | a kick pushed onto the last sixteenth, a kick double on three, a ghost dragged into the backbeat | kick and snare trading eighths, the snare three times on four, the beat stopped after the kick on three, a beat of silence |
| `trap` | trap, cloud rap, emo rap, rage, plugg, phonk, crunk, hyperpop | a hat roll in thirty-seconds, hats in sixteenth triplets, a kick skip | snare triplets over triplet hats, a snare roll in sixteenth triplets, a beat or two dropped out |
| `drill` | UK, NY and Chicago drill | hats in triplets, hats skipping in sixteenth triplets, a kick slide | snare flams late in the beat, a kick and two triplet snares, a beat dropped out |
| `house` | house, deep house, tech house, filter house, electro house, progressive house, slap house | a clap on the last sixteenth, the kick out for a beat, the open hat lifted | a clap roll, the kick out for two beats, a conga run |
| `techno` | techno, dub techno, hard techno, melodic techno | the kick out for a beat, a percussion skip | the kick out for two beats or a bar, a soft clap rising |
| `trance` | trance, big room | the kick out for a beat, a clap on the last sixteenth | a snare roll through the bar, eighths into sixteenths, a roll with the kick out |
| `hard` | hardstyle, gabber | the kick out for a beat, the kick doubled on the last beat | the kick rolled in sixteenths, the kick in triplets, a snare roll through the bar, the kick out for two beats |
| `club` | Jersey club | the kick in triplets on the last beat, the snare doubled | the kick in triplets for two beats, a snare roll in sixteenth triplets, a beat of silence |
| `dnb` | drum and bass, jungle, breakcore | ghost notes after the backbeat, the kick on the and | a snare rush, a break edit, a thirty-second stutter |
| `reggae` | reggae | the rim into the next bar, the open hat on the and | a triplet roll into the drop, toms down in triplets |
| `dembow` | reggaeton, baile funk | the timbal's sixteenths, the rim pushed ahead | a timbal roll, a stop on the last beat, a snare rush |
| `afro` | afrobeats, amapiano, afro house | a conga pickup, the shaker out for a beat, the rim into the next bar | a conga run, the kick out under the log drum, a talking drum |
| `garage` | UK garage, grime | a snare skip, a kick skip, a rim skip | a shuffled snare run, a beat of silence |
| `halftime` | dubstep, future bass | the kick doubled, a ghost snare | snare triplets, a rush, a stutter, a beat of silence before the drop |
| `rock` | art rock, neo-psychedelia | the snare doubled, the kick pushed | toms down the kit, the snare into the toms, a bar of snare and toms, sixteenths on the snare |
| `funk` | funk, g-funk, new jack swing | ghost notes around the backbeat, kicks on the and and the a, a drag | sixteenths accented in threes, four on the snare, toms |
| `disco` | nu-disco, dance pop | the snare on the last sixteenth, the open hat on the and | sixteenths on the snare, four on the snare, toms |
| `jazzy` | lo-fi, jazz rap, neo-soul | ghost notes, a lazy kick on the and, the hats out for a beat | a soft drag, a soft roll, a beat of silence |
| `synthwave` | synthwave | the snare doubled, the kick pushed | toms down the kit, the snare into the toms, sixteenths on the snare |
| `breakbeat` | big beat, trip hop, IDM | ghost notes after the backbeat, the kick pushed | a break edit, a rush, a stutter, a beat of silence |
| `rnb` | R&B, 90s R&B, alt R&B | hats in sixteenth triplets, a ghost before the one | a drag, snare triplets, a beat of silence |
| `steady` | ambient, and a genre of your own | the kick out for a beat | a roll, triplets, a rush |

The drummer is the one of the genre the song's backbeat lane (its snare, else its clap) plays as, else the song's: drums
generated from another genre bring its drummer with them, and `lane("snare", #{ genre: "funk" })` gives a song
funk's fills. A fusion's drummer is its last genre's, which plays its drums.

## Bass lines

A bass part is written in one of the genres' [idioms](#bass-idioms) or in one of the motifs below (`bass::write`).

`bass::bassline` writes a one-bar motif once per part and transposes it onto every chord, which is how a producer
writes a bass line: one groove, followed through the changes. The motif's intervals are relative to the chord's bass
note, and some are resolved against the sounding chord (its third, its fifth), so the line fits every chord. The root
of each chord is the one nearest the previous root, unless that strays more than 7 semitones from the middle of the
part's register.

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
| `glide` | the same, long, gliding up an octave or to the fifth into the end of each two-bar phrase: a trap hook's 808 |
| `drill` | the same, moving through the key's tones over the root (its third, fourth, fifth, seventh, octave), sliding from note to note |
| `acid` | sixteenths on a pentatonic, with rests, accents and slides, snapped to the key |

A style the writer does not know plays `sustain`.

### Bass idioms

An idiom (`idiom::Idiom`) is the way a genre's bass players play, as they would: a few bars a player plays over a
chord (its cells), the bars they end a phrase on (its turns), how much of each step they hold a note for, and how often
they vary the details from bar to bar. A take plays one of the cells, answers it with another in the second bar of a
four-bar phrase half the time, and ends every phrase on a turn, so that a line has phrases instead of one bar over and
over. A cell is a bar of steps, sixteen sixteenths (or 12 or 24, in triplets):

| step | the note |
|---|---|
| `.`, `-` | a rest; the note before going on |
| `R`, `r` | the chord's root, its bass note; softly |
| `L`, `8` | the root an octave down, an octave up (the other way where that does not fit the register, and the fifth that way where neither does) |
| `3`, `5`, `7` | the chord's third, fifth and seventh above the root (the key's seventh on a chord with none) |
| `2`, `4`, `6` | the key's second, fourth and sixth above the root |
| `A` | an approach: a semitone under the note after it, or over it coming down, or from the other side when that is the note before (an enclosure) |
| `W` | a walk: the key's step from the note before towards the note after, or the semitone between them |
| `x` | a dead note: short and soft, on the pitch before |
| `'`, `~` | before a note, an accent; after one, a slide into the next |

Reggae's one drop, for one, is `.... R-.R -.5- 8-.7`: nothing on the one, the root on beat two and before three, the
fifth around the drop, the octave on four and the seventh leading down to the next bar. The details a take varies,
each with a chance of its own: a dead note in an empty step after a note, a root off the downbeat moved an octave, the
last note before a new chord turned into an approach to it, the new chord's first note pushed an eighth ahead of its
bar, a note off the beat left out, a slide into the next note. Notes are played harder on the downbeat, the beats and
the eighths than in between, and one at a time, each ending where the next begins unless it slides into it. Each
chord's root is the one nearest the note the line comes to it from, low enough in the register to leave room for the
octave above it (within an octave of the register's bottom at least, so that every root has a place).

A take's cells come from the high half of the part's seed and its details from the low half (and the progression), so
a variation of a take keeps its rhythm and changes what it plays around it, and a new seed is another take.

| idiom | how it plays | genres |
|---|---|---|
| `one_drop` | nothing on the one, roots, fifths and octaves around the drop on three, room between | reggae |
| `steppers` | a note on every beat, roots, fifths and octaves | reggae |
| `slap` | the one hit hard, octaves popped on the offbeat sixteenths, dead notes between | funk |
| `funk` | fingerstyle sixteenths: syncopated roots, fifths and octaves, ghosted, pushed into the changes | funk, G-funk, new jack swing, nu-disco |
| `disco` | root and octave in eighths, walking up into the next chord | nu-disco, filter house, dance pop |
| `garage` | skipping: long and short notes off the beat, octave jumps, swung | UK garage |
| `dembow` | the root on the three-three-two of the tresillo, held under the kick | reggaeton |
| `afro` | smooth and syncopated, on the tresillo's pushes, gliding into the next chord early | afrobeats, afro house |
| `log_drum` | short pitched hits off the beat, jumping octaves and fifths, bending now and then | amapiano |
| `walking` | a note on every beat, chord tones and walks, a chromatic approach into each change | jazz rap, lo-fi, boom bap, neo-soul |
| `neo_soul` | behind the beat and melodic, passing tones and ghosts, sliding into the chords | neo-soul, R&B, alternative R&B |
| `soul` | eighths through the chord tones, pickups into the changes | chipmunk soul, boom bap |
| `rnb` | smooth syncopated sixteenths, sevenths and octaves, slides into the next note | 90s R&B |
| `gfunk` | the Moog bass: laid back and syncopated, sliding up to the octave and back | G-funk, gangsta rap, alternative hip hop |
| `boom_bap` | the root held from the one, a pickup into beat three, long round notes | boom bap, hip hop, jazz rap, gangsta rap |
| `lofi` | sparse and behind the beat, a fifth on the and of two, an approach into the change | lo-fi |
| `two_step` | long notes on the kicks of the two-step, the one and the and of three | drum and bass |
| `jungle_dub` | a deep sub moving in half notes under the break, reggae's roots and fifths | jungle, breakcore |
| `halftime` | long notes and short restarts around the snare on three | dubstep |
| `dub` | deep and held, with room around it | dub techno |
| `house_organ` | short syncopated hits, three, three and two, around the four on the floor | house, deep house, electro house, filter house |
| `deep` | warm and longer, off the beat, the fifth at the end of the bar | deep house, afro house |
| `tech` | rubbery short sixteenths between the kicks, a groove more than a line | tech house |
| `new_jack` | punchy swung sixteenths, octaves and fifths | new jack swing, 90s R&B |
| `grime` | sparse square-wave stabs, a lot of room | grime |
| `trip_hop` | slow and heavy, the root held, a late second note | trip hop |
| `big_beat` | distorted, syncopated eighths and sixteenths with the octave | big beat |
| `rock` | eighths on the root, a fill up through the fifth into the next chord | art rock, neo-psychedelia |
| `drive` | sixteenths on the root, accents on the beats, up to the octave | synthwave |

## Leads, arpeggios and chord parts

**Leads** play one of the genres' [lead idioms](#lead-idioms), or `motif` (`melody::lead`), which is built like a
hook: a one-bar motif played in an A A' A B phrase every four bars.

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
`half`, `stabs`, `offbeat`, `pump` (eighths), `syncopated` or `charleston`; or in one of the genres' comping idioms.

### Comping idioms

The chord parts of a genre comp as its players do (`comp::comp`): idioms built as the [bass idioms](#bass-idioms)
are, cells and turns in phrases, a take's cells from the high half of its seed and its details from the low half, but
written in chords. Each chord is voiced once, leading from the one before, and a step plays it:

| step | what sounds |
|---|---|
| `C`, `c` | the chord, every voice (softly) |
| `T`, `B` | its top voice; its bottom voice |
| `1` to `6` | one voice, counted from the bottom (the top one, of a chord with fewer) |
| `U`, `D` | the chord strummed up from its bottom voice, down from its top, a few hundredths of a beat apart |
| `x` | a muted strum: short and soft |
| `.`, `-`, `'` | a rest; what sounds before going on; an accent on the next step |

A take varies a soft chord into an empty step after a hit, leaves a hit off the beat out, pushes a new chord's first
hit an eighth ahead of its bar, and on the last sixteenth before a new chord plays that chord a semitone up, slipping
into it, as gospel and funk players slide into their changes.

| idiom | how it plays | genres |
|---|---|---|
| `house_piano` | short stabs off the beat, three, three and two around the kick | house, deep house, tech house, UK garage, amapiano, dance pop, progressive house |
| `gospel` | chords pushed ahead of the beat, short and long, slipped into | neo-soul, jazz rap, funk, nu-disco, afrobeats, amapiano |
| `lofi_comp` | one chord on the one, another late in the bar, room between | lo-fi, jazz rap, boom bap, trip hop |
| `trap_keys` | the chord held from the one, its top voice picked out above it | trap, the drill genres, hip hop, cloud rap, plugg |
| `rnb_keys` | smooth extended chords, pushed into the next, a passing chord slipped in | R&B, 90s R&B, G-funk |
| `ballad` | the chord broken from its bottom voice to its top and back, let ring | art rock |
| `bubble` | reggae's organ: soft chords on the eighths, the offbeats pressed | reggae |
| `skank` | a short chord chopped on every offbeat | reggae, afrobeats |
| `scratch` | funk's chicken scratch: muted sixteenths, a chord hit hard on the backbeat | funk, nu-disco |
| `strum` | down on the beats, up on the offbeats, in eighths | neo-psychedelia, emo rap |
| `highlife` | a bright picked pattern through the chord's upper voices | afrobeats, afro house |
| `arpeggio` | the chord's voices one at a time, up and back | emo rap, neo-psychedelia |
| `legato` | strings: each chord held, bowed again in a phrase's last bar | neo-soul, G-funk, trip hop, ambient |
| `ostinato` | strings: one voice after another in short driving eighths | the drill genres, art rock, gangsta rap |
| `pizzicato` | plucked strings: short notes through the chord, room between | grime, UK drill |
| `disco_strings` | stabs off the beat, and a run up through the chord into the next phrase | nu-disco, chipmunk soul |
| `tremolo` | strings: the chord bowed in fast soft sixteenths, for tension | ambient, trip hop |
| `brass_stabs` | a horn section's stabs: short, hard and syncopated | big beat, crunk, new jack swing, Chicago drill, baile funk |

### Lead idioms

A lead plays its genre's way of writing a top line, in the same notes as the [bass idioms](#bass-idioms)
(`idiom::line`) in the lead's register: its tones are each chord's root, third, fifth, seventh and octave and the key's
steps above the root, so a riff follows the changes as a producer's does, and its phrases end on a turn. A cycle of a
lead is four bars at least, the progression played again until it is. A new seed is another take; a variation keeps
its cells and draws its octaves, slides, approaches and pushes again.

| idiom | how it goes | genres |
|---|---|---|
| `gfunk_whine` | a high portamento synth gliding between long notes, from the fifth up to the octave | G-funk, gangsta rap |
| `trap_bells` | a dark motif in eighths, the fifth and the third round the root, an octave now and then | trap, plugg, cloud rap, rage, hip hop, hyperpop |
| `drill_melody` | a piano or a flute bouncing in triplets, dark and sliding | UK, NY and Chicago drill |
| `afro_flute` | a bright call pushed ahead of the beat and answered, room between | afrobeats, amapiano, afro house |
| `dembow_hook` | the melody on the dembow's tresillo, three, three and two | reggaeton, baile funk |
| `synth_lead` | long notes leaping an octave, falling home at the phrase's end | synthwave, trance, neo-psychedelia, dance pop, drum and bass, progressive house, slap house, hardstyle, melodic techno |
| `trance_lead` | gated eighths up the chord to the octave and back, a falling run to end the phrase | trance, electro house, big room, progressive house, hardstyle |
| `organ_riff` | short notes off the beat, syncopated round the kick | house, deep house, nu-disco, UK garage, amapiano |
| `disco_hook` | bright octaves and chord tones in eighths, a run down to end it | nu-disco, dance pop, future bass, hyperpop |
| `horn_riff` | short and punchy, the root and the fifth with rests between, answered | boom bap, funk, jazz rap, reggae, big beat, crunk, new jack swing, baile funk |
| `reggae_horns` | a long, lazy line answering the singer, from the fifth down to the root | reggae, jungle |
| `riff` | eighths and sixteenths round the root, leaping an octave | grime, electro house, rage, dubstep, big beat, crunk, big room, progressive house, slap house, hardstyle, hard techno, gabber, hyperpop, jersey club, baile funk |
| `garage_chop` | a chopped vocal: short stabs skipping off the beat | UK garage, house, deep house, future bass, slap house, jersey club |
| `lofi_melody` | lazy and sparse, on the sevenths and ninths, slipping into the next chord | lo-fi, jazz rap, neo-soul, trip hop, 90s R&B, alt hip hop, breakcore |
| `rnb_runs` | long smooth notes, and a run down the scale at the phrase's end | R&B, 90s R&B, alt R&B, neo-soul, new jack swing, G-funk |
| `rock_lead` | a guitar's eighths up the chord and back, a bend into the root | art rock, neo-psychedelia, emo rap |
| `ambient_line` | a note now and then, held long, an octave away | ambient, cloud rap, plugg, alt R&B, art rock, emo rap, trip hop, drum and bass, jungle, IDM, afro house, melodic techno, breakcore |
| `techno_stab` | one note, hypnotic, off the beat or every three sixteenths against it, a fifth now and then | techno, hard techno, gabber |
| `phonk_cowbell` | a Memphis melody in sixteenths on a pitched cowbell | phonk |
| `piano_riff` | the 90s' dark piano: the fifth hammered, falling to the third and the root | gangsta rap, hip hop, trap, Chicago drill, boom bap, alt hip hop, breakcore |

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
instrument stands in with one the genre does not name, so a genre that names none of them plays none. Nor does it
play the presets that date a song to the 80s (`presets::DATED`: the FM tine piano, leads, basses and mallets, the
square and pulse-width leads and pads, the string machine and the chip) beyond those its parts name, unless its
palette for the role is `retro`: grime keeps its square lead and UK garage its FM bass, while a hip hop song draws
none of them.

- `synth`: one of those synths, varied widely (`compose::widen`: the filter, envelopes, detune, vibrato, FM, drive
  and glide each moved up or down by half again at most, now and then a voice more or less in the unison, or a soft
  octave above it). A part's own synth is varied too, by half as much.
- an instrument category (`strings`, `brass`, `mallets`, `electric piano` ...): any installed instrument of it whose
  recorded range holds most of the part's register (within 5 semitones of its outermost samples), whose articulation
  suits the role (a pad sustains: no pizzicato, no one-shot, no piano; a lead is one player, not a section; chords on
  strings, brass or woodwinds are the section's; a bass starts low enough), and whose pack's licence lets music made
  with it be published as one likes: packs without a licence, and share-alike or non-commercial ones, are only chosen
  by hand. The instruments of an era gone (`palette::dated`: an FM piano, a CP-80, a Pianet, and General MIDI's synth
  pads, strings, brass, leads and basses, the polyphony of an old sound card) play only where the genre names them or
  its palette is `retro`, and General MIDI's others (its upright piano, its guitars) only where nothing else of their
  kind is installed. With no such instrument installed, a synth stands in: one of those synths that stands in for the category
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
