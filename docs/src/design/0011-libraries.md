# 0011 Libraries of sounds and effects, made, brought and shared

Status: proposed

## Context

Today claudius plays its own synth presets and effects, the curated packs in `registry.json` ([0007](0007-sample-licensing.md)),
Strudel sample maps added with `claudius-packs add-map`, the person's sample folders, and Apple Loops. People want more
than that:

- to bring their own sounds: recordings they made, or processed sounds they are proud of;
- to make sounds and effects in claudius itself, and keep them;
- to find what others made in a marketplace, free for now, and to publish their own there;
- to keep some libraries for themselves only.

A sound is either recorded audio, or a procedure that makes it. An effect is a procedure over up to N input tracks
with one output ([0010](0010-processors.md)), which should be able to stream in real time.

The constraints:

- **Speed.** Scripts are Rhai, an interpreter, far too slow per sample, as 0010 explains.
- **Strudel.** Strudel patterns run in claudius; using sounds has to stay Strudel's way.
- **Downloads.** A downloaded library must not be able to do anything but make sound.
- **Saved songs.** They have to keep opening.
- **Publishing.** Licences matter more once anyone can publish.

## Decision

**One format, the library.** A library is a folder, or a `.zip` of one, with a `library.json` manifest: name, version,
author, licence and description, its files, and its items. An item is one of:

- **a recorded sound**: audio files and their mapping (root notes, velocity layers, round robins, loop points), as the
  packs' instruments have today;
- **a procedural sound**: a recorded graph of synthesis blocks, played per note with the note's pitch, velocity and
  length, and with knobs;
- **an effect**: a recorded graph over 1 to 8 inputs, with knobs, as in 0010.

The curated packs are read as libraries too, so there is one loader and one resolver.

**Definitions are data.** Sounds and effects are defined in the script language, by a function that runs once on
stand-ins and is recorded as a graph, the way 0010 defines processors:

```js
sound("mine:glass", |note| osc("sine", note).fm(osc("sine", note * 3.5), knob("bite", 2, 0, 8))
    .env(2, 900).lpf(knob("tone", 6000, 200, 16000)));
effect("mine:pump", |music, kick| music.duck(kick, knob("depth", 0.7, 0, 1)));
```

A library therefore holds audio and graphs, not code. Installing one cannot run anything; it can only make sound.
Graphs have size limits, as scripts do. The graph's blocks are native Rust, so a procedural sound costs what a built-in
preset costs.

**Graphs stream.** The graph runtime processes blocks of samples (128 by default), keeping each block's state, rather
than whole-song buffers:

- An offline render runs a graph block by block over the song.
- The studio can run the same graph live: audition a sound, or turn an effect's knob and hear it at once, without a new
  render.
- Memory stays bounded by the block, not the song.

Today's whole-buffer effects keep working. They move to blocks one at a time, each move held to the tests below.

**Using sounds stays Strudel's.** A library sound plays with `s("mine:glass")`, `.bank("mine")` and `n()`.
`samples("github:user/repo")` and `samples(url)` load a Strudel sample map as a library, which is what `add-map` does
today, so Strudel code that brings its own samples runs in claudius. The definitions (`sound`, `effect`, `process`) are
claudius' own; the docs mark them as extensions to Strudel.

**Where libraries live:**

- **Personal libraries** are folders in `~/Music/claudius/libraries/`. They are made with `claudius library new`, with
  the studio's *Save as sound* or *Save as effect*, or by a script's `library("mine")`. They stay on the computer.
- **Installed libraries** come from the marketplace, a URL or GitHub, into claudius' data folder. They are pinned to a
  version and its SHA-256, like the packs.
- **Songs** name sounds and effects as `library:item@version`, and keep a copy of the graphs they use. A song still
  plays when a library changes or goes away; its audio files are fetched again when needed.

**The marketplace** is a small service, free for now:

