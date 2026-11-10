# The critic and the ears

After every render, `judge::judge(project, rendered, ears)` answers one question, is this song any good, with
evidence of three kinds, plus a fourth when earshot's models are loaded:

| component | what it looks at | weight |
|---|---|---|
| `mix` | meters on the master (loudness, true peak, tonal balance, stereo width, mono low end), and each track's level against its role's | 0.30 |
| `arc` | the energy arc: does loudness follow the arrangement's energy, is the drop louder than the break | 0.20 |
| `theory` | the score itself: the lead, the bass, voice leading, the form | 0.20 |
| `genre` | the ears: does it read as its genre rather than the genres it is confused with, or stock background music | 0.15 |
| `production` | the ears: does it read as professionally produced rather than as a typical flaw | 0.15 |
| `refs` | the ears, with reference tracks: how typical it is of them | 0.15 |
| `time` | the recordings on its tracks (a remake's pieces, loops, takes): does each keep the song's tempo and beat | cuts the total |

Each component scores 0 to 1. The total is the weighted mean of the components present, times 100, so a verdict
without ears is out of the same 100. Then the total is cut, by stock music (below) and by `time`: a recording out of
the song's time leaves it as little as 35 % of itself. [0006 A critic with ears](../design/0006-critic.md) records why the critic is
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
            {"severity": 0.13392813023001673, "target": "band:air+",
             "what": "fizzy top end (air -16.2 dB, healthy up to -17)"},
            {"severity": 0.05999999999999996, "target": "genre",
             "what": "sounds more like 'techno music' than 'house music'"},
            {"severity": 0.052000000000000046, "target": "production",
             "what": "sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause"}],
 "ears": {"genre_query": "house music", "genre_z": 3.71, "best_alt": "techno music", "best_alt_z": 3.86,
          "production_z": 1.7, "worst_flaw": "a low quality recording with hiss", "worst_flaw_z": 1.33,
          "sounds_like": ["techno music", "tech house music", "house music", "drum and bass music"],
          "p_real": 0.537, "audioset": [], "ref_typicality_z": null},
 "summary": "86/100 (arc 0.94, genre 0.71, mix 0.93, production 0.59, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -28.6 dB, healthy from -26); fizzy top end (air -16.2 dB, healthy up to -17); sounds more like 'techno music' than 'house music'"}
```

An **issue** is a problem in words, a severity from 0 to 1, and a **target**: where in the blueprint a fix lives.
Issues are sorted worst first, and the summary names the first three after "Fix first". The targets are a small
vocabulary that people, assistants and the evolution loop all read:

| target | meaning | what evolution tries more often |
|---|---|---|
| `master` | loudness or true peak of the master | |
| `band:<band>+`, `band:<band>-` | too much or too little energy in a band | levels of the tracks that live in that band, drum sounds and kits, patches, space, pan |
| `width+`, `width-` | too narrow, too wide or phasey, or a low end that is not mono | the same |
| `level:<track>+`, `level:<track>-` | a track louder or quieter than its genre makes it | that track's level, turned the way back |
| `sound:<part>` | a part plays a sound its genre has no place for | that part's preset and patch |
| `form` | the arc, or a section out of its phrases | layers, section flags, drum variants, section lengths, build-ups |
| `form:short`, `form:long` | short or long for its genre | a section played again or left out, a section longer or shorter (`form::change`), section lengths the way the song should go |
| `form:intro` | slow to get under way | the intro shorter, or left out |
| `form:same`, `form:section` | a stretch heard as one section, too long | of neighbours alike, the second with a part more or less or on the other progression; the longest section shorter |
| `form:peaks-`, `form:peaks+` | the hook never comes back, a drop genre's song never drops, or the peak comes far too often | the peak and its way in again; a later one left out |
| `part:<name>` | a part's writing | that part's seed, style, density and register |
| `genre` | it reads as another genre, or unlike the references | presets, styles, drum patterns and sounds, kits, tempo, progressions |
| `production` | it reads as one of the flaws | as for the bands |
| `time:<track>` | the recording on a track keeps a tempo of its own, or wanders off the song's beat | |

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

A genre whose masters part from these sets its own ranges in its profile (`mix.bands`): trap and UK drill let the
sub reach −1 dB of the whole, since their 808 carries most of the song's energy, and let the mids and highs above it
fall lower than usual (mid from −22, presence from −31, air from −35).

Outside its range, a band costs 0.12 × severity, where severity is the distance in dB over 6 (at most 1), and the
issue says what is likely wrong: `too much sub (boomy): lower the bass / kick, or highpass the music bus`,
`muddy 250 Hz-1 kHz: too much pad / chord body`, `no air above 8 kHz: hats and cymbals too quiet or too dark`.

**Width.** The correlation between left and right over the whole master: above 0.96 is `nearly mono` (−0.08),
below 0.15 `too wide / phasey` (−0.1). Below 150 Hz the correlation must stay above 0.9, or the low end is
`not mono` (−0.08), which matters for club systems and vinyl.

**Balance.** Every track should play at the level its genre makes it at: its role's target, the genre's offset for
the role, and the level the genre gives the track itself, such as trap's clap 4 dB under its role or deep house's
transitions 3 dB under theirs (`compose::made_level`). The [auto-mixer](rendering.md#the-auto-mixer) sets each track
to that level and measures it the same way, on its hits.
Engineers who mix the same song set its lead within about 1.5 dB of one another, so a track within 3 dB of its
level is a choice. Further away, it costs 0.2 × severity, where severity is the distance beyond those 3 dB over 6
(at most 1). The issue names the track and the way to turn it: `chords is 9.0 dB louder than the chords level: it
covers the rest`, target `level:chords+`. The master's meters cannot hear this. A song with one track 9 dB too loud
can keep every band in its range.

## Arc

A producer shapes energy across a song: a breakdown drops back, a build rises, the drop hits. The critic measures
each section's loudness as the median short-term loudness (3 s blocks) inside it, leaving out its first 1.5 s and
last 1 s so transitions do not count, and compares that with the energy the blueprint gives the section:

- the score starts at `0.5 + 0.5 × r`, where `r` is the correlation between energies and loudnesses across sections
  (`arc_correlation`), and an `r` under 0.5 raises `loudness does not follow the arrangement's energy`;
- the loudest drop or hook should be 3 to 7 LU louder than the quietest break (`drop_minus_break_lu`); under 3 LU
  costs up to 0.3 and raises `the drop is only 2.1 LU louder than the breakdown (aim for 3-7): thin the break or push
  the drop`.

A song with fewer than two measurable sections scores 1. Each section's loudness is kept in the measures as
`section_<n>_<name>_lufs`, numbered in the arrangement's order (`section_05_drop_lufs`), and the MCP tools print them
as one `loudness by section` line: the critic compares only the drop with the break, and these let a person or an
assistant compare any two.

## Theory

The critic reads the project's notes, not the audio, so these checks are exact:

