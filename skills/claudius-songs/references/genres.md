# Starting points for the sixty-three genres

Two or three genres fuse with `+`: `song("trip_hop+idm", 3)` (or `new_song` with genre `trip_hop+idm`) is trip-hop's
song, harmony, parts and mix with IDM's drums, and IDM's sounds lent to the parts. The first genre leads, the last
plays the drums. Fuse when the person describes a mix of styles ("trip-hop but with glitchy drums", "disco with
house drums"): it is one call, and the result is a song like any other to edit.

`new_song` (or `song("<genre>", seed)`) writes a complete song in any of these, and is the quickest start. The
sketches below are for when you build from patterns: each is a skeleton to grow, not a finished song. They write the
harmony with `prog()` and let the bass and chords follow it (`chord("prog")`), so that the parts agree with each
other and with what the critic checks the notes against. `catalog`
lists the genres by family with their other names and tempo ranges, and user genres when the person has added some.
When the person names a style rather than a genre ("90s rap", "something west coast", "dusty"), `find_genre` with
their words answers the genres for it, best first; `new_song` also takes another name of a genre that is no other
genre's ("90s hip hop" is `boom_bap`).

Any of them can be built on samples from free libraries: `new_song` with `sampled: true`, or with `sample_search`
naming a kind of record ("70s soul", "bossa nova guitar", "film score"; not the genre's own name, which finds other
people's tracks rather than records to sample). The song takes a phrase's key, tempo and chords, played faster or
slower by whole semitones as a sampler would, or chopped onto the song's grid at its own pitch at a tempo a multiple of
its own (two thirds, three quarters, four thirds, three halves: the history says `chopped onto the song's grid`; its
`sample` track's `retime`), in place of the genre's chords or keys, with a break and a texture from what the genre's
producers dig for. A break off the song's tempo is chopped onto the grid the same way. Another passage of the same record plays as `sample2` in some sections, on its
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
roll, a distorted 808 or a deep sub under a dark minor loop of a few chords on piano or Rhodes, strings now and then,
a melody that loops on the piano. Not a lounge: no vibraphone, organ, flute or jazz sevenths, which make it sound like
elevator music; `jazz_rap` is the genre for those.

```js
blank("hip_hop");
tempo(92);
key("C minor");
prog("A", "i VI iv V");
track("drums", s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~, ~ [sd,cp] ~ [sd,cp], hh*8"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~ ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1.0));
mix("bass", #{ drive: 6 });
track("keys", chord("prog").struct("x ~ ~ ~ x ~ ~ ~").voicing(45, 69).s("keys").room(0.2));
```

## alt_hip_hop (76-98 BPM)

Alternative hip hop, layered as its producers layer it: lush ninth and eleventh chords on a Rhodes or a Wurlitzer
stacked on warm pads and strings, a distorted 808 with a fuzzy synth bass an octave over it (`fuzz`, a double of the
`bass`), a swung kit with a clap on the snare, soul records chopped under it, a lead that sings and a counter-melody
that answers it, and a beat switch into half time (`switch`) before the last hooks. Its songs are layered
(complexity 0.8) unless asked otherwise; `complexity: 0.5` gives the genre as written.

```js
blank("alt_hip_hop");
tempo(88);
swing(0.57);
key("Bb major");
prog("A", "IVmaj9 iii7 vi9 ii9");
track("drums", s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~, ~ [sd,cp] ~ [sd,cp], hh*8, ~ ~ ~ rim ~ ~ rim ~"));
part("bass", #{ style: "syncopated" });
part("fuzz", #{});
part("keys", #{ style: "charleston" });
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

## g_funk (88-100 BPM)

West Coast G-funk of the 90s, laid back and swung: a punchy kick, a snare and clap on 2 and 4, swung sixteenth hats
and a shaker, a Minimoog bass bouncing through funk lines, a Rhodes vamping on minor sevenths, and the whine: a high
synth lead that glides a long way from note to note (`glide_lead`). A voice through a talk box (`talkbox`, the vocal
lead) answers in the hooks.

```js
blank("g_funk");
tempo(94);
swing(0.57);
key("D minor");
prog("A", "i7 iv7");
track("drums", s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~, ~ [sd,cp] ~ [sd,cp], hh*16, ~ sh ~ sh ~ sh ~ sh").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ ~ x ~ x ~ ~ ~ x ~ x ~").s("moog_bass").legato(0.7));
track("keys", chord("prog").struct("x ~ ~ ~ ~ x ~ ~").voicing(50, 74).s("epiano").room(0.25));
track("whine", n("<[7 ~ ~ 9] [7 ~ 4 ~]>").scale("D:minor").octave(5).s("glide_lead").delay(0.2).level(-4));
```

## gangsta_rap (84-96 BPM)

The hardcore hip hop of the 90s, built on samples: a hard, dusty break, a menacing minor piano or string loop from an
old film score, chopped and crushed, low strings hanging under it, a sub that follows the kick, a high eerie piano note
now and then, vinyl crackle. Its first `new_song` gathers its samples, as boom bap's does; without them it plays the
drum lanes and the piano. A sketch from patterns:

```js
blank("gangsta_rap");
tempo(90);
swing(0.55);
key("E minor");
prog("A", "i VI");
track("drums", s("bd ~ ~ [~ bd] ~ ~ bd ~, ~ sd ~ sd, hh*8").gain("1 0.8"));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~ ~ ~ x ~ ~ ~ ~ ~").s("sub_bass").legato(0.9));
track("piano", n("<[0 ~ 2 ~ 1 ~ 0 ~] [5 ~ 4 ~ ~ ~ ~ ~]>").scale("E:minor").octave(4).s("keys").room(0.2));
track("strings", chord("prog").voicing(40, 64).s("strings").level(-4));
```

## jazz_rap (86-100 BPM)

The hip hop of the Native Tongues, built on jazz records: a swung, dusty break, an upright bass that walks or loops a
figure, a Rhodes, a piano or a vibraphone on sevenths and ninths, a muted trumpet, a sax or a flute answering in the
hooks, vinyl crackle. This is where the lounge's sounds belong. Its first `new_song` gathers jazz records; a sketch
from patterns:

```js
blank("jazz_rap");
tempo(94);
swing(0.58);
key("D dorian");
prog("A", "i9 IV9");
track("drums", s("bd ~ ~ [~ bd] ~ bd ~ ~, ~ sd ~ sd, hh*8, ~ ~ ~ rim ~ ~ rim ~").gain("1 0.75"));
track("bass", chord("prog").root().octave(2).struct("x ~ x ~ x ~ x ~").s("deep_bass").legato(0.8));
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(50, 74).s("epiano").room(0.2));
track("horns", n("<[4 ~ 6 4] [2 ~ 0 ~]>").scale("D:dorian").octave(5).s("brass_lead").room(0.3).level(-2));
```

## chipmunk_soul (80-96 BPM)

The hip hop of the early 2000s, built on soul records played fast enough that their singers' voices rise (its phrase
plays up to six semitones up): a hard programmed kick, a snare and a clap cracking together on 2 and 4, crisp hats, a
deep bass under the sample's chords, horn stabs and strings in the hooks. Its first `new_song` gathers soul records
with voices on them; without them a Rhodes plays the chords. A sketch from patterns:

```js
blank("chipmunk_soul");
tempo(88);
key("Eb major");
prog("A", "IVmaj7 iii7 vi7 V");
track("drums", s("bd ~ ~ ~ ~ ~ ~ bd ~ ~ bd ~ ~ ~ ~ ~, ~ [sd,cp] ~ [sd,cp], hh*8").gain("1 0.85"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ ~ ~ ~ x ~").s("deep_bass").legato(0.9));
track("keys", chord("prog").struct("x ~ ~ ~ x ~ ~ ~").voicing(50, 74).s("epiano").room(0.2));
track("horns", chord("prog").struct("~ ~ x ~ ~ ~ ~ ~").voicing(55, 76).s("brass_stab").level(-3));
```

## crunk (80-104 BPM)

The Southern club rap of the 2000s: booming 808 kicks and a long 808 bass, a loud snare and clap cracking together,
straight eighth hats with a roll now and then, a squealing synth riff on one or two minor chords, big brass-synth stabs
in the hooks, loud and driven.

```js
blank("crunk");
tempo(100);
key("F minor");
prog("A", "i i VI i");
track("drums", s("bd ~ bd ~, ~ [sd,cp] ~ [sd,cp], hh*8").gain("1 0.9"));
track("808", chord("prog").root().octave(1).struct("x ~ x ~").s("808").legato(1).level(5));
mix("808", #{ drive: 14 });
track("riff", n("<[7 ~ 7 6 7 ~ 4 ~] [5 ~ 5 4 2 ~ 0 ~]>").scale("F:minor").octave(5).s("saw_lead").level(-2));
track("stabs", chord("prog").struct("x ~ ~ ~ ~ ~ x ~").voicing(52, 72).s("brass_stab"));
```

## cloud_rap (128-148 BPM)

Dreamy, hazy trap in half time: a soft 808 that holds its notes, a clap on beat 3 drowned in reverb, sparse hats, airy
pads and voices washed out and slowed, lush sevenths that drift, everything in long reverb and tape.

```js
blank("cloud_rap");
tempo(138);
key("A minor");
prog("A", "VImaj7 iv7 i7 v7");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ ~ cp ~, hh*4").gain("0.9 0.7").room(0.35));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~").s("808").legato(1).level(4));
track("pad", chord("prog").voicing(50, 76).s("air_pad").room(0.7));
track("voice", n("<[4 ~ ~ ~ 2 ~ ~ ~] [0 ~ ~ ~ ~ ~ ~ ~]>").scale("A:minor").octave(5).s("vox_lead").room(0.6).delay(0.3).level(-4));
```

## emo_rap (130-170 BPM)

Melodic SoundCloud rap in half time, built on a guitar: a clean or acoustic guitar loop picking a sad minor
progression (`i VI III VII`), a distorted 808 that slides, a snare or clap on beat 3, rolling hats. Its first
`new_song` gathers a guitar loop; without one the guitar part plays, on General MIDI's clean guitar when nothing better
is installed.

```js
blank("emo_rap");
tempo(150);
key("E minor");
prog("A", "i VI III VII");
track("drums", s("[bd ~ ~ bd] [~ ~ bd ~], ~ ~ sd ~, hh*8").gain("1 0.85"));
track("rolls", s("~ ~ ~ [hh*3]").gain(0.5));
track("808", chord("prog").root().octave(1).struct("x ~ ~ x ~ ~ x ~").s("808").level(6));
mix("808", #{ drive: 14 });
track("guitar", chord("prog").struct("x*8").arp("updown").voicing(52, 76).s("harp_pluck").room(0.35).delay(0.15));
```

## rage (150-170 BPM)

Aggressive synth trap in half time: a loud, distorted supersaw lead ringing over one or two minor chords, a supersaw
arpeggio under it, a pounding 808 driven to the edge, a snare on beat 3, fast hats, loud and bright.

```js
blank("rage");
tempo(160);
key("C# minor");
prog("A", "i i VI i");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ ~ sd ~, hh*16").gain("1 0.8"));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~").s("808").legato(1).level(5));
mix("808", #{ drive: 18 });
track("lead", n("<[4 ~ 4 ~ 2 ~ 0 ~] [4 ~ 6 ~ 4 ~ ~ ~]>").scale("C#:minor").octave(5).s("supersaw").level(-1));
track("arp", chord("prog").struct("x*16").arp("updown").voicing(55, 79).s("supersaw").level(-6));
```

## plugg (140-160 BPM)

Soft, bright trap in half time: lush major and minor sevenths and ninths on a Rhodes and an airy pad, a bell or a
pluck singing a simple melody, a sliding 808 under a sparse kick, a clap on beat 3, hats in triplet rolls.

```js
blank("plugg");
tempo(150);
key("F major");
prog("A", "IVmaj7 iii7 vi9 ii9");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ ~ cp ~, hh*4").gain("0.9 0.7"));
track("rolls", s("~ ~ ~ [hh*3]").gain(0.4));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~").s("808").legato(1).level(5));
track("keys", chord("prog").struct("x ~ ~ ~ ~ ~ ~ ~").voicing(52, 76).s("epiano").room(0.35));
track("bells", n("<[4 ~ 2 ~ 4 ~ 5 ~] [4 ~ 2 ~ 0 ~ ~ ~]>").scale("F:major").octave(5).s("bell").delay(0.3).level(-3));
```

## ny_drill (138-146 BPM)

Brooklyn's drill, felt at half time: heavy 808s that slide and bounce between octaves under a sparse kick, the drill
snare on beat 3 answered late, skippy hats in threes, a dark choir or strings, a minor piano figure, a bell or flute
line in the hooks. Its sample searches dig for R&B hooks to flip.

```js
blank("ny_drill");
tempo(142);
key("F minor");
prog("A", "i VI");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ sd, [hh hh hh]*4").gain("1 0.75"));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ x ~ ~ x ~ ~ ~ ~ ~").s("808").legato(1).level(6));
mix("808", #{ drive: 16 });
track("choir", chord("prog").voicing(48, 72).s("vowel_pad").room(0.45));
track("keys", n("<[0 ~ 2 ~ 4 ~ 2 ~] [5 ~ 4 ~ 2 ~ ~ ~]>").scale("F:minor").octave(4).s("keys").room(0.3));
```

## chicago_drill (130-150 BPM)

The drill of the early 2010s in half time: menacing synth brass and strings on dark minor chords, an eerie bell or
piano figure over them, booming 808s that follow the kick, a clap and snare on beat 3, hats in triplet rolls.

```js
blank("chicago_drill");
tempo(140);
key("G minor");
prog("A", "i iv");
track("drums", s("[bd ~ ~ bd] [~ ~ bd ~], ~ ~ [sd,cp] ~, hh*8").gain("1 0.9"));
track("rolls", s("~ ~ [hh*3] [hh*3]").gain(0.5));
track("808", chord("prog").root().octave(1).struct("x ~ ~ x ~ ~ x ~").s("808").level(6));
track("brass", chord("prog").struct("x ~ ~ ~ x ~ ~ ~").voicing(48, 70).s("brass_stab").level(-2));
track("bells", n("<[0 ~ 4 ~ 3 ~ 2 ~] [0 ~ ~ ~ ~ ~ ~ ~]>").scale("G:minor").octave(5).s("bell").level(-5));
```

## rnb_90s (66-100 BPM)

Hip hop soul and the slow jams of the 90s: a swung sampler beat with a soft snare and a finger snap on 2 and 4,
sixteenth hats and a shaker, the bright DX7 electric piano (`tine_piano`, which this genre plays as its own) or a
Rhodes on lush sevenths and ninths, a round synth bass that slides, strings in the hooks, a sax or a sung line.

```js
blank("rnb_90s");
tempo(76);
swing(0.57);
key("Ab major");
prog("A", "IVmaj9 iii7 ii9 V9");
track("drums", s("bd ~ ~ [~ bd] ~ ~ bd ~, ~ [sd,snap] ~ [sd,snap], hh*16, sh*8").gain("0.9 0.6"));
track("bass", chord("prog").root().octave(2).struct("x ~ ~ x ~ ~ x ~").s("moog_bass").legato(0.8));
track("keys", chord("prog").struct("x ~ ~ ~ ~ x ~ ~").voicing(50, 76).s("tine_piano").room(0.3));
```

## new_jack_swing (100-118 BPM)

The R&B of the late 80s and early 90s: a drum machine swung hard (`swing(0.62)`), a punchy kick, a big snare and clap
on 2 and 4, shuffling sixteenth hats, synth stabs and brassy hits on minor sevenths, a slap synth bass that pops, the
DX7 electric piano, a synth lead answering the hooks.

```js
blank("new_jack_swing");
tempo(108);
swing(0.62);
key("C minor");
prog("A", "i7 IV7");
track("drums", s("bd ~ ~ bd bd ~ ~ ~, ~ [sd,cp] ~ [sd,cp], hh*16").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("x ~ x x ~ x ~ x").s("rubber_bass").legato(0.5));
track("stabs", chord("prog").struct("~ x ~ ~ ~ x ~ x").voicing(55, 76).s("brass_stab"));
track("keys", chord("prog").struct("x ~ ~ ~ x ~ ~ ~").voicing(52, 76).s("tine_piano").room(0.25).level(-3));
```

## alt_rnb (60-90 BPM)

Dark, atmospheric R&B: pads and voices in long reverb, a Rhodes, an organ or a piano on minor sevenths that hang, an
808 or a sub that holds, a sparse kick, a snare or clap on 2 and 4 with a tail, hats that roll now and then.

```js
blank("alt_rnb");
tempo(70);
key("C minor");
prog("A", "i7 VImaj7 iv7 v7");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ sd ~ sd, hh*8").gain("0.85 0.6").room(0.3));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~").s("808").legato(1).level(4));
track("keys", chord("prog").voicing(48, 74).s("epiano").room(0.45));
track("pad", chord("prog").voicing(48, 74).s("glass_pad").room(0.7).level(-2));
```

## big_room (126-130 BPM)

Festival EDM: a booming four-on-the-floor kick with a long tail, claps and offbeat hats, tribal toms, builds that
strip to almost nothing, and a drop of the kick and one huge, simple riff in a hall of reverb. Keep the drop sparse:
the kick, the bass on the offbeats and the riff. "edm" also names electro house, future bass and trance: ask which
the person means, or start with big room for the festival sound.

```js
blank("big_room");
tempo(128);
key("F minor");
prog("A", "i i VI VII");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4, ~ ~ ~ lt ~ ~ lt ~ ~ ~ ~ lt ~ lt lt ~").gain("1 0.85"));
mix("drums", #{ reverb: 0.1 });
track("bass", chord("prog").root().octave(2).struct("[~ x]*4").s("saw_bass").duck(0.85));
track("riff", n("<[0 ~ 0 3 ~ 0 5 ~] [0 ~ 0 3 ~ 7 5 ~]>").scale("F:minor").octave(4).s("supersaw").distort(4).room(0.5));
```

## progressive_house (124-128 BPM)

Euphoric and melodic: four on the floor, a clap and offbeat hats, piano chords pumping off the beat, a supersaw hook
and plucked arpeggios over warm minor and major progressions, long breakdowns that bloom into the drop.

```js
blank("progressive_house");
tempo(126);
key("A minor");
prog("A", "VI VII i v");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4, sh*16").gain("1 0.8"));
track("bass", chord("prog").root().octave(2).struct("[~ x]*4").s("saw_bass").duck(0.7));
track("piano", chord("prog").struct("~ x ~ x ~ x ~ x").voicing(52, 76).s("house_piano").role("keys").room(0.3).duck(0.35));
track("hook", n("<[4 ~ 4 2 ~ 4 5 ~] [7 ~ 5 4 ~ 2 0 ~]>").scale("A:minor").octave(5).s("supersaw").room(0.35).delay(0.25));
track("arp", chord("prog").struct("x*16").arp("up").voicing(60, 84).s("pluck").delay(0.3).level(-4));
```

## slap_house (120-126 BPM)

The 2020s' pop house: a punchy four-on-the-floor kick and a snappy clap, a metallic plucked bass bouncing between
octaves and pumped hard by the sidechain, sparse minor chords, a deep vocal-like lead. The bass is the hook: give it
the octave jumps and the sidechain.

```js
blank("slap_house");
tempo(124);
key("G minor");
prog("A", "i VI III VII");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4").gain("1 0.85"));
track("bass", n("<[0 ~ 7 0 ~ 7 ~ 0] [0 ~ 7 0 ~ 5 7 ~]>").scale("G:minor").octave(2).s("fm_bass").duck(0.9).distort(3));
track("chords", chord("prog").struct("x ~ ~ ~ x ~ ~ ~").voicing(55, 74).s("stab").room(0.3).duck(0.6));
track("lead", n("<[2 ~ 0 ~] [4 ~ 2 0]>").scale("G:minor").octave(4).s("vox_lead").room(0.3).delay(0.2));
```

## hardstyle (150-155 BPM)

A distorted, pitched kick with a long ringing tail carries the low end, a reverse bass bounces on every offbeat, a
euphoric supersaw lead soars over strings and choirs in long breakdowns, and the kick rolls into the drops. The kick
is the genre: drive it hard and let it ring, and keep everything else out from under it with the sidechain.

```js
blank("hardstyle");
tempo(150);
key("A minor");
prog("A", "i VI III VII");
track("kick", s("bd*4").distort(12).level(3));
track("hats", s("~ cp ~ cp, [~ hh]*4").gain(0.8));
track("bass", chord("prog").root().octave(2).struct("[~ x]*4").s("dist_bass").duck(0.95));
track("lead", n("<[0 2 4 7 4 2 4 ~] [5 4 2 0 2 ~ 0 ~]>").scale("A:minor").octave(5).s("supersaw").room(0.35).delay(0.2));
track("strings", chord("prog").voicing(48, 76).s("strings").role("pad").room(0.5));
```

## hard_techno (145-155 BPM)

A heavily distorted kick pounding every beat with a rumbling bass rolling under it, relentless offbeat hats and
rides, metallic percussion, industrial stabs, an acid or saw lead; short breaks that slam back in.

```js
blank("hard_techno");
tempo(150);
key("F minor");
prog("A", "i i i bII");
track("kick", s("bd*4").distort(12).level(2));
track("hats", s("[~ hh]*4, rd*8, ~ perc ~ ~ perc ~ ~ perc ~ ~ perc ~ ~ perc ~ ~").gain(0.6));
track("bass", chord("prog").root().octave(1).struct("[~ x x x]*4").s("sub_bass").duck(0.85).distort(3));
track("stab", chord("prog").struct("~ ~ x ~").s("stab").distort(4).delay(0.3));
```

## gabber (170-190 BPM)

Rotterdam hardcore: a kick distorted until it booms and buzzes like a square wave hammers every beat, offbeat hats
and a clap on 2 and 4, minor riffs on hoover-like saws, screeching stabs. Drive the kick into the clipper and leave
the low end to it alone.

```js
blank("gabber");
tempo(180);
key("E minor");
prog("A", "i i VI VII");
track("kick", s("bd*4").distort(14).level(3));
track("hats", s("~ cp ~ cp, [~ hh]*4").gain(0.8));
track("riff", n("<[0 ~ 0 ~ 3 ~ 2 ~] [0 ~ 0 ~ 5 ~ 3 2]>").scale("E:minor").octave(4).s("saw_lead").distort(6).room(0.25));
track("stabs", chord("prog").struct("~ x ~ x").voicing(52, 72).s("stab").distort(4));
```

## hyperpop (140-170 BPM)

Maximal, glitchy internet pop in half time: blown-out 808s, trap hats in rolls, bright supersaw chords, bubbly bell
and chip leads up high, sugary major-key hooks, everything overdriven and crushed.

```js
blank("hyperpop");
tempo(158);
key("E major");
prog("A", "I V vi IV");
track("drums", s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~, ~ ~ sd ~, hh*16").gain("1 0.8"));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ x ~").s("808").legato(1).level(4));
mix("808", #{ drive: 20 });
track("chords", chord("prog").struct("x ~ x ~ x ~ x ~").voicing(55, 79).s("supersaw").distort(3));
track("lead", n("<[4 ~ 4 5 4 ~ 2 ~] [7 ~ 6 4 ~ 2 4 ~]>").scale("E:major").octave(6).s("bell").crush(10));
```

## jersey_club (135-145 BPM)

Five kicks to a bar that run into a bounce of threes (steps 1, 5, 9, 12 and 15 of sixteen) and burst into triplets
at a phrase's end, a snare doubling the end of the run, bed-squeak stabs between the kicks, an 808 following the
kick, vocal-chop stabs and bright chords chopped short.

```js
blank("jersey_club");
tempo(140);
key("C minor");
prog("A", "i VI III VII");
track("drums", s("bd ~ ~ ~ bd ~ ~ ~ bd ~ ~ bd ~ ~ bd ~, ~ ~ ~ ~ ~ ~ ~ ~ sd ~ ~ sd ~ ~ ~ ~, ~ hh ~ hh").gain("1 0.85"));
track("squeak", s("~ ~ perc ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ perc ~ ~").gain(0.6));
track("808", chord("prog").root().octave(1).struct("x ~ ~ ~ x ~ ~ ~ x ~ ~ x ~ ~ x ~").s("808").legato(0.5).level(3));
track("chop", n("<[4 ~ 4 ~ ~ 2 ~ ~] [4 ~ 5 4 ~ 2 ~ 0]>").scale("C:minor").octave(5).s("vox_lead").delay(0.2));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(55, 76).s("stab").room(0.25));
```

## afro_house (120-124 BPM)

A rolling four-on-the-floor under congas, shakers and tribal toms, a deep melodic bass, minor chords on plucks and
marimbas, a haunting flute line; long, hypnotic grooves.

```js
blank("afro_house");
tempo(122);
swing(0.54);
key("A minor");
prog("A", "i7 iv7");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4, sh*16").gain("1 0.8"));
track("perc", s("~ ~ perc ~ perc ~ ~ perc ~ perc ~ ~ perc ~ perc ~, ~ ~ ~ lt ~ ~ lt ~").gain(0.6));
track("bass", chord("prog").root().octave(2).struct("~ ~ x ~ ~ x ~ x").s("deep_bass").legato(0.8));
track("chords", chord("prog").struct("~ x ~ x ~ ~ x ~").voicing(55, 76).s("fm_marimba").delay(0.2));
track("flute", n("<[4 ~ ~ 2 ~ ~ 0 ~] [~ ~ 4 ~ 5 4 ~ ~]>").scale("A:minor").octave(5).s("fm_lead").room(0.4).delay(0.3));
```

## melodic_techno (120-126 BPM)

A deep, rolling four-on-the-floor and a pulsing sixteenth bass, minor arpeggios whose filters slowly open, wide pads,
a long breakdown and a soaring lead that comes back with the kick. Let the arpeggio carry the track and open its
filter across the build.

```js
blank("melodic_techno");
tempo(123);
key("D minor");
prog("A", "i VI III VII");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4, sh*16").gain("1 0.7"));
track("bass", chord("prog").root().octave(1).struct("[~ x x x]*4").s("saw_bass").lpf(900).duck(0.7));
track("arp", chord("prog").struct("x*16").arp("up").voicing(57, 81).s("pluck").lpf(2500).delay(0.35).room(0.4));
track("pad", chord("prog").voicing(45, 74).s("glass_pad").role("pad").room(0.6));
```

## baile_funk (126-136 BPM)

Rio's funk carioca on the tamborzão: a booming low drum in five uneven hits (beat 1, the a of 1, the and of 2, the and
of 3, beat 4), rim slaps answering in its gaps, claps on 2 and 4, an 808 following the kick, chopped shouts and horn
stabs over one or two minor chords. Clip the drums rather than compress them.

```js
blank("baile_funk");
tempo(130);
key("F minor");
prog("A", "i i i VI");
track("drums", s("bd ~ ~ bd ~ ~ bd ~ ~ ~ bd ~ bd ~ ~ ~, ~ cp ~ cp, ~ ~ ~ ~ rim ~ ~ rim ~ ~ ~ rim ~ ~ rim ~").gain("1 0.85").distort(4));
track("808", chord("prog").root().octave(1).struct("x ~ ~ x ~ ~ x ~ ~ ~ x ~ x ~ ~ ~").s("808").legato(0.6).level(4));
track("horns", chord("prog").struct("~ ~ x ~ ~ ~ ~ ~").voicing(52, 74).s("brass_stab"));
```

## breakcore (165-190 BPM)

Breakbeats chopped, rolled and stuttered to pieces, distorted kicks, a deep reese and sub, wistful pads and a piano or
bell melody against the chaos. A `new_song` of breakcore gathers a few sampled breaks before it composes, as jungle's
does; without them its drum lanes play the synthesized kit.

```js
blank("breakcore");
tempo(178);
key("E minor");
prog("A", "i VI III VII");
track("drums", s("bd ~ bd ~ sd ~ ~ sd ~ sd bd sd sd ~ sd sd, hh*16, rd*8").gain("1 0.8").distort(4));
track("bass", chord("prog").root().octave(1).struct("x ~ ~ ~ ~ ~ ~ ~").s("reese").legato(1));
track("pad", chord("prog").voicing(48, 76).s("air_pad").role("pad").room(0.5));
track("bells", n("<[4 ~ 2 ~ 0 ~ 2 ~] [5 ~ 4 ~ 2 ~ ~ ~]>").scale("E:minor").octave(6).s("bell").room(0.4).delay(0.3));
```
