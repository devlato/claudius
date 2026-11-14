// Glass Garden — alt_rock (seed 7)
// song("alt_rock", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(102); key("D minor"); swing(0.5034423);
humanize(0.45);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(1);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.....X.X.X.....");
drum("kick", "verse", "X.......X.......");
lane("kick", #{ late: 1 });
drum("snare", "main", "...oX.......X...");
drum("snare", "verse", "...oX.......X...");
lane("snare", #{ late: 6 });
drum("hat", "main", "x...x.x.x.x.x.x.");
drum("hat", "verse", "o.o.o.o.o...o.o.");
drum("ride", "main", "x.x.x.x.x.x.x.x.");
lane("ride", #{ late: -1 });
drum("crash", "main", "X...............|................|................|................");
drum("tom", "main", "............XXXX");

// parts written by the theory engine: change the style, preset, register, density or seed
part("clean", #{ style: "updown", preset: "pluck", register: [50, 74], density: 0.42864978, rate: 0.5, seed: 3796763344308755361 });
part("gtr_l", #{ style: "half", preset: "supersaw", register: [40, 62], density: 0.45851198, seed: 6674420348178494801 });
part("bass", #{ style: "drive", preset: "rubber_bass", register: [28, 47], density: 0.4087378, seed: 6104030045851553230 });
part("solo", #{ style: "rock_lead", preset: "square_lead", register: [57, 81], density: 0.47301313, seed: -6045021890477887772 });
part("gtr_r", #{ style: "sustain", preset: "supersaw", register: [40, 62], density: 0.5823303, seed: -7892568474930094457, doubles: "gtr_l", late: 7 });

form([
  sec("intro", 8, "clean").energy(0.34030193).variant("main").filter_in(),
  sec("verse", 8, "kick snare hat bass clean").energy(0.41373897).variant("verse"),
  sec("build", 4, "kick snare hat tom bass clean gtr_l").name("pre").energy(0.6407866).prog("B").fills(true),
  sec("hook", 8, "kick snare ride crash bass gtr_l gtr_r").name("chorus").energy(0.9211304).impact(),
  sec("verse", 8, "kick snare hat bass clean").name("verse2").energy(0.47666034).variant("verse").fills(true),
  sec("build", 4, "kick snare hat tom bass clean gtr_l").name("pre2").energy(0.6887422).prog("B").fills(true),
  sec("hook", 4, "kick snare ride crash bass gtr_l gtr_r").name("chorus2").energy(0.98664886).impact(),
  sec("break", 8, "kick hat bass clean").name("bridge").energy(0.45763978),
  sec("hook", 4, "kick snare ride crash bass gtr_l gtr_r solo").name("chorus3").energy(0.9537415).gap().impact(),
  sec("hook", 4, "kick snare ride crash bass gtr_l gtr_r solo").name("chorus4").energy(1),
  sec("outro", 4, "clean gtr_l").energy(0.37864897).variant("main"),
]);

mix("snare", #{ reverb: 0.3 });
mix("hat", #{ pan: 0.25 });
mix("ride", #{ pan: -0.3 });
mix("crash", #{ pan: 0.35, reverb: 0.2 });
mix("tom", #{ pan: -0.2, reverb: 0.3 });
mix("clean", #{ pan: 0.3, reverb: 0.22838272, delay: 0.22325131 });
mix("gtr_l", #{ level: 1, pan: -0.85, reverb: 0.14651032 });
mix("gtr_r", #{ level: 1, pan: 0.85, reverb: 0.081728086 });
mix("solo", #{ level: -1, pan: 0.1, reverb: 0.39236584, delay: 0.2767963 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("clean", [
  #{ fx: "chorus", delay_ms: 12, depth_ms: 3, mix: 0.45, rate_hz: 0.5 },
  #{ fx: "phaser", depth: 0.5090129, feedback: 0.20813528, mix: sections(0.32856825, #{ drop: 0.4385138, hook: 0.4385138 }), rate_hz: 0.36893344, stages: 8 },
]);
fx("gtr_l", [#{ fx: "eq", bands: [#{ freq: 90, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1600, gain_db: 2, kind: "peak", q: 0.8 }, #{ freq: 7000, gain_db: 0, kind: "lowpass", q: 0.7 }] }]);
fx("gtr_r", [
  #{ fx: "eq", bands: [#{ freq: 90, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1300, gain_db: 2, kind: "peak", q: 0.8 }, #{ freq: 6500, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "phaser", depth: 0.6717282, feedback: 0.22995073, mix: sections(0.20815219, #{ drop: 0.37178797, hook: 0.37178797 }), rate_hz: 0.12636338, stages: 4 },
]);
fx("bass", [#{ fx: "saturate", drive_db: 5, kind: "tube", mix: 0.5 }]);
bus("bass", [
  #{ fx: "width", mono_below_hz: 150, width: 1 },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 0, mix: 1, ratio: 3, release_ms: 90, threshold_db: -16 },
]);
bus("drums", [
  #{ fx: "width", mono_below_hz: 150, width: 1 },
  #{ fx: "parallel", chains: [#{ amount: sections(0.25, #{ chorus: 0.5, chorus2: 0.5, chorus3: 0.5 }), fx: [#{ fx: "compressor", attack_ms: 2, knee_db: 2, makeup_db: 0, mix: 1, ratio: 8, release_ms: 80, threshold_db: -28 }, #{ fx: "saturate", drive_db: 5, kind: "tape", mix: 1 }] }], dry: 1 },
  #{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 110, threshold_db: -14 },
]);
bus("music", [#{ fx: "width", mono_below_hz: 150, width: 1 }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 160, threshold_db: -11 },
  #{ fx: "saturate", drive_db: 3, kind: "tape", mix: 0.5 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 1.7264763, lowcut: 234.38934, mix: 1, predelay_ms: 20.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });
