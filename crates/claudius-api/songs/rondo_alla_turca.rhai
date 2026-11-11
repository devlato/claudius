// "Rondo alla Turca": Mozart's Rondo alla Turca (the last movement of the Piano Sonata No. 11, K. 331, 1783) as club
// rap. The music is in the public domain; this is an original arrangement of it, in the style of the club rap of the
// 2000s.
//
// How it is made:
// - Mozart's theme in A minor, note for note: B A G sharp A up to the C, the same turn up to E, the run B A G sharp A
//   twice to the high C, and down to E. Short, plucked strings play it, as club rap plays a string riff.
// - 90 BPM, a bouncing club beat: the kick pushing ahead of the snare, a clap and a snare on two and four, a shaker
//   in sixteenths, the hats in eighths.
// - Am under the theme, and E, its dominant, at the end of each four bars; a deep bass on the roots in a dotted
//   rhythm, and a picked guitar playing the chords up in eighths in the hooks.
// - Intro, hook, verse, hook, verse, hook, outro: the hook is the theme with the guitar and a held line in strings.
blank("hip_hop");
title("Rondo alla Turca");
tempo(90); key("A harmonic minor"); swing(0.55);
prog("A", "Am Am Am E");
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

// Mozart's theme, sixteen steps a bar: the pickup B A G sharp A, then the turns up to C and to E, the run to the high C,
// and down to E
let turca = "<[~@12 b4 a4 gs4 a4] [c5@2 ~@2 d5 c5 b4 c5 e5@2 ~@2 f5 e5 ds5 e5] [b5 a5 gs5 a5 b5 a5 gs5 a5 c6@4 a5@2 c6@2] [b5@2 a5@2 gs5@2 a5@2 e5@4 ~@4]>";
track("riff", note(turca).s("strings").legato(0.3).role("chords"));
track("line", note("<[a4@16] [a4@16] [c5@16] [b4@16]>").s("strings").legato(0.95).room(0.35).level(-6).role("lead"));
track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("deep_bass").legato(0.9).role("bass"));
track("guitar", chord("prog").voicing(60, 76).arp("up").struct("x*8").s("pluck").legato(0.4).pan(0.35)
                  .role("lead"));

form([
  sec("intro", 4, "riff shaker").filter_in(),
  sec("hook", 8, "kick clap snare shaker hat riff bass line guitar").impact(),
  sec("verse", 16, "kick clap snare shaker hat riff bass"),
  sec("hook", 8, "kick clap snare shaker hat riff bass line guitar").impact(),
  sec("verse", 16, "kick clap snare shaker hat riff bass"),
  sec("hook", 8, "kick clap snare shaker hat riff bass line guitar").impact(),
  sec("outro", 4, "riff shaker bass"),
]);
