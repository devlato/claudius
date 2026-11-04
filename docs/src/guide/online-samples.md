# Samples from free online libraries

Drum breaks, vinyl crackle, field recordings, old records, stems: claudius finds them in free online libraries and
downloads them, keeping only what their licences let you make and publish music with.

```sh
claudius samples search "vinyl crackle"
claudius samples get freesound:383762
```

## The libraries

| library | what it has | how claudius asks |
|---|---|---|
| [Openverse](https://openverse.org) | an index of Freesound's sounds (breaks, loops, one-shots, textures, field recordings), Wikimedia Commons' recordings and Jamendo's songs | its API, without a key: 20 searches a minute, 200 a day |
| [Wikimedia Commons](https://commons.wikimedia.org) | recordings Openverse leaves out: old records, orchestras, instruments, free stems | its API |
| [ccMixter](https://ccmixter.org) | stems, a cappellas and samples shared to be remixed | its API |

Each search asks all three, unless `--source` names some. Openverse returns Freesound's high-quality previews (MP3 at
about 128 kbps), which is what claudius downloads from it.

## The licences

A search keeps what you may publish music made with, with at most a credit, as for [sample packs](packs.md):

| licence | kept | what it asks of you |
|---|---|---|
| CC0, a public domain mark from the author | yes | nothing; claudius credits them anyway |
| CC BY | yes | credit, which claudius writes into the song's `credits.txt` |
| public domain by age (records from before 1926, say) | with `--us-public-domain` | nothing in the United States; elsewhere the tune on the record may still belong to its composer's heirs, for 70 years after the composer's death |
| CC BY-SA | with `--share-alike` | the music made with it must carry the same licence |
| non-commercial or no derivatives | never | |

The libraries record the licence each uploader chose. An uploader cannot give away a record they do not own, so
claudius also leaves out uploads whose own titles or tags say they were cut from someone's record ("sampled from",
"70s sample", "vinyl rip"). That is a guard, not a guarantee: a sound you plan to publish is worth a look at its
page, which its credit links.

[0010 Samples from online libraries](../design/0010-online-samples.md) records why these libraries and not YouTube,
Spotify, Apple Music or SoundCloud.

## Where they go, and credits

A sound downloads into the samples folder, `<data dir>/claudius/samples/<library>/` (`CLAUDIUS_SAMPLES` moves it), with
a `.json` beside it: its title, author, licence and page. `claudius samples list` lists them.

A song that plays one, through `.s("/path/to/the/sound.mp3")` in a script or anywhere else a file can be used, credits
it in its `credits.txt`:

```text
Samples:
"Crunchy Drum Break 2 [128bpm]" by deadrobotmusic, CC0 1.0 (https://creativecommons.org/publicdomain/zero/1.0/), https://freesound.org/people/deadrobotmusic/sounds/629052
"Bauchamp - 130 old drum break" by Bauchamp, CC0 (http://creativecommons.org/publicdomain/zero/1.0/deed.en), https://commons.wikimedia.org/wiki/File:Bauchamp_-_130_old_drum_break.ogg
```

A claudius shared with others may read the samples folder, as it reads the sample packs
([What a shared claudius may read and write](samples.md#what-a-shared-claudius-may-read-and-write)).

## Songs built on samples

A song of any genre can be built on samples, as records built on samples are: a phrase from a recording that the song
is built on, a drum break, and a texture under it. Trip-hop, boom bap, lo-fi, jungle, big beat and filter house are
by default, as their records are; a song of another genre is when asked:

```sh
claudius make --genre trip_hop                            # gathers what trip-hop samples first, then composes
claudius make --genre house --sampled                     # house built on the records house producers dig for
claudius make --genre house --sample-search "70s soul"    # ... or on a phrase found for your own words
claudius make --genre lofi --unsampled                    # lo-fi of its own sounds
claudius samples gather trip_hop                          # or gather on its own
claudius make --genre trip_hop --offline                  # play what was gathered before, download nothing
```

`new_song` takes the same as `sampled` and `sample_search`, and a script's `song()` as options of the same names.

### What each genre digs for

A genre's profile says what its producers sample (`sampling`): not its own name or a mood, but kinds of records.
Trip-hop digs for film scores, spy themes, library music and psychedelic soul; boom bap for soul, jazz piano, horn
sections and vibraphone; house for disco, gospel choirs, philly soul and organ; filter house for disco loops it plays
through a low-pass filter that opens in its drops; jungle for funk and jazz drum breaks and dub reggae; trap for
orchestral strings, choirs, flutes and bells; afrobeats for highlife and palm-wine guitar. Each gathering tries the
genre's searches in an order the song's seed picks, and the recordings each finds in an order the seed picks too, so
that two songs dig through different records. Words of your own replace the genre's searches for the phrase; the
break and the texture still come from the genre's.

### How a sample is cut and played

Gathering downloads recordings of a fitting length and cuts each on its beat grid:

| role | what is cut | how a song plays it |
|---|---|---|
| break | whole bars of the break, and its kick, snare and hat | as a loop track (`break`), re-chopped at times, where the genre's sections list it, else in place of the layers the profile names (trip-hop's kit, house's top loop and shaker); the drum lanes it stands in for play the break's own hits |
| phrase | the two bars the ears hear as most like what was asked for ("dark cinematic orchestral strings", or your words), with their key and chords | as a loop track (`sample`) in place of the genre's chords or keys; the song takes its key, its tempo and its chords as progression A. A genre with no major mode plays a major phrase in its relative minor |
| texture | sixteen seconds from the middle | as a loop track (`texture`) under the song |

A phrase plays as a sampler plays a record: faster or slower by whole semitones, its pitch moving with its tempo,
until its tempo (or half or twice it) falls in the genre's range. A soul record at 98 BPM plays four semitones up in a
house song at 123.47 BPM, in a key four semitones above its own, and the song takes that tempo and key, so the phrase
is only resampled, never stretched, and plays in tune with everything else. How far a genre goes is in its profile
(`pitch`: trip-hop slows a record by up to five semitones, house speeds one up by up to five), and the song's lineage
says how far it went (`sampled: phrase 'Old Soul' 4 semitones up`). A phrase no such speed fits is stretched to the
genre's tempo in its own key where claudius can stretch; the web demo, which cannot, goes without it.
Each role keeps its own colour from the profile: the high-pass that leaves the low end to the song's bass, the tape,
the 12-bit crush.

With earshot's models, the ears score each cut against what was asked for, and cuts under z 1.5 are left out and not
fetched again. The cuts go into `<samples>/prepared/`, each with its recording's credit beside it, so a song's
`credits.txt` names them.

### Gathering, a few at a time

The studio and an assistant gather the way `claudius make` does. A new song built on samples whose genre (or words)
has fewer of a role than it keeps starts a job, *sampling for house*, that gathers up to two more of each, then makes
the song with them. Its log shows each recording tried. **Stop** ends the gathering, and the song is made with what
is there. So a genre's collection grows with its songs, to the number its profile keeps (eight phrases, six breaks
and three textures for trip-hop), and each song draws one of each by its seed. A role the libraries had nothing more
of is not searched again until claudius starts again. A claudius shared with others (`--root`) and
`claudius studio --offline` gather nothing: they play what was gathered before, or what stands in for it.

Offline, or before anything is gathered, a genre plays the layers its profile names in a sample's place (trip-hop's
drum lanes for the break, its electric piano and strings for the phrase), so it still makes a song.

## In the web demo

The [studio demo](https://claudius.devlato.com/studio/?mock) samples as well. A trip-hop song there is built on loops
from Freesound under CC0 or CC BY, found as the song is made: a phrase it takes its tempo, key and chords from, a
break, and crackle. The demo asks the website for them, and the website asks Openverse and Freesound, so your browser
contacts no one else. Each track names its recording, author and licence, and the log gives each one's full credit.
