# Your voice

claudius listens to singing the way a vocal coach does. It hears which note you mean at every moment and how near you
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
