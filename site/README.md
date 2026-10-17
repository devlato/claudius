# site

The landing page at [claudius.pages.dev](https://claudius.pages.dev): plain HTML, CSS and a little JavaScript, with no
framework and no build step beyond filling in three placeholders. `cargo xtask site` assembles the whole website in
`target/site`: this folder with its placeholders filled in, the book at `/docs/`, the web studio at `/studio/`, and
`/install.sh` and `/install.ps1`.

## Preview

```sh
python3 -m http.server -d site 8000     # http://localhost:8000, placeholders left as they are
node site/tests/serve.mjs --port 8000   # the same with the placeholders filled in, and _headers and _redirects applied
```

`/docs/`, `/studio/` and the install scripts exist only in the assembled site. To see everything:

```sh
cargo xtask site
node site/tests/serve.mjs --port 8000   # serves target/site when it exists
```

The players fetch `audio/*.json`, so open the page through a server, not as a file.

## Placeholders

| placeholder | becomes |
|---|---|
| `{{version}}` | the workspace version in `Cargo.toml`, e.g. `0.1.0` |
| `{{site_url}}` | where the site is served, without a trailing slash: `https://claudius.pages.dev` |
| `{{repo_url}}` | `https://github.com/devlato/claudius` |

`cargo xtask site` replaces them in every text file it copies, `_headers` and `_redirects` included. They appear in
the HTML (install commands, links, Open Graph tags, the `?v={{version}}` on the CSS and JS), in `_redirects`, and in
a comment in `_headers`.

## What is here

- `index.html` and `404.html`. No inline scripts or styles, so the page runs under `script-src 'self'` and
  `style-src 'self'`.
- `assets/`: `site.css`, `site.js` (players, the hero's step grid, install tabs, copy buttons, code colours),
  `noscript.css` (native audio controls and every panel open when JavaScript is off), the logo, and the studio
  screenshots in `img/`.
- `assets/fonts/`: a subset of Hack, the studio's monospace face (MIT and Bitstream Vera licences, in
  `hack-license.txt`), for code and the LCD readouts. Everything else uses the system's fonts.
- `audio/`: the four demos. Each has the excerpt (`.mp3`), its waveform (`.json`) and the script or the printed song
  that made it (`.rhai`).
- `favicon.svg`, `favicon-32.png`, `apple-touch-icon.png`, `icon-512.png`, `site.webmanifest`, and `og.png`
  (1200×630, the preview image for links).
- `_headers` and `_redirects`: Cloudflare Pages configuration, described below.
- `tests/`, `package.json`, `playwright.config.mjs`: the smoke test. `tools/excerpt.py`: cuts the demos. None of
  these belong on the website, and the smoke test checks that the build leaves them out.

The palette, the LCD, the step grid and the section colours come from the studio
(`crates/claudius-ui/src/theme.rs`); the wordmark is the studio's, outlined from egui's Ubuntu Light.

## Headers and redirects

`_headers` sets a strict Content-Security-Policy on every page: nothing but this site's own scripts, styles, images,
fonts, audio and JSON, no frames, no inline code, and Trusted Types for the DOM. Two paths replace it, because
Cloudflare joins two values of the same header with a comma and a browser then enforces both policies:

- `/docs/*`: mdBook writes inline scripts into every page, and one of them names the content-hashed search index,
  so the policy allows `'unsafe-inline'` scripts and styles there.
- `/studio/*`: `'wasm-unsafe-eval'` to compile the WebAssembly; `'unsafe-inline'` because trunk starts the app from
  an inline module script that names the hashed files of each build; `blob:` audio for the demo's previews.

It also sets `nosniff`, `frame-ancestors 'none'` and `X-Frame-Options: DENY`, a strict referrer policy, a
Permissions-Policy that turns off sensors, camera, microphone and the rest, HSTS, and cache lifetimes: a year for
the files trunk and mdBook name by content hash and for the font, a month for screenshots and audio, a day for this
folder's CSS and JS (their URLs carry `?v={{version}}`). Rename an image or a song to replace it.

`_redirects` gives short links: `/github`, `/issues`, `/releases`, `/license`, `/changelog`, `/docs` to `/docs/`,
`/studio` to `/studio/?mock` (the studio needs `?mock` on a static host, since there is no engine behind it), and
`/install` to the install section.

## Smoke test

```sh
cd site
npm ci
npx playwright install chromium   # once per machine
npx playwright test
```

It serves `target/site` when `cargo xtask site` has built it, otherwise this folder with the placeholders filled in
(`SITE_DIR` picks another folder). The server applies `_headers` and `_redirects` the way Pages does, so a broken
Content-Security-Policy shows up as a console error. It checks that the page loads with no console errors, no CSP
violations, no failed requests and no requests to other sites; that it stays under 300 KB without the audio; that
the players play, pause, play one at a time and seek with the mouse and the keyboard; that every link inside the site
resolves; that the short links redirect; that there is no horizontal scroll at 375 px; and that unknown paths get the
404 page. Against the assembled site it also checks that the tooling is left out, that the docs and the studio demo
start under their own policies, and that the installers are served as text.

To look at the page, `SCREENSHOT_DIR=/tmp/shots npx playwright test screens` writes full-page screenshots at
1440 and 390 px.

## The demos

| file | genre | made with |
|---|---|---|
| `golden-pulse` | deep house, 120 BPM, A dorian | `claudius evolve --genre deep_house --seed 5 --generations 4 --no-library` |
| `paper-lanterns` | lofi, 82 BPM, D minor | `claudius script paper-lanterns.rhai` |
| `night-shift` | techno, 130 BPM, F minor | `claudius script night-shift.rhai` |
| `low-light` | drum and bass, 174 BPM, C minor | `claudius script low-light.rhai` |

Renders are deterministic, so running a script again gives the same song. `golden-pulse.rhai` is the evolved song
printed as a script (`claudius script - --on <song folder> --print < /dev/null`). The scripts use the sample packs
and no Apple Loops; the page credits the packs whose licences ask for it, from each song's `credits.txt`.

To cut an excerpt and its waveform from a song folder:

```sh
python3 site/tools/excerpt.py analyze out/lofi-paper-lanterns-1          # loudness of every bar, by section
python3 site/tools/excerpt.py cut out/lofi-paper-lanterns-1 paper-lanterns 10 15 site/audio --key "D minor"
```

The waveform JSON holds `peaks` and `rms` (600 values each, 0 to 255, read back from the MP3), the `sections` inside
the excerpt in seconds, and `bpm`, `startBar` and `songBars`, which the players use for their bar readout and the hero
for its step grid. If a demo changes, update its card in `index.html`: the length, the score, the bar readout, the
script and the output.
