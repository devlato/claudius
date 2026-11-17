// Glass Garden — hip_hop (seed 7)
// song("hip_hop", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(85); key("D minor"); swing(0.50688463);
humanize(0.25);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X......X........|X.........X.....|X......X........");
drum("kick", "main", "X......X..X.....|X..X......X.....|X......X..X.....|X..X......X.....");
lane("kick", #{ late: -2 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 4 });
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 7 });
drum("hat", "alt", "x.x.x.x.x.x.x.x.");
drum("hat", "main", "x.....x.....x.....x.....x.....x.....x..x..x.....|x.....x.....XXXxxxxxxxxxx.....x.....x..x..x.....|x.....x.....x.....x.....x.....x.....x..x..x.....|x.....x.....x.....x.....x.....x.....x..x..x.....", 48);
lane("hat", #{ late: -1 });
drum("ohat", "main", "..............x.");
drum("perc", "main", "...x..x.........");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "boom_bap", register: [26, 43], density: 0.53341293, seed: -2526400537787473278 });
part("keys", #{ style: "lofi_comp", preset: "keys", register: [45, 69], density: 0.55524325, seed: 6553130387488681005 });
part("lead", #{ style: "trap_bells", preset: "house_piano", register: [60, 79], density: 0.31182876, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 72], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 8, "keys pad hat").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick snare hat perc bass keys").energy(0.61373895),
  sec("hook", 8, "*").energy(0.8907866).impact(),
  sec("verse", 8, "kick snare hat perc bass keys pad").name("verse2").energy(0.6711304).variant("alt").fills(true).gap(),
  sec("hook", 8, "*").name("hook2").energy(0.9266603).impact(),
  sec("break", 4, "keys pad lead").energy(0.4387422).gap(),
  sec("hook", 4, "*").name("hook3").energy(0.98664886).gap().impact(),
  sec("hook", 4, "*").name("hook4").energy(1),
  sec("outro", 4, "keys pad").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("clap", #{ level: -4, reverb: 0.18 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.3, reverb: 0.15 });
mix("keys", #{ reverb: 0.3223227 });
mix("lead", #{ level: -2, reverb: 0.24518426, delay: 0.38817757 });
mix("pad", #{ level: -3, reverb: 0.54785424 });
mix("fx", #{ level: -2, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.6 },
  #{ fx: "filter", cutoff: 4500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [
  #{ fx: "saturate", drive_db: 3, kind: "tape", mix: 0.5 },
  #{ fx: "filter", cutoff: 6000, drive: 1, mix: 1, mode: "lp", res: 0.05 },
  #{ fx: "width", mono_below_hz: 200, width: 1.2 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", []);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.5 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 90, threshold_db: -12 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -10 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 1.5106667, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass" },
  #{ sound: "sub_bass", fx: [
  #{ fx: "saturate", drive_db: 24, kind: "tape", mix: 1 },
  #{ fx: "filter", cutoff: 300, drive: 1, mix: 1, mode: "hp", res: 0.1 },
  #{ fx: "filter", cutoff: 4500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "chorus", delay_ms: 14, depth_ms: 4, mix: 0.6, rate_hz: 0.7 },
  #{ fx: "gain", db: -6 },
] },
]);
