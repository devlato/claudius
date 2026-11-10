# Reading the verdict and acting on it

The verdict comes back after every render: a total out of 100, the component scores, the measures, the issues sorted
worst first, what the listening models hear, and a summary whose "Fix first" names the three costliest issues. On the
command line it is `verdict.json` in the song's folder, and `claudius judge song.wav` judges any file.

## The components

| score | listens to | weight |
|---|---|---|
| mix | loudness, true peak, the balance of six frequency bands, stereo width, a mono low end, each track's level against its role's | 0.30 |
| arc | whether loudness follows the arrangement, and the drop against the break | 0.20 |
| theory | the lead and bass against the key and chords, voice leading, the form | 0.20 |
| genre | whether it sounds like its genre more than the genres it is confused with, and more than stock background music (needs the models) | 0.15 |
| production | whether it sounds professionally produced more than like a typical flaw (needs the models) | 0.15 |
| aesthetics | how much listeners would enjoy it and how well it is produced, as Audiobox Aesthetics, a model of trained listeners' ratings, hears three of its ten-second windows (needs the models) | 0.15 |

Without the listening models (`--no-ears`, Intel Macs, the web demo) the total is out of the first three. A recording
out of the song's time cuts it ([below](#a-recording-out-of-time-timetrack)), and so does music buried in noise, with
the models or without them ([`noisy`](#what-the-models-hear-genre-production)).

## Every issue, and a change that answers it

Change one thing, render, and read the new verdict: a fix usually moves the total by a point or less, and the issue
going away is the real sign it worked. The examples use the track names of a generated song; `get_song` shows the
names in the song you are editing.

### Loudness (`master`)

- `loudness -12.4 LUFS, target -9.0` or `true peak -0.2 dBTP over the -1.0 ceiling`: the master normally lands on
  its target, so a miss means the mix fights it, most often a bass or kick far louder than the rest. Bring the loud
  part down before touching `master(#{ lufs: ..., ceiling: ... })`.

### Frequency bands (`band:<band>+` too much, `band:<band>-` too little)

The bands are sub (20-60 Hz), low (60-250), lowmid (250 Hz-1 kHz), mid (1-4 kHz), presence (4-8 kHz) and air
(8-16 kHz). A note's fundamental decides which band it feeds: only notes below about B1 (MIDI 35, 62 Hz) reach the
sub, so a deep sound playing in octave 2 still leaves it empty.

| issue | change |
|---|---|
| `not enough sub: the kick and bass lack weight` | move the bass down an octave: `part("bass", #{ register: [28, 43] })` for a generated bass, `.octave(1)` on a pattern bass (`chord("prog").root().octave(1)`); a new sound alone rarely helps |
| `too much sub (boomy)` | `mix("bass", #{ level: -2 })`, the bass's register up a few notes, or `.hpf(40)` on a pattern bass |
| `the low end is too heavy (kick + bass)` | the bass or kick 1 to 2 dB down |
| `a thin low end: bass or kick too quiet` | the bass up, or a fuller sound: `part("bass", #{ preset: "sub_bass" })` |
| `muddy 250 Hz-1 kHz: too much pad / chord body` | `.hpf(200)` on pads and chords, less reverb, a pad 2 to 3 dB down |
| `hollow low mids: chords and pads are missing` | chords or a pad up, or a chord part in the sections that sound empty |
| `boxy or harsh mids` | the lead or chords down, or a darker sound (`.lpf(3000)`) |
| `recessed mids: the lead and chords are buried` | the lead or chords up, the pad down |
| `harsh 4-8 kHz` | hats, clap and snare down |
| `dull 4-8 kHz: hats, claps and snare lack bite` | hats and clap 2 dB up: `mix("hat", #{ level: 2 })` |
| `fizzy top end` | hats and cymbals down, `.lpf(8000)` on a bright synth |
| `no air above 8 kHz: hats and cymbals too quiet or too dark` | hats up, or a brighter hat sound |

The healthy ranges are the same for every genre. A lofi song is meant to sound dusty: its genre lowpasses the drums
and keys and darkens the master, so `no air` and `dull 4-8 kHz` on lofi are usually its own sound. Turning the hats up
moves them a little; leave them when that dusty sound is what the person asked for, and say so.

### Stereo (`width+`, `width-`)

- `nearly mono`: pan the hats, percussion and second parts apart (`mix("perc", #{ pan: -0.3 })`), and give pads and
  chords reverb or delay.
- `too wide / phasey`: bring pans back toward the middle and lower the effect sends.
- `low end not mono`: keep the kick and bass in the centre (`pan: 0`), and take wide effects off them.

### A track's level (`level:<track>+` too loud, `level:<track>-` too quiet)

- `chords is 9.0 dB louder than the chords level: it covers the rest`: the mixer sets every track to the level its
  genre makes it at (most at `level: 0`, a few under it, such as trap's clap at −4), and a track's `level` moves it
  from there. Bring it back within 3 dB: `get_song` shows each track's level, so undo the change that moved it. The
  master's meters can't hear one track drowning the others, so this issue may be the only sign of it.

### The arrangement (`form`, `form:<what>`)

The form is judged against the genre's own forms, loosely, and on what a listener hears of it: how long it runs, how
soon it gets going, whether its sections can be told apart, and whether its hook comes back. The order of the sections
is not judged.

- `the drop is only 1.8 LU louder than the breakdown (aim for 3-7)`: thin the break (fewer layers:
  `section("break", #{ layers: "pad chords" })`) or fill the drop (`layer("drop", "lead", true)`, `energy: 1.0`).
- `loudness does not follow the arrangement's energy`: section energies that disagree with what plays in them. Give
  intros, breaks and outros fewer tracks, and drops the most.
- `2:00 long, where trance's forms run 3:26-3:35` (`form:short`, or `form:long` for a long one): far shorter or
  longer than the genre's songs run. Play a verse and hook again, or in a drop genre a break, a build and a second
  drop (`add_section`, or `form()`), or lengthen sections (`section("drop", #{ bars: 32 })`); to shorten, leave a
  repeat out (`remove_section`) or shorten the intro and outro. A short song the person asked for stays short.
- `the song takes 1:09 (32 bars) to get under way, where ambient's intros run 8 bars at most` (`form:intro`):
  shorten the intro (`section("intro", #{ bars: 8 })`) or start on the first verse.
- `drop and drop2 play alike, 64 bars heard as one section where trance's longest is 32` (`form:same`): the two play
  the same tracks, drums and chords, so nothing marks where one ends. Change the second: a part out or in
  (`layer("drop2", "lead", false)`), another drum variant (`section("drop2", #{ variant: "alt" })`) or the other
  progression (`prog: "B"`).
- `drop plays 64 bars, where trance's longest section is 32` (`form:section`): shorten it, or split it in two with
  something changing in the second.
- `the hook comes once, where dance pop's forms bring it 3 times` (`form:peaks-`): bring the hook back later, after
  another verse. `the song never drops, where trance's forms do`: give it a build and a drop
  (`sec("drop", 32, "*").impact()`). `the hook comes 7 times` (`form:peaks+`): leave a repeat out. A song of verses
  alone is fine in a hook genre.
- `a section is not a whole number of 4-bar phrases`: make every section 4, 8, 16 or 32 bars.

### The notes (`part:lead`, `part:bass`, `part:<name>`)

- `30% of the lead is outside A minor`: write the melody on scale degrees (`n("0 2 4").scale("A:minor")`), or give
  a generated lead a new seed: `part("lead", #{ seed: 11 })`.
- `only 40% of the lead's strong-beat notes are chord tones`: put chord tones on beats 1 and 3, or reseed the part.
- `the lead spans 26 semitones` or `70% of the lead's moves are leaps over a fifth`: keep the melody within about
  an octave and move mostly by step; a generated lead takes a narrower `register`.
- `the bass plays the chord root on only 40% of chord changes`: `chord("prog").root()` for a pattern bass, or a
  bass style that lands on the root (`part("bass", #{ style: "root8" })`).
- `chords: voices jump 7.5 semitones per chord change`: `.voicing()` on a pattern, which leads voices smoothly.

### A part's sound (`sound:<part>`)

- `chords plays synth patch 'whistle', which house has no place for`: give the part one of the sounds its genre
  plays (`get_song` lists the presets each part offers), such as `part("chords", #{ preset: "epiano" })` in house.
  When the person asked for that sound, keep it and say why the critic counts it against the song.

### What the models hear (`genre`, `production`)

- `sounds more like 'techno music' than 'house music'`, or `sounds only faintly like 'house music'`: move toward the
  genre's own sounds and habits ([genres.md](genres.md)): for house, softer drums, swing, 7th chords on electric
  piano. Treat it as a hint; neighbouring genres often sit close, and the person's taste decides.
- `sounds more like 'inspirational corporate background music' than ...` (or another stock reading: corporate pop,
  children's piano, ukulele, chiptune): the song is too clean and polite for its genre. This one is not a matter of
  taste: dirty the sounds (tape, saturation, a crushed copy of the drums), darken the harmony, swap bright synth
  presets for recorded instruments, and take out what is there only to be pleasant.
- `sounds about as much like a hissy recording as a professional mix: ...` (or `more like`): the issue ends with the
  flaw's usual cause. Fix the band and width issues first, since most production readings follow from an unbalanced
  spectrum or too much reverb, and try sampled instruments in place of synth patches, which read as more
  professional. "About as much" is a near tie and a faint reading; don't chase it. Lofi is not marked down for hiss
  or a phone-like sound, which are its style.
- `the listening model expects listeners to enjoy it 5.3 of 10, where records they like rate about 7.5: ...`
  (`genre`): the material itself, more than the mix. A clearer hook (a lead with a motif that comes back), a stronger
  groove (the genre's own drums and swing) and the genre's own sounds lift it most; it rates ten-second windows, so a
  song that takes a long time to get going rates low at its start.
- `the listening model rates its production 6.6 of 10, where professional records rate about 8.3: ...`
  (`production`): fix the band, width and level issues first, then try recorded instruments for the synth patches
  and less drive. A genre whose sound is a dusty or distorted recording (lofi, boom bap, trip hop, gabber, hyperpop
  and others) is not held to it. The `aes_*` measures show all four of the model's readings; usefulness and
  complexity are not scored.
- `noisy: the notes are buried in noise, as noise layers, bitcrushing or heavy clipping bury them (the music's body
  0.35 flat, where clean songs read under 0.22): ...` (`production`): measured on the master without the models, so
  it holds in the web demo too, and it cuts the total to as little as a quarter of itself. Find what fills the
  spectrum: a `bitcrush` or a hard `saturate` on the master or a bus (`get_song` lists the chains), heavy `distort` or
  `crush` on the parts that carry the chords and the bass, a noise layer or a `vinyl` hiss turned up. Take those off or
  well down, one at a time, and render; `body_flatness` in the measures falls as the notes come back out.

### A recording out of time (`time:<track>`)

The recordings on a song's tracks (a remake's `original`, its `sample`, `sample2`, `flip` and `break`, a loop, a take)
are judged on whether they keep the song's time, as their onsets say. Voices are not: they keep the time the rest of
their recording keeps. The verdict has a `time` score when one is judged, and a recording clearly out of time cuts the
total to as little as 35 % of itself, whatever else is right, since it sounds wrong at once.

- `'original' keeps a tempo of its own, about 126 BPM, against the song's 130: the recording on it is off the song's
  time`: the recording's tempo was misread when it was analysed (`analyze` shows what claudius heard in it). Tell the
  person first: it is the fault they would hear before any other. Mute the track, or turn it well down, where the
  song's drums play against it; a flip (`flip: true`) cuts short pieces of it, which stray less than all of it does.
- `'break' drifts off the song's beat, by up to 60 ms`: at the song's tempo, but wandering off it, as a band that
  speeds up and slows down does where its beats were not followed. Mute the track where it wanders, or remake with
  another seed or genre; a loop that drifts within itself was cut from bars that were misread.

## When to stop

High 80s means the critic finds nothing big. It cannot tell whether the melody is catchy or the drop exciting, so
ask the person. Stop when they're happy, and don't push a change they dislike because it scores better.
