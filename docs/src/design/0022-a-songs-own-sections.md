# 0022 A song's own sections

Status: accepted

## Context

A remake, a flip, a rework and the sampler all start from the sections the analysis hears in a recording: the flip
takes its hook from the loudest of them and goes on through the recording a section at a time, a rework is its form
bar for bar, and a sampled passage starts where one of them starts. Until now the analysis cut a song into 4-bar
blocks from its first bar, put a boundary where neighbouring blocks differed most (8 bars apart at least, 32 at most),
and named each section by its loudness alone: 0.85 of the loudest or more was a drop.

That suits a dance track, whose sections are its loudness. A song is not: on six songs a person had in their
Downloads (Alizée, Depeche Mode, The Prodigy, Telenova, The Killers), every one came out as an intro, a row of drops
and an outro, its verses, pre-choruses and choruses nowhere. A section could not be 6 bars or 17, nor start where its
phrase does, so a piece cut from one could end while the recording was still building, and the song made on it would
drop early. An agent reworking two songs over MCP asked for the same: phrases to the beat, sections labelled by what
repeats, boundaries in the singer's breaths, any length.

## Decision

The analysis hears a song's own sections (`analysis/structure.rs`), with no model: signal processing on what it
already reads, the stems when the ears separate them.

- **Each bar** is heard as its harmony (the pitch classes of each of its four beats, from the bass and the rest), its
  timbre (cepstral coefficients of its spectrum), how loud and bright it is, its kit, its bass and how much of it a
  voice sings (the words `cuts` hears in the vocals, over the bars round each, so a breath between lines is no change).
- **A section starts** where how the bars sound stops being like it was (Foote 2000, the novelty of their
  similarity); where the chords a passage plays change, whatever their order; where a run of bars that comes back 8 bars
  or more later starts or ends (a diagonal stripe in the harmony's similarity: it marks where both verses start,
  however unlike each other the bars within a verse are); where the voice, the kit or the loudness comes in or goes. The
  harmony is not compared bar by bar as the sound is: chords change every bar within a section. Of every way to cut
  the bars, the one whose boundaries are most novel, less a cost for each, under a preference for 8 and 16 bars that
  never forbids another length: a clear boundary at 17 bars outweighs it (dynamic programming over the bars). The bars
  are counted as a phrase's: a bar over 3.2 s, of a slow song or one heard at half its tempo, as two. Each change is
  scaled to the song's clearest, but never by more than what a real change measures, so a steady loop's noise is no
  boundary, and sections that rise together, bar by bar, into a louder one are one build.
- **Sections that play the same music**, in the same key or up to two semitones away (a last chorus a step up), share
  a letter: they start and go on alike in their chords and their sound (average linkage).
- **What each is**: in a song a voice sings, the chorus is the music it sings again and again that is loudest and
  sung most, ends the singing and does not start it; the verse is the other music it sings again, that leads to the
  chorus; a pre-chorus comes between, a post-chorus is loud music no one sings right after a chorus, a bridge is sung
  once after two choruses (or after one, 8 bars long or more), the intro and outro play before and after the voice, an
  instrumental or a break between (Goto 2006 finds a chorus by its repeats; Paulus, Müller and Klapuri 2010 review the
  methods). A track no one sings in (the stems apart, its vocals all but silent) keeps intro, build, drop, break and
  outro, by loudness, with the music a drop plays a drop wherever it plays. With no stems, the voice is not heard, and
  a song is taken as sung wherever it plays fully.
- **A build** is the bars at the end of a section that rise into a louder one, bar by bar; it resolves where the next
  section starts. **A cut** is where a piece of a section is best taken from the recording: its first bar's line, or
  the quiet before its first phrase when that is sung before the bar.
- `analyze` says all of it, the phrases sung to the beat as a DAW writes positions (`27.2-31.4`), and a section's
  label maps to the kinds a song of claudius's plays (a chorus as its hook, a pre-chorus as a build), so the blueprint
  does not change.

## Evaluation

`examples/eval_structure.rs` measures any version, from its analyses (`claudius analyze --json`) or making them, on two
sets of songs downloaded to the computer and never into the repository. The 4-bar blocks this replaces were measured
from claudius 0.10.0's analyses, on the bar grid of their first downbeat and tempo, as the new one was for a like
comparison.

**JamendoLyrics**: 78 songs under Creative Commons licences, their lyrics in stanzas and each line timed by hand. A
stanza that starts other music than the one before is where a sung section starts; the stanza sung again most often is
the chorus.

| | 4-bar blocks | new, the mix alone |
|---|---|---|
| stanzas starting other music heard within a bar of a section's start | 14% of 661 | 60% |
| sections starting within a bar of a stanza, or of where the singing stops (a lower bound) | 44% of 282 | 57% of 892 |
| choruses heard as choruses | 66% of 204 | 53% |
| other stanzas heard as choruses | 55% of 497 | 38% |
| stanzas sung alike heard as the same music | (no letters) | 54% of 416 |

On the first 25 songs, with the stems apart as the studio hears them: starts heard 57% (58% from the mix, 17% before),
sections on a stanza 46%, choruses 59% (46% from the mix), other stanzas taken for choruses 44% (34%), stanzas sung
alike as the same music 71% (48%). Hearing the voice groups the music that comes back far better and names more
choruses, and puts in more boundaries where the singing comes and goes.

**SALAMI**: 60 recordings of its Internet Archive part (live, a jam band as often as a pop group) whose first
annotator marked a verse and a chorus, against each of its annotators, from the mix alone.

| | 4-bar blocks | new |
|---|---|---|
| boundaries within 0.5 s, F-measure | 0.081 | 0.154 |
| boundaries within 3 s, F-measure | 0.356 | 0.486 |
| the time heard as its function (verse, chorus, ...) | 25% | 26% |
| the time heard as a chorus, F-measure | 0.347 | 0.230 |

Published methods of this kind score 0.22 to 0.30 and 0.45 to 0.55 on SALAMI's boundaries: the new one is among them
at 3 s and under them at half a second, where its sections start on bar lines and live recordings drift. The blocks
scored their chorus by calling nearly everything a drop.

On claudius's own house song of the cookbook (intro 8, groove 16, break 8, build 8, drop 16, outro 8), it hears all
six sections to the bar, the intro and the build as one music and the groove, the drop and the outro as another, and
names them intro, drop, break, build, drop, drop; the blocks heard a drop of 24 bars, the break, and a drop of 32.

## Consequences

- A remake's hook is taken from the choruses (the kind a song of claudius's plays as its hook), and a rework's and a
  rebuild's sections are named for what they are (`verse2`, `pre_chorus`, `chorus3`).
- A flip still goes through the recording in pieces as long as the song's sections; that a build plays a stretch of
  the recording that ends where its chorus starts, so the drop lands on the recording's resolution, is the next step
  (the remix's `go_through`).
- Without the ears the voice is unknown, and an instrumental between verses can be taken for a verse.
- Methods like these find a section's start within half a second about a quarter of the time on the Harmonix Set
  (structural features and spectral clustering: 0.26 to 0.28 of the hits, F-measure), and name no function, only
  letters. All-In-One (Kim and Nam 2023; code, weights and the annotations it learnt from all MIT) finds them 0.60 to
  0.66 of the time and names intro, verse, chorus, bridge, instrumental and outro, from 0.3 million parameters a fold;
  it learnt from HTDemucs stems, and claudius separates with Spleeter. Ported to Rust, it is the reader to use with
  the ears, once measured on claudius's stems; these rules stay for a song without them, the browser's included, and
  for what the model does not say: the sung phrases, the cuts, the builds and the pre-chorus.
