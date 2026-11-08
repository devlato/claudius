// "Mountain Kings": Grieg's In the Hall of the Mountain King (from Peer Gynt, 1875) as drum and bass. The melody is
// in the public domain; this is an original arrangement of it, in the style of the drum and bass of the 2010s and
// 2020s.
//
// How it is made:
// - Grieg's theme, in B minor, four bars: up the scale, then down chromatically (F#, F, E) and back, a tune built
//   to speed up. Drum and bass gives it the speed at once: 172 BPM.
// - The genre's two-step beat: a kick on 1 and the "and" of 3, a snare on 2 and 4, rolling hats and ghost snares.
// - Bm F# Bm D under it, and a reese bass, the detuned, growling bass of the genre, on the roots.
// - The theme alone on a piano in the intro and the breakdown, then on a supersaw in the drops.
blank("dnb");
title("Mountain Kings");
tempo(172); key("B minor"); swing(0.5);
prog("A", "i V i III");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.........X.....");
drum("snare", "....X..o....X..o");
drum("hat", "x.x.x.x.x.x.x.xx");
mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.25, level: -2 });

// the theme, an eighth note a step, the long notes held
let king = "<[b3 c#4 d4 e4 f#4 d4 f#4@2] [f4 c#4 f4@2 e4 c4 e4@2] [b3 c#4 d4 e4 f#4 d4 f#4 b4] [a4 f#4 d4 f#4 a4@4]>";
track("piano", note(king).s("keys").legato(0.8).room(0.3).role("keys"));
track("lead", note(king).add(12).s("supersaw").legato(0.85).room(0.25).delay(0.1).role("lead"));
track("reese", chord("prog").root().struct("x ~ ~ ~ x ~ x ~").s("reese").legato(0.9).role("bass"));
track("pad", chord("prog").voicing(52, 72).s("pad").role("pad").room(0.4));
fx("pad", [#{ fx: "width", width: 1.5 }]);

form([
  sec("intro", 16, "piano pad").filter_in(),
  sec("build", 8, "hat piano pad").build().gap(),
  sec("drop", 32, "kick snare hat reese pad lead").impact(),
  sec("break", 16, "piano pad"),
  sec("build", 8, "hat piano pad reese").build().gap(),
  sec("drop", 32, "kick snare hat reese pad lead").impact(),
  sec("outro", 16, "kick snare hat pad"),
]);
