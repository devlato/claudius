# Infinite Radio

[Infinite Radio](https://claudius.devlato.com/radio/) is a station of songs claudius writes as it plays them, one
genre after another, each mixed into the next as a DJ mixes records. Nothing on it is prerecorded: the engine of the
[studio demo](studio.md) writes and renders every song in the page, while the one before it plays, and the page plays
them on two decks of its own (Web Audio). A song of a genre built on samples plays recordings from free libraries, as
the genre's producers dig for records ([below](#samples)).

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

- **A blend**, as a DJ mixes on a three-band mixer. The next song starts on a bar line of the outgoing one's outro
  (its last 16 bars, when it has no outro of 8 bars or more), on its own first downbeat. Its fader comes up over the
  first quarter of the mix with its bass and its mids held back, so that its highs come in first; halfway, on the
  phrase, the basses swap, its mids come in round the swap and the outgoing song's leave just after, and the outgoing
  song is thinned out by a rising high-pass and faded out over the second half. Between two songs of the genres DJs
  blend over long intros and outros of beats alone (house, techno and trance, drum and bass, EDM, bass music, hard
  dance), the mix runs as long as the next song's intro, up to 32 bars; between songs that sing from their first bars,
  over a phrase of 8. When the outgoing song's beat stops before its end (an outro without drums) or the next one's
  starts after a quiet intro, laying the one over the other would leave the music without a beat for as long as the
  mix lasts, so the beat is handed over instead: the next song's intro runs under the outgoing one's last bars with a
  beat (up to 8 of them, its start left out to fit; at least 2, its bass held back), and its beat comes in on the
  downbeat where the other's ended, the outgoing song faded over its last beat and its quiet outro left out.
- **An echo out.** A song in another tempo: the outgoing song's last beat goes into a feedback echo on a dotted
  eighth, the song is cut on the bar, and the next one comes in on its own downbeat a bar later while the echoes die
  away, its bass held back for two bars.
- **A record stop.** After a hip hop or R&B song, where DJs do it, about one time in three a song in another tempo
  comes in instead after the outgoing one slows to a stop like a record whose turntable is switched off.

Each change is scheduled on the audio clock as soon as the next song arrives, so it lands on the beat even in a tab
no one is looking at. A song that arrives too late to blend on the outro comes in on the outgoing one's next bar line.

## The genres

**Genres**, under the controls, keeps the station to the genres chosen: one, and every song is of it, at other tempos
and in other keys within its range; or a few, each song mixed into the next as before (fewer than five come round
again sooner than every fifth song). A choice holds from the next song the station writes, and the first song is one
of them. Now and then, about one song in six, a station kept to two genres or more fuses two of them into one song
(house + techno, as fusions are written in the studio), at the first one's tempo and in its key. The address keeps the
choice (`/radio/?genres=house,deep_house`), so a station of your own can be shared and starts on its genres. Opening
the list before tuning in starts the engine, which says what it plays.

## Skip, keep, open

- **Skip** brings the next song in from the playing one's next beat, as a DJ cuts a record short. A song that was to
  blend in (at the playing one's tempo, in a key of its notes) is mixed in over two bars: its bass held back until the
  second bar, where the basses swap, and the playing song thinned and faded out under it. Another goes in after the
  playing song's last beat goes into the echo and it is cut, as an echo out ends a song. While the next song is still
  being written, it comes in as soon as it is.
- **Download this song** makes an MP3 of the song on air, as it plays, in the engine's worker (as the studio demo
  exports), and saves it.
- **Open it in the studio** opens the [studio demo](studio.md#carrying-on-in-claudius) on the same song, which its
  engine writes again from what the station asked for: its genre, seed, tempo and key, and the recordings it sampled,
  which the studio fetches and cuts again before it writes the song. There every part of it can be changed, sampled,
  exported or carried on in claudius.

## Samples

The station samples as each genre does, but not every song: about two songs in three of a genre built on samples (trip
hop, boom bap, lo-fi, phonk and the others whose producers dig for records) are built on samples, and the rest on the
genre's own sounds alone. Before the engine writes a song built on samples, the page looks for a few recordings through the
site's search of Freesound, Jamendo and Wikimedia Commons (under CC0 or CC BY, as the studio demo does): two of each of
the genre's searches for each part it samples, in an order of the song's own. The engine chooses which of each search's
finds are worth fetching (of a length the part takes, and none whose own words say it was cut from someone's record),
and cuts them; the song is written on those it cut, for up to 45 seconds while the song before plays (20 for the first,
which you wait for). A part nothing is found for goes without, and with no search at all the song plays what stands in
for samples in its genre. What a song samples is credited under it while it is on air.

## In the app

The studio app has Infinite Radio too: **Infinite Radio…** in the **?** menu, or Cmd+K. It plays the same way, with an
engine of its own writing the station's songs while your song stays as it is, and the same controls: Tune in, Skip, the
genres to keep to, **Save this song** (an MP3 in `~/Music/claudius/radio`), and **Open it in the studio**, which makes
the song on air the studio's, the very song you heard (undo brings back the song you had). Your song and the radio take
turns: playing one pauses the other. It plays on while its window is closed
([design 0024](../design/0024-the-radio-in-the-app.md)).

In the app the station samples as the studio's songs do: on the recordings gathered on this computer, and a few more
the radio finds in the free libraries before it writes the song, for up to a minute while the song before plays (20
seconds for the first song, which you wait for). With no internet it plays what it has gathered,
or what stands in for samples in its genre. What a song samples is credited under it while it is on air, and **Save
this song** writes the credits beside the MP3 (`<title>.credits.txt`), as the licences of the recordings ask.

The page is `site/radio/index.html` and `site/assets/radio.js`; it drives the engine as the studio demo does (the
`new_song` command, the renders it sends back), and `site/tests/radio.spec.mjs` checks it goes on air and mixes a song
into the next (`?soon` starts the first song 20 seconds before it gives way), that a station kept to two genres plays
only them, hands its song over as an MP3, links it to the studio and skips to the next, and that a trip hop song finds
its recordings through the search (a stand-in answers it), credits them on air and hands them to the studio.
