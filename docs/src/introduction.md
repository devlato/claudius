# claudius

claudius makes music the way a producer does in a DAW. It picks sounds, writes drum patterns, bass lines, chords and
melodies with music theory, arranges them into a song with an intro, builds and drops, mixes and masters it, listens
to the result, and changes what it does not like.

Every sound is real audio. There is no neural audio generation and no General MIDI: drums and synths are
synthesized in code, and the rest comes from recordings (a grand piano, a Rhodes, string sections, real TR-808 and
TR-909 drum machines, Apple Loops if you have GarageBand, or your own files). Notes exist only as the score that
plays those sounds, like a piano roll.

After every change claudius judges the song: is it loud enough and clean at the top, does the drop hit harder than
the breakdown, are the bass notes on the chords, does it sound like the genre it claims to be? Each problem comes
with what to change.

## Four ways to use it

- **The studio.** An app with an arrangement view, step sequencer, mixer, harmony editor and script editor. It runs
  as a native window and in the browser. See [the studio](getting-started/studio.md).
- **Scripts.** A small language in which a song is a few lines of code: `track("drums", s("bd*4, [~ cp]*2"))`. Any
  song claudius makes can be printed as a script. See the [language reference](language/overview.md).
- **An assistant.** Connect Claude, ChatGPT or Cursor and ask for a song; it composes, listens, and revises while you
  hear every change in the studio. See [working with an assistant](getting-started/assistants.md).
- **The command line.** `claudius make --genre lofi` writes a finished song to a folder. See
  [command line](guide/cli.md).

## Where to start

- **You have never made music.** Install it, make [your first song](getting-started/first-song.md), then follow
  [how a track is built](learn/basics/how-a-track-is-built.md). The chapters explain every term as it comes up and
  give you something to listen to on every page.
- **You make music already.** Read [claudius for DAW users](learn/producers/daw-users.md) and keep the
  [quick reference](language/quick-reference.md) open. If you know Strudel or TidalCycles, start with
  [coming from Strudel](learn/producers/strudel.md).
- **You want to work on claudius.** Start with [a tour of the repository](contributing/onboarding.md) and the
  [architecture](engineering/architecture.md).
