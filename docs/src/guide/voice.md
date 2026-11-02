# Your voice

claudius listens to singing the way a vocal coach does, and tunes it the way a careful producer would. It hears which note you mean at every moment and how near you
come to it, and it learns how you sing: where you tend to go sharp or flat, how widely your notes scatter, how fast you
settle on a note, and your vibrato. It hears this in any recording of one voice, a [take](samples.md#takes) in your
song or a file, and the same reading turns your melodies into the notes you meant when you
[turn a take into notes](samples.md#turning-a-take-into-notes).

## How a take is sung

In the studio, right-click a take's row in the arrangement and choose **How is it sung?**. Over MCP, an assistant
calls `assess_voice` with the take's name, or with an audio file's `path`. On the command line, `claudius voice` hears
a file:

```text
$ claudius voice "Lights and Sound Vox Melody.caf"
12 notes, 75% within 25 cents of the note meant and 100% within 50; range C4-A#4; flat by 13 cents in the middle (F4),
sharp by 11 cents at the top (A#4); held notes scatter 20 cents either way, so a note lands within 25 cents 75% of
the time; settles on a note in 30 ms; furthest off: G4 at 6.8 s (-46 cents), A4 at 3.3 s (+38 cents), A4 at 7.3 s
(+35 cents)
```

A cent is a hundredth of a semitone. A held note about 25 cents off starts to sound sour against the music, and 50
cents is halfway to the next note.

- **The notes.** How many notes it heard, and how many of them were sung within 25 and within 50 cents of the note
  meant. The furthest off are named, with where they are in the recording and how far off they were.
- **The range.** The lowest and highest notes meant.
- **Sharp or flat.** Where you sing above or below the note, at the bottom of your range, in the middle and at the
  top. Many singers go flat as they climb, or sharp when they push. A place you sing in tune is left out.
- **The scatter.** How widely your held notes land around where you usually put them, and from that, how often a
  note lands within 25 cents.
- **Settling.** How long a note takes to reach where you hold it: the scoop or slide into it.
- **Vibrato.** How fast and how wide your vibrato is on notes long enough to hear one.

A take is heard in the song's key and over its chords, from the section it starts with. For a file, `--key "A minor"`
(`key` over MCP) gives the key it is sung in. `--notes` lists every note with where it was sung, and `--json` gives
all of it as data.

## How claudius reads it

- **Pitch.** claudius reads the voice's pitch every 10 ms with YIN, then finds each period again at the full sample
  rate, to about a cent.
- **Where the notes are.** A model of how notes go, each scooped into, held and let go, finds where each note starts
  and ends, so that a scoop, a slide or a vibrato is part of its note and a breath or a note sung again begins
  another.
- **Which note each one is.** A singer puts a note off by so much for the whole of it, so each note is read once,
  from where it was held, as this singer sings. The song settles what the voice leaves in doubt: the tones of its key
  and of the chord under the note are likelier than others, and a step likelier than a leap. The more widely you
  scatter, the more the song decides. A note 40 cents under F♯ in C major is read as an F sung sharp from a loose
  singer, and as the F♯ it nearly is from a precise one.
- **Learning the singer.** From your singing alone, claudius reads the notes as anyone would sing them, learns how
  you sing from how near they came, and reads them again as you sing them, twice over.

It wants one voice at a time: a dry take or an a cappella, not a full mix. A voice more than half a semitone off on
most notes, with nothing to sing against, is heard as singing the notes next to them; sung over the song, or with a
key, it is heard against those.

## Checking your pitch

A pitch check tells claudius how you sing without any take at all. In the studio's Record window, press **Check my
pitch**, or let an assistant ask for one. Six short phrases of four notes play, one at a time, and you sing each back in
the gap after it, in whatever octave suits you. It takes about a minute. Wear headphones, so that the microphone
hears you and not the phrases.

```text
24 of 24 notes sung back (1 octave below the notes played), 63% within 25 cents of the note and 96% within 50; by step
71% within 25 cents, by leap 40%; range A2-A3; flat by 18 cents at the top (A3); held notes scatter 14 cents either
way, so a note lands within 25 cents 79% of the time; settles on a note in 90 ms; a vibrato of 5.6 Hz, 22 cents either
way, on long notes; furthest off: A3 (-41 cents), G3 (-36 cents), F#3 (+33 cents)
```

Besides what [a take](#how-a-take-is-sung) tells, a check knows the notes you were asked for, so it says how many you
sang back, in which octave, and how much nearer you land a note reached by step than one reached by a leap. The
phrases walk a major scale between A3 and A4, mostly by step. An assistant can give other notes with `low` and `high`.

The voice it hears becomes yours. The Record window shows it, and from then on [tuning](#tuning-a-voice) sings your
takes as the check heard you, unless a tuner's `voice` says otherwise. claudius keeps it in `voice.json` in its
settings folder ([Files and settings](files.md)), on this computer only. A new check replaces it, and deleting the file
forgets it.

An assistant asks with `request_pitch_check`. The studio's Record window opens for it (or the link the assistant gives
opens one), you sing it, and the assistant, which has been waiting with `wait_for_pitch_check`, hears what claudius
heard. **Not now** tells it you will do it another time.

## Tuning a voice

The tuner moves each note of a voice onto the note it means, as far as its singer needs. It learns how the take is
sung, as above, so it knows that you go flat at the top and that your notes usually land within 15 cents. Your usual
error goes. Of the scatter, only what is within `humanize` cents of the note stays, as expression. A precise singer
is left nearly alone and a loose one is brought in. Each note is read against the song's key and the chord under it,
so a note sung between two goes to the one the music has.

In the studio, right-click a take's row and choose **Tune it**:

| setting | what it does |
|---|---|
| **Naturally** | each note brought on over 40 ms, 10 cents of your own kept, the vibrato as sung: the scoop into each note stays |
| **Tightly** | on the note within 15 ms, 3 cents of your own kept |
| **Hard** | snapped onto each note at once and held dead still: the robotic sound |
| **Not at all** | the tuner taken out |

The voice keeps its own character: claudius moves it period by period (TD-PSOLA), so it sounds like you singing a
little higher or lower, not like a tape sped up. The tuner is an effect, first in the take's chain, so the
[graph](../getting-started/studio.md#the-panels) shows it and its settings, and undo takes it out. An assistant tunes a
track with `tune`, and a script with `fx()`:

```js no-run
fx("take1", [#{ fx: "tune", speed_ms: 20, humanize: 5 }, #{ fx: "compressor", threshold_db: -20, ratio: 3, attack_ms: 5, release_ms: 80 }]);
fx("take1", [#{ fx: "tune", strength: sections(0.5, #{ hook: 1 }) }]);
```

| setting | default | what it does |
|---|---|---|
| `strength` | 1 | how much of the way to the notes, from 0 to 1; it can move, by section or along a curve, like a mix |
| `speed_ms` | 40 | how fast a note is brought onto its pitch after it starts; 0 at once |
| `humanize` | 10 | the cents of your own wandering around a note that stay |
| `vibrato` | 1 | how much of the vibrato stays, from 0 to 1 |
| `scale` | `"chords"` | what the notes are read against: `"chords"` (the song's key and the chord under each note), `"key"`, `"chromatic"` (any note), or only some notes, `#{ notes: ["C", "D", "E", "G", "A"] }` |
| `voice` | learnt | the singer, as `assess_voice --json` gives one; learnt from the take itself when not given |

On the command line, `claudius tune` tunes a file outside any song (with `--key` or `--notes`) and writes the tuned
recording beside it:

```text
$ claudius tune "Lights and Sound Vox Melody.caf" --key "F major"
12 notes tuned, moved 10 cents on average and 27 at most; wrote Lights and Sound Vox Melody-tuned.wav
```

`--speed 0 --humanize 0 --vibrato 0` is the hard-tuned sound, and `--strength` how much of the way to go.
