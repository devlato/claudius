# Security model

claudius runs on your computer, opens audio files, runs scripts written by people and assistants, downloads models
and samples, and answers HTTP. [SECURITY.md](https://github.com/devlato/claudius/blob/main/SECURITY.md) is the summary
and says how to report a vulnerability. This page is the full model: who claudius trusts, what it defends against,
how, and where the defences end.

## What is worth protecting

- **Your files.** claudius can read audio and write song folders; it must not become a way for someone else to read
  or write anything else.
- **Your computer's resources.** Rendering is heavy by nature; nothing outside should be able to make claudius use
  unbounded memory, disk or time.
- **Control of the session.** Whoever can send commands can run scripts, start jobs, open and save files.
- **The integrity of what runs.** The models and samples claudius downloads, and the releases you install.
- **The licences of what you publish.** A song should not quietly contain samples you may not publish.

## Who is who

| actor | trusted? | can reach claudius by |
|---|---|---|
| you, at your computer | yes | the studio, the CLI, `curl` |
| assistants you connect | acting for you; their scripts are untrusted code | MCP over stdio or HTTP |
| programs and users on the same computer | not defended against | 127.0.0.1, like you |
| web pages in your browser | no | requests your browser sends to 127.0.0.1 |
| machines on your network | no | nothing, unless you listen beyond loopback |
| whoever holds your token | treated as you | a tunnel or the network, with `--public` |
| hosts claudius downloads from | partly: content is checked where it can be | |
| files and blueprints you open | no | their contents |

Two of these deserve a sentence each. A local process is outside the model: it can already read your files, and
claudius listening on loopback gives it nothing it lacks. A token holder is inside the model on purpose: the token is
how you reach your own claudius from a phone or a cloud assistant, so whoever has it is you.

## The network

### Loopback by default

`claudius serve` and `studio` listen on `127.0.0.1` unless `--host` says otherwise. Listening on another address
without a token prints `warning: listening on <host> without --token: anyone who can reach this port controls
claudius`. The server prints its address only once the port is bound; a taken port is an error, `port <port> is
taken, by another claudius or another program (--port picks another)`.

### Host names: no DNS rebinding

A web page cannot read `http://127.0.0.1:7878` directly, but it can point a domain it controls at 127.0.0.1 and talk
to claudius as a same-origin page. That attack (DNS rebinding) is stopped by the `Host` header: unless the server is
`--public`, every request must name `localhost`, `127.0.0.1` or `[::1]`, and anything else gets 403. The MCP endpoint
has rmcp's own allowed-hosts check as a second layer, turned off for `--public` servers. The Host check is not
authentication: a program can send `Host: localhost` to a claudius listening on the network, which is why listening
beyond loopback takes a token.

### Cross-site requests: no forms, no `no-cors`

Without CORS, a page on another site can still *send* simple requests (a form post, a `fetch` in `no-cors` mode), even
if it cannot read the answer. claudius makes every state-changing request impossible to send that way:

- `POST /api/command` requires `Content-Type: application/json`, which no simple request can carry (415 otherwise);
- the MCP endpoint requires `Content-Type: application/json` and `Accept: application/json, text/event-stream`, and
  every request after `initialize` needs the `mcp-session-id` from a response the page cannot read;
- `GET` routes change nothing (the one side effect is encoding an MP3 of a render that exists).

CORS then allows reading answers only from pages served by `http://localhost`, `http://127.0.0.1` or `http://[::1]` on
any port, so a local web app can use the API and a remote site cannot. The allowlist includes every local port, which
means any local web server's pages, consistent with not defending against local processes.

### Tokens

With a token (`--token`, `CLAUDIUS_TOKEN`, or the one `--public` makes when it is given none), every request must
carry it as `Authorization: Bearer`, as the `claudius_token` cookie, or as `?token=`. A request with `?token=` also
sets the cookie, `HttpOnly; SameSite=Strict; Path=/`, and `Secure` when the request came through https, which tunnels
report with `X-Forwarded-Proto: https`; over plain HTTP on a LAN a `Secure` cookie would be dropped, so it is left
off there. The comparison takes the same time wherever the strings first differ. CORS preflights are answered without
the token so a browser can learn what is allowed; the request itself then needs it. A request without the right
token gets `401` with `this claudius needs its token (Authorization: Bearer <token>, or ?token=<token>)`, which is
how `claudius mcp`, `studio` and `doctor` tell a claudius that wants a token from nothing at all.

What the token does not do:

- It does not expire or rotate. Restart with a new token to revoke the old one.
- A `?token=` URL lands in browser history and in the logs of the tunnel and any proxy in between. The bearer header
  avoids that where the client supports it.
- Anyone with the token can do everything you can through the API, including reading and writing files inside the
  shared folders below. Treat it like a password: `openssl rand -hex 16` makes one of the same strength as the
  tokens claudius makes.

### `--public`

`--public` lifts the Host check so a tunnel's or a LAN's host name is accepted, and switches on the file policy below.
A public claudius always has a token: without `--token` it makes one from 16 random bytes from the operating system
and prints it once the port is bound:

```text
claudius at http://127.0.0.1:7920: web UI at /, API at /api, MCP at /mcp (token required)
shared with a new token: open http://127.0.0.1:7920/?token=265f6449352030ce55bd79c3f4179c65 (through your tunnel, its address with ?token=265f6449352030ce55bd79c3f4179c65), or send Authorization: Bearer 265f6449352030ce55bd79c3f4179c65. --token sets your own.
```

Each start makes a new one, so a restart revokes the last. `--host` with any address but loopback makes one the same
way, with or without `--public`: the Host check refuses browsers, but a program on the network can send
`Host: localhost`, so only the token keeps it out.

## Files

### The policy

A claudius only you can reach reads and writes wherever your user can, because that is what a local app does. A
shared claudius (`serve --public`, `serve --host` with an address other than loopback, or `serve --files <dir>`)
checks every path it is given against a list of roots,
`config::shared_roots()` plus the `--files` folders:

- **Reading** (`config::path_allowed`): the path must exist and, canonicalised (symbolic links and `..` resolved),
  start with one of the canonicalised roots. `config::read_error` turns a refusal into a message. Used for scripts'
  `.s(path)`, `load` / `open` (and every audio file a loaded blueprint plays), `analyze` and `remake`.
- **Writing** (`config::write_allowed`): the nearest existing ancestor of the path must pass the reading check, and
  every remaining component must be a plain name, so `allowed/../outside` and `allowed/missing/../../outside` are
  refused. Used for `save`.

`config::shared_roots()` is the song folder (`~/Music/claudius`), claudius' cache folder, the sample packs folder and
the two Apple Loops folders. Sample-pack instruments and Apple Loops one-shots are found by name, not path, and stay
available. The command line is not subject to the policy: you are running it.

### Its limits

- **Check, then use.** The check and the use are separate system calls. A local process that can swap a path for a
  symbolic link between them can redirect a read or write; local processes are outside the model, and a remote token
  holder has no command that creates links.
- **Links already inside a root.** Writing `song.wav` into a folder follows a symbolic link already there. If you keep
  links inside `~/Music/claudius` to places you do not want written, a token holder can write through them.
- **Existence.** `config::read_error` says `<path> does not exist` only for a path that would be inside the roots;
  any other path gets `<path> is outside the folders this claudius may read`, whether it exists or not. A token
  holder learns which files exist only inside the folders it may read anyway.
- **Roots that do not exist yet.** Roots are canonicalised, and one that does not exist is left out. Until
  `~/Music/claudius` exists, a shared claudius refuses to save there (`<path> is outside the folders this claudius may
  write`), including the default `save` without a path. This is listed as a known gap below.
- **Unshared mode is open.** An assistant connected to an unshared claudius can open and save files anywhere your user
  can, through `open`, `save` and `.s()`. That is the trust you give it by connecting it; `serve --files <dir>` fences
  it in.

## Scripts

Scripts come from you, from assistants, from files you open and from printed songs, so they run in a sandbox
(`script::limits`):

- no file, network or process functions exist; the only file access is resolving `.s(path)`, under the file policy;
- `import` finds no modules (`DummyModuleResolver`) and `eval` is disabled;
- 5 million operations, 10 seconds, call depth 64, expression depth 128 (64 in functions), strings of 200,000
  characters, arrays of 10,000 items, maps of 1,000 entries;
- a pattern query may do 4 million steps of work and make 262,144 events; `*`, `/`, `fast`, `slow` and `ply` take
  factors above 0 and up to 256; a track may play 1,024 notes per bar; mini-notation is limited in length (4,096
  characters), nesting (64), repeats and euclidean steps (256);
- a song is at most 512 bars and 20 minutes, with at most 64 tracks and 12,000 track-seconds (20 tracks of 10
  minutes), because a render holds every track in memory; this is checked after every script and edit and again
  before rendering, where a pattern with several sounds counts as a track for each.

The fuzz targets run arbitrary text through the parser and the script engine every week. The event budget is what
bounds a query's memory. `track("x", s("bd*100000000"))` is refused while parsing, and the whole `claudius script`
process peaks at 33 MB; the densest pattern the speed limit allows, `track("x", s("[bd*256]*256"))`, stops at the
event budget with the process at 134 MB.

Pattern tracks in a blueprint are code: arranging a song evaluates them, with the same sandbox and only the pattern
functions.

## Files and blueprints you open

- **Audio** is decoded by symphonia, a Rust decoder, in `claudius-decode`, which forbids `unsafe` code itself. When
  symphonia cannot read a file and ffmpeg is installed, ffmpeg decodes it, and earshot decodes with ffmpeg whenever
  it is installed, to match the Python reference: C code, run as a separate process with the file name as an
  argument (never through a shell) and `-nostdin`. `CLAUDIUS_FFMPEG=none` turns that off. Every ffmpeg and ffprobe
  input goes through `claudius_decode::ffmpeg_input`, which allows only the `file` protocol and the demuxers of
  files that hold their own audio. A playlist (`.m3u8`) or a list of files is refused, because it would make ffmpeg
  open files the path check never saw, outside the folders of a shared claudius.
- **Decoding is bounded**: a file gives at most 30 minutes of stereo at 48 kHz (`MAX_SAMPLES`, as many samples in
  other layouts), and a longer one is refused (`TooLong`) before it fills the memory. ffmpeg is asked for no more
  than that (`-t`), and its output is read as it comes.
- **Blueprints** are data, with three things in them that act: pattern code (sandboxed, above), file paths (checked by
  the policy on a shared claudius), and numbers. Lengths are bounded, but not every effect parameter is, so a
  hand-crafted blueprint can make a render slow or large within the song-length limit. Open blueprints from people you
  trust as you would open their project files in a DAW.
- **Text that reaches an assistant**: song titles, sound names and file names from a blueprint or a file appear in the
  MCP tools' answers, where they could carry instructions aimed at the assistant (prompt injection). claudius passes
  them through as data; the assistant's host decides what to trust.

## Downloads

| what | from | checked |
|---|---|---|
| CLAP and AudioSet models | huggingface.co, at pinned revisions | SHA-256 of every file, pinned in `earshot/src/models.rs`; a mismatch is deleted and never used |
| curated packs from GitHub | raw.githubusercontent.com at pinned commits | the commit pins the content; sizes against GitHub's listing when there is one |
| curated Strudel maps | the map and every file it names on raw.githubusercontent.com at pinned commits | the commit; a map whose `_base` names a branch is pinned to the map's commit, or by the registry's `map_base`, and an install refuses a file that is not at a commit |
| `tr909`, the FreePats organ | audiorealism.se, freepats.zenvoid.org (archives) | the size and SHA-256 in the registry; a mismatch, in a download or in the cache, is deleted |
| Strudel maps you add | any URL you give, HTTPS or HTTP | nothing beyond the length, and at most 1 GiB a file |

Archives are unpacked into a scratch folder with the system's `tar` (or `unzip`, `7z`); bsdtar and GNU tar strip
absolute paths and refuse `..` members by default. Every symbolic link in the unpacked tree is then deleted, and the
walk over it follows none, because a member named like a sample but linking to `~/.ssh/id_rsa` would otherwise be
copied into the pack. The members are copied into the pack under sanitised relative paths (`..` cannot survive
`sanitize`), so what lands in the pack's folder stays inside it. A download whose size is not known in advance stops
at 1 GiB, and one that sends more than its known size is cut off. A `GITHUB_TOKEN` in the environment is sent only to `api.github.com`. Every sample is audio and goes through the
decoder above. The packs on an unclear licence are opt-in and refuse to install without `--accept-licence`.

## Releases and the build

- Release archives are built by the release workflow from `main` after a reviewed release pull request, with a
  `SHA256SUMS` file and a build provenance attestation (`gh attestation verify <archive> --repo devlato/claudius`). The
  install scripts check the sums. [0009 Releases through pull requests](../design/0009-releases.md) explains the
  process.
- Third-party GitHub Actions are pinned by commit SHA, checkouts do not keep credentials, and workflow permissions are
  the minimum each job needs. Tools downloaded in CI (lychee, actionlint) are checked against SHA-256 sums.
- `cargo deny` checks every dependency's licence, advisories and source on every pull request; dependabot proposes
  updates.
- Building `earshot` downloads a prebuilt ONNX Runtime through the `ort` crate (its `download-binaries` feature), which
  is linked statically. That download is a build-time dependency on the `ort` project's hosting.

## Known gaps

These are open, and listed so that nobody has to rediscover them:

1. Sample pack archives (`tr909`, the FreePats organ) and user maps are not pinned by hash; a compromised host could
   serve different audio. GitHub-hosted packs are pinned by commit.
2. The token never expires, and a `?token=` URL stays in browser history and in proxy and tunnel logs.
3. Caches do not shrink by themselves: decoded audio and Apple Loops cuts on disk, a session's last ten renders (other
   sessions' are deleted after 12 hours), and the one-shot and warp caches in memory. A token holder who keeps
   rendering and saving can fill the disk.
4. `cancel` stops a mutation or an evolution after the current step but never a render, which runs to its end.

Report anything else privately, as SECURITY.md describes.
