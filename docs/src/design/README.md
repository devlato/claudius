# About these records

These are claudius' architecture decision records: one short page per decision that shapes the code and would be
expensive to reverse. Each records what the situation was, what was decided, and what that costs, so that someone
who wants to change it later starts from the reasons instead of guessing them.

They are not design documents for features. A record is worth writing when a reasonable newcomer would ask "why on
earth is it done this way?", and the answer is a trade-off rather than a mistake.

| record | in one line |
|---|---|
| [0001 Real audio only](0001-real-audio-only.md) | every sound is synthesized in code or recorded; no generated audio, no General MIDI |
| [0002 Rust end to end](0002-rust.md) | engine, server, UI, ears and decoder in one language, no Python at runtime |
| [0003 The blueprint is the song](0003-blueprint.md) | a song is stored as its decisions; the arranged notes are derived |
| [0004 Rhai with Strudel-style patterns](0004-script-language.md) | an embedded, sandboxed language with live-coding patterns inside strings |
| [0005 One session, many clients](0005-one-session.md) | the studio, browsers and assistants all edit one song |
| [0006 A critic with ears](0006-critic.md) | meters, theory and a calibrated CLAP model judge every render; some evidence is held out |
| [0007 Which samples we ship](0007-sample-licensing.md) | none: curated packs are downloaded from their authors, and only clear licences are on by default |
| [0008 egui for native and web](0008-egui.md) | one immediate-mode UI compiled natively and to WebAssembly, talking only HTTP |
| [0009 Releases through pull requests](0009-releases.md) | a release is a reviewed pull request; merging it builds and publishes |
| [0010 Samples from online libraries](0010-online-samples.md) | samples come from libraries made to be used (Openverse, Commons, ccMixter), never from YouTube or the streaming services, and only under licences that allow publishing |
| [0011 Finding by meaning, on the device](0011-meaning-on-device.md) | static word embeddings and a flat index in plain Rust, in the engine, find actions by what the words mean on any computer, the browser too |
| [0012 A history that only grows](0012-history.md) | every change is an operation in a log, with an id, what it was made on, and what it changed path by path; undo and going back are operations too, ready for a CRDT |
| [0013 Requests in plain words, read on the device](0013-requests.md) | a request is read by its words, the examples nearest it in meaning and rules on the song into what claudius would do, a button for each reading, scored on rounds of new requests before tuning to them |
| [0014 A small model taught to read requests](0014-request-model.md) | Cactus's Needle 3, taught in Rust on claudius's tools with low-rank adapters, reads on the device what the readings read nothing of; its calls are checked, said again in the readings' words and grounded in the song |
| [0015 Requests said aloud, heard on the device](0015-spoken-requests.md) | Cactus's Whistle, ported to Rust on needle-core, hears what is said to the palette; silence is heard as nothing, words heard near ones claudius knows are put right, and the words go into the field as if typed |
| [0016 Settings any later claudius reads](0016-settings-any-later-claudius-reads.md) | every settings file has a version and is read field by field, a field that does not read costing only itself; earlier versions migrate as they are read, and what a later version wrote is kept |
| [0017 A rebuild that sounds like the recording](0017-rebuild-from-its-parts.md) | proposed: a rebuild is measured on songs claudius renders itself and made from better parts, HTDemucs stems, YourMT3+'s notes, each instrument on a sound like its own, and chords from the notes |
| [0020 Changing a song's genre](0020-change-a-songs-genre.md) | accepted: a song in another genre keeps its form, chords, key, melody, recordings and written notes, and the genre's drums, bass, parts, sounds and mix play it, as one change undo takes back |
| [0021 Every kind of sample, ranked and varied](0021-every-kind-of-sample.md) | proposed: the sampler offers sung phrases, hook and passage loops, drum breaks, bass lines and stabs, each scored as its kind, and a varied set of them, each piece saying what it is |

## The format

Each record has a status and three parts:

- **Context**: the forces at play when the decision was made: what claudius needed, what constrained it, what the
  options were.
- **Decision**: what was decided, stated plainly, and the options that were rejected, with why.
- **Consequences**: what follows, good and bad. The bad ones matter most; a record that lists only benefits is a
  sales pitch.

A status is `accepted`, `superseded by NNNN`, or `deprecated`. Records are not edited to match later thinking: when a
decision changes, a new record supersedes the old one, and the old one gets a line pointing to it. Fixing a typo or
a broken link is fine.

## Adding one

1. Copy the shape of an existing record into `docs/src/design/NNNN-short-title.md`, with the next free number.
2. Write it while the decision is being made, in the pull request that implements it or before. Half a page to a page
   is enough; link to the code and the engineering pages for detail.
3. Add it to `docs/src/SUMMARY.md` under "Design decisions" and to the table above.
4. If it replaces an earlier record, set that one's status to `superseded by NNNN`.

Use the pull request title `docs(design): NNNN short title`, so the record shows up in the changelog.
