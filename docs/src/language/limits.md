# Errors and limits

When a script fails, nothing it did is kept: the song stays as it was before the script ran. This page shows what
errors look like, lists the mistakes that do *not* raise an error, and gives the limits a script runs under.

## What an error looks like

`claudius script` prints the error and exits with status 1:

```js error
song("house", 9);
tempo(300);
```

```text
Error: Runtime error: tempo between 40 and 240 BPM (line 2, position 1)
```

The line and position point at the call that failed. In a chain, that is the innermost call that raised the error,
so a bad pattern string points at `s(` rather than at `track(`. The studio and the MCP tools report the same message.

Errors come in four kinds:

| begins with | meaning | example |
|---|---|---|
| `Syntax error:` | the script is not valid Rhai | `Expecting ')' to close the arguments list of this function call 'track'` |
| `Function not found:` | no function by that name takes these argument types | `Function not found: octave (Pattern, f64)` |
| `Variable not found:` | a name without quotes | `s(bd)` instead of `s("bd")` |
| `Runtime error:` | a claudius function refused its arguments | `unknown sound 'bdd'`, `no track 'x'` |

"Function not found" covers typos (`trak`, `gian`), a decimal where an integer is needed (`octave(4.5)`,
`bars_per_chord(1.5)`), a missing or extra argument, and a method called on the wrong thing. The last one is common:
`track("x", s("bd")).gain(0.5)` calls `.gain()` on what `track()` returns, which is nothing, and fails with
`Function not found: gain ((), f64)`. Put the method inside the parentheses.

## Errors by function

The messages, as printed after `Runtime error:`.

| function | message |
|---|---|
| `song()`, `blank()` | `unknown genre 'polka' (try: deep_house, dnb, future_bass, house, lofi, synthwave, techno, trap)` |
| `song()` options | `tempo between 40 and 240 BPM`; `cannot read key 'H' (try "A minor", "F#m", "Eb", "D dorian")`; `song() has no option 'tempo' (options: seed bpm key loops short samples)` |
| `tempo()`, `bpm()` | `tempo between 40 and 240 BPM`; `tempo needs a number` |
| `key()` | `cannot read key 'H minor' (try "A minor", "F#m", "Eb", "D dorian")` |
| `swing()`, `humanize()` | `swing needs a number`, `humanize needs a number` |
| `prog()` | `cannot read chord 'Q7' (Roman numerals like i7 VImaj7 bVII, or names like Am7)` |
| `drum()` | `no drum lane 'bongo', and drum() makes lanes only for drum names (bd kick sd ...); the song's lanes: none yet. For other sounds, track("bongo", s("..."))` |
| `drum_sound()` | `no drum lane 'bongo' (lanes: none yet); drum() makes one`; `'pad' is not a drum sound (a drum name, a kit sound, or a sample)`; the `track()` sound errors below |
| `part()` | `no part 'keys' (the song has: ...; house parts: bass chords pad lead arp); track() writes your own`; `a bass part has no style 'funky' (styles: ...)`; `unknown preset 'nope' (synths: ...; or an installed instrument such as piano, rhodes, string_ensemble)`; `part() has no option 'styles' (options: style preset register density seed rate)` |
| `mix()`, `mute()`, `solo()`, `remove()` ... | `no track 'nope'`; `mix() has no option 'volume' (options: level pan reverb room delay duck mute solo)` |
| `master()` | `master() has no option 'loudness' (options: lufs ceiling)` |
| `track()` | `track 'x': unknown sound 'bdd' (drums: ...; synths: ...; sampled: ...)` |
| `track()`, unreadable words | `track 'x': cannot read 'h3' as a note (...)`; `track 'x': cannot read 'Xyz' as a chord (...)`; `track 'x': n() takes scale degrees (0 1 2 -1 ...), not 'e'` |
| `track()` with `.bank()` | `track 'x': no 'bongo' in bank 'tr909' (installed packs: ...; kits: tr909 tr808 tr707 acoustic uzu)` |
| `track()` without packs | `track 'x': no sample packs are installed for bank 'tr909' (install them with: claudius-packs install --defaults)`; a sampled name: `unknown sound 'rhodes' (...; sampled instruments need packs: claudius-packs install --defaults)` |
| `track()` under `--synth-only` | `track 'x': bank 'tr909' plays sample packs, which are off in this session (--synth-only)`; a sampled name: `unknown sound 'rhodes' (...; sampled instruments are off in this session (--synth-only))` |
| `track()` with a file | `track 'x': /path/vox.wav does not exist`; `track 'x': /path/vox.wav is outside the folders this claudius may read` |
| `track()`, dense patterns | `track 'x': this pattern is too dense to play: it makes more than 262144 events`; `track 'x': this pattern is too dense to play: it needs more than 4000000 steps of work`; `track 'x' plays 2048 notes per bar; the limit is 1024` |
| `form()` | `form() takes sec(...) items`; `section 'drop' uses 'snare', which is not a track (tracks: ...)`; `section 'drop' plays progression 'C', which the song does not have (progressions: A B; prog() adds one)` |
| `sec()` | `unknown section kind 'party' (intro verse build drop break hook groove outro)`; `layers are a string ("kick hats") or an array of names` |
| `.energy()` | `energy needs a number` |
| `section()` | `no section 'nope'`; `no section 12`; `section() takes a name or an index`; the two `form()` checks; `section() has no option 'bar' (options: ...)` |
| `layer()` | `no section 'nope'`; `layer() takes a section name or index`; `no track 'leed' (tracks: ...)` |
| `stack()`, `cat()` | `stack() takes patterns`; `stack() needs at least one pattern` |
| `.fast()`, `.slow()`, `.ply()` | `fast takes a number above 0 and up to 256, not 1000`; `fast needs a number`; `ply takes a whole number from 1 to 256, not 1.5` |
| `.late()`, `.early()`, `.off()` ... | `late needs a number` |
| `.gain()`, `.struct()` ... | `expected a number, a pattern string or a pattern` |
| `.scale()` | `unknown scale 'Q:minor' (try "A:minor", "C:major", "D:dorian", "E:minor:pentatonic")` |
| `.arp()` | `unknown arp 'sideways' ("up", "down" or "updown")` |
| mini-notation | see [Mini-notation errors](mini-notation.md#errors) |
| the whole song | `the song is 768 bars long; the limit is 512`; `the song is 36 minutes long; the limit is 20`; `the song has 70 tracks; the limit is 64` (checked when the script ends, and printed as `Error: the song is ...`, without `Runtime error:` or a line number) |

Errors about the words of a pattern (`cannot read ... as a note`) are raised by `track()`, when the pattern is first
played, so they point at the `track(` line.

## Silent mistakes

These run without an error and do something you may not expect.

| you write | what happens |
|---|---|
| `key("D minor")` after `prog("A", "Am F C G")` | the progression moves to D minor (`prog()` stored numerals) |
| a `chord()` pattern with chord names, no matching `prog()` | the critic checks the melody and bass against progression `A`, which on a `blank()` song is the tonic chord alone |
| `note(...).s("pad")`, `chord(...).s("pad")` with no `.role()` | mixed and judged as a `lead` or `chords`, not a `pad` (see [Roles](mixing.md#roles)) |
| `<a b c@3>` | weights inside `< >` are ignored: each item still takes one cycle; write `<a b c!3>` (see [Alternation](mini-notation.md#groups-stacks-and-alternation)) |
| `.every(4, \|x\| x.s("piano"))` | track settings inside a closure are ignored |
| `s("bd").s("piano")` | `.s()` on a sound pattern is ignored |
| `.bank("apple")` without GarageBand or Logic content | the synthesized drums play |
| `sec("drop", 8).variant("alt")` on a lane without `alt` | the lane plays `main` |

## Limits

A script runs under these limits, so a mistake cannot hang claudius or fill its memory:

| limit | value | error |
|---|---|---|
| running time | 10 seconds | `the script ran for more than 10 seconds` |
| operations | 5,000,000 | `Too many operations` |
| function call depth | 64 | `Stack overflow` |
| expression nesting | 128 (64 inside functions) | `Syntax error: Expression exceeds maximum complexity` |
| string length | 200,000 characters | `Length of string too large` |
| array length | 10,000 items | `Size of array/BLOB too large` |
| speed | `*`, `/`, `fast()`, `slow()` and `ply()` above 0 and up to 256 | `'*' takes a number above 0 and up to 256, not 1000` |
| events per pattern query | 262,144 | `this pattern is too dense to play: it makes more than 262144 events` |
| work per pattern query | 4,000,000 steps | `this pattern is too dense to play: it needs more than 4000000 steps of work` |
| notes per bar, per track | 1,024 (averaged over 64 bars) | `track 'x' plays N notes per bar; the limit is 1024` |
| song length | 512 bars and 20 minutes | `the song is N bars long; the limit is 512` |
| tracks | 64, and 12,000 track-seconds (20 tracks of 10 minutes, 40 of 5) | `the song has N tracks; the limit is 64` |
| a pattern string | 4096 characters, brackets 64 deep, 1024 steps per sequence, `!` 256 times, 256 euclid slots | see [Mini-notation errors](mini-notation.md#errors) |
| modules and `eval` | not available | `Module not found`, `reserved keyword 'eval' is disabled` |

A script reads no files except the audio files named in `.s()`, and writes none. When claudius is shared beyond your
computer, `.s()` reads only inside claudius's own folders (see [Your own files](sounds.md#your-own-files)).

## What the language does not do

- **Time**: every bar has four beats; there are no other time signatures, no tempo changes within a song, and no key
  changes within a song.
- **Automation**: there is no general automation lane. Filter sweeps come from section flags
  ([`build`](form.md#build), [`filter_in`](form.md#filter_in), the final outro), and level changes come from layers
  and energy.
- **Sound design**: scripts choose presets and instruments and add simple inserts; they do not edit synth patches or
  drum voices (see [Sounds](sounds.md#sounds-you-cannot-set-from-a-script)).
- **Arrangement edits per bar**: a track plays the same pattern in every section it is in, from the section's start.
  For a variation in one section, make a second track and put it in that section only.
