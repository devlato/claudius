// "Joy Machine": Beethoven's Ode to Joy (from his Ninth Symphony, 1824) as 2010s festival EDM. The melody is in the
// public domain; this is an original arrangement of it, in the style of the decade's main-stage anthems.
//
// How it is made:
// - The theme as Beethoven wrote it, in D major, eight bars: stepwise, almost all on the beat, and so easy to sing
//   that a festival crowd can shout it back.
// - Reharmonized the EDM way, I V vi V I V vi I (D A Bm A D A Bm D): the minor sixth chord in the third bar gives
//   the old tune the bittersweet lift of a modern anthem.
// - Stated first on a piano, alone, in the breakdown; then a snare roll, a riser and a beat of silence; then the
//   drop plays it on a supersaw over pumping chords, a bass on every offbeat and four on the floor.
blank("electro_house");
title("Joy Machine");
tempo(128); key("D major"); swing(0.5);
prog("A", "I V vi V I V vi I");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("ohat", "..x...x...x...x.");
mix("ohat", #{ pan: -0.15 });
track("roll", s("<sd*4 sd*4 sd*8 sd*8 sd*16 sd*16 sd*16 sd*16>")
                .gain("<0.35 0.4 0.5 0.55 0.65 0.75 0.85 1>").role("snare"));

// the theme: degrees of D major (0 D, 1 E, 2 F#, 3 G, 4 A), a quarter note each, the long notes held
let ode = "<[2 2 3 4] [4 3 2 1] [0 0 1 2] [2@3 1 1@4] [2 2 3 4] [4 3 2 1] [0 0 1 2] [1@3 0 0@4]>";
track("piano", n(ode).octave(4).s("keys").legato(0.9).room(0.35).role("keys"));
track("lead", n(ode).octave(4).s("supersaw").legato(0.95).duck(0.6).room(0.25).delay(0.1).role("lead"));
track("chords", chord("prog").voicing(50, 69).s("supersaw").legato(1).duck(0.8).role("chords").level(-2));
track("bass", chord("prog").root().struct("~ x ~ x ~ x ~ x").s("saw_bass").legato(0.5).duck(0.7));
track("pad", chord("prog").voicing(52, 72).s("warm_pad").role("pad").room(0.4));
fx("pad", [#{ fx: "width", width: 1.5 }]);

form([
  sec("intro", 16, "kick ohat bass").filter_in(),
  sec("break", 16, "piano pad"),
  sec("build", 8, "kick roll pad piano").build().gap(),
  sec("drop", 16, "kick clap ohat bass chords lead").impact(),
  sec("break", 8, "pad piano"),
  sec("build", 8, "kick roll pad lead").build().gap(),
  sec("drop", 16, "kick clap ohat bass chords lead").impact(),
  sec("outro", 8, "kick ohat bass"),
]);
