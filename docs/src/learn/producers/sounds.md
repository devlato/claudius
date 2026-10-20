# Sounds, samples and sound design

claudius plays two kinds of audio: sounds synthesized in code and sounds that were recorded. There is no General MIDI
and no neural audio generation; a note is only ever the score that plays one of these sounds. This page covers where
each comes from, how genres choose them, and how much of them you can shape. The names you can use are in the
[Sounds reference](../../language/sounds.md).

## Synthesized

**Drum voices.** Thirteen drum circuits rendered in code: kick, snare, clap, closed and open hats, ride, crash, tom,
rim, shaker, cowbell, snap and a tuned percussion voice. Each has nine parameters: tune, decay, tone, punch, snap,
drive, pitch-envelope depth and time, and room. The generator draws them from the genre's ranges (a house kick decays
in 0.32–0.55 s with 3–8 dB of drive) and, in genres that ask for it, tunes the kick's fundamental to the tonic or
fifth of the key. A variation (`bd:3`) moves the voice a few steps through its parameters.

**The synth engine.** Oscillators with polyBLEP anti-aliasing (saw, square, pulse, triangle, sine), unison with
per-voice analog drift, a sub oscillator and noise, FM with a tine component for electric pianos, state-variable
and ladder filters with an envelope, key tracking and velocity, an LFO for vibrato, filter and amplitude (free or
tempo-synced), glide, mono and legato modes, and drive. Twenty factory presets cover basses (`sub_bass`, `808`,
`reese`, `acid`, `wobble` ...), keys (`epiano`, `keys`, `organ`), pads, plucks, leads and bells. A generated part takes
a copy of a preset with its cutoff and detune varied a little; evolution tweaks cutoff, resonance, release, drive and
detune further.

**Transitions.** Risers, downlifters, impacts, reverse crashes and filter sweeps are rendered at the length the
arrangement asks for, with the riser pitched to the key. The crash is a drum voice or a recorded one-shot.

`claudius sounds` renders every drum voice, preset and transition into `out/sounds/` for auditioning.

## Recorded

**Sample packs.** Multi-sampled instruments and drum machines under licences that allow making and publishing music
with them. `claudius-packs install --defaults` installs 22 packs (a 3.1 GB download, 1.5 GB on disk):

| pack | what | licence |
|---|---|---|
| `salamander` | Yamaha C5 grand piano | CC BY 3.0 |
| `epianos` | Wurlitzer, CP80, Pianet | CC BY 3.0 |
| `jrhodes` | Rhodes Mark I | free for music |
| `vsco2` | chamber orchestra: strings, brass, woodwinds, harp, timpani | CC0 |
| `vcsl` | mallets, bowed vibraphone, wine glasses, pipe organ, sax, harmonica, a TX81Z FM synth, zithers, world percussion, effects | CC0 |
| `tr808`, `tr909`, `tr707` | Roland drum machines | CC0; free use; public domain |
| `uzu`, `synthdrums` | electronic drum kits | Unlicense; CC0 |
| `muldjord` | an acoustic drum kit | CC BY 4.0 |
| `freepats` | upright piano, organ, guitars, basses, synth pads and leads, glasses of water, a hang | CC0 |
| `doublebass`, `growlybass` | upright and electric bass | CC0 |
| `voice` | a sung vowel and a choir | CC0 |
| `solocello`, `erhu`, `steelpan` | a solo cello, an erhu, a steel pan | CC0; Unlicense |
| `sonicpi` | Sonic Pi's one-shots, drones, vinyl, tabla and loops | CC0 |
| `body`, `zamenhof` | body percussion; electronic glitch kits | CC0 |
| `miscellania` | sound effects: sci-fi, metal, glass, chains, bells, glissandi | CC0 |

