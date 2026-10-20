# Sample packs

The sample packs are recordings of real instruments and drum machines, mapped so that claudius can play any note at
any velocity with them. Every pack in the default set is under a licence that lets you make and publish music with
it. Without packs claudius synthesizes every sound; with them, genres reach for a Rhodes, an upright bass or a real
909 where those belong.

Packs live in `~/Library/Application Support/claudius/packs` on macOS and `~/.local/share/claudius/packs` on Linux,
or in `$CLAUDIUS_PACKS`. Each pack is a folder with an `index.json` (its licence, attribution, files and how every
instrument maps onto them) and a `files/` folder.

## The packs

`claudius-packs list` shows every pack with its state and licence. The 22 default packs hold 253 instruments:

| pack | what | instruments | licence | credit required | on disk |
|---|---|---|---|---|---|
| `salamander` | Yamaha C5 grand piano, 5 velocity layers | 1 | CC BY 3.0 | yes | 226 MB |
| `epianos` | Wurlitzer EP200, Yamaha CP80, Hohner Pianet T | 3 | CC BY 3.0 | yes | 21 MB |
| `jrhodes` | 1977 Rhodes Mark I, 5 crossfaded layers | 1 | free for making music; no redistribution of the samples | no | 38 MB |
| `vsco2` | VS Chamber Orchestra 2: strings, brass, woodwinds, harp, timpani, key-split sections | 36 | CC0 | no | 284 MB |
| `vcsl` | mallets, bowed vibraphone, wine glasses, pipe organ, ocarina, tenor sax, harmonica, didgeridoo, a TX81Z FM synth, zithers, world and orchestral percussion, effects | 51 | CC0 | no | 238 MB |
| `tr808` | Roland TR-808 by Michael Fischer, every knob at five positions | 16 | CC0 | no | 13 MB |
| `tr909` | Roland TR-909 by Audio Realism, accent layers and knob sweeps | 11 | free for personal and commercial use, no redistribution | no | 26 MB |
| `tr707` | Roland TR-707, all 15 sounds | 13 | public domain | no | 0.5 MB |
| `uzu` | the uzu drum kit made for Strudel | 15 | Unlicense | no | 2.4 MB |
| `muldjord` | MuldjordKit, an acoustic rock kit (bank `acoustic`) | 11 | CC BY 4.0 | yes | 135 MB |
| `freepats` | upright piano, FM piano, drawbar organ, basses, guitars, synth pads, strings and leads, glasses of water, a hang | 21 | CC0 | no | 157 MB |
| `doublebass` | double bass pizzicato with round robins | 1 | CC0 | no | 30 MB |
| `growlybass` | Squier Jazz Bass, fingered | 1 | CC0 | no | 67 MB |
| `voice` | a low male "ah", and a 5-voice choir made from it | 2 | CC0 | no | 27 MB |
| `sonicpi` | Sonic Pi's samples: kicks, snares, hats, electronic one-shots, glitches, bass hits, drones, vinyl, tabla, loops | 18 | CC0 | no | 33 MB |
| `solocello` | a solo cello, bowed and plucked | 2 | CC0 | no | 60 MB |
| `erhu` | an erhu: long and short bows, sul tasto | 3 | CC0 | no | 43 MB |
| `steelpan` | a tenor steel pan, 5 velocity layers | 1 | Unlicense | no | 38 MB |
| `synthdrums` | drums made on synthesizers after vintage analog kits | 11 | CC0 | no | 2 MB |
| `body` | body percussion: claps, snaps, stomps, chest and belly hits | 14 | CC0 | no | 20 MB |
| `zamenhof` | two kits of electronic glitches | 2 | CC0 | no | 56 MB |
| `miscellania` | sound effects: aliens, zaps, sirens, metal, breaking glass, chains, bells, glockenspiel glissandi | 19 | CC0 | no | 41 MB |

The download is about 3.1 GB, larger than the 1.5 GB on disk, because `vsco2`, `vcsl`, `tr909`, `doublebass`,
`growlybass`, `voice`, `solocello`, `erhu`, `body`, `zamenhof` and `miscellania` arrive as WAV and are converted to
lossless FLAC as they install; each conversion is read back and compared with the WAV. Where a pack was cut for size
(the piano keeps 5 of its 16 velocity layers, VSCO leaves out the tremolo and muted brass), the pack's description in
`claudius-packs list` says what was left out.

Two more packs install only when you name them:

| pack | what | licence |
|---|---|---|
| `splendid` | Steinway grand released into the public domain by AKAI, 4 layers (77 MB) | public domain |
| `piano-lite` | Strudel's default piano: Salamander, one layer, MP3 (10 MB) | CC BY 3.0 |

Three are **opt-in**: their licences are unclear or carry conditions, and they install only with `--accept-licence`.

