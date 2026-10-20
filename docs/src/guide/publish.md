# Publishing

Spotify, Apple Music, Amazon Music, YouTube Music, TikTok, Tidal and Deezer take music only from distributors:
DistroKid, CD Baby, TuneCore, Amuse, SoundCloud for Artists and others. A distributor's upload form asks for an audio
file, a cover and a page of details, and stores turn a release down for a cover of the wrong size, a sound without a
licence or a title in capitals. `claudius publish` checks a song the way they do, and writes a release kit: the files a
distributor takes and every answer its form asks for.

```sh
claudius publish out/lofi-hollow-harbor-5 --artwork ~/Pictures/hollow-harbor.jpg \
  --artist "Ana Lima" --writer "Ana Lima" --made assisted --date 2026-12-04
claudius publish out/lofi-hollow-harbor-5 --check     # check again, and write nothing
```

```text
Hollow Harbor by Ana Lima
  artwork  3000 × 3000 JPEG
  audio    3:12, -12.5 LUFS, -2.3 dB true peak
  made     written with help: an assistant or claudius' generator wrote some of it, and you shaped it

  ..  audio    the master is -12.5 LUFS: Spotify plays it 1 dB quieter and Apple Music 3 dB quieter, as they play songs at about -14 and -16 LUFS
  ..  sounds   Strudel piano (Salamander, one layer), in keys, asks for credit, which release.txt has: Salamander Grand Piano V3 by Alexander Holm (CC BY 3.0), Strudel edition by Felix Roos, https://creativecommons.org/licenses/by/3.0/

wrote the release kit to out/lofi-hollow-harbor-5/release:
  Ana Lima - Hollow Harbor.wav           WAV, 16-bit, 44.1 kHz: the one to upload, which every distributor takes
  Ana Lima - Hollow Harbor (24-bit).wav  WAV, 24-bit, 48 kHz: for distributors that take high-resolution audio
  cover.jpg                              3000 × 3000 JPEG
  release.txt                            what to enter in the distributor's form
  release.json                           the same, for programs
```

Checks marked `--` are problems: a store would turn the release down, so no kit is written until they are fixed, and
the command fails. `!` marks a warning, worth fixing, and `..` a note, worth knowing. `--json` prints the checks as
JSON.

## In the studio

**publish** in the toolbar opens the Publish dialog. Drop the cover onto the studio (the native studio also takes its
path), fill in what stores ask, and say how the song was made; each field is sent as you leave it, and the checks
below, of the latest render, come back with the answer. **Write the release kit** writes it into `release/` in the
song's folder once nothing stops the release. The details and the artwork stay with the song: saving it, or writing
the kit, puts `release.json` and `artwork.jpg` (or `artwork.png`) in its folder, and opening that folder again brings
them back. In the web studio, the browser reads the dropped cover and sends it to claudius.

## Over MCP and HTTP

An assistant has the `publish` tool. It passes on what the person said (the artist, the songwriters, the artwork's
file, how the song was made) and gets the checks back, and with `write`, it writes the kit too:

```json
{"artist": "Ana Lima", "writers": ["Ana Lima"], "artwork": "/Users/ana/Pictures/hollow-harbor.jpg", "made": "assisted", "write": true}
```

What the tool leaves out stays as it was. Its description, and the claudius-songs skill, tell the assistant to ask
the person for these, and never to answer how the song was made for them. Over the [HTTP API](http-api.md),
`set_release` sets the details and an artwork file, `POST /api/release/artwork` takes the artwork as the file itself,
`write_kit` writes the kit, and `State.release` holds the checks.

## The release kit

The kit goes into `release/` in the song's folder, or the folder `--out` names. Writing it again replaces it, and
removes the files of the kit before it, which a new title would otherwise leave behind.

| file | what it is |
|---|---|
| `<artist> - <title>.wav` | the master as WAV, 16-bit at 44.1 kHz: the file to upload, which every distributor takes |
| `<artist> - <title> (24-bit).wav` | the master, 24-bit at 48 kHz, for distributors that take high-resolution audio |
| `cover.jpg` or `cover.png` | the artwork, exactly as you gave it |
| `release.txt` | every answer the forms ask for, what to say about AI, the credits the sounds' licences ask for, and the checks |
| `release.json` | the same for programs, with the artwork's and the audio's measurements |

`release.txt` reads like this, ready to copy into a form:

```text
For the form
  Title             Hollow Harbor
  Version           (none)
  Primary artist    Ana Lima
  Featuring         (none)
  Release           Hollow Harbor, a single
  Genre             Hip-Hop/Rap, Lo-Fi Hip-Hop
  Release date      2026-12-04
  Record label      Ana Lima
  Songwriters       Ana Lima
  Producers         (none)
  ℗ line            2026 Ana Lima
  © line            2026 Ana Lima
  ISRC              (leave it empty: the distributor assigns one)
  UPC               (leave it empty: the distributor assigns one)
  Explicit lyrics   no
  Lyrics language   none (instrumental)
  Length            3:12
```

## What you give

Everything you give is kept in the song's folder as `release.json`, so the next `claudius publish` of that song needs
none of it again; each flag replaces what it says. A song without one starts from its own title and genre, and takes
the artist, songwriters, producers, label and owner from the latest release among the song folders next to it.

| flag | what it is |
|---|---|
| `--artwork` | the cover: a square JPEG or PNG, 3000 × 3000 |
| `--artist` | the name the song comes out under |
| `--title`, `--version` | the title (the song's, unless you give one) and a version such as "Extended Mix" |
| `--featuring` | a featured artist (repeatable) |
| `--album` | the album or EP the song is on; without one it is a single |
| `--genre`, `--subgenre` | as stores list them: the song's genre gives a start (house is Dance, House) |
| `--date` | the release date, `YYYY-MM-DD` |
| `--label` | the record label; the artist's name when you have none, as distributors advise |
| `--writer` | a songwriter's full legal name (repeatable): publishing royalties are paid to it |
| `--producer` | a producer (repeatable) |
| `--owner` | who owns the recording and the composition, for the ℗ and © lines; the artist by default |
| `--isrc`, `--upc` | the codes, if you have them; the distributor assigns them otherwise |
| `--explicit` | the lyrics are explicit (`--explicit false` takes it back) |
| `--made` | how the song was made: `written`, `assisted` or `generated` (below) |

## How the song was made

Distributors ask whether a song is made with AI: DistroKid asks whether it includes AI-generated music, vocals or
lyrics, and Spotify shows the AI credits distributors send. claudius makes no audio, vocals or lyrics with AI: its
sounds are synthesized or recorded. Who wrote the music is the question, and only you can answer it:

| `--made` | when | what `release.txt` says |
|---|---|---|
| `written` | you wrote the notes, the parts and the arrangement, with claudius as the instrument | no AI: answer no where a distributor asks |
| `assisted` | an assistant or claudius' generator wrote some of it, and you shaped it | AI-assisted composition: answer yes, and credit AI-assisted composition where AI credits are asked for |
| `generated` | an assistant or claudius' generator wrote it | AI-generated composition: answer yes, and credit AI-generated composition |

Until it is said, the release has a problem: every distributor asks, and the answer is yours to give. Release one
song at a time, a song you would play to someone: stores remove songs uploaded in bulk to collect plays, and
distributors close the accounts behind them.

## The checks

### Artwork

| check | | why |
|---|---|---|
| a JPEG or a PNG | problem | stores take nothing else: a WebP, HEIC or TIFF is named, to save again |
| square | problem | every store shows covers 1:1 |
| at least 1400 × 1400 | problem | Apple Music's minimum |
| at most 10000 × 10000 | problem | Spotify's maximum |
| 3000 × 3000 | warning | what stores ask for; TuneCore takes no less than 1600, and CD Baby and TuneCore no more than 3000 |
| RGB | problem | CMYK is for print, and stores take RGB only, for black and white art too |
| full colour, 8 bits a channel | warning | Spotify takes 24-bit colour: no palette, no 16 bits a channel |
| opaque | problem | stores need artwork with nothing see-through; an unused alpha channel is a note |
| sRGB | warning | stores show covers as sRGB, so Display P3 or Adobe RGB colours shift |
| stored upright | warning | stores ignore the tag that turns a photo, so a turned one shows sideways |
| JPEG quality 80 or more | warning | a lower one shows blocks and smears on a large screen; the quality is read from the file's tables |
| sharp | warning | stores turn down blurry or pixelated art |
| at most 10 MB | warning | TuneCore's limit (CD Baby's is 25 MB) |

