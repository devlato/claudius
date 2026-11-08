// "Island Tide": 2010s tropical house, the sunny, slowed-down house that filled summer playlists in the middle of
// the decade (Kygo, Thomas Jack, Felix Jaehn). An original song in that style.
//
// How it is made:
// - House slowed and softened: a gentle four on the floor at 112 BPM, snaps instead of claps, and shakers.
// - Bright major chords, I V vi IV (G D Em C), on a soft electric piano.
// - The hook is a marimba-like mallet melody: plucked, woody and percussive, where other house tracks would put a
//   synth or a voice.
// - A plucked bass on the offbeats and only a little pumping: everything breathes rather than thumps.
blank("deep_house");
title("Island Tide");
tempo(112); key("G major"); swing(0.54);
prog("A", "I V vi IV");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("perc", "....X.......X...");
drum_sound("perc", "snap");
drum("shaker", "oxoxoxoxoxoxoxox");
drum("ohat", "..o...o...o...o.");
mix("kick", #{ level: -2 });
mix("perc", #{ reverb: 0.25 });
mix("shaker", #{ pan: -0.3 });
mix("ohat", #{ pan: 0.25 });

track("bass", chord("prog").root().struct("~ x ~ x ~ x ~ x").s("pluck_bass").legato(0.4).duck(0.4));
track("keys", chord("prog").voicing(53, 71).struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.7).room(0.25).pan(-0.2).duck(0.3).role("keys"));
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").duck(0.3));
fx("pad", [#{ fx: "width", width: 1.4 }]);
track("mallet", n("<[4 ~ 4 2 4 ~ 7 ~] [4 ~ 4 2 1 ~ ~ ~] [2 ~ 2 4 5 ~ 4 2] [0 ~ 2 ~ 4 ~ ~ ~]>")
                  .octave(4).s("mallet").legato(0.5).delay(0.2).room(0.3).pan(0.15).role("lead"));

form([
  sec("intro", 8, "shaker keys pad").filter_in(),
  sec("verse", 16, "kick perc shaker bass keys"),
  sec("build", 8, "kick perc shaker bass keys pad mallet").build(),
  sec("drop", 16, "kick perc shaker ohat bass keys pad mallet").impact(),
  sec("break", 8, "keys pad mallet"),
  sec("build", 8, "kick perc shaker bass keys pad").build(),
  sec("drop", 16, "kick perc shaker ohat bass keys pad mallet").impact(),
  sec("outro", 8, "kick shaker keys"),
]);
