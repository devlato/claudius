# earshot (Rust port)

A Rust port of **[earshot](https://github.com/Leonxlnx/earshot)** by Leon Lin (MIT, commit
`108763c62648c78dbac14f5bebcd3c61ac7931cd`): "ears" for code that cannot listen. It gives claudius calibrated
CLAP text matching, AudioSet tags, a real-vs-synthesized judgement, objective measurements and a beat grid,
with **no Python at runtime**. The two neural models run as fp32 ONNX exports on ONNX Runtime (via `ort`, linked
statically). All the DSP (mel front ends, resampling, loudness, onsets, tempo, key, ...) is reimplemented.
ffmpeg is used for decoding, exactly as earshot does.

earshot's copyright notice is kept in [LICENSE-earshot](LICENSE-earshot). The models keep their own licences; see
[Licensing](#licensing).

## What is ported

| earshot module | Rust | notes |
|---|---|---|
| `audio.py` (`_decode`, `load`, windows) | `audio` | ffmpeg f32le at 48 kHz. Stereo decode duplicates mono sources at unity gain. `load_mono` is ffmpeg's `-ac 1`. `downmix(l, r)` reproduces that downmix in memory: **0.70710677·(L+R)**, measured for wav/flac/mp3/aac at 16/24/32-bit (residual 2e-8). `mean_mono` = numpy `x.mean(axis=1)`. |
| `models/clap.py` + `vocab.py` embedding | `clap`, `Ears::embed*` | The ClapFeatureExtractor front end, bit-for-bit up to f64 rounding: repeat-pad, reflect centring, periodic Hann 1024 / hop 480, Slaney mel 50 Hz–14 kHz, `power_to_db` floor 1e-10. Also `_prep` (<480 samples, `zero` pad) and the `repeatpad` / `zero` / `spaced` modes. Windowing follows `spans_for` / `embed_spans` / `embed_files`: ≤10 s is one window; longer files get 10 s windows with a 5 s hop, the last flush with the end, at most 6 evenly spaced; window embeddings are averaged and renormalised. Text: tokenizer + text encoder, unit vectors, averaged over the 5 templates. |
| `vocab.py`, `match.py` | `vocab`, `matching` | `vocab.json` and `vocab_stats.json` are embedded (`include_str!`). Covered: reference-mean bias removal, per-sound z over the 322 terms, percentile, `rank_in_vocab`, verdict bands (strong ≥3, plausible ≥2.5, weak ≥2), `p_vs_alts` softmax with `logit_scale` 38.75, multi-query forced choice, `top_terms` with category filters, `nearest_category` / UNVALIDATED, `zf`, and the match summary text. |
| `realness.py` | `realness`, `Ears::realness*` | Complete: `signal_features` (all 13), both heads, the fusion, the pure-tone rule, `lift_quiet`, clip vs file thresholds, 4 s windows for sounds over 6 s, facts / silence / CLAP-prompt evidence, and the summary / note / segment text. |
| `models/ast.py` | `ast`, `Ears::audioset*` | Uses the **numpy path** transformers takes, because torchaudio is not installed in earshot's venv: frames of 400 / hop 160, DC removal, pre-emphasis 0.97, symmetric Hann, 512-point FFT, kaldi mel (128 bands, 20 Hz–8 kHz, triangles in mel space), log with floor 1.19e-7, 1024 frames, (x+4.2677)/(2·4.5690). Then 16 kHz, 10.24 s windows with a half-window hop and the last flush, max or mean reduction, sigmoid, labels from `config.json`. |
| `features.py` (`measure` and helpers) | `features`, `librosa` | Every field. BS.1770 LUFS / LRA / short-term, true peak (scipy `resample_poly` 4×), Welch spectrum, bands, bandwidth, brightness (scipy STFT), envelopes. librosa 1.0 ports: `onset_strength`, `onset_detect` with backtracking, `beat_track` with its tempogram tempo, `chroma_cqt` with `estimate_tuning` / piptrack / VQT with per-octave 2:1 resampling, `effects.hpss` (+ istft), `yin`, `rms`. |
| `compare.py` | `compare`, `typicality`, `describe::compare_files` | `feats`, `active_lufs`, `active_length`, `_typicality`, `differences` in words, robust-z feature comparison. |
| `search.py` | `search`, `Ears::find*`, `Ears::rank`, `describe::rank_files` | `rank` and `find` with every helper: `event_candidates`, `window_candidates`, `snap_window`, `multi_sound`, `query_kind`, `_select`, `_dedupe`, edit gaps, head/tail cuts, refinement, two-stage search over 60 s. Returns trims (start, end, z, verdict, ...). |
| `timeline.music_grid` | `grid` | `_pulse_envs`, `_period`, `_downbeat_phase`, the DP beat tracker, `tempo_alt`, and confidence, including earshot's float32 storage points. |
| `describe.py` | `describe` | Full describe (tags, hints, per-category, AudioSet, character, measurements, distinct windows) and its summary text. |
| `doctor.py` | CLI `doctor` | ffmpeg / ffprobe, cache, models; `--load` times the models. |

**Not ported** (out of scope): video, sheet, critique / profile (launch-film scorecards), llm, forge,
mcp_server, the rest of timeline (events, sections, sync), and earshot's on-disk decode / embedding caches.
Only text embeddings are cached here; decoding a file again is cheap.

### Resampling without soxr

earshot resamples with python-soxr (`HQ`), and libsoxr is LGPL. Measurement shows that soxr HQ's effective
decimation filter is a linear-phase **Kaiser-windowed sinc**: cut-off 0.47843591 cycles per output sample,
β 13.0400706, half-length 96.833 output samples (2:1 uses β 13.0515 / 97.29). `dsp::resample` evaluates
exactly that, with zero signal outside the input, no delay, and output length `round(n·out/in)`. That matches
soxr within **1.2e-7** for 48→16 kHz, **2.1e-7** for 48→32 kHz and 2:1, and **3–4e-4** of full scale for
48→22.05 kHz (soxr interpolates its coefficients for that ratio; on low-passed content the error is 8e-5).
No soxr code or tables are used.

## Public API

```rust
pub const SR: u32 = 48000;

pub mod audio {
    pub fn load_stereo(path: &Path) -> Result<(Vec<f32>, Vec<f32>)>;   // true levels, mono sources duplicated
    pub fn load_mono(path: &Path) -> Result<Vec<f32>>;                 // ffmpeg -ac 1 (model input convention)
    pub fn downmix(l: &[f32], r: &[f32]) -> Vec<f32>;                  // 0.70710677 * (L + R), = ffmpeg -ac 1
    pub fn mean_mono(l: &[f32], r: &[f32]) -> Vec<f32>;                // (L + R) / 2 (what timeline feeds music_grid)
    pub fn ffmpeg() -> PathBuf; pub fn ffprobe() -> PathBuf; pub fn list_media(..); pub fn windows(..); ...
}
pub mod features {
    #[derive(Serialize)] pub struct Measure { duration, peak_dbfs, true_peak_dbtp, rms_dbfs, lufs, short_term_max_lufs,
        lra_lu, clipped_samples, dc, noise_floor_dbfs, dynamic_range_db, digital_silence_frac, side_to_mid_db,
        correlation, attack_at, peak_at, attack_ms, decay_ms, centroid_hz, rolloff95_hz, bands_db, bandwidth_hz,
        flatness, brightness_hz, tonal_peak_ratio_db, harmonic_ratio, f0_hz, onsets, onset_count, onsets_per_s,
        tempo, beats, beat_ibi_std_ms, key, error }      // Option<_> where Python may omit the key
    pub fn measure(l: &[f32], r: &[f32], music: bool, light: bool) -> Measure;
    pub fn integrated_lufs(l: &[f32], r: &[f32]) -> f64;
    pub fn true_peak_dbtp(l: &[f32], r: &[f32]) -> f64;
    pub fn lra(l: &[f32], r: &[f32]) -> Option<f64>;
    pub fn loudness_curve(l: &[f32], r: &[f32], win: f64, hop: f64) -> (Vec<f64>, Vec<f64>);
    pub fn onsets(mono: &[f32]) -> Vec<f64>; pub fn key_estimate(mono: &[f32]) -> Vec<KeyGuess>; pub fn text(..) -> String;
}
pub mod grid {
    pub struct Grid { pub tempo: Option<f64>, pub tempo_alt: Option<f64>, pub beats: Vec<f64>, pub downbeats: Vec<f64>,
                      pub pulse_clarity: Option<f64>, pub beat_regularity: Option<f64>, pub confidence: String,
                      pub note: String }
    pub fn music_grid(mono: &[f32]) -> Grid;
}
pub struct EarsOptions { pub cache_dir: Option<PathBuf>, pub threads: usize, pub load_ast: bool, pub offline: bool } // impl Default
pub struct Ears { .. }   // Send + Sync
impl Ears {
    pub fn load(opts: EarsOptions) -> Result<Ears>;
    pub fn embed(&self, mono: &[f32]) -> Result<Vec<f32>>;                        // 512-d unit, embed_files semantics
    pub fn embed_many(&self, monos: &[&[f32]]) -> Result<Vec<Vec<f32>>>;           // batched across arrays
    pub fn embed_with(&self, mono: &[f32], pad: Pad, max_win: usize) -> Result<Vec<f32>>;
    pub fn embed_spans(&self, x: &[f32], spans: &[(f64, f64)], pad: Pad) -> Result<Vec<Vec<f32>>>;
    pub fn embed_text(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;             // template-averaged
    pub fn embed_text_raw(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;         // single prompt (realness prompts)
    pub fn scores(&self, emb: &[f32], queries: &[&str]) -> Result<Vec<Score>>;    // cosine, z, percentile, rank_in_vocab, vocab_size
    pub fn match_query(&self, emb: &[f32], query: &str, alts: &[&str]) -> Result<Match>; // + verdict, validated, p_vs_alts, alts, closest_vocab
    pub fn match_queries(&self, emb: &[f32], queries: &[&str], alts: &[&str]) -> Result<Vec<Match>>;
    pub fn top_terms(&self, emb: &[f32], k: usize, exclude_categories: &[&str]) -> Vec<Term>;
    pub fn realness(&self, mono: &[f32], emb: Option<&[f32]>) -> Result<Realness>;
    pub fn audioset(&self, mono: &[f32]) -> Result<Vec<f32>>;                       // 527 probabilities (max over windows)
    pub fn audioset_reduce(&self, mono: &[f32], reduce: Reduce) -> Result<Vec<f32>>;
    pub fn audioset_labels(&self) -> &[String];
    pub fn find(&self, mono: &[f32], query: &str, top: usize) -> Result<Vec<Trim>>;  // start, end, z, verdict, kind, cut, ...
    pub fn rank(&self, embs: &[Vec<f32>], query: &str) -> Result<Vec<(usize, Score)>>;
    pub fn query_kind(&self, query: &str) -> Result<(String, f64)>;
    // file conveniences (mirroring the Python API)
    pub fn describe(&self, path: &Path) -> Result<describe::Description>;
    pub fn match_file(&self, path: &Path, queries: &[&str], alts: &[&str]) -> Result<describe::FileMatch>;
    pub fn find_file(&self, path: &Path, query: &str, top: usize, opt: &FindOptions) -> Result<FindResult>;
    pub fn realness_file(&self, path: &Path) -> Result<Realness>;
    pub fn embed_file(&self, path: &Path) -> Result<Vec<f32>>;
}
pub fn typicality(emb: &[f32], refs: &[Vec<f32>], k: usize) -> Typicality;        // mean_cos_knn, z, percentile, k, verdict
pub fn measure_file(path: &Path, music: bool, light: bool) -> Result<Measure>;
// describe::rank_files(&ears, &paths, query, top), describe::compare_files(&ears, path, &refs, k): Python-shaped results
```

Every result struct derives `Serialize`; the JSON keys match earshot's.

### CLI

```text
earshot [--threads N] [--offline] <command> [args] [--json]
  describe PATH [--top N]                      calibrated tags, AudioSet labels, measurements
  match PATH QUERY... [--alts ALT...]          calibrated z + verdict (+ forced choice)
  rank QUERY PATH... [--top N]                 rank files / folders by a description
  find PATH QUERY [--top N] [--kind auto|event|texture] [--win S] [--hop S]
  measure FILE... [--music] [--light]          objective measurements (no models)
  realness PATH...                             real / pro-produced vs code-synthesized
  grid PATH                                    tempo, beats, downbeats, pulse clarity (no models)
  compare PATH --refs REF... [-k K]            nearest references, typicality, feature differences
  doctor [--load]                              ffmpeg, cache, models (--load: time them)
```

Text output reproduces earshot's plain-language summaries (`measure`, `match`, `describe`, `compare`, `rank`,
`find` and `realness` were checked byte for byte against the Python CLI on the fixtures). Errors print
`earshot <cmd>: error: ...` to stderr and exit 1; with `--json` the error is also printed as JSON on stdout.

