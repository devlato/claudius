# Sounds

Every sound in claudius is real audio: synthesized in code (drum voices, the synth engine) or recorded (sampled
instruments and drum machines from the sample packs, Apple Loops, your own files). There is no General MIDI and no
neural audio. A sound name appears in two places:

- the words of an `s()` pattern: `s("bd*4, [~ cp]*2")`;
- `.s(name)` on a `note()`, `n()`, `chord()` or `seq()` pattern: `note("a1 c2").s("upright_bass")`.

`claudius sounds` renders every drum voice, synth preset and transition effect into `out/sounds/` so you can hear
them; `claudius-packs aliases` lists every sampled-instrument name your installed packs answer to.

## How a name is resolved

A number after the last colon is a [variation](#variations) (`bd:3`, `tr808:sd:2`); the rest is the name, colons
included (`tr808:sd`, `vsco2:violins`). When a track is made, each name is looked up in this order, and the first
match wins:

1. With [`.bank()`](#drum-machines-and-kits) set, the name is looked up in that bank only.
2. **Drum names** play the synthesized drum voices.
3. **Synth presets** play the synth engine.
4. **The sample packs**: sampled instruments (`rhodes`, `vsco2:violins`) and kit sounds (`tr808:sd`).
5. **Files**: a name containing `/` or ending in `.wav`, `.mp3`, `.aif`, `.aiff`, `.flac`, `.caf` or `.m4a`.

Anything else is an error that lists what exists:

```js error
blank("house");
track("x", s("bdd*4"));
```

```text
Error: Runtime error: track 'x': unknown sound 'bdd' (drums: bd kick sd snare cp clap hh ch hat oh ohat rd ride cr crash lt mt ht tom rim rs sh shaker cb cowbell snap perc; synths: sub_bass 808 reese acid saw_bass pluck_bass organ_bass wobble pluck supersaw pad warm_pad stab epiano keys bell organ strings square_lead chip; sampled: piano upright rhodes wurlitzer hammond string_ensemble violins cellos pizzicato brass trumpet flute harp marimba vibes kalimba upright_bass guitar choir sax ... (claudius-packs aliases)) (line 2, position 1)
```

Because drum names and synth presets come first, a few names play synthesis even when a recording by that name
exists. Ask for the recording by another name, or exactly as `pack:instrument` (`claudius-packs instruments` lists
them):

| name | plays | for the recording, write |
|---|---|---|
| `epiano` | the FM electric piano synth | `rhodes`, `wurlitzer` |
| `keys` | the FM keys synth | `piano`, `upright` |
| `organ` | the drawbar organ synth | `hammond`, `church_organ` |
| `strings` | the string-machine synth | `string_ensemble`, `violins` |
| `pad`, `warm_pad` | the synth pads | `synth_pad`, `new_age_pad`, `bowed_pad` |
| `shaker`, `cowbell` | the synthesized drum voices | `vcsl:shaker`, `tr808:sh`, `tr808:cb` |

`lead` is not a synth preset: it plays the sampled FreePats square lead. The synth leads are `square_lead`,
`supersaw` and the other [synth presets](#synth-presets).

## Drum names

These names play the synthesized drum voices. Each one also decides the [mixing role](mixing.md#roles) of its track,
wherever it appears in the name: `bd:3` and `tr909:bd` are kicks, `tr808:sd` is a snare, `vcsl:shaker` a shaker.

| names | voice | role |
|---|---|---|
| `bd`, `kick` | kick | kick |
| `sd`, `snare` | snare | snare |
| `cp`, `clap` | clap | clap |
| `hh`, `ch`, `hat` | closed hi-hat | hat |
| `oh`, `ohat` | open hi-hat | ohat |
| `rd`, `ride` | ride | ride |
| `cr`, `crash` | crash | crash |
| `lt`, `mt`, `ht`, `tom` | tom (low 90 Hz, mid 130 Hz, high 180 Hz; `tom` is the default tuning) | tom |
| `rim`, `rs` | rim shot | perc |
| `sh`, `shaker` | shaker | shaker |
| `cb`, `cowbell` | cowbell | perc |
| `snap` | finger snap | perc |
| `perc` | a tuned percussion hit | perc |

## Synth presets

The synth engine has polyBLEP oscillators with unison and analog drift, state-variable and ladder filters, FM,
glide and drive. These are its factory presets; the role is what [`role_of`](mixing.md#roles) assumes for a
pattern track.

| preset | sound | mono | assumed role |
|---|---|---|---|
| `sub_bass` | a sine sub with a little drive | yes, tied notes | bass |
| `808` | a sine with a pitch drop, long decay and heavy drive; glides | yes | bass |
| `reese` | two detuned saws and a saw an octave down, a slowly moving ladder filter; glides | yes | bass |
| `acid` | one saw into a resonant ladder filter with a snappy envelope; velocity opens it; glides | yes | bass |
| `saw_bass` | detuned saws and a square an octave down, filter envelope | yes | bass |
| `pluck_bass` | square and saw with a short filter pluck | yes | bass |
| `organ_bass` | three sine drawbars (fundamental, octave, two octaves) | yes | bass |
| `wobble` | detuned saws and a square, ladder filter swept by a tempo-synced LFO | yes, tied notes | bass |
| `pluck` | three detuned saws and a square an octave up, fast filter decay | no | by pitch |
| `supersaw` | seven detuned saws plus three an octave down, delayed vibrato | no | by pitch |
| `pad` | five saws and a triangle, slow attack, long release, slow filter drift | no | by pitch |
| `warm_pad` | triangles and a pulse, slower and darker than `pad` | no | by pitch |
| `stab` | detuned saws and a square, short with a filter snap | no | by pitch |
| `epiano` | FM tine electric piano | no | by pitch |
| `keys` | brighter FM keys with a shorter decay | no | by pitch |
| `bell` | FM bell, long decay | no | by pitch |
| `organ` | five sine drawbars, sustained | no | by pitch |
| `strings` | five detuned saws, slow attack, delayed vibrato | no | by pitch |
| `square_lead` | square and saw an octave up, vibrato; glides | yes | by pitch |
| `chip` | a 25 % pulse, short | no | by pitch |

"By pitch" means a pattern track gets the role `chords` when it is a `chord()` pattern, `bass` when its median note
is below E3, and `lead` otherwise. Mono presets play one note at a time; with glide, overlapping notes slide (see
[`legato()`](pattern-functions.md#legato)).

## Sampled instruments

With the default packs installed (`claudius-packs install --defaults`), these names play multi-sampled recordings
with velocity layers and round robins. Most have several aliases; `claudius-packs aliases` prints them all.

| family | names |
|---|---|
| pianos and keys | `piano` (Salamander grand), `upright`, `rhodes`, `wurlitzer`, `cp80`, `pianet`, `fm_piano` |
| organs | `hammond`, `church_organ` |
| strings | `string_ensemble`, `violins`, `violas`, `cellos`, `contrabass`, `pizzicato`, `spiccato` |
| brass and winds | `brass`, `brass_stabs`, `trumpet`, `horn`, `trombone`, `tuba`, `woodwinds`, `flute`, `oboe`, `clarinet`, `bassoon`, `piccolo`, `sax`, `harmonica` |
| mallets and bells | `harp`, `marimba`, `vibes`, `xylophone`, `glock`, `bells`, `kalimba`, `timpani` |
| bass and guitar | `upright_bass`, `bass` (electric), `pick_bass`, `synth_bass`, `guitar` (nylon), `electric_guitar`, `jazz_guitar` |
| voices | `choir`, `voice` |
| synths, recorded | `synth_pad`, `new_age_pad`, `bowed_pad`, `choir_pad`, `synth_strings`, `synth_brass`, `lead` |
| percussion | `bongo`, `conga`, `cajon`, `darbuka`, `frame_drum`, `tambourine`, `cabasa`, `claves`, `woodblock`, `agogo`, `guiro`, `triangle`, `sleigh_bells`, `vibraslap`, `mark_tree`, `claps`, `gong`, `suspended_cymbal` |

`pack:instrument` names one exactly, in `.s()` or in an `s()` pattern: `note("c4 e4").s("vsco2:violins")`,
`s("vcsl:shaker*8")`.

Every recorded instrument has a playable range; notes outside it are pitched from the nearest sample, which sounds
stretched far from home. Songs that play instruments from packs whose licences ask for credit get a `credits.txt` in
their folder.

A sampled instrument name used without its pack installed is an unknown sound, and the error ends with
`sampled instruments need packs: claudius-packs install --defaults`. A session started with `--synth-only` (`make`,
`evolve`, `serve`, `studio` and `mcp` take it) plays no recordings at all: a sampled name ends its error with
`sampled instruments are off in this session (--synth-only)`, and `.bank("tr909")` raises
`bank 'tr909' plays sample packs, which are off in this session (--synth-only)`.

```js packs
blank("lofi");
key("D minor");
track("keys", chord("<Dm9 Gm9>").voicing(50, 72).s("rhodes"));
track("bass", note("<d2 g1>").struct("x ~ ~ x ~ x ~ ~").s("upright_bass"));
track("bongos", s("~ bongo [~ bongo] bongo:2").gain(0.6));
```

Sampled percussion from the packs gets the `perc` [role](mixing.md#roles) by itself.

## Drum machines and kits

`.bank(name)` plays the drum names of an `s()` pattern on a recorded drum machine or kit from the sample packs:

| bank | aliases | what |
|---|---|---|
| `tr909` | `909` | Roland TR-909 |
| `tr808` | `808` | Roland TR-808 |
| `tr707` | `707` | Roland TR-707 |
| `muldjord` | `acoustic`, `acoustic_kit`, `rock_kit` | a recorded acoustic kit |
| `uzu` | | the uzu drum kit |

Each kit has a subset of `bd sd cp hh oh rd cr lt mt ht rim cb sh` (the 909 has no shaker, the acoustic kit no
clap); names like `kick` and `snare` map to them. A name the bank does not have is an error:

```js error
blank("house");
track("drums", s("bd*4, bongo*2").bank("tr909"));
```

```text
Error: Runtime error: track 'drums': no 'bongo' in bank 'tr909' (installed packs: salamander epianos jrhodes vsco2 vcsl tr808 tr909 tr707 uzu muldjord freepats doublebass growlybass voice; kits: tr909 tr808 tr707 acoustic uzu) (line 2, position 1)
```

The bank applies to every word of the pattern. To mix kits, or a kit and a sampled instrument, in one pattern, name
each kit sound with its bank instead: `s("tr909:bd*4, tr808:cp*2, bongo*2")`. Strudel bank names (`RolandTR909`) work
as aliases, and Strudel sample maps added with `claudius-packs add-map` become banks too.

`.bank("apple")` cuts drum one-shots from installed Apple Loops (GarageBand or Logic content) instead. On a machine
without Apple Loops it falls back to the synthesized voices, without an error.

```js packs
blank("house");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4").bank("tr909"));
```

## Variations

A number after the last colon (`bd:3`, `tr808:sd:2`) or `.n(n)` picks variation *n* of a sound (0 is the
default):

- on a kit, the recording numbered *n*: the 909 has 126 kicks, the 808 has 25;
- on a synthesized drum, the same voice moved a few steps through its parameters (tuning, decay, tone ...), the same
  way every time;
- a variation a recording does not have is an error (`no 'bd:200' in bank 'tr909'`).

```js packs
blank("techno");
track("kick", s("bd*4").bank("tr909").n("<0 0 0 7>"));
```

## Your own files

`.s(path)` plays an audio file as a sampler, on any `note()`, `n()` or `chord()` pattern:

```js no-run
track("vox", note("c4 ~ c4 [c4 c4]").s("/Users/me/Samples/vox.wav"));
```

- `c4` plays the file as recorded; other notes play it faster or slower, like a sampler without time-stretch.
- On a track whose role is not a drum role, each note is cut at its length plus 50 ms, so `legato()` shapes it.
- Paths cannot go inside a pattern string, so a file is set with `.s()`, or with
  [`drum_sound()`](song-functions.md#drum_sound) for a drum lane of a generated song.
- A missing file is an error: `track 'vox': /Users/me/Samples/vox.wav does not exist`.
- When claudius is shared beyond your computer (a server reached by others), it reads files only from its own
  folders (saved songs, its caches, the sample packs, Apple Loops) and refuses others with
  `... is outside the folders this claudius may read`.

To use a song or a loop as material, `claudius chop` cuts bar loops and drum one-shots from it into the user
library, and `claudius make --sample <file>` flips a recording into a new song. See
[Remaking a song from audio](../cookbook/remake.md).

## Sounds you cannot set from a script

A script names sounds; it does not edit them. The synth presets' parameters and the tuning and decay of a generated
song's drum voices are set by the generator and changed by
[evolution](../learn/producers/genres-and-evolution.md). A script can swap them for other sounds:
[`drum_sound()`](song-functions.md#drum_sound) for a drum lane, [`part()`](song-functions.md#part) for a generated
part's preset. Pattern tracks can add filters, a bitcrusher and saturation (see
[Track settings](pattern-functions.md#track-settings)).
