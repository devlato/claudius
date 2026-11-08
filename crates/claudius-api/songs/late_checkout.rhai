// "Late Checkout": 2020s tech house, the bass-led club music that filled festivals and charts in the decade (Fisher,
// Chris Lake, John Summit). An original song in that style.
//
// How it is made:
// - 126 BPM, between house and techno: a punchy kick on every beat, a clap on 2 and 4, offbeat hats, shakers in
//   sixteenths and a rimshot that loops through a delay.
// - The bassline is the hook: a rubbery bass in short sixteenths between the kicks, a groove more than a melody, with
//   a turn at the end of every four bars that leads back to the root.
// - Few chords (Gm Gm Eb F) and fewer notes: a short stab now and then, pumping with the kick, and a filtered pad
//   only in the breakdown.
// - Long, tight structure: sixteen bars to bring the groove in, a breakdown that takes the kick away, and a build
//   that snaps back into the drop.
blank("tech_house");
title("Late Checkout");
tempo(126); key("G minor"); swing(0.53);
prog("A", "i i VI VII");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("hat", "..x...x...x...xo");
drum("ohat", "......o.......x.");
drum("shaker", "xoxoxoxoxoxoxoxo");
drum("perc", "...o.....o....x.");
mix("clap", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, delay: 0.1 });

track("bass", note("<[~ ~ g1 ~ g1 ~ ~ g1 ~ ~ g1 ~ g1 ~ ~ g1] [~ g1 g1 ~ ~ ~ g1 ~ ~ g1 g1 ~ ~ ~ g1 ~] [~ ~ eb2 ~ eb2 ~ ~ eb2 ~ ~ eb2 ~ eb2 ~ ~ eb2] [~ ~ f1 ~ f1 ~ ~ f1 ~ ~ c2 ~ a1 ~ ~ f#1]>")
                .s("rubber_bass").legato(0.5).duck(0.5).level(-2));
track("stab", chord("prog").voicing(52, 72).struct("~ ~ ~ x ~ ~ x ~ ~ ~ ~ ~ ~ ~ ~ ~").s("stab").legato(0.3).room(0.2).delay(0.2).duck(0.4).role("chords"));
fx("stab", [#{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
track("pad", chord("prog").voicing(48, 72).s("warm_pad").room(0.4).duck(0.5).role("pad").lpf(2400));
fx("pad", [#{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
track("roll", s("<sd*4 sd*4 sd*8 sd*8 sd*16 sd*16 sd*16 sd*16>")
                .gain("<0.35 0.4 0.5 0.55 0.65 0.75 0.85 1>").role("snare"));

form([
  sec("intro", 16, "kick hat shaker perc").filter_in(),
  sec("groove", 16, "kick clap hat shaker perc bass"),
  sec("drop", 16, "kick clap hat ohat shaker perc bass stab").impact(),
  sec("break", 16, "shaker pad stab"),
  sec("build", 8, "kick shaker roll bass").build().gap(),
  sec("drop", 16, "kick clap hat ohat shaker perc bass stab").impact(),
  sec("outro", 16, "kick hat shaker perc"),
]);
