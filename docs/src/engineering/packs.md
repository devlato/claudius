# Sample packs

`crates/claudius-packs` turns freely licensed sample libraries, published in a dozen different layouts, into
instruments the engine can play by name. It is a library (used by the engine, the server and the CLI) and the
`claudius-packs` binary. [Sample packs](../guide/packs.md) is the user's side; this page is how it works.

| file | what it does |
|---|---|
| `registry.json`, `catalog.rs` | the curated packs: what each contains, its licence as read from the source, how to fetch and map it |
| `install.rs` | builds a pack's index from its registry entry (sources, key-split composites) and installs it; removes packs |
| `install/from_sfz.rs`, `install/from_names.rs`, `install/from_map.rs` | instruments from SFZ files, from file names, from Strudel maps |
| `fetch.rs` | HTTP: GitHub listings, parallel resumable downloads, archives, WAV to FLAC |
| `sfz.rs` | an SFZ reader |
| `strudel.rs` | Strudel sample maps and the `github:` shorthand |
| `notes.rs` | note names, velocity and round-robin markers in file names, drum name synonyms |
| `model.rs` | the installed index format (`index.json`) |
| `resolve.rs` | names to instruments: aliases, banks, kits |
| `decode.rs` | decoding at the file's own rate, and the process-wide sample cache |
| `resample.rs` | band-limited interpolation for pitch and rate |
| `player.rs` | rendering notes with an instrument |
| `lib.rs` | the public API: `packs`, `install`, `remove`, `resolve`, `instrument`, `credits_for`, `add_sample_map` |

## The registry

`registry.json` is embedded in the binary. Each pack records:

- what it is (`title`, `description`, `categories`) and what was left out for size;
- its licence (`licence`, `licence_url`), the credit line exactly as it should appear (`attribution`), whether the
  licence requires it (`attribution_required`), and `licence_note`: what the licence text says and where it was read,
  quoted, so a reviewer can check it without re-reading the source;
- whether it is in the default set, `opt_in` with a `warning`, or `lazy`;
- where it comes from (`fetch`): a GitHub repository at a pinned commit, or an archive URL with a prefix to strip and
  the archive's `size` and `sha256`;
- `flac`, to convert WAV downloads losslessly, and bank `aliases` (`909`, `rolandtr909`);
- its instruments, each built from one of four kinds of source.

