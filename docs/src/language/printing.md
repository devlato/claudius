# Songs as scripts

Every song can be printed as a script, whether it was generated, written by hand, edited in the studio, mutated or
remade from audio. The printed script is the readable form of the song: its tempo and key, its progressions, every
drum lane as step strings, every generated part with its settings, every pattern track's code, the form and the mix
settings that differ from the defaults.

## The guarantee

**Running a song's printed script on that same song changes nothing.** The test suite checks this for generated
songs in several genres and for songs built from patterns. It is what makes the printed script useful: copy it,
change one line, run it, and only that line changes.

The guarantee is about running the script *on the song it came from*. Run anywhere else, a printed script is a
recipe for the song only as far as [the header](#the-header) and [What is not printed](#what-is-not-printed) allow.

## Where to get it

| where | how |
|---|---|
| command line | `claudius script song.rhai --print` prints the song a script makes, instead of rendering it |
| any song folder | `echo "" \| claudius script - --on out/house-crimson-tide-9 --print` prints a saved song (an empty script changes nothing) |
| song folders | folders written by `claudius script` and by saving in the studio (`save`) have `song.rhai` |
| studio | the script tab shows the open song as a script |
| assistants | the MCP tool `get_song` includes it |
| HTTP | `GET /api/script` |

## What a printed script looks like

This script:

```js
blank("house");
title("Night Drive"); tempo(122); key("A minor"); swing(0.54);
prog("A", "Am7 Fmaj7 Dm7 Em7");
bars_per_chord(1);
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").room(0.25));
drum("snare", "....x.......x...");
form([sec("intro", 8, "drums").filter_in(), sec("drop", 16, "*").impact()]);
mix("keys", #{ level: -2 });
```

prints as:

```text
// Night Drive — house (seed 1)
// blank("house");   // uncomment to build this song from nothing (as it is, the script edits the song open)
title("Night Drive"); tempo(122); key("A minor"); swing(0.54);
humanize(0.25);
prog("A", "i7 VImaj7 iv7 v7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("snare", "main", "....x.......x...");

// pattern tracks (mini-notation, Strudel style)
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").room(0.25));

form([
  sec("intro", 8, "drums").filter_in(),
  sec("drop", 16, "*").impact(),
]);

mix("keys", #{ level: -2 });
mix("fx", #{ reverb: 0.25 });
```

Things to notice:

- Chord names are printed as the Roman numerals they were stored as.
- `bars_per_chord()` and `master()` always print. Other settings print only when they differ from what the song
  would have without them: `humanize()` when it is not 0, a section method when it differs from the kind's
  [default](form.md#sec), a `mix()` field when it differs from the track's starting mix.
- A pattern track's mix line compares against the pattern's own settings, so `.room(0.25)` stays in the code and is
  not repeated in `mix()`.
- The drum lane prints every variant, with the variant name, so a step string from the studio's sequencer reads back
  exactly. A lane that plays something other than what its name gives (a kit sound, a sample, another drum voice,
  a variation such as `bd:3`) also prints a [`drum_sound()`](song-functions.md#drum_sound) line. A one-shot cut
  from Apple Loops or your library prints as a comment with its label, such as
  `// shaker: shaker cut from 'Cosmic Harmony Shaker' (Apple Loops)`: the cut file lives in claudius's cache, so a
  path would mean nothing on another computer.
- A generated part prints its style, preset, register, density and seed, and an arp part its `rate`.
- `mix("fx", ...)` is the transition effects' mix (crashes and risers), which comes from the genre.

## How values print

- **Numbers** print as the shortest text that reads back as the same value: `swing(0.54)`,
  `density: 0.42195085`. Nothing is rounded.
- **Drum steps** print at 16 steps per bar when every hit sits on a sixteenth, else 32, else 48 (with the number
  after the string). A hit prints as `X` at velocity 0.85 or more, `x` from 0.5, `o` below. Reading the string back
  keeps the exact velocities as long as the string is unchanged, so `drum("hat", "main", "x...")` on a lane whose
  hits print that way leaves them alone. A lane whose bars are all the same prints one bar.
- **Seeds** print as signed 64-bit integers, so a large seed can print negative (`seed: -8861390911444144378`); it
  reads back as the same seed.
- **Closures** print with the values they used: `.every(2, |x| x.add(7))`, even if the script wrote `x.add(up)`.

## What is not printed

These are part of the song but not of its script. Running the printed script on the same song keeps them; running it
anywhere else does not recreate them.

- the settings the generator or evolution gave a synthesized drum voice (its tuning, decay and the rest): no drum
  word plays them, so there is nothing to print. A voice that a drum word plays exactly, such as the variation
  `bd:3`, prints as that word;
- one-shots cut from Apple Loops or your library, which print as a comment naming them;
- the patch of a generated part beyond its preset name (the generator's variation and evolution's tweaks), its voice
  count and voicing style;
- Apple Loops layered into the song as loops, and flipped samples;
- the effects on tracks and buses, the reverb and delay settings, and the master chain (all from the genre);
- the transition sounds and settings;
- the song's history (the lineage shown in the studio's log).

## The header

The first two lines say what the song is and how it began: `// song("house", 9);` for a generated song,
`// blank("house");` for one that began empty. The second line is a comment, so the script edits whatever song is
open: running it on its own song changes nothing. With no song open (`claudius script` without `--on`), a script
starts from a blank house song, so a printed script run on its own needs that line. Uncomment it to start over:

- A song that began with `blank()` and was made by scripts is rebuilt exactly, unless it has something from the list
  above.
- A generated song is generated again from its seed, and the lines after the header re-apply the edits. `song()`
  picks the Apple Loops one-shots again from your library. What was changed after generation and is in the list
  above (evolution's tweaks, a flipped sample) is not recreated.
