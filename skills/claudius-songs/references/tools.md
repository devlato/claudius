# The MCP tools and the commands

With MCP, claudius keeps one song open in a session that the studio shows live; every edit renders unless told not
to. On the command line, each command reads and writes song folders instead.

| MCP tool | arguments | command line |
|---|---|---|
| `new_song` | `genre`, `seed`, `bpm`, `key`, `loops` (build around an Apple Loops family), `sampled` (build it on samples from free libraries; default as the genre is), `sample_search` (a kind of record for its phrase: "70s soul"), `complexity` (0 to 1: 0 the genre's core, 0.5 as written, 1 doubled instruments, a counter-melody, a pad, more percussion; default the genre's own) | `claudius make --genre house --seed 9 --bpm 124 --key "A minor"`, `claudius make --genre house --sample-search "70s soul"` |
| `get_song` | | `echo \| claudius script - --on SONG --print` (an empty script prints the song) |
| `run_script` | `code`, `listen` (default true) | `claudius script edit.rhai --on SONG --out DIR` |
| `script_reference` | | `claudius reference` |
| `listen` | `stems` | `claudius render SONG/blueprint.json --stems` |
| `mutate` | `count` (1-32, default 4), `keep_best` | `claudius mutate SONG/blueprint.json --n 4` |
| `evolve` | `generations` (1-20, default 3), `children` (2-32, default 6) | `claudius evolve --genre house --generations 4` |
| `status` | | |
| `undo`, `redo`, `stop` | | |
| `catalog` | | `claudius genres`; `claudius-packs instruments` (installed next to `claudius`) |
| `find_genre` | `query`: what the person said ("90s", "west coast", "r&b", "dusty") | `claudius genres west coast` |
| `save` | `path` (default `~/Music/claudius/<genre>-<title>-<seed>`) | every command writes a song folder |
| `export` | `formats` (`["mp3", "flac:16", "opus"]`), `path` | `claudius export SONG -f mp3,flac:16,opus` |
| `publish` | what the person said: `artist`, `writers`, `artwork` (a file), `made`, and `title`, `album`, `genre`, `date` ...; `write` (default false: only check), `path` | `claudius publish SONG --artwork FILE --artist NAME --writer "FULL NAME" --made written` (`--check` only checks) |
| `open` | `path`: a song folder, a `blueprint.json`, a `.rhai` script, or a project the online demo saved (a `.zip`, unpacked into a song folder of its own) | `--on SONG` |
| `analyze` | `path` of any audio file | `claudius analyze song.mp3` |
| `tune` | `track`; `strength`, `speed_ms` (40 natural, 15 tight, 0 hard), `humanize`, `vibrato`, `scale`, `off` | `claudius tune vocal.wav --key "A minor"` |
| `assess_voice` | `take`, or `path` (and `key`) of a recording of one voice | `claudius voice vocal.wav --key "A minor"` |
| `remake` | `path`, `genre`, `seed`, `bpm`, `flip` (true: a new song on its hook), `sampled` (false: the skeleton only) | `claudius remake song.mp3 --genre lofi [--bpm 82] [--flip] [--skeleton]` |
| `harmonize` | `part` (a melody), `style` (`block`: a chord under each note; `sustain`: a pad; or another chord style, or a comping idiom such as `skank` or `gospel`), `sound`, `name` | `part("lead_chords", #{ role: "keys", harmonizes: "lead", style: "block" })` |
| `generate_track` | `what` (`drums`, `percussion`, `bass`, `chords`, `keys`, `pad`, `strings`, `lead`, `arp`), `genre` (any; the song's when left out), `replace` (a part to write again that way), `name`, `listen` | `part("reggae_bass", #{ role: "bass", style: "one_drop" })`, then `vary_track` |
| `vary_track` | `track`, `take` (another take instead of a variation), `listen` | `part("bass", #{ seed: 123 })` |
| `swap_sound` | `track` (a part or drum lane; none: the whole song), `to` (one the reply lists, or any preset, instrument or drum sound; with `only: "drums"`, a kit), `only` (`drums` or `parts`), `listen` | `part("bass", #{ preset: "moog_bass" })`, `drum_sound("kick", "tr808:bd")` |
| `add_take` | `path` of a recording, `from` (a section's name), `name`, `role` (`vocal`, `lead`, `chords`, `keys`, `bass`, `drums`, `texture`) | |
| `trim_take` | `track` (a take), `start` and `end` in seconds into its recording | |
| `search_recordings` | `query` (a few words), `limit`: the free libraries' recordings as `library:<id>`, a `path` for `add_take`, `sample_song`, `analyze` and `remake` | `claudius samples search WORDS` |
| `graph` | (the song's signal flow in words: tracks through their effects into buses, the master, out) | |
| `sample_pieces` | `path` (or `library:<id>`), `seed`, `count`, `bars`, `find`: pieces offered as cut and treated, before any goes in | |
| `redraw_pieces` | `seed`: the same recording's pieces drawn again | |
| `keep_pieces` | `keep` (`[{id, treated}]`), `sections` | |
| `sample_song` | `path` of a recording, `count` (1-4, default 2), `bars` (1, 2 or 4), `find` ("a person singing"), `sections` | `claudius sample FILE --on SONG --count 2 --in drop` |
| `transcribe_drums` | `take` (a beat played or beatboxed with the song) or `path` (a drum loop), `bars` (1, 2 or 4), `variant` (main), `bpm`, `keep_take` | a script: `drums_from("FILE", #{ bars: 2 });` |
| `extract_groove` | `path` of a recording with drums, `name` (default the file's), `lanes` (default all), `bpm` (when known) | a script: `groove("funk", "FILE", 96); lane("hat", #{ groove: "funk" });` |
| `request_pitch_check` | `low`, `high` (MIDI; default A3 to A4, sung in any octave), `note` (for the person) | |
| `wait_for_pitch_check` | `id` (from `request_pitch_check`), `seconds` (50, 1 to 300) | |
| `request_take` | `from` (a section's name), `role`, `name`, `note` (what to record, for the person) | |
| `wait_for_take` | `id` (from `request_take`), `seconds` (50, 1 to 300) | |
| `transcribe_take` | `take`, `sounds` (presets or instruments, one part each), `name`, `role`, `chromatic`, `keep_take`, `style` (line, harmony, octaves, chords, bass, arp, pad) | |
| `add_notes` | `from` (a section), `notes` (`[{t, d, p, v}]`: beats from its start, length, MIDI pitch, velocity), `sounds`, `name`, `role`, `style` | |
| `command` | one raw API command, for edits scripts do not cover | |

Notes:

- `run_script`, `new_song`, `listen`, `undo` and `redo` wait for the render and return the verdict. `mutate`,
  `evolve` and `save` wait up to about four minutes; if the job is still running, `status` follows it.
- `get_song` is the way to read the open song: tempo, key, progressions, sections with their tracks, every track's
  sound and mix, the latest verdict, and the whole song as a script.
- `command` takes JSON such as `{"cmd": "add_section", "after": 2, "kind": "break", "bars": 8}`, `move_section`,
  `remove_section`, `toggle_layer`, `set_drum_step`, `set_trig`, `set_lane`, `set_section`, `set_part`,
  `add_track` (`{"cmd": "add_track", "name": "lead2", "kind": "part", "role": "lead", "sound": "saw_lead"}`, or
  `"kind": "drum"` and a drum sound for a lane), `remove_track`, `set_notes` (`{"cmd": "set_notes", "part": "lead",
  "prog": "A", "notes": [{"t": 0, "d": 0.5, "p": 69}]}`; no `notes` gives them back to the theory engine) and
  `set_mix`. Scripts do nearly everything these do, more readably: `part("lead2", #{ role: "lead" })`, `drum()`,
  `remove()`, `notes()`. `get_song`'s parts carry each cycle's notes, as the theory engine writes them or as written.
- `command` also takes what is played on pads and turntables: `add_hits` (`{"cmd": "add_hits", "from": "intro",
  "hits": [{"lane": "kick", "t": 0}, {"lane": "clap", "t": 1, "v": 0.7}], "bars": 1}`, beats from the section's start,
  folded into the lanes' pattern) and `add_scratch`, a take of the record (`bars` bars of a `track` from `bar`, of
  the whole mix with `"track": "*"`, or a `recording`, a WAV file in base64 recorded off a microphone) moved along
  `moves`: `t` seconds into the scratch, `at` seconds into the record, `fader` 0 shut to 1 open. A baby scratch
  goes forward and back over the record's start, `[{"t": 0, "at": 0}, {"t": 0.12, "at": 0.15}, {"t": 0.24, "at": 0}]`
  and so on; a chirp closes the fader as the record comes back.
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
