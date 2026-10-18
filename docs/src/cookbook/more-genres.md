# Drum and bass, synthwave, future bass

Three styles that lean on synthesizers: liquid drum and bass with a rolling breakbeat under pads, synthwave's
eighties arpeggios and gated drums, and future bass's pumping supersaw chords. All three use sampled drums, so they
need the default sample packs (`claudius setup`).

## Drum and bass: "Liquid Orbit"

```js packs
blank("dnb");
title("Liquid Orbit");
tempo(174); key("F minor"); swing(0.5);
prog("A", "Fm Db Ab Eb");
prog("B", "Db Eb Fm Fm");
bars_per_chord(2);

track("kick",  s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~").bank("acoustic"));
track("snare", stack([s("~ sd ~ sd"), s("~ ~ ~ ~ ~ ~ ~ sd:1 ~ ~ sd:1 ~ ~ ~ ~ ~").gain(0.35)]).bank("acoustic").room(0.15));
track("hat",   s("hh*8").bank("acoustic").gain("[0.7 0.45]*4").pan(0.15));
track("ride",  s("[~ rd]*4").bank("acoustic").gain(0.45).pan(-0.2));

track("bass",  note("<f1!2 db1!2 ab1!2 eb1!2>").s("reese").legato(1.0));
track("pad",   chord("prog").voicing(48, 72).s("pad").role("pad").room(0.4).level(-2));
track("arp",   chord("prog").struct("x x").arp("updown").voicing(60, 76).s("pluck").role("arp").delay(0.3).room(0.3));
track("keys",  chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(55, 72).s("rhodes").room(0.3).level(-2));

form([
  sec("intro", 16, "pad arp").filter_in(),
  sec("build", 8, "hat ride pad arp keys").gap(),
  sec("drop", 32, "kick snare hat ride bass pad arp keys").impact(),
  sec("break", 16, "pad arp keys"),
  sec("drop", 16, "kick snare hat ride bass pad arp keys").impact(),
  sec("outro", 8, "pad arp"),
]);
```

Why:

- **The two-step.** At 174 BPM, a kick on beat 1 and on the *and* of 3 (`bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd`), with the snare on 2
  and 4: the basic drum and bass rhythm. Two ghost snares (variation `sd:1`, at a third of the level) in a separate
  layer of the `stack()` give it the shuffle of a chopped breakbeat; a gain pattern on the whole snare would have
  turned down the backbeat with them.
- **A recorded kit.** The acoustic kit sounds like a sampled break, which is where the style comes from. The ride on
  the offbeats keeps the top end moving at half the hats' rate.
- **Reese bass.** Two detuned saws a bar at a time, on the chord roots two bars each, so the bass is one slow,
  growling movement under fast drums.
- **Three harmonic layers at three speeds**: a pad holding the chords, a Rhodes on a slow syncopated rhythm, and a
  pluck arpeggio (`updown` over two events a bar) running eighths with a long delay.
- **Long sections.** Drum and bass phrases are long; the first drop is 32 bars, and the song is 96 bars, 2:12 at
  174 BPM. Anything under 90 seconds would cost form points.

```text
  ears: 'drum and bass music' z +3.4 (closest confusion 'techno music' +2.7); production z +1.1 vs 'a low quality recording with hiss' +0.3; p_real 0.76
        sounds like: drum and bass music, energetic and driving music, synthwave music, inspirational corporate background music
  verdict: 93/100 (arc 0.96, genre 0.88, mix 0.99, production 0.72, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -26.5 dB, healthy from -26)
```

The best score in the cookbook. The energy arc is nearly perfect because the sections are clearly layered: the intro
and break have no drums at all, the drops have everything.

Try:

- `.s("wobble")` on the bass for a heavier, neurofunk movement (its filter LFO is tempo-synced).
- Busier hats: `hh*16` with `.gain("[0.7 0.3 0.5 0.3]*4")` for a rolling sixteenth feel.
- Replace the arp with a lead melody in F minor, with chord tones on beats 1 and 3 of each bar.

