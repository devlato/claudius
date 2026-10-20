# The MCP tools and the commands

With MCP, claudius keeps one song open in a session that the studio shows live; every edit renders unless told not
to. On the command line, each command reads and writes song folders instead.

| MCP tool | arguments | command line |
|---|---|---|
| `new_song` | `genre`, `seed`, `bpm`, `key`, `loops` (build around an Apple Loops family) | `claudius make --genre house --seed 9 --bpm 124 --key "A minor"` |
| `get_song` | | `echo \| claudius script - --on SONG --print` (an empty script prints the song) |
| `run_script` | `code`, `listen` (default true) | `claudius script edit.rhai --on SONG --out DIR` |
| `script_reference` | | `claudius reference` |
| `listen` | `stems` | `claudius render SONG/blueprint.json --stems` |
| `mutate` | `count` (1-32, default 4), `keep_best` | `claudius mutate SONG/blueprint.json --n 4` |
| `evolve` | `generations` (1-20, default 3), `children` (2-32, default 6) | `claudius evolve --genre house --generations 4` |
| `status` | | |
| `undo`, `redo`, `stop` | | |
| `catalog` | | `claudius genres`; `claudius-packs instruments` (installed next to `claudius`) |
| `save` | `path` (default `~/Music/claudius/<genre>-<title>-<seed>`) | every command writes a song folder |
| `export` | `formats` (`["mp3", "flac:16", "opus"]`), `path` | `claudius export SONG -f mp3,flac:16,opus` |
| `open` | `path`: a song folder, a `blueprint.json` or a `.rhai` script | `--on SONG` |
| `analyze` | `path` of any audio file | `claudius analyze song.mp3` |
| `remake` | `path`, `genre`, `seed`, `bpm` | `claudius remake song.mp3 --genre lofi [--bpm 82]` |
| `command` | one raw API command, for edits scripts do not cover | |

Notes:

- `run_script`, `new_song`, `listen`, `undo` and `redo` wait for the render and return the verdict. `mutate`,
  `evolve` and `save` wait up to about four minutes; if the job is still running, `status` follows it.
- `get_song` is the way to read the open song: tempo, key, progressions, sections with their tracks, every track's
  sound and mix, the latest verdict, and the whole song as a script.
- `command` takes JSON such as `{"cmd": "add_section", "after": 2, "kind": "break", "bars": 8}`, `move_section`,
  `remove_section`, `toggle_layer`, `set_drum_step`, `set_section`, `set_part` and `set_mix`. Scripts do nearly
  everything these do, more readably.
- `SONG` is a song folder. `make`, `script` and `remake` write theirs as `<genre>-<title>-<seed>/` inside `--out`,
  which is `out/` in the current folder unless given; `render` writes into the blueprint's own folder, or straight
  into `--out`. Each prints the path at the end. When the person needs to find the song, give a folder they know:
  `--out ~/Music/claudius`.
- There is no undo on the command line, and none is needed: every step writes a new folder, and the one before is
  the undo. Give each step its own `--out`. Writing an edit into the folder it reads from replaces the song you are
  editing, because the folder name stays the same.
- A command-line song folder holds `song.wav`, `song.mp3`, `blueprint.json` (the whole song),
  `song.rhai` (the song as a script), `project.json`, `verdict.json`, `report.json`, `stems/` when asked for, and
  `credits.txt` when sampled instruments need credit. `claudius judge SONG/song.wav` judges it again, with the
  arrangement checks because `project.json` sits next to it.
- `claudius script` without `--on` starts from nothing: begin the script with `song()` or `blank()`.
