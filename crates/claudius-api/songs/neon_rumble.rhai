// "Neon Rumble": 2020s bass house, the growling, bass-led club music of the decade's festivals (Jauz, Joyryde,
// Habstrakt). An original song in that style, built to show a layered sound: its bass is one line played by three.
//
// How it is made:
// - House at 126 BPM: a kick on every beat, a clap on 2 and 4, offbeat hats, shakers and a looping rimshot.
// - The bass is the hook, and it is a rack of three sounds playing the same notes, each with one job:
// - a clean sine sub under 90 Hz for the weight, which alone ducks under the kick, so the kick and the bass never
//   fight while the growl above holds;
// - a reese from 90 Hz to 1.2 kHz, distorted hard and filtered: the growl you hear on a phone;
// - a saw an octave up over 1.2 kHz, chopped by a gate into a sixteenth-note stutter, for the edge.
// - Each layer keeps its own band, through crossovers that add back up to the whole, and the rack sums them before
//   the bass's own effects, so it is still one track and one fader.
// - Few chords (Fm Fm Db Eb) on short stabs, and a pad only in the breakdown, so the bass has the room.
blank("tech_house");
title("Neon Rumble");
tempo(126); key("F minor"); swing(0.52);
prog("A", "i i VI VII");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("hat", "..x...x...x...xo");
drum("shaker", "xoxoxoxoxoxoxoxo");
drum("perc", "...o.....o....x.");
mix("hat", #{ pan: 0.15 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, delay: 0.1 });

// one bass line, sixteen steps a bar, the turn at the end of four bars leading home
track("bass", note("<[f1 ~ ~ f1 ~ f1 ~ ~ f1 ~ ~ f2 ~ f1 ~ ~] [f1 ~ ~ f1 ~ f1 ~ ~ f1 ~ ab1 ~ f1 ~ c2 ~] [db1 ~ ~ db1 ~ db1 ~ ~ db1 ~ ~ db2 ~ db1 ~ ~] [eb1 ~ ~ eb1 ~ eb1 ~ ~ eb2 ~ ~ eb1 ~ g1 ~ ~]>")
                .s("sub_bass").legato(0.55));
// and the three sounds that play it
rack("bass", [
  #{ sound: "sub_bass", band: [0, 90], duck: 0.85 },
  #{ sound: "reese", band: [90, 1200], level: -1, fx: [
    #{ fx: "saturate", kind: "hard", drive_db: 14, mix: 1 },
    #{ fx: "filter", mode: "lp", cutoff: 1800, res: 0.2 },
  ] },
  #{ sound: "saw_lead", band: [1200, 0], level: -9, transpose: 12, fx: [
    #{ fx: "gate", steps: [1, 0, 1, 1, 0, 1, 0, 1], step_beats: 0.25, depth: 0.9, smooth_ms: 4 },
  ] },
]);
track("stab", chord("prog").voicing(53, 72).struct("~ ~ ~ x ~ ~ x ~ ~ ~ ~ ~ ~ ~ ~ ~").s("stab").legato(0.3).room(0.2).delay(0.2).duck(0.4).role("chords"));
fx("stab", [#{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
track("pad", chord("prog").voicing(48, 72).s("warm_pad").room(0.4).duck(0.5).role("pad").lpf(2400));
fx("pad", [#{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
track("roll", s("<sd*4 sd*4 sd*8 sd*8 sd*16 sd*16 sd*16 sd*16>")
                .gain("<0.35 0.4 0.5 0.55 0.65 0.75 0.85 1>").role("snare"));

form([
  sec("intro", 16, "kick hat shaker perc").filter_in(),
  sec("groove", 16, "kick clap hat shaker perc bass"),
  sec("drop", 16, "kick clap hat shaker perc bass stab").impact(),
  sec("break", 16, "shaker pad stab"),
  sec("build", 8, "kick shaker roll bass").build().gap(),
  sec("drop", 16, "kick clap hat shaker perc bass stab").impact(),
  sec("outro", 16, "kick hat shaker perc"),
]);
