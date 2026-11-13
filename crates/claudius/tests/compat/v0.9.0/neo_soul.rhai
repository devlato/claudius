// Glass Garden — neo_soul (seed 7)
// song("neo_soul", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(73); key("D major"); swing(0.56204814);
humanize(0.55);
prog("A", "vi7 ii7 V7 Imaj7");
prog("B", "Imaj7 vi7 ii7 V7");
bars_per_chord(1);
master(#{ lufs: -11, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....");
lane("kick", #{ late: -8 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 19 });
drum("hat", "main", "x.x.x.x.x.x.x..x");
lane("hat", #{ late: 1 });
drum("rim", "main", "..............o.");
lane("rim", #{ late: 22 });
drum("ride", "main", "..o.o.o.o...o...");
lane("ride", #{ late: 3 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "neo_soul", register: [28, 48], density: 0.53341293, seed: -2526400537787473278, late: 25 });
part("keys", #{ style: "gospel", preset: "epiano", register: [50, 76], density: 0.55524325, seed: 6553130387488681005 });
part("lead", #{ style: "rnb_runs", preset: "pluck", register: [62, 84], density: 0.29522207, seed: -4694233468511451402 });
part("strings", #{ style: "legato", preset: "strings", register: [52, 79], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 8, "keys").energy(0.44030192).variant("main"),
  sec("verse", 8, "kick snare hat rim bass keys").energy(0.563739),
  sec("hook", 4, "kick snare hat ride bass keys lead strings").energy(0.7907866),
  sec("verse", 8, "kick snare hat rim bass keys strings").name("verse2").energy(0.6211304).fills(true).gap(),
  sec("hook", 4, "kick snare hat ride bass keys lead strings").name("hook2").energy(0.82666034),
  sec("break", 4, "keys").name("bridge").energy(0.2887422).gap(),
  sec("hook", 4, "kick snare hat ride bass keys lead strings").name("hook3").energy(0.88664883).gap(),
  sec("hook", 4, "kick snare hat ride bass keys lead strings").name("hook4").energy(1),
  sec("outro", 4, "keys strings bass").energy(0.2537415).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.2 });
mix("rim", #{ pan: -0.15, reverb: 0.15 });
mix("ride", #{ pan: -0.25, reverb: 0.1 });
mix("keys", #{ reverb: 0.3662758 });
mix("lead", #{ reverb: 0.2860483, delay: 0.23290655 });
mix("strings", #{ reverb: 0.54785424 });
mix("fx", #{ level: -4, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [#{ fx: "chorus", delay_ms: 12, depth_ms: 2.5, mix: 0.3, rate_hz: 0.4 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("strings", [#{ fx: "width", mono_below_hz: 200, width: 1.4 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 2, mix: 1, ratio: 2.5, release_ms: 120, threshold_db: -14 },
  #{ fx: "saturate", drive_db: 3, kind: "tape", mix: 0.4 },
  #{ fx: "width", mono_below_hz: 120, width: 1 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1.5, flutter: 0.1, lowpass_hz: 18000, wow: 0.1 },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -12 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.9422858, lowcut: 234.38934, mix: 1, predelay_ms: 20.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", fold: [28, 40] },
]);
