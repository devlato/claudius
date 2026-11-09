# Rendering, mixing and mastering

`render::render(project, options)` turns an arranged project into a mastered stereo file. It works the way a mix
engineer does in a DAW, in a fixed order, and everything it does is decided by the project: there is no hidden state
and no randomness that is not seeded from the project's seed and the track's name.

Everything inside claudius runs at 48 kHz (earshot's rate) on planar stereo `f32` buffers (`audio::Stereo`).
A file at another rate is converted as it is decoded, through a windowed sinc (16 taps a side under a Hann window,
cutting below the lower Nyquist). A conversion between two rates comes back to the same positions between input
samples (160 of them from 44.1 kHz), so `audio::resample_rate` works out each one's weights once and counts the
positions in whole numbers. It decodes a 3.6-minute song ten times as fast as weighing every tap anew, which reading
at any step (`resample_step`, the sampler's varispeed) still does, and agrees with it to a rounding of a 32-bit float.

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
| `loop` | the loop is [warped](../guide/samples.md#warping) to the tempo and key, then each clip copies it in, re-sliced by `chop` when set, with fades of at least 1 ms in and 2 ms out; a [take](../guide/samples.md#takes)'s clips (`once`) play it from their offset to its end and not again, warped between silences instead of wrapping |

Details that make it sound played rather than printed:

- **Swing** moves every off-beat sixteenth late when notes are turned into sample positions, so the blueprint stays
  on the grid and one number changes the feel of every part.
- **Feel** moves each note by its track's `late_ms` (a lane's or part's, every hit alike) and, with `humanize`, by a
  drift (`Timing::feel`). The drift is a 1/f noise over the song's sixteenths (Voss and McCartney's: ten octaves of
  Gaussian draws, each held twice as long as the one before, summed and scaled to unit variance), so a hit moves with
  the ones around it and wanders over up to 32 bars. It is what Hennig et al. measured in players (PLOS ONE, 2011) and
  what their listeners preferred to independent jitter. Its standard deviation is 15 ms × `humanize` × a role's share
  (kick 0.5, snare and clap 0.8, hats 1, other percussion 1.2, parts 0.6), capped at three. The drum lanes take 60 % of
  theirs from a stream they share (one player) and the rest from their own. Every draw comes from the song's seed and
  the track's name at that sixteenth, so a render, a choke's cut and the kick's duck all see the same onset. Velocity
  varies by ±5 % × `humanize`.
- **Velocity** of a one-shot scales its level by `velocity^1.6`.
- **Choke groups** stop a ringing hit at the next onset of any track in the same group, with a 4 ms fade (the open
  hat stops when the closed hat plays). Pack instruments get the same cut times.
- **Samples on melodic roles** stop at the note's length plus 50 ms, so `legato()` shapes them; on drum roles they
  play to the end.

## The auto-mixer

A producer balances by ear: the kick here, the bass a little under it, the pad well back. claudius does the same with a
meter. First the gain is staged, as an engineer stages it into a chain of plug-ins: a track with inserts or a drive
whose sound is more than 6 dB from its role's target (below) is brought to 6 dB from it before them (`Song::finish`;
at most 30 dB up, and a recording's residue, under −45 LUFS, not up at all, so that the mix still hears it as residue
and raises it no more than 12 dB). A compressor's threshold and a saturator's drive then work on it as they were set to: a pad made
20 dB hot is not driven into distortion, nor a quiet recording left under its compressor. A sound made within 6 dB of
its role's target goes in as it was made. After its inserts and its drive (a tanh saturator `drive_db` hard, when it
has one), each track's **active level** is measured: the 80th percentile of its momentary loudness (0.4 s blocks every 0.1 s) over the blocks where it
is audible (above −50 LUFS). Momentary blocks hear each hit about as the ear does. A 3 s window would hear a clap on
two and four with the silence between its hits, 3 to 4 dB under how loud its hits are, and an impact or a reverse
crash 5 to 8 dB under, so a mix balanced on it played every sparse track louder than its target. The track is then
scaled to:

```text
target = role target + genre's offset for the role (mix.levels) + the track's level_db
```

