// "Ice Avenue": 2010s trap, the Atlanta sound that took over hip hop in the second half of the decade (Metro
// Boomin, Migos, 21 Savage). An original song in that style.
//
// How it is made:
// - 140 BPM felt in half time: a clap on 3, a kick placed around it, and hats that roll in triplets and thirty-
//   seconds at the end of the bar, the trap hi-hat.
// - The 808 is both kick and bass: long, booming notes on the roots.
// - A dark harmonic minor loop, i VI iv V (C#m A F#m G#), its melody a slow, cinematic string line: the eerie loop
//   a producer builds the beat around. Trap's melodies are dark and simple, so that the drums and the 808 lead.
// - String chords under the hook and a pad under it all, and a sparse intro with the line alone, so the beat drops
//   in.
blank("trap");
title("Ice Avenue");
tempo(140); key("C# harmonic minor"); swing(0.5);
prog("A", "i VI iv V");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar unless a number follows
drum("kick", "X.........X..x..");
drum("clap", "........X.......");
drum("hat", "main", "x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....xxxxxxxxxxxx", 48);
drum("ohat", "..............x.");
mix("clap", #{ reverb: 0.2, level: -3 });
mix("hat", #{ pan: 0.25 });
mix("ohat", #{ pan: -0.25 });

track("808", chord("prog").root().struct("x ~ ~ ~ ~ ~ x ~").s("808").legato(1).role("bass"));
track("line", n("<[7@2 4@2] [5@2 2@2] [3@2 0@2] [4@3 1]>")
                .octave(4).s("strings").legato(1).delay(0.2).room(0.45).pan(-0.15).role("pad"));
track("strings", chord("prog").voicing(55, 76).s("strings").room(0.4).role("pad").pan(0.2));
fx("strings", [#{ fx: "width", width: 1.6 }]);
track("pad", chord("prog").voicing(48, 67).s("pad").room(0.4).role("pad").level(-3));
fx("pad", [#{ fx: "width", width: 1.6 }]);

form([
  sec("intro", 8, "line pad").filter_in(),
  sec("hook", 16, "kick clap hat ohat 808 line strings pad").impact(),
  sec("verse", 16, "kick clap hat 808 line pad"),
  sec("hook", 16, "kick clap hat ohat 808 line strings pad").impact(),
  sec("verse", 16, "kick clap hat 808 line pad"),
  sec("hook", 16, "kick clap hat ohat 808 line strings pad").impact(),
  sec("outro", 8, "line strings"),
]);
