# 0014 A small model taught to read requests

Status: accepted

## Context

The readings of [0013](0013-requests.md) are rules and examples. They read most requests in microseconds, but a
request in words they have not met is read wrong or not at all: of 39 requests written after they were last tuned,
they read 32. A model that has learned a language reads words it was not shown. It has to run on the person's
computer, as the readings do, on any processor, and it has to be taught there too: claudius is Rust from end to end
([0002](0002-rust.md)), with no Python to install, to run a model with or to teach one with.

Cactus Compute's Needle 3 is made for this kind of work. Its 121 million parameters read a request and the tools it is
given, and write the calls. Its weights are 35 MB, in two and four bits, under the Apache 2.0 licence, and
`needle-infer` runs it in plain Rust, in the browser too (MIT). Untaught, it read 27 of 330 example requests on
claudius's tools (8%), nearly all of them undos.

## Decision

claudius teaches Needle 3 to read its requests, and asks it only about what the readings read nothing of
(`claudius_intent::needle`, `teach`).

- **What it reads**: sixteen tools, one for each thing a request can ask, named as people say it (`set_volume`, with
  `way` louder or quieter, `by_db` and `to_db`; `swap_sound`; `cut_track` from a time), each argument in the request's
  own words, its tracks as said ("the toms", "everything but the vocals"). They are 756 tokens, read once when the
  model loads and kept, so a request costs only its own tokens and its answer; the tools, a request and its answer fit
  the 1024 positions the model attends to.
- **What it is taught**: lessons made from 245 ways of saying the requests, each said 20 times with other tracks,
  numbers (in digits, or in words: "seven"), sounds, keys and times, and requests no tool serves (an action's name,
  talk, a question, something asked not to be done), one lesson in ten, which call nothing (`teach::lessons`). Each
  answer is a line of reasoning that quotes the words each argument comes from, then the call, as Cactus teaches
  Needle.
- **How it is taught**: `claudius-tune`, in Rust, runs Needle 3 in candle on the CPU, on its weights as needle-core
  reads them; its forward pass gives needle-core's logits to 1.3·10⁻⁵ of their size (`claudius-tune parity`). Only
  low-rank adapters are taught (rank 16, beside the five projections of each block's attention: its queries, keys,
  values, gate and output), with the loss on the answer alone. The model is run to 8 of its 20 layers, a depth
  Needle 3 is made to answer from, at less than half the cost. 5,456 lessons, one in ten held out, four times over:
  47 minutes on a laptop's ten cores. The adapters are 1.6 MB in half precision and built into claudius
  (`data/needle-adapters.safetensors`), with a fingerprint of the tools they were taught with.
- **How claudius reads with it**: on a thread of its own, the model downloads once (from Hugging Face, at a pinned
  revision, checked by its size and SHA-256), reads the tools, then reads the latest request the palette asks about.
  claudius runs the model a token at a time itself, needle-core's kernels with the adapters added, and decodes
  greedily, the call held to the tools' grammar. It refuses a call that does not fit its tool (words where a number
  goes), one whose reasoning quotes no words the request says, or words it does not, and a reasoning that runs past 48
  tokens, as a model's does that cannot tell what is asked. A call it keeps is said again in the words the readings
  read ("set the guitar's drive to 95%", `teach::said`), and the readings ground it in the song as they ground a
  request: its tracks, its values from what the song has now, its change in the history. The proposals say they are
  the model's (`by_model`), and nothing changes until one is picked.

**The evals** (`tests/asks.rs`, with the model's file) read every example with the model, alone and where the
readings read nothing, and 30 requests that ask for no change (a question, talk, an action's name), for which it
should offer nothing. `tests/needle.rs` holds claudius's step of the model to needle-core's.

| | requests | the readings | the model alone | the readings, then the model |
|---|---|---|---|---|
| every example | 369 | 362 | 273 | 363 |
| round 5, written after the readings were tuned | 39 | 32 | 23 | 33 |

Where the readings read nothing, the model offered no wrong change for an example, and it offered a change for 2 of
the 30 requests that ask for none ("share this with a friend" as an undo). Its checks refuse one call in eleven,
three in four of them wrong, and one of the three changes it would offer for requests that ask none.

## Consequences

- The model reads some of what the readings miss, one of the seven in round 5 ("bring the choir in again"), at a
  cost of 35 MB downloaded once, about 100 MB of memory, and about half a second for each request the readings
  cannot read, on one core. Requests the readings read are answered as fast as before.
- Alone, it reads three requests in five that it was not taught, against the readings' four in five: it was taught on
  ways of saying things made from templates, and people's requests stray further from them. The readings stay first.
- The example requests are the model's test, not its lessons, so they measure the readings and the model alike.
- It reads in the app and not yet in the demo in the browser. needle-infer runs there, but half a second on the page's
  one thread would hold the studio up; it needs a worker.
- A change to the tools needs the adapters taught again: `cargo run --release -p claudius-tune -- train
  needle3.cact crates/claudius-intent/data/needle-adapters.safetensors` (`--depth`, `--epochs`, `--each`, `--lr`),
  and claudius refuses adapters taught with other tools. `claudius-tune check` compares the trainer's model with the
  app's, logits and decoding.
- Teaching it on the example requests the readings learn from, as people wrote them, and on more of its layers, are
  the next things to try, each measured on a round of requests written after it.
