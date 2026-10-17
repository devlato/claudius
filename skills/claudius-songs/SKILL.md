---
name: claudius-songs
description: >-
  Makes and edits songs with claudius, a beatmaker that composes, arranges, mixes and masters music from real
  synthesized and sampled sounds and judges every render with a critic. Covers starting a song in a genre, writing
  beats, basslines, chords and melodies as Strudel-style patterns, arranging builds and drops, mixing, fixing what
  the critic finds, and saving the result. Use it whenever someone wants to make, change, improve or finish a song,
  beat or loop with claudius, mentions its MCP tools (new_song, run_script, listen, mutate) or the claudius
  command, or asks for house, techno, trap, lofi, drum and bass, synthwave or future bass music while claudius is
  installed or connected.
---

# Making songs with claudius

claudius writes music the way a producer works in a DAW: drum lanes, a bass, chords, a lead, an arrangement with
builds and drops, a mix and a master. Every sound is real audio, synthesized in code or recorded (sample packs,
Apple Loops, the person's own files), never General MIDI. After each change it renders the song and a critic
judges it. You do the work; the person you make music with decides what sounds good.

## Find out how to reach it

- **MCP tools** (`new_song`, `get_song`, `run_script`, `listen` and the others, often shown with a `claudius`
  prefix). Prefer these: the person hears every change in the claudius studio while you work.
- **The `claudius` command**, when you have a terminal but no tools. A song is a folder (`song.wav`, `song.rhai`,
  `blueprint.json`, `verdict.json`); [references/tools.md](references/tools.md) maps every tool to its command.
- **Neither**: claudius is not installed or not connected. Installing takes `curl -fsSL
  https://claudius.pages.dev/install.sh | sh` (macOS, Linux) and `claudius setup`; `claudius connect` adds it to an
  assistant. If the claudius-setup skill is available, it covers this in detail.

## The loop

1. **Get the brief.** Genre or mood, tempo feel, a song it should be like, how long. Ask one or two questions at
   most, pick sensible defaults for the rest, and say which you picked.
2. **Start.** `new_song` with a genre (and seed, bpm, key if they matter) gives a complete, arranged song in 10 to
   60 seconds. A script that begins with `blank("<genre>")` builds one from your own patterns instead. Use the
   generator when the person wants a song; use patterns when they describe the parts.
3. **Listen.** Every edit through the tools renders and returns the verdict. On the command line, `claudius script`
   and `claudius render` write the folder and print the verdict.
4. **Change one thing**, from what the person says or from the verdict's "Fix first" list, then listen again. A
   render takes 10 to 60 seconds and a fix usually moves the score a point or less, so changing several things at
   once hides which one helped and which one hurt.
5. **Undo** a change that made it worse (`undo`; on the command line, go back to the previous folder), and tell the
   person you did. A change the person asked for, or one that serves the brief, can cost a little: under a point with
   no new issue is a matter of taste, so keep it and say what it cost. Undo it when it costs more or raises an issue.
6. **Save** when they're happy (`save`), and tell them where the folder is. On the command line, give `--out` a
   folder they know, such as `~/Music/claudius`: by default songs go into `out/` in the current folder.

## Editing with scripts

A script runs against the song that is open and changes only what it names. `get_song` returns the whole song as a
script; running it unchanged changes nothing, which also makes it the best way to read a song before editing it.

Before your first script in a session, read the language reference from the installed claudius (`script_reference`,
or `claudius reference`): it always matches the version that runs your code. The essentials, with recipes, are in
[references/script.md](references/script.md).

```js
blank("house");
tempo(124);
key("A minor");
prog("A", "i VI III VII");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("bass", chord("prog").root().struct("~ x ~ x").s("sub_bass"));
track("keys", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing().s("epiano").room(0.25));
form([sec("intro", 8, "drums keys"), sec("drop", 16, "*").impact(), sec("break", 8, "keys"), sec("drop", 16, "*")]);
```

- Several edits in a row: `run_script` with `listen: false`, then one `listen`.
- The script is the song's arrangement, parts and mix, not all of it: the master and bus effects (a lofi song's tape
  and vinyl crackle, say) come from the genre, and a script cannot change them yet. On the command line they are in
  `blueprint.json` (`mix.master` and `mix.buses`), which `claudius render` plays after a careful edit.
