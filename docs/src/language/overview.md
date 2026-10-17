# Overview

A claudius script is a [Rhai](https://rhai.rs) program with two kinds of functions on top:

- **Song edits** change the song that is open: `tempo(124)`, `key("A minor")`, `prog("A", "i7 VImaj7 iv7 v7")`,
  `mix("pad", #{ level: -4 })`, `form([...])`.
- **Patterns** describe music in [Strudel](https://strudel.cc)-style mini-notation, and `track(name, pattern)` turns
  a pattern into a track of the song: `track("hats", s("[~ hh]*4"))`.

Here is a complete song in eight lines. It builds everything itself, so it runs anywhere:

```js
blank("house");
tempo(122); key("A minor"); prog("A", "Am7 Fmaj7 Dm7 Em7"); bars_per_chord(1);
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("bass",  chord("prog").root().struct("~ x ~ x ~ x ~ [x x]").s("organ_bass").legato(0.7));
track("keys",  chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").room(0.25));
track("lead",  n("<[4 ~ 4 2] [5 ~ 2 0] [3 ~ 0 2] [4 ~ ~ ~]>").octave(4).s("pluck").delay(0.2));
form([sec("intro", 8, "drums keys").filter_in(), sec("drop", 16, "*").impact(),
      sec("break", 8, "keys lead"), sec("drop", 16, "*")]);
```

You hear a four-on-the-floor house groove in A minor at 122 BPM: kick, claps on 2 and 4, offbeat hats, an organ bass on
the offbeats playing the root of each chord, electric piano stabs and a plucked melody. The intro opens a lowpass filter
over eight bars, the first drop lands with a crash.

## How a script runs

A script runs top to bottom against the *current song* and changes only what it names. There is always a current
song:

- `song("house", 7)` replaces it with a song the generator writes (genre, seed); see [`song()`](song-functions.md#song).
- `blank("house")` replaces it with an empty song: one 8-bar section, no tracks, and the tonic chord as its only
  progression; see [`blank()`](song-functions.md#blank).
- Without either, edits apply to the song that is open: the one in the studio, the folder you pass with
  `claudius script --on`, or, when there is none, a blank house song.

Edits apply in order, so a later line sees what an earlier line did. `key("D minor")` after `prog()` transposes the
progression; `prog()` after `key()` reads chord names in the new key. Where order matters, the reference says so.

When the script ends, the song is arranged, rendered and judged. That happens outside the script: a script only
describes the song.

## Where scripts run

| where | how |
|---|---|
| command line | `claudius script song.rhai` renders into `out/<genre>-<title>-<seed>/`; `--on <folder>` edits a saved song; `--print` prints the result as a script instead of rendering |
| studio | the script tab runs against the open song; every change renders and is judged in the background |
| assistants | the MCP tool `run_script` (read `script_reference` first) |
| HTTP | `POST /api/command` with `{"cmd": "run_script", "code": "..."}` |

`claudius reference` prints the compact reference that assistants get.

## The examples in this reference

Every example in these pages is run by the test suite. An example that starts with `song(...)` or `blank(...)`
stands alone. One that does not edits a generated song, the same one every time: `song("house", 9)`, "Crimson
Tide", 124 BPM in G dorian, with the tracks `kick clap hat ohat shaker perc bass chords pad lead arp`, the sections
`intro groove break drop outro` and the progressions `A` and `B`. To try such an example yourself, put
`song("house", 9);` on the line before it.

```js
// on the open song: faster, and the hats out of the intro
tempo(126);
layer("intro", "hat", false);
```

## Reading the reference

Signatures use plain names for parameters and say what each one accepts:

```js no-run
tempo(bpm)                 // bpm: number, 40..240
drum(lane, variant, steps, per_bar)
```

*number* means an integer or a decimal (`124` or `124.5`). *integer* means a whole number: Rhai keeps the two apart,
so `bars_per_chord(1.5)` is an error, not a rounding. A *pattern string* is mini-notation in quotes. Where a number
is clamped into a range rather than rejected, the reference says so.

## The pages

- [Rhai in five minutes](rhai.md): the language underneath.
- [Mini-notation](mini-notation.md): the grammar inside the strings.
- [Patterns and time](patterns.md): cycles, bars, sections and randomness.
- [Pattern functions](pattern-functions.md): every pattern constructor and method.
- [Pitch](pitch.md): notes, scales, chords, Roman numerals, voicing and arpeggios.
- [Sounds](sounds.md): drum names, synth presets, sampled instruments, kits and files.
- [Song functions](song-functions.md): tempo, key, progressions, drums, parts and tracks.
- [Form and sections](form.md): the arrangement and what each section flag does.
- [Mixing functions](mixing.md): roles, levels, sends, sidechain, buses and the master.
- [Songs as scripts](printing.md): how a song prints, and the round-trip guarantee.
- [Errors and limits](limits.md): error messages and the limits a script runs under.
- [Quick reference](quick-reference.md): everything on one page.