The sharpness check measures the width of the strong edges in the picture. In a sharp photo or type, the sharpest of
them cross in one to three pixels; in a picture enlarged k times, none is narrower than about k pixels. It finds covers
enlarged from a third of their size or less, and says what size they look enlarged from. Art that is soft on purpose,
glass or mist, has no sharp edges either, so the warning says to ignore it then.

Stores also check what is in the picture, which claudius cannot see, and `release.txt` lists it: the only text is the
artist's name and the title, spelled as in the form; no web addresses, QR codes, social media names or logos, store
logos, prices, or "CD"; an image you made or have a licence for; not the artwork of another release; and nothing
blurry, pixelated, turned or cut off.

### Audio

| check | | why |
|---|---|---|
| 30 seconds or longer | warning | Spotify counts a play from 30 seconds on, so a shorter song earns nothing there |
| silence at the start under 2 s, at the end under 5 s | warning | listeners expect a song to start, and to end when it does |
| true peak at most -1 dB | warning | stores' encoders can clip a master above it; `master(#{ ceiling: -1.0 })` keeps it under |
| louder than -14 LUFS | note | Spotify and Apple Music turn songs down to about -14 and -16 LUFS: nothing to fix |

### Sounds

A release needs the rights to every sound in it, so each recorded sound the song plays is checked against where it
came from:

| sounds from | what it means |
|---|---|
| packs under CC0, the Unlicense or public domain, and the curated packs with licences of their own (jRhodes, the TR-909 pack) | nothing to do: their licences allow music made with them |
| packs under CC BY (Salamander, Greg Sullivan's E-Pianos, MuldjordKit, the Strudel piano) | a note: credit them, as `release.txt` does, in the release's credits or notes and in descriptions |
| packs whose licence is unknown (Tidal drum machines, Dirt-Samples) | a problem: swap those sounds |
| packs under CC BY-SA (Mridangam) | a problem: a song made with them has to be shared under the same licence, which stores rule out |
| packs that forbid commercial use or new works (NC, ND) | a problem |
| a pack this machine does not have | a problem: its licence cannot be checked |
| a sample map you added, with a licence of its own | a warning: read it |
| Apple Loops | a note: Apple's licence allows releasing music made with them, though not the loops on their own |
| your own recordings, and chops of them | a warning: release them only if you made them or have cleared their samples |

### Details

| check | | why |
|---|---|---|
| an artist | problem | and not "Various Artists", "Unknown" or the like |
| how the song was made | problem | [above](#how-the-song-was-made) |
| one artist per name | warning | others go under featuring: stores add them to the title |
| a title that is a title | warning | stores turn down placeholders ("Track 3", "Untitled"), promotion and producer tags ("Free Download", "Prod. by"), featured artists, capitals and emoji |
| songwriters, by full legal name | warning | distributors ask for them, and a stage name is not one |
| an ISRC and a UPC, if given, that are codes | problem | 12 characters (`USRC17607839`), and 12 or 13 digits with a check digit |
| a date that is a date | problem | `YYYY-MM-DD`; a date that has passed is a warning, one less than four weeks ahead a note: stores take days to weeks, and Spotify's editors hear songs pitched a week before |

## Where to send it

Make an account with a distributor, upload the 16-bit WAV and the cover, and copy the answers from `release.txt`.
They all ask much the same; the main differences are what they charge, how much of the earnings they keep, and the
stores they reach. SoundCloud for Artists distributes from SoundCloud with an Artist Pro subscription, and SoundCloud
and Bandcamp also take the same WAV and cover directly, with the credits in the description.