## Models

| model | Hugging Face repo @ pinned revision | files | size |
|---|---|---|---|
| CLAP (laion/larger_clap_general, fp32 ONNX) | `Xenova/larger_clap_general` @ `d78c3994912c441f0151a5cc44d86f6b7bf4c861` | `onnx/audio_model.onnx`, `onnx/text_model.onnx`, `tokenizer.json` | 282 + 502 + 2 MB |
| AudioSet AST (MIT/ast-finetuned-audioset-10-10-0.4593, fp32 ONNX) | `Xenova/ast-finetuned-audioset-10-10-0.4593` @ `249a1fbf0286b40e7f1ed687a8ae396997bf7dc6` | `onnx/model.onnx`, `config.json` | 347 MB |

Files are downloaded on first use from `https://huggingface.co/{repo}/resolve/{revision}/{file}` into
`$EARSHOT_HOME` (default `~/.cache/earshot-rs`), under `models/<repo with / as -->/<revision>/<file>`.
Each download goes to `<file>.<pid>.part`, is length-checked, fsynced and renamed into place, so an
interrupted download never leaves a corrupt model. `EARSHOT_OFFLINE=1` (or `EarsOptions::offline`) refuses to
download and fails with a clear message. The ONNX I/O, inspected with `ort`:

- CLAP audio: `input_features [B, 1, 1001, 64] f32` → `audio_embeds [B, 512]`. No `is_longer` input: fusion is off.
- CLAP text: `input_ids [B, T] i64` → `text_embeds [B, 512]`. There is **no attention mask**, so texts are
  batched by equal token length instead of being padded.
