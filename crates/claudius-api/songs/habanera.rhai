// "Habanera": the Habanera from Bizet's Carmen (1875) as synth-funk rap. The music is in the public domain; this is an
// original arrangement of it, in the style of the synth-funk rap of the 2020s.
//
// How it is made:
// - Bizet's bass, note for note: the habanera's rhythm, a dotted eighth, a sixteenth and two eighths, on D, A, F and A,
//   the A chord's in its bar; a rubbery synth bass plays it, as funk would.
// - Bizet's line, the famous one: down from D by half steps, C sharp, C, B, B flat, to the A held, twice; a brass lead
//   plays it over brass stabs on the offbeats.
// - 110 BPM, a swung boom bap beat: the kick skipping ahead, the snare and a clap on two and four, the hats in eighths,
//   a tom at the end of every other bar.
// - D minor, and A, its dominant, in the third bar of four; a vowel pad holds the chords, as a chant would.
blank("boom_bap");
title("Habanera");
tempo(110); key("D harmonic minor"); swing(0.57);
prog("A", "Dm Dm A7 Dm");
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

// Bizet's bass, sixteen steps a bar: the habanera's dotted rhythm twice a bar, on D minor, and on A in the third bar
track("bass", note("<[d2@3 a2 f2@2 a2@2 d2@3 a2 f2@2 a2@2] [d2@3 a2 f2@2 a2@2 d2@3 a2 f2@2 a2@2] [a1@3 e2 cs2@2 e2@2 a1@3 e2 cs2@2 e2@2] [d2@3 a2 f2@2 a2@2 d2@3 a2 f2@2 a2@2]>")
                .s("rubber_bass").legato(0.6).duck(0.4).role("bass"));
track("stab", chord("prog").voicing(57, 76).struct("~ x ~ ~ ~ x ~ ~ ~ x ~ ~ ~ ~ x ~").s("brass_stab").legato(0.25)
                .duck(0.3).pan(0.15).role("chords"));
// Bizet's line: down by half steps from D to the A, held, twice
track("hook", note("<[d5@4 cs5@2 c5@2 b4@4 bb4@2 a4@2] [a4@12 ~@4] [d5@4 cs5@2 c5@2 b4@4 bb4@2 a4@2] [a4@12 ~@4]>")
                .s("brass_lead").legato(0.7).delay(0.2).role("lead"));
track("chant", chord("prog").voicing(55, 72).s("vowel_pad").room(0.5).role("pad"));

form([
  sec("intro", 8, "bass chant").filter_in(),
  sec("verse", 16, "kick snare clap hat tom bass stab"),
  sec("hook", 8, "kick snare clap hat tom bass stab hook chant").impact(),
  sec("verse", 16, "kick snare clap hat tom bass stab"),
  sec("break", 4, "chant"),
  sec("hook", 8, "kick snare clap hat tom bass stab hook chant").impact(),
  sec("hook", 8, "kick snare clap hat tom bass stab hook chant"),
  sec("outro", 4, "chant hat"),
]);
