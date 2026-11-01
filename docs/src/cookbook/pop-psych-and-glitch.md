# Dance-pop, neo-psychedelia, art rock, IDM, dub techno

Five more styles where the production carries the song: a pop hook that blooms in the chorus, guitars and organ
swirling through phasers and flangers, a piano that breaks up as the song grows, bells crushed and drowned in turn,
chords lost in their own echoes. As in [the breaks and disco recipes](breaks-and-disco.md), each starts from a
generated song and adds layers with [effect chains](../language/mixing.md#effect-chains), and runs without the sample
packs.

## Dance-pop: "Neon Promise"

```js
song("dance_pop", 1);
title("Neon Promise");
fx("hook", [
  #{ fx: "eq", bands: [#{ kind: "highpass", freq: 220 }, #{ kind: "peak", freq: 3000, gain_db: 2, q: 1 }] },
  #{ fx: "compressor", threshold_db: -20, ratio: 4, attack_ms: 3, release_ms: 70, makeup_db: 2 },
  #{ fx: "parallel", dry: 1, chains: [
    #{ amount: sections(0, #{ hook: 0.3 }), fx: [#{ fx: "reverb", decay: 3.5, predelay_ms: 40, mix: 1 }, #{ fx: "filter", mode: "hp", cutoff: 600 }] },
  ] },
]);
fx("chords", [#{ fx: "width", width: sections(1, #{ hook: 1.6 }), mono_below_hz: 200 }]);
```

Why:

- **What the genre already does.** A song form (verse, pre-chorus, chorus, three times, with a bridge), a pop beat
  in the verses (the kick on 1 and 3 and a push) and four on the floor in the choruses, a pumping synth bass,
  plucked chords and a hook in a singer's range.
- **A hook that sings.** The hook is treated like a vocal: high-passed, a presence lift at 3 kHz, compressed at 4:1.
  In the choruses only, a long, high-passed reverb blooms beside it, a third as loud, the way a chorus vocal opens
  up.
- **A wider chorus.** The chords widen to 1.6 in the choruses and stay as they are elsewhere, mono below 200 Hz.

```text
  ears: 'upbeat dance pop music' z +3.0 (closest confusion 'house music' +2.9); production z +1.2 vs 'a low quality recording with hiss' +0.7; p_real 0.72
        sounds like: celebratory and joyful music, playful and quirky music, confident and cool music, techno music
  verdict: 87/100 (arc 0.87, genre 0.66, mix 1.00, production 0.64, theory 1.00)
```

Dance-pop and house share the kick, the tempo and the sidechain; the verses' pop beat, the song form and a hook that
carries the song are what tell them apart.

Try:

- `prog("A", "vi IV I V")` for the minor-feeling version of the same four chords.
- `mix("hook", #{ level: 2 })` to push the hook in front, as a mix with a singer would.

## Neo-psychedelia: "Saffron Halo"

```js
song("neo_psychedelia", 3);
title("Saffron Halo");
fx("organ", [
  #{ fx: "tremolo", beats: 0.5, depth: 0.25 },
  #{ fx: "flanger", rate_hz: 0.08, depth_ms: 3, delay_ms: 4, feedback: 0.55, mix: sections(0.2, #{ "break": [0.2, 0.7] }) },
]);
fx("lead", [
  #{ fx: "tape", wow: 0.8, flutter: 0.35, drive_db: 3, lowpass_hz: 8000 },
  #{ fx: "delay", beats: 1.5, feedback: 0.5, damping: 0.6, pingpong: true, mix: sections(0.15, #{ outro: [0.15, 0.6] }) },
]);
```

Why:

- **What the genre already does.** A loose acoustic kit, a droning electric bass with a folded copy beside it,
  guitars through a phaser that swirls harder in the bridge, a chorus, and fuzz that grows in the choruses; an organ
  with tremolo and chorus; a flanged pad; tape wobble on the master.
- **An organ that takes off.** A slow flanger sweeps the organ, two parts dry to one wet, and across the bridge it
  rises to 70 %: the jet-plane swoosh of psychedelic records.
- **A melody that melts.** Heavy tape wow on the lead, and a dotted-half-note delay of its own whose mix rises across
  the outro, so the song ends in echoes.

```text
  ears: 'dreamy psychedelic pop music with phased guitars and organ' z +3.6 (closest confusion 'indie pop music' +3.0); production z +1.5 vs 'a distorted clipped recording' +0.5; p_real 0.57
        sounds like: indie pop music, funk music, rock music with electric guitars, upbeat corporate pop music
  verdict: 93/100 (arc 0.94, genre 0.89, mix 0.99, production 0.75, theory 1.00). Fix first: boxy or harsh mids (mid -7.4 dB, healthy up to -8)
```

The style reads as indie pop, rock or funk depending on the seed: a bass that walks pulls it toward funk, which is why
the generated bass holds or pulses instead.

Try:

- `key("A lydian")` for the floating, raised-fourth sound of the dreamier records.
- `fx("guitar", [#{ fx: "phaser", rate_hz: 0.15, stages: 8, depth: 0.9, feedback: 0.6, mix: 0.7 }])` for a slower,
  deeper swirl.

## Art rock: "Glass Cathedral"

```js
song("art_rock", 3);
title("Glass Cathedral");
fx("piano", [
  #{ fx: "parallel", dry: 1, chains: [
    #{ amount: sections(0, #{ verse: [0, 0.4], hook: 0.25 }), fx: [#{ fx: "bitcrush", bits: 6, downsample: 8 }, #{ fx: "filter", mode: "bp", cutoff: 1600, res: 0.35 }] },
  ] },
]);
fx("strings", [#{ fx: "filter", mode: "lp", cutoff: sections(1800, #{ build: [1800, 9000], hook: 9000 }), res: 0.1 }]);
```

Why:

- **What the genre already does.** The arc of the song: a quiet piano and a crushed, arpeggiated synth, then drums
  and bass, a rising section with a riser and toms, a loud climax, a fall to piano and strings, the climax again. The
  arpeggio is gated harder across the verses; the guitars' distorted copy rises from a fifth to most of the sound in
  the climaxes, and the drums are compressed in parallel harder there.
- **A piano that breaks up.** Across each verse a bitcrushed, band-passed copy of the piano fades in beside the clean
  one, up to 40 %, and stays at a quarter in the climaxes: the acoustic sound turns digital as the song grows.
- **Strings that open into the climax.** Their lowpass goes from 1.8 kHz to 9 kHz across the rise and stays open in
  the climaxes.

```text
  ears: 'progressive rock music with piano, strings and synthesizers' z +3.0 (closest confusion 'indie pop music' +1.6); production z +1.5 vs 'a recording made on a phone' +0.1; p_real 0.77
        sounds like: funk music, jazz music, playful and quirky music, celebratory and joyful music
  verdict: 96/100 (arc 0.98, genre 0.91, mix 0.99, production 0.83, theory 1.00). Fix first: muddy 250 Hz-1 kHz: too much pad / chord body (lowmid -3.6 dB, healthy up to -4)
```

The arc is nearly perfect because the form is written as one: the climaxes have everything, and the intro, the fall
and the outro have almost nothing.

Try:

- `tempo(76)` for a slower, heavier version; the arpeggio's eighths become a slow pulse.
- `section("fall", #{ layers: "piano" })` for a fall to the piano alone.

## IDM: "Prism Engine"

```js
song("idm", 2);
title("Prism Engine");
fx("bells", [
  #{ fx: "parallel", dry: sections(1, #{ drop: 0.6 }), chains: [
    #{ amount: sections(0, #{ groove: [0, 0.3], drop: 0.5 }), fx: [#{ fx: "bitcrush", bits: 5, downsample: 7 }, #{ fx: "filter", mode: "bp", cutoff: 2600, res: 0.45 }] },
    #{ amount: sections(0.1, #{ "break": [0.1, 0.6] }), fx: [#{ fx: "reverb", decay: 6, predelay_ms: 60, mix: 1 }] },
  ] },
]);
fx("bass", [#{ fx: "filter", mode: "ladder", cutoff: sections(600, #{ groove: [600, 1800], drop: 2400 }), res: 0.5, drive: 1.4 }]);
```

Why:

- **What the genre already does.** Two-bar broken beats on synthesized drums, with ratchets on the hats and snares,
  ghost notes, euclidean clicks and toms; the drums crushed in parallel, and in the peak compressed and folded too;
  pads gated harder as each groove goes on; a delay in sixteenth-note dots that feeds back across the break.
- **Bells in three states.** The bells run clean, crushed and drowned at once, in proportions that change by section:
  the crushed, band-passed copy rises across the grooves and takes half the sound in the peak, where the clean bells
  drop to 60 %; a six-second reverb swells across the break.
- **A bass that opens up.** A resonant ladder filter on the bass opens across the grooves and stays open in the peak.

```text
  ears: 'glitchy intelligent dance music with complex broken beats and bells' z +4.7 (closest confusion 'drum and bass music' +2.9); production z +1.3 vs 'a distorted clipped recording' +0.8; p_real 0.76
        sounds like: minimal electronic music, future bass music, drum and bass music, dubstep music
  verdict: 94/100 (arc 0.95, genre 1.00, mix 1.00, production 0.63, theory 1.00). Fix first: boxy or harsh mids (mid -8.0 dB, healthy up to -8)
```

IDM accepts "a cheap synthesized beep": synthesized sounds are what the style is made of, so they are not counted
against it.

Try:

- `tempo(136)` and `swing(0.5)` for a faster, more rigid machine.
- `part("bells", #{ seed: 11 })` for another melody through the same three states.

## Dub techno: "Harbour Fog"

```js
song("dub_techno", 3);
title("Harbour Fog");
send_fx(#{
  delay: #{ fx: "delay", beats: 0.75, feedback: sections(0.45, #{ "break": [0.45, 0.75], drop: [0.5, 0.62] }), damping: 0.75, pingpong: true, mix: 1 },
  reverb: #{ fx: "reverb", decay: 3.2, damping: 0.65, predelay_ms: 35, width: 1.4, mix: 1, lowcut: 300 },
});
fx("rim", [#{ fx: "filter", mode: "bp", cutoff: sections(2500, #{ drop: [2500, 4500] }), res: 0.4 }]);
```

Why:

- **What the genre already does.** A soft, steady kick, rimshots and quiet hats, one or two minor chords stabbed and
  sent half into the delay and half into the reverb, through a lowpass that breathes with the arrangement (it opens
  across the intro and the groove, closes across the break, opens again in the deep section); a deep sub; the music
  bus dips in the break.
- **Echoes that swell and settle.** The delay the sends feed is darker than the genre's, and its feedback climbs to
  0.75 across the break, so the chord echoes pile up into a cloud, then settles between 0.5 and 0.62 in the deep
  section.
- **A rimshot that brightens.** A resonant band-pass on the rim lane rises from 2.5 to 4.5 kHz across the deep
  section, and its delay send carries the change into the echoes.

```text
  ears: 'dub techno music with echoing chords' z +3.5 (closest confusion 'techno music' +3.7); production z +1.5 vs 'a low quality recording with hiss' +0.9; p_real 0.75
        sounds like: techno music, drum and bass music, house music, tech house music
  verdict: 87/100 (arc 0.83, genre 0.71, mix 1.00, production 0.66, theory 1.00). Fix first: sounds more like 'techno music' than 'dub techno music with echoing chords'
```

Dub techno is techno, and the ears often say so; what makes it dub is the space around the chords, which the delay's
feedback and the long reverb give it.

Try:

- `bars_per_chord(8)` and `prog("A", "i7 i7 iv7 i7")` for a slower, more hypnotic harmony.
- `mix("chords", #{ delay: 0.8 })` to send even more of the chords into the echoes.
