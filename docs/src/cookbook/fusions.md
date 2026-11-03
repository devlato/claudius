# Fusions

Two or three genres joined by `+` make one: `song("art_rock+idm", 4)`, `claudius make --genre trip_hop+idm`, or
**Fused with** in the studio's New song dialog. The first genre leads: its tempo, keys and harmony, its form, its
parts and its mix, and what the ears listen for. The genre after it plays the drums, with its kits and its drums
bus, where the first genre's drums played. It lends its first sound and its writing styles to the parts of the same
role, and brings in now and then the parts the first genre has nothing like, and a quarter of its progressions.
[Fusing genres](../learn/producers/genres-and-evolution.md#fusing-genres) has the details.

A fusion is a starting point: the ears hear it as whatever it now sounds like, which is the point of fusing.

## Art rock with IDM's drums: "Paper Satellites"

```js
song("art_rock+idm", 4);
title("Paper Satellites");
fx("piano", [#{ fx: "parallel", dry: 1, chains: [
  #{ amount: sections(0, #{ hook: 0.3 }), fx: [#{ fx: "bitcrush", bits: 6, downsample: 6 }, #{ fx: "filter", mode: "bp", cutoff: 1800, res: 0.3 }] },
] }]);
```

Why:

- **Art rock's song, IDM's machine.** The arc of art rock (a quiet piano and arpeggio, a rise, loud climaxes, a fall)
  over IDM's synthesized two-bar broken beats with ratchets, clicks and toms, crushed in parallel on IDM's drums bus.
- **Sounds from both.** The arpeggio and the lead may pick up IDM's bells and arpeggio sounds; the piano, strings,
  the clean guitar and the two distorted power-chord guitars of the climaxes stay art rock's.
- **A piano that breaks up in the climaxes.** A crushed, band-passed copy beside the piano at 30 % in the hooks.

```text
  ears: 'progressive rock music with piano, strings and synthesizers' z +3.6 (closest confusion 'chiptune video game music' +2.9); production z +1.0 vs 'a recording made on a phone' +0.7; p_real 0.66
        sounds like: chiptune video game music, ambient synth pad music, uplifting and hopeful music, inspirational corporate background music
  verdict: 82/100 (arc 0.97, genre 0.93, mix 1.00, production 0.59, theory 0.99). Fix first: sounds close to stock background music ('chiptune video game music'): too clean and polite for 'progressive rock music with piano, strings and synthesizers'
```

The machine drums and IDM's bells pull the song toward chiptune, which the critic counts against it as stock music;
the arc stays nearly perfect, because the form is art rock's. Fusing the other way, `idm+art_rock`, keeps IDM's song
over art rock's acoustic kit.

## Dub techno with trip-hop's drums: "Low Tide Relay"

```js
song("dub_techno+trip_hop", 2);
title("Low Tide Relay");
```

Why:

- **Echoes over a break.** Dub techno's chords drowned in feedback and reverb, its filter breathing with the
  arrangement, its sub, over trip-hop's swung two-bar break and ghost snares on an acoustic kit, compressed and
  crushed in parallel on trip-hop's drums bus, at dub techno's 124 BPM.
- **No new parts to write.** One line makes the whole song: what each genre is good at, taken from each.

```text
  ears: 'dub techno music with echoing chords' z +2.9 (closest confusion 'house music' +3.4); production z +1.5 vs 'a distorted clipped recording' +0.8; p_real 0.70
        sounds like: trap beat, house music, hip hop beat with heavy bass, tech house music
  verdict: 85/100 (arc 0.88, genre 0.51, mix 1.00, production 0.68, theory 1.00). Fix first: sounds more like 'house music' than 'dub techno music with echoing chords'; a thin low end: bass or kick too quiet (low -7.2 dB, healthy from -7)
```

A heavy break at 124 BPM is closer to house and hip hop than to dub techno's soft kick, and the ears say so.
`tempo(96)` takes it back toward trip-hop, with the echoes kept.

## Big beat with electro house's drums: "Static Riot"

```js
song("big_beat+electro_house", 3);
title("Static Riot");
```

Why:

- **Big beat's riff and acid, a dance-floor kick.** Big beat's acid line, stabs and riff, its form of builds and
  drops, over electro house's four-on-the-floor, claps and offbeat hats.
- **A bass between the two.** The bass keeps big beat's sounds and register and picks up electro house's offbeat
  and syncopated styles.

```text
  ears: 'big beat breakbeat music with distorted drums' z +2.8 (closest confusion 'techno music' +4.0); production z +1.4 vs 'a low quality recording with hiss' +0.7; p_real 0.65
        sounds like: techno music, drum and bass music, confident and cool music, tech house music
  verdict: 84/100 (arc 0.93, genre 0.36, mix 1.00, production 0.68, theory 0.99). Fix first: sounds more like 'techno music' than 'big beat breakbeat music with distorted drums'; 7% of the lead is outside E minor
```

Without its breakbeat, big beat reads as techno: the breaks are most of what the style is. The fusion the other way
round, `electro_house+big_beat`, keeps electro house's form and puts big beat's crushed breaks under it.

Try:

- `song("trip_hop+idm", 3)` for trip-hop's dark chords and strings over IDM's broken machine beats.
- `song("nu_disco+filter_house", 7)` for a disco band over filter house's drums, squashed on its drums bus.
- Three genres: `song("house+dnb+lofi", 2)` is house's song with lo-fi's drums, and drum and bass's sounds, styles
  and parts lent to it on the way: the last genre plays the drums.
