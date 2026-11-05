# Starting points for the thirty-six genres

Two or three genres fuse with `+`: `song("trip_hop+idm", 3)` (or `new_song` with genre `trip_hop+idm`) is trip-hop's
song, harmony, parts and mix with IDM's drums, and IDM's sounds lent to the parts. The first genre leads, the last
plays the drums. Fuse when the person describes a mix of styles ("trip-hop but with glitchy drums", "disco with
house drums"): it is one call, and the result is a song like any other to edit.

`new_song` (or `song("<genre>", seed)`) writes a complete song in any of these, and is the quickest start. The
sketches below are for when you build from patterns: each is a skeleton to grow, not a finished song. They write the
harmony with `prog()` and let the bass and chords follow it (`chord("prog")`), so that the parts agree with each
other and with what the critic checks the notes against. `catalog`
lists the genres with their tempo ranges, and user genres when the person has added some.

Any of them can be built on samples from free libraries: `new_song` with `sampled: true`, or with `sample_search`
naming a kind of record ("70s soul", "bossa nova guitar", "film score"; not the genre's own name, which finds other
people's tracks rather than records to sample). The song takes a phrase's key, tempo and chords, played faster or
slower by whole semitones as a sampler would, in place of the genre's chords or keys, with a break and a texture from
what the genre's producers dig for. Another passage of the same record plays as `sample2` in some sections, on its
own chords (progression A2): the louder of the two where the song lifts, the other elsewhere. To keep one passage all
through, give those sections `sample` in place of `sample2` and progression A, so that the parts play the phrase's
chords under it. Where either passage plays, the lead and the arpeggio leave the section's layers, so that the
record carries the melody: put them back in a section's layers to play one over it. The song's history names the
move its phrase was treated with (filtered until it lifts, darkened, a
telephone's band in the intro and the breaks, overloaded, smashed, crushed): the effects of it are in the `sample`
track's mix, to describe in musical terms when you say what was sampled. Garage, future bass, house and some others
also chop a note a voice sang on another record: a part, `vox`, plays it on the song's chords in the drops and hooks
(the history says where it was sung and whether it plays a motif or an arpeggio); edit or remove it like any part.
Funk, reggae, phonk, the hip hop and soul genres and the disco ones sometimes sample a bass note (slap bass, dub
bass, an upright's pluck, a fretless, an 808): the song's `bass` part keeps its notes and plays them on it as a
sampler would, within an octave of where it was played (the history says `bass '<recording>', played at E1`). Its
sound is a `sample` like any other: set the part's sound to a synth to go back.
Trip-hop, boom bap, lo-fi, jungle, big beat and filter house are
built on samples unless `sampled: false`. When the person wants a sampled song, say which record kind you searched
for, and offer another if the phrase is not to their taste.

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
keys and bells. The 808 is the loudest thing in the song: 6 dB over a bass's usual level, low, driven, centred and
dry. When a person wants it to hit harder, `mix("bass", #{ drive: 18, level: 9 })` overloads it.

```js
blank("trap");
tempo(140);
key("C minor");
prog("A", "i i VI VII");
track("drums", s("[bd ~ ~ bd] [~ ~ bd ~], ~ ~ sd ~, hh*8").gain("1 0.9"));
track("rolls", s("~ ~ ~ [hh*3]").gain(0.5));
track("808", chord("prog").root().octave(1).struct("x ~ ~ x ~ ~ x ~").s("808").level(6));
mix("808", #{ drive: 12 });
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

## trip_hop (78-94 BPM)

Built on samples, as trip-hop records are: a slowed drum break, a dark phrase from a recording (film-score strings, a
harpsichord), vinyl crackle, a deep dub bass under them, strings that swell in the hooks, dub delays and long reverbs.
A `new_song` of trip-hop gathers a few samples from free libraries before it composes, which takes a minute or two,
until the genre has as many as it keeps; the songs after that draw on what was gathered. The song takes the phrase's key, tempo and chords, and its drum
lanes play the break's own hits. Without samples (offline, or a shared claudius) it plays electric piano and strings
instead. A sketch from patterns, without samples:

```js
blank("trip_hop");
tempo(88);
swing(0.54);
key("D minor");
prog("A", "i VI III VII");
track("drums", s("bd ~ ~ ~ ~ ~ bd bd, ~ ~ sd ~ ~ ~ sd ~, [hh hh]*4").gain("1 0.8"));
track("bass", chord("prog").root().struct("x ~ ~ ~ ~ ~ x ~").s("sub_bass").legato(0.9));
track("keys", chord("prog").struct("x ~ ~ ~").voicing().s("epiano").room(0.4).delay(0.3));
track("strings", chord("prog").voicing(52, 76).s("strings").role("pad").room(0.5).lpf(2500));
```

## big_beat (124-138 BPM)

Loud, crunched breakbeats, a squelching acid riff, stabs, filter sweeps into big drops. Distortion is the style:
crush the drums in parallel rather than all the way.

```js
blank("big_beat");
tempo(132);
key("E minor");
prog("A", "i i VII VII");
track("drums", s("bd ~ ~ bd ~ ~ bd ~, ~ sd ~ sd, hh*16").gain("1 0.8"));
track("acid", chord("prog").root().octave(2).struct("x*16").s("acid").lpf(1400).distort(4));
track("stabs", chord("prog").struct("~ ~ x ~").voicing().s("stab").crush(10).room(0.3));
fx("drums", [#{ fx: "parallel", dry: 1, chains: [
  #{ amount: 0.5, fx: [#{ fx: "compressor", threshold_db: -30, ratio: 12, attack_ms: 0.5, release_ms: 50 },
                       #{ fx: "saturate", kind: "hard", drive_db: 10 }] },
] }]);
```

## electro_house (126-130 BPM)

Four on the floor, big claps, a dirty bass riff in the mids pumped hard by the sidechain, a supersaw hook.

```js
blank("electro_house");
tempo(128);
key("F minor");
prog("A", "i VI III VII");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("bass", chord("prog").root().octave(2).struct("~ x ~ x ~ x x ~").s("saw_bass").distort(8).duck(0.8));
track("lead", chord("prog").struct("x ~ x ~ ~ x ~ x").voicing(64, 84).s("supersaw").duck(0.4).room(0.25));
```

## nu_disco (112-124 BPM)

Four on the floor with open hats on the offbeats, an octave-jumping bass, muted guitar or electric piano chords,
strings.

```js
blank("nu_disco");
tempo(118);
key("A minor");
prog("A", "i7 iv7");
track("drums", s("bd*4, [~ cp]*2, [~ oh]*4, hh*16").gain("1 0.7"));
track("bass", note("<[a1 a2]*4 [d2 d3]*4>").s("pluck_bass"));
track("guitar", chord("prog").struct("~ x ~ x ~ x ~ x").voicing(55, 74).s("pluck").pan(0.3));
track("strings", chord("prog").voicing(60, 80).s("strings").role("pad").room(0.4));
```

## filter_house (118-126 BPM)

A looped disco phrase pumping under the kick, through a resonant lowpass that opens and closes with the arrangement,
and a phaser.

```js
blank("filter_house");
tempo(122);
key("D dorian");
prog("A", "i7 IV7");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("loop", chord("prog").struct("x ~ x ~ ~ x ~ x").voicing(52, 72).s("stab").duck(0.8));
fx("loop", [
  #{ fx: "filter", mode: "lp", cutoff: [[0, 400], [32, 6000]], res: 0.5 },
  #{ fx: "phaser", rate_hz: 0.2, stages: 6, depth: 0.7, feedback: 0.4, mix: 0.45 },
]);
track("bass", note("<[d2 d3]*4 [g1 g2]*4>").s("organ_bass").duck(0.6));
```

## dance_pop (116-128 BPM)

A song first: verse, pre-chorus and a big chorus, a pop beat in the verses and four on the floor in the choruses,
bright claps, a pumping synth bass, plucked chords and a hook a voice could sing.

```js
blank("dance_pop");
tempo(122);
key("C major");
prog("A", "I V vi IV");
track("drums", s("bd ~ bd [~ bd], [~ cp]*2, hh*8"));
track("bass", chord("prog").root().struct("~ x ~ x ~ x ~ x").s("pluck_bass").duck(0.6));
track("chords", chord("prog").struct("~ x ~ x").voicing(55, 76).s("pluck").duck(0.5).room(0.2));
track("hook", n("<[4 ~ 4 2 4 ~ 5 4] [2 ~ 2 0 2 ~ 4 ~]>").scale("C:major").octave(5).s("pluck").delay(0.2));
```

## neo_psychedelia (96-122 BPM)

A loose live kit, a fuzzy electric bass, guitars swirling through phasers and choruses, organ, dreamy pads, sunny
mixolydian chords, long reverbs and tape wobble.

```js
blank("neo_psychedelia");
tempo(108);
key("G mixolydian");
prog("A", "I VII IV I");
track("drums", s("bd ~ sd ~, hh*8").gain("1 0.75").room(0.2));
track("bass", chord("prog").root().struct("x x x x").s("saw_bass").distort(4));
track("guitar", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(52, 76).s("pluck").room(0.4));
fx("guitar", [
  #{ fx: "phaser", rate_hz: 0.3, stages: 6, depth: 0.8, feedback: 0.45, mix: 0.5 },
  #{ fx: "chorus", rate_hz: 0.5, depth_ms: 4, delay_ms: 14, mix: 0.8 },
]);
track("organ", chord("prog").voicing(50, 72).s("organ").role("keys").room(0.3));
```

## art_rock (70-100 BPM)

Grows from a quiet piano and a glitchy synth arpeggio into a loud climax, as a band plays it: an acoustic kit with
fills, a pick or finger bass that distorts in the climaxes, strings, a clean guitar arpeggio, and two distorted
power-chord guitars panned wide in the rise and the climaxes. Dark minor, phrygian and harmonic-minor chords, a long arc
rather than a loop, tape on the mix, and no EDM risers or impacts.

```js
blank("art_rock");
tempo(92);
key("E minor");
prog("A", "i VI III VII");
track("piano", chord("prog").struct("x ~ x ~").voicing(48, 74).s("keys").role("keys").room(0.35));
track("arp", chord("prog").struct("x*8").arp("up").voicing(60, 84).s("pluck").role("arp").crush(10).delay(0.3));
track("drums", s("bd ~ ~ bd sd ~ ~ ~, hh*8").gain("1 0.7"));
form([sec("intro", 8, "piano arp"), sec("verse", 16, "piano arp drums"), sec("hook", 16, "*")]);
```

## idm (96-140 BPM)

Intricate broken beats with ratchets and ghost notes, glitches and gated bursts, crystalline bells, detuned pads, odd
modal harmony, every part processed further as the track goes on.

```js
blank("idm");
tempo(118);
key("D lydian");
prog("A", "Imaj7 IImaj7");
track("drums", s("bd ~ [~ bd] ~ ~ bd ~ ~, ~ ~ sd ~ ~ [sd sd] ~ ~, hh*16?0.3").gain("1 0.8"));
track("bells", n("<[0 4 7 11] [2 ~ 9 7]>").scale("D:lydian").octave(5).s("bell").delay(0.35).room(0.35));
track("pad", chord("prog").voicing(48, 76).s("warm_pad").role("pad").room(0.5));
fx("drums", [#{ fx: "parallel", dry: 1, chains: [
  #{ amount: 0.35, fx: [#{ fx: "bitcrush", bits: 7, downsample: 5 }, #{ fx: "filter", mode: "hp", cutoff: 600 }] },
] }]);
```

## dub_techno (116-126 BPM)

A soft steady kick, rimshots, one or two minor chords stabbed and drowned in long echoes that feed back and swell, a
filter on them that breathes with the arrangement, a deep sub, a fog of reverb.

```js
blank("dub_techno");
tempo(120);
key("C minor");
prog("A", "i7");
track("drums", s("bd*4, [~ hh]*4, [~ ~ ~ rim]*2").gain("1 0.6"));
track("chords", chord("prog").struct("~ x ~ ~").voicing(50, 70).s("stab").room(0.4).delay(0.6));
track("bass", chord("prog").root().struct("x ~ ~ ~").s("sub_bass").legato(0.9));
send_fx(#{ delay: #{ fx: "delay", beats: 0.75, feedback: 0.6, damping: 0.7, pingpong: true, mix: 1 } });
```

## boom_bap (86-96 BPM)

90s hip hop, built on samples: a dusty drum break with a lazy sampler swing, a soul or jazz phrase from a record,
chopped and crushed to 12 bits, vinyl crackle, an upright or sub bass, horn or flute phrases in the hooks. The first
`new_song` of boom bap gathers its samples, as trip-hop's does. Without them it plays the drum lanes and a piano or
Rhodes in their place. `swing(0.58)` gives the lean. A sketch from patterns, without samples:

```js
blank("boom_bap");
tempo(90);
swing(0.58);
key("D minor");
prog("A", "i9 iv9 VImaj7 v7");
track("drums", s("bd ~ ~ [~ bd] ~ bd ~ ~, ~ sd ~ sd, hh*8").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ x ~ ~").s("sub_bass").legato(0.8));
track("keys", chord("prog").struct("x ~ ~ ~ ~ ~ x ~").voicing(50, 74).s("epiano").room(0.25));
track("horns", n("<[7 ~ 9 7] [5 ~ 3 ~]>").scale("D:minor").octave(5).s("brass_lead").room(0.3));
```

## uk_drill (138-146 BPM)

Felt at half time: a snare on the third beat, skippy hats in threes, sliding 808s that follow a sparse kick, dark
minor and harmonic minor chords on piano, strings or choir, a bell line in the hooks. The 808 is as loud and as
driven as trap's.

```js
blank("uk_drill");
tempo(142);
key("G minor");
prog("A", "i VI");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ sd, [hh hh hh]*4").gain("1 0.75"));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1).level(6));
mix("808", #{ drive: 12 });
track("keys", chord("prog").voicing(52, 76).s("keys").room(0.35));
track("bells", n("<[0 ~ 3 2] [7 ~ 5 ~]>").scale("G:minor").octave(5).s("bell").delay(0.25));
```

## dubstep (138-142 BPM)

At half time: a big snare on the third beat over a sparse kick, a wobbling mid bass over a clean sub, dark minor and
phrygian chords on pads, risers into heavy drops.

```js
blank("dubstep");
tempo(140);
key("F minor");
prog("A", "i VI");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~, ~ sd, hh*8").gain("1 0.7"));
track("sub", chord("prog").root().octave(1).struct("x ~ ~ ~").s("sub_bass").legato(1));
track("wobble", n("0 0 ~ 0 3 ~ 0 ~").scale("F:minor").octave(2).s("wobble"));
track("pad", chord("prog").voicing(48, 74).s("pad").role("pad").room(0.5));
form([sec("intro", 16, "pad"), sec("build", 8, "pad drums").build().gap(), sec("drop", 32, "*").impact()]);
```

## uk_garage (130-136 BPM)

A heavy shuffle: a two-step kick that skips the third beat, snares on 2 and 4, shuffled hats and shakers, a
bouncing organ or sub bass, soulful minor 7th chords on organ or Rhodes. `swing(0.62)` is the shuffle.

```js
blank("uk_garage");
tempo(132);
swing(0.62);
key("A minor");
prog("A", "i7 iv7 VImaj7 v7");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ sd ~ sd, [~ hh]*4, sh*16").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ ~ x ~").s("organ_bass").legato(0.7));
track("chords", chord("prog").struct("~ x ~ ~ x ~ ~ ~").voicing(55, 76).s("organ").room(0.2));
```

## jungle (160-172 BPM)

A chopped, rolling breakbeat with ghost snares on an acoustic kit, a deep sub or reese bass at half the speed of the
drums, dub chords and stabs with long delays, warm pads.

```js
blank("jungle");
tempo(166);
key("E minor");
prog("A", "i7 iv7");
track("drums", s("bd ~ bd ~ sd ~ ~ sd ~ sd bd ~ sd ~ ~ sd, hh*8, rd*8").gain("1 0.8"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~").s("reese").legato(1));
track("chords", chord("prog").struct("~ x ~ ~").voicing(55, 74).s("stab").room(0.35).delay(0.4));
track("pad", chord("prog").voicing(48, 74).s("pad").role("pad").room(0.5));
```

## afrobeats (100-115 BPM)

A rolling West African groove: a kick on one and three, claps on 2 and 4, a rim on the clave, shakers and congas,
bright chords on guitar plucks and keys, a warm sub, a sung-sounding lead in the hooks.

```js
blank("afrobeats");
tempo(106);
swing(0.55);
key("A major");
prog("A", "I vi IV V");
track("drums", s("bd ~ bd ~, ~ cp ~ cp, rim ~ ~ rim ~ ~ rim ~ ~ ~ rim ~ rim ~ ~ ~, sh*16").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ ~ x ~").s("sub_bass").legato(0.8));
track("guitar", chord("prog").struct("~ x ~ x ~ x ~ x").voicing(55, 76).s("pluck").delay(0.15));
track("lead", n("<[4 ~ 2 4] [5 4 2 ~]>").scale("A:major").octave(5).s("vox_lead").room(0.3));
```

## amapiano (110-116 BPM)

A steady kick under shakers and open hats, the log drum (a pitched, woody bass in syncopated lines), jazzy minor 7th
and 9th chords on piano and Rhodes, airy pads, a whistle or flute line, long grooves that hold back.

```js
blank("amapiano");
tempo(113);
swing(0.56);
key("F minor");
prog("A", "i9 iv9");
track("drums", s("bd*4, [~ cp]*2, sh*16, [~ oh]*4").gain("1 0.7"));
track("logdrum", chord("prog").root().octave(2).struct("~ ~ x ~ ~ x ~ ~ x ~ ~ ~ x ~ x ~").s("log_drum").role("bass"));
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(52, 76).s("keys").room(0.3));
```

## reggaeton (88-100 BPM)

The dembow: a kick on every beat and a snare after the first and before the third of each half bar, minor chords on
plucks, pianos and pads, a sub on the roots, a synth or voice-like lead.

```js
blank("reggaeton");
tempo(94);
key("C minor");
prog("A", "i VI III VII");
track("drums", s("bd*4, ~ ~ ~ sd ~ ~ sd ~ ~ ~ ~ sd ~ ~ sd ~, hh*8").gain("1 0.75"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ x ~ ~ ~").s("sub_bass").legato(0.9));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(55, 76).s("pluck").room(0.25));
```

## trance (134-140 BPM)

Four on the floor with offbeat open hats, a rolling bass, euphoric minor progressions under supersaw chords and a
soaring lead, arpeggios, a long breakdown that builds back into the drop.

```js
blank("trance");
tempo(138);
key("A minor");
prog("A", "i VI III VII");
track("drums", s("bd*4, [~ cp]*2, [~ oh]*4").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("[~ x x x]*4").s("saw_bass"));
track("chords", chord("prog").voicing(55, 79).s("supersaw").role("chords").room(0.35));
track("arp", chord("prog").struct("x*16").arp("up").voicing(60, 84).s("pluck").role("arp").delay(0.3));
form([sec("intro", 16, "drums arp"), sec("break", 16, "chords arp").build().gap(), sec("drop", 32, "*").impact()]);
```

## neo_soul (72-92 BPM)

A laid-back groove on a live kit with ghost notes and a cross-stick, rich 9th and 11th chords on Rhodes, a round bass
that walks between roots, soft strings, a sung-sounding lead. `swing(0.58)` leans it back.

```js
blank("neo_soul");
tempo(84);
swing(0.58);
key("E dorian");
prog("A", "i9 IV9");
track("drums", s("bd ~ ~ [~ bd] ~ ~ bd ~, ~ sd ~ sd, hh*8, [~ ~ ~ rim]*2").gain("1 0.7"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ x ~ ~").s("rubber_bass").legato(0.9));
track("keys", chord("prog").struct("x ~ ~ ~ ~ x ~ ~").voicing(50, 76).s("epiano").room(0.3));
```

## hip_hop (84-100 BPM)

Modern hip hop between boom bap and trap: a hard kick, a snare and clap on 2 and 4, crisp eighth hats with the odd
roll, an 808 or a deep sub under piano, Rhodes and strings, a melody that loops on piano, pizzicato or flute.

```js
blank("hip_hop");
tempo(92);
key("C minor");
prog("A", "i7 iv7 VImaj7 V7");
track("drums", s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~, ~ [sd,cp] ~ [sd,cp], hh*8, ~ ~ ~ rim ~ ~ rim ~"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~ ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1.0));
track("keys", chord("prog").struct("x ~ ~ ~ x ~ ~ ~").voicing(50, 74).s("keys").room(0.2));
```

## rnb (64-80 BPM)

Contemporary R&B: lush 9th chords on Rhodes and piano, an 808 or a warm sub that slides, sixteenth hats that roll, finger
snaps and a soft clap on the backbeat, airy pads, a sung or chopped hook. The vocal chops `sampled: true` gathers
suit it best.

```js
blank("rnb");
tempo(72);
swing(0.56);
key("F minor");
prog("A", "i9 iv9 VImaj9 V9");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ sd ~ sd, hh*16, ~ snap ~ ~").gain("1 0.7 0.85 0.7"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1.0));
track("keys", chord("prog").struct("x ~ ~ ~ ~ ~ x ~").voicing(50, 76).s("epiano").room(0.3));
```

## funk (96-116 BPM)

Everything on the one: a syncopated kick, a snare on 2 and 4 with ghost notes between, sixteenth hats, a finger or
slap bass off the beat, clavinet and a rhythm guitar chopping sixteenths, horn stabs, long vamps on one chord.

```js
blank("funk");
tempo(104);
swing(0.55);
key("E dorian");
prog("A", "i7 IV7");
track("drums", s("bd ~ ~ bd ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ bd, ~ ~ ~ ~ sd ~ [~ sd] ~ ~ ~ ~ ~ sd ~ ~ [~ sd], hh*16").gain("1 0.6"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ ~ x ~ ~ x ~ ~ x ~ x ~").s("pluck_bass").legato(0.5));
track("clav", chord("prog").struct("~ x ~ x ~ x x ~ ~ x ~ x ~ x x ~").voicing(52, 74).s("clav"));
```

## tech_house (124-128 BPM)

Between house and techno: a punchy kick on every beat, a rolling offbeat bassline that carries the groove, a clap on 2
and 4, shakers and rims, percussion that loops, a stab now and then, chopped vocals, long builds into tight drops.

```js
blank("tech_house");
tempo(126);
key("A minor");
prog("A", "i");
track("drums", s("bd*4, ~ cp ~ cp, [~ hh]*4, sh*16, ~ ~ ~ rim ~ ~ ~ ~ ~ rim ~ ~ ~ ~ rim ~").gain("1 0.8"));
track("bass", chord("prog").root().octave(1).struct("~ x x ~ ~ x x ~ ~ x x ~ ~ x x ~").s("rubber_bass").legato(0.6));
```

## ambient (60-90 BPM)

Slow and spacious, a soft pulse or none: long pads and string drones that evolve, washes of reverb and delay, a few
notes of piano or mallet over slow chords, field recordings under it (the textures `sampled: true` gathers), sections
that swell and fade rather than drop.

```js
blank("ambient");
tempo(70);
key("D lydian");
prog("A", "Imaj9 IImaj7");
track("pad", chord("prog").voicing(45, 76).s("air_pad").room(0.6));
track("keys", n("<0 ~ 4 ~> <2 ~ ~ 6>").scale("D:lydian").octave(5).s("mallet").room(0.5).delay(0.3));
```

## grime (138-142 BPM)

London's sound: sparse, jagged drums with the snare on the third beat, square-wave and sine basslines in short bursts,
cold eski synths, pizzicato or string stabs, sub drops, eight-bar sections made for MCs.

```js
blank("grime");
tempo(140);
key("G minor");
prog("A", "i VI");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~, ~ ~ sd ~, hh ~ hh hh, ~ ~ ~ ~ ~ ~ rim ~ ~ ~ ~ rim ~ ~ ~ ~"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~ x ~ ~ ~").s("pluck_bass").legato(0.4));
track("lead", n("<[0 ~ 3 ~ 5 ~ 3 ~] [7 ~ 5 ~ 3 ~ 2 ~]>").scale("G:minor").octave(5).s("square_lead"));
```

## phonk (130-145 BPM)

Memphis rap's dark sound remade: a distorted 808 that slides, an 808 cowbell playing the melody (`cowbell_lead`), a
hard kick and a trap snare and clap on the third beat, hats that drive and roll, Memphis vocal chops (`sampled: true`),
tape grime over all of it.

```js
blank("phonk");
tempo(140);
key("C phrygian");
prog("A", "i II");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ ~ [sd,cp] ~, hh*8"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1.0));
track("cowbell", n("0 ~ 3 0 ~ 5 3 ~ 0 ~ 3 0 ~ 7 5 3").scale("C:phrygian").octave(5).s("cowbell_lead"));
```

## reggae (66-84 BPM)

Laid back: the one drop, kick and cross-stick together on the third beat, a hat skipping in swung eighths, a deep bass
that leaves the one empty, guitar and organ skanking on the offbeats, horns in the choruses, dub's echo.

```js
blank("reggae");
tempo(76);
swing(0.6);
key("A minor");
prog("A", "i iv");
track("drums", s("~ ~ [bd,rim] ~, hh*8").gain("1 0.7"));
track("bass", chord("prog").root().octave(2).struct("~ ~ x ~ ~ x x ~").s("deep_bass").legato(0.8));
track("guitar", chord("prog").struct("~ x ~ x").voicing(55, 76).s("clav").delay(0.2));
```
