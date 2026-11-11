# 0016 Settings any later claudius reads

Status: accepted

## Context

claudius is released every day that it changes, and people update when they update: one day later, or a year and
three hundred versions later. Either way, what they set should still be set. Before 0.6 the settings were kept as each
version's own structs. The studio kept its settings as RON under eframe's key, and the server kept the voice a pitch
check heard as JSON, each read as a whole. A field a later version renamed, or one of a type it changed, made the
whole file unreadable, and every setting in it went back to its default.

## Decision

Every settings file is versioned and read field by field (`claudius_api::settings::Fields`, which the browser's
studio has too).

- **A version in every file**: a JSON object with `"version"`, the version of its kind that wrote it. A file without
  one is version 0, as every claudius before 0.6 wrote them.
- **Each field read on its own**: a field that is not there, or does not read as its type, is its default alone, and
  the rest are read as they were. A file that is not JSON at all, cut short or another file, is read as no settings.
- **Migrations in order**: a version that renames or reshapes a field changes the earlier versions' files to its own
  as it reads them (`Fields::rename`), from the version before it on, and a test reads a file of every earlier version.
- **What a later version wrote is kept**: a field this claudius does not know is written back as it was, so that
  going back to an older claudius and forward again loses nothing.

The files are:

- the studio's settings, in eframe's storage under `settings`: a file in the app, the page's local storage in a
  browser. The RON a studio before 0.6 kept under eframe's key is read as version 0, once, and kept as JSON from then
  on;
- `voice.json`, the voice a pitch check heard;
- `telemetry.json`, the answer about usage statistics and the day each kind of event last went out;
- `updates.json`, whether to update by itself and the day it last looked.

## Consequences

- A setting survives any number of versions. A change to a setting's shape comes with its migration and a test of the
  earlier file, in the same pull request.
- A setting whose meaning changes in a way no migration can carry is a new field with a new name, and the old one is
  read for as long as a file may hold it.
- Songs and projects need the same, with a corpus of the files every release wrote: that is a change of its own.
