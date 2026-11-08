// "Floor Shaker": 2000s crunk, the shouted, 808-heavy club rap out of Atlanta in the middle of the decade (Lil Jon,
// Ying Yang Twins, Three 6 Mafia). An original song in that style.
//
// How it is made:
// - 84 BPM with a booming 808 kick and an 808 bass that is the same note held long, and claps on 2 and 4 loud
//   enough to shout over.
// - Harmonic minor, i i VI V (Ebm Ebm Cb Bb): the raised seventh in the V chord gives it its menace.
// - A blaring synth riff of a few notes, a half step from the root and back, repeated until it is a chant.
// - Brass stabs hitting with the claps, and strings under the hook: the drama of crunk is cheap and huge.
blank("crunk");
title("Floor Shaker");
tempo(84); key("Eb harmonic minor"); swing(0.5);
prog("A", "i i VI V");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.....x...x.....");
drum("clap", "....X.......X...");
drum("snare", "....x.......x...");
drum("hat", "x.x.x.x.x.x.x.x.");
mix("clap", #{ reverb: 0.2, level: 1 });
mix("hat", #{ pan: 0.25 });

track("bass", chord("prog").root().struct("x ~ ~ ~ ~ ~ x ~").s("808").legato(1).role("bass"));
track("riff", n("<[0 ~ 0 1 0 ~ ~ ~] [0 ~ 0 1 0 ~ -1 ~] [0 ~ 0 1 0 ~ ~ ~] [4 ~ 3 ~ 1 ~ 0 ~]>")
                .octave(4).s("saw_lead").legato(0.6).room(0.2).role("lead"));
track("stab", chord("prog").voicing(55, 72).struct("~ x ~ x").s("brass_stab").legato(0.3).pan(-0.25).role("chords"));
track("strings", chord("prog").voicing(55, 76).s("strings").room(0.35).role("pad").pan(0.25));
fx("strings", [#{ fx: "width", width: 1.5 }]);

form([
  sec("intro", 8, "riff strings").filter_in(),
  sec("verse", 16, "kick clap snare hat bass riff"),
  sec("hook", 8, "kick clap snare hat bass riff stab strings").impact(),
  sec("verse", 16, "kick clap snare hat bass riff"),
  sec("hook", 8, "kick clap snare hat bass riff stab strings").impact(),
  sec("break", 8, "strings riff"),
  sec("hook", 8, "kick clap snare hat bass riff stab strings").impact(),
  sec("outro", 8, "kick clap bass riff"),
]);
