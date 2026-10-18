# Command line

`claudius` is one program with a subcommand per task. `claudius --help` lists them and `claudius <command> --help`
shows the flags of one. The sample packs have a second program, `claudius-packs`, described in
[Sample packs](packs.md).

Every command that fails prints `Error: ` and the reason on stderr and exits with status 1.

## Make a song

### `claudius make`

Composes, arranges, mixes, masters and judges a new song, and writes it into a folder.

```sh
claudius make --genre deep_house --seed 7 --short
```

```text
ears loaded in 3.8 s
Glass Garden — deep_house at 118 BPM in D minor, 52 bars (1:46)
  progression A: i7 VImaj7 iv7 v7
  progression B: VImaj7 v7 iv7 i7
  form: intro 8 | groove 8 | break 8 | build 4 | drop 16 | outro 8
  tracks: kick (synth kick), clap (synth clap), hat (hat cut from 'Silver Hi-Hat 04' (Apple Loops)), shaker (shaker cut from 'Ride With Me Shaker' (Apple Loops)), perc (perc cut from 'Latin Day Bongo 05' (Apple Loops)), bass (synth patch 'pluck_bass'), chords (synth patch 'keys'), pad (sampled string_ensemble (vsco2:strings)), lead (synth patch 'pluck'), crash (crash cut from 'Beautiful Memories Percussion 02' (Apple Loops)), riser (synth riser)
  master: -11.0 LUFS, true peak -1.0 dBTP (pre-master -10.3 LUFS); done in 11.4 s
  ears: 'house music' z +3.7 (closest confusion 'techno music' +3.9); production z +1.7 vs 'a low quality recording with hiss' +1.3; p_real 0.63
        sounds like: techno music, tech house music, house music, drum and bass music
  verdict: 86/100 (arc 0.94, genre 0.71, mix 0.93, production 0.59, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -28.6 dB, healthy from -26); production reads close to 'a low quality recording with hiss' (z +1.3) vs professionally mixed (z +1.7); fizzy top end (air -16.2 dB, healthy up to -17)
  wrote out/deep_house-glass-garden-7/song.mp3
```

The result depends on what is installed: the same genre and seed give the same song on any machine with the same
packs and Apple Loops, and a different one, in its sounds and possibly its drum patterns, on a machine without
them.

| flag | default | what it does |
|---|---|---|
| `-g`, `--genre` | `house` | a genre from `claudius genres` |
| `-s`, `--seed` | the clock | the same genre, seed and options give the same song |
| `--bpm` | from the genre's range | tempo |
| `--key` | from the genre | `"Am"`, `"F# minor"`, `"Eb"`, `"D dorian"` |
| `--short` | off | halves every section (at least 4 bars), for a quick sketch |
| `--stems` | off | also writes every track to `stems/<track>.wav` (32-bit float) |
| `--samples` | `0.75` | the chance that a drum lane uses a one-shot cut from Apple Loops or your library instead of a synthesized voice |
| `--synth-only` | off | synthesizes every sound: no sample packs, Apple Loops or user library |
| `--no-library` | off | leaves out Apple Loops and your user library; the sample packs still play |
| `--sample <file>` | none | flips a recording into the song, warped to its tempo and key; repeatable |
| `--loops` | off | builds the song around a family of Apple Loops (their chords, bass and beat), GarageBand style |
| `--no-ears` | off | judges with meters and theory only, without loading earshot's models |
| `--refs <path>` | none | reference tracks or folders; with two or more, the verdict scores how typical the song is of them |
| `--tags` | off | also runs the AudioSet tagger and prints its labels as held-out evidence |
| `-o`, `--out` | `out` | the folder the song folder goes into, relative to where you run the command |

A seed from the clock is a number below one million, printed in the folder name, so you can make the same song again.

The song folder is `<out>/<genre>-<title>-<seed>`, for example `out/deep_house-glass-garden-7`, and holds:

