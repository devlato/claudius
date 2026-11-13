# 0020 Changing a song's genre

Status: accepted

## Context

People make a song, like it, and want to hear it as something else: their house track as lofi, a pop song as drum and
bass, a sketch in a fusion. Until now that meant a new song in the other genre, which keeps nothing, or a remake of the
render, which listens to the audio and keeps all of it. Neither is the same song in another style.

What makes a song itself, and what makes it a genre, are mostly different things. The form, the chords and the key,
the melody, a vocal take or a sampled phrase are the song; the drums, the bass line's style, the chords' rhythm and
voicing, the sounds, the swing and the mix are the genre. A cover in another style keeps the first and remakes the
second.

## Decision

`claudius::regenre::change` builds the song again in the new genre (`compose::generate`, at the song's key, unsampled so
that no phrase of a library brings other chords) and carries the song over:

- **Kept as they are:** the title, the form (each section's name, kind, length, energy, progression and transitions,
  and its drum variant where the genre's drums have it), the harmony, the key, the recordings it plays (takes, loops,
  sampled phrases), its pattern tracks and grooves.
- **Kept on the genre's parts:** the notes written out on a part go to the genre's part of the same role, and the lead's
  melody, as the theory engine writes it for the old song, is written out on the genre's lead, note for note. A part the
  genre has no part of its role for stays as it was.
- **The genre's:** the drums, bass, chords, pads and the rest, their sounds, swing, mix and transitions' sounds.
- **Where each plays:** in each section the genre's tracks of the kinds that played there before, a drum lane by what it
  does in a kit (kick, backbeat, hats, percussion), a part by its role; tracks of kinds the song never had play where a
  section's energy calls for them, as a song like a recording places them.
- **Tempo:** the song's when recordings play in it, which keep time with it; otherwise the nearest of the song's, its
  half and its double that the genre plays, as a remake chooses. At half its tempo or less, two of its bars become one
  (the sections' bars, the bars each chord holds and its notes' beats halved), so that it keeps its length and its
  pace: a 4-minute house song at 118 BPM is a 3:25 lofi song at 70, not a 7-minute one. A chord held a bar cannot hold
  half of one, so there each section plays its chords half as many times. At twice its tempo or more, each bar becomes
  two.

It is one edit (`Command::ChangeGenre`), so undo brings the song back; the reply and the history say what was kept.
The studio (the genre under the title, and Variations), MCP (`change_genre`), the HTTP API and the command line
(`claudius change-genre`) all reach it.

## Rejected

- **A remake of the render** (`remake` on the song's own audio): it keeps the old genre's drums and sounds in the
  recording it plays, and loses every editable note.
- **Swapping sounds only** (`swap_sound`): the drums' patterns, the bass style and the mix stay the old genre's, so it
  sounds like the old song on other instruments rather than the song in another genre.
- **Keeping every part's notes:** the chords' rhythm and the bass line are much of what a genre is; keeping them would
  only change the sounds. Only what someone wrote, and the melody, are kept.
