# Apple Loops and your own samples

Besides synthesis and the [sample packs](packs.md), claudius plays three kinds of recordings: Apple Loops when
GarageBand or Logic content is installed, cuts of songs you give it, and any audio file a script names.

## Apple Loops

### Where claudius finds them

GarageBand and Logic keep a database of every installed loop with its tempo, length in beats, key and scale,
instrument, genre and mood. claudius reads the newest `LoopsDatabaseV*.db` in `~/Music/Audio Music Apps/Databases`
or in GarageBand's container (`~/Library/Containers/com.apple.garageband10/Data/Music/Audio Music Apps/Databases`),
read-only, and keeps the loops whose files exist. A full install has about 14,000; `claudius doctor` counts them:

```text
  ok  Apple Loops     14096 loops and samples
```

The count includes your [user library](#chopping-a-song). When there is no database, every Apple Loops feature
quietly falls back to synthesis or the packs.

### How a song uses them

**Drum one-shots.** When `make`, `song()` in a script, or `new_song` writes a drum lane, it may cut the lane's sound
from a loop instead of synthesizing it: a kick from a loop whose subtype is Kick, a hat from a Hi-hat loop, a clap
from a loop with "Clap" in its name, percussion from Conga, Bongo or Clave loops (a cowbell only where a genre
writes one). Loops tagged with a
GarageBand genre that suits the song's genre (Deep House and Tech House for house, Hip Hop for trap and lofi) are four
times as likely. The chance is `--samples`, or `samples` in `song()`; it is 0.75 unless you change it, for `make`,
scripts and `new_song` alike, so the same seed makes the same song everywhere. When the genre puts
the song's drums on a sample-pack kit, the kit wins.

To cut a hit, claudius finds the onsets in the loop and scores each hit by its level, the room before the next hit,
how much of the previous hit bleeds into it, and whether its decay suits the lane (long for a kick, short for a
closed hat). The best one is trimmed to its decay, faded, normalised to −1 dBFS and cached as a WAV in
`~/Library/Caches/claudius/samples/` (`~/.cache/claudius/samples/` on Linux), so the loop is read once. The track's
sound then reads like `hat cut from 'Silver Hi-Hat 04' (Apple Loops)`. Whether a lane uses a one-shot is drawn the same
way with or without Apple Loops, so the drum patterns of a seed are the same on every machine and only their sounds
differ.

**Loops.** House, deep house and techno have a percussion loop part called `top`. claudius picks a Drums or
Percussion loop of a suitable subtype and genre within 23 % of the song's tempo (the closer, the likelier),
warps it to the tempo, and plays it in the sections that list `top`. Tonal loops are also transposed to the song's
key by the shortest way, a minor loop to the minor tonic (the relative minor in a major key).

**Loop families.** `make --loops`, `song("house", #{ loops: true })` and `new_song` with `"loops": true` build the
whole song around a family of loops produced together: loops in one folder, at one tempo, whose names share a prefix
before the instrument word ("Bliss Abyss" in "Bliss Abyss Chord Layers 01"). The song takes the family's tempo and
key, reads the chords of its harmony loop bar by bar from their pitch content (helped by the family's bass loop), and
plays the family's chords, bass and beat loops as tracks, the way you would start a song in GarageBand. Families with
a bass loop and a genre that suits the song are favoured.

**Scripts.** `.bank("apple")` cuts drum names from Apple Loops instead of synthesizing them:
`s("bd*4, [~ cp]*2").bank("apple")`.

### Warping

A loop plays at the song's tempo and key through Signalsmith Stretch, which changes tempo and pitch in one pass.
When the tempo changes by less than 1.5 % and the key not at all, claudius resamples instead (varispeed), which adds
no artefacts at all. The loop is stretched as three copies end to end and the middle one kept, so it wraps without a
click. Warped loops are cached in `~/Library/Caches/claudius/stretched/`.

### The licence

Apple's licence for GarageBand and Logic content lets you use the loops in music you make; read Apple's software
licence agreement for the exact terms. claudius reads the loops where GarageBand or Logic installed them and never
copies them anywhere, except the one-shots it cuts, which stay in its cache. It does not redistribute them, and
nothing in the claudius repository or its releases contains Apple content.

`--no-library` (on `make`, `evolve`, `studio`, `serve` and `mcp`) keeps Apple Loops and your user library out of
the song while the sample packs still play; `--synth-only` keeps the packs out too; `--samples 0` keeps only the
one-shots out.

## Chopping a song

`claudius chop` turns a recording into material:

```sh
claudius chop ~/Music/reference.mp3 --bars 2
```

It analyses the song (tempo, downbeat, key, sections), then writes into `out/chops/<file name>/`:

- one loop per section, `--bars` bars long from the section's first downbeat (`01-intro-2bars.wav`, `02-drop-...`),
  tagged with the tempo, the length in beats, the key and the section's energy;
- a `kick.wav`, `clap.wav` and `hat.wav`, each cut from the sparsest section that still has that instrument.

Each file is added to the user library, `~/Library/Caches/claudius/user/library.json`, which claudius merges with the
Apple Loops index: from then on, the one-shots can be picked for drum lanes like Apple Loops one-shots are.

`--find "a vocal chop"` also cuts out the moments that best match a description, using earshot's search, as
`find-a-vocal-chop-1.wav` and so on (`--top` of them per description).

## Flipping a recording into a song

`claudius make --sample <file>` builds a song around a recording, the way a producer flips a sample:

```sh
claudius make --genre lofi --sample out/chops/reference/03-drop-2bars.wav
```

- The recording's tempo, length and key come from the user library when it is a chop. Otherwise claudius analyses
  it with the genre's tempo range and uses 4, 8, 12 or 16 beats of it.
- It is warped to the song's tempo and transposed to the song's key by the shortest way.
- Six times in ten (when it is at least four half-beat slices long), it is re-chopped: an eight-slice motif that
  starts each half bar on the slice of the same position and fills the rest with random slices. Otherwise it plays as
  recorded.
- It becomes a loop track named `sample1` (then `sample2` ...), with the role `sample`, ducked under the kick and
  sent to the reverb, and plays in the drops, hooks and grooves, and in some breaks.

The song's history records it as `flipped '<file>' (<tempo> BPM, <beats> beats, transposed <semitones>)`, with
`, re-chopped` at the end when it was.

To write a whole new song on another song's skeleton instead, use `claudius remake`; see
[Remaking a song from audio](../cookbook/remake.md).

## Sampling a song's vocals

The auto-sampler takes the best sung phrases of a recording (a song, an a cappella, a vocal loop) and puts them into
the open song, as a producer samples a vocal: `claudius sample song.mp3 --on SONG`, `sample_song` for an assistant,
or **Sample its vocals into the song** in the studio's New song dialog, after choosing the file.

- **Separating the voice.** With earshot's models, claudius first takes the recording apart into its vocals, drums,
  bass and the rest (Spleeter's nets, about 8 s for a four-minute song, kept for the next time), and hears the voice
  on its own from then on.
- **Finding the phrases.** claudius finds the recording's tempo, key and bars (the beats a band plays, when it speeds
  up and slows down), and scores every one- and two-bar span (`--bars` asks for 1, 2 or 4): how much of it holds a
  pitch in a voice's range, A2 to C6, and how much of its sound is the voice (with the voice apart, its share of the
  span; else how much sits in the middle of the stereo picture, where a voice is mixed). With the ears, it also listens
  to the best of them for "a person singing" (or what `--find` says, such as "a rap verse"), and their reading leads.
  It keeps the best two (`--count`, up to four), no two overlapping.
- **Cleaning them.** With the voice apart, each phrase is the voice as it was sung and mixed, in stereo, its rumble
  under 90 Hz out. Without it, each keeps the middle of the stereo picture and its sustained part, without what
  strikes (the drums, by harmonic-percussive separation), above 160 Hz and below 11 kHz. Either way, at −1 dBFS,
  faded in and out, and as long as its bars were played, each made one tempo.
- **Fitting them.** Each becomes a loop of its own, `vox1`, `vox2` ...: the phrase, then the bars of silence before
  it comes round again, warped to the song's tempo and moved to its key by the shortest way, as a flipped sample is. A change of the
  song's tempo or key moves it too. A voice is always stretched, never sped up or slowed down, and moved to the key
  with its formants (the resonances of the singer's throat) kept where they were, so that three semitones down it
  still sounds like the same singer, not a giant, and three up not a chipmunk. Any track with the `vocal` role is
  warped this way, a take you sang as well.
- **Placing them.** The phrases go where a vocal suits the song: the drops and hooks first, then the breaks, the
  verses and grooves, the builds (`--in` names the sections or kinds). In each place, a phrase plays from the
  section's first bar or answers from the bar after, whichever its notes agree with better: its pitch profile against
  the chord tones under each time it plays. It plays only in the sections with room for it (an answer needs room
  after the phrase it answers); when no section asked for has room for a phrase that long, nothing is sampled and the
  reply says so.
- **How often.** A phrase comes round as `--every` says (`every` for an assistant): `once` from the start of each of
  its sections, `2`, `4`, `8` or `16` (once every so many bars, silent between), or `through` (all through each).
  Said nothing, the song decides by its genre and by what its sections play: once in each section where a lead or a
  voice sings already, as two melodies at once say neither; else every other time round, answered, in a genre built
  on samples (those that sample by default: boom bap, lo-fi, trip-hop, jazz rap, chipmunk soul, filter house and
  others), whose producers loop what they sample; and once every 8 bars in the rest, a voice that comes back now and
  then. The reply says which, and why.
- **Mixing them.** A phrase is mixed as a vocal (−15 LUFS while it plays, a little under): high-passed at 200 Hz,
  a little presence at 3 kHz, compressed 3:1, with some reverb and delay and a light duck under the kick. A lead
  that plays with it steps back 1.5 dB, once: a lead that already plays beside a phrase sampled before stays where
  it is.

The phrases are kept with the song, in `takes/` in its folder, and go with it when it is saved elsewhere or its folder
moves, as takes do. The reply says what came of it in its first line (which the studio's toast shows, and undo
names), then where each phrase came from and went:

```text
sampled 2 phrases of Many Miracles Vocal Melody.caf into the song: vox1, vox2
vox1: bar 6 of Many Miracles Vocal Melody.caf (the ears +2.0 for 'a person singing', 60% sung), in its own key at 122 BPM, once in each of the drops and hooks (drop), from the first bar, as a lead or a voice sings there already, on chords it agrees with 74%
vox2: bars 11-12 of Many Miracles Vocal Melody.caf (the ears +1.9 for 'a person singing', 54% sung), in its own key at 122 BPM, every 8 bars from the first bar in the breaks (break), now and then, as house is not built on samples, on chords it agrees with 83%
```

### Other ways to sample

Phrases that come round are one way producers sample a recording. The auto-sampler knows three more, which
`--how` asks for (`how` for an assistant); phrases are the way when none is said:

- **A passage** (`--how passage`): a whole stretch of the recording, 4, 8 or 16 bars of its best sung part (`--bars`;
  8 where a section has room for them, else 4), the better where one of its own sections starts, played through as
  the verse or chorus of the sections it goes in: the drops and hooks, unless `--in` says. It is the voice alone when
  claudius can take it apart, else the record as it plays with its lows out, so that the song's bass and kick play
  under it. The song's chords in those sections follow the passage's, moved into the song's key as the passage is,
  when the recording plays chords there and the analysis heard the chord of every bar of it; the reply names them.
  An a cappella plays none (with its voice apart, what is left under it is all but silent), and the song keeps its
  own. A trance record is built on a vocal this way, and a hip-hop beat on a soul record's chorus.
- **Chops** (`--how chops`): the best sung phrase (`--bars` 1, 2 or 4; 2 when none is said) cut in eighths of its
  beats, and the six slices that hold the most voice played in a new syncopated rhythm, two bars round, the second
  moving the motif on and ending in a stutter. It is a track of its own, `chops`, in the drops and hooks: the voice as
  an instrument.
- **A stab** (`--how stabs`): one short hit of it, the sharpest syllable of its best phrases (80 to 350 ms), on a
  drum lane of its own, `vox_stab`, struck on a few beats of every two bars and moved to the song's key, in the drops
  and hooks and the verses. Its rhythm is a drum lane's: change its steps in the Drums tab, or with
  `set_drum_pattern`.

```text
sampled a passage of Many Miracles Vocal Melody.caf into the song: passage
passage: bars 1-8 of Many Miracles Vocal Melody.caf, its voice (the ears +2.7 for 'a person singing', 35% sung), 5 semitones up into F minor at 126 BPM, played through from the first bar of the drops and hooks (drop, drop2), the song's chords there following it (i i7 III iv7 i7 v7 i i7)
```

In the studio, the **Add a recording** window offers each way as a card under **How should it go in?**: **Whole**,
**Its sung phrases**, **Pieces of it** (every kind of piece of it, below), **A passage**, **Chops** and **A stab**.
**Plays in** chooses the song's sections it goes in
(none chosen: where it suits), and the button at the foot (**Sample a passage**, **Chop it**, **Sample a stab**) puts
it in. In the New song dialog, **Sample it into the song** takes a way too.

### What the window hears first

As soon as the window holds a recording, claudius listens to it, as `analyze` does, and the window shows what it
heard before anything is asked: its length, tempo and key, whether a voice sings in it, its sections over its
waveform, and each phrase its voice sings, marked where it is. Click a phrase to hear it. A recording chosen from a
free library is fetched and heard the same way.

### Choosing the pieces first

With **Its sung phrases** chosen, **Choose its phrases →** opens the **Sample** window instead of sampling straight
away: up to four pieces the auto-sampler found, each a phrase sung whole, numbered as they come in the recording and
marked on its timeline, to hear as it was cut (cleaned of the recording's drums) and through a treatment, side by side.
A piece is cut in the quiet before the phrase's first word and after its last note has died away, never inside a word,
and takes the whole bars of the recording the phrase is sung in, silent outside it. The treatments are a producer's:
dusty tape over vinyl crackle, an echo into a long hall, lo-fi crushing, a slow phaser over a chorus, a telephone's
band, a stutter on the last beat, a reversed tail. Press **▶ as cut** or **▶ treated** to hear a piece (**…** while it
is on its way), tick the pieces you want (on the list or on the timeline), and choose **as cut** or its treatment for
each; the two most sung start ticked, as they were cut. **Each time** says how often a phrase comes round in its
sections: **As the song suits** (the default, as above), **Once, as it starts**, **Every 4 bars**, **Every 8 bars** or
**All through it**. **Put 2 phrases in the song** puts the ticked ones in, in the sections chosen in **Plays in**, a
treated one through its treatment, and closes the windows; **← back** returns to the ways, and **Cancel** closes it
with nothing added. **Other phrases** draws other phrases among the good ones, with other treatments. When a piece
cannot play, the window says why.

An assistant does the same with `sample_pieces` (the recording's `path`, or `library:<id>`; `seed`, `count`, `bars`,
`find`, `kinds`), which lists the pieces, where each is in the recording's form ("in hook 2") and where to hear it,
`redraw_pieces` for another take, and `keep_pieces` with the pieces to keep, whether treated, and how often they come
round (`every`, as above).

With **Pieces of it** chosen, **Choose its pieces →** opens the **Sample** window with every kind of piece the
recording holds: its sung phrases, then, under **Its drums, bass and loops**, the best piece of each other kind and
more of a kind where they differ (below), each saying what it is and why it scored as it did ("Drum break · Intro,
bars 1-2", "the drums alone, busy, it loops cleanly"). None of those starts ticked; tick the ones you want, as for a
phrase. When it has no phrase to offer (no voice sings in it, say), the window says why above the other pieces.
**Another take** draws other pieces of each kind among the good ones, other than those it offered last. Besides its phrases, `sample_pieces` offers the same, as producers sample records
([design 0021](../design/0021-every-kind-of-sample.md)), each cut on the recording's bars:

| Kind | What it is | Kept, it plays |
|---|---|---|
| `hook` | a loop of 1 to 8 bars of its hook, the section with chords that pulls most toward being one, without its drums, bass and voice | all through the drops and hooks, in the song's key |
| `passage` | a loop of 2 to 8 bars of another passage with chords | all through the other sections but the intro and the outro |
| `break` | its drums, 1, 2 or 4 bars | as a drum loop where the song's drums play, at its own pitch, cut onto the song's grid when its tempo is far from the song's |
| `bass` | its bass line, 1, 2 or 4 bars | in place of the song's bass, in the song's key |

Each is scored as its kind: how alone what it is cut from plays in its bars, how well it loops (its first beat against
the beat after its end), and what its kind asks besides (how busy a break is, how much a bass line moves, how often a
hook comes back, how unlike the hook another passage is). Each says why, in words ("the drums alone, busy, it loops
cleanly"), and goes into the sections `keep_pieces` names, when it names any; a phrase kept with no room in them leaves
the other pieces kept, and the reply says why it did not go in. `kinds` asks for some only (`phrase`, `hook`,
`passage`, `break`, `bass`); a recording no voice sings in, or none of whose phrases fits the bars asked, offers the
rest and says why there is no phrase. They come from the recording's stems when earshot separates them; without its
models, a break and the loops come from the mix's percussive and harmonic parts, and a bass line, which the mix does
not give apart, is not offered.

What is offered is varied as well as good. Besides the best of each kind come up to two more of the kinds producers
sample most (of 42,447 samples on WhoSampled, 37% are drum loops and 34% bass lines: Bryan & Wang, ISMIR 2011), and of
those the song's genre samples (a house song leans toward breaks and hooks), each only where it is good and unlike
those already offered: a recording's sparse groove and its busy one, never two cuts of one groove. Asked for one kind,
it offers up to four of it. Another take draws other pieces than the take before, where there are any. Several pieces
of a kind kept with no sections named share the sections their kind plays in, the fuller in the louder, so that none
plays over another.

Behind the window are `POST /api/sample/pieces` (the recording itself), the state's
`pieces`, each with where its phrase is sung (`from`, `to`) and the section it is in, and `GET /api/pieces/<file>` for
each piece to hear ([HTTP API](http-api.md#routes)). What `analyze` hears in a recording says where its voice sings
each phrase too (`phrases`). The online demo offers pieces the same way, in the page.

Without earshot's models, it goes by the pitch and the stereo picture alone: in a full mix, a lead synth held in the
middle can then pass for a voice. Sample only what you may use: a recording of someone else's song needs their
permission before you release what you made with it.

### Chopping a piece onto pads

A break, a loop or a phrase kept chopped (`keep_pieces` with `"chopped": true`) is cut at its onsets, as producers
chop a record onto a sampler's pads. SuperFlux hears the onsets (Böck & Widmer, DAFx 2013): how much each of 24 bands
an octave rises over the same bands a moment before, which the wandering of a vibrato does not set off. The hits that
sound as one share a pad (its kicks, its snares, its hats), eight pads at most, and each pad is a drum lane of its own
(`chop`, `chop2` ...) that strikes it where its hits came and as hard, straight and with none of the song's groove, so
that the lanes play the piece as it was. Each hit plays at its own speed, so a break played at another tempo keeps
every hit whole, as no loop stretched to fit it does. Then play them otherwise, in the Drums tab or [from a
phone](phone.md). A phrase chopped plays in the drops and hooks, as chops of a voice do, and a piece with too few hits
to chop says so.

## Takes

A take is a recording made for the song: a vocal, a guitar, a melody sung into a microphone. A loop starts over in
every section it is in; a take plays once, from the start of the section it was recorded over, and on through the
sections after it for as long as it lasts. Record one with the studio's [Record](../getting-started/studio.md#the-panels)
button. When you stop, the take waits for you: **Play it back** plays the song from a bar before it with the take in it,
**Record again** records it over, **Keep it** adds it to the song, and **Throw it away** forgets it. Or bring an audio
file: drop it on the studio, or press the **+** button after Record and choose it (or [search the free
libraries](online-samples.md#choosing-one-in-the-studio) there for one), and **Add it whole as a track** in the window
that opens adds it as a take from the section you pick (**Sample its vocals...** offers its sung phrases to
choose from instead, as [above](#choosing-the-pieces-first) describes). A whole song works too, to trim and put effects on as you
like. You can also send the file to the [HTTP API](http-api.md#routes), or give it to an assistant, which adds it with
`add_take`:

```sh
curl -X POST 'http://127.0.0.1:7878/api/takes?from=drop&name=vox&role=vocal' \
  -H 'Content-Type: audio/wav' --data-binary @vocal.wav
```

- **The mix.** The take is mixed like any track, at the loudness of its role: `vocal` (-15 LUFS while it plays),
  `lead` (-18, the default), `chords`, `keys`, `bass`, `drums` or `texture`. The critic hears it with the rest.
- **Where it is kept.** claudius keeps each take as 24-bit FLAC, named after its sound, in `takes/` in the song's
  folder, or in `~/Music/claudius/takes` while the song has no folder yet. A save copies the song's takes into the
  folder it writes, so a song folder carries its recordings, and a song folder that moved or came from another
  computer finds them in its own `takes/`.
- **Tempo and key.** When the song's tempo changes, the take is [warped](#warping) to it, between silences rather than
  wrapped around as a loop is. It does not follow a change of key.
- **The arrangement.** The take is a track of kind `take`, on in the sections it reaches. `toggle_layer` turns it off
  in a section, which silences that stretch of it. In a section before the one it starts with, it plays nothing.
- **Trimming it.** **Trim...** in its track's menu in the arrangement (right-click its name) keeps part of the
  recording, from a start to an end in seconds, so that it starts later in itself or stops sooner (`trim_take`, with
  `track`, `start` and `end`). It still plays from the start of its section, and the sections past its new end drop
  it. claudius keeps the cut beside the recording it came from, so undo brings the take back whole. Effects go on it in
  the [mixer](../getting-started/studio.md#the-panels), as on any track.
- **Releasing it.** [Publishing](publish.md) lists takes with your own recordings: release them only if you made them or
  have cleared their rights.

### A take an assistant asks for

An assistant can ask you to record something for the song ("sing the hook over the chorus"), with `request_take`. The
studio's Record window opens, set up for it: the section the take starts with, what it is, its name, and a line
saying what the assistant asked for. When no studio is open, the assistant gives you a link that opens one, such as
`http://127.0.0.1:7878/?record=1`.

Record the take, play it back, and record it again as often as you like. **Send it** adds it to the song, and the
assistant, which has been waiting for it with `wait_for_take`, hears what claudius made of it and goes on from there:
it sets the take's level, or turns it into notes. **Not now** tells the assistant you will not record it. Notes played
on a MIDI keyboard answer the request as soon as you stop.

### Turning a take into notes

A melody sung, hummed, whistled or played one note at a time can become notes, which any instrument then plays: in the
studio, right-click the take's row in the arrangement and choose **Turn the melody into notes**; over MCP,
`transcribe_take`. claudius reads the notes the take means, as [Your voice](voice.md#how-claudius-reads-it) says,
and puts them in the song's key (unless `chromatic`) and on its sixteenth-note grid. They become a new part named `melody`,
a lead unless told otherwise, on in the sections the take plays in, and the take goes quiet. Several sounds make one
part each, so a set of instruments plays the same line: `"sounds": ["piano", "strings"]`. The parts are ordinary parts
afterwards, whose sound, level and sections you change as you would any other's.

- **One note at a time.** Chords, a strummed guitar or a full mix are heard as no melody, or a confused one.
- **The notes follow the sections.** Each section the take covers keeps the stretch of melody sung over it, by its
  progression. Another section with that progression plays the same notes, and a section the part is turned on in
  later, which the take never covered, has the part write its own.
- **A style.** `style` turns the line into something else, in the song's key, and gives the part the role that goes
  with it unless you give one:

  | style | what plays | the part |
  |---|---|---|
  | `line` (the default) | the line as sung | a lead |
  | `harmony` | the line, and a third above each note | a lead |
  | `octaves` | the line, doubled an octave down | a lead |
  | `chords` | a triad under each note, the note on top | chords |
  | `bass` | the line, moved by octaves into a bass's register | a bass |
  | `arp` | each note broken into a rising arpeggio of sixteenths | an arp |
  | `pad` | a chord held through each bar the line sounds in, on the note heard as the bar begins | a pad |

  In the studio, the take's menu lets you pick one before the sound.

### Turning a take into drums

A beat, played on a kit or beatboxed into the microphone, can become the song's drums. In the studio, right-click the
take's row in the arrangement, choose **Turn it into drums** and a length (1, 2 or 4 bars); over MCP,
`transcribe_drums`. claudius hears each hit, names it a kick, a snare or a hat, and sets it on the song's sixteenth
grid from the take's first beat. It folds every time round into a pattern of that length, writes it into the song's
kick, snare (or clap) and hat lanes as their `main` patterns, and the take goes quiet. A step you hit only some of
the times round plays with that chance, as a [trig](../language/song-functions.md#trig). The lanes are ordinary lanes
afterwards: change a step in the step sequencer, give it a trig, or put the lane in a groove.

- **How it hears.** A kick adds most of what it adds under 150 Hz, a hat over 5 kHz, and a snare, a clap or a
  beatboxer's "k" in between. claudius finds those three among the take's own hits, so a mouth's kick and a kit's
  both sort as kicks. A hat struck with a kick is both.
- **What to check.** It hears a kit or a clear beatbox well, and gets some hits wrong: the release of a beatboxed snare
  can come out as a kick a step later, and a snare layered with a sub as a kick. The step sequencer shows what it
  wrote; right-click a step to change it.
- **A recording of drums.** A drum loop or a break goes in the same way, as `transcribe_drums` with a `path`, or
  [`drums_from()`](../language/song-functions.md#drums_from) in a script. Its tempo is the one given, or one in its file's
  name (`92bpm`), or heard, and the grid starts at its own downbeat.

### Notes played on a keyboard

What you play on a MIDI keyboard in the studio's [Record](../getting-started/studio.md#the-panels) window, chords and
all, becomes a new part named `played`, which the instrument or synth you choose plays. The notes go onto the grid
you choose, or stay as played. Over the [HTTP API](http-api.md#commands) and MCP, `add_notes` does the same with
notes given one by one: `t` and `d` in beats from the start of section `from`, `p` a MIDI pitch (60 is middle C) and
`v` a velocity from 0 to 1. Like a take's melody, the notes go into the sections they reach, by each section's
progression, and a `style` makes them harmony, octaves, chords, a bass line, an arpeggio or a pad, as above.

### Drums played on pads

Drum pads on MIDI (a pad controller, a drum app sending MIDI from a phone or tablet) play the song's drums: in the
Record window, choose **drums** for what a MIDI take is. Each note is a drum of the General MIDI drum map, which pads
and drum apps send (36 the kick, 38 the snare, 39 the clap, 42 the closed hat, 46 the open hat, 37 a rim), played on
the song's lane of that drum, or of one that stands in for it (a clap for a snare, a closed hat for an open one). The
take becomes those lanes' `main` patterns, folded as [a beat played or beatboxed](#turning-a-take-into-drums) is, and
the window says what had no lane in the song. Over the [HTTP API](http-api.md#commands) and MCP, `add_hits` does the
same with hits given one by one: the lane struck, `t` in beats from the start of section `from`, and `v`. A phone
plays the song's pads, keys and turntable from [the phone's page](phone.md) too.

## Files in scripts

A script plays any audio file through [`.s()`](../language/sounds.md#your-own-files) on a `note()`, `n()` or
`chord()` pattern:

```js no-run
track("vox", note("c4 ~ c4 [c4 c4]").s("/Users/me/Samples/vox.wav"));
```

- The file plays like a sampler: `c4` at its recorded pitch, other notes faster or slower.
- On a track whose role is not a drum role, each note stops at its length plus 50 ms; drum roles play the whole file.
- A name counts as a file when it contains `/` or ends in `.wav`, `.mp3`, `.aif`, `.aiff`, `.flac`, `.caf` or
  `.m4a`. It cannot go inside a mini-notation string, where `/` means "slower".
- claudius decodes WAV, AIFF, FLAC, MP3, AAC and ALAC (M4A), CAF and Ogg Vorbis itself, and asks ffmpeg for anything
  else when it is installed.

The song stores the file's path, not its audio, so moving or deleting the file breaks the song. Every decoded file is
cached as raw 48 kHz audio in `~/Library/Caches/claudius/decoded/` (about 23 MB per minute of stereo), keyed by its
path, size and modification time.

## What a shared claudius may read and write

A claudius only you can reach reads and writes wherever you ask. A claudius others can reach checks every path it is
given against a list of folders:

| how it runs | the folders |
|---|---|
| `claudius serve --public`, `claudius serve --host` with an address other than loopback, or `claudius serve --files <dir>` | `~/Music/claudius`, claudius' cache folder, the sample packs folder, the [online samples](online-samples.md) folder, `/Library/Audio/Apple Loops`, `~/Library/Audio/Apple Loops`, plus every `--files` folder |
| anything else (`studio`, `serve`, `mcp`) | no limit |

The checks:

- **Reading.** A file named in `.s()`, opened with `load` / `open`, or given to `analyze`, `remake`, `add_take` or `sample_song` must exist and,
  after links and `..` are resolved, lie inside one of the folders. Opening a `blueprint.json` also checks every
  audio file the song plays.
- **Writing.** `save` may create a folder when its nearest existing parent lies inside one of the folders and the
  rest of the path is plain names, so `<allowed>/../escape` is refused.

Refusals name the path. A path outside the folders gets the same answer whether or not it exists, so a shared
claudius does not tell anyone which files you have; `does not exist` is said only of a path inside them. The last
line comes from a claudius whose sample packs folder was `/tmp/claudius-packs-demo` (`CLAUDIUS_PACKS`):

```text
/etc/hosts is outside the folders this claudius may read
/tmp/cl-elsewhere is outside the folders this claudius may write
track 'v': /etc/hosts is outside the folders this claudius may read
track 'v': /tmp/claudius-packs-demo/nope.wav does not exist
```

A folder that does not exist yet is not on the list: until something creates `~/Music/claudius`, a shared claudius
refuses to save there ([Troubleshooting](troubleshooting.md#a-shared-claudius-refuses-to-save)). Sample-pack
instruments and Apple Loops are found by name, not by path, so they play in every mode. The [security
model](../engineering/security.md#files) explains the policy and its limits.
