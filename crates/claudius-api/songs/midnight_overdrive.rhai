// "Midnight Overdrive": 2020s synth-pop, the 80s revival that topped the charts at the start of the decade
// (The Weeknd, Dua Lipa, M83's echo in it). An original song in that style: none of its notes come from a record.
//
// How it is made:
// - Fast and straight: 168 BPM with no swing, a drum machine's kick on 1 and 3 and a snare on 2 and 4, so it runs
//   at a sprint while the chords change only once a bar.
// - The snare's gated reverb, the sound of 1984: a big room that a gate cuts off an eighth after each hit.
// - i VI III VII (Cm Ab Eb Bb), the loop under most of the revival's choruses: minor, but it climbs.
// - A bass in octaves on eighths, and an arpeggio in sixteenths over every chord, which keep it moving under a
//   melody that leaves room.
// - The hook is a synth, not a voice: a bright saw lead an octave above the arpeggio, in the choruses only.
blank("synthwave");
title("Midnight Overdrive");
tempo(168); key("C minor"); swing(0.5);
prog("A", "i VI III VII");
prog("B", "VI VII i i");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.......x.......");
drum("snare", "....X.......X...");
drum("hat", "o.x.o.x.o.x.o.x.");
mix("snare", #{ level: 1 });
mix("hat", #{ pan: 0.2 });
fx("snare", [
  #{ fx: "reverb", decay: 1.6, predelay_ms: 4, mix: 0.45 },
  #{ fx: "gate", steps: [1, 0], step_beats: 0.5, depth: 1, smooth_ms: 6 },
]);
// a tom fill down the kit at the end of every fourth bar
track("tom", s("<~ ~ ~ [~ ~ ~ [lt mt ht ~]]>").level(-2));

track("bass", note("<[c2 c3]*4 [ab1 ab2]*4 [eb2 eb3]*4 [bb1 bb2]*4>").s("moog_bass").legato(0.6));
track("arp", chord("prog").voicing(60, 76).struct("x*4").arp("up").s("pluck").role("arp").delay(0.2).pan(-0.2));
track("pad", chord("prog").voicing(55, 72).s("solina").role("pad").room(0.3));
track("lead", n("<[4 ~ 4 6 4 3 2 ~] [7 ~ 7 6 5 ~ 2 ~] [6 ~ 4 6 9 ~ 8 ~] [8 ~ 7 6 3 ~ ~ ~]>")
                .octave(4).s("saw_lead").legato(0.8).room(0.2).delay(0.15));

master(#{ lufs: -11 });

form([
  sec("intro", 8, "arp pad").filter_in(),
  sec("verse", 16, "kick snare hat bass arp"),
  sec("build", 8, "kick snare hat bass arp pad tom").build(),
  sec("hook", 16, "kick snare hat bass arp pad lead").impact(),
  sec("verse", 16, "kick snare hat bass arp pad"),
  sec("build", 8, "kick snare hat bass arp pad tom").build(),
  sec("hook", 16, "kick snare hat bass arp pad lead").impact(),
  sec("break", 8, "arp pad").prog("B"),
  sec("build", 8, "kick snare hat bass arp pad tom").build(),
  sec("hook", 16, "kick snare hat bass arp pad lead").impact(),
  sec("outro", 8, "arp pad bass"),
]);
