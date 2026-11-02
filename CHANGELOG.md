# Changelog

All notable changes to claudius are listed here. The format follows [Keep a Changelog](https://keepachangelog.com),
and claudius uses [semantic versioning](https://semver.org): until 1.0, a minor version may change the script
language or the API, and the changelog says how.

## [Unreleased]

Claudius updates itself from this release on: the studio downloads a new release in the background and offers to
restart into it, and `claudius update` does the same in a terminal
([Updates](https://github.com/devlato/claudius/blob/main/docs/src/guide/updates.md)). The releases before this one
can't, so install this one the way you installed the last.

Sample packs installed by 0.2 can hold damaged FLAC files: two notes of VCSL's harmonica take hundreds of MB and
play as noise. `claudius-packs install --defaults` converts such files again and downloads nothing else.

Eight more default packs, and 21 more instruments in VCSL and FreePats: Sonic Pi's samples, a solo cello, an erhu, a
steel pan, synth drums, body percussion, glitch kits and sound effects, among them a hang, wine glasses and a TX81Z.
`claudius setup` downloads them, about 0.9 GB more ([Sample packs](https://github.com/devlato/claudius/blob/main/docs/src/guide/packs.md)).

The studio's demo at [claudius.devlato.com/studio](https://claudius.devlato.com/studio/?mock) plays claudius' own
sounds instead of a rough synth: every synth preset as claudius renders it, the TR-707 and TR-808 drum machines, in
stereo, with reverb, delay and the kick's ducking, each track as loud as claudius' mix makes its role, and the
master at the genre's loudness.

The studio records, in the app and in the browser: a take from a microphone or an audio interface, which plays
once from the section it starts with and goes with the song, and what a MIDI keyboard plays, as a part's notes. A sung, hummed or played line becomes notes for an instrument or several, as it is or as harmony,
octaves, chords, a bass line, an arpeggio or a pad. **Measure** plays clicks and listens for them, so that takes land
in time ([Takes](https://github.com/devlato/claudius/blob/main/docs/src/guide/samples.md#takes)).

A take waits when it stops: play it back in the song, record it again, or keep it. An assistant can ask you for one
(`request_take`): the studio's Record window opens for it, and once you send the take, the assistant, which has been
waiting for it (`wait_for_take`), carries on with it
([A take an assistant asks for](https://github.com/devlato/claudius/blob/main/docs/src/guide/samples.md#a-take-an-assistant-asks-for)).

Scripts reach every effect: a chain on any track, bus or the master, `parallel` chains side by side at their own
amounts, buses of your own, and settings that move through the song or by section, such as a filter that opens across
every build or a reverb that only the break drowns in. The printed song lists every chain
([Effect chains](https://github.com/devlato/claudius/blob/main/docs/src/language/mixing.md#effect-chains)).

Ten new genres: `trip_hop`, `big_beat`, `electro_house`, `nu_disco`, `filter_house`, `dance_pop`,
`neo_psychedelia`, `art_rock`, `idm` and `dub_techno`. Their profiles use the new effect chains themselves: parallel
compression and crushed copies on the drums, a dub delay that feeds back harder in the breaks, an acid filter that
opens across the builds, a disco loop that sweeps through a lowpass with the arrangement, guitars that turn distorted
in the climaxes, pads gated harder as a groove goes on. The cookbook has a recipe for each
([Trip-hop, big beat, electro house, nu-disco, filter house](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/breaks-and-disco.md);
[Dance-pop, neo-psychedelia, art rock, IDM, dub techno](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/pop-psych-and-glitch.md)).

Genres fuse: `trip_hop+idm`, `art_rock+idm` or `house+dnb+lofi` is one genre, in scripts, on the command line, for
assistants and in the studio's New song dialog (**Fused with**). The first genre leads the song, its harmony, parts
and mix, and the last plays the drums and lends its sounds
([Fusions](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/fusions.md)).

The auto-sampler samples a recording's vocals into a song: it finds the best sung phrases on the recording's own beat
grid, cleans them of its drums, fits them to the song's tempo and key, and places each where its notes agree with
the song's chords, mixed as a vocal (`claudius sample`, `sample_song`, and the studio's New song dialog;
[Sampling a song's vocals](https://github.com/devlato/claudius/blob/main/docs/src/guide/samples.md#sampling-a-songs-vocals)).

The studio has a Graph tab: the song as its signal flows, each track through its effects into its bus, the buses
and returns into the master, out, on a canvas to drag and zoom. Every node is editable beside it: a track's mix,
bus, sections and sound, an effect's settings or its place, a chain's new effects, the master's loudness. Assistants
read the same graph with the `graph` tool and change chains with `set_fx`.

Every drum step can do what a drum machine's trig does: play by chance, play only on some times round (every other
bar, the fourth, or the last bar before the next section), roll in a ratchet that swells or fades, flam, land early or
late of the grid, and change its pitch, decay, tone and pan for that stroke. Right-click a step in the step sequencer
and open **Trig**, or write `trig()` in a script; the step shows what it does
([The drum machine](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/drum-machine.md)).

claudius hears how a voice is sung: the notes it means, how near each came, and how its singer sings (where they go
sharp or flat by register, how widely their notes scatter, how fast they settle, their vibrato). Right-click a take
and choose **How is it sung?**, run `claudius voice` on a file, or let an assistant call `assess_voice`
([Your voice](https://github.com/devlato/claudius/blob/main/docs/src/guide/voice.md)). Turning a take into notes reads
the same way, so a singer's flat high notes become the notes they meant.

A tuner moves a voice onto the notes it means, as far as its singer needs: their usual error goes, and of their scatter
only a few cents stay, as expression. It reads each note against the song's chords, brings it on as fast as asked
(slowly enough to keep the scoop, or at once for the hard-tuned sound) and keeps the voice's character. Right-click a
take and choose **Tune it**, add the `tune` effect in a script or the graph, run `claudius tune`, or let an assistant
call `tune`.

A pitch check plays six short phrases to sing back, in any octave, and learns from them how you sing, how much nearer
you land a step than a leap included; your takes are tuned as it heard you from then on. Press **Check my pitch** in the
Record window, or let an assistant ask with `request_pitch_check` and wait with `wait_for_pitch_check`
([Checking your pitch](https://github.com/devlato/claudius/blob/main/docs/src/guide/voice.md#checking-your-pitch)).

`claudius publish` checks a song the way stores and distributors check a release (its artwork's size, colours and
sharpness, its audio, the licences of its sounds, and its details, including how it was made) and writes a release
kit: the WAV and the cover a distributor takes, and the answers its form asks for
([Publishing](https://github.com/devlato/claudius/blob/main/docs/src/guide/publish.md)).

## [0.2.0] - 2026-10-19

### Added

- studio: Download the sample packs from the studio ([#17](https://github.com/devlato/claudius/pull/17))
- macos: claudius.app in a disk image, one app for Apple silicon and Intel Macs ([#13](https://github.com/devlato/claudius/pull/13))
- windows: An installer, with the studio in the Start menu ([#14](https://github.com/devlato/claudius/pull/14))
- linux: .deb, .rpm and AppImage packages, with the studio in the app menu ([#15](https://github.com/devlato/claudius/pull/15))

### Changed

- critic: Read realness over eight windows, and encode the MP3 while the critic listens ([#18](https://github.com/devlato/claudius/pull/18))

### Fixed

- packs: Setup installs a pack whose WAV files have a long fmt chunk, and gets the ears when a pack fails ([#19](https://github.com/devlato/claudius/pull/19))
- mcp: Wait for the ears to load before exiting, instead of crashing ([#20](https://github.com/devlato/claudius/pull/20))
- remake: Play a remake at a tempo its genre plays ([#21](https://github.com/devlato/claudius/pull/21))
- What a newcomer misread: download sizes, save messages, ears loading, MCP schemas ([#22](https://github.com/devlato/claudius/pull/22))
- critic: Advice a person can act on, instead of z-scores and near ties ([#23](https://github.com/devlato/claudius/pull/23))

## [0.1.0] - 2026-10-18

The first public release.

### Added

- A beatmaker that composes, arranges, mixes and masters songs in eight genres (house, deep house, techno, trap,
  lofi, drum and bass, synthwave, future bass), using real audio only: synthesized drums and synths, 162 sampled
  instruments from 14 freely licensed sample packs, Apple Loops when GarageBand is installed, and your own files.
- A critic that judges every render: loudness and tonal balance, the energy arc of the arrangement, music theory
  checks, and earshot's CLAP model for genre fit and production quality. Every issue names what to change.
- Mutation and evolution: songs change the way a producer would change them, and the critic keeps the better ones.
- A script language: Rhai with Strudel-style mini-notation patterns. Every song prints as a script that runs back
  into the same song.
- The studio, a native and web app (egui) with arrangement, step sequencer, mixer, harmony, script and verdict
  panels.
- MCP tools, over stdio and HTTP, so Claude, ChatGPT, Cursor, Codex and other assistants can make and edit songs
  while you listen, and skills that teach them the craft: making and editing songs, remixing recordings, and setting
  claudius up. `claudius connect` installs both.
- Analysis and remakes of any song: tempo, key, chords, sections and groove, rebuilt in any genre.
- `claudius setup`, `doctor` and `connect` for installing the sounds and models, checking an installation, and
  configuring assistants.
- Builds for macOS on Apple silicon and Intel (Intel Macs have no ears: their verdicts come from the meters and the
  theory checks), Linux on x86_64 and ARM64 with a headless build for machines without sound or a display, and
  Windows as a preview. One-line installers for each, which come with every release and are in its checksums.

[unreleased]: https://github.com/devlato/claudius/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/devlato/claudius/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/devlato/claudius/releases/tag/v0.1.0
