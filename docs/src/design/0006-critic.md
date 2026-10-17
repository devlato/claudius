# 0006 A critic with ears

Status: accepted

## Context

claudius revises songs: evolution tries dozens of variations, and assistants edit a song, listen and edit again. Both
need feedback after every render, faster than a person can listen and in a form they can act on.

Measurements alone (loudness, spectrum, width) say whether a master is technically sound, not whether a house track
sounds like house. A learned judge can say the latter, but every model can be pleased without the music getting
better: optimise against it and you get songs that score well with it. earshot, the library claudius listens with,
warns about exactly this. No model exists that rates music quality with clear licences and data behind it.

## Decision

The critic (`judge.rs`, `ears.rs`) combines evidence that cannot all be gamed at once:

- **mix**: model-free meters on the master (loudness and true peak against the targets, energy per band against a
  healthy range, stereo width, a mono low end);
- **arc**: whether section loudness follows the energy the arrangement intends, and whether the drop is clearly louder
  than the break;
- **theory**: checks on the notes themselves (lead in key and on chord tones, singable range, bass on the roots,
  smooth voice leading, a sound form);
- **ears**, when earshot's models are installed: one CLAP embedding of the master, scored with earshot's calibrated
  z-scores against the genre and the genres it is confused with, and against "professionally mixed" versus six named
  flaws; with reference tracks, typicality among them.

Every problem is an issue with a severity, a sentence, and a target naming where the fix lives (`band:sub+`,
`part:lead`, `form`), which evolution uses to choose what to mutate and assistants read as advice. Two readings are
**held out**: the realness estimate and the AudioSet tags are reported and never scored, so that a rising score
without rising held-out readings shows the scored parts being gamed. Without the models, the critic works with the
first three components.

Rejected: listening by people alone, which cannot drive an evolutionary search. Meters alone, which cannot hear genre.
Scoring the AudioSet tags or realness too, which would leave nothing held out. Training a model of our own, for lack
of data we could license.

## Consequences

- Verdicts explain themselves: "the drop is only 2.1 LU louder than the breakdown (aim for 3-7)" is something a person
  or an assistant can act on, and the targets let evolution act on it automatically.
- The critic works offline and without the 785 MB of models, with less to say.
- The ears are only as good as CLAP on music: earshot validated it for matching sound effects, house, tech house and
  techno sit close together for it, and fully synthesized songs read only marginally professional. Genre and
  production scores are hints.
- The healthy band ranges are one target for every genre, so dark genres such as lofi collect issues that are style,
  not fault.
- A high score means no measurable problems and a confident genre reading. It does not mean a good song, and the docs
  say so wherever scores appear.
- Evolution judges previews (the build-up and first drop), which keeps it affordable but misses problems elsewhere in
  the song.
- The models cost a download, about half a gigabyte of memory, and a few seconds to load; verdicts made before they load
  have no ears.
