// "Chihiro Study": a production study of Billie Eilish's CHIHIRO (2024), the slow-building electronic song from Hit
// Me Hard and Soft. The record's tempo, key, groove and sound, with an original melody and bass line: no note of the
// record's.
//
// How it is made:
// - 110 BPM: a kick on every beat and a finger snap on 2 and 4 is the whole beat at first; a woodblock-like rim joins
//   it halfway, and hats and a shaker open the top up in the choruses.
// - E minor round i VI III VII (Em C G D), two bars a chord, so the song breathes.
// - A palm-muted, funky bass in sixteenths carries the groove from the first bar to the last, under a pad that pumps
//   with the kick and an electric piano doubling its chords.
// - An arpeggio climbs through the chords in each chorus, its filter opening across the section the way the record
//   swells.
// - The song keeps adding: a second pad, a lower arpeggio, and a long last section where everything plays.
blank("deep_house");
title("Chihiro Study");
tempo(110); key("E minor"); swing(0.52);
prog("A", "i VI III VII");
bars_per_chord(2);

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

// the bass, palm-muted sixteenths round each chord, two bars a chord
track("bass", note("<[e2 ~ e2 e3 ~ e2 ~ d2 e2 ~ ~ e2 g2 ~ a2 ~] [e2 ~ e2 e3 ~ e2 ~ b1 d2 ~ e2 ~ g2 ~ fs2 ~] [c2 ~ c2 c3 ~ c2 ~ b1 c2 ~ ~ c2 e2 ~ g2 ~] [c2 ~ c2 c3 ~ c2 ~ g1 b1 ~ c2 ~ d2 ~ e2 ~] [g1 ~ g1 g2 ~ g1 ~ fs1 g1 ~ ~ g1 b1 ~ d2 ~] [g1 ~ g1 g2 ~ g1 ~ d1 fs1 ~ g1 ~ a1 ~ b1 ~] [d2 ~ d2 d3 ~ d2 ~ c2 d2 ~ ~ d2 fs2 ~ a2 ~] [d2 ~ d2 d3 ~ d2 ~ a1 c2 ~ d2 ~ e2 ~ fs2 ~]>")
                .s("pluck_bass").legato(0.35).role("bass"));
track("pad", chord("prog").voicing(52, 71).s("warm_pad").duck(0.7).room(0.35).role("pad"));
track("ep", chord("prog").voicing(55, 74).struct("x ~ ~ x ~ ~ x ~").s("epiano").legato(0.6).duck(0.4).role("keys"));
track("lead", n("<[4@2 3 2@3 0@2] [~@8] [2@2 4 5@3 4@2] [~@8] [4@2 6 7@3 6@2] [~@8] [5@2 4 3@3 1@2] [~@8]>").octave(4)
                .s("vox_lead").legato(0.9).room(0.4).delay(0.25).role("lead"));
track("arp", chord("prog").voicing(60, 84).arp("up").struct("x*16").s("supersaw").legato(0.4).role("arp").level(-4));
track("arp_low", chord("prog").voicing(48, 64).arp("up").struct("x*8").s("pluck").legato(0.5).role("arp"));
track("pad2", chord("prog").voicing(64, 84).s("air_pad").duck(0.6).room(0.5).role("pad"));
fx("pad2", [#{ fx: "width", width: 1.7, mono_below_hz: 200 }]);

form([
  sec("intro", 8, "pad bass").filter_in(),
  sec("verse", 16, "kick snap pad ep bass lead"),
  sec("hook", 16, "kick snap hat pad ep bass arp lead"),
  sec("verse", 16, "kick snap rim pad ep bass lead"),
  sec("hook", 16, "kick snap rim hat shaker pad ep bass arp pad2 lead").impact(),
  sec("groove", 16, "kick snap rim hat shaker pad ep bass arp arp_low pad2").name("climax"),
  sec("outro", 16, "kick snap rim hat shaker pad ep bass arp arp_low pad2"),
]);
// the arpeggio's filter opens across each chorus, holds open through the climax and closes across the outro
fx("arp", [#{ fx: "filter", mode: "lp", res: 0.25,
              cutoff: sections(500, #{ hook: [500, 7000], climax: 7000, outro: [7000, 1500] }) }]);
