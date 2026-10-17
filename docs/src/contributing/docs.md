# Writing docs

The docs have three audiences, and each page is written for one of them:

- **People who have never made music.** The "Learn to make music" chapters. Every musical word is explained the
  first time it appears, with something to listen to.
- **Producers and musicians.** The "For producers" chapters and the cookbook. They know what a ii–V–I or a
  sidechain is; tell them how claudius does it and where it differs from their DAW.
- **Engineers.** The engineering chapters, the design records and the contributor guide. They want to know how the
  code works and why it is built that way.

The language reference serves everyone: it is the precise description, and the tutorials link into it.

## Voice

Write the way you would explain something to a colleague sitting next to you: plainly, in full sentences, with the
thing on screen. Use "you". Use the present tense. Say what happens, not what "can" or "will" happen.

Be specific. "The kick drops out for the last bar of a build" is useful; "builds add tension" is not. If a sentence
would be true of any music software, cut it.

Prefer one good example to three paragraphs of explanation. Show the code, say what you will hear, then explain the
part that is not obvious.

Keep paragraphs short. A page that needs a summary at the end is too long or badly ordered.

## Words we do not use

These either say nothing or sound like a brochure: *comprehensive, robust, seamless, leverage, delve, unlock,
empower, elevate, harness, cutting-edge, state-of-the-art, game-changer, effortless, blazing, powerful, world-class,
journey, dive in, deep dive, in today's world, whether you're X or Y, it's worth noting, in conclusion*.

Avoid *simply*, *just* and *easy*: if the reader is stuck, they now also feel stupid. Avoid exclamation marks and
emoji. Do not open a page by restating its title, and do not end it by restating what it said.

## Formatting

- Headings in sentence case: "Song structure", not "Song Structure".
- Code, function names, file names and commands in backticks: `track()`, `song.rhai`, `claudius serve`.
- Units with a space: 124 BPM, −6 dB, −9 LUFS, 8 kHz. Bars and beats are counted from 1 in prose ("the snare on
  beats 2 and 4"), steps from 1 too ("a hit on step 5").
- Tables are for reference material (function lists, parameters). Explanations go in prose.
- Link the first mention of a function to its entry in the language reference.

## Examples are tested

Every fenced block marked `js` is run by `cargo test -p claudius --test docs_examples`, so an example that stops
working fails the build. The fence's info string says how:

| fence | what the test does |
|---|---|
| ` ```js ` | runs it; it must succeed |
| ` ```js packs ` | runs it when the sample packs are installed (it uses sampled instruments) |
| ` ```js error ` | runs it; it must fail (for showing error messages) |
| ` ```js no-run ` | shows it without running it (fragments, signatures) |

An example runs against an open song: house, seed 9, with the tracks `kick clap hat ohat shaker perc bass chords
pad lead arp`, the sections `intro groove break drop outro` and the progressions `A` and `B`. An example that starts
its own song with `song(...)` or `blank(...)` does not depend on any of that.

Lines starting with `~` are run but hidden on the page, for setup the reader does not need to see:

```js
~blank("house");
tempo(124);
track("kick", s("bd*4"));
```

When you show output (a verdict, a log line, an error), paste what the program printed. Do not paraphrase it.

## Music terms

In the "Learn to make music" chapters, explain a term the first time it is used and give the reader a way to hear
it. "A bar is a group of four beats; count 1 2 3 4 along with the kick" is better than a definition from a
dictionary.

Elsewhere, assume the reader knows the term, and link to the beginner chapter that explains it.

## Accuracy

Run what you describe. If the docs and the program disagree, the program is right and the docs get fixed, or the
program has a bug and gets an issue. Do not describe features that do not exist yet.

## Checking your work

```sh
cargo test -p claudius --test docs_examples   # every example runs
cargo xtask docs                               # builds the book into target/book
```
