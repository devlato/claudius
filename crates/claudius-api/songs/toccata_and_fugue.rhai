// "Toccata and Fugue in D Minor": Bach's Toccata and Fugue in D minor (BWV 565, around 1704) as rap rock. The music is
// in the public domain; this is an original arrangement of it, in the style of the rap rock of the early 2000s.
//
// How it is made:
// - Bach's opening, note for note: the A with its quick turn down to G and back, held, then the run down, G F E D, to
//   the C sharp and home on D; then all of it an octave lower. An organ plays it alone first, as Bach wrote it.
// - 100 BPM, a rock beat with a hip hop swing: the kick and snare tight, the hats steady in eighths, a crash at the top
//   of every four bars of the hook.
// - The verse drives on Dm Bb C A, a bass that steps down to the dominant, in distorted guitars: a supersaw driven
//   through a tube in power chords, roots and fifths, chorused wide, with a picked bass doubling the roots in eighths.
// - The hook is Bach's opening on the guitars, over D minor and its dominant, with strings swelling behind.
// - Intro, verse, hook, verse, hook, a break where the organ plays it alone again, the last hook twice as long, and the
//   organ to end.
blank("art_rock");
title("Toccata and Fugue in D Minor");
tempo(100); key("D harmonic minor"); swing(0.52);
prog("A", "Dm Bb C A");
prog("C", "Dm A Dm A");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X.....x.X.x.....");
drum("snare", "....X.......X...");
drum("hat", "x.x.x.x.x.x.x.x.");
drum("crash", "main", "X...............|................|................|................", 16);
mix("crash", #{ level: -3 });
mix("hat", #{ level: 3 });
mix("snare", #{ level: 1 });

// Bach's opening, sixteen steps a bar: the turn on A, the run down to C sharp and D, then the same an octave lower
let toccata = "<[a4 g4 a4@10 ~@4] [g4 f4 e4 d4 cs4@6 d4@6] [a3 g3 a3@10 ~@4] [g3 f3 e3 d3 cs3@6 d3@6]>";
track("organ", note(toccata).add(12).s("organ").legato(0.95).room(0.6).role("keys"));
track("riff", note(toccata).s("supersaw").legato(0.9).role("lead"));
fx("riff", [
  #{ fx: "saturate", kind: "tube", drive_db: 18 },
  #{ fx: "filter", mode: "lp", cutoff: 3200, res: 0.15 },
  #{ fx: "width", width: 1.4, mono_below_hz: 150 },
]);
// the guitars: power chords, root and fifth, driven through a tube
track("guitars", chord("prog").root().transpose(12).struct("x*8").superimpose(|x| x.transpose(7))
                   .s("supersaw").legato(0.9).role("chords"));
fx("guitars", [
  #{ fx: "saturate", kind: "tube", drive_db: 16 },
  #{ fx: "filter", mode: "lp", cutoff: 3000, res: 0.15 },
  #{ fx: "chorus", rate_hz: 0.5, depth_ms: 3, delay_ms: 12, mix: 0.4 },
  #{ fx: "width", width: 1.6, mono_below_hz: 150 },
]);
track("bass", chord("prog").root().struct("x*8").s("pluck_bass").legato(0.7).role("bass"));
fx("bass", [#{ fx: "saturate", kind: "tube", drive_db: 6 }]);
track("strings", chord("prog").voicing(55, 76).s("strings").room(0.45).role("pad").level(-4));

form([
  sec("intro", 4, "organ").prog("C"),
  sec("verse", 16, "kick snare hat bass guitars"),
  sec("hook", 8, "kick snare hat crash bass riff strings").prog("C").impact(),
  sec("verse", 16, "kick snare hat bass guitars"),
  sec("hook", 8, "kick snare hat crash bass riff strings").prog("C").impact(),
  sec("break", 8, "organ").prog("C"),
  sec("hook", 16, "kick snare hat crash bass riff strings").prog("C").impact(),
  sec("outro", 4, "organ").prog("C"),
]);
