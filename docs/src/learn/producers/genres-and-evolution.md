# Genres, mutation and evolution

A genre in claudius is a data file: the production knowledge of a style written down as tempo ranges, harmonic
styles, forms, drum grammars, part writers, sounds, transitions and mix targets. The generator composes from it,
the critic judges against it, and evolution mutates songs within it. This page covers all three.

## The genres

`claudius genres` lists them:

| genre | BPM | swing | modes | master | kits used | ears listen for (and against) |
|---|---|---|---|---|---|---|
| `house` | 120–126 | 0.50–0.56 | minor, dorian, major | −10 LUFS | 909 (45 %), 707 (10 %) | house music (techno, minimal, corporate pop, drum and bass) |
| `deep_house` | 118–124 | 0.53–0.58 | minor, dorian, major | −11 | 909 (30 %), 808 (20 %) | house music (techno, minimal, lo-fi hip hop, jazz) |
| `techno` | 126–134 | 0.50–0.52 | minor, phrygian, dorian | −9 | 909 (50 %), 808 (15 %) | techno music (house, minimal, drum and bass, dubstep) |
| `trap` | 130–150 | 0.50–0.54 | minor, harmonic minor, phrygian | −9 | 808 (60 %) | trap beat (heavy-bass hip hop, lo-fi hip hop, dubstep, house) |
| `lofi` | 70–90 | 0.56–0.63 | major, minor, dorian | −12.5 | 808 (30 %), acoustic (30 %) | lo-fi hip hop beat (jazz, heavy-bass hip hop, solo piano, trap) |
| `dnb` | 170–176 | 0.50–0.51 | minor, dorian, phrygian | −8.5 | acoustic (30 %) | drum and bass music (dubstep, techno, house, trap) |
| `future_bass` | 140–160 | 0.50–0.53 | major, minor | −9 | 808 (30 %) | future bass music (dubstep, corporate pop, trap, house) |
| `synthwave` | 85–118 | straight | minor, dorian | −10.5 | 707 (40 %), 808 (20 %) | synthwave music (corporate pop, chiptune, house, ambient pad) |
| `trip_hop` | 78–94 | 0.51–0.56 | minor, phrygian, dorian | −11 | acoustic (50 %), 808 (10 %) | trip hop music (lo-fi hip hop, heavy-bass hip hop, ambient pad, cinematic orchestral) |
| `big_beat` | 124–138 | 0.50–0.54 | minor, mixolydian, phrygian, dorian | −8.5 | acoustic (45 %), 909 (20 %), 808 (10 %) | big beat breakbeat music with distorted drums (drum and bass, techno, rock with electric guitars, heavy-bass hip hop) |
| `electro_house` | 126–130 | 0.50–0.52 | minor, dorian | −8 | 909 (50 %) | dirty electro house dance music with big synth leads (house, dubstep, techno, corporate pop) |
| `nu_disco` | 112–124 | 0.50–0.56 | minor, dorian, major | −10 | acoustic (30 %), 707 (20 %), 909 (15 %) | disco music (house, corporate pop, synthwave, jazz) |
| `filter_house` | 118–126 | 0.50–0.55 | dorian, minor, mixolydian | −9 | 909 (50 %) | french house music with a filtered disco loop (house, funk, techno, corporate pop) |
| `dance_pop` | 116–128 | 0.50–0.52 | major, minor, dorian | −8.5 | 909 (30 %), 808 (20 %) | upbeat dance pop music (corporate pop, house, synthwave, future bass) |
| `neo_psychedelia` | 96–122 | 0.50–0.55 | mixolydian, dorian, major, lydian | −10 | acoustic (70 %) | dreamy psychedelic pop music with phased guitars and organ (rock with electric guitars, indie pop, synthwave, funk) |
| `art_rock` | 76–112 | 0.50–0.53 | minor, dorian, phrygian, lydian, major | −10.5 | acoustic (70 %) | progressive rock music with piano, strings and synthesizers (rock with electric guitars, cinematic orchestral, indie pop, ambient pad) |
| `idm` | 96–140 | 0.50–0.56 | lydian, dorian, minor, phrygian, major | −10 | none: synthesized | glitchy intelligent dance music with complex broken beats and bells (glitch hop, drum and bass, ambient pad, chiptune) |
| `dub_techno` | 116–126 | 0.50–0.54 | minor, dorian, phrygian | −10.5 | 909 (30 %), 808 (20 %) | dub techno music with echoing chords (techno, minimal, ambient pad, house) |
| `boom_bap` | 86–96 | 0.55–0.62 | minor, dorian, phrygian | −10 | acoustic (45 %), 808 (15 %) | boom bap hip hop beat (lo-fi hip hop, trap, jazz, funk) |
| `uk_drill` | 138–146 | 0.50–0.53 | minor, harmonic minor, phrygian | −9 | 808 (50 %) | uk drill beat (trap, heavy-bass hip hop, dubstep, grime) |
| `dubstep` | 138–142 | 0.50–0.52 | minor, phrygian, harmonic minor | −9 | 909 (30 %), 808 (25 %) | dubstep music (drum and bass, trap, electro house, techno) |
| `uk_garage` | 130–136 | 0.58–0.66 | minor, dorian, major | −9.5 | 909 (35 %), 808 (25 %) | uk garage music (house, drum and bass, r&b, dubstep) |
| `jungle` | 160–172 | 0.50–0.53 | minor, dorian, phrygian | −9.5 | acoustic (60 %) | jungle music (drum and bass, breakbeat, dubstep, reggae) |
| `afrobeats` | 100–115 | 0.52–0.58 | major, minor, dorian | −9.5 | 808 (30 %), acoustic (30 %) | afrobeats music (dancehall, reggaeton, house, r&b) |
| `amapiano` | 110–116 | 0.54–0.60 | minor, dorian, major | −10 | 909 (30 %), 808 (30 %) | amapiano music (deep house, afrobeats, house, jazz) |
| `reggaeton` | 88–100 | 0.50–0.53 | minor, harmonic minor, dorian | −9 | 808 (50 %) | reggaeton music (dancehall, latin pop, trap, afrobeats) |
| `trance` | 134–140 | 0.50–0.51 | minor, dorian, major | −9 | 909 (60 %) | trance music (techno, house, electro house, synthwave) |
| `neo_soul` | 72–92 | 0.55–0.62 | dorian, minor, major | −11 | acoustic (60 %) | neo soul music (r&b, jazz, lo-fi hip hop, funk) |

