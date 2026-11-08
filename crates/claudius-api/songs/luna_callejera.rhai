// "Luna Callejera": 2020s reggaeton, the Latin urban pop that led streaming charts through the decade (Bad Bunny,
// Karol G, J Balvin). An original song in that style.
//
// How it is made:
// - The dembow at 95 BPM: a kick on every beat, and a snare on the "and" before 2 and 4 and the beat after, the
//   "boom-ch-boom-chick" every reggaeton song rides on.
// - i VI III VII (Gm Eb Bb F): minor and moody, the loop under countless choruses of the genre.
// - A deep 808 holding the roots, plucked chords on the offbeats, and a pad that swells under the hook.
// - A vocal-like lead in short, catchy phrases: the hook a singer would hum.
blank("reggaeton");
title("Luna Callejera");
tempo(95); key("G minor"); swing(0.5);
prog("A", "i VI III VII");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("snare", "...x..x....x..x.");
drum("hat", "x.x.x.x.x.x.x.x.");
drum("perc", "..x.......x...x.");
mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.25, level: -2 });
mix("perc", #{ pan: -0.3 });

track("808", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("808").legato(0.9).role("bass"));
track("pluck", chord("prog").voicing(57, 74).struct("~ x ~ x ~ x ~ x").s("pluck").legato(0.3).pan(-0.25).role("chords"));
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").room(0.3));
fx("pad", [#{ fx: "width", width: 1.5 }]);
track("vox", n("<[4 ~ 4 ~ 2 ~ 4 7] [7 ~ 5 ~ 4 ~ ~ ~] [4 ~ 4 ~ 2 ~ 1 2] [1 ~ ~ ~ ~ ~ ~ ~]>")
               .octave(4).s("vox_lead").legato(0.7).delay(0.2).room(0.25).role("lead"));

form([
  sec("intro", 8, "pluck pad").filter_in(),
  sec("verse", 16, "kick snare hat perc 808 pluck"),
  sec("hook", 16, "kick snare hat perc 808 pluck pad vox").impact(),
  sec("verse", 16, "kick snare hat perc 808 pluck"),
  sec("hook", 16, "kick snare hat perc 808 pluck pad vox").impact(),
  sec("break", 8, "pluck pad vox"),
  sec("hook", 16, "kick snare hat perc 808 pluck pad vox").impact(),
  sec("outro", 8, "kick snare 808 pluck"),
]);
