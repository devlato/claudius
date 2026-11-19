# 0024 The radio in the app

Status: accepted

## Context

The site's [radio](../getting-started/radio.md) writes songs as it plays them, a genre at a time, and mixes each into
the next as a DJ mixes records. It is a page: the studio demo's engine in a Web Worker writes and renders each song,
and the page schedules the change on Web Audio's clock (`site/assets/radio.js`). The plan asked for the radio in the
desktop app too. The app has neither a worker nor Web Audio, and its own song must not be disturbed by the station's.

## Decision

- **The station is shared code.** `claudius_session::radio` plans as the page does, ported from it with tests: the
  genres a station plays (less the few with no beat to mix on or too harsh for a radio, kept to the ones chosen),
  the first song from a set of openers, and each next one, three times in four a genre whose tempo range takes the
  playing song's tempo (or its double or half) in a key with the same notes, else another tempo and key.
- **An engine of its own.** The app's radio runs a `claudius_session::host::Host` on a thread, as the demo does, and
  asks it for one song at a time: `new_song` with the station's genre, seed, tempo and key, then a whole render. The
  person's song, in the studio's session, is untouched.
- **Each change rendered as audio.** Where the page schedules gain, filter and echo changes on its clock, the app
  renders each change once the next song is in (`radio::change`): a blend under the outro with the basses swapping
  halfway and a rising high-pass, an echo out on the bar, or a record stop. A skip renders a cut on the next beat
  (`radio::cut`). The radio's player (`audio::radio`) plays one stream of pieces placed at frames of it: the song on
  air up to the change, the change, the next song from where the change leaves it. The station places pieces ahead of
  where the stream is and may place them again; the audio thread plays whatever covers its frame.
- **The same controls as the page.** Tune in and pause, Skip, the genres to keep to, Save this song (an MP3 in the
  songs folder's `radio`), and Open it in the studio, which asks the studio's session for the same `new_song`, so that
  undo brings back the song that was open. The studio's song and the radio take turns: one started pauses the other.

## Consequences

- The page and the app plan the same way from two sources, JavaScript and Rust. The Rust station is the one the
  engine could serve the page from later.
- A change is exact to the sample, and costs a few hundred milliseconds of rendering when the next song comes in.
- The first song takes as long as a whole render of it, as the app has no first bars to play meanwhile yet.
