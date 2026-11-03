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
