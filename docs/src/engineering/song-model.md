# The song model

claudius works with two documents, both in `crates/claudius/src/model.rs`:

- a **`Blueprint`** is a song's genome: the choices a producer makes (tempo, key, progressions, form, drum patterns,
  how each part is written, every sound, the mix), stored as data small enough to read, diff and mutate;
- a **`Project`** is the arranged timeline a blueprint expands into, like a DAW session: tracks with notes or audio
  clips, a chord track, sections, and effect chains with automation.

Everything that edits a song edits its blueprint. The project is derived on demand by `compose::arrange` and thrown
away after rendering. [0003 The blueprint is the song](../design/0003-blueprint.md) records why.

Units, everywhere: time in beats from the start of the song (4 beats to a bar, no other metre), pitch in MIDI
semitones (60 is middle C), velocity from 0 to 1, levels in dB, sample rate 48 kHz.

## The blueprint

This is the head of a real one, house with seed 9 (`claudius make --genre house --seed 9` writes it to
`blueprint.json`):

```json
{"version": 1, "title": "Crimson Tide", "genre": "house", "seed": 9, "bpm": 124.0,
 "key": {"tonic": 7, "mode": "dorian"}, "swing": 0.5257842, "humanize": 0.25,
 "harmony": {"progressions": {"A": ["i7", "ii7", "i7", "IV7"], "B": ["i7", "VIImaj7", "i7", "IV7"]},
             "bars_per_chord": 1},
 "form": [{"name": "intro", "kind": "intro", "bars": 8, "energy": 0.4, "prog": "A",
           "layers": ["kick", "hat", "perc", "top"], "variant": "sparse",
           "filter_in": true, "build": false, "gap": false, "impact": false, "fills": false}, ...],
 "drums": [...], "parts": [...], "loops": [], "patterns": [], "transitions": {...}, "mix": {...},
 "lineage": ["generated: house seed 9"]}
```

| field | meaning |
|---|---|
| `version` | the format, 1 |
| `title`, `genre`, `seed` | the genre names a profile in `data/genres/`; the seed is the one it was generated with |
| `bpm`, `key`, `swing`, `humanize` | `key` is a tonic pitch class (C = 0) and a mode; `swing` 0.5 is straight, about 0.66 a triplet feel; `humanize` 0 to 1 loosens timing and velocity |
| `harmony` | named progressions of Roman numerals and how many bars each chord lasts |
| `form` | the sections, in order |
| `drums` | drum lanes with step patterns |
| `parts` | melodic parts written by the theory engine |
| `loops` | recorded loops (Apple Loops, flipped samples) |
| `patterns` | tracks written as pattern code by scripts |
| `transitions` | the crash, riser, impact and their switches |
| `mix` | the reverb and delay returns, bus chains, sidechain release, per-role level offsets, the master |
| `lineage` | what was done to the song, one line per change, oldest first |

### Harmony

