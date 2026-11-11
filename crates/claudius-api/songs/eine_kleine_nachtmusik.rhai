// "Eine kleine Nachtmusik": the opening of Mozart's Eine kleine Nachtmusik (Serenade No. 13, K. 525, 1787) as
// synth-pop. The music is in the public domain; this is an original arrangement of it, in the style of the synth-pop
// of the 2020s.
//
// How it is made:
// - Mozart's theme, note for note: G and D, G and D, then up the G chord to the D held; C and A, C and A, then down by
//   F sharp to the D held. A soft square plays it in the verses, a bright saw an octave up in the hooks.
// - 108 BPM, the beat of 80s synth-pop: a kick on one and before three, a big reverbed snare on two and four, the hats
//   in eighths, a tom fill into each hook.
// - G and D, the theme's own two chords, a bar each twice; a saw bass pulsing in eighths and a string machine holding
//   the chords, pumped by the kick, and a plucked arpeggio up through the chords in the hooks.
// - Intro, verse, build, hook, twice, a break of the pad and the theme alone, the last hook twice, and the arpeggio to
//   end.
blank("synthwave");
title("Eine kleine Nachtmusik");
tempo(108); key("G major"); swing(0.5);
prog("A", "I I V V");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X.....x.X.......");
drum("snare", "....X.......X...");
drum("hat", "x.x.x.x.x.x.x.x.");
drum("tom", "main", "................|................|................|........x.x.xxxx", 16);
mix("snare", #{ reverb: 0.45 });
mix("hat", #{ pan: 0.2, level: 3 });

// Mozart's theme, sixteen steps a bar: G D G D, up the chord to D, C A C A, down by F sharp to D
let mozart = "<[g4@4 ~@2 d4@2 g4@4 ~@2 d4@2] [g4@2 d4@2 g4@2 b4@2 d5@8] [c5@4 ~@2 a4@2 c5@4 ~@2 a4@2] [c5@2 a4@2 fs4@2 a4@2 d4@8]>";
track("theme", note(mozart).s("square_lead").legato(0.8).lpf(2400).room(0.3).role("lead"));
track("hook", note(mozart).add(12).s("saw_lead").legato(0.8).lpf(3200).delay(0.2).room(0.3).role("lead"));
track("bass", chord("prog").root().struct("x*8").s("saw_bass").legato(0.5).duck(0.35).role("bass"));
track("pad", chord("prog").voicing(52, 71).s("solina").room(0.4).duck(0.3).role("pad"));
fx("pad", [#{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
track("arp", chord("prog").voicing(59, 79).arp("up").struct("x*16").s("pluck").legato(0.4).delay(0.15).pan(-0.25)
               .role("arp"));

form([
  sec("intro", 4, "pad theme").filter_in(),
  sec("verse", 8, "kick snare hat bass pad theme"),
  sec("build", 4, "kick snare hat bass pad tom").build(),
  sec("hook", 8, "kick snare hat bass pad arp hook").impact(),
  sec("verse", 8, "kick snare hat bass pad theme"),
  sec("build", 4, "kick snare hat bass pad tom").build(),
  sec("hook", 8, "kick snare hat bass pad arp hook").impact(),
  sec("break", 8, "pad theme"),
  sec("hook", 8, "kick snare hat bass pad arp hook").impact(),
  sec("hook", 8, "kick snare hat bass pad arp hook theme"),
  sec("outro", 4, "pad arp"),
]);
