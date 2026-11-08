// "Sunday Lowrider": 1990s G-funk, the laid-back West Coast hip hop of the early part of the decade (Dr. Dre, Snoop
// Dogg, Warren G). An original song in that style.
//
// How it is made:
// - 94 BPM and loose: a swung beat with a clap doubling the snare, made for driving slowly with the windows down.
// - Dorian funk: two chords, Gm7 and C7, a vamp from 70s funk records slowed down and replayed rather than sampled.
// - A rubbery synth bass that slides between roots and fifths, played like a funk bassist would.
// - The signature: a high, whining synth lead that glides from note to note, sitting far above everything.
// - Electric piano chords and soft strings fill the middle without crowding the voice's range.
blank("g_funk");
title("Sunday Lowrider");
tempo(94); key("G dorian"); swing(0.58);
prog("A", "i7 i7 IV7 IV7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X......x.x......");
drum("snare", "....X.......X...");
drum("clap", "....x.......x...");
drum("hat", "xoxoxoxoxoxoxoxo");
mix("clap", #{ pan: 0.15, level: -2 });
mix("hat", #{ pan: 0.25 });

track("bass", note("<[g1 ~ ~ g1 ~ d2 ~ f2] [g1 ~ ~ g1 ~ bb1 c2 d2] [c2 ~ ~ c2 ~ g2 ~ bb2] [c2 ~ ~ c2 ~ e2 f2 f#2]>")
                .s("moog_bass").legato(0.7));
track("keys", chord("prog").voicing(53, 70).struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.8).pan(-0.25).role("keys"));
track("whine", n("<[7@2 6 4] [6@3 ~] [3@2 4 5] [4@4]>").octave(4).s("glide_lead").legato(1).delay(0.25).room(0.3).role("lead"));
track("strings", chord("prog").voicing(55, 74).s("strings").room(0.35).role("pad").level(-3).pan(0.2));
fx("strings", [#{ fx: "width", width: 1.4 }]);

form([
  sec("intro", 8, "keys strings whine").filter_in(),
  sec("verse", 16, "kick snare clap hat bass keys"),
  sec("hook", 8, "kick snare clap hat bass keys strings whine"),
  sec("verse", 16, "kick snare clap hat bass keys"),
  sec("hook", 8, "kick snare clap hat bass keys strings whine"),
  sec("verse", 16, "kick snare clap hat bass keys strings"),
  sec("hook", 8, "kick snare clap hat bass keys strings whine"),
  sec("outro", 8, "keys bass whine"),
]);
