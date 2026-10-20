# Quick reference

Everything on one page. Each name links to its full entry.

## Starting

| | |
|---|---|
| [`song("house", 7)`](song-functions.md#song) | a generated song; options `#{ seed, bpm, key, short, loops, samples }` |
| [`blank("house")`](song-functions.md#blank) | an empty song: one 8-bar section, no tracks, progression `A` = the tonic chord, the genre's mix |
| (neither) | edit the open song, or a blank house song when none is open |

## Song edits

| | |
|---|---|
| [`title("Night Drive")`](song-functions.md#title) | |
| [`tempo(124)`](song-functions.md#tempo), `bpm(124)` | 40..240 |
| [`key("A minor")`](song-functions.md#key) | `"F#m"`, `"Eb"`, `"D dorian"`, `"E harmonic minor"` |
| [`swing(0.56)`](song-functions.md#swing) | 0.5 straight .. 0.8; sixteenth-note swing |
| [`humanize(0.3)`](song-functions.md#humanize) | 0..1 |
| [`prog("A", "i7 VImaj7 iv7 v7")`](song-functions.md#prog) | numerals, chord names, slash chords; sections play `A`, breaks `B` (or `A`) |
| [`bars_per_chord(2)`](song-functions.md#bars_per_chord) | bars each chord lasts, 1..8 |
| [`drum("kick", "X...x...")`](song-functions.md#drum) | `X` 1.0, `x` 0.8, `o` 0.45, `1`-`9`, `.` rest; `drum(lane, variant, steps, per_bar)` |
| [`drum_sound("kick", "tr909:bd")`](song-functions.md#drum_sound) | a drum name, a kit sound (or a name and a bank), a file |
| [`part("bass", #{ ... })`](song-functions.md#part) | `style preset register density seed rate`; adds the genre's part when the song has none |
| [`track("hats", pattern)`](song-functions.md#track) | a pattern track; new ones join sections with energy ≥ 0.6 |
| [`mix("pad", #{ ... })`](mixing.md#mix) | `level` −40..12, `pan` −1..1, `reverb`/`room`, `delay`, `duck` 0..1, `mute`, `solo` |
| [`master(#{ lufs: -9, ceiling: -1 })`](mixing.md#master) | −24..−5 LUFS, −3..−0.1 dBTP |
| [`mute()` `unmute()` `solo()` `unsolo()` `remove()`](song-functions.md#mute-unmute-solo-unsolo-remove) | by track name |

## Form

| | |
|---|---|
| [`form([sec(...), ...])`](form.md#form) | the whole form; repeated names become `drop2`, `drop3` |
| [`sec(kind, bars, layers)`](form.md#sec) | kinds `intro verse build drop break hook groove outro`; layers `"kick hat"`, `[...]`, `"*"` |
| [`.name() .energy() .prog() .variant()`](form.md#section-methods) | |
| [`.build() .gap() .impact() .fills() .filter_in()`](form.md#what-the-flags-do) | each also takes `true` / `false` |
| [`section("drop", #{ ... })`](form.md#section) | `bars energy prog variant name layers build gap impact fills filter_in`; by name or 0-based index |
| [`layer("break", "lead", true)`](form.md#layer) | one track on or off in one section |

## Patterns

| constructor | values |
|---|---|
| [`s("bd sd")`, `sound()`](pattern-functions.md#s-and-sound) | sound names |
| [`note("c3 e3 g3")`](pattern-functions.md#note) | note names (octave 3 by default) or MIDI numbers |
| [`n("0 2 4")`](pattern-functions.md#n) | scale degrees (`.scale()`, else the song's key) |
| [`chord("<Am7 F/A>")`](pattern-functions.md#chord) | chord symbols, slash chords, numerals, `prog`, `prog:B` |
| [`seq("1 0 1 1")`](pattern-functions.md#seq) | plain values |
| [`stack([...])`, `cat([...])`, `silence()`](pattern-functions.md#stack) | together / one per bar / nothing |

## Mini-notation

| | |
|---|---|
| `a b c` | sequence over one bar |
| `~`, `-` | rest |
| `[a b]`, `{a b}` | group in one step |
| `a, b` | together |
| `<a b c>` | one per bar |
| `a \| b` | random pick per bar |
| `a*2`, `a/2` | faster, slower; above 0, up to 256 |
| `a@3`, `a _ _` | weight |
| `a!3`, `a !` | repeat |
| `a?`, `a?0.3` | drop at random |
| `a(3,8)`, `a(3,8,2)` | euclidean |
| `bd:3`, `tr808:sd:2` | variation: a number after the last colon |
| `F/A`, `V7/V` | a slash before a letter stays in the word |

## Pattern methods

| | |
|---|---|
| [`.fast(2) .slow(2)`](pattern-functions.md#fast-and-slow) | speed; above 0, up to 256 |
| [`.late(0.25) .early(0.125)`](pattern-functions.md#late-and-early) | shift (cycles = bars) |
| [`.rev() .ply(2)`](pattern-functions.md#rev) | reverse each bar, repeat each event (1 to 256 times) |
| [`.every(4, f) .lastOf(4, f)`](pattern-functions.md#every-and-lastof) | first / last bar of every n, from the section start |
| [`.sometimes(f) .often(f) .rarely(f) .almostNever(f) .almostAlways(f) .sometimesBy(p, f)`](pattern-functions.md#sometimes-and-its-relatives) | per event, seeded |
| [`.off(0.125, f) .superimpose(f)`](pattern-functions.md#off) | shifted copy, copy on top |
| [`.degrade() .degradeBy(0.3)`](pattern-functions.md#degrade-and-degradeby) | drop events, seeded |
| [`.struct("x ~ x x") .mask("1 0 1 1")`](pattern-functions.md#struct) | rhythm from another pattern |
| [`.euclid(3, 8) .euclidRot(3, 8, 2)`](pattern-functions.md#euclid-and-euclidrot) | euclidean rhythm |
| [`.gain() .velocity() .legato()`](pattern-functions.md#per-event-controls) | per event; numbers or patterns |
| [`.add(2) .transpose(12)`](pattern-functions.md#add-and-transpose) | add to the value (degrees on `n()`) / semitones always |
| [`.n("0 1 2")`](pattern-functions.md#n-on-a-sound-pattern) | variation, on sound patterns |

## Track settings

| | |
|---|---|
| `.s(name)`, `.sound(name)` | instrument for note / degree / chord patterns |
| `.bank("tr909")` | `tr909 tr808 tr707 acoustic uzu synthdrums sonicpi body apple` |
| `.scale("A:minor")` | also `dorian phrygian lydian mixolydian harmonic_minor minor_pentatonic pentatonic blues chromatic` |
| `.octave(4)` | octave of degree 0 and of note names without one |
| `.voicing()`, `.voicing(48, 72)` | chord range, default 52..74 |
| `.arp("up")` | `up down updown` |
| `.root()` | a chord pattern's roots (or slash basses), octave 2 |
| `.role("pad")` | mixing role |
| `.level(-3) .pan(-0.4) .room(0.3) .delay(0.2) .duck(0.5)` | mix |
| `.lpf(1200) .hpf(200) .crush(8) .distort(6)` | inserts |

## Sounds

| | |
|---|---|
| drums | `bd sd cp hh oh rd cr lt mt ht rim sh cb snap perc` (and `kick snare clap hat ohat ride crash tom rs shaker cowbell ch`) |
| synths | `sub_bass 808 reese acid saw_bass pluck_bass organ_bass wobble pluck supersaw pad warm_pad stab epiano keys bell organ strings square_lead chip` |
| sampled | `piano upright rhodes wurlitzer hammond string_ensemble violins cellos pizzicato brass trumpet flute harp marimba vibes kalimba upright_bass bass guitar choir sax bongo conga ...` |
| exact | `vsco2:violins`, `tr808:sd`, in `.s()` or in `s()` patterns |
| files | `.s("/path/sound.wav")`: `c4` plays it as recorded |

## Roles and targets (LUFS while playing)

`kick` −12.5 · `snare` −13.5 · `clap` −14.5 · `hat` −17.5 · `ohat` −18.5 · `perc` −19.5 · `shaker` −21.5 · `ride`
−20.5 · `tom` −18 · `crash` −20 · `bass` −14.5 · `keys` −17 · `chords` −18 · `lead` −18 · `arp` −20.5 · `pad` −21.5 ·
`vocal` −15 · anything else −19.
