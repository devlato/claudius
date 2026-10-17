# Song structure

Your song so far plays every layer at once from the first bar to the last. Real songs change: they start small,
build, release, breathe, and end. Each stretch of a song with its own set of layers is a **section**, and the plan of
sections is the song's **structure** (or form).

## Sections in dance music

Listen to the song you made on the first page again (`claudius make --genre house --seed 9`) and follow its
sections:

| section | what happens | typical layers |
|---|---|---|
| intro | the song starts small, often only drums | kick, hats |
| groove | the main beat with bass and chords | most of the drums, bass, chords |
| break (breakdown) | the drums stop; space and atmosphere | pad, chords, maybe the melody |
| build | tension rises towards the drop | a snare roll, a rising noise, no kick |
| drop | everything comes back at once | everything |
| outro | the song winds down | kick, hats, bass |

Sections are usually 4, 8 or 16 bars long. Ears expect change at those points: a section of 7 bars feels like
someone tripped.

## Writing the form

`form()` lists the sections in order. Each `sec()` gives the kind of section, its length in bars, and the tracks that
play in it:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("kick",   s("bd*4"));
track("hats",   s("[~ hh]*4"));
track("bass",   chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.5));
track("pad",    chord("prog").s("pad").role("pad"));
form([
  sec("intro", 8, "kick hats"),
  sec("groove", 8, "kick hats bass chords"),
  sec("break", 8, "pad chords"),
  sec("drop", 8, "kick hats bass chords pad"),
]);
```

Thirty-two bars, about a minute. Listen to how much bigger the drop feels after the break, though it adds only the
pad to the groove. Taking things away does as much as adding them.

## Energy

Every section has an **energy**, from 0 (silent) to 1 (as intense as the song gets). claudius gives each kind a
default (an intro 0.4, a break 0.35, a drop 0.95) and the critic checks that the song really gets louder where the
energy rises. You can set it with `.energy(0.3)` when a section is quieter than its kind suggests.

## Making the drop land

Four small touches turn a list of sections into a song that flows. Add them as methods on a section:

- `.filter_in()` on the intro: the music starts muffled and opens up across the section, like walking into the room
  where the party is. (The drums stay clear.)
- `.build()` on the section before the drop: a rising noise (a **riser**) and a sweep that thins out the low end,
  both growing until the drop.
- `.gap()` on that same section: one beat of silence at its very end. The drop hits harder after a breath.
- `.impact()` on the drop: a crash cymbal and a boom on its first beat.

A drummer would also play a **snare roll** into the drop: hits getting faster and louder. Make one as its own track,
one bar of each speed, and play it only in the build:

```js
blank("house");
tempo(120);
track("roll", s("<sd*2 sd*4 sd*8 sd*16>").gain("<0.5 0.65 0.8 1>"));
form([sec("build", 4, "roll").build()]);
```

Two hits in bar 1, four in bar 2, eight, then sixteen, getting louder each bar. Since the angle brackets start over
in every section, the roll always begins with the build.

## A second progression

The break is a good place for different chords. Sections play progression `A` unless they say otherwise, and breaks
play progression `B` when the song has one (until now, yours did not, so the break played `A`). Give the song a `B`:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
prog("B", "F G Am Am");
bars_per_chord(1);
track("pad", chord("prog").s("pad").role("pad"));
form([sec("groove", 8, "pad"), sec("break", 8, "pad")]);
```

The same pad track plays A in the groove and B in the break, because `chord("prog")` follows whatever progression
the section plays, and so does a bass written with `.root()`. The melody, though, was written note by note for
progression `A`, so keep it out of the break, or tell a section to play `A` with `.prog("A")`.

## The song so far

The whole song: 60 bars, two minutes at 120 BPM.

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
```

The pad plays in the intro so the opening filter has something to open: the drums are never filtered. The build has
no kick: taking the kick away for four bars makes its return in the drop the biggest moment of the song. A section of
kind `build` gets the riser and the sweep without being asked; `.gap()` adds the breath.

## Try this

- Make the break longer (16 bars) and put the melody in it with `.prog("A")`.
- Add a short break and a second drop before the outro: `sec("break", 4, "pad")` and a copy of the drop line. A
  second section with the same name is renamed `drop2` automatically. (`"*"` instead of a list means every track, which
  here would include the roll.)
- Remove `.gap()` and listen to the drop again. Then put it back.

[Form and sections](../../language/form.md) explains every section setting exactly.
