// Glass Garden — uk_garage (seed 7)
// song("uk_garage", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(130); key("D minor"); swing(0.59376925);
humanize(0.25);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X......X...X....|X.........X.....|X......X...X....");
drum("snare", "main", "....X.....o.X...");
lane("snare", #{ late: 4 });
drum("hat", "main", "..xx..xx..xx..xx");
lane("hat", #{ late: -3 });
drum("ohat", "main", "..x.......x.....");
drum("shaker", "main", "..o..o..o..o..oo");
lane("shaker", #{ late: -1 });
drum("rim", "main", "...x.......x....");
lane("rim", #{ late: 4 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "garage", preset: "deep_bass", register: [28, 46], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "house_piano", preset: "epiano", register: [55, 76], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "organ_riff", preset: "vox_lead", register: [64, 86], density: 0.52791286, seed: -7892568474930094457 });

form([
  sec("intro", 8, "kick hat shaker chords").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick snare hat shaker rim bass chords").energy(0.66373897),
  sec("hook", 8, "kick snare hat ohat shaker rim bass chords lead pad").energy(0.8907866).impact(),
  sec("break", 4, "pad lead hat").energy(0.32113045).build(true),
  sec("verse", 8, "kick snare hat shaker rim bass chords pad").name("verse2").energy(0.7266603).fills(true).gap(),
  sec("hook", 8, "kick snare hat ohat shaker rim bass chords lead pad").name("hook2").energy(0.9387422).gap().impact(),
  sec("hook", 8, "kick snare hat ohat shaker rim bass chords lead pad").name("hook3").energy(0.98664886),
  sec("outro", 4, "kick hat shaker chords").energy(0.4076398).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.18 });
mix("ohat", #{ pan: -0.12 });
mix("shaker", #{ pan: -0.3 });
mix("rim", #{ pan: 0.3, reverb: 0.15 });
mix("bass", #{ duck: 0.4 });
mix("chords", #{ reverb: 0.29302064, delay: 0.1465493, duck: 0.35 });
mix("lead", #{ reverb: 0.2043202, delay: 0.38817757, duck: 0.25 });
mix("fx", #{ level: -3, reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 90, threshold_db: -11 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 130, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 150, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1, kind: "tanh", mix: 0.2 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 1.7264763, lowcut: 281.2672, mix: 1, predelay_ms: 17.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.25210696, mix: 1, pingpong: true } });
