# Changelog

All notable changes to claudius are listed here. The format follows [Keep a Changelog](https://keepachangelog.com),
and claudius uses [semantic versioning](https://semver.org): until 1.0, a minor version may change the script
language or the API, and the changelog says how.

## [Unreleased]

claudius keeps count of how fast it runs: each render, each listening of the ears and each recording decoded, the
seconds of audio and the seconds they took. `claudius serve` answers `GET /api/metrics` with the totals in Prometheus'
text format, for a scraper or a look. The usage statistics, for those who turned them on, now also send each kind's
speed (seconds of audio a second, to two figures) at most once a day, so that a release that renders slower shows; the
events have a `value` for it (`"v": 2`), and older versions' events still count.

The phone's turntable plays records a DJ would reach for: the song's samples (the phrase it was built on, a flip, a
take), its voices, its parts, a sampled break, or the whole mix, never a lone kick or clap. With the motor on, the
record spins in time with the song, and a pitch fader moves it up to 16 % either way, with a button back to time. A
finger holds it as a DJ's does: resting, it drags the record slower the harder it presses (read from how large the
fingertip's touch is, since a phone measures no force) and stops it under a firm press; moving, it grabs it to scratch
or spin it by hand; and let go mid-swipe, the record runs on faster, or spins back, until the slipmat brings it back.
The motor brings the platter up to speed and the brake stops it in a third of a second, as a DJ's turntable does.
It also puts on samples from the library: the phrases, vocal chops and breaks gathered for the song's genre, and Apple
Loops and your own loops of its kinds, each spinning at the speed that plays it in time with the song, and a scratch
on one credits it in the song's `credits.txt`. **● mic** records a record off the phone's microphone, up to 8 seconds,
where the browser lets the page use one (the demo's page, or the page opened on the computer running claudius); a
scratch on it is a take like any other (`add_scratch` has a `recording`).

Songs play with a feel. `humanize` no longer jitters each hit on its own by a few milliseconds, too little to hear:
each hit now drifts with the ones around it, behind the beat for a while and ahead for a while, as a player's does
(a 1/f drift, which listeners prefer in Hennig et al.'s study), by about 15 ms at 1, the kick less and the hats
more. The drum lanes share part of the drift, as one drummer's limbs. A lane or a part can also play every hit
behind or ahead of the beat on purpose (`lane("snare", #{ late: 25 })`, `part("bass", #{ late: 15 })`,
`set_lane`'s and `set_part`'s `late_ms`, a **feel** slider in the sequencer's lane settings), as an MPC's shift
timing moves a pad: J Dilla's snare late and kick early. The genres that play that way do it from the start: a
neo-soul snare 18 to 38 ms behind and its bass 12 to 30, a boom bap or lo-fi backbeat laid back, reggae's rim and
bass behind, a funk drummer's hats pushed. Songs made before sound a little looser than they did.

Hip hop sounds like hip hop rather than a lounge: no more vibraphone leads, organ, pizzicato, choir or soul-jazz
vamps, but dark minor loops of a few chords on a piano or a Rhodes, strings under them now and then, a ticking hat,
and a distorted 808 or sub. And a song sounds of the 80s only where its genre does: the FM tine piano, leads, basses
and mallets, the square and pulse-width leads and pads, the string machine and the chip are drawn only in a genre
whose palette is `retro` (synthwave) or whose parts name them (grime's square lead, UK garage's FM bass), and so are
an installed FM piano, CP-80 or Pianet and General MIDI's synth patches, the cheap polyphony of an old sound card;
General MIDI's other sounds play only where nothing else of their kind is installed. Alternative hip hop, neo-soul
and R&B play Rhodes and Wurlitzers, pianos, real strings and warm or vocal synths instead of a DX7 piano, an FM lead
standing in for a flute and a string machine.

Genres can be searched, and are grouped by family. The New song dialog has a search box above the genres, which it
lists under their families (hip hop, house, bass music ...) until one types: then it shows those that answer, best
first. A search finds a genre by its name, by the other names, eras, scenes and artists it goes by (`aliases`: "90s
hip hop", "golden age", "west coast", "r&b"), by its family and by its sound ("dusty"). So does `claudius genres
WORDS`, `GET /api/genres?q=WORDS` and the MCP tool `find_genre`; `catalog` lists the genres by family with their
other names, and `new_song` and `remake` take another name that is no other genre's ("90s hip hop" is boom bap). A
genre file of your own can say its `family` and `aliases` too.

Fourteen more genres, most of them the hip hop and R&B people asked for by name: G-funk (the West Coast's Minimoog
bass and the whine of a new synth lead that glides a long way, `glide_lead`), gangsta rap (the 90s' hardcore dark
piano and string loops on dusty breaks), jazz rap (the Native Tongues' jazz records, upright bass and horns), chipmunk
soul (the 2000s' soul records sped up until the voices rise), crunk, cloud rap, emo rap (built on a guitar loop),
rage, plugg, NY drill, Chicago drill, 90s R&B (hip hop soul and slow jams on the DX7 piano), new jack swing and
alternative R&B. Each is searched by what it is called (`west coast`, `90s r&b`, `pbr&b`, `sped up soul` ...).
`drill` now finds all three drills rather than naming UK drill.

**Remake** keeps the song recognizable: it reworks the recording rather than building a new song on its hook. All of
it plays through, from its first downbeat to its end, at its own speed and pitch, its voice, chords and melodies as
they are, without its drums and with its low end cut; the song is its own sections bar for bar, on its chords bar by
bar and in its swing; and the genre plays under it, its drums, its bass on those chords' roots in place of the
recording's, its break and textures and its treatment of a sampled phrase, its mix. It plays at the recording's tempo, or
double it, so that nothing is stretched. The remix it made before, a new song of the genre on the
recording's hook, a passage of its verses, its drums, a flip of it and its sung phrases, is **Flip into** in the New
song dialog, and `flip: true` in `remake` (`--flip`, `?flip=true`).

The online demo listens to the recordings you choose, as claudius does, instead of making up what it hears: **Analyze**
analyzes the file, **Remake** reworks it and **Flip into** flips it, **Sample its vocals into the song** samples them
(it said "the demo samples no recordings: start `claudius serve` to sample one"), and **Take its groove** takes it
(it said "the demo hears no grooves"). It all runs in the page, and what a song keeps of a recording goes in the
project you save.

The demo records takes too: the record button, which only the app and a claudius' own page had, records a voice or
an instrument over the song in the browser, and the take joins the song and the project you save. A take's menu in
the arrangement works there as in claudius: **How is it sung?** hears how it is sung, **Turn it into drums** turns a
played or beatboxed beat into the drum lanes, and **Turn the melody into notes** plays what was sung as a part, which
the demo turned down before (it said "the demo hears no singing" and "the demo hears no drums").

## [0.4.0] - 2026-11-06

The phone plays the song in the studio as well as following it: **▶** plays it and **■** stops it, so a take needs no
trip to the computer. With the song stopped, ● starts it a bar before the section it is at, and the take starts with
that section.

**Remake** remixes a recording in the genre you pick, rather than writing a new song only on its skeleton. The new
song is built on the recording's own sound. Its hook without its drums plays in the hooks, and the song takes its key
and chords. Another passage of it plays in the other sections, and its drums are the break where the genre plays one.
A flip of it, half beats cut from all over it on the hook's chords, plays in the intro and the outro, and its sung
phrases come in where they agree with the chords. The genre plays it at the recording's own tempo, or at a multiple of
it as a sampler would. `sampled: false` (`--skeleton` on the command line) remakes the skeleton only, as before.

A new genre, `alt_hip_hop`: alternative hip hop in the vein of Tyler, the Creator's records, layered as its producers
layer it. It has lush ninth and eleventh chords (a new progression style, `lush_soul`, with its minor iv borrowed and
chromatic mediants) on a Rhodes stacked on warm synths, strings and a flute; a distorted 808 with a fuzzy synth bass
an octave over it; a swung kit with a clap on the snare; soul records chopped under it; and a beat switch into half
time before the last hooks. Its songs are layered at complexity 0.8 unless asked otherwise.

A new song can be as layered as you like: **Layers** in the New song dialog, `--complexity` on the command line and
`complexity` in `new_song` and `song()`, from 0 to 1. At 0.5 a song is its genre as written; toward 1 its harmony and
lead are doubled on second instruments (an octave up, or in unison on another sound), its bass an octave up through a
high-pass, and a counter-melody, an arpeggio over the hooks, a pad, a clap on the snare, a shaker and a percussion line
come in where the song lifts, with more of the samples a sampled genre colours its songs with and the drums a break
stands in for played over it. Toward 0 it is thinned to its core. A double is a part that plays another part's notes
(`doubles` and `octave` in `part()`), so it follows that part through edits and mutations.

A sample doesn't always play slowed down or sped up to the song's tempo. A phrase can also be chopped onto the song's
grid at its own pitch, at two thirds, three quarters, four thirds or three halves of its tempo, as an MPC chops a
record: each half beat plays from the song's half beat, so its bars and chords line up. A 120 BPM record plays at its
own pitch in a boom bap song at 90. The song's seed picks among the ways that suit the record and the genre: chopping
suits a record with a clear pulse in a genre that chops, and house still speeds its records up. A break off the song's
tempo is chopped onto the grid too, rather than stretched, so its hits stay crisp. In the web demo, where nothing
stretches, a phrase no speed fits now plays chopped instead of being left out.

On a phone playing along, a quick second tap plays again instead of zooming the page in, wherever it lands, and two
fingers apart are two fingers rather than a pinch. The pads and the keys are drawn on a canvas, where every finger is
its own, so several play at once and a finger slid along the keys glides from key to key.

A scratch sounds like a turntable's, on the phone and in the take it makes. The platter follows the finger as a
platter follows a hand, its speed changing smoothly through each turn back, where it jumped every few milliseconds. A
record moved fast whistles no highs folded back: the needle reads it from a copy whose highs are taken out as far as
its speed needs. The crossfader opens within a hair of its travel, as a scratch DJ's does. The record waits at its
first sound, which a sticker marks, and the deck takes a voice, a sample, a lead or a stab when the song has one. Let
go, the record comes up to speed (or to rest) in a fraction of a second, as a real one does.

The [studio demo](https://claudius.devlato.com/studio/?mock) mutates and evolves songs, as claudius does. Its engine
judges the variations one after another in the page, without the ears, and keeps the best when it beats the song;
**Stop** keeps the best judged so far.

The [studio demo](https://claudius.devlato.com/studio/?mock)'s download button hands over the song as FLAC,
lossless at about half the size of the WAV it gave before. Its export menu offers WAV, AIFF, FLAC, MP3 and Opus,
which the demo's engine writes in its Web Worker, so the page doesn't stop while a song encodes.

A song begun in the demo can be carried on in claudius. The export menu's **claudius project (.zip)** holds the
song, every sample it plays (cut from the free libraries' recordings, in lossless FLAC) and, beside each, the
recording it came from: its page, who made it and its licence, with their credits in `credits.txt`. **Open** the zip
in the studio, or `open` it from an assistant, and claudius unpacks it into a song folder of its own under
`~/Music/claudius` and opens it there, samples and all. The `blueprint.json` of the folder Safari unpacks the zip
into opens the same way.

Some genres sample a bass note, as their records do: funk a slap bass, reggae a dub bass, phonk a distorted 808,
boom bap, lo-fi and trip-hop an upright's pluck, and the disco, soul and hip hop genres a bass guitar, a fretless or
an 808 now and then. The song's bass keeps its notes and plays them on the note, as a sampler would. A note is cut
from its attack, at the pitch it sounds at: an 808's C1 is found, and so is a low note whose fundamental is weaker
than its octave. A kick, a drum loop, a whole song or a note out of a full mix isn't kept.

The mix balances each track on how loud its hits are. It measured a track over 3-second windows, which hear a clap on
two and four with the silence between its hits, so the sparse tracks played louder than their levels said: snares and
claps by about 4 dB, crashes by 3, impacts by 5 and reverse crashes by 7, an impact as loud as the kick. Measured over
the 0.4-second windows that hear each hit, every track now plays within about a dB of its level, and the snares, claps
and transitions no longer crowd the bass and the chords.

The critic hears each track against the others. It judged the mix by meters on the master alone, so a song with its
chords 9 dB too loud could still score in the high 80s. Now a track more than 3 dB from the level its genre makes it
at is named and costs the mix (`chords is 9.0 dB louder than the chords level`, target `level:chords+`), and evolution turns it
back. A part playing a sound its genre has no place for costs the song (`sound:<part>`). The critic is held to this
by tests that break a song it passes, one fault at a time.

Eight new genres, thirty-six in all:
- **hip hop:** modern, between boom bap and trap.
- **R&B:** lush ninths on Rhodes over an 808, with rolling hats and snaps.
- **funk:** on the one, with ghost-note snares, clavinet, rhythm guitar and horns.
- **tech house:** a rolling offbeat bass under looping percussion.
- **ambient:** pads, drones and field recordings that swell and fade.
- **grime:** jagged 140 BPM drums with square-wave bass and eski synths.
- **phonk:** a distorted 808, with a new `cowbell_lead` preset playing the melody the way an 808's cowbell does.
- **reggae:** the one drop, with a skank on the offbeats and dub's echo.

Each digs for its own kinds of record when it samples.

A song of any genre can be built on samples from free libraries, as records built on samples are: a phrase the song
takes its key, tempo and chords from, a drum break, a texture. Each genre digs for its own kinds of record (house for
disco, gospel and philly soul, trap for orchestral strings, choirs and flutes, afrobeats for highlife guitar), tried in
an order each song's seed picks, or for words of your own (`new_song` with `sample_search: "70s soul"`,
`claudius make --sample-search`). A phrase plays as a sampler plays a record, faster or slower by whole semitones until
it falls in the genre's tempo, and the song takes the tempo and key that come of it, so it is resampled, not stretched,
and in tune. Lo-fi, jungle, big beat and filter house are now built on samples by default, like trip-hop and boom bap;
`sampled: false` (`--unsampled`) makes them of their own sounds. A genre's collection grows a few recordings with each
new song ([Songs built on samples](https://claudius.devlato.com/docs/guide/online-samples.html#songs-built-on-samples)).
The [studio demo](https://claudius.devlato.com/studio/?mock) samples every genre the same way, from Freesound, Jamendo
and Wikimedia Commons: claudius itself cuts each recording in the page, a phrase out of a whole song as the app cuts
one, where the demo cut only loops whose titles stated a tempo. In the studio and the demo alike, the New song dialog's
**auto-sample from free libraries** turns it on or off for the song, and its field takes the kind of record to search
the phrase for.

A song built on samples plays two passages of its record, as records built on samples change what they play from one
part to the next: the phrase, now chosen from bars the recording plays again, so that it loops as the record does,
and two bars from another section of the recording (or on other chords). The louder of the two plays where the song
lifts, in its drops and hooks, and the other in its verses, breaks, intro and outro, each on its own chords (the
`sample` and `sample2` tracks).

A song treats its phrase as producers treat a record, with one move that its seed picks from those its genre makes:
held behind a filter that opens as the song lifts, darkened, narrowed to a telephone's band in the intro and the
breaks, overloaded or smashed, or crushed to 12 bits. Trip-hop darkens and filters, big beat overloads, lo-fi darkens
and crushes, and filter house keeps its own sweep. The song's history names the move.

Songs chop vocals from other records, as garage, house and future bass do: a gathering finds a note a voice holds on
an a cappella or a vocal sample (the steadiest and loudest of about a third of a second), and a new part, `vox`, plays
it on the song's chords in the drops and hooks, moved from the note it was sung at as a sampler would, as a motif or an
arpeggio within a fifth of it. Eleven genres dig for vocals, each for its own kind (R&B vocals for garage, gospel and
diva vocals for house, ragga for jungle), and it works in the web demo too, since a note has no tempo to fit.

Songs play only sounds their genre has a place for. A part that leaves its genre's own sound plays another synth of
the same family (the other electric piano, another analog bass, another soft lead), any synth pad for a pad, or an
installed instrument of a kind its genre's palette lists. Bells, winds and brass, whose sound pierces a mix, play only
where the genre plays one of their kind itself. So a house or drum & bass lead no longer rings a bell or whistles, an
installed glockenspiel or tubular bells play only where the genre plays bells (trap, UK drill, IDM), and a flute or a
saxophone only where one of the genre's parts names it. Where a song's sampled phrase plays, its lead and arpeggio
step aside, so the record carries the melody.

Searches for samples leave out the recordings of papers' supplementary material, which Wikimedia Commons holds
by the thousand under their journals' names for them (`…-pone.0062397.s008.oga`): a vowel recorded for a study of
voices holds a note, but it is no music. The studio demo's searches leave them out too.

A recording decodes ten times as fast: converting its rate (44.1 kHz, most often) to claudius' 48 kHz works out its
filter's weights once for each position between samples, where it worked them out for every sample, and what comes out
agrees to a rounding of a 32-bit float. The demo cuts a 3.6-minute song into a phrase in about 4.5 s where it took 18.

An edit renders again only the tracks it changed: a level, a pan, a send or the master renders no track again, and
another track's notes only that track, so mixing waits for the mix alone (a level change in the web demo takes about
13 s where the whole song takes 31). What comes out is the same to the sample as a fresh render. The tracks it keeps
leave out their silent stretches, about half of a song, so they take half the memory, which matters most in the
web demo on a phone.

In the [studio demo](https://claudius.devlato.com/studio/?mock), an edit is heard where you are listening within a
second: the twelve seconds from the playhead, rendered again with the edit through the same effects as the whole song
(within a rounding of it, at the loudness of the render before), while the whole song renders again behind them. Where
the browser has cores to spare, the demo renders a song's tracks side by side in copies of its engine, as the app does
on threads, and comes out the same: a four-minute song renders whole in about 19 s where it took 29. The heaviest
track, a song's chords or pads, is shared out too, each copy rendering the notes of a stretch of it.

A phone plays along: the studio's new phone button shows a QR code that opens drum pads, keys and a turntable on a
phone on the same Wi-Fi, with the song's own sounds. The phone plays them itself, so a tap sounds at once, and follows
the studio as it plays, so a take lands where it was heard. A take on the pads becomes the song's drum lanes, on the
keys a new part, and a scratch a take of its own ([Playing from a phone](https://claudius.devlato.com/docs/guide/phone.html)).
The [studio demo](https://claudius.devlato.com/studio/?mock) plays along with a phone too, on any network, through a
relay on the site. A MIDI controller plugged into what opens the page plays its pads, keys and turntable, the jog wheel
and crossfader learned once, and MIDI drum pads in the Record window play the song's drum lanes by the General MIDI
drum map. Assistants and scripts reach the same through the new `add_hits` and `add_scratch` commands.

The [studio demo](https://claudius.devlato.com/studio/?mock) runs claudius itself: the same composer, edits, script
language, renderer and critic as the app, in a Web Worker of the page, in place of the stand-in that made every genre
sound alike. Its songs are the app's, every track and section of them, on claudius' own synths and drums; a
script runs as in the app, and a song file opens. A new song plays its first 16 bars a few seconds after it is made,
and the whole of it once that has rendered. What needs a computer of one's own, such as recording, packs and
evolution, the demo says it leaves to the app.

Trap's and UK drill's 808 hits like a subwoofer: 6 dB louder than before, lower, driven through a saturator and kept
dry and centred, so it is the loudest thing in the song. Every track has a drive in the mixer (`drive`, 0 to 36 dB, in
`mix()` and `set_mix` too) that overloads it at the loudness its level gives it. The critic takes an 808 genre's
heavier sub as its sound rather than a fault, and Evolve no longer sends a bass or the kick into the reverb or pans
them.

Boom bap is built on samples too: a dusty drum break, a soul or jazz phrase from a record, chopped and crushed to 12
bits, and vinyl crackle, gathered from free libraries as trip-hop's are. The ears no longer hear it as stock ukulele
music; they hear the hip hop family, lo-fi hip hop (boom bap's offspring) first.

The web demo samples too: a trip-hop song in the [studio demo](https://claudius.devlato.com/studio/?mock) is built
on loops from Freesound under CC0 or CC BY, found as the song is made. Its tempo, key and chords come from the phrase,
and a drum break (re-chopped in the fills) and vinyl crackle play with it, each credited in its track and in the log.
The browser asks only the site, whose Worker asks Openverse and Freesound.

The studio and assistants gather samples on demand too: the first new trip-hop song gathers its break, phrase and
crackle as a job, with progress and its log, then makes the song with them. `claudius studio --offline` and a shared
claudius gather nothing.

Four genres play their signature sounds instead of whatever the palette drew, and the ears hear the first three
as themselves more often:

- dubstep's wobble bass, a sine sub and a big snare-and-clap on the third beat, on an 808 kit, with hats no longer
  on the offbeat, which the ears heard as techno;
- future bass's supersaw chords, plucked and vocal leads, and no chip or mallet sounds, which the ears heard as
  chiptune;
- synthwave's saw and Moog basses, string-machine and PWM pads, saw and square leads, chorus and a wobbling tape;
- amapiano's log drum, a new synth preset (`log_drum`): a pitched, woody knock that glides from note to note. The ears
  still hear its close kin, afrobeats, first.

Art rock plays like a band rather than a dance track. It has an acoustic kit with fills, a pick or finger bass that
distorts in the climaxes, piano, strings, a clean guitar arpeggio, and two distorted power-chord guitars panned wide
in the rise and the climaxes, over dark minor and phrygian chords. EDM risers, impacts and downlifters, harps,
mallets and music-box bells are gone. Chord parts can be voiced as power chords (`"voicing": "power"`: the root, its
fifth and the octave).

Trip-hop is built on samples now, as trip-hop records are: a slowed drum break whose own hits the drum lanes play, a
dark phrase from a recording that the song takes its key, tempo and chords from, and vinyl crackle under it. `claudius
make` gathers them first from free online libraries, cuts them on their beat grids and lets the ears choose the ones
that sound right; `claudius samples gather <genre>` gathers on its own, and `--offline` plays what was gathered. Other
genres can do the same with a `sampling` section in their profile
([Genres built on samples](https://github.com/devlato/claudius/blob/main/docs/src/guide/online-samples.md#genres-built-on-samples)).

`claudius samples` finds sounds in free online libraries (Freesound and Wikimedia Commons through Openverse, Commons
itself, ccMixter) and downloads them: drum breaks, vinyl crackle, field recordings, stems, old records. It keeps only
what the licence lets you publish music made with (CC0, public domain, CC BY), leaves out uploads that say they were
cut from someone's record, and every song that plays a downloaded sound credits it in its `credits.txt`
([Samples from free online libraries](https://github.com/devlato/claudius/blob/main/docs/src/guide/online-samples.md)).

Verdicts are stricter about style. Every genre is now measured against stock library music too (corporate background
music, children's piano music, ukulele music, chiptune), and a song that sounds as much like it as like its genre
loses up to 40 % of its score, with an issue saying so. Songs that scored 90 while sounding like an advert's background
music now score in the 60s ([The critic](https://github.com/devlato/claudius/blob/main/docs/src/engineering/critic.md)).

### Added

- samples: Samples from free online libraries, kept only under licences that allow publishing ([#95](https://github.com/devlato/claudius/pull/95))
- genres: Trip-hop built on samples from free libraries ([#96](https://github.com/devlato/claudius/pull/96))
- genres: Art rock plays like a band, with power chords ([#99](https://github.com/devlato/claudius/pull/99))
- genres: Dubstep, future bass, synthwave and amapiano play their signature sounds ([#100](https://github.com/devlato/claudius/pull/100))
- server: The studio and assistants gather a genre's samples when a song of it is made ([#98](https://github.com/devlato/claudius/pull/98))
- demo: The web demo samples free loops for trip-hop ([#97](https://github.com/devlato/claudius/pull/97))
- genres: Boom bap built on samples ([#101](https://github.com/devlato/claudius/pull/101))
- studio: The download button downloads the song ([#102](https://github.com/devlato/claudius/pull/102))
- mix: A drive on every track, and an 808 that hits like a subwoofer ([#103](https://github.com/devlato/claudius/pull/103))
- demo: The studio demo runs claudius itself, in a Web Worker ([#106](https://github.com/devlato/claudius/pull/106))
- demo: Open with a deep house song ([#107](https://github.com/devlato/claudius/pull/107))
- api: Add_hits and add_scratch turn pads and a turntable into drums and takes ([#108](https://github.com/devlato/claudius/pull/108))
- studio: Play along from a phone: pads, keys and a turntable that send takes into the song ([#109](https://github.com/devlato/claudius/pull/109))
- demo: Play along from a phone with the studio demo, through a relay on the site ([#110](https://github.com/devlato/claudius/pull/110))
- studio: MIDI drum pads play the song's lanes, and a MIDI controller plays the phone's page ([#111](https://github.com/devlato/claudius/pull/111))
- sampling: A song of any genre can be built on samples, from the records its producers dig for or words of your own ([#113](https://github.com/devlato/claudius/pull/113))
- demo: The demo samples every genre as the app does, claudius itself cutting whole recordings in the page ([#114](https://github.com/devlato/claudius/pull/114))
- studio: The New song dialog builds a song on samples from free libraries, from the genre's own records or words of your own ([#115](https://github.com/devlato/claudius/pull/115))
- demo: An edit is heard where you listen within a second, the same effects run on the bars at the playhead ([#117](https://github.com/devlato/claudius/pull/117))
- sampling: A sampled song plays another passage of its record where it lifts ([#118](https://github.com/devlato/claudius/pull/118))
- sampling: A song treats its phrase with a move its genre's producers make: filtered until it lifts, darkened, a telephone's band, overloaded, crushed ([#121](https://github.com/devlato/claudius/pull/121))
- sampling: Songs chop vocals from other records, a sung note played on the song's chords where it lifts ([#122](https://github.com/devlato/claudius/pull/122))
- judge: The critic hears a track out of balance and a sound out of place ([#128](https://github.com/devlato/claudius/pull/128))
- demo: Download the song as FLAC, or as WAV, AIFF, MP3 or Opus ([#130](https://github.com/devlato/claudius/pull/130))
- genres: Hip hop, R&B, funk, tech house, ambient, grime, phonk and reggae ([#129](https://github.com/devlato/claudius/pull/129))
- demo: Save the song as a project that claudius opens ([#132](https://github.com/devlato/claudius/pull/132))
- sampling: Sample a bass note that the song's bass plays on ([#133](https://github.com/devlato/claudius/pull/133))
- demo: Mutate and evolve songs in the demo's engine ([#134](https://github.com/devlato/claudius/pull/134))
- sampling: Chop a sample onto the song's grid at a multiple of its tempo ([#135](https://github.com/devlato/claudius/pull/135))
- compose: A song as layered as you ask, from its genre's core to instruments on instruments ([#139](https://github.com/devlato/claudius/pull/139))
- genres: Alternative hip hop, layered as its producers layer it ([#140](https://github.com/devlato/claudius/pull/140))
- remake: Remix a recording in a genre, built on its hook, its drums and a flip of it ([#141](https://github.com/devlato/claudius/pull/141))
- phone: Play the song and record a take from the phone ([#138](https://github.com/devlato/claudius/pull/138))

### Changed

- render: An edit renders again only the tracks it changed, and comes out the same to the sample ([#116](https://github.com/devlato/claudius/pull/116))
- audio: A recording decodes ten times as fast, its rate converted with each position's weights worked out once ([#120](https://github.com/devlato/claudius/pull/120))
- demo: The demo renders a song's tracks side by side in copies of its engine, and comes out the same ([#119](https://github.com/devlato/claudius/pull/119))
- render: A kept track leaves out its silent blocks, half a song, so it takes half the memory ([#124](https://github.com/devlato/claudius/pull/124))
- demo: The demo shares out its heaviest track in parts too, its costs measured, and comes out the same ([#125](https://github.com/devlato/claudius/pull/125))

### Fixed

- site: The hero links every system's download again ([#93](https://github.com/devlato/claudius/pull/93))
- critic: Songs that sound like stock background music no longer score well ([#94](https://github.com/devlato/claudius/pull/94))
- demo: A song built on samples renders in the browser instead of panicking for a process id ([#112](https://github.com/devlato/claudius/pull/112))
- samples: Searches leave out papers' supplementary recordings, which hold a note but are no music ([#123](https://github.com/devlato/claudius/pull/123))
- compose: Songs play only the sounds their genre has a place for ([#126](https://github.com/devlato/claudius/pull/126))
- mix: Balance each track on how loud its hits are ([#127](https://github.com/devlato/claudius/pull/127))
- judge: A track at the level its genre makes it at is no fault ([#131](https://github.com/devlato/claudius/pull/131))
- scratch: A scratch that sounds like a turntable's ([#137](https://github.com/devlato/claudius/pull/137))
- phone: Taps play without zooming, on pads and keys drawn on a canvas ([#136](https://github.com/devlato/claudius/pull/136))

## [0.3.0] - 2026-11-03

Claudius updates itself from this release on: the studio downloads a new release in the background and offers to
restart into it, and `claudius update` does the same in a terminal
([Updates](https://github.com/devlato/claudius/blob/main/docs/src/guide/updates.md)). The releases before this one
can't, so install this one the way you installed the last.

Sample packs installed by 0.2 can hold damaged FLAC files: two notes of VCSL's harmonica take hundreds of MB and
play as noise. `claudius-packs install --defaults` converts such files again and downloads nothing else.

Eight more default packs, and 21 more instruments in VCSL and FreePats: Sonic Pi's samples, a solo cello, an erhu, a
steel pan, synth drums, body percussion, glitch kits and sound effects, among them a hang, wine glasses and a TX81Z.
`claudius setup` downloads them, about 0.9 GB more ([Sample packs](https://github.com/devlato/claudius/blob/main/docs/src/guide/packs.md)).

The studio's demo at [claudius.devlato.com/studio](https://claudius.devlato.com/studio/?mock) plays claudius' own
sounds instead of a rough synth: every synth preset as claudius renders it, the TR-707 and TR-808 drum machines, in
stereo, with reverb, delay and the kick's ducking, each track as loud as claudius' mix makes its role, and the
master at the genre's loudness.

The studio records, in the app and in the browser: a take from a microphone or an audio interface, which plays
once from the section it starts with and goes with the song, and what a MIDI keyboard plays, as a part's notes. A sung, hummed or played line becomes notes for an instrument or several, as it is or as harmony,
octaves, chords, a bass line, an arpeggio or a pad. **Measure** plays clicks and listens for them, so that takes land
in time ([Takes](https://github.com/devlato/claudius/blob/main/docs/src/guide/samples.md#takes)).

A take waits when it stops: play it back in the song, record it again, or keep it. An assistant can ask you for one
(`request_take`): the studio's Record window opens for it, and once you send the take, the assistant, which has been
waiting for it (`wait_for_take`), carries on with it
([A take an assistant asks for](https://github.com/devlato/claudius/blob/main/docs/src/guide/samples.md#a-take-an-assistant-asks-for)).

Scripts reach every effect: a chain on any track, bus or the master, `parallel` chains side by side at their own
amounts, buses of your own, and settings that move through the song or by section, such as a filter that opens across
every build or a reverb that only the break drowns in. The printed song lists every chain
([Effect chains](https://github.com/devlato/claudius/blob/main/docs/src/language/mixing.md#effect-chains)).

Ten new genres: `trip_hop`, `big_beat`, `electro_house`, `nu_disco`, `filter_house`, `dance_pop`,
`neo_psychedelia`, `art_rock`, `idm` and `dub_techno`. Their profiles use the new effect chains themselves: parallel
compression and crushed copies on the drums, a dub delay that feeds back harder in the breaks, an acid filter that
opens across the builds, a disco loop that sweeps through a lowpass with the arrangement, guitars that turn distorted
in the climaxes, pads gated harder as a groove goes on. The cookbook has a recipe for each
([Trip-hop, big beat, electro house, nu-disco, filter house](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/breaks-and-disco.md);
[Dance-pop, neo-psychedelia, art rock, IDM, dub techno](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/pop-psych-and-glitch.md)).

Five more: `boom_bap`, 90s hip hop with a sampler's lazy swing and a piano loop; `uk_drill`, at half time with
sliding 808s and skippy hats in threes; `dubstep`, a wobble over a clean sub under a big snare on the third beat;
`uk_garage`, a shuffled two-step with organ and Rhodes chords; and `jungle`, a chopped, rolling breakbeat over a deep
sub, its chords lost in long delays. The studio's demo plays them too, and the cookbook has a recipe for each
([Boom bap, UK drill, dubstep, UK garage, jungle](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/hip-hop-and-uk-bass.md)).

And five more: `afrobeats`, a clave, congas and shakers under guitar plucks; `amapiano`, the log drum's woody bass
under a steady kick and jazzy piano; `reggaeton`, on the dembow; `trance`, a rolling bass and supersaws with a long
breakdown that builds back into the drop; and `neo_soul`, a laid-back live kit under 9th and 11th chords on Rhodes.
Each has a recipe in the cookbook too
([Afrobeats, amapiano, reggaeton, trance, neo-soul](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/afro-latin-trance-and-soul.md)).

Genres fuse: `trip_hop+idm`, `art_rock+idm` or `house+dnb+lofi` is one genre, in scripts, on the command line, for
assistants and in the studio's New song dialog (**Fused with**). The first genre leads the song, its harmony, parts
and mix, and the last plays the drums and lends its sounds
([Fusions](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/fusions.md)).

The auto-sampler samples a recording's vocals into a song: it finds the best sung phrases on the recording's own beat
grid, cleans them of its drums, fits them to the song's tempo and key, and places each where its notes agree with
the song's chords, mixed as a vocal (`claudius sample`, `sample_song`, and the studio's New song dialog;
[Sampling a song's vocals](https://github.com/devlato/claudius/blob/main/docs/src/guide/samples.md#sampling-a-songs-vocals)).

The studio has a Graph tab: the song as its signal flows, each track through its effects into its bus, the buses
and returns into the master, out, on a canvas to drag and zoom. Every node is editable beside it: a track's mix,
bus, sections and sound, an effect's settings or its place, a chain's new effects, the master's loudness. Assistants
read the same graph with the `graph` tool and change chains with `set_fx`.

Every drum step can do what a drum machine's trig does: play by chance, play only on some times round (every other
bar, the fourth, or the last bar before the next section), roll in a ratchet that swells or fades, flam, land early or
late of the grid, and change its pitch, decay, tone and pan for that stroke. Right-click a step in the step sequencer
and open **Trig**, or write `trig()` in a script; the step shows what it does
([The drum machine](https://github.com/devlato/claudius/blob/main/docs/src/cookbook/drum-machine.md)).

Each drum lane can keep its own time: a length of its own, so hats twelve steps long turn against a kick of sixteen,
a speed (triplets over a straight beat), a swing of its own, and a groove: a swing, a shuffle, a dragged `dilla`
feel, or the groove of a record. **Take its groove** under **New song** (or `extract_groove`, or `groove()` in a
script) measures where each sixteenth of a recording's drums lands and how hard, and puts the lanes in that feel.
Right-click a lane in the step sequencer for its time and feel, or write `lane()` in a script.

Beatbox a beat and it becomes the song's drums: right-click a take in the arrangement and choose **Turn it into
drums**. claudius names each hit a kick, a snare or a hat, folds the times round into a pattern of 1, 2 or 4 bars,
writes it into the drum lanes (a step hit only some of the times round plays with that chance) and mutes the take.
A drum loop goes in the same way, with `transcribe_drums` or `drums_from()`. The arrangement's right-click menu on a
row opens again: it had come up empty.

New songs sound far less alike. Every part draws its sound from 46 synth presets (26 new: FM, rubber, moog and
distorted basses; saw, PWM, brass, FM and vowel leads and a whistle; a string machine, glass, PWM, breathy and vowel
pads; a tine piano, a clav, a house piano, brass stabs, an overdriven organ, FM mallets and a harp pluck), each varied
so that no two songs play quite the same patch, and from every instrument of the installed sample packs that suits its
genre and role: strings, brass, woodwinds, mallets, pianos, electric pianos, guitars, organs, choirs and more. No two
parts of a song play the same sound, and a drum lane sometimes plays a kit of its own, so a song's drums come from
more than one machine. Packs without a licence or with share-alike terms are never drawn; choose them by hand. The
studio's Parts tab offers every installed instrument that can play a part. Chords, keys and pads on an instrument
recorded in mono get a light chorus, as a single-voice synth does, so they do not sit dead centre.

Nor do new songs share a shape or a mix. A genre's forms are templates: each song lengthens or shortens their
sections, and may add a bridge, a third verse and chorus, a lift into the first chorus or a last chorus played twice,
start or end cold, or thin its intro, its first verse or its outro. Its mix is its own too: the reverb's length,
darkness and pre-delay, the delay's time and bounce, every part's sends, a colour on some parts (a phaser that
deepens in the drops, tape on the keys, a tube on the bass), a high-pass on the music in the breaks, and the master's
saturation ([A form and a mix of its own](https://github.com/devlato/claudius/blob/main/docs/src/engineering/theory.md#a-form-and-a-mix-of-its-own)).

Their harmony bends the rules a little, as people's does. The progression library has 29 more progressions (86 in
all), among them borrowed chords (`I iii IV iv`, `I bVI bVII I`), secondary dominants, a tritone substitute and the
Neapolitan. A song may answer its loop, playing it again with another way back to its first chord (its dominant, a
borrowed chord, a suspension) and sometimes a chord coloured on the way, and its choruses may play a progression of
their own. Sevenths, ninths and suspensions keep to the key's notes: a dominant no longer gains a major seventh, and
the fifth of a minor key no longer a ninth the key does not have. The critic no longer counts a lead note the chord
under it has (a borrowed chord's third, a dominant's leading note) as outside the key
([The generator's harmony](https://github.com/devlato/claudius/blob/main/docs/src/learn/producers/harmony.md#the-generators-harmony)).

claudius hears how a voice is sung: the notes it means, how near each came, and how its singer sings (where they go
sharp or flat by register, how widely their notes scatter, how fast they settle, their vibrato). Right-click a take
and choose **How is it sung?**, run `claudius voice` on a file, or let an assistant call `assess_voice`
([Your voice](https://github.com/devlato/claudius/blob/main/docs/src/guide/voice.md)). Turning a take into notes reads
the same way, so a singer's flat high notes become the notes they meant.

A tuner moves a voice onto the notes it means, as far as its singer needs: their usual error goes, and of their scatter
only a few cents stay, as expression. It reads each note against the song's chords, brings it on as fast as asked
(slowly enough to keep the scoop, or at once for the hard-tuned sound) and keeps the voice's character. Right-click a
take and choose **Tune it**, add the `tune` effect in a script or the graph, run `claudius tune`, or let an assistant
call `tune`.

A pitch check plays six short phrases to sing back, in any octave, and learns from them how you sing, how much nearer
you land a step than a leap included; your takes are tuned as it heard you from then on. Press **Check my pitch** in the
Record window, or let an assistant ask with `request_pitch_check` and wait with `wait_for_pitch_check`
([Checking your pitch](https://github.com/devlato/claudius/blob/main/docs/src/guide/voice.md#checking-your-pitch)).

The studio adds and removes tracks. **+ Track** in the arrangement adds a part of any role (a second lead, keys on a
techno song) or a drum lane, with its name and its sound from a browser of the synths made for the role, the
installed instruments, and the drum voices and kits; right-click a row to remove a track. Scripts make a part of
any role with `part("lead2", #{ role: "lead" })`, and assistants use `add_track` and `remove_track`
([The studio](https://github.com/devlato/claudius/blob/main/docs/src/getting-started/studio.md#the-panels)).

The studio has a piano roll: a part's notes over a progression, as the theory engine writes them, to change note by
note (add, move, lengthen, take out); the first change writes the notes out, and **Back to the theory engine**
gives them back. Scripts write a part's notes with `notes("lead", "A", [[0, 0.5, 69], ...])` and give them back
with `clear_notes()`, assistants with `set_notes`, and a song printed as a script now keeps the notes a part has
written out, from a take or a keyboard too
([`notes()`](https://github.com/devlato/claudius/blob/main/docs/src/language/song-functions.md#notes)).

`claudius publish` checks a song the way stores and distributors check a release (its artwork's size, colours and
sharpness, its audio, the licences of its sounds, and its details, including how it was made) and writes a release
kit: the WAV and the cover a distributor takes, and the answers its form asks for
([Publishing](https://github.com/devlato/claudius/blob/main/docs/src/guide/publish.md)).

### Added

- studio: Connect an assistant from the app ([#25](https://github.com/devlato/claudius/pull/25))
- studio: A short tour the first time the studio opens ([#26](https://github.com/devlato/claudius/pull/26))
- Opt-in anonymous usage statistics ([#27](https://github.com/devlato/claudius/pull/27))
- macos: A disk image for each chip, and the app is called Claudius ([#29](https://github.com/devlato/claudius/pull/29))
- songs: Every release opens the songs earlier ones saved ([#30](https://github.com/devlato/claudius/pull/30))
- updates: Claudius downloads new releases and installs them on restart ([#31](https://github.com/devlato/claudius/pull/31))
- brand: A new logo, a C cut out of colourful ripples ([#32](https://github.com/devlato/claudius/pull/32))
- connect: Set claudius up with each assistant's own command ([#33](https://github.com/devlato/claudius/pull/33))
- assistants: A Claude Code plugin with the skills, and a Claude Desktop extension ([#37](https://github.com/devlato/claudius/pull/37))
- install: Download Claudius and drag it into Applications, no terminal needed ([#35](https://github.com/devlato/claudius/pull/35))
- packs: Eight more default packs and 21 more instruments: Sonic Pi's samples, a solo cello, an erhu, a steel pan, synth drums, body percussion, glitch kits and sound effects ([#45](https://github.com/devlato/claudius/pull/45))
- export: Songs export to WAV, AIFF, FLAC, Apple Lossless, MP3, AAC, Opus and Ogg Vorbis, and song.mp3 no longer needs ffmpeg ([#46](https://github.com/devlato/claudius/pull/46))
- studio: An export menu, and export over MCP and the HTTP API ([#47](https://github.com/devlato/claudius/pull/47))
- publish: Check a song for release, and write a release kit for a distributor ([#48](https://github.com/devlato/claudius/pull/48))
- studio: A Publish dialog, and publishing over MCP and the HTTP API ([#49](https://github.com/devlato/claudius/pull/49))
- studio: The demo plays claudius' own synth presets and TR-707 and TR-808 drums, mixed as claudius mixes ([#56](https://github.com/devlato/claudius/pull/56))
- takes: Recordings a song plays once from a section, sent to /api/takes or added with add_take ([#58](https://github.com/devlato/claudius/pull/58))
- studio: Record a take from a microphone or an audio interface ([#59](https://github.com/devlato/claudius/pull/59))
- takes: Turn a take's melody into notes that an instrument, or several, plays ([#60](https://github.com/devlato/claudius/pull/60))
- studio: Record what a MIDI keyboard plays, and write notes into a song with add_notes ([#61](https://github.com/devlato/claudius/pull/61))
- takes: A sung or played line becomes harmony, octaves, chords, a bass line, an arp or a pad ([#62](https://github.com/devlato/claudius/pull/62))
- studio: Measure how late takes land, and set the latency from it ([#63](https://github.com/devlato/claudius/pull/63))
- studio: Record takes in the browser studio ([#64](https://github.com/devlato/claudius/pull/64))
- studio: Record what a MIDI keyboard plays in the browser studio ([#65](https://github.com/devlato/claudius/pull/65))
- studio: Choose files on this computer instead of typing paths ([#67](https://github.com/devlato/claudius/pull/67))
- studio: Record the takes an assistant asks for, and play a take back before keeping it ([#68](https://github.com/devlato/claudius/pull/68))
- script: Set any effect chain, run chains in parallel, and move settings by section ([#69](https://github.com/devlato/claudius/pull/69))
- genres: Trip-hop, big beat, electro house, nu-disco and filter house ([#70](https://github.com/devlato/claudius/pull/70))
- genres: Dance-pop, neo-psychedelia, art rock, IDM and dub techno ([#71](https://github.com/devlato/claudius/pull/71))
- genres: Fuse two or three genres into one ([#72](https://github.com/devlato/claudius/pull/72))
- sampler: Sample a recording's vocals into a song ([#74](https://github.com/devlato/claudius/pull/74))
- studio: A graph of the song's signal, editable node by node ([#75](https://github.com/devlato/claudius/pull/75))
- voice: Hear the notes a singer means, and how they sing ([#76](https://github.com/devlato/claudius/pull/76))
- fx: Tune, which moves a voice onto the notes it means ([#77](https://github.com/devlato/claudius/pull/77))
- voice: A pitch check that plays phrases to sing back and learns the voice from them ([#78](https://github.com/devlato/claudius/pull/78))
- drums: Trigs, so a step plays by chance and condition, ratchets, flams, nudges and locks its sound ([#79](https://github.com/devlato/claudius/pull/79))
- drums: Lanes with their own length, speed, swing and groove, and grooves heard in recordings ([#80](https://github.com/devlato/claudius/pull/80))
- drums: Turn a beat, played or beatboxed, into the drum lanes ([#81](https://github.com/devlato/claudius/pull/81))
- compose: Songs draw their sounds from every pack and 46 synth presets ([#82](https://github.com/devlato/claudius/pull/82))
- compose: Every song plays a form and a mix of its own ([#83](https://github.com/devlato/claudius/pull/83))
- theory: Harmony that bends the rules: answered loops, choruses of their own, 29 more progressions ([#85](https://github.com/devlato/claudius/pull/85))
- studio: Add and remove tracks, with a sound browser ([#86](https://github.com/devlato/claudius/pull/86))
- studio: A piano roll, and notes() to write a part's notes ([#87](https://github.com/devlato/claudius/pull/87))
- genres: Boom bap, UK drill, dubstep, UK garage and jungle ([#88](https://github.com/devlato/claudius/pull/88))
- genres: Afrobeats, amapiano, reggaeton, trance and neo-soul ([#90](https://github.com/devlato/claudius/pull/90))

### Changed

- server: Compress the web studio, a third of its size, and cache its hashed files ([#39](https://github.com/devlato/claudius/pull/39))
- render: Saturation and the master stage stream their oversampling, the same samples faster ([#40](https://github.com/devlato/claudius/pull/40))

### Fixed

- install: Install claudius.app on a Mac, which opens without Gatekeeper's warning ([#28](https://github.com/devlato/claudius/pull/28))
- server: A session's renders go when it ends, and the tests keep theirs in target/ ([#41](https://github.com/devlato/claudius/pull/41))
- docs: Readable type, at the sizes the theme meant, and the new logo in the menu bar ([#43](https://github.com/devlato/claudius/pull/43))
- packs: Every FLAC conversion is read back, and installing again repairs the damaged ones ([#44](https://github.com/devlato/claudius/pull/44))
- deps: Update rustls-webpki, webpki-roots and webpki-root-certs for RUSTSEC-2026-0381, 0382 and 0383 ([#52](https://github.com/devlato/claudius/pull/52))
- deps: Update zip to 9.0.5 for GHSA-m665-v7pm-7grj ([#55](https://github.com/devlato/claudius/pull/55))
- studio: Say claudius, never the server ([#66](https://github.com/devlato/claudius/pull/66))
- compose: Chords, keys and pads on mono recordings get the light chorus ([#84](https://github.com/devlato/claudius/pull/84))
- model: Songs open from blueprint.json with every time exactly as saved ([#89](https://github.com/devlato/claudius/pull/89))

## [0.2.0] - 2026-10-19

### Added

- studio: Download the sample packs from the studio ([#17](https://github.com/devlato/claudius/pull/17))
- macos: claudius.app in a disk image, one app for Apple silicon and Intel Macs ([#13](https://github.com/devlato/claudius/pull/13))
- windows: An installer, with the studio in the Start menu ([#14](https://github.com/devlato/claudius/pull/14))
- linux: .deb, .rpm and AppImage packages, with the studio in the app menu ([#15](https://github.com/devlato/claudius/pull/15))

### Changed

- critic: Read realness over eight windows, and encode the MP3 while the critic listens ([#18](https://github.com/devlato/claudius/pull/18))

### Fixed

- packs: Setup installs a pack whose WAV files have a long fmt chunk, and gets the ears when a pack fails ([#19](https://github.com/devlato/claudius/pull/19))
- mcp: Wait for the ears to load before exiting, instead of crashing ([#20](https://github.com/devlato/claudius/pull/20))
- remake: Play a remake at a tempo its genre plays ([#21](https://github.com/devlato/claudius/pull/21))
- What a newcomer misread: download sizes, save messages, ears loading, MCP schemas ([#22](https://github.com/devlato/claudius/pull/22))
- critic: Advice a person can act on, instead of z-scores and near ties ([#23](https://github.com/devlato/claudius/pull/23))

## [0.1.0] - 2026-10-18

The first public release.

### Added

- A beatmaker that composes, arranges, mixes and masters songs in eight genres (house, deep house, techno, trap,
  lofi, drum and bass, synthwave, future bass), using real audio only: synthesized drums and synths, 162 sampled
  instruments from 14 freely licensed sample packs, Apple Loops when GarageBand is installed, and your own files.
- A critic that judges every render: loudness and tonal balance, the energy arc of the arrangement, music theory
  checks, and earshot's CLAP model for genre fit and production quality. Every issue names what to change.
- Mutation and evolution: songs change the way a producer would change them, and the critic keeps the better ones.
- A script language: Rhai with Strudel-style mini-notation patterns. Every song prints as a script that runs back
  into the same song.
- The studio, a native and web app (egui) with arrangement, step sequencer, mixer, harmony, script and verdict
  panels.
- MCP tools, over stdio and HTTP, so Claude, ChatGPT, Cursor, Codex and other assistants can make and edit songs
  while you listen, and skills that teach them the craft: making and editing songs, remixing recordings, and setting
  claudius up. `claudius connect` installs both.
- Analysis and remakes of any song: tempo, key, chords, sections and groove, rebuilt in any genre.
- `claudius setup`, `doctor` and `connect` for installing the sounds and models, checking an installation, and
  configuring assistants.
- Builds for macOS on Apple silicon and Intel (Intel Macs have no ears: their verdicts come from the meters and the
  theory checks), Linux on x86_64 and ARM64 with a headless build for machines without sound or a display, and
  Windows as a preview. One-line installers for each, which come with every release and are in its checksums.

[unreleased]: https://github.com/devlato/claudius/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/devlato/claudius/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/devlato/claudius/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/devlato/claudius/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/devlato/claudius/releases/tag/v0.1.0
