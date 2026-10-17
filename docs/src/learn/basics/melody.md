# Melody

The **melody** is the part of a song you could sing back: one note at a time, on top of everything else. In
electronic music it is often called the **lead**, or the **hook** when it is the catchiest bit.

## Notes from the key

Write melodies with `n()`: every number is a note of the key, so you cannot hit a wrong note.
In A minor:

| number | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| note | A | B | C | D | E | F | G | A (higher) |

A first idea, with eight steps in the bar:

```js
blank("house");
tempo(120);
key("A minor");
track("melody", n("4 ~ 2 ~ 0 ~ 2 4").octave(4).s("pluck"));
```

E, C, A, C, E: it falls and climbs back. `.octave(4)` puts the melody above the chords; `pluck` is a short, bright
synth sound that cuts through.

## Fitting the chords

Play that same bar over all four chords and something sounds off, most of all over the G in bar 4. The reason: each
chord is made of three notes, and a melody note that is part of the chord underneath sounds settled, while one that
isn't sounds tense. A little tension is good; too much on the strongest beats sounds like a mistake.

The strong beats are **beat 1 and beat 3** of each bar, steps 1 and 5 when the bar has eight steps. Here are the
chord notes of our progression as numbers:

| chord | notes | numbers |
|---|---|---|
| Am | A C E | 0 2 4 |
| F | F A C | 5 0 2 (or 7 9) |
| C | C E G | 2 4 6 |
| G | G B D | 6 1 3 (or 8 10) |

So write each bar for its chord, with a chord note on steps 1 and 5:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("chords", chord("prog").s("epiano"));
track("melody", n("<[4 ~ 2 ~ 0 ~ 2 4] [5 ~ 4 ~ 2 ~ ~ ~] [4 ~ 2 ~ 4 ~ 6 4] [6 ~ 4 ~ 3 ~ ~ ~]>")
                  .octave(4).s("pluck"));
```

Each pair of square brackets is one bar, and the angle brackets play them in turn. Check bar 2 against the table: F
on beat 1 (5) and C on beat 3 (2), both notes of the F chord. The notes in between (the `4` in bar 2 is E) pass
through, which is what makes a line move.

## Repetition

Listen to the four bars again: bars 1 and 3 start the same way (4, then 2), and bars 2 and 4 both end with a held
note. Repeating a short idea, a **motif**, with small changes is how melodies become memorable. Four bars that are all
different are hard to remember; four identical bars are boring. Aim between.

Rests matter as much as notes: the held note at the end of bars 2 and 4 is a breath, and it splits the four bars
into a question and an answer.

## Echo

A **delay** repeats a sound a moment later, quieter, like an echo in a canyon. It makes a short melody sound wider and
fills the gaps:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("melody", n("<[4 ~ 2 ~ 0 ~ 2 4] [5 ~ 4 ~ 2 ~ ~ ~] [4 ~ 2 ~ 4 ~ 6 4] [6 ~ 4 ~ 3 ~ ~ ~]>")
                  .octave(4).s("pluck").delay(0.2));
```

`.delay(0.2)` sends a fifth of the sound to the song's echo. The echoes come in time with the beat.

## Try this

- Play the melody an octave higher every fourth bar: `.every(4, |x| x.add(7))`. Adding 7 moves every note up seven
  steps of the scale, which is one octave, so it still fits the chords.
- Swap the sound: `bell`, `square_lead`, or with the sample packs installed `piano`, `vibes`, `flute`.
- Add a quieter copy an eighth note behind, an octave up: `.off(0.125, |x| x.add(7).gain(0.5))`.
- Write your own four bars using the table. Put chord notes on steps 1 and 5, and anything from the key in between.

## The song so far

```js
blank("house");
title("First Light");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);

track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
track("bass",   chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.5));
track("pad",    chord("prog").s("pad").role("pad"));
track("melody", n("<[4 ~ 2 ~ 0 ~ 2 4] [5 ~ 4 ~ 2 ~ ~ ~] [4 ~ 2 ~ 4 ~ 6 4] [6 ~ 4 ~ 3 ~ ~ ~]>")
                  .octave(4).s("pluck").delay(0.2));
```

Render it. Every layer plays all the time, for eight bars: it sounds like the loudest moment of a song, with nothing
leading up to it. The next page fixes that.
