// Glass Garden — phonk (seed 7)
// song("phonk", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(131); key("D phrygian"); swing(0.50688463);
humanize(0.1);
prog("A", "i II i VII");
prog("B", "i II III II");
bars_per_chord(1);
master(#{ lufs: -8, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X...............|X.........X.....|X...............");
drum("snare", "main", "........X.......");
drum("clap", "main", "........X.......");
drum("hat", "alt", "x...........x...........x...........x.....x.....|x...........x...........o.x.x.x.x.X.x.....x.....|x...........x...........x...........x.....x.....|x...........X.Xx.xx.xx.xx...........x.....x.....", 48);
drum("hat", "main", "x.....x...........o.xx.xx.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....o.x.x.x.x.X.x.....x.....x.....x.....|x.....x.....x.....x.....x.....x...........X.xx.x", 48);
drum("ohat", "main", "..............x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "glide", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("cowbell", #{ style: "phonk_cowbell", preset: "cowbell_lead", register: [62, 82], density: 0.52104914, seed: -6710496186308540014 });

form([
  sec("intro", 8, "cowbell hat").energy(0.59030193).variant("main"),
  sec("hook", 8, "kick snare clap hat ohat bass cowbell pad").energy(0.91373897).impact(),
  sec("verse", 8, "kick snare hat bass cowbell").energy(0.6907866).variant("alt").gap(),
  sec("hook", 8, "kick snare clap hat ohat bass cowbell pad").name("hook2").energy(0.97113043).impact(),
  sec("break", 4, "cowbell pad").energy(0.42666033).gap(),
  sec("hook", 8, "kick snare clap hat ohat bass cowbell pad").name("hook3").energy(0.98874223).gap().impact(),
  sec("hook", 8, "kick snare clap hat ohat bass cowbell pad").name("hook4").energy(0.98664886),
  sec("outro", 4, "cowbell hat").energy(0.4076398).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("clap", #{ level: -3, reverb: 0.12 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("cowbell", #{ reverb: 0.21976548, delay: 0.122124426 });
mix("fx", #{ level: -2, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 9, kind: "tanh", mix: 1 },
  #{ fx: "filter", cutoff: 4500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("cowbell", [#{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.6 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 5, kind: "tanh", mix: 0.6 },
  #{ fx: "compressor", attack_ms: 5, knee_db: 4, makeup_db: 2, mix: 1, ratio: 4, release_ms: 70, threshold_db: -10 },
]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 3, flutter: 0.15, lowpass_hz: 12000, wow: 0.2 },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 2, flutter: 0.08, lowpass_hz: 15000, wow: 0.08 },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.5, release_ms: 100, threshold_db: -9 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.0790477, lowcut: 328.14508, mix: 1, predelay_ms: 15.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.21008913, mix: 1, pingpong: true } });
