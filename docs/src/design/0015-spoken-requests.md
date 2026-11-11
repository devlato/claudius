# 0015 Requests said aloud, heard on the device

Status: accepted

## Context

A person making music often has their hands on an instrument or a controller, and saying "mute the drums" is quicker
than reaching for the keyboard. The palette reads requests as text ([0013](0013-requests.md),
[0014](0014-request-model.md)), so a speech model has to turn what is said into text. It has to run on the person's
computer, as the readings and the request model do, on any processor, and in Rust ([0002](0002-rust.md)).

Cactus Compute's Whistle is made for this. It is 16.9 MB, under the Apache 2.0 licence, an encoder and decoder built
of Needle 3's parts: a log-mel front end, a convolution stem, eight encoder blocks, and eight decoder layers that
attend to the encoder. It hears seven languages, in up to 30 seconds at once.

## Decision

`crates/whistle` ports Whistle to Rust, on needle-core's kernels, from Cactus's JAX reference and its notes on porting
the engine. It was checked against whistle-rust, another port, at every stage: the front end's features to within
10⁻⁶ of them, the encoder's output to a cosine similarity above 0.999, the decoder's logits for the same input, and the
transcripts.

- **Listening**: the palette's microphone records the default input until a pause of 1.2 s after speech, or for 15 s
  at most, and sends the WAV file of it to be heard (`POST /api/hear`).
- **Hearing** (`claudius_session::hearing`): the recording is decoded, mixed to mono and resampled to 16 kHz.
  Nothing as loud as speech at its loudest (-40 dBFS) is heard as nothing, before the model sees it: a speech model
  raises the level of what it hears, and hears words in the hiss of a quiet room ("Thank you."). What is said is cut
  out of the silence around it, from a tenth of a second before it to a quarter after: a person pauses after clicking,
  and after a pause of a fifth of a second Whistle heard "mute the drums" as "Mew the drums". Whistle hears English,
  the language the readings read, with a beam search of five. It downloads the first time (from Hugging Face, at a
  pinned revision, checked by its size and SHA-256).
- **Putting it right** (`claudius_intent::spoken`): a word heard that claudius does not know, one letter from one word
  it does and from no other, is taken as that word, the song's tracks first, then the words of the studio's actions
  and of the example requests ("undu" as undo, "base" as bass when the song has a bass). The readings read times as
  they are said ("one minute two seconds") and numbers in words up to 99, for typed requests too.
- The words go into the palette's field as if they were typed, with the buttons they find, and nothing changes until
  one is picked.

**The evals** (`tests/hearing.rs`, with a folder of clips) hear each clip as the palette would and count the words
missed against the words said. On nine requests said by two of macOS's voices, and three clips of silence, hiss and
noise, 3 of 51 words were missed, and nothing was heard in the silence; with a second of silence before each request,
as after a click, 2 of 51. The worst was "more reverb on the piano", heard as "Moreiver bone the piano". A 3-second
request is heard in about half a second on one core, in about 50 MB. In the browser (`site/tests/palette.spec.mjs`,
with a clip and the model), the palette's microphone opens, listens until the pause, and puts what was said in its
field with the row it finds.

## Consequences

- Saying a request costs 17 MB downloaded once and half a second.
- A word the model mishears that claudius cannot put right stays wrong in the field, where the person sees it and can
  type over it. Biasing the model's search towards claudius's words (the song's tracks, the actions, the requests'
  words) is the next step.
- The demo on the website has no microphone: Whistle runs in a page, but half a second on its one thread would hold the
  studio up, and needs a worker, as Needle does.
- The clips are said by macOS's voices, which claudius does not ship, so the eval runs where a folder of clips is
  given; requests people record of themselves are the clips to gather next.
