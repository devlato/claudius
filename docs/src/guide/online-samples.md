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

Searches also leave out the recordings of papers' supplementary material, which a bot imports to Wikimedia Commons
under the names their journals give them (`…-pone.0062397.s008.oga`, `…_MOESM1_ESM`): a vowel recorded for a study
of voices holds a note, but it is no music. The studio demo's searches leave them out too.

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

A [scratch](phone.md) is a take of a record cut from the song, so it keeps the credits of what that record held (a
sampled phrase, a break, a sample pack's instrument) beside its file, `<take>.flac.sources.json`. The song's
`credits.txt` names them for as long as it plays the scratch, even once it no longer plays the tracks the record was
cut from.

A take or a phrase cut from a downloaded recording credits it the same way, beside its file: adding one with
`add_take` or sampling it with `sample_song` keeps its credit with what the song plays.

A claudius shared with others may read the samples folder, as it reads the sample packs
([What a shared claudius may read and write](samples.md#what-a-shared-claudius-may-read-and-write)).

## Choosing one in the studio

The studio's **Add a recording** window (the **+** after the record button) searches the same libraries: type a few
words and press **Search**, press ▶ beside a recording to hear it, and **Use it** to choose it. Then **Add it whole as
a track** adds it as a [take](samples.md#takes) to trim and put effects on, and **Sample its vocals into the song**
samples it. Either way claudius downloads it once into the samples folder, and the song credits it. Behind the window
are `GET /api/library/search?q=WORDS` and `GET /api/library/audio?id=ID` ([HTTP API](http-api.md#routes)).

Wherever a command takes a recording's path (`add_take`, `sample_song`, `analyze` and `remake`), `library:<id>` names
one a search found: claudius downloads it and uses the file. An assistant searches with `search_recordings`, which
lists each as `library:<id>`:

```text
library:freesound:629052  "Rain on a window" by Someone, CC BY 4.0, 64 s (https://freesound.org/s/629052/)
```

A claudius that may not write its samples folder (one shared with `--files` elsewhere) answers that it keeps no
recordings from the libraries.

## Songs built on samples

A song of any genre can be built on samples, as records built on samples are: a phrase from a recording that the song
is built on, a drum break, and a texture under it, and in some genres a sung note chopped for a part or a bass note
the song's bass plays on. Trip-hop, boom bap, lo-fi, jungle, big beat and filter house are
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

Gathering downloads recordings of a fitting length and cuts each on its bars: the grid of its tempo, or, for a band
that speeds up and slows down, the beats it plays, each bar of a piece then stretched to one length so that the piece
keeps one tempo:

| role | what is cut | how a song plays it |
|---|---|---|
| break | whole bars of the break, and its kick, snare and hat | as a loop track (`break`), re-chopped at times, where the genre's sections list it, else in place of the layers the profile names (trip-hop's kit, house's top loop and shaker); the drum lanes it stands in for play the break's own hits |
| phrase | the two bars the ears hear as most like what was asked for ("dark cinematic orchestral strings", or your words; without the ears, the loudest), rather two the recording plays again, which loop as it does, with their key and chords; and two bars of another passage, from another section of the recording or on other chords | as a loop track (`sample`) in place of the genre's chords or keys; the song takes its key, its tempo and its chords as progression A. The other passage plays as `sample2` in some of the sections that list the phrase, on its own chords as progression A2 (the genre's B stays the contrast it gives the sections on it): the louder of the two where the song lifts (its drops and hooks) and the other elsewhere, or every other section in a song that only lifts or never does. A genre with no major mode plays a major phrase in its relative minor. Where either passage plays, the song's lead and arpeggio step aside, and the record carries the melody |
| texture | sixteen seconds from the middle | as a loop track (`texture`) under the song |
| vocal | a note a voice holds, chopped: the steadiest and loudest of about a third of a second (with the ears, the one of the best eight most like what was asked for), with the note it was sung at | as a part (`vox`) in the drops and hooks (else the loudest sections) that plays the chop as a sampler would: moved from where it was sung to the song's chords, as a lead's motif or an arpeggio within a fifth of it, through a vocal's mix (a high-pass at 200 Hz, a little presence, compression, reverb and delay). Garage, future bass and house chop one most often; trip-hop, boom bap, lo-fi, jungle and drum & bass now and then |
| bass | a note a bass plays on its own, chopped from its attack: from a recording of a note or a few (not a song, and not one that calls itself a kick, a drum or a sound effect), the steadiest and loudest of about two thirds of a second, within 15 dB of the recording's loudest, with most of its energy under 300 Hz and little above 2 kHz (a note out of a full mix has hats and voices over it). With the ears, the one of the best eight most like what was asked for. Its note is where it sounds: the pitch tracker hears the recording twice as fast, so that an 808's C1 is found, and a note whose fundamental is weaker than its octave, as a bass guitar's low notes are, is found an octave down | on the song's own bass part, where it plays, with its notes: a sampler plays them on the note, moved from where it was played, within an octave of it. Funk, reggae and phonk sample one most often (slap bass, dub bass, distorted 808s); boom bap, lo-fi and trip-hop an upright's pluck, neo-soul and R&B a fretless, the disco genres a bass guitar, hip hop and trap an 808, now and then |

A phrase meets the genre's tempo as a producer plays a record, in one of three ways. The song's seed picks among the
ways that cost about the least, so not every song does the same:

- **Faster or slower by whole semitones**, as a sampler plays a record, its pitch moving with its tempo, until its
  tempo (or half or twice it) falls in the genre's range. A soul record at 98 BPM plays four semitones up in a house
  song at 123.47 BPM, in a key four semitones above its own, and the song takes that tempo and key, so the phrase is
  only resampled and plays in tune with everything else. How far a genre goes is in its profile (`pitch`: trip-hop
  slows a record by up to five semitones, house speeds one up by up to five).
- **Chopped onto the song's grid at its own speed**, at a tempo two thirds, three quarters, four thirds or three halves
  of its own (where you give the song's tempo, any from two thirds of it to three halves, never half or twice it, which
  would count its bars at half or double time). Each half beat of it plays from the song's half beat, so its bars are
  the song's and its chords change where the song's do, and its pitch is kept. Where the song is faster each slice is
  cut short, and where it is slower a gap follows it, as with an MPC's chops. A 120 BPM record plays at its own pitch in
  a boom bap song at 90, three quarters of its tempo; one at 80 plays in house at 120, three halves of it. Chopping
  suits a recording with a clear pulse, whose slices are its hits, in a genre that chops (`chop` in the profile: boom
  bap, grime, phonk and IDM most). A recording that flows, such as a string section, is chopped only when nothing else
  fits it, since its chops would gate it, and house speeds its records up, as house does.
- **Stretched in tune** to the genre's tempo, where claudius can stretch, when the stretch is small or nothing else
  fits.

The song's lineage says how far it went, which way, and where the other passage plays (`sampled: phrase 'Old Soul' 4
semitones up (another passage of it in hook, hook2)`, or `... chopped onto the song's grid at its own pitch (played at
120 BPM)`). Both passages play the same way. A break plays as it is within a few percent of the song's tempo, and
otherwise chopped onto the song's grid at its own pitch, as jungle's producers chop one, since a break stretched far
smears its hits. The web demo cannot stretch, so a phrase there is sped up or chopped, and a break off the song's
tempo is chopped.
Each role keeps its own colour from the profile: the high-pass that leaves the low end to the song's bass, the tape,
the 12-bit crush.

On top of that colour, a song treats its phrase with one move its genre's producers make, which the song's seed picks
(`treat` in the profile says which moves and how often). The moves:
- **filtered until it lifts**: held behind a low-pass at 1.4 kHz that opens across the build into each drop or hook,
  then out of the way there;
- **darkened**: a low-pass at 2.6 kHz all through;
- **a telephone's band in the intro and the breaks**: 450 Hz to 3.2 kHz there, as it is elsewhere;
- **overloaded** and **smashed**: the mixer's drive at 9 or 15 dB;
- **crushed**: 12 bits at half the rate;
- **as it is**.

Trip-hop darkens and filters, big beat overloads, lo-fi darkens and crushes, and filter house keeps its own sweep. A
move is made only where the phrase plays for it to be heard: a song whose phrase never plays in a drop or a hook is not
filtered until it lifts. The song's lineage names the move (`sampled: phrase 'Old Soul' 4 semitones up, filtered until
it lifts`), and the second passage plays through the same.

With earshot's models, the ears score each cut against what was asked for, and cuts under z 1.5 are left out and not
fetched again. The cuts go into `<samples>/prepared/<role>/<genre>/`, or `asked-<words>-<hash>/` for words of your own, so
that a recording gathered for two of them is cut for each, each with its recording's credit beside it, so a song's
`credits.txt` names them.

### Gathering, a few at a time

The studio and an assistant gather the way `claudius make` does. A new song built on samples whose genre (or words)
has fewer of a role than it keeps starts a job, *sampling for house*, that gathers up to two more of each, then makes
the song with them. Its log shows each recording tried. **Stop** ends the gathering, and the song is made with what
is there. So a genre's collection grows with its songs, to the number its profile keeps (eight phrases, six breaks,
three textures and four vocals for trip-hop), and each song draws one of each by its seed. A role the libraries had nothing more
of is not searched again until claudius starts again. A claudius shared with others (`--root`) and
`claudius studio --offline` gather nothing: they play what was gathered before, or what stands in for it.

Offline, or before anything is gathered, a genre plays the layers its profile names in a sample's place (trip-hop's
drum lanes for the break, its electric piano and strings for the phrase), so it still makes a song.

## In the web demo

The [studio demo](https://claudius.devlato.com/studio/?mock) samples as well, as the app does: a song of a genre
built on samples, or one asked to be, is built on recordings from Freesound, Jamendo and Wikimedia Commons under CC0 or
CC BY, found as the song is made from what the genre digs for or your words, loops and whole songs alike. claudius
itself cuts them, in the page, as the app cuts them: a phrase of two bars out of a whole song, in its key, with its
chords, played up or down by whole semitones into the genre's tempo, or chopped onto the song's grid at its own
pitch. The demo has no ears, so its phrase is the loudest two bars of a recording's harmonic sections, and it cannot
stretch, so a recording that no speed fits plays chopped. The demo asks the website for the recordings, and the website asks Openverse and the libraries, so
your browser contacts no one else; a visitor may ask for 30 new searches a minute. Each track names its recording,
and the log gives each one's full credit.

A song the demo sampled goes on in claudius: its [project](../getting-started/studio.md#carrying-on-in-claudius)
holds what the demo cut from each recording, with the recording's `.json` beside it, as a sound downloaded here has
one, so claudius credits them in the song's `credits.txt` from then on.