| pack | what | why it is opt-in |
|---|---|---|
| `tidal-drum-machines` | Strudel's default banks, about 70 drum machines | no licence is declared and the provenance is unknown |
| `dirt-samples` | the TidalCycles Dirt-Samples | no licence, and some samples come from commercial records |
| `mridangam` | mridangam strokes by Arthur Carabott | CC BY-SA 4.0: share-alike may extend to your music |

All three install lazily: their samples download the first time a song plays them.
[0007 Which samples we ship](../design/0007-sample-licensing.md) explains how packs are chosen.

## Installing

```sh
claudius setup                          # the default packs and the listening models, asking first
claudius-packs install --defaults       # the default packs only
claudius-packs install splendid          # one pack
claudius-packs install mridangam --accept-licence
```

| flag | what it does |
|---|---|
| `--defaults` | every default pack |
| `--lazy` | records the pack now and downloads each sample the first time it plays |
| `--threads N` | parallel downloads (8) |
| `--accept-licence` | allows an opt-in pack (`--accept-license` works too) |

Curated packs come from their authors' GitHub repositories at pinned commits, except `tr909` (a zip from
audiorealism.se) and the FreePats drawbar organ (a `.tar.xz` from freepats.zenvoid.org), which are unpacked with the
system's `tar` (or `unzip`). Each file downloads to a `.part` file, is checked against its expected size, and is
renamed into place, so an interrupted install resumes where it stopped: run the same command again. Installing a
pack that is already there downloads only what is missing and deletes files its index no longer uses.

Listing a repository uses GitHub's API, which allows 60 requests an hour without an account. If an install fails
with `set GITHUB_TOKEN if the API rate limit was hit`, set `GITHUB_TOKEN` (or `GH_TOKEN`) to any GitHub token and run
it again.

## Removing

```sh
claudius-packs remove splendid
```

`remove` deletes the folder `<packs>/<name>` and everything in it. It takes a plain pack name, as `claudius-packs list`
shows it, and only for a pack that is installed (its folder has an `index.json`) or that the registry knows, which
covers a half-finished install. Anything else is refused and nothing is deleted:

```text
$ claudius-packs remove ..
Error: '..' is not a pack name
$ claudius-packs remove nonexistentpack
Error: pack nonexistentpack is not installed
```

Paths are refused the same way as `..`: `a/b` and absolute paths are "not a pack name" too.

## Names

A script or a genre asks for an instrument by name, and the installed packs decide what plays:

| you write | plays |
|---|---|
| `piano` | a friendly alias: the first installed of `salamander:piano`, `splendid:piano`, `freepats:upright_piano`, `piano-lite:piano` |
| `vsco2:violins` | that pack's instrument, by name or by one of its aliases (`vsco2:violin_ensemble`) |
| `tr808:sd:3` | variation 3 of that instrument |
| `vinyl_scratch` | a variation by its own name, when it starts with its instrument's (Sonic Pi's names): `sonicpi:vinyl:3` |
| `kick`, `bd`, `hh` | a bare drum name: the first installed kit of `tr909`, `tr808`, `muldjord`, `tr707`, `uzu` that has it |
| `RolandTR909_bd` | a Strudel bank name: bank `RolandTR909`, sound `bd` |
| `bd` with bank `tr909` | that kit's kick |

`claudius-packs resolve` answers what a name plays on your machine, and `claudius-packs aliases` lists every friendly
name with what it resolves to now (264 of them with the default packs):

```text
$ claudius-packs resolve piano
salamander:piano
$ claudius-packs resolve bd:2 909
tr909:bd:2
$ claudius-packs resolve sd acoustic
muldjord:sd
$ claudius-packs resolve theremin
Error: nothing installed matches theremin
```

