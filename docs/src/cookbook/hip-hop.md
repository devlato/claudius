# Trap and lofi

Two descendants of hip-hop at opposite ends: trap is fast hi-hats, a booming 808 and dark minor melodies at
140 BPM in half time; lofi is a dusty, swung loop of jazz chords at 84 BPM that sounds like a worn record. Both
recipes need the default sample packs (`claudius setup`).

## Trap: "Paper Satellite"

```js packs
blank("trap");
title("Paper Satellite");
tempo(140); key("C harmonic minor"); swing(0.5);
prog("A", "Cm Ab Fm G");
prog("B", "Ab G Cm Cm");
bars_per_chord(2);

track("kick",  s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~").bank("tr808"));
track("clap",  s("~ ~ [cp, sd] ~").bank("tr808").room(0.2));
track("hat",   s("<[hh*2 hh*2 hh*2 hh*2] [hh*2 hh*2 hh*2 hh*6] [hh*2 hh*2 hh*2 hh*2] [hh*2 hh*4 hh*2 hh*8]>")
                 .bank("tr808").gain("[0.9 0.6]*4").pan(0.12));
track("ohat",  s("~ ~ ~ oh ~ ~ ~ ~").bank("tr808").gain(0.5).pan(-0.2));
track("808",   note("<c2!2 ab1!2 f1!2 g1!2>").struct("x ~ ~ ~ ~ ~ ~ x ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1.0).duck(0));

track("keys",  chord("prog").voicing(55, 72).s("piano").room(0.3).level(-1));
track("pad",   chord("prog").voicing(48, 70).s("choir").role("pad").room(0.4).level(-4));
track("lead",  n("<[7 ~ 6 7 4 ~ ~ ~] [~ ~ 2 3 4 ~ 2 ~] [7 ~ 6 7 5 ~ ~ ~] [~ ~ 4 5 7 ~ 5 ~] [5 ~ 4 5 3 ~ ~ ~] [~ ~ 2 3 5 ~ 3 ~] [6 ~ 4 6 8 ~ ~ ~] [4 ~ ~ ~ 6 ~ ~ ~]>")
                 .octave(4).s("bell").room(0.3).delay(0.25));

form([
  sec("intro", 8, "keys pad lead").filter_in(),
  sec("hook", 16, "kick clap hat ohat 808 keys pad lead").impact(),
  sec("verse", 16, "kick clap hat 808 keys"),
  sec("hook", 16, "kick clap hat ohat 808 keys pad lead").impact(),
  sec("outro", 8, "keys pad lead"),
]);
```

Why:

- **Half time.** At 140 BPM the clap lands on beat 3 only (`~ ~ [cp, sd] ~`), so the groove feels like 70 BPM while
  the hats run at full speed. The clap is layered with the snare in one step (`[cp, sd]`), a standard trap trick for
  a thicker backbeat.
- **Hi-hat rolls.** The hats are written beat by beat, with a sixteenth-triplet roll (`hh*6`) on the last beat of
  bar 2, and sixteenths on beat 2 and thirty-seconds on beat 4 of bar 4. The `<...>` cycles four bars, so the rolls
  come back at the same place in every phrase, and the accents (`[0.9 0.6]*4`) keep the eighths from sounding like a
  machine gun.
- **The 808 is the bass.** It shares the kick's rhythm exactly (`x ~ ~ ~ ~ ~ ~ x ~ ~ x`), holds each note until the
  next (`legato(1.0)`), and follows the chord roots two bars at a time (`<c2!2 ab1!2 f1!2 g1!2>`). `duck(0)` turns off
  the sidechain that pattern tracks get by default: ducking an 808 under a kick on the same beat would cut off its
  attack.
- **Harmonic minor.** C harmonic minor gives G major as the fifth chord (`Cm Ab Fm G`), with a B that pulls hard back
  to C: the dark, classical tension of a lot of trap. The bell melody uses that B on the G chord.
- **Choir and piano.** A sustained piano and a low choir pad are the cinematic bed under the drums; the verse drops
  the choir, the open hat and the lead so the hook has somewhere to return to.

```text
  ears: 'trap beat' z +3.4 (closest confusion 'hip hop beat with heavy bass' +3.2); production z +1.0 vs 'a recording made on a phone' +0.8; p_real 0.39
        sounds like: trap beat, hip hop beat with heavy bass, lo-fi hip hop beat, a success chime
  verdict: 88/100 (arc 0.97, genre 0.79, mix 0.94, production 0.56, theory 1.00). Fix first: production reads close to 'a recording made on a phone' (z +0.8) vs professionally mixed (z +1.0); too much sub (boomy): lower the bass / kick, or highpass the music bus (sub -1.8 dB, healthy up to -4); a thin low end: bass or kick too quiet (low -7.7 dB, healthy from -7)
```

The low end is the 808's: a lot of sub, not much above it. That is trap's sound and the critic's taste disagreeing.
Drive on the 808 (`.distort(6)`) adds harmonics in the 60–250 Hz band, where small speakers can hear it, without
changing its notes; `.level(-2)` on it trims the sub.

