// Glass Garden — hyperpop (seed 7)
// song("hyperpop", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(142); key("D major"); swing(0.5034423);
humanize(0.03);
prog("A", "I bVII IV I");
prog("B", "IV I V vi");
bars_per_chord(1);
master(#{ lufs: -7, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X...............|X.........X.....|X...............");
drum("kick", "main", "X.........X.....|X..X......X..X..|X.........X.....|X..X......X..X..");
drum("snare", "main", "........X.......");
drum("hat", "alt", "x.....x.....x.....x.....x.....x.....X.X.x.x.x.x.|o.xx.xx.xx.Xx.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....", 48);
drum("hat", "main", "x.....x.....x.....x...........o.xx.x......o.xx.x|x.....x.....x.....x.....x.....x...........o.xx.x|x.....x.....ooxxxxxxxxXXx.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....o.xx.xx.xx.X", 48);
drum("ohat", "main", "..............x.");
drum("perc", "main", "x..x..x..x...x..");
drum("perc", "sparse", ".....x.......x..");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "glide", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("chords", #{ style: "pump", preset: "pwm_pad", register: [55, 79], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "trap_bells", preset: "square_lead", register: [69, 88], density: 0.58638126, seed: -3681901744659677821 });
part("arp", #{ style: "up", preset: "chip", register: [60, 86], density: 0.49242017, rate: 0.25, seed: 805921219908132358 });

form([
  sec("intro", 8, "lead arp").energy(0.59030193).variant("main"),
  sec("verse", 8, "kick snare hat bass chords lead").energy(0.66373897).variant("alt"),
  sec("hook", 8, "*").energy(0.9407866).impact(),
  sec("verse", 8, "kick snare hat bass chords lead").name("verse2").energy(0.6711304).prog("B").variant("alt").fills(true).gap(),
  sec("hook", 8, "*").name("hook2").energy(0.9766603).impact(),
  sec("break", 4, "arp pad chords").name("switch").energy(0.4887422),
  sec("hook", 4, "kick snare hat ohat perc bass chords lead arp pad").name("hook3").energy(0.98664886).gap().impact(),
  sec("hook", 4, "kick snare hat ohat perc bass chords lead arp pad").name("hook4").energy(1),
  sec("outro", 8, "lead arp").energy(0.3537415).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: 0.3, delay: 0.2 });
mix("chords", #{ reverb: 0.43953097, duck: 0.5 });
mix("lead", #{ level: 1, reverb: 0.2043202, delay: 0.31054208 });
mix("arp", #{ level: -4, reverb: 0.3043635, delay: 0.17807142 });
mix("fx", #{ level: -3, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("kick", [#{ fx: "saturate", drive_db: 6, kind: "hard", mix: 0.5 }]);
fx("bass", [
  #{ fx: "saturate", drive_db: 12, kind: "hard", mix: 1 },
  #{ fx: "filter", cutoff: 6000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.6 },
  #{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.3 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("lead", [
  #{ fx: "bitcrush", bits: 10, downsample: 2, mix: 0.35 },
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.3 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 5, kind: "tanh", mix: 0.7 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.5 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 2, mix: 1, ratio: 2.5, release_ms: 80, threshold_db: -9 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 9000, gain_db: 1.5, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 100, threshold_db: -8 },
  #{ fx: "saturate", drive_db: 2.5, kind: "hard", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.31764442, decay: 1.7264763, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.45, feedback: 0.25210696, mix: 1, pingpong: true } });
