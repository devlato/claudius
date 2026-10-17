# claudius for DAW users

If you work in Ableton, Logic, FL Studio or Bitwig, most of claudius will be familiar: tracks, an arrangement in
sections, a mixer with sends and a sidechain, a master chain with a limiter. What differs is how you edit them. There
is no free timeline of clips: the arrangement is a grid of sections and tracks (the studio lets you drag sections and
paint which tracks play in each), and the notes come from patterns and parts. You describe the song (in a script, in
the studio's panels, or through an assistant), claudius arranges, renders and judges it, and you listen and revise. A
render of a three-minute song takes 10 to 40 seconds.

## Where things are

| in a DAW | in claudius |
|---|---|
| project | a song: its *blueprint* (`blueprint.json`, the editable description) and the *project* it arranges into (`project.json`, notes and clips on a timeline) |
| arrangement view | the form: [sections](../../language/form.md) in order, each listing the tracks that play in it |
| clip or region | a track in a section; it plays from the section's first bar |
| MIDI clip and piano roll | a [pattern](../../language/patterns.md) (`note()`, `n()`, `chord()`), or a part the theory engine writes |
| drum rack and step sequencer | a [drum lane](../../language/song-functions.md#drum) with step strings and named variants, or an `s()` pattern |
| instrument | a [sound](../../language/sounds.md): a synth preset, a sampled instrument, a drum kit, an audio file |
| mixer channel | [`mix()`](../../language/mixing.md#mix): level, pan, two sends, sidechain depth, mute, solo |
| channel inserts | filters, bitcrusher and saturation on pattern tracks; the genre's chains elsewhere |
| groups and buses | four fixed buses: `drums`, `bass`, `music`, `fx` |
| return tracks | one reverb and one delay return, set by the genre |
| sidechain compressor | `duck` on each track, keyed from the kick |
| master chain and limiter | the genre's master chain and a loudness stage aimed at a LUFS target ([`master()`](../../language/mixing.md#master)) |
| automation | section flags: a lowpass opening in an intro, a highpass sweep in a build, a lowpass closing the outro |
| markers | section names |
| groove templates | [`swing()`](../../language/song-functions.md#swing) (sixteenth swing) and [`humanize()`](../../language/song-functions.md#humanize) |
| export | `song.wav` (24-bit, 48 kHz), `song.mp3`, and stems with `--stems` |
| undo history | undo and redo in the studio; every song also records its lineage of edits |

There is one tempo, one key and one time signature (4/4) per song. There is no MIDI input, MIDI export or live
recording.

## The same groove, two ways

A house groove programmed in a step sequencer:

```js
blank("house");
tempo(124);
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("hat",  "..x...x...x...x.");
drum("hat", "sparse", "......x.......x.");
```

And in pattern form:

```js
blank("house");
tempo(124);
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
```

They sound alike but behave differently. Drum lanes are what the generator writes: they have named variants that
sections switch between (`main`, `sparse`, `alt`), and the arranger rewrites them for builds (a snare roll, the kick
dropping out) and phrase-end fills. Pattern tracks are never rewritten; they give you the whole pattern language
(alternation, euclidean rhythms, randomness, sampled kits such as `tr909:bd`), and you write the variation yourself.
To put a drum lane on a recorded kit, [`drum_sound("kick", "tr909:bd")`](../../language/song-functions.md#drum_sound)
swaps its sound and keeps its pattern.
Most of the [cookbook](../../cookbook/four-on-the-floor.md) uses patterns.

## Levels are relative

A DAW fader is an absolute gain. In claudius every track has a [role](../../language/mixing.md#roles) (`kick`,
`bass`, `pad`, `lead` ...), every role has a target loudness, and each track is gained to its target while it plays.
`level` is an offset from that target. A pad at `level: -3` sits 3 dB below where a pad normally sits, whatever its
patch or velocity, and adding a track does not change the balance of the others.

The consequence: gain staging is done for you, and the first mix decision is the role. A track's role comes from its
sound: drums and percussion find theirs, a bass preset or a low line is `bass`, a chord pattern is `chords`, and
anything else melodic is treated as a `lead`. Set `.role("pad")` or `.role("arp")` on pattern tracks that are not
leads.

## Sections replace clips

A DAW lets every clip differ. In claudius a track plays the same material in every section it is in, starting from
the section's first bar: an alternation starts again from its first item, `every(4, ...)` counts from the section,
the progression starts on its first chord. To vary a part in one section, make a second track and put it only there:

```js
blank("house");
tempo(124); key("A minor"); prog("A", "Am7 Fmaj7 Dm7 Em7"); bars_per_chord(1);
track("drums",  s("bd*4, [~ cp]*2, [~ hh]*4"));
track("keys",   chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano"));
track("keys16", chord("prog").struct("x*16").legato(0.4).s("epiano"));   // the busy version, drop only
form([sec("groove", 16, "drums keys"), sec("drop", 16, "drums keys16")]);
```

## Harmony is written once

A DAW has no idea what key you are in. claudius does: the song has a key and named progressions in Roman numerals
(`prog("A", "i7 VImaj7 iv7 v7")`), sections choose a progression, and chord patterns, generated parts and the critic
all read it. Write `chord("prog")` and the chords follow whatever progression the section plays; change the key and
everything written in numerals or scale degrees moves with it. See [Harmony](harmony.md).

## The loop you work in

1. Write or change the song: a script, an edit in the studio, or a request to an assistant.
2. claudius arranges and renders it (10 to 40 seconds for a full song), then judges it.
3. Listen. Read the verdict: scores for mix, energy arc, theory, genre and production, with each issue naming what to
   change.
4. Revise, or let [evolution](genres-and-evolution.md) try variations for you.

Every song prints as a script ([Songs as scripts](../../language/printing.md)), so whatever produced it (the
generator, the studio, an assistant), you can read it and edit it as text.
