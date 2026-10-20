# The claudius script language, briefly

The installed claudius prints its complete reference with `script_reference` (MCP) or `claudius reference`; read
that for anything not here. This file is the part you need most often, with recipes that are run by claudius' tests.

- [Rhai in a paragraph](#rhai-in-a-paragraph)
- [Song edits](#song-edits)
- [Patterns](#patterns)
- [Sounds](#sounds)
- [Recipes](#recipes)
- [Mistakes to avoid](#mistakes-to-avoid)

## Rhai in a paragraph

Scripts are [Rhai](https://rhai.rs), which reads like JavaScript: `let x = 3;`, `if`, `for i in 0..4 { }`,
closures `|x| x.fast(2)`, maps `#{ level: -3, pan: 0.2 }`, arrays `["kick", "bass"]`. Statements end with `;`.
Integers and decimals are different types: counts (`ply(2)`, `octave(4)`, the bars of `sec()`) take integers.

## Song edits

| function | does |
|---|---|
| `song("house")`, `song("trap", 7)`, `song("techno", #{ seed: 3, bpm: 130, key: "F minor" })` | generate a complete song |
| `blank("lofi")` | an empty song: the genre's tempo, one 8-bar section, the tonic chord as progression `A` |
| `title("Night Drive")`, `tempo(124)`, `key("A minor")`, `swing(0.56)`, `humanize(0.3)` | song settings |
| `prog("A", "i VI III VII")`, `bars_per_chord(2)` | progressions (numerals or chord names like `Am F C G`) and chord length |
| `drum("kick", "x...x...x...x...")`, `drum("hat", "sparse", "..x...x...x...x.")` | a drum lane's pattern (variant `main` by default): `X` 1.0, `x` 0.8, `o` 0.45, `1`-`9` tenths, `.` a rest, `\|` a bar line |
| `drum_sound("kick", "tr909:bd")` | what a drum lane plays: a drum word, a kit sound, or a sample file |
| `part("bass", #{ style: "syncopated", preset: "reese", density: 0.6, seed: 5 })` | a part the theory engine writes |
| `track("hats", s("[~ hh]*4"))` | a track that plays a pattern (replaces a track of that name) |
| `mix("pad", #{ level: -4, pan: -0.3, reverb: 0.4, delay: 0.2, duck: 0.5 })` | level in dB from the role's target; sends 0 to 1; `duck` is the sidechain from the kick |
| `mute("hat")`, `solo("lead")`, `remove("perc")`, `master(#{ lufs: -9, ceiling: -1 })` | the rest of the mix |
| `form([sec("intro", 8, "kick hat pad"), sec("drop", 16, "*").impact()])` | the arrangement: kind, bars, tracks |
| `section("drop", #{ energy: 1.0, layers: "kick bass lead", prog: "B" })`, `layer("break", "lead", true)` | edit one section |

Section kinds are `intro verse build drop break hook groove outro`, and sections take `.energy(0..1)`, `.prog("B")`,
`.variant("sparse")`, `.build()` (riser, snare roll, highpass sweep), `.impact()` (crash on the downbeat), `.gap()`
(a beat of silence before the next section), `.fills()` and `.filter_in()`. `"*"` means every track.

## Patterns

One cycle of a pattern is one bar, and every section starts its patterns from their first bar.

| constructor | events are |
|---|---|
| `s("bd sd [~ bd] sd")` | sounds by name |
| `note("c3 e3 g3")`, `note("<a1 f1 c2 g1>")` | pitches (octave 3 when none is given) or MIDI numbers |
| `n("0 2 4 7").scale("A:minor")` | scale degrees (the song's key without `.scale()`) |
| `chord("<Am7 Fmaj7 C G>")`, `chord("<i VI III VII>")`, `chord("prog")` | chords; `prog` follows the song's progression |
| `seq("1 0 1 1")`, `stack([a, b])`, `cat([a, b])` | plain values, layers, one pattern per bar |

Inside the strings (mini-notation):

| | |
|---|---|
| `a b c` | a sequence that divides the bar |
| `~` | a rest |
| `[a b]` | a group in one step |
| `<a b c>` | one per bar, in turn |
| `a, b` | at the same time |
| `a*4`, `[a b]/2` | faster, slower (1 to 256) |
| `a@3`, `a!3` | weight (lasts 3 steps), repeat |
| `a?`, `a?0.3` | sometimes left out |
| `a(3,8)`, `a(5,8,2)` | euclidean rhythm (hits, steps, rotation) |
| `bd:3`, `tr808:sd`, `F/A` | a variation, a kit sound, a slash chord |

Methods on patterns:

- time: `.fast(2) .slow(2) .late(0.25) .early(0.125) .rev() .ply(2)`
- variation: `.every(4, |x| x.fast(2))`, `.lastOf(4, |x| x.rev())`, `.sometimes(|x| x.ply(2))` (also `often`,
  `rarely`, `almostNever`, `almostAlways`, `sometimesBy(0.3, f)`), `.off(0.125, |x| x.add(7))`,
  `.superimpose(|x| x.add(12))`, `.degradeBy(0.3)`
- rhythm from another pattern: `.struct("x ~ x x")`, `.mask("1 0 1 1")`, `.euclid(3, 8)`
- per event: `.gain(0.8)`, `.velocity("1 0.6")`, `.legato(0.5)`, `.add(2)` (scale degrees on `n()`),
  `.transpose(-12)` (always semitones)
- track settings: `.s("808")` (the instrument), `.bank("tr909")`, `.octave(4)`, `.voicing()`, `.arp("updown")`,
  `.root()` (a chord's root: a bass line), `.level(-3) .pan(-0.4) .room(0.3) .delay(0.2) .duck(0.5) .lpf(1200)
  .hpf(200) .crush(8) .distort(6)`, `.role("lead")`

## Sounds

- Drum words, synthesized: `bd kick sd snare cp clap hh ch hat oh ohat rd ride cr crash lt mt ht tom rim rs sh
  shaker cb cowbell snap perc`.
- Real drum machines and kits from the sample packs: `.bank("tr909")`, `"tr808"`, `"tr707"`, `"acoustic"`, `"uzu"`,
  `"synthdrums"`, `"sonicpi"` (Sonic Pi's `bd`, `sn`, `hat`), `"body"` (`stomp clap snap chest`).
- Synth presets: `sub_bass 808 reese acid saw_bass pluck_bass organ_bass wobble pluck supersaw pad warm_pad stab
  epiano keys bell organ strings square_lead chip`.
- Sampled instruments when their packs are installed: `piano upright rhodes wurlitzer hammond string_ensemble
  violins cellos pizzicato brass trumpet flute harp marimba vibes kalimba upright_bass electric_bass guitar choir
  voice sax solo_cello erhu steel_pan hang water_glasses wine_glasses bowed_vibraphone dan_tranh clavisynth` and more;
  `catalog` (or `claudius-packs instruments`) lists what is installed.
- One-shots and effects in groups, picked with a variation (`s("zaps:3")`, `.n()`): `elec glitch glitch_kit ambi
  vinyl loop tabla aliens zaps sirens metal_hits glass_breaks chains glock_gliss ocean_drum`; Sonic Pi's sounds also
  by their own names (`s("vinyl_scratch")`, `s("bd_haus")`).
- A file plays as a sampler: `note("c4 ~ c4 [c4 c4]").s("/path/to/vox.wav")`; `c4` is the pitch it was recorded at.

## Recipes

A beat with a variation every fourth bar, on a song of its own:

```js
blank("techno");
tempo(130);
track("kick", s("bd*4"));
track("hats", s("[~ hh]*4").gain(0.7).every(4, |x| x.fast(2)));
track("perc", s("rim(3,8,2)").gain(0.6).pan(0.3));
```

A bass that follows the song's chords, and chords that lead smoothly into each other:

```js
blank("deep_house");
key("A minor");
prog("A", "i7 iv7 VImaj7 v7");
track("bass", chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass").legato(0.8));
track("keys", chord("prog").struct("~ x ~ x").voicing(52, 72).s("epiano").room(0.3));
track("arp", chord("prog").struct("x*8").arp("updown").s("pluck").level(-6).delay(0.25));
```

A melody on scale degrees that answers itself every other bar:

```js
blank("lofi");
key("D minor");
track("lead", n("<[0 2 4 ~] [4 ~ 2 0]>").scale("D:minor:pentatonic").octave(4).s("bell").delay(0.3)
              .lastOf(4, |x| x.rev()));
```

An arrangement with a build, a gap and a drop, on the open song's tracks:

```js
form([
  sec("intro", 8, "kick hat pad").filter_in(),
  sec("groove", 16, "kick clap hat bass chords"),
  sec("break", 8, "pad chords"),
  sec("build", 8, "kick hat bass").build().gap(),
  sec("drop", 16, "*").impact(),
  sec("outro", 8, "kick hat"),
]);
```

Changes to a generated song: a new kick pattern, another bass style, a quieter pad:

```js
drum("kick", "x...x...x...x..x");
part("bass", #{ style: "syncopated" });
mix("pad", #{ level: -4, reverb: 0.45 });
section("drop", #{ energy: 1.0 });
```

Real instruments, when the sample packs are installed:

```js packs
blank("lofi");
tempo(84);
swing(0.58);
track("drums", s("bd ~ ~ bd, ~ sd, hh*8?0.15").bank("tr808"));
track("keys", chord("<Dm9 Gm9 Bbmaj7 A7>").voicing(50, 72).s("rhodes").room(0.3));
track("bass", note("<d2 g1 bb1 a1>").struct("x ~ ~ x ~ x ~ ~").s("upright_bass"));
```

## Mistakes to avoid

- **Editing when you meant to start over.** A script edits the open song. Start with `song()` or `blank()` for a
  fresh one; with no song open at all, a script edits a blank house song.
- **`drum()` with a name that isn't a drum word.** It makes lanes only for drum words; anything else is a
  `track()`.
- **A new track that plays somewhere else than you meant.** A new track, part or drum lane plays where the form
  already names it (generated forms name the genre's optional parts), otherwise in every section with energy 0.6 or
  more. Print the song afterwards and place it with `form()`, `section()` or `layer()`.
- **Pieces a generated song doesn't use.** A generated song can define a progression `B` that no section plays, and
  name optional parts the generator left out this time, which stay silent. `section("<name>", #{ prog: "B" })` uses
  the other progression; `part("<name>", #{ ... })` adds the missing part where the form names it.
- **Guessing names.** Unknown sounds, options, chords and notes are errors that list what is accepted. Read the
  list, or `catalog`, instead of trying variants.
- **`transpose` and `add`.** `transpose` is always semitones; `add` adds to the value, which on `n()` means scale
  degrees.
- **Paths inside mini-notation.** A file path goes in `.s(path)`, never inside the pattern string.
- **Reading a printed song.** It leaves out what a section kind does by default: a `build` section builds, and drops,
  grooves and hooks have fills, without saying so.
- **Limits.** `*`, `/`, `fast`, `slow` and `ply` go up to 256; a track plays at most 1024 notes a bar; a song is at
  most 512 bars and 20 minutes.
