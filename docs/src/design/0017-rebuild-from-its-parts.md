# 0017 A rebuild that sounds like the recording

Status: proposed

## Context

A rebuild (`remake` with `rebuild: true`, `rebuild.rs`) makes a recording again from its parts, so that every note of
it can be changed. People who tried it heard almost nothing of the recording in what came out. Three steps lose it:

- **Separation.** The stems come from Spleeter (`stems.rs`, `earshot::separate`), which scores 5.9 dB SDR on
  MUSDB18-HQ: much of each stem is the others, and the transcription after it writes that bleed down as notes.
- **Transcription.** Basic Pitch (`transcribe.rs`) writes the bass and the rest as notes. It was made for one
  instrument at a time: on the 2025 AMT Challenge's multi-instrument test it scores an F1 of 0.06, where the best
  systems reach 0.6.
- **Instruments.** The rest (keys, guitars, synths, together) becomes one part, played on the genre's chord sound, so
  a guitar riff comes back as a pad.

## Decision

Rebuild from better parts, and measure every step.

1. **Measure first.** An evaluation of the whole rebuild on songs claudius renders itself, whose notes are known
   exactly: each part's note F1 (onset, pitch and offset, as `mir_eval` counts them), and how near the rebuilt song's
   chords, tempo and drums come to the original's. Each later step has to raise these numbers, and CI keeps them from
   falling.
2. **Separation** by Mel-Band RoFormer (Kim's vocal model, MIT) for the voice and kuielab's MDX-Net (MIT, about 7 dB)
   for the drums, the bass and the rest, in pure Rust as the ears are, in place of Spleeter.
3. **Transcription** by YourMT3+ (F1 0.59 on the same test), ported to Rust from its paper: its weights are
   Apache-2.0, and its code GPL-3.0, so the port follows the paper and not the code. It writes each instrument's notes
   with its General MIDI program.
4. **Instruments of their own.** Each instrument transcribed becomes a part, played on the claudius sound nearest its
   program (a guitar on a guitar), rather than the rest on the genre's chords. The genre still gives the mix and the
   master, and any sound can still be changed.
5. **Chords from the notes** of every instrument, rather than from the mix, whose drums and voice blur them.

Not taken: HTDemucs-ft separates better (vocals 9.2, drums 10.1, bass 10.4 dB), but its weights are for research
only, and MuScriptor's weights are CC BY-NC.

## Consequences

- More models to download on first use, each pinned by its size and SHA-256 as the ears' are; without the ears a
  rebuild does what it does now.
- A rebuild takes longer, as transformer separation and transcription of a whole song on a laptop's CPU do.
- Each step lands as a pull request of its own, with the evaluation's numbers before and after it.
