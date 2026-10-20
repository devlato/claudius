# Exporting

A song's master is `song.wav` in its folder, 24-bit at 48 kHz, and every song folder has a 256 kbps `song.mp3` beside
it. `claudius export` writes the master in any other format:

```sh
claudius export out/deep_house-liquid-avenue-21 --format flac       # liquid-avenue.flac in the song's folder
claudius export out/deep_house-liquid-avenue-21 -f mp3,wav:16,opus   # three files at once
claudius export out/deep_house-liquid-avenue-21 -f mp3 -o ~/Desktop/liquid-avenue.mp3
claudius export ~/Music/mix.wav -f aac --title "Night Bus" --artist "Ana"
claudius export --list                                                # every format, and any this machine cannot write
```

```text
wrote out/deep_house-liquid-avenue-21/liquid-avenue.mp3 (MP3, 320 kbps, 8.4 MB)
wrote out/deep_house-liquid-avenue-21/liquid-avenue.wav (WAV, 16-bit, 44.1 kHz, 36.9 MB)
wrote out/deep_house-liquid-avenue-21/liquid-avenue.opus (Opus, 160 kbps, 4.3 MB)
```

The files are named after the song's title; when two share an extension, the later ones also carry their format
(`liquid-avenue-mp3-192.mp3`). `--out` names the file for one format, or the folder for several.

In the studio, the **export** menu in the toolbar downloads the latest render in any of the formats below, as a file
named after the song; formats this machine cannot write are greyed out, and say why. An assistant has the `export`
tool, which writes into the song's folder (or a `path`), rendering first when the song changed since its last render:

```json
{"formats": ["mp3", "wav:16", "opus"]}
```

Over the [HTTP API](http-api.md), `GET /api/export/{version}/{name}.{ext}?format=flac:16` answers with that render
as a download named `{name}.{ext}`, encoded on the first request and kept with the render; `Command::Export`
(`{"cmd": "export", "formats": ["mp3"], "path": "..."}`) writes the files as the tool does.

## Formats

| format | what it writes | for |
|---|---|---|
| `wav` | WAV, 24-bit, 48 kHz | the master as it is: mastering engineers, other DAWs |
| `wav:16` | WAV, 16-bit, 44.1 kHz | CD, and distributors that ask for CD quality |
| `wav:32` | WAV, 32-bit float, 48 kHz | more processing elsewhere |
| `aiff`, `aiff:16` | AIFF, 24 or 16-bit | Logic, GarageBand and Apple's other apps |
| `flac`, `flac:16` | FLAC, 24 or 16-bit | lossless at about four fifths of the size or less: Bandcamp, archives |
| `alac`, `alac:16` | Apple Lossless in an `.m4a` | lossless in Apple Music and on an iPhone |
| `mp3` | MP3, 320 kbps | sharing, and every player there is |
| `mp3:v0` | MP3, variable bitrate (about 256 kbps) | the same quality in smaller files |
| `mp3:192`, `mp3:128` | MP3 | smaller files still |
| `aac` | AAC in an `.m4a`, 256 kbps | Apple Music, the iPhone, the web |
| `opus`, `opus:96` | Opus, 160 or 96 kbps | the web: the smallest files at a given quality |
| `ogg` | Ogg Vorbis, quality 7 | games, and players of Ogg |

A format is written `format[:quality][@rate]`:

- the quality is the bits of a lossless file (16 or 24, or 32 for a float WAV), the kbps of MP3 (32 to 320), AAC (64 to
  320) or Opus (32 to 256), `v0` to `v9` for MP3's variable bitrate, and 0 to 10 for Vorbis;
- the rate is `@44100` or `@48000`. A 16-bit file is 44.1 kHz unless you say otherwise; everything else keeps the
  master's 48 kHz, and Opus is always 48 kHz.

To 44.1 kHz the master is resampled with a filter flat to 20 kHz that keeps what cannot exist at the lower rate
95 dB down, and 16-bit files are dithered, so quiet endings fade instead of breaking up.

## For the web

Every current browser plays MP3, AAC, Opus, Ogg Vorbis, FLAC and WAV; Safari plays Ogg Opus and Ogg Vorbis from 18.4
(macOS 15.4, iOS 18.4). Opus makes the smallest files for streaming, and MP3 or AAC reach the oldest devices. AIFF and
Apple Lossless are for Apple's apps: Chrome and the browsers built on it play neither.

## Tags

Files carry the song's title (or `--title`), `--artist`, `--album`, and the name and version of the claudius that
made them: ID3v2 in MP3, Vorbis comments in FLAC, Opus and Ogg. AAC and Apple Lossless carry them when ffmpeg writes
them; Core Audio writes none.

## How each format is written

WAV, AIFF, FLAC, MP3 and Opus are written by claudius itself, in Rust, and Ogg Vorbis by libvorbis (aoTuV), which is
built in: none needs another program. Every FLAC is decoded again before it is kept, and stored verbatim if it does
not come back exactly. AAC and Apple Lossless come from the system's encoder: Core Audio on macOS, which is Apple's own
AAC encoder, and ffmpeg on Windows and Linux. Where neither is there, those two formats are unavailable, `--list` says
so, and exporting them fails with `AAC needs Core Audio (macOS) or ffmpeg, and this machine has neither`.

Measured on a song against the usual encoders at the same bitrate (the spectrum's difference from the master, in
third-octave bands), claudius' MP3 matches LAME's at 192 and 320 kbps, and its Opus matches libopus at 160 kbps.
