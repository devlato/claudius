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
  token: the one you give with `--token`, or a random 128-bit one it makes and prints when you give none. The token
  goes on every request, as `Authorization: Bearer` or once as `?token=` in the browser, which sets an HttpOnly,
  SameSite=Strict cookie (also `Secure` when the request came through https, as tunnels report with
  `X-Forwarded-Proto`). Tokens are compared in constant time.
- **Files stay in their folders.** A claudius started with `--public` or `--files` reads and writes files only inside
  `~/Music/claudius`, its caches and the sound libraries, plus the `--files` folders you name, after resolving links
  and `..`. It does not tell which files exist outside them: a path outside is refused with the same message whether
  or not it exists.
- **Scripts are sandboxed.** Scripts (from you, an assistant, or a file) cannot write files, import modules or call
  `eval`, and the only files they can read are the audio files they play, under the folder rules above. They stop
  after 5 million operations or 10 seconds, and strings, arrays and maps have size limits. A pattern stops when it
  needs more than 4 million steps of work or makes more than 262,144 events, which also bounds its memory; `*`, `/`,
  `fast`, `slow` and `ply` take factors up to 256, mini-notation is limited in length and nesting, and a song is at
  most 512 bars and 20 minutes long.
- **Downloads are checked.** The listening models come from pinned revisions on the Hugging Face Hub and are checked
  against pinned SHA-256 hashes before use. Sample packs come over HTTPS from their authors' hosts and are checked
  for size; they are audio and are decoded by symphonia, a memory-safe Rust decoder.
- **Releases are verifiable.** Every release lists SHA-256 sums, and each archive has a build provenance attestation
  you can check with `gh attestation verify <file> --repo devlato/claudius`. The install script checks the sums.

## Known limits

- Sample pack files are not yet pinned by hash, only by size and source. A compromised pack host could serve
  different audio.
- A token never expires, and one sent as `?token=` can stay in the browser's history and in the logs of a proxy or
  tunnel in front of claudius. Start the server again with a new token to revoke one.
- `ffmpeg`, when installed, is used to decode formats symphonia cannot read and to write MP3. claudius passes it file
  names as arguments (never through a shell), but its decoders are C code.
