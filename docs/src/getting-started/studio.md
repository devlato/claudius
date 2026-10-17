# The studio

```sh
claudius studio
```

opens the studio in its own window. `claudius serve` runs the same studio in your browser at
<http://127.0.0.1:7878>. Both are views of one session: you can have the window, a browser tab and an assistant
working on the same song at once, and each sees the others' changes as they happen.

Every change is rendered and judged in the background. The arrangement shows when the render you hear is older than
the song, and the score badge in the toolbar updates when the new verdict arrives, a few seconds later.

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
generations of that. Both run in the background and can be stopped. **Save** writes the song folder.

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
