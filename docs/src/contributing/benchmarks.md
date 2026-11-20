# Benchmarks

A change to the critic, the analysis of recordings, the stems, the request model or the generator's choices cannot be
read off its diff: a threshold moved, a model taught again or a new way of writing a part may make claudius better or
worse, and usually better on some music and worse on other music. So each such change is an experiment, run on a
benchmark and compared with the last accepted change before it ships. `AGENTS.md` has the rules in short; this page
says why they are what they are and how to follow them.

## Real music is the yardstick

claudius is meant to make music that stands next to the music people listen to, and to hear recordings the way
people hear them. Both are measured on real songs. A benchmark built from claudius's own songs measures claudius
against itself: songs made before and after a change and judged by claudius's own critic tell you whether something
broke, never whether claudius got closer to real music, and tuning on them pulls claudius further from it.

A benchmark's songs are many and varied: many in each genre, across decades, languages and cultures, instruments,
and ways of mixing and producing. A handful of songs from one source shows how claudius does on that source. Each
song carries where it comes from and under what licence, and the labels it is sliced by (genre, decade, language,
source). The audio stays out of the repository; a benchmark names its songs and how to fetch them.

## A baseline, and an experiment against it

Every accepted change keeps its benchmark results as the baseline: not only the totals but each item's result (each
request read right or wrong, each song's section score), so that the next experiment can be paired with it item by
item. An experiment runs the same benchmark, at the same settings, on the same items. The baselines are kept in
`baselines/` at the root of the repository, one file a benchmark, and the pull request whose change is accepted
replaces its benchmark's file with its own results.

Pairing matters because items differ far more than versions do. Two versions' averages over 300 songs can sit within
noise of each other while one version is better on 40 songs and worse on 10; the pairs show that, and the averages
hide it.

## The statistics

Give each comparison as the difference, its 95% confidence interval and a significance test, overall and in each
slice. Which test depends on what each item yields:

| Each item yields | Test | Interval |
|---|---|---|
| right or wrong (a request read right) | McNemar's exact test on the items right in one version and wrong in the other | bootstrap over items |
| a score (a song's section F-measure, a critic's error on a song) | Wilcoxon signed-rank test on the paired differences | bootstrap of the mean difference over items |
| a measure of the whole set (agreement with people as a rank correlation, a distance between two sets of songs) | paired bootstrap: resample the items, recompute the measure for both versions | the bootstrap's percentiles |

Use at least 10,000 bootstrap resamples. Slices are many tests at once, so correct them together (Holm's method)
before saying a slice got worse. Before running, say how large a difference the benchmark can show: with too few
items a real gain is "not shown", not "no change", and the answer is more items, not a looser test.

## Comparing two runs

`cargo xtask bench compare BASE NEW` does the comparison above for two runs of a benchmark: the baseline's results
and the experiment's. Each file holds a line of JSON for each item:

```json
{"item": "a1f3c09e", "slice": {"set": "round10"}, "metrics": {"right": true, "wrong": false},
 "better": {"wrong": "lower"}, "primary": ["right"]}
```

Each line is one item, here on two lines to fit. `item` is the same id in every run of the benchmark; `slice` the
labels it is counted under; each metric is `true` or `false` (right or wrong) or a number (a score), better higher
unless `better` says lower. `primary` names the metrics the change is made to improve; the others are guards it must
not make worse (without it, every metric is primary, and with it empty, `"primary": []`, none is: the change is judged
on a measure of the whole set by a test of its own, and these are its guards), but for those `shown` names: compared
and shown, never judged, for a level of which neither more nor less is better. The command prints, for each metric, the whole set and every
slice with the difference, its interval, the items better and worse and the test, then the ids of the items that got
worse, as Markdown for the pull request. Which way a metric went is its test's: for a score, whether the ranks of the
items that got better outweigh those of the items that got worse, so that a few items much better cannot carry a
change most items got worse with. It fails unless each primary metric is better overall, its mean too, and no metric
is worse overall or in a slice after Holm's correction, and when there is nothing to compare (no items, or a primary
metric no item has); `--report` prints the same without failing. Its paths are taken from the root of the
repository, as every `cargo xtask` path is.

## When a change ships

A change ships when it improves its benchmark significantly and no slice gets significantly worse. Name the items
that got worse even then, and look at them: a song made worse is a song a person will hear. When the change does not
pass, iterate on it until it does. Never move the bar, drop items or loosen a test to get a pass.

Settings are chosen on one part of a benchmark and the result is reported on another part the tuning never saw.
Some sets are sealed: written before the version they measure and read only through their scores, so that nobody
working on claudius learns their items. Never print, open or quote a sealed item.

## In the pull request

The pull request names the benchmark and the baseline it was compared with, and shows the comparison: the
difference with its interval and test, each slice, and the items that got worse. When the change is accepted, its
results become the next baseline.

## Benchmarks today

- **The request model**: the rounds in `crates/claudius-intent/tests/asks.rs`, typed by people of many kinds before
  the version they measure. The open rounds print what they miss; the sealed ones only their scores.
- **The critic on real songs**: `cargo run --release -p claudius --example eval_critic -- DIR` judges songs from
  MTG-Jamendo (independent artists' releases under Creative Commons licences, in every genre) as they are, and with
  each of seven flaws a listener hears put into them (clipped, muddy, thin, noisy, gaps, a copy a semitone up, a copy
  falling out of time), and writes a line for `bench compare` for each. What a change to the critic is made to
  improve is how often a song is judged above its flawed copy; it must not judge more real songs bad, nor praise more
  flawed ones. The total of a song as it is is shown, not judged: a critic rating every song 100 would raise it. Each shard of the dataset is a random one percent of it: choose settings on shards 0 and 1, and report
  the result on shard 2 (`--shards 2..2`), which the tuning never saw. It holds no chart hits: what it says of the
  songs people hear most is still to be measured.
- **The generator against real music**: `cargo run --release -p claudius --example eval_genres -- embed DIR --shards
  0..49` hears the songs of MTG-Jamendo that stand for claudius's genres (by their tags), CLAP's embedding of each, and
  `run REAL` composes claudius's songs of each genre and hears them the same way, writing a line for `bench compare`
  for each genre: the kernel audio distance to the genre's real songs (KAD, what a change to the generator is made to
  lower), how typical its songs are and how often each is heard as its own genre (guards), and how much they differ
  from one another (shown). Tune on the even shards (`--half tune`, seeds 1 to 20) and confirm on the odd ones
  (`--half confirm`, seeds 101 to 120). A change to one genre or a few is judged on its songs: `run --songs FILE`
  writes a line for each, and `kad REAL BEFORE AFTER` tests the genres' KAD on two runs' lines, made with the same
  seeds, by the paired bootstrap over the seeds; `bench compare` on the same lines keeps how typical each song is and
  whether it is heard as its genre as guards. Fifteen genres have enough songs in shards 0 to 49 to be measured, 15 or more
  in each half; amapiano, phonk and synthwave have no honest tag, and electro house too few songs. The real songs'
  embeddings are not kept here: the songs' licences are the artists' own.
- **The analysis of sections and of where a cut falls**: the examples `eval_structure` (JamendoLyrics and SALAMI) and
  `eval_cuts` (JamendoLyrics), which print their measures.