| file | what it is |
|---|---|
| `song.wav` | the master, 48 kHz, 24-bit |
| `song.mp3` | a 256 kbps copy, when ffmpeg is installed |
| `blueprint.json` | the song's genome: open it, edit it, render it again |
| `song.rhai` | the song as a script, for `claudius script` or the studio's script editor |
| `project.json` | the arranged session the blueprint expanded into |
| `report.json` | tempo, key, form, loudness and every track's level and gain |
| `verdict.json` | the critic's scores, measures and issues |
| `credits.txt` | attributions, when the song plays sample-pack instruments |
| `stems/` | with `--stems` |

[Files and settings](files.md#song-folders) compares this with the folders `claudius script` and the studio write.

### `claudius script`

Runs a [script](../language/overview.md) and renders the song it makes or edits.

```sh
claudius script song.rhai                       # a script that starts its own song
claudius script edit.rhai --on out/house-glass-haze-9
echo 'tempo(120); mute("lead");' | claudius script - --on out/house-glass-haze-9 --print
```

| flag | default | what it does |
|---|---|---|
| `<file>` | | the script, or `-` to read it from stdin |
| `--on <path>` | none | edits this song (a `blueprint.json` or a song folder) instead of starting with none |
| `--print` | off | prints the resulting song as a script and renders nothing |
| `--no-ears`, `--refs`, `--tags` | | as for `make` |
| `-o`, `--out` | `out` | where the song folder goes |

The folder has the same files as `make` writes. The script's log goes to
stderr: the song it started, the sounds each track resolved to (`track hats: hh`), and anything it printed with
`print()` or `debug()`. A script that ends without a song fails:

```text
Error: the script made no song (start it with song("house") or blank("house"))
```

Script errors come back with the line and position; [Errors and limits](../language/limits.md) lists them.

```text
Error: Runtime error: tempo between 40 and 240 BPM (line 1, position 1)
```

### `claudius render`

Renders a `blueprint.json` again, for example after you edited it by hand.

| flag | default | what it does |
|---|---|---|
| `<blueprint>` | | the blueprint file |
| `--stems` | off | stems too |
| `--no-ears`, `--refs`, `--tags` | | as for `make` |
| `-o`, `--out` | the blueprint's folder | where to write; without it the song files next to the blueprint are replaced |

### `claudius mutate`

Writes variations of a blueprint, each rendered in full, with what changed.

```sh
claudius mutate out/house-glass-haze-9/blueprint.json -n 4
```

Each variation goes into `<blueprint folder>/mutations/<n>/` (or `-o`), and one line per variation reports the
change and its verdict: `1: <what changed> -> <score> (<folder>)`. Unlike mutation in the studio, these mutations
are not steered by the critic's issues.

| flag | default | what it does |
|---|---|---|
| `<blueprint>` | | the blueprint to vary |
| `-n` | `4` | how many variations |
| `-s`, `--seed` | the clock | the random choices |
| `--no-ears`, `--refs`, `--tags` | | as for `make` |
| `-o`, `--out` | `<blueprint folder>/mutations` | where they go |

### `claudius evolve`

Runs an evolutionary search: mutation and crossover over generations, each candidate judged on a preview of its
build-up and first drop. [Evolution and analysis](../engineering/evolution-and-analysis.md) explains the search.

```sh
claudius evolve --genre techno --seed 4 --start 2 --generations 2 --children 3 --preview-bars 16 --keep 0 --no-ears
```

```text
evolving techno (2 start, 2 generations x 3 children, 16-bar previews, without ears)
  g0-0  90.5  Velvet Orbit
  g0-1  97.1  Hollow Harbor
  g1-0  97.1  from g0-1: tempo 128 -> 126 BPM
  g1-1  90.4  from g0-0: mix: bass -1.3 dB (now -1.3); form: outro now 12 bars
  g1-2  90.5  from g0-0: ride: synth voice parameter 0 -> 0.931
generation 1: best 97.1
  g2-0  97.1  from g0-1: form: peak drums main -> sparse; tempo 128 -> 130 BPM
  g2-1  97.1  from g0-1: form: groove drums main -> sparse; drums (main): nudged a hat hit by +0.25 beats
  g2-2  91.4  from g1-0: crossover: drums and form of one parent, harmony and parts of the other; form: break drums main -> sparse; form: build loses gap
generation 2: best 97.1
searched 8 candidates in 7 s; rendering the best 0 in full
```

A child that beats the best parent so far is marked with `+`. Every candidate goes into
`<out>/evolve-<genre>-<seed>/evolution.json` (id, generation, parent, change, score, summary), and the best `--keep`
are rendered in full into numbered folders next to it, with their history printed.

| flag | default | what it does |
|---|---|---|
| `-g`, `--genre` | `house` | the genre |
| `--from <blueprint>` | none | starts from this song instead of fresh ones |
| `-s`, `--seed` | the clock | the fresh songs use seeds `seed`, `seed + 1`, ...; the search uses it too |
| `--start` | `4` | how many fresh songs start the search |
| `--generations` | `5` | generations |
| `--population` | `4` | parents kept per generation |
| `--children` | `8` | children per generation |
| `--preview-bars` | `32` | bars auditioned per candidate |
| `--keep` | `3` | best songs rendered in full at the end |
| `--samples`, `--synth-only`, `--no-library`, `--loops` | | as for `make` |
| `--no-ears`, `--refs`, `--tags` | | as for `make` |
| `-o`, `--out` | `out` | where the run folder goes |

### `claudius remake`

Analyses an audio file and writes a new song on its skeleton: tempo, key, swing, form, progressions, drum habits
and bass style, with the genre's sounds and mix. It prints the analysis (as `analyze` does) and the new song.

| flag | default | what it does |
|---|---|---|
| `<file>` | | any audio file claudius can decode |
| `-g`, `--genre` | `house` | the genre of the new song; its tempo range guides the beat tracker |
| `-s`, `--seed` | the clock | the new song's own choices |
| `--no-ears`, `--refs`, `--tags` | | as for `make` |
| `-o`, `--out` | `out` | the folder is `remake-<genre>-<title>-<seed>` |

## Listen and judge

### `claudius judge`

Gives the critic's verdict on any audio file. When the file has a `project.json` next to it (as `make` writes), the
arrangement and theory checks run too; without one, only the meters on the master and the ears do.

```sh
claudius judge out/deep_house-glass-garden-7/song.wav --no-ears
```

```text
out/deep_house-glass-garden-7/song.wav: 95/100 (arc 0.94, mix 0.93, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -28.6 dB, healthy from -26); fizzy top end (air -16.2 dB, healthy up to -17)
  arc_correlation: 0.89
  band_air_db: -16.2
  band_low_db: -2.6
  band_lowmid_db: -5.5
  band_mid_db: -12.2
  band_presence_db: -28.6
  band_sub_db: -11.4
  bass_roots_on_changes: 1
  drop_minus_break_lu: 3.5
  lead_chord_tones_on_strong_beats: 1
  lead_in_key: 1
  lead_range_semitones: 12
  lead_stepwise: 0.63
  length_s: 106
  low_end_correlation: 1
  lra_lu: 3.73
  lufs: -10.98
  stereo_correlation: 0.86
  true_peak_db: -1.05
  voice_leading_chords: 1
  voice_leading_pad: 1.1
  - [0.44] dull 4-8 kHz: hats, claps and snare lack bite (presence -28.6 dB, healthy from -26) (band:presence-)
  - [0.13] fizzy top end (air -16.2 dB, healthy up to -17) (band:air+)
```

| flag | default | what it does |
|---|---|---|
| `<file>` | | the audio file |
| `-g`, `--genre` | the project's, or none | the genre the ears compare against; without one they report the closest genre in earshot's vocabulary |
| `--no-ears`, `--refs`, `--tags` | | as for `make` |
| `--json` | off | prints the whole verdict as JSON |

[The critic and the ears](../engineering/critic.md) explains every measure and score.

### `claudius analyze`

Reads a song the way a producer does before remaking it.

```sh
claudius analyze out/deep_house-glass-garden-7/song.wav --genre deep_house
```

```text
118.0 BPM (pulse clarity 0.55), Bb major (confidence 0.43), swing 0.50, 52 bars from 0.00 s
  progression A: iii7 Imaj7 vi7 vii  |  B: Imaj7 Imaj7  (1 bar(s) per chord)
  bar   0 +16  intro   energy 0.83  kick bass hats
  bar  16 +12  break   energy 0.30  kick hats
  bar  28 +24  drop    energy 1.00  kick bass hats
  kick  X.-xX.xxX.x.X.x-
  snare X.xXXxxxXxXxX-Xx
  hats  x-Xxx.X-xXXxx.XX
  bass style: offbeat
```

This is claudius reading its own D minor song: the tempo is exact and progression `A` has the song's chords in their
order (Dm7, B♭maj7, Gm7, Am), but named in B♭ major, and the break's progression `B` came out as one chord where the
song has four. Analysis of full mixes is approximate; the
[engineering page](../engineering/evolution-and-analysis.md#reading-a-song) says where it goes wrong. In the step
grids, `X` is a hit in almost every bar, `x` a common one, `.` a rare one and `-` none.

| flag | default | what it does |
|---|---|---|
| `<file>` | | the audio file |
| `-g`, `--genre` | none | a genre whose tempo range guides the beat tracker (else 70 to 180 BPM, centred on 120) |
| `--json` | off | the whole analysis as JSON, including the chord of every bar |

## Make material

### `claudius chop`

Cuts a recording into material: one bar loop per section it finds and a kick, clap and hat one-shot, saved as WAV
files and added to the user library, where `make` and scripts find them.

```sh
claudius chop out/deep_house-glass-garden-7/song.wav --bars 2
```

```text
chopped 3 loops and 3 one-shots (added to the user library):
  out/chops/song/01-intro-2bars.wav (118 BPM, 8 beats, key A#)
  out/chops/song/02-break-2bars.wav (118 BPM, 8 beats, key A#)
  out/chops/song/03-drop-2bars.wav (118 BPM, 8 beats, key A#)
  kick: out/chops/song/kick.wav
  clap: out/chops/song/clap.wav
  hat: out/chops/song/hat.wav
```

The analysis is printed first, as `analyze` prints it.

| flag | default | what it does |
|---|---|---|
| `<file>` | | the recording |
| `--bars` | `2` | bars per loop |
| `-g`, `--genre` | none | a tempo range for the beat tracker |
| `--find <description>` | none | also cuts out the moments that best match a description ("a vocal chop", "a snare hit"); repeatable; needs earshot's models |
| `--top` | `3` | cuts per `--find` |
| `-o`, `--out` | `out/chops` | the files go into `<out>/<file name>/` |

[Apple Loops and your own samples](samples.md#chopping-a-song) shows how to use the chops.

### `claudius sounds`

Renders every built-in synthesized sound to WAV files, so you can hear them: the 13 drum voices in `drums/`, the 20
synth presets playing a phrase in `synth/`, and five transition effects in `fx/`.

| flag | default | what it does |
|---|---|---|
| `-o`, `--out` | `out/sounds` | where they go |

## Information

| command | prints |
|---|---|
| `claudius genres` | every genre with its tempo range and description, including your own genre files |
| `claudius reference` | the script language reference, the same text `/api/reference` and the MCP tool `script_reference` return |
| `claudius --version` | the version |

```text
$ claudius genres
deep_house   118-124 BPM  Deep house: softer four-on-the-floor, shuffled hats and shakers, minor 9th chords on electric piano and organ, a round syncopated bass, long warm reverbs
dnb          170-176 BPM  Drum and bass: 174 BPM two-step breakbeat (kick on 1 and the and-of-3, snares on 2 and 4 with ghost notes), a growling reese bass, atmospheric pads, long intro and breakdown
...
```

## Run the studio and the server

These three commands run a session: one song with undo, renders and verdicts, which the studio window, browsers,
assistants and the HTTP API share. They take the same session flags:

| flag | default | what it does |
|---|---|---|
| `--port` | `7878` (`CLAUDIUS_PORT`) | the port the HTTP server listens on |
| `--no-ears` | off | no earshot models; verdicts use meters and theory |
| `--synth-only` | off | synthesizes every sound: no sample packs, Apple Loops or user library |
| `--no-library` | off | no Apple Loops or user library; the sample packs still play |
| `--no-auto-render` | off | renders only when asked, instead of after every change |
| `--web <dir>` | found automatically (`CLAUDIUS_WEB`) | the built web studio; see [Files and settings](files.md#the-web-studio) |

### `claudius studio`

Opens the studio window. It first asks `http://127.0.0.1:<port>` what answers there:

- a claudius: the window opens on that session, says `a claudius is already running on port 7878: opening the studio
  on it`, and ignores the session flags;
- a claudius that asks for a token: it stops, since the window cannot send one, and says to open
  `http://127.0.0.1:<port>/?token=<token>` in a browser or to start another studio with `--port`;
- nothing: it starts a session and an HTTP server on 127.0.0.1 and `--port` (which also serves the web studio and
  MCP), then opens the window on it. A port taken by another program stops it with `port 7878 is taken, by another
  claudius or another program (--port picks another)`.

A `claudius` built without the `studio` feature prints the address to open in a browser and keeps serving.

### `claudius serve`

Runs the session and the HTTP server without a window: the web studio at `/`, the [HTTP API](http-api.md) under
`/api`, and [MCP](mcp.md) at `/mcp`.

```text
$ claudius serve
claudius at http://127.0.0.1:7878: web UI at /, API at /api, MCP at /mcp
```

It prints the address once the port is bound; a taken port stops it with `port 7878 is taken, by another claudius
or another program (--port picks another)`.

| flag | default | what it does |
|---|---|---|
| `--host` | `127.0.0.1` | the address to listen on; `0.0.0.0` listens on every interface. Any address but loopback requires a token and limits file access, as `--public` does |
| `--public` | off | answers requests addressed to any host name (a tunnel, your LAN), requires a token (one is made when `--token` is not given), and limits file access to claudius' own folders |
| `--token <secret>` | none (`CLAUDIUS_TOKEN`) | requires this token on every request; a blank one counts as none |
| `--files <dir>` | none | a folder commands and scripts may read and write, besides `~/Music/claudius`, claudius' caches and the sound libraries; repeatable |

`--public` without `--token` makes a random token and prints how to use it:

```text
$ claudius serve --public --port 7920
claudius at http://127.0.0.1:7920: web UI at /, API at /api, MCP at /mcp (token required)
shared with a new token: open http://127.0.0.1:7920/?token=<token> (through your tunnel, its address with ?token=<token>), or send Authorization: Bearer <token>. --token sets your own.
```

The token is new at every start; pass `--token` to keep one across restarts, as an assistant's configuration needs.
[MCP tools](mcp.md#chatgpt-and-other-assistants-in-the-cloud) shows the tunnel setup and the [security
model](../engineering/security.md) what each flag protects. `--host` with any address but loopback makes a token
the same way, with or without `--public`: browsers on the network are refused by the host-name check, and the token
keeps out programs that send `Host: localhost`.

`--public`, `--host` with an address other than loopback, or `--files` limits file access to `~/Music/claudius`,
claudius' caches and the sound libraries, plus the `--files` folders; without any of them, commands read and write
wherever you can. [What a shared claudius may read and
write](samples.md#what-a-shared-claudius-may-read-and-write) has the details.

### `claudius mcp`

Speaks MCP over stdin and stdout, for assistants that start claudius themselves (Claude Desktop, Claude Code,
Cursor). It first asks the address in `--url`, or `http://127.0.0.1:<port>`, whether a claudius is running there:

- if one answers, every tool works on that session: the song open in your studio;
- if one answers but asks for a token that `--token` did not give (or gave wrong), it fails with
  `Error: the claudius at <url> asks for a token: pass --token or set CLAUDIUS_TOKEN (a wrong one is refused too)`;
- if none answers and `--url` was given, it fails with `Error: no claudius answers at <url>`;
- otherwise it starts a session of its own and serves the web studio on `--port`, so you can open it to listen.

What it decided goes to stderr, which MCP clients show in their logs:

```text
claudius mcp: driving the claudius app at http://127.0.0.1:7878
```

| flag | default | what it does |
|---|---|---|
| `--url` | `http://127.0.0.1:<port>` | a claudius elsewhere |
| `--token` | none (`CLAUDIUS_TOKEN`) | the token of that claudius |
| session flags | | used only when it runs a session of its own |

## Set up and check

### `claudius setup`

Downloads what does not ship with the program: the default sample packs and earshot's listening models. On a
terminal it asks before each download; elsewhere (and with `--yes`) it does not ask.

| flag | what it does |
|---|---|
| `-y`, `--yes` | does not ask |
| `--no-packs` | skips the sample packs |
| `--no-models` | skips the models |
| `--tags` | also downloads the AudioSet tagger, 347 MB more, used by `--tags` |

The prompt shows what is missing and how much it downloads: about 2.2 GB for the default packs, which take 1.2 GB on
disk because several arrive as WAV and are converted to FLAC, and 785 MB for the models (the prompt says about
0.8 GB, or 1.1 GB with `--tags`). An interrupted pack download resumes where it stopped; a model file that was cut off
downloads again from its start, and the files that finished are kept. After the models arrive, the first load spends
about 30 s embedding earshot's vocabulary once.

### `claudius doctor`

Checks the installation and says what is missing.

```text
$ claudius doctor
claudius 0.1.0 (macos aarch64)
  ok  web studio      ~/.local/share/claudius/web
  ok  sample packs    14 default packs (1.2 GB) in ~/Library/Application Support/claudius/packs
  ok  ears            models in ~/.cache/earshot-rs
  ok  Apple Loops     14096 loops and samples
  ok  ffmpeg          9.0.2 (writes song.mp3)
  ok  port 7878       free
  ..  Claude Desktop  not connected: `claudius connect claude-desktop`
  ..  Cursor          not connected: `claudius connect cursor`

Everything claudius needs is here.
```

`ok` is fine, `--` counts as a problem, `..` is optional. The port line says whether `--port` (or `CLAUDIUS_PORT`) is
free, already serving claudius (`claudius is running here`, also for one that asks for a token), or taken by another
program.

### `claudius connect`

Adds claudius to an assistant, or removes it: its MCP configuration, and the [skills](../getting-started/assistants.md#skills)
that teach it claudius. [MCP tools](mcp.md#setting-up-a-client) covers each client.

| argument or flag | what it does |
|---|---|
| `claude-desktop` | edits `claude_desktop_config.json`, and writes the skills as zips into `~/Music/claudius/skills/` to upload in Claude's settings |
| `claude-code` | runs `claude mcp add --scope user ...`, and installs the skills in `~/.claude/skills/` |
| `cursor` | edits `~/.cursor/mcp.json`, and installs the skills in `~/.cursor/skills/` |
| `codex` | edits `[mcp_servers.claudius]` in `~/.codex/config.toml`, and installs the skills in `~/.codex/skills/` |
| `print` | prints the configuration for any other client |
| `--http` | connects over HTTP to a running studio instead of starting `claudius mcp` (not for Claude Desktop or Codex) |
| `--no-skills` | connects the MCP server only |
| `--remove` | takes claudius, and its skills, out again |
| `--port` | the port the `--http` URL uses (`CLAUDIUS_PORT`) |

Every configuration file it edits keeps a `.bak` copy of the old one. Installing the skills replaces only claudius'
own skill folders (`claudius-songs`, `claudius-remix`, `claudius-setup`). An assistant reads its configuration when
it starts, so `connect` ends by saying how to restart it: quit and reopen Claude Desktop, restart Cursor or reload its
window, start a new Codex or Claude Code session.

### `claudius skills`

```sh
claudius skills --out ~/my-project/.claude/skills
```

Writes the skills that teach an assistant claudius into a folder (`claudius-skills` by default): a folder with a
`SKILL.md` for each, and a zip of each for the Claude apps. They are built into the binary, so they match it.
