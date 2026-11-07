# The studio

```sh
claudius studio
```

opens the studio in its own window. `claudius serve` runs the same studio in your browser at
<http://127.0.0.1:7878>. Both are views of one session: you can have the window, a browser tab and an assistant
working on the same song at once, and each sees the others' changes as they happen.

Every change is rendered and judged in the background. The right end of the toolbar says what claudius is doing
(**Rendering**, with how far it has got, and Cancel for a job that can stop), or whether what plays is the song as it
is now: **Up to date**, or **Changes not rendered yet** with a **Render** button. The score badge updates when the new
verdict arrives, a few seconds later.

## The first time

The first time it opens, a short tour points at the parts of the studio in five cards: making a song, listening, the
verdict, the editors, and connecting an assistant, with the Connect buttons right in the last card. The app then asks
once whether to send anonymous usage statistics, which stay off unless you say yes ([what they are](../guide/privacy.md)).
The tour never blocks the studio. **Skip the tour** or Esc ends it for good, and **?** in the bottom right corner,
**Take the tour**, starts it again.

The first time it starts, claudius downloads its listening models (about 0.8 GB) in the background; the verdict
judges with meters and music theory until they are ready, and the log says when they are. The first time it remakes
or samples a recording, it downloads the nets that separate a recording's vocals, drums and bass (about 160 MB). If the default sample
packs are not installed, the studio says so and offers **Download the packs** (a 3.1 GB download): until then, the
sampled instruments are synthesized. The **Sounds** tab lists every pack, with a button for each one that is not
installed.

## Starting a song

**New song** asks for a genre and, if you like, a seed, tempo and key. The same genre and seed always give the same
song, so a seed is a way to come back to one you liked. **Fused with** fuses a second genre into it: the first leads
the song, and the second plays the drums and lends its sounds ([Fusions](../cookbook/fusions.md)). **Samples**,
**auto-sample from free libraries**, builds the song on recordings from free libraries, as records built on samples
are: a phrase it takes its key, tempo and chords from, a drum break, a texture. It starts ticked for the genres built
on samples (trip-hop, boom bap, lo-fi, jungle, big beat, filter house), and for any other genre you tick it. Its field
takes the kind of record to search the phrase for ("70s soul", "bossa nova guitar"); empty, it shows the records the
genre's producers dig for, and the song picks among them, and the dice puts one of them in the field
([Songs built on samples](../guide/online-samples.md#songs-built-on-samples)). **Layers** says how layered the song
is, from the genre's core to its harmony and lead doubled on second instruments, a counter-melody, a pad and more
percussion; ticked, it is as the genre writes it ([How layered a song is](../learn/producers/genres-and-evolution.md#how-layered-a-song-is)).
It can also start from a recording: **Choose a file** under
"Or start from an audio file", then **Analyze** shows what claudius hears in it, and **Remake** reworks it in the
genre so that it stays the song it is: all of it plays through, its voice and its chords as they are without its drums
and its bass, in its own sections and on its chords, with the genre's drums and bass under it. **Flip into**
makes a new song of the genre on its sound instead: its hook, another passage of it, its drums, a flip cut from all
over it and its sung phrases, at a tempo the genre plays. **Sample its vocals into the song** puts its best sung phrases into the song
that is open ([Sampling a song's vocals](../guide/samples.md#sampling-a-songs-vocals)), and **Take its groove** puts
the open song's drum lanes in the feel of the recording's drums ([Grooves](../cookbook/drum-machine.md#a-groove-from-a-record)).
While claudius works on the file, the dialog says what it is doing (taking it apart into its voice, drums, bass and
the rest, listening to it, sampling its voice, remaking it), with how far it has got when it knows, and so does the
line at the top right. What changed the song closes the dialog when it is done: the song shows with its new tracks
(`vox1`, `vox2` ... for sampled phrases), the toast says what came of it in a line ("sampled 2 phrases of song.mp3
into the song: vox1, vox2"), the render it starts is on the line at the top, and the dialog keeps the whole answer
for when you open it again, as the **Log** tab does. **Analyze**, and a file claudius could not use, leave the dialog
open with the answer in it.
**Open** opens a song you saved: choose the `blueprint.json` in its folder,
or a `.rhai` script. The studio reads the file on its own computer, in a browser too, and nothing of it is kept but
the song it opens. It also opens a project the demo saved, a `.zip`: claudius unpacks it into a song folder of its own
([Carrying on in claudius](#carrying-on-in-claudius)).

## The panels

- **Arrangement.** The sections of the song from left to right (intro, build, drop and so on), each with its length
  and a meter of how intense it is, and below them one row per track, with an icon for what it is (drums, keys, a
  bass, a pad, a line, a voice, a sample, an effect) or that it is muted or soloed. A track plays where it has a
  clip, one for each run of sections it plays in, shaded by how intense each is; a drum lane's shows how busy each
  bar is. Click a cell, or drag across cells, to switch a track on or off in those sections. Drag a section to
  move it; right-click it to duplicate, insert or remove one. **+ Track** adds a track: one
  [written the way a genre plays it](#writing-a-track-in-a-style) (**In a style**), a part (a bass, chords, a
  pad, keys, a lead or an arpeggio the theory engine writes over the song's chords, written the way the genre writes
  that role) or a drum lane to fill in under Drums, its name, and its sound, from the synths made for the role, the
  installed instruments, or the drum voices and kits; a new track plays in the sections with energy 0.6 or more,
  ready for you to move. Right-click a row to mute or solo it, [swap its sound](#swapping-a-sound), switch it on or
  off everywhere, or remove it (Undo brings it back). A part's row also offers **Add chords under it**: chords that fit its melody and the song's
  chords, a chord on each of its notes, a pad under it, stabs or offbeats, as a new part that
  [follows the melody](../language/song-functions.md#part) as it changes. Right-click a [take](../guide/samples.md#takes)'s row
  to hear [how it is sung](../guide/voice.md) (**How is it sung?**), to [tune it](../guide/voice.md#tuning-a-voice)
  (**Tune it**: naturally, tightly, hard, or not at all), to turn a beat you played or beatboxed into the song's
  drum lanes ([**Turn it into drums**](../guide/samples.md#turning-a-take-into-drums)), or to turn the melody you
  sang into notes an instrument plays, as you sang it or as harmony, octaves, chords, a bass line, an arpeggio or a
  pad. The waveform of the latest render is underneath; click it to jump.
- **Section.** The selected section's length, its intensity (how full and loud it plays), which chord progression
  it plays, its drum pattern, its transitions (a filter sweep, a build-up, a pause at the end, an impact, drum
  fills), and a switch for each track playing in it.
- **Verdict.** The score and its parts (Mix, Shape, Harmony, and with the ears Genre, Production and Records), the
  things to fix, each with how much it costs and where to fix it, and, folded under **What it measured**, what
  earshot hears and the meters.

Under them, the tabs hold the editors: **Drums**, **Instruments**, **Notes**, **Chords**, **Mixer** and **Sounds**, and
at the right end **Script**, **Signal flow** and **Log**.

- **Drums.** A step sequencer for every drum track. Click a step to toggle it, drag to paint, shift-drag to set how
  hard a hit is. Right-click a step and open **Trig** to set the chance it plays, when it plays (`1:2`, `last` ...),
  a ratchet of strokes inside it and how they swell or fade, a flam, a nudge off the grid, and its pitch, decay, tone
  and pan for that stroke ([The drum machine](../cookbook/drum-machine.md)). A step with a trig shows it: its chance
  as a number, its condition, a ratchet's strokes, a dot for a flam, a diamond for the sound it changes, and a nudged
  step sits off the grid. Right-click a lane for its own time and feel: a length of its own (steps past it are faint,
  showing the steps that come round again there), its speed, a swing of its own or the song's, and a groove and how
  much of it. The lane's name shows what it has.
- **Instruments.** The bass, chords, pad, lead and arpeggio parts: their writing style, their sound (a synth or a
  sampled instrument, with the swap button beside it), their range and how busy they are. **Reseed** rewrites a part.
- **Notes.** A part's notes over one of its progressions, as a piano roll: a grid of beats and pitches. Click an
  empty place to add a note, drag a note to move it, drag its right edge to make it longer or shorter, right-click it
  to take it out; the grid (1/16 to 1/2) is what notes snap to. The notes the theory engine writes are shown faint,
  and the first change writes them out for you to edit; **Back to the theory engine** lets it write them again. The
  notes come round every time the part plays that progression.
- **Chords.** The chord progressions, as chords in the song's key and as numerals, and which sections play which.
- **Mixer.** Level, pan, reverb, delay, how much each track ducks under the kick, and its drive, which overloads it
  (rougher at the same loudness); mute and solo; meters from the last render; the master's loudness and true peak.
- **Sounds.** The sample packs, with a button for each one that is not installed, and every sound a part or a script
  can play: the sampled instruments by pack, the synths and the drum voices. Click an instrument or a synth to put it
  on one of the song's parts.
- **Script.** The whole song as a script. Edit it and press Cmd+Enter (Ctrl+Enter on Windows and Linux) to run it.
  The sound browser lists every drum, synth and instrument name.
- **Signal flow.** The song as its signal flows, on a canvas to drag around and zoom: each track (its sound, its level,
  the sections it plays in) runs through a node for each of its effects into its bus; each bus runs through its
  effects into the master, and the master through its chain out at its loudness. Curves show the sends into the
  reverb and delay returns, and the kick ducking the tracks it ducks. Click a node to change it beside the canvas:
  a track's level, pan, sends, duck, drive, bus, mute and solo, the sections it plays in and its sound; an effect's
  settings (numbers to drag, switches, modes, and everything else as the JSON the song keeps it as), its place in its
  chain; a bus's or the master's chain; the master's loudness. Right-click a node to add an effect after it, move
  it or take it out. Every change undoes. The arranger's own filter moves, width and saturation are added at render
  and are not on the graph.
- **Log.** What happened, and the song's history.

**Variations** tries a few versions of the song, with a change or two each, and keeps the best one if it beats the
current song (how many is in its menu); **Improve** runs rounds of that, each from the best version so far. Both run
in the background and can be stopped, and in the demo too, where its engine judges the variations one after another
without the ears, so they take a while longer. **Save**, after **Open**, hands you the song. In a browser it
downloads it: the demo (where it is **Download**) as a FLAC file, lossless at about half a WAV's size, and a claudius
as an MP3, which also saves the song's folder on the computer claudius runs on. In the app it saves the folder, under
`~/Music/claudius`, and shows it. When the song changed since its last render, the download waits for the new render.
The **Export** menu downloads the song as WAV, AIFF, FLAC, Apple Lossless, MP3, AAC, Opus or Ogg Vorbis
([Exporting](../guide/export.md)), and renders each track on its own as a WAV (stems). In the demo it offers WAV,
AIFF, FLAC, MP3 and Opus, which the demo's engine writes in the page; AAC and Apple Lossless need the system's
encoder, and Ogg Vorbis a library the demo leaves out. In the demo it also offers the song itself, as a project to
carry on with in claudius ([Carrying on in claudius](#carrying-on-in-claudius)). **Publish…**, at its foot, checks the
song as stores do and writes what a distributor takes ([Publishing](../guide/publish.md)).

**Record**, the red button after the loop button, records a [take](../guide/samples.md#takes) from a microphone or
an audio interface, in the studio app, in the browser studio that `claudius serve` serves and in the demo, which
keeps it in the page with the song (and in the project it saves). The **+** after it opens **Add a recording**: a song
or a sound of your own, chosen or dropped anywhere on the studio, added whole as a track that you trim and put effects
on, or with its vocals sampled into the song ([Takes](../guide/samples.md#takes)).

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

## Writing a track in a style

**+ Track**, then **In a style**, writes a track the way a genre plays it, into the song as it is: its key, its
chords and its form. Choose what (drums, percussion, bass, chords, keys, pad, strings, lead or arp) and the genre,
the song's own or any other, and press **Generate**. A part is written as that genre's own part of the role is: in its
idiom (reggae's one drop, funk's slap, the dembow, a jazz walk, a trap 808 ...), on a sound from its palette, in its
register and with its mix; strings play the genre's strings, or a pad on strings. It goes in as a new track,
`<genre>_<what>`, where the song plays its part of that role, or with **in place of** ticked, it writes the song's
part again, keeping its name, mix and sections. Drums write the genre's lanes from its templates and kits in place
of the song's lanes of their names and add the others beside them; percussion adds the genre's shakers, rims and
percussion beside the song's drums. Each press is another take.

Right-click a part's or a drum lane's row for **Another take**, written again (a part with a new seed, a lane from
the template of the genre it was written as: the song's, or the one its drums were generated from), or **Vary it**:
the same take with other details. A part keeps its rhythm, the cells of its
idiom, and draws its dead notes, octaves, approaches and pushes again; a lane is varied a step or two, its hits on
the strong beats kept. The Instruments tab has **Vary** beside **Reseed**. Every one is an edit that undoes.

## Swapping a sound

A sound you do not like is a click from another. Hover a part's or a drum lane's name in the arrangement, or a lane's
under Drums, and click the swap button where its kind or its length is written: the track plays the next sound that fits
it. A part goes through its genre's own sounds first, then what the genre draws for its role (synths, and the
installed instruments that can play its range), then sounds further afield; a drum lane goes through the installed
kits, the genre's first, then its synthesized voice in four tunings. It never takes a sound another part plays, and
block chords under a part change with it. Click again for the next, and Undo to swap back. Right-click the row for
**Another sound**, and **Swap the sound to**: every sound it could play, by how near its genre each is, with the one it
plays ticked. The Instruments tab has the same button beside each part's sound.

**Sounds**, after Improve, swaps the whole song's. **Swap every sound** puts each part on its next sound and the drums
on the next kit, and **Swap the parts' sounds** leaves the drums as they are. **Another drum kit** and **Put the drums
on** move every drum lane onto a kit, or back to the synthesized voices, and leave the parts. Every note, pattern and
mix setting stays. A swap is an edit like any other: the song's script says it as `part("bass", #{ preset:
"moog_bass" })` and `drum_sound("kick", "tr808:bd")`, and an assistant swaps a sound with
[`swap_sound`](../guide/mcp.md).

## Carrying on in claudius

A song begun in the [demo](https://claudius.devlato.com/studio/?mock) goes on in claudius. The demo's **Export** menu
downloads it as **claudius project (.zip)**: its `blueprint.json` and `song.rhai`, and under `samples/` every sample
it plays, cut from the free libraries' recordings, with the recording it came from beside each (its page, who made it
and its licence) in a `.json`, and their credits in `credits.txt`. A sample is kept as 24-bit FLAC, and a drum hit as
the 32-bit WAV the demo cut it as, so nothing of them is lost.

[Install claudius](install.md), then **Open** the zip in the studio, or have an assistant `open` it: claudius unpacks
it into a song folder of its own under `~/Music/claudius`, named after the file, and opens the song there, playing
the samples the zip brought. From then on it is a song like any other: it saves into that folder, and credits the
recordings it plays in the folder's `credits.txt`
([Where they go, and credits](../guide/online-samples.md#where-they-go-and-credits)). A zip with a file that would
land outside its folder is not opened, nor one that would unpack to more than 2 GB or 10,000 files.

Safari unpacks a zip it downloads into a folder. Choose the `blueprint.json` in that folder: the studio app brings the
samples beside it along, as the zip would have, and the song opens in a folder of its own all the same. A project's
folder zipped again, as Finder or Explorer zips one, opens as the demo's zip does.

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
