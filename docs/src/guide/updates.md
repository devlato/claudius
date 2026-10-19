# Updates

Claudius looks for a new release once a day, downloads it in the background, and installs it when you say so.

## In the studio

When a new release has been downloaded and checked, a bar under the toolbar says so. **Restart to update** installs
it, closes the studio, and opens the new version. **What's new** opens its release notes, and **Later** puts it off
until the studio opens next. While it downloads, the status bar shows how far it is.

The **?** menu has **Check for updates**, which looks now and says what it found, and **Update automatically**, which
turns the daily look off and on.

## How each install updates

| installed with | the update |
|---|---|
| the Mac app (the install script, or a disk image dragged to Applications) | the new disk image for your Mac; its app takes the old one's place. The app has to be in a folder you can write in, as Applications is |
| an AppImage | the new AppImage takes the old file's place |
| the install script on Linux, or on a Mac with `--no-app` | the new archive's programs and web studio take the old ones' place in `~/.local` |
| the Windows installer, or `install.ps1` | the new installer runs once Claudius has closed |
| Homebrew, cargo, the `.deb` or `.rpm`, a build from source | nothing is downloaded: the bar says how it updates (`brew upgrade claudius`, your package manager, ...) |

## From the command line

```sh
claudius update             # downloads the latest release, and installs it after asking
claudius update --yes       # without asking
claudius update --check     # only says whether there is a newer release
claudius update --auto off  # the studio stops looking (on turns it back on)
```

## Checked before it is installed

The latest release is the one GitHub's `releases/latest` points to. Its file for your computer is downloaded over
HTTPS, and checked against the SHA-256 sums the release lists in `SHA256SUMS`; a file that does not match is thrown
away. The releases cannot be changed once published, and each has a provenance attestation (see
[Install](../getting-started/install.md)).

## Your songs

Every release opens the songs that earlier ones saved: the test suite keeps the songs each release saved and opens
them all ([saved songs keep opening](../engineering/song-model.md#saved-songs-keep-opening)). A song saved by a newer
Claudius than yours says so, and asks you to update.

## What it sends

The daily look is one request to `github.com` for the address of the latest release, and, when there is a newer one,
the download of its checksums and its file for your computer. GitHub sees these requests, as it sees any download,
and nothing about you or your music goes with them. It is off with `claudius update --auto off`, **Update
automatically** in the studio's **?** menu, or `CLAUDIUS_UPDATES=off`, and in CI.

`CLAUDIUS_UPDATE_SINCE=0.1.0` makes Claudius act as if it were that version, to try an update against the latest
release.
