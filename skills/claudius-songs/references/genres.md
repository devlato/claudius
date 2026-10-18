# Starting points for the eight genres

`new_song` (or `song("<genre>", seed)`) writes a complete song in any of these, and is the quickest start. The
sketches below are for when you build from patterns: each is a skeleton to grow, not a finished song. They write the
harmony with `prog()` and let the bass and chords follow it (`chord("prog")`), so that the parts agree with each
other and with what the critic checks the notes against. `catalog`
lists the genres with their tempo ranges, and user genres when the person has added some.

## house (120-126 BPM)

Four on the floor, offbeat hats, claps on 2 and 4, soulful 7th chords, a bouncing offbeat bass.

```js
blank("house");
tempo(124);
key("A minor");
prog("A", "i7 VImaj7 IIImaj7 VII7");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("bass", chord("prog").root().struct("~ x ~ x ~ x ~ x").s("pluck_bass"));
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing().s("epiano").room(0.25));
```

## deep_house (118-124 BPM)

Softer kick, shuffled hats and shakers, minor 9th chords on electric piano or organ, a round syncopated bass, long
warm reverbs. `swing(0.56)` helps.

```js
blank("deep_house");
tempo(121);
swing(0.56);
prog("A", "i9 iv9");
track("drums", s("bd*4, [~ hh]*4, sh*16").gain("1 0.8"));
track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("sub_bass").legato(0.9));
track("keys", chord("prog").struct("~ x ~ ~ ~ x ~ ~").voicing().s("organ").room(0.45));
```

## techno (126-134 BPM)

A driving, distorted kick, offbeat hats and rides, a rolling bass between the kicks, one- or two-chord stabs in minor
or phrygian, long builds; an acid line is optional.

```js
blank("techno");
tempo(130);
key("F minor");
prog("A", "i i i bII");
track("kick", s("bd*4").distort(4));
track("hats", s("[~ hh]*4, rd*8").gain(0.6));
track("bass", chord("prog").root().octave(1).struct("[~ x x x]*4").s("acid").lpf(900));
track("stab", chord("prog").struct("~ ~ x ~").s("stab").delay(0.3));
```

## trap (130-150 BPM)

Half-time feel with the snare on beat 3, syncopated kicks, 808s gliding on the kicks, rolling hi-hats, dark minor
keys and bells.

```js
blank("trap");
tempo(140);
key("C minor");
prog("A", "i i VI VII");
track("drums", s("[bd ~ ~ bd] [~ ~ bd ~], ~ ~ sd ~, hh*8").gain("1 0.9"));
track("rolls", s("~ ~ ~ [hh*3]").gain(0.5));
track("808", chord("prog").root().octave(1).struct("x ~ ~ x ~ ~ x ~").s("808"));
track("bells", n("<[0 ~ 3 ~] [5 ~ 3 2]>").scale("C:minor").octave(5).s("bell").level(-6));
```

## lofi (70-90 BPM)

A lazy swung boom-bap beat, jazzy 7th and 9th chords on electric piano, a walking or held bass, tape wobble and a
dusty, lowpassed sound.

```js
blank("lofi");
tempo(82);
swing(0.6);
key("D minor");
prog("A", "i9 iv9 VImaj7 v7");
track("drums", s("bd ~ ~ bd, ~ sd, hh*8?0.2").lpf(6000));
track("keys", chord("prog").struct("x ~ ~ x ~ x ~ ~").voicing(50, 72).s("epiano").room(0.3).late(0.02));
track("bass", chord("prog").root().struct("x ~ ~ ~ x ~ ~ ~").s("sub_bass"));
```

## dnb (170-176 BPM)

A two-step breakbeat (kick on 1 and the and-of-3, snares on 2 and 4 with ghost notes), a growling reese bass,
atmospheric pads, long intro and breakdown.

```js
blank("dnb");
tempo(174);
key("E minor");
prog("A", "i VI");
track("drums", s("bd ~ ~ ~ ~ ~ bd ~ , ~ ~ sd ~ ~ [~ sd?0.4] sd ~, hh*16").gain("1 0.85"));
track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ ~ ~").s("reese").legato(1.5));
track("pad", chord("prog").voicing().s("pad").room(0.5).level(-4));
```

## synthwave (85-118 BPM)

Straight 80s drums with a big reverberant snare on 2 and 4, a driving eighth-note saw bass, arpeggios, lush pads,
a chorused square or supersaw lead over minor progressions.

```js
blank("synthwave");
tempo(100);
key("A minor");
prog("A", "i VI III VII");
track("drums", s("bd ~ sd ~, hh*8").gain("1 0.8"));
track("bass", chord("prog").root().struct("x*8").s("saw_bass"));
track("arp", chord("prog").struct("x*16").arp("up").s("pluck").level(-5));
track("pad", chord("prog").voicing().s("pad").room(0.4).level(-6));
```

## future_bass (140-160 BPM)

Half-time drums, lush 7th and 9th supersaw chords chopped in syncopated rhythms and pumped by the sidechain, a bright
plucky hook, builds into drops.

```js
blank("future_bass");
tempo(150);
key("F major");
prog("A", "IVmaj7 V7 iii7 vi7");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~, ~ ~ ~ ~ sd ~ ~ ~, hh*8").gain("1 0.7"));
track("chords", chord("prog").struct("x ~ x x ~ x ~ x").voicing().s("supersaw").duck(0.8));
track("hook", n("<[0 2 4 2] [4 5 4 ~]>").octave(5).s("pluck").delay(0.25));
```
