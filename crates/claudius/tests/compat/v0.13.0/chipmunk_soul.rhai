// Glass Garden — chipmunk_soul (seed 7)
// song("chipmunk_soul", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(81); key("D major"); swing(0.5268846);
humanize(0.25);
prog("A", "bVImaj7 bVII9 Imaj9 Imaj9");
prog("B", "IVmaj9 iii7 vi9 ii9 IVmaj9 iv6 Imaj9 Imaj9");
bars_per_chord(1);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X.....X...X.....|X......X..X.....|X.....X...X.....");
lane("kick", #{ late: -2 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 8 });
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 3 });
drum("hat", "main", "x.x.x.x.x.x.x.x.");
lane("hat", #{ late: -2 });
drum("ohat", "main", "..............x.");
lane("ohat", #{ late: 1 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "boom_bap", preset: "deep_bass", register: [28, 45], density: 0.42864978, seed: -37300132614189792, late: 9 });
part("keys", #{ style: "rnb_keys", preset: "epiano", register: [50, 74], density: 0.5272447, seed: 5396775171407160694 });
part("horns", #{ style: "brass_stabs", preset: "brass_stab", register: [55, 76], density: 0.5823303, seed: -7892568474930094457 });

form([
  sec("intro", 8, "hat keys").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick snare clap hat bass keys").energy(0.61373895),
  sec("hook", 4, "kick snare clap hat ohat bass horns strings keys").energy(0.8907866).impact(),
  sec("verse", 8, "kick snare clap hat bass strings keys").name("verse2").energy(0.6711304).fills(true).gap(),
  sec("hook", 4, "kick snare clap hat ohat bass horns strings keys").name("hook2").energy(0.8966603).impact(),
  sec("break", 4, "strings keys").name("bridge").energy(0.4387422).gap(),
  sec("hook", 8, "kick snare clap hat ohat bass horns strings keys").name("hook3").energy(0.93664885).gap().impact(),
  sec("hook", 4, "kick snare clap hat ohat bass horns strings keys").name("hook4").energy(1),
  sec("outro", 4, "hat keys").energy(0.3537415).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("clap", #{ level: -3, reverb: 0.18 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("keys", #{ reverb: 0.29302064 });
mix("horns", #{ level: -3, reverb: 0.2043202 });
mix("fx", #{ level: -3, reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("horns", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.45 },
  #{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 100, threshold_db: -11 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 140, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 1.5106667, lowcut: 281.2672, mix: 1, predelay_ms: 17.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.21008913, mix: 1, pingpong: true } });
