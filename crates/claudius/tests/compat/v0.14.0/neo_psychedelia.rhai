// Glass Garden — neo_psychedelia (seed 7)
// song("neo_psychedelia", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(97); key("D mixolydian"); swing(0.5086058);
humanize(0.5);
prog("A", "I VII IV I");
prog("B", "I v IV I");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.......X.X.....");
drum("kick", "sparse", "x...............");
lane("kick", #{ late: 1 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 12 });
drum("hat", "main", "xxxxxxxxxxxxxxxx");
drum("hat", "sparse", "o.o.o.o.o.o.o.o.");
drum("ride", "main", "o.o...o.o.o.o.o.");
lane("ride", #{ late: 2 });
drum("tom", "main", "..............xx");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "sustain", register: [32, 51], density: 0.42864978, seed: -37300132614189792 });
part("guitar", #{ style: "half", preset: "pluck", register: [53, 77], density: 0.5272447, seed: 5396775171407160694 });
part("organ", #{ style: "ballad", preset: "organ", register: [49, 73], density: 0.5144286, seed: 336892620131319353 });
part("synth", #{ style: "sustain", preset: "warm_pad", register: [50, 77], density: 0.46618003, seed: 4033930860755562766 });
part("lead", #{ style: "rock_lead", preset: "fm_lead", register: [58, 77], density: 0.37301314, seed: -6045021890477887772 });

form([
  sec("intro", 8, "synth guitar").energy(0.4903019),
  sec("verse", 8, "kick snare hat bass organ").energy(0.46373898).fills(true),
  sec("hook", 8, "*").name("chorus").energy(0.84078664).impact(),
  sec("verse", 8, "kick snare hat bass guitar organ").name("verse2").energy(0.52113044).gap(),
  sec("hook", 8, "*").name("chorus2").energy(0.8766603).impact(),
  sec("break", 8, "lead").name("bridge").energy(0.28874218).gap(),
  sec("hook", 4, "*").name("chorus3").energy(0.98664886).gap().impact(),
  sec("hook", 4, "*").name("chorus4").energy(1),
  sec("outro", 8, "synth guitar organ").energy(0.3537415),
]);

mix("snare", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.25 });
mix("ride", #{ pan: -0.3, reverb: 0.15 });
mix("tom", #{ pan: -0.2, reverb: 0.25 });
mix("guitar", #{ pan: 0.25, reverb: 0.5127861, delay: 0.24424885 });
mix("organ", #{ pan: -0.25, reverb: 0.24518426 });
mix("synth", #{ reverb: 0.608727 });
mix("lead", #{ reverb: 0.52315444, delay: 0.41519445 });
mix("fx", #{ reverb: 0.45 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "parallel", chains: [#{ amount: 0.3, fx: [#{ fx: "saturate", drive_db: 12, kind: "fold", mix: 1 }, #{ fx: "filter", cutoff: 500, drive: 1, mix: 1, mode: "hp", res: 0 }] }], dry: 1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("guitar", [
  #{ fx: "phaser", depth: 0.8, feedback: 0.45, mix: sections(0.4, #{ "break": [0.4, 0.8] }), rate_hz: 0.3, stages: 6 },
  #{ fx: "parallel", chains: [#{ amount: sections(0.12, #{ hook: 0.35 }), fx: [#{ fx: "saturate", drive_db: 12, kind: "fold", mix: 1 }, #{ fx: "filter", cutoff: 5000, drive: 1, mix: 1, mode: "lp", res: 0 }] }], dry: 1 },
  #{ fx: "chorus", delay_ms: 14, depth_ms: 4, mix: 0.8, rate_hz: 0.5 },
]);
fx("organ", [
  #{ fx: "tremolo", beats: 0.5, depth: 0.25 },
  #{ fx: "chorus", delay_ms: 10, depth_ms: 3, mix: 0.6, rate_hz: 0.8 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("synth", [
  #{ fx: "flanger", delay_ms: 4, depth_ms: 3, feedback: 0.5, mix: 0.5, rate_hz: 0.12 },
  #{ fx: "width", mono_below_hz: 150, width: 1.4416066 },
]);
fx("lead", [#{ fx: "tape", drive_db: 2, flutter: 0.3, lowpass_hz: 9000, wow: 0.6 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 16.7555, knee_db: 6, makeup_db: 1, mix: 1, ratio: 3.0463443, release_ms: 132.5543, threshold_db: -13.541128 },
  #{ fx: "width", mono_below_hz: 120, width: 1 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "width", mono_below_hz: 140, width: 1 },
  #{ fx: "tape", drive_db: 3, flutter: 0.2, lowpass_hz: 15000, wow: 0.3 },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -13 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 3.6687622, lowcut: 234.38934, mix: 1, predelay_ms: 35.852707, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.33614263, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", band: [0, 90], level: -3, fold: [28, 40] },
  #{ sound: "pluck_bass", band: [90, 0] },
]);
