# Chords and progressions

A **chord** is several notes played at the same time. Chords carry the mood of a song more than anything else: the
same beat sounds hopeful over one chord sequence and sad over another.

## Three notes at once

On the last page, `n("0 2 4")` played A, C and E one after another. Play them together and you have a chord:

```js
blank("house");
tempo(120);
key("A minor");
track("arpeggio", n("0 2 4 ~").octave(3).s("epiano"));
track("chord",    n("[0, 2, 4] ~ ~ ~").octave(3).s("epiano"));
```

The comma inside the brackets stacks the notes instead of playing them in turn. A chord built from every other note of
the scale (1st, 3rd and 5th) is a **triad**, the basic chord of almost all Western music.

## Major and minor

Triads come in two main colours. Listen to these two chords, one bar each:

```js
blank("house");
tempo(120);
track("chords", chord("<C Cm>").s("epiano"));
```

`C` is **C major**: bright and stable. `Cm` is **C minor**: darker. The only difference is the middle note, one
semitone lower in the minor chord. In chord names, a capital letter alone is major (`F`, `G`), and `m` after the
letter means minor (`Am`, `Dm`).

## A progression

A sequence of chords that repeats is a **progression**. The bass from the last page already played the roots of
ours: A, F, C, G. Here are the chords themselves:

```js
blank("house");
tempo(120);
key("A minor");
track("chords", chord("<Am F C G>").s("epiano"));
track("bass",   note("<a1 f1 c2 g1>").struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
```

A minor, F major, C major, G major, one bar each. This is one of the most used progressions in pop and dance music:
it starts sad, lifts, and the G at the end pulls back to the beginning. The bass plays each chord's root at the
start of its bar, so the two agree.

## Telling the song its chords

Writing the chords inside a pattern works, but claudius can do more when the song itself knows its progression. Set
it once with `prog()`, and play it with `chord("prog")`:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("chords", chord("prog").s("epiano"));
```

`prog("A", ...)` names the song's main progression `A`. `bars_per_chord(1)` makes each chord last one bar (a blank
song starts that way; a 2 would hold each chord for two bars). `chord("prog")` plays whatever chord the progression
has at each moment. Now the song's harmony is in one place: other parts can follow it, claudius checks your melody
against it, and a second progression for another section is one more line.

## A bass that follows

The bass can follow the progression too. `.root()` makes a chord pattern play only each chord's **root**, the note
the chord is named after (A for A minor), down in the bass register:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("chords", chord("prog").s("epiano"));
track("bass",   chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
```

The bass plays A, F, C and G, the same rhythm as before, now taken from the progression. Change the chords in
`prog()` and the bass changes with them.

## Rhythm for chords

A held chord is calm. Chords played in a rhythm, called **stabs**, drive the groove:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("kick",   s("bd*4"));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.5));
```

The rhythm `x ~ ~ x ~ ~ x ~` puts three chords in each bar, unevenly spaced (3 + 3 + 2 eighth notes). It is one of the
oldest rhythms in dance music, and it rubs nicely against the steady kick.

You did not choose which A, C and E to play, or in which order. claudius picks the notes of each chord so they move as
little as possible from the chord before. Musicians call that **voice leading**, and it is why the chords sound
connected rather than jumping around.

## A pad underneath

A **pad** is a soft, long sound holding the chords in the background. It fills the space and makes quiet sections feel
full:

```js
blank("house");
tempo(120);
key("A minor");
prog("A", "Am F C G");
bars_per_chord(1);
track("pad", chord("prog").s("pad").role("pad"));
```

`.role("pad")` tells the mixer what this track is, so it sits quietly behind everything else. You will see roles
again on the [mixing page](mixing.md).

## The numbers in the progression

If you print your song as a script (you will on the [last page](finishing.md)), the progression shows up as
`prog("A", "i VI III VII")`. These are **Roman numerals**: they name each chord by its place in the key. `i` is the
chord on the first note of the scale (A in A minor), `VI` the chord on the sixth (F), and so on; lower case means a
minor chord, capitals a major one. Written this way, the progression works in any key: in D minor, `i VI III VII` is
Dm, B♭, F, C.

## Try this

- **Seventh chords** add one more note on top and sound richer, jazzier: `prog("A", "Am7 Fmaj7 Cmaj7 G7")`.
- A **brighter** progression in the same key: `prog("A", "F G Am Am")`.
- Hold each chord for two bars with `bars_per_chord(2)`. The chords, the pad and the bass all follow.
- Arpeggiate the chords, playing their notes one after another:
  `chord("prog").struct("x*4").arp("up").s("pluck")`.

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
```

[Pitch](../../language/pitch.md#chord-symbols) lists every chord name claudius understands.
