# The drum machine

A drum lane is a step sequencer, and each step can do more than strike. [`trig()`](../language/song-functions.md#trig)
gives a step what a drum machine's trig gives it:

- a chance to play, and a condition for when;
- a ratchet of strokes inside it, and a flam;
- a nudge off the grid;
- and what it changes in the sound for that stroke: its pitch, decay, tone and pan.

In the studio, right-click a step in the [step sequencer](../getting-started/studio.md#the-panels) and open **Trig**.
Each recipe below starts from a generated song and changes only its drums.

## Hats that roll

Trap's hats move in rolls. A ratchet puts several strokes inside one sixteenth, and a ramp makes them swell into the
next beat or fade away from it. A roll on the condition `"1:2"` comes every other time round the pattern, so the
four-bar loop doesn't repeat itself exactly.

```js
song("trap", 5);
trig("hat", 6, #{ ratchet: 4, ramp: 1 });                   // 64ths swelling into beat 3
trig("hat", 30, #{ ratchet: 3, ramp: -1 });                 // a triplet fading out of bar 2
trig("hat", 46, #{ ratchet: 6, ramp: 1, cond: "1:2" });     // a long roll, every other time round
trig("perc", 14, #{ pitch: 5, flam: 12 });
```

## A turnaround in the last bar

A condition counts the times round a pattern from each section's start, so `"last"` plays only in the bar before the
next section. That's where a drummer puts a fill: a clap pickup, a kick, a doubled hat. `"4:4"` plays every fourth
time round, closing each four-bar phrase.

```js
song("house", 3);
trig("clap", 15, #{ v: 0.6, cond: "last" });
trig("kick", 14, #{ v: 0.7, cond: "last" });
trig("hat", 14, #{ ratchet: 2, cond: "4:4" });
trig("ohat", 10, #{ prob: 0.5, decay: 1.6 });                // now and then, longer
```

## Loose and late

Lofi leans back. Snares nudged a little late of the grid drag, ghost notes that play only some of the time keep the
groove from looping, and a flam thickens the backbeat. The chance is drawn afresh each time round, but it's the same
in every render of the song, so what you like stays.

```js
song("lofi", 2);
trig("snare", 4, #{ nudge: 0.12 });
trig("snare", 12, #{ nudge: 0.12, flam: 14 });
trig("snare", 7, #{ v: 0.3, prob: 0.5 });                   // a ghost, half the time
trig("hat", 11, #{ v: 0.4, prob: 0.6, nudge: 0.2 });
```

## Hats that move

Locks change the sound for one stroke. Hats panned left and right of each other, one darker, one shorter, make a
two-step pattern that moves in the stereo field.

```js
song("dnb", 4);
trig("hat", 2, #{ pan: -0.5, tone: 0.3 });
trig("hat", 6, #{ pan: 0.5, tone: -0.3 });
trig("hat", 10, #{ pan: -0.5, decay: 0.6 });
trig("hat", 14, #{ pan: 0.5, decay: 0.6 });
trig("snare", 4, #{ flam: 10 });
trig("snare", 12, #{ flam: 10 });
```

## Percussion by chance

Techno's percussion can be generative: steps that play only sometimes, each at its own pitch, never come round the same
way twice in a row. A ride that doubles every time round but the first lets the section in clean.

```js
song("techno", 7);
for s in [1, 3, 9, 11] {
    trig("perc", s, #{ prob: 0.35, pitch: s % 3 - 1 });
}
trig("ride", 14, #{ ratchet: 2, cond: "!first" });
```

Judged without the listening models, these five render at 95 to 97 out of 100, as their songs do without the trigs.
