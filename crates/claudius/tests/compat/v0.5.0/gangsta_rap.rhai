// Glass Garden — gangsta_rap (seed 7)
// song("gangsta_rap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(85); key("D minor"); swing(0.53860575);
humanize(0.35);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X.........X.....|X......X..X.....|X.........X.....");
lane("kick", #{ late: -1 });
drum("snare", "main", "...oX.......X...");
lane("snare", #{ late: 14 });
drum("hat", "main", "x.x.x.x.x.x.x.x.");
drum("ohat", "main", "................|..............x.|................|..............x.");
lane("ohat", #{ late: -2 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "gfunk", preset: "sub_bass", register: [28, 43], density: 0.53341293, seed: -2526400537787473278, late: 8 });
part("keys", #{ style: "trap_keys", preset: "keys", register: [45, 70], density: 0.55524325, seed: 6553130387488681005 });
part("lead", #{ style: "gfunk_whine", preset: "house_piano", register: [72, 88], density: 0.21691826, seed: -3681901744659677821 });
part("strings", #{ style: "ostinato", preset: "strings", register: [40, 64], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 8, "strings keys").energy(0.4903019).variant("main"),
  sec("verse", 8, "bass kick snare hat keys").energy(0.61373895),
  sec("hook", 4, "ohat bass strings lead kick snare hat keys").energy(0.84078664),
  sec("verse", 8, "bass strings kick snare hat keys").name("verse2").energy(0.6711304).fills(true).gap(),
  sec("hook", 4, "ohat bass strings lead kick snare hat keys").name("hook2").energy(0.8766603),
  sec("verse", 8, "bass kick snare hat keys").name("verse3").energy(0.6887422).fills(true).gap(),
  sec("hook", 8, "ohat bass strings lead kick snare hat keys").name("hook3").energy(0.93664885).gap(),
  sec("hook", 4, "ohat bass strings lead kick snare hat keys").name("hook4").energy(1),
  sec("outro", 4, "strings keys").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("keys", #{ reverb: 0.29302064 });
mix("lead", #{ level: -5, reverb: 0.32691234, delay: 0.23290655 });
mix("strings", #{ level: -4, reverb: 0.3652362 });
mix("fx", #{ reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [
  #{ fx: "tape", drive_db: 3, flutter: 0.15, lowpass_hz: 9000, wow: 0.25 },
  #{ fx: "bitcrush", bits: 12, downsample: 2, mix: 1 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("strings", []);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 5, kind: "tube", mix: 0.5 },
  #{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3.5, release_ms: 100, threshold_db: -12 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 2, flutter: 0.08, lowpass_hz: 16000, wow: 0.08 },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 150, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.56764436, decay: 1.4027619, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.65, feedback: 0.21008913, mix: 1, pingpong: false } });
