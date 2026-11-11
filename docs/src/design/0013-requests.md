# 0013 Requests in plain words, read on the device

Status: accepted

## Context

People ask the command palette for changes in their own words: "lets increase overload for guitar track to 95%",
"swap all toms for snares", "cut the guitar from 1:02 where drums start", "undo the last 5 commands", "go back to
before we added the whistles". The palette should make them on any laptop and in the browser, without a large model,
and never before the person has seen what claudius understood. That means a button that says what it would change,
from what to what, and a button for each reading when the words could mean more than one thing. How well requests are
read has to be measured, on requests phrased in many ways, with a set of examples that keeps growing and that could
one day tune a model.

A language model that calls tools would read requests best. The small ones made for devices are still tens of
megabytes and some hundreds of milliseconds a request on a laptop, which is for later (see Consequences). What runs
now has to answer as the person types.

## Decision

`claudius_intent::ask` reads a request in three steps, in microseconds, in the engine (a server's, and the demo's in
the page):

- **What it asks**: an undo, a redo, going back to before a change; a track's level, drive, reverb, delay or pan; the
  tempo or the key; a track's sound swapped, cut from a time, muted, unmuted, soloed or removed. This is read from the
  words each kind is asked in, each with how surely it tells the kind, and from the example requests nearest it in
  meaning (`data/asks.jsonl`, through the palette's embedding model, [0011](0011-meaning-on-device.md)). The meaning
  is read without the words that name a track, which tell what a request acts on, not what it asks. A word that
  surely tells a kind leads, and the nearest examples break ties; where no word does, the examples decide. Undo, redo
  and going back are only read from words of their own, since nothing else needs naming for them.
- **What it acts on, and by how much**, read off the song by rules. The tracks it names: by name, role or sound, the
  drums as a group, tracks named together as one, "everything" but what comes after "except". Its numbers and their
  units: 95%, -6 dB, 128 BPM, 1:02, "by 3", "x3". Which way: "louder", "too dry", "the bass is a bit loud". The sound
  to swap to, the section a time falls in, and the change in the history it names.
- **What claudius would do**: the commands, and what they do in words, from the song's values now to those asked
  ("Increase guitar's drive from 20% to 95%"). A word that several tracks answer to gives a proposal for each, and
  tracks named together give one. A reading that would change nothing (muting a muted track) proposes nothing, and a
  reading far less likely than the best is not offered beside it.

The palette shows the proposals before the actions, and nothing is done until the person picks one. A proposal's
edits go as one `together` command, which the history records as one change named for what the button said, so undo
takes it back in one step. Undo, redo and going back go as they are.

**The evals** (`tests/asks.rs`) read every example on one song and check the first proposal's kind, tracks, numbers,
time, sound and change, by words alone and with the model. They also check that an action's name, and what claudius
cannot do on a song, propose nothing, since Enter does the first row.

The examples are in three sets. The first is what the readings learn from. The second is held out of what they learn
from. The third, `later`, is requests written after the readings were last tuned. A `later` set is scored once,
before anything is tuned to it, then its misses are fixed and it joins the held out. These are the scores of four
rounds of new requests, before tuning to them:

| round | requests | by words | by meaning |
|---|---|---|---|
| 1 | 41 | 26 (63%) | 30 (73%) |
| 2 | 41 | 32 (78%) | 35 (85%) |
| 3 | 39 | 35 (90%) | 36 (92%) |
| 4 | 34 | 28 (82%) | 30 (88%) |

Round 4 asked more tersely and casually than round 3 ("kick down a bit", "just vocals", "make it 128"), and one of
its misses was a mistake in the example itself. After the fourth round, all 330 examples read right both ways.

## Consequences

- Requests are read in tens of microseconds on any computer and in the browser. Every one is confirmed by a button
  that says what it does, from what to what, so a misreading costs a glance, not a change.
- The readings are rules and examples, so a request in words they have not met is read wrong or not at all. Each
  round of new requests showed it: one in ten to one in three were missed before tuning. The examples are the remedy.
  A request people phrase in a new way goes in as `later`, is scored, and is then fixed.
- A request for two things at once is read as one of them. A request for taste rather than a setting ("make it
  groovier") finds actions, Improve the song among them, rather than edits.
- A small model that turns requests into tool calls on the device would read what the rules cannot. Cactus's Needle
  is one: its weights are open, and a runtime for it in plain Rust (`needle-infer`) runs in the browser too. The
  examples, each with what it should do, are the start of the set it would be tuned and measured on. The rules would
  stay, to check its calls against the song and to answer until it has downloaded.