| check | measure | issue when |
|---|---|---|
| the lead stays in key | `lead_in_key`, the share of lead notes in the key or in the chord sounding with them | under 95 % |
| the lead lands on chord tones | `lead_chord_tones_on_strong_beats`, on beats 1 and 3 | under 60 % |
| the lead is singable | `lead_range_semitones` | over 19 semitones |
| the lead moves by steps | `lead_stepwise`, and the share of leaps over a fifth | more than 20 % leaps |
| the bass plays the roots | `bass_roots_on_changes`, the share of chord changes where the bass hits the chord's bass note | under 70 % |
| chords move smoothly | `voice_leading_<track>`, the average movement per voice per chord change, for every chords, pad and keys track | over 4.5 semitones |
| the form holds a listener | `length_s`, `intro_s`, `sections_alike`, `peaks` | as [Form](#form) says |
| every part plays a sound its genre has a place for | the rules the genre's own draws follow (`compose::has_place`, [Sounds for parts and drums](theory.md#sounds-for-parts-and-drums)) | a part that does not: `chords plays synth patch 'whistle', which house has no place for`, target `sound:chords` |

The theory score is the mean of a score per check that applies: a song without a lead or bass is not marked down for
them. A part whose sound has no place in its genre adds a check it fails outright, a 0 to that mean.

## Form

The form checks are one of the theory checks, and like them read the whole song, even when the evolution loop renders
only a preview of it (`judge::judge_whole`). They hold a song to its genre's own forms (`GenreProfile::forms`, at the
genre's tempos), loosely. Liking music follows an inverted U over how familiar and how complex it is
([Chmiel & Schubert 2017](https://doi.org/10.1177/0305735617697507)), and the songs that rose highest in the Billboard
charts were neither the most typical of their peers nor the least ([Askin & Mauskapf
2017](https://doi.org/10.1177/0003122417728662)): a song is held to its genre's ways, with room around them. And the
checks look only at what listeners have been found to hear of a form, which is less than its order. Reordering the
sections of unfamiliar pop songs left how predictable and how likeable people found them largely as it was: they
listen section by section ([Rolison & Edworthy 2012](https://doi.org/10.1525/mp.2012.29.3.269)), as they do to a
Mozart symphony ([Karno & Konečni 1992](https://doi.org/10.2307/40285538)). What they hear is each section as it
comes, and where it gives way to the next, by a change of timbre, loudness or register
([Deliège 1987](https://doi.org/10.2307/40285378)).

| check | measure | issue when | target |
|---|---|---|---|
| its length | `length_s` | under the genre's shortest form over 1.5, or over 1.5 times its longest and over 5:30: `2:00 long, where trance's forms run 3:26-3:35` | `form:short`, `form:long` |
| its way in | `intro_s`, where the first section that is not an intro starts | after twice the genre's longest intro (16 bars at least): `the song takes 1:09 (32 bars) to get under way, where ambient's intros run 8 bars at most` | `form:intro` |
| its phrases | every section a multiple of 4 bars | any other length | `form` |
| what is heard as one section | `sections_alike`: neighbours with the same texture, the tracks that play, where in the bar each drum strikes and the chords (the transitions mark a boundary, and are left out) | neighbours alike, or one section, longer together than 1.5 times the genre's longest section and 32 bars: `drop and drop2 play alike, 64 bars heard as one section where trance's longest is 32` | `form:same`, `form:section` |
| its peaks | `peaks`, its hooks and drops | a hook that comes once where the genre's forms bring theirs at least twice; a song of a drop genre that never drops; more than two over the most the genre's forms have | `form:peaks-`, `form:peaks+` |

An issue costs the form's score 0.1 to 0.3, the more the further off the song is, and a song without a genre is held
to 90 s to 5:30 and its phrases. Why each check is as it is:

- **Its length.** A genre's forms are its shapes at its tempos, and its records run shorter and longer than they do,
  so the critic allows half again either way, and finds 5:30 long for none of them. The check catches a song cut to a
  sketch or run on far past its genre; inside that the length is a matter of taste.
- **Its way in.** A quarter of the songs streamed on Spotify are skipped within their first five seconds, and skips
  rise again just after a section changes ([Montecchio, Roy & Pachet 2020](https://doi.org/10.1371/journal.pone.0239418)).
  The intros of the US top ten shortened from about 20 seconds in the mid-1980s to about 5 by 2015, though an
  artist's more popular songs did not have the shorter ones ([Léveillé Gauvin
  2018](https://doi.org/10.1177/1029864917698010)). So the check catches only a way in far longer than the genre's:
  genres differ, a DJ track or an ambient one taking its time, and the bound is twice the genre's own longest intro.
  A quiet first verse is the song begun.
- **What is heard as one section.** Dance music adds, changes or removes a layer after 2, 4, 8, 16 or 32 bars
  ([Solberg & Jensenius 2016](https://doi.org/10.18061/emr.v11i3-4.5023)). Neighbours that change none of what marks
  a boundary sound as one section, so they are measured as one: 32 bars of a chorus played twice is a chorus, 64 bars
  of a drop played twice is a long time with nothing new. Notes are not compared: the arranger writes each section's
  afresh, and a new melody over the same texture does not make a new section.
- **Its peaks.** Repetition is what makes a hook: a song repeated within itself is easier to take in, and songs whose
  lyrics repeat more reached the top of the charts sooner ([Nunes, Ordanini & Valsesia
  2015](https://doi.org/10.1016/j.jcps.2014.12.004)). In a drop genre the drop is what the break routine builds to,
  and the part listeners respond to most, in their skin conductance and in their own words
  ([Solberg & Dibben 2019](https://doi.org/10.1525/mp.2019.36.4.371)). A song of verses alone, in a hook genre, is a
  form of its own and is not marked down.

Not checked: the order of the sections, which listeners hardly hear, and how surprising the music is against how
predictable, which is what pleasure in it follows ([Cheung et al. 2019](https://doi.org/10.1016/j.cub.2019.09.067)),
but chord by chord rather than section by section.

## Time

A recording that plays at another tempo than the song, or drifts off its beat, sounds wrong at once: its drums flam
against the song's, its chords change off the bars. The mix, the arc and the theory cannot hear it, and neither can
the ears: CLAP sums up ten seconds at a time as a description, and a house song with a sample a few percent slow
still reads as house. So the critic measures it, on the audio of each track that plays a recording (a loop or a take:
a remake's `original`, its hook and passages, a break, a flip), as the renderer makes it, before the mix
(`render::kept_time`, kept with the track's sound and reported with it as `time`):

- its **onsets**: the spectral flux of the track where it rises over its own mean around it (an eighth of a second each
  way), each by how much;
- how tightly they gather at one place on the song's sixteenths, `on_grid`: the length of their mean on a circle one
  sixteenth round, weighted, from 0 (anywhere) to 1 (all at one place);
- the same on the sixteenths of every tempo from 0.7 to 1.4 times the song's, a quarter of a percent apart and then a
  fiftieth around the best: the tempo they gather on most tightly, `own_bpm`, and how tightly, `own`;
- `drift_ms`: how far each four bars' place on the song's sixteenths lies from the whole's, the most of any whose
  onsets gather there at least half as tightly as the whole's do (a fill, a riser or a wash gathers at no place, and
  the place its leftover points to would read as wander).

Onsets rather than beats tracked, because a beat tracker slips by half a beat on a guitar or a voice and would take its
own slip for the recording's. A recording whose onsets gather on no grid near the song's tempo (`own` under 0.15: a
pad, a voice alone) is not judged. Nor is a track with the `vocal` role: a few syllables, looped, line up by chance on
a tempo a whole number of whose sixteenths fit the loop (33 or 28 of them in a two-bar phrase), and a sung phrase
keeps the time the rest of its recording keeps, which is judged. Then:

| issue | when | severity |
|---|---|---|
| `'original' keeps a tempo of its own, about 126 BPM, against the song's 130: the recording on it is off the song's time` | `on_grid` under half of `own`, at a tempo more than 0.5 % from the song's | `(0.7 − on_grid / own) / 0.4` |
| `'break' drifts off the song's beat, by up to 60 ms` | `drift_ms` over 40 (a band playing live, its bars each made one length, wanders 30) | `(drift_ms − 30) / 40` |

Each severity is scaled by how clearly the recording keeps a grid (`(own − 0.15) / 0.15`, at most 1), and `time` is 1
less the worst. The total is multiplied by `0.35 + 0.65 × time`, so a recording clearly off the song's tempo leaves a
song at most 35 of its points, whatever else is right. The measures keep `time_<track>_on_grid`, `_own`, `_own_bpm`
and `_drift_ms` for each recording judged; a song with none to judge has no `time` score, and its total is as it was.

A remake's recording declared 3 % off its tempo, as a misread tempo would leave it, is found keeping 126.2 BPM in a song
at 130, its verdict 32 where it was 91; 0.7 % off, 129.1 BPM, the same. The same remake with its tempo read right
keeps 130.01, its `on_grid` its `own`.

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
the genres it is most often confused with (`alts`: `"techno music"`, `"minimal electronic music"`, ...). Every genre is
also measured against five readings of stock library music: `inspirational corporate background music`, `upbeat
corporate pop music`, `playful children's piano music`, `cheerful ukulele music` and `chiptune video game music`. A
song that sounds like an advert's background music has failed its genre, however clearly it beats its neighbours. The
genre score rewards a high `genre_z` and a margin over the best alternative, stock music included:

```text
genre = 0.5 × clamp((genre_z − 1) / 2.5) + 0.5 × clamp((genre_z − best_alt_z + 1) / 2)
```

An issue is raised when an alternative beats the genre, `sounds more like 'techno music' than 'house music'`, or when
`genre_z` is under 2.5, `sounds only faintly like 'house music' (the nearest other reading: 'techno music')`. Its
severity is the larger of `(2.5 − genre_z) / 2.5` and `(best_alt_z − genre_z) / 2.5`. The issues leave the `z` values
out; the `ears` line of the command line and the MCP tools prints them.

Stock music also cuts the whole verdict. When the strongest stock reading is plausible (`stock_z` of 2.5 or more) and
within one z of the genre's, the total loses up to 40 % of itself, in proportion:

```text
stock share = clamp(stock_z − genre_z + 1)        total × (1 − 0.4 × stock share)
```

The issue, with the stock share as its severity, says `sounds close to stock background music ('inspirational
corporate background music'): too clean and polite for 'trip hop music'`; when a stock reading beats the genre
outright, the `sounds more like` issue says it instead, with that severity. `stock_share` is among the measures.

**Production.** The best of two descriptions of a professional track (`a professionally mixed and mastered music
track`, `a high quality studio recording`) against the best of six flaws (a cheap synthesized beep, hiss, clipping,
a computer-generated tone, low-bitrate artefacts, a phone recording):

```text
production = clamp(0.5 + (production_z − worst_flaw_z) / 4)
```

with an issue when the difference is under 0.5. The issue names the flaw in a few words and where it usually comes from
in a song of claudius': `sounds about as much like a hissy recording as a professional mix: noise-built sounds (hats,
shakers, risers) and long reverbs are the usual cause`. It says "more like" only when the flaw leads by more than 0.5.
Its severity is `(0.5 − difference) / 2.5`: a near tie is a faint reading, about 0.2, so the issues that name a change
come before it, and a flaw that leads by 1.5 reaches 0.8.

A genre can list flaws that are its sound in `accepted_flaws`, and its production reading leaves them out: lofi
accepts the hiss and the phone recording, since vinyl crackle, tape hiss and a lowpassed drum bus are the style.

**References.** With `--refs` and at least two reference tracks, the song's typicality among them (the mean cosine to
its five nearest references, as a `z` against the references' own spread) scores `clamp((z + 3) / 4)`, and an issue
is raised under −1.

**What it sounds like.** `sounds_like` lists the four vocabulary terms that describe the master best, leaving out the
quality and production categories, for people to read.

### Held-out evidence

Two readings are reported and never scored: `p_real`, earshot's estimate that the audio is a real, produced recording
rather than something synthesized in code (in the command line's verdicts, the mean over eight 4-second windows spread
across the song, where earshot would read all of them), and the AudioSet tagger's top six labels over three
10-second excerpts (with `--tags`). Optimising against a model can please the model without making the
music better, which is earshot's own warning. Keeping some evidence out of the score leaves a way to notice when
evolution is gaming the scored parts: the scores rise and the held-out readings do not.

## Where the critic is weak

The critic measures; it does not have taste. A high total means no measurable problems and a confident genre
reading, not a good song.

- The mix, arc and theory checks are what the generator aims at: the mixer sets each track to its level, the
  arranger writes the arc, the composer writes in key. So a song claudius makes meets most of them by construction,
  and they say most when something has been changed by hand or by evolution. No listener has checked yet that a
  higher total is a song people prefer.
- The ears' genre and production readings compare the audio with descriptions (CLAP). Published audits of such
  judges find their scores follow how well the audio matches its description more than how good it is. They tell a
  song that sounds like its genre from one that does not, but they do not rank good songs. Nor do they hear time: a
  song whose sample plays against its drums reads as its genre as well as one whose sample keeps time, which is why
  `time` measures it on the tracks themselves instead of asking a better-trained CLAP.
- `time` judges the recordings that keep a grid. A voice, a pad, or a recording too sparse to gather on one is taken
  at its word, and so are notes, which play where the song puts them.

- earshot was validated for matching sound effects, not music. House, tech house and techno sit close together for
  it, and fully synthesized songs read only marginally "professionally mixed"; songs with sampled instruments read
  better.
- The healthy band ranges are one target for nearly every genre: only the 808 genres set their own. A lofi track is
  meant to be darker than they allow, so it gets `no air` issues that are style, not fault.
- `judge` on an arbitrary file without its `project.json` has no sections and no notes: the arc scores 1, the theory
  checks only the length, and the length is estimated at 120 BPM.
- The evolution loop renders a preview of each candidate, 32 bars around its first drop or hook, so its mix, arc, time
  and ears are the preview's, and a change outside it is judged by its notes and its form alone.
- The form checks hold a song to its genre's forms, which are claudius' templates of the genre rather than
  measurements of its records. They count the sections named hooks and drops, so a song whose hook is a riff running
  through its verses has none to count.

Judge finished songs by ear too.
