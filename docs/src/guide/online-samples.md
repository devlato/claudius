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

## Genres built on samples

Trip-hop is built on samples, as trip-hop records are: a drum break, a phrase from a recording that the song is built
on, and a texture under it. Its profile says what to look for (`sampling`), and `claudius make` gathers them before it
composes:

```sh
claudius make --genre trip_hop          # gathers what trip-hop samples first, then composes
claudius samples gather trip_hop        # or gather on its own
claudius make --genre trip_hop --offline  # play what was gathered before, download nothing
```

Gathering searches the libraries for each role's words ("slow drum break", "cinematic strings", "vinyl crackle"),
downloads recordings of a fitting length, and cuts each on its beat grid:

| role | what is cut | how a song plays it |
|---|---|---|
| break | whole bars of the break, and its kick, snare and hat | as a loop track (`break`) in the sections that list it, re-chopped at times; the drum lanes play the break's own hits |
| phrase | the two bars the ears hear as most like what the genre listens for ("dark cinematic orchestral strings"), with their key and chords | as a loop track (`sample`); the song takes its key, its tempo when the genre's range allows, and its chords as progression A. A genre with no major mode plays a major phrase in its relative minor |
| texture | sixteen seconds from the middle | as a loop track (`texture`) under the song |

With earshot's models, the ears score each cut against what the genre listens for, and cuts under z 1.5 are left out
and not fetched again. The cuts go into `<samples>/prepared/`, each with its recording's credit beside it, so a song's
`credits.txt` names them. A genre keeps a few of each (four breaks, four phrases, two textures for trip-hop), and each
song draws one by its seed.

Offline, or before anything is gathered, a genre plays the layers its profile names in a sample's place (trip-hop's
drum lanes for the break, its electric piano and strings for the phrase), so it still makes a song.

## In the web demo

The [studio demo](https://claudius.devlato.com/studio/?mock) samples as well. A trip-hop song there is built on loops
from Freesound under CC0 or CC BY, found as the song is made: a phrase it takes its tempo, key and chords from, a
break, and crackle. The demo asks the website for them, and the website asks Openverse and Freesound, so your browser
contacts no one else. Each track names its recording, author and licence, and the log gives each one's full credit.