Try:

- Glide: overlap the 808 notes (`legato(1.2)`) and they slide into each other, since the `808` preset has glide.
- A sparser verse: `section("verse", #{ layers: "kick clap 808 keys" })` takes the hats out too.
- Swap the bell for `pizzicato` or `flute` (sampled), two other trap lead sounds.

## Lofi: "Rain on the Avenue"

```js packs
blank("lofi");
title("Rain on the Avenue");
tempo(84); key("F major"); swing(0.6);
prog("A", "Gm9 C9 Fmaj9 Dm9");
prog("B", "Bbmaj7 C9 Am7 Dm9");
bars_per_chord(1);

track("kick",  s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~").bank("tr808").gain(0.9));
track("snare", s("~ sd ~ sd").bank("acoustic").gain(0.8).room(0.15));
track("hat",   s("hh*8?0.1").bank("tr808").gain("[0.6 0.35]*4").pan(0.2).level(4));
track("ohat",  s("~ ~ ~ ~ ~ ~ ~ oh").bank("tr808").gain(0.5).pan(-0.2).level(2));

track("bass",  note("<g1 c2 f1 d2>").struct("x ~ ~ x ~ x ~ ~").s("upright_bass").legato(0.9));
track("keys",  chord("prog").struct("x ~ ~ ~ ~ x ~ ~").voicing(50, 72).s("rhodes").room(0.3).late(0.02));
track("lead",  n("<[2 ~ ~ 1 2 ~ 4 ~] [~ ~ 4 3 1 ~ ~ ~] [2 ~ ~ 4 6 ~ 4 ~] [5 ~ 4 2 0 ~ ~ ~]>")
                 .octave(4).s("kalimba").room(0.35).delay(0.2).level(-1));

form([
  sec("intro", 4, "keys").filter_in(),
  sec("verse", 16, "kick snare hat bass keys"),
  sec("hook", 16, "kick snare hat ohat bass keys lead"),
  sec("verse", 16, "kick snare hat ohat bass keys lead"),
  sec("outro", 8, "keys lead"),
]);
```

Why:

- **The ii–V–I.** `Gm9 C9 Fmaj9 Dm9` is the jazz cadence in F major with ninths on every chord, then the relative
  minor to loop back. Lofi borrows its harmony from jazz; the ninths are what make it sound wistful rather than
  plain.
- **Heavy swing and lazy keys.** Swing 0.6 drags every off-beat sixteenth, and `.late(0.02)` puts the Rhodes a hair
  behind the beat, like a player leaning back. The keys play on beat 1 and the *and* of 3 only.
- **Mixed kits.** An 808 kick (mono, deep) with the acoustic kit's snare (a real drum in a room), and 808 hats with
  `?0.1` dropping one hit in ten so the pattern never repeats exactly. The kick plays on beat 1, on the last
  sixteenth of beat 2 and on the *and* of 3: a boom-bap pattern that pushes into the second half of the bar.
- **Upright bass** on the chord roots with long notes, and a **kalimba** melody: acoustic, plucked, warm.
- **The genre does the rest.** `blank("lofi")` brings lofi's mix: a bitcrusher and a lowpass on the drums bus, tape
  wow and flutter, vinyl crackle and hiss on the master, and a quiet −12.5 LUFS target. That darkening is why the hats
  are 4 dB up: without it the critic finds no air above 8 kHz.

```text
  ears: 'lo-fi hip hop beat' z +4.0 (closest confusion 'hip hop beat with heavy bass' +3.3); production z +2.0 vs 'a low quality recording with hiss' +1.8; p_real 0.40
        sounds like: lo-fi hip hop beat, hip hop beat with heavy bass, jazz music, minimal electronic music
  verdict: 89/100 (arc 0.90, genre 0.92, mix 0.97, production 0.55, theory 1.00). Fix first: production reads close to 'a low quality recording with hiss' (z +1.8) vs professionally mixed (z +2.0); recessed mids: the lead and chords are buried (mid -19.0 dB, healthy from -18); too much sub (boomy): lower the bass / kick, or highpass the music bus (sub -3.2 dB, healthy up to -4)
```

The ears hear "a low quality recording with hiss" because there is hiss: the vinyl on the master is the style. The
first version of this recipe used the acoustic kit's kick and a vibraphone lead, and the critic read it as jazz
(`reads as 'jazz music' (z +3.5) more than 'lo-fi hip hop beat' (z +3.1)`) and found the low end not mono (the
acoustic kick is a stereo room recording). The 808 kick and the kalimba fixed both.

Try:

- `.s("piano")` on the keys and `.lpf(3000)` for a muffled, sampled-from-a-record piano.
- `swing(0.66)` for a full triplet shuffle.
- A B section: `sec("hook", 16, ...).prog("B")` plays the `Bbmaj7 C9 Am7 Dm9` progression, but write a second lead
  for it, since this one fits `A`.
