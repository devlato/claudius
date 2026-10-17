# Bass lines

The bass is the lowest part of a song. It does two jobs at once: it gives the beat weight (you feel it as much as hear
it), and it tells your ear which chord the music is on, by playing that chord's lowest note. A good bass line locks
to the kick drum and moves with the chords.

## Low notes

Bass notes live in octaves 1 and 2. Compare:

```js
blank("house");
tempo(120);
track("high", note("a3 ~ ~ ~").s("sub_bass"));
```

```js
blank("house");
tempo(120);
track("low", note("a1 ~ ~ ~").s("sub_bass"));
```

Both are the note A. The second is two octaves lower, and on good headphones or speakers you feel it more than you
hear it. `sub_bass` is a pure, round synthesizer bass, which is why it nearly disappears on laptop speakers.

## One note per bar

Our song will move through four chords, one per bar: A minor, F, C and G. You will build those chords on the next
page. The bass plays the note each chord is named after, its **root**: A, F, C, G.

```js
blank("house");
tempo(120);
key("A minor");
track("kick", s("bd*4"));
track("bass", note("<a1 f1 c2 g1>").s("sub_bass"));
```

The angle brackets `< >` mean "one per bar, in turn": A in bar 1, F in bar 2, C in bar 3, G in bar 4, then A again.
Each note lasts its whole bar. Listen to how the music seems to change mood every bar, even without chords: that is
the bass doing its second job.

`c2` is higher than `a1` and `g1`, so the line moves down, up, down without big jumps: A, then F a little below it, C
above, G below again. Keeping a bass line in a small range makes it smooth.

## Rhythm

A bass that holds one note per bar is a drone. Give it a rhythm with `.struct()`, which takes the notes from one
pattern and the timing from another:

```js
blank("house");
tempo(120);
key("A minor");
track("kick", s("bd*4"));
track("bass", note("<a1 f1 c2 g1>").struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
```

`"x ~ ~ x ~ x ~ ~"` divides the bar into eight steps, two per beat, and plays on three of them. Count *one and two
and three and four and*: the notes fall on *one*, on the *and* after two, and on the *and* after three. Each `x` plays
whatever note the bar has. The first note lands with the kick on beat one, so you hear the chord change exactly when
the bar starts; the other two fall between kicks, which is what makes the line groove.

## Note length

Each note lasts until the next step unless you shorten it. `.legato(0.5)` makes every note half as long, so the bass
bounces instead of sliding from note to note:

```js
blank("house");
tempo(120);
key("A minor");
track("kick", s("bd*4"));
track("bass", note("<a1 f1 c2 g1>").struct("x ~ ~ x ~ x ~ ~").s("sub_bass").legato(0.5));
```

## Bass sounds

| sound | character |
|---|---|
| `sub_bass` | pure and deep, almost felt rather than heard |
| `organ_bass` | round with some body; the classic house bass |
| `saw_bass` | buzzy and present, cuts through on small speakers |
| `808` | a deep boom that drops in pitch: hip-hop and trap |
| `reese` | two detuned saws, moving and dark: drum and bass |

Swap `sub_bass` for each and listen. On laptop speakers, `organ_bass` and `saw_bass` come through much more clearly,
because they have higher overtones on top of the low note.

## Try this

- The house **offbeat bass**: `.struct("~ x ~ x ~ x ~ x")`. The bass plays on every *and*, between the kicks, and the
  kick and bass take turns.
- **Octave jumps**: `note("<[a1 a2]*4 [f1 f2]*4 [c2 c3]*4 [g1 g2]*4>").s("saw_bass").legato(0.5)` jumps between the
  root and the same note an octave up, four times a bar: an eighties sound.
- Start the bass a sixteenth late with `.late(0.0625)` and hear the groove lean back.

## The song so far

```js
blank("house");
title("First Light");
tempo(120);
key("A minor");
track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
track("bass",   note("<a1 f1 c2 g1>").struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
```
