// Glass Garden — dnb (seed 7)
// song("dnb", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(170); key("D minor"); swing(0.50172114);
humanize(0.1);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(2);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X......X..X.....");
drum("kick", "main", "X.........X.....");
drum("snare", "main", "o...X.o.....X...");
drum("hat", "main", "xxxxx.x.x.x.x.x.");
drum("ride", "main", "o...o.o.o.o.o.o.");
drum("shaker", "main", "ooo.ooo.ooo.oooo");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "reese", register: [26, 41], density: 0.42864978, seed: -37300132614189792 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 74], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "synth_lead", preset: "epiano", register: [64, 86], density: 0.4863813, seed: -3681901744659677821 });
part("arp", #{ style: "up", preset: "harp_pluck", register: [60, 84], density: 0.47421524, rate: 0.25, seed: 6104030045851553230 });

form([
  sec("intro", 4, "pad hat ride").energy(0.4903019).variant("main"),
  sec("build", 8, "pad hat ride snare lead").energy(0.513739).gap(),
  sec("drop", 16, "kick snare hat ride shaker bass pad lead").energy(0.9407866).fills(false).impact(),
  sec("break", 8, "pad lead arp hat").energy(0.37113047).build(true).gap(),
  sec("drop", 16, "*").name("drop2").energy(0.9766603).impact(),
  sec("drop", 16, "*").name("drop3").energy(0.98874223),
  sec("outro", 8, "pad hat ride").energy(0.33664882).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.15 });
mix("ride", #{ pan: -0.25 });
mix("shaker", #{ pan: 0.35 });
mix("bass", #{ duck: 0.5 });
mix("pad", #{ reverb: 0.5860413, duck: 0.4 });
mix("lead", #{ reverb: 0.24518426, delay: 0.38817757, duck: 0.25 });
mix("arp", #{ reverb: 0.3043635, delay: 0.26710713, duck: 0.3 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("pad", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.5 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 90, threshold_db: -11 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2.5, kind: "tanh", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 3.0213335, lowcut: 328.14508, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "wobble" },
  #{ sound: "wobble", fx: [
  #{ fx: "saturate", drive_db: 24, kind: "tube", mix: 1 },
  #{ fx: "filter", cutoff: 300, drive: 1, mix: 1, mode: "hp", res: 0.1 },
  #{ fx: "filter", cutoff: 3000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "chorus", delay_ms: 14, depth_ms: 4, mix: 0.6, rate_hz: 0.7 },
  #{ fx: "gain", db: -12 },
] },
]);
