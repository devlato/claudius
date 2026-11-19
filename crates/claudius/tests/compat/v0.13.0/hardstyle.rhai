// Glass Garden — hardstyle (seed 7)
// song("hardstyle", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(150); key("D minor"); swing(0.5);
humanize(0.03);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(1);
master(#{ lufs: -7, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", ".xx...xx..x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...x...x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "offbeat", preset: "reese", register: [33, 48], density: 0.42864978, seed: -37300132614189792 });
part("lead", #{ style: "trance_lead", preset: "saw_lead", register: [64, 83], density: 0.5590558, seed: 5396775171407160694 });
part("chords", #{ style: "pump", preset: "pwm_pad", register: [55, 79], density: 0.56008077, seed: -1629742732023437997 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 76], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 4, "kick hat bass").energy(0.64030194),
  sec("groove", 8, "kick clap hat ohat bass lead").energy(0.763739).fills(false),
  sec("break", 16, "pad chords lead").energy(0.3407866),
  sec("build", 4, "clap hat pad chords lead").energy(0.6711304).prog("B").gap(),
  sec("drop", 16, "*").energy(0.9766603).impact(),
  sec("drop", 16, "*").name("drop2").energy(0.98874223),
  sec("outro", 8, "kick hat bass").energy(0.48664883),
]);

mix("clap", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.12 });
mix("bass", #{ duck: 0.95 });
mix("lead", #{ level: 1, reverb: 0.5127861, delay: 0.24424885, duck: 0.4 });
mix("chords", #{ reverb: 0.2860483, delay: 0.15527104, duck: 0.6 });
mix("pad", #{ reverb: 0.608727, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("kick", [
  #{ fx: "saturate", drive_db: 10, kind: "fold", mix: 0.55 },
  #{ fx: "filter", cutoff: 9000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
]);
fx("bass", [
  #{ fx: "saturate", drive_db: 8, kind: "hard", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("lead", [
  #{ fx: "saturate", drive_db: 5, kind: "tanh", mix: 0.4 },
  #{ fx: "width", mono_below_hz: 200, width: 1.5 },
  #{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 },
]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.5 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("pad", [#{ fx: "width", mono_below_hz: 150, width: 1.4416066 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 3, kind: "hard", mix: 0.3 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 80, threshold_db: -10 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 180, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 110, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "hard", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 3.237143, lowcut: 300.01837, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });
