# 0021 Every kind of sample, ranked and varied

Status: proposed

## Context

The Sample window offers a recording's sung phrases: up to four that it sings whole, each as it was cut and through a
treatment, the best of them on the first take and a seeded draw among the near-best on each other
(`sampler::pieces`). A remake's flip takes more from a recording: its hook, another passage, its drums as a break,
and bits of it from all over (`remix`). The two choose in different ways, and neither offers what producers take most.

Of 42,447 samples on WhoSampled, 37% are drum loops, 34% bass lines, 17% hooks, 7% whole tracks and 2% vocals (Bryan &
Wang, ISMIR 2011). A list of four phrases from the same chorus is also less use than four different things: what is
offered should be good of its kind, and varied.

## Decision

The Sample window, `sample_pieces` and the auto-sampler offer pieces of six kinds, cut on the recording's downbeats
from its stems (its vocals, drums, bass and the rest; from the mix without the ears), each scored for what makes a
piece of its kind good, and a varied set of them.

| Kind | What it is | From | Scored by |
|---|---|---|---|
| Sung phrase | a phrase sung whole, as now | vocals | how deep the quiet is at its cuts, isolation, repetition, salience, how steady its pitch is |
| Hook loop | 1, 2, 4 or 8 bars of the section repeated most and most completely | the instruments, the voice too when it is the hook | how well it stands for the repetitions (thumbnail fitness, Müller, Jiang & Grosche 2013), loopability, salience, isolation |
| Passage loop | 1 to 8 bars from elsewhere | the mix, the instruments or one stem | loopability, isolation, how unlike the hook it is |
| Drum break | the drums alone, a bar at least and 2 to 4 at best; with the bass, said so | drums, bass | isolation, how busy it is, loopability |
| Bass line | 1 to 4 bars of the bass alone | bass | isolation, loopability, how much it moves |
| Stab | one sharp onset of a clear chord and a beat of its decay | the instruments | attack, how clear its chord is, a clean tail, isolation |

**Rules before scores**, never relaxed: no cut inside a sung note or a word (`cuts`); each cut moved back to the
loudness minimum before it, then to a zero crossing, with a short pre-roll so a downbeat keeps its transient; a loop's
ends within 30 ms of their downbeats. A stem is alone only when every other is inactive, under -20 dB (as Cheston et
al. take it), and it is about 20 dB over them.

**Loopability**, as LoopMaker measures it (Shi & Mysore, CHI 2018): how much the loop's first beats resemble the beats
after its end, over 4-beat chunks, 1.0 × the chroma cosine + 0.6 × the MFCC distance + 0.2 × the loudness difference, in
the stems it is cut from. The chord change from its end back to its start should be one the recording makes; notes
ringing on past its end cost.

**Quality**: a weighted geometric mean of the piece's features, each on a scale of 0 to 1 fixed for it (isolation
from -10 to 20 dB, how busy a break is and how much a bass line moves up to a half of its beats, the rest as they
are) and none under a fifth, so that one weak feature lowers a piece without ruling it out. A scale rather than a rank
among the candidates, so that a hair's more loopability does not count as much as 20 dB of isolation, and so that the
pieces of different kinds can be weighed against each other.

**Variety**: a greedy DPP (Chen, Zhang & Zhou, NeurIPS 2018) over L = diag(q^a) S diag(q^a), where q is the quality
and S the largest of the pieces' overlap in time, their being of the same section and stem, and the cosine of their
features. The first take is that list; each other take is a seeded draw (Gumbel-top-k, Kool et al., ICML 2019) among
the pieces above a quality bar, those shown before weighed down. The reply says the seed, as now.

**How many of each**: the WhoSampled shares by default, moved toward what the genre's producers sample (its sample
search: "funk drum break" asks for breaks) and what the person asks for: the words to find, or the kinds named.

**Each piece says what it is**: its kind, where it is in the recording, its length, key and tempo ("a 4-bar drum
break, the drums alone, 92 BPM, bars 33-36"), and in words why it scored as it did.

**A piece kept plays as its kind does**: a phrase as a vocal loop, as now; a hook or passage loop as a loop where the
song's chords and keys play; a break as a drum loop beside the genre's drums or in their place; a bass line in place
of the bass; a stab struck as a drum lane, as now.

**Advice, not rules.** The scores decide what is offered first. They never hide a kind the person asks for, and the
person or their assistant can ask for any kind and any number, and read why each piece scored as it did.

## Steps

1. The kinds and their scores in the Sample window and `sample_pieces`, each piece with its kind (`claudius_api`),
   and `kinds` to ask for some.
2. Variety: the DPP, the seeded draws, the shares of each kind.
3. Pieces kept, placed by their kinds.
4. Chop sets of breaks and loops at their onsets (SuperFlux, Böck & Widmer, DAFx 2013), laid out on pads: today's
   chops cut a phrase in eighths.
5. The flip and `sample_song` choose by the same scores.

## Evaluation

- Cut safety on JamendoLyrics, as `eval_cuts` measures it now: no cut inside a word.
- Loops: songs built from loops of the Freesound Loop Dataset, whose boundaries are known, found within 30 ms; a
  listening test counts audible jumps.
- Hooks: top-1 and top-3 overlap with the chorus labels of Harmonix.
- Isolation: how often "the drums alone" and "a cappella" are right, on MUSDB18 and MedleyDB with their true stems.
- What producers took: on Sample100, whether a piece among the first few overlaps what was sampled, and is its kind.
- Variety at the same quality: how unlike each other a list's pieces are, the sections, stems and kinds it covers, how
  much two takes overlap; then a preference test against today's sampler.

## Consequences

- Sampling a recording offers what producers take from records, and not only its voice, so a song can be built on a
  break or a bass line as well as on a phrase.
- The thresholds (250 ms, 10 dB, 20 dB, 6 dB, 30 ms) are where to start, tuned on the evaluation sets; only the -20 dB
  inactivity comes from the literature.
- Each step is its own pull request with its tests; the Sample window shows each kind as it comes.
