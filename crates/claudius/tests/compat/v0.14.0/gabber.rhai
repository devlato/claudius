// Glass Garden — gabber (seed 7)
// song("gabber", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(171); key("D minor"); swing(0.5);
humanize(0.02);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -7, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..x...x...x...x.");
drum("ohat", "main", "..o...x...x...x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("lead", #{ style: "riff", preset: "square_lead", register: [55, 74], density: 0.37864977, seed: -37300132614189792 });
part("chords", #{ style: "offbeat", preset: "supersaw", register: [52, 72], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 74], density: 0.5823303, seed: -7892568474930094457 });

form([
  sec("intro", 4, "kick hat").energy(0.64030194).variant("main"),
  sec("groove", 16, "kick clap hat ohat lead").energy(0.813739).fills(false),
  sec("break", 8, "pad chords lead").energy(0.3407866),
  sec("build", 4, "clap hat chords lead").energy(0.6711304).prog("B").gap(),
  sec("drop", 16, "kick clap hat ohat lead chords bass").name("peak").energy(0.9766603).impact(),
  sec("break", 4, "chords lead").name("break2").energy(0.3887422).prog("A"),
  sec("drop", 8, "kick clap hat ohat lead chords bass").name("peak2").energy(0.98664886).gap().impact(),
  sec("drop", 8, "kick clap hat ohat lead chords bass").name("peak3").energy(1),
  sec("outro", 4, "kick hat").energy(0.4537415).variant("main"),
]);

mix("clap", #{ reverb: 0.2 });
mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.1 });
mix("lead", #{ reverb: 0.22838272, delay: 0.29766843, duck: 0.3 });
mix("chords", #{ reverb: 0.43953097, duck: 0.6 });
mix("pad", #{ reverb: 0.4086404, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("kick", [
  #{ fx: "saturate", drive_db: 14, kind: "hard", mix: 0.85 },
  #{ fx: "filter", cutoff: 7000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
]);
fx("lead", [
  #{ fx: "saturate", drive_db: 8, kind: "hard", mix: 0.5 },
  #{ fx: "phaser", depth: 0.5090129, feedback: 0.20813528, mix: sections(0.32856825, #{ drop: 0.4385138, hook: 0.4385138 }), rate_hz: 0.36893344, stages: 8 },
]);
fx("chords", [#{ fx: "saturate", drive_db: 5, kind: "hard", mix: 0.4 }]);
fx("pad", [#{ fx: "phaser", depth: 0.6717282, feedback: 0.22995073, mix: sections(0.20815219, #{ drop: 0.37178797, hook: 0.37178797 }), rate_hz: 0.12636338, stages: 4 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "hard", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 5, kind: "hard", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 5, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 70, threshold_db: -10 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 180, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 100, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 3, kind: "hard", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.373905, lowcut: 328.14508, mix: 1, predelay_ms: 20.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });
