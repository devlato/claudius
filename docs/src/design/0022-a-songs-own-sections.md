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

The analysis hears a song's own sections (`analysis/structure.rs`), with no model of its own: signal processing on
what it already reads, the stems when the ears separate them, and a small tagger that tells a voice in them from what
the separation left in the vocals.

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
- **The voice** is where `cuts` hears it in the separated vocals and the ears' AudioSet tagger hears a voice there
  too (`stems::voiced`): a guitar riff or a synth line the separation left in the vocals sings nothing. The tagger is
  CED-Tiny (Dinkel et al. 2024; 5.5 million parameters, Apache-2.0), asked about the phrases a few lines at a time
  (those less than a second apart, up to the 10.11 s it hears at once), and a stretch is a voice's from 0.03, chosen
  on half of JamendoLyrics' songs and confirmed on the other half (Evaluation).
- **What the voice sings**, beat by beat, where the stems are apart, counts as much as the chords: in the runs that
  come back, so a chorus's tune coming back marks where it starts, and between two sections both sung. Many songs play
  one loop of chords all through, and only the tune tells their verse from their chorus.
- **Sections that play the same music**, in the same key or up to two semitones away (a last chorus a step up), share
  a letter: they start and go on alike in their chords, in what is sung over them where both are sung, and in their
  sound (average linkage).
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

On the first 25 songs, with the stems apart as the studio hears them: starts heard 62% (58% from the mix, 17% before),
sections on a stanza 52% (57%), choruses 51% (46%), other stanzas taken for choruses 32% (34%), stanzas sung alike as
the same music 36% (48%). Heard without its tune, the voice had found the starts less well (57%, 46% on a stanza),
named more choruses (59%) but taken more stanzas for one (44%), and put stanzas sung alike in the same music twice as
often (71%): the tune, which a voice's stem hears less steadily than the rest hears its chords, parts two choruses
sung a little differently. It is kept for what it finds that nothing else does (below).

**The voice tagger** (CED-Tiny), on the same 78 songs against when lines are sung, every 10 ms, its threshold chosen
on alternate songs and confirmed on the others: on the half it was not chosen on, precision 0.827 to 0.850, recall
0.876 to 0.874, and a voice heard more than 2 s from any line in 18.6% of that time to 12.4%, with no song's F1 down by
more than 0.02 (on all 78: 0.849 to 0.868, 0.886 to 0.884, 14.7% to 9.6%). With the stems read back from the cache both
times, every song's analysis comes out the same, byte for byte, with the tagger as without it: at 0.03 it keeps every
phrase these songs sing, and what it takes out is what no voice sings. On seven recordings with lyrics timed by LRCLIB
the far-off voice went from 53% to 43% on The Prodigy's "Firestarter", 34% to 28% and 17% to 13% on two more, and
stayed at 74% on one.

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

On the song that started this, Alizée's "J'ai pas vingt ans", one loop of chords all through, the 4-bar blocks heard
an intro, four drops of 16 to 32 bars and an outro. The new reading, its stems apart, hears an intro, a verse of 8
bars, the verse's music of 16, a chorus of 16 from bar 33, the verse's music again from bar 49, choruses on from bar 65,
and an outro; against its lyrics as LRCLIB times them, its second verse starts at bar 51 (heard at 49) and its second
chorus at bar 65 (heard at 65). Without the tune, the loop had read as 28 sections of four bars, all one music and
nearly all named chorus, and a flip's build would have landed in the middle of its first verse.

On claudius's own house song of the cookbook (intro 8, groove 16, break 8, build 8, drop 16, outro 8), it hears all
six sections to the bar, the intro and the build as one music and the groove, the drop and the outro as another, and
names them intro, drop, break, build, drop, drop; the blocks heard a drop of 24 bars, the break, and a drop of 32.

## Consequences

- A remake's hook is taken from the choruses (the kind a song of claudius's plays as its hook), and a rework's and a
  rebuild's sections are named for what they are (`verse2`, `pre_chorus`, `chorus3`).
- A flip's build plays the recording's bars that end where one of its choruses or drops starts, first those the
  recording's own builds lead into, and its drop plays on from there (`remix::landings`): the song's drop comes where
  the recording's build resolves. On "J'ai pas vingt ans" flipped into deep house, the build plays bars 25 to 32 and
  the drop the chorus from 33; on the cookbook's house song, its own build, 33 to 40, and its drop from 41.
- Without the ears the voice is unknown, and an instrumental between verses can be taken for a verse.
- Methods like these find a section's start within half a second about a quarter of the time on the Harmonix Set
  (structural features and spectral clustering: 0.26 to 0.28 of the hits, F-measure), and name no function, only
  letters. All-In-One (Kim and Nam 2023; code, weights and the annotations it learnt from all MIT) finds them 0.60 to
  0.66 of the time and names intro, verse, chorus, bridge, instrumental and outro, from 0.3 million parameters a fold;
  it learnt from HTDemucs stems, and claudius separates with Spleeter. Ported to Rust, it is the reader to use with
  the ears, once measured on claudius's stems; these rules stay for a song without them, the browser's included, and
  for what the model does not say: the sung phrases, the cuts, the builds and the pre-chorus.
