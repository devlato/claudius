// Glass Garden — dubstep (seed 7)
// song("dubstep", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(138); key("D minor"); swing(0.5034423);
humanize(0.1);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X.....X.........|X.........X.....|X.....X.........");
drum("snare", "main", "........X.......");
drum("hat", "main", "x.x...x.x.x.x.xx");
drum("ohat", "main", "................|..............x.|................|..............x.");
drum("perc", "main", "......x......x..|......x......x..|......x......x..|......x.........");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("sub", #{ style: "sustain", preset: "deep_bass", register: [24, 38], density: 0.42864978, seed: -37300132614189792 });
part("wobble", #{ style: "syncopated", preset: "dist_bass", register: [38, 55], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 74], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "motif", preset: "mallet", register: [62, 86], density: 0.3655252, seed: 6845145886153731458 });

form([
  sec("intro", 4, "pad hat").energy(0.44030192).variant("main"),
  sec("build", 4, "pad hat snare lead").energy(0.513739).gap(),
  sec("drop", 16, "kick snare hat ohat perc sub wobble pad").energy(0.9407866).fills(false).impact(),
  sec("break", 8, "pad lead hat").energy(0.37113047).build(true).gap(),
  sec("drop", 16, "*").name("drop2").energy(0.9766603).impact(),
  sec("drop", 16, "*").name("drop3").energy(0.98874223),
  sec("outro", 8, "pad hat").energy(0.28664884).variant("main"),
]);

mix("snare", #{ reverb: 0.2 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.3, reverb: 0.2 });
mix("sub", #{ level: -3, duck: 0.3 });
mix("wobble", #{ level: 1.5, duck: 0.3 });
mix("pad", #{ reverb: 0.36777636, duck: 0.4 });
mix("lead", #{ reverb: 0.3652362, delay: 0.22258927, duck: 0.25 });
mix("fx", #{ level: -2, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("sub", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("wobble", [#{ fx: "saturate", drive_db: 6.7100997, kind: "tube", mix: 0.31752482 }]);
fx("pad", [
  #{ fx: "width", mono_below_hz: 200, width: 1.5 },
  #{ fx: "chorus", delay_ms: 11.434565, depth_ms: 2.0585866, mix: 0.27567205, rate_hz: 0.23690873 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 5, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 80, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.5897145, lowcut: 328.14508, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });
