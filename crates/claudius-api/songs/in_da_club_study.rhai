// "In Da Club Study": a production study of 50 Cent's In Da Club (2003), the Dr. Dre and Mike Elizondo beat that
// topped the decade. The record's tempo, key, groove and sound, with an original melody, bass line and drums: no note
// of the record's.
//
// How it is made:
// - 90 BPM, a beat that leans off the grid: the kick lands after the beat as often as on it, a clap snaps on 2 and 4
//   over a snare, and a shaker and hats ride the sixteenths with the offbeats pushed.
// - C-sharp minor with its raised seventh, mostly one chord (C#m C#m A G#): the beat is a vamp, and the turn at the
//   end of four bars is all the harmony there is.
// - Staccato synth strings play the riff, short and dry, in the gaps the kick leaves; the same strings, held high and
//   thin, whine over the chorus.
// - A deep, round synth bass holding the roots, and in the chorus an eight-note figure plucked on the chord tones,
//   where the record has its guitar.
// - Less is more, but big: few parts, each loud and clear, with almost no reverb.
blank("hip_hop");
title("In Da Club Study");
tempo(90); key("C# harmonic minor"); swing(0.55);
prog("A", "C#m C#m A G#");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X..x...x..X..x..");
drum("clap", "....X.......X...");
drum("snare", "....x.......x..o");
drum("shaker", ".x.x.x.x.x.x.x.x");
drum("hat", "x.x.x.x.x.x.x.x.");
mix("clap", #{ level: -4 });
mix("snare", #{ level: -3 });
mix("shaker", #{ pan: -0.3, level: -2 });
mix("hat", #{ pan: 0.25 });

// the riff: short strings in the gaps the kick leaves
track("riff", note("<[~ cs4 ~ e4 ~ ~ gs4 ~ fs4 ~ e4 ~ ~ cs4 ~ ~] [~ cs4 ~ e4 ~ ~ gs4 ~ b4 ~ gs4 ~ fs4 ~ e4 ~] [~ cs4 ~ e4 ~ ~ a4 ~ gs4 ~ e4 ~ ~ cs4 ~ ~] [~ bs3 ~ ds4 ~ ~ gs4 ~ fs4 ~ ds4 ~ bs3 ~ ~ ~]>")
                .s("strings").legato(0.25).role("chords"));
track("whine", n("<[7 ~ ~ ~ 9 ~ 8 ~] [7 ~ ~ ~ 6 ~ 4 ~] [5 ~ ~ ~ 7 ~ 9 ~] [8@4 ~ ~ ~ ~]>").octave(5)
                .s("strings").legato(0.95).role("lead"));
track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("deep_bass").legato(0.9).role("bass"));
track("guitar", chord("prog").voicing(60, 76).arp("up").struct("x*8").s("pluck").legato(0.4).pan(0.35)
                  .role("lead"));

form([
  sec("intro", 4, "riff shaker").filter_in(),
  sec("hook", 8, "kick clap snare shaker hat riff bass whine guitar").impact(),
  sec("verse", 16, "kick clap snare shaker hat riff bass"),
  sec("hook", 8, "kick clap snare shaker hat riff bass whine guitar").impact(),
  sec("verse", 16, "kick clap snare shaker hat riff bass"),
  sec("hook", 8, "kick clap snare shaker hat riff bass whine guitar").impact(),
  sec("outro", 4, "riff shaker bass"),
]);
