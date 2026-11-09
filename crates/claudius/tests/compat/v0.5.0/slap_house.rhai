// Glass Garden — slap_house (seed 7)
// song("slap_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(120); key("D minor"); swing(0.5034423);
humanize(0.04);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(1);
master(#{ lufs: -8, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...o...x.");
drum("shaker", "main", "..o.ooo.oo....oo");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "pluck_bass", register: [36, 55], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "stabs", preset: "stab", register: [55, 74], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "synth_lead", preset: "harp_pluck", register: [57, 76], density: 0.51008075, seed: -1629742732023437997 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 72], density: 0.4087378, seed: 6104030045851553230 });

form([
  sec("intro", 8, "chords lead hat").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick hat shaker chords lead").energy(0.563739).variant("sparse").fills(true),
  sec("build", 4, "clap hat chords lead").energy(0.59078664).gap(),
  sec("drop", 8, "kick clap hat ohat shaker bass chords lead").energy(0.97113043).impact(),
  sec("verse", 8, "kick hat shaker chords lead pad").name("verse2").energy(0.57666034).prog("B").variant("sparse"),
  sec("build", 4, "clap hat chords lead pad").name("build2").energy(0.6387422).gap(),
  sec("drop", 12, "*").name("drop2").energy(0.98664886).impact(),
  sec("drop", 12, "*").name("drop3").energy(1),
  sec("outro", 8, "kick hat chords").energy(0.3537415).variant("main"),
]);

mix("clap", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: 0.3 });
mix("bass", #{ duck: 0.9 });
mix("chords", #{ reverb: 0.43953097, delay: 0.18318664, duck: 0.6 });
mix("lead", #{ reverb: 0.24518426, delay: 0.31054208, duck: 0.4 });
mix("pad", #{ reverb: 0.54785424, duck: 0.5 });
mix("fx", #{ level: -3, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "fold", mix: 0.3 },
  #{ fx: "filter", cutoff: 3500, drive: 1, mix: 1, mode: "lp", res: 0.25 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("chords", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", []);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 6, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 80, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 110, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.1580954, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });
