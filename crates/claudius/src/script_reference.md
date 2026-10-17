# claudius script

A claudius script is [Rhai](https://rhai.rs) (JavaScript-like: `let`, `if`, `for`, closures `|x| ...`, maps
`#{ k: v }`, arrays `[a, b]`) with two kinds of functions: **song edits** that change the current song, and
**patterns** in [Strudel](https://strudel.cc)-style mini-notation that become tracks. Every sound is real audio:
synthesized in code (drum voices, synth presets) or recorded (Apple Loops one-shots, your files). Notes are only the
score.

A script runs against the current song and changes only what it names. `get_song` (MCP) or `GET /api/script` prints
the whole song as a script; running that script on the song changes nothing, so it is also the way to read a song.

## Starting

| | |
|---|---|
| `song("house")`, `song("trap", 7)` | generate a new song (genre, seed) |
| `song("techno", #{ seed: 3, bpm: 130, key: "F minor", loops: true, short: true, samples: 0.5 })` | with options: `loops` builds it around a family of Apple Loops, `samples` (0..1, default 0.75) is the chance each drum lane is cut from a recording rather than synthesized |
| `blank("house")` | an empty song: the genre's tempo, one 8-bar section, progression `A` = the tonic chord, no tracks |

Without either, edits apply to the current song (a blank house song when there is none). Option maps are checked:
an option a function does not know is an error, not ignored.

## Song edits

| | |
|---|---|
| `title("Night Drive")` | |
| `tempo(124)` | BPM 40..240 |
| `key("A minor")` | also `"F#m"`, `"Eb"`, `"D dorian"`, `"C# phrygian"`. Progressions are Roman numerals, so they follow (a slash bass moves with them). A change of mode moves diatonic chords to the new mode on their degree and keeps borrowed chords' sound (`bIII` in F major is `III` in F minor); tonal loops are re-pitched |
| `swing(0.56)` | 0.5 straight, 0.66 triplet feel |
| `humanize(0.3)` | timing and velocity looseness 0..1 |
| `prog("A", "i7 VImaj7 iv7 v7")` | a progression, one chord per `bars_per_chord` bars. Roman numerals (`i ii° III bVII V/V iiø7`), slash chords (`VI/A`, `F/A`) or chord names (`Am7 Fmaj7 Dm7 Em7`, converted to numerals in the current key). Sections play progression `A` unless they say otherwise; breaks play `B`, or `A` when there is no `B` |
| `bars_per_chord(2)` | how many bars each chord of every progression lasts (1..8) |
| `drum("kick", "x...x...x...x...")` | a drum lane's `main` pattern: `X` accent, `x` hit, `o` ghost, `1`-`9` velocity 0.1-0.9, `.` rest, `\|` bar line (ignored); 16 steps per bar |
| `drum("hat", "sparse", "..x...x...x...x.")` | another variant (sections pick a variant: `main`, `sparse`, `alt`, `half`, ...) |
| `drum("hat", "main", "x.x.x.x.x.x.x.x.x.x.x.x.x.x.x.x.", 32)` | 32 (or 48 for triplets) steps per bar. A new lane needs a drum word as its name (`kick`, `bd`, `snare`, `hat`, `ohat`, `clap`, `perc` ...): the name gives its sound and its role |
| `drum_sound("kick", "tr909:bd")`, `drum_sound("snare", "cp")`, `drum_sound("kick", "bd", "tr808")`, `drum_sound("perc", "/Users/me/Samples/rim.wav")` | what a lane plays: a synthesized drum word, a kit sound from the packs (optionally with a bank), or a sample file |
| `part("bass", #{ style: "syncopated", preset: "reese", register: [28, 48], density: 0.6, seed: 5 })` | a part written by the theory engine: any subset of `style preset register density seed rate`. `seed` rewrites it. On a song without that part (a blank one), any part the genre has is added first |
| `mix("lead", #{ level: -3, pan: 0.2, reverb: 0.3, delay: 0.15, duck: 0.4, mute: false, solo: false })` | level in dB from the role's target; sends 0..1; `duck` = sidechain from the kick |
| `mute("hat")`, `unmute("hat")`, `solo("lead")`, `unsolo("lead")`, `remove("perc")` | while any track is soloed, only soloed tracks play (and get judged) |
| `master(#{ lufs: -9, ceiling: -1 })` | loudness target of the master |
| `track("hats", s("[~ hh]*4"))` | a pattern track (below); replaces any track of that name. A new track plays in every section with energy 0.6 or more, or in every section when no section lists any tracks yet |

Part styles, by the part's role: bass `sustain pulse root8 offbeat octave rolling syncopated kick_follow acid walking
reese` · chords, keys and pads `sustain half stabs offbeat pump syncopated charleston` · arps `up down updown random
pinky` (with `rate`, the note length in beats: 0.25 is sixteenths) · leads `motif`. A preset is any synth preset or
sampled instrument (below).

### Form

```js
form([
  sec("intro", 8, "kick hat pad").filter_in(),
  sec("build", 8, "kick hat bass").build(),           // riser, snare roll, highpass sweep
  sec("drop", 16, "*").impact(),                       // "*" = every track; crash + impact on the downbeat
  sec("break", 8, "pad chords").prog("B").gap(),       // a beat of silence before the next section
  sec("drop", 16, "*").energy(1.0).variant("alt"),     // repeated names become drop2, drop3 ...
  sec("outro", 8, "kick hat"),
]);
section("drop", #{ energy: 0.9, layers: "kick bass lead", fills: true });   // edit one section by name or index
layer("break", "lead", true);                                               // switch one track on/off in a section
```

Section kinds: `intro verse build drop break hook groove outro`. Section methods: `.name(s) .energy(0..1) .prog(name)
.variant(name) .build() .gap() .impact() .fills() .filter_in()` (each flag also takes `true`/`false`).

## Patterns

A pattern is a cycle of events; **one cycle is one bar**. Each section starts its patterns from their first cycle.

| constructor | events are |
|---|---|
| `s("bd sd [~ bd] sd")`, `sound(...)` | sounds by name |
| `note("c3 e3 g3")`, `note("<a1 f1 c2 g1>")`, `note("60 64 67")` | pitches (names with octave, default octave 3, or MIDI numbers) |
| `n("0 2 4 7")` | scale degrees (scale from `.scale()`, default: the song's key) |
| `chord("<Am7 Fmaj7 C G>")`, `chord("<i VI III VII>")`, `chord("prog")` | chords: names, Roman numerals, or `prog` / `prog:B` = the song's progression at that moment |
| `seq("1 0 1 1")` | plain values (for `.struct`, `.mask`, `.gain` ...) |
| `stack([a, b])`, `cat([a, b])`, `silence()` | layer patterns / one per cycle |

### Mini-notation (inside the strings)

| | |
|---|---|
| `a b c` | a sequence dividing the cycle |
| `~` or `-` | rest |
| `[a b]` | a group in one step |
| `<a b c>` | one per cycle (alternation) |
| `a, b` | play together (at the top level or inside `[ ]` / `< >`) |
| `a \| b` | pick one at random per cycle |
| `a*4`, `[a b]/2` | faster / slower |
| `a@3` | weight (takes 3 steps) |
| `a!3`, `a !` | repeat |
| `a?`, `a?0.3` | drop at random (50 % / 30 %) |
| `a(3,8)`, `a(3,8,2)` | euclidean rhythm (pulses, steps, rotation) |
| `a _ _` | hold over the next steps |
| `bd:3` | sample / variation 3 of `bd`; `tr808:sd` (a kit sound) and `F/A` (a slash chord) are single words |

### Pattern methods

| | |
|---|---|
| `.fast(2) .slow(2) .late(0.25) .early(0.125) .rev() .ply(2)` | time |
| `.every(4, \|x\| x.fast(2))`, `.lastOf(4, \|x\| x.rev())` | transform every n-th cycle (first / last of n) |
| `.sometimes(f) .often(f) .rarely(f) .almostNever(f) .almostAlways(f) .sometimesBy(0.3, f)` | transform random events |
| `.off(0.125, \|x\| x.add(12))` | a shifted, transformed copy on top |
| `.superimpose(\|x\| x.add(7))` | a transformed copy on top |
| `.degrade() .degradeBy(0.3)` | drop random events |
| `.struct("x ~ x x")`, `.mask("1 0 1 1")`, `.euclid(3, 8)`, `.euclidRot(3, 8, 2)` | rhythm from another pattern |
| `.gain(0.8) .velocity("1 0.6") .legato(0.5) .add(2) .transpose(-12)` | per event; numbers or patterns (`.gain("1 0.7 0.9 0.7")`). `add` adds to the value (scale degrees on `n()`, semitones on `note()`); `transpose` is always semitones |
| `.n("0 1 2")` | on sound patterns: which variation (`bd:0 bd:1 ...`) |

### Track settings (on the pattern a track plays)

| | |
|---|---|
| `.s("808")`, `.sound("pluck")` | the instrument for note / degree / chord patterns |
| `.bank("apple")` | drum names cut from Apple Loops recordings instead of synthesized |
| `.scale("A:minor")`, `"D:dorian"`, `"E:minor:pentatonic"`, `"C:blues"` | scale for `n()` |
| `.octave(4)` | octave for `n()` |
| `.voicing()`, `.voicing(48, 72)` | voice chords with voice leading in a range (MIDI) |
| `.arp("up")`, `"down"`, `"updown"` | arpeggiate chords over each event |
| `.root()` | chords play only their root (or slash bass) in octave 2 (`.octave(n)` moves it): a bass line that follows `chord("prog")` |
| `.level(-3) .pan(-0.4) .room(0.3) .delay(0.2) .duck(0.5) .lpf(1200) .hpf(200) .crush(8) .distort(6)` | mix and inserts |
| `.role("lead")` | mixing role when it is not obvious (kick, snare, clap, hat, perc, bass, chords, pad, lead, arp, keys). Without it: a drum word anywhere in the sound's name gives its drum role (`tr808:sd` is a snare), percussion from the packs is `perc`, bass presets and low notes are `bass`, chord patterns are `chords`, the rest `lead` |

### Sounds

- Drums (synthesized; `.bank("apple")` cuts them from recordings): `bd kick sd snare cp clap hh ch hat oh ohat rd ride
  cr crash lt mt ht tom rim rs sh shaker cb cowbell snap perc`.
- Real drum machines and kits from the sample packs: `s("bd*4, [~ sd]*2, hh*8").bank("tr909")`. Banks: `tr909`
  (`909`), `tr808` (`808`), `tr707` (`707`), `acoustic` (a recorded rock kit), `uzu`; `bd:2` picks a variation.
- Synth presets: see `catalog` / `GET /api/sounds` (e.g. `808 sub_bass reese acid pluck supersaw pad epiano keys organ
  bell`).
- Sampled instruments, when their packs are installed (`claudius-packs install --defaults`): `piano upright rhodes
  wurlitzer cp80 hammond church_organ` · `string_ensemble violins violas cellos contrabass pizzicato spiccato brass
  brass_stabs trumpet horn trombone tuba woodwinds flute oboe clarinet bassoon piccolo harp timpani` · `marimba vibes
  xylophone glock bells kalimba` · `upright_bass electric_bass synth_bass guitar nylon_guitar electric_guitar
  jazz_guitar` · `choir voice sax harmonica` · percussion `bongo conga cajon tambourine shaker claves cowbell woodblock
  triangle gong`. `pack:instrument` picks one exactly (`vsco2:violins`). Names that are also synth presets (`epiano
  organ strings keys pad warm_pad`) or drum words (`shaker cowbell`) play the synthesized sound: ask for the recording
  by another name (`rhodes`, `hammond`, `string_ensemble`) or as `pack:instrument` (`claudius-packs instruments` lists
  them). `lead` is the sampled square lead; the synth leads are `square_lead`, `supersaw` and the others above. Parts take them too: `part("chords", #{ preset: "piano" })`.
- An audio file plays as a sampler through `.s(path)`: `note("c4 ~ c4 [c4 c4]").s("/Users/me/Samples/vox.wav")`
  (`c4` plays it as recorded, other notes re-pitch it). Paths cannot go inside mini-notation strings.
- Words a pattern cannot read are errors: `note("c3 zz")`, `chord("H7")`, `n("0 e")` say what they expected.

## Examples

```js
// a beat from nothing
blank("house"); tempo(122); key("A minor");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4, hh*16?0.2").gain("1 0.8"));
track("bass",  note("<a1 a1 f1 g1>").struct("~ x ~ x ~ x ~ [x x]").s("808").legato(0.8));
track("keys",  chord("<Am9 Fmaj7 Dm9 Em7>").struct("x ~ ~ x ~ ~ x ~").voicing().s("epiano").room(0.25));
track("lead",  n("<[0 2 4 7] [4 7 9 7]>").scale("A:minor").octave(4).s("pluck").delay(0.3).every(4, |x| x.rev()));
form([sec("intro", 8, "drums keys"), sec("drop", 16, "*").impact(), sec("break", 8, "keys lead"), sec("drop", 16, "*")]);
```

```js packs
// real instruments (needs the sample packs)
blank("lofi"); tempo(84); key("D minor"); swing(0.58);
track("drums", s("bd ~ ~ bd, ~ sd, hh*8?0.15").bank("tr808").gain("1 0.7"));
track("keys",  chord("<Dm9 Gm9 Bbmaj7 A7>").voicing(50, 72).s("rhodes").room(0.3).late(0.02));
track("bass",  note("<d2 g1 bb1 a1>").struct("x ~ ~ x ~ x ~ ~").s("upright_bass"));
track("lead",  n("<[4 ~ 2 0] [~ 4 5 4]>").scale("D:minor:pentatonic").octave(4).s("vibes").delay(0.25).sometimes(|x| x.ply(2)));
track("pad",   chord("<Dm Gm Bb A>").voicing(48, 70).s("string_ensemble").level(-6));
form([sec("intro", 4, "keys pad"), sec("groove", 16, "*"), sec("break", 8, "keys lead pad"), sec("groove", 16, "*")]);
```

```js
// edit a generated song
tempo(126); key("F minor");
drum("kick", "x...x...x...x..x");
part("lead", #{ style: "motif", preset: "supersaw", seed: 42 });
mix("pad", #{ level: -4, reverb: 0.45 });
section("drop", #{ energy: 1.0 });
```