The instruments have velocity layers and round robins (the Salamander piano has five layers; the 909 kick eleven
layers and 126 variations). A song that plays a pack whose licence asks for attribution gets a `credits.txt` in its
folder. `claudius-packs list` shows what is installed, `claudius-packs audition` renders every instrument, and
`claudius-packs add-map github:user/repo` adds any Strudel sample map, recording its licence (or warning when it has
none).

**Apple Loops.** When GarageBand or Logic content is installed, claudius finds it where Apple put it and uses it
three ways: as one-shots cut from drum loops for the drum lanes of generated songs (each lane uses a recorded hit
with a 75 % chance in `claudius make`), as texture loops (house's `top` percussion loop), and, with `--loops` or
`song(genre, #{ loops: true })`, as the backbone of a song built around one loop family: its chords, bass and beat,
warped to the tempo and transposed to the key. Apple licenses the loops for use in your own music; claudius reads
them in place and only caches the one-shots it cuts (in `~/Library/Caches/claudius`). In patterns,
`s("bd sd").bank("apple")` plays one-shots cut from them.

**Your recordings.** `.s("/path/file.wav")` plays a file as a sampler (pitch by playback speed, `c4` as recorded).
`claudius chop song.mp3` cuts bar loops and drum one-shots from any recording into your library, and `--find "a
vocal chop"` uses the ears to pull out moments that match a description. `claudius make --sample file` flips a
recording into a new song: warped to the tempo, transposed to the key, re-sliced in half-beat pieces (or played
straight) and placed in the high-energy sections. See [Remaking a song from audio](../../cookbook/remake.md).

## How genres choose sounds

Every genre profile lists, per part, the presets and instruments it may use, most preferred first where it matters:
`"rhodes|epiano"` plays the sampled Rhodes when its pack is installed and the FM electric piano when not. Lofi reaches
for Rhodes, upright bass, vibes and kalimba; house for 909s, organ basses, stabs and piano; trap for an 808, bells
and pizzicato. Drum lanes are played on a recorded machine with a genre-specific chance (909 in 45 % of house songs
and 50 % of techno songs, 808 in 60 % of trap songs, the acoustic kit in 30 % of lofi and drum and bass songs) and
stay synthesized otherwise. With the packs or Apple Loops missing, those sounds fall back to synthesis and the rest
of the song is unchanged: the same seed writes the same notes and drum patterns either way.

## Shaping a sound

From a script you choose sounds; you do not edit them. What you can do:

- **pick** a preset, an instrument, a kit or a file, per track;
- **play** it differently: velocity, note length, register, glide (overlapping notes on mono presets);
- **process** a pattern track with `.hpf()`, `.lpf()`, `.crush()` and `.distort()` (see
  [Inserts](../../language/mixing.md#inserts-on-pattern-tracks)), and place it with level, pan and the two sends;
- **swap** a generated part's preset with `part("chords", #{ preset: "piano" })`, which keeps its notes, and a drum
  lane's sound with [`drum_sound("kick", "tr909:bd")`](../../language/song-functions.md#drum_sound), which keeps its
  pattern.

Synth patch parameters, drum voice parameters, bus chains and the master chain are set by the generator and the genre,
and changed by [mutation](genres-and-evolution.md) (patch tweaks, drum voice edits, moving a song's drums to another
kit). A filtered, crushed piano:

```js packs
blank("lofi");
key("D minor"); prog("A", "Dm9 Gm9 Bbmaj7 A7"); bars_per_chord(1);
track("keys", chord("prog").voicing(50, 72).s("piano").lpf(2500).crush(10).room(0.3));
```

## Sounds and the critic

The ears hear recordings and synthesis differently. Songs built only from synthesis read as less "professionally
mixed" to earshot than songs with sampled instruments, and as more "computer-generated". When the production score
is low, swapping a synth part for a recorded instrument (piano, Rhodes, strings, a real drum machine) is usually the
most effective change. The genre score responds to the genre's signature sounds: a 909 for house and techno, an 808
for trap, a dusty kit and a Rhodes for lofi.
