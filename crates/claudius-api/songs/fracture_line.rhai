// "Fracture Line": 2010s dubstep, the half-time, bass-first sound that crossed from London's clubs to festival
// stages at the start of the decade (Skrillex, Rusko, Flux Pavilion). An original song in that style.
//
// How it is made:
// - 140 BPM in half time: the kick on 1 and the snare on 3, so it feels like 70 while the hats run at 140.
// - The bass is the lead: a wobble whose filter moves on its own, played in a syncopated rhythm, with a distorted
//   bass doubling it in the drop.
// - A dark minor key with a phrygian half step (Fm Gb), and almost no harmony: the drop is drums and bass.
// - An eerie, filtered saw melody and a pad carry the intro and the breakdown, and a riser and a beat of silence
//   set up each drop.
blank("dubstep");
title("Fracture Line");
tempo(140); key("F phrygian"); swing(0.5);
prog("A", "i i II i");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.........X.....");
drum("snare", "........X.......");
drum("hat", "x.x.x.x.x.x.x.xx");
mix("snare", #{ reverb: 0.25, level: 1 });
mix("hat", #{ pan: 0.25 });

track("sub", chord("prog").root().struct("x ~ ~ ~ ~ ~ ~ ~").s("sub_bass").legato(1).role("bass").level(-3));
track("wobble", note("<[f1 ~ f1 f1 ~ f1 ~ ~] [f1 ~ f1 ~ ab1 ~ f1 ~] [gb1 ~ gb1 gb1 ~ gb1 ~ ~] [f1 ~ f1 ~ eb1 ~ c1 ~]>")
                  .s("wobble").legato(0.9).role("bass"));
track("growl", note("<[f1 ~ f1 f1 ~ f1 ~ ~] [f1 ~ f1 ~ ab1 ~ f1 ~] [gb1 ~ gb1 gb1 ~ gb1 ~ ~] [f1 ~ f1 ~ eb1 ~ c1 ~]>")
                 .s("dist_bass").legato(0.9).role("bass"));
track("melody", n("<[0 ~ ~ 2 ~ ~ 1 ~] [0@4] [1 ~ ~ 3 ~ ~ 2 ~] [1@2 0@2]>").octave(4).s("saw_lead").legato(0.9).lpf(3500).delay(0.3).room(0.35).role("lead"));
track("pad", chord("prog").voicing(52, 70).s("pad").room(0.4).role("pad"));
fx("pad", [#{ fx: "width", width: 1.5 }]);

form([
  sec("intro", 16, "melody pad").filter_in(),
  sec("build", 8, "kick hat melody pad sub").build().gap(),
  sec("drop", 16, "kick snare hat sub wobble growl").impact(),
  sec("break", 16, "melody"),
  sec("build", 8, "kick hat melody pad sub").build().gap(),
  sec("drop", 16, "kick snare hat sub wobble growl melody").impact(),
  sec("outro", 16, "kick snare hat sub pad"),
]);