| instrument source | used for | the registry entry says |
|---|---|---|
| an SFZ file | pianos, orchestra, basses, the acoustic kit | the `.sfz` path; optionally `keep_layers` (velocity layers to keep, widened to cover every velocity), `keys`, `exclude`, an `octave` correction for mappings at written pitch, `kit` (drum name to keys) or `kit_auto` (one variant per key range) |
| file names | the drum machines | a glob (`bd8/*.WAV`), `each` file its own variant, `first` the file for variant 0, or `vel`, a regex whose group is the velocity layer (the 909's `accent(\d+)`) |
| a Strudel map | uzu, the lazy opt-in packs | the map's URL at a pinned commit, keys to leave out, and `map_base` when the map's `_base` names a branch of another repository |
| other instruments | ensembles | `compose`: key ranges taken from other instruments (strings: contrabass below MIDI 36, cellos to 57, violas to 64, violins above) |

Instruments also carry playing settings: `release`, `attack`, `veltrack` (how much velocity changes the level),
`one_shot` (play to the end whatever the note length), `mono` (a new note cuts the last, as on drum machines),
`retrigger` (a repeated key fades the previous voice, as on a piano), `unison` (the choir is one sampled voice in a
five-voice ensemble), and `aliases`.

## Installing

`install::install_spec` builds the pack's `PackIndex` and fetches its files:

1. **Sources.** A GitHub source is read from `raw.githubusercontent.com` at the pinned commit; when an instrument is
   given as a file glob, the repository is also listed through GitHub's tree API, which gives each file's size. For
   an archive it downloads the file into the cache, checks its size and SHA-256 against the registry, unpacks it
   with the system's `tar` (bsdtar reads zip, 7z and tar; GNU systems fall back to `unzip` and `7z`), deletes any
   symbolic links in it, and strips the prefix. A Strudel map's `_base` on a branch of the map's own repository is
   moved to the map's commit (`strudel::pin_base`), and every file of a curated map must be at a commit.
2. **Instruments.** SFZ files are read with `sfz.rs`: `#define` and `#include` (including mid-line includes, which the
   sfzinstruments mappings use), the `<control> <global> <master> <group> <region>` inheritance chain, and default
   controller values; `install/from_sfz.rs` turns each region into a `Zone`. File-name sources
   (`install/from_names.rs`) are parsed with `notes.rs`, which reads note names (`C4`, `Db2`, Tidal's `As4`),
   velocity and dynamic markers, and round-robin markers out of names. Strudel maps go through
   `install/from_map.rs`, and `compose` instruments are assembled from the others in `install.rs`.
3. **Files.** Every file the zones use gets a path inside the pack (`files/...`, sanitised so `..` cannot leave the
   pack folder) and a download URL, or a copy from the unpacked archive. Files no zone uses are dropped, and files on
   disk the new index does not reference are deleted, so updating a pack also cleans it.
4. **Downloads** run on 8 threads. Each goes to `<file>.part`, resumes with an HTTP range request when a `.part` is
   already there, is checked against the expected size when the listing gave one (and against `Content-Length`
   always), and is renamed into place, so an interrupted install leaves
   only `.part` files and resumes on the next run. A Git LFS pointer is recognised and fetched again from
   `media.githubusercontent.com`. Failures are retried three times with backoff, except 4xx answers other than 429.
5. **FLAC.** With `flac`, a WAV is encoded losslessly with `flacenc` as it arrives (float WAV becomes 24-bit).
6. **The index** is written to `index.json.tmp` and renamed, so a pack is either installed or not.

A lazy pack stops after step 3 and records each file's URL in its index; the player downloads a file the first time a
note needs it. `add_sample_map` builds a lazy user pack from any Strudel map, taking the licence from the map's
`_license` or GitHub's licence detection.

## The index

`<packs>/<pack>/index.json` is a `PackIndex`: the pack's metadata, its `files` (path, URL, size, whether to convert
to FLAC), and its `instruments`. An instrument has one or more `variants` (the `:2` in `tr808:sd:2`), and a variant is
a list of `Zone`s:

| zone field | meaning |
|---|---|
| `file`, `root` | the sample and the MIDI pitch it was recorded at (fractional for detuned regions) |
| `lokey`..`hikey`, `lovel`..`hivel` | the keys and velocities it answers |
| `xfin`, `xfout` | velocity crossfades, with SFZ's power curve so overlapping layers keep constant power |
| `seq_len`, `seq_pos` | round robin: the cycle length and this zone's place in it |
| `lorand`, `hirand` | random alternatives |
| `gain_db`, `pan`, `offset`, `end`, `keytrack` | level, balance, sample range, how much the pitch follows the key |
| `loop_mode`, `loop_start`, `loop_end` | sustain loops |
| `attack`, `hold`, `decay`, `sustain`, `release` | the amplitude envelope, when the zone overrides the instrument's |

The installed index is the only thing the player reads, so a pack can be built by hand: `tests/offline.rs` writes a
pack of sine waves and an index, and plays it.

## Names

`resolve::resolve_in(installed, name, bank)` turns a name into an `InstrumentRef` (pack, instrument, variant), in this
order:

1. with a bank, the bank's pack (by name or alias) and the instrument in it, by name, alias or drum synonym; or a
   Strudel-style instrument named `<bank>_<sound>` in any pack;
2. `pack:instrument[:variant]`, or `bank:sound` when the first part is a Strudel bank;
3. friendly aliases (`ALIASES`), each a list of candidates in order of preference, the first installed one winning:
   `piano` is `salamander:piano`, else `splendid:piano`, else `freepats:upright_piano`, else `piano-lite:piano`;
4. bare drum names, in the first kit of `KIT_PREFERENCE` (`tr909`, `tr808`, `muldjord`, `tr707`, `uzu`) that has
   them;
5. exact instrument names and per-instrument aliases, default packs first, then user packs, then opt-in packs;
6. a drum synonym in any pack, a `Bank_sound` name, and finally a pack name alone (its first instrument).

`notes::drum_canonical` maps drum synonyms to canonical names (`kick`, `kd`, `bassdrum` to `bd`), which is how
`s("kick").bank("808")` finds the 808's `bd`. Resolution is cheap and runs every time a script names a sound; the
installed indexes are cached per process (`refresh()` forgets them). `set_disabled(true)`, which `--synth-only`
calls, makes `installed()` return no packs, so every name falls through to the synthesizer.

## The player

`Instrument::render(notes, len)` renders `NoteOn`s (start and gate in output samples, a fractional pitch, a velocity)
into a 48 kHz stereo buffer:

- **Zone selection** (`select`): the zones that cover the key and velocity, with round robin counted per key (so
  repeated notes alternate samples) and random alternatives drawn from a hash of the note's start, index and key, not
  a random stream, so the same notes always give the same audio.
- **Pitch** is `(pitch − root) × keytrack` semitones. The source's sample rate is folded into the same step, so a
  44.1 kHz sample played a fifth up is interpolated once, not resampled and then pitched. Interpolation is a
  Kaiser-windowed sinc with 16 zero crossings a side, widened when reading faster than one source sample per output
  sample, so pitching up filters out what would alias.
- **Level.** Velocity picks layers and, through `veltrack`, scales the level as `(v / 127)^(2 × veltrack)` (or a
  `vel_curve`). Each instrument is normalised once, so that the loudest zone nearest middle C peaks at −6 dBFS at
  full velocity; that keeps instruments from different libraries at comparable levels before the mixer balances them.
- **Envelope and loops.** Attack, hold, decay, sustain and release, with sustain loops for zones that loop while the
  key is held. `one_shot` instruments ignore the gate.
- **Cuts.** `mono` instruments fade the previous voice in 4 ms when a new note starts; `retrigger` instruments fade a
  repeated key's previous voice over 120 ms; `render_choked` cuts every voice sounding at a choke time in 8 ms (the open
  hat under the closed hat).
