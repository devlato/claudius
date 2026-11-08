// "Flashbulb": 2000s electropop, the club-ready pop at the end of the decade (Lady Gaga, Britney Spears, Kesha).
// An original song in that style.
//
// How it is made:
// - 124 BPM four on the floor, with a snare roll into each chorus as the pre-chorus.
// - i VII VI VII (Bm A G A): the chords rock back and forth instead of resolving, which keeps a chorus hammering.
// - A buzzing synth bass on eighths that ducks under the kick, and stabs on the offbeats: the electro in electropop.
// - The verse is thin (kick, bass, a square lead) so the chorus can add the stabs, the claps, a pad and the hook.
// - The hook is short and repeated, climbing each time it comes round, and doubled an octave up by a pluck, the
//   way a topline's double sits under a pop vocal.
blank("dance_pop");
title("Flashbulb");
tempo(124); key("B minor"); swing(0.5);
prog("A", "i VII VI VII");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("hat", "..x...x...x...x.");
mix("hat", #{ pan: 0.2, level: 2 });
// the pre-chorus roll as a pattern: quarters, eighths, then sixteenths, louder each bar
track("roll", s("<sd*4 sd*8 sd*8 sd*16>").gain("<0.4 0.55 0.7 0.9>").role("snare"));

track("bass", chord("prog").root().struct("x*8").s("saw_bass").legato(0.5).duck(0.6));
track("stab", chord("prog").voicing(57, 74).struct("~ x ~ x ~ x ~ x").s("stab").legato(0.2).duck(0.5).pan(0.2).role("chords"));
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").duck(0.6).pan(-0.2));
track("verse", n("<[0 ~ 0 2 ~ 0 ~ ~] [6 ~ 6 8 ~ 6 ~ ~] [5 ~ 5 7 ~ 5 ~ ~] [6 ~ 6 8 ~ 4 ~ ~]>")
                 .octave(4).s("square_lead").legato(0.6).role("lead"));
track("hook", n("<[4 4 4 ~ 2 ~ 4 ~] [6 6 6 ~ 8 ~ 6 ~] [7 7 7 ~ 9 ~ 7 ~] [8 ~ 7 ~ 6 ~ 4 ~]>")
                .octave(4).s("supersaw").legato(0.7).room(0.2).delay(0.15).role("lead"));
track("double", n("<[4 4 4 ~ 2 ~ 4 ~] [6 6 6 ~ 8 ~ 6 ~] [7 7 7 ~ 9 ~ 7 ~] [8 ~ 7 ~ 6 ~ 4 ~]>")
                  .octave(5).s("pluck").legato(0.5).pan(0.3).role("lead"));

form([
  sec("intro", 8, "kick hat bass").filter_in(),
  sec("verse", 16, "kick hat bass verse"),
  sec("build", 8, "kick clap hat bass pad roll").build(),
  sec("hook", 16, "kick clap hat bass stab pad hook double").impact(),
  sec("verse", 16, "kick clap hat bass verse"),
  sec("build", 8, "kick clap hat bass pad roll").build().gap(),
  sec("hook", 16, "kick clap hat bass stab pad hook double").impact(),
  sec("outro", 8, "kick hat bass stab"),
]);
