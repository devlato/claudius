// Glass Garden — trap (seed 7)
// song("trap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(131); key("D minor"); swing(0.50688463);
humanize(0.15);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X......X........|X.........X.....|X......X........");
drum("kick", "main", "X......X..X.....|X..X............|X......X........|X.....X.........");
drum("snare", "main", "........X...o...");
drum("clap", "main", "........X.......");
drum("hat", "alt", "x.....x.....X.Xx.xx.xx.xx.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....ooxxxxxxxxXXx.....x.....x.....x.....", 48);
drum("hat", "main", "x.....x..x..x..x..x.....x..x..x..x..X.Xx.xx.xx.x|x.....x..x..x..x..x.....x..x..x..x..x.....x..x..|x.....x..x..x..x..x.....x..x..x..x..ooxxxxxxxxXX|x.....x..x..x..x..x.....x..x..x..x..x.....x..x..", 48);
drum("ohat", "main", "................");
drum("perc", "main", "x.....x...x.....");
drum("perc", "sparse", "..........x.....");
drum_sound("perc", "snap");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "808", register: [28, 43], density: 0.5726087, seed: 2642474102756894640 });
part("keys", #{ style: "half", preset: "keys", register: [55, 76], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "motif", preset: "whistle", register: [67, 88], density: 0.46008074, seed: -1629742732023437997 });

form([
  sec("intro", 8, "keys pad hat").energy(0.4903019).variant("main"),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").energy(0.86373895).fills(false).impact(),
  sec("verse", 8, "kick snare hat perc bass keys").energy(0.6407866).variant("alt").fills(true).gap(),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").name("hook2").energy(0.9211304).fills(false).impact(),
  sec("verse", 4, "kick snare hat bass keys pad").name("verse2").energy(0.57666034).variant("alt"),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").name("hook3").energy(0.98874223).fills(false).gap().impact(),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").name("hook4").energy(0.98664886).fills(false),
  sec("outro", 4, "keys pad lead").energy(0.3576398).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("clap", #{ level: -4, reverb: 0.15 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.4, reverb: 0.25 });
mix("keys", #{ reverb: 0.43953097, delay: 0.122124426 });
mix("lead", #{ reverb: 0.24518426, delay: 0.38817757 });
mix("fx", #{ level: -2, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.5 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 90, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 150, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.9422858, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });
