// Glass Garden — hard_techno (seed 7)
// song("hard_techno", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(146); key("D minor"); swing(0.50172114);
humanize(0.05);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(4);
master(#{ lufs: -7.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..............x.");
drum("clap", "main", "....X.......X...");
drum("ride", "main", "ooo.ooo.o.o.ooo.");
drum("perc", "main", "..x...x..x..x..x");
drum("perc", "sparse", "......x.......x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "offbeat", preset: "dist_bass", register: [28, 43], density: 0.42864978, seed: -37300132614189792 });
part("lead", #{ style: "riff", preset: "acid", register: [57, 74], density: 0.32104918, seed: -6710496186308540014 });
part("chords", #{ style: "offbeat", preset: "stab", register: [50, 70], density: 0.54910505, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "strings", register: [45, 70], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 4, "kick hat perc").energy(0.64030194),
  sec("groove", 16, "kick hat ohat clap perc ride bass").energy(0.763739).fills(false),
  sec("break", 4, "chords lead ride").energy(0.39078662).prog("A"),
  sec("build", 4, "clap hat ride chords lead").energy(0.6711304).gap(),
  sec("drop", 16, "kick hat ohat clap perc ride bass lead chords").name("peak").energy(0.9766603).impact(),
  sec("break", 4, "pad lead ride").name("break2").energy(0.3887422),
  sec("build", 4, "clap hat ride pad lead").name("build2").energy(0.68664885).prog("B").gap(),
  sec("drop", 12, "kick hat ohat clap perc ride bass lead chords").name("peak2").energy(1).impact(),
  sec("drop", 12, "kick hat ohat clap perc ride bass lead chords").name("peak3").energy(0.9537415),
  sec("outro", 4, "kick hat perc").energy(0.5116369),
]);

mix("kick", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.15 });
mix("clap", #{ reverb: 0.25 });
mix("ride", #{ pan: 0.3 });
mix("perc", #{ pan: -0.35, reverb: 0.15, delay: 0.2 });
mix("bass", #{ duck: 0.85 });
mix("lead", #{ reverb: 0.43953097, delay: 0.36637327, duck: 0.3 });
mix("chords", #{ reverb: 0.2860483, delay: 0.4658131, duck: 0.5 });
mix("pad", #{ reverb: 0.608727, duck: 0.5 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("kick", [#{ fx: "saturate", drive_db: 8, kind: "hard", mix: 0.5 }]);
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "tube", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("lead", [
  #{ fx: "saturate", drive_db: 6, kind: "hard", mix: 0.4 },
  #{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 },
]);
fx("chords", [
  #{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.4 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("pad", [#{ fx: "width", mono_below_hz: 150, width: 1.4416066 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 8, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 90, threshold_db: -11 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 120, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2.5, kind: "hard", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 3.7766669, lowcut: 187.51147, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.37816042, mix: 1, pingpong: true } });
