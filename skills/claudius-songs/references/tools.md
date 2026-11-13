# The MCP tools and the commands

With MCP, claudius keeps one song open in a session that the studio shows live; every edit renders unless told not
to. On the command line, each command reads and writes song folders instead.

| MCP tool | arguments | command line |
|---|---|---|
| `new_song` | `genre`, `seed`, `bpm`, `key`, `loops` (build around an Apple Loops family), `sampled` (build it on samples from free libraries; default as the genre is), `sample_search` (a kind of record for its phrase: "70s soul"), `complexity` (0 to 1: 0 the genre's core, 0.5 as written, 1 doubled instruments, a counter-melody, a pad, more percussion; default the genre's own) | `claudius make --genre house --seed 9 --bpm 124 --key "A minor"`, `claudius make --genre house --sample-search "70s soul"` |
| `make_song` | `brief`: what the person asked for, in their words ("a chill lo-fi beat around 80 bpm in D minor, minimal"), `seed`; answers with what it read, the song in plain words and the song | `claudius make --genre lofi --bpm 80 --key "D minor" --complexity 0.25` |
| `examples` | `name` (none: the list): original songs in the style of the hits of each decade, classics in the public domain remade, songs built of layered sounds, and studies of famous records (their tempo, key, chords, groove and sound, with melodies of their own), each with notes on how its style is made | `claudius examples`, `claudius examples NAME` (the script) |
| `open_example` | `name`: opens it as the song, rendered and judged | `claudius examples NAME > song.rhai && claudius script song.rhai` |
| `get_song` | (it begins with the song in plain words, for the person) | `echo \| claudius script - --on SONG --print` (an empty script prints the song) |
| `run_script` | `code`, `listen` (default true) | `claudius script edit.rhai --on SONG --out DIR` |
| `script_reference` | | `claudius reference` |
| `listen` | `stems` | `claudius render SONG/blueprint.json --stems` |
| `quick_fix` | `target` (an issue's, as the verdict's `quick fix (<target>)` lines name it; none: all of them): the fixes that need no search, a recording's tempo, a track's level, a band of the master, as one edit | |
| `mutate` | `count` (1-32, default 4), `keep_best` | `claudius mutate SONG/blueprint.json --n 4` |
| `evolve` | `generations` (1-20, default 3), `children` (2-32, default 6) | `claudius evolve --genre house --generations 4` |
| `status` | | |
| `undo`, `redo`, `stop` | | |
| `history` | `limit` (20, 1 to 200): the changes made to the song, the latest first, each with its id, what it changed in words, and whether `go_back` can reach it | |
| `go_back` | `to`: a change's id from `history`. The song as it was before that change, which undoes it and every change since; `redo` brings them back one at a time | |
| `catalog` | | `claudius genres`; `claudius-packs instruments` (installed next to `claudius`) |
| `find_genre` | `query`: what the person said ("90s", "west coast", "r&b", "dusty") | `claudius genres west coast` |
| `save` | `path` (default `~/Music/claudius/<genre>-<title>-<seed>`) | every command writes a song folder |
| `export` | `formats` (`["mp3", "flac:16", "opus"]`), `path` | `claudius export SONG -f mp3,flac:16,opus` |
| `publish` | what the person said: `artist`, `writers`, `artwork` (a file), `made`, and `title`, `album`, `genre`, `date` ...; `write` (default false: only check), `path` | `claudius publish SONG --artwork FILE --artist NAME --writer "FULL NAME" --made written` (`--check` only checks) |
| `open` | `path`: a song folder, a `blueprint.json`, a `.rhai` script, or a project the online demo saved (a `.zip`, unpacked into a song folder of its own) | `--on SONG` |
| `analyze` | `path` of any audio file | `claudius analyze song.mp3` |
| `tune` | `track`; `strength`, `speed_ms` (40 natural, 15 tight, 0 hard), `humanize`, `vibrato`, `scale`, `off` | `claudius tune vocal.wav --key "A minor"` |
| `assess_voice` | `take`, or `path` (and `key`) of a recording of one voice | `claudius voice vocal.wav --key "A minor"` |
| `remake` | `path`, `genre`, `seed`, `bpm`, `flip` (true: a new song on its hook and passage after passage of it), `rebuild` (true: the song itself as editable parts), `sampled` (false: the skeleton only) | `claudius remake song.mp3 --genre lofi [--bpm 82] [--flip] [--rebuild] [--skeleton]` |
| `song_like` | `genre`, `seed`: a new song like the recording `analyze` heard last (its tempo, key, chords, form and swing, none of its sound), without listening to it again | `claudius remake song.mp3 --genre lofi --skeleton` |
| `harmonize` | `part` (a melody), `style` (`block`: a chord under each note; `sustain`: a pad; or another chord style, or a comping idiom such as `skank` or `gospel`), `sound`, `name` | `part("lead_chords", #{ role: "keys", harmonizes: "lead", style: "block" })` |
| `generate_track` | `what` (`drums`, `percussion`, `bass`, `chords`, `keys`, `pad`, `strings`, `lead`, `arp`), `genre` (any; the song's when left out), `replace` (a part to write again that way), `name`, `listen` | `part("reggae_bass", #{ role: "bass", style: "one_drop" })`, then `vary_track` |
| `vary_track` | `track`, `take` (another take instead of a variation), `listen` | `part("bass", #{ seed: 123 })` |
| `swap_sound` | `track` (a part or drum lane; none: the whole song), `to` (one the reply lists, or any preset, instrument or drum sound; with `only: "drums"`, a kit), `only` (`drums` or `parts`), `listen` | `part("bass", #{ preset: "moog_bass" })`, `drum_sound("kick", "tr808:bd")`; a part played by a rack, `rack()` with the new sound where its old one played and the sub under it kept |
| `set_layers` | `track` (a part, a drum lane, or a pattern track's sound `beat.kick`), `layers` (1 to 8, each `sound`, `bank`, `band` [low, high] Hz, `level` dB, `pan`, `transpose`, `duck`, `fx`), `listen`: several sounds play the track at once, each in its band; `get_song` shows a track's layers as this takes them | `rack("bass", [#{ sound: "sub_bass", band: [0, 100] }, ...])` |
| `add_take` | `path` of a recording, `from` (a section's name), `name`, `role` (`vocal`, `lead`, `chords`, `keys`, `bass`, `drums`, `texture`) | |
| `trim_take` | `track` (a take), `start` and `end` in seconds into its recording | |
| `search_recordings` | `query` (a few words), `limit`: the free libraries' recordings as `library:<id>`, a `path` for `add_take`, `sample_song`, `analyze` and `remake` | `claudius samples search WORDS` |
| `graph` | (the song's signal flow in words: tracks through their effects into buses, the master, out) | |
| `sample_pieces` | `path` (or `library:<id>`), `seed`, `count`, `bars`, `find`: pieces offered as cut and treated, before any goes in | |
| `redraw_pieces` | `seed`: the same recording's pieces drawn again | |
| `keep_pieces` | `keep` (`[{id, treated}]`), `sections`, `every` (as for `sample_song`) | |
| `sample_song` | `path` of a recording, `how` (`phrases`, the default; `passage`, a verse or chorus of it; `chops`, its voice cut into an instrument; `stabs`, a syllable struck as a drum), `count` (1-4, default 2), `bars` (1, 2 or 4; a passage 4, 8 or 16), `find` ("a person singing"), `sections`, `every` (how often each phrase comes round: `once`, `2`, `4`, `8`, `16` or `through`; as the song suits by default) | `claudius sample FILE --on SONG --count 2 --in drop`, `--how passage`, `--every 4` |
| `transcribe_drums` | `take` (a beat played or beatboxed with the song) or `path` (a drum loop), `bars` (1, 2 or 4), `variant` (main), `bpm`, `keep_take` | a script: `drums_from("FILE", #{ bars: 2 });` |
| `extract_groove` | `path` of a recording with drums, `name` (default the file's), `lanes` (default all), `bpm` (when known) | a script: `groove("funk", "FILE", 96); lane("hat", #{ groove: "funk" });` |
| `request_pitch_check` | `low`, `high` (MIDI; default A3 to A4, sung in any octave), `note` (for the person) | |
| `wait_for_pitch_check` | `id` (from `request_pitch_check`), `seconds` (50, 1 to 300) | |
| `wait_for_message` | `after` (the last message's id you saw; none: from now on), `seconds` (50, 1 to 300): what the person writes to you in the studio's chat, with the messages before it; while you wait the studio says you listen (`chat_listen`) and lets them write, so call it again at once to stay. When claudius started you itself for its chat (over the Agent Client Protocol), what the person writes is your prompt and your answer goes to the chat as you write it: wait for nothing, and send nothing | |
| `send_message` | `text`, `reply_to` (the id of their message): you, in the studio's chat (`chat_answer`); they are notified when the app is in the background | |
| `request_take` | `from` (a section's name), `role`, `name`, `note` (what to record, for the person) | |
| `wait_for_take` | `id` (from `request_take`), `seconds` (50, 1 to 300) | |
| `transcribe_take` | `take`, `sounds` (presets or instruments, one part each), `name`, `role`, `chromatic`, `keep_take`, `style` (line, harmony, octaves, chords, bass, arp, pad) | |
| `add_notes` | `from` (a section), `notes` (`[{t, d, p, v}]`: beats from its start, length, MIDI pitch, velocity), `sounds`, `name`, `role`, `style` | |
| `install_packs` | `names` (from `catalog`; none: the default packs that are missing, about 3 GB) | `claudius-packs install` (installed next to `claudius`) |
| `command` | `command`: one raw API command, checked before it goes (a name or a field it does not take is not sent, and the answer says what it takes); `explain`: only say what it takes; `listen` | |

## Everything the studio does

Each control in the studio sends one command, and an assistant can send every one: with the tool named, or with
`command` and the command's JSON.

| in the studio | tool | command |
|---|---|---|
| New song, from a genre | `new_song` | `new_song` |
| New song, from an example | `open_example` | `open_text` |
| New song, from a recording: remake it, flip it, rebuild it, sample it (its phrases, a passage, chops or a stab), take its groove, see what is in it; in that window, a song like it, its tempo, its key and chords | `remake` (`flip`, `rebuild`), `sample_song`, `extract_groove`, `analyze`; `song_like`, `run_script` (`tempo(...)`, `key(...)`, `prog(...)`) | `remake`, `sample_song`, `extract_groove`, `analyze`; `song_like`, `run_script` |
| Open a song folder, a blueprint or a script; a file's text | `open`; `command` | `load`; `open_text` |
| The chat: what the person writes to an assistant listening, clearing it | `wait_for_message`, `send_message`; `command` | `chat_listen`, `chat_answer`; `chat_say`, `clear_chat` |
| Save (Download in the demo), Export, Publish | `save`, `export`, `publish` | `save`, `export`, `set_release`, `write_kit` |
| Render, and each track on its own | `listen` (`stems`) | `render` |
| Variations, Improve, Cancel | `mutate`, `evolve`, `stop` | `mutate`, `evolve`, `cancel` |
| Undo, Redo | `undo`, `redo` | `undo`, `redo` |
| the palette's Go back to before a change | `history`, then `go_back` | `go_back` |
| the palette's What your words ask: a request in plain words, confirmed, made as one change undo takes back | `command` | `together` |
| Sounds, a row's swap button | `swap_sound` | `swap_sound` |
| Tempo, Key, Swing | `command` | `set_tempo`, `set_key`, `set_swing` |
| a cell of the arrangement (a double-click, Enter, or the paint tool): a track on or off in a section; the track inspector's "plays here" | `command` | `toggle_layer` |
| a section's settings; Duplicate, Insert after; Remove; dragging it | `command` | `set_section`; `add_section`; `remove_section`; `move_section` |
| + Track, In a style | `command`, `generate_track` | `add_track`, `generate_track` |
| + Track, Recording: a file, a search of the free libraries, Record | `add_take`, `search_recordings`, `sample_pieces`; `request_take` | `add_take`, `sample_pieces`; `take` |
| a row's Remove, Another take or Vary it, Add chords under it | `command`, `vary_track`, `harmonize` | `remove_track`, `vary_track`, `harmonize` |
| a take's Trim, How is it sung?, Tune it, Turn it into drums, its melody as notes | `trim_take`, `assess_voice`, `tune`, `transcribe_drums`, `transcribe_take` | `trim_take`, `assess_voice`, `tune`, `transcribe_drums`, `transcribe_take` |
| Drums: a step, a pattern, a step's Trig, a lane's time and feel | `command` | `set_drum_step`, `set_drum_pattern`, `set_trig`, `set_lane` |
| Instruments: style, sound, range, density, Reseed | `command` | `set_part` |
| Notes: a part's notes over a progression | `command` | `set_notes` |
| Chords: a progression's chords | `command` | `set_progression` |
| Mixer and Signal flow: level, pan, sends, duck, drive, mute, solo, bus; effects | `command` | `set_mix`; `set_fx` |
| Script: Run | `run_script` | `run_script` |
| Sounds: Download the packs | `install_packs` | `install_packs` |
| Record, and a take the person sends; Not now | `request_take`, `wait_for_take`; `command` | `request_take`; `cancel_take_request` |
| Add a recording: whole, or its pieces sampled | `add_take`; `sample_pieces`, `redraw_pieces`, `keep_pieces` | `add_take`; `sample_pieces`, `redraw_pieces`, `keep_pieces` |
| Check my pitch, and turning it down | `request_pitch_check`, `wait_for_pitch_check`; `command` | `request_pitch_check`; `cancel_pitch_check` |
| Played on a phone: pads, keys, the turntable | `command`, `add_notes`, `command` | `add_hits`, `add_notes`, `add_scratch` |

A script says most of these more readably (`tempo(124)`, `section("drop", #{ energy: 0.9 })`, `mix("bass", #{ level:
-2 })`), and several at once.

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
- `command` takes `together` for several edits that are one change: `{"cmd": "together", "said": "Mute kick and
  snare", "commands": [{"cmd": "set_mix", "track": "kick", "muted": true}, {"cmd": "set_mix", "track": "snare",
  "muted": true}]}`. The history records them as one, and `undo` takes them all back at once and calls it `said`. If
  one fails, none is made. Edits only: no `undo`, `redo`, `go_back` or `together` inside it. A script run is one
  change too.
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
