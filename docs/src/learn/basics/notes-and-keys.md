# Notes, scales and keys

Drums mostly don't have a pitch: a kick is a thump, not a note. Bass, chords and melody do. Before writing them, you
need three ideas: notes, scales and keys.

## Notes

Play this and listen to the sound climbing:

```js
blank("house");
tempo(120);
track("up", note("c4 d4 e4 f4 g4 a4 b4 c5").s("epiano"));
```

Each sound is a **note**, and how high or low it sounds is its **pitch**. Notes are named with the letters `a` to
`g`. After `g` the names start again at `a`, and the number says which round you are in, the **octave**: `c4` to
`c5` is one octave up. Listen to the first and last note: they sound like the same note, one higher. That sameness
is why they share a name.

`c4` is middle C, the C in the middle of a piano. Low notes for bass sit around octave 1 and 2 (`a1`, `e2`), chords
around 3 and 4, melodies around 4 and 5.

## Semitones, sharps and flats

The eight notes above are the white keys of a piano. Between most of them there is a black key, a note halfway
between. The smallest step between two neighbouring notes, white or black, is a **semitone**. The in-between notes
are written with `#` (**sharp**, a semitone up) or `b` (**flat**, a semitone down): `c#4` is the black key directly above
`c4`, and it is the same note as `db4`. There are twelve semitones in an octave.

```js
blank("house");
tempo(120);
track("all twelve", note("c4 c#4 d4 d#4 e4 f4 f#4 g4 g#4 a4 a#4 b4").s("epiano"));
```

Every note in that bar is a semitone higher than the one before. It sounds like stepping up a staircase, with no
sense of where it is going. Music usually picks fewer notes than that.

## Scales

A **scale** is a selection of seven (sometimes five) of the twelve notes that sound good together. Listen to two:

```js
blank("house");
tempo(120);
track("major", n("0 1 2 3 4 5 6 7 6 5 4 3 2 1 0 ~").scale("C:major").octave(4).s("epiano"));
```

```js
blank("house");
tempo(120);
track("minor", n("0 1 2 3 4 5 6 7 6 5 4 3 2 1 0 ~").scale("A:minor").octave(3).s("epiano"));
```

The first is **C major**: bright, settled, the "do re mi" you know. The second is **A minor**: darker, a little sad.
They use exactly the same seven notes, the white keys, but A minor starts and ends on A. The note a scale is built on
and comes home to is its **tonic**, and it changes the whole mood.

`n()` plays a scale by numbers instead of note names: 0 is the tonic, 1 the next note of the scale, and so on; 7 is
the tonic an octave up. That is why the same numbers gave two different tunes: they were counted in two scales.

## Keys

A song's **key** is the scale it uses: "A minor" means the A minor scale, with A as home. Set it for the whole song:

```js
blank("house");
tempo(120);
key("A minor");
track("tune", n("0 2 4 2 0 ~ ~ ~").octave(4).s("epiano"));
```

`n()` without `.scale()` uses the song's key. The numbers 0, 2 and 4 skip every other note of the scale (A, C, E),
which is a pattern you will meet again on the [chords page](chords.md).

Writing notes as numbers in the key has two advantages. Every number is in the scale, so nothing sounds wrong by
accident. And if you change the key, everything written this way moves with it: change `key("A minor")` to
`key("D minor")` and render again. The tune is the same, only lower.

## Try this

- Play the five-note **pentatonic** scale, which sounds good in almost any order:
  `n("0 1 2 3 4 3 2 1").scale("A:minor:pentatonic").octave(4)`. Shuffle the numbers; it still works.
- Try `key("A major")` with the `tune` above and hear the same numbers turn bright.
- Write a short tune of your own with numbers from 0 to 7 and `~` for rests. Start and end on 0 and it will sound
  finished.

## The song so far

The drums from the last page, now in A minor. You won't hear a difference yet: the key matters from the next page on.

```js
blank("house");
title("First Light");
tempo(120);
key("A minor");
track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
```

[Pitch](../../language/pitch.md) has every scale name and the exact rules for notes.
