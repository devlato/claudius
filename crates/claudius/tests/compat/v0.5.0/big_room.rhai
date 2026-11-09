// Glass Garden — big_room (seed 7)
// song("big_room", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(126); key("D minor"); swing(0.50172114);
humanize(0.04);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(2);
master(#{ lufs: -7.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...x...x.");
drum("perc", "main", "......x.......x.");
drum_sound("perc", "tom");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "offbeat", preset: "sub_bass", register: [33, 48], density: 0.53341293, seed: -2526400537787473278 });
part("lead", #{ style: "riff", preset: "supersaw", register: [60, 79], density: 0.45524326, seed: 6553130387488681005 });
part("chords", #{ style: "sustain", preset: "brass_stab", register: [55, 76], density: 0.44522208, seed: -4694233468511451402 });
part("pad", #{ style: "sustain", preset: "glass_pad", register: [48, 74], density: 0.5752324, seed: 8524255199291047346 });

form([
  sec("intro", 4, "kick hat perc bass").energy(0.59030193),
  sec("groove", 8, "kick clap hat ohat perc bass chords").energy(0.66373897).fills(false),
  sec("build", 8, "clap hat chords lead").energy(0.6407866).gap(),
  sec("drop", 8, "kick clap hat ohat perc bass lead").energy(0.97113043).impact(),
  sec("break", 8, "pad chords lead arp").energy(0.32666034),
  sec("build", 4, "clap hat pad chords lead arp").name("build2").energy(0.6887422).prog("B").gap(),
  sec("drop", 12, "kick clap hat ohat perc bass lead chords").name("drop2").energy(0.98664886).impact(),
  sec("drop", 12, "kick clap hat ohat perc bass lead chords").name("drop3").energy(1),
  sec("outro", 4, "kick hat perc bass").energy(0.40374148),
]);

mix("kick", #{ reverb: 0.06 });
mix("clap", #{ reverb: 0.3 });
mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.1 });
mix("perc", #{ pan: -0.25, reverb: 0.2 });
mix("bass", #{ duck: 0.85 });
mix("lead", #{ reverb: 0.5860413, delay: 0.18318664, duck: 0.35 });
mix("chords", #{ reverb: 0.2860483, delay: 0.15527104, duck: 0.6 });
mix("pad", #{ reverb: 0.608727, duck: 0.45 });
mix("fx", #{ reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "tube", mix: 0.4 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("lead", [
  #{ fx: "saturate", drive_db: 5, kind: "tanh", mix: 0.35 },
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 },
]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("pad", [#{ fx: "width", mono_below_hz: 150, width: 1.4416066 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.35 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 8, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 90, threshold_db: -11 },
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.3 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 160, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 1, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 120, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.31764442, decay: 3.0213335, lowcut: 281.2672, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.45, feedback: 0.29412478, mix: 1, pingpong: true } });
