// "Blue Hour Static": 1990s trip hop, the slow, dark breakbeat music out of Bristol in the middle of the decade
// (Massive Attack, Portishead, Tricky). An original song in that style.
//
// How it is made:
// - 88 BPM and swung, a hip hop beat slowed into a sulk: the kick lands late in the bar and the snare is heavy and
//   wet.
// - Dust: the drums run through tape and a lowpass and carry vinyl crackle, as if sampled from an old record.
// - i VI iv V (Gm Eb Cm D): the minor key with a major V, cinematic rather than bluesy.
// - A sub bass that holds each root for the whole bar, far under everything.
// - Electric piano chords held under a lowpass, wide dark strings, and a vibraphone melody that leaves long gaps
//   for the echo.
blank("trip_hop");
title("Blue Hour Static");
tempo(88); key("G minor"); swing(0.6);
prog("A", "i VI iv V");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X......x..x.....");
drum("snare", "....X.......X...");
drum("hat", "x.o.x.o.x.o.x.o.");
mix("kick", #{ level: 1 });
mix("snare", #{ reverb: 0.35 });
mix("hat", #{ pan: 0.3 });
fx("kick", [#{ fx: "tape", wow: 0.1, flutter: 0.1, drive_db: 2, lowpass_hz: 8000 }]);
fx("snare", [#{ fx: "tape", wow: 0.1, flutter: 0.1, drive_db: 2, lowpass_hz: 8000 }, #{ fx: "vinyl", crackle: 0.25, hiss_db: -48 }]);

track("bass", chord("prog").root().struct("x").s("sub_bass").legato(1));
track("keys", chord("prog").voicing(50, 70).struct("x ~ ~ ~ ~ ~ ~ ~").s("epiano").legato(1).room(0.35).pan(-0.3).role("keys").lpf(3500).level(2));
track("strings", chord("prog").voicing(55, 76).s("strings").room(0.5).role("pad").pan(0.3));
fx("strings", [#{ fx: "width", width: 1.6 }]);
track("lead", n("<[4@3 2] [5@3 4] [3@2 2 3] [1@4]>").octave(4).s("mallet").legato(0.95).delay(0.4).room(0.45).level(2));

// quieter than a club track: trip hop is heard at home, late
master(#{ lufs: -12 });

form([
  sec("intro", 8, "keys strings").filter_in(),
  sec("verse", 16, "kick snare hat bass keys"),
  sec("hook", 16, "kick snare hat bass keys strings lead"),
  sec("break", 8, "keys strings"),
  sec("verse", 16, "kick snare hat bass keys"),
  sec("hook", 16, "kick snare hat bass keys strings lead"),
  sec("outro", 8, "keys strings"),
]);