- AST: `input_values [B, 1024, 128] f32` → `logits [B, 527]`.

Text embeddings (raw, per prompt) are cached in `$EARSHOT_HOME/text/clap-d78c3994912c.bin`. The cache is
merged with what other processes wrote and replaced atomically. The 322 terms × 5 templates are computed once.
Because the 500 MB text encoder only loads on a cache miss, a warm start never loads it.
`clap.unload_text()` releases it again.

## Parity with the Python earshot

`parity/make_fixtures.py` (dev only, run with the claudius venv) writes synthetic media into `parity/media/`:
sine, noise burst, click, sweep, kick, harmonic tone, and a 15 s drum loop (1.5 MB in total). It also uses four
Apple Loops by absolute path (`4th Dimension Beat 01`, `Phantom Pulse Bass`, `Anders - Verse` (24 s),
`Dusky Blues Guitar Chords`), skipped when missing. It dumps the Python results to `parity/fixtures/*.json`.
`tests/parity.rs` compares against them. The oracle ran CLAP and AST in fp32 on this M1 (earshot uses bf16
only on CPUs with native bf16).

Results on all 11 inputs:

| check | tolerance | result |
|---|---|---|
| decode (stereo / mono lengths, sums), in-memory `downmix` vs ffmpeg `-ac 1` | exact / 1e-6 | identical; downmix max err < 1e-6 |
| resampler vs soxr HQ (48→16/32 kHz; 48→22.05 kHz) | – | max \|d\| 1.2e-7 / 2.1e-7; 3.2–4.4e-4 |
| CLAP log-mel front end (sampled rows / columns, sum) | – | max \|d\| **3.6e-15 dB** |
| CLAP audio embedding cosine | ≥ 0.999 | **1 − cos ≤ 3.0e-10** on every input (typically 1e-11; incl. 2- and 4-window files) |
| CLAP text embedding cosine (raw and templated), token ids | ≥ 0.999 | **1 − cos ≤ 3.1e-12**, token ids identical |
| z-scores (10 queries × 11 inputs, incl. house / techno / "professionally mixed and mastered") | ≤ 0.05 | **0.000** (identical after rounding to 0.01), percentile and rank identical |
| match verdicts, p_vs_alts, multi-query forced choice, validated flag | – | identical |
| top terms (k = 8, quality / production excluded) | – | identical lists |
| AST probabilities (527 classes) | ≤ 0.02 max abs | **≤ 2.1e-5** (sine), typically 1e-6 |
| realness p_real / verdict / per-window segments | ≤ 0.02 | **identical** (summary text byte-identical) |
| features.measure: LUFS, true peak, RMS, LRA, short-term max, noise floor, bands, centroid, rolloff, bandwidth, brightness, flatness, attack / decay, correlation, side/mid, dc | 0.1 dB etc. | **identical** on every input |
| measure librosa parts: onsets, onset rate, HPSS harmonic ratio, yin f0, tempo, beats, key (+ r) | – | **identical** (e.g. 50/50 onsets, 29/29 beats, Eb major r 0.366) |
| music_grid tempo / beats / downbeats / pulse clarity / confidence | ≤ 0.5 BPM | **identical** (120.0, 80.0, 154.1 BPM) |
| compare feats + differences in words | – | identical |
| typicality (k = 3, every input vs the rest) | – | \|Δz\| ≤ 2.3e-5 |
| rank (7 files, "a kick drum") | – | identical order and z |
| find trims (4 files) | – | identical trims and z, except one tie (see below) |
| describe summaries (3 files) | – | byte-identical |

