# Credits

claudius is built on other people's work: the libraries it is made of, the code it ports, the models it listens with,
the fonts it draws with, the sounds it plays and the research its engine follows. This page names them, each under
its licence. Every library is on [Libraries](credits/libraries.md), which the book writes from Cargo.lock.

## Models

claudius runs these on the computer itself, on [ONNX Runtime](https://onnxruntime.ai) or, for Cactus's models,
[needle-infer](https://crates.io/crates/needle-infer). Each is downloaded from Hugging Face the first time something
needs it, and none of them writes music: they listen, tag and read
([design 0011](design/0011-meaning-on-device.md), [0014](design/0014-request-model.md),
[0015](design/0015-spoken-requests.md)).

| Model | What claudius does with it | From | Licence |
|---|---|---|---|
| [CLAP, larger general](https://huggingface.co/laion/larger_clap_general) | the ears: how much a sound, a phrase or a song is like words ("a person singing"), and finding sounds by meaning | LAION ([Wu et al., ICASSP 2023](https://arxiv.org/abs/2211.06687)); the [ONNX export](https://huggingface.co/Xenova/larger_clap_general) is Xenova's | Apache-2.0 |
| [AST, fine-tuned on AudioSet](https://huggingface.co/MIT/ast-finetuned-audioset-10-10-0.4593) | the ears: which of AudioSet's sounds a recording holds | MIT ([Gong, Chung & Glass, Interspeech 2021](https://arxiv.org/abs/2104.01778)); the [ONNX export](https://huggingface.co/Xenova/ast-finetuned-audioset-10-10-0.4593) is Xenova's | BSD-3-Clause |
| [Basic Pitch](https://huggingface.co/spotify/basic-pitch) | the notes played or sung in a take or a recording | Spotify ([Bittner et al., ICASSP 2022](https://arxiv.org/abs/2203.09893)) | Apache-2.0 |
| [Spleeter, four stems](https://huggingface.co/Best-Practice/spleeter-4stems-onnx) | a recording taken apart into its voice, drums, bass and the rest | Deezer ([Hennequin et al., JOSS 2020](https://doi.org/10.21105/joss.02154)), MIT; the ONNX export is Best-Practice's, Apache-2.0 | MIT, Apache-2.0 |
| [Audiobox Aesthetics](https://huggingface.co/facebook/audiobox-aesthetics) | the critic: how much listeners would enjoy a song, and how well it is made | Meta ([Tjandra et al., 2025](https://arxiv.org/abs/2502.05139)); the [ONNX export](https://huggingface.co/Microck/audiobox-aesthetics-onnx) is Microck's | CC BY 4.0 |
| [potion-base-8M](https://huggingface.co/minishlab/potion-base-8M) | finding a command, a sound or a genre by the meaning of the words for it | Minish Lab ([Model2Vec](https://github.com/MinishLab/model2vec)) | MIT |
| [Needle](https://huggingface.co/Cactus-Compute/needle3) | reading a request in plain words, in the palette | [Cactus Compute](https://github.com/cactus-compute/cactus) | Apache-2.0 |
| [Whistle](https://huggingface.co/Cactus-Compute/whistle) | hearing a request said aloud | [Cactus Compute](https://github.com/cactus-compute/cactus) | Apache-2.0 |

## Code it ports

| Crate | A port of | Licence |
|---|---|---|
| `earshot` | [earshot](https://github.com/Leonxlnx/earshot), the Python library of "ears" for code: its models run on ONNX Runtime, its signal processing (mel front ends, loudness, onsets, the beat grid) written again in Rust | MIT |
| `whistle` | Cactus's JAX reference for Whistle ([cactus-compute/cactus](https://github.com/cactus-compute/cactus) at `bafcf73`: `whistle.py`, `architecture.py`, `export.py`, `quantize.py`) and the engine's behaviour Cactus documents | Apache-2.0 |

claudius's patterns follow [TidalCycles](https://tidalcycles.org)' and [Strudel](https://strudel.cc)'s mini-notation
and spread Euclidean rhythms with Bjorklund's algorithm as they run it ([design 0004](design/0004-script-language.md)):
the notation is theirs, the implementation claudius's own, with none of their code.

## Encoders, decoders and the programs it may call

MP3, FLAC and Opus are written by [rusty_mp3](https://crates.io/crates/rusty_mp3),
[flacenc](https://crates.io/crates/flacenc) and [opus-pure](https://crates.io/crates/opus-pure), and Ogg Vorbis by
[libvorbis](https://xiph.org/vorbis/) with [aoTuV](https://ao-yumi.github.io/aotuv_web/)'s and Lancer's tuning
(`aotuv_lancer_vorbis_sys`, BSD-3-Clause), built in ([Exporting](guide/export.md)).
[symphonia](https://github.com/pdeljanov/Symphonia) reads the recordings claudius opens. AAC and Apple Lossless come
from the system's encoder, Core Audio's `afconvert` on macOS, and [ffmpeg](https://ffmpeg.org) elsewhere and for a file
symphonia cannot read, when it is installed: claudius runs them as programs and ships neither.

## Fonts and icons

| Font | Where | Licence |
|---|---|---|
| [Inter](https://rsms.me/inter/) | the studio's text | SIL Open Font License 1.1 |
| [Lucide](https://lucide.dev) | the studio's icons | ISC |
| the fonts egui ships ([epaint_default_fonts](https://crates.io/crates/epaint_default_fonts): Hack, Noto Emoji, Ubuntu, and emoji-icon-font) | the studio, for what Inter does not cover | each its own, as that crate lists them |

## Sounds

The [sample packs](guide/packs.md#the-packs) are public domain, CC0 and CC BY recordings, each credited as its licence
asks ([their credits](guide/packs.md#credits), [design 0007](design/0007-sample-licensing.md)). A song that samples a
recording from [Freesound, Wikimedia Commons, Openverse, ccMixter or Jamendo](guide/online-samples.md#the-libraries)
keeps that recording's credit with what plays it ([credits](guide/online-samples.md#where-they-go-and-credits)).

## Research

What the engine draws on, by the part of it that does. A paper new to the engine goes here in the pull request that
uses it.

### Listening to a recording

- Ellis (2007), "Beat tracking by dynamic programming", *Journal of New Music Research* 36(1):
  [doi:10.1080/09298210701653344](https://doi.org/10.1080/09298210701653344). The beats.
- Krumhansl & Kessler (1982), "Tracing the dynamic changes in perceived tonal organization in a spatial representation
  of musical keys", *Psychological Review* 89(4):
  [doi:10.1037/0033-295X.89.4.334](https://doi.org/10.1037/0033-295X.89.4.334). The key.
- de Cheveigné & Kawahara (2002), "YIN, a fundamental frequency estimator for speech and music", *JASA* 111(4):
  [doi:10.1121/1.1458024](https://doi.org/10.1121/1.1458024). The pitch of a voice.
- Mauch et al. (2015), "Computer-aided melody note transcription using the Tony software: accuracy and efficiency",
  TENOR 2015: [paper](https://www.tenor-conference.org/proceedings/2015/04-Mauch-Tony.pdf). The notes a voice sings.
- Moulines & Charpentier (1990), "Pitch-synchronous waveform processing techniques for text-to-speech synthesis using
  diphones", *Speech Communication* 9(5-6):
  [doi:10.1016/0167-6393(90)90021-Z](https://doi.org/10.1016/0167-6393(90)90021-Z). TD-PSOLA, which moves a voice to
  the song's key with its formants kept.
- ITU-R BS.1770, "Algorithms to measure audio programme loudness and true-peak audio level":
  [recommendation](https://www.itu.int/rec/R-REC-BS.1770). Loudness.

### Sampling a recording

- Bohak & Marolt (2012), ISMIR 2012: [paper](https://ismir2012.ismir.net/event/papers/451_ISMIR_2012.pdf). A singer's
  rests part the phrases.
- Nakano, Ogata, Goto & Hiraga (2008), "Analysis and automatic detection of breath sounds in unaccompanied singing
  voice", ICMPC 2008: [paper](https://staff.aist.go.jp/t.nakano/PAPER/ICMPC2008nakano.pdf). And so do their breaths.
- Jehan (2005), "Creating music by listening", PhD thesis, MIT:
  [thesis](https://dspace.mit.edu/handle/1721.1/42172). A cut goes back to the quiet before an onset.
- Kool, van Hoof & Welling (2019), "Stochastic beams and where to find them: the Gumbel-top-k trick for sampling
  sequences without replacement", ICML 2019: [arXiv:1903.06059](https://arxiv.org/abs/1903.06059). Another take of the
  pieces, and of a flip, drawn among the good ones.
- Stoller, Durand & Ewert (2019), "End-to-end lyrics alignment for polyphonic music using an audio-to-character
  recognition model", ICASSP 2019: [arXiv:1902.06797](https://arxiv.org/abs/1902.06797). JamendoLyrics, the songs whose
  every word is timed, which measure how often a cut falls inside one.

### Rhythm and feel

- Toussaint (2005), "The Euclidean algorithm generates traditional musical rhythms", Bridges 2005:
  [paper](https://cgm.cs.mcgill.ca/~godfried/publications/banff.pdf). Euclidean rhythms.
- Longuet-Higgins & Lee (1984), "The rhythmic interpretation of monophonic music", *Music Perception* 1(4):
  [doi:10.2307/40285271](https://doi.org/10.2307/40285271). Syncopation.
- Hennig et al. (2011), "The nature and perception of fluctuations in human musical rhythms", *PLOS ONE* 6(10):
  [doi:10.1371/journal.pone.0026457](https://doi.org/10.1371/journal.pone.0026457). The drift a player's timing
  wanders with.

### The critic

- Chmiel & Schubert (2017), [doi:10.1177/0305735617697507](https://doi.org/10.1177/0305735617697507); Askin &
  Mauskapf (2017), [doi:10.1177/0003122417728662](https://doi.org/10.1177/0003122417728662). Liking follows an
  inverted U over how familiar and how complex a song is.
- Rolison & Edworthy (2012), [doi:10.1525/mp.2012.29.3.269](https://doi.org/10.1525/mp.2012.29.3.269); Karno & Konečni
  (1992), [doi:10.2307/40285538](https://doi.org/10.2307/40285538); Deliège (1987),
  [doi:10.2307/40285378](https://doi.org/10.2307/40285378). Form and the grouping a listener hears.
- Montecchio, Roy & Pachet (2020), [doi:10.1371/journal.pone.0239418](https://doi.org/10.1371/journal.pone.0239418).
  Where listeners skip.
- Solberg & Jensenius (2016), [doi:10.18061/emr.v11i3-4.5023](https://doi.org/10.18061/emr.v11i3-4.5023); Solberg &
  Dibben (2019), [doi:10.1525/mp.2019.36.4.371](https://doi.org/10.1525/mp.2019.36.4.371). The build and the drop.
- Cheung et al. (2019), [doi:10.1016/j.cub.2019.09.067](https://doi.org/10.1016/j.cub.2019.09.067). Uncertainty and
  surprise.
- Tjandra et al. (2025), [arXiv:2502.05139](https://arxiv.org/abs/2502.05139). Audiobox Aesthetics, above.

The [critic's page](engineering/critic.md) says what each of them does in it.
