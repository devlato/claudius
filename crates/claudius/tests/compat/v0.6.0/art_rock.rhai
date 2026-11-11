// Glass Garden — art_rock (seed 7)
// song("art_rock", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(72); key("D minor"); swing(0.5051635);
humanize(0.4);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(1);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.....X.X.......|X.....X.X.......|X.....X.X.......|X.......X.......");
drum("kick", "sparse", "x.........x.....");
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 7 });
drum("hat", "main", "x...x.x.xox.x.xo");
drum("hat", "sparse", "o.o.o.o.o...o.o.");
drum("ride", "main", "o.o.o.o.o.o.o.o.");
lane("ride", #{ late: -1 });
drum("tom", "main", ".............x.x");

// parts written by the theory engine: change the style, preset, register, density or seed
part("piano", #{ style: "half", preset: "keys", register: [48, 76], density: 0.42864978, seed: -37300132614189792 });
part("arp", #{ style: "up", preset: "pluck", register: [60, 84], density: 0.5272447, rate: 0.25, seed: 6674420348178494801 });
part("bass", #{ style: "rock", preset: "saw_bass", register: [28, 47], density: 0.5823303, seed: -7892568474930094457 });
part("strings", #{ style: "legato", preset: "strings", register: [45, 81], density: 0.47421524, seed: 805921219908132358 });
part("guitar", #{ style: "pinky", preset: "harp_pluck", register: [52, 76], density: 0.5554895, rate: 0.5, seed: -6045021890477887772 });
part("dist", #{ style: "pump", preset: "clav", register: [40, 64], density: 0.45818564, seed: 673023462843144272 });
part("dist2", #{ style: "sustain", preset: "supersaw", register: [40, 64], density: 0.57625425, seed: -3266531712626834822 });
part("lead", #{ style: "rock_lead", preset: "saw_lead", register: [62, 81], density: 0.42546016, seed: -2362906824261301826 });

form([
  sec("intro", 8, "piano arp").energy(0.19030192).filter_in(),
  sec("verse", 8, "kick hat piano arp bass").energy(0.41373897).variant("sparse").fills(true),
  sec("verse", 8, "kick snare hat piano arp bass strings guitar").name("verse2").energy(0.59078664),
  sec("build", 4, "snare hat tom piano strings guitar arp dist dist2").name("rise").energy(0.6711304).prog("B").gap(),
  sec("hook", 4, "kick snare hat ride tom bass piano strings guitar arp lead dist dist2").name("climax").energy(0.9766603).impact(),
  sec("break", 4, "piano strings").name("fall").energy(0.2887422).gap(),
  sec("hook", 4, "kick snare hat ride tom bass piano strings guitar arp lead dist dist2").name("climax2").energy(0.98664886).gap().impact(),
  sec("hook", 4, "kick snare hat ride tom bass piano strings guitar arp lead dist dist2").name("climax3").energy(1),
  sec("outro", 4, "piano arp").energy(0.20374149),
]);

mix("snare", #{ reverb: 0.35 });
mix("hat", #{ pan: 0.25 });
mix("ride", #{ pan: -0.3 });
mix("tom", #{ pan: -0.2, reverb: 0.35 });
mix("piano", #{ pan: -0.1, reverb: 0.3197358 });
mix("arp", #{ pan: 0.25, reverb: 0.3662758, delay: 0.36637327 });
mix("strings", #{ reverb: 0.54785424 });
mix("guitar", #{ pan: 0.35, reverb: 0.39236584, delay: 0.41519445 });
mix("dist", #{ level: 1.5, pan: -0.45, reverb: 0.20944993 });
mix("dist2", #{ level: 1.5, pan: 0.45, reverb: 0.16981599 });
mix("lead", #{ reverb: 0.3805865, delay: 0.33949146 });
mix("fx", #{ reverb: 0.4 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("piano", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "auto_pan", beats: 2, depth: 0.51514673 },
]);
fx("arp", [
  #{ fx: "bitcrush", bits: sections(12, #{ hook: 8 }), downsample: 2, mix: 0.5 },
  #{ fx: "parallel", chains: [#{ amount: sections(0, #{ verse: [0, 0.7] }), fx: [#{ fx: "gate", depth: 1, smooth_ms: 3, step_beats: 0.25, steps: [1, 0.3, 1, 0, 1, 0.6, 0, 1] }] }], dry: sections(1, #{ verse: [1, 0.3] }) },
]);
fx("bass", [#{ fx: "saturate", drive_db: sections(6, #{ climax: 14, climax2: 14, rise: 10 }), kind: "tube", mix: 0.6 }]);
fx("strings", [#{ fx: "phaser", depth: 0.6717282, feedback: 0.22995073, mix: sections(0.20815219, #{ drop: 0.37178797, hook: 0.37178797 }), rate_hz: 0.12636338, stages: 4 }]);
fx("guitar", [#{ fx: "chorus", delay_ms: 12, depth_ms: 3, mix: 0.6, rate_hz: 0.4 }]);
fx("dist", [
  #{ fx: "eq", bands: [#{ freq: 85, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1700, gain_db: 3, kind: "peak", q: 0.8 }] },
  #{ fx: "saturate", drive_db: 24, kind: "hard", mix: 1 },
  #{ fx: "eq", bands: [#{ freq: 5200, gain_db: 0, kind: "lowpass", q: 0.7 }, #{ freq: 400, gain_db: -2, kind: "peak", q: 1 }] },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("dist2", [
  #{ fx: "eq", bands: [#{ freq: 85, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1700, gain_db: 3, kind: "peak", q: 0.8 }] },
  #{ fx: "saturate", drive_db: 24, kind: "hard", mix: 1 },
  #{ fx: "eq", bands: [#{ freq: 5200, gain_db: 0, kind: "lowpass", q: 0.7 }, #{ freq: 400, gain_db: -2, kind: "peak", q: 1 }] },
  #{ fx: "phaser", depth: 0.81336445, feedback: 0.49436772, mix: sections(0.376867, #{ drop: 0.5986754, hook: 0.5986754 }), rate_hz: 0.50267774, stages: 8 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 6, kind: "tape", mix: 0.5 }]);
bus("drums", [
  #{ fx: "parallel", chains: [#{ amount: sections(0.15, #{ hook: 0.45 }), fx: [#{ fx: "compressor", attack_ms: 1, knee_db: 2, makeup_db: 0, mix: 1, ratio: 10, release_ms: 70, threshold_db: -30 }, #{ fx: "saturate", drive_db: 6, kind: "tape", mix: 1 }] }], dry: 1 },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.2, release_ms: 120, threshold_db: -14 },
  #{ fx: "width", mono_below_hz: 120, width: 1 },
]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 90, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "saturate", drive_db: 4, kind: "tape", mix: 0.6 },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "eq", bands: [#{ freq: 9000, gain_db: -2.5, kind: "highshelf", q: 0.7 }, #{ freq: 300, gain_db: 1, kind: "peak", q: 0.8 }] },
  #{ fx: "compressor", attack_ms: 30, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.6, release_ms: 200, threshold_db: -12 },
  #{ fx: "saturate", drive_db: 3, kind: "tape", mix: 0.5 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 3.0213335, lowcut: 234.38934, mix: 1, predelay_ms: 30.852709, width: 1.4589927 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });
