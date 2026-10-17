# 0001 Real audio only

Status: accepted

## Context

claudius is meant to make songs the way a producer does in GarageBand or FL Studio, and to make them sound like a
producer's work. There were four ways to get sound:

- **Generated audio**: a neural model that outputs waveforms from a description. It can sound impressive, but the
  result is a finished recording: there is no kick to move, no bass line to rewrite, no mix to rebalance. Its
  provenance depends on training data nobody can audit, and it needs a large model and, in practice, a GPU.
- **General MIDI**: notes played through a General MIDI or SoundFont set. Every note stays editable, but GM sound sets
  sound like GM sets, and the free SoundFonts that sound better often have unclear licences.
- **Synthesis only**: drums and synths rendered in code. Fully editable, small, deterministic, licence-free. But
  earshot's ears, and people, hear a fully synthesized song as only marginally "professionally mixed"; a real piano,
  strings or a recorded 909 make a large difference.
- **Recordings**: multi-sampled instruments, drum machines, loops. They sound real, cost disk space, and each set comes
  with a licence.

## Decision

Every sound in claudius is real audio: either synthesized in code (13 drum voices, a subtractive and FM synth engine
with 20 presets, risers and impacts) or recorded (sampled instruments and drum machines from sample packs, Apple
Loops where GarageBand or Logic installed them, chops of songs you give it, any file a script names). Notes exist only
as the score that plays those sounds, like a piano roll.

The model enforces it: `model::Sound` has a variant for each kind of audio source and none for anything else.
Synthesis is always available as the fallback, so a song can be made with nothing installed, and recordings replace it
where they exist (a genre's `"piano|epiano"` preset plays the sampled piano when its pack is installed).

Generated audio was rejected because it cannot be edited at the level a producer works at, its output cannot be
attributed, and its runtime cost dwarfs the rest of claudius. General MIDI was rejected for its sound, and because the
packs claudius uses are mapped per instrument with velocity layers and round robins, which GM sets rarely have.

## Consequences

- Everything in a song stays editable and explainable: every note has a pitch and a time, every sound a name, every
  level a reason. Mutation, scripts and the critic all depend on that.
- Rendering is deterministic: the same blueprint and the same installed sounds give the same audio.
- The good-sounding version needs a download: about 2.2 GB of sample packs (1.2 GB on disk), and the sound of a song
  depends on what is installed, so the same seed sounds different on a machine without packs.
- Sounds arrive at wildly different levels and timbres, so claudius needs an automatic mixer that balances every track
  to its role's loudness, a mastering stage, and a critic to check the result (see [Rendering](../engineering/rendering.md)).
- Sampled instruments have the limits of their recordings: the default piano keeps 5 of its 16 velocity layers for
  size, the only clearly licensed choir is built from one sung vowel, and there are no vocals with words.
- Licensing becomes claudius' problem: which packs it may download by default, what credits a song needs. That is
  [0007](0007-sample-licensing.md).
- claudius cannot make sounds that exist only as recordings it does not have, and will never "imagine" a sound. That is
  the point, and also a real limit next to generated audio.
