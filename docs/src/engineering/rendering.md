# Rendering, mixing and mastering

`render::render(project, options)` turns an arranged project into a mastered stereo file. It works the way a mix
engineer does in a DAW, in a fixed order, and everything it does is decided by the project: there is no hidden state
and no randomness that is not seeded from the project's seed and the track's name.

Everything inside claudius runs at 48 kHz (earshot's rate) on planar stereo `f32` buffers (`audio::Stereo`).

```text
 per track, in parallel (rayon)
 ┌──────────────────────────────────────────────────────────────────────────────────────────┐
 │ notes / clips ──▶ source ──▶ insert chain ──▶ gain to the role's target ──▶ sidechain ──▶ pan │──┐
 └──────────────────────────────────────────────────────────────────────────────────────────┘  │
        ┌────────────────────────────────────────────┬───────────────────────────────────┘
        ▼                                            ▼ sends (reverb, delay)
  bus: drums │ bass │ music │ fx                  returns: convolution reverb, ping-pong delay
  (each bus chain, in parallel)                   (wet only, ducked 30 % by the kick)
        └──────────────────────────┬─────────────────┘
                                   ▼
                        sum ─▶ premaster LUFS ─▶ master chain ─▶ finalize (gain search,
                                                                 soft clip, limiter, trim)
                                                                         ▼
                                                    master, trimmed after its tail, 0.5 s fade
```

## The length of a render

A render buffer is the song's length plus 3 s of tail, plus the longest tail of any track's insert chain (a reverb's
decay, a delay's echoes until they fall 60 dB), capped at 6 s. At the end, the master is cut where it falls below
−80 dBFS for good (never shorter than the song) and faded out over 0.5 s. A project over 512 bars or 20 minutes, or
with more tracks than `model::check_tracks` allows, is refused before anything is allocated.

## Sources

Each track plays its notes or clips through its `Sound`:

| sound | how it plays |
|---|---|
| `drum` | the voice is rendered once from its parameters and cached for the process, keyed by the sound, the pitch and the seed; other pitches are resampled from it like a sampler |
| `sample` | the file is decoded once (and cached on disk, see [Files](../guide/files.md#caches)), sliced to `start`..`end`, faded, and played like a drum, pitch relative to `root` |
| `synth` | the [synth engine](#the-synth-engine) renders the notes |
| `instrument` | the sample-pack player renders the notes ([Sample packs](packs.md#the-player)) |
| `fx` | each note renders a transition effect of the note's length: `riser`, `downlifter`, `impact`, `reverse_crash`, `sweep_up` or `sweep_down`; risers are tuned to the key's tonic |
| `loop` | the loop is [warped](../guide/samples.md#warping) to the tempo and key, then each clip copies it in, re-sliced by `chop` when set, with fades of at least 1 ms in and 2 ms out |

Details that make it sound played rather than printed:

- **Swing** moves every off-beat sixteenth late when notes are turned into sample positions, so the blueprint stays
  on the grid and one number changes the feel of every part.
- **Humanisation** adds timing jitter with a standard deviation of `humanize` × 4 ms (× 0.6 for parts, so drums
  carry the looseness) and velocity jitter of ±5 % × `humanize`, from a generator seeded by the track's name.
- **Velocity** of a one-shot scales its level by `velocity^1.6`.
- **Choke groups** stop a ringing hit at the next onset of any track in the same group, with a 4 ms fade (the open
  hat stops when the closed hat plays). Pack instruments get the same cut times.
- **Samples on melodic roles** stop at the note's length plus 50 ms, so `legato()` shapes them; on drum roles they
  play to the end.

## The auto-mixer

A producer balances by ear: the kick here, the bass a little under it, the pad well back. claudius does the same with a
meter. After its inserts, each track's **active level** is measured: the 80th percentile of its short-term loudness
(3 s blocks every 0.5 s) over the blocks where it is audible (above −50 LUFS), falling back to momentary blocks
(0.4 s) for tracks that play only a few hits. The track is then scaled to:

```text
target = role target + genre's offset for the role (mix.levels) + the track's level_db
```

with the gain clamped to −40..+30 dB. The role targets, in LUFS while the track plays:

| role | target | role | target | role | target |
|---|---|---|---|---|---|
| kick | −12.5 | bass | −14.5 | riser | −19 |
| snare | −13.5 | chords | −18 | impact | −16 |
| clap | −14.5 | keys | −17 | downlifter | −22 |
| hat | −17.5 | pad | −21.5 | reverse | −21 |
| ohat | −18.5 | lead | −18 | drums (loops) | −14.5 |
| shaker | −21.5 | arp | −20.5 | vocal | −15 |
| perc | −19.5 | top | −20 | sample, synth | −18 |
| ride | −20.5 | crash | −20 | texture | −27 |
| tom | −18 | | | anything else | −19 |

So `level_db` is a level relative to where the role belongs, not a fader position, and `mix("pad", #{ level: -3 })`
means "3 dB quieter than a pad usually sits". Genres shift roles with `levels`: trap pushes the bass up 1.5 dB and
pulls the kick and hats down 1 dB, lofi lifts the keys 2 dB. The report and the studio's mixer show each track's
measured level and the gain it got.

## Sidechain

The kick drives one duck curve for the whole song: at each kick onset the gain falls to zero over 3 ms, then recovers
along `1 − (1 − x)²` over `duck_release` beats (0.25 in trap and drum and bass, 0.45 in future bass, which pumps the
hardest). A track with `duck` `d` is multiplied by
`1 − d × (1 − curve)`, so `duck: 0.55` on the bass pumps it hard and `duck: 0.2` on a lead only breathes. The reverb
and delay returns are ducked by 0.3 too, which keeps the low end clear on the kick.

## Buses and returns

Tracks go to one of four buses, `drums`, `bass`, `music` and `fx`. Each bus runs its chain: the genre's (house
compresses the drums 2:1 from −12 dB and highpasses the music at 110 Hz), the arrangement's filter automation on
`music` and `bass`, and the three inserts every song gets (width with mono low end on `music` and `bass`, light
saturation on `drums`). The buses run in parallel.

Each track's post-fader signal also feeds the two returns in proportion to its `reverb` and `delay` sends:

- **Reverb** is a convolution with an impulse response generated in code: three bands of decorrelated noise (below
  500 Hz, 500 Hz to 4 kHz, above 4 kHz) decaying at different rates, so damping makes the highs die faster, plus seven
  early reflections in the first 70 ms. The input is cross-fed 20 % between channels so a hard-panned source fills
  both sides, and the wet signal is high-passed at `lowcut` and low-passed between 9 kHz and 5 kHz depending on damping,
  because a noise tail reads as hiss up there. The impulse is seeded, so a reverb sounds the same every render.
- **Delay** is a tempo-synced feedback delay in beats, ping-pong when asked, with a lowpass and a 180 Hz highpass in
  the feedback path, so repeats darken and do not build up low end. The return keeps only the repeats.

## Mastering

The buses and returns sum to the pre-master, whose integrated loudness goes into the report. The master chain from
the genre runs next (house: a 25 Hz highpass and a +1 dB shelf at 10 kHz, a gentle 1.6:1 compressor, tape
saturation), and then `finalize` brings the result to the genre's LUFS target under the ceiling (−1 dBTP unless
`master()` changes it):

1. Each pass applies an input gain, a soft clipper (2× oversampled, linear to 70 % of the ceiling minus 0.1 dB, then a
   tanh curve that never passes it, which shaves drum transients the way mastering engineers clip before the
   limiter), and a lookahead brickwall limiter at the ceiling minus 0.5 dB (1.5 ms lookahead, 50 ms release).
2. If the true peak still pokes over the ceiling, the pass trims the output to 0.05 dB under it.
3. The integrated loudness of the result is measured, and the next gain comes from a secant step on (gain, loudness),
   with the slope clamped to 0.15..1.5 and each step to ±6 dB. It stops within 0.15 LU of the target or after seven
   passes.

The critic flags a master more than 1 LU from its target or over its ceiling; the pipeline test asserts both on every
CI run.

| genre | target |
|---|---|
| dnb | −8.5 LUFS |
| techno, trap, future bass | −9 LUFS |
| house | −10 LUFS |
| synthwave | −10.5 LUFS |
| deep house | −11 LUFS |
| lofi | −12.5 LUFS |

## Loudness and peaks

`loudness.rs` implements ITU-R BS.1770: K-weighting with the 48 kHz coefficients, momentary (0.4 s) and short-term
(3 s) blocks from prefix sums of the weighted power, integrated loudness with the absolute gate at −70 LUFS and the
relative gate 10 LU below, loudness range (the 10th to 95th percentile of short-term loudness, gated 20 LU below), and
the active level above. A test checks it on the standard's reference: a −3 dBFS 1 kHz sine on both channels reads
−3.0 LUFS.

`fx::true_peak_db` measures true peak by 4× oversampling with a windowed sinc, evaluated only around samples within
about 4.4 dB of the sample peak, where inter-sample overs live.

## Effects

Every effect is plain data, so blueprints store, mutate and diff effect chains. In JSON an effect is tagged by `fx`:
`{"fx": "compressor", "threshold_db": -12, "ratio": 2.0, "attack_ms": 12, "release_ms": 120, "knee_db": 6}`.
Parameters that can move are `Param`s: a number, or a curve of `[beat, value]` points, interpolated per sample.

| effect | what it is |
|---|---|
| `gain` | gain in dB (automatable) |
| `eq` | RBJ biquad bands: lowpass, highpass, bandpass, notch, peak, low and high shelf, allpass |
| `filter` | a resonant filter with automatable cutoff: a zero-delay-feedback state-variable filter, or a saturating 4-pole ladder |
| `compressor` | stereo-linked feed-forward compressor, soft knee, gain reduction smoothed in dB |
| `limiter` | the lookahead brickwall limiter |
| `saturate` | a 2× oversampled waveshaper: `tanh`, `tube`, `tape`, `hard`, `fold` |
| `bitcrush` | bit depth and sample-and-hold |
| `reverb`, `delay` | as on the returns, for inserts |
| `chorus`, `flanger`, `phaser` | modulated delays and allpass stages |
| `width` | mid/side width, with the side removed below `mono_below_hz` |
| `tremolo`, `autopan` | tempo-synced |
| `gate` | a rhythmic volume gate (trance gate) from a list of step gains |
| `transient` | a transient shaper (a fast and a slow envelope split attack from sustain) |
| `tape`, `vinyl` | wow and flutter with saturation and a lowpass; crackle and hiss |
| `tape_stop`, `stutter`, `reverse` | edits on a region of beats: a slowdown to a halt, a beat repeat, a reversal |

## The synth engine

`synth/voice.rs` renders subtractive and two-operator FM voices. A `Patch` has:

- oscillators (`sine`, `saw`, `square`, `pulse`, `tri`, `noise`), band-limited with polyBLEP, each with a level, an
  octave and semitone offset, and a unison stack (voices, detune in cents, stereo width) with slow random pitch drift
  per voice (`analog`);
- a sub oscillator an octave down, a noise level, and an optional FM modulator (ratio, index, its own decay and
  sustain, and a fast-decaying ratio-14 "tine" for electric-piano bark);
- a filter (state-variable lowpass, highpass, bandpass or notch, or the ladder) with an envelope depth in octaves,
  key tracking and velocity sensitivity;
- amplitude and filter ADSRs, a pitch envelope (the drop that makes an 808 thump), an LFO (free or tempo-synced) to
  vibrato, cutoff and amplitude, glide, drive, and velocity sensitivity.

Polyphonic patches render every note as its own voice, in parallel batches of 96. Mono patches (basses, acid, leads)
render one voice that follows the notes, retriggering its envelopes unless notes overlap and the patch has `legato`
or `glide`, in which case it slides. Envelopes, pitch and filter coefficients update every 16 samples, with amplitude
interpolated across the block so there is no zipper noise.

The 20 presets in `synth/presets.rs` are `sub_bass`, `808`, `reese`, `acid`, `saw_bass`, `pluck_bass`, `organ_bass`,
`wobble`, `pluck`, `supersaw`, `pad`, `warm_pad`, `stab`, `epiano`, `keys`, `bell`, `organ`, `strings`,
`square_lead` and `chip`, each tagged with the roles it suits. A part holds a full copy of its patch, which the
generator varies slightly and evolution tweaks (cutoff, resonance, release, drive, detune).

## Drum voices and transition effects

`synth/drums.rs` renders 13 voices in the spirit of the 808 and 909 circuits: kick, snare, clap, closed and open hat,
ride, crash, tom, rim, shaker, cowbell, snap and perc. They are pitch-enveloped sines, clusters of detuned square waves
for metal, and shaped noise. All of them share one set of nine parameters (tune, decay, tone, punch, snap, drive,
pitch envelope, pitch time, room), each with a range per voice, so mutation can explore every voice the same way.
Every hit is rendered at −1 dBFS peak.

`synth/fxsounds.rs` renders the transitions: a riser (noise swept up through a resonant bandpass, with a detuned saw
climbing two octaves), a downlifter (the reverse), an impact (a sub boom and a noise burst), a reverse crash that ends
exactly on the downbeat, and noise sweeps up or down.

`claudius sounds` renders all of them, and every preset playing a phrase, into `out/sounds/`.

## Stems

With stems on, each track's buffer is kept after its gain, sidechain and pan, before the buses and returns: dry
stems, as a DAW bounces tracks without their sends. A song folder gets them as 32-bit float WAVs
(`song::write_folder`, from `make --stems`, `render --stems` and every `save`), since a track can peak above full
scale before the master's limiter. The server also writes 24-bit copies to its render cache and serves those at
`/api/stems/{version}/{track}.wav`.

## Speed

A render's cost is mostly the synthesized and sampled melodic tracks, the reverb convolution, and the master's
search, which runs the master stage up to seven times. On the machine these docs were written on, a full four-minute
house song renders in about 11 s. `CLAUDIUS_DEBUG=1` prints the time of every track and stage; this is a 64-bar
house sketch made of synthesis:

```text
render: track hat in 0.01 s
render: track kick in 0.02 s
...
render: track bass in 1.29 s
render: track chords in 1.50 s
render: track arp in 1.51 s
render: tracks + buses + returns in 3.20 s
render: master chain done at 4.12 s (-9.87 LUFS)
finalize: input -9.87 LUFS, gain -0.13 -> -10.72 LUFS (tp -1.05)
finalize: gain +0.59 -> -10.19 LUFS (tp -1.05)
finalize: gain +0.85 -> -9.96 LUFS (tp -1.05)
render: finalized at 6.22 s
```

The tracks render in parallel, so the slowest track sets the pace of the first stage; the three `finalize` lines are
the gain search converging on −10 LUFS.

`cargo bench -p claudius` measures generating, arranging and rendering 8 bars, and records results in
`target/criterion`.
