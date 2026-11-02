# Afrobeats, amapiano, reggaeton, trance, neo-soul

Five grooves from further afield: a clave and congas, a log drum under a steady kick, the dembow, a trance
breakdown, a laid-back live kit. As in [the other recipes](hip-hop-and-uk-bass.md), each starts from a generated song
and changes it with [effect chains](../language/mixing.md#effect-chains), and runs without the sample packs.

## Afrobeats: "Lagos Sunset"

```js
song("afrobeats", 4);
title("Lagos Sunset");
fx("guitar", [
  #{ fx: "chorus", rate_hz: 0.8, depth_ms: 2, delay_ms: 10, mix: 0.35 },
  #{ fx: "delay", beats: 0.25, feedback: 0.25, pingpong: true, mix: 0.2 },
]);
mix("conga", #{ pan: -0.35, reverb: 0.2 });
mix("shaker", #{ pan: 0.4 });
```

Why:

- **What the genre already does.** A kick on one and three with a pickup, claps on 2 and 4, a rim on the clave,
  shakers in sixteenths and congas tuned to the key; bright major and minor chords on guitar plucks and keys, a warm
  sub, a sung-sounding lead in the hooks; three hooks and a break.
- **A guitar that ripples.** A quick chorus and a ping-pong echo a sixteenth long make the guitar's plucks ripple
  between the speakers.
- **Percussion around the beat.** The congas move left and into the reverb, the shaker right, so the groove fills
  the space around the kick and the claps.

```text
  ears: 'afrobeats music' z +3.2 (closest confusion 'reggaeton music' +2.7); production z +1.6 vs 'a recording made on a phone' +0.5; p_real 0.74
        sounds like: playful and quirky music, upbeat corporate pop music, funk music, celebratory and joyful music
  verdict: 93/100 (arc 0.97, genre 0.80, mix 1.00, production 0.76, theory 1.00)
```

The ears hear afrobeats first and reggaeton second: the two share a dancing, syncopated kick, and the clave, the
congas and the guitar are afrobeats' own.

Try:

- `swing(0.56)` for a looser roll.
- `part("lead", #{ preset: "whistle" })` for a whistled hook.

## Amapiano: "Soweto Steps"

```js
song("amapiano", 1);
title("Soweto Steps");
fx("logdrum", [
  #{ fx: "filter", mode: "lp", cutoff: sections(700, #{ drop: [700, 1800], drop2: [900, 2400] }), res: 0.35 },
  #{ fx: "saturate", kind: "tube", drive_db: 4, mix: 0.4 },
]);
fx("keys", [#{ fx: "parallel", dry: 1, chains: [
  #{ amount: sections(0, #{ break2: 0.4 }), fx: [#{ fx: "reverb", decay: 4, predelay_ms: 30, mix: 1 }, #{ fx: "filter", mode: "hp", cutoff: 500 }] },
] }]);
```

Why:

- **What the genre already does.** A steady kick under shakers and offbeat open hats, a clap on 2 and 4 and a rim;
  the log drum, a pitched bass of short, woody notes in syncopated lines; jazzy minor 7th and 9th chords on piano
  and Rhodes, airy pads, a whistle or flute; long grooves, breaks that hold back, and drops.
- **A log drum that opens.** A resonant lowpass on the log drum opens from 700 Hz to 1.8 kHz across the first drop
  and from 900 Hz to 2.4 kHz across the second, and a tube gives it its knock.
- **Keys that drift off.** In the second break only, a long reverb beside the keys, high-passed at 500 Hz, comes in
  at 40 %.

```text
  ears: 'amapiano music' z +2.3 (closest confusion 'afrobeats music' +4.2); production z +1.1 vs 'a low quality recording with hiss' +0.6; p_real 0.80
        sounds like: techno music, confident and cool music, drum and bass music, celebratory and joyful music
  verdict: 82/100 (arc 0.93, genre 0.26, mix 1.00, production 0.61, theory 1.00). Fix first: sounds more like 'afrobeats music' than 'amapiano music'; sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause
```

Amapiano grew out of South African house, jazz and kwaito, and the ears, which know afrobeats better, often hear
that instead; the log drum is what makes it amapiano. The hiss they hear is the shakers and open hats, which the
genre lives on.

Try:

- `part("logdrum", #{ style: "kick_follow" })` for a log drum that answers the kick.
- `mix("lead", #{ delay: 0.45 })` for a whistle that echoes further.

## Reggaeton: "Noche Eterna"

```js
song("reggaeton", 4);
title("Noche Eterna");
fx("snare", [
  #{ fx: "eq", bands: [#{ kind: "peak", freq: 2500, gain_db: 3, q: 1 }] },
  #{ fx: "compressor", threshold_db: -18, ratio: 4, attack_ms: 2, release_ms: 50, makeup_db: 2 },
]);
fx("lead", [
  #{ fx: "saturate", kind: "tape", drive_db: 4 },
  #{ fx: "delay", beats: 0.75, feedback: 0.3, pingpong: true, mix: 0.25 },
]);
```

Why:

- **What the genre already does.** The dembow: a kick on every beat and the snare after the first and before the
  third of each half bar; hats, a rim, shakers in the hooks; minor chords on plucks, piano or strings, a sub on the
  roots, a synth or voice-like lead; three hooks and a break.
- **A snare that snaps.** A 3 dB lift at 2.5 kHz and fast compression bring the dembow's snare to the front, where
  the rhythm lives.
- **A warmer lead.** Tape saturation, then a ping-pong delay of a dotted eighth.

```text
  ears: 'reggaeton music' z +4.0 (closest confusion 'afrobeats music' +4.3); production z +1.6 vs 'a cheap synthesized beep' +0.7; p_real 0.79
        sounds like: hip hop beat with heavy bass, trap beat, tech house music, house music
  verdict: 90/100 (arc 0.96, genre 0.68, mix 1.00, production 0.73, theory 1.00). Fix first: sounds more like 'afrobeats music' than 'reggaeton music'
```

The ears hear afrobeats and trap in it as well; the dembow is what tells reggaeton from both.

Try:

- `tempo(90)` for a slower, heavier sway.
- `part("chords", #{ preset: "keys" })` for piano chords in place of the plucks.

## Trance: "Afterglow"

```js
song("trance", 4);
title("Afterglow");
fx("chords", [
  #{ fx: "filter", mode: "lp", cutoff: sections(4000, #{ "break": [800, 6000], build: [6000, 12000], drop: [12000] }), res: 0.25 },
  #{ fx: "width", width: 1.6, mono_below_hz: 200 },
]);
```

Why:

- **What the genre already does.** Four on the floor with offbeat open hats and a clap on 2 and 4, a rolling bass, a
  euphoric minor progression under supersaw chords, a lead and an arpeggio; a long breakdown of pads, chords and the
  lead alone, a build that rolls on the clap, then the drop.
- **Chords that open with the song.** A lowpass on the chords rises from 800 Hz to 6 kHz across the breakdown, on to
  12 kHz across the build, and stays open in the drop; elsewhere it sits at 4 kHz. Above 200 Hz they are widened to
  1.6.

```text
  ears: 'trance music' z +3.8 (closest confusion 'techno music' +4.2); production z +1.0 vs 'a low quality recording with hiss' +0.7; p_real 0.74
        sounds like: techno music, drum and bass music, confident and cool music, chiptune video game music
  verdict: 87/100 (arc 0.94, genre 0.66, mix 1.00, production 0.57, theory 1.00). Fix first: sounds more like 'techno music' than 'trance music'; sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause
```

Trance and techno share the kick, the tempo and the hats, and the ears hear techno in it; the breakdown and the
chords that open across it are trance's own.

Try:

- `bars_per_chord(4)` for a slower, more hypnotic harmony.
- `mix("lead", #{ reverb: 0.5 })` for a lead that soars further back.

## Neo-soul: "Velvet Hours"

```js
song("neo_soul", 4);
title("Velvet Hours");
fx("keys", [
  #{ fx: "tremolo", beats: 0.5, depth: 0.2 },
  #{ fx: "chorus", rate_hz: 0.35, depth_ms: 3, delay_ms: 14, mix: 0.35 },
]);
bus("drums", [
  #{ fx: "compressor", threshold_db: -18, ratio: 3, attack_ms: 25, release_ms: 150, makeup_db: 3 },
  #{ fx: "saturate", kind: "tape", drive_db: 4, mix: 0.5 },
  #{ fx: "width", width: 1.0, mono_below_hz: 120 },
]);
```

Why:

- **What the genre already does.** A laid-back groove on a live kit, with ghost notes and a cross-stick in the
  verses and a ride in the hooks; rich 9th and 11th chords on Rhodes, a round bass that walks between roots, soft
  strings, a sung-sounding lead; verses, hooks, and a bridge of the keys alone.
- **A Rhodes that breathes.** A gentle tremolo in eighth notes and a slow chorus, the sound of a Rhodes' own vibrato
  through a chorus pedal.
- **Drums from a record.** The drums are compressed slowly, with a 25 ms attack that lets each hit keep its front,
  then warmed on tape; below 120 Hz they stay mono.

```text
  ears: 'neo soul music' z +2.8 (closest confusion 'jazz music' +3.1); production z +1.5 vs 'a recording made on a phone' +0.4; p_real 0.64
        sounds like: jazz music, funk music, hip hop beat with heavy bass, indie pop music
  verdict: 89/100 (arc 0.97, genre 0.55, mix 1.00, production 0.78, theory 1.00). Fix first: sounds more like 'jazz music' than 'neo soul music'
```

Neo-soul sits between jazz and R&B, and the ears hear both; the laid-back kit under the Rhodes is what makes it
neo-soul.

Try:

- `swing(0.6)` for a lazier lean.
- `part("keys", #{ preset: "tine_piano" })` for a brighter electric piano.
