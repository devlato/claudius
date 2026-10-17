# 0003 The blueprint is the song

Status: accepted

## Context

A claudius song is edited from many directions: a script changes the tempo, the studio paints drum steps, mutation
swaps a bass style, an assistant rewrites the form, analysis of an audio file produces a new song. It has to be saved,
undone, printed as a script and evolved, and two versions of it have to be comparable.

A DAW stores the arranged session: tracks with their notes and clips. That is what it plays, and it is the obvious
thing to store. But a session records results, not decisions. A bass line stored as 144 notes no longer knows that it
was an off-beat house bass over a progression; changing the key means moving every note; trying another style means
writing it again by hand; and a mutation operating on notes can only nudge notes.

## Decision

A song is its `Blueprint` (`crates/claudius/src/model.rs`): the decisions a producer makes, stored as data. Tempo, key
and swing; progressions as Roman numerals; the form; drum lanes as step patterns; each melodic part as a recipe (a
role, a style, a register, a density and a seed of its own); pattern tracks as the code that makes them; every sound;
the mix. The arranged `Project` with its notes and clips is derived from it by `compose::arrange`, deterministically,
every time it is needed, and thrown away after rendering.

The blueprint is what is saved (`blueprint.json`), what undo restores, what scripts print and edit, and what mutation
and crossover change. Notes are stored in a blueprint only where no recipe produced them: hand-written or analysed
notes in a part's `notes`, and the drum steps.

Rejected: storing the project as the truth, for the reasons above, and storing both, which would leave two copies of
the song that drift apart the first time one is edited.

## Consequences

- Edits happen at the level a producer thinks at. A new seed is a new bass line in the same style; a new key moves every
  part, because progressions are numerals and parts are written from them; a style change rewrites a part in a
  sentence. Mutation operators are a few lines each ([Evolution](../engineering/evolution-and-analysis.md)).
- A blueprint is small: about 30 KB of JSON for a full house song, against about 300 KB for its project. Undo keeps 200
  whole blueprints, and evolution hashes them to skip duplicates.
- A song can print as a script that runs back into itself ([Patterns and the script engine](../engineering/patterns.md#printing-a-song)),
  because every field is a decision a script function can state.
- The notes of a generated part depend on the writer's code. When theory code changes, an existing blueprint renders
  differently: a saved song is not frozen across versions of claudius. The generator snapshot tests make such changes
  visible in review, but they cannot keep old songs as they were.
- Single notes of a generated part cannot be edited in place. To change one, the part has to become hand-written notes
  or a pattern track, and then it no longer follows the key and style.
- A track plays the same way in every section it is in; there is no per-bar arrangement edit. A variation takes a
  second track.
- Everything that derives the project must be deterministic, which shapes the random number generator, the seeds stored
  in parts, and the hash-based randomness of patterns ([Architecture](../engineering/architecture.md#determinism)).
