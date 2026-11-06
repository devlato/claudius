# Evolution and analysis

Two parts of the engine work on whole songs at once. Evolution (`evolve.rs`) searches for a better song by editing
blueprints the way a producer revises a track and keeping what the critic prefers. Analysis (`analysis/`) reads a
song from audio, so that a new one can be written on its skeleton.

## Mutation

`evolve::mutate(blueprint, genre, library, issues, rng)` returns a changed copy and a sentence saying what changed.
It picks one of 25 operators, applies it, and keeps the result if the blueprint actually changed; otherwise it tries
another, up to 12 times, and as a last resort writes every part again with new seeds (`rewrote every part`).

| level | operators | what one does, in the words it logs |
|---|---|---|
| harmony | `harmony_sub`, `harmony_new` | `harmony A: chord 2: VI -> iv (third relation)`; a new progression from a style of the key's family |
| drums | `drum_edit`, `drum_regen`, `drum_sound`, `drum_kit` | `drums (main): added a hat hit at beat 2.75`, removed, nudged by a sixteenth, or re-accented; a new pattern from the lane's template; a one-shot from the library or a nudge of one synth parameter; every lane onto another kit |
| parts | `part_seed`, `part_style`, `part_preset`, `patch_tweak`, `part_register`, `part_density` | `bass: rewrote the part`; `chords: style stabs -> charleston`; a preset from the genre's list; cutoff, resonance, release, drive or detune of a synth patch; the register moved by 3, 5 or 12 semitones; a lead's density |
| mix | `mix_level`, `mix_send`, `mix_duck`, `mix_pan`, `space` | `mix: bass -1.3 dB (now -1.3)`; a reverb or delay send; a part's sidechain depth; the pan of anything but kick, bass, snare and clap; the reverb's decay or the delay's time |
| form | `form_layer`, `form_bars`, `form_flags`, `form_variant` | `form: lead out of break`; a section 4 or 8 bars longer or shorter (4 to 48); a fill, gap, impact or filter sweep switched; a section's drum variant |
| feel | `tempo`, `key`, `swing` | `tempo 128 -> 126 BPM` within the genre's range; `key A minor -> C minor` (tonal loops follow); swing nudged |
| library | `loop_swap` | another Apple Loop for a loop part |

Every operator stays inside what the genre allows: styles and presets from the genre's lists, tempo within its
range, levels within −12 to +9 dB, pans within ±0.7.

**`drum_kit`** moves every drum lane onto another installed kit: the genre's kits first, then `tr909`, `tr808`,
`tr707`, `muldjord` and `uzu`, never the kit the song is already on. When the song is on a kit, one time in four it
goes back to the synthesized voices instead. It logs `drums: played on the tr808 kit` or
`drums: back to the synthesized voices`.

**Issues steer the choice.** All operators start with weight 1. For each of the verdict's four worst issues, the
operators that address its target are multiplied by `1 + 4 × severity`: a `band:sub+` issue of severity 0.5 makes the
level, drum sound, kit, patch, space and pan operators three times as likely. `mix_level` goes further and, three
times in four, turns the track the band issue points at the right way (too much sub turns the kick or bass down);
the part operators pick a part an issue names seven times in ten.

Every mutation appends its sentence to the blueprint's `lineage`, so a song carries its own history.

## Crossover

`evolve::crossover(a, b)` makes a child from two parents in one of two ways, half the time each: the drums and form of
one with the harmony and parts of the other, or the drums of one with everything else of the other. Both keep a
coherent song, because the harmony and the parts written over it travel together.

## The search

`evolve::run` is a (μ + λ) search: every candidate ever judged stays in the pool, and each generation's parents are
the best `population` of all of them.

```text
initial songs ──judge──▶ pool
                          │  each generation:
                          │    parents = best `population` of the pool
                          │    children: pick a parent (biased to the better ones),
                          │              crossover with another parent (20 %),
                          │              then 1 or 2 mutations steered by the parent's issues;
                          │              skip any blueprint already seen
                          │    judge every child in parallel (rayon) and add it to the pool
                          ▼
                     every candidate, best first
```

- **Previews.** A candidate is judged on its build-up and first drop, not the whole song: `preview_window` takes the
  first drop or hook and up to 16 bars before it, `preview_bars` (32) in all, and the rest of the song is cut away with
  `Project::slice`. A whole song takes seconds to render; a preview a fraction of that, so a generation of eight
  children takes 10 to 40 s with the ears.