### Known deviations

- **find ties**: when candidates have equal rounded z, Python orders them by numpy's unstable `argsort` and
  Rust by a stable sort. On the 15 s loop, picks 3–5 are tied at z 4.13 / 4.12, so the same moments come back
  in a different order and one of three equally scored trims is swapped. Scores are unaffected.
- **realness signal measurements on noiseless synthetic signals**: soxr leaves float32 round-off noise
  (around −140…−170 dB) in its output. For a pure sine, a 1 ms click or a clean sweep, that noise *is* the
  measured value of `log_flatness`, `peakiness_db`, `spec_floor_db`, `quiet_flat`, and of the nonzero-sample
  count behind `crest_db` / `texture_kurt`. Running Python's own `signal_features` on our resampler's output
  (within 2.4e-7 of soxr's) reproduces our numbers, so the formulas match and only the noise differs. It never
  changed p_real or a verdict: the signal head has fusion weight 0, the same facts fire, and the pure-tone rule
  is unaffected. Real recordings have noise floors far above this level. The test lists these exceptions
  explicitly.
- **48 → 22.05 kHz** (librosa parts only): up to 4e-4 of full scale from soxr (see above). Onsets, beats,
  tempo, key, f0 and HPSS still matched exactly on every fixture.
- `find(&mono, ..)` reads segment peaks from the mono array; `find_file` uses the stereo decode like earshot.
- `list_media` also accepts `.caf` (Apple Loops), which earshot's extension list lacks.
- Not ported on purpose: earshot's decode / embedding / realness result caches (results are identical without
  them), bf16 inference, `--start/--end` file ranges in the CLI (slice the arrays instead).

