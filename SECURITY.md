# Security

## Reporting a vulnerability

Please report it privately through GitHub: **Security → Report a vulnerability** on
[github.com/devlato/claudius](https://github.com/devlato/claudius/security/advisories/new). Do not open a public
issue for it.

Say what you found, how to reproduce it, and what an attacker could do with it. You will get an answer within a
week. Once a fix is released, the advisory is published with credit to you unless you prefer otherwise.

Fixes go into the latest release only; claudius is young and has no long-term support branches.

## What claudius protects

claudius runs on your computer and talks to three kinds of clients: its own studio (native or in a browser), MCP
clients such as Claude Desktop, and anything you point at its HTTP API. The design assumes the network and other
websites are hostile, and that whoever holds your claudius token is you.

- **Only you, by default.** The server listens on 127.0.0.1 and answers only requests addressed to a loopback host
  name, which stops DNS-rebinding attacks from web pages. Commands must be JSON, so a web page cannot send them with
  a plain form post, and cross-origin requests are allowed only from `localhost` pages.
- **Sharing it.** `claudius serve --public` accepts other host names (a tunnel, your LAN) and always requires a
  token: the one you give with `--token`, or a random 128-bit one it makes and prints when you give none. Listening
  on any address but loopback (`--host`) requires one the same way. The token
  goes on every request, as `Authorization: Bearer` or once as `?token=` in the browser, which sets an HttpOnly,
  SameSite=Strict cookie (also `Secure` when the request came through https, as tunnels report with
  `X-Forwarded-Proto`). Tokens are compared in constant time.
- **Files stay in their folders.** A claudius others can reach (`--public`, or `--host` with an address other than
  loopback) or started with `--files` reads and writes files only inside `~/Music/claudius`, its caches and the sound
  libraries, plus the `--files` folders you name, after resolving links and `..`. It does not tell which files exist outside them: a path outside is refused with the same message whether
  or not it exists.
- **Scripts are sandboxed.** Scripts (from you, an assistant, or a file) cannot write files, import modules or call
  `eval`, and the only files they can read are the audio files they play, under the folder rules above. They stop
  after 5 million operations or 10 seconds, and strings, arrays and maps have size limits. A pattern stops when it
  needs more than 4 million steps of work or makes more than 262,144 events, which also bounds its memory; `*`, `/`,
  `fast`, `slow` and `ply` take factors up to 256, mini-notation is limited in length and nesting, and a song is at
  most 512 bars and 20 minutes long, with at most 64 tracks and fewer in a long song, because a render holds every
  track in memory.
- **Downloads are checked.** The listening models come from pinned revisions on the Hugging Face Hub and are checked
  against pinned SHA-256 hashes as they are downloaded; a file that does not match is never used. Sample packs come
  over HTTPS: from GitHub repositories at pinned commits, and two archives from their authors' sites, pinned by size
  and SHA-256. An unpacked archive keeps no symbolic links, so nothing is read through one from elsewhere on your
  computer. The samples are audio, decoded by symphonia, a memory-safe Rust decoder, or by ffmpeg (below).
- **Releases are verifiable.** Every release lists the SHA-256 sums of its archives and install scripts, and has a
  build provenance attestation for them. The install script checks the sums, and this checks that a file was built
  by the release workflow from `main`:

  ```sh
  gh attestation verify <file> --repo devlato/claudius \
    --signer-workflow devlato/claudius/.github/workflows/release.yml --source-ref refs/heads/main
  ```

## How the code gets here

- **`main` changes only through pull requests.** A ruleset requires every change to arrive as a pull request whose
  checks pass (*ci ok*) on a branch that is up to date, keeps the history linear and signed, and forbids force pushes
  and deletion, for admins too. Release tags (`v*`) are created only by the release workflow and can never be moved
  or deleted.
- **Every pull request is checked** for secrets (gitleaks, over its commits and the whole tree), for known
  vulnerabilities and unwanted licences in the dependencies (cargo-deny; OSV-Scanner every week too), and for unsafe
  workflow patterns (zizmor, actionlint). CodeQL scans the Rust code and the workflows, and OpenSSF Scorecard scores
  the repository's practices. The crates forbid `unsafe` code.
- **Workflows run what was reviewed.** Every action is pinned to a commit, and the repository allows only GitHub's
  own actions and a short list of others. Tools CI downloads are pinned and checked against their SHA-256. The
  default token is read-only, and a pull request from a fork runs nothing until a maintainer approves it.
- **Releases are built from nothing, on `main`.** The release workflow refuses any other branch (but the hotfix
  branches, which only admins can push), restores no cache, and builds and downloads everything it links in that
  run. The jobs that hold the release tokens run no build at all. Those tokens are secrets of environments that only
  `main` may use, so a workflow pushed to another branch cannot read them; the website is deployed the same way.
- **Releases cannot change once published**, and their archives carry the SHA-256 sums and attestations above.

`scripts/github-setup.sh` makes these settings, so they are written down and can be checked.

## Known limits

- Packs you add yourself (`claudius-packs add-map`) are not pinned: their files are whatever the map's host serves.
- A token never expires, and one sent as `?token=` can stay in the browser's history and in the logs of a proxy or
  tunnel in front of claudius. Start the server again with a new token to revoke one.
- `ffmpeg`, when installed, decodes what the ears listen to (as the Python earshot does) and the formats symphonia
  cannot read, and writes MP3. claudius passes it file names as arguments (never through a shell) and lets it open
  only that file, with the demuxers of ordinary audio and video files: no playlists, lists of files or network
  addresses. Its decoders are still C code.
