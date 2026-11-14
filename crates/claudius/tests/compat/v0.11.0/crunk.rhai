// Glass Garden — crunk (seed 7)
// song("crunk", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(81); key("D minor"); swing(0.5051635);
humanize(0.08);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -8, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.....X.X.......|X.......X.X.....|X.....X.X.......|X.......X.X.....");
drum("kick", "main", "X.......X.......");
drum("snare", "main", "....X.......X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "x...x...x...x...x...x...x...x.x.|x...x...x...x...x...x...x...x.x.|x...x...x...x...x...x.......Xxxx|x...x...x...x...x...x...x...x.x.", 32);
drum("ohat", "main", "..............x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "glide", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("lead", #{ style: "horn_riff", preset: "brass_lead", register: [60, 82], density: 0.4710492, seed: -6710496186308540014 });
part("stabs", #{ style: "stabs", preset: "brass_stab", register: [52, 72], density: 0.54910505, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "pad", register: [48, 72], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 8, "lead hat").energy(0.59030193).variant("main"),
  sec("hook", 4, "kick snare clap hat bass lead stabs").energy(0.91373897).fills(false).impact(),
  sec("verse", 8, "kick snare clap hat bass lead").energy(0.6907866).gap(),
  sec("hook", 4, "kick snare clap hat ohat bass lead stabs").name("hook2").energy(0.9211304).fills(false).impact(),
  sec("verse", 8, "kick snare clap hat bass lead pad").name("verse2").energy(0.69666034).variant("alt").fills(true),
  sec("break", 4, "lead pad").name("breakdown").energy(0.3887422).prog("A"),
  sec("hook", 8, "*").name("hook3").energy(0.98664886).fills(false).gap().impact(),
  sec("hook", 8, "*").name("hook4").energy(1).fills(false),
  sec("outro", 4, "lead hat").energy(0.3537415).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("clap", #{ level: -1, reverb: 0.2 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("lead", #{ level: -2, reverb: 0.43953097, delay: 0.122124426 });
mix("stabs", #{ level: -2, reverb: 0.2860483 });
mix("pad", #{ level: -5, reverb: 0.4869816 });
mix("fx", #{ level: -2, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 8, kind: "tanh", mix: 1 },
  #{ fx: "filter", cutoff: 5000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("lead", [
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.4 },
  #{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 },
]);
fx("stabs", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("pad", [#{ fx: "width", mono_below_hz: 150, width: 1.4416066 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.6 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.5 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 90, threshold_db: -10 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 120, threshold_db: -8 },
  #{ fx: "saturate", drive_db: 2.5, kind: "tanh", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.5106667, lowcut: 281.2672, mix: 1, predelay_ms: 17.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.21008913, mix: 1, pingpong: true } });
