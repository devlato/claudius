# Privacy and usage statistics

claudius runs on your computer and sends nothing about you or your music anywhere. It goes on the network for four
things only: the downloads you ask for (the sample packs, from GitHub and their authors' sites, and the listening
models, from Hugging Face), the [update](updates.md) check, once a day, to GitHub, an assistant you connect from
outside your computer if you set that up, and anonymous usage statistics, which are off until you turn them on.

## Usage statistics

They stay off unless you say yes. The studio asks once, at the end of its first-run tour, with two answers and neither
one picked for you; closing the tour without answering leaves them off. The studio's **?** menu has the same switch,
and from a terminal:

```sh
claudius telemetry        # whether they are on, and an event exactly as it is sent
claudius telemetry on
claudius telemetry off
```

When they are on, claudius sends three kinds of events, each at most once a day:

| event | when | detail |
|---|---|---|
| `started` | a command starts: `studio`, `serve`, `mcp`, `make` and the rest | none |
| `setup` | `claudius setup`, or the studio's pack download, ends | `ok`, `models` (the models did not download), or `packs:<name>` (the first pack that did not install) |
| `panic` | claudius stops on a panic, a bug | the file and line, such as `claudius/src/render.rs:123`, never the message |

An event is these seven fields and nothing else:

```json
{
  "v": 1,
  "event": "started",
  "command": "studio",
  "version": "0.2.0",
  "os": "macos",
  "arch": "aarch64",
  "detail": ""
}
```

It never holds a file name, a path, a song or any part of one, your name, or an identifier: nothing tells one person's
events from another's. It is sent in the background with a 3-second timeout and never sent again, so a computer
without a network notices nothing.

## Where they go

To `https://claudius-telemetry.devlato.workers.dev/v1/event`, a Cloudflare Worker whose source is `telemetry/` in this
repository. It refuses anything that is not an event of the shape above, and writes the seven fields to Workers
Analytics Engine, which keeps them for three months. It writes no IP address and nothing else from the request.
Cloudflare, which runs it, sees each request the way any web server sees one.

## Keeping them off

Any of these keeps them off, whatever the answer:

- `DO_NOT_TRACK=1`;
- `CLAUDIUS_TELEMETRY=0` (or `off`);
- `CI` set, as on continuous integration;
- a debug build (`cargo run` without `--release`), which sends only to `CLAUDIUS_TELEMETRY_URL`, when that is set.

The answer is kept in `telemetry.json` in claudius' settings folder (`~/Library/Application Support/claudius` on
macOS, `~/.config/claudius` on Linux, `%APPDATA%\claudius` on Windows), with the day each kind of event last went out.
Deleting the file is the same as never having answered.

## For maintainers

- `node telemetry/test.mjs` checks that the Worker takes what claudius sends and refuses everything else; a test in
  `claudius-cli` checks that it knows every command.
- `npx wrangler deploy` in `telemetry/` publishes it. Analytics Engine has to be enabled on the Cloudflare account once,
  in the dashboard.
- `scripts/telemetry-report.sh [days]` sums the events of the last 30 days (or `days`) by event, command, version and
  system. It needs `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN`, a token that can read Account Analytics.
