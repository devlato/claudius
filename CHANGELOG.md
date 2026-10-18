# Changelog

All notable changes to claudius are listed here. The format follows [Keep a Changelog](https://keepachangelog.com),
and claudius uses [semantic versioning](https://semver.org): until 1.0, a minor version may change the script
language or the API, and the changelog says how.

## [Unreleased]

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
- MCP tools, over stdio and HTTP, so Claude, ChatGPT, Cursor and other assistants can make and edit songs while you
  listen.
- Analysis and remakes of any song: tempo, key, chords, sections and groove, rebuilt in any genre.
- `claudius setup`, `doctor` and `connect` for installing the sounds and models, checking an installation, and
  configuring assistants.
- Builds for macOS on Apple silicon and Intel (Intel Macs have no ears: their verdicts come from the meters and the
  theory checks), Linux on x86_64 and ARM64 with a headless build for machines without sound or a display, and
  Windows as a preview. One-line installers for each, which come with every release and are in its checksums, and a
  Homebrew formula.
