# Reading the verdict and acting on it

The verdict comes back after every render: a total out of 100, the component scores, the measures, the issues sorted
worst first, what the listening models hear, and a summary whose "Fix first" names the three costliest issues. On the
command line it is `verdict.json` in the song's folder, and `claudius judge song.wav` judges any file.

## The components

| score | listens to | weight |
|---|---|---|
| mix | loudness, true peak, the balance of six frequency bands, stereo width, a mono low end | 0.30 |
| arc | whether loudness follows the arrangement, and the drop against the break | 0.20 |
| theory | the lead and bass against the key and chords, voice leading, the form | 0.20 |
| genre | whether it sounds like its genre more than the genres it is confused with (needs the models) | 0.15 |
| production | whether it sounds professionally produced more than like a typical flaw (needs the models) | 0.15 |

Without the listening models (`--no-ears`, Intel Macs) the total is out of the first three.

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

### The arrangement (`form`)

- `the drop is only 1.8 LU louder than the breakdown (aim for 3-7)`: thin the break (fewer layers:
  `section("break", #{ layers: "pad chords" })`) or fill the drop (`layer("drop", "lead", true)`, `energy: 1.0`).
- `loudness does not follow the arrangement's energy`: section energies that disagree with what plays in them. Give
  intros, breaks and outros fewer tracks, and drops the most.
- `45 s long (most tracks run 2-5 minutes)`: add sections or lengthen them in `form()`.
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

### What the models hear (`genre`, `production`)

- `sounds more like 'techno music' than 'house music'`, or `sounds only faintly like 'house music'`: move toward the
  genre's own sounds and habits ([genres.md](genres.md)): for house, softer drums, swing, 7th chords on electric
  piano. Treat it as a hint; neighbouring genres often sit close, and the person's taste decides.
- `sounds about as much like a hissy recording as a professional mix: ...` (or `more like`): the issue ends with the
  flaw's usual cause. Fix the band and width issues first, since most production readings follow from an unbalanced
  spectrum or too much reverb, and try sampled instruments in place of synth patches, which read as more
  professional. "About as much" is a near tie and a faint reading; don't chase it. Lofi is not marked down for hiss
  or a phone-like sound, which are its style.

## When to stop

High 80s means the critic finds nothing big. It cannot tell whether the melody is catchy or the drop exciting, so
ask the person. Stop when they're happy, and don't push a change they dislike because it scores better.
