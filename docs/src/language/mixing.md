# Mixing functions

claudius mixes the way an engineer starts a mix: every track has a **role**, every role has a target loudness, and
each track is brought to its target while it plays. Your settings are offsets from that starting point. This page
covers roles, levels, panning, sends, sidechain, buses, inserts and the master, and the functions that set them.
[Mixing and mastering](../learn/producers/mix-and-master.md) explains how to use them.

## Roles

A role says what a track is in the mix. Each role has a target loudness, measured in LUFS while the track plays
(silence does not count), before the master stage:

| role | target | | role | target |
|---|---|---|---|---|
| `kick` | −12.5 | | `bass` | −14.5 |
| `snare` | −13.5 | | `keys` | −17.0 |
| `clap` | −14.5 | | `chords` | −18.0 |
| `hat` | −17.5 | | `lead` | −18.0 |
| `ohat` | −18.5 | | `synth` | −18.0 |
| `tom` | −18.0 | | `sample` | −18.0 |
| `perc` | −19.5 | | `arp` | −20.5 |
| `crash` | −20.0 | | `pad` | −21.5 |
| `ride` | −20.5 | | `vocal` | −15.0 |
| `shaker` | −21.5 | | `top` | −20.0 |
| `drums` | −14.5 | | `texture` | −27.0 |

Any other role name gets −19. The transition effects have their own: `riser` −19, `impact` −16, `downlifter` −22,
`reverse` −21. The drum roles (`kick snare clap hat ohat shaker perc ride tom crash`) also get the full amount of
[humanize](song-functions.md#humanize), and an audio file on a drum role rings out instead of stopping at its note
length.

How a track gets its role:

- a drum lane: the role of its name when the name is a [drum name](sounds.md#drum-names) (`kick` and `bd` are
  kicks), otherwise the role of the sound it plays;
- a generated part: its role (`bass`, `chords`, `pad`, `lead`, `arp`, `keys`);
- a pattern track: `.role(name)` when the pattern sets it; otherwise, for each sound in it, in this order: the role
  of a drum name anywhere in the sound's name (`tr808:sd` is a snare), `perc` for percussion from the sample packs
  (`conga`, `bongo`), `bass` for the bass synth presets (`808` is a preset first, so it stays `bass`), `chords` for a
  `chord()` pattern, `bass` when the track's median note is below E3 (MIDI 52), and `lead` for everything else.

The last rule is the one to watch. A pad written with `note()` or `chord()`, an arpeggio, a sampled string section:
all become `lead` or `chords` unless you say otherwise, and a `lead` is mixed 3.5 dB louder than a `pad` and checked
by the critic as the melody. Set the role:

```js packs
blank("house");
track("pad",   chord("<Am F>").s("pad").role("pad"));
track("arp",   n("0 2 4 7").scale("A:minor").octave(4).fast(2).s("pluck").role("arp"));
track("conga", s("~ conga ~ [conga conga]"));   // perc without being told
```

Genres shift some targets: techno's kick +1 dB; trap's bass +1.5, kick −1, hat −1; lofi's kick −1.5, snare −0.5,
keys +2.

## Level

The gain a track receives is *target of its role* + *genre offset* + *its level*, minus its own measured loudness,
limited to −40..+30 dB. `level` is therefore relative: `level: -4` on a pad puts it 4 dB under where a pad normally
sits, whatever its sound or velocity. Two tracks with the same role and level play equally loud.

## Pan

`pan` runs from −1 (left) through 0 to 1 (right) and works as a balance control: the far side is turned down and the
near side stays at full level. At ±0.5 the far side is about 5 dB down; at ±1 it is silent.

## Sends

Every track has two sends, `reverb` and `delay`, from 0 to 1. They feed two returns shared by the whole song, whose
settings come from the genre:

| genre | reverb | delay |
|---|---|---|
| house | 2.2 s, 20 ms predelay, low cut 300 Hz | 0.75 beats (dotted eighth), feedback 0.35, ping-pong |
| deep_house | 2.6 s, 25 ms, 280 Hz | 0.75 beats, 0.3 |
| techno | 3.0 s, 30 ms, 350 Hz | 0.5 beats (eighth), 0.45 |
| trap | 1.8 s, 15 ms, 300 Hz | 0.75 beats, 0.3 |
| lofi | 1.6 s, 10 ms, 250 Hz, dark | 0.75 beats, 0.3, dark |
| dnb | 2.8 s, 20 ms, 350 Hz | 0.75 beats, 0.35 |
| future_bass | 2.4 s, 20 ms, 300 Hz, bright | 0.75 beats, 0.35 |
| synthwave | 3.2 s, 30 ms, 250 Hz, wide | 0.75 beats, 0.4 |

The sends are taken after the track's level, sidechain and pan. The returns are ducked by the kick too, by 30 % of
the full depth, so tails breathe with the groove.

## Sidechain

`duck` (0..1) is the depth of sidechain ducking from the kick. At each kick, the track's gain falls to 1 − `duck`
3 ms before the hit and recovers over the genre's release time (0.25 beats in trap and dnb, 0.45 in future bass,
0.3 to 0.35 elsewhere), along a curve that comes back quickly at first. At `duck: 0.85` a chord nearly vanishes on
each kick and swells back: the future-bass pump.

The kick is the first track whose role is `kick`; a song without one ducks nothing. Pattern tracks start with
`duck` 0.3, except drum sounds and the kick itself, which start at 0. Generated parts take the genre's values (in
house: bass 0.55, chords 0.45, pad 0.5, lead 0.2).

## Buses

Each track plays into a bus, and the buses are summed with the reverb and delay returns into the master:

| bus | gets | always has |
|---|---|---|
| `drums` | drum lanes, and pattern tracks that play only drum sounds (drum names, kit sounds, percussion from the packs) | a tanh saturator (2.5 dB, half wet) after the genre's chain, unless the genre already saturates |
| `bass` | generated bass parts, and pattern tracks whose role is `bass` | mono below 140 Hz |
| `music` | everything else | width 1.25 with mono below 160 Hz |
| `fx` | crashes, risers, impacts, downlifters, reverse crashes | |

On top, each genre has its own chain per bus (house: a glue compressor on the drums and a 110 Hz highpass on the
music bus; lofi: a bitcrusher, a lowpass and a compressor on the drums). The arranger adds filter automation to the
`music` and `bass` buses: the lowpass of [`filter_in()`](form.md#what-the-flags-do), the highpass sweep of a build,
the lowpass closing in a final outro. The drums bus is never filtered. A script cannot move a track to another bus or
edit the bus chains.

## Inserts on pattern tracks

A pattern track can carry its own effects, set with [track settings](pattern-functions.md#track-settings). They
run in this order, before the level stage:

1. `.hpf(hz)`: a highpass filter (resonance 0.1);
2. `.lpf(hz)` / `.cutoff(hz)`: a lowpass filter (resonance 0.15);
3. `.crush(bits)`: a bitcrusher, 1 to 24 bits;
4. `.distort(db)`: tanh saturation with that much drive.

Because the level stage comes after, saturation and filtering change the tone, not the loudness.

## Functions

### `mix()`

```js no-run
mix(track, options)   // options: map with any of the fields below
```

| field | type | range (clamped) |
|---|---|---|
| `level` | number, dB relative to the role's target | −40..12 |
| `pan` | number | −1..1 |
| `reverb` (or `room`) | number, send | 0..1 |
| `delay` | number, send | 0..1 |
| `duck` | number, sidechain depth | 0..1 |
| `mute` | bool | |
| `solo` | bool | |

`mix()` works on drum lanes, parts, loops and pattern tracks. On a pattern track it overrides what the pattern set
(`.level()`, `.pan()` ...), until the next `track()` for that name resets the mix to the pattern's settings. The
names `fx` and `transitions` address the transition effects (crashes, risers, impacts).

Errors: `no track 'nope'`; `mix() has no option 'volume' (options: level pan reverb room delay duck mute solo)`.

```js
mix("pad", #{ level: -4, reverb: 0.45 });
mix("lead", #{ pan: 0.2, delay: 0.25, duck: 0.2 });
mix("fx", #{ level: -3 });
```

### `master()`

```js no-run
master(options)   // options: map
```

| field | meaning | range (clamped) | default |
|---|---|---|---|
| `lufs` | integrated loudness of the finished master | −24..−5 | the genre's |
| `ceiling` | true-peak ceiling, dBTP | −3..−0.1 | −1 |

The master stage runs the genre's master chain (EQ, a glue compressor, saturation; tape and vinyl in lofi), then
searches for the input gain that lands the song within 0.15 LU of `lufs`, through a soft clipper 0.1 dB under the
ceiling and a limiter 0.5 dB below it, and trims the result if the true peak still pokes over. The genre targets are house −10,
deep_house −11, techno −9, trap −9, lofi −12.5, dnb −8.5, future_bass −9, synthwave −10.5 LUFS.

Every printed song has a `master()` line with these two values (see [Printing a song](printing.md)).

Errors: `master() has no option 'loudness' (options: lufs ceiling)`.

```js
master(#{ lufs: -9, ceiling: -1 });
```

### `mute()`, `solo()` and the others

[`mute()`, `unmute()`, `solo()`, `unsolo()` and `remove()`](song-functions.md#mute-unmute-solo-unsolo-remove) are
on the song functions page. When any track is soloed, everything else is left out of the arrangement, including the
transition effects (unless `fx` is soloed), and the critic judges only what is left.
