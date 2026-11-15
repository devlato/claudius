// Glass Garden — jazz_rap (seed 7)
// song("jazz_rap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(87); key("D major"); swing(0.5703269);
humanize(0.4);
prog("A", "vi7 ii7 V7 Imaj7");
prog("B", "Imaj7 vi7 ii7 V7");
bars_per_chord(1);
master(#{ lufs: -11, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X.........X.....|X......X..X.....|X.........X.....");
lane("kick", #{ late: -1 });
drum("snare", "main", "...oX.......X...");
lane("snare", #{ late: 20 });
drum("hat", "main", "x.x.x.x.x.x.x.x.");
lane("hat", #{ late: 1 });
drum("ohat", "main", "................|..............x.|................|..............x.");
lane("ohat", #{ late: -1 });
drum("rim", "main", "...x..x.........");
lane("rim", #{ late: 9 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "walking", preset: "deep_bass", register: [31, 50], density: 0.42864978, seed: -37300132614189792, late: 13 });
part("keys", #{ style: "lofi_comp", preset: "epiano", register: [50, 74], density: 0.5272447, seed: 5396775171407160694 });
part("horns", #{ style: "horn_riff", preset: "whistle", register: [60, 82], density: 0.38674772, seed: -7892568474930094457 });

form([
  sec("intro", 8, "bass keys").energy(0.4903019).variant("main"),
  sec("verse", 8, "rim bass kick snare hat keys").energy(0.61373895),
  sec("hook", 4, "ohat rim bass horns kick snare hat keys").energy(0.84078664),
  sec("verse", 8, "rim bass vibes kick snare hat keys").name("verse2").energy(0.6711304).fills(true).gap(),
  sec("hook", 4, "ohat rim bass horns vibes kick snare hat keys").name("hook2").energy(0.8766603),
  sec("verse", 8, "rim bass kick snare hat keys").name("verse3").energy(0.6887422).fills(true).gap(),
  sec("hook", 8, "ohat rim bass horns vibes kick snare hat keys").name("hook3").energy(0.93664885).gap(),
  sec("hook", 4, "ohat rim bass horns vibes kick snare hat keys").name("hook4").energy(1),
  sec("outro", 4, "bass keys").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.18 });
mix("ohat", #{ pan: -0.15 });
mix("rim", #{ pan: -0.3, reverb: 0.15 });
mix("keys", #{ reverb: 0.29302064 });
mix("horns", #{ level: -2, reverb: 0.2043202, delay: 0.15527104 });
mix("fx", #{ reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [
  #{ fx: "tape", drive_db: 2, flutter: 0.15, lowpass_hz: 11000, wow: 0.2 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("horns", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.45 },
  #{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 2, mix: 1, ratio: 2.8, release_ms: 110, threshold_db: -13 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 2, flutter: 0.1, lowpass_hz: 17000, wow: 0.1 },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 160, threshold_db: -12 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.4027619, lowcut: 281.2672, mix: 1, predelay_ms: 17.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.21008913, mix: 1, pingpong: false } });
