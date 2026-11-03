// Glass Garden — big_beat (seed 7)
// song("big_beat", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(125); key("D minor"); swing(0.50688463);
humanize(0.2);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X.....X...X....X|X.........X.....|X.....X...X.....");
drum("kick", "sparse", "X.......X.......");
drum("snare", "main", "..o.X.o.....X...|....X....o..X...|..o.X.o.....X...|....X....o..X...");
drum("hat", "main", "x.xxxxx.x.xxx.xo");
drum("hat", "sparse", "x.x.x.x.x.x.x.x.");
drum("ride", "main", "o...o.o...o.o...");
drum("perc", "main", ".............xxx");
drum_sound("perc", "tom");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "saw_bass", register: [31, 48], density: 0.42864978, seed: -37300132614189792 });
part("acid", #{ style: "acid", preset: "acid", register: [36, 55], density: 0.4710492, seed: -6710496186308540014 });
part("stabs", #{ style: "offbeat", preset: "pad", register: [52, 72], density: 0.5823303, seed: -7892568474930094457 });
part("riff", #{ style: "motif", preset: "pluck", register: [55, 76], density: 0.51092225, seed: 6104030045851553230 });

form([
  sec("intro", 8, "kick snare hat acid").energy(0.59030193),
  sec("groove", 8, "kick snare hat ride perc bass acid stabs").energy(0.66373897).fills(false),
  sec("build", 4, "snare hat acid stabs riff").energy(0.6407866).gap(),
  sec("drop", 8, "*").energy(0.97113043).impact(),
  sec("break", 4, "acid stabs perc").energy(0.37666035),
  sec("build", 4, "snare hat acid riff").name("build2").energy(0.6887422).gap(),
  sec("drop", 12, "*").name("drop2").energy(0.98664886).impact(),
  sec("drop", 12, "*").name("drop3").energy(1),
  sec("outro", 8, "kick snare hat acid").energy(0.40374148),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.3 });
mix("ride", #{ pan: -0.35 });
mix("perc", #{ pan: -0.4, reverb: 0.15 });
mix("bass", #{ duck: 0.35 });
mix("acid", #{ delay: 0.1465493, duck: 0.3 });
mix("stabs", #{ pan: 0.2, reverb: 0.24518426, delay: 0.31054208, duck: 0.45 });
mix("riff", #{ reverb: 0.1826181, delay: 0.13355356, duck: 0.25 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "hard", mix: 0.45 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("acid", [
  #{ fx: "filter", cutoff: sections(900, #{ build: [900, 5500], drop: [4500, 2500], intro: [400, 900], outro: [900, 400] }), drive: 1.6, mix: 1, mode: "ladder", res: sections(0.55, #{ build: [0.55, 0.8] }) },
  #{ fx: "saturate", drive_db: 8, kind: "fold", mix: 0.35 },
]);
fx("stabs", [
  #{ fx: "bitcrush", bits: 10, downsample: 2, mix: 0.3 },
  #{ fx: "chorus", delay_ms: 12, depth_ms: 3.5, mix: 0.9, rate_hz: 0.5 },
]);
fx("riff", [
  #{ fx: "parallel", chains: [#{ amount: 0.5, fx: [#{ fx: "saturate", drive_db: 16, kind: "hard", mix: 1 }, #{ fx: "filter", cutoff: 500, drive: 1, mix: 1, mode: "hp", res: 0 }] }, #{ amount: 0.3, fx: [#{ fx: "bitcrush", bits: 6, downsample: 4, mix: 1 }, #{ fx: "filter", cutoff: 1800, drive: 1, mix: 1, mode: "bp", res: 0.3 }] }], dry: 0.8 },
  #{ fx: "chorus", delay_ms: 14, depth_ms: 4, mix: 0.8, rate_hz: 0.4 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.5 }]);
bus("drums", [
  #{ fx: "parallel", chains: [#{ amount: 0.55, fx: [#{ fx: "compressor", attack_ms: 0.5, knee_db: 2, makeup_db: 0, mix: 1, ratio: 12, release_ms: 50, threshold_db: -32 }, #{ fx: "saturate", drive_db: 10, kind: "hard", mix: 1 }] }, #{ amount: 0.25, fx: [#{ fx: "bitcrush", bits: 8, downsample: 3, mix: 1 }, #{ fx: "filter", cutoff: 450, drive: 1, mix: 1, mode: "hp", res: 0 }] }], dry: 1 },
  #{ fx: "compressor", attack_ms: 5, knee_db: 4, makeup_db: 1, mix: 1, ratio: 3, release_ms: 80, threshold_db: -12 },
  #{ fx: "width", mono_below_hz: 120, width: 1 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 120, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 1.9422858, lowcut: 328.14508, mix: 1, predelay_ms: 20.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.4, feedback: 0.29412478, mix: 1, pingpong: true } });
