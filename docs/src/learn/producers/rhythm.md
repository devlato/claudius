# Rhythm and groove

Rhythm in claudius has three layers: where notes sit (patterns and step strings), how hard they are played
(velocity), and how the grid is bent when the song renders (swing and humanize). This page gives the exact behaviour
of each and the idioms that come out of them.

## The grid

Pattern time is continuous: a cycle is one bar, and mini-notation divides it however you like. Common subdivisions:

| feel | mini-notation | step string |
|---|---|---|
| quarters | `bd*4` | `x...x...x...x...` |
| eighths | `hh*8` | `x.x.x.x.x.x.x.x.` |
| sixteenths | `hh*16` | `xxxxxxxxxxxxxxxx` |
| offbeat eighths | `[~ hh]*4` | `..x...x...x...x.` |
| eighth triplets | `hh*12` | 12 steps per bar: `drum("hat", "main", "xxxxxxxxxxxx", 12)` |
| sixteenth triplets | `hh*24` | 48 steps per bar, every second step |
| thirty-seconds | `hh*32` | 32 steps per bar |

Drum lanes ([`drum()`](../../language/song-functions.md#drum)) are a step grid with 16 steps per bar by default, or any
resolution from 1 to 96. When a lane prints as a script, it uses 16 steps if every hit sits on a sixteenth, then 32,
then 48.

## Velocity

| where | how |
|---|---|
| step strings | `X` 1.0, `x` 0.8, `o` 0.45 (ghost), `1` to `9` for 0.1 to 0.9 |
| patterns | `.gain()` or `.velocity()`, per event: `.gain("[0.5 0.3 0.9 0.3]*4")` |
| defaults | 0.9 for sound patterns, 0.8 for pitched patterns |

Velocity drives level and tone the way the instrument does. Synthesized drums and audio files scale level by
velocity to the power 1.6 (0.5 is about 10 dB below 1.0); the sampled kits switch velocity layers (the 909 has eleven)
as well; synth presets open their filters. The track as a whole is then levelled to its role's target, so velocity
shapes the groove, not the track's loudness.

A sixteenth hat with accents on the off-beat eighths and soft ghosts between:

```js packs
blank("house");
tempo(124);
track("hats", s("hh*16").bank("tr909").gain("[0.35 0.2 0.8 0.2]*4"));
```

## Swing

[`swing(amount)`](../../language/song-functions.md#swing) is sixteenth-note swing for the whole song:

- each off-beat sixteenth (the second of each pair inside an eighth note) moves to *amount* of the way through the
  eighth; 0.5 is straight, 0.66 is a triplet feel; the range is 0 to 0.8, and up to 0.501 is straight;
- only notes within 0.01 beats of a sixteenth grid position move, so eighth notes, triplets and notes nudged with
  `.late()` are untouched;
- it applies at render to every track: drum lanes, parts and pattern tracks alike.

The genres' generated swing ranges are a good guide: house 0.50–0.56, deep house 0.53–0.58, lofi 0.56–0.63, trap
0.50–0.54, techno 0.50–0.52, drum and bass 0.50–0.51, future bass 0.50–0.53, synthwave straight.

Since swing is global, a part that should sit straight in a swung song has to avoid off-beat sixteenths (use eighths
or triplets), and a part that should lag can use `.late(0.01)` to `.late(0.03)`, which also takes it off the grid that
swing moves.

## Humanize

[`humanize(amount)`](../../language/song-functions.md#humanize) adds random timing and velocity at render:

- timing: a normal distribution with a standard deviation of 4 ms × *amount* on drum roles, 2.4 ms × *amount* on
  others (synth tracks: 3 ms and 1.8 ms);
- velocity: ±5 % × *amount* (one standard deviation);
- seeded by the song and the track name, so every render is the same.

Generated songs use 0.25 in house, 0.35 in deep house, 0.6 in lofi, 0.15 in trap and 0.1 elsewhere. `blank()` keeps
the genre's value.

## Idioms

**Ghost notes** are low-velocity hits between the main ones. In a step string, `o`; in a pattern, a quiet layer in a
`stack()`, since a gain pattern applies by time and would turn down the backbeat too:

```js packs
blank("lofi");
tempo(86); swing(0.6);
track("snare", stack([s("~ sd ~ sd").gain(0.9), s("~ ~ ~ [~ sd] ~ ~ [~ sd] ~").gain(0.3)]).bank("acoustic"));
```

**Rolls** are fast repeats inside one beat. Write the bar beat by beat, with the roll in the beat that has it:

```js packs
blank("trap");
tempo(140);
track("hats", s("<[hh*2 hh*2 hh*2 hh*2] [hh*2 hh*2 hh*2 hh*6] [hh*2 hh*2 hh*2 hh*2] [hh*2 hh*4 hh*2 hh*8]>")
                .bank("tr808").gain("[0.9 0.6]*4"));
```

Bar 2 ends with a sixteenth-triplet roll (six in a beat), bar 4 with thirty-seconds. Generated trap hats get rolls from
the drum grammar instead: random beats replaced by bursts of thirty-seconds or triplets with a rising or falling
velocity ramp, most often on the last beat of a bar and rarely on the first.

**Euclidean rhythms** spread k hits over n steps as evenly as possible, which gives most of the world's clave-like
patterns: `(3,8)` is the tresillo (`x..x..x.`), `(5,8)` the cinquillo (`x.xx.xx.`), `(5,16)` and `(7,16)` are good
percussion lines over a four-on-the-floor kick. A third number rotates the rhythm left by that many steps (see
[Euclidean rhythms](../../language/mini-notation.md#euclidean-rhythms)).

```js packs
blank("techno");
tempo(130);
track("kick", s("bd*4").bank("tr909"));
track("rim",  s("rim(5,16,2)").bank("tr909").gain(0.6).pan(0.3));
track("perc", s("cb(3,8,1)").gain(0.4).pan(-0.3));
```

**Polymeter.** There is no `{ }` polymeter, but a fractional `fast()` runs a short figure across bar lines. A
three-step figure on a sixteenth grid is three sixteenths long, so it repeats 16/3 times per bar:

```js
blank("techno");
track("perc", s("rim ~ ~").fast(16.0 / 3.0).gain(0.5));   // lands one sixteenth earlier each bar
```

It restarts with every section, like every pattern.

**Kick and bass together.** A bass that plays exactly with the kick uses the kick's rhythm as its structure. Keep the
rhythm in a variable so the two cannot drift apart:

```js packs
blank("trap");
tempo(140); key("C minor");
let kick = "x ~ ~ ~ ~ ~ ~ x ~ ~ x ~ ~ ~ ~ ~";
track("kick", s("bd").struct(kick).bank("tr808"));
track("808",  note("<c2!2 ab1!2 f1!2 g1!2>").struct(kick).s("808").legato(1.0));
```

Generated parts do the same with the `kick_follow` bass style, which plays on every kick of the main kick pattern.

## Drum lanes in the arrangement

Drum lanes change with the sections they play in:

- each section plays the lane's variant it asks for (`main`, `sparse`, `alt`, `half` ...), or `main` when the lane
  does not have it; intros and outros ask for `sparse`;
- `fills()` replaces the last one or two beats of every 8-bar phrase on the snare (or clap) lane with a fill, and
  sometimes drops the kick for the last half beat;
- `build()` turns the snare (or clap) lane into a snare roll for the second half of the build and drops the kick for
  its last bar.

[Form and sections](../../language/form.md#what-the-flags-do) has the exact rules. Pattern tracks are never rewritten;
make fills with `lastOf()` and builds with a track that only plays in the build:

```js packs
blank("house");
tempo(124);
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4").bank("tr909").lastOf(8, |x| x.ply(2)));
track("roll",  s("<sd*2 sd*2 sd*4 sd*4 sd*8 sd*8 sd*16 sd*16>").bank("tr909")
                 .gain("<0.4 0.45 0.5 0.55 0.65 0.75 0.85 1>"));
form([sec("groove", 16, "drums"), sec("build", 8, "roll").gap(), sec("drop", 16, "drums").impact()]);
```

## The generator's grooves

Generated drum lanes come from each genre's drum grammar: for every lane, a probability of a hit on each sixteenth
step (1.0 always hits), a velocity range scaled by the metrical weight of the step, a chance of ghost notes, a chance
of rolls, and a chance that the last bar of the 4-bar pattern varies. Some lanes use a euclidean rhythm instead
(house percussion is a rotated `(5,16)`). The same seed gives the same grooves; mutation edits single hits, nudges
them by a sixteenth, re-accents them or regenerates a lane.
