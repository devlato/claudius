// "Glass Penthouse": 2010s alternative R&B, the dark, slow, reverb-drowned R&B of the decade's mixtapes and charts
// (The Weeknd, Frank Ocean, PARTYNEXTDOOR). An original song in that style.
//
// How it is made:
// - Slow and sparse, 75 BPM: a kick that hits twice in two bars, a snare and a quiet clap dragging late on 2 and 4
//   with long tails, hats that roll at the end of every other bar.
// - Minor sevenths that hang, two bars each (Fm7 Dbmaj7 Bbm7 Cm7), on a worn electric piano that pushes into the
//   next chord.
// - An 808 holding each root, rounded off so it is felt more than heard, and a glassy pad drowned in reverb.
// - The voice's line, on a vocal-like lead: long smooth notes, and a run down the scale at the end of the phrase,
//   as an R&B singer would sing it.
blank("alt_rnb");
title("Glass Penthouse");
tempo(75); key("F minor"); swing(0.52);
prog("A", "i7 VImaj7 iv7 v7");
bars_per_chord(2);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar, | between bars
drum("kick", "X.........x.....|..X........o....");
drum("snare", "....X.......X...");
drum("clap", "....X.......X...");
drum("hat", "X.X.X.XoX.X.X.Xo|X.X.X.XoX.XxXxxx");
drum("ohat", "..............o.");
lane("snare", #{ late: 9 });
lane("clap", #{ late: 9 });
mix("snare", #{ reverb: 0.4 });
mix("clap", #{ level: -5, reverb: 0.45 });
mix("hat", #{ pan: 0.15, reverb: 0.15 });
mix("ohat", #{ pan: -0.15, reverb: 0.3 });

track("808", chord("prog").root().struct("x ~ ~ ~ ~ ~ ~ ~").s("808").legato(1).role("bass"));
fx("808", [#{ fx: "saturate", kind: "tanh", drive_db: 3, mix: 0.6 }, #{ fx: "filter", mode: "lp", cutoff: 3500 }]);
track("keys", chord("prog").voicing(48, 72).struct("x ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~ x ~ ~ ~").s("epiano").legato(0.9).room(0.45).role("keys"));
fx("keys", [#{ fx: "tape", wow: 0.2, flutter: 0.1, drive_db: 1.5, lowpass_hz: 9000 }, #{ fx: "width", width: 1.4, mono_below_hz: 200 }]);
track("pad", chord("prog").voicing(48, 74).s("glass_pad").room(0.7).role("pad").level(-2));
fx("pad", [#{ fx: "chorus", rate_hz: 0.3, depth_ms: 3, delay_ms: 14, mix: 0.3 }, #{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
// degrees of F minor (0 F, 2 Ab, 4 C, 5 Db, 7 F an octave up), sixteen steps a bar, the long notes held
track("voice", n("<[4@5 5@2 4 2@8] [7@2 6@2 4@2 2@2 4@8] [2@5 3@2 2 0@8] [5@2 4@2 2@2 0@2 2@8] [7@5 8@2 7 5@8] [10@2 9@2 7@2 5@2 7@8] [8@5 9@2 8 6@8] [9 8 7 6 5@2 4@10]>")
                 .octave(4).s("vox_lead").legato(0.95).room(0.55).delay(0.3).role("lead").level(-3));

form([
  sec("intro", 4, "keys pad"),
  sec("verse", 16, "kick snare clap hat 808 keys"),
  sec("hook", 16, "kick snare clap hat ohat 808 keys pad voice"),
  sec("verse", 16, "kick snare clap hat 808 keys"),
  sec("hook", 16, "kick snare clap hat ohat 808 keys pad voice"),
  sec("break", 8, "keys voice"),
  sec("outro", 8, "keys pad"),
]);
