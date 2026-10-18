# Listening, judging, finishing

Your song has every layer, a structure and a mix. This page is about the part producers spend most of their time on:
listening critically, finding what is wrong, and fixing it until the song is done.

## Render the whole song

Save the song from the [mixing page](mixing.md) as `song.rhai` and render it:

```sh
claudius script song.rhai
```

This is what claudius printed for it:

```text
track kick: bd
track clap: cp
track hats: hh
track shaker: sh
track roll: sd
track bass: sub_bass
track chords: epiano
track pad: pad
track melody: pluck
ears loaded in 3.6 s
First Light — house at 120 BPM in A minor, 60 bars (2:00)
  progression A: i VI III VII
  progression B: VI VII i i
  form: intro 8 | groove 16 | break 8 | build 4 | drop 16 | outro 8
  tracks: kick (synth kick), clap (synth clap), hats (synth closed_hat), shaker (synth shaker), roll (synth snare), bass (synth patch 'sub_bass'), chords (synth patch 'epiano'), pad (synth patch 'pad'), melody (synth patch 'pluck'), crash (synth crash), riser (synth riser), impact (synth impact), downlifter (synth downlifter), reverse_crash (synth reverse_crash)
  master: -10.0 LUFS, true peak -1.0 dBTP (pre-master -10.2 LUFS); done in 12.8 s
  ears: 'house music' z +3.4 (closest confusion 'techno music' +3.1); production z +1.2 vs 'a low quality recording with hiss' +0.7; p_real 0.47
        sounds like: house music, tech house music, techno music, drum and bass music
  verdict: 87/100 (arc 0.80, genre 0.80, mix 0.99, production 0.63, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -26.4 dB, healthy from -26)
  wrote out/house-first-light-1/song.mp3
```

Line by line:

- **track ...**: each track and the sounds it found.
- **ears loaded**: claudius loaded its listening models (the first run downloads them; later runs take a few seconds
  on an idle machine, longer on a busy one).
- **First Light**: title, style, tempo, key, length.
- **progression**: your chords, written as [Roman numerals](chords.md#the-numbers-in-the-progression).
- **form**: your sections and their lengths.
- **tracks**: every track and what plays it. The last five are the transition sounds your section flags asked for:
  the riser in the build, a reversed cymbal swelling into the drop, and a crash, an impact and a falling
  "downlifter" on the drop's first beat.
- **master**: how loud the finished song is. claudius makes it as loud as songs in the style usually are.
- **ears** and **sounds like**: what the listening model hears. More below.
- **verdict**: the critic's score, and what to fix first.

## Listen before you read

The score is useful, but your ears come first. Listen to the whole song once without stopping, on the best speakers or
headphones you have, then on something small (a phone or a laptop). Note anything that bothers you: a part you can't
hear, a part that is too loud, a moment that drags. Then read the verdict and see whether it agrees.

## The verdict

The critic listens in three ways and gives each a score from 0 to 1:

- **mix**: measurements of the finished audio. Is it as loud as it should be, without distortion? Is the sound
  balanced from deep bass to bright treble? Is it wide enough, and is the bass centred?
- **arc**: does the song get louder where it should? It compares the loudness of each section with its
  [energy](structure.md#energy), and checks that the drop is clearly louder than the break.
- **theory**: the notes. Is the melody in the key? Are its strongest notes part of the chords? Does the bass play
  the chords' roots when they change?

When the listening models are loaded (they are by default), two more:

- **genre**: does it sound like the style you chose, more than the styles it is usually confused with?
- **production**: does it sound like a professionally produced record, more than like a cheap or broken recording?

With `--no-ears`, or on a computer the listening models cannot run on (claudius then says `earshot is unavailable`),
the critic judges with the first three only, and the score is out of those.

The total out of 100 combines them. The **Fix first** list names the problems that cost the most, with numbers and a
hint. Ours has one:

- `dull 4-8 kHz: hats, claps and snare lack bite (presence -26.4 dB, healthy from -26)`. The bright, crisp range where
  hats and claps live is a little quiet compared with the rest. The fix is in the hint: turn the hats and clap up.

The ears line says the song reads as house (z +3.4) a little more than as its closest confusion, techno (z +3.1). The
genre model's readings are hints, not rules: two styles that share a tempo or a drum sound often sit close together
for it.

## Fix and render again

One change for the one problem: the hats and the clap 2 dB louder.

```js
blank("house");
title("First Light");
tempo(120);
key("A minor");
prog("A", "Am F C G");
prog("B", "F G Am Am");
bars_per_chord(1);

track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
track("roll",   s("<sd*2 sd*4 sd*8 sd*16>").gain("<0.5 0.65 0.8 1>"));
track("bass",   chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("sub_bass"));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.5));
track("pad",    chord("prog").s("pad").role("pad"));
track("melody", n("<[4 ~ 2 ~ 0 ~ 2 4] [5 ~ 4 ~ 2 ~ ~ ~] [4 ~ 2 ~ 4 ~ 6 4] [6 ~ 4 ~ 3 ~ ~ ~]>")
                  .octave(4).s("pluck").delay(0.2));

form([
  sec("intro", 8, "kick hats pad").filter_in(),
  sec("groove", 16, "kick clap hats shaker bass chords"),
  sec("break", 8, "pad chords"),
  sec("build", 4, "clap hats pad roll melody").gap(),
  sec("drop", 16, "kick clap hats shaker bass chords pad melody").impact(),
  sec("outro", 8, "kick hats bass"),
]);

mix("hats",   #{ pan: 0.2, level: 2 });
mix("clap",   #{ level: 2 });
mix("shaker", #{ pan: -0.3 });
mix("bass",   #{ duck: 0.5 });
mix("chords", #{ reverb: 0.25, duck: 0.4 });
mix("pad",    #{ level: -3, reverb: 0.4 });
mix("melody", #{ reverb: 0.3 });
```

The new verdict:

```text
  ears: 'house music' z +3.4 (closest confusion 'techno music' +3.1); production z +1.2 vs 'a low quality recording with hiss' +0.7; p_real 0.47
        sounds like: house music, tech house music, techno music, drum and bass music
  verdict: 87/100 (arc 0.80, genre 0.79, mix 1.00, production 0.64, theory 1.00)
```

The mix score went from 0.99 to 1.00, and the critic has nothing left to fix first. The total stays at 87: it weighs
all five scores, and the mix is only one of them. That is a typical round: a small change, a point or less, and a
problem gone.

## Knowing when to stop

A score in the high 80s means the critic found no big measurable problem. It does not mean the song is good; it has
no taste, only measurements. Some things it cannot tell you: whether the melody is catchy, whether the drop is
exciting, whether the song is too long. So:

- Fix what the critic finds when you can hear the problem too.
- Ignore a suggestion that makes the song worse to your ears. The genre model, in particular, is a hint.
- Stop when the song does what you wanted and nothing bothers you on three different speakers.

As an example of the critic's limits: the same song played on a real 909 drum machine with a sampled piano and
strings (`.bank("tr909")`, `.s("piano")`, `.s("string_ensemble")`) scores four points lower than the version with
synthesizers, because the genre model hears it as closer to drum and bass. Whether it sounds better is your call.

## Keep and share it

- **The audio**: `song.mp3` and `song.wav` in the song's folder are the finished, mastered song.
- **The script**: `song.rhai` in that folder is the song as claudius prints it, which you can read and edit. Your own
  `song.rhai` stays the clearest description of what you made.
- **The rest**: `blueprint.json` is the song in full detail, `verdict.json` the critic's whole report.

To see your song as claudius describes it, print it instead of rendering: `claudius script song.rhai --print`. You
will find your progressions as Roman numerals and your tracks exactly as you wrote them.

## Where to go next

- [The studio](../../getting-started/studio.md): the same song in a window, with a step sequencer, a mixer and the
  verdict updating as you edit.
- [Working with an assistant](../../getting-started/assistants.md): ask Claude or another assistant to make and fix
  songs with you.
- [The cookbook](../../cookbook/four-on-the-floor.md): complete songs in eight styles, with every choice explained.
- [For producers](../producers/daw-users.md): the same ideas in more depth, once you are comfortable with these.
