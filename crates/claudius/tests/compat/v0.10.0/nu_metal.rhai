// Glass Garden — nu_metal (seed 7)
// song("nu_metal", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(89); key("D phrygian"); swing(0.50688463);
humanize(0.3);
prog("A", "i II i VII");
prog("B", "i II III II");
bars_per_chord(1);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "half", "X.....X.........|X.......X.X.....|X.....X.........|X.......X.X.....");
drum("kick", "main", "X..X..X..XX.....|X..X..X.X....X..|X..X..X..XX.....|X..X..X.X....X..");
drum("kick", "verse", "X.....X...X.....|X.........X.....|X.....X...X.....|X.........X.....");
lane("kick", #{ late: 1 });
drum("snare", "half", "........X.......");
drum("snare", "main", "....X...o...X...");
drum("hat", "half", "x...x.x.x.x.x.x.");
drum("hat", "main", "x.x.x.x.x.x.x.x.");
lane("hat", #{ late: 1 });
drum("crash", "main", "X...............|................|X...............|................");

// parts written by the theory engine: change the style, preset, register, density or seed
part("chug_l", #{ style: "chug", preset: "supersaw", register: [33, 52], density: 0.42864978, seed: -37300132614189792 });
part("gtr_l", #{ style: "sustain", preset: "supersaw", register: [36, 57], density: 0.56008077, seed: -1629742732023437997 });
part("bass", #{ style: "root8", preset: "dist_bass", register: [26, 40], density: 0.5554895, seed: 6734273059743298459 });
part("clean", #{ style: "up", preset: "pluck", register: [62, 81], density: 0.5689536, rate: 0.5, seed: 673023462843144272 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 72], density: 0.57625425, seed: -3266531712626834822 });
part("chug_r", #{ style: "chug", preset: "supersaw", register: [33, 52], density: 0.5272447, seed: 5396775171407160694, doubles: "chug_l", late: 7 });
part("gtr_r", #{ style: "sustain", preset: "supersaw", register: [36, 57], density: 0.47421524, seed: 805921219908132358, doubles: "gtr_l", late: 5 });

form([
  sec("intro", 8, "clean chug_l").energy(0.69030195).variant("main"),
  sec("verse", 8, "kick snare hat bass chug_l chug_r clean").energy(0.66373897).variant("verse"),
  sec("hook", 4, "kick snare hat crash bass gtr_l gtr_r pad").name("chorus").energy(0.9907866).impact(),
  sec("verse", 8, "kick snare hat bass chug_l chug_r clean").name("verse2").energy(0.72113043).variant("verse").fills(true).gap(),
  sec("hook", 4, "kick snare hat crash bass gtr_l gtr_r pad").name("chorus2").energy(0.9766603).impact(),
  sec("break", 4, "clean pad hat").energy(0.3887422),
  sec("hook", 8, "kick snare hat crash bass chug_l chug_r").name("breakdown").energy(0.93664885).prog("B").variant("half").fills(false).gap().impact(),
  sec("hook", 8, "kick snare hat crash bass gtr_l gtr_r chug_l chug_r pad").name("chorus3").energy(1).impact(),
  sec("hook", 4, "kick snare hat crash bass gtr_l gtr_r chug_l chug_r pad").name("chorus4").energy(0.9537415),
  sec("outro", 4, "clean pad").energy(0.5116369).variant("main"),
]);

mix("snare", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.25 });
mix("crash", #{ pan: -0.35, reverb: 0.15 });
mix("chug_l", #{ level: 1.5, pan: -0.85, reverb: 0.036541235 });
mix("chug_r", #{ level: 1.5, pan: 0.85, reverb: 0.058604125 });
mix("gtr_l", #{ level: 1.5, pan: -0.85, reverb: 0.04903685 });
mix("gtr_r", #{ level: 1.5, pan: 0.85, reverb: 0.073047236 });
mix("clean", #{ level: -3, pan: 0.4, reverb: 0.55853313, delay: 0.292831 });
mix("pad", #{ level: -4, reverb: 0.50944793 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("chug_l", [
  #{ fx: "eq", bands: [#{ freq: 70, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 120, gain_db: 2, kind: "peak", q: 1 }, #{ freq: 550, gain_db: -3, kind: "peak", q: 1 }, #{ freq: 2400, gain_db: 2, kind: "peak", q: 0.8 }, #{ freq: 6500, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "saturate", drive_db: 9, kind: "hard", mix: 0.5 },
  #{ fx: "auto_pan", beats: 2, depth: 0.51514673 },
]);
fx("chug_r", [
  #{ fx: "eq", bands: [#{ freq: 70, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 120, gain_db: 2, kind: "peak", q: 1 }, #{ freq: 500, gain_db: -3, kind: "peak", q: 1 }, #{ freq: 2100, gain_db: 2, kind: "peak", q: 0.8 }, #{ freq: 6000, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "saturate", drive_db: 10, kind: "hard", mix: 0.5 },
]);
fx("gtr_l", [
  #{ fx: "eq", bands: [#{ freq: 70, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 550, gain_db: -2.5, kind: "peak", q: 1 }, #{ freq: 2200, gain_db: 2.5, kind: "peak", q: 0.8 }, #{ freq: 7000, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "saturate", drive_db: 8, kind: "hard", mix: 0.5 },
  #{ fx: "chorus", delay_ms: 8.467328, depth_ms: 3.3550498, mix: 0.33927074, rate_hz: 0.24641252 },
]);
fx("gtr_r", [
  #{ fx: "eq", bands: [#{ freq: 70, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 500, gain_db: -2.5, kind: "peak", q: 1 }, #{ freq: 1900, gain_db: 2.5, kind: "peak", q: 0.8 }, #{ freq: 6500, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "saturate", drive_db: 9, kind: "hard", mix: 0.5 },
  #{ fx: "chorus", delay_ms: 8.684588, depth_ms: 2.5733013, mix: 0.37727153, rate_hz: 0.35640424 },
]);
fx("bass", [#{ fx: "saturate", drive_db: 8, kind: "tube", mix: 0.6 }]);
fx("clean", [#{ fx: "chorus", delay_ms: 14, depth_ms: 4, mix: 0.5, rate_hz: 0.3 }]);
fx("pad", [#{ fx: "phaser", depth: 0.81336445, feedback: 0.49436772, mix: sections(0.376867, #{ drop: 0.5986754, hook: 0.5986754 }), rate_hz: 0.50267774, stages: 8 }]);
bus("bass", [
  #{ fx: "width", mono_below_hz: 150, width: 1 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 0, mix: 1, ratio: 4, release_ms: 70, threshold_db: -16 },
]);
bus("drums", [
  #{ fx: "width", mono_below_hz: 150, width: 1 },
  #{ fx: "parallel", chains: [#{ amount: 0.4, fx: [#{ fx: "compressor", attack_ms: 1, knee_db: 2, makeup_db: 0, mix: 1, ratio: 10, release_ms: 60, threshold_db: -30 }, #{ fx: "saturate", drive_db: 6, kind: "tape", mix: 1 }] }], dry: 1 },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.8, release_ms: 90, threshold_db: -14 },
]);
bus("music", [#{ fx: "width", mono_below_hz: 150, width: 1 }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.3, release_ms: 140, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 3, kind: "tape", mix: 0.5 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.6185715, lowcut: 281.2672, mix: 1, predelay_ms: 17.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.29412478, mix: 1, pingpong: true } });
