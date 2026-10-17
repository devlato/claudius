# Mixing and mastering

A claudius mix starts where an engineer's first pass ends: every track at a sensible level for what it is, the kick
ducking what should breathe, everything on a bus with glue, and a master landing on a loudness target under a
true-peak ceiling. Your work is the second pass. This page explains the signal flow, the genre chains, and how to act
on the critic's mix report. The functions are in [Mixing functions](../../language/mixing.md).

## Signal flow

For every track, in this order:

1. the sound plays its notes, with swing and humanize;
2. its inserts run: the genre's (a chorus on some pads, tape on lofi keys) or the pattern's (`.hpf() .lpf() .crush()
   .distort()`);
3. it is measured while it plays and gained to *role target + genre offset + level* (limited to −40..+30 dB);
4. it is ducked by the kick by `duck`;
5. it is panned;
6. it feeds its bus, and the reverb and delay returns by its sends.

Then each bus runs its chain, the buses and the returns (ducked 30 % by the kick) are summed, the master chain runs,
and the loudness stage brings the result to the target.

## Starting values

Roles first: a track's role sets its level, and the critic's theory checks read roles too (see
[Roles](../../language/mixing.md#roles)). Then these are reasonable starting points; the genre profiles use similar
values for generated parts:

| track | level | duck | sends |
|---|---|---|---|
| bass | 0 | 0.45–0.65 (house, techno), 0 (trap 808 with its own kick pattern) | none |
| chords, stabs | 0 to −2 | 0.35–0.45; 0.85 for a future-bass pump | reverb 0.2–0.35, delay 0.05–0.3 |
| pad | −2 to −4 | 0.4–0.5 | reverb 0.35–0.45 |
| lead | 0 | 0.15–0.3 | reverb 0.25–0.35, delay 0.2–0.25 |
| arp | −2 | 0.3–0.4 | delay 0.25–0.3, pan ±0.15–0.2 |
| hats, percussion | 0 | 0 | pan ±0.1–0.35, a little delay on percussion |
| clap, snare | 0 | 0 | reverb 0.15–0.3 |

## Bus chains by genre

Besides what every song gets (a tanh saturator on the drums bus unless the genre already saturates there, the bass
bus mono below 140 Hz, the music bus at width 1.25 and mono below 160 Hz), each genre has its own chains:

| genre | drums | bass | music |
|---|---|---|---|
| house | compressor −12 dB, 2:1, 12/120 ms | | highpass 110 Hz |
| deep house | compressor −14 dB, 1.8:1 | | highpass 100 Hz, high shelf −1.5 dB at 9 kHz |
| techno | compressor −12 dB, 2.5:1, 10/100 ms; tanh saturation | | highpass 130 Hz |
| trap | compressor −10 dB, 2:1, 8/90 ms | tanh saturation 3 dB | highpass 140 Hz |
| lofi | bitcrusher (11 bits, half rate, 35 %), lowpass 8.5 kHz, compressor −14 dB, 2.5:1 | | highpass 90 Hz |
| drum and bass | compressor −11 dB, 2.5:1, 8/90 ms | tube saturation 4 dB | highpass 140 Hz |
| future bass | compressor −11 dB, 2.2:1 | | highpass 130 Hz |
| synthwave | compressor −12 dB, 2:1 | | highpass 110 Hz |

The music bus highpass is why pads and chords never fight the bass: anything on the `music` bus below 90–140 Hz is
gone. Give a part the `bass` role (and so the bass bus) only if it is the bass.

## The master

| genre | master chain | target |
|---|---|---|
| house | highpass 25 Hz, high shelf +1 dB at 10 kHz; compressor −10 dB, 1.6:1; tape saturation | −10 LUFS |
| deep house | highpass 25 Hz; compressor −11 dB, 1.5:1; tape saturation | −11 |
| techno | highpass 28 Hz; compressor −10 dB, 2:1; tanh saturation 3 dB | −9 |
| trap | highpass 24 Hz; compressor −9 dB, 1.8:1; tanh saturation | −9 |
| lofi | highpass 30 Hz, high shelf −2.5 dB at 8 kHz; tape (wow, flutter, lowpass 11 kHz); vinyl crackle and hiss; compressor −14 dB | −12.5 |
| drum and bass | highpass 26 Hz; compressor −9 dB, 2:1; tanh saturation | −8.5 |
| future bass | highpass 26 Hz, high shelf +1.5 dB at 10 kHz; compressor −9 dB, 1.8:1; tanh saturation | −9 |
| synthwave | highpass 25 Hz; compressor −11 dB, 1.6:1; tape saturation | −10.5 |

After the chain, the loudness stage searches for the input gain that lands the song on the target (within 0.15 LU),
through a soft clipper 0.1 dB under the ceiling and a limiter 0.5 dB under it, then trims if the true peak is still
over. [`master()`](../../language/mixing.md#master) changes the target (−24 to −5 LUFS) and the ceiling (−3 to −0.1
dBTP, default −1).

## Reading the mix report

The critic measures the master and reports each problem as an issue with its numbers. The mix score starts at 1 and
loses a little for each:

| check | healthy | issue text begins |
|---|---|---|
| loudness | within 1 LU of the target | `loudness -11.6 LUFS, target -10.0` |
| true peak | at most 0.1 dB over the ceiling | `true peak -0.6 dBTP over the -1.0 ceiling` |
| sub, 20–60 Hz | −14 to −4 dB of the total | `too much sub (boomy)` / `not enough sub` |
| low, 60–250 Hz | −7 to −1 dB | `the low end is too heavy` / `a thin low end` |
| low mids, 250 Hz–1 kHz | −12 to −4 dB | `muddy 250 Hz-1 kHz` / `hollow low mids` |
| mids, 1–4 kHz | −18 to −8 dB | `boxy or harsh mids` / `recessed mids` |
| presence, 4–8 kHz | −26 to −15 dB | `harsh 4-8 kHz` / `dull 4-8 kHz` |
| air, 8–16 kHz | −30 to −17 dB | `fizzy top end` / `no air above 8 kHz` |
| stereo | correlation 0.15 to 0.96 | `nearly mono` / `too wide / phasey` |
| low end | correlation at least 0.9 below 150 Hz | `low end not mono` |

The band values are each band's share of the master's total energy, in dB, so they move together: cutting the sub
raises everything else's share. What usually fixes each:

- **too much sub, thin low end**: the bass sits too low or is too pure. Lower its level, move it up an octave, or use
  a bass with harmonics (`saw_bass`, `organ_bass`, `reese` instead of `sub_bass`); a long 808 kick adds sub too.
- **muddy low mids**: pads and chords voiced low. Raise their `.voicing()` range or lower their level.
- **recessed mids**: the lead and chords are buried; raise them, or choose brighter sounds (piano over a warm pad).
- **dull presence, no air**: hats, claps, shakers and rides too quiet or too dark. Raise their levels, add an open hat
  or a ride. Lofi's master darkens the top end on purpose, so it needs louder hats than other genres.
- **fizzy top end**: hats and bright synths (supersaws) too loud; lower the hats or `.lpf()` the synth.
- **nearly mono**: everything important sits in the middle. Pan the hats and percussion, raise the reverb and delay
  sends of chords and pads (the returns are wide), prefer sampled stereo instruments for pads.
- **low end not mono**: a stereo recording below 150 Hz on the drums bus, usually the acoustic kit's kick (the drums
  bus is not made mono). Use a 909 or 808 kick.

The ears add a production score: how strongly the master reads as "a professionally mixed and mastered music track"
against six named flaws (a cheap synthesized beep, a low quality recording with hiss, a distorted clipped recording,
a computer-generated electronic tone, a low bitrate mp3 with artifacts, a recording made on a phone). It moves with
sound choice more than with mix settings: sampled instruments and real drum machines help most, and heavy
distortion or a buzzy synth lead pull it towards the flaws.

## Stems

`claudius make --stems` and `claudius render blueprint.json --stems` write every track as its own WAV into
`stems/`, after its level, sidechain and pan and before the buses, so the stems sum to the mix without the bus
chains, the returns and the master.
