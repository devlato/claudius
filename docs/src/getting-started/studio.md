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
song, so a seed is a way to come back to one you liked. **Fused with** fuses a second genre into it: the first leads
the song, and the second plays the drums and lends its sounds ([Fusions](../cookbook/fusions.md)). It can also start from a recording: **Choose a file** under
"Or start from an audio file", then **Analyze** shows what claudius hears in it, and **Remake** writes a new song on
its tempo, key, chords, form and groove. **Sample its vocals into the song** puts its best sung phrases into the song
that is open ([Sampling a song's vocals](../guide/samples.md#sampling-a-songs-vocals)), and **Take its groove** puts
the open song's drum lanes in the feel of the recording's drums ([Grooves](../cookbook/drum-machine.md#a-groove-from-a-record)). **Open** opens a song you saved: choose the `blueprint.json` in its folder,
or a `.rhai` script. The studio reads the file on its own computer, in a browser too, and nothing of it is kept but
the song it opens.

## The panels

- **Arrangement.** The sections of the song from left to right (intro, build, drop and so on), and below them one row
  per track. Click a cell, or drag across cells, to switch a track on or off in those sections. Drag a section to
  move it; right-click it to duplicate, insert or remove one. **+ Track** adds a track: a part (a bass, chords, a
  pad, keys, a lead or an arpeggio the theory engine writes over the song's chords, written the way the genre writes
  that role) or a drum lane to fill in under Steps, its name, and its sound, from the synths made for the role, the
  installed instruments, or the drum voices and kits; a new track plays in the sections with energy 0.6 or more,
  ready for you to move. Right-click a row to mute or solo it, switch it on or off everywhere, or remove it (Undo
  brings it back). Right-click a [take](../guide/samples.md#takes)'s row
  to hear [how it is sung](../guide/voice.md) (**How is it sung?**), to [tune it](../guide/voice.md#tuning-a-voice)
  (**Tune it**: naturally, tightly, hard, or not at all), to turn a beat you played or beatboxed into the song's
  drum lanes ([**Turn it into drums**](../guide/samples.md#turning-a-take-into-drums)), or to turn the melody you
  sang into notes an instrument plays, as you sang it or as harmony, octaves, chords, a bass line, an arpeggio or a
  pad. The waveform of the latest render is underneath; click it to jump.
- **Section.** The selected section's length, energy, drum variant, which chord progression it plays, and the
  switches for a build, a gap before the next section, an impact, drum fills, and a filter that opens over the
  section.
- **Steps.** A step sequencer for every drum track. Click a step to toggle it, drag to paint, shift-drag to set how
  hard a hit is. Right-click a step and open **Trig** to set the chance it plays, when it plays (`1:2`, `last` ...),
  a ratchet of strokes inside it and how they swell or fade, a flam, a nudge off the grid, and its pitch, decay, tone
  and pan for that stroke ([The drum machine](../cookbook/drum-machine.md)). A step with a trig shows it: its chance
  as a number, its condition, a ratchet's strokes, a dot for a flam, a diamond for the sound it changes, and a nudged
  step sits off the grid. Right-click a lane for its own time and feel: a length of its own (steps past it are faint,
  showing the steps that come round again there), its speed, a swing of its own or the song's, and a groove and how
  much of it. The lane's name shows what it has.
- **Parts.** The bass, chords, pad, lead and arpeggio parts: their writing style, their sound (a synth or a sampled
  instrument), their range and how busy they are. **Reseed** rewrites a part.
- **Piano roll.** A part's notes over one of its progressions, on a grid of beats and pitches. Click an empty place
  to add a note, drag a note to move it, drag its right edge to make it longer or shorter, right-click it to take it
  out; the grid (1/16 to 1/2) is what notes snap to. The notes the theory engine writes are shown faint, and the
  first change writes them out for you to edit; **Back to the theory engine** lets it write them again. The notes
  come round every time the part plays that progression.
- **Mixer.** Level, pan, reverb, delay, how much each track ducks under the kick, and its drive, which overloads it
  (rougher at the same loudness); mute and solo; meters from the last render; the master's loudness and true peak.
- **Harmony.** The chord progressions, as chords in the song's key and as numerals, and which sections play which.
- **Script.** The whole song as a script. Edit it and press Cmd+Enter (Ctrl+Enter on Windows and Linux) to run it.
  The sound browser lists every drum, synth and instrument name.
- **Graph.** The song as its signal flows, on a canvas to drag around and zoom: each track (its sound, its level,
  the sections it plays in) runs through a node for each of its effects into its bus; each bus runs through its
  effects into the master, and the master through its chain out at its loudness. Curves show the sends into the
  reverb and delay returns, and the kick ducking the tracks it ducks. Click a node to change it beside the canvas:
  a track's level, pan, sends, duck, drive, bus, mute and solo, the sections it plays in and its sound; an effect's
  settings (numbers to drag, switches, modes, and everything else as the JSON the song keeps it as), its place in its
  chain; a bus's or the master's chain; the master's loudness. Right-click a node to add an effect after it, move
  it or take it out. Every change undoes. The arranger's own filter moves, width and saturation are added at render
  and are not on the graph.
- **Verdict.** The score and its parts, the issues with what to change, and what earshot hears.
- **Log.** What happened, and the song's history.

**Mutate** tries a few variations and keeps the best one if it beats the current song; **Evolve** runs several
generations of that. Both run in the background and can be stopped. The **download** button, after undo and redo,
hands you the song. In a browser it downloads it: the demo as a WAV file, and a claudius as an MP3, which also saves
the song's folder on the computer claudius runs on. In the app it saves the folder, under `~/Music/claudius`, and
shows it. When the song changed since its last render, the download waits for the new render. The **export** menu
downloads the song as WAV, AIFF, FLAC, Apple Lossless, MP3, AAC, Opus or Ogg Vorbis
([Exporting](../guide/export.md)). **publish** checks the song as stores do and writes what a distributor takes
([Publishing](../guide/publish.md)).

**Record**, the red button after the loop button, records a [take](../guide/samples.md#takes) from a microphone or
an audio interface, in the studio app and in the browser studio that `claudius serve` serves. The demo has none,
since a take goes to a claudius, which keeps it with the song.

The phone button after it opens **Play from a phone**: a QR code for a phone on the same Wi-Fi, which opens drum pads,
keys and a turntable with the song's sounds there, and sends the takes played on them into the song
([Playing from a phone](../guide/phone.md)).

- **Setting it up.** Choose the input and its channel, the section the take starts with, a bar or two of count-in, and
  what it is (a vocal, a guitar playing chords ...). A MIDI keyboard's take is a part of the sound you choose, and a
  MIDI drum pad's, chosen as **drums**, plays the song's drum lanes
  ([Drums played on pads](../guide/samples.md#drums-played-on-pads)).
- **Recording.** The song plays from the count-in, and the take records from the section's first downbeat until you
  press Stop or the song ends. Then the take waits: **Play it back** plays it in the song, from a bar before it,
  **Record again** records it over, and **Keep it** sends it to claudius, which plays it once from that section, mixed
  like any other track.
- **For an assistant.** When an assistant asks you for a take, the Record window opens set up for it, with a line
  saying what to record. **Send it** sends the take to claudius and to the assistant, and **Not now** tells the
  assistant you will not record it ([A take an assistant asks for](../guide/samples.md#a-take-an-assistant-asks-for)).
- **Checking your pitch.** **Check my pitch** plays six short phrases for you to sing back, one at a time. claudius
  learns from it how you sing, shows your voice under the button, and tunes your takes so
  ([Checking your pitch](../guide/voice.md#checking-your-pitch)). An assistant can ask for one too, and the window
  opens for it.
- **Monitoring.** Wear headphones, or the microphone records the song too. The studio does not play the input back,
  so to hear yourself without delay, use your interface's direct monitoring.
- **Timing.** Press **Measure** next to Latency once for each setup: claudius plays eight clicks, listens for them
  and sets the latency from how late they come back, so that takes land in time. Turn the speakers up a little, or
  hold the headphones to the microphone, while it listens. A laptop's own speakers and microphone, for one, are
  late by more than they report: on a MacBook Pro, 45 ms. A MIDI keyboard has a Latency of its own, set by hand.
- **In a browser.** The browser asks whether the page may use the microphone the first time the Record window opens.
  It records on `localhost` or over https only, and reports less of its latency than the app does: measure there too.
  A MIDI keyboard records in Chrome, Edge and Firefox (Safari has no Web MIDI): choose **MIDI keyboards: let the page
  use them** in the input list once, and the browser asks.
- **A MIDI keyboard.** Choose it as the input, then what plays your notes (an instrument or a synth), the part they make
  (keys, a lead, chords, a pad, an arp or a bass), the grid they go to (or "as played") and a style, as for a take's
  melody. While you record, a soft
  guide tone plays what you play, for a keyboard with no sound of its own. When you stop, the notes become a new part
  ([Notes played on a keyboard](../guide/samples.md#notes-played-on-a-keyboard)).

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
