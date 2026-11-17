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
  voice sings (the words `cuts` hears in the vocals).
- **A section starts** where what came before stops being like what comes after: the novelty of the bars'
  similarity, in harmony and in sound (Foote 2000); where the music that repeats changes (structural features, Serrà
  et al. 2014); where the voice, the kit or the loudness comes in or goes. Of every way to cut the bars, the one whose
  boundaries are most novel, less a cost for each, under a preference for 8 and 16 bars that never forbids another
  length: a clear boundary at 17 bars outweighs it (dynamic programming over the bars). Each change is scaled to the
  song's clearest, but never by more than what a real change measures, so a steady loop's noise is no boundary.
- **Sections that play the same music**, in the same key or up to two semitones away (a last chorus a step up), share
  a letter: they start and go on alike in their chords and their sound (average linkage).
- **What each is**: in a song a voice sings, the chorus is the music it sings again and again that is loudest and
  sung most, ends the singing and does not start it; the verse is the other music it sings again, that leads to the
  chorus; a pre-chorus comes between, a post-chorus is loud music no one sings right after a chorus, a bridge is sung
  once after a chorus, the intro and outro play before and after the voice, an instrumental or a break between
  (Goto 2006 finds a chorus by its repeats; Paulus, Müller and Klapuri 2010 review the methods). A track no one sings
  in keeps intro, build, drop, break and outro, by loudness, with the music a drop plays a drop wherever it plays. With
  no voice heard apart, a song is taken as sung wherever it plays fully.
- **A build** is the bars at the end of a section that rise into a louder one, bar by bar; it resolves where the next
  section starts. **A cut** is where a piece of a section is best taken from the recording: its first bar's line, or
  the quiet before its first phrase when that is sung before the bar.
- `analyze` says all of it, the phrases sung to the beat as a DAW writes positions (`27.2-31.4`), and a section's
  label maps to the kinds a song of claudius's plays (a chorus as its hook, a pre-chorus as a build), so the blueprint
  does not change.

## Evaluation

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
