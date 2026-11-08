// "Coral Skies": 2010s future bass, the bright, swelling sound of the middle of the decade (Flume, The Chainsmokers,
// Illenium). An original song in that style.
//
// How it is made:
// - 150 BPM in half time: a kick on 1 and a snare on 3 make it feel like 75, while the hats roll at full speed,
//   in triplets at the end of every other bar.
// - Seventh chords, IVmaj7 V iii7 vi7 (Amaj7 B G#m7 C#m7 in E major): bright, with a catch in the minor ones.
// - The drop's chords are the instrument: a wide supersaw held for the whole bar and ducking hard under the kick,
//   so each chord swells back in after every hit. That pump is the genre's signature.
// - Vocal chops play the hook: short, high, bouncing between chord tones.
// - An 808 under it all holds each root, and the verse is only a pad, a snap and the 808, so the drop arrives.
blank("future_bass");
title("Coral Skies");
tempo(150); key("E major"); swing(0.5);
prog("A", "IVmaj7 V iii7 vi7");
bars_per_chord(1);

// drums in steps: X accent, x hit, o ghost, . rest; 16 a bar unless a number follows
drum("kick", "X.........X.....");
drum("snare", "........X.......");
drum("hat", "main", "x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....xxxxxxxxxxxx", 48);
drum("ohat", "..x...........x.");
drum("clap", "........x.......");
drum("perc", "....x.....x...x.");
drum_sound("perc", "snap");
mix("snare", #{ reverb: 0.25 });
mix("clap", #{ level: -3, pan: 0.1 });
mix("hat", #{ pan: 0.2 });
mix("perc", #{ pan: -0.3, reverb: 0.25 });
fx("hat", [#{ fx: "filter", mode: "lp", cutoff: 7500 }]);

track("bass", chord("prog").root().struct("x ~ ~ ~ ~ ~ ~ ~").s("808").legato(1).role("bass"));
track("chords", chord("prog").voicing(52, 79).s("supersaw").legato(1).duck(0.85).room(0.2).role("chords"));
// grit on the chords and the chops, so the brightness has an edge rather than a polish
fx("chords", [
  #{ fx: "tape", drive_db: 5, wow: 0.2, flutter: 0.1, lowpass_hz: 8500 },
  #{ fx: "width", width: 1.4, mono_below_hz: 160 },
]);
track("vox", n("<[4 ~ 6 ~ 7 ~ 6 4] [4 ~ 6 ~ 8 ~ 7 ~] [2 ~ 4 ~ 6 ~ 4 2] [7 ~ 6 ~ 4 ~ ~ ~]>")
               .octave(4).s("vox_lead").legato(0.6).delay(0.25).room(0.3).role("lead"));
fx("vox", [#{ fx: "saturate", kind: "tube", drive_db: 6, mix: 0.6 }]);
track("pad", chord("prog").voicing(50, 71).s("warm_pad").role("pad"));
fx("pad", [#{ fx: "width", width: 1.3, mono_below_hz: 150 }]);

form([
  sec("intro", 8, "pad").filter_in(),
  sec("verse", 16, "kick perc hat bass pad"),
  sec("build", 8, "snare hat pad chords vox").build().gap(),
  sec("drop", 16, "kick snare clap hat ohat perc bass chords vox").impact(),
  sec("verse", 16, "kick perc hat bass pad"),
  sec("break", 8, "pad"),
  sec("build", 8, "snare hat pad chords vox").build().gap(),
  sec("drop", 16, "kick snare clap hat ohat perc bass chords vox").impact(),
  sec("outro", 8, "pad"),
]);
