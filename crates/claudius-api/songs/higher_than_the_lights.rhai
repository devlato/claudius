// "Higher Than the Lights": 2010s festival EDM, the progressive and big-room house that filled festival main
// stages around 2012 (Avicii, Swedish House Mafia, Martin Garrix). An original song in that style.
//
// How it is made:
// - vi IV I V (Am F C G) at 128 BPM: the four chords under half the decade's anthems, sad and euphoric at once.
// - A piano states the chords in the breakdown, so the drop is a return to something the crowd already knows.
// - The build: a snare roll doubling every two bars, a riser and a beat of silence before the drop.
// - The drop: a supersaw lead playing the hook, supersaw chords under it, a bass on every offbeat, and everything
//   but the drums ducking under the kick, which makes the whole mix pump.
blank("electro_house");
title("Higher Than the Lights");
tempo(128); key("C major"); swing(0.5);
prog("A", "vi IV I V");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("ohat", "..x...x...x...x.");
drum("hat", "o.o.o.o.o.o.o.o.");
mix("ohat", #{ pan: -0.15 });
mix("hat", #{ pan: 0.2 });
// the build's roll, doubling every two bars, as a pattern: one bar of it at a time, louder each bar
track("roll", s("<sd*4 sd*4 sd*8 sd*8 sd*16 sd*16 sd*16 sd*16>")
                .gain("<0.35 0.4 0.5 0.55 0.65 0.75 0.85 1>").role("snare"));

track("piano", chord("prog").voicing(55, 74).struct("x ~ ~ x ~ ~ x ~").s("house_piano").legato(0.6).room(0.3).role("keys"));
track("bass", chord("prog").root().struct("~ x ~ x ~ x ~ x").s("saw_bass").legato(0.5).duck(0.7));
track("chords", chord("prog").voicing(55, 72).s("supersaw").role("chords").duck(0.8).room(0.15));
track("lead", n("<[9 9 ~ 8 7 ~ 5 ~] [7 ~ 5 7 ~ 10 9 ~] [9 ~ 8 7 ~ 4 ~ ~] [8 ~ 6 8 ~ 11 10 9]>")
                .octave(4).s("supersaw").legato(0.85).duck(0.6).room(0.25).delay(0.15));

form([
  sec("intro", 16, "kick hat piano").filter_in(),
  sec("break", 16, "piano chords"),
  sec("build", 8, "kick roll piano chords").build().gap(),
  sec("drop", 16, "kick clap ohat hat bass chords lead").impact(),
  sec("break", 8, "piano"),
  sec("build", 8, "kick roll piano chords lead").build().gap(),
  sec("drop", 16, "kick clap ohat hat bass chords lead").impact(),
  sec("outro", 8, "kick hat piano"),
]);
