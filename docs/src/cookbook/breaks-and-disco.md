# Trip-hop, big beat, electro house, nu-disco, filter house

Five styles where the production is half the song: dusty breaks and dub delays, distorted drums crushed in parallel,
a bass riff dirtied in the drop only, a band glued on a bus of its own, a disco loop sweeping through a filter. Each
recipe starts from a generated song and adds layers of processing with [effect chains](../language/mixing.md#effect-chains):
`parallel` chains side by side, buses of your own, and settings that move by section, which follow the form when you
move a section. They run without the sample packs; with them, the drums and some parts play recorded kits and
instruments.

## Trip-hop: "Grey Harbour"

```js
song("trip_hop", 2);
title("Grey Harbour");
fx("keys", [
  #{ fx: "parallel", dry: 1, chains: [
    #{ amount: 0.35, fx: [#{ fx: "bitcrush", bits: 8, downsample: 4 }, #{ fx: "filter", mode: "bp", cutoff: 1400, res: 0.3 }] },
  ] },
  #{ fx: "phaser", rate_hz: 0.15, stages: 4, depth: 0.6, feedback: 0.3, mix: 0.35 },
]);
fx("lead", [
  #{ fx: "filter", mode: "hp", cutoff: 320 },
  #{ fx: "saturate", kind: "tape", drive_db: sections(2, #{ hook: [2, 8] }) },
]);
send_fx(#{ delay: #{ fx: "delay", beats: 0.75, feedback: sections(0.3, #{ "break": [0.3, 0.7], outro: [0.3, 0.8] }), damping: 0.6, pingpong: true, mix: 1 } });
```

Why:

- **What the genre already does.** It is built on samples from free online libraries
  ([Genres built on samples](../guide/online-samples.md#genres-built-on-samples)): here a 96 BPM drum break, whose own
  kick, snare and hat the drum lanes play; two bars of "Scary & Suspense", a string phrase the song takes its key
  (E flat minor), tempo and chords from; vinyl crackle under it all; and a sub bass, cellos and a high melody around
  them. The drums go through parallel compression with tape drive and a crushed, lowpassed copy; the music bus dips
  3.5 dB in the breaks. Without gathered samples the song plays electric piano and strings in the phrase's place, and
  its drum lanes in the break's.
- **A broken copy of the keys.** A `parallel` chain bitcrushes the electric piano and band-passes it around 1.4 kHz,
  at a third of the level, under the clean sound: the chords keep their warmth and gain a worn, sampled edge.
- **A melody that wears in.** The lead's tape drive rises from 2 to 8 dB across each hook, by section, so the second
  half of every hook is grainier than the first.
- **Dub throws.** The delay the sends feed repeats longer as each break goes on (feedback 0.3 to 0.7), and longer
  still into the outro, so the echoes swell where the drums drop out.

```text
  ears: 'trip hop music' z +2.8 (closest confusion 'lo-fi hip hop beat' +3.5); production z +1.9 vs 'a distorted clipped recording' +1.1; p_real 0.83
        sounds like: lo-fi hip hop beat, house music, glitch hop music, hip hop beat with heavy bass
  verdict: 86/100 (arc 0.92, genre 0.46, mix 1.00, production 0.70, theory 1.00). Fix first: sounds more like 'lo-fi hip hop beat' than 'trip hop music'
```

Its `credits.txt` names the three samples, their authors and licences. Lo-fi hip hop is the style trip-hop is most
often taken for: both are slow, swung, dusty and sampled, and the ears still hear lo-fi first here. A darker phrase and
heavier drums pull it toward trip-hop; jazzy sevenths and comping electric piano pull it back toward lo-fi.

Try:

- `fx("strings", [])` and hear how much of the mood the filter that opens in the hooks carried.
- `send_fx(#{ delay: #{ fx: "delay", beats: 1.5, feedback: 0.45, damping: 0.7, pingpong: true, mix: 1 } })` for
  slower, darker echoes.

## Big beat: "Rubber Siren"

```js
song("big_beat", 1);
title("Rubber Siren");
fx("riff", [
  #{ fx: "filter", mode: "ladder", cutoff: sections(1800, #{ build: [600, 6000], drop: 4000 }), res: 0.6, drive: 1.5 },
  #{ fx: "parallel", dry: 0.8, chains: [
    #{ amount: 0.5, fx: [#{ fx: "saturate", kind: "fold", drive_db: 14 }, #{ fx: "filter", mode: "hp", cutoff: 700 }] },
  ] },
  #{ fx: "chorus", rate_hz: 0.4, depth_ms: 4, delay_ms: 14, mix: 0.8 },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ kind: "highpass", freq: 30 }] },
  #{ fx: "compressor", threshold_db: -10, ratio: 2.2, attack_ms: 15, release_ms: 120, knee_db: 6 },
  #{ fx: "saturate", kind: "tanh", drive_db: sections(2, #{ drop: 4 }), mix: 0.4 },
] });
```

Why:

- **What the genre already does.** A fast two-bar break on an acoustic kit (or a 909), crushed in parallel: one copy
  compressed at 12:1 and clipped, one bitcrushed to 8 bits and high-passed, both under the clean drums. An acid line
  whose ladder filter opens across each build and whose resonance climbs with it.
- **A riff that screams in the build.** The riff's own ladder filter sweeps from 600 Hz to 6 kHz across every build
  and settles at 4 kHz in the drops; a folded, high-passed copy beside it adds the fuzz without thinning its body,
  and a wide chorus pulls it out of the centre.
- **A master that gets hotter in the drops.** The master's saturator drives 4 dB in the drops and 2 elsewhere. The
  loudness stage still lands the song on its target, so the drops are denser rather than louder.

```text
  ears: 'big beat breakbeat music with distorted drums' z +4.2 (closest confusion 'drum and bass music' +3.2); production z +1.7 vs 'a recording made on a phone' +0.5; p_real 0.84
        sounds like: funk music, playful and quirky music, jazz music, drum and bass music
  verdict: 96/100 (arc 0.95, genre 0.99, mix 1.00, production 0.81, theory 1.00)
```

Big beat accepts sounding distorted: the critic does not count "a distorted clipped recording" against it, as it
does not count hiss against lo-fi.

Try:

- `part("acid", #{ seed: 7 })` for another acid line on the same filter moves.
- `tempo(138)` for a faster, rave-ier take; the delay and the break stay in time.

## Electro house: "Voltage Room"

```js
song("electro_house", 2);
title("Voltage Room");
fx("lead", [
  #{ fx: "saturate", kind: "tanh", drive_db: 4, mix: 0.3 },
  #{ fx: "bitcrush", bits: sections(16, #{ build: [16, 6] }), downsample: 1, mix: sections(0, #{ build: [0, 0.6] }) },
  #{ fx: "width", width: sections(1, #{ drop: 1.5 }) },
]);
mix("chords", #{ bus: "synths" });
mix("pad", #{ bus: "synths" });
bus("synths", [
  #{ fx: "compressor", threshold_db: -18, ratio: 3, attack_ms: 10, release_ms: 100, makeup_db: 2 },
  #{ fx: "eq", bands: [#{ kind: "highpass", freq: 180 }] },
]);
```

Why:

- **What the genre already does.** Four on the floor, a big clap, a saw bass riff in the mids pumped at 0.75 by the
  sidechain, and beside it a folded, high-passed copy whose filter opens across the builds and whose level rises in
  the drops: the dirt arrives with the drop.
- **A lead that breaks up before the drop.** Across each build, the lead's bitcrusher goes from 16 bits to 6 and its
  mix from nothing to 60 %: the hook disintegrates into the drop, then comes back clean and 1.5 times as wide.
- **A synth bus.** The chords and the pad go to a bus of their own, compressed together and high-passed at 180 Hz,
  so they sit as one wall behind the lead and leave the low mids to the bass.

```text
  ears: 'dirty electro house dance music with big synth leads' z +3.9 (closest confusion 'techno music' +3.5); production z +1.3 vs 'a cheap synthesized beep' +1.0; p_real 0.59
        sounds like: techno music, tech house music, house music, chiptune video game music
  verdict: 89/100 (arc 0.86, genre 0.86, mix 1.00, production 0.57, theory 1.00). Fix first: sounds about as much like a cheap synth as a professional mix: sampled instruments in place of synth patches usually read better
```

The ears place electro house between house and techno, which is where it comes from; the genre profile asks for
the dirty bass and the big leads by name, so that the song has to have both to read as itself.

Try:

- `section("drop", #{ energy: 1.0, layers: "kick clap hat ohat bass lead" })` for a stripped, riff-only drop.
- `mix("bass", #{ duck: 0.9 })` for an even harder pump.

## Nu-disco: "Mirrorball Lane"

```js
song("nu_disco", 1);
title("Mirrorball Lane");
mix("guitar", #{ bus: "band" });
mix("keys", #{ bus: "band" });
mix("strings", #{ bus: "band" });
bus("band", [
  #{ fx: "eq", bands: [#{ kind: "highpass", freq: 120 }, #{ kind: "highshelf", freq: 8000, gain_db: 1.5 }] },
  #{ fx: "compressor", threshold_db: -20, ratio: 2.5, attack_ms: 20, release_ms: 150, makeup_db: 2 },
]);
fx("strings", [
  #{ fx: "phaser", rate_hz: 0.25, stages: 6, depth: 0.7, feedback: 0.35, mix: sections(0.2, #{ "break": [0.2, 0.7], hook: 0.3 }) },
  #{ fx: "width", width: 1.4, mono_below_hz: 200 },
]);
```

Why:

- **What the genre already does.** Four on the floor with open hats on the offbeats, an octave or walking bass under
  a compressor, a muted guitar (a recorded clean guitar with the packs) chorused and compressed hard, electric piano
  on the offbeats, strings, and sometimes a brassy hook.
- **The band on one bus.** Guitar, keys and strings go through one high-passed, brightened, compressed bus, the way
  a disco session glued its rhythm section and strings together before the mix.
- **Strings that swirl into the break.** The phaser's mix rises from 20 % to 70 % across the break and settles at
  30 % in the hooks; the strings are spread 1.4 times wider, and mono below 200 Hz.

```text
  ears: 'disco music' z +3.8 (closest confusion 'jazz music' +3.3); production z +1.8 vs 'a distorted clipped recording' +1.2; p_real 0.90
        sounds like: funk music, playful and quirky music, jazz music, celebratory and joyful music
  verdict: 91/100 (arc 0.91, genre 0.86, mix 1.00, production 0.63, theory 1.00)
```

Funk is what the ears hear first, and that is the family the style comes from, so the genre is not checked against
it; jazz, house, synthwave and corporate pop are the confusions that count.

Try:

- `part("bass", #{ style: "walking" })` for a busier, more seventies bass.
- `key("D major")` and `prog("A", "Imaj7 vi7 ii7 V7")` for a sunnier hook.

## Filter house: "Velours"

```js
song("filter_house", 2);
title("Velours");
fx("bass", [
  #{ fx: "filter", mode: "lp", cutoff: sections(900, #{ groove: [900, 1800], drop: 2600 }), res: 0.35 },
  #{ fx: "parallel", dry: 1, chains: [
    #{ amount: sections(0, #{ drop: 0.35 }), fx: [#{ fx: "saturate", kind: "tube", drive_db: 12 }, #{ fx: "filter", mode: "hp", cutoff: 400 }] },
  ] },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ kind: "highpass", freq: 30 }] },
  #{ fx: "compressor", threshold_db: -12, ratio: 2.2, attack_ms: 10, release_ms: 120, knee_db: 4 },
  #{ fx: "saturate", kind: "tape", drive_db: sections(1.5, #{ drop: 3 }), mix: 0.35 },
] });
```

Why:

- **What the genre already does.** The loop, a disco phrase of chords pumped at 0.8 by the kick, goes through a
  resonant lowpass that follows the arrangement section by section: it opens across the intro and the groove, sits
  wide open in the drops, closes across each break and again across the outro. A phaser and a chorus move it, and a
  hard compressor squashes it.
- **A bass that follows the loop.** Its own lowpass opens across the groove and sits at 2.6 kHz in the drops, where
  a tube-driven copy, high-passed at 400 Hz, comes in beside it.
- **A tape-hot drop.** The master's tape saturation drives twice as hard in the drops.

```text
  ears: 'french house music with a filtered disco loop' z +4.2 (closest confusion 'house music' +3.7); production z +1.3 vs 'a low quality recording with hiss' +1.2; p_real 0.53
        sounds like: house music, tech house music, drum and bass music, techno music
  verdict: 87/100 (arc 0.95, genre 0.87, mix 0.93, production 0.51, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -28.9 dB, healthy from -26); sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause; fizzy top end (air -16.1 dB, healthy up to -17)
```

The filtered loop takes the presence band with it while it is closed, which the critic hears as dull hats; a brighter
hat (`drum_sound("hat", "tr909:hh")`) or a clap a little louder (`mix("clap", #{ level: 1.5 })`) answers it.

Try:

- `section("break", #{ layers: "loop" })` for a break of nothing but the loop closing down.
- `fx("loop", [])` to hear the loop unfiltered: the arrangement goes flat.
