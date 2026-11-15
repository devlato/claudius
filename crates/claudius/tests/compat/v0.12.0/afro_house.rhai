// Glass Garden — afro_house (seed 7)
// song("afro_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(120); key("D minor"); swing(0.5268846);
humanize(0.2);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(2);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 3 });
drum("hat", "main", "..x...xx..x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("shaker", "main", "o.o.ooo..oooo.o.");
lane("shaker", #{ late: -1 });
drum_sound("conga", "perc");
drum("conga", "main", "..x.x.......x...");
lane("conga", #{ late: 3 });
drum("tom", "main", "x.....x...x.....");
drum("tom", "sparse", "..........x.....");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "deep", register: [31, 48], density: 0.5995956, seed: 3796763344308755361 });
part("chords", #{ style: "highlife", preset: "fm_marimba", register: [55, 76], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "afro_flute", preset: "fm_kalimba", register: [64, 81], density: 0.36008075, seed: -1629742732023437997 });
part("pad", #{ style: "sustain", preset: "glass_pad", register: [48, 74], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 4, "kick shaker conga tom").energy(0.5403019),
  sec("groove", 16, "kick clap hat shaker conga tom bass chords").energy(0.66373897).fills(false),
  sec("break", 8, "pad chords lead shaker").energy(0.3407866),
  sec("build", 4, "clap hat shaker conga chords lead pad").energy(0.57113045).prog("B"),
  sec("drop", 16, "*").energy(0.9266603).impact(),
  sec("drop", 16, "*").name("drop2").energy(0.98874223),
  sec("outro", 8, "kick shaker conga tom bass").energy(0.38664883),
]);

mix("clap", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.18 });
mix("shaker", #{ pan: -0.3 });
mix("conga", #{ pan: -0.35, reverb: 0.15 });
mix("tom", #{ pan: 0.3, reverb: 0.2 });
mix("bass", #{ duck: 0.45 });
mix("chords", #{ reverb: 0.43953097, delay: 0.24424885, duck: 0.35 });
mix("lead", #{ reverb: 0.32691234, delay: 0.4658131 });
mix("pad", #{ reverb: 0.54785424, duck: 0.35 });
mix("fx", #{ level: -4, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", [#{ fx: "width", mono_below_hz: 200, width: 1.4 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.35 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 110, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -11 },
  #{ fx: "saturate", drive_db: 2, kind: "tape", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.8055239, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.29412478, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", band: [0, 90], level: -3, duck: 0.225, fold: [28, 40] },
  #{ sound: "organ_bass", band: [90, 0] },
]);
