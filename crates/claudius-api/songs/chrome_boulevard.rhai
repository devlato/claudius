// "Chrome Boulevard": 2000s French house, the filtered disco loops out of Paris at the turn of the decade (Daft
// Punk's Discovery era, Stardust, Modjo). An original song in that style.
//
// How it is made:
// - A loop, not a progression: two bars of minor-seventh disco chords (Gm7 Cm7) that play the whole song, on a
//   plucked guitar-like chop, an electric piano and stabs, as if cut from a record.
// - The filter is the arrangement: a lowpass on the loop that is closed in the intro, opens across the build and
//   is wide open in the drop, then closes again for the break.
// - Heavy pumping: the loop and the bass duck deep under the kick, which is the French touch's breathing.
// - A vocoder-like lead (a vowel synth through a chorus) sings a short phrase over the drop.
blank("filter_house");
title("Chrome Boulevard");
tempo(124); key("G minor"); swing(0.54);
prog("A", "i7 i7 iv7 iv7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("ohat", "..x...x...x...x.");
drum("shaker", "o.x.o.x.o.x.o.x.");
mix("ohat", #{ level: 2 });
mix("shaker", #{ pan: -0.3 });

track("bass", note("<[g1 g2 ~ g1 bb1 ~ g2 f2] [g1 g2 ~ g1 bb1 ~ d2 f2] [c2 c3 ~ c2 eb2 ~ c3 bb1] [c2 c3 ~ c2 eb2 ~ g2 bb2]>")
                .s("rubber_bass").legato(0.5).duck(0.8));
track("loop", chord("prog").voicing(55, 72).struct("x ~ x x ~ x ~ x").s("pluck").legato(0.35).duck(0.9).pan(-0.25).role("chords"));
track("stab", chord("prog").voicing(60, 76).struct("~ ~ x ~ ~ ~ x ~").s("stab").legato(0.2).duck(0.9).pan(0.25).role("chords"));
track("keys", chord("prog").voicing(52, 70).struct("x ~ ~ ~ x ~ ~ ~").s("epiano").legato(0.9).duck(0.9).role("keys"));
fx("loop", [#{ fx: "filter", mode: "lp", res: 0.25,
               cutoff: sections(9000, #{ intro: [350, 1400], groove: 2200, build: [2200, 9000], "break": [3000, 600] }) }]);
fx("stab", [#{ fx: "filter", mode: "lp", res: 0.2,
               cutoff: sections(9000, #{ groove: 1800, build: [1800, 9000], "break": [3000, 600] }) }]);
fx("keys", [#{ fx: "filter", mode: "lp", res: 0.2,
               cutoff: sections(9000, #{ intro: [350, 1400], groove: 2200, build: [2200, 9000], "break": [3000, 600] }) }]);
track("vocoder", n("<[4 ~ 4 ~ 3 ~ 2 ~] [4 ~ 4 ~ 6 ~ 4 ~] [2 ~ 2 ~ 4 ~ 0 ~] [2 ~ 4 ~ 6 ~ ~ ~]>")
                   .octave(4).s("vox_lead").legato(0.8).role("lead").delay(0.15));
fx("vocoder", [#{ fx: "chorus", rate_hz: 0.8, depth_ms: 4, delay_ms: 12, mix: 0.5 }]);

form([
  sec("intro", 16, "kick shaker loop keys"),
  sec("groove", 16, "kick clap shaker bass loop stab keys"),
  sec("build", 8, "kick clap shaker bass loop stab keys").build(),
  sec("drop", 16, "kick clap ohat shaker bass loop stab keys vocoder").impact(),
  sec("break", 8, "loop keys"),
  sec("build", 8, "kick clap shaker bass loop stab keys").build(),
  sec("drop", 16, "kick clap ohat shaker bass loop stab keys vocoder").impact(),
  sec("outro", 8, "kick shaker loop keys"),
]);
