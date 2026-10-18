# Install

## macOS and Linux

```sh
curl -fsSL https://github.com/devlato/claudius/releases/latest/download/install.sh | sh
```

The script comes with every release, next to the archives, and is in the release's checksums. It downloads the
latest release for your computer from GitHub, checks it against the published SHA-256 sums, and installs it under
`~/.local`: the programs `claudius` and `claudius-packs` in `~/.local/bin`, and the web studio in
`~/.local/share/claudius/web`. It tells you if `~/.local/bin` is not on your `PATH` yet.

When you run it in a terminal, it offers to download the sounds and the ears straight away (below). To do that
without being asked, pass `--yes`:

```sh
curl -fsSL https://github.com/devlato/claudius/releases/latest/download/install.sh | sh -s -- --yes
```

Run as root, it installs the programs and leaves the downloads for later: the packs are archives from other sites,
so run `claudius setup` as the user who makes the music.

`CLAUDIUS_INSTALL_DIR` puts the programs in another folder (the web studio goes next to it, in
`../share/claudius/web`), and `CLAUDIUS_VERSION=0.1.0` installs a specific release.

## Windows

In PowerShell:

```sh
irm https://github.com/devlato/claudius/releases/latest/download/install.ps1 | iex
```

## The sounds and the ears

The programs are small. The sounds and the listening models are downloaded once, when you agree to it:

```sh
claudius setup
```

| what | size | where |
|---|---|---|
| 14 sample packs: pianos, electric pianos, strings, brass, woodwinds, mallets, basses, guitars, voice, real drum machines | about 1.1 GB | `~/Library/Application Support/claudius/packs` on macOS, `~/.local/share/claudius/packs` on Linux |
| earshot's listening models (CLAP) | about 0.8 GB | `~/.cache/earshot-rs` |

The studio offers the same download of the packs when they are missing, and downloads the models by itself the
first time it starts. Each model file is checked against a pinned SHA-256 hash. Without the packs, claudius synthesizes every sound; without
the models, the verdict uses meters and music theory only. `claudius setup --help` lists the options, and
`claudius-packs list` shows every pack with its licence.

claudius uses Apple Loops automatically when GarageBand or Logic content is installed. ffmpeg is optional: with it,
songs are also saved as MP3.

## Check the installation

```sh
claudius doctor
```

```text
claudius 0.1.0 (macos aarch64)
  ok  web studio      ~/.local/share/claudius/web
  ok  sample packs    14 default packs (1.2 GB) in ~/Library/Application Support/claudius/packs
  ok  ears            models in ~/.cache/earshot-rs
  ok  Apple Loops     14096 loops and samples
  ok  ffmpeg          9.0.2 (writes song.mp3)
  ok  port 7878       free
  ..  Claude Desktop  not connected: `claudius connect claude-desktop`

Everything claudius needs is here.
```

## From source

You need Rust (the version in `rust-toolchain.toml`; [rustup](https://rustup.rs) picks it up) and, for the web
studio, [trunk](https://trunkrs.dev).

```sh
git clone https://github.com/devlato/claudius
cd claudius
cargo install --locked --path crates/claudius-cli
cargo install --locked --path crates/claudius-packs
cargo xtask web          # builds the web studio into crates/claudius-ui/dist
claudius setup
```

A `claudius` built from a checkout finds the web studio in that checkout. The [development
setup](../contributing/setup.md) covers everything else.

## Uninstall

```sh
rm ~/.local/bin/claudius ~/.local/bin/claudius-packs
rm -r ~/.local/share/claudius/web
```

The sample packs, the models and the caches are kept, so that reinstalling does not download them again. To remove
them too, delete the packs folder `claudius doctor` shows (`~/Library/Application Support/claudius/packs` on macOS,
`~/.local/share/claudius/packs` on Linux), `~/.cache/earshot-rs`, and `~/Library/Caches/claudius` (`~/.cache/claudius`
on Linux). Songs you saved stay in `~/Music/claudius`.
