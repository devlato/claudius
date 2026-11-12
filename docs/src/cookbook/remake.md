# Remaking a song from audio

`claudius remake` listens to a recording and reworks it in any genre, so that it stays the song it is: all of it
plays through, its voice and its chords as they are, without its drums and its bass (taken apart from the rest with
earshot's models; without them, its low end), in its own sections and on its chords, with the genre's drums, bass and
mix under it. With `--flip` it makes a new song of the genre on the
recording's own sound instead: its hook and passage after passage of it, its voice in them, its drums and a flip cut
from all over it, under the genre's drums, bass, parts and mix. With `--rebuild` it makes the song itself again, every part of
it played on claudius's sounds: its drums, its bass and the rest as the notes and hits it plays, so that each can be
changed. With `--skeleton` it writes a new song on the recording's skeleton only (tempo, key, chords, sections, drum
habits, the style of its bass), with none of its audio. This recipe remakes a song whose answers we know, so you can
see what the analysis gets right and what it gets wrong, reworks it, flips it, rebuilds it, remakes its skeleton, then
edits and evolves the result.

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

- **Tempo, key, length, bar grid**: right. The tempo comes from onset autocorrelation, the tempo whose grid lies on
  the onsets best and the kit's backbeat ([how](../engineering/evolution-and-analysis.md#reading-a-song)), and a grid
  fitted to the kicks; the key from the chroma of the whole song.
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

## Rework it in another genre

```sh
claudius remake out/house-glass-avenue-1/song.mp3 --genre trap --seed 3
```

It prints the analysis again, then the rework:

```text
Faded Engine — trap at 122 BPM in A minor, 64 bars (2:06)
  progression A: i7 VI iv7 VII III VImaj7 iv7 bi° i VI iv7 v i VI iv7 v i VI iv7 v i VI iv7 v
  progression B: i VII v7 i7 i VII v7 i7
  progression C: III VImaj7 iv7 v7 VImaj7 VImaj7 iv7 VImaj7 i VImaj7 iv7 v i VImaj7 iv7 v i VImaj7 iv7 v ...
  form: drop 24 | break 8 | drop2 32
  tracks: kick (sampled bd (tr808:bd)), snare (sampled sd (tr808:sd)), clap (sampled cp (tr808:cp)), hat (sampled hh (tr808:hh)), ohat (sampled oh (tr808:oh)), perc (sampled rim (tr808:rim)), bass (synth patch '808'), original (loop song, all of it (122 BPM)), crash (synth crash), impact (synth impact)
  ears: 'trap beat' z +3.5 (closest confusion 'hip hop beat with heavy bass' +4.2); production z +1.4 vs 'a distorted clipped recording' +0.6; p_real 0.55
        sounds like: hip hop beat with heavy bass, trap beat, drum and bass music, glitch hop music
  wrote out/remake-trap-faded-engine-3/song.mp3
```

Its history says what it kept:

```text
reworked from 'song.mp3': all 64 bars of it, its voice and its chords as they are, without its drums and its bass, in its own 3 sections and on its chords; trap's bass, kick, snare, clap, hat, ohat, perc under it
```

