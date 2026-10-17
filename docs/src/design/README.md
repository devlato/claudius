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
