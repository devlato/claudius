// Glass Garden — filter_house (seed 7)
// song("filter_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(118); key("D dorian"); swing(0.5086058);
humanize(0.15);
prog("A", "i7 IV9 i7 VIImaj7");
prog("B", "i7 VIImaj7 IV7 IV7");
bars_per_chord(1);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "x.xx.xx...x...xx");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...x...x.");
drum("shaker", "main", "ooo.oo....ooooo.");
drum("perc", "main", "x.....x...x.....");
drum("perc", "sparse", "..........x.....");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("loop", #{ style: "stabs", preset: "tine_piano", register: [52, 74], density: 0.4411646, seed: 1065738501430591281 });
part("bass", #{ style: "offbeat", preset: "saw_bass", register: [33, 50], density: 0.47236422, seed: -771037852412648822 });
part("keys", #{ style: "offbeat", preset: "epiano", register: [55, 76], density: 0.4036526, seed: 4480519692905280314 });

form([
  sec("intro", 4, "kick hat loop").energy(0.3903019).filter_in(),
  sec("groove", 8, "kick clap hat ohat shaker loop bass").energy(0.66373897).fills(false),
  sec("break", 4, "loop keys").energy(0.3407866).gap(),
  sec("drop", 16, "*").energy(0.97113043).impact(),
  sec("break", 4, "loop").name("break2").energy(0.32666034),
  sec("drop", 8, "*").name("drop2").energy(0.98874223).gap().impact(),
  sec("drop", 8, "*").name("drop3").energy(0.98664886),
  sec("outro", 4, "kick hat loop").energy(0.45763978),
]);

mix("clap", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ level: -3, pan: -0.3 });
mix("perc", #{ pan: 0.4, delay: 0.15 });
mix("loop", #{ reverb: 0.09135309, duck: 0.8 });
mix("bass", #{ duck: 0.6 });
mix("keys", #{ pan: -0.35, reverb: 0.24518426, delay: 0.23290655, duck: 0.5 });
mix("fx", #{ reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("loop", [
  #{ fx: "filter", cutoff: sections(1600, #{ "break": [3500, 500], drop: [6000, 9500], groove: [1600, 4500], intro: [450, 1600], outro: [1600, 400] }), drive: 1, mix: 1, mode: "lp", res: 0.55 },
  #{ fx: "phaser", depth: 0.7, feedback: 0.4, mix: 0.45, rate_hz: 0.2, stages: 6 },
  #{ fx: "chorus", delay_ms: 13, depth_ms: 3.5, mix: 0.9, rate_hz: 0.35 },
  #{ fx: "compressor", attack_ms: 5, knee_db: 3, makeup_db: 3, mix: 1, ratio: 6, release_ms: 80, threshold_db: -22 },
  #{ fx: "auto_pan", beats: 2, depth: 0.51514673 },
]);
fx("bass", [#{ fx: "filter", cutoff: sections(1200, #{ drop: 2400 }), drive: 1, mix: 1, mode: "lp", res: 0.3 }]);
fx("keys", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 10, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 100, threshold_db: -12 },
  #{ fx: "saturate", drive_db: 4, kind: "tape", mix: 0.35 },
]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "saturate", drive_db: 4, kind: "tape", mix: 0.3 },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 10, knee_db: 4, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 120, threshold_db: -12 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tape", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.7264763, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });
