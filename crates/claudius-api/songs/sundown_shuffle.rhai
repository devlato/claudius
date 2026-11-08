// "Sundown Shuffle": 2000s UK garage, the two-step sound that ruled British radio at the turn of the decade (Craig
// David, Artful Dodger, MJ Cole). An original song in that style.
//
// How it is made:
// - Two-step at 132 BPM: the kick skips beat 3 and lands just after it instead, the snare holds 2 and 4, and a heavy
//   swing makes every sixteenth shuffle.
// - Minor ninth chords on an organ, chopped into stabs, as on the records that took house's chords and broke its
//   beat.
// - An organ bass that bounces off the kick, an open hat on the offbeats, and rimshots ticking between the hats.
// - A chopped vocal-like lead answering the stabs in the choruses, short phrases with gaps for the MC.
blank("uk_garage");
title("Sundown Shuffle");
tempo(132); key("C minor"); swing(0.62);
prog("A", "i9 i9 iv9 v7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.........x.x...");
drum("snare", "....X.......X...");
drum("hat", "x.o.x.o.x.o.x.o.");
drum("ohat", "..x...x...x...x.");
drum("rim", "..x.....x..x....");
mix("hat", #{ pan: 0.25 });
mix("ohat", #{ pan: -0.15, level: -2 });
mix("rim", #{ pan: -0.3, level: -2 });

track("bass", chord("prog").root().struct("x ~ ~ x ~ x ~ ~").s("organ_bass").legato(0.6));
track("organ", chord("prog").voicing(55, 72).struct("~ x ~ ~ x ~ x ~").s("organ").legato(0.3).pan(-0.2).role("chords"));
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").pan(0.2));
fx("pad", [#{ fx: "width", width: 1.4 }]);
track("vox", n("<[~ 4 ~ 4 6 ~ 4 ~] [~ 4 ~ 2 ~ ~ 0 ~] [~ 3 ~ 3 5 ~ 3 ~] [~ 4 ~ ~ 1 ~ ~ ~]>")
               .octave(4).s("vox_lead").legato(0.6).delay(0.2).room(0.25).role("lead"));

form([
  sec("intro", 8, "hat rim organ").filter_in(),
  sec("verse", 16, "kick snare hat rim bass organ"),
  sec("hook", 16, "kick snare hat ohat rim bass organ pad vox"),
  sec("verse", 16, "kick snare hat rim bass organ"),
  sec("hook", 16, "kick snare hat ohat rim bass organ pad vox"),
  sec("break", 8, "organ pad"),
  sec("hook", 16, "kick snare hat ohat rim bass organ pad vox"),
  sec("outro", 8, "kick hat rim bass"),
]);