## Timings

Release build, Apple M1 Max, ONNX Runtime 1.31 CPU, default 4 intra-op threads (min(4, cores/2) like earshot;
`EARSHOT_THREADS` overrides), from `cargo run --release --example bench` and the parity tests. The machine was
busy (load average 13–22 from VMs and other builds), so these are conservative. On a quieter moment CLAP was
0.13–0.17 s per window as well.

| step | time |
|---|---|
| `Ears::load`, warm cache (CLAP audio + AST sessions; text encoder deferred) | 3.7 s (the CLAP audio graph alone takes ~3.5 s to initialise) |
| `Ears::load`, first run with models cached (embeds the vocabulary: 1,610 prompts, loads the text encoder) | ~27–30 s, once |
| one CLAP window (10 s), incl. the 3 ms mel front end | 0.15 s |
| CLAP, batch of 8 windows | 0.13 s / window |
| first new text after start (loads the 500 MB text encoder) / next new text | 2.5 s / 0.05 s |
| one AST window (10.24 s) | 1.0–1.4 s (0.7 s with 8 threads) |
| `realness` of a 4 s / 24 s sound | 0.2 s / 1.6 s |
| `features::measure` (music, full) on 24 s / 15 s / 4 s | 1.7 / 1.0 / 0.2 s |
| `music_grid` on 24 s | 0.11–0.15 s |
| `find` on a 15 s / 24 s recording (59 candidates) | 7.8 s / 8.4 s |

Two ONNX Runtime settings were chosen by measurement. **Graph optimisations are off**: with them the AST run
peaked at 3.3 GB instead of 0.7 GB, and CLAP at 0.6 GB instead of 0.46 GB, with no gain in speed. **AST runs one
window per call**, because batching is slower on CPU (1.3 s for one window vs 3.2 s per window in a batch of 4).
Downloading the 1.13 GB of models took 28 s on this connection.

## Build, run, test

The crate is a member of the claudius workspace; run these from the repository root.

