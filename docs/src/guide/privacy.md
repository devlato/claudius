# Privacy and usage statistics

claudius runs on your computer and sends nothing about you or your music anywhere. It listens to the microphone only
while the studio's Record window is open (its meter shows what the input hears), or while the command palette listens
for what you say to it, after you click its microphone, in a browser once you have allowed the page to use it. It
keeps the takes you record with your song, on your computer, and what you say to the palette nowhere: it is heard on
your computer and let go. What a
[pitch check](voice.md#checking-your-pitch) learns of your voice stays there too, in its settings folder. It goes on
the network for five things only: the downloads you ask for (the sample packs, from GitHub and their authors' sites,
the listening models, from Hugging Face, the command palette's three small models, from Hugging Face too, the first
time you type in the palette, the first time it cannot make out a request and the first time you say something to it,
and the recordings `claudius samples` finds);
the samples a
[song built on samples](online-samples.md#songs-built-on-samples) gathers as it is made, from Openverse,
Wikimedia Commons and ccMixter (`--offline` keeps it from asking); the [update](updates.md) check, once a day, to
GitHub; an assistant you connect from outside your computer if you set that up; and anonymous usage statistics, which
are off until you turn them on.

What you type in the [command palette](../getting-started/studio.md#finding-an-action) stays on your computer: the
models that find what your words mean and read your requests run in claudius's engine there (or in the claudius you
connected the studio to), and the demo's runs in your browser, on the copy the site serves.

## The website

[claudius.devlato.com](https://claudius.devlato.com) makes no requests to anyone else from your browser: no
analytics, no cookies, no fonts or scripts from other servers. The studio demo's samples are the one thing it fetches
for you. When the demo makes a song built on samples, it asks the site, and the site's Worker
(`site/worker/index.js`) asks Openverse for recordings and Freesound, Jamendo or Wikimedia Commons for their audio. It
sends them the search's words (the kinds of record the genre digs for, or the words you typed for it) and nothing
about you, and they see the site, not your address. What they answer is kept in Cloudflare's cache, searches for a day
and audio for a year. To keep the site within what Openverse lets it ask, Cloudflare's rate limiting counts each
address's searches that the cache has not kept, 30 a minute at most; the site itself records none of them.

A phone [playing along with the demo](phone.md#with-the-demo) reaches the demo's tab through a room of the site's
relay (`site/worker/room.js`, a Durable Object on Cloudflare): what the tab and the phone say to each other (the song's
sounds, the time, the takes) passes through it and is kept nowhere. A room's name is 128 random bits that the demo's QR
code alone carries, and only the site's own pages may join one.

## Usage statistics

They stay off unless you say yes. The studio asks once, at the end of its first-run tour, with two answers and neither
one picked for you; closing the tour without answering leaves them off. The studio's **?** menu has the same switch,
and from a terminal:

```sh
claudius telemetry        # whether they are on, and an event exactly as it is sent
claudius telemetry on
claudius telemetry off
```

When they are on, claudius sends four kinds of events, each at most once a day:

| event | when | detail |
|---|---|---|
| `started` | a command starts: `studio`, `serve`, `mcp`, `make` and the rest | none |
| `setup` | `claudius setup`, or the studio's pack download, ends | `ok`, `models` (the models did not download), or `packs:<name>` (the first pack that did not install) |
| `panic` | claudius stops on a panic, a bug | the file and line, such as `claudius/src/render.rs:123`, never the message |
| `perf` | claudius renders a song, listens to one with the ears, or decodes a recording | `render`, `ears` or `decode`, and in `value` how fast: seconds of audio a second, to two figures (`38`: it rendered a minute of song in about a second and a half) |

An event is these eight fields and nothing else (`value` is 1, but for a `perf` event's speed):

```json
{
  "v": 2,
  "event": "started",
  "command": "studio",
  "version": "0.5.0",
  "os": "macos",
  "arch": "aarch64",
  "detail": "",
  "value": 1
}
```

claudius 0.4 and before send the same without `value` (`"v": 1`), and the endpoint still takes those.

It never holds a file name, a path, a song or any part of one, your name, or an identifier: nothing tells one person's
events from another's. It is sent in the background with a 3-second timeout and never sent again, so a computer
without a network notices nothing.

## Where they go

To `https://claudius-telemetry.devlato.workers.dev/v1/event`, a Cloudflare Worker whose source is `telemetry/` in this
repository. It refuses anything that is not an event of the shape above, and writes its fields to Workers Analytics
Engine, which keeps them for three months. It writes no IP address and nothing else from the request.
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
