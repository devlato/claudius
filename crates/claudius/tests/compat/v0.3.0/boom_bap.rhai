// Glass Garden — boom_bap (seed 7)
// song("boom_bap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(87); key("D minor"); swing(0.56204814);
humanize(0.45);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X.........X...X.|X......X..X.....|X.........X...X.");
drum("snare", "main", "...oX.......X...");
drum("hat", "main", "x.x.x.x.x.x.x.x.");
drum("ohat", "main", "................|..............x.|................|..............x.");
drum("rim", "main", "...x..x.........");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "walking", preset: "sub_bass", register: [31, 48], density: 0.53341293, seed: -2526400537787473278 });
part("keys", #{ style: "half", preset: "keys", register: [50, 74], density: 0.55524325, seed: 6553130387488681005 });
part("horns", #{ style: "motif", preset: "fm_marimba", register: [60, 82], density: 0.29522207, seed: -4694233468511451402 });
part("pad", #{ style: "sustain", preset: "pwm_pad", register: [52, 76], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 8, "keys hat").energy(0.4903019).variant("main"),
  sec("verse", 8, "kick snare hat rim bass keys").energy(0.61373895),
  sec("hook", 4, "kick snare hat ohat rim bass keys horns").energy(0.84078664),
  sec("verse", 8, "kick snare hat rim bass keys pad").name("verse2").energy(0.6711304).fills(true).gap(),
  sec("hook", 4, "*").name("hook2").energy(0.8766603),
  sec("verse", 8, "kick snare hat rim bass keys").name("verse3").energy(0.6887422).fills(true).gap(),
  sec("hook", 8, "*").name("hook3").energy(0.93664885).gap(),
  sec("hook", 4, "*").name("hook4").energy(1),
  sec("outro", 4, "keys hat pad").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.18 });
mix("ohat", #{ pan: -0.15 });
mix("rim", #{ pan: -0.3, reverb: 0.15 });
mix("keys", #{ reverb: 0.29302064 });
mix("horns", #{ reverb: 0.2043202, delay: 0.15527104 });
mix("pad", #{ reverb: 0.42610887 });
mix("fx", #{ reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [
  #{ fx: "tape", drive_db: 2, flutter: 0.15, lowpass_hz: 11000, wow: 0.2 },
  #{ fx: "width", mono_below_hz: 200, width: 1.3 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("horns", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", []);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.5 },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 100, threshold_db: -12 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 2, flutter: 0.1, lowpass_hz: 17000, wow: 0.1 },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 160, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.2948573, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.21008913, mix: 1, pingpong: false } });
