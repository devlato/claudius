// "Für Elise": Beethoven's Für Elise (Bagatelle No. 25 in A minor, WoO 59, 1810) as minimal electropop. The music is
// in the public domain; this is an original arrangement of it, in the style of the minimal electropop of the 2010s.
//
// How it is made:
// - Beethoven's opening, note for note: E and D sharp rocking, down by B, D and C to A, the A minor and E chords
//   broken up to it, and the turn again; laid in sixteenths across three bars of four, and a bar's rest before it
//   comes back. A plucked synth plays it, chorused wide.
// - 135 BPM: a kick on every beat and a finger snap on two and four, then a bridge at half time where the kick and a
//   snare drop to the half and the hats roll.
// - A distorted bass bouncing on A and E in eighths, with an 808 under it on the roots, and an 808 sliding between
//   them in the bridge.
// - The theme alone opens and ends the song; the verses are the bass and the beat, the hooks bring the theme back.
blank("hip_hop");
title("Für Elise");
tempo(135); key("A harmonic minor"); swing(0.5);
prog("A", "Am E Am Am");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X...X...X...X...");
drum("snap", "....X.......X...");
drum("kick", "half", "X.........X.....");
drum("snare", "half", "........X.......");
drum("hat", "half", "x.x.x.x.x.x.xxxxx.x.x.x.x.x.x.x.x.x.x.x.x.x.xxxxxxxxx.x.x.x.x.x.", 32);
mix("snap", #{ reverb: 0.35, level: 2 });
mix("hat", #{ pan: 0.2 });

// Beethoven's opening, sixteen steps a bar: E D sharp E D sharp E B D C to A, the chords broken up to B and to C, the
// same again, and down to A; then a bar's rest
let elise = "<[e5 ds5 e5 ds5 e5 b4 d5 c5 a4@2 ~ c4 e4 a4 b4@2] [~ e4 gs4 b4 c5@2 ~ e4 e5 ds5 e5 ds5 e5 b4 d5 c5] [a4@2 ~ c4 e4 a4 b4@2 ~ e4 c5 b4 a4@4] [~@16]>";
track("elise", note(elise).s("pluck").legato(0.5).delay(0.15).room(0.25).role("lead"));
fx("elise", [
  #{ fx: "chorus", rate_hz: 0.8, depth_ms: 3, delay_ms: 10, mix: 0.5 },
  #{ fx: "width", width: 1.7, mono_below_hz: 200 },
]);
track("bass", note("<[a1 ~ a2 ~ a1 a1 ~ c2] [e1 ~ e2 ~ gs1 ~ b1 ~] [a1 ~ a2 ~ e2 ~ c2 ~] [a1 ~ a2 ~ a1 a1 ~ g1]>")
                .s("dist_bass").legato(0.6).duck(0.3).role("bass"));
track("sub", chord("prog").root().transpose(-12).struct("x").s("808").legato(1).role("bass").level(-3));
track("slide", note("<[a1@6 b1 c2] [e1@8] [a1@6 g1 f1] [e1@4 gs1@4]>").s("808").legato(1).role("bass"));

form([
  sec("intro", 4, "elise").energy(0.3),
  sec("verse", 16, "kick snap bass sub").energy(0.6),
  sec("build", 4, "kick snap bass").energy(0.55).build(),
  sec("hook", 8, "kick snap bass sub elise").energy(0.8).impact(),
  sec("hook", 8, "kick bass elise snap").name("post").energy(0.85),
  sec("verse", 16, "kick snap bass sub").energy(0.6),
  sec("build", 4, "kick snap bass").energy(0.55).build(),
  sec("hook", 8, "kick snap bass sub elise").energy(0.8).impact(),
  sec("hook", 8, "kick bass elise snap").name("post").energy(0.85),
  sec("groove", 16, "kick snare hat slide elise").name("bridge").variant("half").energy(0.9).impact(),
  sec("outro", 4, "elise").energy(0.3),
]);
fx("kick", [#{ fx: "saturate", kind: "hard", drive_db: sections(0, #{ bridge: 16 }) }]);
