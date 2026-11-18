# 0023 The live mix, first by windows

Status: accepted

## Context

A fader moved in the studio was heard only when the whole song had rendered again: the edit, a render of every
track's mix through the buses, returns and master, the master's loudness search, the file written, fetched and decoded.
On a four-minute song that takes a few seconds, and moving a fader by ear, the way people mix, is not possible.

Design [0018](0018-processors.md) and [0019](0019-libraries.md) name the live mix that fixes it: each track's signal
before its fader kept from the last render, and a real-time mixer playing them, its buses, returns and master running
block by block in a runtime that a callback on the computer and an AudioWorklet in a browser both play. That runtime
asks every effect on a bus, a return or the master to work in blocks, keeping its state between them, and today's
effects work over whole buffers. Moving them is the larger part of the work, and each move is to be held to tests.

A render already keeps what the live mix needs. Each track's sound before the mix is kept between renders
(`render::Kept`), so an edit of the mix makes no track again, and `render::render_window` mixes any stretch of the song
from the kept sounds through the buses, returns and master, starting early enough for every effect to settle, at the
loudness the last render settled. Its windows equal the whole render there to within 1e-6. The demo's engine already
uses it (`Host::preview`): an edit is first the last render with the twelve seconds from the playhead rendered again
and crossfaded in, handed to its player as a new render, in about 0.8 s in a browser. A server did nothing of the sort,
and doing it the demo's way would send the whole song again, a 24-bit WAV of 70 MB for four minutes, for each move of a
fader.

## Decision

The live mix comes first by windows of the song, and the runtime of 0019 replaces them later.

- **The server keeps** what each render leaves for windows: the kept sounds, shared rather than copied
  (`Kept::share`, each sound behind an `Arc`), and the loudness the render settled.
- **`/api/window?from=&secs=`** mixes that stretch of the song as it is now (`App::window`), up to ten seconds, and
  sends the song's version, the window's first sample and its rate, then its samples as 32-bit floats. When an edit
  changed a track's own sound (its notes, its sound, a section) or the song's length, it says so with a 409, and the
  next render plays the change, as before.
- **The studio**, while the song plays and is newer than the render playing, asks for a window from just after the
  playhead (0.15 s, about what one takes to come), then the next, a second each, up to four seconds ahead of the
  playhead, and again from the playhead when the song changes again (`live::Live`).
- **The desktop player plays each window** in the render's place, faded in and out over 5 ms against what plays
  beneath it, so that the mix changing never clicks; windows of one version that follow on are one stretch, with no
  fade between them. The next render replaces them all.

- **The page's player** (the web studio connected to a claudius) plays on Web Audio: the render decoded once into a
  buffer, played from it on the audio clock. A window goes into the buffer over what the render holds there, faded in
  and out over 5 ms as on the desktop, and a source started again at the moment it would be fades in over the one
  playing, which may have read the buffer when it started; both play the same up to the window, which comes after it.
  It once played a whole file through an audio element, over which nothing could be played.

The demo's engine makes no windows yet: its preview (the last render with the twelve seconds from the playhead
rendered again and crossfaded in) becomes windows made in its worker as the server makes them, the next step.

## Consequences

- A fader, a pan, a send or an effect on a bus, a return or the master is heard a few tenths of a second after it
  moves, while the song plays.
- Each window mixes the seconds before it again, for the effects to settle: a second of song costs a few seconds of
  mixing, and four seconds ahead are kept. On a laptop that is a small share of a core; the runtime of 0019 removes the
  repetition.
- What a window plays is what the next render plays, but for the master's loudness, which the render searches again
  over the whole song: a fader moved far shifts the level a little when the render comes.
- An edit of what a track plays is heard when the render comes, as before.

## Rejected

- **Waiting for the runtime of 0019.** Every effect has to move to blocks first; windows give the studio a live mix
  now from code that already renders the song, and the runtime replaces them where it is ready.
- **Streaming the whole song from the server as it renders.** A stream renders ahead of the playhead by what the
  connection buffers, so an edit is heard only after that; windows asked for from the playhead are heard at once.