- **Unison.** An instrument with `unison` plays every note as several copies, detuned, panned and staggered by up to
  `delay_ms`, at 1/√n gain each: the choir is the solo voice five times, ±14 cents, 35 ms apart at most.
- **Speed.** Voices render in parallel batches of 64 and are summed in a fixed order.

Decoded samples come from the cache in `decode.rs`: WAV through hound (keeping its loop points), everything else
through `claudius-decode`, at the file's own rate, with the silent tail trimmed and a short fade. The cache is shared
by the whole process and bounded by `CLAUDIUS_PACKS_CACHE_MB` (3072): over budget, the least recently used samples
that no voice holds are dropped until it is at 80 %. Concurrent requests for one file wait for a single decode.

## Credits

`credits_for(names)` resolves the instrument names a song plays, collects their packs, and writes the packs that
require attribution first, then the rest as courtesy credits, then a warning for packs with an unknown licence. A
credit line is the pack's `attribution`, its licence when the attribution does not already name it, and the licence
URL. The engine calls it with the instruments of a project's tracks, so a song credits what it used and nothing else.

## Checking a pack

`claudius-packs audition` renders every instrument (an arpeggio and a held chord for pitched instruments, a few hits
for drums, a beat for every kit with `bd`, `sd` and `hh`) and checks each pitched one for octave mistakes in its
mapping: a correctly mapped harmonic sound has nothing at half its fundamental, and its fundamental is not tens of dB
below the octave above. That is how a mapping at written pitch shows up; the registry corrects the ones it knows with
`octave` (the piccolo, xylophone and glockenspiel, the growly bass, the solo voice, the drawbar organ). Tubular bells
are skipped, since a chime has no partial at its nominal pitch.

## Adding a pack

1. Read the licence where the samples are published, and quote it in `licence_note`. A pack enters the default set
   only if its licence allows making and publishing music with it without conditions beyond attribution;
   [0007 Which samples we ship](../design/0007-sample-licensing.md) has the rules and the cases that were refused.
2. Pin the source: a commit for GitHub (`rev`); for an archive its `size` and `sha256`
   (`shasum -a 256 <file>`); for a Strudel map, its URL at a commit, and `map_base` at a commit when its `_base` names
   a branch of another repository. `every_download_is_pinned` checks the registry.
3. Write the instruments, start with `claudius-packs install <name>`, and listen with
   `claudius-packs audition /tmp/audition --pack <name>`, fixing the mapping until the audition reports no problems.
4. Check the names: `claudius-packs resolve` and `aliases`, and whether the new pack should be a candidate in
   `ALIASES` or `KIT_PREFERENCE`.
5. Run `cargo test -p claudius-packs`, and the network test with `cargo test -p claudius-packs -- --ignored`.
