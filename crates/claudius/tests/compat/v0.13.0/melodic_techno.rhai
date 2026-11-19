// Glass Garden — melodic_techno (seed 7)
// song("melodic_techno", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(120); key("D minor"); swing(0.5034423);
humanize(0.05);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "x.xx.xx...x...xx");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "......x.......o.");
drum("shaker", "main", "o.o..o....oooooo");
drum("perc", "main", "x..x..x...x..x..");
drum("perc", "sparse", "..x.......x.....");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "rolling", register: [30, 45], density: 0.53341293, seed: -2526400537787473278 });
part("arp", #{ style: "updown", preset: "pluck", register: [57, 81], density: 0.55524325, rate: 0.25, seed: -6710496186308540014 });
part("lead", #{ style: "ambient_line", preset: "pwm_lead", register: [64, 81], density: 0.399105, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "air_pad", register: [45, 74], density: 0.5752324, seed: 8524255199291047346 });

form([
  sec("intro", 4, "kick hat shaker bass").energy(0.5403019),
  sec("groove", 16, "kick clap hat shaker perc bass arp").energy(0.66373897).fills(false),
  sec("break", 16, "pad arp lead").energy(0.3407866),
  sec("build", 4, "clap hat shaker pad arp lead").energy(0.6211304).prog("B").gap(),
  sec("drop", 16, "*").energy(0.9766603).impact(),
  sec("drop", 16, "*").name("drop2").energy(0.98874223),
  sec("outro", 8, "kick hat shaker arp").energy(0.38664883),
]);

mix("clap", #{ reverb: 0.35 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, reverb: 0.2, delay: 0.3 });
mix("bass", #{ duck: 0.7 });
mix("arp", #{ reverb: 0.5860413, delay: 0.42743546, duck: 0.3 });
mix("lead", #{ reverb: 0.36777636, delay: 0.4658131, duck: 0.3 });
mix("pad", #{ reverb: 0.6695997, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.4 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "filter", cutoff: sections(900, #{ build: [900, 1800], drop: 1800 }), drive: 1, mix: 1, mode: "lp", res: 0.2 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("arp", [
  #{ fx: "filter", cutoff: sections(1800, #{ build: [1800, 6000], drop: 6000 }), drive: 1, mix: 1, mode: "lp", res: 0.3 },
  #{ fx: "auto_pan", beats: 4, depth: 0.32336643 },
]);
fx("lead", [
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "tape", drive_db: 2.2879617, flutter: 0.094686925, lowpass_hz: 9513.441, wow: 0.11845437 },
]);
fx("pad", [#{ fx: "width", mono_below_hz: 200, width: 1.5 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 100, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 150, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 4.3161907, lowcut: 328.14508, mix: 1, predelay_ms: 35.852707, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.37816042, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", fold: [28, 40] },
]);
