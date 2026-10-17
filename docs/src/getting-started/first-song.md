# Your first song

This page takes you from a fresh install to a song you made and changed yourself, in about ten minutes. It assumes
you have followed [Install](install.md) and have the `claudius` command.

## Check the installation

```sh
claudius doctor
```

```text
claudius 0.1.0 (macos aarch64)
  ok  web studio      ~/hobby/claudius/crates/claudius-ui/dist
  ok  sample packs    14 default packs (1.2 GB) in ~/Library/Application Support/claudius/packs
  ok  ears            models in ~/.cache/earshot-rs, AudioSet tagger too
  ok  Apple Loops     14096 loops and samples
  ok  ffmpeg          9.0.2 (writes song.mp3)
  ok  port 7878       free
  ..  Claude Desktop  not connected: `claudius connect claude-desktop`
  ..  Cursor          not connected: `claudius connect cursor`

Everything claudius needs is here.
```

Two things matter for this page: the **sample packs** (recorded instruments and drum machines) and the **ears** (the
listening models the critic uses). If either is missing, `claudius setup` downloads them: about 1.1 GB of packs and
0.8 GB of models, once. Apple Loops are optional (claudius uses them when GarageBand or Logic content is installed),
and without ffmpeg you get `song.wav` but no `song.mp3`.

## Make a song

```sh
claudius make --genre house --seed 9
```

claudius composes a song in the style, arranges it, renders it with real sounds, mixes and masters it, and judges
the result. On this machine it printed:

```text
ears loaded in 3.5 s
Crimson Tide — house at 124 BPM in G dorian, 64 bars (2:04)
  progression A: i7 ii7 i7 IV7
  progression B: i7 VIImaj7 i7 IV7
  form: intro 8 | groove 16 | break 8 | drop 24 | outro 8
  tracks: kick (sampled bd (tr909:bd)), clap (sampled cp (tr909:cp)), hat (sampled hh (tr909:hh)), ohat (sampled oh (tr909:oh)), shaker (shaker cut from 'Cosmic Harmony Shaker' (Apple Loops)), perc (sampled rim (tr909:rim)), bass (synth patch 'saw_bass'), chords (synth patch 'stab'), pad (synth patch 'warm_pad'), lead (synth patch 'pluck'), arp (synth patch 'bell'), crash (synth crash), riser (synth riser), impact (synth impact), downlifter (synth downlifter), reverse_crash (synth reverse_crash)
  master: -10.0 LUFS, true peak -1.0 dBTP (pre-master -9.3 LUFS); done in 24.2 s
  ears: 'house music' z +3.2 (closest confusion 'techno music' +3.7); production z +1.6 vs 'a low quality recording with hiss' +1.7; p_real 0.77
        sounds like: techno music, drum and bass music, tech house music, house music
  verdict: 84/100 (arc 0.92, genre 0.58, mix 1.00, production 0.47, theory 1.00). Fix first: production reads close to 'a low quality recording with hiss' (z +1.7) vs professionally mixed (z +1.6); reads as 'techno music' (z +3.7) more than 'house music' (z +3.2)
  wrote out/house-crimson-tide-9/song.mp3
```

The same genre and seed give the same song on any machine. What is installed changes only which recordings play:
without Apple Loops, the shaker cut from them is synthesized, and without the sample packs so are the 909 drums, but
the notes and drum patterns stay the same. Leave out `--seed` and you get a new song each time.

Open `out/house-crimson-tide-9/song.mp3` and listen. Reading the summary while it plays:

- **Crimson Tide**: the title the generator chose, the style, tempo, key and length.
- **progression**: the chord sequences, as Roman numerals ([what those are](../learn/basics/chords.md#the-numbers-in-the-progression)).
- **form**: the sections and their lengths in bars. The drop is the loud part in the middle.
- **tracks**: every part and the sound playing it: a real Roland TR-909 for most of the drums, synthesizers for bass,
  chords, pad, lead and arpeggio, and the transition effects at the end.
- **verdict**: the critic's score out of 100 and what it would fix first. [Listening, judging,
  finishing](../learn/basics/finishing.md) explains every part of it.

The folder holds the song (`song.mp3`, and `song.wav` in 24-bit), the song's full description (`blueprint.json`),
the arranged timeline (`project.json`), the critic's report (`verdict.json`) and credits for the recorded
instruments whose licences ask for them (`credits.txt`).

## Change it

A script can edit a saved song. Make a file called `edit.rhai`:

```js packs
tempo(122);
part("chords", #{ preset: "piano" });
mix("lead", #{ level: -2, reverb: 0.35 });
section("drop", #{ bars: 32 });
```

It slows the song down a little, plays the chords on a sampled grand piano instead of a synth stab, pulls the lead
back and into more reverb, and makes the drop 32 bars long. Run it on the song:

```sh
claudius script edit.rhai --on out/house-crimson-tide-9
```

```text
ears loaded in 6.2 s
Crimson Tide — house at 122 BPM in G dorian, 72 bars (2:22)
  progression A: i7 ii7 i7 IV7
  progression B: i7 VIImaj7 i7 IV7
  form: intro 8 | groove 16 | break 8 | drop 32 | outro 8
  tracks: kick (sampled bd (tr909:bd)), clap (sampled cp (tr909:cp)), hat (sampled hh (tr909:hh)), ohat (sampled oh (tr909:oh)), shaker (shaker cut from 'Cosmic Harmony Shaker' (Apple Loops)), perc (sampled rim (tr909:rim)), bass (synth patch 'saw_bass'), chords (sampled piano (salamander:piano)), pad (synth patch 'warm_pad'), lead (synth patch 'pluck'), arp (synth patch 'bell'), crash (synth crash), riser (synth riser), impact (synth impact), downlifter (synth downlifter), reverse_crash (synth reverse_crash)
  master: -10.1 LUFS, true peak -1.0 dBTP (pre-master -9.3 LUFS); done in 22.4 s
  ears: 'house music' z +3.4 (closest confusion 'techno music' +3.7); production z +1.6 vs 'a low quality recording with hiss' +1.6; p_real 0.80
        sounds like: techno music, tech house music, house music, drum and bass music
  verdict: 85/100 (arc 0.92, genre 0.63, mix 1.00, production 0.49, theory 1.00). Fix first: production reads close to 'a low quality recording with hiss' (z +1.6) vs professionally mixed (z +1.6); reads as 'techno music' (z +3.7) more than 'house music' (z +3.4)
  wrote out/house-crimson-tide-9/song.mp3
```

Everything you did not name stayed as it was. The chords are now `sampled piano`, the song is 72 bars, and the score
moved a point: the critic is a guide, and your ears decide whether the piano is better.

The edited song is written back into the same folder, because its genre, title and seed are the same, so the
original files are replaced. To keep both, add `title("Crimson Tide (piano)")` to the script, or pass
`--out another-folder`.

## Read the song as a script

Every song can be printed as a script that describes it completely enough to edit. An empty script changes nothing,
so this prints the song as it is:

```sh
echo "" | claudius script - --on out/house-crimson-tide-9 --print
```

The output starts like this:

```text
// Crimson Tide — house (seed 9)
// song("house", 9);   // uncomment to start over from the generator
title("Crimson Tide"); tempo(122); key("G dorian"); swing(0.5257842);
humanize(0.25);
prog("A", "i7 ii7 i7 IV7");
prog("B", "i7 VIImaj7 i7 IV7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum_sound("kick", "tr909:bd");
```

and goes on with every drum lane and the sound it plays, every part, the form and the mix. Copy any line into your
`edit.rhai`, change it, and run the script again. [Songs as scripts](../language/printing.md) explains the format.

## Make a beat from nothing

`blank()` starts an empty song instead of a generated one. Save this as `beat.rhai`:

```js
blank("house");
tempo(124); key("A minor");
prog("A", "Am7 Fmaj7 Dm7 Em7"); bars_per_chord(1);
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4"));
track("bass",  chord("prog").root().struct("~ x ~ x ~ x ~ [x x]").s("organ_bass").legato(0.7));
track("keys",  chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").room(0.25));
form([sec("intro", 8, "drums keys").filter_in(), sec("drop", 16, "*").impact()]);
```

and render it with `claudius script beat.rhai`. You hear eight bars of drums with electric piano chords coming into
focus as a filter opens on them, then a crash and sixteen bars with an organ bass bouncing between the kicks on the
root of each chord. Each line is one idea: the style, the tempo and key, the chords, three tracks, and two
sections.

## Where to go from here

- Never made music before? [How a track is built](../learn/basics/how-a-track-is-built.md) starts a short course that
  ends with a song you understand, line by line.
- Already make music? [claudius for DAW users](../learn/producers/daw-users.md) maps what you know onto claudius.
- [The studio](studio.md) is the same engine in a window: an arrangement view, a step sequencer, a mixer and the
  verdict updating as you edit.
- [Working with an assistant](assistants.md): `claudius connect claude-desktop` (or `claude-code`, `cursor`) lets
  an assistant make and edit songs while you listen.
- [The cookbook](../cookbook/four-on-the-floor.md) has complete songs in eight styles.
