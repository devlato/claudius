# The radio

[claudius radio](https://claudius.devlato.com/radio/) is a station of songs claudius writes as it plays them, one
genre after another, each mixed into the next as a DJ mixes records. Nothing on it is recorded: the engine of the
[studio demo](studio.md) writes and renders every song in the page, while the one before it plays, and the page plays
them on two decks of its own (Web Audio).

## What it plays

The station plays every genre but the few with no beat to mix on or too harsh for a radio (ambient, IDM, breakcore,
gabber, hardstyle, hard techno). Its first song is one of a handful that start a set well (house, deep house, nu disco,
hip hop, lo-fi, afrobeats, drum and bass, synthwave, trap, funk), at the middle of its genre's tempo range; it plays
its first bars while the rest renders, about half a minute in. A song is never one of the last four genres it played.

## How it mixes

The next song is written as soon as one goes on air, and three times in four it is one that mixes into it beat for
beat: a genre whose tempo range takes the playing song's tempo, or twice or half of it, written at exactly that tempo
and in a key with the same notes as the playing one's (in the mode its genre family is usually in: hip hop, drum and
bass, techno and house in minor, pop, Latin and soul in major), or now and then a fifth up or down, a step round the
circle of fifths. Since every song is written at the tempo it will be mixed at, nothing is sped up or slowed down, and
no song is heard out of its key.

- **A blend.** The next song starts on a bar line of the outgoing one's outro (its last 16 bars, when it has no outro
  of 8 bars or more), on its own first downbeat, its bass cut. It fades in over the first half of the mix, up to 16
  bars, as long as its intro takes; halfway, the basses swap, and the outgoing song is thinned out by a rising
  high-pass and faded out over the second half.
- **An echo out.** A song in another tempo: the outgoing song's last beat goes into a feedback echo on a dotted
  eighth, the song is cut on the bar, and the next one comes in on its own downbeat a bar later while the echoes die
  away, its bass held back for two bars.
- **A record stop.** One time in five, a song in another tempo comes in instead after the outgoing one slows to a stop
  like a record whose turntable is switched off.

Each change is scheduled on the audio clock as soon as the next song arrives, so it lands on the beat even in a tab
no one is looking at. A song that arrives too late to blend on the outro comes in on the outgoing one's next bar line.

The page is `site/radio/index.html` and `site/assets/radio.js`; it drives the engine as the studio demo does (the
`new_song` command, the renders it sends back), and `site/tests/radio.spec.mjs` checks it goes on air and mixes a song
into the next (`?soon` starts the first song 20 seconds before it gives way).