- a Cloudflare Worker for the API, R2 for the files and D1 for the index;
- sign-in with GitHub, for publishing only: searching and installing public libraries need no account;
- each published version is immutable and named by its content hash.

Each library has a visibility:

- *public*: listed and searchable;
- *unlisted*: anyone with the link;
- *private*: the owner only. This is also how a person carries personal libraries to another computer.

The command line has `claudius library search`, `install`, `publish` and `remove`. The studio has a library browser.
MCP has the same, through `run_script` and `catalog`, plus commands for searching and installing.

**Licences and safety:**

- Publishing asks for a licence (CC0, CC BY, CC BY-SA, personal use only, or the author's own text) and for the
  author's word that they have the right to publish.
- The index shows the licence, and claudius' `credits.txt` covers library sounds as it covers packs.
- Uploads are checked: audio formats claudius decodes, sizes and lengths within limits, and safe names (no path
  traversal, no zip bombs).
- A reported library can be hidden while it is looked at.

**Tests come first.** Before the engine changes, regression nets go in at every level of the pyramid. Each change in
the work that follows then shows what it did to the sound:

1. **The sound of today's songs.** Short synth-only renders of each genre, with their metrics: loudness, true peak,
   energy per band, and a hash of the samples on each platform. A change meant to be bit-exact must keep the hash. A
   change meant to alter the sound updates the metrics on purpose, in review. Null tests compare two engines sample by
   sample.
2. **Every DSP block and effect.**
   - Impulse and frequency responses, and envelope timing, measured against expected values.
   - Stability: no NaN, infinity or denormals.
   - The same output in blocks of any size as over the whole buffer.
3. **Properties.** Random graphs give finite, bounded and deterministic output whatever the block size.
4. **The language.**
   - Definitions print and rebuild the same graph.
   - A corpus of Strudel snippets has to parse and play.
   - Errors say what to do.
5. **The format.**
   - Manifests across versions, with fixtures, as blueprints have.
   - Name resolution and its precedence, and credits.
   - Safe unpacking, and the manifest parser under fuzzing.
6. **Integration.**
   - The server and MCP against a fake marketplace.
   - The command line end to end.
   - The studio's library browser through `CLAUDIUS_UI_PLAN`.
7. **The marketplace service.**
   - The Worker's tests: sign-in, visibility, immutability, quotas.
   - A contract test of the schema claudius and the Worker share.
8. **Performance budgets.** Benchmarks with limits: a graph's throughput as a multiple of real time, render time per
   genre, and peak memory, checked in CI with a margin.

**In this order:**

1. The tests above.
2. The format, personal libraries, and bringing sounds (folders, `samples()`).
3. The graph runtime, with effects and procedural sounds, rendering offline.
4. The studio's *Save as* and the library browser.
5. The marketplace.
6. Live playback of effects and procedural sounds in the studio.

Rejected:

- **Native plugins (VST3, CLAP, AU) inside libraries.** Untrusted native code, and a build for each platform.
- **Script functions called per sample.** Too slow, as in 0010.
- **A marketplace made only of a GitHub index.** Publishing waits for a pull request, and there is no private
  visibility.
- **Libraries in the claudius repository.** The redistribution and size problems of 0007.

Kept for later: WebAssembly modules for DSP the graph's blocks cannot express. Sandboxed, portable and fast, they would
be the one kind of code a library could carry.

## Consequences

- **A service to run.** The marketplace has an operator: storage costs past R2's free tier, moderation, takedowns, and
  backups of D1.
- **Licences are the authors'.** The marketplace states each one, claudius credits it, and nobody reviews the audio
  before it is published.
- **Songs carry graphs.** A song that uses a library carries the graphs it needs, so the blueprint gets a format 2 when
  it does ([0010](0010-processors.md)).
- **Effects are rewritten for streaming.** Every effect moves to blocks; the tests in step 1 keep each move honest.
- **Live playback** asks more of the studio's audio path than playing a file. That is why it is the last step.
