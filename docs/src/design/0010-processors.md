# 0010 Processors: effects with several tracks in

Status: proposed

## Context

Every effect in claudius works on the audio of one track, a bus, a return or the master. The only signal that crosses
from one track to another is the sidechain duck, and that is built from the kick's notes, not its sound. Producers
reach for more: a pad gated by the hi-hats, chords pumped by the real kick, a bass whose filter opens with the drums'
envelope, strings that crossfade into a pad over a build, a synth vocoded by a voice, several tracks mixed with gains
that move through the song.

What is asked for is effects as functions: each takes up to some number of tracks, has parameters a person turns as
knobs and that can follow time, and gives one track back, which then mixes like any other. They have to be available
wherever the rest of claudius is: scripts, the studio and MCP.

The constraints:

- Rendering is offline, over whole-song buffers: a four-minute stereo track is about 96 MB at 48 kHz.
- Scripts are Rhai, an interpreter: far too slow to call per sample, and its closures cannot be stored. A song has to
  print back to a script that rebuilds it exactly ([printing](../language/printing.md)), and saved songs have to keep
  opening in later releases ([0003](0003-blueprint.md)).
- The engine already stores parameters that change over time (`Param`: a constant or (beat, value) points), but only
  the arranger writes them. The compressor already takes a sidechain key, which nothing passes.

## Decision

**A processor is a named node of the blueprint** (`Blueprint.processors`). It has:

- a `name`, which is the name of the track it outputs;
- its `inputs`: tracks of the song, or other processors;
- a graph of operations from a built-in set, over those inputs;
- `knobs`: named parameters with a range and a default, which the graph reads;
- the output's `mix`: level, pan, sends, bus and inserts, as any track has.

**Scripts define one with a function whose arguments are its inputs.** The number of arguments is the number of inputs,
up to eight:

```js
process("pump", ["chords", "kick"], |chords, kick| chords.duck(kick, knob("depth", 0.7, 0, 1)));
process("swell", ["pad", "strings"], |pad, strings| pad.crossfade(strings, at(#{ intro: 0, drop: 1 })));
process("talk", ["chords", "vox"], |chords, vox| chords.vocode(vox, #{ bands: 24 }).gain(-3));
```

The function runs once, when the script runs, on stand-ins for the tracks. What it does to them is recorded as a graph,
the way a pattern function is printed by running it on a placeholder. The graph is saved; the function is not, so the
song prints back as `process(...)` lines that rebuild it.

**Values** wherever an operation takes a number:

- a number;
- a knob, `knob(name, default, min, max)`, which the studio shows and MCP can set;
- a curve over the song's beats, `curve([[0, 0.2], [64, 1]])`;
- a value for each section, `at(#{ intro: 0.2, drop: 1 })`;
- a pattern in mini-notation, one cycle a bar, `"0 0.5 1 0.5"`;
- a function of the beat, `|beat| 0.5 + 0.5 * sin(beat / 8.0)`, sampled into a curve when the script runs;
- another signal's envelope, `kick.envelope(5, 120)`.

**The operations** start with:

- `gain`, `pan`, `lpf`, `hpf`, `bpf`, `saturate`, `delay`, and `reverb`, with a recorded space or the generated one;
- `compress(key, ...)`, `duck(key, depth)`, `gate(key, depth)`;
- `mix(other, gain)`, `crossfade(other, position)`, `ringmod(other)`, `vocode(modulator, ...)`;
- `envelope(attack, release)`;
- `convolve(other, seconds)`, capped in length.

`catalog` lists them, each with its arguments and their ranges.

**Rendering** splits the per-track pass in three:

1. Every track's source and insert effects: its signal before the fader.
2. The processors, in the order of their inputs. A cycle is refused when the script runs. Each processor reads the
   pre-fader signals of its inputs.
3. Level, duck, pan, sends and bus, for each track that still plays and for each processor's output.

A track a processor processes no longer plays on its own, unless it is marked `keep`. Keys do: the inputs of `duck`,
`gate`, `compress` and `envelope` that only steer.

**The studio** gives each processor a strip in the mixer, with its knobs. A turned knob sends `set_knob`, done as a
script call like the mixer's other edits. **MCP** builds processors through `run_script`; `catalog` lists the operations
and the recorded spaces; `get_song` lists each processor with its inputs and knobs; and `command` takes `set_knob`.

**Saved songs.** `processors` has a default, so songs without processors stay in blueprint format 1. A song with
processors is written in format 2. An older claudius would otherwise drop the field silently and play the raw inputs;
with format 2, it refuses the song with its "update claudius to open it" message.

Rejected:

- **Functions called per sample**: about 10^7 interpreter calls per track and second of audio.
- **Only a fixed list of processor kinds** (duck, crossfade, vocoder): it cannot express the mixing the request is
  about.
- **A graph of tracks edited in the studio**: the script is where songs are built, and a graph editor would be a second
  language.
- **Processing in real time while the song plays**: claudius renders offline. The studio plays the render, as it does
  for everything else.

## Consequences

- **Memory.** A processor's output is one more track buffer, and its inputs stay alive until the processors have run.
  `MAX_TRACKS` counts processors.
- **Convolution.** Convolving one track with another needs partitioned convolution, or a short cap on the kernel:
  today's `convolve` sizes one FFT to the whole kernel.
- **Slices and evolution.** `Project::slice`, which previews and evolution use, shifts processor curves as it does
  filter curves. Mutation and evolution leave processors alone at first.
- **The documentation grows.** The language pages get a page on processors; the mixing pages, the script reference and
  the songs skill describe them; `catalog` documents each operation.
