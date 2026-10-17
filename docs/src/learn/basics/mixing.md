# Mixing

**Mixing** is making every part of a song audible and placing it: how loud each track is, how far left or right, how
close or far away. A good mix lets you follow any part you choose to listen to.

claudius does a first mix for you. This page shows what it does and how to adjust it.

## Roles: the automatic balance

Every track has a **role** that says what it is: `kick`, `snare`, `hat`, `bass`, `chords`, `pad`, `lead` and so on.
Each role has a loudness it is usually mixed at, and claudius brings every track to that loudness. Kick and bass are
loud, hats and shakers quieter, pads quieter still, behind everything.

Most tracks get the right role on their own: a track playing `bd` is a kick, one playing `sub_bass` is a bass, a
`chord()` pattern is chords. A melodic track with nothing else to go on is treated as the lead. That is why the pad
had `.role("pad")` on the [chords page](chords.md): without it, the pad would be mixed as loud as a melody.

## Level

To change how loud a track is, give it a **level** in decibels (dB), relative to where its role puts it:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("kick", s("bd*4"));
track("pad",  chord("prog").s("pad").role("pad"));
mix("pad", #{ level: -3 });
```

`level: -3` is three decibels quieter than usual for a pad. Decibels are how loudness is measured: 1 dB is about the
smallest change you can notice, 3 dB is a clear step, and 10 dB sounds roughly half as loud. Use small numbers;
anything beyond ±6 usually means the track has the wrong role or the wrong sound.

## Left and right

**Pan** places a track between the left speaker (−1) and the right (1). Kick, bass, snare or clap and the lead stay in
the middle; hats, percussion and other small parts spread out to the sides, which makes room in the centre:

```js
blank("house");
tempo(120);
track("kick",   s("bd*4"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
mix("hats",   #{ pan: 0.2 });
mix("shaker", #{ pan: -0.3 });
```

Listen on headphones: the hats tick slightly to the right, the shaker to the left.

## Space: reverb and delay

**Reverb** is the sound of a room: the reflections that make a sound seem far away. **Delay** is a distinct echo.
Each track sends some of itself to the song's reverb and delay, from 0 (none) to 1 (a lot):

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.5));
mix("chords", #{ reverb: 0.3 });
```

Short, plucky sounds (stabs, melodies) and pads like reverb; kick and bass sound best dry, close and solid. Too much
reverb on everything makes a mix sound washed out and far away.

## Ducking under the kick

In dance music, the bass and chords get quieter for a split second every time the kick hits, then swell back. This
is called **sidechain** ducking, and it gives the kick room and the whole track a pumping, breathing feel. Each track
has a `duck` amount from 0 (none) to 1 (the track nearly vanishes on every kick):

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("kick", s("bd*4"));
track("pad",  chord("prog").s("pad").role("pad"));
mix("pad", #{ duck: 0.7 });
```

Listen for the pad pulsing with the kick. Pattern tracks start at 0.3, which you hear as a gentle movement; 0.5 is
clearly pumping, 0.8 is the exaggerated pump of future bass.

## Listening to one thing

While mixing, it helps to hear one track alone. `solo()` plays only the soloed tracks, `mute()` silences one:

```js
blank("house");
tempo(120);
key("A minor");
track("kick", s("bd*4"));
track("bass", note("<a1 f1 c2 g1>").struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
solo("kick");
solo("bass");
```

This renders only the kick and the bass, so you can hear whether they fit together. Remove the `solo()` lines before
your final render; the critic judges only what plays.

## Try this

- Turn the melody up 2 dB and down 2 dB, rendering each time. Which one lets you hear the chords?
- Give the bass `duck: 0` and then `duck: 0.6`, and listen to the low end with the kick.
- Put a lot of reverb on the clap (`reverb: 0.5`) and hear it move back into a big room.

## The song so far

The mix lines go at the end of the script, after `form()`:

```js
blank("house");
title("First Light");
tempo(120);
key("A minor");
prog("A", "Am F C G");
prog("B", "F G Am Am");
bars_per_chord(1);

track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
track("roll",   s("<sd*2 sd*4 sd*8 sd*16>").gain("<0.5 0.65 0.8 1>"));
track("bass",   chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.5));
track("pad",    chord("prog").s("pad").role("pad"));
track("melody", n("<[4 ~ 2 ~ 0 ~ 2 4] [5 ~ 4 ~ 2 ~ ~ ~] [4 ~ 2 ~ 4 ~ 6 4] [6 ~ 4 ~ 3 ~ ~ ~]>")
                  .octave(4).s("pluck").delay(0.2));

form([
  sec("intro", 8, "kick hats pad").filter_in(),
  sec("groove", 16, "kick clap hats shaker bass chords"),
  sec("break", 8, "pad chords"),
  sec("build", 4, "clap hats pad roll melody").gap(),
  sec("drop", 16, "kick clap hats shaker bass chords pad melody").impact(),
  sec("outro", 8, "kick hats bass"),
]);

mix("hats",   #{ pan: 0.2 });
mix("shaker", #{ pan: -0.3 });
mix("bass",   #{ duck: 0.5 });
mix("chords", #{ reverb: 0.25, duck: 0.4 });
mix("pad",    #{ level: -3, reverb: 0.4 });
mix("melody", #{ reverb: 0.3 });
```

[Mixing functions](../../language/mixing.md) has every setting and the loudness of every role.
