# The critic and the ears

After every render, `judge::judge(project, rendered, ears)` answers one question, is this song any good, with
evidence of three kinds, plus a fourth when earshot's models are loaded:

| component | what it looks at | weight |
|---|---|---|
| `mix` | meters on the master: loudness, true peak, tonal balance, stereo width, mono low end | 0.30 |
| `arc` | the energy arc: does loudness follow the arrangement's energy, is the drop louder than the break | 0.20 |
| `theory` | the score itself: the lead, the bass, voice leading, the form | 0.20 |
| `genre` | the ears: does it read as its genre rather than the genres it is confused with | 0.15 |
| `production` | the ears: does it read as professionally produced rather than as a typical flaw | 0.15 |
| `refs` | the ears, with reference tracks: how typical it is of them | 0.15 |

Each component scores 0 to 1. The total is the weighted mean of the components present, times 100, so a verdict
without ears is out of the same 100. [0006 A critic with ears](../design/0006-critic.md) records why the critic is
built this way.

## The verdict

This is the `verdict.json` of `claudius make --genre deep_house --seed 7 --short`, with the measures cut short:

```json
{"total": 86.41901839224477,
 "scores": {"arc": 0.9449047645757822, "genre": 0.7125, "mix": 0.9315307700243037,
            "production": 0.5925, "theory": 1.0},
 "measures": {"lufs": -10.98185847420735, "true_peak_db": -1.0499999523162842, "band_presence_db": -28.6,
              "drop_minus_break_lu": 3.5, ...},
 "issues": [{"severity": 0.4366487862341189, "target": "band:presence-",
             "what": "dull 4-8 kHz: hats, claps and snare lack bite (presence -28.6 dB, healthy from -26)"},
            {"severity": 0.40750000000000003, "target": "production",
             "what": "production reads close to 'a low quality recording with hiss' (z +1.3) vs professionally mixed (z +1.7)"},
            {"severity": 0.13392813023001673, "target": "band:air+",
             "what": "fizzy top end (air -16.2 dB, healthy up to -17)"},
            {"severity": 0.05999999999999996, "target": "genre",
             "what": "reads as 'techno music' (z +3.9) more than 'house music' (z +3.7)"}],
 "ears": {"genre_query": "house music", "genre_z": 3.71, "best_alt": "techno music", "best_alt_z": 3.86,
          "production_z": 1.7, "worst_flaw": "a low quality recording with hiss", "worst_flaw_z": 1.33,
          "sounds_like": ["techno music", "tech house music", "house music", "drum and bass music"],
          "p_real": 0.634, "audioset": [], "ref_typicality_z": null},
 "summary": "86/100 (arc 0.94, genre 0.71, mix 0.93, production 0.59, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -28.6 dB, healthy from -26); production reads close to 'a low quality recording with hiss' (z +1.3) vs professionally mixed (z +1.7); fizzy top end (air -16.2 dB, healthy up to -17)"}
```

An **issue** is a problem in words, a severity from 0 to 1, and a **target**: where in the blueprint a fix lives.
Issues are sorted worst first, and the summary names the first three after "Fix first". The targets are a small
vocabulary that people, assistants and the evolution loop all read:

| target | meaning | what evolution tries more often |
|---|---|---|
| `master` | loudness or true peak of the master | |
| `band:<band>+`, `band:<band>-` | too much or too little energy in a band | levels of the tracks that live in that band, drum sounds and kits, patches, space, pan |
| `width+`, `width-` | too narrow, too wide or phasey, or a low end that is not mono | the same |
| `form` | the arc or the length | layers, section flags, drum variants, section lengths |
| `part:<name>` | a part's writing | that part's seed, style, density and register |
| `genre` | it reads as another genre, or unlike the references | presets, styles, drum patterns and sounds, kits, tempo, progressions |
| `production` | it reads as one of the flaws | as for the bands |

## Mix

**Loudness and peak.** More than 1 LU from the master's target costs 0.1 and raises an issue of severity 0.3; a true
peak more than 0.1 dB over the ceiling costs 0.15 (severity 0.5). The loudness range is measured and reported but not
scored.

**Tonal balance.** The master's power spectrum (Welch, 8192-point frames, mono) is split into six bands, and each
band's share of the energy above 20 Hz is compared with a healthy range for a modern electronic master. The ranges are
built around a spectrum whose energy per octave is flat from 40 to 150 Hz and falls about 3 dB per octave above:

| band | range | healthy share |
|---|---|---|
| sub | 20–60 Hz | −14 to −4 dB |
| low | 60–250 Hz | −7 to −1 dB |
| lowmid | 250 Hz–1 kHz | −12 to −4 dB |
| mid | 1–4 kHz | −18 to −8 dB |
| presence | 4–8 kHz | −26 to −15 dB |
| air | 8–16 kHz | −30 to −17 dB |

Outside its range, a band costs 0.12 × severity, where severity is the distance in dB over 6 (at most 1), and the
issue says what is likely wrong: `too much sub (boomy): lower the bass / kick, or highpass the music bus`,
`muddy 250 Hz-1 kHz: too much pad / chord body`, `no air above 8 kHz: hats and cymbals too quiet or too dark`.

**Width.** The correlation between left and right over the whole master: above 0.96 is `nearly mono` (−0.08),
below 0.15 `too wide / phasey` (−0.1). Below 150 Hz the correlation must stay above 0.9, or the low end is
`not mono` (−0.08), which matters for club systems and vinyl.

## Arc

A producer shapes energy across a song: a breakdown drops back, a build rises, the drop hits. The critic measures
each section's loudness as the median short-term loudness (3 s blocks) inside it, leaving out its first 1.5 s and
last 1 s so transitions do not count, and compares that with the energy the blueprint gives the section:

- the score starts at `0.5 + 0.5 × r`, where `r` is the correlation between energies and loudnesses across sections
  (`arc_correlation`), and an `r` under 0.5 raises `loudness does not follow the arrangement's energy`;
- the loudest drop or hook should be 3 to 7 LU louder than the quietest break (`drop_minus_break_lu`); under 3 LU
  costs up to 0.3 and raises `the drop is only 2.1 LU louder than the breakdown (aim for 3-7): thin the break or push
  the drop`.

A song with fewer than two measurable sections scores 1.

## Theory

The critic reads the project's notes, not the audio, so these checks are exact:

| check | measure | issue when |
|---|---|---|
| the lead stays in key | `lead_in_key`, the share of lead notes in the key | under 95 % |
| the lead lands on chord tones | `lead_chord_tones_on_strong_beats`, on beats 1 and 3 | under 60 % |
| the lead is singable | `lead_range_semitones` | over 19 semitones |
| the lead moves by steps | `lead_stepwise`, and the share of leaps over a fifth | more than 20 % leaps |
| the bass plays the roots | `bass_roots_on_changes`, the share of chord changes where the bass hits the chord's bass note | under 70 % |
| chords move smoothly | `voice_leading_<track>`, the average movement per voice per chord change, for every chords, pad and keys track | over 4.5 semitones |
| the form is sound | `length_s`; every section a multiple of 4 bars | under 90 s or over 330 s; any other section length |

The theory score is the mean of a score per check that applies: a song without a lead or bass is not marked down for
them.

## The ears

The ears are [earshot](https://github.com/Leonxlnx/earshot), ported to Rust in `crates/earshot` with CLAP and the
AudioSet tagger running as ONNX models; [its README](https://github.com/devlato/claudius/blob/main/crates/earshot/README.md)
documents the port and its parity with the Python original. `ears::Ears` wraps it for claudius.

**One embedding.** The master is downmixed to mono the way the models were calibrated (ffmpeg's −3 dB per channel)
and embedded once with CLAP: a song up to 10 s long is one window; a longer one gets 10 s windows every 5 s, the last
flush with the end, at most six spread evenly, averaged and renormalised. Every question below is answered from that
one 512-dimensional vector, which is why the ears add only about a second to a verdict.

**Calibrated scores.** A raw CLAP similarity between audio and a text means little on its own: some texts match
everything. earshot calibrates it against its vocabulary of 322 terms:

```text
u = cos(audio, text) − text · m      m: the mean embedding of earshot's reference recordings
z = (u − mean of u over the 322 terms) / their standard deviation
```

A `z` says how much better this text describes this sound than the vocabulary does on average; earshot calls 2.5
plausible and 3 strong.

**Genre.** Each genre profile names what the ears should hear (`query`, `"house music"` for house and deep house) and
the genres it is most often confused with (`alts`: `"techno music"`, `"minimal electronic music"`, ...). The genre
score rewards a high `genre_z` and a margin over the best alternative:

```text
genre = 0.5 × clamp((genre_z − 1) / 2.5) + 0.5 × clamp((genre_z − best_alt_z + 1) / 2)
```

An issue is raised when `genre_z` is under 2.5 or an alternative beats it: `reads as 'techno music' (z +3.6) more than
'house music' (z +3.4)`.

**Production.** The best of two descriptions of a professional track (`a professionally mixed and mastered music
track`, `a high quality studio recording`) against the best of six flaws (a cheap synthesized beep, hiss, clipping,
a computer-generated tone, low-bitrate artefacts, a phone recording):

```text
production = clamp(0.5 + (production_z − worst_flaw_z) / 4)
```

with an issue when the difference is under 0.5.

**References.** With `--refs` and at least two reference tracks, the song's typicality among them (the mean cosine to
its five nearest references, as a `z` against the references' own spread) scores `clamp((z + 3) / 4)`, and an issue
is raised under −1.

**What it sounds like.** `sounds_like` lists the four vocabulary terms that describe the master best, leaving out the
quality and production categories, for people to read.

### Held-out evidence

Two readings are reported and never scored: `p_real`, earshot's estimate that the audio is a real, produced recording
rather than something synthesized in code (in the command line's verdicts), and the AudioSet tagger's top six labels
over three 10-second excerpts (with `--tags`). Optimising against a model can please the model without making the
music better, which is earshot's own warning. Keeping some evidence out of the score leaves a way to notice when
evolution is gaming the scored parts: the scores rise and the held-out readings do not.

## Where the critic is weak

The critic measures; it does not have taste. A high total means no measurable problems and a confident genre
reading, not a good song.

- earshot was validated for matching sound effects, not music. House, tech house and techno sit close together for
  it, and fully synthesized songs read only marginally "professionally mixed"; songs with sampled instruments read
  better.
- The healthy band ranges are one target for every genre: a lofi track is meant to be darker than they allow, so it
  gets `no air` issues that are style, not fault.
- `judge` on an arbitrary file without its `project.json` has no sections and no notes: the arc scores 1, the theory
  checks only the length, and the length is estimated at 120 BPM.
- The evolution loop judges 32-bar previews, so a preview's length issue (`30 s long`) is in every candidate's
  verdict; it shifts every score by the same amount and does not change which candidate wins.

Judge finished songs by ear too.