Progressions are Roman numerals **in the key's own mode** (`VI` in A minor is F major), so changing the key moves
every chord with it, and `key()` in a script re-spells diatonic numerals when the mode changes (`Imaj7` in E major
becomes `i7` in F minor). Generated songs have two, `A` and `B`; sections play `A` unless they say otherwise, and
breaks play `B`. Scripts may add more names. [Composition and theory](theory.md#progressions) covers the numeral
grammar.

### Form

Each `SectionSpec` is a block on the arrangement:

| field | meaning |
|---|---|
| `name` | unique in the song (`drop`, `drop2`) |
| `kind` | `intro`, `verse`, `build`, `drop`, `break`, `hook`, `groove` or `outro` |
| `bars`, `energy` | length, and how intense it should be (0 to 1); the critic compares energy with loudness |
| `prog` | which progression it plays |
| `layers` | the names of the drum lanes, parts, loops and pattern tracks that play in it |
| `variant` | which drum pattern variant its lanes play (`main`, `sparse`, `alt`, `half`, ...) |
| `filter_in` | a lowpass opens across the section |
| `build` | a snare roll, a riser and a highpass sweep into the next section, the kick out for its last bar |
| `gap` | a beat of silence before the next section |
| `impact` | a crash and an impact on its downbeat |
| `fills` | drum fills at the end of every 8 bars |

Layers are names, not references: a section may list a layer that no track has (a genre's optional `top` loop when
the library had none), and it plays nothing.

### Drum lanes

A `DrumLane` has a `sound`, a `pattern_bars` length (4 in generated songs), and `patterns`: a map from variant name
to hits, each hit a position in beats within the pattern and a velocity. Variants of one lane all have the same
length; when a script sets a longer one, the others are repeated up to it. `pitch` transposes the lane, `choke` names
a group in which a new hit cuts the previous one (the open hat stops when the closed hat plays), and `mix` is the
lane's [`TrackMix`](#the-mix).

### Parts

A `Part` stores how to write notes, not the notes:

| field | meaning |
|---|---|
| `role` | `bass`, `chords`, `pad`, `lead`, `arp` or `keys`: which writer runs, and its target level |
| `style` | the writer's style: a bass style (`offbeat`, `rolling`, `kick_follow`, ...), a chord rhythm (`stabs`, `pump`, `charleston`, ...), an arpeggio pattern, or `motif` for leads |
| `register` | the lowest and highest MIDI note it may use |
| `density` | how busy a lead is (0 to 1) |
| `voices`, `voicing` | how many notes a chord has and how it spreads (`close`, `open`, `spread`) |
| `rate` | an arpeggio's step in beats |
| `seed` | the part's own random seed |
| `notes` | notes written by hand or by analysis, per progression, which replace the writer |
| `sound`, `mix` | its sound and mix |

Storing the recipe makes the part small, readable as a script line
(`part("bass", #{ style: "syncopated", preset: "saw_bass", register: [33, 52], density: 0.42195085, seed: ... })`),
and editable at the level a producer thinks at: a new seed is a new bass line in the same style, a new key moves the
same line. The writer is deterministic, so the recipe is enough to get the same notes back.

### Loops and pattern tracks

A `LoopPart` plays a recorded loop (`Sound::Loop` with its tempo, length in beats and key): `transpose` in
semitones, and optionally `chop`, a slice order over `slice_beats`-long slices, for flipped samples.

A `PatternTrack` is a track a script wrote: `code` is the pattern expression exactly as written
(`s("bd*4, [~ cp]*2")`), `sounds` maps each sound label in it to the `Sound` it resolved to when the track was
written, and `role` is set when the pattern plays one sound. The code is the source of truth: the project
re-evaluates it at every arrangement, which is what lets a song print back as the script that made it. Storing the
resolved sounds keeps a song's sounds stable if, say, another sample pack is installed later.

### Sounds

`Sound` is an enum, and every variant is audio:

| variant | what plays |
|---|---|
| `drum { voice, params }` | a synthesized drum voice with nine parameters (tune, decay, tone, punch, snap, drive, pitch envelope and time, room) |
| `synth { preset, patch }` | the synth engine with a full copy of a preset's patch, possibly varied or mutated |
| `sample { path, root, start, end, label }` | a recorded one-shot played like a sampler; `root` is the note that plays it as recorded |
| `fx { fx }` | a transition effect rendered in code: `riser`, `downlifter`, `impact`, `reverse_crash`, `sweep_up`, `sweep_down` |
| `instrument { name, label }` | a multi-sampled instrument from a pack, by its canonical `pack:instrument` name; `label` is what it was asked for by |
| `loop { path, bpm, beats, key, tonal, label }` | a recorded loop, warped to the song's tempo and, when `tonal`, transposed to its key |

There is no variant for General MIDI or generated audio, which is the point of [0001 Real audio
only](../design/0001-real-audio-only.md). `Blueprint::files()` lists every file path a song plays, which the server's
file policy checks when a blueprint is opened.

### The mix

Every track has a `TrackMix`:

| field | meaning |
|---|---|
| `level_db` | dB relative to the target loudness of the track's role, not an absolute fader |
| `pan` | −1 to 1 |
| `fx` | insert effects |
| `reverb`, `delay` | send levels to the two returns, 0 to 1 |
| `duck` | sidechain depth from the kick, 0 to 1 |
| `bus` | `drums`, `bass`, `music` or `fx` |
| `mute`, `solo` | while any track is soloed, only soloed tracks are arranged |

The song's `MixPlan` holds the reverb and delay effects of the returns, an insert chain per bus, `duck_release` (the
sidechain recovery, in beats), `levels` (a dB offset per role) and the master (`fx`, a `lufs` target and a
`ceiling_db`, −1 by default). Effects are plain data too; [Rendering, mixing and
mastering](rendering.md#effects) lists them.

### Size limits

A song is at most `MAX_BARS` (512) bars and `MAX_SECONDS` (20 minutes) long, because a render holds every track in
memory at 48 kHz. `Blueprint::check_size` enforces it after every script and edit, and `render::render` refuses a
longer project.

## The project

`compose::arrange` (in `compose/arrange.rs`, with `cycle_spans` and `part_cycle`) turns a blueprint into a `Project`:

| field | meaning |
|---|---|
| `title`, `genre`, `bpm`, `key`, `swing`, `humanize`, `seed` | copied from the blueprint |
| `bars` | the total length |
| `sections` | `name`, `kind`, `start_bar`, `bars`, `energy` per section |
| `chords` | the chord track: one `ChordSpan` (start, length, chord) per chord, across the whole song |
| `tracks` | the tracks, each with a `role`, a `sound`, `notes` or `clips`, a transpose, a chop, a choke group and a mix |
| `mix` | the blueprint's mix plan, with the arrangement's automation and fixed inserts added to the buses |

A `Note` is `{t, d, p, v}`: start and duration in beats, pitch, velocity. A `Clip` places a loop: start and duration
in song beats, an offset into the warped loop, a gain and fades. These are the first three notes of the kick track
in the song above:

```json
{"name": "kick", "role": "kick", "sound": {"kind": "drum", "voice": "kick", "params": {...}},
 "notes": [{"t": 0.0, "d": 0.25, "p": 60, "v": 0.9350958}, {"t": 1.0, "d": 0.25, "p": 60, "v": 0.93947273},
           {"t": 2.0, "d": 0.25, "p": 60, "v": 0.9417365}], "clips": [], ...}
```

### What arranging does

For every section, `arrange`:

- **drums**: tiles the lane's pattern for the section's variant (or `main`) across the section. In a build, the snare
  (or clap) lane turns into a roll over the last half of the section (at least one bar, at most four), speeding up
  from quarters to thirty-seconds, and the kick drops out for the last bar. With `fills`, the end of every 8-bar phrase gets a fill of one or two beats on the snare lane
  (`roll`, `groove` or `triplets`), and the kick sometimes skips the last half beat.
- **parts**: writes one cycle of each part per progression, once, and tiles it across every section that plays the
  part with that progression, so a hook comes back the same each time. A lead's cycle is at least 16 beats.
- **loops**: one clip per section the loop plays in.
- **pattern tracks**: evaluates the code and places each section's notes from the pattern's first cycle, one bar per
  cycle. A pattern with several sounds becomes one track per sound, named `<track>.<sound>` (`drums.bd`), each mixed
  by its own role.
- **transitions**: a crash on every impact downbeat and every 8 bars of a drop or hook, impacts and downlifters on
  impact sections, a riser of up to `riser_bars` over a build, a reverse crash into the next impact.

Then, across the song, it drops muted tracks (and, when any track is soloed, every track that is not), cuts a beat of
silence before every section with `gap` (risers and reverse crashes play through), and adds automation to the
`music` and `bass` buses: a lowpass that opens from 350 Hz across `filter_in` sections and closes towards 500 Hz over
the last 60 % of a final outro, and a highpass that sweeps from 10 Hz to 900 Hz across each build. Finally it adds
three inserts every song gets unless its genre already has them: width 1.25 with mono below 160 Hz on `music`, mono
below 140 Hz on `bass`, and light tanh saturation on `drums`.

Arranging is deterministic: fills draw from `seed ^ hash(lane) ^ section index`, parts from their own seeds.

### Slices

`Project::slice(from_bar, to_bar)` cuts a project down to a range of bars, shifting notes, clips, chord spans,
sections, and the automation curves and time-based effects (tape stop, stutter, reverse) of tracks and buses.
Evolution auditions candidates on slices, and the tests render slices to stay fast.

## Saving and loading

A blueprint's JSON is its save format: `blueprint.json` in every song folder, and what `open` and `claudius render`
read. Most fields have serde defaults (no loops, no pattern tracks, an empty lineage, a `main` variant), so a file
that leaves them out still loads. A pattern track whose code no longer evaluates is skipped when the song is
arranged, without an error. Scripts check a pattern when they add it, so that takes a hand-edited blueprint, or one
saved by a version of claudius that accepted more (a pattern with a speed-up above 256 from before that limit).
`song::write_folder` writes a project to `project.json` in every song folder only so that `claudius judge` can check
the arrangement and theory of a rendered file; nothing reads it back as a song.
