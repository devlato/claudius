# Development setup

You need Rust, a handful of cargo tools, and on Linux a few system packages. Everything else (ONNX Runtime for the
ears, wasm-bindgen for the web studio) is downloaded by the build itself.

## Rust

Install [rustup](https://rustup.rs). `rust-toolchain.toml` at the root pins the Rust that CI and the releases
build with (1.96.0), with rustfmt, clippy and the `wasm32-unknown-unknown` target, and rustup installs all of it
the first time you run `cargo` in the repository. Your machine and CI run the same clippy, so its lints do not
change under you. Moving to a newer Rust is a pull request of its own, with whatever its new lints ask for.

`rust-version` in `Cargo.toml` is the oldest Rust the workspace builds with: 1.96 too, because 1.95 cannot infer
some of the types in `claudius`. The `msrv` job in CI checks it.

## System packages

**macOS**: the Xcode command line tools (`xcode-select --install`), for the C and C++ that some crates compile
(SQLite, signalsmith-stretch) and the clang runtime that ONNX Runtime links against.

**Linux** (Debian and Ubuntu; the list is what a bare `ubuntu:24.04` needs on top of a compiler):

```sh
sudo apt install build-essential pkg-config libssl-dev libasound2-dev libclang-dev
```

`pkg-config` and `libssl-dev` let ort download ONNX Runtime at build time (only builds with the ears need them),
`libasound2-dev` is ALSA for the studio's audio output (only builds with the studio window), and `libclang-dev` is
for the bindgen step of signalsmith-stretch. On Fedora:
`sudo dnf install gcc-c++ pkgconf-pkg-config openssl-devel alsa-lib-devel clang-devel`. The native studio window
also needs X11 or Wayland and OpenGL at run time, which any desktop has; no `-dev` packages are needed for them.

The Linux build needs glibc 2.38 or newer (Ubuntu 24.04, Debian 13, Fedora 39), because the ONNX Runtime library
that ort downloads was built against it.

**Windows** builds are experimental and not checked yet: you need the Visual Studio C++ build tools and LLVM (for
bindgen). Build without the ears there (below).

## Features

`claudius` has two cargo features, both on by default:

- `ears`: earshot's listening models, through ONNX Runtime. Without it the verdicts use meters and music theory,
  `claudius setup` skips the models, and `claudius doctor` says "ears: not in this build". ONNX Runtime has no
  build for Intel Macs, so they never get the ears, and the Windows build leaves them out until someone has run it
  there with them.
- `studio`: the native studio window (egui). Without it `claudius studio` points you at `claudius serve`, which
  still serves the web studio, and the binary links no ALSA, X11 or OpenGL. That is the headless build for Linux
  servers.

Code that needs earshot goes behind `#[cfg(feature = "ears")]`, the window behind `studio`. `cargo xtask ci`
checks the builds without each of them, so a gated path cannot stop compiling unnoticed.

## Tools

| tool | what it is for |
|---|---|
| [trunk](https://trunkrs.dev) 0.21 | builds the web studio (WebAssembly) |
| [cargo-nextest](https://nexte.st) | runs the tests; `cargo xtask ci` falls back to `cargo test` without it |
| [mdBook](https://rust-lang.github.io/mdBook/) 0.5 | builds this book |
| [cargo-deny](https://embarkstudios.github.io/cargo-deny/) | checks licences, advisories and where crates come from (`deny.toml`) |
| [typos](https://github.com/crate-ci/typos) | finds misspellings (`_typos.toml`) |
| [git-cliff](https://git-cliff.org) | turns commit messages into release notes (`cliff.toml`) |
| [lychee](https://lychee.cli.rs) | checks the links between the pages of the built site |
| [actionlint](https://github.com/rhysd/actionlint), [shellcheck](https://www.shellcheck.net) | check the workflows and the scripts in `scripts/` |
| Node.js 24 | runs the website's smoke test (Playwright) |

All of them have prebuilt binaries. With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), which
downloads those instead of compiling:

```sh
cargo binstall trunk cargo-nextest mdbook cargo-deny typos-cli git-cliff lychee
```

On macOS, Homebrew has all of them too:

```sh
brew install trunk cargo-nextest mdbook cargo-deny typos-cli git-cliff lychee actionlint shellcheck node
```

## Building each part

`cargo xtask` runs the repository's chores; `cargo xtask help` lists them. The ones you need day to day:

| command | builds | into |
|---|---|---|
| `cargo build` | every crate (the dev profile is optimized, so songs render at a usable speed) | `target/debug` |
| `cargo build -p claudius-cli --no-default-features --features studio` | `claudius` without the ears, as on Intel Macs and Windows | `target/debug` |
| `cargo build -p claudius-cli --no-default-features --features ears` | `claudius` without the studio window: the headless build | `target/debug` |
| `cargo xtask web` | the web studio, with trunk | `crates/claudius-ui/dist` |
| `cargo xtask docs` | this book | `target/book` |
| `cargo xtask site` | the website: the landing page, the book at `/docs/`, the studio at `/studio/` and the install scripts | `target/site` |
| `cargo xtask dist` | a release archive for this machine, as the release workflow builds it (`--headless` for the Linux server build, `--no-ears` without the ears) | `target/dist` |

The first build downloads ONNX Runtime (about 120 MB) for earshot, into `~/Library/Caches/ort.pyke.io` on macOS and
`~/.cache/ort.pyke.io` on Linux.

For the docs and the site there are live previews: `mdbook serve docs` rebuilds the book on every save at
`http://localhost:3000`, and `node site/tests/serve.mjs` serves `target/site` the way Cloudflare Pages does (headers,
redirects, the 404 page) at `http://127.0.0.1:4321`.

## Running the studio from source

```sh
cargo xtask web                                  # the web studio, once and after changes to the UI
cargo run --release -p claudius-cli -- setup     # the sample packs and the listening models, once (about 2 GB)
cargo run --release -p claudius-cli -- studio    # the native window, with the engine inside
cargo run --release -p claudius-cli -- serve     # or the studio in the browser at http://127.0.0.1:7878
```

A `claudius` built in a checkout serves the web studio from `crates/claudius-ui/dist` of that checkout.

When you work on the UI, `NO_COLOR=true trunk serve` in `crates/claudius-ui` rebuilds it on every save and serves it
at `http://127.0.0.1:8090`. Open `http://127.0.0.1:8090/?server=http://127.0.0.1:7878` to drive a `claudius serve`
running next to it, or `http://127.0.0.1:8090/?mock` for the demo that needs no server. (`NO_COLOR=true` because
trunk rejects the common `NO_COLOR=1`; `cargo xtask web` sets it for you.)

## Before you push

`cargo xtask ci` runs what every pull request has to pass first: `cargo fmt --check`, clippy with warnings as
errors on every target, clippy and `cargo check` of the builds without the ears or the studio window, the tests,
and the examples in the docs. CI runs more; when you changed the matching part, run it too:

| you changed | run |
|---|---|
| the docs or the site | `cargo xtask site`, then `lychee --offline --include-fragments --root-dir "$PWD/target/site" 'target/site/**/*.html'` for the links, and `npm ci && npx playwright test` in `site/` |
| dependencies | `cargo deny check` |
| anything with words in it | `typos` |
| workflows or scripts | `actionlint` and `shellcheck scripts/*.sh` |
| benchmarks | `cargo bench --no-run` |
| code that the Intel Mac build compiles | `rustup target add x86_64-apple-darwin`, then `cargo check --target x86_64-apple-darwin -p claudius-cli --no-default-features --features studio` (CI's `intel mac build` job) |