with the gain clamped to −40..+30 dB, and to +12 dB for a recording that plays under −45 LUFS: what a stem or a filter
left of what it took out (a hiss, a smear of drums), which raised to its role's level would only be heard as that. The
critic then says it is quieter than its level. The role targets, in LUFS while the track plays:

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
| `tremolo`, `auto_pan` | tempo-synced |
| `gate` | a rhythmic volume gate (trance gate) from a list of step gains |
| `transient` | a transient shaper (a fast and a slow envelope split attack from sustain) |
| `tape`, `vinyl` | wow and flutter with saturation and a lowpass; crackle and hiss |
| `tape_stop`, `stutter`, `reverse` | edits on a region of beats: a slowdown to a halt, a beat repeat, a reversal |
| `tune` | a voice moved onto the notes it means (`voice::tune`, [below](evolution-and-analysis.md#tuning-a-voice)): the notes read against the song's key and chords, which `arrange` sets as the tuner's `harmony` in beats, and moved by TD-PSOLA (`dsp/psola.rs`) |

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

The 50 presets in `synth/presets.rs` ([Synth presets](../language/sounds.md#synth-presets) lists them) are each
tagged with the roles they suit, at least six for every role, and say what they are in a few words
(`presets::about`): the table's words for each, which a test keeps the same, and which the studio shows over a preset's
name. The styles say what they play the same way (`compose::style_about`: an idiom's `about`, or the words of a
motif, a chord rhythm or an arpeggio). A part holds a full copy of its patch, which the
generator varies (`compose::widen`: a little for a genre's own sound, more for one drawn from its palette) and
evolution tweaks (cutoff, resonance, release, drive, detune). No preset after the first twenty may share its name with
an instrument in the packs' registry, as a preset hides an instrument of its name.

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

A session renders again only the tracks an edit changed (`render::render_kept`). It keeps each track's sound through
its effects and drive from the render before, with the level the auto-mixer heard in it, under a key made of
everything that sound comes from:
- the track itself, but for what the mix does after its drive (its level, pan, sends, duck and bus);
- the song's tempo, swing, humanisation, seed, key and chords;
- the render's length, the onsets that choke the track, and the size and time of any file it plays.

A level, pan or send, the master, or another track's notes render no track again. Only the stages after the tracks
run: levelling, buses, returns and the master. The mix levels, ducks and pans each kept sound as it sums it into its
bus and sends, in the order a fresh render does, so what comes out is the same to the sample (a test checks both).

A kept sound is held in blocks of 4096 samples, without the blocks that are silent all through. A song's tracks rest
much of it: 40% of the blocks of a deep-house song are silent, more than half of trip-hop's and lo-fi's, and 64% of
trap's, so they take about half the memory a sound held whole would. The mix skips a silent block, which would add
nothing, and reads any stretch back to the sample, a playhead window's too.
In the web demo, a level change on one track renders again in about 13 s where the whole song takes 31.
Installing packs forgets what was kept, since an instrument may sound otherwise.

Where there are no threads, in a browser, the tracks are made elsewhere and kept instead. `Kept::stale` lists the
tracks a render would make anew, with the key each will be kept under; `render::track_sound` makes one, in another
worker; and `Kept::keep` keeps it, so the render only mixes. The demo's engine shares a song's tracks out among
copies of itself this way (see [The demo](ui.md#the-demo)). A test checks that tracks made so render the song as it
would have rendered alone.

A synth that plays its notes apart (not a mono one, whose voice glides from note to note) can be made in parts too.
`render::track_part` renders the notes that sound in a stretch of the song (`voice::render_range`), each as a render
renders it, and adds them from silence in the order a render adds them. So every sample of a part is the one a render
makes there, and `render::track_joined` puts the parts side by side and runs the track's effects. `render::part_bounds`
places the stretches so that the notes beginning in each sound about as long, joins a stretch no note sounds in to the
next, and says what share of the work each part has: a part renders whole every note that sounds in it. A test checks
a track made in three parts against the track made whole.

### Hearing an edit at once

`render::render_window` renders a stretch of the song as the whole song's render has it there.
- The tracks come from what is kept; a track the edit changed is rendered whole first, as a whole render would.
- The buses, returns and master run from early enough that every effect has settled as the whole song's would have: the longest tail of their chains, and a second, and back to the start of any tape stop, stutter or reverse on them.
- They also run a quarter of a second past the window's end, for the limiter's lookahead and the lag of tape and the true-peak filters.
- Every effect knows where its buffer lies in the whole song (`fx::Ctx::start` and `total`), so its automation, LFOs, gates, bitcrush hold and vinyl surface are where they would be. A whole render starts them at 0, as before, and comes out the same to the byte.

The one thing a window cannot know is the master's loudness: its gain and true-peak trim are measured over the whole song (`finalize`). A window holds them as the last whole render settled them (`Settled`). With those held, a window is within about 2×10⁻⁷ of the whole song's render (−135 dBFS, a rounding or two of a 32-bit float), in house, lo-fi, synthwave and trip-hop alike (a test checks it). An edit that makes the song louder or quieter is levelled again by the next whole render.

The demo's engine uses it to preview an edit where the person is listening: the studio says where its playhead is with each command, and an edit to a song heard whole is first the last render with the twelve seconds from the playhead on rendered again and crossfaded in. That arrives in about 0.8 s in the browser, and the whole song follows. A song whose length an edit changed (its tempo, its form) gets its first bars first instead, as a new song does.

`cargo bench -p claudius` measures generating, arranging and rendering 8 bars, and records results in
`target/criterion`.
