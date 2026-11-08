// "Canon Study": Pachelbel's Canon in D (written around 1700) as a 2020s lo-fi beat, the swung, dusty hip hop of
// study playlists. The music is in the public domain; this is an original arrangement of it in that style.
//
// How it is made:
// - The Canon's ground: eight chords that never change, D A Bm F#m G D G A, the progression that pop songs have
//   borrowed for three centuries. Here each chord lasts a bar.
// - Its first violin line as the melody, one long note a chord, F# E D C# B A B C#, on a soft mallet: the descending
//   line everyone knows.
// - Lo-fi's sound: 80 BPM, a lazy swung beat with the snare late, an electric piano under a lowpass, and the whole
//   thing worn with tape and vinyl crackle.
blank("lofi");
title("Canon Study");
tempo(80); key("D major"); swing(0.6);
humanize(0.35);
prog("A", "I V vi iii IV I IV V");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X......x..x.....");
drum("snare", "....X.......X...");
drum("hat", "x.x.x.x.x.x.x.x.");
lane("snare", #{ late: 18 });
mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.25, level: -3 });
bus("drums", [#{ fx: "tape", wow: 0.3, flutter: 0.2, drive_db: 3, lowpass_hz: 12000 }, #{ fx: "vinyl", crackle: 0.4, hiss_db: -46 }]);

// the ground bass, as Pachelbel wrote it: D A B F# G D G A, a note a chord
track("bass", note("<d2 a1 b1 f#1 g1 d2 g1 a1>").s("sub_bass").legato(1));
track("keys", chord("prog").voicing(52, 71).struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.8).pan(-0.25).role("keys").lpf(3500));
fx("keys", [#{ fx: "tape", wow: 0.4, flutter: 0.25, drive_db: 2, lowpass_hz: 6000 }, #{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
// the first violin's line: degrees of D major (0 D ... 6 C#, 7 D), F# E D C# B A B C# an octave up
track("melody", n("<9 8 7 6 5 4 5 6>").octave(4).s("mallet").legato(1).delay(0.3).room(0.35).pan(0.2).role("lead"));
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").room(0.4));
fx("pad", [#{ fx: "width", width: 1.6, mono_below_hz: 200 }]);

form([
  sec("intro", 8, "keys melody").filter_in(),
  sec("verse", 16, "kick snare hat bass keys"),
  sec("hook", 16, "kick snare hat bass keys melody pad"),
  sec("verse", 16, "kick snare hat bass keys melody"),
  sec("hook", 16, "kick snare hat bass keys melody pad"),
  sec("outro", 8, "keys melody"),
]);
