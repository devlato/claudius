// Glass Garden — rage (seed 7)
// song("rage", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(151); key("D minor"); swing(0.5034423);
humanize(0.05);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -7.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X...............|X.........X.....|X...............");
drum("kick", "main", "X.........X.....|X..X......X.....|X.........X.....|X..X......X.....");
drum("snare", "main", "........X.......");
drum("hat", "alt", "x...x...XXxxxxxxx...x...x...x...|x...x...x...x...x...x...x...x...|x...x...x...x...x...x...XXxxxxxx|x...x...x...x...x...x...x...x...", 32);
drum("hat", "main", "x.....x.....x.....x.....x.....x.....X.X.x.x.x.x.|x.....x.....X.Xx.xx.xx.xXXXxxxxxxxxxx.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....", 48);
drum("ohat", "main", "................");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "glide", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("lead", #{ style: "trap_bells", preset: "supersaw", register: [60, 82], density: 0.4710492, seed: -6710496186308540014 });
part("arp", #{ style: "pinky", preset: "supersaw", register: [55, 79], density: 0.649105, rate: 0.25, seed: -1629742732023437997 });
part("pad", #{ style: "sustain", preset: "pad", register: [48, 72], density: 0.4087378, seed: 6104030045851553230 });

form([
  sec("intro", 8, "lead arp").energy(0.59030193).variant("main"),
  sec("hook", 8, "*").energy(0.91373897).impact(),
  sec("verse", 8, "kick snare hat bass lead").energy(0.7407866).variant("alt").gap(),
  sec("hook", 8, "*").name("hook2").energy(0.97113043).impact(),
  sec("break", 4, "arp pad").name("switch").energy(0.47666034).prog("A"),
  sec("hook", 8, "*").name("hook3").energy(0.98874223).gap().impact(),
  sec("hook", 8, "*").name("hook4").energy(0.98664886),
  sec("outro", 4, "lead arp").energy(0.4076398).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("lead", #{ level: -1, reverb: 0.3662758, delay: 0.18318664 });
mix("arp", #{ level: -5, reverb: 0.24518426, delay: 0.31054208 });
mix("pad", #{ level: -6, reverb: 0.54785424 });
mix("fx", #{ level: -3, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 10, kind: "hard", mix: 1 },
  #{ fx: "filter", cutoff: 5500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("lead", [
  #{ fx: "saturate", drive_db: 6, kind: "hard", mix: 0.5 },
  #{ fx: "width", mono_below_hz: 200, width: 1.5 },
  #{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 },
]);
fx("arp", [
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.4 },
  #{ fx: "auto_pan", beats: 2, depth: 0.3342294 },
]);
fx("pad", []);
bus("bass", [#{ fx: "saturate", drive_db: 5, kind: "tanh", mix: 0.7 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.5 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 2, mix: 1, ratio: 2.5, release_ms: 80, threshold_db: -9 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 110, threshold_db: -8 },
  #{ fx: "saturate", drive_db: 2.5, kind: "hard", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 1.7264763, lowcut: 281.2672, mix: 1, predelay_ms: 17.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });
