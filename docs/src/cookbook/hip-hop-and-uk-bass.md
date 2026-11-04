# Boom bap, UK drill, dubstep, UK garage, jungle

Five styles built on their drums and their bass: a sampler's lazy swing, a half-time snare over sliding 808s, a
wobble, a shuffle, a chopped breakbeat. As in [the other recipes](pop-psych-and-glitch.md), each starts from a
generated song and changes it with [effect chains](../language/mixing.md#effect-chains), and runs without the sample
packs.

## Boom bap: "Dusty Corners"

```js
song("boom_bap", 3);
title("Dusty Corners");
bus("drums", [
  #{ fx: "compressor", threshold_db: -16, ratio: 4, attack_ms: 15, release_ms: 90, makeup_db: 3 },
  #{ fx: "saturate", kind: "tape", drive_db: 5, mix: 0.6 },
  #{ fx: "parallel", dry: 1, chains: [
    #{ amount: 0.3, fx: [#{ fx: "bitcrush", bits: 12, downsample: 2 }, #{ fx: "filter", mode: "hp", cutoff: 300 }] },
  ] },
]);
fx("keys", [
  #{ fx: "filter", mode: "hp", cutoff: 180 },
  #{ fx: "tape", wow: 0.3, flutter: 0.2, drive_db: 3, lowpass_hz: 10000 },
]);
```

Why:

- **What the genre already does.** Three verses and hooks built on samples from free libraries
  ([Songs built on samples](../guide/online-samples.md#songs-built-on-samples)). Here they are a hip hop drum loop,
  whose own kick, snare and hat the drum lanes play, a neo-soul chord loop the song takes its key and chords from,
  chopped and crushed to 12 bits, and a texture under them, with an organ bass and horns in the hooks. Without
  gathered samples, the drum lanes and a piano or Rhodes play in their place.
- **A sampler's drums.** The drums bus is compressed at 4:1 with an attack slow enough to let the kick and snare
  crack through, then saturated on tape; beside it, a third as loud, a copy crushed to 12 bits and half the sample
  rate, above 300 Hz, for the grit of an old sampler.
- **A loop that leaves room, without samples.** When the keys play in the phrase's place, they are high-passed at
  180 Hz, as a sampled loop is thinned to sit over a bass, and run through a tape that wavers a little.

```text
  ears: 'boom bap hip hop beat' z +3.3 (closest confusion 'lo-fi hip hop beat' +5.0); production z +1.7 vs 'a low bitrate mp3 with artifacts' +1.1; p_real 0.86
        sounds like: lo-fi hip hop beat, hip hop beat with heavy bass, glitch hop music, minimal electronic music
  verdict: 86/100 (arc 0.96, genre 0.45, mix 1.00, production 0.66, theory 1.00). Fix first: sounds more like 'lo-fi hip hop beat' than 'boom bap hip hop beat'
```

Its `credits.txt` names the three samples. Lo-fi hip hop grew out of boom bap, sampled loops and all, and the ears
hear it first; the harder drums, the brighter mix and the song form of verses and hooks are what boom bap keeps of its
own.

Try:

- `swing(0.62)` for a lazier lean.
- `part("horns", #{ preset: "whistle" })` for a whistled hook in place of the horns.

## UK drill: "Southside Glide"

```js
song("uk_drill", 2);
title("Southside Glide");
fx("bass", [
  #{ fx: "saturate", kind: "tube", drive_db: 6, mix: 0.5 },
  #{ fx: "eq", bands: [#{ kind: "peak", freq: 110, gain_db: 2, q: 1 }] },
]);
fx("keys", [
  #{ fx: "filter", mode: "lp", cutoff: sections(2500, #{ hook: [2500, 9000] }), res: 0.2 },
  #{ fx: "width", width: 1.5, mono_below_hz: 200 },
]);
```

Why:

- **What the genre already does.** Around 140 BPM felt at half time: a snare on the third beat, answered late every
  other bar, hats in threes that roll, a sparse kick with an 808 that follows it; dark minor and harmonic minor
  chords on piano, strings or choir, and a bell or flute line in the hooks. The song opens on its hook.
- **An 808 that growls.** Tube saturation, half wet, gives the 808 harmonics a phone speaker can play, and a 2 dB
  lift at 110 Hz gives it weight.
- **Keys that open in the hooks.** A lowpass on the keys rises from 2.5 to 9 kHz across each hook and stays at
  2.5 kHz elsewhere, so the verses stay dark; above 200 Hz they are widened to 1.5.

```text
  ears: 'uk drill beat' z +2.4 (closest confusion 'hip hop beat with heavy bass' +3.4); production z +1.0 vs 'a recording made on a phone' +0.4; p_real 0.47
        sounds like: hip hop beat with heavy bass, trap beat, lo-fi hip hop beat, an electric piano
  verdict: 84/100 (arc 0.99, genre 0.29, mix 1.00, production 0.67, theory 1.00). Fix first: sounds more like 'hip hop beat with heavy bass' than 'uk drill beat'
```

Drill is a child of trap, and the ears hear the family; the snare that answers late and the hats in threes are what
drill has of its own.

Try:

- `tempo(144)` for a more urgent roll.
- `mix("bass", #{ level: 2 })` for an 808 that leads the mix.

## Dubstep: "Concrete Tide"

```js
song("dubstep", 4);
title("Concrete Tide");
fx("wobble", [
  #{ fx: "filter", mode: "lp", cutoff: sections(900, #{ drop: [600, 2400], drop2: [900, 3200] }), res: 0.5 },
  #{ fx: "saturate", kind: "fold", drive_db: 6, mix: 0.4 },
]);
fx("snare", [#{ fx: "parallel", dry: 1, chains: [
  #{ amount: 0.4, fx: [#{ fx: "reverb", decay: 1.8, predelay_ms: 10, mix: 1 }] },
] }]);
```

Why:

- **What the genre already does.** 140 BPM at half time: a big snare on the third beat over a sparse kick; in the
  drops, a wobbling mid bass over a clean sub; pads on dark minor and phrygian chords; eight-bar risers into the
  drops and a stripped back break between them.
- **A wobble that opens.** A resonant lowpass on the wobble climbs from 600 Hz to 2.4 kHz across the first drop and
  from 900 Hz to 3.2 kHz across the second, so the second hits harder; a wavefolder after it adds grit.
- **A bigger snare.** A reverb of its own beside the snare, at 40 %, gives it a short, heavy tail.

```text
  ears: 'dubstep music' z +3.1 (closest confusion 'drum and bass music' +3.3); production z +0.7 vs 'a distorted clipped recording' +0.9; p_real 0.39
        sounds like: hip hop beat with heavy bass, drum and bass music, dubstep music, synthwave music
  verdict: 86/100 (arc 1.00, genre 0.59, mix 0.99, production 0.46, theory 1.00). Fix first: sounds about as much like a clipped recording as a professional mix: less drive and saturation, or a quieter master; sounds more like 'drum and bass music' than 'dubstep music'; fizzy top end (air -16.6 dB, healthy up to -17)
```

Dubstep's bass is meant to be dirty, and the ears read some of that as clipping; a gentler `drive_db: 3` on the
wavefolder, or `master(#{ lufs: -10 })`, trades some of the grit for a cleaner reading.

Try:

- `part("wobble", #{ style: "sustain" })` for one long wobble a chord.
- `mix("sub", #{ level: 2 })` for more weight under it.

## UK garage: "Late Bus Home"

```js
song("uk_garage", 2);
title("Late Bus Home");
swing(0.64);
fx("lead", [
  #{ fx: "filter", mode: "bp", cutoff: 1800, res: 0.3 },
  #{ fx: "delay", beats: 0.375, feedback: 0.35, pingpong: true, mix: 0.3 },
]);
fx("chords", [
  #{ fx: "chorus", rate_hz: 0.6, depth_ms: 3, delay_ms: 12, mix: 0.4 },
  #{ fx: "width", width: 1.4, mono_below_hz: 200 },
]);
```

Why:

- **What the genre already does.** 130 to 136 BPM with a heavy shuffle: a two-step kick on the one and after the
  third beat, never on it; snares on 2 and 4 with ghost notes, shuffled hats, shakers and a rim; a bouncing sub or
  organ bass under minor 7th chords on organ, Rhodes or stabs; verses and hooks around a break.
- **More shuffle.** `swing(0.64)` pushes every second sixteenth later still.
- **A lead like a chopped vocal.** A band-pass at 1.8 kHz narrows the lead to a voice's middle, and a ping-pong
  delay of a dotted sixteenth (0.375 beats) makes it skip between the speakers.
- **Chords that shimmer.** A chorus and a width of 1.4 above 200 Hz spread the chords around the lead.

```text
  ears: 'uk garage music' z +2.5 (closest confusion 'house music' +3.7); production z +1.6 vs 'a distorted clipped recording' +0.9; p_real 0.64
        sounds like: house music, tech house music, drum and bass music, funk music
  verdict: 83/100 (arc 0.93, genre 0.31, mix 1.00, production 0.68, theory 1.00). Fix first: sounds more like 'house music' than 'uk garage music'
```

Garage grew out of house, and the ears often say so; the kick that skips the third beat and the shuffle are what make
it garage.

Try:

- `swing(0.6)` for a tighter, straighter groove.
- `part("chords", #{ preset: "organ" })` for the organ stabs of the late 90s.

## Jungle: "Rinse at Dawn"

```js
song("jungle", 1);
title("Rinse at Dawn");
send_fx(#{ delay: #{ fx: "delay", beats: 0.75, feedback: sections(0.4, #{ "break": [0.4, 0.7] }), damping: 0.6, pingpong: true, mix: 1 } });
bus("drums", [
  #{ fx: "compressor", threshold_db: -14, ratio: 4, attack_ms: 3, release_ms: 60, makeup_db: 3 },
  #{ fx: "saturate", kind: "tube", drive_db: 3, mix: 0.4 },
]);
mix("chords", #{ delay: 0.5 });
```

Why:

- **What the genre already does.** 160 to 172 BPM: a chopped breakbeat with ghost snares on an acoustic kit, a deep
  sub or reese bass moving at half the speed of the drums, dub chords and stabs sent into long delays, warm pads; a
  long intro, a build, and two drops around a break.
- **Echoes that pile up.** The delay the sends feed climbs from 0.4 to 0.7 feedback across the break, so the chords'
  echoes build into the second drop, and the chords send half their signal into it.
- **A squashed break.** The drums bus is compressed at 4:1 with a 3 ms attack, the way a sampler's break was squashed
  to make it roll, then warmed with a tube.

```text
  ears: 'jungle music' z +2.7 (closest confusion 'drum and bass music' +2.9); production z +0.8 vs 'a low quality recording with hiss' +0.6; p_real 0.84
        sounds like: jazz music, energetic and driving music, drum and bass music, aggressive music
  verdict: 86/100 (arc 0.99, genre 0.54, mix 1.00, production 0.55, theory 1.00). Fix first: sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause; sounds more like 'drum and bass music' than 'jungle music'
```

Jungle is drum and bass's parent, and the ears know the family; the chopped break and the space around the chords
are what make it jungle.

Try:

- `tempo(160)` for a slower roll.
- `mix("pad", #{ reverb: 0.6 })` for a deeper fog behind the break.