```sh
cargo build --release -p earshot                            # library + target/release/earshot
./target/release/earshot doctor --load                      # downloads models on first use (~1.13 GB)
./target/release/earshot match kick.wav "a punchy kick drum" --alts "a snare drum hit"
cargo test -p earshot                                       # fast parity checks, no models (~7 s in debug)
cargo test --release -p earshot -- --ignored --nocapture    # model parity checks, prints every number (~70 s)
cargo run --release -p earshot --example bench              # timings
cargo run --release -p earshot --example mem -- ast 1 0 1 1 0   # ONNX Runtime memory / speed per session setting
cargo run --release -p earshot --example inspect_onnx -- ~/.cache/earshot-rs/models/*/*/onnx/*.onnx   # model I/O
.venv/bin/python crates/earshot/parity/make_fixtures.py     # regenerate fixtures (dev only)
```

`build.rs` links clang's compiler runtime (`libclang_rt.osx.a`, found via `clang --print-runtime-dir`) on
Apple targets: ONNX Runtime's prebuilt static library calls `__isPlatformVersionAtLeast` from its CoreML
provider, and rustc does not link that runtime. The directive propagates to any binary using the crate.

## Integrating into claudius

- **Dependency**: `earshot = { path = "../earshot" }`. The root workspace excludes this crate, and a path
  dependency on it works as is.
- **Threads**: `Ears` is `Send + Sync`. Load it once, for example in a background thread at startup, and share
  it via `Arc<Ears>`. Each model's session sits behind a mutex, so concurrent calls into the same model are
  serialised and each uses `threads` cores. The pure-DSP functions (`features::*`, `grid::music_grid`,
  `audio::*`, `typicality`) need no `Ears` and run in parallel freely.
- **First run**: downloads 1.13 GB (CLAP audio 282 MB, text 502 MB, AST 347 MB, tokenizer 2 MB) and spends about
  27 s embedding the vocabulary once. After that a load takes about 4 s and needs no network. Use
  `load_ast: false` to skip the 347 MB AST (then `audioset` / `describe` return an error). For a progress UI, call
  `earshot::models::set_progress(Box::new(|file, done, total| ..))` before `Ears::load`; without a callback each
  download prints one line to stderr.
- **Environment**: `EARSHOT_HOME` (cache root), `EARSHOT_THREADS`, `EARSHOT_OFFLINE=1`, and
  `EARSHOT_FFMPEG` / `EARSHOT_FFPROBE`. macOS GUI apps don't inherit the shell PATH, so the tool lookup falls back
  to `/opt/homebrew/bin` and `/usr/local/bin`.
- **Audio conventions**: everything works at 48 kHz f32. Feed models (`embed`, `realness`, `audioset`, `find`)
  ffmpeg's mono, i.e. `audio::load_mono` or `audio::downmix(l, r)` for buffers you rendered yourself; earshot's
  calibration was measured on that convention. Feed `features::measure` the stereo pair (true levels), and
  `grid::music_grid` `audio::mean_mono(l, r)` (what earshot's timeline uses).
- **Memory / binary**: peak RSS is 1.06 GB with CLAP audio + AST loaded and running. Loading the text encoder on a
  text-cache miss raises the peak to about 2.4 GB; call `ears.clap.unload_text()` to release it, and it reloads
  on the next miss. The binary is about 35 MB (ONNX Runtime is linked statically; only system dylibs and
  frameworks are dynamic).

## Licensing

- This port: MIT. It is a derivative of earshot © 2026 Leon Lin, MIT ([LICENSE-earshot](LICENSE-earshot)).
  `data/vocab.json`, `data/vocab_stats.json` and `data/realness_model.json` are copied from earshot.
- CLAP model weights: LAION `larger_clap_general`, **Apache-2.0**. ONNX export by Xenova (same licence).
- AST model weights: MIT CSAIL `ast-finetuned-audioset-10-10-0.4593`, **BSD-3-Clause**. ONNX export by Xenova.
- Libraries: ONNX Runtime (MIT, statically linked via `ort`, MIT/Apache-2.0), `tokenizers` (Apache-2.0),
  `realfft` / RustFFT (MIT/Apache-2.0), `ureq` with rustls (MIT/Apache-2.0), serde (MIT/Apache-2.0).
- No LGPL code is used: the soxr-equivalent filter is an independent Kaiser-sinc design whose parameters were
  fitted to measured responses. librosa (ISC) and scipy (BSD-3) algorithms are reimplemented, not copied.
- Model files are downloaded at runtime and never stored in this repository.
