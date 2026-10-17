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

The "against" list matters: the genre score is high only when a song reads as its genre *more than* as the styles it
is usually confused with.

## What a profile contains

Each profile (`crates/claudius/data/genres/*.json`) has:

| field | what it decides |
|---|---|
| `bpm`, `swing`, `humanize` | tempo and feel ranges |
| `modes`, `harmony`, `markov`, `ext`, `bars_per_chord` | keys, [harmonic styles](harmony.md#the-generators-harmony), how often a fresh Markov progression is written, chord extensions, chord lengths |
| `forms` | the section plans, with layers, energies and flags |
| `drums` | the lanes: per-step hit probabilities and velocity ranges per variant, or a euclidean rhythm; drum voice parameter ranges; which Apple Loops one-shots may replace the voice; choke groups (open and closed hat) |
| `kits` | which recorded drum machines the drums may be played on, and how often |
| `parts` | bass, chords, pad, lead, arp, keys: their presets and instruments, writing styles, registers, density, voicing, and the chance a song leaves the part out |
| `loops` | the Apple Loops a song may layer, by instrument, genre and descriptors |
| `transitions` | risers, impacts, downlifters, reverse crashes, riser length |
| `mix` | the loudness target, reverb and delay, bus and master chains, sidechain release, role level offsets |
| `query`, `alts` | what the ears should hear, and its confusions |

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
