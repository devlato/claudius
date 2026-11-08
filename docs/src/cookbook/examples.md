# Example songs, the 1990s to the 2020s

claudius comes with example songs, each in the style of the hits of one scene of a decade: Bristol trip hop, big
beat, boom bap, G-funk and piano house; French house, electropop, UK garage, trance and crunk; festival EDM, future
bass, dubstep, tropical house and trap; the 80s revival in synth-pop and the disco revival. Each is a
script whose opening notes say how its style is made: the tempo and the groove, the chords, the sounds, the
arrangement and the mix. Opening one is a way to hear how a style is built, to read how, and to start a song from it.

None of them copies a record. A style is named for the artists it is heard on, so you know what to expect, but
every melody, bass line, chord voicing and drum pattern is the example's own: a hit's melody and words belong to
the people who wrote it, and claudius plays only what it is free to. The examples play claudius' own synths and
drum voices, so they sound the same without the sample packs, offline and in the
[online demo](https://claudius.devlato.com/studio/?mock).

## Opening one

- **In the studio:** **New song**, then **From an example**. Choose a card (hover it for its notes) and **Open it**.
  It replaces the song that is open, which undo brings back, and the **Script** tab shows the script with its notes.
- **With an assistant:** `examples` lists them and, given a name, says how that one is made; `open_example` opens one
  in the studio, rendered and judged. Ask for "the 90s trip hop example" or "show me how French house is made".
- **On the command line:** `claudius examples` lists them, and `claudius examples NAME` prints one's script:

```sh
claudius examples blue_hour_static > blue_hour_static.rhai
claudius script blue_hour_static.rhai --out ~/Music/claudius
```

The scripts are in the repository too, under
[`crates/claudius-api/songs`](https://github.com/devlato/claudius/tree/main/crates/claudius-api/songs).

## The songs

| decade | song | style | heard on | listen for |
|---|---|---|---|---|
| 1990s | Blue Hour Static (`blue_hour_static`) | Bristol trip hop | Massive Attack, Portishead, Tricky | a slow, swung beat through tape and vinyl, a sub bass holding each root, wide dark strings, a vibraphone line with long echoes |
| 1990s | Acid Rocket (`acid_rocket`) | big beat | The Prodigy, The Chemical Brothers, Fatboy Slim | a sped-up break crushed in parallel, an acid bass line that is the hook, two chords only |
| 1990s | Concrete Diaries (`concrete_diaries`) | boom bap | Nas, Pete Rock, A Tribe Called Quest | a snare 14 ms late and humanized hits, a kick that changes over four bars, a jazz loop on a taped clavinet, horns through a phaser in the hook |
| 1990s | Sunday Lowrider (`sunday_lowrider`) | G-funk | Dr. Dre, Snoop Dogg, Warren G | a dorian funk vamp, a rubbery bass sliding between roots and fifths, a high whining lead gliding far above it all |
| 1990s | Diamond Floor (`diamond_floor`) | piano house | Robin S, Black Box, CeCe Peniston | house piano chords hammered on the offbeats, an organ bass between the kicks, a diva's chopped phrases as the hook |
| 2000s | Chrome Boulevard (`chrome_boulevard`) | French filter house | Daft Punk, Stardust, Modjo | a two-bar disco loop whose lowpass filter is the arrangement, deep pumping under the kick, a vocoder-like lead |
| 2000s | Flashbulb (`flashbulb`) | electropop | Lady Gaga, Britney Spears, Kesha | chords that rock back and forth instead of resolving, a buzzing bass on eighths, offbeat stabs, a hook doubled an octave up |
| 2000s | Sundown Shuffle (`sundown_shuffle`) | UK garage | Craig David, Artful Dodger, MJ Cole | the two-step kick that skips beat 3, a heavy shuffle, organ stabs of minor ninths, an organ bass bouncing off the kick |
| 2000s | Afterlight Horizon (`afterlight_horizon`) | uplifting trance | Tiësto, ATB, Paul van Dyk | the rolling bass after every kick, a plucked arpeggio climbing the chords, a breakdown with the anthem alone, a supersaw drop |
| 2000s | Floor Shaker (`floor_shaker`) | crunk | Lil Jon, Ying Yang Twins, Three 6 Mafia | a booming 808, claps loud enough to shout over, a harmonic minor riff a half step from the root, brass stabs |
| 2010s | Higher Than the Lights (`higher_than_the_lights`) | festival EDM | Avicii, Swedish House Mafia, Martin Garrix | vi IV I V on a piano in the breakdown, a snare roll and a beat of silence before the drop, a supersaw hook and a mix that pumps |
| 2010s | Coral Skies (`coral_skies`) | future bass | Flume, The Chainsmokers, Illenium | half time at 150 BPM, a held supersaw that swells back after every kick, triplet hat rolls, vocal chops |
| 2010s | Fracture Line (`fracture_line`) | dubstep | Skrillex, Rusko, Flux Pavilion | half time at 140 BPM, a wobble bass as the lead doubled by a distorted one, a phrygian half step, a drop of drums and bass alone |
| 2010s | Island Tide (`island_tide`) | tropical house | Kygo, Thomas Jack, Felix Jaehn | house slowed to 112 BPM, snaps and shakers, bright major chords, a marimba-like mallet hook |
| 2010s | Ice Avenue (`ice_avenue`) | trap | Metro Boomin, Migos, 21 Savage | hats rolling in triplets and thirty-seconds, an 808 that is kick and bass, a dark harmonic minor string line |
| 2020s | Midnight Overdrive (`midnight_overdrive`) | synth-pop, the 80s revival | The Weeknd, Dua Lipa, M83 | 168 BPM straight, a gated-reverb snare, an octave bass and a sixteenth arpeggio under a saw lead |
| 2020s | Satin Signal (`satin_signal`) | disco-pop | Dua Lipa, Doja Cat, The Weeknd | an open hat on every offbeat, an octave disco bass, a guitar-like chop and congas, strings that stab only in the choruses |

Each script starts from `blank()` in its genre, so it brings the genre's mix (its buses, master chain and loudness)
and writes everything else itself. To make one your own, change a line and run it again: the tempo, the
progression, a pattern, a sound. [Starting a song](../language/song-functions.md#starting-a-song) and the
[quick reference](../language/quick-reference.md) say what each function does.