- **Duplicates** are skipped by hashing each blueprint's JSON, so the search never judges the same song twice.
- **Parents** are drawn with `min(random(n), random(n))`, which favours the better half without ignoring the rest.
- **Stopping.** A cancel flag is checked before each generation.

Each candidate records its id (`g2-1`: generation 2, child 1), its parent, what changed and its score. The command line
writes them all to `evolution.json` and renders the best in full; the session keeps the best only if it beats the
song it started from.

The studio's **Mutate** (the `mutate` command and MCP tool) is a single generation without crossover: `count`
children of the current song, one or two mutations each, steered by the current song's issues, judged on the same
32-bar previews, and the best kept if it beats the current song's preview score.

## Reading a song

`analysis::song::analyze(audio, tempo_range)` reads a whole mixed song the way a producer does before remaking it:
tempo, downbeat, key, the chord of every bar, sections with their energy and what plays in them, drum habits per
sixteenth, swing and a bass style. It uses no models: everything is signal processing in `analysis/`.

| file | what it does |
|---|---|
| `onsets.rs` | onset strength as log-compressed spectral flux, optionally in a frequency band; peaks above a moving median; onsets refined to where the energy starts rising: the first millisecond above a tenth of the loudest, from a hop before the peak frame's window to a hop after it (log flux peaks when an attack first comes into the window, at its end) |
| `beats.rs` | tempo by autocorrelation of the onset envelope, beats by dynamic programming (Ellis 2007), then a constant-tempo grid and the downbeat |
| `chroma.rs` | pitch-class energy, keys by Krumhansl-Kessler profiles, chords by template matching |
| `melody.rs` | a [take](../guide/samples.md#takes)'s melody as notes, read by `voice/` ([below](#hearing-a-melody)) |
| `song.rs` | the rest: sections, progressions, drum steps, swing, and `to_blueprint` |
| `groove.rs` | the [groove](../language/song-functions.md#groove) of a recording's drums ([below](#hearing-a-groove)) |
| `drums.rs` | a beat's hits, played or beatboxed, named kick, snare or hat and folded into lanes ([below](#hearing-drums)) |

**Tempo.** The onset envelope adds the full-band flux to twice the kick-band flux (30 to 160 Hz), because in dance
music the kick marks the beat. Its autocorrelation, weighted by a prior centred on 120 BPM (or on the middle of the
range a genre gives) and helped by the autocorrelation at twice the lag, gives the beat period. Beats tracked by
dynamic programming drift and insert beats through breakdowns, but music made in a DAW keeps one tempo, so the
tracker then fits a constant grid, trying every tempo within 0.5 % in steps of 0.005 BPM and every phase, and keeps the
one that lands on the most onset energy.

**Downbeat.** Of the four beat phases, the downbeat is the one with the best mix of three cues, each scaled to its
best phase: a change of harmony across the beat (chroma a bar before against a bar after, weight 1.5), the biggest
level jumps (sections start on downbeats, 0.75), and kick strength (0.5). Harmony leads because level jumps also come
from pre-drop gaps and risers, and kick strength from rolling basses.

**Key and chords.** Chroma comes from FFT bins close to an equal-tempered pitch, so drums and noise smear less into
it. The key is chosen among the four best Krumhansl-Kessler fits of the loudness-weighted average chroma, by its fit
plus how many bar chords it explains diatonically, how often its tonic chord appears, and how much bass sits on its
tonic. Each bar's chord is matched against triad and seventh templates, with a pull towards the key's diatonic
chords, using a bass chroma read in the second half of each of the first three beats, where the kick has decayed and a
sidechained bass has swelled back.

**Sections and energy.** The song is cut into 4-bar blocks described by their loudness, low and high band levels and
chroma; a boundary goes where neighbouring blocks differ by more than the mean plus half a standard deviation, at least
8 bars after the previous boundary and not in the last 4 bars. Each section's energy is its loudness scaled between
the quietest and loudest sections, from 0.3 to 1. Kinds follow from energy and position: the first section is an
intro and the last an outro unless they are loud, anything at 0.85 or more is a drop, a section of at least 0.45
before a drop is a build, a section without kick or under 0.5 is a break, the rest grooves.

**Progressions.** In the loudest drop, the chords are tested for repeating every 2, 4 or 8 bars, and the cycle that
agrees most becomes progression `A`; when the chords come in pairs, it becomes one chord per two bars. Progression `B`
is read the same way from the longest break or intro.

**Drums, swing and bass.** The kick, snare and hat onset envelopes (by band) are sampled at every sixteenth of every
bar in the sections with a kick and energy of at least 0.6, averaged, and turned into lane templates: a step that hits
in almost every bar becomes certain, weaker steps become probabilities. Swing is the median position of hat onsets in
the second half of each eighth. The bass style is guessed from the kick's step profile: `rolling` when ten or more
sixteenths are busy, `offbeat` when the off-beat eighths carry more than 55 % of the on-beat weight, `sustain` when at
most two are busy, `syncopated` otherwise.

### Hearing a groove

`analysis::groove::hear(audio, bpm)` measures where each sixteenth of a recording's bar lands and how hard. Everything
rests on the grid, and a groove measured against the wrong sixteenths means nothing, so a tempo that is known (given,
an Apple Loop's in GarageBand's catalogue, or `92bpm` in the file's name) is only tuned, within 3 %. Otherwise the
song analysis guesses one, which on a short loop is often off by 3:4 or 4:3 or a few percent (on twelve of Apple's
drum loops it was right for four), and the drums' onsets choose:

- **The tempos tried.** The guess and its octaves, and for a recording of 40 seconds or less the whole bars it lasts
  (half a bar, 1, 2, 4, 8 or 16), which give a loop's tempo exactly. Less likely, and winning only when they fit
  clearly better: 3:4, 4:3, 2:3 and 3:2 of the guess, and 3, 6 or 12 bars. Over a bar or two a guess tuned freely fits
  anything near, so in a recording of 12 seconds or less one within 3 % of an octave of the length is dropped.
- **How well each fits.** Each is tuned in steps of 0.01 % to where the onsets sit nearest its sixteenths: the mean
  squared distance, in sixteenths, weighted by loudness, with the grid's phase where the distances balance. The on-beat
  and the off-beat sixteenths are measured each from where they land on average, so a swing costs nothing; otherwise
  a loop swung at 62 % fits the grid of triplets half as fast again better than its own.
- **The tempo.** The slowest that fits about as well as the best (a grid twice as fine always fits at least as well).
- **The downbeat.** The beats are the sixteenths of four that the loud onsets fall on, and the downbeat is the beat
  with the most energy under 120 Hz.

On fourteen of those loops this finds the tempo of nine of the thirteen it can read (one has too few hits). Three of the
others come out an octave off, which is where the drums sit on sixteenths either way. Telling which octave a drummer
counts takes knowing a snare from a kick, which the onsets alone don't. With the catalogue's tempo, all thirteen are
right.

Each onset is then set against the sixteenth nearest it. Each of the sixteen positions takes the median of its onsets'
distances from the grid (a position with fewer than two keeps no timing), clamped to 0.12 beats, and their mean loudness
against the recording's, clamped to 0.4 to 1.3, as its accent. The beats are put back on the grid by the mean of their
timings, so a downbeat found a little early or late doesn't move the whole groove.

### Hearing drums

`analysis::drums::hear(audio)` names the hits of a beat, played on a kit or beatboxed, and `fold` sets them on a grid
(a take's, from its first beat at the song's tempo; a file's, from `groove::beat`) and folds the times round into a
pattern of 1, 2 or 4 bars.

- **The onsets.** Those of the whole sound and of its top over 5 kHz, which finds a quiet hat under a kick's tail,
  merged within 25 ms (a kick's body arriving after its click is the same hit).
- **What each adds.** Every band at its strongest in the 50 ms after the onset, in 20 ms windows (a period of the
  lowest kick), less its level in the 20 ms before: the tail of the hit before is not part of this one. Of that, the
  shares under 150 Hz, in a drum's body (150 to 800 Hz) and over 5 kHz. An onset that doubles none of the whole
  sound, its low band or its high band is a ripple inside a sound, and one 30 dB under the loudest hit is breath or
  the room.
- **Naming.** Three clusters of those shares, started amid the quarter of hits that lie most low, most in the middle
  and most high, so a stray sound can't take one. Each is named by where its middle lies, and kept only if that is
  near its kind: a kick's 0.12 under 150 Hz, a hat's 0.2 over 5 kHz with next to nothing in the body, a snare's 40 %
  between. Otherwise the recording lacks that kind (a loop of kicks alone still makes three clusters), and its hits
  are named alone: a quarter under 150 Hz is a kick, a half over 5 kHz (or a fifth, with nothing in the body) a hat,
  the rest a snare. Those thresholds come from the shares of about 2,400 hits in GarageBand's single-drum loops.
- **A second sound.** A hit that adds to another kind's band far more than hits of its own kind do there (a quarter
  of them add less) is that kind too: half what that kind's own hits add, or, when every one of them lies under
  other hits (open hats on the kicks), three times what its own kind adds.
- **Folding.** A step hit in at least three quarters of the times round is a hit; one hit in a quarter to three
  quarters plays with that chance (a trig's `prob`). Its velocity is its median level against the loudest of its kind,
  to the power 0.6.

Measured on kits, against 15 mixes of a kick, a snare and a hat loop from GarageBand at one tempo (each loop's own
onsets the truth): kicks 89 % precision and 95 % recall, snares 68 % and 56 %, hats 91 % and 59 %. The hits missed
are mostly quiet hats under other hits and snares layered with a sub, which add as much under 150 Hz as a kick does.
On beatboxing, against 20 four-bar beats built from the recordings of a published beatbox dataset (Mehta and others,
2022; `maxardito/beatbox` on Hugging Face), one performer's kick, snare and hat each, hits are found nearly all the
time (98 to 100 % recall). The kicks are the weak kind, at
49 % precision: a beatboxed snare's release, 12 to 27 dB under it and a sixteenth later, is heard as a kick, and
telling it from a ghost note a sixteenth after a hit would take more than level. Snares are at 65 %, hats at 87 %.

### Writing a new song on it

`analysis::song::to_blueprint(analysis, genre, seed, ...)` generates a song in the genre at the analysed tempo and key,
then replaces its skeleton with the analysis: progressions `A` and `B` and their chord length, the swing, and the form,
one section per analysed section with its length rounded to whole 4-bar phrases, its energy, and layers chosen from
what was heard (the kick when there was a kick, hat-like lanes with hats, bass and chords by energy, the lead in drops
and breaks). Intros filter in, sections before a drop build with a gap, drops after something else get an impact,
quiet sections use the sparse drum variant. The kick, snare, clap and hat lanes get new patterns drawn from the
analysed templates, and the bass takes the analysed style when the genre has it. The sounds and the mix come from the
genre. `claudius remake --skeleton` and the `remake` command with `sampled: false` do exactly this, and the song's
history begins with `remade from <file>: <tempo> BPM, <key>, <n> sections, progression <numerals>`.

### Reworking it

A remake is a rework unless asked otherwise (`remix::rework`), which keeps the recording the song it is. The recording
is split into what sustains and what strikes (`fft::harmonic`), and what sustains, from its first downbeat to the end
of its last bar, is one loop track, `original`, that plays once from the first section through all of them (a take's
`from`): its voice, chords and melodies as they are, through the genre's treatment of a phrase it samples with a
high-pass at 150 Hz at least, which leaves the low end to the genre's bass and kick. The song generated in the genre
at the recording's tempo (`rework_tempo`: from the length of its bars, doubled or halved toward the genre's range, so
that the loop is not stretched) and in its key then takes the recording's skeleton exactly: one section per analysed
section, as long as it was heard, its kind and energy, the layers of the genre's section most like it (of its kind, else
the nearest in energy) that play under a recording (drum lanes, the bass, the break and the textures, not chords,
keys, pads or leads, which would play against its own and against it where a chord was heard wrong), a progression per run of its bars' chords at one chord a bar, and the recording's swing. Parts
the sections do not play are left out. The history says `reworked from '<file>': all <n> bars of it, ...`.

### Flipping it

With `flip`, a remake is a new song on the recording's sound (`remix::remix`): the recording is split into what
sustains and what strikes (`fft::harmonic`), and cut into the pieces a producer would take from it, on the analysis's
bar grid. Its hook
(four bars from the start of the loudest section with chords, a hook or a drop first) without its drums is offered to
the sampling pipeline as the phrase a song is built on (`sampling::apply`, through a library that holds only the
recording's pieces and the libraries' one-shots, loops and textures), with another passage of it as the phrase's second
passage and its drums as the break where the genre samples one. So the song takes the hook's key and chords and a tempo
the genre plays, the recording's own or a multiple of it (`sampling::meets`). A flip of it follows: as many bars as
the hook of half beats, each from another bar of the recording on the hook's chord at that place, in the intro, the
breaks and the outro where the other passage does not play. Then `sampler::sample` brings in its sung phrases. A
recording with no section with chords long enough falls back to the skeleton, and the reply says so. The song's history
says what came from where: `remixed from '<file>': its hook (bars 33-36) without its drums; ...`.

### Where it goes wrong

Analysis is best on steady electronic music with a clear kick, and approximate everywhere. Reading claudius' own D
minor deep house song back:

```text
118.0 BPM (pulse clarity 0.55), Bb major (confidence 0.43), swing 0.50, 52 bars from 0.00 s
  progression A: iii7 Imaj7 vi7 vii  |  B: Imaj7 Imaj7  (1 bar(s) per chord)
```

The tempo and the first downbeat are exact. The chords of `A` (Dm7, B♭maj7, Gm7, Am) are the song's own (Dm7,
B♭maj7, Gm7, Am7) in the right order, named in B♭ major, the key of the song's sixth degree, with the seventh of the
last one missed. Progression `B`, read from a 12-bar break, came out as two bars of B♭maj7, the first of the song's
four chords. The usual failures are these: half or double tempo (a genre's tempo range fixes most), a relative or
neighbouring key, a progression rotated when the first downbeat lands a beat or a bar off, and sections merged when
their energy is close. `CLAUDIUS_DEBUG=1` prints the downbeat cues per phase.

## Hearing a melody

`voice/` reads a voice, one note at a time: its pitch, the notes it means, and how its singer sings.
`analysis::melody::transcribe(audio, key, options)` uses it to turn a take's melody into notes for `take::melody`
(the `transcribe_take` command), and `voice::assess` and `take::assess` (`assess_voice`) to say how it is sung.

- **Pitch** (`voice/pitch.rs`). The take is mixed to mono, low-passed twice at 3 kHz and decimated to 8 kHz, which
  leaves a voice's fundamental and its first harmonics. YIN (de Cheveigné and Kawahara 2002) reads a pitch from 50 Hz
  to 1 kHz every 10 ms, over a 50 ms window: the first dip of the cumulative mean normalised difference under 0.15,
  refined between samples by a parabola, and its depth is the frame's clarity. A frame more than 35 dB under the
  take's loudest is unvoiced, whatever its pitch, and a frame more than four semitones from the median of its five
  neighbours (an octave or a harmonic jumped to) takes the median. Each voiced frame's period is then found again at
  48 kHz, within 3 % of YIN's, as the lag that differs least from itself over 20 ms, which reads a held pitch to a
  cent. `pitch::rough` skips that, for the sampler's look at whole songs.
- **Where the notes are** (`voice/intent.rs`). A hidden Markov model over the notes the take may be singing (pYIN's
  note tracker in Tony, Mauch et al. 2015, works the same way): each note has an attack state, which reaches a
  semitone or so for scoops and slides and costs half its likelihood, and a hold state, read from the pitch's median
  over 150 ms, which keeps a note's edges and loses its vibrato; silence is a state of its own. A frame without a pitch
  is likelier in silence (0.9) than in a note (0.1), and a rise of more than 3 dB within 30 ms speaks for an attack.
  A hold is left at 2 % a frame, an attack settles at 20 %, and silence starts a note at 3 %. Viterbi gives the
  states; a note runs from where the path enters its attack to where it leaves the note or attacks it again, and
  notes under 70 ms are dropped. A note is settled from the first of three frames within 30 cents of its median, and
  its place is the median after that.
- **Which note each one is.** A singer puts a note off by so much for the whole of it, so frames, which would make
  any small error certain, do not decide it. A chain over the notes does: each note is read once, from its place, as a
  normal around where this singer puts each candidate (`Voice::bias_at`) with their scatter (`Voice::spread_at`), and
  a note follows the one before by the song's weights (1 for a tone of the key, 1.6 for a tone of the chord under the
  note, 0.12 for any other) and the step (0.8 for the same note, falling by e for every 5 semitones beyond the
  first); after a silence of more than a second the step counts for nothing. Viterbi names the notes and
  forward-backward gives each its `sure`. The wider the singer's scatter, the further the song moves the line between
  two notes: in C major, from anyone's 30 cents, a note is read as F up to about 70 cents above it.
- **The singer** (`voice/singer.rs`). `fit` learns a `Voice` from notes heard against the notes meant. Notes 0.75
  semitone or more off were another note, missed, and do not count. Where they sing is a least-squares fit of three
  values, at the bottom, the middle and the top of the range, interpolated between, each leaning on in-tune singing by
  a tenth of a note; how widely is the scatter left around that at each place, leaning on anyone's 30 cents by half a
  note. The settling is the median, and the vibrato the median of the notes of 400 ms or more whose detrended pitch
  swings 4 to 8 times a second by 5 cents or more (an autocorrelation peak over 0.3). `Voice::hit` is the chance of a
  note within so many cents, from that normal.
- **Learning from the singing alone.** `voice::assess` reads the notes with anyone's voice, fits the singer to them,
  and reads them again with that voice, twice. A take is read with the song's key and the chords under it from where
  it starts (`intent::Scale::song`).
- **The grid.** Each note's start and end go to the nearest sixteenth, measured from the middle of the frames' windows.
  A note in the song's key keeps its pitch; one outside it goes to the key's tone nearest where it was sung (unless
  `chromatic`), and its velocity follows its loudness. Two notes on one sixteenth keep the longer, and a note ends where
  the next begins.

The notes go into the new parts' `notes`, by the progression of each section the take covers. Written notes may run
over several cycles of their progression, and `arrange` repeats them only after the last cycle they reach. On the
way, `take::styled` can make them harmony, octaves, chords, a bass line, an arpeggio or a pad. It counts by the
key's scale steps (`Key::step`), so a third above or a triad under a note stays in the key.

### Checking a voice

`voice::check::Check` is a pitch check: six phrases of four notes in a random major key between two notes, each a
random walk on the scale, a step of one or two degrees seven times in ten and a leap of three or four otherwise, at 90
BPM, a beat a note. A phrase takes ten beats: its four notes, a beat's rest, four beats to answer in, a beat's rest.
`Check::hear` reads each answer from 200 ms before its first beat to the next phrase, with anyone's voice in no key
(what was sung, not what should have been), and lines the notes up with the phrase's by the least cost: a note sung
for a note played costs how far off it was (a semitone and a half at most), a note not sung 1.2, a note that answers
none 0.8. It tries the phrase two octaves down to one up and keeps the octave that costs least, and fits the voice
(`singer::fit`) to the notes as meant in the singer's octave. A note reached by three semitones or more is a leap.

### Tuning a voice

`voice::tune::tune` moves a voice onto the notes it means, as the `tune` effect, `claudius tune` and the `tune`
command do. It reads the notes (`intent::decode` with the singer given, or `voice::learn` from the voice itself) and
works out a move for every frame, in semitones:

- **Towards the note.** A frame's centre is the mean pitch of the frames around it within 90 ms, inside its note,
  which takes out a vibrato; frames more than 1.5 semitones from where the note is held (a slide, an octave misheard)
  count for nothing and move as the frames around them. Of how far the centre is from the note, the singer's bias goes
  whole; of the rest, a share `humanize² / spread²` stays (all of it when the singer scatters less than `humanize`),
  so the tuner moves a loose singer in and leaves a precise one.
- **When.** The move comes in as `1 - e^(-t / speed_ms)` from the note's start, so a slow tuner leaves the scoop into
  a note, and `strength` (a `Param`, by beat) scales it.
- **The vibrato.** `vibrato` below 1 takes that share of the swing around the centre out as well.
- **Between notes,** the move goes in a straight line from one note's to the next, and a three-frame mean smooths it.

The voiced stretches (voiced frames, with gaps of up to two) are then moved by TD-PSOLA (Moulines and Charpentier
1990, `dsp/psola.rs`). Marks go a period apart, each where the voice looks most like it did a period before (the
best normalised correlation within a sixth of a period), so that every grain is cut at the same point of its period.
Each output mark takes the grain at the input mark nearest it, two periods long with Hann halves as long as the
periods either side, and the next output mark comes the input period on, over the ratio. Laid back where they were
cut, the grains add up to the signal itself; moved, they are divided by how much of them overlaps. The first and the
last period of a stretch are crossfaded from and back to the signal, and frames without a pitch pass as they are.
The grains keep their spectra, so the voice keeps its formants. A move by the same grains, resampled, would shift the
formants instead, which is how a voice can later sound larger or smaller.

The tests sing to it (`voice::sing`): three harmonics with a 5.5 Hz vibrato, breath noise, a 30 ms glide into each
note and breaths where asked. Their melodies come back with every pitch and onset where it was sung, a singer 60 cents
flat comes back as the notes meant once the voice is known, and a singer flat at the top only is learnt as flat there.
