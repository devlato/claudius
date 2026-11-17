// Glass Garden — amapiano (seed 7)
// song("amapiano", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(110); key("D minor"); swing(0.55032694);
humanize(0.25);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 3 });
drum("shaker", "main", "o.o..oo.o.o.ooo.");
lane("shaker", #{ late: -6 });
drum("ohat", "main", "..o...o...x...x.");
drum("rim", "main", "...x..........x.");
lane("rim", #{ late: 3 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("logdrum", #{ style: "log_drum", preset: "log_drum", register: [33, 52], density: 0.5726087, seed: 2642474102756894640 });
part("keys", #{ style: "gospel", preset: "keys", register: [52, 76], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "afro_flute", preset: "mallet", register: [64, 86], density: 0.36008075, seed: -1629742732023437997 });

form([
  sec("intro", 4, "keys shaker ohat").energy(0.4903019).variant("main"),
  sec("groove", 8, "kick clap shaker ohat rim keys logdrum").energy(0.61373895).fills(false),
  sec("break", 4, "keys pad lead shaker").energy(0.3407866).build(true),
  sec("drop", 16, "kick clap shaker ohat rim keys logdrum lead pad").energy(0.9211304).impact(),
  sec("break", 8, "keys pad shaker ohat").name("break2").energy(0.37666035).build(true).gap(),
  sec("drop", 16, "kick clap shaker ohat rim keys logdrum lead pad").name("drop2").energy(0.98874223).impact(),
  sec("drop", 16, "kick clap shaker ohat rim keys logdrum lead pad").name("drop3").energy(0.98664886),
  sec("outro", 4, "kick shaker ohat keys").energy(0.3576398).variant("main"),
]);

mix("clap", #{ reverb: 0.2 });
mix("shaker", #{ pan: -0.3 });
mix("ohat", #{ pan: 0.2 });
mix("rim", #{ pan: 0.35, reverb: 0.15 });
mix("logdrum", #{ duck: 0.2 });
mix("keys", #{ reverb: 0.43953097, delay: 0.122124426 });
mix("lead", #{ reverb: 0.2860483, delay: 0.4658131 });
mix("fx", #{ level: -4, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("logdrum", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [
  #{ fx: "width", mono_below_hz: 200, width: 1.3 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 100, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 160, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 2.373905, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.29412478, mix: 1, pingpong: true } });
