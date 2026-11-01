# Mixing functions

claudius mixes the way an engineer starts a mix: every track has a **role**, every role has a target loudness, and
each track is brought to its target while it plays. Your settings are offsets from that starting point. This page
covers roles, levels, panning, sends, sidechain, buses, effect chains and the master, and the functions that set them.
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
| trip_hop | 3.2 s, 30 ms, 250 Hz, dark | 0.75 beats, 0.32, rising to 0.58 across each break, ping-pong |
| big_beat | 1.8 s, 15 ms, 350 Hz | 0.75 beats, 0.35, ping-pong |
| electro_house | 2.0 s, 20 ms, 350 Hz | 0.75 beats, 0.35, ping-pong |
| nu_disco | 2.0 s, 20 ms, 300 Hz | 0.75 beats, 0.3, ping-pong |
| filter_house | 1.6 s, 15 ms, 300 Hz | 0.75 beats, 0.3, ping-pong |

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
the lowpass closing in a final outro. The drums bus is never filtered. A script moves a track to another bus and sets
any bus's chain ([Effect chains](#effect-chains)): the arranger's filters go in front of the `music` and `bass` chains
whatever they are, and its width and saturator only where the chain has none. A bus of a new name gets nothing from
the arranger.

## Inserts on pattern tracks

A pattern track can carry its own effects, set with [track settings](pattern-functions.md#track-settings). They
run in this order, before the level stage:

1. `.hpf(hz)`: a highpass filter (resonance 0.1);
2. `.lpf(hz)` / `.cutoff(hz)`: a lowpass filter (resonance 0.15);
3. `.crush(bits)`: a bitcrusher, 1 to 24 bits;
4. `.distort(db)`: tanh saturation with that much drive.

Because the level stage comes after, saturation and filtering change the tone, not the loudness.

## Effect chains

Every track has a chain of insert effects, and so do the buses and the master; the two sends feed a reverb and a
delay. A script sets any of them, effect by effect, with the settings the engine keeps (the same map is what
`blueprint.json` stores):

```js
fx("lead", [
  #{ fx: "saturate", kind: "tape", drive_db: 6 },
  #{ fx: "delay", beats: 0.75, feedback: 0.4, pingpong: true, mix: 0.25 },
]);
```

- `fx(track, chain)` replaces a track's whole chain, and `fx("lead", [])` empties it. The chain runs in order, before
  the level stage, so it changes the tone, not the loudness. A pattern track's `.hpf()`, `.lpf()`, `.crush()` and
  `.distort()` are the first effects of its chain, and the next `track()` for that name sets them again.
- `bus(name, chain)` does the same for a bus, `master(#{ fx: chain })` for the master, and
  `send_fx(#{ reverb: effect, delay: effect })` replaces what the sends feed.
- `mix(track, #{ bus: name })` sends a track to another bus. A bus of a new name, with a chain of its own, groups
  tracks to process them together.

```js
mix("chords", #{ bus: "keys" });
mix("pad", #{ bus: "keys" });
bus("keys", [
  #{ fx: "compressor", threshold_db: -20, ratio: 3, attack_ms: 10, release_ms: 120, makeup_db: 2 },
  #{ fx: "width", width: 1.4, mono_below_hz: 200 },
]);
```

The printed song lists every chain there is ([Printing a song](printing.md)), so `get_song` shows what a track goes
through before you change it.

### Parallel chains

`parallel` runs the signal through several chains side by side. Each comes out as loud as its `amount`, over the
signal itself as loud as `dry`. Both are gains: 1 is as loud as it comes, 0.5 is 6 dB down, 0 is gone. A chain can
hold any effects, another `parallel` too.

```js
fx("bass", [#{ fx: "parallel", dry: 1, chains: [
  #{ amount: 0.5, fx: [#{ fx: "compressor", threshold_db: -30, ratio: 10, attack_ms: 1, release_ms: 60 }] },
  #{ amount: 0.3, fx: [#{ fx: "saturate", kind: "fold", drive_db: 18 }, #{ fx: "filter", mode: "hp", cutoff: 700 }] },
] }]);
```

That is parallel compression under the bass, and a folded copy above its fundamental for grit, which the bass keeps
its weight under.

### Settings that move

A setting the table below marks as moving is a number, a curve, or a value for each section:

- A curve is a list of `[beat, value]` points, in beats from the song's start. Between two points it moves in a
  straight line (a cutoff in equal musical steps); before the first point and after the last it holds.
- `sections(rest, #{ section: value })` holds a value through each section it names, or moves through several across
  it: `[0, 1]` rises across the section, `[0, 1, 0]` rises and falls back. Every other section holds `rest`. A key is
  a section's name (`drop2`) or a kind (`build`, `drop`, `hook` ...); a section's own name counts before its kind.
  Moving to the next section's value takes a 32nd of a beat, so that a step does not click.

Values by section follow the form when the song renders: moving, adding or removing a section moves them with it,
where a curve stays on its beats.

```js
fx("pad", [
  #{ fx: "filter", mode: "lp", cutoff: sections(1200, #{ build: [1200, 9000], drop: 9000 }), res: 0.3 },
  #{ fx: "reverb", decay: 4.5, mix: sections(0.15, #{ "break": 0.5 }) },
]);
send_fx(#{ delay: #{ fx: "delay", beats: 0.75, feedback: sections(0.3, #{ outro: [0.3, 0.8] }), pingpong: true, mix: 1 } });
```

`break` is a word Rhai keeps for loops, so as a key it is quoted, or written `breakdown`. `chorus` names the `hook`
kind too.

### The effects

Each effect is a map with its name in `fx` and its settings; a setting with a default may be left out. `mix` is the
share of the effect in what comes out, from 0 (the signal as it was) to 1 (the effect alone), except in `delay`,
where it is how loud the repeats are on top of the signal.

| `fx` | settings (default) | moves |
|---|---|---|
| `gain` | `db` | `db` |
| `eq` | `bands`: a list of `#{ kind, freq, gain_db (0), q (0.707) }`, kind `lowpass highpass bandpass notch peak lowshelf highshelf allpass` | |
| `filter` | `mode` (`lp hp bp notch ladder`), `cutoff` in Hz, `res` (0), `drive` (1, the ladder's), `mix` (1) | `cutoff res mix` |
| `compressor` | `threshold_db`, `ratio`, `attack_ms`, `release_ms`, `knee_db` (0), `makeup_db` (0), `mix` (1) | `mix` |
| `limiter` | `ceiling_db`, `release_ms` | |
| `saturate` | `kind` (`tanh tube tape hard fold`), `drive_db`, `mix` (1) | `drive_db mix` |
| `bitcrush` | `bits` (1 to 24), `downsample` (1: none), `mix` (1) | `bits mix` |
| `reverb` | `decay` in seconds, `damping` (0), `predelay_ms` (0), `width` (1), `mix`, `lowcut` (250 Hz) | `mix` |
| `delay` | `beats`, `feedback` (0 to 0.95), `damping` (0), `pingpong` (false), `mix` | `feedback mix` |
| `chorus` | `rate_hz`, `depth_ms`, `delay_ms`, `mix` | `mix` |
| `flanger` | `rate_hz`, `depth_ms`, `delay_ms`, `feedback`, `mix` | `mix` |
| `phaser` | `rate_hz`, `stages`, `depth`, `feedback`, `mix` | `mix` |
| `width` | `width` (0 mono, 1 as it is, more is wider), `mono_below_hz` (0) | `width` |
| `tremolo`, `auto_pan` | `beats` (one cycle), `depth` | |
| `gate` | `steps` (a level for each step), `step_beats` (0.25), `depth`, `smooth_ms` (0) | |
| `transient` | `attack_db`, `sustain_db` | |
| `tape` | `wow`, `flutter`, `drive_db`, `lowpass_hz` | |
| `vinyl` | `crackle`, `hiss_db` | |
| `tape_stop`, `reverse` | `start` (a beat), `len` (beats) | |
| `stutter` | `start`, `len`, `slice` (beats) | |
| `parallel` | `chains`: a list of `#{ fx: chain, amount (1) }`; `dry` (1) | `amount dry` |

Errors say which effect of which chain is wrong, and how: `fx("lead")'s effect 2: unknown variant `revrb`, expected
one of `gain`, `eq` ...`; `... is a map such as #{ fx: "reverb", ... }`; `missing field `decay``; a setting that
cannot be read as a number, a curve or values by section; `sections(): no section or kind 'chorus2'`.

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
| `bus` | string, the bus the track plays into | letters, digits, `-` and `_` |

`mix()` works on drum lanes, parts, loops and pattern tracks. On a pattern track it overrides what the pattern set
(`.level()`, `.pan()` ...), until the next `track()` for that name resets the mix to the pattern's settings. The
names `fx` and `transitions` address the transition effects (crashes, risers, impacts).

Errors: `no track 'nope'`; `mix() has no option 'volume' (options: level pan reverb room delay duck mute solo bus)`.

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
| `fx` | the master's chain, as for [`fx()`](#effect-chains) | | the genre's |

The master stage runs the genre's master chain (EQ, a glue compressor, saturation; tape and vinyl in lofi), then
searches for the input gain that lands the song within 0.15 LU of `lufs`, through a soft clipper 0.1 dB under the
ceiling and a limiter 0.5 dB below it, and trims the result if the true peak still pokes over. The genre targets are house −10,
deep_house −11, techno −9, trap −9, lofi −12.5, dnb −8.5, future_bass −9, synthwave −10.5, trip_hop −11, big_beat
−8.5, electro_house −8, nu_disco −10 and filter_house −9 LUFS.

Every printed song has a `master()` line with these two values (see [Printing a song](printing.md)).

Errors: `master() has no option 'loudness' (options: lufs ceiling fx)`.

```js
master(#{ lufs: -9, ceiling: -1 });
```

### `mute()`, `solo()` and the others

[`mute()`, `unmute()`, `solo()`, `unsolo()` and `remove()`](song-functions.md#mute-unmute-solo-unsolo-remove) are
on the song functions page. When any track is soloed, everything else is left out of the arrangement, including the
transition effects (unless `fx` is soloed), and the critic judges only what is left.
