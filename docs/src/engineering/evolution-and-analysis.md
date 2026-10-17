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
| `onsets.rs` | onset strength as log-compressed spectral flux, optionally in a frequency band; peaks above a moving median; onsets refined to where the energy starts rising |
| `beats.rs` | tempo by autocorrelation of the onset envelope, beats by dynamic programming (Ellis 2007), then a constant-tempo grid and the downbeat |
| `chroma.rs` | pitch-class energy, keys by Krumhansl-Kessler profiles, chords by template matching |
| `song.rs` | the rest: sections, progressions, drum steps, swing, and `to_blueprint` |

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

### Writing a new song on it

`analysis::song::to_blueprint(analysis, genre, seed, ...)` generates a song in the genre at the analysed tempo and key,
then replaces its skeleton with the analysis: progressions `A` and `B` and their chord length, the swing, and the form,
one section per analysed section with its length rounded to whole 4-bar phrases, its energy, and layers chosen from
what was heard (the kick when there was a kick, hat-like lanes with hats, bass and chords by energy, the lead in drops
and breaks). Intros filter in, sections before a drop build with a gap, drops after something else get an impact,
quiet sections use the sparse drum variant. The kick, snare, clap and hat lanes get new patterns drawn from the
analysed templates, and the bass takes the analysed style when the genre has it. The sounds and the mix come from the
genre. `claudius remake` and the `remake` command do exactly this, and the song's history begins with
`remade from <file>: <tempo> BPM, <key>, <n> sections, progression <numerals>`.

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
