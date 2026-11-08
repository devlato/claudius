// "Afterlight Horizon": 2000s uplifting trance, the euphoric sound of the decade's superclubs (Tiësto, ATB, Paul van
// Dyk). An original song in that style.
//
// How it is made:
// - 138 BPM, four on the floor, and a rolling bass on the three sixteenths after every kick: the trance gallop.
// - A plucked arpeggio climbing through every chord carries the first half, so the energy rises before any melody.
// - i VI III VII (Am F C G), two bars each: minor, but every chord lifts.
// - The breakdown is the heart of it: the drums stop, a swelling pad and the anthem's melody alone, then a snare
//   roll builds it back.
// - The drop plays the melody on a supersaw over supersaw chords, everything ducking under the kick.
blank("trance");
title("Afterlight Horizon");
tempo(138); key("A minor"); swing(0.5);
prog("A", "i VI III VII");
bars_per_chord(2);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("ohat", "..x...x...x...x.");
drum("hat", "o.o.o.o.o.o.o.o.");
mix("ohat", #{ pan: -0.15 });
mix("hat", #{ pan: 0.2 });
track("roll", s("<sd*4 sd*4 sd*8 sd*8 sd*16 sd*16 sd*16 sd*16>")
                .gain("<0.35 0.4 0.5 0.55 0.65 0.75 0.85 1>").role("snare"));

track("bass", chord("prog").root().struct("[~ x x x]*4").s("saw_bass").legato(0.5).duck(0.8));
track("arp", chord("prog").voicing(60, 79).struct("x*4").arp("up").s("pluck").delay(0.25).pan(-0.2).duck(0.5).role("arp"));
track("pad", chord("prog").voicing(52, 72).s("swell_pad").room(0.4).role("pad"));
fx("pad", [#{ fx: "width", width: 1.5 }]);
track("chords", chord("prog").voicing(55, 74).s("supersaw").role("chords").duck(0.8).room(0.2));
track("lead", n("<[4 ~ 4 ~ 2 ~ 4 7] [7@3 6] [5 ~ 5 ~ 4 ~ 5 7] [7@2 9 8] [7 ~ 7 ~ 4 ~ 7 9] [9@3 8] [6 ~ 6 ~ 8 ~ 6 4] [4@4]>")
                .octave(4).s("supersaw").legato(0.9).duck(0.6).delay(0.2).room(0.3).role("lead"));

form([
  sec("intro", 16, "kick hat arp").filter_in(),
  sec("groove", 16, "kick clap ohat hat bass arp"),
  sec("break", 16, "pad lead"),
  sec("build", 8, "kick roll pad arp").build().gap(),
  sec("drop", 16, "kick clap ohat hat bass arp chords lead").impact(),
  sec("break", 8, "pad arp"),
  sec("build", 8, "kick roll pad arp lead").build().gap(),
  sec("drop", 16, "kick clap ohat hat bass arp chords lead").impact(),
  sec("outro", 16, "kick hat bass arp"),
]);