## Synthwave: "Neon Harbor"

```js packs
blank("synthwave");
title("Neon Harbor");
tempo(100); key("A minor"); swing(0.5);
prog("A", "Am F C G");
prog("B", "F G Am Am");
bars_per_chord(1);

track("kick",  s("bd ~ bd ~").bank("tr707"));
track("snare", s("~ sd ~ sd").bank("tr707").room(0.45));
track("hat",   s("hh*16").bank("tr707").gain("[0.6 0.3 0.45 0.3]*4").pan(0.2));

track("bass",  note("<[a1 a2]*4 [f1 f2]*4 [c2 c3]*4 [g1 g2]*4>").s("saw_bass").legato(0.8));
track("arp",   chord("prog").struct("x*4").arp("updown").voicing(57, 76).s("pluck").role("arp").delay(0.25).pan(0.15));
track("pad",   chord("prog").voicing(52, 76).s("strings").role("pad").room(0.45).level(-2));
track("lead",  n("<[4 ~ ~ 4 2 ~ 0 ~] [0 ~ ~ 2 5 ~ 4 ~] [4 ~ ~ 4 6 ~ 4 ~] [6 ~ ~ 4 3 ~ ~ ~]>")
                 .octave(4).s("square_lead").room(0.3).delay(0.2));

form([
  sec("intro", 8, "pad arp").filter_in(),
  sec("verse", 16, "kick snare hat bass arp pad"),
  sec("chorus", 16, "kick snare hat bass arp pad lead").impact(),
  sec("break", 8, "pad lead").prog("A"),
  sec("chorus", 16, "kick snare hat bass arp pad lead").impact(),
  sec("outro", 8, "pad arp"),
]);
```

Why:

- **The 707.** Roland's TR-707 is the drum machine of eighties pop. A kick on 1 and 3, a snare on 2 and 4 with a lot
  of reverb: the big, simple backbeat of the decade.
- **Octave bass.** Each bar alternates the root and its octave in eighths (`[a1 a2]*4`), the driving bass of synth
  pop, on a buzzy `saw_bass`.
- **Arpeggios in sixteenths.** `chord("prog").struct("x*4").arp("updown")` gives four notes per beat from a triad (up
  and down without repeating the ends is four steps), so the arpeggio is straight sixteenths and always on the right
  chord.
- **String machine and square lead.** `strings` is the synth preset, slow attack and vibrato, more Juno than
  orchestra; `square_lead` is a mono square wave with delayed vibrato. Both are synthesized on purpose: this is the
  one style where "computer-generated" is the aesthetic.
- **A pop form.** Verse, chorus, a short break with only pad and lead, and the chorus again: synthwave is song-shaped
  rather than DJ-shaped.

```text
  ears: 'synthwave music' z +3.5 (closest confusion 'upbeat corporate pop music' +3.0); production z +0.5 vs 'a cheap synthesized beep' +0.3; p_real 0.61
        sounds like: synthwave music, inspirational corporate background music, uplifting and hopeful music, upbeat corporate pop music
  verdict: 90/100 (arc 0.95, genre 0.87, mix 1.00, production 0.55, theory 1.00). Fix first: sounds about as much like a cheap synth as a professional mix: sampled instruments in place of synth patches usually read better
```

The ears' worst flaw is "a cheap synthesized beep", which for synthwave is half the point. A sampled `string_ensemble`
pad instead of the `strings` synth would raise production and make it less eighties.

Try:

- `tempo(86)` makes it outrun-slow; `tempo(118)` makes it italo disco.
- A tom fill in the last beat of every fourth bar, with a big reverb:
  `track("toms", s("<~ ~ ~ [~ ~ ~ [lt mt ht ht]]>").bank("tr707").room(0.5))`, added to the choruses' layers.
- `key("F# minor")` and render again: the chords, the arpeggio and the lead follow, because they are written with
  `prog` and `n()`. The bass is written in note names and stays in A; move it with `.transpose(-3)`.

## Future bass: "Golden Bloom"

