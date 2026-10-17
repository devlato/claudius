# Remaking a song from audio

`claudius remake` listens to a recording, works out its skeleton (tempo, key, chords, sections, drum habits, the
style of its bass) and writes a new song on that skeleton in any genre, with its own parts and sounds. No audio from
the original is used. This recipe remakes a song whose answers we know, so you can see what the analysis gets right
and what it gets wrong, then edits and evolves the result.

## A song to remake

Any audio file ffmpeg can read works. To follow along without one, render the house recipe from
[House, deep house and techno](four-on-the-floor.md#house-glass-avenue) first: `claudius script house.rhai` writes
`out/house-glass-avenue-1/song.mp3`. We know exactly what is in it: 122 BPM, A minor, swing 0.58, the progression
`Am7 Fmaj7 Dm7 Em7` (`i7 VImaj7 iv7 v7`), and six sections (intro 8, groove 16, break 8, build 8, drop 16,
outro 8).

## What the analysis hears

```sh
claudius analyze out/house-glass-avenue-1/song.mp3
```

```text
122.0 BPM (pulse clarity 0.58), A minor (confidence 0.75), swing 0.50, 64 bars from 0.00 s
  progression A: III VImaj7 iv7 v7  |  B: i VII v7 i7  (1 bar(s) per chord)
  bar   0 +24  drop    energy 0.97  kick bass hats
  bar  24 + 8  break   energy 0.30  
  bar  32 +32  drop    energy 1.00  kick bass hats
  kick  x...X...X...X...
  snare .-x-X-x-.-x-X-x-
  hats  .-X-x-X-.-X-x-X-
  bass style: syncopated
```

Against what we know:

- **Tempo, key, length, bar grid**: right. The tempo comes from onset autocorrelation and a grid fitted to the kicks;
  the key from the chroma of the whole song.
- **Swing**: missed (0.50 for 0.58). Swing is measured from the hi-hats on the off-beat sixteenths, and this song's
  hats play eighths.
- **Progression**: three chords out of four right. The first chord, Am7 (A C E G), was heard as C (`III`): it
  contains all of C major's notes, and chords are read from chroma, the energy of each note name per bar.
- **Sections**: the analysis cuts the song where the energy changes, in 4-bar steps, and labels the parts by energy.
  It found the break, but merged the intro and groove into one 24-bar "drop", and the build, drop and outro into
  another. It reports which sections have kick, bass and hats.
- **Drum habits**: per sixteenth step, `X` very likely, `x` likely, `.` occasional, `-` never. The kick is exactly
  right. The snare line shows the claps on steps 5 and 13 (beats 2 and 4) plus hat energy leaking into the snare's
  band on the eighths.
- **Bass style**: `syncopated`, a fair name for an offbeat line with pickups.

The analysis is approximate by nature (it reads a finished mix, not stems) and works best on steady electronic music.

## Remake it in another genre

```sh
claudius remake out/house-glass-avenue-1/song.mp3 --genre deep_house --seed 3
```

It prints the analysis again, then the new song:

```text
Faded Engine — deep_house at 122 BPM in A minor, 64 bars (2:06)
  progression A: III VImaj7 iv7 v7
  progression B: i VII v7 i7
  form: drop1 24 | break2 8 | drop3 32
  tracks: kick (sampled bd (tr909:bd)), clap (sampled cp (tr909:cp)), hat (sampled hh (tr909:hh)), shaker (shaker cut from 'Loose Leaf Beat Shaker' (Apple Loops)), perc (sampled rim (tr909:rim)), bass (synth patch 'sub_bass'), chords (synth patch 'keys'), pad (synth patch 'warm_pad'), lead (sampled rhodes (jrhodes:rhodes)), top (loop 'Deep Blue Void Hi-Hat' (House, Apple Loops) (126 BPM)), crash (synth crash), riser (synth riser), downlifter (synth downlifter), reverse_crash (synth reverse_crash)
  master: -11.0 LUFS, true peak -1.0 dBTP (pre-master -8.8 LUFS); done in 13.3 s
  ears: 'house music' z +3.3 (closest confusion 'techno music' +3.1); production z +1.3 vs 'a low quality recording with hiss' +1.2; p_real 0.79
        sounds like: house music, drum and bass music, techno music, tech house music
  verdict: 89/100 (arc 1.00, genre 0.79, mix 0.98, production 0.53, theory 1.00). Fix first: production reads close to 'a low quality recording with hiss' (z +1.2) vs professionally mixed (z +1.3); dull 4-8 kHz: hats, claps and snare lack bite (presence -26.9 dB, healthy from -26)
  wrote out/remake-deep_house-faded-engine-3/song.mp3
```

What the remake keeps from the analysis, and what it takes from the genre:

| from the recording | from the genre and the seed |
|---|---|
| tempo (rounded), key, swing | every sound: kits, Apple Loops one-shots, synth patches, sampled instruments |
| progressions `A` and `B`, bars per chord | every part's notes, written by the theory engine on those progressions |
| the sections: lengths (rounded to 4 bars), kinds and energies | which parts play where, chosen from what the analysis heard (kick, bass, hats) and each section's energy |
| the kick, snare and hat habits, as the drum lanes' step probabilities | the drum lanes the genre has, and its other lanes |
| the bass style, when the genre has it | the mix, transitions and master |

The sections get builds and gaps before drops and impacts on them, an intro (if there is one) filters in, and
breaks, intros and outros play progression `B`. On this machine the shaker was cut from Apple Loops and the `top`
percussion is an Apple Loop; without Apple Loops, the shaker is synthesized and the loop is left out.

The score is high because the skeleton is simple and the sections are clearly layered: the arc check is perfect.

## Edit the remake

The remake is an ordinary song. A script can fix what the analysis got wrong and replace any part with your own.
This one corrects the first chord and puts a written motif in place of the generated Rhodes lead:

```js packs
prog("A", "i7 VImaj7 iv7 v7");
track("lead", n("<[4 ~ 4 2 4 ~ 6 4] [5 ~ 5 4 2 ~ 0 ~] [3 ~ 3 2 0 ~ 2 3] [4 ~ ~ 6 4 ~ ~ ~]>")
                .octave(4).s("rhodes").room(0.3).delay(0.2));
```

```sh
claudius script fix.rhai --on out/remake-deep_house-faded-engine-3
```

```text
  verdict: 87/100 (arc 1.00, genre 0.68, mix 0.98, production 0.46, theory 1.00). Fix first: production reads close to 'a low quality recording with hiss' (z +1.3) vs professionally mixed (z +1.1); dull 4-8 kHz: hats, claps and snare lack bite (presence -26.8 dB, healthy from -26)
  wrote out/deep_house-faded-engine-3/song.mp3
```

The generated parts follow the corrected progression on their own, since they are written from it. The new lead
takes the generated lead's place in every section: both drops and the break. The critic liked it slightly less (the
genre reading moved towards techno), which is a matter of taste you settle by listening. The edited song is written
to `out/deep_house-faded-engine-3/`, next to the remake rather than over it, because a script names its folder after
the genre, title and seed.

## Evolve it

```sh
claudius evolve --genre deep_house --from out/deep_house-faded-engine-3/blueprint.json
```

starts [evolution](../learn/producers/genres-and-evolution.md#evolution) from the edited remake instead of fresh
songs. The lead, a pattern track, keeps its notes; the drums, parts, mix, form and global feel are mutated.

## Using the audio itself

A remake uses none of the recording's sound. Two commands do:

- `claudius chop song.mp3` cuts the recording into bar loops (2 bars by default, `--bars`) and drum one-shots, writes
  them into `out/chops/`, and adds them to your user library, where generated songs can pick them up as drum
  sounds. `--find "a vocal chop"` also uses the ears to cut out the moments that best match a description, three per
  description (`--top`).
- `claudius make --genre house --sample out/chops/<loop>.wav` flips a recording into a new song: warped to the song's
  tempo, transposed to its key, re-sliced in half-beat pieces (or played straight) and placed in the high-energy
  sections, as a track named `sample1`.

Only use recordings you have the right to use this way. `analyze` and `remake` take nothing but measurements from a
recording; `chop` and `make --sample` put its audio in your song.

From an assistant, the MCP tools `analyze` and `remake` take a file path and do the same as the commands; a remake
becomes the song open in the studio.
