# 0012 A history that only grows

Status: accepted

## Context

Undo and redo were two stacks of whole blueprints, at most 200, which a change pushed onto and undo popped. They knew
nothing of what a change did beyond its label, and they forgot. Undo past the 200th was gone, a change undone and
replaced by another was gone, and a song closed and opened again had no undo at all.

What people want of a song's history goes further:

- finding what was done from words of their own ("when did we add the whistles?"), with on-device search (see
  [0011](0011-meaning-on-device.md));
- going back to before a change: undoing it and everything since, in one step, with redo bringing them back;
- in time, more than one person editing one song at once, whose changes merge, which is what a CRDT does.

The simplest history keeps snapshots of the song after each change. A snapshot says nothing about what changed, so
it cannot be searched by what was done, and two people's snapshots cannot be merged. A CRDT needs a log of
operations: each with an id no other replica gives, the operations it was made on, and what it changed, at a grain
fine enough that two people's changes to different things do not collide.

## Decision

`claudius_session::history` keeps every change made to the song as an operation in a log that only grows, beside the
undo and redo stacks, in both the server's session and the browser's demo:

- **An operation** has an id (the session's own random id and its Lamport time), the operations it was made on, when
  it was made, its kind (an edit and the command that made it, an undo or redo of an edit, or a going back to before
  one), what undo calls it, what it did in words, and what it changed in the song.
- **What changed** is the paths of the blueprint's JSON whose values differ, each with its value before and after.
  A list of named things (tracks, drum lanes, sections) is keyed by name, with its order as a value of its own, so a
  change to one track names that track, whatever happens to the others. Another list changes by place, appended to or
  cut short, or whole.
- **Another song in the song's place** (a new song, one opened, a recording remade) is one change of the whole song,
  and so is undoing it. The song's own history begins at the edit that put the song as it is in its place, the latest
  one undo can still take back, so the undos and goings back since replay with the steps they took.
- **Undo, redo and going back are operations too**, whose changes take the song back or forward. Nothing in the log
  is ever taken out or rewritten. Files the song plays that moved (a save copies its takes into its folder) are an
  operation of their own, which changes neither undo nor redo.
- **The song follows from the log**: the changes of every operation, applied in order to no song at all, make it.
  Saving writes the song's own log into its folder (`history.jsonl`), and opening the folder replays it, which brings
  the undo and redo steps back as they were. A song edited by hand since then has the difference recorded as an edit
  of its own.
- **The undo stack keeps whole blueprints**, as before. The log could rebuild them, but the stacks undo in a
  moment, and they cannot fail to restore exactly what was there.
- **The state lists the last 200 changes**, each with its words and whether going back can reach it. `go_back` takes
  the song to before a change, and the MCP tools `history` and `go_back` let an assistant do the same.

## Consequences

- What was done can be found again, by the words each change is described in ("added the whistle track (lead),
  playing the synth whistle; tempo 124 to 128"), and the palette can search them by meaning, as it searches its
  actions.
- A CRDT has what it needs from the log: ids every replica can order, the operations each change was made on, and
  changes at the grain of a setting. Merging two replicas' logs is still to come. Most paths merge as registers, last
  writer wins. The order of a list and a list changed by place need a sequence of their own, and claudius's lists of
  named things already have the names it would hold.
- The log costs memory and disk: a new song's operation holds the whole song, and opening another holds both. A
  song's own log, as a folder keeps it, starts at the operation that began the song.
- Going back reaches only the changes undo can still take back: one undone and replaced, or more than 200 ago, is in
  the log and can be found, but the song goes back to it only by hand. Rebuilding any earlier song from the log is
  possible, and is for later.
- The demo's project download, which the app opens, does not carry the history yet: its blueprint plays the samples
  from the zip, where the page's history plays them from the page.
