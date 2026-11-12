// Glass Garden — rnb_90s (seed 7)
// song("rnb_90s", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(68); key("D major"); swing(0.55032694);
humanize(0.25);
prog("A", "bVImaj7 bVII9 Imaj9 Imaj9");
prog("B", "IVmaj9 iii7 vi9 ii9 IVmaj9 iv6 Imaj9 Imaj9");
bars_per_chord(1);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X.....X.........|X......X..X.....|X.....X.........");
lane("kick", #{ late: -2 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 13 });
drum("snap", "main", "....x.......x...");
lane("snap", #{ late: 14 });
drum("hat", "main", "xxx.x...x.xox.x.|xxx.x...x.xox.x.|xxx.x...x.xox.x.|x...x.x.x.x.xox.");
lane("hat", #{ late: 3 });
drum("ohat", "main", "..............x.|................|..............x.|................");
lane("ohat", #{ late: -1 });
drum("shaker", "main", "..o.o.....o.o...");
lane("shaker", #{ late: 2 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "rnb", preset: "deep_bass", register: [28, 46], density: 0.42864978, seed: -37300132614189792, late: 9 });
part("keys", #{ style: "rnb_keys", preset: "epiano", register: [50, 76], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "solina", register: [50, 76], density: 0.56008077, seed: -1629742732023437997 });
part("lead", #{ style: "rnb_runs", preset: "fm_lead", register: [60, 82], density: 0.25655335, seed: 6104030045851553230 });

form([
  sec("intro", 8, "keys pad").energy(0.4903019).variant("main"),
  sec("verse", 8, "kick snare snap hat shaker bass keys").energy(0.563739),
  sec("hook", 4, "*").energy(0.84078664),
  sec("verse", 4, "kick snare snap hat shaker bass keys pad").name("verse2").energy(0.6211304).fills(true).gap(),
  sec("hook", 4, "*").name("hook2").energy(0.8766603),
  sec("break", 4, "snap bass keys pad lead").name("bridge").energy(0.4887422).gap(),
  sec("hook", 4, "*").name("hook3").energy(0.93664885).gap(),
  sec("hook", 4, "*").name("hook4").energy(1),
  sec("outro", 4, "keys pad lead").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.22 });
mix("snap", #{ level: -4, pan: 0.2, reverb: 0.3 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("shaker", #{ level: -4, pan: -0.35 });
mix("keys", #{ reverb: 0.43953097 });
mix("pad", #{ level: -3, reverb: 0.36777636 });
mix("lead", #{ level: -2, reverb: 0.42610887, delay: 0.17807142 });
mix("fx", #{ reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 10, depth_ms: 2.5, mix: 0.35, rate_hz: 0.6 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 110, threshold_db: -13 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 100, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1, flutter: 0.05, lowpass_hz: 18000, wow: 0.05 },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 160, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.0501904, lowcut: 234.38934, mix: 1, predelay_ms: 25.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });
