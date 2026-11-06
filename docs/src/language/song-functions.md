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
| `sampled` | bool | as the genre is | build it on samples gathered from free libraries: a phrase it takes its key, tempo and chords from, a break, a texture ([Songs built on samples](../guide/online-samples.md#songs-built-on-samples)); trip-hop, boom bap, lo-fi, jungle, big beat and filter house are by default |
| `sample_search` | string | none | play the phrases gathered for these words ("70s soul"); implies `sampled` |
| `complexity` | number, 0 to 1 | the genre's own (0.5 for most) | how [layered](../learn/producers/genres-and-evolution.md#how-layered-a-song-is) it is: 0 the genre's core, 1 its harmony and lead doubled, a counter-melody, a pad, more percussion |

A script downloads nothing: `sampled` and `sample_search` play what `new_song`, `claudius make` or
`claudius samples gather` gathered before, and a song with none at hand plays the genre's own sounds in their place.

The genre is one of `house deep_house techno trap lofi dnb future_bass synthwave trip_hop big_beat electro_house
nu_disco filter_house dance_pop neo_psychedelia art_rock idm dub_techno boom_bap uk_drill dubstep uk_garage jungle
afrobeats amapiano reggaeton trance neo_soul hip_hop alt_hip_hop rnb funk tech_house ambient grime phonk reggae`, two or three of them fused (`trip_hop+idm`:
the first leads, the last plays the drums; see [Fusing genres](../learn/producers/genres-and-evolution.md#fusing-genres)),
or one of your own (see
[Genres](../learn/producers/genres-and-evolution.md)); case, spaces and hyphens do not matter (`"deep house"`). A
negative seed acts as 0. `bpm` and `key` change only the tempo and the key: the title, the form, progression `A` and
the drum patterns are the ones the seed gives without them. A key in another mode than the seed's moves the chords
into that mode, as [`key()`](#key) does. The options are checked like [`tempo()`](#tempo) and
[`key()`](#key) check their arguments, and an option the map should not have is an error rather than ignored (so is a
misspelt one).

Errors: `unknown genre 'polka' (try: afrobeats, alt_hip_hop, amapiano, ambient, art_rock, big_beat, boom_bap, dance_pop, deep_house, dnb, dub_techno, dubstep, electro_house, filter_house, funk, future_bass, grime, hip_hop, house, idm, jungle, lofi, neo_psychedelia, neo_soul, nu_disco, phonk, reggae, reggaeton, rnb, synthwave, tech_house, techno, trance, trap, trip_hop, uk_drill, uk_garage)`;
`tempo between 40 and 240 BPM`; `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")`;
`complexity from 0 (sparse) to 1 (layered)`;
`song() has no option 'tempo' (options: seed bpm key loops short samples sampled sample_search complexity)`.

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
becomes `iv7` in minor. A borrowed chord keeps its sound and is spelled in the new mode: `bIII` in F major (A flat)
becomes `III` in F minor, where it is diatonic. Applied chords (`V/V`) stay as written. Because a borrowed chord can
become diatonic, going to minor and back does not always restore the original: `bVII` becomes `VII` in minor, and
`VII` becomes `vii°` back in major. Tonal Apple Loops in the song are repitched the shortest way (at most six
semitones).

Errors: `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")`.

```js
// G dorian to A minor: progression B, "i7 ii7 i7 VIImaj7", becomes "i7 iiø7 i7 VII7"
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

How loosely the song is played, applied at render: each hit drifts off the grid as a player's does, and its velocity
varies by 5 % × *amount*. A player's hits are not off at random: they leave the grid by 10 to 20 ms, drifting with the
hits around them, ahead for a while and behind for a while, over tens of seconds (a 1/f noise, as Hennig et al. found
in "The nature and perception of fluctuations in human musical rhythms", PLOS ONE, 2011, where listeners preferred
it to a humaniser's independent jitter). So a hit drifts by about 15 ms × *amount* (one standard deviation, three at
most): the kick by half of that, the snare and clap by 80 %, the hats by all of it, the rest of the percussion by
120 %, and a part by 60 %. The drum lanes share part of their drift, as one drummer's limbs, and each part has its
own. The drift is drawn from the song's seed, so it is the same on every render. For a lane or part that plays behind
or ahead of the beat on purpose, every hit alike, see `late` in [`lane()`](#lane) and [`part()`](#part).

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
hat ohat shaker perc. drum_sound("bongo", "tr909:lt") makes a lane of any name with its sound, and track("bongo",
s("...")) a pattern track`.

```js
drum("kick", "X...x...X...x..x");                     // a pickup into the next bar
drum("hat", "sparse", "..x...x...x...x.");
drum("hat", "main", "x.x.x.x.x.x.x.x.x.x.x.x.x.x.xxxx", 32);
```

### `trig()`

```js no-run
trig(lane, step, settings)            // the "main" variant
trig(lane, variant, step, settings)   // step: sixteenths from the pattern's start, from 0; settings: a map
```

Sets what a step does beyond striking, as a drum machine's trig does: when it plays, how it strikes, where, and what
it changes in the sound for that stroke. `step` counts sixteenths from the start of the lane's pattern: 0 is the
downbeat, 4 beat 2, 8 beat 3, 12 beat 4, 16 the next bar's downbeat. A fraction reaches a 32nd or a triplet step. A
step without a hit gets one, and an empty map clears the step's trig.

| setting | range | what it does |
|---|---|---|
| `v` | 0.05 to 1 | the hit's velocity |
| `prob` | 0 to 1 | the chance it plays, drawn afresh each time round the pattern, the same in every render of the song |
| `cond` | `"always"`, `"1:2"` ... `"16:16"`, `"first"`, `"last"`, `"!first"`, `"!last"` | when it plays, counting times round from each section's start: `"1:2"` the first of every two, `"4:4"` the fourth of every four, `"last"` only the last time round in the section |
| `ratchet` | 1 to 8 | strokes within its sixteenth: 2 are 32nds, 3 a triplet, 4 64ths |
| `ramp` | -1 to 1 | how a ratchet's velocity moves: 1 swells to the last stroke, -1 fades from the first |
| `flam` | 0 to 60 | a grace stroke that many ms before it, at half its velocity |
| `nudge` | -0.5 to 0.5 | where it lands off the grid, in sixteenths: below 0 early, above late (after swing) |
| `pitch` | -24 to 24 | semitones, for that stroke |
| `decay` | 0.1 to 4 | its decay, times the lane's: a synthesized drum rings for that long, a sample is cut shorter (it cannot ring longer than it is) |
| `tone` | -1 to 1 | darker below 0, brighter above |
| `pan` | -1 to 1 | a balance for that stroke: -1 all left |

```js
song("trap", 2);
trig("hat", 6, #{ ratchet: 4, ramp: 1 });                     // a roll that swells into beat 3
trig("hat", 14, #{ ratchet: 3, ramp: -1, cond: "1:2" });      // a fading triplet, every other bar
trig("snare", 12, #{ flam: 18 });
trig("kick", 10, #{ v: 0.7, prob: 0.5, cond: "last", pitch: -2 });
```

[`drum()`](#drum) writing a lane's pattern again keeps the trigs of the steps that still strike. The printed script
writes each trig after its pattern, with only the settings it changes. Errors: `trig() has no option 'ratchets'
(options: v prob cond ...)`, `trig()'s ratchet is 1 to 8, not 12`, `trig(): hat's steps go from 0 to 15 (sixteenths),
not 64`, `trig(): hat has no variant 'fill' (variants: main sparse)`, and a condition that does not read: `a step
plays always, a:b (1:2, 3:4 ...: the a-th time round of every b, b up to 16), first, last, !first or !last, not
'often'`.

### `lane()`

```js no-run
lane(lane, settings)   // settings: a map
```

Gives a drum lane a time and feel of its own, apart from the song's:

| setting | range | what it does |
|---|---|---|
| `length` | 0, or up to its pattern's sixteenths | how many sixteenths it plays before it starts again; 0 is its pattern's length. 12 against the bar's 16 is a polymeter: the lane comes round every three beats while the others take four. For a lane longer than a bar, write its pattern that long first with [`drum()`](#drum) |
| `speed` | 0.125 to 4 | how fast it runs through its steps: 2 plays them as 32nds, 0.5 as eighths, 0.75 stretches a bar of steps over a bar and a third |
| `swing` | 0.5 to 0.8, or 0 | its own swing, as [`swing()`](#swing) sets the song's; 0 gives it the song's again |
| `groove` | a groove's name, or `""` | the groove it plays in: one of claudius' own (below) or one of the song's from [`groove()`](#groove); `""` none |
| `groove_amount` | 0 to 1 | how much of the groove: 0.5 moves each stroke half as far and accents it half as much |
| `late` | -80 to 80 ms | how far behind the beat every hit plays, alike, as a drum machine's shift timing moves a pad; below zero, ahead. A snare 20 late lays the backbeat back, a kick 5 early pushes it: the drunk feel J Dilla made on an MPC has the kick early and the snare late, the hats on the grid. Unlike a groove it is in milliseconds, the same at any tempo |

A groove moves each sixteenth of the bar by its own amount and accents it by its own factor, as a drum machine's groove
template does. Only strokes exactly on the song's sixteenths take it: a 32nd, a triplet or a ratchet's inner strokes
stay where they are. A lane in a groove takes its timing from the groove rather than from the song's swing, unless it
has a swing of its own.

| groove | feel |
|---|---|
| `straight` | on the grid |
| `swing54`, `swing58`, `swing62`, `swing66`, `swing71` | each second sixteenth of an eighth that far into the eighth, as a drum machine's swing setting; 66 is a triplet |
| `shuffle` | the off-beat eighths and sixteenths on triplets, a little softer than the beats |
| `dilla` | the off-beats and the backbeat dragged, each by its own amount, and the downbeats leaned on |
| `push` | everything but the beats a little early |
| `lazy` | everything but the beats a little late |

```js
song("house", 3);
lane("hat", #{ length: 12 });                            // round every three beats, against the kick's four
lane("shaker", #{ speed: 1.5, swing: 0.6 });
lane("perc", #{ groove: "dilla", groove_amount: 0.7 });
```

The printed script writes a lane's settings after its pattern, with only those that differ from the song's. Errors:
`lane(): no drum lane 'bongo' (lanes: kick clap hat ohat shaker perc)`, `lane(): hat's length is up to its pattern's
16 steps (a longer pattern first, with drum()), not 24`, `lane()'s speed is 0.125 to 4, not 8`,
`lane()'s swing is 0.5 (straight) to 0.8, or 0 for the song's, not 0.3`, `lane(): no groove 'jazz' (grooves: straight
swing54 ...)`, `lane() has no option 'steps' (options: length speed ...)`.

### `groove()`

```js no-run
groove(name, file)        // file: a recording with drums in it
groove(name, file, bpm)   // bpm: its tempo, when known
groove(name, table)       // table: #{ timing: [16 numbers], accent: [16 numbers] }
```

Keeps a groove with the song as `name`, for [`lane()`](#lane) to put lanes in. From a recording, claudius finds the
tempo and beat grid of its drums, sets every hit against the sixteenth nearest it, and measures where each of the
sixteen sixteenths of the bar lands (the median of its hits' distances from the grid) and how hard (their mean loudness
against the recording's). The beats themselves are put back on the grid, so a downbeat found a little early or late
doesn't move the whole groove. The log says what it heard: `groove funk, heard in loops/funk.wav at 96 BPM: the
off-beat sixteenths at 58% swing, the backbeat 9% of a sixteenth behind, the accents on 1, 2, 3e`.

The recording's tempo is `bpm` when given, else one written in the file's name (`Funk Break 92bpm.wav`,
`loop_85_BPM.aif`), else heard. Heard, a loop's tempo can come out twice or half what it is, as its drums sit on
sixteenths at either, and then the groove is measured on the wrong sixteenths: give the tempo when you know it.

A table writes a groove out: `timing` in beats for each sixteenth (below 0 early, -0.25 to 0.25), `accent` a factor on
each one's velocity (0 to 2). Either can be left out: on the grid, all 1. The printed script writes the song's grooves
as tables before its drums, so it reads back without the recording.

```js
song("lofi", 2);
groove("lean", #{
    timing: [0, 0.04, 0, 0.05, 0.02, 0.04, 0, 0.05, 0, 0.04, 0, 0.05, 0.02, 0.04, 0, 0.05],
    accent: [1, 0.7, 0.9, 0.75, 1.1, 0.7, 0.9, 0.75, 1, 0.7, 0.9, 0.75, 1.1, 0.7, 0.9, 0.75]
});
lane("hat", #{ groove: "lean" });
lane("snare", #{ groove: "lean", groove_amount: 0.5 });
```

Errors: `groove()'s timing is a list of 16 numbers, one a sixteenth, not 2`, `groove()'s accent holds 0 to 2, not
3`, `groove()'s tempo is 40 to 250 BPM, not 400`, a file that can't be read, and one without enough drums in it: `too
few drum hits to hear a groove in (3): it wants a beat, a few bars of it`.

### `drums_from()`

```js no-run
drums_from(file)             // file: a recording of a beat, played or beatboxed
drums_from(file, settings)   // settings: #{ bars, variant, bpm }
```

Writes the beat in a recording into the song's drum lanes. claudius hears each hit and names it a kick, a snare or a
hat, sets the hits on the recording's own grid (its tempo `bpm`, else one in the file's name such as `92bpm`, else
heard; its own downbeat), and folds every time round into a pattern of `bars` bars: 1, 2 or 4, the recording's length
up to 4 when not given. Each kind goes into the song's lane of that role, the snares into its snare lane, else its
clap's, as their `variant` patterns (`main`), and a song without such a lane gets one. A step hit in only some of the
times round plays with that chance, as a [`trig()`](#trig) `prob`. The log says what went where: `beat.wav into drums,
2 bars as their main patterns: kick 6 steps into kick, snare 4 steps into clap, hat 16 steps into hat (2 by chance)`.

[Takes](../guide/samples.md#turning-a-take-into-drums) turn into drums the same way, from the studio or over MCP, on
the song's grid from the take's first beat. The printed script writes the lanes as patterns, so it reads back without
the recording.

Errors: `a beat's pattern is 1, 2 or 4 bars, not 3`, `drums_from() has no option 'beats' (options: bars variant
bpm)`, `drums_from()'s bpm is 40 to 250, not 400`, a file that can't be read, one too short or too quiet to hear a
beat in (`too few drum hits to hear a groove in (0)`), and one without drums: `beat.wav has no drums claudius can
hear: it wants a beat, kicks, snares and hats, played or beatboxed`.

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

On a lane the song does not have, `drum_sound()` adds it, playing the sound and silent until `drum()` writes its
steps, under any name: `drum_sound("bongo", "tr909:lt"); drum("bongo", "..x...x...x.x...");` (`drum()` alone makes
lanes only for drum names). It joins the sections the way a new [track](#track) does. A song printed as a script
writes such a lane's sound before its steps.

Errors: `'lead' is a track of the song that is not a drum lane`; `'pad' is not a drum sound (a drum name, a kit
sound, or a sample)` for a synth preset; the [sound errors](sounds.md#how-a-name-is-resolved) for a name that does
not resolve.

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

With a `role`, `part()` adds a part of your own under any name: `part("lead2", #{ role: "lead" })` is a second lead,
written the way the genre writes its lead (its styles, register and mix, and a sound of its own), and
`part("keys", #{ role: "keys" })` on a techno song, which has no keys, gets the role's styles and synth presets. A
role the part already has changes nothing; another is an error. A song printed as a script writes the role of every
part the genre does not have, so it builds again.

| field | type | effect |
|---|---|---|
| `role` | string | `bass`, `chords`, `pad`, `keys`, `lead` or `arp`: a part of your own, when the song has no part of that name |
| `style` | string | how the part is written (below) |
| `preset` | string | a synth preset or a sampled instrument; naming the current preset keeps its patch |
| `register` | `[low, high]` | MIDI range the part is written in |
| `density` | number, clamped to 0..1 | how busy a lead is |
| `seed` | integer | a new seed rewrites this part, and only this part |
| `rate` | number, clamped to 1/32..4 | arpeggio step in beats (0.25 is sixteenths) |
| `doubles` | string | another part whose notes this one plays, on its own sound: a double; `""` makes it play its own again |
| `octave` | integer, -2 to 2 | how many octaves from that part's notes a double plays them |
| `late` | number, -80 to 80 | how far behind the beat it plays, in milliseconds (ahead, below zero): a bass pulled 15 behind the drums, as a neo-soul bassist plays |

| role | styles |
|---|---|
| bass | `sustain`, `pulse`, `root8`, `offbeat`, `octave`, `rolling`, `syncopated`, `kick_follow` (follows the kick, 808 style), `acid`, `walking`, `reese` |
| chords, pad, keys | `sustain`, `half`, `stabs`, `offbeat`, `pump`, `syncopated`, `charleston` |
| arp | `up`, `down`, `updown`, `random`, `pinky` |
| lead | `motif`, the only one: leads are motifs with A A′ A B phrasing; `density` and `seed` change them |

A style the part's role does not have is an error. A decimal `seed` is ignored.

A double plays the notes of the part it doubles, as they are written or as the theory engine writes them, moved by
its octaves: a producer's layer of the same line on a second instrument. It plays in the sections that name it, and
follows its part through edits, mutations and a new seed. Notes written out on the double itself, with
[`notes()`](#notes) or in the piano roll, take its place, and a double whose part is gone plays notes of its own, as
any part of its role. A part another doubles cannot double one itself. A [layered](../learn/producers/genres-and-evolution.md#how-layered-a-song-is)
song's generator makes doubles this way (`chords_double`, `lead_double`, `bass_double`).

```js
song("house", 3);
part("chords_double", #{ role: "chords", doubles: "chords", octave: 1, preset: "bell" });
```

Errors: `no part 'keys' (the song has: bass chords pad lead arp; house parts: bass chords pad lead arp); a role makes
one of your own (part("keys", #{ role: "keys" })), and track() writes your own notes`; `part 'lead' is a lead part;
remove("lead") first to make it another`; `part(): a role is bass, chords, pad, keys, lead or arp, not 'drums'`; `'kick'
is a track of the song that is not a part: give the part another name`;
`a bass part has no style 'wobbly' (styles: sustain pulse root8 ...)`; `unknown preset 'nope' (synths: ...; or an
installed instrument such as piano, rhodes, string_ensemble)`; `part 'lead' doubles 'keys', which is not a part of
the song`; `part 'keys_double' doubles 'keys', which doubles another part: double that one`; `part 'keys' is doubled
by 'keys_double', so it cannot double another`; `part(): an octave is a whole number from -2 to 2, not 3`;
`part() has no option 'styles' (options: role style preset register density seed rate doubles octave)`.

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

### `notes()`

```js no-run
notes(part, prog, [[start, length, pitch, velocity], ...])
clear_notes(part, prog)
```

Writes out what a part plays over a progression: the notes replace what the theory engine writes there, every time
round, and the part keeps its sound, its mix and its sections. Each note is `[start, length, pitch]` or `[start,
length, pitch, velocity]`: beats from the start of the progression's cycle, beats long, a MIDI pitch from 0 to 127
(60 is middle C), and a velocity from 0 to 1 (0.8 when left out). Notes that run past one cycle make it longer: the
part comes round after the last note it reaches. An empty list rests over that progression. `clear_notes()` lets
the theory engine write the progression again.

The studio's **Piano roll** writes these. A song printed as a script writes a `notes()` line for every progression a
part has written out, by hand, from a take or from a keyboard, so the script plays the same notes.

Errors: `notes(): no part 'nope' (parts: bass chords pad lead arp)`; `notes(): no progression 'Z' (progressions: A
B)`; `notes(): note 2 is [start, length, pitch] or [start, length, pitch, velocity]: ...` for a note it cannot read
(a length of 0, a pitch outside 0 to 127); `notes(): 5000 notes in one progression; the limit is 4096`.

```js
notes("lead", "A", [[0, 0.5, 69], [0.5, 0.5, 72], [1, 1, 76, 1], [3, 0.5, 74, 0.6]]);
notes("bass", "B", []);
clear_notes("bass", "B");
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