```js packs
blank("future_bass");
title("Golden Bloom");
tempo(150); key("D major"); swing(0.5);
prog("A", "Dmaj9 A Bm9 Gmaj9");
prog("B", "Gmaj9 A Bm9 Bm9");
bars_per_chord(1);

track("kick",  s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~").bank("tr909"));
track("clap",  s("~ ~ [cp, sd] ~").bank("tr808").room(0.25));
track("hat",   s("[~ hh]*4, ~ ~ ~ [~ hh hh hh]").bank("tr808").gain(0.6).pan(0.2).level(-3));

track("bass",   note("<d2 a1 b1 g1>").struct("x ~ ~ ~ ~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~").s("saw_bass").legato(1.0).level(-3));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~ ~ ~ x ~ x ~ ~ ~").voicing(55, 79).s("supersaw").legato(0.9).duck(0.85).room(0.25).level(2));
track("pad",    chord("prog").voicing(50, 74).s("pad").role("pad").room(0.4).level(-3));
track("lead",   n("<[4 ~ 2 ~ 4 ~ 5 4] [6 ~ 4 ~ 6 ~ 7 6] [5 ~ 4 ~ 2 ~ 4 ~] [3 ~ 2 ~ 4 ~ ~ ~]>")
                  .octave(4).s("pluck").room(0.3).delay(0.25));

form([
  sec("intro", 8, "pad lead").filter_in(),
  sec("verse", 16, "kick clap hat bass pad lead"),
  sec("build", 8, "clap hat chords pad").gap(),
  sec("drop", 16, "kick clap hat bass chords lead").impact(),
  sec("break", 8, "pad lead"),
  sec("build", 8, "clap hat chords pad").gap(),
  sec("drop", 16, "kick clap hat bass chords lead pad").impact(),
  sec("outro", 8, "pad lead"),
]);
```

Why:

- **The pump.** The supersaw chords duck under the kick at depth 0.85: they nearly vanish on every kick and swell back,
  which is the future-bass sound. The kick only plays twice a bar (beat 1 and the *and* of 3), so the swells are long.
- **Syncopated chords.** `x ~ ~ x ~ ~ x ~ ~ ~ x ~ x ~ ~ ~` is a sixteenth-note rhythm built from threes and twos, the
  chopped chord rhythm of the style. The chords only enter in the builds and drops; the verse keeps them back.
- **Major ninths.** `Dmaj9 A Bm9 Gmaj9` is the pop `I V vi IV` with ninths, bright and bittersweet.
- **Half-time drums at 150 BPM**, with the layered clap and snare on beat 3, as in trap, and a hat skip at the end of
  each bar (`[~ hh hh hh]`).
- **A saw bass on the kick**, not a sub: the first version used `sub_bass` and the critic found too much sub and a
  thin low end at once.

```text
  ears: 'future bass music' z +2.4 (closest confusion 'dubstep music' +2.8); production z +1.1 vs 'a low quality recording with hiss' +0.3; p_real 0.53
        sounds like: synthwave music, hip hop beat with heavy bass, drum and bass music, techno music
  verdict: 85/100 (arc 0.96, genre 0.41, mix 0.96, production 0.70, theory 1.00). Fix first: fizzy top end (air -14.8 dB, healthy up to -17); sounds more like 'dubstep music' than 'future bass music'
```

Future bass is the style the ears are least sure of: it reads close to dubstep and synthwave, which share its
sounds. The air band is high because a supersaw at the front of the mix is bright; `.lpf(9000)` on the chords
tames it. An earlier version with a `chip` arpeggio read as "chiptune video game music", and taking it out helped.

Try:

- `duck(0.6)` on the chords for a subtler pump, or `0.95` for an extreme one.
- Vocal chops in the drops: `track("vox", note("<[~ d3 ~ a2] [~ c#3 ~ a2]>").s("voice").role("vocal").legato(0.5))`,
  sung vowels on chord notes.
- `bars_per_chord(2)` and a slower lead for a more anthemic, half-speed drop.