- **All of it**, from its first downbeat to its end, plays through as `original`, at its own speed and pitch: the
  piano, the pad, the voice and the lead as they are, each section brought halfway to the middle of their levels (by
  9 dB at most, the gain moving over the last beat before a section), since the genre's drums keep one level: a
  quiet verse would all but go under them, and a loud hook over them. The song keeps its shape, at half its range. With earshot's models, claudius takes the recording apart into
  its vocals, drums, bass and the rest (Spleeter's nets, about 8 s for a four-minute song, kept for the next time).
  `original` is then the rest, cut under 80 Hz, which leaves the drums and the low end to trap's kick and 808. When the
  recording sings, its voice plays all through on a track of its own, `vocals`, mixed as a voice, so that you can
  turn it up, give it effects of its own or mute it, and the history says "its voice on a track of its own (vocals)".
  Without the models, what strikes is taken out of it (`fft::harmonic` keeps what sustains), and so is what is under
  150 Hz, voice and all, and the history says "its low end" instead. What is not its voice goes through the genre's
  treatment of a phrase it samples: boom bap's tape and 12 bits, lo-fi's dust. A band that speeds up and slows down
  is followed beat for beat, and its bars are each stretched to one length, so that it stays on the song's grid.
- **Its own sections**, as the analysis heard them, bar for bar: the song is as long as the recording and changes
  where it changes.
- **Its chords**, a bar each, a progression for each run of them (`A`, `B`, `C`), which the bass plays the roots of.
  The analysis hears roots best, and the bass is all of the genre that plays a pitch: chords, pads, keys and leads
  of the genre would play against the recording's own, and against it where a chord was heard wrong (`bi°` above).
- **Its swing**, so that the genre's drums sit with it.

The tempo is the recording's own, or double it when that is nearer the genre's range (two bars of the song to each
of the recording's, a chord each), so that nothing is stretched: 122 BPM is under trap's 130 to 150, and 244 further
from it. It is halved (one bar of the song to two of the recording's) when that is nearer and the recording holds
each chord through two bars, three pairs of bars in four, since a bar of the song plays one chord: a recording of
170 BPM reworked as hip hop plays at 85, its beat in half time, where at 170 hip hop's drums would play as fast as
drum and bass. A slow song heard at twice its tempo, whose bars are each half of one, always holds its chords so, and
plays at its own again. The reply says when the song is in double or half time. `--bpm` sets another tempo, and
stretches the recording to it. The recording is kept in `out/takes/song/`, beside the song. To hear more of the genre,
add its parts back over the recording in a script, or turn the recording down where they play.

## Flip it into another genre

```sh
claudius remake out/house-glass-avenue-1/song.mp3 --genre deep_house --seed 3 --flip
```

It prints the analysis again, then the flip:

```text
Faded Engine — deep_house at 122 BPM in A minor, 120 bars (3:56)
  progression A: bIImaj7 VI iv v bIImaj7 VI iv v7
  progression A2: VImaj7 VII v7 i7 i VII v7 i7
  progression A3: i7 VImaj7 iv7 v7 III VImaj7 iv7 v7
  progression A4: III VImaj7 iv7 v7 III VImaj7 iv7 v III VImaj7 iv7 v7 i7 VImaj7 iv7 v7 i VImaj7 iv7 v7 i VImaj7 iv7 v bIImaj7 VI iv v bIImaj7 VI iv v7
  progression A5: i7 VImaj7 iv7 v i7 VImaj7 iv7 v i VImaj7 iv7 v i VI iv7 v
  progression B: iiø7 v7 i7 i7 iiø7 v7 i7 VI7
  form: intro 16 | groove 16 | break 8 | build 8 | drop 32 | drop2 32 | outro 8
  tracks: kick (sampled bd (tr909:bd)), clap (sampled cp (tr909:cp)), hat (sampled hh (tr909:hh)), shaker (synth shaker), perc (sampled rim (tr909:rim)), bass (synth patch 'deep_bass'), chords (synth patch 'keys'), pad (synth patch 'vowel_pad'), lead (synth patch 'epiano'), sample2 (loop song, another passage (122 BPM)), flip (loop song, a flip of it (122 BPM)), sample3 (loop song, bars 1-8 (122 BPM)), sample4 (loop song, bars 33-64 (122 BPM)), sample5 (loop song, bars 9-24 (122 BPM)), crash (synth crash), riser (synth riser)
  master: -11.1 LUFS, true peak -1.0 dBTP (pre-master -12.1 LUFS); done in 14.6 s
  ears: 'house music' z +3.5 (closest confusion 'techno music' +3.3); production z +1.7 vs 'a low quality recording with hiss' +1.5; p_real 0.68
        sounds like: drum and bass music, house music, techno music, tech house music
        aesthetics (of 10): enjoyment 7.2, production quality 8.1; held out: usefulness 8.1, complexity 5.5
  verdict: 89/100 (aesthetics 0.88, arc 0.94, genre 0.80, mix 1.00, production 0.55, theory 1.00, time 1.00). Fix first: sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause
  wrote out/remake-deep_house-faded-engine-3/song.mp3
```

Its history says what came from where:

```text
remixed from 'song.mp3': its hook (bars 57-64) without its drums, in its own key for the voice in it; another passage of it (bars 25-32); a flip of 64 half beats from 14 of its bars in intro, outro; on through it as the song goes (build bars 1-8, drop bars 33-64, drop2 bars 9-24)
sampled: phrase 'song', filtered until it lifts (another passage of it in groove, break, build)
```

All 64 of its bars play somewhere in the song: the groove and the break 25 to 32, the build 1 to 8, the first drop
33 to 64 on end and the second 9 to 24.

- **Its hook**, eight bars from the start of the loudest section with chords (the drop the analysis hears in its
  last eight bars, 57 to 64), without its drums, is the phrase the song is built on, as a
  [song built on samples](../guide/online-samples.md#songs-built-on-samples) is: the song takes its key, its chords
  (progression `A`) and a tempo it plays at from it. A hook a voice sings in plays in its own key, at its own tempo or
  stretched, rather than sped up or slowed down, which would make its singer a chipmunk or a giant, and the history
  says "in its own key for the voice in it". Chipmunk soul, whose sound is just that, speeds it up anyway. Without
  the models, it is what sustains of the recording.
- **Another passage of it**, the eight bars of its break with the pad and the chords, plays as `sample2` in the
  groove and the break, on its own chords (progression `A2`).
- **On through it**: every other section but the intro, the breaks and the outro plays as many of the recording's
  bars on end as it has itself (32 at most), of the kind it plays (a drop's or a hook's, or the rest's), from where
  the recording has the most of them that no section played before it, each on its own track and its own bars'
  chords: the build plays bars 1 to 8 as `sample3` (`A3`), the first drop the whole second half, 33 to 64, as
  `sample4` (`A4`), and the second drop 9 to 24 as `sample5` (`A5`). So a flip goes through the recording rather
  than coming round to the same two passages for minutes. The hook's own track goes, since no section plays it
  alone any more.
- **Its voice**, when it sings (the ears hear a voice, or a pitch in a voice's range carries a third of a phrase),
  sings on in those passages as it sang there: with the stems apart they are all of it but its drums. Without a voice,
  with the stems apart, they are its bass and the rest.
- **Its drums** are the break in a genre that plays one (deep house does not; boom bap and trip-hop do), cut from its
  busiest bars without the rest: its drum stem, apart, with the models.
- **A flip of it** plays in the intro and the outro: eight bars of half beats, each from another bar of the recording
  that plays the hook's chord there, at the same place in its bar (now and then another), here 14 of its bars, so
  the song is heard in pieces from all over it, still on the hook's chords. Each piece is as loud as the hook (its
  bar brought to the hook's level, by 18 dB at most), so a piece of a quiet verse does not drop away beside one of
  the chorus.

Everything else is the genre's: the drums, the bass, the chords and the pad written on the hook's chords, the form and
the mix. The tempo here is the recording's own, 122 BPM, which deep house plays; another genre plays it at a multiple
of it, faster or slower by whole semitones as a sampler plays a record, or chopped onto its grid at two thirds to three
halves of it (as boom bap, this one plays at 92, three quarters of it, chopped onto the grid at its own pitch, and its own drums are the break). The pieces are kept in
`out/takes/song/`, beside the song.

## Rebuild it, every note yours to change

```sh
claudius remake out/house-glass-avenue-1/song.mp3 --genre house --seed 3 --rebuild
```

A rebuild makes the song itself again, with every part played on claudius's sounds rather than the recording's: its
drums, its bass and the rest of it become notes and hits you can edit, in its own sections and on its chords. It
prints the analysis again, then the rebuilt song:

```text
Faded Engine — house at 122 BPM in A minor, 64 bars (2:06)
  progression A: i7 VImaj7 iv7 v7 III VImaj7 iv7 v7 i7 VImaj7 iv7 v i7 VImaj7 iv7 v7 i VImaj7 iv7 v i VImaj7 iv7 v
  progression B: VImaj7 VII v7 i7 VImaj7 VII v7 i7
  progression C: III VImaj7 iv7 v7 III VImaj7 iv7 v i7 VImaj7 iv7 v7 i7 VImaj7 iv7 v7 i VImaj7 iv v7 i VImaj7 iv7 v
  progression D: bIImaj7 VI iv v bIImaj7 VI iv v
  form: drop 24 | break 8 | drop2 24 | drop3 8
  tracks: kick (sampled bd (tr909:bd)), clap (sampled cp (tr909:cp)), hat (sampled hh (tr909:hh)), bass (synth patch 'deep_bass'), keys (sampled wurlitzer (epianos:wurlitzer)), crash (synth crash)
  master: -10.0 LUFS, true peak -1.1 dBTP (pre-master -11.2 LUFS); done in 7.0 s
  wrote out/remake-house-faded-engine-3/song.mp3
```

Its history says what it heard:

```text
rebuilt from 'song.mp3': its 4 sections bar for bar, on its chords; its drums struck as it strikes them, each section's folded into a pattern of its own (113 on clap, 232 on hat, 243 on kick); its bass as 214 notes on bass; the rest of it as 866 notes on keys
```

- **Its form** is its sections bar for bar, each on a progression of its own, its bars' chords: `A` to `D`, one to
  a section, so that every section plays its own notes rather than the first section's again.
- **Its drums** come from its drum stem (with the ears; see below): every kick, snare or clap and hat struck where it
  strikes them, each section's hits folded into a pattern of its own (the section's drum variant, `drop`, `break`
  ...), a step struck only some of the times round played with that chance, and each lane as far behind or ahead of
  the beat as the recording's hits on the beats are. A clap struck with the kick, as house claps on two and four,
  is heard as both.
- **Its bass and the rest of it** are the notes Spotify's Basic Pitch hears in their stems, on the sixteenth-note
  grid: the bass one note at a time on `bass`, the rest (keys, guitars, synths, pads together) chords and all on
  `keys`, without the faint copies an instrument's overtones leave over its notes, or the notes another instrument
  leaves in a stem 18 dB under its own.
- **Its voice**, when it sings, stays on a track of its own, `vocals`, as a rework keeps it, and the melody it sings
  is written out on `melody`, which plays in no section until you put it in one (`toggle_layer`, or `layers` in a
  script). This song's vowel line is too quiet to count as a voice.

Against what this song is: 99% of the rebuilt kicks and 98% of its hats land on sixteenths the song strikes, and
they miss none of them; about two thirds of its claps are there. Nine of ten bass notes are its own (the pitch on the
same sixteenth), and about two thirds of the piano's, the pad's and the Rhodes' notes. The sections are the analysis's, merged where the energy is close, as for
every remake. The genre and the seed choose the sounds (a 909 kick, a Wurlitzer for the keys): change them with
`set_part` or a script like any song's.

Without the ears' models there are no stems or notes to hear: a rebuild then plays the drum habits of its busiest
sections wherever its kick is heard, and the genre's bass and chord parts on its chords, as a song on its skeleton
does, and its history says so.

## On its skeleton only

```sh
claudius remake out/house-glass-avenue-1/song.mp3 --genre deep_house --seed 3 --skeleton
```

It prints the analysis again, then the new song:

```text
Faded Engine — deep_house at 122 BPM in A minor, 64 bars (2:06)
  progression A: III VImaj7 iv7 v7
  progression B: i VII v7 i7
  form: drop1 24 | break2 8 | drop3 32
  tracks: kick (sampled bd (tr909:bd)), clap (sampled cp (tr909:cp)), hat (sampled hh (tr909:hh)), shaker (shaker cut from 'Loose Leaf Beat Shaker' (Apple Loops)), perc (sampled rim (tr909:rim)), bass (synth patch 'sub_bass'), chords (synth patch 'keys'), pad (synth patch 'warm_pad'), lead (sampled rhodes (jrhodes:rhodes)), top (loop 'Deep Blue Void Hi-Hat' (House, Apple Loops) (126 BPM)), crash (synth crash), riser (synth riser), downlifter (synth downlifter), reverse_crash (synth reverse_crash)
  master: -11.0 LUFS, true peak -1.0 dBTP (pre-master -8.8 LUFS); done in 13.3 s
  ears: 'house music' z +3.3 (closest confusion 'techno music' +3.1); production z +1.3 vs 'a low quality recording with hiss' +1.2; p_real 0.80
        sounds like: house music, drum and bass music, techno music, tech house music
  verdict: 89/100 (arc 1.00, genre 0.79, mix 0.98, production 0.53, theory 1.00). Fix first: sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause; dull 4-8 kHz: hats, claps and snare lack bite (presence -26.9 dB, healthy from -26)
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

Every remake is an ordinary song. A script can fix what the analysis got wrong and replace any part with your own.
This one corrects the first chord of the skeleton remake and puts a written motif in place of the generated Rhodes
lead:

```js packs
prog("A", "i7 VImaj7 iv7 v7");
track("lead", n("<[4 ~ 4 2 4 ~ 6 4] [5 ~ 5 4 2 ~ 0 ~] [3 ~ 3 2 0 ~ 2 3] [4 ~ ~ 6 4 ~ ~ ~]>")
                .octave(4).s("rhodes").room(0.3).delay(0.2));
```

```sh
claudius script fix.rhai --on out/remake-deep_house-faded-engine-3
```

```text
  verdict: 87/100 (arc 1.00, genre 0.68, mix 0.98, production 0.46, theory 1.00). Fix first: sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause; dull 4-8 kHz: hats, claps and snare lack bite (presence -26.8 dB, healthy from -26)
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

## Other ways to use its audio

Besides a rework, a flip and a rebuild, two commands put the recording's sound in a song:

- `claudius chop song.mp3` cuts the recording into bar loops (2 bars by default, `--bars`) and drum one-shots, writes
  them into `out/chops/`, and adds them to your user library, where generated songs can pick them up as drum
  sounds. `--find "a vocal chop"` also uses the ears to cut out the moments that best match a description, three per
  description (`--top`).
- `claudius make --genre house --sample out/chops/<loop>.wav` flips a recording into a new song: warped to the song's
  tempo, transposed to its key, re-sliced in half-beat pieces (or played straight) and placed in the high-energy
  sections, as a track named `sample1`.

Only use recordings you have the right to use this way. `analyze` and `remake --skeleton` take nothing but
measurements from a recording; a rework puts all of its audio in your song, and a flip, `chop` and `make --sample`
pieces of it. A rebuild plays its composition note for note (its melodies, its chords, its bass line), and its voice
when it sings: releasing it is a cover at least, which needs the song's licence, and with the voice the recording's
too.

From an assistant, the MCP tools `analyze` and `remake` take a file path and do the same as the commands (`remake`
with `flip: true` for a flip, `rebuild: true` for a rebuild, `sampled: false` for the skeleton only); a remake becomes
the song open in the studio.
