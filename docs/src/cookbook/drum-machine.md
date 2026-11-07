# The drum machine

A drum lane is a step sequencer, and each step can do more than strike. [`trig()`](../language/song-functions.md#trig)
gives a step what a drum machine's trig gives it:

- a chance to play, and a condition for when;
- a ratchet of strokes inside it, and a flam;
- a nudge off the grid;
- and what it changes in the sound for that stroke: its pitch, decay, tone and pan.

A lane can also keep a time and feel of its own. [`lane()`](../language/song-functions.md#lane) gives it a length (a
polymeter against the others), a speed, a swing and a groove, and [`groove()`](../language/song-functions.md#groove)
takes a groove from a record. A beat you play or beatbox, or a drum loop, becomes the lanes' steps
([Beatbox a beat](#beatbox-a-beat)).

In the studio, right-click a step in the [step sequencer](../getting-started/studio.md#the-panels) and open **Trig**,
or right-click a lane for its time and feel. Each recipe below starts from a generated song and changes only its
drums.

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

## Hats in twelve

A lane of its own length comes round on its own cycle. Hats twelve steps long start their pattern again every three
beats while the kick keeps four, so the accent wanders across the bar and lands back on the downbeat after three bars.
Percussion ten steps long turns against both.

```js
song("techno", 7);
drum("hat", "x.xx.xx.x.x.x.xx");
lane("hat", #{ length: 12 });
lane("perc", #{ length: 10 });
```

## Triplets on a straight beat

A lane's speed scales its steps. At 0.75 its sixteenths become eighth-note triplets, three to a beat, and at 1.5
sixteenth-note triplets, over drums that stay straight.

```js
song("trap", 5);
drum("perc", "x.xx.x.xx.x.x.xx");
lane("perc", #{ speed: 0.75 });
lane("ohat", #{ speed: 1.5 });
```

## Swing on one lane

The song can play straight while one lane swings: hats at 62% over a kick and clap on the grid. `swing: 0` gives the
lane the song's swing again.

```js
song("house", 3);
swing(0.5);
lane("hat", #{ swing: 0.62 });
lane("shaker", #{ swing: 0.58 });
```

## A dragged feel

`dilla` drags each off-beat and the backbeat late by its own amount and leans on the downbeats, the way a drummer
plays behind the beat. Lanes take as much of a groove as `groove_amount` gives them: the hats all of it and the snare
a little less, so the backbeat drags less than the hats.

```js
song("lofi", 2);
lane("hat", #{ groove: "dilla" });
lane("snare", #{ groove: "dilla", groove_amount: 0.6 });
lane("kick", #{ groove: "dilla", groove_amount: 0.4 });
```

## Behind and ahead of the beat

J Dilla's drunk feel was not random: on an MPC he moved whole pads with its shift timing, the snare a few dozen
milliseconds late on every backbeat, the kick a little early, the hats left on the grid, so that the parts pull
against each other the same way in every bar. `late` does that to a lane, or to a part: every hit that many
milliseconds behind the beat (ahead, below zero), at any tempo. The genres that play that way do it already, a
neo-soul snare 18 to 38 ms behind and its bass 12 to 30, a boom bap snare 8 to 24; `lane()` and `part()` set
your own.

```js
song("neo_soul", 4);
lane("snare", #{ late: 32 });    // laid back
lane("kick", #{ late: -8 });     // pushed
part("bass", #{ late: 20 });     // the bass behind the drums
humanize(0.6);                   // and each hit drifts besides, as a player's does
```

`humanize()` is the other half: a drift, not a jitter. Each hit moves with the ones around it, behind for a while and
ahead for a while, as a drummer's does (see [`humanize()`](../language/song-functions.md#humanize)), up to about
15 ms at 1. In the studio, a lane's settings in the sequencer have a **feel** slider for its `late`.

## A groove from a record

A break, a loop or a whole song has a feel of its own: how late its off-beats come, whether its backbeat sits behind
the beat, which sixteenths are hit hardest. [`groove()`](../language/song-functions.md#groove) measures that in a
recording's drums and keeps it with the song, and the lanes play in it as in one of claudius' own. In the studio,
choose the file under **New song**, **From a recording**, then **Put the song's drums in its groove** and **Take
its groove**. An assistant calls `extract_groove`.

```js no-run
song("lofi", 2);
groove("break", "loops/Funk Break 96bpm.wav");     // the tempo in the name is the one it is measured at
lane("hat", #{ groove: "break" });
lane("snare", #{ groove: "break", groove_amount: 0.8 });
```

The tempo matters most. claudius hears it from the drums when it isn't given, but a loop's tempo heard can come out
twice or half what it is, and then the groove is measured against the wrong sixteenths. A tempo in the file's name or
a third argument (`groove("break", "loops/break.wav", 96)`) is used as it is, and `extract_groove` and the studio
also take an Apple Loop's from GarageBand's catalogue.

## Beatbox a beat

The quickest way to a beat of your own is to make it with your mouth. Record a take over a section with the studio's
**Record** button: a kick ("b"), a snare ("k" or "pf") and hats ("ts"), a bar or two round and round. Then right-click
the take in the arrangement, choose **Turn it into drums** and a length. claudius names each hit, puts it on the song's
grid, folds the times round into the pattern, writes it into the kick, snare (or clap) and hat lanes, and mutes the
take. A hat you did only every other time round plays half the time.

The lanes then take everything above: a ratchet on the hats, a groove, a length of their own. An assistant does the
same with `transcribe_drums`, and a drum loop goes in as a file:

```js no-run
song("lofi", 2);
drums_from("loops/Lazy Break 85bpm.wav", #{ bars: 2 });
lane("hat", #{ groove: "dilla" });
trig("hat", 14, #{ ratchet: 3, ramp: 1 });
```

It names a kick, a snare and a hat well when the sounds are clear; check what it wrote in the step sequencer, as a
beatboxed snare's release can come out as a kick a step later.
