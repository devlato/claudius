// "Big Poe Study": a production study of Tyler, the Creator and Pharrell Williams' BIG POE (2025), the strutting
// opener of Don't Tap the Glass. The record's tempo, key, groove and sound, with an original melody, bass line and
// drums: no note of the record's, and none of the records it samples.
//
// How it is made:
// - 110 BPM, a boom bap beat pushed to dance speed: a fat kick that skips, a snare and a clap together on 2 and 4,
//   swung eighth hats, and a tom that drops on the last beat of every other bar.
// - A major on a funk vamp of two chords (A7 D7), the way N.E.R.D. and the Neptunes built a groove on one shape.
// - A rubbery synth bass that jumps octaves on the sixteenths and bends into each bar, ducking under the kick.
// - Bright brass stabs on the offbeats, a brassy synth hook in the chorus, and a vowel pad that chants a held chord over
//   claps in the intro and the break, as a crowd would.
// - Intro, verse, chorus, verse, a break of claps and chant, and the chorus twice to end.
blank("boom_bap");
title("Big Poe Study");
tempo(110); key("A major"); swing(0.57);
prog("A", "A7 D7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X......x..X..x..");
drum("snare", "....X.......X...");
drum("clap", "....x.......x...");
drum("hat", "x.x.x.x.x.x.x.xo");
drum("tom", "main", "................|............X...", 16);
mix("clap", #{ level: -5 });
mix("hat", #{ pan: 0.2, level: -3 });
mix("tom", #{ pan: -0.3, delay: 0.15 });

track("bass", note("<[a1 ~ a2 ~ ~ a1 ~ g1 ~ a1 ~ ~ c2 ~ cs2 ~] [d2 ~ d3 ~ ~ d2 ~ c2 ~ d2 ~ ~ f2 ~ fs2 ~]>")
                .s("rubber_bass").legato(0.6).duck(0.4).role("bass"));
track("stab", chord("prog").voicing(57, 76).struct("~ x ~ ~ ~ x ~ ~ ~ x ~ ~ ~ ~ x ~").s("brass_stab").legato(0.25)
                .duck(0.3).pan(0.15).role("chords"));
track("hook", n("<[4 ~ 4 5 ~ 4 ~ 2] [3 ~ 3 5 ~ 3 1 0]>").octave(5).s("brass_lead").legato(0.5).delay(0.2)
                .role("lead"));
track("chant", chord("prog").voicing(55, 72).s("vowel_pad").room(0.5).role("pad"));

form([
  sec("intro", 8, "clap chant").filter_in(),
  sec("verse", 16, "kick snare clap hat tom bass stab"),
  sec("hook", 8, "kick snare clap hat tom bass stab hook chant").impact(),
  sec("verse", 16, "kick snare clap hat tom bass stab"),
  sec("break", 4, "clap chant"),
  sec("hook", 8, "kick snare clap hat tom bass stab hook chant").impact(),
  sec("hook", 8, "kick snare clap hat tom bass stab hook chant"),
  sec("outro", 4, "chant hat"),
]);
