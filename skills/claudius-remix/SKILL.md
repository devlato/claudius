---
name: claudius-remix
description: >-
  Works from existing audio with claudius: analyzes a song's tempo, key, chords, sections and groove, reworks a
  track in another genre so that it stays recognizable (or flips its hook into a new song, rebuilds it as a project
  of editable parts, or remakes its skeleton),
  chops recordings into bar loops and drum one-shots, plays
  recordings as samples in a song, and adds recordings made for a song (a vocal, a guitar) as takes. Use it when
  someone shares an audio file or a reference track, asks for a song like one they love or a version of a song in
  another style, wants a song's BPM, key or chords, wants to sample, chop or flip a recording with claudius, or has
  recorded something for their song.
---

# Remixing, remaking and sampling with claudius

claudius can work from a recording in four ways, which differ in how much of the original ends up in the result:

| | what it takes from the recording | the original's audio in the result |
|---|---|---|
| **analyze** | tempo, key, chords, sections, drum habits, bass style, as a report | none |
| **remake** | all of it, its voice and chords as they are, reworked: its own sections and chords with a genre's drums, bass and mix under it | yes, all through |
| **remake**, `flip: true` | its hook and passage after passage of it with its voice, its drums and a flip of it, in a new song of a genre | yes, in passages |
| **remake**, `rebuild: true` | the song itself: its drums, its bass and the rest as the notes and hits it plays, on claudius's sounds, every note editable | its voice only, when it sings |
| **remake**, `sampled: false` | that skeleton, rebuilt as a new song with a genre's sounds and production | none |
| **chop and flip** | loops and one-shots cut from it, played in a song | yes |
| **take** | all of it, played once from a section, as recorded for the song | yes |

Choose by what the person wants. "A remix of this song", "this song as house" or "a boom bap version of it" is a
remake: the song stays recognizable. "Flip this track", "make a new beat out of its hook" is a remake with
`flip: true`. "Turn this song into a project", "transcribe it", "reverse engineer it", "give me its parts as MIDI" or
"I want to change its bass line" is a remake with `rebuild: true`. "Something like this song" without its sound is a
remake with `sampled: false`. "Use this vocal" or
"sample this break" is a flip of a chop. "I sang the chorus" or "here's my guitar for the drop" is a take.

## Before you start

- **The file has to be on the computer that runs claudius.** Ask for its path. A claudius shared over a network
  only reads its own folders (`~/Music/claudius`, its caches, the sound libraries, and folders given with `--files`),
  so a file elsewhere needs moving there first. The person can also choose a file on their own computer in the
  studio (New song, From a recording), which reads it and sends claudius what it holds.
- **Rights.** A remake (unless `sampled: false`), a chop and a flip put the original's audio in the song, a rework
  all of it: publishing it needs the right to use that recording. A rebuild plays the original's composition note
  for note, and its voice when it sings: releasing it is a cover at least, which needs the song's licence, and with
  the voice the recording's too. A remake with `sampled: false` uses none of its audio. Say so when the person reworks or flips something they did not make. Apple Loops may be used in music the
  person makes, under Apple's licence.

## Analyze

`analyze` with the path (MCP), or `claudius analyze song.mp3 [--genre house] [--json]`, reports the tempo with its
pulse clarity, the key with a confidence, the swing, progressions `A` and `B` as Roman numerals, the sections with
their energy and what plays in them, the kick, snare and hat patterns as step strings, and the bass style.

Analysis is best on steady electronic music with a clear kick, and approximate everywhere else. Pulse clarity (0 to
1) says how clearly a steady beat stands out; below about 0.3 the tempo is closer to a guess. Its usual mistakes:

- half or double the tempo: `--genre` settles which of the two it is, by the genre's tempo range; it does not pull a
  tempo that is simply outside the range into it;
- the relative or a neighbouring key: Bb major instead of D minor, say;
- major where the music is minor on the same tonic, or the other way round. The numerals then look borrowed (F major
  with `i7` and `bIII`): the chords are right and the key name is not. A lead the critic finds outside the key while
  its strong-beat notes are chord tones points the same way;
- a progression rotated by a bar, or with a seventh missing;
- sections merged when their energy is close, even an intro that is clearly quieter. None is longer than 32 bars (a
  longer one is cut where it changes most, on an 8-bar phrase), and a recording whose sections are all within 1.5 dB
  of each other has no drop: its sections play at a groove's level.

Report it with that in mind ("about 118 BPM, probably D minor"). When the person knows the real tempo or key, theirs
wins: `key("F minor")` on the remake keeps every chord as it sounds (a borrowed `bIII` becomes `III`) and rewrites the
generated parts in the new key.

After `analyze`, `song_like` with a `genre` makes a new song on what it heard (its tempo, key, progressions, form and
swing, none of its sound) without listening to the file again, as the studio's **Make a song like it** does. To give
the open song only its tempo, or its key and chords, run a script: `tempo(118);`, or `key("D minor");` with
`prog("A", ...)`, `prog("B", ...)` and `bars_per_chord(...)` from the progressions it reported.

## Remake

`remake` with `path`, `genre` and `seed` (MCP), or `claudius remake song.mp3 --genre lofi [--seed 3]`, reworks the
recording in the genre, so that it stays the song it is:

- all of it plays through, from its first downbeat to its end, at its own speed and pitch (its sections brought
  halfway to one level, so a quiet verse is not lost under the genre's drums): its voice, its chords and
  its melodies as they are, without its drums and its bass (taken apart from the rest with the ears' models; without
  them, what strikes and what is under 150 Hz), through the genre's treatment of a phrase it samples (boom bap's tape
  and 12 bits, say); a band that speeds up and slows down is followed beat for beat;
- the song is its form bar for bar: its own sections, their kinds, lengths and energies;
- on its chords bar by bar, a progression for each run of them, and in its swing;
- the genre plays under it: its drums, its bass on those chords' roots (in place of the recording's), its break
  and textures, its mix; not its chords, keys, pads or leads, which would play over the recording's own playing and
  singing, and against it where a chord was heard wrong.

Its tempo is the recording's, or double it (two bars of the song to each of the recording's) when that is nearer the genre's range, so that nothing is stretched,
or half it (one bar of the song to two of the recording's) when that is nearer and the recording holds each chord two
bars (a 170 BPM song reworked as hip hop plays at 85, not at drum and bass's speed); `bpm` sets another (and stretches
it). The reply and the song's history say so (`reworked from '<file>': all 96 bars of it, ...`, and `in half time, at
85 BPM to its 170`). The recording is the `original` track: turn it down, filter it or mute it in sections like any loop, and
edit the genre's parts around it. With the ears, when it sings, its voice is apart on a track of its own, `vocals`
(the history says "its voice on a track of its own (vocals)"), and `original` is the rest of it: level, treat or mute
the voice on its own, as a singer's track. A recording too short or without sections (under 8 bars) is remade on its skeleton,
and the reply says so.

With `flip: true` (`--flip`), `remake` makes a new song of the genre on the recording's sound instead, as a song built
on samples is built:

- its hook (eight bars of its loudest section with chords) without its drums is what the song takes its key, its
  chords and its tempo from;
- the song goes on through it: each section but the intro, the breaks and the outro plays as many of its bars on end
  as it has itself (32 at most), of the kind it plays (a hook's or a drop's, or the rest's), from the hook, from
  another passage of it (of its verses when it has any) or from bars no section before it played, each on its own
  track and its own bars' chords (`sample3`, `sample4` ... on `A3`, `A4` ...), so a three-minute flip is not two
  loops;
- when it sings, its voice sings on in those passages as it sang there (with the ears, they are all of it but its
  drums); without a voice, with the ears, they are its bass and the rest;
- its own drums (its drum stem, with the ears) are the break, in a genre that plays one;
- a flip of it, half beats cut from all over it on the hook's chords, each brought to the hook's level, plays in the
  intro, the breaks and the outro.

A flip plays at the recording's own tempo when the genre does, else at a multiple of it, faster or slower by whole
semitones or chopped onto the song's grid; a hook a voice sings in plays in its own key, never sped up into a
chipmunk (except in chipmunk soul, whose sound that is). Its history says what came from where (`remixed from
'<file>': its hook (bars 57-64) without its drums, in its own key for the voice in it; another passage of it
(bars 25-32); ...; on through it as the song goes (build bars 1-8, drop bars 33-64, drop2 bars 9-24)`), so tell the
person which part of their song
plays where. Everything else is the genre's. A sung phrase moved into the song's key keeps its singer's formants.

With `rebuild: true` (`--rebuild`), `remake` makes the song itself again, every part of it played on claudius's
sounds, so that each note can be changed:

- its form bar for bar, each section on a progression of its own bars' chords (`A`, `B` ... one to a section), so
  that each plays its own notes;
- its drums from its drum stem: kicks, snares or claps and hats struck where it strikes them, each section's folded
  into a pattern of its own (the section's drum variant), a step struck only some of the times round played with that
  chance, each lane as late or early as the recording's;
- its bass on `bass`, one note at a time, and the rest of it but its voice (keys, guitars, synths, pads together) on
  `keys`, chords and all: the notes Spotify's Basic Pitch hears in their stems, on the sixteenths;
- its voice, when it sings, on a track of its own (`vocals`), and the melody it sings written out on `melody`, which
  plays in no section until it is put in one (`toggle_layer`).

The genre and the seed choose the sounds, the mix and the master. Its history says what it heard (`rebuilt from
'<file>': its 4 sections bar for bar, on its chords; its drums struck as it strikes them ...; its bass as 214 notes on
bass; the rest of it as 866 notes on keys`): tell the person in those words, and that it is their song to edit now.
The notes are good, not perfect: about nine in ten of a bass line's and two in three of the rest's are right on a
clean electronic mix, fewer on a dense band recording, and the sections are the analysis's. Without the ears' models
(no stems, no notes), it plays the drum habits of the busiest sections and the genre's bass and chords on its chords,
and its history says "its notes not heard": `claudius setup` downloads the models.

With `sampled: false` (`--skeleton`), `remake` composes a new song at the analysed tempo and key instead, with its
progressions, swing, form, section energies and drum habits, in the genre's sounds and production, and none of its
audio. A tempo outside the genre's range becomes half or double when that fits it (a 124 BPM house song as lofi plays
in half time, at the slowest lofi tempo, 70) and the reply says so. Its history starts with `remade from <file>: ...`.

The skeleton remake copies what the analysis read, mistakes included, so check it before polishing anything: print it (`echo
| claudius script - --on SONG --print`, or `get_song`) and compare it with the genre. A short clip, or sections the
analysis merged, gives a form of one or two sections: give it the genre's shape with `form()` (`song("<genre>")`
shows a typical one). A noisy drum read gives drum lanes no producer in that genre would play: rewrite them with
`drum()`, such as a four-on-the-floor kick for house. Fix the key, then the form, then the drums, and only then the
mix.

From then on either remake is an ordinary song: listen, read the verdict, edit it with scripts, save it (the
claudius-songs skill covers that loop). A different seed gives another take on the same recording, and another genre
gives the same song in a new style.

## Chop

On the command line only:

```sh
claudius chop ~/Music/reference.mp3 --bars 2
claudius chop ~/Music/reference.mp3 --find "a vocal chop" --find "a snare hit" --top 3
```

It writes into `out/chops/<file name>/` (or `--out`): one loop per section, `--bars` bars long from its first
downbeat (`02-drop-2bars.wav`), and a `kick.wav`, `clap.wav` and `hat.wav`, each cut from the sparsest section that
still has that drum. Every file is added to the user library, so later songs can pick the one-shots for their drum
lanes. `--find` uses the listening models to cut out the moments that best match a description.

## Flip a recording into a song

- **A new song around it:** `claudius make --genre lofi --sample out/chops/reference/03-drop-2bars.wav`. The
  recording is warped to the song's tempo, transposed to its key, often re-chopped into an eight-slice motif, and
  played in the drops, hooks and grooves as a track called `sample1`.
- **In a script, over MCP too:** play the file as a sampler on a note pattern. `c4` plays it at its own pitch:

```js no-run
track("vox", note("c4 ~ ~ ~ c4 ~ c4 ~").s("/Users/me/Music/claudius/chops/reference/find-a-vocal-chop-1.wav")
             .room(0.3).level(-4));
```

  The path goes in `.s()`, never inside the pattern string. The song stores the path, not the audio, so the file has
  to stay where it is.
- **Apple Loops one-shots** on the computers that have GarageBand or Logic content: `.bank("apple")` on drum words,
  as in `s("bd*4, [~ cp]*2").bank("apple")`.

After a flip, listen and read the verdict like after any change: a sample often needs its level, a little reverb, or
`.hpf()` to leave room for the bass.

## Add a take

When the person recorded something for the song, such as a vocal, a guitar or a melody sung into the microphone, add it
with `add_take`: the file's `path`, and `from`, the section it starts with (`get_song` lists the sections). A take
plays once, from that section's first downbeat and on through the sections after it for as long as it lasts. It is
not looped or chopped. Ask what they did not say:

- **Which section it was recorded over.** The recording has to start on that section's first downbeat, at the song's
  tempo.
- **What it is, for the mix.** `role` is `vocal`, `lead` (the default), `chords`, `keys`, `bass`, `drums` or
  `texture`.
- **A name**, if they want one ("vox", "guitar"). Otherwise it is take1, take2, ...

A recording from a free library goes in the same way: `search_recordings` lists what the libraries offer (CC0,
public domain or CC BY) as `library:<id>`, which `add_take`, `sample_song`, `analyze` and `remake` take as the
`path`; claudius downloads it and the song credits it. Let the person hear it on its page before using it.

A whole song or a long recording goes in the same way; `trim_take` then keeps part of it (`track`, `start` and `end`
in seconds into its recording), so that it starts later in itself or stops sooner, and undo brings it back whole.
Effects go on it as on any track.

claudius keeps a copy with the song, in `takes/` in its folder once it is saved. Then listen and read the verdict: a
take mostly needs its level and a little reverb (`set_mix`). It is the person's own recording, so it may be released
when they made it.

## Sample a song's vocals

When the person wants a vocal from a recording in their song ("sample the vocals from this", "put that hook in my
drop"), call `sample_song` with the recording's `path`. claudius takes the recording's voice apart from the rest
(with the ears), finds its best sung phrases, fits them to the song's tempo and key (stretched, never sped up, its
formants kept), places each where its notes agree with the song's chords (the drops and hooks first, then the breaks
...) and mixes them as vocals, `vox1`, `vox2` ... The reply's first line says what came of it ("sampled 2 phrases of
song.mp3 into the song: vox1, vox2"), the lines after it where each phrase came from and went.

- **What to ask for.** `count` phrases (2), `bars` long (1, 2 or 4), `sections` to put them in by name or kind
  ("the break only"), and `find` when the voice is something else ("a rap verse", "a choir").
- **How often it comes round** (`every`): `once` ("once in each chorus"), `2`, `4`, `8` or `16` ("every four bars"),
  or `through` ("loop it all through the break"). Said nothing, the song decides by its genre and what its sections
  play (once where a lead or a voice sings already, every other time round in a genre built on samples, every 8 bars
  in the others), and the reply says which and why: when the person finds it repeats too much, or not enough, sample
  again with another `every`.
- **The way to sample it** (`how`), from what the person describes:
  - `phrases` (the default): short sung phrases that come round as loops.
  - `passage`: a whole verse or chorus of the recording (4, 8 or 16 `bars`) played through in the drops and hooks,
    the song's chords there following it. Use it for "build the song on that chorus" or "use their verse as my hook",
    as trance and hip-hop producers do.
  - `chops`: its voice cut in eighths and played in a new rhythm, an instrument of the voice ("chopped vocals",
    "make the vocal into a lead").
  - `stabs`: one short syllable struck on a few beats as a drum lane (`vox_stab`), the sample dropped in now and
    then as a rhythm. Change its steps with `set_drum_pattern` (lane `vox_stab`, variant `main`).

  The reply says where each piece came from and went; read it to the person in those words.
- **Afterwards.** Read the reply aloud in musical terms: where each phrase came from in the recording and where it
  went. Listen, and set its level, reverb and delay with `set_mix` like any track; undo takes the phrases out.
- **Whose voice it is.** A sample of someone else's recording needs their permission before the song is released:
  say so when the person samples a song they did not make.
- **To choose the pieces first.** `sample_pieces` offers them before any goes in, numbered as they come in the
  recording, as the studio's Sample window numbers them: each a phrase sung whole, never
  cut inside a word, where it is in the recording ("in hook 2, bars 12-13, 0:31"), as it was cut and through a
  treatment (dusty tape, a washed echo, lo-fi, a phaser, a telephone band, a stutter, a reversed tail), with where to
  hear each. Describe them, let the person pick, then `keep_pieces` with `[{id, treated}]` (and `sections`, `every`,
  as above); `redraw_pieces` draws another take when none suits, of other pieces than the last where there are any.
- **Every kind of piece.** Besides its phrases, `sample_pieces` offers the best piece of each other kind the recording
  holds: `hook` (a loop of its hook, the instruments without the drums, bass and voice), `passage` (a loop of another
  passage), `break` (its drums) and `bass` (its bass line), each saying why it scored as it did ("the drums alone,
  busy, it loops cleanly"), and up to two more of the kinds producers sample most and the song's genre samples where
  they are good and unlike those offered (its sparse groove and its busy one, never two cuts of one). Ask for some
  with `kinds` ("break" offers up to four drum loops to build on). Kept, a break loops where the song's drums play, a
  bass line plays in place of the song's bass, a hook loop in the drops and hooks; two of a kind kept without
  `sections` share the sections, the fuller in the louder. A recording nobody sings in, or none of whose phrases fits
  the `bars` asked, still offers these, and the reply's last line says why there is no phrase ("no phrase: ...").
- **Chopped onto pads.** `keep_pieces` with `chopped: true` cuts a break, a loop or a phrase at its onsets, the hits
  that sound alike sharing a pad, eight at most: drum lanes `chop`, `chop2` ... that play it as it was, each hit at its
  own speed, so a break at another tempo keeps its hits whole. Offer it when the person wants to flip a break, play it
  from pads or a phone, or rearrange its hits; then write their steps with `run_script`
  (`drum("chop2", "x..x..x.x...x...|....x.......x...")`).

## Ask the person to record a take

When the song needs something only the person can give it, such as their voice on the hook, a guitar or a melody they
play on a keyboard, ask for it with `request_take`:

- **`from`**, the section it starts with, and `role`: what it is in the mix (`vocal`, `lead`, `chords`, `keys`,
  `bass`, `drums`, `texture`), or the part notes from a keyboard make (`keys`, `lead`, `chords`, `pad`, `arp`,
  `bass`).
- **`note`**, a few words the Record window shows them ("the chorus melody, sung softly"), and a `name` if the take
  should have one.

The studio's Record window opens for it. Give the person the link `request_take` returns, in case no studio is open,
and say what to do: record it, play it back, record it again until they like it, then press Send it. Then call
`wait_for_take` with the request's id, and call it again for as long as it says they are still at it. They may take
minutes. When the take arrives, `wait_for_take` says what claudius made of it: listen, and set its level and reverb
as for any take (`set_mix`), or turn it into notes. When they press Not now, carry on without it and do not ask again
unless they want to.

## How a take is sung

When the person asks how well they sang, or whether a take is in tune, call `assess_voice` with the take's name (or a
file's `path`, with `key` when you know it). It says how many notes landed within 25 and 50 cents of the note meant,
names the furthest off with where they are, and describes the singer: range, where they go sharp or flat (bottom,
middle, top), how widely their held notes scatter, how fast they settle, their vibrato. Tell them plainly and kindly,
starting with what went well; 25 cents off is where a held note starts to sound sour, so a note named there is worth
singing again. A take far off on most notes may be in another key than the song: check the song's key first.

## Check the person's pitch

A pitch check tells claudius how the person sings before or without a take: call `request_pitch_check` (with a `note`
such as "so that I know how to tune your takes"), give them the link it returns in case no studio is open, and say
what to do: six short phrases play, and they sing each back in the gap after it, in any octave, with headphones on.
Then call `wait_for_pitch_check` with the id until it is in. Tell them what it found, kindly: how many notes landed
near, whether leaps are harder than steps, where they go sharp or flat. From then on `tune` sings their takes as the
check heard them, and says so. When they press Not now, carry on and do not ask again unless they want to.

## Tune a take

When the person wants a take in tune ("fix my pitch", "autotune it", "the T-Pain sound"), call `tune` with the take's
name. It learns how they sing from the take itself and reads each note against the song's chords:

- **Naturally** (the default: `speed_ms` 40, `humanize` 10): the scoop into each note and the vibrato stay. Start here.
- **Tightly**: `speed_ms` 15, `humanize` 3.
- **Hard**, the robotic sound: `speed_ms` 0, `humanize` 0, `vibrato` 0.
- `strength` below 1 goes part of the way. It is a setting that can move, so a script can tune the hook harder than
  the verse with `sections()`; `tune` itself sets one value.
- `off: true` takes the tuner out.

The tuner is an effect first in the take's chain, so `get_song` shows it and undo removes it. Say what you did in
the person's words ("I tuned the take gently; it keeps your slides into notes") and suggest listening.

## Turn a take into notes

When the person sang, hummed or whistled a melody and wants an instrument to play it, call `transcribe_take` with the
take's name and `sounds`, the synth presets or installed instruments to play it (`catalog` lists them).

- **Several sounds** make one part each on the same line, such as `["piano", "strings"]` for a doubled melody.
- **The notes.** claudius puts them in the song's key and on its sixteenth-note grid. `chromatic: true` keeps notes
  outside the key, for a melody that leaves it on purpose.
- **The take goes quiet** unless `keep_take`, since the parts now play it.
- **A style** makes the line something else, in the song's key:
  - `harmony` adds a third above it, and `octaves` doubles it an octave down;
  - `chords` puts a triad under each note;
  - `bass` moves it down into a bass's register;
  - `arp` breaks each note into a rising arpeggio, and `pad` holds a chord through each bar the line sounds in.

  When the person says "make my melody the bass line" or "put chords under it", that's the style. The part gets the
  role that goes with it.

It hears one note at a time: chords, a strummed guitar or a mix give no melody. Afterwards the parts are ordinary
parts: change their sound with `set_part`, their level with `set_mix`. Say what came out ("12 notes from G3 to D5,
played by the piano in the chorus"), and offer to undo it if the person did not like it.
