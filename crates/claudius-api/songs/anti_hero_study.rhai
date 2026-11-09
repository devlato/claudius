// "Anti-Hero Study": a production study of Taylor Swift's Anti-Hero (2022), the Jack Antonoff synth-pop single from
// Midnights. The record's tempo, key, chords, groove and sound, with an original melody and bass line: no note of the
// record's.
//
// How it is made:
// - 97 BPM on a drum machine: a kick on 1 and on the and of 2, a snare on 2 and 4 in a long, bright reverb, the gated
//   snare of the 1980s, and eighth-note hats.
// - E major round IV I V vi (A E B C#m7): the progression starts on the four and never lands on the tonic first, so
//   every bar floats.
// - A synth bass pulsing in eighths on the roots, ducking a little under the kick, and a string machine holding each
//   chord.
// - A soft synth hook between the lines of the verse, and an arpeggio that comes in for the chorus, both on 1980s
//   synth sounds; a tom fill closes each pre-chorus.
// - Verse, pre-chorus, chorus twice, then a quieter bridge on the same chords and the chorus to end.
blank("synthwave");
title("Anti-Hero Study");
tempo(97); key("E major"); swing(0.5);
prog("A", "A E B C#m7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X.....x.X.......");
drum("snare", "....X.......X...");
drum("hat", "x.x.x.x.x.x.x.x.");
drum("tom", "main", "................|................|................|........x.x.xxxx", 16);
mix("snare", #{ reverb: 0.45 });
mix("hat", #{ pan: 0.2, level: 3 });

track("bass", chord("prog").root().struct("x*8").s("saw_bass").legato(0.5).duck(0.35).role("bass"));
track("pad", chord("prog").voicing(52, 71).s("solina").room(0.4).duck(0.3).role("pad"));
fx("pad", [#{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
track("hook", n("<[~ ~ 5 4 5 ~ 3 ~] [~ ~ 4 5 7 ~ 4 ~] [~ ~ 4 6 4 ~ 1 ~] [~ ~ 2 4 2 ~ 0 ~]>")
                .octave(4).s("saw_lead").legato(0.7).lpf(2600).delay(0.2).room(0.3).role("lead"));
track("arp", chord("prog").voicing(59, 79).arp("up").struct("x*16").s("pluck").legato(0.4).delay(0.15).pan(-0.25)
               .role("arp"));

form([
  sec("intro", 4, "pad hook").filter_in(),
  sec("verse", 8, "kick snare hat bass pad hook"),
  sec("build", 4, "kick snare hat bass pad tom").build(),
  sec("hook", 8, "kick snare hat bass pad arp").impact(),
  sec("verse", 8, "kick snare hat bass pad hook"),
  sec("build", 4, "kick snare hat bass pad tom").build(),
  sec("hook", 8, "kick snare hat bass pad arp").impact(),
  sec("break", 8, "pad hook"),
  sec("hook", 8, "kick snare hat bass pad arp hook").impact(),
  sec("outro", 4, "pad arp"),
]);
