---
name: claudius-remix
description: >-
  Works from existing audio with claudius: analyzes a song's tempo, key, chords, sections and groove, remakes a
  track's skeleton as a new song in another genre, chops recordings into bar loops and drum one-shots, and plays
  recordings as samples in a song. Use it when someone shares an audio file or a reference track, asks for a song
  like one they love or a version of a song in another style, wants a song's BPM, key or chords, or wants to sample,
  chop or flip a recording with claudius.
---

# Remixing, remaking and sampling with claudius

claudius can learn from a recording in three ways, which differ in how much of the original ends up in the result:

| | what it takes from the recording | the original's audio in the result |
|---|---|---|
| **analyze** | tempo, key, chords, sections, drum habits, bass style, as a report | none |
| **remake** | that skeleton, rebuilt as a new song with a genre's sounds and production | none |
| **chop and flip** | loops and one-shots cut from it, played in a song | yes |

Choose by what the person wants. "Something like this song" is a remake. "Use this vocal" or "sample this break" is a
flip.

## Before you start

- **The file has to be on the computer that runs claudius.** Ask for its path. A claudius shared over a network
  only reads its own folders (`~/Music/claudius`, its caches, the sound libraries, and folders given with `--files`),
  so a file elsewhere needs moving there first.
- **Rights.** A remake uses none of the original's audio, but a chop or a flip does: publishing a song with a flipped
  sample needs the right to use that recording. Say so when the person flips something they did not make. Apple Loops
  may be used in music the person makes, under Apple's licence.

## Analyze

`analyze` with the path (MCP), or `claudius analyze song.mp3 [--genre house] [--json]`, reports the tempo with its
pulse clarity, the key with a confidence, the swing, progressions `A` and `B` as Roman numerals, the sections with
their energy and what plays in them, the kick, snare and hat patterns as step strings, and the bass style.

Analysis is best on steady electronic music with a clear kick, and approximate everywhere else. Pulse clarity (0 to
1) says how clearly a steady beat stands out; below about 0.3 the tempo is closer to a guess. Its usual mistakes:

- half or double the tempo: `--genre` settles which of the two it is, by the genre's tempo range; it does not pull a
  tempo that is simply outside the range into it;
- the relative or a neighbouring key: Bb major instead of D minor, say;
- major where the music is minor on the same tonic, or the other way round. The numerals then look borrowed (F major
  with `i7` and `bIII`): the chords are right and the key name is not. A lead the critic finds outside the key while
  its strong-beat notes are chord tones points the same way;
- a progression rotated by a bar, or with a seventh missing;
- sections merged when their energy is close, even an intro that is clearly quieter.

Report it with that in mind ("about 118 BPM, probably D minor"). When the person knows the real tempo or key, theirs
wins: `key("F minor")` on the remake keeps every chord as it sounds (a borrowed `bIII` becomes `III`) and rewrites the
generated parts in the new key.

## Remake

`remake` with `path`, `genre` and `seed` (MCP), or `claudius remake song.mp3 --genre lofi [--seed 3]`, composes a
new song at the analysed tempo and key, with its progressions, swing, form, section energies and drum habits, in the
genre's sounds and production. The song's history starts with `remade from <file>: ...`.

The remake copies what the analysis read, mistakes included, so check it before polishing anything: print it (`echo
| claudius script - --on SONG --print`, or `get_song`) and compare it with the genre. A short clip, or sections the
analysis merged, gives a form of one or two sections: give it the genre's shape with `form()` (`song("<genre>")`
shows a typical one). A noisy drum read gives drum lanes no producer in that genre would play: rewrite them with
`drum()`, such as a four-on-the-floor kick for house. Fix the key, then the form, then the drums, and only then the
mix.

From then on the remake is an ordinary song: listen, read the verdict, edit it with scripts, save it (the
claudius-songs skill covers that loop). A different seed gives another take on the same skeleton, and another genre
gives the same song in a new style.

## Chop

On the command line only:

```sh
claudius chop ~/Music/reference.mp3 --bars 2
claudius chop ~/Music/reference.mp3 --find "a vocal chop" --find "a snare hit" --top 3
```

It writes into `out/chops/<file name>/` (or `--out`): one loop per section, `--bars` bars long from its first
downbeat (`02-drop-2bars.wav`), and a `kick.wav`, `clap.wav` and `hat.wav`, each cut from the sparsest section that
still has that drum. Every file is added to the user library, so later songs can pick the one-shots for their drum
lanes. `--find` uses the listening models to cut out the moments that best match a description.

## Flip a recording into a song

- **A new song around it:** `claudius make --genre lofi --sample out/chops/reference/03-drop-2bars.wav`. The
  recording is warped to the song's tempo, transposed to its key, often re-chopped into an eight-slice motif, and
  played in the drops, hooks and grooves as a track called `sample1`.
- **In a script, over MCP too:** play the file as a sampler on a note pattern. `c4` plays it at its own pitch:

```js no-run
track("vox", note("c4 ~ ~ ~ c4 ~ c4 ~").s("/Users/me/Music/claudius/chops/reference/find-a-vocal-chop-1.wav")
             .room(0.3).level(-4));
```

  The path goes in `.s()`, never inside the pattern string. The song stores the path, not the audio, so the file has
  to stay where it is.
- **Apple Loops one-shots** on the computers that have GarageBand or Logic content: `.bank("apple")` on drum words,
  as in `s("bd*4, [~ cp]*2").bank("apple")`.

After a flip, listen and read the verdict like after any change: a sample often needs its level, a little reverb, or
`.hpf()` to leave room for the bass.
