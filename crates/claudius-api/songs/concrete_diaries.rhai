// "Concrete Diaries": 1990s boom bap, the golden age of New York hip hop in the middle of the decade (Nas, Pete
// Rock, A Tribe Called Quest). An original song in that style: none of its notes come from a record.
//
// How it is made:
// - 92 BPM, swung and loose: the snare and the rimshot land 14 ms late and the kick a touch early, and every hit is
//   humanized, which is the lazy, head-nodding feel the drum machines of the time were programmed to have.
// - The "boom" and the "bap": a kick that changes from bar to bar over four bars, and a snare cracking on 2 and 4
//   with a ghost before the second.
// - A jazz loop of minor and major sevenths (Dm7 Fmaj7 Gm7 C7, two bars each), the kind of record a producer dug for
//   and looped: here a clavinet comping, worn with tape and widened with a chorus, as if sampled.
// - A sub bass on the roots under the verse and nothing else: room for the rapper.
// - The hook brings in a horn phrase through a slow phaser and strings, the way a chorus brings in a second sample.
blank("boom_bap");
title("Concrete Diaries");
tempo(92); key("D minor"); swing(0.6);
humanize(0.45);
prog("A", "i7 IIImaj7 iv7 VII7");
bars_per_chord(2);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar, four bars of kick
drum("kick", "X......x..x.....|X.........x.x...|X......x..x.....|X.....x...x..x..");
drum("snare", "....X...o...X...");
drum("hat", "x.x.x.x.x.x.x.xx");
drum("rim", "......x.........");
lane("kick", #{ late: -4 });
lane("snare", #{ late: 14 });
lane("rim", #{ late: 14 });
mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.2 });
mix("rim", #{ pan: -0.3, reverb: 0.15 });

track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("sub_bass").legato(0.8));
fx("bass", [#{ fx: "tape", wow: 0.4, flutter: 0.2, drive_db: 4, lowpass_hz: 12000 }]);
track("keys", chord("prog").voicing(50, 74).struct("x ~ ~ x ~ x ~ ~").s("clav").legato(0.6).role("keys"));
fx("keys", [
  #{ fx: "tape", wow: 0.2, flutter: 0.15, drive_db: 2, lowpass_hz: 11000 },
  #{ fx: "width", width: 1.3, mono_below_hz: 200 },
  #{ fx: "chorus", rate_hz: 0.45, depth_ms: 2.5, delay_ms: 11, mix: 0.4 },
]);
track("horns", n("<[~ ~ ~ ~ 4@2 2 ~] ~ [~ ~ ~ ~ 4@2 2 ~] ~ [~ ~ ~ ~ 5@2 3 ~] ~ [~ ~ ~ ~ 3@2 6 ~] ~>")
                 .octave(4).s("brass_lead").legato(0.9).room(0.25).delay(0.08).role("lead"));
fx("horns", [#{ fx: "phaser", rate_hz: 0.18, stages: 6, depth: 0.85, feedback: 0.3, mix: 0.4 }]);
track("strings", chord("prog").voicing(57, 76).s("strings").room(0.35).role("pad").level(-2));
fx("strings", [#{ fx: "width", width: 1.5 }]);

form([
  sec("intro", 8, "hat keys").filter_in(),
  sec("verse", 16, "kick snare hat rim bass keys"),
  sec("hook", 8, "kick snare hat rim bass keys horns strings"),
  sec("verse", 16, "kick snare hat rim bass keys"),
  sec("hook", 8, "kick snare hat rim bass keys horns strings"),
  sec("verse", 16, "kick snare hat rim bass keys strings").fills(),
  sec("hook", 8, "kick snare hat rim bass keys horns strings"),
  sec("outro", 8, "keys strings"),
]);