The ten after synthwave lean on [effect chains](../../language/mixing.md#effect-chains) in their profiles:
trip-hop's drums go through parallel compression and a crushed copy, and its delay feeds back harder in the breaks;
big beat's acid line opens its filter across the builds; filter house's loop sweeps a resonant lowpass with the
arrangement; art rock's guitars turn distorted in its climaxes; IDM's pads are gated harder as each groove goes on;
dub techno's chord echoes feed back and swell in the break, all of it values by section that follow the form.

The five after dub techno are built on their drums and their bass: boom bap's swung sampler drums under a piano
loop, drill's late snare and hats in threes over a sliding 808, dubstep's wobble over a clean sub, garage's two-step
kick and shuffle, jungle's chopped breakbeat under a deep sub. Each is close kin of a style the ears know well
(lo-fi hip hop, trap, drum and bass, house), and its "against" list names that style, so its genre score runs lower
than most: the ears often hear the family before the genre.

The last five bring grooves of their own: afrobeats' clave, congas and shakers, amapiano's log drum under a steady
kick, reggaeton's dembow, trance's rolling bass and long breakdown, neo-soul's laid-back live kit under ninth and
eleventh chords.

The "against" list matters: the genre score is high only when a song reads as its genre *more than* as the styles it
is usually confused with, and more than stock library music (corporate background music, children's piano music and
the like), which every genre is measured against.

## Fusing genres

Two or three genres joined by `+` are one genre: `trip_hop+idm` in `song()`, `claudius make --genre`, `new_song`, or
**Fused with** in the studio's New song dialog. `genres::fuse` makes it from the profiles:

| from the first genre | from the second |
|---|---|
| tempo, keys, harmony (three quarters of the time), forms, transitions, the mix and the master, what the ears listen for | the drum lanes, their kits, the drums bus, the drum levels, a quarter of the progressions |
| the parts | its first sound and its styles added to each part of the same role, and its parts of roles the first has none of, as optional parts in the sections with energy 0.6 or more |

The second genre's lanes play in the first genre's sections: a lane of the same name where that lane played, the
others where most of the first genre's drums played. Swing is the average of the two, humanize the larger, and the
flaws either genre accepts are accepted. Sounding like the second genre no longer counts against the song: its
query leaves the first genre's list of confusions. With three genres the first two fuse, then the third plays the
drums. A fusion's Apple Loops, its store genre and the demo's sounds are the first genre's. The
[Fusions](../../cookbook/fusions.md) recipes hear three of them.

## What a profile contains

Each profile (`crates/claudius/data/genres/*.json`) has:

| field | what it decides |
|---|---|
| `bpm`, `swing`, `humanize` | tempo and feel ranges |
| `modes`, `harmony`, `markov`, `ext`, `bars_per_chord` | keys, [harmonic styles](harmony.md#the-generators-harmony), how often a fresh Markov progression is written, chord extensions, chord lengths |
| `forms` | the section plans, with layers, energies and flags; each song plays one [its own way](../../engineering/theory.md#a-form-and-a-mix-of-its-own) |
| `drums` | the lanes: per-step hit probabilities and velocity ranges per variant, or a euclidean rhythm; drum voice parameter ranges; which Apple Loops one-shots may replace the voice; choke groups (open and closed hat) |
| `kits`, `kit_mix` | which recorded drum machines the drums may be played on, and how often; how often a lane plays a kit of its own |
| `palette` | for each role, how often a part keeps its own sounds, and which kinds of sound it draws from otherwise: synths, or the installed packs' strings, brass, woodwinds, mallets, pianos, guitars and more |
| `parts` | bass, chords, pad, lead, arp, keys: their own presets and instruments, writing styles, registers, density, voicing, and the chance a song leaves the part out |
| `loops` | the Apple Loops a song may layer, by instrument, genre and descriptors |
| `sampling` | the samples the genre is built on, from [free online libraries](../../guide/online-samples.md#genres-built-on-samples): for a break, a phrase and a texture, what to search for, what the ears should hear, how many to keep, how often a song plays one and re-chops it, its mix, and the layers that play `instead` when there is none |
| `transitions` | risers, impacts, downlifters, reverse crashes, riser length |
| `mix` | the loudness target, reverb and delay, bus and master chains, sidechain release, role level offsets; each song varies its reverb, delay, sends and colours |
| `query`, `alts` | what the ears should hear, and its confusions |
| `accepted_flaws` | flaws the ears may hear that are the genre's sound, such as lofi's hiss, so they do not count against it |

To add a genre or change one, drop a JSON file with the same fields into the `claudius/genres` folder of your
configuration directory (`~/Library/Application Support/claudius/genres/` on macOS, `~/.config/claudius/genres/` on
Linux). A file whose `name` matches a built-in genre replaces it. A good start is a copy of the built-in file
closest to what you want.

## Making songs

```sh
claudius make --genre techno                          # a new song, seeded from the clock
claudius make --genre trap --key "F# minor" --bpm 142 --seed 7 --stems
claudius make --genre deep_house --loops              # built around an Apple Loops family
claudius make --genre lofi --synth-only               # no recordings at all
claudius make --genre house --sample chop.wav         # flip a recording into the song
```

In a script, `song("trap", #{ seed: 7, bpm: 142, key: "F# minor" })` makes the same song, and the edits after it
apply on top. `--bpm` and `--key` change only the tempo and the key: the title, the form, progression `A` and the drum
patterns are the ones `--seed 7` gives without them, except that a key in another mode moves the chords into it, as
`key()` does. The same genre, seed and options make the same song on any
machine, in the studio's New as on the command line; the sample packs and Apple Loops installed change only which
recordings it plays.

## Mutation

A mutation is one edit a producer might try, chosen from 25 operators:

| area | operators |
|---|---|
| harmony | a theory-aware substitution in `A` or `B` (a chord a third away, extended or plain, a secondary dominant, a chord borrowed from the parallel mode, a suspension, a swap, a rotation); a new progression from one of the genre's styles |
| drums | add, remove, nudge or re-accent one hit; regenerate a lane from the grammar; swap a lane's sound (a recorded one-shot, or one synth parameter moved); move every lane to another kit, or back to synthesis |
| parts | rewrite a part (new seed); another style; another preset; tweak the patch (cutoff, resonance, release, drive, detune); move the register; change a lead's density |
| mix | a track 1–3 dB up or down; a reverb or delay send; a sidechain depth; a pan; the reverb's decay or the delay's time |
| form | a track into or out of a section; a section 4 or 8 bars longer or shorter; toggle a section's fills, gap, impact or filter sweep; another drum variant for a section |
| global | tempo by 1–4 BPM within the genre's range; key by up to 5 semitones; swing; another Apple Loop |

Operators are not equally likely. The critic's top four issues raise the weight of the operators that address them,
by 1 + 4 × the issue's severity: band and width problems favour level, sound, kit, patch, space and pan edits; form
problems favour the form operators; a `part:lead` issue favours rewriting the lead; a weak genre reading favours
presets, styles, drum grooves and kits, tempo and harmony. Every mutation records what it did, and the song keeps the
list as its lineage.

Mutation edits what the generator writes: drum lanes, parts, loops and their mix. Pattern tracks are moved between
sections and follow tempo, key and swing, but their code is never changed.

`claudius mutate out/house-crimson-tide-9/blueprint.json -n 4` writes four mutated variants into `mutations/` next
to it, each rendered and judged, with what changed. The MCP tool `mutate` tries a few variations of the open song and
keeps the best if it beats the current one.

## Evolution

`claudius evolve` runs a (μ + λ) evolution: it starts from fresh songs (or `--from` a blueprint), and in each
generation the best songs so far are parents of new children, each made by one or two mutations or by a crossover of
two parents (drums and form from one, harmony and parts from the other) followed by mutations. A child identical to
one already heard is skipped. Each candidate is judged on a preview, the build-up into the first drop and the drop
(`--preview-bars`, 32 by default), and at the end the best are rendered in full.

| option | default | |
|---|---|---|
| `--start` | 4 | fresh songs to start from |
| `--generations` | 5 | |
| `--population` | 4 | parents kept per generation |
| `--children` | 8 | children per generation |
| `--preview-bars` | 32 | bars auditioned per candidate |
| `--keep` | 3 | best songs rendered in full at the end |

A short run, two generations of four children from two deep house songs:

```sh
claudius evolve --genre deep_house --seed 4 --start 2 --generations 2 --children 4 --keep 1
```

```text
ears loaded in 3.7 s
evolving deep_house (2 start, 2 generations x 4 children, 32-bar previews, with ears)
  g0-0  78.4  Velvet Orbit
  g0-1  86.5  Hollow Harbor
  g1-0  86.1  from g0-1: key Bb dorian -> Eb dorian
  g1-1  86.4  from g0-1: form: clap out of build
  g1-2  84.9  from g0-1: hat: sampled hh (tr909:hh) -> hat cut from 'Times of Plenty Open Hi-Hat' (Apple Loops); key Bb dorian -> F# dorian
  g1-3  85.9  from g0-1: harmony A: i7 IV7 -> i7 iv7 v7 i7 (minor_soul); mix: shaker +2.0 dB (now +2.0)
generation 1: best 86.5
  g2-0  85.1  from g0-1: crossover: drums and form of one parent, harmony and parts of the other; drums (sparse): re-accented a hat hit; key Bb dorian -> G dorian
  g2-1  86.1  from g0-1: form: drop loses fills
  g2-2  84.8  from g1-3: form: build now 4 bars; form: groove loses fills
  g2-3  85.8  from g1-0: crossover: drums and form of one parent, harmony and parts of the other; mix: chords sidechain depth 0.28; mix: kick -1.8 dB (now -1.8)
generation 2: best 86.5
searched 10 candidates in 17 s; rendering the best 1 in full
#1 g0-1 (Hollow Harbor): preview 86.5, full song 86.1
```

A `+` marks a child that beats the best parent. In this short run none did, so the winner is a starting song,
`g0-1`, and its full render scores a little under its preview (the preview hears only the build and the first drop).
Small differences in score are within the critic's noise, so treat a gain of half a point as a draw and listen.
Evolution is most useful from a weak start (a score in the 70s, like `g0-0`) or aimed at one problem, and less so for
polishing a song that already scores well.

The MCP tool `evolve` does the same on the open song; a generation takes 10 to 40 seconds.

## Judging against references

`--refs <files or folders>` on `make`, `script`, `evolve`, `render` and `judge` adds a typicality score: how close the
song's ears embedding is to a set of reference tracks you choose (at least two). It counts as a sixth score with the
same weight as genre and production, and an issue appears when the song is unlike the references.

## What the score is

The total is a weighted mean: mix 0.3, arc 0.2, theory 0.2, and with the ears genre 0.15 and production 0.15 (plus
references 0.15 when given). A high score means no measurable problems and a confident genre reading, not a hit.
Optimising against the ears can please the models without the music getting better, which is earshot's own warning;
judge finished songs by ear.
