// Glass Garden — alt_rnb (seed 7)
// song("alt_rnb", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(62); key("D minor"); swing(0.5103269);
humanize(0.2);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...............|..X.............|X...............|..X.............");
drum("kick", "main", "X.....X...X.....");
lane("kick", #{ late: -2 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 13 });
drum("clap", "main", "....x.......x...");
lane("clap", #{ late: 12 });
drum("hat", "alt", "o.......o.......o.......o.......|o.......o.......o.......o.......|o.......XXxxxxxxo.......o.......|o.......o.......o.......o.......", 32);
drum("hat", "main", "x.o.o.o.x.o.x.oo");
lane("hat", #{ late: -2 });
drum("ohat", "main", "................|..............x.|................|..............x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "sustain", preset: "moog_bass", register: [22, 40], density: 0.5995956, seed: 3796763344308755361, late: 4 });
part("keys", #{ style: "lofi_comp", preset: "organ", register: [50, 76], density: 0.5916404, seed: -20428528665040385 });
part("pad", #{ style: "legato", preset: "air_pad", register: [50, 76], density: 0.4036526, seed: 4480519692905280314 });
part("lead", #{ style: "horn_riff", preset: "pluck", register: [60, 82], density: 0.3241591, seed: 9058081625667575456 });

form([
  sec("intro", 8, "pad keys").energy(0.44030192).variant("main"),
  sec("verse", 4, "kick snare hat bass pad").energy(0.46373898).variant("alt"),
  sec("hook", 4, "*").energy(0.7907866),
  sec("verse", 4, "kick snare hat bass keys").name("verse2").energy(0.52113044).variant("alt").fills(true).gap(),
  sec("hook", 4, "*").name("hook2").energy(0.82666034),
  sec("break", 4, "keys lead").name("bridge").energy(0.3387422),
  sec("hook", 4, "*").name("hook3").energy(0.88664883).gap(),
  sec("hook", 4, "*").name("hook4").energy(1),
  sec("outro", 4, "pad keys").energy(0.2537415).variant("main"),
]);

mix("snare", #{ reverb: 0.4 });
mix("clap", #{ level: -5, reverb: 0.45 });
mix("hat", #{ pan: 0.15, reverb: 0.15 });
mix("ohat", #{ pan: -0.15, reverb: 0.3 });
mix("keys", #{ reverb: 0.6592964 });
mix("pad", #{ level: -2, reverb: 0.5720966 });
mix("lead", #{ level: -3, reverb: 0.6695997, delay: 0.26710713 });
mix("fx", #{ level: -5, reverb: 0.5 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.6 },
  #{ fx: "filter", cutoff: 3500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [
  #{ fx: "tape", drive_db: 1.5, flutter: 0.1, lowpass_hz: 9000, wow: 0.2 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("pad", [
  #{ fx: "chorus", delay_ms: 14, depth_ms: 3, mix: 0.3, rate_hz: 0.3 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 11.170334, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.680783, release_ms: 121.50812, threshold_db: -12.541128 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1, flutter: 0.05, lowpass_hz: 15000, wow: 0.08 },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 3.4529526, lowcut: 234.38934, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.3193355, mix: 1, pingpong: true } });
