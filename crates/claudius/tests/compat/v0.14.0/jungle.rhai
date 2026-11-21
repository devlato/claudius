// Glass Garden — jungle (seed 7)
// song("jungle", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(161); key("D minor"); swing(0.5051635);
humanize(0.2);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(2);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.X.......X.....");
drum("snare", "main", "..o.X..X.X..Xo.X|....X..X...oX.X.|..o.X..X.X..Xo.X|....X..X...oX.X.");
lane("snare", #{ late: 3 });
drum("hat", "main", "x.x.x.x.x.x.x.x.");
drum("ride", "main", "o.o.o...o.o.o.o.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "jungle_dub", register: [22, 38], density: 0.5726087, seed: 2642474102756894640 });
part("chords", #{ style: "offbeat", preset: "organ", register: [53, 72], density: 0.45851198, seed: 6674420348178494801 });
part("pad", #{ style: "sustain", preset: "strings", register: [51, 77], density: 0.5823303, seed: -7892568474930094457 });

form([
  sec("intro", 4, "pad ride chords").energy(0.4903019).variant("main"),
  sec("build", 4, "chords hat ride snare").energy(0.46373898).gap(),
  sec("drop", 16, "*").energy(0.9407866).fills(false).impact(),
  sec("break", 8, "pad lead ride").energy(0.32113045).build(true).gap(),
  sec("drop", 16, "kick snare hat ride bass chords pad lead").name("drop2").energy(0.9766603).impact(),
  sec("drop", 16, "kick snare hat ride bass chords pad lead").name("drop3").energy(0.98874223),
  sec("outro", 8, "pad ride chords").energy(0.33664882).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.2 });
mix("ride", #{ pan: -0.25, reverb: 0.15 });
mix("bass", #{ duck: 0.45 });
mix("chords", #{ reverb: 0.5127861, delay: 0.42743546, duck: 0.3 });
mix("pad", #{ reverb: 0.36777636, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 },
  #{ fx: "compressor", attack_ms: 5.585167, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3.655613, release_ms: 77.32335, threshold_db: -11.541128 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -9 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 2.5897145, lowcut: 328.14508, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.33614263, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "808" },
  #{ sound: "808", fx: [
  #{ fx: "saturate", drive_db: 24, kind: "tube", mix: 1 },
  #{ fx: "filter", cutoff: 300, drive: 1, mix: 1, mode: "hp", res: 0.1 },
  #{ fx: "filter", cutoff: 4500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "chorus", delay_ms: 14, depth_ms: 4, mix: 0.6, rate_hz: 0.7 },
  #{ fx: "gain", db: -6 },
] },
]);
