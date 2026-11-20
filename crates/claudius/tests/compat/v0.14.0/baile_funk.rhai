// Glass Garden — baile_funk (seed 7)
// song("baile_funk", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(127); key("D minor"); swing(0.5051635);
humanize(0.06);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -7.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X..X....X..X....");
drum("kick", "main", "X..X..X...X.X...");
drum("clap", "main", "....X.......X...");
drum("rim", "main", "....x..x...x..x.");
drum_sound("conga", "perc");
drum("conga", "main", "........X.......");
drum_sound("timbal", "perc");
drum("timbal", "main", ".x.......x.x..x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("chords", #{ style: "stabs", preset: "stab", register: [52, 74], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "riff", preset: "soft_lead", register: [62, 79], density: 0.3452221, seed: -4694233468511451402 });

form([
  sec("intro", 8, "chords lead conga").energy(0.59030193).variant("main"),
  sec("hook", 8, "*").energy(0.91373897).impact(),
  sec("verse", 8, "kick clap rim conga timbal bass chords").energy(0.7407866).variant("alt").gap(),
  sec("hook", 8, "*").name("hook2").energy(0.97113043).impact(),
  sec("break", 4, "chords lead conga").energy(0.42666033),
  sec("hook", 8, "*").name("hook3").energy(0.98874223).gap().impact(),
  sec("hook", 8, "*").name("hook4").energy(0.98664886),
  sec("outro", 4, "chords conga rim").energy(0.4076398).variant("main"),
]);

mix("clap", #{ reverb: 0.1 });
mix("rim", #{ pan: 0.25 });
mix("conga", #{ pan: -0.3 });
mix("timbal", #{ pan: 0.3 });
mix("chords", #{ reverb: 0.29302064, delay: 0.122124426 });
mix("lead", #{ reverb: 0.16345617, delay: 0.23290655 });
mix("fx", #{ level: -3, reverb: 0.15 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 8, kind: "hard", mix: 0.9 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("chords", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.6 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.5 },
  #{ fx: "compressor", attack_ms: 5, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 70, threshold_db: -10 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 160, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 100, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "hard", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 1.2948573, lowcut: 328.14508, mix: 1, predelay_ms: 15.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.21008913, mix: 1, pingpong: true } });
