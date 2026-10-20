# The studio

```sh
claudius studio
```

opens the studio in its own window. `claudius serve` runs the same studio in your browser at
<http://127.0.0.1:7878>. Both are views of one session: you can have the window, a browser tab and an assistant
working on the same song at once, and each sees the others' changes as they happen.

Every change is rendered and judged in the background. The arrangement shows when the render you hear is older than
the song, and the score badge in the toolbar updates when the new verdict arrives, a few seconds later.

## The first time

The first time it opens, a short tour points at the parts of the studio in five cards: making a song, listening, the
verdict, the editors, and connecting an assistant, with the Connect buttons right in the last card. The app then asks
once whether to send anonymous usage statistics, which stay off unless you say yes ([what they are](../guide/privacy.md)).
The tour never blocks the studio. **Skip the tour** or Esc ends it for good, and **?** in the bottom right corner,
**Take the tour**, starts it again.

The first time it starts, claudius downloads its listening models (about 0.8 GB) in the background; the verdict
judges with meters and music theory until they are ready, and the log says when they are. If the default sample
packs are not installed, the studio says so and offers **Download the packs** (a 3.1 GB download): until then, the
sampled instruments are synthesized. The **Sounds** tab lists every pack, with a button for each one that is not
installed.

## Starting a song

**New song** asks for a genre and, if you like, a seed, tempo and key. The same genre and seed always give the same
song, so a seed is a way to come back to one you liked. **Open** loads a song folder you saved or a `.rhai` script.

## The panels

- **Arrangement.** The sections of the song from left to right (intro, build, drop and so on), and below them one row
  per track. Click a cell, or drag across cells, to switch a track on or off in those sections. Drag a section to
  move it; right-click it to duplicate, insert or remove one. The waveform of the latest render is underneath; click
  it to jump.
- **Section.** The selected section's length, energy, drum variant, which chord progression it plays, and the
  switches for a build, a gap before the next section, an impact, drum fills, and a filter that opens over the
  section.
- **Steps.** A step sequencer for every drum track. Click a step to toggle it, drag to paint, shift-drag to set how
  hard a hit is.
- **Parts.** The bass, chords, pad, lead and arpeggio parts: their writing style, their sound (a synth or a sampled
  instrument), their range and how busy they are. **Reseed** rewrites a part.
- **Mixer.** Level, pan, reverb, delay and how much each track ducks under the kick; mute and solo; meters from the
  last render; the master's loudness and true peak.
- **Harmony.** The chord progressions, as chords in the song's key and as numerals, and which sections play which.
- **Script.** The whole song as a script. Edit it and press Cmd+Enter (Ctrl+Enter on Windows and Linux) to run it.
  The sound browser lists every drum, synth and instrument name.
- **Verdict.** The score and its parts, the issues with what to change, and what earshot hears.
- **Log.** What happened, and the song's history.

**Mutate** tries a few variations and keeps the best one if it beats the current song; **Evolve** runs several
generations of that. Both run in the background and can be stopped. **Save** writes the song folder, and the
**export** menu downloads the song as WAV, AIFF, FLAC, Apple Lossless, MP3, AAC, Opus or Ogg Vorbis
([Exporting](../guide/export.md)). **publish** checks the song as stores do and writes what a distributor takes
([Publishing](../guide/publish.md)).

## Help and assistants

**?** in the bottom right corner opens the help menu: **Take the tour**, **Connect an assistant**, **Check for
updates** and **Update automatically** and the switch for anonymous usage statistics (in the app), the documentation,
and a form to report a problem. When a new release has been downloaded, a bar under the toolbar offers **Restart to
update** ([Updates](../guide/updates.md)). Connect an assistant lists Claude Desktop, Claude Code, Cursor and Codex, says which are connected,
and connects one with a click, the way `claudius connect` does ([Working with an assistant](assistants.md)). The
assistant then makes and edits the song open here, and you hear every change.

## Keys

| key | |
|---|---|
| Space | play or pause |
| Home | back to the start |
| L | loop the song or the selected section |
| ← → | select the previous or next section |
| Cmd+Z, Cmd+Shift+Z | undo, redo (Ctrl on Windows and Linux) |
| Cmd+Enter | run the script, in the script editor |

## When something does not work

`claudius doctor` checks the installation. If the studio window opens but stays empty, the engine may not have
started: `claudius serve` prints why. [Troubleshooting](../guide/troubleshooting.md) has more.
