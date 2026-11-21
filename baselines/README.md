# Baselines

The benchmark results of the last accepted change to each part of claudius that learns or listens, item by item, as
`cargo xtask bench compare` reads them: the baseline the next experiment is compared with
([Benchmarks](../docs/src/contributing/benchmarks.md)). The pull request whose change is accepted replaces its
benchmark's file here with its own results, so each file is what `main` does today.

| file | benchmark | how it is made |
|---|---|---|
| `critic-real-songs.jsonl` | The critic on real songs: 1,716 songs of MTG-Jamendo (shards 0 to 2), each as it is and with each of seven flaws | `cargo run --release -p claudius --example eval_critic -- DIR --shards 0..2` |
| `genres-tune.jsonl` | The generator against real music, the tuning half: 20 songs of each of the 23 genres with 15 real songs or more in each half (seeds 1 to 20), against the real songs of the even shards: 2,590 songs of MTG-Jamendo in all, those of its shards 0 to 4, 15 to 19 and 35 topped up from the other shards to 40 of each genre a half | `eval_genres embed DIR --shards 0..4,15..19,35 --top-up 40`, `eval_genres beats DIR --real REAL`, then `eval_genres run REAL --beats BEATS --seeds 1..20 --half tune` |
| `genres-confirm.jsonl` | The same, the held-out half: seeds 101 to 120 against the odd shards | `eval_genres run REAL --beats BEATS --seeds 101..120 --half confirm` |
| `remakes-trip_hop-flip-tune.jsonl` | Remakes against real music, the tuning half: 40 songs of the even shards flipped into trip hop (seeds 1 and 2), each heard against the real trip hop songs of those shards | `eval_genres remake REAL --sources DIR --genre trip_hop --how flip --seeds 1..2 --half tune --most 40 --primary typical,slowed` |
| `remakes-trip_hop-flip-confirm.jsonl` | The same, the held-out half: 40 songs of the odd shards, seeds 101 and 102 | `eval_genres remake REAL --sources DIR --genre trip_hop --how flip --seeds 101..102 --half confirm --most 40 --primary typical,slowed` |

Compare an experiment with a baseline:

```sh
cargo xtask bench compare baselines/critic-real-songs.jsonl experiment.jsonl
```

An experiment runs the same benchmark at the same settings on the same items; `bench compare` refuses two runs of
different items.
