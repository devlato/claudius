# 0025 The signal flow as the song's lanes

Status: accepted

## Context

The studio shows a song three ways, as equals in the main area: the arrangement (its sections and the parts that play
in them), the script, and the signal flow, which is every path the sound takes from the sources to the finished song.
The signal flow began as a node graph and became a board (`panels::board`): a card for each track, its effects as
chips of text, curved wires into a card for each bus and return, and the master. It fitted itself to the window,
could not be zoomed or panned, said an effect in words rather than showing what it does, and left the sound's path to
be read off wires that cross. People who mix think of a mixer's channel strips and a device chain: each track's
sound running through its devices in order, each device showing what it does (an equalizer's curve, a compressor's
threshold and ratio), into the buses, the returns and the master.

An effect could be added, moved, changed and removed, but not switched off to hear the track without it and back
again, which is how a mix is judged; removing it lost its settings.

## Decision

The signal flow is drawn as the song's lanes, the arrangement's tracks in the arrangement's order and colours, each
one row the sound runs through from left to right.

- **A track's lane**: its source (the instrument, the kit or the recording it plays), each effect of its chain as a
  block in order, then its fader, pan and meter, then its sends to the reverb and delay returns. Its output runs into
  its bus.
- **The buses, the returns and the master** are lanes of the same kind under the tracks, their chains as blocks,
  ending in the finished song.
- **A block shows what its effect does**: an equalizer or a filter its frequency curve, a compressor or a limiter
  its transfer curve with its threshold and ratio, saturation its curve and drive, a delay its time and feedback, a
  reverb its decay. Zoomed out, a block is its name and its main value; zoomed in, its picture, with its power button
  and its × along its top.
- **Everything is edited where it is**: a click chooses a block and opens all its settings in the inspector beside the
  lanes, a drag moves it along its chain or into another, a + between two blocks adds one there, × or Delete removes
  it, a power button switches it off, a fader's level and pan drag where they are, and the keyboard reaches every
  block.
- **Zoom and pan as a map does**: around the pointer with a pinch or the scroll wheel held with Ctrl or Cmd, dragging
  the background to pan, and a button to fit it all to the window.
- **An effect can be switched off**: wrapped as an effect of its own, `{"fx": "off", "effect": {"fx": "gain", "db":
  3}}`, it keeps its place in its chain and all its settings, and the renderer passes the sound by it untouched;
  switched on again, it is unwrapped. Every earlier song reads as it did (0016), and a script writes it the same way:
  `#{fx: "off", effect: #{fx: "gain", db: 3}}`.

The studio and the web demo draw it from the same code, and the MCP's `graph` tool describes the same flow in words.

Rejected: keeping a node graph with better drawing (the sound's path stays a tangle of wires to follow, and a node
graph is a programmer's picture, not a musician's); a mixer of channel strips alone (it hides the order of the
effects, which is half of what a chain is); bypass as a list beside each chain (it would have to follow every move of
the effects it names); `"on": false` among an effect's settings (it reads better, but a claudius that knows nothing of
`on` ignores a field it does not know and plays the effect switched on, without a word, where one that does not know
`off` says so).

## Consequences

- Each kind of effect needs its own picture in a block, and a new kind of effect a new picture (a kind without one is
  drawn as its name and settings, never left out).
- Everything that reads a chain has to know `off`: the renderer passes it by, and the summaries, the critic's reading
  of a chain and the words for an effect say what it wraps and that it is off. A song saved with an effect switched off
  does not open in a claudius older than the one that wrote it, which says why rather than playing it wrong.
- A whole song's lanes are more than a window holds; zoomed out they are blocks too small to edit, which is what the
  zoom is for.
- The board's code goes; its parts that stay (a track's level, pan, mute, solo and sends, the right-click menus, the
  inspector beside it) move into the lanes.