In scripts, drum names and synth presets are looked up before the packs, so `s("bd")` plays the synthesized kick
and `.s("epiano")` the FM synth. [Sounds](../language/sounds.md#how-a-name-is-resolved) has the full order and the
names to use for the recordings.

## Banks and variations

A bank is a drum kit picked by name. `s("bd*4, [~ sd]*2, hh*8").bank("tr909")` plays all three on the 909. Banks
answer to the pack's name and its aliases:

| bank | also | kit |
|---|---|---|
| `tr909` | `909`, `rolandtr909`, `tr-909` | Roland TR-909 |
| `tr808` | `808`, `rolandtr808`, `tr-808` | Roland TR-808 |
| `tr707` | `707`, `rolandtr707`, `tr-707` | Roland TR-707 |
| `muldjord` | `acoustic`, `acoustic_kit`, `rock_kit` | MuldjordKit |
| `uzu` | `uzu-drumkit` | uzu drum kit |
| `synthdrums` | `synth_drums`, `synth_percussion` | FreePats synthesizer percussion |
| `sonicpi` | `sonic_pi`, `sonic-pi` | Sonic Pi's kicks, snares, hats and rides, and its other groups |
| `body` | `body_percussion` | body percussion: `stomp`, `clap`, `snap`, `chest`, `belly`, `heel` ... |

Drum names have synonyms (`kick`, `kd`, `bassdrum` for `bd`; `snare` for `sd`; `rimshot`, `sidestick` for `rim`; and
so on), so `s("kick").bank("808")` works, and `s("bd sd hh").bank("sonicpi")` plays Sonic Pi's `bd`, `sn` and `hat`. A variation picks another recording of the same drum: `bd:2` in the pattern,
or `.n("0 1 2")`. The drum machines have many:

- `tr808`: every knob at five positions, 25 variations of `bd`, `sd` and the cymbal (tone by decay, or tone by
  snappy), 5 of the toms, congas and open hat;
- `tr909`: variation 0 of every drum plays its 11 accent levels as velocity layers; the others are the knob sweeps
  (125 for `bd` and `sd`, 121 for the toms, the decay of the hats, the tuning of the cymbals);
- `tr707`: one recording per sound;
- `sonicpi`: each group's sounds, in the order of their names, which they also answer to: `vinyl:3` is
  `vinyl_scratch`, `bd:6` is `bd_haus`.

`claudius-packs instruments` shows how many variations each instrument has.

## Strudel sample maps

Any [Strudel](https://strudel.cc) sample map can be added as a pack of your own:

```sh
claudius-packs add-map github:tidalcycles/uzu-drumkit       # reads strudel.json on the main branch
claudius-packs add-map github:user/repo/branch/sub/folder   # strudel.json in that folder
claudius-packs add-map https://example.org/samples.json
```

```text
added tidalcycles-uzu-drumkit (16 sounds, licence Unlicense, lazy)
```

The pack is named after the GitHub owner and repository (`tidalcycles-uzu-drumkit`; a curated pack's name is
refused), and installs lazily: each sample downloads the first time a song plays it. Its sounds take the names in the map, so a map with `RolandTR909_bd` answers to
`s("bd").bank("RolandTR909")`. The licence comes from the map's `_license` field or, failing that, from the GitHub
repository. When there is none, it is recorded as `unknown`, `add-map` prints a warning, and every song that uses
the pack says so in its `credits.txt`. `claudius-packs install <name>` downloads all of a user pack's samples at once.

## Credits

A song that plays sample-pack instruments gets a `credits.txt` next to it with the packs it used, and only those:

```text
Sounds:
- E-Pianos (Yamaha CP80, Hohner Pianet T, Wurlitzer EP200) by Greg Sullivan (CC BY 3.0), SFZ mapping by kinwie / sfzinstruments https://creativecommons.org/licenses/by/3.0/
- jRhodes3d by Jeff Learman [jRhodes licence: free for making music (CC0 for musicians); redistributing the samples: CC BY-NC 4.0] https://github.com/sfzinstruments/jlearman.jRhodes3d/blob/master/LICENSE
- Roland TR-808 samples by Michael Fischer (Technopolis, 1994), packaged by TidalCycles [CC0-1.0] https://creativecommons.org/publicdomain/zero/1.0/
- VS Chamber Orchestra 2: Community Edition by Versilian Studios (CC0); recorded by Sam Gossner and Simon Dalzell [CC0-1.0] https://creativecommons.org/publicdomain/zero/1.0/
```

The packs whose licence requires credit (CC BY) come first, then the others as a courtesy, then a
`Warning: licence unknown for ...` line for any pack without a known licence. When you publish the song, put the
lines of the packs that require credit where you credit the music.

`claudius-packs credits` prints the required credits of every installed pack, and `claudius-packs credits piano
rhodes bd@tr909` those of the packs behind specific names (`name@bank` for a drum in a bank).

## Auditioning

| command | what it does |
|---|---|
| `claudius-packs instruments` | every installed instrument with its range, velocity layers, round robins, zones and variations |
| `claudius-packs render <name> <out.wav> [--bank B] [--notes 60,64,67] [--secs S]` | renders a chord with one instrument |
| `claudius-packs audition [dir] [--pack P]` | renders every instrument (an arpeggio, or a few hits for drums, and a beat per kit) into `dir` (`/tmp/claudius-packs-audition`), and reports instruments that are silent or seem mapped an octave off |

```text
$ claudius-packs instruments
salamander:piano                     piano                pitched   A0..C8   layers 5  rr 1  zones 150  variants 1
epianos:wurlitzer                    electric piano       pitched   A1..G#6  layers 4  rr 1  zones 48   variants 1
vcsl:marimba                         mallets              pitched   F2..C7   layers 1  rr 1  zones 30   variants 1   one-shot
...
```

## Memory

Decoded samples stay in memory once played, up to 3072 MB by default (`CLAUDIUS_PACKS_CACHE_MB` changes the
budget). When a new sample takes the cache over budget, the samples used longest ago that nothing is playing are
dropped until it is back to 80 % of the budget; they are decoded again the next time they play.
