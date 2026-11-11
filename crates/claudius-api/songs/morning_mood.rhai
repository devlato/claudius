// "Morning Mood": Grieg's Morning Mood (from Peer Gynt, 1875) as slow-building electronic pop. The music is in the
// public domain; this is an original arrangement of it, in the style of the electronic pop of the 2020s.
//
// How it is made:
// - Grieg's flute tune, note for note: down the E major pentatonic from B, B G sharp F sharp E, and back up to G sharp,
//   twice; then up to the C sharp and down to E. Its six notes a bar are laid in eighths across a bar of four, the last
//   held. A breathy, whistled lead plays it, as the flute does, an octave up.
// - 110 BPM, deep house that builds: a kick on every beat, a finger snap on two and four, a rim and offbeat hats, a
//   shaker as it grows.
// - E E A E under the tune, a bar each: a plucked bass bouncing on the roots, a warm pad and an electric piano holding
//   the chords, pumped by the kick.
// - It builds by adding: the pad and the bass alone, then the beat, then a bright arpeggio whose filter opens over the
//   hooks, and a second, airy pad at the top; the climax is the tune over all of it.
blank("deep_house");
title("Morning Mood");
tempo(110); key("E major"); swing(0.52);
prog("A", "I I IV I");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X...X...X...X...");
drum("snap", "....X.......X...");
drum("rim", "..x....x..x..x..");
drum("hat", "..x...x...x...x.");
drum("shaker", "xoxoxoxoxoxoxoxo");
mix("snap", #{ reverb: 0.3 });
mix("rim", #{ pan: 0.35, level: -4 });
mix("hat", #{ pan: -0.2, level: 1 });
mix("shaker", #{ pan: 0.25 });

// Grieg's tune, eight steps a bar: B G sharp F sharp E F sharp G sharp twice, up to C sharp, and down to E
let morning = "<[b4 gs4 fs4 e4 fs4 gs4@3] [b4 gs4 fs4 e4 fs4 gs4@3] [fs4 gs4 b4 gs4 b4 cs5@3] [gs4 cs5 b4 gs4 fs4 e4@3]>";
track("flute", note(morning).add(12).s("whistle").legato(0.95).room(0.4).delay(0.2).role("lead"));
track("bass", note("<[e2 ~ e2 e3 ~ e2 ~ b1 e2 ~ ~ e2 gs2 ~ b2 ~] [e2 ~ e2 e3 ~ e2 ~ b1 e2 ~ e2 ~ fs2 ~ gs2 ~] [a1 ~ a1 a2 ~ a1 ~ e1 a1 ~ ~ a1 cs2 ~ e2 ~] [e2 ~ e2 e3 ~ e2 ~ b1 e2 ~ e2 ~ ds2 ~ b1 ~]>")
                .s("pluck_bass").legato(0.35).role("bass"));
track("pad", chord("prog").voicing(52, 71).s("warm_pad").duck(0.7).room(0.35).role("pad"));
track("ep", chord("prog").voicing(55, 74).struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.6).duck(0.4).role("keys"));
track("arp", chord("prog").voicing(60, 84).arp("up").struct("x*16").s("supersaw").legato(0.4).role("arp").level(-4));
track("pad2", chord("prog").voicing(64, 84).s("air_pad").duck(0.6).room(0.5).role("pad"));
fx("pad2", [#{ fx: "width", width: 1.7, mono_below_hz: 200 }]);

form([
  sec("intro", 8, "pad bass").filter_in(),
  sec("verse", 16, "kick snap pad ep bass flute"),
  sec("hook", 16, "kick snap hat pad ep bass arp flute"),
  sec("verse", 16, "kick snap rim pad ep bass flute"),
  sec("hook", 16, "kick snap rim hat shaker pad ep bass arp pad2 flute").impact(),
  sec("groove", 16, "kick snap rim hat shaker pad ep bass arp pad2 flute").name("climax"),
  sec("outro", 16, "kick snap rim hat shaker pad ep bass arp pad2"),
]);
fx("arp", [#{ fx: "filter", mode: "lp", res: 0.25,
              cutoff: sections(500, #{ hook: [500, 7000], climax: 7000, outro: [7000, 1500] }) }]);
