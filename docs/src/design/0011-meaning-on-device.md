# 0011 Finding by meaning, on the device

Status: accepted

## Context

The command palette (Cmd+K) found the studio's actions by their names and a few words listed for each. People say
what they want in words of their own ("make it sound better", "sing over it", "go back"), and the palette should find
what they mean. It has to do so on the computer claudius runs on, a weak laptop with integrated graphics and a browser
tab included, without sending the words anywhere. Two later features will ask the same of it: finding what was done
to a song from a description of it, and commands in plain words, which need a model that turns words into claudius's
commands.

These were the options:

- **A sentence transformer** (MiniLM, bge-small: 22 to 33 million parameters, run with ONNX Runtime). It finds
  meaning well, but takes tens of milliseconds a query on a weak processor and 30 to 130 MB, and ONNX Runtime is not in
  the web build.
- **A vector database extension** (sqlite-vec). It is C, reached through FFI and built for each target, the web too.
  claudius's lists hold hundreds of entries, which a flat scan searches exactly in microseconds.
- **Static embeddings** (Model2Vec's potion models, MIT). They hold a vector for each word piece, learned from a
  sentence transformer, and a phrase is the mean of its pieces' vectors. Embedding is a lookup and a sum, with no
  neural network run at all.
- **A small function-calling model** (Cactus's Needle, 121 million parameters, Apache-2.0). It turns words into
  commands rather than searching, and it is far heavier than search needs.

## Decision

`claudius-intent`, a crate of plain Rust whose only dependencies are serde and serde_json, finds by meaning:

- **Static embeddings**: potion-base-8M (29,528 pieces, 256 numbers each), from a pinned revision checked against its
  hashes. claudius keeps it as a file of its own, each number a byte with a scale for each vector: 7.9 MB, read in a
  few milliseconds, with every cosine as it was to a hundredth.
- **A flat index in memory** (`Index`): the exact cosine to every vector. There is no approximate index until a list
  grows past tens of thousands.
- **Names and meaning together** (`Finder`): a name that matches is found at once and is never outranked by a guess,
  meaning finds what names miss, and an action both find ranks highest.
- **Where the studio's commands go**: a server's session answers the palette (`Host::find`, `GET /api/find`), and
  the demo answers on the studio's own thread, since its engine may be rendering for seconds when a search takes
  microseconds. Natively the model downloads the first time the palette is typed in, the demo fetches the site's
  copy, and until it is there names find everything.
- **Evals**: `tests/meaning.rs` holds the phrasings people use for each action, in one set the ranking was tuned on and
  two held out from it, and fails when fewer than 95% of the first set (85% of the held-out ones) find their action
  among the first three.

## Consequences

- Search is fast and light everywhere. On an Apple M-series laptop a search takes about 50 microseconds, and embedding
  the actions, once as the model comes, a third of a millisecond. It needs no graphics card and no ONNX Runtime, and it
  adds 8 MB to the web demo, fetched only once the palette is used.
- Static embeddings know words, not sentences. Word order is lost ("undo the redo" reads as "redo the undo"), and so
  is negation, and a word the vocabulary has never seen counts for nothing. Matching names and the words listed for
  each action make up for much of it, and the evals keep count of the rest.
- The model knows English only.
- Each phrasing added to the catalogue or to the evals is data for a later fine-tune or a larger model. The evals stay
  as they are when the model changes, and a model that does worse on them does not ship.
- Searching a song's history needs nothing new: the same index over its changes, each described in words. Commands in
  plain words need a function-calling model, which is a decision for a record of its own. Such a model must ask
  claudius for facts (the tracks, the history) rather than guess them, and claudius shows the person what it understood
  and acts only once they confirm.