- An error names the line and what it accepts (`unknown sound 'bdd' (drums: bd kick ...)`). Fix that and run again;
  don't guess around it.

## Reading the verdict

The total out of 100 combines mix (the meters), arc (does loudness follow the arrangement), theory (the notes), and,
when the listening models are loaded, genre and production. Each issue has a severity, a sentence, and a target that
says where the fix lives:

| target | what to change |
|---|---|
| `master` | loudness or true peak: usually the mix balance, rarely `master()` |
| `band:<band>+` / `band:<band>-` | the tracks that live in that band: levels, sounds, filters |
| `width+` / `width-` | panning, stereo pads and effects, a centred bass |
| `form` | section energies and layers, section lengths, the song's length |
| `part:<name>` | how that part is written: its style, seed, density or register, or its pattern |
| `genre`, `production` | sounds, patterns and tempo typical of the genre; fix band issues first |

[references/verdict.md](references/verdict.md) has every issue the critic raises and a change that answers it.

A score in the high 80s means nothing measurable is badly wrong; it does not mean the song is good. The genre
reading is a hint, since styles at the same tempo sound alike to the model. Fix what the person can hear too, and
stop when they're happy rather than chasing points against their taste.

## Making a section hit harder

Every verdict measures each section's loudness: the tools' reply has a "loudness by section" line, and on the command
line `verdict.json` has a `section_<n>_<name>_lufs` measure for each.
The critic itself only checks the drop against the break, so compare the drop with the groove and the build too: a
drop that should hit harder is usually 1 to 3 LU above the groove and 3 to 7 above the break.

`mix()` sets a track for the whole song, so sections differ by what plays in them and how:

- which tracks play: thin the sections before the drop (`section("build", #{ layers: "..." })`), and keep a layer
  or two for the drop only (`layer("drop", "lead", true)`);
- drum variants: a `sparse` pattern for the quieter sections (`drum("kick", "sparse", "x.......x.......")`,
  `section("groove", #{ variant: "sparse" })`), velocities in the step string (`X` accents, `1` to `9` for tenths);
- the moment itself: `.build()` (riser, snare roll, filter sweep) and `.gap()` (a beat of silence) on the section
  before, `.impact()` (crash and hit) on the drop.

A section's `energy` is what the critic expects of it and decides where new tracks go; it does not change the sound
by itself.

## Letting claudius try things

- `mutate` (count, keep_best) tries a few variations a producer would try, biased toward the issues, and keeps the
  best one if it beats the song. Good for ideas or when the score is stuck.
- `evolve` (generations, children) searches longer, 10 to 40 seconds per generation. When it outlasts the call,
  follow it with `status`.

Afterwards, tell the person what changed (the log says) and offer to undo it.

## Talking about the music

- Describe each change in musical terms: "the hats are 2 dB louder and sit a little right; the drop now opens with
  a crash".
- With beginners, explain a word the first time (a "drop" is where the full beat comes back in). With producers,
  use DAW terms.
- claudius cannot sing or write lyrics. It plays recorded voices (`voice`, `choir`) and chops of the person's own
  recordings (see the claudius-remix skill).

## Saving and sharing

`save` (or `claudius script ... --out DIR`) writes `song.wav`, `song.mp3` when ffmpeg is installed, `song.rhai`,
`blueprint.json`, `verdict.json`, and `credits.txt` when the song uses sample packs that ask for credit; `save` also
writes `stems/`, one file per track, as does `claudius render --stems`.
Tell the person about `credits.txt` before they publish: CC BY packs must be credited. Paths always mean the
computer running claudius, and a claudius shared over a network only reads and writes its own folders.

## Reference files

- [references/script.md](references/script.md): the language, sounds, recipes and the mistakes to avoid
- [references/verdict.md](references/verdict.md): every issue the critic raises, and what to change
- [references/genres.md](references/genres.md): a starting point for each of the eight genres
- [references/tools.md](references/tools.md): the MCP tools and the commands, side by side
