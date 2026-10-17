# Fuzzing

Two targets, run with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) on a nightly toolchain:

| target | what it feeds |
|---|---|
| `mini_notation` | arbitrary text to the mini-notation parser, then queries the pattern |
| `script` | arbitrary text to the script engine |

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run mini_notation -- -max_total_time=300
cargo +nightly fuzz run script -- -max_total_time=300
```

A crash leaves its input in `fuzz/artifacts/<target>/`. Add it as a regular test before fixing the bug, so it stays
fixed. CI runs both targets for five minutes every week.
